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
use cantus_core::{ClipId, Command, Project, Ticks, TrackId};

/// What pattern mode is playing, and where the song was.
#[derive(Debug, Default)]
pub struct State {
    /// The project this state belongs to (`CantusApp::epoch`): another one starts afresh.
    epoch: u64,
    /// The mode as last seen (a change — by the key, a click, an undo — is acted on once).
    active: bool,
    region: Option<PatternRegion>,
    /// The note clip the pattern was taken from.
    clip: Option<(TrackId, ClipId)>,
    /// The song's position when pattern mode began.
    song_at: Option<Ticks>,
    /// A seek to the pattern's top not yet seen landed, and the frames since it was sent:
    /// a position the engine reported from before it lands is not a reason to seek again.
    sought: Option<(Ticks, u32)>,
}

/// Frames a seek is waited for before a position outside the pattern seeks again (half a
/// second at 60 frames a second: many callbacks at the largest buffer).
const SEEK_WAIT: u32 = 30;

/// Whether to seek to the pattern's top `start`: on entering, or when the engine's place
/// `pos` is outside `[start, end)` — unless a seek there was sent and has not been seen to
/// land (an engine that posts its old place after the seek, a callback longer than a UI
/// frame, would make it seek twice and play the pattern's first block twice). A place
/// inside the pattern means the seek landed; one not seen within [`SEEK_WAIT`] frames is
/// sent again.
fn should_seek(
    sought: &mut Option<(Ticks, u32)>,
    entered: bool,
    pos: Ticks,
    start: Ticks,
    end: Ticks,
) -> bool {
    let inside = pos >= start && pos < end;
    if inside {
        *sought = None;
    }
    let waiting = match sought {
        Some((at, n)) if *at == start && *n < SEEK_WAIT => {
            *n += 1;
            true
        }
        _ => false,
    };
    if entered || (!inside && !waiting) {
        *sought = Some((start, 0));
        return true;
    }
    false
}

impl State {
    /// The pattern being played (pattern mode only).
    pub fn region(&self) -> Option<&PatternRegion> {
        self.region.as_ref().filter(|_| self.active)
    }
}

/// The current pattern of `app`: the note clip being edited — or, when the selection is
/// something else (a track, an audio clip, nothing), the pattern already playing, as long
/// as its clip is still there (FL keeps its pattern until another is chosen).
pub fn current(app: &CantusApp) -> Option<(PatternRegion, (TrackId, ClipId))> {
    let chosen = app
        .selected_track
        .zip(app.selected_clip)
        .and_then(|(t, c)| PatternRegion::of(app.project(), t, c).map(|r| (r, (t, c))));
    chosen.or_else(|| {
        let (t, c) = app.topbar.pattern.clip?;
        PatternRegion::of(app.project(), t, c).map(|r| (r, (t, c)))
    })
}

/// PAT / SONG (L): switch the mode — one undo step, as the metronome's switch is. Not in
/// the middle of a take: it belongs to the song.
pub fn toggle(app: &mut CantusApp) {
    let on = !app.project().pattern_mode;
    if on && app.is_recording() {
        app.set_error(t("topbar.mode.not_while_recording"));
        return;
    }
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
/// the playhead in and out, keep it inside the pattern, and send the engine a new snapshot
/// when what plays changed.
pub fn follow(app: &mut CantusApp) {
    let on = app.project().pattern_mode;
    // Another project (new, opened, a demo, recovered): its own state, from nothing.
    if app.topbar.pattern.epoch != app.epoch {
        app.topbar.pattern = State {
            epoch: app.epoch,
            ..State::default()
        };
    }
    let mut entered = false;
    if on != app.topbar.pattern.active {
        app.topbar.pattern.active = on;
        if on {
            // An undo or a redo can bring pattern mode back during a take: the take ends
            // first, in the song it was recorded into.
            if app.is_recording() {
                app.stop_recording();
            }
            app.topbar.pattern.song_at = Some(app.engine.position());
            entered = true;
        } else {
            app.topbar.pattern.region = None;
            app.topbar.pattern.clip = None;
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
    let (region, clip) = match now {
        Some((r, c)) => (Some(r), Some(c)),
        None => (None, None),
    };
    if region != app.topbar.pattern.region {
        app.topbar.pattern.region = region;
        app.topbar.pattern.clip = clip;
        app.force_resync();
    }
    // FL starts the pattern from its top; then the playhead stays in it: Home, a second
    // Stop, a click on the ruler —
    // any place outside it is the pattern's top (the engine wraps only inside the loop).
    let (start, end) = match &app.topbar.pattern.region {
        Some(r) => (r.start, r.end()),
        None => (
            Ticks::ZERO,
            crate::views::bars::bar_len(app.project(), Ticks::ZERO),
        ),
    };
    let pos = app.engine.engine_position();
    if should_seek(&mut app.topbar.pattern.sought, entered, pos, start, end) {
        app.seek(start);
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
    app.set_error(crate::i18n::tf("topbar.mode.no_record", &[("key", &key)]));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Review 2: a position from before the seek (posted by a callback already running when
    /// it was sent) does not seek again; once the seek is seen to land, a place outside the
    /// pattern does; a seek never seen to land is sent again after a while.
    #[test]
    fn a_stale_position_after_a_seek_does_not_seek_again() {
        let (start, end) = (Ticks(3_840), Ticks(7_680));
        let mut sought = None;
        assert!(
            should_seek(&mut sought, true, Ticks(100), start, end),
            "entering"
        );
        // The callback that was running posts its old place: no second seek.
        assert!(!should_seek(&mut sought, false, Ticks(120), start, end));
        // The seek lands.
        assert!(!should_seek(&mut sought, false, start, start, end));
        assert!(sought.is_none());
        // Home, a click on the ruler: outside again, a seek.
        assert!(should_seek(&mut sought, false, Ticks(0), start, end));
        // Never seen to land: sent again after the wait.
        for _ in 0..SEEK_WAIT {
            assert!(!should_seek(&mut sought, false, Ticks(0), start, end));
        }
        assert!(should_seek(&mut sought, false, Ticks(0), start, end));
        // Another pattern: its own seek at once.
        let (s2, e2) = (Ticks(7_680), Ticks(11_520));
        assert!(should_seek(&mut sought, false, Ticks(0), s2, e2));
    }
}
