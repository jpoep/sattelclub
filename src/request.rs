use std::error::Error;

use anyhow::Context;
use chrono::{Datelike, NaiveDate};
use derive_more::Display;
use reqwest::{Client, header};
use scraper::{Html, Selector};

use crate::config::{Config, SessionCache, User};

// ---------------------------------------------------------------------------
// Error / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Display)]
pub enum SignupError {
    Full,
    NotYetOpen,
    AlreadySignedUp,
    Unknown(Box<dyn Error>),
}

pub type SignupResponse = Result<(), SignupError>;

// ---------------------------------------------------------------------------
// Strava client
// ---------------------------------------------------------------------------

/// A thin wrapper around `reqwest::Client` that carries the session cookie and
/// a CSRF token extracted from a page load.
pub struct StravaClient {
    inner: Client,
    session_cookie: String,
}

impl StravaClient {
    pub fn new(cache: &SessionCache) -> anyhow::Result<Self> {
        // We manage the cookie manually via the `Cookie` header so that we get
        // full control over exactly what is sent (and avoid the cookie jar
        // following redirects and silently swapping sessions).
        let inner = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0")
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

    /// Fetch the raw HTML of a URL, sending the session cookie.
    pub async fn get_html(&self, url: &str) -> anyhow::Result<String> {
        let response = self
            .inner
            .get(url)
            .header(header::COOKIE, self.cookie_header())
            .send()
            .await
            .with_context(|| format!("GET {url}"))?;

        let status = response.status();

        // Strava redirects to /login when the session has expired.
        if status.is_redirection() {
            let location = response
                .headers()
                .get(header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if location.contains("/login") || location.contains("login") {
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

        Ok(response.text().await.context("Reading response body")?)
    }

    /// Check whether the current session is still valid by loading the club
    /// page and checking the response.
    ///
    /// Returns `Ok(html)` if the session is valid, or an `Err` with an
    /// actionable human-readable message if the session has expired.
    pub async fn validate_session(&self, club_id: &str) -> anyhow::Result<String> {
        let url = format!("https://www.strava.com/clubs/{club_id}");
        self.get_html(&url).await
    }

    /// Scrape the CSRF token from a page's `<meta name="csrf-token">` tag.
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
}

// ---------------------------------------------------------------------------
// Event scraping
// ---------------------------------------------------------------------------

/// Minimal information about a Strava club event.
#[derive(Debug, Clone)]
pub struct StravaEvent {
    /// The signup / join-event URL path (e.g. `/club_events/123/join`).
    pub join_path: String,
    /// Human-readable title, for logging.
    pub title: String,
}

/// Parse the club-page HTML and return all upcoming events.
///
/// Strava renders upcoming events as `<li>` items inside an
/// `.upcoming-events` list. Each item contains an `<a>` whose `href` ends
/// with `/join` (the signup link).
pub fn scrape_upcoming_events(html: &str) -> Vec<StravaEvent> {
    let document = Html::parse_document(html);

    // The upcoming-events section — Strava's markup at time of writing:
    //   <ul class="upcoming-events">
    //     <li class="...">
    //       <a class="... btn-join ..." href="/club_events/123456/join">Join</a>
    //       <div class="event-name"><span>Tuesday Club Ride</span></div>
    //     </li>
    //   </ul>
    //
    // We look for anchor tags whose href contains "/join" inside elements that
    // also have an event-name or similar container. As a fallback we also try
    // any anchor ending in /join anywhere on the page.

    let anchor_sel = Selector::parse("a[href*='/join']").expect("static selector");
    let name_sel = Selector::parse(".event-name, .club-event-title, h4, h3").ok();

    let mut events = Vec::new();

    for anchor in document.select(&anchor_sel) {
        let href = match anchor.value().attr("href") {
            Some(h) if h.contains("club_events") && h.ends_with("/join") => h.to_owned(),
            _ => continue,
        };

        // Try to find a nearby title element (sibling / parent).
        let title = name_sel
            .as_ref()
            .and_then(|sel| {
                // Walk up to the containing <li> or <div> and search within it.
                anchor.ancestors().find_map(|node| {
                    let el = scraper::ElementRef::wrap(node)?;
                    el.select(sel)
                        .next()
                        .map(|t| t.text().collect::<String>().trim().to_owned())
                })
            })
            .unwrap_or_else(|| href.clone());

        events.push(StravaEvent {
            join_path: href,
            title,
        });
    }

    events
}

// ---------------------------------------------------------------------------
// SignupRequest
// ---------------------------------------------------------------------------

pub struct SignupRequest {
    pub date: NaiveDate,
    pub user: User,
    pub event: StravaEvent,
    pub club_id: String,
}

impl Config {
    pub fn get_next_ride_date(&self) -> NaiveDate {
        let today = chrono::Local::now().date_naive();
        let mut next_ride_date = today;
        while next_ride_date.weekday() != self.ride_weekday {
            next_ride_date = next_ride_date.succ_opt().expect("Invalid date");
        }
        next_ride_date
    }
}

impl SignupRequest {
    pub fn for_event(config: &Config, user: &User, event: StravaEvent) -> Self {
        SignupRequest {
            date: config.get_next_ride_date(),
            user: user.clone(),
            event,
            club_id: config.strava.club_id.clone(),
        }
    }

    /// Attempt to sign `self.user` up for `self.event`.
    ///
    /// Strava's web signup is a POST to `/club_events/{id}/join` with the CSRF
    /// token in the `X-CSRF-Token` header and `Content-Type: application/json`.
    /// The response is typically a JSON object `{"success": true}` on success or
    /// a redirect / error body otherwise.
    pub async fn make_request(&self, client: &StravaClient, csrf_token: &str) -> SignupResponse {
        let url = format!("https://www.strava.com{}", self.event.join_path);

        let result = client
            .inner
            .post(&url)
            .header(header::COOKIE, client.cookie_header())
            .header("X-CSRF-Token", csrf_token)
            .header(header::CONTENT_TYPE, "application/json")
            .header(
                header::ACCEPT,
                "application/json, text/javascript, */*; q=0.01",
            )
            .header("X-Requested-With", "XMLHttpRequest")
            .body("{}")
            .send()
            .await;

        match result {
            Err(e) => {
                return Err(SignupError::Unknown(Box::new(e)));
            }
            Ok(resp) => {
                let status = resp.status();

                // 302 redirect often means "not yet open" — the event page
                // redirects back to the club page without registering signup.
                if status.is_redirection() {
                    return Err(SignupError::NotYetOpen);
                }

                if status == reqwest::StatusCode::UNPROCESSABLE_ENTITY {
                    // Strava returns 422 when the user is already signed up.
                    return Err(SignupError::AlreadySignedUp);
                }

                if !status.is_success() {
                    let body = resp.text().await.unwrap_or_default();
                    // Heuristic: if the response mentions "full" the event is full.
                    if body.to_lowercase().contains("full") {
                        return Err(SignupError::Full);
                    }
                    return Err(SignupError::Unknown(
                        format!("HTTP {status}: {body}").into(),
                    ));
                }

                // Parse JSON response if present.
                let body = resp.text().await.unwrap_or_default();
                if body.to_lowercase().contains("\"success\":false") {
                    return Err(SignupError::Unknown("Server returned success:false".into()));
                }

                Ok(())
            }
        }
    }
}
