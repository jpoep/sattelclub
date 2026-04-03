use std::error::Error;

use crate::{
    config::User,
    request::{SignupError, SignupResponse},
};

pub struct SignupState {
    pub user: User,
    pub state: State,
}

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
                SignupError::Full => State::Done(Reason::Full),
                SignupError::NotYetOpen => State::Pending,
                SignupError::AlreadySignedUp => State::Done(Reason::Success),
                SignupError::Unknown(e) => State::Done(Reason::Error(e)),
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
