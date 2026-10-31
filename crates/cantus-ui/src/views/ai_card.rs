//! The confirmation card (ADR 0081 §10): a call waiting for the user, in the corner-card
//! stack (`views::audio_notice`, the same as the plug-in and freeze cards), non-modal —
//! who asks, "wants to:", what it would do in the user's language, what the plan touches,
//! how long it waits; *Allow*, *Allow for this session* (an edit tool only), *Deny*; the ✕
//! denies. The oldest call first; the title counts the others. Everything an agent or a
//! project says is shown as plain text: nothing in it is markup, nothing is followed.
//!
//! Who asks (review 5 #5): the name the client gives itself — quoted, invisible and
//! direction characters taken out, filled in one pass, elided in its middle — beside what
//! actually let it in: the token's client, the bridge, or Cantus itself. A client cannot
//! word the card by naming itself.
//!
//! A press is taken only on a card that has been in front for [`ARMED`] (review 5 #4): a
//! second click meant for the card just answered never answers the next one, which
//! appears under the cursor.

use crate::app::CantusApp;
use crate::i18n::t;
use cantus_mcp::Answer;
use std::time::{Duration, Instant};

/// How long a card is in front before its buttons answer.
pub const ARMED: Duration = Duration::from_millis(600);

/// What the card shows.
#[derive(Clone, Debug)]
pub struct Card {
    /// The call's id: the card is keyed by it.
    pub id: u64,
    /// Who asks: the client's own name, quoted (shortened in its middle when long).
    pub who: String,
    /// What let it in, drawn in full after the name (review 6 #5): "· with the “default”
    /// token", "· inside Cantus" — and what it says it is ("through a bridge, as it says").
    pub via: String,
    /// "wants to:" (and how many more wait).
    pub title: String,
    /// What it would do, what it touches, how long it waits.
    pub body: String,
    /// Label, the call and the answer.
    pub buttons: Vec<(String, (u64, Answer))>,
    /// What the ✕ does.
    pub close: (u64, Answer),
}

/// The words for what let a caller in: always the token it carried (review 6 #3: the
/// bridge's header is the client's to send, so it never stands for the token), and what
/// it says it is.
fn via_words(via: Option<&str>, bridge: bool) -> String {
    let fill = super::host::word;
    let base = match via {
        None => t("ai.card.via_app"),
        Some(token) => {
            let token = cantus_ai::quote_name(&fill, token);
            fill("ai.card.via_token", &[("token", token)])
        }
    };
    let base = if bridge {
        fill("ai.card.via_bridge", &[("via", base)])
    } else {
        base
    };
    fill("ai.card.via", &[("via", base)])
}

/// The card for the oldest call waiting, if any (none while the AI area is off).
pub fn card(app: &CantusApp) -> Option<Card> {
    if !app.settings.ai.enabled {
        return None;
    }
    let fill = super::host::word;
    let agents = app.ai.agents()?;
    let ask = agents.asks().next()?;
    let more = agents.asks().count() - 1;
    let who = cantus_ai::quote_name(&fill, &ask.caller.name);
    let via = via_words(ask.caller.via.as_deref(), ask.caller.bridge);
    let title = if more == 0 {
        t("ai.card.wants")
    } else {
        fill("ai.card.wants_more", &[("n", more.to_string())])
    };
    let mut body = ask.said.clone();
    // The plan in words when it says more than the sentence (not the same words unquoted).
    let plain = |s: &str| -> String {
        s.chars()
            .filter(|c| !matches!(c, '“' | '”' | '«' | '»' | '"'))
            .collect::<String>()
            .to_lowercase()
    };
    if let Some(p) = ask
        .preview
        .as_ref()
        .filter(|p| plain(p) != plain(&ask.said))
    {
        body.push_str(" — ");
        body.push_str(p);
    }
    let left = ask
        .deadline
        .saturating_duration_since(Instant::now())
        .as_secs();
    body.push('\n');
    body.push_str(&fill(
        "ai.card.waits",
        &[("time", format!("{}:{:02}", left / 60, left % 60))],
    ));
    let mut buttons = vec![(t("ai.card.allow"), (ask.id, Answer::Allow))];
    if ask.grantable {
        buttons.push((
            t("ai.card.allow_session"),
            (ask.id, Answer::AllowForSession),
        ));
    }
    buttons.push((t("ai.card.deny"), (ask.id, Answer::Deny)));
    Some(Card {
        id: ask.id,
        who,
        via,
        title,
        body,
        buttons,
        close: (ask.id, Answer::Deny),
    })
}

/// The frame notes which call is in front, and since when (for [`pressed`]).
pub fn note_front(app: &mut CantusApp, now: Instant) {
    let front = card(app).map(|c| c.id);
    match (front, app.ai.card_front) {
        (None, _) => app.ai.card_front = None,
        (Some(id), Some((seen, _))) if seen == id => {}
        (Some(id), _) => app.ai.card_front = Some((id, now)),
    }
}

/// How long until the card for call `id` answers presses (`None`: it does now).
pub fn armed_in(app: &CantusApp, id: u64, now: Instant) -> Option<Duration> {
    match app.ai.card_front {
        Some((front, since)) if front == id => ARMED
            .checked_sub(now.saturating_duration_since(since))
            .filter(|d| !d.is_zero()),
        _ => Some(ARMED),
    }
}

/// A press on the card's button or ✕: taken only for the call in front, and only once it
/// has been in front for [`ARMED`] — else ignored (review 5 #4).
pub fn pressed(app: &mut CantusApp, id: u64, a: Answer, now: Instant) {
    match app.ai.card_front {
        Some((front, since)) if front == id && now.saturating_duration_since(since) >= ARMED => {
            answer(app, id, a)
        }
        _ => {}
    }
}

/// The user's answer.
pub fn answer(app: &mut CantusApp, id: u64, answer: Answer) {
    if let Some(a) = app.ai.agents_mut() {
        a.answer(id, answer);
    }
    // Carried out now, not at the next frame.
    super::pump(app);
}
