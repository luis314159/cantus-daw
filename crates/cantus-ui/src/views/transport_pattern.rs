//! Pattern mode in the application (ADR 0052; the model is `cantus_core::pattern`):
//!
//! - **What plays.** While the song is in pattern mode, the engine is handed
//!   [`project_to_play`] — the project with only the current pattern's clips and the loop
//!   on its window — instead of the song (`CantusApp::sync_engine`). The current pattern is
//!   the clip the piano roll and the channel rack edit (`selected_track`, `selected_clip`);
//!   choosing another clip changes what plays at once, as choosing a pattern does in FL.
//! - **Where the playhead goes.** Into the pattern on entering (FL starts the pattern from
//!   its top); back where the song was on leaving. Undo and redo of the switch go the same
//!   way: [`follow`] watches the mode, not the key.
//! - **Recording** writes into the song, so pattern mode refuses it with a clear hint
//!   (switch to song mode, L) rather than record a take nobody could place.
//! - **Export** renders the song, always: pattern mode is a way of listening, not a part
//!   of the piece.

use crate::app::CantusApp;
use crate::i18n::t;
use cantus_core::pattern::{audition, PatternRegion};
use cantus_core::{Command, Project, Ticks};

/// What pattern mode is playing, and where the song was.
#[derive(Debug, Default)]
pub struct State {
    /// The mode as last seen (a change — by the key, a click, an undo — is acted on once).
    active: bool,
    region: Option<PatternRegion>,
    /// The song's position when pattern mode began.
    song_at: Option<Ticks>,
}

impl State {
    /// The pattern being played (pattern mode only).
    pub fn region(&self) -> Option<&PatternRegion> {
        self.region.as_ref().filter(|_| self.active)
    }
}

/// The current pattern of `app`: the clip being edited.
pub fn current(app: &CantusApp) -> Option<PatternRegion> {
    let (track, clip) = app.selected_track.zip(app.selected_clip)?;
    PatternRegion::of(app.project(), track, clip)
}

/// PAT / SONG (L): switch the mode — one undo step, as the metronome's switch is.
pub fn toggle(app: &mut CantusApp) {
    let on = !app.project().pattern_mode;
    if app.exec(Command::SetPatternMode { on }) {
        follow(app);
        app.set_status(t(if on {
            "topbar.mode.now_pattern"
        } else {
            "topbar.mode.now_song"
        }));
    }
}

/// Once a frame (and after the switch): follow the mode and the current pattern — move
/// the playhead in and out, and send the engine a new snapshot when what plays changed.
pub fn follow(app: &mut CantusApp) {
    let on = app.project().pattern_mode;
    if on != app.topbar.pattern.active {
        app.topbar.pattern.active = on;
        if on {
            app.topbar.pattern.song_at = Some(app.engine.position());
        } else {
            app.topbar.pattern.region = None;
            if let Some(at) = app.topbar.pattern.song_at.take() {
                app.seek(at);
            }
            app.force_resync();
            return;
        }
    }
    if !on {
        return;
    }
    let now = current(app);
    if now != app.topbar.pattern.region {
        let entered = app.topbar.pattern.region.is_none();
        app.topbar.pattern.region = now;
        app.force_resync();
        // Into the pattern: from its top on entering or when the pattern changes and the
        // playhead is outside it.
        let pos = app.engine.position();
        let (start, end) = match &app.topbar.pattern.region {
            Some(r) => (r.start, r.end()),
            None => (Ticks::ZERO, app.project().time_signature.ticks_per_bar()),
        };
        if entered || pos < start || pos >= end {
            app.seek(start);
        }
    }
}

/// The project the engine plays: in pattern mode, the pattern alone looped; else `None`
/// (the song as it is).
pub fn project_to_play(app: &CantusApp) -> Option<Project> {
    if !app.project().pattern_mode {
        return None;
    }
    Some(audition(app.project(), app.topbar.pattern.region.as_ref()))
}

/// Recording in pattern mode is refused, with the way out in the status line. Returns
/// whether it was.
pub fn refuse_record(app: &mut CantusApp) -> bool {
    if !app.project().pattern_mode || app.is_recording() {
        return false;
    }
    let key = app
        .keymap
        .bindings_for(crate::keymap::Action::TogglePatternMode)
        .first()
        .map(|b| b.display())
        .unwrap_or_default();
    app.set_error(crate::i18n::tf(
        "topbar.mode.no_record",
        &[("key", &key)],
    ));
    true
}
