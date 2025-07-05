use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub first_name: String,
    pub surname: String,
    pub email: String,
    pub enabled: bool,
}

