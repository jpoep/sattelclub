use std::process::exit;

use crate::config::{CONFIG_FILE_NAME, Config, ConfigError, get_first_config_dir};

pub mod config;

static DEFAULT_CONFIG_FILE: &str = include_str!("config/sattelclub.default.toml");

fn main() {
    let config = Config::from_config_file();
    let config = unwrap_or_exit(config);
    
}

fn unwrap_or_exit<T>(config: Result<T, ConfigError>) -> T {
    match config {
        Ok(config) => config,
        Err(e) => {
            let e = match e {
                ConfigError::ParseError(e) => {
                    eprintln!("Configuration file is malformed. {e}");
                    exit(1);
                }
                ConfigError::IoError(e) => e,
            };
            let config_dir = get_first_config_dir();
            eprintln!("Error loading configuration: {}", e);
            eprintln!(
                "Generating default configuration to {}",
                config_dir.display()
            );
            match std::fs::create_dir_all(&config_dir) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Failed to create config directory: {}", e);
                    exit(1);
                }
            }
            let config_file_path = config_dir.join(CONFIG_FILE_NAME);
            match std::fs::write(&config_file_path, DEFAULT_CONFIG_FILE) {
                Ok(_) => {
                    eprintln!(
                        "Default configuration written to {}",
                        config_file_path.display()
                    );
                }
                Err(e) => {
                    eprintln!("Failed to write default configuration: {}", e);
                    exit(1);
                }
            }
            exit(1)
        }
    }
}
