// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use anyhow::{bail, Context, Result};
use compositor::MediaDescriptor;
use futures::{SinkExt, StreamExt};
use reqwest::header::SEC_WEBSOCKET_PROTOCOL;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use tokio::net::TcpStream;
use tt::{
    tungstenite::{client::IntoClientRequest, Message},
    MaybeTlsStream, WebSocketStream,
};
use types::{
    core::{ParticipantId, StreamingTargetId},
    signaling::{
        control::{event::ControlEvent, state::ControlState, AssociatedParticipant, Participant},
        media::{
            peer_state::MediaPeerState, MediaSessionState, MediaSessionType,
            ParticipantSpeakingState,
        },
        recording::{
            peer_state::RecordingPeerState, state::RecorderStreamInfo, StreamStatus, StreamUpdated,
        },
        recording_service::{
            command::RecordingServiceCommand, event::RecordingServiceEvent,
            state::RecordingServiceState,
        },
    },
};

use crate::{http::HttpClient, settings::ControllerSettings, signaling::incoming::Error};

#[derive(Debug)]
pub struct Signaling {
    /// Own participant id
    _id: Option<ParticipantId>,

    /// List of all other participants in the conference
    participants: HashMap<ParticipantId, ParticipantState>,

    connection: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

#[derive(Debug, Clone)]
pub struct ParticipantState {
    pub display_name: String,
    pub consents: bool,
    publishing: HashMap<MediaSessionType, MediaSessionState>,
}

impl ParticipantState {
    fn from_incoming(p: &Participant) -> Result<Self> {
        let media: MediaPeerState = p
            .get_module::<MediaPeerState>()
            .ok()
            .flatten()
            .unwrap_or_default();
        let recording: RecordingPeerState = p
            .get_module::<RecordingPeerState>()
            .ok()
            .flatten()
            .unwrap_or_default();
        let control: ControlState = p
            .get_module::<ControlState>()?
            .context("participant is missing control state")?;
        let mut publishing = HashMap::new();
        if let Some(camera) = media.state.video {
            publishing.insert(MediaSessionType::Video, camera);
        }

        if let Some(screen) = media.state.screen {
            publishing.insert(MediaSessionType::Screen, screen);
        }

        Ok(Self {
            display_name: control.display_name,
            consents: recording.consents_recording,
            publishing,
        })
    }

    #[must_use]
    pub fn publishes(&self, typ: MediaSessionType) -> Option<MediaSessionState> {
        if !self.consents {
            return None;
        }
        self.publishing.get(&typ).copied()
    }
}

/// Event emitted by [`Signaling::run`]
#[derive(Debug)]
pub enum Event {
    JoinSuccess {
        participant_id: ParticipantId,
        event_title: String,
        streaming_targets: BTreeMap<StreamingTargetId, RecorderStreamInfo>,
    },
    ParticipantJoined(ParticipantId),
    ParticipantUpdated(ParticipantId),
    ParticipantLeft(ParticipantId),

    SdpOffer(MediaDescriptor, String),
    SdpCandidate(MediaDescriptor, TrickleCandidate),
    SdpEndOfCandidates(MediaDescriptor),

    SpeakerUpdated(ParticipantSpeakingState),
    FocusUpdate(Option<ParticipantId>),
    MediaConnectionError(Error),
    Close,

    Start(BTreeSet<StreamingTargetId>),
    Pause(BTreeSet<StreamingTargetId>),
    Stop(BTreeSet<StreamingTargetId>),
}

impl Signaling {
    /// This constructor is used by the integration tests to mock data.
    #[allow(dead_code)]
    pub fn new(
        id: Option<ParticipantId>,
        participants: HashMap<ParticipantId, ParticipantState>,
        connection: WebSocketStream<MaybeTlsStream<TcpStream>>,
    ) -> Self {
        Self {
            _id: id,
            participants,
            connection,
        }
    }

    pub async fn connect(
        client: &HttpClient,
        settings: &ControllerSettings,
        room_id: &str,
    ) -> Result<Self> {
        let ticket = client.start(settings, room_id).await?;

        let mut websocket_request = settings.websocket_url().into_client_request()?;
        websocket_request.headers_mut().insert(
            SEC_WEBSOCKET_PROTOCOL,
            format!("opentalk-signaling-json-v1.0,ticket#{ticket}").try_into()?,
        );

        let (mut stream, _) = tt::connect_async(websocket_request)
            .await
            .context("failed create websocket connection")?;

        stream
            .send(Message::Text(serde_json::to_string(&serde_json::json!({
                "namespace":"control",
                "payload": {
                    "action":"join",
                    "display_name": "recorder"
                }
            }))?))
            .await?;

        Ok(Self {
            _id: None,
            participants: HashMap::new(),
            connection: stream,
        })
    }

    pub async fn run(&mut self) -> Result<Event> {
        loop {
            tokio::select! {
                msg = self.connection.next() => {
                    if let Some(msg) = msg {
                        let msg = msg.context("Failed to receive websocket message")?;
                        if let Some(event) = self.handle_websocket_message(msg).await? {
                            return Ok(event);
                        }
                    } else {
                        bail!("unexpected websocket disconnection");
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    async fn handle_websocket_message(&mut self, msg: Message) -> Result<Option<Event>> {
        log::trace!("handle_websocket_message: {msg:#?}");
        let parse_result = match msg {
            Message::Text(ref s) => serde_json::from_str::<incoming::Message>(s),
            Message::Binary(ref b) => serde_json::from_slice::<incoming::Message>(b),
            Message::Ping(data) => {
                self.connection.send(Message::Pong(data)).await?;
                return Ok(None);
            }
            Message::Pong(_) => return Ok(None),
            Message::Close(_) => {
                let _ = self.connection.close(None).await;
                return Ok(Some(Event::Close));
            }
            Message::Frame(_) => unreachable!("send-only message"),
        };

        let msg = match parse_result {
            Ok(msg) => msg,
            Err(e) => {
                log::debug!("Unknown incoming message {msg:?}, {e}");
                return Ok(None);
            }
        };

        match msg {
            incoming::Message::Control(msg) => match msg {
                ControlEvent::JoinSuccess(state) => self.handle_join_success(state),
                ControlEvent::Joined(participant) => Ok(self.handle_joined(&participant)),
                ControlEvent::Update(participant) => Ok(self.handle_update(&participant)),
                ControlEvent::Left(AssociatedParticipant { id }) => {
                    self.participants.remove(&id);
                    Ok(Some(Event::ParticipantLeft(id)))
                }
                ref other => {
                    log::error!("Event {other:#?} not implemented for recorder.");
                    Ok(None)
                }
            },
            incoming::Message::RecordingService(msg) => match msg {
                RecordingServiceCommand::StartStreams { target_ids } => {
                    Ok(Some(Event::Start(target_ids)))
                }
                RecordingServiceCommand::PauseStreams { target_ids } => {
                    Ok(Some(Event::Pause(target_ids)))
                }
                RecordingServiceCommand::StopStreams { target_ids } => {
                    Ok(Some(Event::Stop(target_ids)))
                }
            },
            incoming::Message::Media(msg) => match msg {
                incoming::MediaMessage::SdpOffer(sdp) => {
                    Ok(Some(Event::SdpOffer(sdp.source.into(), sdp.sdp)))
                }
                incoming::MediaMessage::SdpCandidate(candidate) => Ok(Some(Event::SdpCandidate(
                    candidate.source.into(),
                    candidate.candidate,
                ))),
                incoming::MediaMessage::SdpEndOfCandidates(source) => {
                    Ok(Some(Event::SdpEndOfCandidates(source.into())))
                }
                incoming::MediaMessage::WebRtcUp(_) | incoming::MediaMessage::WebRtcDown(_) => {
                    Ok(None)
                }
                incoming::MediaMessage::FocusUpdate(focus) => {
                    Ok(Some(Event::FocusUpdate(focus.focus)))
                }
                incoming::MediaMessage::WebRtcSlow(slow) => {
                    log::warn!("Slow participant {:?}", slow.source);
                    Ok(None)
                }
                incoming::MediaMessage::Error(error) => {
                    Ok(Some(Event::MediaConnectionError(error)))
                }
                incoming::MediaMessage::SpeakerUpdated(speak_state) => {
                    Ok(Some(Event::SpeakerUpdated(speak_state)))
                }
            },
        }
    }

    fn handle_update(&mut self, participant: &Participant) -> Option<Event> {
        let Some(state) = self.participants.get_mut(&participant.id) else {
            log::error!("Got update for unknown participant {:?}", participant.id);
            return None;
        };

        let id = participant.id;
        *state = match ParticipantState::from_incoming(participant) {
            Ok(p) => p,
            Err(e) => {
                log::error!("Failed to parse incoming Update message: {e}");
                return None;
            }
        };

        Some(Event::ParticipantUpdated(id))
    }

    fn handle_joined(&mut self, participant: &Participant) -> Option<Event> {
        let id = participant.id;
        let participant = match ParticipantState::from_incoming(participant) {
            Ok(p) => p,
            Err(e) => {
                log::error!("Failed to parse incoming Joined message: {e}");
                return None;
            }
        };
        self.participants.insert(id, participant);
        Some(Event::ParticipantJoined(id))
    }

    fn handle_join_success(
        &mut self,
        state: types::signaling::control::event::JoinSuccess,
    ) -> std::prelude::v1::Result<Option<Event>, anyhow::Error> {
        let streaming_targets = state
            .get_module::<RecordingServiceState>()?
            .context("No Service State has been found")?
            .streams;

        self.participants = match state
            .participants
            .into_iter()
            .map(|p| {
                let id = p.id;
                ParticipantState::from_incoming(&p).map(|ps| (id, ps))
            })
            .collect::<Result<HashMap<_, _>>>()
        {
            Ok(p) => p,
            Err(e) => {
                log::error!("Failed to parse incoming JoinSuccess message: {e}");
                return Ok(None);
            }
        };

        Ok(Some(Event::JoinSuccess {
            participant_id: state.id,
            event_title: state.event_info.map(|ei| ei.title).unwrap_or_default(),
            streaming_targets,
        }))
    }

    pub fn participants(&self) -> &HashMap<ParticipantId, ParticipantState> {
        &self.participants
    }

    pub fn participant(&self, id: &ParticipantId) -> Result<&ParticipantState> {
        let Some(participant_state) = self.participants.get(id) else {
            bail!("Participant {id} joined but not state exists");
        };

        Ok(participant_state)
    }

    pub async fn start_subscribe(&mut self, descriptor: MediaDescriptor) -> Result<()> {
        self.send(outgoing::Message::Media(outgoing::MediaMessage::Subscribe(
            descriptor.into(),
        )))
        .await
    }

    pub async fn send_answer(&mut self, descriptor: MediaDescriptor, sdp: String) -> Result<()> {
        self.send(outgoing::Message::Media(outgoing::MediaMessage::SdpAnswer(
            outgoing::Sdp {
                sdp,
                target: descriptor.into(),
            },
        )))
        .await
    }

    pub async fn send_candidate(
        &mut self,
        descriptor: MediaDescriptor,
        candidate: TrickleCandidate,
    ) -> Result<()> {
        self.send(outgoing::Message::Media(
            outgoing::MediaMessage::SdpCandidate(outgoing::SdpCandidate {
                candidate,
                target: descriptor.into(),
            }),
        ))
        .await
    }

    pub async fn send_end_of_candidates(&mut self, descriptor: MediaDescriptor) -> Result<()> {
        self.send(outgoing::Message::Media(
            outgoing::MediaMessage::SdpEndOfCandidates(outgoing::Target {
                target: descriptor.participant_id,
                media_session_type: descriptor.media_type,
            }),
        ))
        .await
    }

    pub async fn send_stream_update(
        &mut self,
        target_id: StreamingTargetId,
        status: StreamStatus,
    ) -> Result<()> {
        self.send(outgoing::Message::RecordingService(
            RecordingServiceEvent::StreamUpdated(StreamUpdated { target_id, status }),
        ))
        .await
    }

    async fn send(&mut self, msg: outgoing::Message) -> Result<()> {
        log::trace!("send signaling message {:?}", msg);
        self.connection
            .send(Message::Text(
                serde_json::to_string(&msg).context("failed to serialize message")?,
            ))
            .await
            .context("failed to send message")
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Payload<'s, T> {
    pub namespace: &'s str,
    pub payload: T,
}

pub mod incoming {

    use super::{ParticipantId, TrickleCandidate};
    use compositor::MediaDescriptor;
    use serde::{Deserialize, Serialize};
    use types::signaling::{
        control::event::ControlEvent,
        media::{MediaSessionType, ParticipantSpeakingState},
        recording_service::command::RecordingServiceCommand,
    };

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "namespace", content = "payload", rename_all = "snake_case")]
    pub enum Message {
        Control(ControlEvent),
        Media(MediaMessage),
        RecordingService(RecordingServiceCommand),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case", tag = "message")]
    pub enum MediaMessage {
        SdpOffer(Sdp),
        SdpCandidate(SdpCandidate),
        SdpEndOfCandidates(Source),
        #[serde(rename = "webrtc_up")]
        WebRtcUp(Source),
        #[serde(rename = "webrtc_down")]
        WebRtcDown(Source),
        /// A webrtc connection experienced package loss
        #[serde(rename = "webrtc_slow")]
        WebRtcSlow(Link),

        #[serde(rename = "focus_update")]
        FocusUpdate(FocusUpdate),

        #[serde(rename = "speaker_updated")]
        SpeakerUpdated(ParticipantSpeakingState),

        #[serde(rename = "error")]
        Error(Error),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Sdp {
        pub sdp: String,
        #[serde(flatten)]
        pub source: Source,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SdpCandidate {
        pub candidate: TrickleCandidate,
        #[serde(flatten)]
        pub source: Source,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct Source {
        pub source: ParticipantId,
        pub media_session_type: MediaSessionType,
    }

    impl From<Source> for MediaDescriptor {
        fn from(value: Source) -> Self {
            MediaDescriptor {
                participant_id: value.source,
                media_type: value.media_session_type,
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "lowercase")]
    pub enum LinkDirection {
        Upstream,
        Downstream,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct Link {
        pub direction: LinkDirection,
        #[serde(flatten)]
        pub source: Source,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct FocusUpdate {
        pub focus: Option<ParticipantId>,
    }

    /// Represents a error of the janus media module
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case", tag = "error")]
    pub enum Error {
        InvalidSdpOffer,
        HandleSdpAnswer,
        InvalidCandidate,
        InvalidEndOfCandidates,
        InvalidRequestOffer(Source),
        InvalidConfigureRequest(Source),
        PermissionDenied,
    }
}

pub mod outgoing {
    use super::{ParticipantId, TrickleCandidate};
    use crate::signaling::MediaSessionType;
    use serde::{Deserialize, Serialize};
    use types::signaling::recording_service::event::RecordingServiceEvent;

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(tag = "namespace", content = "payload", rename_all = "snake_case")]
    pub enum Message {
        #[allow(unused)]
        Control(ControlMessage),
        Media(MediaMessage),
        RecordingService(RecordingServiceEvent),
    }

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case", tag = "action")]
    pub enum ControlMessage {
        Join(Join),
    }

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub struct Join {
        display_name: String,
    }

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case", tag = "action")]
    pub enum MediaMessage {
        Subscribe(Target),
        SdpAnswer(Sdp),
        SdpCandidate(SdpCandidate),
        SdpEndOfCandidates(Target),
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Sdp {
        pub sdp: String,
        #[serde(flatten)]
        pub target: Target,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct SdpCandidate {
        pub candidate: TrickleCandidate,
        #[serde(flatten)]
        pub target: Target,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Target {
        pub target: ParticipantId,
        pub media_session_type: MediaSessionType,
    }
}

impl From<MediaDescriptor> for outgoing::Target {
    fn from(value: MediaDescriptor) -> Self {
        outgoing::Target {
            target: value.participant_id,
            media_session_type: value.media_type,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrickleCandidate {
    pub candidate: String,
    #[serde(rename = "sdpMLineIndex")]
    pub sdp_m_line_index: u64,
}
