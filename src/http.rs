// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

//! HTTP calls made by this library (except for websockets)

use anyhow::{bail, Result};
use bytes::Bytes;
use futures::TryStream;
use openidconnect::{reqwest::Error, AccessToken, HttpRequest, HttpResponse, OAuth2TokenResponse};
use reqwest::{Body, StatusCode};
use serde::{Deserialize, Serialize};
use std::{future::Future, pin::Pin};
use tokio::sync::RwLock;

use crate::settings::{AuthSettings, ControllerSettings};

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

    pub async fn start(&self, settings: &ControllerSettings, room_id: &str) -> Result<String> {
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
                .json(&StartRequest { room_id })
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

    pub async fn upload_render<S>(
        &self,
        settings: &ControllerSettings,
        room_id: &str,
        stream: S,
    ) -> Result<()>
    where
        S: TryStream + Send + Sync + 'static,
        S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
        Bytes: From<S::Ok>,
    {
        // TODO: do not hardcode the filename
        let uri = format!(
            "{}/services/recording/upload_render?room_id={room_id}&filename=recording.mp4",
            settings.v1_api_base_url()
        );

        // TODO: do not refresh access-token always here. Cannot refresh on request failure since the body
        // consumes the stream. So we can only make the request once. We could solve this by having the body stream be
        // created by a callback or something.
        let token = {
            let l = self.access_token.read().await;
            l.clone()
        };

        self.refresh_access_tokens(token).await?;

        let token = {
            let l = self.access_token.read().await;
            l.clone()
        };

        let response = self
            .client
            .post(&uri)
            .bearer_auth(token.secret())
            .body(Body::wrap_stream(stream))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            bail!("unexpected status code {:?}", response.status())
        }
    }
}

/// Error returned by the `start` function when the given digits were incorrect
#[derive(Debug, thiserror::Error)]
#[error("given credentials were invalid")]
pub struct InvalidCredentials;

#[derive(Serialize)]
struct StartRequest<'s> {
    room_id: &'s str,
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
