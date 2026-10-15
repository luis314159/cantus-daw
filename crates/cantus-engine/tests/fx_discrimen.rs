//! Discrimen through the whole engine (ADR 0056): the parts summed unprocessed are the
//! input to the bit — in zero-latency mode the render itself, with the look-ahead the same
//! render later by exactly the latency it reports; the same render at every block size
//! while it works; live equal to offline.

#[path = "support/modfx.rs"]
mod modfx;
mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use modfx::*;
use support::*;

#[test]
fn discrimen_at_its_defaults_renders_exactly_as_no_discrimen() {
    let reference = render_of(&demo_project());
    for sens in [0.0, 0.5, 1.0] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::TransientSplit);
        set_all(&mut p, fx, &[("sens", sens), ("speed", 3.0)]);
        assert_null(&reference, &render_of(&p), 0.0, &format!("sens {sens}"));
    }
}

#[test]
fn the_look_ahead_is_the_render_later_by_its_latency() {
    let base = demo_project();
    let reference = render_of(&base);
    let mut p = base.clone();
    let fx = add_effect(&mut p, EffectKind::TransientSplit);
    set_all(&mut p, fx, &[("mode", 1.0)]);
    let slot = strip_of(&mut p, 0).effects[0].clone();
    let lat = cantus_engine::EffectUnit::new(&slot, 48_000.0).latency();
    assert_eq!(lat, 96, "2 ms at 48 kHz");
    let out = render_of(&p);
    let shift = 2 * lat;
    assert!(out[..shift].iter().all(|x| *x == 0.0));
    assert_null(
        &reference[..reference.len() - shift],
        &out[shift..],
        0.0,
        "look-ahead",
    );
}

fn busy(p: &mut Project, fx: EffectId, mode: f32) {
    set_all(
        p,
        fx,
        &[
            ("mode", mode),
            ("sens", 0.7),
            ("t.gain", 4.0),
            ("t.comp", 0.4),
            ("t.tilt", 2.0),
            ("b.gain", -3.0),
            ("b.comp", 0.5),
            ("b.tilt", -2.0),
            ("out", -1.0),
        ],
    );
}

#[test]
fn the_render_does_not_depend_on_the_block_size_while_it_works() {
    for mode in [0.0, 1.0] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::TransientSplit);
        busy(&mut p, fx, mode);
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
    for mode in [0.0, 1.0] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::TransientSplit);
        busy(&mut p, fx, mode);
        let offline = render_of(&p);
        assert_null(
            &offline,
            &realtime(&p, offline.len()),
            0.0,
            &format!("mode {mode}"),
        );
    }
}
