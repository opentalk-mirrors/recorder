// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use anyhow::{bail, Context, Result};
use compositor::StreamId;
use futures::{SinkExt, StreamExt};
use reqwest::header::SEC_WEBSOCKET_PROTOCOL;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::net::TcpStream;
use tt::{
    tungstenite::{client::IntoClientRequest, Message},
    MaybeTlsStream, WebSocketStream,
};
use uuid::Uuid;

use crate::{
    http::HttpClient,
    settings::ControllerSettings,
    signaling::incoming::{Error, MediaSessionState},
};

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
    #[must_use]
    fn from_incoming(p: incoming::Participant) -> Self {
        let mut publishing = HashMap::new();
        if let Some(camera) = p.media.video {
            publishing.insert(MediaSessionType::Camera, camera);
        }

        if let Some(screen) = p.media.screen {
            publishing.insert(MediaSessionType::ScreenCapture, screen);
        }

        Self {
            display_name: p.control.display_name,
            consents: p.recording.consents_recording,
            publishing,
        }
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
    JoinSuccess(ParticipantId, String),
    ParticipantJoined(ParticipantId),
    ParticipantUpdated(ParticipantId),
    ParticipantLeft(ParticipantId),

    SdpOffer(StreamId<ParticipantId>, String),
    SdpCandidate(StreamId<ParticipantId>, TrickleCandidate),
    SdpEndOfCandidates(StreamId<ParticipantId>),

    FocusUpdate(Option<ParticipantId>),
    MediaConnectionError(Error),
    Close,
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

    async fn handle_websocket_message(&mut self, msg: Message) -> Result<Option<Event>> {
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
                log::error!("Failed to parse incoming message {msg:?}, {e}");
                return Ok(None);
            }
        };

        match msg {
            incoming::Message::Control(msg) => match msg {
                incoming::ControlMessage::JoinSuccess(state) => {
                    self.participants = state
                        .participants
                        .into_iter()
                        .map(|p| (p.id, ParticipantState::from_incoming(p)))
                        .collect();

                    Ok(Some(Event::JoinSuccess(state.id, state.event_info.title)))
                }
                incoming::ControlMessage::Joined(participant) => {
                    let id = participant.id;

                    self.participants
                        .insert(id, ParticipantState::from_incoming(participant));

                    Ok(Some(Event::ParticipantJoined(id)))
                }
                incoming::ControlMessage::Update(participant) => {
                    if let Some(state) = self.participants.get_mut(&participant.id) {
                        let id = participant.id;

                        *state = ParticipantState::from_incoming(participant);

                        Ok(Some(Event::ParticipantUpdated(id)))
                    } else {
                        log::error!("Got update for unknown participant {}", participant.id.0);
                        Ok(None)
                    }
                }
                incoming::ControlMessage::Left { id } => {
                    self.participants.remove(&id);
                    Ok(Some(Event::ParticipantLeft(id)))
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
            },
        }
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

    pub async fn start_subscribe(&mut self, stream_id: StreamId<ParticipantId>) -> Result<()> {
        self.send(outgoing::Message::Media(outgoing::MediaMessage::Subscribe(
            stream_id.into(),
        )))
        .await
    }

    pub async fn send_answer(
        &mut self,
        stream_id: StreamId<ParticipantId>,
        sdp: String,
    ) -> Result<()> {
        self.send(outgoing::Message::Media(outgoing::MediaMessage::SdpAnswer(
            outgoing::Sdp {
                sdp,
                target: stream_id.into(),
            },
        )))
        .await
    }

    pub async fn send_candidate(
        &mut self,
        stream_id: StreamId<ParticipantId>,
        candidate: TrickleCandidate,
    ) -> Result<()> {
        self.send(outgoing::Message::Media(
            outgoing::MediaMessage::SdpCandidate(outgoing::SdpCandidate {
                candidate,
                target: stream_id.into(),
            }),
        ))
        .await
    }

    pub async fn send_end_of_candidates(
        &mut self,
        stream_id: StreamId<ParticipantId>,
    ) -> Result<()> {
        self.send(outgoing::Message::Media(
            outgoing::MediaMessage::SdpEndOfCandidates(outgoing::Target {
                target: stream_id.id,
                media_session_type: stream_id.media_type,
            }),
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ParticipantId(pub Uuid);

impl std::fmt::Display for ParticipantId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub mod incoming {

    use super::{MediaSessionType, ParticipantId, TrickleCandidate};
    use compositor::{StreamId, StreamStatus};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct JoinSuccess {
        pub id: ParticipantId,
        pub participants: Vec<Participant>,
        pub event_info: EventInfo,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct EventInfo {
        pub title: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Participant {
        pub id: ParticipantId,
        pub control: ControlData,
        #[serde(default)]
        pub media: MediaData,
        #[serde(default)]
        pub recording: RecordingData,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ControlData {
        pub display_name: String,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct MediaData {
        pub video: Option<MediaSessionState>,
        pub screen: Option<MediaSessionState>,
        pub is_presenter: bool,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct RecordingData {
        #[serde(default)]
        pub consents_recording: bool,
    }

    #[derive(Debug, Serialize, Deserialize, Copy, Clone)]
    pub struct MediaSessionState {
        pub video: bool,
        pub audio: bool,
    }

    impl std::fmt::Display for MediaSessionState {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                MediaSessionState {
                    video: true,
                    audio: true,
                } => write!(f, "video+audio"),
                MediaSessionState {
                    video: true,
                    audio: false,
                } => write!(f, "video only"),
                MediaSessionState {
                    video: false,
                    audio: true,
                } => write!(f, "audio only"),
                MediaSessionState {
                    video: false,
                    audio: false,
                } => write!(f, "none"),
            }
        }
    }

    impl From<MediaSessionState> for StreamStatus {
        fn from(state: MediaSessionState) -> Self {
            StreamStatus {
                has_audio: state.audio,
                has_video: state.video,
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "namespace", content = "payload", rename_all = "snake_case")]
    pub enum Message {
        Control(ControlMessage),
        Media(MediaMessage),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case", tag = "message")]
    pub enum ControlMessage {
        JoinSuccess(JoinSuccess),
        Joined(Participant),
        Update(Participant),
        Left { id: ParticipantId },
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

    impl From<Source> for StreamId<ParticipantId> {
        fn from(value: Source) -> Self {
            StreamId::new(value.source, value.media_session_type)
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
    use super::{MediaSessionType, ParticipantId, TrickleCandidate};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(tag = "namespace", content = "payload", rename_all = "snake_case")]
    pub enum Message {
        #[allow(unused)]
        Control(ControlMessage),
        Media(MediaMessage),
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

impl From<StreamId<ParticipantId>> for outgoing::Target {
    fn from(value: StreamId<ParticipantId>) -> Self {
        outgoing::Target {
            target: value.id,
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

type MediaSessionType = compositor::MediaSessionType;

#[must_use]
pub fn media_types() -> impl DoubleEndedIterator<Item = MediaSessionType> {
    compositor::media_types()
}
