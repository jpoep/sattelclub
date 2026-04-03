use std::error::Error;

use chrono::NaiveDate;
use derive_more::Display;

use crate::config::{Config, User};
use crate::strava::client::StravaClient;
use crate::strava::scraper::StravaEvent;

// ---------------------------------------------------------------------------
// Error / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Display)]
pub enum SignupError {
    /// The signup window is not yet open — caller should retry later.
    NotYetOpen,
    /// The ride is at capacity.
    Full,
    /// The user is already signed up.
    AlreadySignedUp,
    /// Anything else unexpected.
    Unknown(Box<dyn Error>),
}

pub type SignupResult = Result<(), SignupError>;

// ---------------------------------------------------------------------------
// SignupRequest
// ---------------------------------------------------------------------------

/// A pending signup for one user for one event.
pub struct SignupRequest {
    /// The date of the target ride, for logging.
    pub date: NaiveDate,
    pub user: User,
    pub event: StravaEvent,
}

impl SignupRequest {
    pub fn new(config: &Config, user: &User, event: StravaEvent) -> Self {
        SignupRequest {
            date: config.get_next_ride_date(),
            user: user.clone(),
            event,
        }
    }

    /// Attempt to RSVP for this event and interpret the response.
    pub async fn execute(&self, client: &StravaClient, csrf_token: &str) -> SignupResult {
        let result = client
            .put_rsvp(&self.event.club_id, &self.event.event_id, csrf_token)
            .await;

        let (status, body) = match result {
            Err(e) => return Err(SignupError::Unknown(e.into())),
            Ok(pair) => pair,
        };

        // A redirect means the signup window is not yet open.
        if status.is_redirection() {
            return Err(SignupError::NotYetOpen);
        }

        if status.is_success() {
            return Ok(());
        }

        // Interpret non-success bodies for a more specific error.
        let body_lc = body.to_lowercase();

        if body_lc.contains("capacity") || body_lc.contains("full") {
            return Err(SignupError::Full);
        }

        if body_lc.contains("already") {
            return Err(SignupError::AlreadySignedUp);
        }

        Err(SignupError::Unknown(
            format!("HTTP {status}: {body}").into(),
        ))
    }
}
