// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

//! HTTP calls made by this library (except for websockets)

use std::{
    future::Future,
    pin::Pin,
    time::{Duration, Instant},
};

use anyhow::{bail, Context, Result};
use futures::{SinkExt, StreamExt};
use openidconnect::{reqwest::Error, AccessToken, HttpRequest, HttpResponse, OAuth2TokenResponse};
use reqwest::{header::HeaderValue, StatusCode};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};
use tt::{
    tungstenite::{client::IntoClientRequest, Message},
    WebSocketStream,
};
use types_common::{streaming::StreamingTargetId, time::Timestamp};

use crate::settings::{AuthSettings, ControllerSettings};

const CHUNK_LIMIT: u32 = 950;

// TODO: Replace with version from opentalk-types
#[derive(Clone)]
pub(crate) struct FileExtension(String);

#[derive(Debug, Clone, thiserror::Error)]
#[error("reached chunk limit")]
pub struct ChunkUploadLimitReached;

#[derive(Debug, Clone)]
pub(crate) struct UploadLimitReached {
    pub(crate) id: StreamingTargetId,
}

impl FileExtension {
    #[must_use]
    pub(crate) fn webm() -> Self {
        Self("webm".to_string())
    }

    #[must_use]
    pub(crate) fn str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug)]
pub(crate) struct HttpClient {
    client: reqwest::Client,
    oidc: openidconnect::core::CoreClient,
    access_token: RwLock<AccessToken>,
}

impl HttpClient {
    /// This constructor is used by the integration tests to mock data.
    #[allow(dead_code)]
    pub(crate) fn new(
        client: reqwest::Client,
        oidc: openidconnect::core::CoreClient,
        access_token: RwLock<AccessToken>,
    ) -> Self {
        Self {
            client,
            oidc,
            access_token,
        }
    }

    pub(crate) async fn discover(settings: &AuthSettings) -> Result<Self> {
        let client = reqwest::Client::new();

        let metadata = openidconnect::core::CoreProviderMetadata::discover_async(
            settings.issuer.clone(),
            async_http_client(client.clone()),
        )
        .await?;

        let oidc = openidconnect::core::CoreClient::new(
            settings.client_id.clone(),
            Some(settings.client_secret.clone()),
            settings.issuer.clone(),
            metadata.authorization_endpoint().clone(),
            metadata.token_endpoint().cloned(),
            None,
            metadata.jwks().clone(),
        );

        let response = oidc
            .exchange_client_credentials()
            .request_async(async_http_client(client.clone()))
            .await?;

        Ok(Self {
            client,
            oidc,
            access_token: RwLock::new(response.access_token().clone()),
        })
    }

    async fn refresh_access_tokens(&self, invalid_token: AccessToken) -> Result<()> {
        let mut token = self.access_token.write().await;

        if token.secret() != invalid_token.secret() {
            return Ok(());
        }

        let response = self
            .oidc
            .exchange_client_credentials()
            .request_async(async_http_client(self.client.clone()))
            .await?;

        *token = response.access_token().clone();

        Ok(())
    }

    pub(crate) async fn start(
        &self,
        settings: &ControllerSettings,
        room_id: &str,
        breakout_room: Option<&str>,
    ) -> Result<String> {
        let uri = format!("{}/services/recording/start", settings.v1_api_base_url());

        // max 10 authentication tries
        for _ in 0..10 {
            let token = {
                // Scope the access to the lock to avoid holding it for the entire loop-body
                let l = self.access_token.read().await;
                l.clone()
            };

            let response = self
                .client
                .post(&uri)
                .bearer_auth(token.secret())
                .json(&StartRequest {
                    room_id,
                    breakout_room,
                })
                .send()
                .await?;

            match response.status() {
                StatusCode::OK => {
                    let response = response.json::<StartResponse>().await?;

                    return Ok(response.ticket);
                }
                StatusCode::UNAUTHORIZED => {
                    let ApiError { code } = response.json::<ApiError>().await?;

                    if code == "unauthorized" {
                        self.refresh_access_tokens(token).await?;
                    } else {
                        bail!(InvalidCredentials);
                    }
                }
                code => bail!("unexpected status code {code:?}"),
            }
        }

        bail!("failed to authorize")
    }

    pub(crate) async fn upload_render(
        &self,
        settings: &ControllerSettings,
        room_id: &str,
        file_extension: FileExtension,
        mut receiver: broadcast::Receiver<Vec<u8>>,
        sender: broadcast::Sender<UploadLimitReached>,
        id: StreamingTargetId,
    ) -> Result<()> {
        let timestamp = Timestamp::now();
        let uri = format!(
            "{}/services/recording/upload?room_id={room_id}&file_extension={}&timestamp={}",
            settings
                .v1_api_base_url()
                .replace("https", "wss")
                .replace("http", "ws"),
            file_extension.str(),
            urlencoding::encode(&timestamp.to_string()),
        );

        log::debug!("connect websocket to {uri}");
        let ws_stream = if let Ok((ws_stream, _response)) =
            self.websocket_connect(uri.clone()).await
        {
            ws_stream
        } else {
            log::debug!("Unable to connect to the websocket, refresh access token and retry it");
            self.refresh_access_tokens(self.access_token.read().await.clone())
                .await
                .context("unable to refresh the access token")?;

            self.websocket_connect(uri).await?.0
        };
        let (mut tx, mut rx) = ws_stream.split();

        let mut last_pong = Instant::now();

        let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(15));
        loop {
            tokio::select! {
                Some(message) = rx.next() => {
                    log::trace!("received message {message:?}");
                    match message {
                        Ok(Message::Ping(data)) => tx.send(Message::Pong(data)).await.unwrap(),
                        Ok(Message::Pong(msg)) => {
                            if msg == b"heartbeat"[..] {
                                last_pong = Instant::now();
                            }
                        }
                        Ok(_) => {}
                        Err(err) => log::error!("Received websocket error for upload stream: {err:?}"),
                    }
                }
                _ = heartbeat_interval.tick() => {
                    if Instant::now().duration_since(last_pong) > Duration::from_secs(20) {
                        tx.close().await?;
                        bail!("Upload canceled, there was no websocket heartbeat within 15 seconds.");
                    }

                    tx.send(tt::tungstenite::Message::Ping("heartbeat".as_bytes().to_owned()))
                        .await
                        .context("Data could not be send to the websocket")?;
                }
                result = receiver.recv() => {
                    let Ok(bytes) = result else {
                        break;
                    };

                    let part_num = u32::from_be_bytes(bytes[..4].try_into().unwrap_or_default());
                    tx.send(tt::tungstenite::Message::Binary(bytes))
                        .await
                        .context("Data could not be send to the websocket")?;

                    // Limit the chunk count to the fixed maximum chunk amount
                    // by S3, it's defined to be 1000, but we need *some buffer* since we cannot
                    // perfectly control it to stop at exactly the upper limit
                    // so currently, the CHUNK_LIMIT is set to 950 to have some wiggle-room.
                    if part_num > CHUNK_LIMIT {
                        let limit_reached = UploadLimitReached { id };
                        sender.send(limit_reached)?;
                    }
                }
            }
        }
        Ok(())
    }

    async fn websocket_connect(
        &self,
        uri: String,
    ) -> Result<(
        WebSocketStream<tt::MaybeTlsStream<tokio::net::TcpStream>>,
        openidconnect::http::Response<std::option::Option<Vec<u8>>>,
    )> {
        let token = {
            let l = self.access_token.read().await;
            l.clone()
        };

        let mut request = uri.into_client_request().unwrap();
        request.headers_mut().insert(
            reqwest::header::AUTHORIZATION,
            HeaderValue::from_str(format!("Bearer {}", token.secret()).as_str())
                .context("HeaderValue is not valid")?,
        );

        tt::connect_async(request).await.map_err(Into::into)
    }
}

/// Error returned by the `start` function when the given digits were incorrect
#[derive(Debug, thiserror::Error)]
#[error("given credentials were invalid")]
pub(crate) struct InvalidCredentials;

#[derive(Serialize)]
struct StartRequest<'s> {
    room_id: &'s str,
    breakout_room: Option<&'s str>,
}

#[derive(Deserialize)]
struct ApiError {
    code: String,
}

#[derive(Deserialize)]
struct StartResponse {
    ticket: String,
}

type BoxedHttpResponseFuture =
    Box<dyn Future<Output = Result<HttpResponse, Error<reqwest::Error>>> + Send>;

fn async_http_client(
    client: reqwest::Client,
) -> impl Fn(HttpRequest) -> Pin<BoxedHttpResponseFuture> {
    move |request| Box::pin(async_http_client_inner(client.clone(), request))
}

async fn async_http_client_inner(
    client: reqwest::Client,
    request: HttpRequest,
) -> Result<HttpResponse, Error<reqwest::Error>> {
    let mut request_builder = client
        .request(request.method, request.url.as_str())
        .body(request.body);
    for (name, value) in &request.headers {
        request_builder = request_builder.header(name.as_str(), value.as_bytes());
    }
    let request = request_builder.build().map_err(Error::Reqwest)?;

    let response = client.execute(request).await.map_err(Error::Reqwest)?;

    let status_code = response.status();
    let headers = response.headers().clone();
    let chunks = response.bytes().await.map_err(Error::Reqwest)?;
    Ok(HttpResponse {
        status_code,
        headers,
        body: chunks.to_vec(),
    })
}
