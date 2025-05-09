// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use std::{fmt::Display, net::IpAddr, str::FromStr};

use anyhow::{bail, Context, Result};
use compositor::{ClockFormat, EncoderType};
use config::{Config, Environment, File, FileFormat, FileSourceFile};
use lapin::uri::AMQPUri;
use openidconnect::{ClientId, ClientSecret, IssuerUrl};
use serde::{Deserialize, Deserializer};

const S3_MINIMUM_CHUNK_SIZE: usize = 5 * 1024 * 1024;
const S3_MAXIMUM_CHUNK_SIZE: usize = S3_MINIMUM_CHUNK_SIZE * 1024;

#[derive(Debug, Deserialize)]
pub(crate) struct Settings {
    pub(crate) auth: AuthSettings,
    pub(crate) controller: ControllerSettings,
    pub(crate) monitoring: Option<MonitoringSettings>,
    pub(crate) rabbitmq: RabbitMqSettings,
    pub(crate) recorder: Option<RecorderSettings>,
}

fn discover_config_file(
    config_arg_path: Option<&String>,
) -> Result<File<FileSourceFile, FileFormat>> {
    use std::fs::exists;

    if let Some(path) = config_arg_path {
        return Ok(File::new(path, FileFormat::Toml));
    }

    if exists("recorder.toml").unwrap_or_default() {
        return Ok(File::new("recorder.toml", FileFormat::Toml));
    }

    if let Some(dirs) = directories::BaseDirs::new() {
        let config_path = dirs.config_dir().join("recorder.toml");

        if exists(&config_path).unwrap_or_default() {
            let name = config_path
                .to_str()
                .context("Failed to convert config path to UTF8")?;

            return Ok(File::new(name, FileFormat::Toml));
        }
    }

    if exists("/etc/opentalk/recorder.toml").unwrap_or_default() {
        return Ok(File::new("/etc/opentalk/recorder.toml", FileFormat::Toml));
    }

    bail!("Failed to find a configuration file");
}

impl Settings {
    pub(crate) fn load(config_arg_path: Option<&String>) -> Result<Self> {
        Config::builder()
            .add_source(discover_config_file(config_arg_path)?)
            .add_source(Environment::with_prefix("OPENTALK_REC").separator("__"))
            .build()?
            .try_deserialize()
            .context("Failed to parse configuration file")
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
    #[serde(deserialize_with = "clamp_chunk_size", default = "default_chunk_size")]
    pub(crate) upload_chunk_size: usize,
}

fn default_chunk_size() -> usize {
    S3_MINIMUM_CHUNK_SIZE
}

fn clamp_chunk_size<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    let value = usize::deserialize(deserializer)?;
    let clamped = value.clamp(S3_MINIMUM_CHUNK_SIZE, S3_MAXIMUM_CHUNK_SIZE);

    if value != clamped {
        log::warn!("Chunk size is {value}, expected chunk size to be inbetween {S3_MINIMUM_CHUNK_SIZE} and {S3_MAXIMUM_CHUNK_SIZE}");
    }

    Ok(clamped)
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
    #[serde(default = "default_http_address")]
    pub(crate) addr: IpAddr,
}

fn default_http_port() -> u16 {
    11411
}

fn default_http_address() -> IpAddr {
    [0, 0, 0, 0].into()
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
