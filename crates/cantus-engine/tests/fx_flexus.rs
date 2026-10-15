//! Flexus through the whole engine (ADR 0055): a wire at its defaults and at mix 0, with no
//! latency; through zero, the same render later by exactly the latency it reports (delay
//! compensation), and an automated switch re-planned in line; the centre-point null across
//! the whole engine; the same render at every block size while it works; and live equal
//! to offline.

#[path = "support/modfx.rs"]
mod modfx;
mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use modfx::*;
use support::*;

#[test]
fn flexus_at_its_defaults_renders_exactly_as_no_flexus() {
    // Mix 0, through zero off: a wire with no latency, to the bit.
    let base = demo_project();
    let reference = render_of(&base);
    let mut p = base.clone();
    let fx = add_effect(&mut p, EffectKind::Flanger);
    let slot = strip_of(&mut p, 0).effects[0].clone();
    assert_eq!(cantus_engine::EffectUnit::new(&slot, 48_000.0).latency(), 0);
    assert_null(&reference, &render_of(&p), 0.0, "Flexus at its defaults");
    // Mix 0 with everything else busy: still a wire.
    set_all(&mut p, fx, &[("depth", 8.0), ("fb", 0.5), ("sync", 1.0)]);
    assert_null(&reference, &render_of(&p), 0.0, "Flexus at mix 0");
}

/// Through zero the unit reports its centre; plain, nothing.
#[test]
fn the_through_zero_switch_sets_the_latency() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Flanger);
    for (tz, lat) in [(0.0, 0), (1.0, 240)] {
        set(&mut p, fx, "tz", tz);
        let slot = strip_of(&mut p, 0).effects[0].clone();
        assert_eq!(
            cantus_engine::EffectUnit::new(&slot, 48_000.0).latency(),
            lat
        );
    }
}

/// The switch automated — through zero on at beat 4, off at beat 6, at mix 0 — re-plans
/// compensation as an edit would: inside the stretch the whole mix is the reference later
/// by the centre (the other strips wait too), and in line again after it.
#[test]
fn an_automated_through_zero_switch_keeps_the_mix_in_line() {
    let base = demo_project();
    let reference = render_of(&base);
    let mut p = base.clone();
    let fx = add_effect(&mut p, EffectKind::Flanger);
    let point = |beat: i64, v: f32| {
        let mut a = AutoPoint::new(Ticks::beats(beat), v);
        a.shape = Shape::Hold;
        a
    };
    let clip = p.make_automation_clip(
        "tz",
        AutoTarget::Effect {
            effect: fx,
            key: "tz".into(),
        },
        Ticks::ZERO,
        Ticks::beats(8),
        vec![point(0, 0.0), point(4, 1.0), point(6, 0.0)],
    );
    p.tracks[0].clips.push(clip);
    let out = render_of(&p);
    let lat = 240;
    let beat = BEAT as usize;
    // The compensation lines of the other strips fill once the switch lands.
    for f in (4 * beat + 4 * lat)..(6 * beat - 16) {
        for c in 0..2 {
            assert_eq!(
                out[2 * f + c].to_bits(),
                reference[2 * (f - lat) + c].to_bits(),
                "frame {f} (channel {c}) through zero"
            );
        }
    }
    for f in (6 * beat + 4 * lat + 16)..(8 * beat) {
        assert_eq!(
            out[2 * f].to_bits(),
            reference[2 * f].to_bits(),
            "frame {f} after"
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
