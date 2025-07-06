use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub first_name: String,
    pub surname: String,
    pub email: String,
    pub enabled: bool,
}

impl User {
    pub fn name(&self) -> String {
        format!("{} {}", self.first_name, self.surname)
    }
}
