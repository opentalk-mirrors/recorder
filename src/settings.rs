// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use std::{fmt::Display, str::FromStr};

use compositor::{ClockFormat, EncoderType};
use config::{Config, ConfigError, Environment, File, FileFormat};
use lapin::uri::AMQPUri;
use openidconnect::{ClientId, ClientSecret, IssuerUrl};
use serde::{Deserialize, Deserializer};

#[derive(Debug, Deserialize)]
pub(crate) struct Settings {
    pub(crate) auth: AuthSettings,
    pub(crate) controller: ControllerSettings,
    pub(crate) monitoring: Option<MonitoringSettings>,
    pub(crate) rabbitmq: RabbitMqSettings,
    pub(crate) recorder: Option<RecorderSettings>,
}

impl Settings {
    pub(crate) fn load(file_name: &str) -> Result<Self, ConfigError> {
        Config::builder()
            .add_source(File::new(file_name, FileFormat::Toml))
            .add_source(Environment::with_prefix("OPENTALK_REC").separator("__"))
            .build()?
            .try_deserialize()
    }

    #[must_use]
    pub(crate) fn encoder_type(&self) -> EncoderType {
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
pub(crate) struct AuthSettings {
    pub(crate) issuer: IssuerUrl,
    pub(crate) client_id: ClientId,
    pub(crate) client_secret: ClientSecret,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ControllerSettings {
    pub(crate) domain: String,
    #[serde(default)]
    pub(crate) insecure: bool,
}

impl ControllerSettings {
    #[must_use]
    pub(crate) fn websocket_url(&self) -> String {
        let scheme = if self.insecure { "ws" } else { "wss" };

        format!("{scheme}://{}/signaling", self.domain)
    }

    #[must_use]
    pub(crate) fn v1_api_base_url(&self) -> String {
        let scheme = if self.insecure { "http" } else { "https" };

        format!("{scheme}://{}/v1", self.domain)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct MonitoringSettings {
    #[serde(default = "default_http_port")]
    pub(crate) port: u16,
}

impl Default for MonitoringSettings {
    fn default() -> Self {
        Self {
            port: default_http_port(),
        }
    }
}

fn default_http_port() -> u16 {
    11411
}

#[derive(Debug, Deserialize)]
pub(crate) struct RabbitMqSettings {
    #[serde(deserialize_with = "from_str")]
    pub(crate) uri: AMQPUri,
    pub(crate) queue: String,
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

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct RecorderSettings {
    pub(crate) clock_format: ClockFormat,
    pub(crate) display: bool,
    // Sets the default value when max_load is not present in the config.toml to
    // the return value of the function `default_max_load`
    #[serde(default = "default_max_load")]
    pub(crate) max_load: u8,
    pub(crate) hardware_acceleration: Option<HardwareAcceleration>,
}

#[must_use]
pub(crate) const fn default_max_load() -> u8 {
    80
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "manufacturer")]
#[serde(rename_all = "lowercase")]
pub(crate) enum HardwareAcceleration {
    Intel(HardwareAccelerationIntel),
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct HardwareAccelerationIntel {
    pub(crate) device: Option<String>,
}
