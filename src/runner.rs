use std::time::Duration;

use crate::config::Config;
use crate::strava::client::StravaClient;
use crate::strava::scraper::{EventButtonState, find_next_event};
use crate::strava::signup::{SignupError, SignupRequest};

/// Outcome of a single run (one pass through the club page).
pub enum RunOutcome {
    /// No event card found on the page — nothing to do.
    NoEvent,
    /// Event exists but signup window is not yet open.
    NotYetOpen,
    /// Every enabled user was signed up (or was already signed up).
    Done,
}

/// Top-level run loop. Called once per checking interval tick.
///
/// 1. Fetches the club page and validates the session.
/// 2. Scrapes the event card to determine button state.
/// 3. For each enabled user, attempts to RSVP.
///
/// Returns early with a descriptive [`RunOutcome`] when there is nothing to
/// do, so `main` stays completely free of business logic.
pub async fn run(config: &Config, client: &StravaClient) -> anyhow::Result<RunOutcome> {
    let club_id = &config.strava.club_id;

    let html = client.fetch_club_page(club_id).await?;

    let csrf_token = match StravaClient::extract_csrf_token(&html) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Warning: could not extract CSRF token: {e}");
            eprintln!("Continuing without it — the RSVP will likely be rejected.");
            String::new()
        }
    };

    let event = match find_next_event(&html) {
        None => return Ok(RunOutcome::NoEvent),
        Some(e) => e,
    };

    match event.button {
        EventButtonState::Full => {
            println!("Event {} is at capacity.", event.event_id);
            return Ok(RunOutcome::Done);
        }
        EventButtonState::AlreadySignedUp => {
            println!("Already signed up for event {}.", event.event_id);
            return Ok(RunOutcome::Done);
        }
        EventButtonState::Open => {
            println!("Event {} is open for signup.", event.event_id);
        }
    }

    let enabled_users: Vec<_> = config.users.iter().filter(|u| u.enabled).collect();

    if enabled_users.is_empty() {
        println!("No enabled users in config — nothing to sign up.");
        return Ok(RunOutcome::Done);
    }

    for user in enabled_users {
        let request = SignupRequest::new(config, user, event.clone());
        sign_up_user(&request, client, &csrf_token, config.checking_interval).await;
    }

    Ok(RunOutcome::Done)
}

/// Drive the retry loop for a single user until the signup either succeeds or
/// reaches a terminal state.
async fn sign_up_user(
    request: &SignupRequest,
    client: &StravaClient,
    csrf_token: &str,
    retry_interval: Duration,
) {
    let name = request.user.name();
    let date = request.date;

    println!("Signing up {name} for ride on {date}…");

    loop {
        match request.execute(client, csrf_token).await {
            Ok(()) => {
                println!("✓ {name} signed up for ride on {date}.");
                return;
            }

            Err(SignupError::AlreadySignedUp) => {
                println!("✓ {name} is already signed up for ride on {date}.");
                return;
            }

            Err(SignupError::Full) => {
                println!("✗ Ride on {date} is full — could not sign up {name}.");
                return;
            }

            Err(SignupError::NotYetOpen) => {
                println!(
                    "  Signup not yet open for {name} on {date} — \
                     retrying in {retry_interval:?}…"
                );
                tokio::time::sleep(retry_interval).await;
            }

            Err(SignupError::Unknown(e)) => {
                eprintln!("✗ Error signing up {name}: {e}");
                return;
            }
        }
    }
}
