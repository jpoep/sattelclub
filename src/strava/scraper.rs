use scraper::{Html, Selector};

/// The state of the signup button for the next upcoming event.
#[derive(Debug, Clone)]
pub enum EventButtonState {
    /// Signup is open (`data-at-capacity="false"`).
    Open,
    /// Ride is full (`data-at-capacity="true"`).
    Full,
    /// The RSVP button is absent — the current user is already signed up.
    AlreadySignedUp,
}

/// Everything we need to know about one upcoming club event.
#[derive(Debug, Clone)]
pub struct StravaEvent {
    /// Numeric club ID, from the `group-event-title` href.
    pub club_id: String,
    /// Numeric group event ID, from the `rsvp-view` div's `id` attribute.
    pub event_id: String,
    /// State of the signup button.
    pub button: EventButtonState,
}

/// Parse the club-page HTML and return the first upcoming event, or `None` if
/// no event card is present.
///
/// ## Page structure
///
/// ```html
/// <a class="group-event-title"
///    href="/clubs/{clubId}/group_events/{eventId}">...</a>
///
/// <div class="rsvp-view" id="{eventId}">
///   <ul>
///     <li>
///       <!-- present when open or full, absent when already signed up -->
///       <a class="button rsvp-js" data-at-capacity="false">I'm In</a>
///     </li>
///   </ul>
/// </div>
/// ```
///
/// | Data         | Source                                                 |
/// |--------------|--------------------------------------------------------|
/// | Event ID     | `div.rsvp-view` `id` attribute                         |
/// | Club ID      | `a.group-event-title` `href`, second path segment      |
/// | Button state | `a.rsvp-js` `data-at-capacity`; absent → already in   |
pub fn find_next_event(html: &str) -> Option<StravaEvent> {
    let document = Html::parse_document(html);

    let rsvp_sel = Selector::parse("div.rsvp-view").expect("static selector");
    let title_sel = Selector::parse("a.group-event-title").expect("static selector");
    let btn_sel = Selector::parse("a.rsvp-js").expect("static selector");

    // Event ID: the rsvp-view div's own id attribute.
    let rsvp_div = document.select(&rsvp_sel).next()?;
    let event_id = rsvp_div.value().attr("id")?.to_owned();

    // Club ID: second path segment of the title anchor's href.
    // href = "/clubs/<club_id>/group_events/<event_id>"
    let club_id = document
        .select(&title_sel)
        .next()
        .and_then(|a| a.value().attr("href"))
        .and_then(|href| {
            let mut parts = href.trim_start_matches('/').splitn(4, '/');
            parts.next(); // "clubs"
            parts.next().map(str::to_owned)
        })
        .unwrap_or_default();

    // Button state: presence and data-at-capacity of the rsvp-js anchor.
    let button = match rsvp_div.select(&btn_sel).next() {
        None => EventButtonState::AlreadySignedUp,
        Some(btn) => match btn.value().attr("data-at-capacity") {
            Some("true") => EventButtonState::Full,
            _ => EventButtonState::Open,
        },
    };

    Some(StravaEvent {
        club_id,
        event_id,
        button,
    })
}
