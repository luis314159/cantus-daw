//! What a surface asks, as the commands and actions of the application. A surface can only do
//! what the screen can: every mixer change is an ordinary command (a fader's moves merge into
//! one undo step until the finger lifts), every transport key is the keymap's own action.

use crate::app::CantusApp;
use crate::keymap::Action;
use cantus_core::external_surface::{SurfaceAction, Target};
use cantus_core::{Command, StripId, Ticks};

/// The strip of mixer index `i`.
fn strip_id(app: &CantusApp, t: Target) -> Option<Option<StripId>> {
    match t {
        Target::Master => Some(None),
        Target::Strip(i) => app.project().strips.get(i).map(|s| Some(s.id)),
    }
}

/// How far a held rewind or forward moves the playhead each interface frame, in ticks.
pub const SCAN_TICKS: i64 = cantus_core::PPQ / 4;
/// A jog tick, in ticks: a sixteenth.
pub const JOG_TICKS: i64 = cantus_core::PPQ / 4;

/// Apply one action. Returns whether it changed anything the screen should say.
pub fn apply(app: &mut CantusApp, a: &SurfaceAction) -> bool {
    match a {
        // A fader or a V-pot: the surfaces' gesture (`service::gesture_move`), one undo step
        // from the first touch to the last release.
        SurfaceAction::Gain { .. } | SurfaceAction::Pan { .. } => match move_command(app, a) {
            Some((id, cmd)) => super::service::gesture_move(app, id, cmd),
            None => false,
        },
        SurfaceAction::Mute { target, on } => match strip_id(app, *target) {
            Some(Some(id)) => app.exec(Command::SetStripMute { id, muted: *on }),
            _ => false,
        },
        SurfaceAction::Solo { target, on } => match strip_id(app, *target) {
            Some(Some(id)) => app.exec(Command::SetStripSolo { id, soloed: *on }),
            _ => false,
        },
        SurfaceAction::Arm { target, on } => {
            let Some(Some(id)) = strip_id(app, *target) else {
                return false;
            };
            let tracks: Vec<_> = app
                .project()
                .tracks
                .iter()
                .filter(|t| t.strip == Some(id) && t.armed != *on)
                .map(|t| t.id)
                .collect();
            let mut any = false;
            for t in tracks {
                any |= app.exec(Command::SetTrackArmed { id: t, armed: *on });
            }
            any
        }
        SurfaceAction::Select(t) => match strip_id(app, *t) {
            Some(Some(id)) => {
                app.selected_strip = Some(id);
                app.selected_strips = vec![id];
                true
            }
            _ => false,
        },
        // A touch: the gesture ends when the last fader is let go (`service::feed`), never
        // the mouse's.
        SurfaceAction::Touch { .. } => false,
        SurfaceAction::Play => run(app, Action::PlayPause),
        SurfaceAction::Stop => run(app, Action::Stop),
        SurfaceAction::Record => run(app, Action::Record),
        SurfaceAction::Cycle => run(app, Action::ToggleLoop),
        SurfaceAction::Click => run(app, Action::ToggleMetronome),
        SurfaceAction::Undo => run(app, Action::Undo),
        SurfaceAction::Redo => run(app, Action::Redo),
        SurfaceAction::Save => run(app, Action::Save),
        SurfaceAction::Marker => run(app, Action::AddMarker),
        SurfaceAction::PreviousMarker => run(app, Action::PreviousMarker),
        SurfaceAction::NextMarker => run(app, Action::NextMarker),
        SurfaceAction::ReturnToZero => {
            app.seek(Ticks::ZERO);
            true
        }
        SurfaceAction::Jog(d) => {
            move_playhead(app, i64::from(*d) * JOG_TICKS);
            true
        }
        // Held rewind/forward and scrub are the service's (it repeats them each frame).
        SurfaceAction::Rewind(_) | SurfaceAction::Forward(_) | SurfaceAction::Scrub(_) => false,
    }
}

/// A fader's or a V-pot's move as the command it sets and the target it moves (the gesture's
/// key of it).
pub fn move_command(app: &CantusApp, a: &SurfaceAction) -> Option<(String, Command)> {
    match a {
        SurfaceAction::Gain { target, db } => match strip_id(app, *target)? {
            Some(id) => Some((
                format!("gain {}", id.0),
                Command::SetStripGain { id, gain_db: *db },
            )),
            None => Some((
                "gain master".into(),
                Command::SetMasterGain { gain_db: *db },
            )),
        },
        SurfaceAction::Pan { target, pan } => match strip_id(app, *target)? {
            Some(id) => Some((
                format!("pan {}", id.0),
                Command::SetStripPan { id, pan: *pan },
            )),
            None => Some(("pan master".into(), Command::SetMasterPan { pan: *pan })),
        },
        _ => None,
    }
}

fn run(app: &mut CantusApp, action: Action) -> bool {
    app.run_action(action);
    true
}

/// The playhead moved by `ticks` (never before the start).
pub fn move_playhead(app: &mut CantusApp, ticks: i64) {
    let now = app.engine.position();
    app.seek(Ticks((now.0 + ticks).max(0)));
}
