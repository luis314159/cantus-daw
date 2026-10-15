//! Impetus through the whole engine (ADR 0056): a wire at its defaults and at 0 % attack
//! and sustain in every band and with a key; the same render at every block size while it
//! shapes, keyed or not; live equal to offline; and the key changes what is shaped.

#[path = "support/modfx.rs"]
mod modfx;
mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use modfx::*;
use support::*;

#[test]
fn impetus_at_its_defaults_renders_exactly_as_no_impetus() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    add_effect(&mut p, EffectKind::Transient);
    assert_null(&reference, &render_of(&p), 0.0, "Impetus at its defaults");
}

#[test]
fn at_zero_amounts_every_band_and_a_key_are_a_wire() {
    let reference = render_of(&demo_project());
    for band in [0.0, 1.0, 2.0] {
        for keyed in [false, true] {
            let mut p = demo_project();
            let fx = add_effect(&mut p, EffectKind::Transient);
            set_all(&mut p, fx, &[("band", band), ("speed", 3.0), ("xover", 900.0)]);
            if keyed {
                let bass = p.tracks[1].strip.unwrap();
                strip_of(&mut p, 0).effects[0].sidechain = Some(bass);
            }
            assert_null(
                &reference,
                &render_of(&p),
                0.0,
                &format!("band {band} keyed {keyed}"),
            );
        }
    }
}

fn busy(p: &mut Project, fx: EffectId, band: f32) {
    set_all(
        p,
        fx,
        &[
            ("attack", 0.8),
            ("sustain", -0.5),
            ("speed", 8.0),
            ("band", band),
            ("xover", 700.0),
            ("out", -1.0),
        ],
    );
}

#[test]
fn the_render_does_not_depend_on_the_block_size_while_it_shapes() {
    for (band, keyed) in [(0.0, false), (1.0, true), (2.0, false)] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::Transient);
        busy(&mut p, fx, band);
        if keyed {
            let bass = p.tracks[1].strip.unwrap();
            strip_of(&mut p, 0).effects[0].sidechain = Some(bass);
        }
        let reference = render_with(&p, 512);
        assert!(reference != render_with(&demo_project(), 512), "it shapes");
        for block in [1usize, 7, 64, 1000, 4096] {
            assert_null(
                &reference,
                &render_with(&p, block),
                0.0,
                &format!("band {band} keyed {keyed}, block {block}"),
            );
        }
    }
}

#[test]
fn live_playback_renders_what_the_offline_render_does() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Transient);
    busy(&mut p, fx, 1.0);
    let offline = render_of(&p);
    assert_null(&offline, &realtime(&p, offline.len()), 0.0, "live = offline");
}

#[test]
fn a_key_changes_what_is_shaped() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Transient);
    busy(&mut p, fx, 0.0);
    let own = render_of(&p);
    let bass = p.tracks[1].strip.unwrap();
    strip_of(&mut p, 0).effects[0].sidechain = Some(bass);
    let keyed = render_of(&p);
    assert!(keyed != own, "the key drives the detector");
}
