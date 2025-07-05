use std::{error::Error, time::Duration};

use chrono::{NaiveTime, Weekday};
use derive_more::Display;
use serde::{Deserialize, Serialize};
use toml::from_str;

use crate::config::{path::find_config_path, user::User};
pub use path::{CONFIG_FILE_NAME, get_first_config_dir};

mod path;
mod user;

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub users: Vec<User>,
    pub base_url: String,
    pub ride_id: String,
    pub checking_interval: Duration,
    pub signup_weekday: Weekday,
    pub ride_weekday: Weekday,
    pub check_from: NaiveTime,
}

impl Config {
    pub fn from_config_file() -> Result<Self, ConfigError> {
        find_config_path()
            .map(|path| {
                let config_content =
                    std::fs::read_to_string(path).map_err(|e| ConfigError::IoError(e))?;
                from_str(&config_content).map_err(|e| ConfigError::ParseError(e))
            })
            .ok_or(ConfigError::IoError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                PathError,
            )))?
    }
}

pub enum ConfigError {
    IoError(std::io::Error),
    ParseError(toml::de::Error),
}

#[derive(Debug, Display)]
struct PathError;

impl Error for PathError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_deserializable() {
        // This test ensures that the default TOML config file, which serves as a template
        // for users, is always valid and can be deserialized into the Config struct.
        // The path is relative to this file (sattelclub/src/config.rs).
        let default_config_content = include_str!("config/sattelclub.default.toml");
        let result: Result<Config, _> = from_str(default_config_content);
        assert!(
            result.is_ok(),
            "Failed to deserialize default config: {:?}",
            result.err()
        );
    }
}
