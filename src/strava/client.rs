use anyhow::Context;
use reqwest::{Client, StatusCode, header};
use scraper::{Html, Selector};

use crate::config::SessionCache;

/// A thin wrapper around [`reqwest::Client`] that carries the Strava session
/// cookie and encapsulates all HTTP mechanics.
///
/// All knowledge of headers, redirect behaviour, and Strava-specific URL
/// patterns lives here. Nothing outside this module needs to touch `reqwest`
/// directly.
pub struct StravaClient {
    inner: Client,
    session_cookie: String,
}

impl StravaClient {
    pub fn new(cache: &SessionCache) -> anyhow::Result<Self> {
        // We manage the cookie manually via the `Cookie` header so we have
        // full control over what is sent and redirects don't silently swap
        // sessions.
        let inner = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("Mozilla/5.0 (X11; Linux x86_64; rv:149.0) Gecko/20100101 Firefox/149.0")
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            inner,
            session_cookie: cache.session_cookie.clone(),
        })
    }

    fn cookie_header(&self) -> String {
        format!("_strava4_session={}", self.session_cookie)
    }

    /// Fetch the raw HTML of `url`, sending the session cookie.
    ///
    /// Returns an actionable error if the session has expired — Strava
    /// redirects to `/login` in that case.
    pub async fn get_html(&self, url: &str) -> anyhow::Result<String> {
        let response = self
            .inner
            .get(url)
            .header(header::COOKIE, self.cookie_header())
            .send()
            .await
            .with_context(|| format!("GET {url}"))?;

        let status = response.status();

        if status.is_redirection() {
            let location = response
                .headers()
                .get(header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if location.contains("login") {
                anyhow::bail!(
                    "Session expired. Please refresh _strava4_session in tokens.toml \
                     from your browser devtools."
                );
            }
            anyhow::bail!("Unexpected redirect to: {location}");
        }

        if !status.is_success() {
            anyhow::bail!("HTTP {status} fetching {url}");
        }

        response.text().await.context("Reading response body")
    }

    /// Validate the session by fetching the club page.
    ///
    /// Returns the raw HTML on success so callers can reuse it without a
    /// second request.
    pub async fn fetch_club_page(&self, club_id: &str) -> anyhow::Result<String> {
        let url = format!("https://www.strava.com/clubs/{club_id}");
        self.get_html(&url).await
    }

    /// Scrape the CSRF token from `<meta name="csrf-token" content="...">`.
    pub fn extract_csrf_token(html: &str) -> anyhow::Result<String> {
        let document = Html::parse_document(html);
        let sel = Selector::parse(r#"meta[name="csrf-token"]"#).expect("static selector is valid");
        document
            .select(&sel)
            .next()
            .and_then(|el| el.value().attr("content"))
            .map(|s| s.to_owned())
            .ok_or_else(|| anyhow::anyhow!("CSRF token not found in page HTML"))
    }

    /// Send the RSVP PUT request for a group event.
    ///
    /// ```text
    /// PUT /clubs/{clubId}/group_events/{eventId}/rsvp?leave=false
    /// X-CSRF-Token: <token>
    /// X-Requested-With: XMLHttpRequest
    /// Accept: text/javascript, application/javascript, ...
    /// Content-Length: 0
    /// ```
    ///
    /// Returns the HTTP status code and response body so that callers can
    /// interpret the result without knowing anything about the HTTP layer.
    pub async fn put_rsvp(
        &self,
        club_id: &str,
        event_id: &str,
        csrf_token: &str,
    ) -> anyhow::Result<(StatusCode, String)> {
        let url = format!(
            "https://www.strava.com/clubs/{club_id}/group_events/{event_id}/rsvp?leave=false"
        );
        let referer = format!("https://www.strava.com/clubs/{club_id}");

        let response = self
            .inner
            .put(&url)
            .header(header::COOKIE, self.cookie_header())
            .header("X-CSRF-Token", csrf_token)
            .header("X-Requested-With", "XMLHttpRequest")
            .header(
                header::ACCEPT,
                "text/javascript, application/javascript, \
                 application/ecmascript, application/x-ecmascript",
            )
            .header(header::REFERER, referer)
            .header(header::ORIGIN, "https://www.strava.com")
            .header(header::CONTENT_LENGTH, "0")
            .send()
            .await
            .with_context(|| format!("PUT rsvp for event {event_id}"))?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Ok((status, body))
    }
}
