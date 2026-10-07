//! What the soak found, each pinned by a test (round 22, *soak*).
//!
//! 1. **An effect removed and put back by undo before the engine caught up stayed
//!    silent.** The handle remembers which effect units the engine holds; the remove's
//!    snapshot makes the engine throw the unit away, but the undo's snapshot went out
//!    before that news came back, so no unit was sent again — and nothing re-sent it until
//!    the next edit. An undo storm (or a fast Ctrl+Z after a delete) left inserts
//!    bypassed without a word. The application now re-sends the snapshot when an effect
//!    of the project is not known to the engine (`CantusApp::sync_engine`); the engine's
//!    side of it is reported to the integrator.

use cantus_core::{EffectKind, Project, Ticks};
use cantus_engine::EngineHandle;
use cantus_ui::CantusApp;

fn project() -> (Project, cantus_core::StripId, cantus_core::EffectId) {
    let mut p = Project::new("fx");
    let mut t = p.make_track("Bass");
    let mut clip = p.make_clip("Bass", Ticks::ZERO, Ticks::beats(8));
    let n = p.make_note(Ticks::ZERO, Ticks::beats(4), 40, 100);
    clip.notes.push(n);
    t.clips.push(clip);
    let comp = p.make_effect(EffectKind::Compressor);
    let id = comp.id;
    let strip = t.strip.expect("a strip");
    p.strip_mut(strip).unwrap().effects.push(comp);
    p.tracks.push(t);
    (p, strip, id)
}

#[test]
fn an_effect_put_back_by_a_fast_undo_is_running_again() {
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
    let (p, strip, fx) = project();
    let (h, mut proc) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::from_parts(Some(p), h, false);
    let mut out = vec![0f32; 512];
    let mut cycle = |app: &mut CantusApp| {
        proc.process(&mut out, 2);
        app.engine.poll();
        app.sync_engine();
    };
    for _ in 0..3 {
        cycle(&mut app);
    }
    assert!(app.engine.knows_effect(fx));
    let dropped = app.engine.stats().dropped_events;

    // Remove and undo inside one engine round-trip: both snapshots are queued before the
    // engine has handed the removed unit back.
    app.remove_effect(strip, fx);
    app.sync_engine();
    app.undo();
    app.sync_engine();
    for _ in 0..6 {
        cycle(&mut app);
    }
    assert!(
        app.engine.knows_effect(fx),
        "the effect is back in the project but the engine has no unit for it"
    );
    // And it stays served: no snapshot after this one finds its unit missing.
    let settled = app.engine.stats().dropped_events;
    for _ in 0..6 {
        cycle(&mut app);
    }
    assert_eq!(app.engine.stats().dropped_events, settled);
    assert!(settled >= dropped);
}
