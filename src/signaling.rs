use crate::settings::Settings;
use anyhow::{bail, Context, Result};
use futures::{SinkExt, StreamExt};
use rand::{distributions::Alphanumeric, Rng};
use reqwest::header::{
    CONNECTION, HOST, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_PROTOCOL, SEC_WEBSOCKET_VERSION, UPGRADE,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::net::TcpStream;
use tt::tungstenite::http::Request;
use tt::tungstenite::Message;
use tt::{MaybeTlsStream, WebSocketStream};
use uuid::Uuid;

pub struct Signaling {
    /// Own participant id
    id: ParticipantId,

    /// List of all other participants in the conference
    participants: Vec<ParticipantState>,

    connection: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

pub enum Event {}

struct ParticipantState {
    id: ParticipantId,
}

impl Signaling {
    pub async fn connect(client: Client, settings: Arc<Settings>, room_id: String) -> Result<Self> {
        let key: Vec<u8> = rand::thread_rng()
            .sample_iter(Alphanumeric)
            .take(32)
            .collect();

        #[derive(Deserialize)]
        struct StartResponse {
            ticket: String,
        }

        let ticket_response = client
            .post(format!(
                "{}/rooms/recorder/start",
                settings.controller.api_base_url()
            ))
            .json(&serde_json::json!({ "room_id": room_id }))
            .send()
            .await?
            .json::<StartResponse>()
            .await?;

        let websocket_request = Request::get(settings.controller.websocket_url())
            .header(
                SEC_WEBSOCKET_PROTOCOL,
                format!("k3k-signaling-json-v1.0,ticket#{}", ticket_response.ticket),
            )
            .header(SEC_WEBSOCKET_KEY, key)
            .header(SEC_WEBSOCKET_VERSION, "13")
            .header(HOST, &settings.controller.domain)
            .header(CONNECTION, "Upgrade")
            .header(UPGRADE, "websocket")
            .body(())?;

        let (mut stream, _) = tt::connect_async(websocket_request)
            .await
            .context("connect")?;

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
            id,
            participants: participants
                .into_iter()
                .map(|p| ParticipantState { id: p.id })
                .collect(),
            connection: stream,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            tokio::select! {
                msg = self.connection.next() => {
                    if let Some(msg) = msg {
                        let msg = msg.context("Failed to receive websocket message")?;
                        self.handle_websocket_message(msg).await?;
                    } else {
                        bail!("unexpected websocket disconnection");
                    }
                }
            }
        }
    }

    async fn handle_websocket_message(&mut self, msg: Message) -> Result<()> {
        let parse_result = match msg {
            Message::Text(ref s) => serde_json::from_str::<incoming::Message>(s),
            Message::Binary(ref b) => serde_json::from_slice::<incoming::Message>(b),
            Message::Ping(data) => {
                self.connection.send(Message::Pong(data)).await?;
                return Ok(());
            }
            Message::Pong(_) => return Ok(()),
            Message::Close(_) => todo!(),
            Message::Frame(_) => unreachable!("send-only message"),
        };

        let msg = match parse_result {
            Ok(msg) => msg,
            Err(e) => {
                log::error!("Failed to parse incoming message {msg:?}, {e}");
                return Ok(());
            }
        };

        match msg {
            incoming::Message::Control(msg) => match msg {},
            incoming::Message::Media(msg) => match msg {
                incoming::MediaMessage::SdpOffer(sdp) => todo!(),
                incoming::MediaMessage::SdpCandidate(candidate) => todo!(),
                incoming::MediaMessage::SdpEndOfCandidates(_) => todo!(),
                incoming::MediaMessage::WebRtcUp(_) => todo!(),
                incoming::MediaMessage::WebRtcDown(_) => todo!(),
            },
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct Payload<'s, T> {
    pub namespace: &'s str,
    pub payload: T,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ParticipantId(pub Uuid);

mod incoming {
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
    }

    #[derive(Debug, Deserialize)]
    #[serde(tag = "namespace", content = "payload", rename_all = "snake_case")]
    pub enum Message {
        Control(ControlMessage),
        Media(MediaMessage),
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum ControlMessage {}

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "snake_case", tag = "message")]
    pub enum MediaMessage {
        SdpOffer(Sdp),
        SdpCandidate(SdpCandidate),
        SdpEndOfCandidates(Source),
        WebRtcUp(Source),
        WebRtcDown(Source),
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

    #[derive(Debug, Deserialize)]
    pub struct Source {
        pub source: ParticipantId,
        pub media_session_type: MediaSessionType,
    }
}

mod outgoing {
    use super::{MediaSessionType, ParticipantId, TrickleCandidate};
    use serde::Serialize;

    #[derive(Debug, Serialize)]
    #[serde(tag = "namespace", content = "payload", rename_all = "snake_case")]
    pub enum Action {
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaSessionType {
    Video,
    Screen,
}
