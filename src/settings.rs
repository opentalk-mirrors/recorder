// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use std::{fmt::Display, str::FromStr};

use compositor::{ClockFormat, EncoderType, RTMPParameters, WebMParameters};
use config::{Config, ConfigError, Environment, File, FileFormat};
use lapin::uri::AMQPUri;
use openidconnect::{ClientId, ClientSecret, IssuerUrl};
use serde::{Deserialize, Deserializer};

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RecorderSettings {
    pub clock_format: ClockFormat,
    pub sinks: Vec<RecorderSink>,
    // Sets the default value when max_load is not present in the config.toml to
    // the return value of the function `default_max_load`
    #[serde(default = "default_max_load")]
    pub max_load: u8,
    pub hardware_acceleration: Option<HardwareAcceleration>,
}

#[must_use]
pub const fn default_max_load() -> u8 {
    80
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
pub enum RecorderSink {
    Display,
    WebM(WebMParameters),
    Rtmp(RTMPParameters),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "manufacturer")]
#[serde(rename_all = "lowercase")]
pub enum HardwareAcceleration {
    Intel(HardwareAccelerationIntel),
}

#[derive(Clone, Debug, Deserialize)]
pub struct HardwareAccelerationIntel {
    pub device: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub auth: AuthSettings,
    pub controller: ControllerSettings,
    pub rabbitmq: RabbitMqSettings,
    pub recorder: Option<RecorderSettings>,
}

impl Settings {
    pub fn load(file_name: &str) -> Result<Self, ConfigError> {
        Config::builder()
            .add_source(File::new(file_name, FileFormat::Toml))
            .add_source(Environment::with_prefix("OPENTALK_REC").separator("__"))
            .build()?
            .try_deserialize()
    }

    #[must_use]
    pub fn encoder_type(&self) -> EncoderType {
        self.recorder
            .as_ref()
            .and_then(|settings| settings.hardware_acceleration.as_ref())
            .map_or(
                EncoderType::CPU,
                |hardware_acceleration| match hardware_acceleration {
                    HardwareAcceleration::Intel(_) => EncoderType::VAAPI,
                },
            )
    }
}

#[derive(Debug, Deserialize)]
pub struct AuthSettings {
    pub issuer: IssuerUrl,
    pub client_id: ClientId,
    pub client_secret: ClientSecret,
}

#[derive(Debug, Deserialize)]
pub struct ControllerSettings {
    pub domain: String,
    #[serde(default)]
    pub insecure: bool,
}

impl ControllerSettings {
    #[must_use]
    pub fn websocket_url(&self) -> String {
        let scheme = if self.insecure { "ws" } else { "wss" };

        format!("{scheme}://{}/signaling", self.domain)
    }

    #[must_use]
    pub fn v1_api_base_url(&self) -> String {
        let scheme = if self.insecure { "http" } else { "https" };

        format!("{scheme}://{}/v1", self.domain)
    }
}

#[derive(Debug, Deserialize)]
pub struct RabbitMqSettings {
    #[serde(deserialize_with = "from_str")]
    pub uri: AMQPUri,
    pub queue: String,
}

fn from_str<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: FromStr,
    T::Err: Display,
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    FromStr::from_str(&s).map_err(serde::de::Error::custom)
}
