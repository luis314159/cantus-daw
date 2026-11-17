//! A unit's **view**: which of its drawings its window shows (Vetus's record or cassette,
//! ADR 0130) — a user preference per unit kind, like a colourway (ADR 0127 §Colourways):
//! never in a song, never heard. `settings.json`'s `unit_views` (`{ "vetus": "cassette" }`)
//! is applied on the thread that draws by `CantusApp::apply_unit_views` — at start in
//! `CantusApp::assemble` and whenever one changes; the exported plug-in's window has no
//! settings file and keeps a choice for its session.

use std::cell::RefCell;

thread_local! {
    /// The views chosen on this thread: (unit, view id).
    static VIEWS: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
}

/// Every choice at once (the settings' map).
pub fn set_views<'a>(choices: impl IntoIterator<Item = (&'a str, &'a str)>) {
    VIEWS.with(|v| {
        let mut v = v.borrow_mut();
        v.clear();
        v.extend(
            choices
                .into_iter()
                .map(|(u, id)| (u.to_string(), id.to_string())),
        );
    });
}

/// One unit's choice.
pub fn set_view(unit: &str, id: &str) {
    VIEWS.with(|v| {
        let mut v = v.borrow_mut();
        v.retain(|(u, _)| u != unit);
        v.push((unit.to_string(), id.to_string()));
    });
}

/// The view chosen for `unit`, if any.
pub fn view_of(unit: &str) -> Option<String> {
    VIEWS.with(|v| {
        v.borrow()
            .iter()
            .find(|(u, _)| u == unit)
            .map(|(_, id)| id.clone())
    })
}

/// Whether `unit`'s chosen view is `id`, the default answering for none.
pub fn is(unit: &str, id: &str, default: &str) -> bool {
    VIEWS.with(|v| {
        let v = v.borrow();
        let chosen = v.iter().find(|(u, _)| u == unit).map(|(_, i)| i.as_str());
        chosen.unwrap_or(default) == id
    })
}
