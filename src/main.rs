use std::process::exit;

use clap::Parser;

use crate::config::{CONFIG_FILE_NAME, Config, ConfigError, get_first_config_dir};
use crate::strava::client::StravaClient;

pub mod config;
pub mod runner;
pub mod strava;

static DEFAULT_CONFIG_FILE: &str = include_str!("config/sattelclub.default.toml");

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(name = "sattelclub", about = "Auto sign-up for Strava club rides")]
struct Cli {
    /// Print the raw HTML of the club page and exit.
    /// Useful for inspecting the page structure during development.
    #[arg(long)]
    dump_html: bool,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let config = unwrap_or_exit(Config::from_config_file());

    if config.strava.club_id.is_empty() {
        eprintln!(
            "Error: strava.clubId is not set in sattelclub.toml.\n\
             Please add the numeric Strava club ID (visible in the club URL)."
        );
        exit(1);
    }

    let cache = match config.load_session_cache() {
        Ok(c) => c,
        Err(ConfigError::IoError(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!(
                "tokens.toml not found. Please create it alongside sattelclub.toml\n\
                 and paste your _strava4_session cookie value into the `sessionCookie` field.\n\
                 How: DevTools → Application → Cookies → https://www.strava.com → _strava4_session"
            );
            exit(1);
        }
        Err(e) => {
            eprintln!("Error reading tokens.toml: {}", config_error_message(e));
            exit(1);
        }
    };

    if cache.session_cookie.is_empty() {
        eprintln!(
            "sessionCookie is empty in tokens.toml.\n\
             How: DevTools → Application → Cookies → https://www.strava.com → _strava4_session"
        );
        exit(1);
    }

    let client = match StravaClient::new(&cache) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to build HTTP client: {e}");
            exit(1);
        }
    };

    // --dump-html: fetch the club page and print raw HTML, then exit.
    if cli.dump_html {
        match client.fetch_club_page(&config.strava.club_id).await {
            Ok(html) => {
                println!("{html}");
                return;
            }
            Err(e) => {
                eprintln!("Error fetching club page: {e}");
                exit(1);
            }
        }
    }

    // Normal run: hand off entirely to the runner.
    if let Err(e) = runner::run(&config, &client).await {
        eprintln!("Fatal error: {e}");
        exit(1);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn config_error_message(e: ConfigError) -> String {
    match e {
        ConfigError::IoError(e) => e.to_string(),
        ConfigError::ParseError(e) => e.to_string(),
    }
}

fn unwrap_or_exit<T>(result: Result<T, ConfigError>) -> T {
    match result {
        Ok(v) => v,
        Err(ConfigError::ParseError(e)) => {
            eprintln!("Configuration file is malformed: {e}");
            exit(1);
        }
        Err(ConfigError::IoError(e)) => {
            let config_dir = get_first_config_dir();
            eprintln!("Could not read configuration: {e}");
            eprintln!("Writing default configuration to {}", config_dir.display());

            if let Err(e) = std::fs::create_dir_all(&config_dir) {
                eprintln!("Failed to create config directory: {e}");
                exit(1);
            }

            let config_file = config_dir.join(CONFIG_FILE_NAME);
            match std::fs::write(&config_file, DEFAULT_CONFIG_FILE) {
                Ok(_) => eprintln!("Default config written to {}", config_file.display()),
                Err(e) => eprintln!("Failed to write default config: {e}"),
            }

            exit(1);
        }
    }
}
