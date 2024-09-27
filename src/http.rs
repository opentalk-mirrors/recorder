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
use types_common::time::Timestamp;

use crate::settings::{AuthSettings, ControllerSettings};

const S3_MINIMUM_STORAGE_SIZE: usize = 5_000_000;

// TODO: Replace with version from opentalk-types
#[derive(Clone)]
pub struct FileExtension(String);

impl FileExtension {
    #[must_use]
    pub fn webm() -> Self {
        Self("webm".to_string())
    }

    #[must_use]
    pub fn str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug)]
pub struct HttpClient {
    client: reqwest::Client,
    oidc: openidconnect::core::CoreClient,
    access_token: RwLock<AccessToken>,
}

impl HttpClient {
    /// This constructor is used by the integration tests to mock data.
    #[allow(dead_code)]
    pub fn new(
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

    pub async fn discover(settings: &AuthSettings) -> Result<Self> {
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

    pub async fn start(
        &self,
        settings: &ControllerSettings,
        room_id: &str,
        breakout_room: &Option<String>,
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

    pub async fn upload_render(
        &self,
        settings: &ControllerSettings,
        room_id: &str,
        file_extension: FileExtension,
        mut receiver: broadcast::Receiver<Vec<u8>>,
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
        let mut latest_data = Vec::<u8>::new();

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

                    // Only the latest data should be the smallest chunk in
                    // S3, this helps so send the fanilized first chunk and afterwards send the small
                    // chunk.
                    if bytes.len() < S3_MINIMUM_STORAGE_SIZE{
                        latest_data = bytes;
                    } else {
                        tx.send(tt::tungstenite::Message::Binary(bytes))
                            .await
                            .context("Data could not be send to the websocket")?;
                    }
                }
            }
        }

        // Send the smallest chunk afterwards, otherwise S3 will reject it.
        if !latest_data.is_empty() {
            tx.send(tt::tungstenite::Message::Binary(latest_data))
                .await
                .context("Data could not be send to the websocket")?;
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
pub struct InvalidCredentials;

#[derive(Serialize)]
struct StartRequest<'s> {
    room_id: &'s str,
    breakout_room: &'s Option<String>,
}

#[derive(Deserialize)]
struct ApiError {
    code: String,
}

#[derive(Deserialize)]
struct StartResponse {
    ticket: String,
}

fn async_http_client(
    client: reqwest::Client,
) -> impl Fn(
    HttpRequest,
) -> Pin<Box<dyn Future<Output = Result<HttpResponse, Error<reqwest::Error>>> + Send>> {
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
