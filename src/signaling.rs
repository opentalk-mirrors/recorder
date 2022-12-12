use crate::http::HttpClient;
use crate::settings::Settings;
use crate::signaling::incoming::Error;
use anyhow::{bail, Context, Result};
use futures::{SinkExt, StreamExt};
use reqwest::header::SEC_WEBSOCKET_PROTOCOL;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpStream;
use tt::tungstenite::client::IntoClientRequest;
use tt::tungstenite::Message;
use tt::{MaybeTlsStream, WebSocketStream};
use uuid::Uuid;

pub struct Signaling {
    /// Own participant id
    _id: ParticipantId,

    /// List of all other participants in the conference
    participants: HashMap<ParticipantId, ParticipantState>,

    connection: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

pub struct ParticipantState {
    pub display_name: String,
    pub consents: bool,
    publishing: HashMap<MediaSessionType, incoming::MediaSessionState>,
}

impl ParticipantState {
    fn from_incoming(p: incoming::Participant) -> Self {
        Self {
            display_name: p.control.display_name,
            consents: p.recording.consents_recording,
            publishing: p.media.data,
        }
    }

    pub fn publishes(&self, typ: MediaSessionType) -> bool {
        self.publishing.contains_key(&typ) && self.consents
    }

    pub fn is_showing_video(&self, typ: MediaSessionType) -> bool {
        self.publishing
            .get(&typ)
            .map(|state| state.video)
            .unwrap_or_default()
    }
}

/// Event emitted by [`Signaling::run`]
#[derive(Debug)]
pub enum Event {
    ParticipantJoined(ParticipantId),
    ParticipantUpdated(ParticipantId),
    ParticipantLeft(ParticipantId),

    SdpOffer(ParticipantId, MediaSessionType, String),
    SdpCandidate(ParticipantId, MediaSessionType, TrickleCandidate),
    SdpEndOfCandidates(ParticipantId, MediaSessionType),

    FocusUpdate(Option<ParticipantId>),
    MediaConnectionError(Error),
    Close,
}

impl Signaling {
    pub async fn connect(
        client: Arc<HttpClient>,
        settings: Arc<Settings>,
        room_id: &str,
    ) -> Result<Self> {
        let ticket = client.start(&settings.controller, room_id).await?;

        let mut websocket_request = settings.controller.websocket_url().into_client_request()?;
        websocket_request.headers_mut().insert(
            SEC_WEBSOCKET_PROTOCOL,
            format!("k3k-signaling-json-v1.0,ticket#{}", ticket).try_into()?,
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

        let (id, participants) =
            if let Some(Message::Text(text)) = stream.next().await.transpose()? {
                let payload = serde_json::from_str::<Payload<incoming::JoinSuccess>>(&text)
                    .context("invalid join_success message")?;

                (payload.payload.id, payload.payload.participants)
            } else {
                bail!("unexpected websocket response")
            };

        Ok(Self {
            _id: id,
            participants: participants
                .into_iter()
                .map(|p| (p.id, ParticipantState::from_incoming(p)))
                .collect(),
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
                incoming::MediaMessage::SdpOffer(sdp) => Ok(Some(Event::SdpOffer(
                    sdp.source.source,
                    sdp.source.media_session_type,
                    sdp.sdp,
                ))),
                incoming::MediaMessage::SdpCandidate(candidate) => Ok(Some(Event::SdpCandidate(
                    candidate.source.source,
                    candidate.source.media_session_type,
                    candidate.candidate,
                ))),
                incoming::MediaMessage::SdpEndOfCandidates(source) => Ok(Some(
                    Event::SdpEndOfCandidates(source.source, source.media_session_type),
                )),
                incoming::MediaMessage::WebRtcUp(_) => Ok(None),
                incoming::MediaMessage::WebRtcDown(_) => Ok(None),

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

    pub async fn start_subscribe(
        &mut self,
        id: ParticipantId,
        typ: MediaSessionType,
    ) -> Result<()> {
        self.send(outgoing::Message::Media(outgoing::MediaMessage::Subscribe(
            outgoing::Target {
                target: id,
                media_session_type: typ,
            },
        )))
        .await
    }

    pub async fn send_answer(
        &mut self,
        id: ParticipantId,
        typ: MediaSessionType,
        sdp: String,
    ) -> Result<()> {
        self.send(outgoing::Message::Media(outgoing::MediaMessage::SdpAnswer(
            outgoing::Sdp {
                sdp,
                target: outgoing::Target {
                    target: id,
                    media_session_type: typ,
                },
            },
        )))
        .await
    }

    pub async fn send_candidate(
        &mut self,
        id: ParticipantId,
        typ: MediaSessionType,
        candidate: TrickleCandidate,
    ) -> Result<()> {
        self.send(outgoing::Message::Media(
            outgoing::MediaMessage::SdpCandidate(outgoing::SdpCandidate {
                candidate,
                target: outgoing::Target {
                    target: id,
                    media_session_type: typ,
                },
            }),
        ))
        .await
    }

    pub async fn send_end_of_candidates(
        &mut self,
        id: ParticipantId,
        typ: MediaSessionType,
    ) -> Result<()> {
        self.send(outgoing::Message::Media(
            outgoing::MediaMessage::SdpEndOfCandidates(outgoing::Target {
                target: id,
                media_session_type: typ,
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

#[derive(Debug, Deserialize, Serialize)]
struct Payload<'s, T> {
    pub namespace: &'s str,
    pub payload: T,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ParticipantId(pub Uuid);

mod incoming {
    use std::collections::HashMap;

    use super::{MediaSessionType, ParticipantId, TrickleCandidate};
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    pub struct JoinSuccess {
        pub id: ParticipantId,
        pub participants: Vec<Participant>,
    }

    #[derive(Debug, Deserialize)]
    pub struct Participant {
        pub id: ParticipantId,
        pub control: ControlData,
        #[serde(default)]
        pub media: MediaData,
        #[serde(default)]
        pub recording: RecordingData,
    }

    #[derive(Debug, Deserialize)]
    pub struct ControlData {
        pub display_name: String,
    }

    #[derive(Debug, Default, Deserialize)]
    pub struct MediaData {
        #[serde(flatten)]
        pub data: HashMap<MediaSessionType, MediaSessionState>,
    }

    #[derive(Debug, Default, Deserialize)]
    pub struct RecordingData {
        #[serde(default)]
        pub consents_recording: bool,
    }

    #[derive(Debug, Deserialize, Copy, Clone)]
    pub struct MediaSessionState {
        pub video: bool,
        pub audio: bool,
    }

    #[derive(Debug, Deserialize)]
    #[serde(tag = "namespace", content = "payload", rename_all = "snake_case")]
    pub enum Message {
        Control(ControlMessage),
        Media(MediaMessage),
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "snake_case", tag = "message")]
    pub enum ControlMessage {
        Joined(Participant),
        Update(Participant),
        Left { id: ParticipantId },
    }

    #[derive(Debug, Deserialize)]
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

    #[derive(Debug, Deserialize)]
    pub struct Sdp {
        pub sdp: String,
        #[serde(flatten)]
        pub source: Source,
    }

    #[derive(Debug, Deserialize)]
    pub struct SdpCandidate {
        pub candidate: TrickleCandidate,
        #[serde(flatten)]
        pub source: Source,
    }

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    pub struct Source {
        pub source: ParticipantId,
        pub media_session_type: MediaSessionType,
    }

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "lowercase")]
    pub enum LinkDirection {
        Upstream,
        Downstream,
    }

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    pub struct Link {
        pub direction: LinkDirection,
        #[serde(flatten)]
        pub source: Source,
    }

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    pub struct FocusUpdate {
        pub focus: Option<ParticipantId>,
    }

    /// Represents a error of the janus media module
    #[derive(Debug, Deserialize, PartialEq, Eq)]
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

mod outgoing {
    use super::{MediaSessionType, ParticipantId, TrickleCandidate};
    use serde::Serialize;

    #[derive(Debug, Serialize)]
    #[serde(tag = "namespace", content = "payload", rename_all = "snake_case")]
    pub enum Message {
        #[allow(unused)]
        Control(ControlMessage),
        Media(MediaMessage),
    }

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "snake_case", tag = "action")]
    pub enum ControlMessage {}

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "snake_case", tag = "action")]
    pub enum MediaMessage {
        Subscribe(Target),
        SdpAnswer(Sdp),
        SdpCandidate(SdpCandidate),
        SdpEndOfCandidates(Target),
    }

    #[derive(Debug, Serialize)]
    pub struct Sdp {
        pub sdp: String,
        #[serde(flatten)]
        pub target: Target,
    }

    #[derive(Debug, Serialize)]
    pub struct SdpCandidate {
        pub candidate: TrickleCandidate,
        #[serde(flatten)]
        pub target: Target,
    }

    #[derive(Debug, Serialize)]
    pub struct Target {
        pub target: ParticipantId,
        pub media_session_type: MediaSessionType,
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrickleCandidate {
    pub candidate: String,
    #[serde(rename = "sdpMLineIndex")]
    pub sdp_m_line_index: u64,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MediaSessionType {
    #[serde(rename = "video")]
    Camera,
    #[serde(rename = "screen")]
    ScreenCapture,
}
