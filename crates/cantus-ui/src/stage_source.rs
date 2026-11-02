//! Where the stage's poses come from (ADR 0091 §1).
//!
//! The dancers live on the interface (`crate::dance_live`, the dancer's): every track
//! hosting a dancer, its character, its pose at the position **heard** (ADR 0022) from the
//! engine's feed — its numbers after automation and modulation, the notes it was given —
//! or, before its unit arrives, from the transport and its own numbers. The stage takes
//! that list once a frame and adds what it needs to show it: the track's name, and where it
//! shows (`view.mode`, `view.top`).
//!
//! The companion window dances to the transport on its own (`transport_frame`), with a
//! character chosen in it (`resolve`).

use super::{StageDancer, StageState, ViewMode};
use crate::app::CantusApp;
use cantus_core::{Ticks, PPQ};
use cantus_dance::character::Character;
use cantus_dance::{cast, DanceFrame};
use std::sync::Arc;

/// The character a `character` value names: one of the cast (`builtin:<id>`), else the
/// default character (a missing one draws the default and says so — never an error).
pub fn resolve(id: &str) -> Arc<dyn Character> {
    cast::by_id(id).unwrap_or_else(cast::default_character)
}

/// The song where it is heard: position, seconds, tempo, playing, and the bar it is in.
pub fn transport_frame(app: &CantusApp) -> DanceFrame {
    let p = app.project();
    let at = app.engine.position().max(Ticks::ZERO);
    let map = crate::views::bars::map(p);
    let tick = at.0 as f64;
    let (bar, _, _) = map.bbt(at);
    let bar_start = map.bar_start(at);
    let bar_len = map.bar_end(at) - bar_start;
    DanceFrame {
        beats: tick / PPQ as f64,
        seconds: map.seconds_at(tick),
        tempo_bpm: map.bpm_at(tick) as f32,
        playing: app.engine.is_playing(),
        bar: (bar - 1).max(0) as i32,
        bar_start_beats: bar_start.0 as f64 / PPQ as f64,
        bar_beats: (bar_len.0 as f64 / PPQ as f64) as f32,
        ..DanceFrame::default()
    }
}

/// Gather this frame's dancers into `st.dancers`: the live dancers that show, in track
/// order.
pub fn gather(app: &CantusApp, st: &mut StageState) {
    let mut out = std::mem::take(&mut st.dancers);
    out.clear();
    for d in crate::dance_live::DanceLive::dancers(app) {
        let mode = ViewMode::of(f32::from(d.view_mode));
        if mode == ViewMode::Hidden {
            continue;
        }
        let name = app.project().track(d.track).map_or("", |t| t.name.as_str());
        out.push(StageDancer {
            track: d.track,
            name: st.name(d.track, name),
            character: d.character,
            pose: d.pose,
            mode,
            top: d.on_top,
        });
    }
    st.dancers = out;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_character_draws_the_default() {
        let d = cast::default_character();
        assert_eq!(resolve("builtin:nobody-here").info().id, d.info().id);
        assert_eq!(resolve("file:Characters/gone.json").info().id, d.info().id);
        let default = cantus_core::inst::dancer::DEFAULT_CHARACTER;
        assert_eq!(resolve(default).info().id, d.info().id);
    }

    #[test]
    fn modes_read_and_write_their_values() {
        for m in ViewMode::ALL {
            assert_eq!(ViewMode::of(m.value()), m);
        }
        assert_eq!(ViewMode::of(7.0), ViewMode::Floating);
    }
}
