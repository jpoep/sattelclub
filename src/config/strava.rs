use serde::{Deserialize, Serialize};

/// Strava-specific configuration read from `sattelclub.toml`.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StravaConfig {
    /// The numeric Strava club ID (the last path segment of the club URL).
    pub club_id: String,
}

/// Session state persisted to `tokens.toml`.
///
/// Users manually copy the `_strava4_session` cookie from their browser's
/// DevTools (Application → Cookies → www.strava.com) and paste it here.
/// A session that gets used weekly will typically last for months.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SessionCache {
    pub session_cookie: String,
}
