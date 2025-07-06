use std::error::Error;

use derive_more::Display;

use crate::{
    config::User,
    request::{SignupError, SignupResponse, groupride_response::GrouprideErrorResponse},
};

pub struct SignupState {
    pub user: User,
    pub state: State,
}

#[derive(Debug, Display)]
struct StringError(String);

impl Error for StringError {}

impl SignupState {
    pub fn new(user: User) -> Self {
        SignupState {
            user,
            state: State::Pending,
        }
    }

    pub fn apply_result(&mut self, result: SignupResponse) {
        self.state = match result {
            Ok(_) => State::Done(Reason::Success),
            Err(error) => match error {
                SignupError::ErrorResponse(groupride_error_response) => {
                    match groupride_error_response {
                        GrouprideErrorResponse::RideFull => State::Done(Reason::Full),
                        GrouprideErrorResponse::AlreadySignedUp => State::Done(Reason::Success),
                        GrouprideErrorResponse::RideNotFound => State::Pending,
                        GrouprideErrorResponse::UnknownError(e) => {
                            State::Done(Reason::Error(Box::new(StringError(e))))
                        }
                    }
                }
                SignupError::Unknown(error) => State::Done(Reason::Error(error)),
            },
        };
    }
}

pub enum State {
    Pending,
    Done(Reason),
}

pub enum Reason {
    Success,
    Full,
    Error(Box<dyn Error>),
}
