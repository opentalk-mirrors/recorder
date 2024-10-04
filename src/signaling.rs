// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use core::fmt::Debug;
use std::collections::{BTreeMap, HashMap};

use anyhow::{bail, Context, Result};
use compositor::MediaDescriptor;
use futures::{SinkExt, StreamExt};
use reqwest::header::SEC_WEBSOCKET_PROTOCOL;
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tt::{
    tungstenite::{client::IntoClientRequest, Message},
    MaybeTlsStream, WebSocketStream,
};
use types::signaling::{
    control::{self, event::JoinSuccess, state::ControlState, Participant},
    media::{peer_state::MediaPeerState, MediaSessionState, MediaSessionType},
    recording::{
        peer_state::RecordingPeerState, state::RecorderStreamInfo, StreamStatus, StreamUpdated,
    },
    recording_service::{event::RecordingServiceEvent, state::RecordingServiceState},
};
use types_common::streaming::StreamingTargetId;
use types_signaling::ParticipantId;

use crate::{http::HttpClient, settings::ControllerSettings};

#[derive(Debug)]
pub struct Signaling {
    /// Own participant id
    _id: Option<ParticipantId>,

    pub connection: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

#[allow(dead_code)]
enum Substream {
    Low = 0,
    Medium = 1,
    High = 2,
}

#[derive(Debug, Clone)]
pub struct ParticipantState {
    pub display_name: String,
    pub consents: bool,
    publishing: HashMap<MediaSessionType, MediaSessionState>,
}

impl ParticipantState {
    pub(crate) fn from_incoming(p: &Participant) -> Result<Self> {
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
    pub(crate) fn publishes(&self, typ: MediaSessionType) -> Option<MediaSessionState> {
        if !self.consents {
            return None;
        }
        self.publishing.get(&typ).copied()
    }
}

impl Signaling {
    /// This constructor is used by the integration tests to mock data.
    #[allow(dead_code)]
    pub fn new(
        id: Option<ParticipantId>,
        connection: WebSocketStream<MaybeTlsStream<TcpStream>>,
    ) -> Self {
        Self {
            _id: id,
            connection,
        }
    }

    pub async fn connect(
        client: &HttpClient,
        settings: &ControllerSettings,
        room_id: &str,
        breakout_id: &Option<String>,
    ) -> Result<Self> {
        let ticket = client.start(settings, room_id, breakout_id).await?;

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
            connection: stream,
        })
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) async fn handle_websocket_message(
        &mut self,
        msg: Message,
    ) -> Result<Option<incoming::Message>> {
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
                return Ok(None);
            }
            Message::Frame(_) => unreachable!("send-only message"),
        };

        match parse_result {
            Ok(msg) => Ok(Some(msg)),
            Err(e) => {
                log::debug!("Unknown incoming message {msg:?}, {e}");
                Err(e.into())
            }
        }
    }

    pub async fn recv_new_signal(&mut self) -> Result<Option<incoming::Message>> {
        let msg = self.connection.next().await;

        let Some(msg) = msg else {
            bail!("Failed to receive websocket message");
        };

        let msg = msg.context("Failed to receive websocket message")?;

        self.handle_websocket_message(msg).await
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

    pub async fn send_configuration(
        &mut self,
        descriptor: MediaDescriptor,
        video: bool,
    ) -> Result<()> {
        log::trace!("send_configuration for descriptor '{descriptor:?}' with video '{video}'");
        self.send(outgoing::Message::Media(outgoing::MediaMessage::Configure(
            outgoing::Configure {
                configuration: outgoing::Configuration {
                    video,
                    substream: Substream::High as usize,
                },
                target: outgoing::Target {
                    target: descriptor.participant_id,
                    media_session_type: descriptor.media_type,
                },
            },
        )))
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

pub(crate) fn handle_join_success(
    state: &JoinSuccess,
) -> Result<BTreeMap<StreamingTargetId, RecorderStreamInfo>> {
    let recording_service_state = state
        .get_module::<RecordingServiceState>()?
        .context("No Service State has been found")?;

    let streaming_targets = recording_service_state.streams;

    Ok(streaming_targets)
}

pub(crate) fn handle_joined(
    participant: &Participant,
    participants: &mut HashMap<ParticipantId, ParticipantState>,
) -> Result<()> {
    let id = participant.id;
    let participant = match ParticipantState::from_incoming(participant) {
        Ok(p) => p,
        Err(e) => {
            log::error!("Failed to parse incoming Joined message: {e}");
            return Err(e);
        }
    };
    participants.insert(id, participant);
    Ok(())
}

pub(crate) fn handle_left(
    id: &control::AssociatedParticipant,
    participants: &mut HashMap<ParticipantId, ParticipantState>,
) {
    participants.remove(&id.id);
}

pub(crate) fn handle_update(
    participant: &Participant,
    participants: &mut HashMap<ParticipantId, ParticipantState>,
) {
    let Some(state) = participants.get_mut(&participant.id) else {
        log::error!("Got update for unknown participant {:?}", participant.id);
        return;
    };

    *state = match ParticipantState::from_incoming(participant) {
        Ok(p) => p,
        Err(e) => {
            log::error!("Failed to parse incoming Update message: {e}");
            return;
        }
    };
}

pub(crate) fn process_participants(
    state: &JoinSuccess,
) -> Result<HashMap<ParticipantId, ParticipantState>> {
    match state
        .participants
        .clone()
        .into_iter()
        .map(|p| {
            let id = p.id;
            ParticipantState::from_incoming(&p).map(|ps| (id, ps))
        })
        .collect::<Result<HashMap<_, _>>>()
    {
        Ok(p) => Ok(p),
        Err(e) => {
            log::error!("Failed to parse incoming JoinSuccess message: {e}");
            Err(e)
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Payload<'s, T> {
    pub namespace: &'s str,
    pub payload: T,
}

pub mod incoming {

    use compositor::MediaDescriptor;
    use serde::{Deserialize, Serialize};
    use types::signaling::{
        control::event::ControlEvent,
        media::{MediaSessionType, ParticipantSpeakingState},
        recording_service::command::RecordingServiceCommand,
    };

    use super::{ParticipantId, TrickleCandidate};

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
    use serde::{Deserialize, Serialize};
    use types::signaling::recording_service::event::RecordingServiceEvent;

    use super::{ParticipantId, TrickleCandidate};
    use crate::signaling::MediaSessionType;

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
        Configure(Configure),
        SdpAnswer(Sdp),
        SdpCandidate(SdpCandidate),
        SdpEndOfCandidates(Target),
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Configure {
        pub configuration: Configuration,
        #[serde(flatten)]
        pub target: Target,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Configuration {
        pub video: bool,
        pub substream: usize,
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
