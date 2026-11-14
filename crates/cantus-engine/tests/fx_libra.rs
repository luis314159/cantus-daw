//! Libra through the whole engine (ADR 0133, provisional): a wire at its defaults, the gain
//! heard exactly, a delay exact to the sample, the same render at every block size, live
//! equal to offline.

#[path = "support/modfx.rs"]
mod modfx;
mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use modfx::*;
use support::*;

#[test]
fn libra_at_its_defaults_renders_exactly_as_no_libra() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    add_effect(&mut p, EffectKind::Utility);
    assert_null(&reference, &render_of(&p), 0.0, "Libra at its defaults");
}

#[test]
fn minus_six_db_is_half_and_the_delay_is_whole_samples() {
    // One track, so the master is that strip's sound alone.
    let reference = render_of(&lead_only());
    let mut p = lead_only();
    let fx = add_effect(&mut p, EffectKind::Utility);
    set_all(&mut p, fx, &[("out", -6.020_6), ("delay.l", 1.0)]);
    let out = render_of(&p);
    // The left 48 samples late (1 ms at 48 kHz), both at half: once the gain has glided.
    let frames = reference.len() / 2;
    let mut worst = 0f32;
    for f in 4_800..frames {
        let want_l = reference[2 * (f - 48)] * 0.5;
        let want_r = reference[2 * f + 1] * 0.5;
        worst = worst
            .max((out[2 * f] - want_l).abs())
            .max((out[2 * f + 1] - want_r).abs());
    }
    assert!(worst < 1e-5, "{worst}");
}

fn busy(p: &mut Project) -> EffectId {
    let fx = add_effect(p, EffectKind::Utility);
    set_all(
        p,
        fx,
        &[
            ("dc", 1.0),
            ("chan", 1.0),
            ("pol.r", 1.0),
            ("delay.r", 2.2),
            ("width", 1.5),
            ("pan", 0.3),
            ("out", -3.0),
        ],
    );
    fx
}

#[test]
fn the_render_does_not_depend_on_the_block_size() {
    let mut p = demo_project();
    busy(&mut p);
    let reference = render_with(&p, 512);
    for block in [1usize, 7, 64, 1000, 4096] {
        assert_null(
            &reference,
            &render_with(&p, block),
            0.0,
            &format!("block {block}"),
        );
    }
}

#[test]
fn live_playback_renders_what_the_offline_render_does() {
    let mut p = demo_project();
    busy(&mut p);
    let offline = render_of(&p);
    assert_null(&offline, &realtime(&p, offline.len()), 0.0, "live");
}
