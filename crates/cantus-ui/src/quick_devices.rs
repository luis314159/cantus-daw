//! **The devices used last, one click away** (round 39, ADR 0110): the add-track key's
//! right-click menu offers the instruments chosen last, and a mixer strip's right-click
//! menu the effects inserted last — FL Studio's channel-rack *+* list and Ableton's
//! browser *Recent*, without opening the picker. Two clicks add a track with the
//! instrument used yesterday, or put the compressor on a strip, where the picker's way is
//! four.
//!
//! The list is the picker's own *recently used* (`picker_prefs`, most recent first): a
//! device chosen here goes to its front, as one chosen in the picker does.

use crate::app::CantusApp;
use crate::picker::PickPurpose;
use crate::picker_catalog::Role;

/// How many recent devices a menu offers.
pub const QUICK: usize = 5;

/// One recent device: its catalogue key and its name.
#[derive(Clone, Debug, PartialEq)]
pub struct Quick {
    pub key: String,
    pub name: String,
}

/// The picker's catalogue brought up to date (it is cached: cheap when nothing changed).
pub fn warm(app: &mut CantusApp) {
    let sounds = app.samples.content().instruments();
    app.picker_view.refresh(&app.plugins.db, &sounds);
}

/// The devices of `role` chosen last, most recent first (at most [`QUICK`]).
pub fn recent(app: &mut CantusApp, role: Role) -> Vec<Quick> {
    warm(app);
    recent_seen(app, role)
}

/// [`recent`] from the catalogue as it was last brought up to date ([`warm`]): for a menu
/// built where the application cannot be changed.
pub fn recent_seen(app: &CantusApp, role: Role) -> Vec<Quick> {
    let v = &app.picker_view;
    v.prefs
        .recent
        .iter()
        .filter_map(|k| v.device(k))
        .filter(|d| d.role == role)
        .take(QUICK)
        .map(|d| Quick {
            key: d.key.clone(),
            name: d.name.clone(),
        })
        .collect()
}

/// Add the device `key` for `purpose`, as the picker does when it is chosen there; it goes
/// to the front of the recent list. Returns whether it was found.
pub fn add(app: &mut CantusApp, key: &str, purpose: PickPurpose) -> bool {
    let Some(choice) = app.picker_view.device(key).map(|d| d.choice.clone()) else {
        return false;
    };
    app.picker_view.prefs.used(key);
    app.picker_view.store_prefs();
    app.picker_view.last_choice = Some((purpose.clone(), choice.clone()));
    app.apply_pick(purpose, choice);
    true
}
