pub mod groupride_response;

use std::{collections::HashMap, error::Error};

use chrono::{Datelike, NaiveDate};
use derive_more::Display;
use reqwest::Client;

use crate::{
    config::{Config, User},
    request::groupride_response::{GrouprideErrorResponse, GrouprideResponse},
};

#[derive(Display)]
pub enum SignupError {
    ErrorResponse(GrouprideErrorResponse),
    Unknown(Box<dyn Error>),
}

impl SignupError {
    fn error_response_from_message(message: String) -> Self {
        SignupError::ErrorResponse(GrouprideErrorResponse::from_message(message))
    }
}

#[derive(Debug, Display)]
pub struct UnknownResponseError(pub String);

impl Error for UnknownResponseError {}

pub type SignupResponse = Result<(), SignupError>;

pub struct SignupRequest {
    pub url: String,
    pub ride_id: String,
    pub date: NaiveDate,
    pub user: User,
}

impl Config {
    fn get_next_ride_date(&self) -> NaiveDate {
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
                url: config.base_url.clone(),
                ride_id: config.ride_id.clone(),
                user: user.clone(),
                date: config.get_next_ride_date(),
            })
            .collect()
    }

    pub async fn make_request(&self) -> SignupResponse {
        let client = Client::new();

        let response = client.post(&self.url).form(&self.form_data()).send().await;
        let response = response.map_err(|e| SignupError::Unknown(Box::new(e)))?;
        let response = response
            .error_for_status()
            .map_err(|e| SignupError::Unknown(Box::new(e)))?;

        let text = response
            .text()
            .await
            .map_err(|e| SignupError::Unknown(Box::new(e)))?;

        let groupride_response: GrouprideResponse = serde_json::from_str(&text)
            .map_err(|_| SignupError::Unknown(Box::new(UnknownResponseError(text.to_string()))))?;

        if let Some(error) = groupride_response.error {
            Err(SignupError::error_response_from_message(error))
        } else {
            Ok(())
        }
    }

    fn form_data(&self) -> HashMap<String, String> {
        let mut form_data = HashMap::new();
        form_data.insert("firstName".to_string(), self.user.first_name.clone());
        form_data.insert("lastName".to_string(), self.user.surname.clone());
        form_data.insert("email".to_string(), self.user.email.clone());
        form_data.insert("slug".to_string(), self.slug());
        form_data.insert("termsCheckbox".to_string(), "on".to_string());
        form_data
    }

    fn slug(&self) -> String {
        let id = self.ride_id.clone();
        let date = self.date.format("%Y-%m-%d").to_string();
        format!("{id}-{date}")
    }
}
