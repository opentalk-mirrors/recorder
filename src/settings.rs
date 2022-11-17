use std::{fmt::Display, str::FromStr};

use config::{Config, ConfigError, Environment, File, FileFormat};
use lapin::uri::AMQPUri;
use openidconnect::{ClientId, ClientSecret, IssuerUrl};
use serde::{Deserialize, Deserializer};

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub auth: AuthSettings,
    pub controller: ControllerSettings,
    pub rabbitmq: RabbitMqSettings,
}

impl Settings {
    pub fn load(file_name: &str) -> Result<Self, ConfigError> {
        Config::builder()
            .add_source(File::new(file_name, FileFormat::Toml))
            .add_source(Environment::with_prefix("K3K_REC").separator("__"))
            .build()?
            .try_deserialize()
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
    pub fn websocket_url(&self) -> String {
        let scheme = if self.insecure { "ws" } else { "wss" };

        format!("{scheme}://{}/signaling", self.domain)
    }

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
