// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    sync::Arc,
};

use compositor::GstBinErrorExt;
use gst::{prelude::ObjectExt, Promise};
use gst_sdp::SDPMessage;
use gst_webrtc::{WebRTCSDPType, WebRTCSessionDescription};
use opentalk_recorder::signaling::{incoming, outgoing};
use tokio::sync::{mpsc, Mutex};
use types::{
    common::event::EventInfo,
    core::{EventId, ParticipantId, StreamingTargetId, TariffId, Timestamp},
    signaling::{
        control::{
            event::{ControlEvent, JoinSuccess},
            state::ControlState,
            AssociatedParticipant, Participant,
        },
        media::{peer_state::MediaPeerState, MediaSessionState, ParticipantMediaState},
        recording::{
            peer_state::RecordingPeerState,
            state::{RecorderStreamInfo, RecordingTarget, StreamStartOption},
            StreamStatus,
        },
        recording_service::{command::RecordingServiceCommand, state::RecordingServiceState},
        ModuleData, ModulePeerData,
    },
};

use super::{webrtc::create_pipeline, User};

#[derive(Clone)]
pub(crate) struct MockController {
    users: Arc<Mutex<BTreeMap<usize, User>>>,
    to_recorder_tx: mpsc::Sender<incoming::Message>,
}

impl MockController {
    pub(crate) fn run(
        users: Arc<Mutex<BTreeMap<usize, User>>>,
        to_recorder_tx: mpsc::Sender<incoming::Message>,
        mut to_controller_rx: mpsc::Receiver<outgoing::Message>,
    ) -> Self {
        log::info!("Start mocked controller");
        let mock_controller = Self {
            users,
            to_recorder_tx,
        };
        tokio::spawn({
            let mock_controller = mock_controller.clone();
            async move {
                while let Some(message) = to_controller_rx.recv().await {
                    match message {
                        outgoing::Message::Control(outgoing::ControlMessage::Join(_)) => {
                            mock_controller.on_join().await
                        }
                        outgoing::Message::Media(outgoing::MediaMessage::Subscribe(target)) => {
                            mock_controller.on_sdp_subscribe(target).await
                        }
                        outgoing::Message::Media(outgoing::MediaMessage::SdpAnswer(sdp)) => {
                            mock_controller.on_sdp_answer(sdp).await
                        }
                        outgoing::Message::Media(outgoing::MediaMessage::SdpCandidate(
                            sdp_candidate,
                        )) => mock_controller.on_sdp_candidate(sdp_candidate).await,
                        outgoing::Message::Media(outgoing::MediaMessage::SdpEndOfCandidates(
                            target,
                        )) => mock_controller.on_sdp_end_of_candidates(target).await,
                        // I can't think of any meaningful test, considering
                        // that the Recorder <-> Controller communication for streaming
                        // is mostly triggered by the frontend, and not a *true* event <-> action.
                        outgoing::Message::RecordingService(_) => {}
                    }
                }
            }
        });
        mock_controller
    }

    async fn on_join(&self) {
        let users = self.users.lock().await;
        let participants: Vec<Participant> = users
            .values()
            .map(|user| user.participant.clone())
            .collect::<Vec<_>>();

        let mut module_data = ModuleData::new();
        let _ = module_data.insert(&RecordingServiceState {
            streams: BTreeMap::new(),
        });

        let join_success = incoming::Message::Control(ControlEvent::JoinSuccess(JoinSuccess {
            id: ParticipantId::generate(),
            participants,
            event_info: Some(EventInfo {
                title: "Test Recording Title".to_string(),
                id: EventId::generate(),
                is_adhoc: false,
            }),
            display_name: "".to_string(),
            avatar_url: None,
            role: types::signaling::Role::User,
            closes_at: None,
            tariff: Box::new(types::common::tariff::TariffResource {
                id: TariffId::nil(),
                name: "".to_owned(),
                quotas: HashMap::new(),
                enabled_modules: HashSet::new(),
                disabled_features: HashSet::new(),
                modules: HashMap::new(),
            }),
            module_data,
            is_room_owner: false,
        }));

        self.to_recorder_tx
            .send(join_success)
            .await
            .expect("unable to send join success event to recorder");
    }

    async fn on_sdp_subscribe(&self, target: outgoing::Target) {
        let mut users = self.users.lock().await;

        let user = users
            .values_mut()
            .find(|user| user.participant.id == target.target)
            .expect("unable to find user for sdp subscribe");

        let pipeline = tokio::task::spawn_blocking({
            let id = user.participant.id;
            let media_session_type = target.media_session_type;
            let to_recorder_tx = self.to_recorder_tx.clone();

            move || create_pipeline(id, media_session_type, to_recorder_tx)
        })
        .await
        .expect("unable to create webrtc pipeline");
        user.webrtc_pipeline = Some(pipeline);
    }

    async fn on_sdp_answer(&self, sdp: outgoing::Sdp) {
        let mut users = self.users.lock().await;

        let user = users
            .values_mut()
            .find(|user| user.participant.id == sdp.target.target)
            .expect("unable to find user for sdp answer");

        if let Some(ref webrtc_pipeline) = user.webrtc_pipeline {
            let webrtc = webrtc_pipeline.by_name_with_context("webrtc").unwrap();
            let answer =
                SDPMessage::parse_buffer(sdp.sdp.as_bytes()).expect("unable to parse sdp message");
            let answer = WebRTCSessionDescription::new(WebRTCSDPType::Answer, answer);

            webrtc.emit_by_name::<()>("set-remote-description", &[&answer, &None::<Promise>]);
        }
    }

    async fn on_sdp_candidate(&self, sdp_candidate: outgoing::SdpCandidate) {
        let mut users = self.users.lock().await;

        let user = users
            .values_mut()
            .find(|user| user.participant.id == sdp_candidate.target.target)
            .expect("unable to find user for sdp candidate");

        if let Some(ref webrtc_pipeline) = user.webrtc_pipeline {
            let webrtc = webrtc_pipeline.by_name_with_context("webrtc").unwrap();

            webrtc.emit_by_name::<()>(
                "add-ice-candidate",
                &[
                    &(sdp_candidate.candidate.sdp_m_line_index as u32),
                    &sdp_candidate.candidate.candidate,
                ],
            );
        }
    }

    async fn on_sdp_end_of_candidates(&self, target: outgoing::Target) {
        let mut users = self.users.lock().await;

        let user = users
            .values_mut()
            .find(|user| user.participant.id == target.target)
            .expect("unable to find user for sdp end of candidated");

        if let Some(ref webrtc_pipeline) = user.webrtc_pipeline {
            webrtc_pipeline
                .by_name_with_context("webrtc")
                .unwrap()
                .emit_by_name::<()>("add-ice-candidate", &[&0u32, &None::<String>]);
        }
    }

    pub(crate) async fn send_join_success(&self) {
        let users = self.users.lock().await;
        let participants: Vec<Participant> = users
            .values()
            .map(|user| user.participant.clone())
            .collect::<Vec<_>>();

        let mut module_data = ModuleData::new();
        module_data
            .insert(&RecordingServiceState {
                streams: BTreeMap::from([(
                    StreamingTargetId::from_u128(0),
                    RecorderStreamInfo::Recording(RecordingTarget {
                        stream_start_options: StreamStartOption {
                            auto_connect: true,
                            status: StreamStatus::Inactive,
                            start_paused: false,
                        },
                    }),
                )]),
            })
            .unwrap();
        self.to_recorder_tx
            .send(incoming::Message::Control(ControlEvent::JoinSuccess(
                JoinSuccess {
                    id: ParticipantId::generate(),
                    participants,
                    event_info: Some(EventInfo {
                        title: "Test Recording Title".to_string(),
                        id: EventId::generate(),
                        is_adhoc: false,
                    }),
                    display_name: "".to_string(),
                    avatar_url: None,
                    role: types::signaling::Role::User,
                    closes_at: None,
                    tariff: Box::new(types::common::tariff::TariffResource {
                        id: TariffId::nil(),
                        name: "".to_owned(),
                        quotas: HashMap::new(),
                        enabled_modules: HashSet::new(),
                        disabled_features: HashSet::new(),
                        modules: HashMap::new(),
                    }),
                    module_data,
                    is_room_owner: false,
                },
            )))
            .await
            .expect("unable to send join success event to recorder");
    }

    pub(crate) async fn send_start_stream(&self) {
        self.to_recorder_tx
            .send(incoming::Message::RecordingService(
                RecordingServiceCommand::StartStreams {
                    target_ids: BTreeSet::from([StreamingTargetId::from_u128(0)]),
                },
            ))
            .await
            .expect("unable to send start recording event to recorder");
    }

    pub(crate) async fn send_joined(&mut self, index: usize) -> Participant {
        let mut participant = Participant {
            id: ParticipantId::generate(),
            module_data: ModulePeerData::new(),
        };
        let _ = participant.module_data.insert(&ControlState {
            display_name: format!("MockUser {index}"),
            role: types::signaling::Role::User,
            avatar_url: None,
            participation_kind: types::core::ParticipationKind::User,
            hand_is_up: false,
            joined_at: Timestamp::now(),
            left_at: None,
            hand_updated_at: Timestamp::now(),
            is_room_owner: true,
        });

        let _ = participant.module_data.insert(&MediaPeerState {
            state: ParticipantMediaState {
                video: None,
                screen: None,
            },
            is_presenter: false,
        });

        let _ = participant.module_data.insert(&RecordingPeerState {
            consents_recording: false,
        });

        let join_event = ControlEvent::Joined(participant.clone());

        self.to_recorder_tx
            .send(incoming::Message::Control(join_event))
            .await
            .expect("unable to send joined event to recorder");

        participant
    }

    pub(crate) async fn send_left(&mut self, participant: &Participant) {
        let left_event = ControlEvent::Left(AssociatedParticipant { id: participant.id });

        self.to_recorder_tx
            .send(incoming::Message::Control(left_event))
            .await
            .expect("unable to send left event to recorder");
    }

    pub(crate) async fn send_update_media(
        &mut self,
        participant: &mut Participant,
        audio: bool,
        video: bool,
        screen: bool,
    ) {
        participant
            .update_module::<MediaPeerState, _>(|update| {
                if video || audio {
                    update.state.video = Some(MediaSessionState { video, audio })
                };

                if screen {
                    update.state.screen = Some(MediaSessionState {
                        video: true,
                        audio: false,
                    })
                }
            })
            .ok()
            .flatten()
            .unwrap();

        self.to_recorder_tx
            .send(incoming::Message::Control(ControlEvent::Update(
                participant.clone(),
            )))
            .await
            .expect("unable to send update event to recorder");
    }

    pub(crate) async fn send_update_consent(
        &mut self,
        participant: &mut Participant,
        consent: bool,
    ) {
        participant
            .update_module::<RecordingPeerState, _>(|update| update.consents_recording = consent)
            .ok()
            .flatten()
            .unwrap();

        self.to_recorder_tx
            .send(incoming::Message::Control(ControlEvent::Update(
                participant.clone(),
            )))
            .await
            .expect("unable to send update event to recorder");
    }

    pub(crate) async fn send_update_focus(&mut self, participant: Option<&Participant>) {
        self.to_recorder_tx
            .send(incoming::Message::Media(
                incoming::MediaMessage::FocusUpdate(incoming::FocusUpdate {
                    focus: participant.map(|participant| participant.id),
                }),
            ))
            .await
            .expect("unable to send update event to recorder");
    }
}
