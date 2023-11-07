// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use std::{collections::BTreeMap, sync::Arc};

use gst::{prelude::ObjectExt, traits::GstBinExt, Promise};
use gst_sdp::SDPMessage;
use gst_webrtc::{WebRTCSDPType, WebRTCSessionDescription};
use opentalk_recorder::signaling::{
    incoming::{self, EventInfo},
    outgoing, ParticipantId,
};
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

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
                    }
                }
            }
        });
        mock_controller
    }

    async fn on_join(&self) {
        let users = self.users.lock().await;
        let participants: Vec<incoming::Participant> = users
            .values()
            .map(|user| user.participant.clone())
            .collect::<Vec<_>>();
        self.to_recorder_tx
            .send(incoming::Message::Control(
                incoming::ControlMessage::JoinSuccess(incoming::JoinSuccess {
                    id: ParticipantId(Uuid::new_v4()),
                    participants,
                    event_info: EventInfo {
                        title: "Test Recording Title".to_string(),
                    },
                }),
            ))
            .await
            .expect("unable to send join success event to recorder");
    }

    async fn on_sdp_subscribe(&self, target: outgoing::Target) {
        let mut users = self.users.lock().await;

        let user = users
            .values_mut()
            .find(|user| user.participant.id.0 == target.target.0)
            .expect("unable to find user for sdp subscribe");

        let pipeline = tokio::task::spawn_blocking({
            let id = user.participant.id.0;
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
            let webrtc = webrtc_pipeline
                .by_name("webrtc")
                .expect("unable to find webrtc pipeline for sdp answer");
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
            let webrtc = webrtc_pipeline
                .by_name("webrtc")
                .expect("unable to find webrtc pipeline for sdp candidate");

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
            let webrtc = webrtc_pipeline
                .by_name("webrtc")
                .expect("unable to find webrtc pipeline for sdp end of candidates");

            webrtc.emit_by_name::<()>("add-ice-candidate", &[&0u32, &None::<String>]);
        }
    }

    pub(crate) async fn send_join_success(&self) {
        let users = self.users.lock().await;
        let participants: Vec<incoming::Participant> = users
            .values()
            .map(|user| user.participant.clone())
            .collect::<Vec<_>>();
        self.to_recorder_tx
            .send(incoming::Message::Control(
                incoming::ControlMessage::JoinSuccess(incoming::JoinSuccess {
                    id: ParticipantId(Uuid::new_v4()),
                    participants,
                    event_info: EventInfo {
                        title: "Test Recording Title".to_string(),
                    },
                }),
            ))
            .await
            .expect("unable to send join success event to recorder");
    }

    pub(crate) async fn send_joined(&mut self, index: usize) -> incoming::Participant {
        let participant = incoming::Participant {
            id: ParticipantId(Uuid::new_v4()),
            control: incoming::ControlData {
                display_name: format!("MockUser {index}"),
            },
            media: incoming::MediaData {
                video: None,
                screen: None,
                is_presenter: false,
            },
            recording: incoming::RecordingData {
                consents_recording: false,
            },
        };

        let join_event = incoming::ControlMessage::Joined(participant.clone());

        self.to_recorder_tx
            .send(incoming::Message::Control(join_event))
            .await
            .expect("unable to send joined event to recorder");

        participant
    }

    pub(crate) async fn send_left(&mut self, participant: &incoming::Participant) {
        let left_event = incoming::ControlMessage::Left { id: participant.id };

        self.to_recorder_tx
            .send(incoming::Message::Control(left_event))
            .await
            .expect("unable to send left event to recorder");
    }

    pub(crate) async fn send_update_media(
        &mut self,
        participant: &mut incoming::Participant,
        audio: bool,
        video: bool,
        screen: bool,
    ) {
        participant.media.video = if video || audio {
            Some(incoming::MediaSessionState { video, audio })
        } else {
            None
        };

        participant.media.screen = if screen {
            Some(incoming::MediaSessionState {
                video: true,
                audio: false,
            })
        } else {
            None
        };

        self.to_recorder_tx
            .send(incoming::Message::Control(
                incoming::ControlMessage::Update(participant.clone()),
            ))
            .await
            .expect("unable to send update event to recorder");
    }

    pub(crate) async fn send_update_consent(
        &mut self,
        participant: &mut incoming::Participant,
        consent: bool,
    ) {
        participant.recording.consents_recording = consent;

        self.to_recorder_tx
            .send(incoming::Message::Control(
                incoming::ControlMessage::Update(participant.clone()),
            ))
            .await
            .expect("unable to send update event to recorder");
    }

    pub(crate) async fn send_update_focus(&mut self, participant: Option<&incoming::Participant>) {
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
