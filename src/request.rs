use std::error::Error;

use chrono::{Datelike, NaiveDate};
use derive_more::Display;

use crate::config::{Config, User};

#[derive(Display)]
pub enum SignupError {
    Full,
    NotYetOpen,
    AlreadySignedUp,
    Unknown(Box<dyn Error>),
}

pub type SignupResponse = Result<(), SignupError>;

pub struct SignupRequest {
    pub date: NaiveDate,
    pub user: User,
}

impl Config {
    fn get_next_ride_date(&self) -> chrono::NaiveDate {
        let today = chrono::Local::now().date_naive();
        let mut next_ride_date = today;

        while next_ride_date.weekday() != self.ride_weekday {
            next_ride_date = next_ride_date.succ_opt().expect("Invalid date");
        }

        next_ride_date
    }
}

impl SignupRequest {
    pub fn from_config(config: &Config) -> Vec<Self> {
        config
            .users
            .iter()
            .map(|user| SignupRequest {
                user: user.clone(),
                date: config.get_next_ride_date(),
            })
            .collect()
    }

    pub async fn make_request(&self) -> SignupResponse {
        // TODO: implement Strava signup request
        todo!("Strava signup not yet implemented")
    }
}
