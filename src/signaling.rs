use crate::settings::Settings;
use anyhow::{Context, Result, bail};
use futures::{SinkExt, StreamExt};
use rand::{distributions::Alphanumeric, Rng};
use reqwest::header::{
    CONNECTION, HOST, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_PROTOCOL, SEC_WEBSOCKET_VERSION, UPGRADE,
};
use reqwest::Client;
use serde::Deserialize;
use std::sync::Arc;
use tt::tungstenite::http::Request;
use tt::tungstenite::Message;

pub struct Connection {}

impl Connection {
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

        if let Some(Message::Text(text)) = stream.next().await.transpose()? {
            let payload = serde_json::from_str::<proto::Payload<proto::JoinSuccess>>(&text).context("invalid join_success")?;

            println!("{:?}", payload);
        } else {
            bail!("unexpected websocket response")
        }

        todo!()
    }
}

mod proto {
    use serde::Deserialize;


    #[derive(Debug, Deserialize)]
    pub struct Payload<T> {
        pub payload: T
    }

    #[derive(Debug, Deserialize)]
    pub struct JoinSuccess {
        pub id: String,
    }
}
