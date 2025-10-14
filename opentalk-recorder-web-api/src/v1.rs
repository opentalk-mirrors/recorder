// SPDX-License-Identifier: EUPL-1.2
// SPDX-FileCopyrightText: OpenTalk Team <mail@opentalk.eu>
use std::fmt::Debug;

use async_trait::async_trait;
use axum::{
    extract::{self, State},
    http::StatusCode,
    routing::post,
    Json,
};
use opentalk_client::types::common::rooms::BreakoutRoomId;
use serde::Deserialize;

use super::Router;

pub trait Backend: Send + Sync + Clone + Sized {}

const API_VERSION: &str = "/v1";

#[derive(Debug, Deserialize, Clone)]
pub struct InitializeRecording {
    pub room: String,
    pub breakout: Option<BreakoutRoomId>,
}

pub enum RecorderAction {
    Init,
}

#[async_trait]
pub trait RecorderBackend: Clone + Send + Sync {
    async fn init(&self, recording: InitializeRecording) -> (StatusCode, Json<String>);
}

// TODO: This should be refactored with the https://git.opentalk.dev/opentalk/backend/services/controller/-/issues/1136
async fn init<B: RecorderBackend>(
    State(ctx): State<B>,
    extract::Json(recording): extract::Json<InitializeRecording>,
) -> (axum::http::StatusCode, axum::Json<std::string::String>) {
    ctx.init(recording).await
}

pub fn routes<B: RecorderBackend + 'static>() -> Router<B> {
    Router::<B>::new().nest(API_VERSION, Router::new().route("/init", post(init::<B>)))
}
