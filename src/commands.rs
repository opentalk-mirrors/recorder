//! Commands this recorder receives via RabbitMQ

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct StartRecording {
    pub room: String,
    pub breakout: Option<String>,
}
