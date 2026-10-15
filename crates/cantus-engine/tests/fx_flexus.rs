//! Flexus through the whole engine (ADR 0055): a wire at its defaults and at mix 0; through
//! zero, the same render later by exactly the latency it reports (delay compensation); the
//! centre-point null across the whole engine; the same render at every block size while it
//! works; and live equal to offline.

#[path = "support/modfx.rs"]
mod modfx;
mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use modfx::*;
use support::*;

#[test]
fn flexus_at_its_defaults_is_the_render_later_by_its_latency() {
    // Its constant latency (the through-zero centre, in both paths) is reported, so the
    // strip comes out exactly that much later and otherwise untouched.
    let base = lead_only();
    let reference = render_of(&base);
    let mut p = base.clone();
    add_effect(&mut p, EffectKind::Flanger);
    let out = render_of(&p);
    let shift = 2 * 240;
    assert!(out[..shift].iter().all(|x| *x == 0.0));
    assert_null(
        &reference[..reference.len() - shift],
        &out[shift..],
        0.0,
        "Flexus at its defaults",
    );
}

/// The switch moves nothing delay compensation planned: both settings report the same.
#[test]
fn the_through_zero_switch_keeps_the_latency() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Flanger);
    for tz in [0.0, 1.0] {
        set(&mut p, fx, "tz", tz);
        let slot = strip_of(&mut p, 0).effects[0].clone();
        assert_eq!(
            cantus_engine::EffectUnit::new(&slot, 48_000.0).latency(),
            240
        );
    }
}

#[test]
fn through_zero_at_mix_zero_is_the_render_later_by_its_latency() {
    let base = lead_only();
    let reference = render_of(&base);
    let mut p = base.clone();
    let fx = add_effect(&mut p, EffectKind::Flanger);
    set_all(&mut p, fx, &[("tz", 1.0), ("depth", 8.0), ("fb", 0.5)]);
    let slot = strip_of(&mut p, 0).effects[0].clone();
    assert_eq!(
        cantus_engine::EffectUnit::new(&slot, 48_000.0).latency(),
        240
    );
    let out = render_of(&p);
    let shift = 2 * 240;
    assert!(out[..shift].iter().all(|x| *x == 0.0));
    assert_null(
        &reference[..reference.len() - shift],
        &out[shift..],
        0.0,
        "through zero",
    );
}

/// The centre point through the whole engine: the lead's strip comes out silent, to the bit.
#[test]
fn the_centre_point_silences_the_strip_to_the_bit() {
    let mut p = lead_only();
    let fx = add_effect(&mut p, EffectKind::Flanger);
    set_all(
        &mut p,
        fx,
        &[
            ("tz", 1.0),
            ("mode", 2.0),
            ("manual", 0.5),
            ("invert", 1.0),
            ("mix", 0.5),
        ],
    );
    let out = render_of(&p);
    assert!(out.iter().all(|x| x.to_bits() == 0), "exact silence");
}

#[test]
fn the_render_does_not_depend_on_the_block_size_while_it_works() {
    for (mode, tz) in [(0.0, 0.0), (0.0, 1.0), (1.0, 0.0)] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::Flanger);
        set_all(
            &mut p,
            fx,
            &[
                ("mode", mode),
                ("tz", tz),
                ("mix", 0.5),
                ("fb", -0.6),
                ("sync", 1.0),
                ("div", 8.0),
                ("spread", 90.0),
            ],
        );
        let reference = render_with(&p, 512);
        for block in [1usize, 7, 64, 1000, 4096] {
            assert_null(
                &reference,
                &render_with(&p, block),
                0.0,
                &format!("mode {mode} tz {tz}, block {block}"),
            );
        }
    }
}

#[test]
fn live_playback_renders_what_the_offline_render_does() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Flanger);
    set_all(
        &mut p,
        fx,
        &[("mix", 0.5), ("fb", 0.6), ("sync", 1.0), ("tz", 1.0)],
    );
    let offline = render_of(&p);
    assert_null(&offline, &realtime(&p, offline.len()), 0.0, "live");
}
