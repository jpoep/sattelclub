use std::process::exit;

use clap::Parser;

use crate::{
    config::{CONFIG_FILE_NAME, Config, ConfigError, get_first_config_dir},
    request::{SignupRequest, StravaClient, scrape_upcoming_events},
    state::{SignupState, State},
};

pub mod config;
pub mod request;
pub mod state;

static DEFAULT_CONFIG_FILE: &str = include_str!("config/sattelclub.default.toml");

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(name = "sattelclub", about = "Auto sign-up for Strava club rides")]
struct Cli {
    /// Print the raw HTML of the club page and exit (useful for debugging
    /// the event scraping selectors without running the full signup loop).
    #[arg(long)]
    dump_html: bool,

    /// Override the club ID from the config file (useful for one-off checks).
    #[arg(long)]
    club_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let config = Config::from_config_file();
    let config = unwrap_or_exit(config);

    let club_id = cli
        .club_id
        .clone()
        .unwrap_or_else(|| config.strava.club_id.clone());

    if club_id.is_empty() {
        eprintln!(
            "Error: strava.clubId is not set in sattelclub.toml.\n\
             Please add the numeric Strava club ID (visible in the club URL)."
        );
        exit(1);
    }

    // Load and validate the session.
    let cache = match config.load_session_cache() {
        Ok(c) => c,
        Err(ConfigError::IoError(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!(
                "tokens.toml not found. Please create it alongside sattelclub.toml and paste \
                 your _strava4_session cookie value into the `sessionCookie` field.\n\
                 See the comment in the default tokens.toml for instructions."
            );
            exit(1);
        }
        Err(e) => {
            print_config_error(e, "tokens.toml");
            exit(1);
        }
    };

    if cache.session_cookie.is_empty() {
        eprintln!(
            "sessionCookie is empty in tokens.toml.\n\
             Please paste your _strava4_session cookie value there.\n\
             How: DevTools → Application → Cookies → https://www.strava.com → _strava4_session"
        );
        exit(1);
    }

    let client = match StravaClient::new(&cache) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create HTTP client: {e}");
            exit(1);
        }
    };

    // Validate session and fetch club HTML (we need it for both --dump-html
    // and the normal run loop).
    println!("Validating session by fetching club page for club '{club_id}'…");
    let club_html = match client.validate_session(&club_id).await {
        Ok(html) => html,
        Err(e) => {
            eprintln!("Error: {e}");
            exit(1);
        }
    };
    println!("Session is valid.");

    // --dump-html: print the raw HTML and exit.
    if cli.dump_html {
        println!("{club_html}");
        return;
    }

    // Extract CSRF token for POST requests.
    let csrf_token = match StravaClient::extract_csrf_token(&club_html) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Warning: could not extract CSRF token: {e}");
            eprintln!("POSTing without CSRF token — this will likely fail.");
            String::new()
        }
    };

    // Scrape upcoming events.
    let events = scrape_upcoming_events(&club_html);

    if events.is_empty() {
        println!("No upcoming events found on the club page. Nothing to do.");
        return;
    }

    // For now, target the first upcoming event (the next ride).
    let event = events.into_iter().next().unwrap();
    println!("Found event: '{}' ({})", event.title, event.join_path);

    // Build a signup request for every enabled user.
    let enabled_users: Vec<_> = config.users.iter().filter(|u| u.enabled).collect();

    if enabled_users.is_empty() {
        println!("No enabled users in config. Nothing to do.");
        return;
    }

    let requests: Vec<SignupRequest> = enabled_users
        .iter()
        .map(|u| SignupRequest::for_event(&config, u, event.clone()))
        .collect();

    for request in &requests {
        println!(
            "Will sign up {} for ride on {}",
            request.user.name(),
            request.date
        );
    }

    // Run the signup loop.
    for request in &requests {
        println!(
            "Requesting signup for ride on {}: {}",
            request.date,
            request.user.name()
        );

        let mut state = SignupState::new(request.user.clone());

        while matches!(state.state, State::Pending) {
            tokio::time::sleep(config.checking_interval).await;

            let response = request.make_request(&client, &csrf_token).await;
            state.apply_result(response);

            match &state.state {
                State::Done(reason) => match reason {
                    state::Reason::Success => {
                        println!(
                            "Successfully signed up {} for ride on {}",
                            request.user.name(),
                            request.date
                        );
                    }
                    state::Reason::Full => {
                        println!(
                            "Ride is full, could not sign up {} for ride on {}",
                            request.user.name(),
                            request.date
                        );
                    }
                    state::Reason::Error(e) => {
                        eprintln!("Error signing up {}: {}", request.user.name(), e);
                    }
                },
                State::Pending => {
                    println!(
                        "Still pending signup for {} on {} — retrying in {:?}",
                        request.user.name(),
                        request.date,
                        config.checking_interval,
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn print_config_error(e: ConfigError, file: &str) {
    match e {
        ConfigError::ParseError(e) => {
            eprintln!("{file} is malformed: {e}");
        }
        ConfigError::IoError(e) => {
            eprintln!("Error reading {file}: {e}");
        }
    }
}

fn unwrap_or_exit<T>(result: Result<T, ConfigError>) -> T {
    match result {
        Ok(v) => v,
        Err(e) => {
            let io_err = match e {
                ConfigError::ParseError(e) => {
                    eprintln!("Configuration file is malformed. {e}");
                    exit(1);
                }
                ConfigError::IoError(e) => e,
            };
            let config_dir = get_first_config_dir();
            eprintln!("Error loading configuration: {io_err}");
            eprintln!(
                "Generating default configuration to {}",
                config_dir.display()
            );
            match std::fs::create_dir_all(&config_dir) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Failed to create config directory: {e}");
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
                    eprintln!("Failed to write default configuration: {e}");
                    exit(1);
                }
            }
            exit(1)
        }
    }
}
