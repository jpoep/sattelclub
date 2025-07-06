use derive_more::Display;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrouprideResponse {
    pub error: Option<String>,
    pub success_data: Option<GrouprideSuccessData>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrouprideSuccessData {
    pub slug: String,
    pub email: String,
    pub is_waitlist: bool,
}

#[derive(Display)]
pub enum GrouprideErrorResponse {
    AlreadySignedUp,
    RideNotFound,
    RideFull,
    UnknownError(String),
}

impl GrouprideErrorResponse {
    pub fn from_message(message: String) -> Self {
        match message.as_str() {
            "It looks like you are already signed up with your email address" => {
                GrouprideErrorResponse::AlreadySignedUp
            }
            "Groupride doesn't exist!" => GrouprideErrorResponse::RideNotFound,
            "Groupride is full!" => GrouprideErrorResponse::RideFull,
            _ => GrouprideErrorResponse::UnknownError(message),
        }
    }
}
