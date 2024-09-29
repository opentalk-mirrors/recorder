// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use core::fmt::Debug;
use std::collections::{BTreeMap, HashMap};

use anyhow::{bail, Context, Result};
use futures::{SinkExt, StreamExt};
use reqwest::header::SEC_WEBSOCKET_PROTOCOL;
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tt::{
    tungstenite::{client::IntoClientRequest, Message},
    MaybeTlsStream, WebSocketStream,
};
use types::signaling::{
    media::{peer_state::MediaPeerState, MediaSessionType},
    recording::{
        peer_state::RecordingPeerState, state::RecorderStreamInfo, StreamStatus, StreamUpdated,
    },
    recording_service::{event::RecordingServiceEvent, state::RecordingServiceState},
};
use types_common::streaming::StreamingTargetId;
use types_control::{self, event::JoinSuccess, state::ControlState};
use types_signaling::{AssociatedParticipant, Participant, ParticipantId};

use crate::{http::HttpClient, settings::ControllerSettings};

#[derive(Debug)]
pub(crate) struct Signaling {
    /// Own participant id
    _id: Option<ParticipantId>,

    pub(crate) connection: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

#[allow(dead_code)]
enum Substream {
    Low = 0,
    Medium = 1,
    High = 2,
}

#[derive(Debug, Clone)]
pub(crate) struct ParticipantState {
    pub(crate) display_name: String,
    pub(crate) consents: bool,
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
        })
    }
}

impl Signaling {
    /// This constructor is used by the integration tests to mock data.
    #[allow(dead_code)]
    pub(crate) fn new(
        id: Option<ParticipantId>,
        connection: WebSocketStream<MaybeTlsStream<TcpStream>>,
    ) -> Self {
        Self {
            _id: id,
            connection,
        }
    }

    pub(crate) async fn connect(
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

    pub(crate) async fn recv_new_signal(&mut self) -> Result<Option<incoming::Message>> {
        let msg = self.connection.next().await;

        let Some(msg) = msg else {
            bail!("Failed to receive websocket message");
        };

        let msg = msg.context("Failed to receive websocket message")?;

        self.handle_websocket_message(msg).await
    }

    pub(crate) async fn send_stream_update(
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
    id: &AssociatedParticipant,
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
    pub(crate) namespace: &'s str,
    pub(crate) payload: T,
}

pub(crate) mod incoming {

    use serde::{Deserialize, Serialize};
    use types::signaling::recording_service::command::RecordingServiceCommand;
    use types_control::event::ControlEvent;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "namespace", content = "payload", rename_all = "snake_case")]
    pub(crate) enum Message {
        Control(ControlEvent),
        RecordingService(RecordingServiceCommand),
    }
}

pub(crate) mod outgoing {
    use serde::{Deserialize, Serialize};
    use types::signaling::recording_service::event::RecordingServiceEvent;

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(tag = "namespace", content = "payload", rename_all = "snake_case")]
    pub(crate) enum Message {
        #[allow(unused)]
        Control(ControlMessage),
        RecordingService(RecordingServiceEvent),
    }

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case", tag = "action")]
    pub(crate) enum ControlMessage {
        Join(Join),
    }

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub(crate) struct Join {
        display_name: String,
    }
}
