use std::{error::Error, time::Duration};

use chrono::{Datelike, Local, NaiveDate, NaiveTime, Weekday};
use derive_more::Display;
use serde::{Deserialize, Serialize};
use toml::from_str;

use crate::config::path::find_config_path;
pub use path::{CONFIG_FILE_NAME, get_first_config_dir};
pub use strava::{SessionCache, StravaConfig};
pub use user::User;

mod path;
pub mod strava;
mod user;

static TOKENS_FILE_NAME: &str = "tokens.toml";

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub users: Vec<User>,
    pub checking_interval: Duration,
    pub signup_weekday: Weekday,
    pub ride_weekday: Weekday,
    pub check_from: NaiveTime,
    pub strava: StravaConfig,
}

impl Config {
    pub fn from_config_file() -> Result<Self, ConfigError> {
        find_config_path()
            .map(|path| {
                let config_content = std::fs::read_to_string(path).map_err(ConfigError::IoError)?;
                from_str(&config_content).map_err(ConfigError::ParseError)
            })
            .ok_or(ConfigError::IoError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                PathError,
            )))?
    }

    /// Returns the directory that contains the config file, or falls back to
    /// the first writable config directory.
    fn config_dir() -> std::path::PathBuf {
        use crate::config::path::{find_config_path, get_first_config_dir};
        find_config_path()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(get_first_config_dir)
    }

    pub fn load_session_cache(&self) -> Result<SessionCache, ConfigError> {
        let path = Self::config_dir().join(TOKENS_FILE_NAME);
        let content = std::fs::read_to_string(&path).map_err(ConfigError::IoError)?;
        toml::from_str(&content).map_err(ConfigError::ParseError)
    }

    pub fn save_session_cache(&self, cache: &SessionCache) -> Result<(), ConfigError> {
        let path = Self::config_dir().join(TOKENS_FILE_NAME);
        let content = toml::to_string(cache).expect("SessionCache is always serialisable");
        std::fs::write(path, content).map_err(ConfigError::IoError)
    }

    /// Returns the date of the next occurrence of `ride_weekday` on or after
    /// today (local time).
    pub fn get_next_ride_date(&self) -> NaiveDate {
        let today = Local::now().date_naive();
        let mut date = today;
        while date.weekday() != self.ride_weekday {
            date = date.succ_opt().expect("date arithmetic overflowed");
        }
        date
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
        let default_config_content = include_str!("config/sattelclub.default.toml");
        let result: Result<Config, _> = from_str(default_config_content);
        assert!(
            result.is_ok(),
            "Failed to deserialize default config: {:?}",
            result.err()
        );
    }
}
