//! **Bringing the dancers back** (round 40, the owner's report of 2026-11-18: "once the
//! dancer is hidden, I don't know how to bring it back").
//!
//! Two things hide a dancer: its own *Hide* (the pill over it: its `view.mode` becomes
//! hidden, in the project) and the stage's switch (*View ▸ Stage*, Alt+D:
//! `settings.stage.hidden`, the machine's). One action undoes either:
//!
//! - [`show`] (*View ▸ Stage* and its key when nothing shows): the switch on and, when every
//!   dancer of the song is hidden, each back where it was before — floating or docked, as
//!   last seen ([`StageState::last_shown`](super::StageState)), floating when it was never
//!   seen this session — in **one** undo step (`cmd.stage.show_dancers`);
//! - [`show_dancer`] (the track's menu, the editor's *Show*, the notice's *Show again*):
//!   one dancer the same way.
//!
//! *View ▸ Stage*'s tick is [`on_screen`]: a dancer floats with the stage on, or one is
//! docked and the Stage tab is in the workspace — what the user sees, not the switch.
//!
//! A dancer's *Hide* leaves a card in the corner stack ([`Notice`], `audio_notice`): "Keys
//! hidden — Alt+D or View ▸ Stage shows it again", with *Show again*, for [`NOTICE_SECS`].

use super::ViewMode;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::keymap::Action;
use crate::views::workspace::Pane;
use cantus_core::{Command, InstrumentKind, TrackId};
use std::time::{Duration, Instant};

/// How long the card after a *Hide* stays, in seconds.
pub const NOTICE_SECS: f64 = 12.0;

/// The undo step of [`show`]: every hidden dancer back.
pub const SHOW_ALL: &str = "cmd.stage.show_dancers";
/// The undo step of [`show_dancer`].
pub const SHOW_ONE: &str = "cmd.stage.show_dancer";
/// The undo step of a dancer's *Hide*.
pub const HIDE_ONE: &str = "cmd.stage.hide_dancer";

/// The card a dancer's *Hide* leaves.
#[derive(Clone, Debug)]
pub struct Notice {
    pub track: TrackId,
    /// The history's revision right after the hide: while it is still this, *Show again*
    /// is the hide's own undo (the history stays as it was).
    pub revision: u64,
    pub at: Instant,
}

impl Notice {
    /// Whether it still shows at `now`.
    pub fn live(&self, now: Instant) -> bool {
        now.duration_since(self.at) < Duration::from_secs_f64(NOTICE_SECS)
    }
}

/// Every dancer of the song and where it shows (`view.mode`), in track order.
pub fn each(app: &CantusApp) -> impl Iterator<Item = (TrackId, ViewMode)> + '_ {
    app.project().tracks.iter().filter_map(|t| {
        let n = t.instrument.native()?;
        (n.kind == InstrumentKind::Dancer).then(|| (t.id, ViewMode::of(n.get("view.mode"))))
    })
}

/// [`each`], gathered.
pub fn dancers(app: &CantusApp) -> Vec<(TrackId, ViewMode)> {
    each(app).collect()
}

/// Where a dancer track's character shows (`None`: no dancer on that track).
pub fn mode_of(app: &CantusApp, track: TrackId) -> Option<ViewMode> {
    let n = app.project().track(track)?.instrument.native()?;
    (n.kind == InstrumentKind::Dancer).then(|| ViewMode::of(n.get("view.mode")))
}

/// Whether a dancer in `mode` is on screen now: the stage on, and floating, or docked with
/// the Stage tab in the workspace.
fn seen(app: &CantusApp, mode: ViewMode) -> bool {
    !app.settings.stage.hidden
        && match mode {
            ViewMode::Floating => true,
            ViewMode::Docked => app.pane_shown(Pane::Stage),
            ViewMode::Hidden => false,
        }
}

/// Whether this dancer track's character is on screen now.
pub fn dancer_on_screen(app: &CantusApp, track: TrackId) -> bool {
    mode_of(app, track).is_some_and(|m| seen(app, m))
}

/// Whether any dancer is on screen now: *View ▸ Stage*'s tick.
pub fn on_screen(app: &CantusApp) -> bool {
    each(app).any(|(_, m)| seen(app, m))
}

/// Where a hidden dancer goes back to: where it was last seen, else floating.
pub fn back_to(app: &CantusApp, track: TrackId) -> ViewMode {
    app.stage
        .last_shown
        .get(&track)
        .copied()
        .filter(|m| *m != ViewMode::Hidden)
        .unwrap_or(ViewMode::Floating)
}

/// *View ▸ Stage* and its key: hide every dancer when one is on screen, else show them.
pub fn toggle(app: &mut CantusApp) {
    if on_screen(app) {
        hide_all(app);
    } else {
        show(app);
    }
}

/// The stage off: no dancer over the workspace and the Stage tab closed (each dancer's own
/// `view.mode` untouched).
pub fn hide_all(app: &mut CantusApp) {
    app.settings.stage.hidden = true;
    app.hide_pane(Pane::Stage);
    app.store_settings();
}

/// Put the dancers on screen: the stage on; when every dancer of the song is hidden, each
/// back where it was, in one undo step; the Stage tab back when one is docked. A song with
/// no dancer says how to add one.
pub fn show(app: &mut CantusApp) {
    app.settings.stage.hidden = false;
    let all = dancers(app);
    if all.is_empty() {
        app.set_status(t("stage.status.none"));
    } else if all.iter().all(|(_, m)| *m == ViewMode::Hidden) {
        let tracks: Vec<TrackId> = all.iter().map(|(t, _)| *t).collect();
        restore(app, &tracks, SHOW_ALL);
    }
    reveal_docked(app);
    app.stage.notice = None;
    app.store_settings();
}

/// Every dancer on screen (the Stage tab's *Show the dancers*): the stage on, each hidden one
/// back where it was, in one undo step.
pub fn show_every(app: &mut CantusApp) {
    app.settings.stage.hidden = false;
    let tracks: Vec<TrackId> = dancers(app).iter().map(|(t, _)| *t).collect();
    restore(app, &tracks, SHOW_ALL);
    reveal_docked(app);
    app.stage.notice = None;
    app.store_settings();
}

/// How many of the song's dancers are not on screen.
pub fn hidden_count(app: &CantusApp) -> usize {
    dancers(app)
        .into_iter()
        .filter(|(_, m)| !seen(app, *m))
        .count()
}

/// Put one dancer on screen: the stage on, the dancer back where it was if it was hidden
/// (its own undo step), the Stage tab if it is docked there.
pub fn show_dancer(app: &mut CantusApp, track: TrackId) {
    if mode_of(app, track).is_none() {
        return;
    }
    app.settings.stage.hidden = false;
    if mode_of(app, track) == Some(ViewMode::Hidden) {
        restore(app, &[track], SHOW_ONE);
    }
    reveal_docked(app);
    if app.stage.notice.as_ref().is_some_and(|n| n.track == track) {
        app.stage.notice = None;
    }
    app.store_settings();
}

/// The notice's *Show again*: the hide undone — its own undo while nothing came after it
/// (the history as it was), else the dancer shown as a step of its own.
pub fn show_again(app: &mut CantusApp) {
    let Some(n) = app.stage.notice.take() else {
        return;
    };
    let untouched = app.bus.revision() == n.revision && app.bus.undo_label_key() == Some(HIDE_ONE);
    if untouched {
        app.undo();
        app.settings.stage.hidden = false;
        reveal_docked(app);
        app.store_settings();
    } else {
        show_dancer(app, n.track);
    }
}

/// A dancer's own *Hide* (its pill): its `view.mode` hidden as one undo step, where it was
/// remembered, and the card that says how to bring it back.
pub fn hide_dancer(app: &mut CantusApp, track: TrackId) {
    let Some(was) = mode_of(app, track) else {
        return;
    };
    if was == ViewMode::Hidden {
        return;
    }
    app.stage.last_shown.insert(track, was);
    let cmd = Command::Batch {
        label_key: HIDE_ONE.into(),
        commands: vec![set_mode(track, ViewMode::Hidden)],
    };
    if app.exec(cmd) {
        app.stage.notice = Some(Notice {
            track,
            revision: app.bus.revision(),
            at: Instant::now(),
        });
    }
}

/// The card's words: its title (the track hidden) and how to bring it back, with the key
/// the keymap gives *View ▸ Stage* now (none: the menu alone).
pub fn notice_words(app: &CantusApp, n: &Notice) -> (String, String) {
    let name = app
        .project()
        .track(n.track)
        .map_or_else(String::new, |t| t.name.clone());
    let title = tf("stage.notice.hidden", &[("track", &name)]);
    let key = app.keymap.display_for(Action::ToggleStage);
    let body = if key.is_empty() {
        t("stage.notice.body_menu")
    } else {
        tf("stage.notice.body", &[("key", &key)])
    };
    (title, body)
}

/// The card while it shows (gone once its time is up, or its dancer is).
pub fn notice(app: &CantusApp, now: Instant) -> Option<&Notice> {
    app.stage
        .notice
        .as_ref()
        .filter(|n| n.live(now) && mode_of(app, n.track) == Some(ViewMode::Hidden))
}

/// Let the card go once its time is up or its dancer shows again; when it goes, in how long.
pub fn prune(app: &mut CantusApp, now: Instant) -> Option<Duration> {
    if notice(app, now).is_none() {
        app.stage.notice = None;
        return None;
    }
    let n = app.stage.notice.as_ref()?;
    Some(Duration::from_secs_f64(NOTICE_SECS).saturating_sub(now.duration_since(n.at)))
}

fn set_mode(track: TrackId, mode: ViewMode) -> Command {
    Command::SetInstrumentValue {
        track,
        key: "view.mode".into(),
        value: mode.value(),
    }
}

/// The hidden ones of `tracks` back where they were, as one undo step labelled `label`.
fn restore(app: &mut CantusApp, tracks: &[TrackId], label: &str) {
    let commands: Vec<Command> = tracks
        .iter()
        .filter(|t| mode_of(app, **t) == Some(ViewMode::Hidden))
        .map(|t| set_mode(*t, back_to(app, *t)))
        .collect();
    if commands.is_empty() {
        return;
    }
    app.exec(Command::Batch {
        label_key: label.into(),
        commands,
    });
}

/// The Stage tab back in the workspace when a dancer is docked there.
fn reveal_docked(app: &mut CantusApp) {
    if each(app).any(|(_, m)| m == ViewMode::Docked) {
        app.reveal_pane(Pane::Stage);
    }
}

/// Remember where each dancer shows while it shows (`gather`, every frame): where a hidden
/// one goes back to.
pub(crate) fn remember(st: &mut super::StageState, track: TrackId, mode: ViewMode) {
    if mode != ViewMode::Hidden && st.last_shown.get(&track) != Some(&mode) {
        st.last_shown.insert(track, mode);
    }
}
