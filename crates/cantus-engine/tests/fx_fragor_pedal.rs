//! Fragor's five circuits as pedals of their own (round 41, ADR 0152): **each pedal is Fragor
//! with only its slot filled, to the bit.** For every circuit, the pedal's unit
//! (`fx::fragor_pedal`, built with its one circuit) and Fragor's own unit, configured with
//! the one-slot Fragor the pedal is (`fragor_pedal::as_fragor`), are driven side by side
//! through the plug-in contract: the same input, the same settings at the same samples
//! (defaults, noon, heavy, drawn at random, the circuit taken out and put back, the gate,
//! input and output, mix, mono and the three oversampling factors changing mid-note), a seek
//! (`snap`) and a reset, silence long enough to rest (and skipped while resting, as the engine
//! does), at blocks of 1, 64, 1024 and random splittings. Every block: the samples, the
//! latency, rest and its reason, and all 64 numbers of the telemetry — the same bits.

mod support;
use cantus_core::fx::fragor_pedal::{as_fragor, knobs, switch, KINDS};
use cantus_core::{EffectKind, EffectSpec};
use cantus_dsp::math;
use cantus_engine::fx::{make, FxUnit};
use support::heavy::Rng;

const SR: f32 = 48_000.0;

/// A guitar's phrase at a guitar's level (some hundreds of millivolts): plucked notes with
/// their harmonics, hiss under them for the gate, and 1.2 s of exact silence (the pedals
/// rest after a second of it), then one more note.
fn phrase(frames: usize) -> (Vec<f32>, Vec<f32>) {
    let mut seed = 0x51ce_d00du32;
    let mut noise = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32 * 2.0 - 1.0
    };
    let notes = [
        (0.0f32, 110.0f32),
        (0.35, 164.8),
        (0.7, 220.0),
        (2.45, 146.8),
    ];
    let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
    for i in 0..frames {
        let t = i as f32 / SR;
        if (1.2..2.4).contains(&t) {
            continue; // exact silence
        }
        let mut x = noise() * 0.002;
        for (at, f) in notes {
            if t >= at {
                let u = t - at;
                let env = math::exp(-3.0 * u) * (1.0 - math::exp(-u * 2000.0));
                let w = std::f32::consts::TAU * f * u;
                x += env
                    * 0.3
                    * (math::sin(w) + 0.4 * math::sin(2.0 * w) + 0.2 * math::sin(3.0 * w));
            }
        }
        l[i] = x;
        r[i] = x * 0.93 + noise() * 0.001;
    }
    (l, r)
}

/// What happens at a sample: new settings, a seek, a reset.
#[derive(Clone)]
enum Ev {
    Set(EffectSpec),
    Snap,
    Reset,
}

fn spec(kind: EffectKind, values: &[(&str, f32)]) -> EffectSpec {
    let mut s = EffectSpec::new(kind);
    for (k, v) in values {
        s.set(k, *v)
            .unwrap_or_else(|| panic!("{kind:?}: no key {k}"));
    }
    s
}

/// A pedal drawn at random: each parameter with probability `p`, the circuit in or not.
fn drawn(kind: EffectKind, rng: &mut Rng, in_: Option<bool>) -> EffectSpec {
    let mut s = support::heavy::random_effect(kind, 0.7, rng);
    let i = kind.fragor_circuit().unwrap();
    if let Some(on) = in_ {
        s.set(switch(i), if on { 1.0 } else { 0.0 });
    }
    // The gate drawn over its whole range closes on most of a phrase: keep it musical half
    // the time.
    if rng.chance(0.5) {
        s.set("gate", -90.0);
    }
    s
}

/// The scenarios for one circuit: (name, events at samples).
fn scenarios(kind: EffectKind) -> Vec<(String, Vec<(usize, Ev)>)> {
    let i = kind.fragor_circuit().unwrap();
    let (sw, k) = (switch(i), knobs(i));
    let s = |t: f32| (t * SR) as usize;
    let mut out = vec![
        (
            "defaults".to_string(),
            vec![(0, Ev::Set(EffectSpec::new(kind)))],
        ),
        ("noon".into(), vec![(0, Ev::Set(spec(kind, &[(sw, 1.0)])))]),
        (
            "heavy".into(),
            vec![(
                0,
                Ev::Set(spec(
                    kind,
                    &[
                        (sw, 1.0),
                        (k[0], 1.0),
                        (k[1], 0.15),
                        (k[2], 0.9),
                        ("gate", -50.0),
                        ("in", 12.0),
                        ("out", -6.0),
                        ("mix", 0.7),
                        ("mono", 1.0),
                        ("os", 2.0),
                    ],
                )),
            )],
        ),
        (
            "structure".into(),
            vec![
                (0, Ev::Set(spec(kind, &[(sw, 1.0), (k[0], 0.7)]))),
                (s(0.2), Ev::Set(spec(kind, &[(sw, 0.0), (k[0], 0.7)]))),
                (s(0.4), Ev::Set(spec(kind, &[(sw, 1.0), ("os", 0.0)]))),
                (
                    s(0.55),
                    Ev::Set(spec(kind, &[(sw, 1.0), ("os", 2.0), ("mono", 1.0)])),
                ),
                (
                    s(0.8),
                    Ev::Set(spec(kind, &[(sw, 1.0), ("mix", 0.3), ("in", -6.0)])),
                ),
                (s(1.0), Ev::Snap),
                (
                    s(1.05),
                    Ev::Set(spec(kind, &[(sw, 1.0), ("gate", -40.0), (k[2], 0.2)])),
                ),
                (s(2.5), Ev::Set(spec(kind, &[(sw, 0.0), ("gate", -60.0)]))),
            ],
        ),
    ];
    for seed in 0..3u64 {
        let mut rng = Rng::new(0xF4A6_0000 + seed * 977 + i as u64);
        let mut evs = vec![(0, Ev::Set(drawn(kind, &mut rng, Some(true))))];
        for t in [0.15, 0.3, 0.5, 0.8, 1.1, 2.42, 2.6] {
            let on = rng.chance(0.8);
            evs.push((s(t), Ev::Set(drawn(kind, &mut rng, Some(on)))));
        }
        evs.push((s(0.9), Ev::Snap));
        evs.push((s(2.5), Ev::Reset));
        evs.sort_by_key(|e| e.0);
        out.push((format!("drawn {seed}"), evs));
    }
    out
}

/// One side: the unit, how it is configured from a pedal's spec.
struct Side {
    unit: Box<dyn FxUnit>,
    map: fn(&EffectSpec) -> EffectSpec,
}

fn pedal(kind: EffectKind) -> Side {
    Side {
        unit: make(kind, SR).expect("a pedal unit"),
        map: |s| s.clone(),
    }
}

fn fragor() -> Side {
    Side {
        unit: make(EffectKind::Pedal, SR).expect("Fragor's unit"),
        map: as_fragor,
    }
}

/// Everything one block leaves to compare.
#[derive(PartialEq, Debug)]
struct Seen {
    l: Vec<u32>,
    r: Vec<u32>,
    latency: usize,
    rest: Option<&'static str>,
    tel: Vec<u32>,
}

/// Runs `evs` over `input` through `side`, cut into blocks by `next` (and at every event);
/// a block of silence into a resting unit is skipped, as the engine does.
fn run(
    side: &mut Side,
    input: &(Vec<f32>, Vec<f32>),
    evs: &[(usize, Ev)],
    mut next: impl FnMut() -> usize,
) -> Vec<Seen> {
    let n = input.0.len();
    let (mut at, mut e, mut seen) = (0, 0, Vec::new());
    while at < n {
        while e < evs.len() && evs[e].0 <= at {
            match &evs[e].1 {
                Ev::Set(s) => side.unit.configure(&(side.map)(s)),
                Ev::Snap => side.unit.snap(),
                Ev::Reset => side.unit.reset(),
            }
            e += 1;
        }
        let stop = evs.get(e).map_or(n, |x| x.0.min(n));
        let len = next().max(1).min(stop - at);
        let (mut l, mut r) = (
            input.0[at..at + len].to_vec(),
            input.1[at..at + len].to_vec(),
        );
        let silent = l.iter().chain(&r).all(|x| x.to_bits() == 0);
        if silent && side.unit.at_rest() {
            side.unit.skip(len);
        } else {
            side.unit.process(&mut l, &mut r, None);
        }
        seen.push(Seen {
            l: l.iter().map(|x| x.to_bits()).collect(),
            r: r.iter().map(|x| x.to_bits()).collect(),
            latency: side.unit.latency(),
            rest: side.unit.rest_blocker(),
            tel: side.unit.telemetry().iter().map(|x| x.to_bits()).collect(),
        });
        at += len;
    }
    seen
}

fn first_difference(a: &[Seen], b: &[Seen]) -> Option<String> {
    for (j, (x, y)) in a.iter().zip(b).enumerate() {
        if x != y {
            let what = if x.l != y.l || x.r != y.r {
                "samples"
            } else if x.latency != y.latency {
                "latency"
            } else if x.rest != y.rest {
                "rest"
            } else {
                "telemetry"
            };
            return Some(format!(
                "block {j}: the {what} differ ({:?} / {:?})",
                x.rest, y.rest
            ));
        }
    }
    (a.len() != b.len()).then(|| "a different number of blocks".to_string())
}

fn the_pedal_is_fragor_with_its_slot_alone(kind: EffectKind) {
    let input = phrase((2.9 * SR) as usize);
    let mut failures = Vec::new();
    for (name, evs) in scenarios(kind) {
        let mut reference: Option<Vec<u32>> = None;
        let mut splits = Rng::new(0x5_1171 + kind as u64);
        for block in [1usize, 64, 1024, 0] {
            let mut a = pedal(kind);
            let mut b = fragor();
            let mut ra = splits.clone();
            let mut rb = splits.clone();
            let cut = |r: &mut Rng| if block == 0 { r.block() } else { block };
            let x = run(&mut a, &input, &evs, || cut(&mut ra));
            let y = run(&mut b, &input, &evs, || cut(&mut rb));
            splits.next_u64();
            if let Some(d) = first_difference(&x, &y) {
                failures.push(format!("{kind:?} {name} at blocks of {block}: {d}"));
            }
            // The pedal's output does not depend on the block size either.
            let mut out: Vec<u32> = x.iter().flat_map(|s| s.l.iter().copied()).collect();
            out.extend(x.iter().flat_map(|s| s.r.iter().copied()));
            match &reference {
                None => reference = Some(out),
                Some(r) => {
                    if *r != out {
                        failures.push(format!("{kind:?} {name}: blocks of {block} differ from 1"));
                    }
                }
            }
        }
        // It sounds where it should: the circuit changes the phrase except at the defaults.
        let out = reference.unwrap();
        let same = out
            .iter()
            .zip(input.0.iter().chain(&input.1))
            .all(|(p, q)| *p == q.to_bits());
        assert_eq!(same, name == "defaults", "{kind:?} {name}: sounding or not");
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_green_overdrive_is_fragor_with_its_slot_alone() {
    the_pedal_is_fragor_with_its_slot_alone(KINDS[0]);
}

#[test]
fn the_op_amp_distortion_is_fragor_with_its_slot_alone() {
    the_pedal_is_fragor_with_its_slot_alone(KINDS[1]);
}

#[test]
fn the_silicon_distortion_is_fragor_with_its_slot_alone() {
    the_pedal_is_fragor_with_its_slot_alone(KINDS[2]);
}

#[test]
fn the_germanium_fuzz_is_fragor_with_its_slot_alone() {
    the_pedal_is_fragor_with_its_slot_alone(KINDS[3]);
}

#[test]
fn the_triangle_muff_is_fragor_with_its_slot_alone() {
    the_pedal_is_fragor_with_its_slot_alone(KINDS[4]);
}

/// What each pedal costs (ADR 0152's measurement), in a release build on a quiet machine:
/// `cargo test --release -p cantus-engine --test fx_fragor_pedal -- --ignored --nocapture`.
/// For each circuit, stereo at 4x on a guitar's phrase, best of five: the pedal, Fragor with
/// that circuit alone in a slot, and Fragor's two-pedal board of that circuit and the next
/// (the case its preset budget is set for); and the memory each holds for its circuits.
#[test]
#[ignore]
fn cpu_of_each_pedal() {
    let secs = 4.0;
    let n = (secs * SR) as usize;
    let input = {
        let (mut l, mut r) = phrase(n);
        // No silence: the circuits run the whole time.
        for i in 0..n {
            let t = i as f32 / SR;
            if (1.2..2.4).contains(&t) {
                let u = t - 1.2;
                let x = 0.25 * math::exp(-2.0 * u) * math::sin(std::f32::consts::TAU * 196.0 * u);
                l[i] = x;
                r[i] = x * 0.9;
            }
        }
        (l, r)
    };
    let cost = |unit: &mut Box<dyn FxUnit>| -> f64 {
        let mut best = f64::MAX;
        for _ in 0..5 {
            unit.reset();
            let (mut l, mut r) = input.clone();
            let t = std::time::Instant::now();
            for (a, b) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
                unit.process(a, b, None);
            }
            best = best.min(t.elapsed().as_secs_f64());
            std::hint::black_box((&l, &r));
        }
        best / f64::from(secs) * 100.0
    };
    println!("| Circuit | Pedal | Fragor, one slot | Fragor, two slots | Pedal / two slots |");
    println!("|---|---|---|---|---|");
    for (i, kind) in KINDS.iter().enumerate() {
        let mut p = make(*kind, SR).unwrap();
        let s = spec(*kind, &[(switch(i), 1.0)]);
        p.configure(&s);
        let mut one = make(EffectKind::Pedal, SR).unwrap();
        one.configure(&as_fragor(&s));
        let mut two = make(EffectKind::Pedal, SR).unwrap();
        let mut f = as_fragor(&s);
        f.set("s2", ((i + 1) % 5) as f32 + 1.0);
        two.configure(&f);
        let (a, b, c) = (cost(&mut p), cost(&mut one), cost(&mut two));
        println!(
            "| {kind:?} | {a:.2} % | {b:.2} % | {c:.2} % | {:.2} |",
            a / c
        );
    }
    println!(
        "memory: one circuit {} bytes; Fragor holds ten, a pedal two",
        std::mem::size_of::<cantus_dsp::fragor::Pedal>()
    );
}
