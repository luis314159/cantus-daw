//! Concentus through the whole engine (ADR 0055): a wire at its defaults and at mix 0 in
//! every mode, the same render at every block size while it works, and live equal to
//! offline.

mod support;
#[path = "support/modfx.rs"]
mod modfx;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use modfx::*;
use support::*;

#[test]
fn concentus_at_its_defaults_renders_exactly_as_no_concentus() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    add_effect(&mut p, EffectKind::Chorus);
    assert_null(&reference, &render_of(&p), 0.0, "Concentus at its defaults");
}

#[test]
fn at_mix_zero_every_mode_is_a_wire() {
    let reference = render_of(&demo_project());
    for mode in 0..8 {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::Chorus);
        set_all(
            &mut p,
            fx,
            &[("mode", mode as f32), ("tone", 4_000.0), ("voices", 4.0)],
        );
        assert_null(&reference, &render_of(&p), 0.0, &format!("mode {mode}"));
    }
}

#[test]
fn the_render_does_not_depend_on_the_block_size_while_it_works() {
    for mode in [0.0, 2.0, 5.0, 7.0] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::Chorus);
        set_all(
            &mut p,
            fx,
            &[
                ("mode", mode),
                ("mix", 0.5),
                ("sync", 1.0),
                ("voices", 4.0),
                ("out", -2.0),
            ],
        );
        let reference = render_with(&p, 512);
        assert!(reference != render_with(&demo_project(), 512), "it works");
        for block in [1usize, 7, 64, 1000, 4096] {
            assert_null(
                &reference,
                &render_with(&p, block),
                0.0,
                &format!("mode {mode}, block {block}"),
            );
        }
    }
}

#[test]
fn live_playback_renders_what_the_offline_render_does() {
    for mode in [1.0, 7.0] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::Chorus);
        set_all(&mut p, fx, &[("mode", mode), ("mix", 0.6), ("sync", 1.0)]);
        let offline = render_of(&p);
        assert_null(
            &offline,
            &realtime(&p, offline.len()),
            0.0,
            &format!("mode {mode}"),
        );
    }
}
