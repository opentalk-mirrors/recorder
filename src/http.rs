// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

//! HTTP calls made by this library (except for websockets)

use std::time::{Duration, Instant, SystemTime};

use anyhow::{bail, Context, Result};
use chrono::{serde::ts_seconds::deserialize as from_ts, DateTime, Utc};
use futures::{SinkExt, StreamExt};
use jsonwebtoken::{self, decode, DecodingKey, Validation};
use openidconnect::{
    core::{
        CoreAuthDisplay, CoreAuthPrompt, CoreClient, CoreErrorResponseType, CoreGenderClaim,
        CoreJsonWebKey, CoreJweContentEncryptionAlgorithm, CoreJwsSigningAlgorithm,
        CoreProviderMetadata, CoreRevocableToken, CoreTokenType,
    },
    AccessToken, EmptyAdditionalClaims, EmptyExtraTokenFields, EndpointNotSet, EndpointSet,
    IdTokenFields, OAuth2TokenResponse, RevocationErrorResponseType, StandardErrorResponse,
    StandardTokenIntrospectionResponse, StandardTokenResponse,
};
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

type OidcClient = openidconnect::Client<
    EmptyAdditionalClaims,
    CoreAuthDisplay,
    CoreGenderClaim,
    CoreJweContentEncryptionAlgorithm,
    CoreJsonWebKey,
    CoreAuthPrompt,
    StandardErrorResponse<CoreErrorResponseType>,
    StandardTokenResponse<
        IdTokenFields<
            EmptyAdditionalClaims,
            EmptyExtraTokenFields,
            CoreGenderClaim,
            CoreJweContentEncryptionAlgorithm,
            CoreJwsSigningAlgorithm,
        >,
        CoreTokenType,
    >,
    StandardTokenIntrospectionResponse<EmptyExtraTokenFields, CoreTokenType>,
    CoreRevocableToken,
    StandardErrorResponse<RevocationErrorResponseType>,
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointSet,
    EndpointNotSet,
>;

#[derive(Debug)]
pub(crate) struct HttpClient {
    client: reqwest::Client,
    oidc: OidcClient,
    access_token: RwLock<AccessToken>,
}

impl HttpClient {
    /// This constructor is used by the integration tests to mock data.
    #[allow(dead_code)]
    pub(crate) fn new(
        client: reqwest::Client,
        oidc: OidcClient,
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

        let metadata =
            CoreProviderMetadata::discover_async(settings.issuer.clone(), &client).await?;

        let oidc = CoreClient::new(
            settings.client_id.clone(),
            settings.issuer.clone(),
            metadata.jwks().clone(),
        )
        .set_client_secret(settings.client_secret.clone())
        .set_auth_uri(metadata.authorization_endpoint().clone())
        .set_token_uri(
            metadata
                .token_endpoint()
                .context("Missing token endpoint url in OIDC metadata")?
                .clone(),
        );

        let response = oidc
            .exchange_client_credentials()
            .request_async(&client)
            .await?;

        Ok(Self {
            client,
            oidc,
            access_token: RwLock::new(response.access_token().clone()),
        })
    }

    pub(crate) async fn start(
        &self,
        settings: &ControllerSettings,
        room_id: &str,
        breakout_room: Option<&str>,
    ) -> Result<String> {
        let uri = format!("{}/services/recording/start", settings.v1_api_base_url());

        let token = self.get_valid_access_token().await?;

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

                Ok(response.ticket)
            }
            StatusCode::UNAUTHORIZED => {
                let ApiError { code } = response.json::<ApiError>().await?;

                bail!("failed to authorize, {code:?}");
            }
            code => bail!("unexpected status code {code:?}"),
        }
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

        let token = self.get_valid_access_token().await?;

        log::debug!("connect websocket to {uri}");
        let (ws_stream, _response) = self.websocket_connect(uri.clone(), token).await?;
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

                    tx.send(tt::tungstenite::Message::Ping("heartbeat".into()))
                        .await
                        .context("Data could not be send to the websocket")?;
                }
                result = receiver.recv() => {
                    let Ok(bytes) = result else {
                        break;
                    };

                    let part_num = u32::from_be_bytes(bytes[..4].try_into().unwrap_or_default());
                    tx.send(tt::tungstenite::Message::Binary(bytes.into()))
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

    async fn get_valid_access_token(&self) -> Result<AccessToken> {
        let mut token = self.access_token.write().await;

        // Refresh access token if it's expired
        if check_if_token_is_expired(token.secret())? {
            let response = self
                .oidc
                .exchange_client_credentials()
                .request_async(&self.client)
                .await?;

            *token = response.access_token().clone();
        }

        Ok(token.clone())
    }

    async fn websocket_connect(
        &self,
        uri: String,
        token: AccessToken,
    ) -> Result<(
        WebSocketStream<tt::MaybeTlsStream<tokio::net::TcpStream>>,
        openidconnect::http::Response<std::option::Option<Vec<u8>>>,
    )> {
        let mut request = uri.into_client_request().unwrap();
        request.headers_mut().insert(
            reqwest::header::AUTHORIZATION,
            HeaderValue::from_str(format!("Bearer {}", token.secret()).as_str())
                .context("HeaderValue is not valid")?,
        );

        tt::connect_async(request).await.map_err(Into::into)
    }
}

#[derive(Deserialize)]
struct TokenClaims {
    #[serde(deserialize_with = "from_ts")]
    exp: DateTime<Utc>,
}

/// Check if the token is expired or is expiring within a minute
fn check_if_token_is_expired(token: &str) -> Result<bool> {
    let mut validation = Validation::default();
    validation.insecure_disable_signature_validation();
    validation.validate_exp = false;
    validation.validate_aud = false;

    let token = decode::<TokenClaims>(token, &DecodingKey::from_secret(&[]), &validation)?;

    let now = DateTime::<Utc>::from(SystemTime::now());

    Ok(now > token.claims.exp + Duration::from_secs(60))
}

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
