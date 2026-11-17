//! The controller presets: each valid and compiled, applied keeping the modulator's own,
//! recognised again, named in both languages, and each formula with the shape it says.

use super::*;
use crate::automation::AutoTarget;
use crate::model::{ModulatorId, StripId};
use crate::modulator::{ModRoute, Modulator};

fn modulator(kind: ModKind, routes: usize) -> Modulator {
    let mut m = Modulator::new(ModulatorId(7), kind);
    m.name = "Mine".into();
    m.amount = 0.8;
    m.routes = (0..routes)
        .map(|k| ModRoute {
            target: [
                AutoTarget::StripPan(StripId(3)),
                AutoTarget::StripWidth(StripId(3)),
                AutoTarget::StripGain(StripId(3)),
            ][k]
                .clone(),
            depth: 0.5,
        })
        .collect();
    m
}

#[test]
fn every_preset_is_valid_and_every_formula_compiles() {
    assert!(all_compile());
    assert_eq!(of_kind(ModKind::XyPad).count(), 4);
    assert_eq!(of_kind(ModKind::Formula).count(), 9);
    for p in PRESETS {
        let src = (p.source)();
        assert_eq!(src.kind(), p.kind, "{}", p.id);
        assert!(src.is_valid(), "{}", p.id);
        assert!(
            p.links.iter().all(|l| l.output < p.kind.outputs()),
            "{}",
            p.id
        );
        assert_eq!(by_id(p.id), Some(p));
        if let ModSource::Formula(f) = &src {
            formula::Program::compile(&f.text).unwrap_or_else(|e| panic!("{}: {e:?}", p.id));
        }
    }
    assert!(by_id("formula.nothing").is_none());
}

#[test]
fn apply_keeps_the_modulator_s_own_and_is_matched_again() {
    for p in PRESETS {
        for routes in [0, 1, 3] {
            let mut m = modulator(p.kind, routes);
            if let ModSource::XyPad(x) = &mut m.source {
                x.look = 2;
            }
            if let ModSource::Formula(f) = &mut m.source {
                f.look = 1;
            }
            let look = match &m.source {
                ModSource::XyPad(x) => x.look,
                ModSource::Formula(f) => f.look,
                _ => 0,
            };
            let a = apply(&m, p);
            assert!(a.is_valid(), "{} with {routes} routes", p.id);
            assert_eq!(a.id, m.id);
            assert_eq!(a.name, m.name);
            assert_eq!(a.routes, m.routes);
            assert_eq!(a.amount, m.amount);
            assert_eq!(a.muted, m.muted);
            assert_eq!(look_of(&a.source), look, "{}: the face's look kept", p.id);
            assert_eq!(matching(&a), Some(p), "{}", p.id);
            // The links: one per route the preset has a link for, in order.
            for (k, r) in a.routes.iter().enumerate() {
                let (output, range) = a.source.link(&r.target);
                match p.links.get(k) {
                    Some(l) => assert_eq!((output, range), (l.output, l.range), "{}", p.id),
                    None => assert_eq!((output, range), (0, None)),
                }
            }
        }
    }
}

#[test]
fn a_preset_applied_over_another_kind_or_after_playing_is_still_named() {
    // An LFO turned into a pad by a preset.
    let lfo = modulator(ModKind::Lfo, 2);
    let p = by_id("xy.filter_sweep").unwrap();
    let a = apply(&lfo, p);
    assert!(a.is_valid());
    assert_eq!(matching(&a), Some(p));
    // Played: the puck moved, a knob turned — still the preset.
    let mut played = a.clone();
    played.source.set_input(0, 0.9);
    played.source.set_input(1, 0.2);
    assert_eq!(matching(&played), Some(p));
    let mut f = apply(&lfo, by_id("formula.sine").unwrap());
    f.source.set_input(0, 0.9);
    assert_eq!(matching(&f).map(|p| p.id), Some("formula.sine"));
    // A setting changed: no longer the preset.
    if let ModSource::XyPad(x) = &mut played.source {
        x.inertia_ms = 81.0;
    }
    assert_eq!(matching(&played), None);
    if let ModSource::Formula(x) = &mut f.source {
        x.text = "a".into();
    }
    assert_eq!(matching(&f), None);
    assert_eq!(matching(&lfo), None);
    // No two presets are the same settings.
    for p in PRESETS {
        let m = apply(&modulator(p.kind, 0), p);
        assert_eq!(matching(&m), Some(p));
    }
}

#[test]
fn tidy_links_drops_what_no_route_reads() {
    let p = by_id("xy.width_pan").unwrap();
    let mut m = apply(&modulator(ModKind::XyPad, 2), p);
    let ModSource::XyPad(x) = &m.source else {
        unreachable!()
    };
    assert_eq!(x.links.len(), 2);
    m.routes.remove(0);
    m.tidy_links();
    let ModSource::XyPad(x) = &m.source else {
        unreachable!()
    };
    assert_eq!(x.links.len(), 1);
    assert_eq!(x.links[0].target, m.routes[0].target);
    assert_eq!(x.links[0].output, 1);
    // An LFO has none to tidy.
    let mut l = modulator(ModKind::Lfo, 1);
    l.tidy_links();
    assert!(l.is_valid());
}

/// The formula's value at a song position, its knobs at `k` (120 BPM, played from 0).
fn at(text: &str, k: [f64; 3], beats: f64) -> f32 {
    let p = formula::Program::compile(text).unwrap();
    p.output(&formula::Env {
        a: k[0],
        b: k[1],
        c: k[2],
        beats,
        seconds: beats * 0.5,
        tempo: 120.0,
    })
}

fn knobs(id: &str) -> [f64; 3] {
    match (by_id(id).unwrap().source)() {
        ModSource::Formula(f) => f.inputs.map(f64::from),
        _ => unreachable!(),
    }
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

#[test]
fn the_waves_have_their_shapes_at_their_rate() {
    let k = knobs("formula.sine");
    // A bar (4 beats) a cycle at a = 0.34.
    for (beat, want) in [(0.0, 0.5), (1.0, 1.0), (2.0, 0.5), (3.0, 0.0), (5.0, 1.0)] {
        assert!(near(at(SINE, k, beat), want), "sine at {beat}");
    }
    for (beat, want) in [(0.0, 0.0), (1.0, 0.5), (2.0, 1.0), (3.0, 0.5)] {
        assert!(near(at(TRIANGLE, k, beat), want), "triangle at {beat}");
    }
    for (beat, want) in [(0.0, 0.0), (1.0, 0.25), (2.0, 0.5), (3.0, 0.75)] {
        assert!(near(at(SAW, k, beat), want), "saw at {beat}");
    }
    for (beat, want) in [(0.5, 1.0), (1.9, 1.0), (2.1, 0.0), (3.9, 0.0)] {
        assert!(near(at(SQUARE, k, beat), want), "square at {beat}");
    }
    // The depth knob: half depth swings half as far about the centre; the phase knob
    // moves the cycle on.
    assert!(near(at(SINE, [k[0], 0.0, 0.5], 1.0), 0.75));
    assert!(near(at(SINE, [k[0], 0.25, 1.0], 0.0), 1.0));
    // The rate knob: a beat a cycle at 0.67, four bars at 0.
    assert!(near(at(SINE, [0.67, 0.0, 1.0], 0.25), 1.0));
    assert!(near(at(SINE, [0.0, 0.0, 1.0], 4.0), 1.0));
    // The pulse: a beat a cycle, open for the width b.
    let k = knobs("formula.pulse");
    for (beat, want) in [(0.1, 1.0), (0.24, 1.0), (0.3, 0.0), (0.9, 0.0), (1.1, 1.0)] {
        assert!(near(at(PULSE, k, beat), want), "pulse at {beat}");
    }
    assert!(near(at(PULSE, [k[0], 0.75, 1.0], 0.6), 1.0));
}

#[test]
fn the_random_holds_each_beat_and_the_gate_opens_on_each() {
    let k = knobs("formula.random_beat");
    let v: Vec<f32> = (0..16)
        .map(|b| at(RANDOM_BEAT, k, f64::from(b) + 0.1))
        .collect();
    for b in 0..16 {
        let held = at(RANDOM_BEAT, k, f64::from(b) + 0.9);
        assert_eq!(v[b as usize], held, "held through beat {b}");
        assert!((0.0..=1.0).contains(&held));
    }
    let distinct = v.windows(2).filter(|w| w[0] != w[1]).count();
    assert!(distinct >= 12, "{distinct} changes in 16 beats");
    // Another sequence with b.
    let other: Vec<f32> = (0..16)
        .map(|b| at(RANDOM_BEAT, [k[0], 0.5, 1.0], f64::from(b) + 0.1))
        .collect();
    assert_ne!(v, other);
    let k = knobs("formula.gate_beats");
    for b in 0..4 {
        let b = f64::from(b);
        assert!(near(at(GATE_BEATS, k, b), 0.0), "closed on the beat's edge");
        assert!(
            near(at(GATE_BEATS, k, b + 0.25), 1.0),
            "open in its first half"
        );
        assert!(
            near(at(GATE_BEATS, k, b + 0.75), 0.0),
            "closed in its second"
        );
        assert!(near(at(GATE_BEATS, k, b + 0.49), 0.5), "a soft edge");
    }
    // A longer gate with b.
    assert!(near(at(GATE_BEATS, [k[0], 0.9, 1.0], 0.75), 1.0));
}

#[test]
fn the_envelope_strikes_each_bar_and_the_climb_rises_over_its_length() {
    let k = knobs("formula.env_bar");
    for bar in 0..3 {
        let b0 = 4.0 * f64::from(bar);
        assert!(near(at(ENV_BAR, k, b0), 0.0), "silent on the bar line");
        assert!(at(ENV_BAR, k, b0 + 0.12) > 0.9, "struck");
        assert!(at(ENV_BAR, k, b0 + 2.0) < 0.1, "decayed half a bar on");
        assert!(at(ENV_BAR, k, b0 + 1.0) > at(ENV_BAR, k, b0 + 2.0));
    }
    assert!(
        at(ENV_BAR, [k[0], 0.0, 1.0], 2.0) > 0.3,
        "a slow decay with b = 0"
    );
    let k = knobs("formula.climb");
    // 16 bars (64 beats) at a = 0.25, straight at b = 0.5.
    for (beat, want) in [
        (0.0, 0.0),
        (16.0, 0.25),
        (32.0, 0.5),
        (64.0, 1.0),
        (200.0, 1.0),
    ] {
        assert!(near(at(CLIMB, k, beat), want), "climb at {beat}");
    }
    assert!(
        at(CLIMB, [k[0], 0.0, 1.0], 32.0) > 0.8,
        "bent early with b = 0"
    );
    assert!(
        at(CLIMB, [k[0], 1.0, 1.0], 32.0) < 0.1,
        "bent late with b = 1"
    );
    assert!(near(at(CLIMB, [0.0, 0.5, 1.0], 2.0), 0.5), "a bar at a = 0");
}

#[test]
fn the_figure_eight_loops_over_two_bars() {
    let ModSource::XyPad(x) = (by_id("xy.figure_eight").unwrap().source)() else {
        unreachable!()
    };
    let path = x.path.as_ref().unwrap();
    assert_eq!(path.points.len(), FIGURE_EIGHT_POINTS);
    assert!(x.play_path && path.smooth);
    let close = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).abs() < 1e-5 && (a[1] - b[1]).abs() < 1e-5;
    assert!(close(path.at(0.0), [0.5, 0.5]));
    assert!(close(path.at(2.0), [0.9, 0.5]));
    assert!(close(path.at(6.0), [0.1, 0.5]));
    assert!(close(path.at(8.0), path.at(0.0)));
    // Its two lobes, the right one and the left one.
    assert!(path.at(1.0)[1] > 0.8 && path.at(1.0)[0] > 0.7);
    assert!(path.at(5.0)[1] > 0.8 && path.at(5.0)[0] < 0.3);
}

/// Every preset is named and described in English and in Spanish.
#[test]
fn every_preset_has_its_words_in_both_languages() {
    let en = include_str!("../../cantus-ui/locales/fx/controllers.en.txt");
    let es = include_str!("../../cantus-ui/locales/fx/controllers.es.txt");
    let keys = |text: &'static str| -> Vec<&'static str> {
        text.lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .filter_map(|l| l.split_once('=').map(|(k, v)| (k.trim(), v.trim())))
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, _)| k)
            .collect()
    };
    let (en, es) = (keys(en), keys(es));
    for p in PRESETS {
        for key in [
            format!("fxd.preset.{}", p.id),
            format!("fxd.preset.{}.what", p.id),
        ] {
            assert!(en.contains(&key.as_str()), "{key} in English");
            assert!(es.contains(&key.as_str()), "{key} in Spanish");
        }
    }
}
