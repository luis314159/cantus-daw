//! Vibrans through the whole engine (ADR 0055): a wire at its defaults and at depth 0, the
//! same render at every block size while it works, and live equal to offline.

mod support;
#[path = "support/modfx.rs"]
mod modfx;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use modfx::*;
use support::*;

#[test]
fn vibrans_at_its_defaults_renders_exactly_as_no_vibrans() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    add_effect(&mut p, EffectKind::Vibrato);
    assert_null(&reference, &render_of(&p), 0.0, "Vibrans at its defaults");
}

#[test]
fn at_depth_zero_it_is_a_wire_whatever_else_is_set() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Vibrato);
    set_all(
        &mut p,
        fx,
        &[("sync", 1.0), ("shape", 1.0), ("rise", 300.0), ("spread", 90.0)],
    );
    assert_null(&reference, &render_of(&p), 0.0, "depth 0");
}

fn busy(p: &mut Project, fx: EffectId) {
    set_all(
        p,
        fx,
        &[
            ("depth", 45.0),
            ("rate", 6.0),
            ("rise", 150.0),
            ("spread", 60.0),
            ("mix", 0.8),
            ("out", -1.0),
        ],
    );
}

#[test]
fn the_render_does_not_depend_on_the_block_size_while_it_works() {
    for sync in [0.0, 1.0] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::Vibrato);
        busy(&mut p, fx);
        set(&mut p, fx, "sync", sync);
        let reference = render_with(&p, 512);
        assert!(reference != render_with(&demo_project(), 512), "it works");
        for block in [1usize, 7, 64, 1000, 4096] {
            assert_null(
                &reference,
                &render_with(&p, block),
                0.0,
                &format!("sync {sync}, block {block}"),
            );
        }
    }
}

#[test]
fn live_playback_renders_what_the_offline_render_does() {
    for sync in [0.0, 1.0] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::Vibrato);
        busy(&mut p, fx);
        set(&mut p, fx, "sync", sync);
        let offline = render_of(&p);
        assert_null(&offline, &realtime(&p, offline.len()), 0.0, &format!("sync {sync}"));
    }
}
