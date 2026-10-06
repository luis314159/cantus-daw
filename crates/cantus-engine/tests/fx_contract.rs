//! The contract every plug-in keeps, checked for every kind at once.
//!
//! - At its defaults it renders finite samples, the same at every block size, inside a
//!   project.
//! - **Split invariance** (ADR 0031 §1.5, risk 1): the unit alone renders the same bits at
//!   any split — blocks of 1, of 4096, and random splits between — at its defaults, at the
//!   bench's *heavy* settings (`support/heavy.rs`) and at settings drawn at random over every
//!   parameter's range, keyed and not. Per-node segmentation calls each insert with whole
//!   passes instead of the global segments, which this makes irrelevant.
//! - **At rest is exact** (ADR 0031 §5.1): the twin test. Feed signal, then silence until the
//!   unit says `at_rest()`; then a twin that keeps processing zeros (and must output exact
//!   zeros and stay at rest), a twin that is skipped (told so by `skip`), a twin that is
//!   reset and a brand-new unit (both skipped as far as the first has run, so counters
//!   agree) must all render the same bits when signal returns at a random sample, each at
//!   its own random splits. When the parameters also move at that moment, the skipped twin
//!   must still agree (suspension is exact); the reset and new ones are only reported.
//!
//! Each plug-in's own tests go further; this one catches the kind that breaks the family's
//! floor.

mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::{EffectUnit, FxNote};
use support::heavy::{self, Rng, SAMPLE_RATE};
use support::*;

#[test]
fn every_kind_is_finite_and_block_size_independent_at_its_defaults() {
    for kind in EffectKind::ALL {
        let mut p = demo_project();
        add_effect(&mut p, kind);
        let reference = render_with(&p, 512);
        assert!(
            reference.iter().all(|x| x.is_finite()),
            "{kind:?} produced a sample that is not a number"
        );
        for block in [1usize, 64, 1000] {
            assert_null(
                &reference,
                &render_with(&p, block),
                0.0,
                &format!("{kind:?} at block size {block}"),
            );
        }
    }
}

#[test]
fn every_kind_is_block_size_independent_at_heavy_settings_in_a_project() {
    for kind in EffectKind::ALL {
        let mut p = demo_project();
        let id = add_effect(&mut p, kind);
        strip_of(&mut p, 0)
            .effects
            .iter_mut()
            .find(|e| e.id == id)
            .unwrap()
            .spec = heavy::heavy(kind);
        let reference = render_with(&p, 512);
        assert!(
            reference.iter().all(|x| x.is_finite()),
            "{kind:?} (heavy) produced a sample that is not a number"
        );
        for block in [1usize, 64, 1000, 4096] {
            assert_null(
                &reference,
                &render_with(&p, block),
                0.0,
                &format!("{kind:?} (heavy) at block size {block}"),
            );
        }
    }
}

// ---- the unit alone ------------------------------------------------------------------------

/// What a unit is fed: audio, a key, and notes at absolute frames (for the kinds that hear
/// them).
#[derive(Clone, Debug, Default)]
struct Input {
    l: Vec<f32>,
    r: Vec<f32>,
    kl: Vec<f32>,
    kr: Vec<f32>,
    keyed: bool,
    notes: Vec<(usize, FxNote)>,
}

impl Input {
    fn silence(frames: usize, keyed: bool) -> Self {
        Input {
            l: vec![0.0; frames],
            r: vec![0.0; frames],
            kl: vec![0.0; frames],
            kr: vec![0.0; frames],
            keyed,
            notes: Vec::new(),
        }
    }

    /// The bench's signal in sections of random level — silence, quiet, nominal, and hot
    /// enough to drive every limiter and clipper — with a key of its own and, for the kinds
    /// that hear notes, notes on and off.
    fn music(frames: usize, keyed: bool, notes: bool, rng: &mut Rng) -> Self {
        let (sl, sr) = heavy::signal(SAMPLE_RATE as usize);
        let mut x = Input::silence(frames, keyed);
        let (mut at, mut gain, mut kgain) = (0usize, 1.0f32, 1.0f32);
        let start = rng.range(0, sl.len() - 1);
        let mut until = 0;
        for i in 0..frames {
            if i >= until {
                until = i + rng.range(64, 9_000);
                let levels = [0.0, 0.05, 0.5, 1.0, 2.0, 4.0];
                gain = levels[rng.range(0, levels.len() - 1)];
                kgain = levels[rng.range(0, levels.len() - 1)];
            }
            let j = (start + at) % sl.len();
            at += 1;
            x.l[i] = sl[j] * gain;
            x.r[i] = sr[j] * gain;
            x.kl[i] = sr[(j + 4_801) % sl.len()] * kgain;
            x.kr[i] = sl[(j + 9_601) % sl.len()] * kgain;
        }
        if notes {
            let mut f = 0;
            let mut held: Vec<u8> = Vec::new();
            while f < frames {
                if held.is_empty() || rng.chance(0.6) {
                    let pitch = rng.range(36, 84) as u8;
                    let velocity = rng.range(1, 127) as u8;
                    x.notes.push((f, FxNote::On { pitch, velocity }));
                    held.push(pitch);
                } else {
                    let pitch = held.remove(rng.range(0, held.len() - 1));
                    x.notes.push((f, FxNote::Off { pitch }));
                }
                if rng.chance(0.05) {
                    x.notes.push((f, FxNote::AllOff));
                    held.clear();
                }
                f += rng.range(1, 6_000);
            }
            // Every note ends with the music: what follows may come to rest.
            x.notes.push((frames - 1, FxNote::AllOff));
        }
        x
    }

    fn frames(&self) -> usize {
        self.l.len()
    }
}

/// Run `x` through `u` at the splits `block` chooses, handing each note to the call that
/// contains its frame. Calls `after` with the unit after every call.
fn drive(
    u: &mut EffectUnit,
    x: &Input,
    mut block: impl FnMut() -> usize,
    mut after: impl FnMut(&EffectUnit, &[f32], &[f32]),
) -> (Vec<f32>, Vec<f32>) {
    let (mut l, mut r) = (x.l.clone(), x.r.clone());
    let n = x.frames();
    let (mut at, mut ni) = (0, 0);
    while at < n {
        let b = block().clamp(1, n - at);
        while ni < x.notes.len() && x.notes[ni].0 < at + b {
            u.note(x.notes[ni].0.saturating_sub(at), x.notes[ni].1);
            ni += 1;
        }
        let key = x.keyed.then(|| (&x.kl[at..at + b], &x.kr[at..at + b]));
        u.process_keyed(&mut l[at..at + b], &mut r[at..at + b], key);
        after(u, &l[at..at + b], &r[at..at + b]);
        at += b;
    }
    (l, r)
}

fn unit(spec: &EffectSpec) -> EffectUnit {
    let slot = EffectSlot {
        id: EffectId(1),
        bypass: false,
        mix: 1.0,
        sidechain: None,
        midi_from: None,
        spec: spec.clone(),
    };
    EffectUnit::new(&slot, SAMPLE_RATE)
}

/// Where two renders first differ, bit for bit; `None` when they are the same bits.
fn first_difference(a: &(Vec<f32>, Vec<f32>), b: &(Vec<f32>, Vec<f32>)) -> Option<String> {
    for (ch, (x, y)) in [(&a.0, &b.0), (&a.1, &b.1)].into_iter().enumerate() {
        if x.len() != y.len() {
            return Some(format!("lengths {} and {}", x.len(), y.len()));
        }
        if let Some(i) = (0..x.len()).find(|&i| x[i].to_bits() != y[i].to_bits()) {
            return Some(format!(
                "channel {ch}, frame {i}: {:e} against {:e}",
                x[i], y[i]
            ));
        }
    }
    None
}

/// The settings a kind is checked at: its defaults, the bench's heavy ones, and `random`
/// drawn over the whole range (half the parameters moved, or all of them).
fn specs(kind: EffectKind, random: usize, rng: &mut Rng) -> Vec<(String, EffectSpec)> {
    let mut v = vec![
        ("default".to_string(), EffectSpec::new(kind)),
        ("heavy".to_string(), heavy::heavy(kind)),
    ];
    for i in 0..random {
        let p = if i % 3 == 2 { 1.0 } else { 0.5 };
        v.push((
            format!("random #{i}"),
            heavy::random_effect(kind, p, rng),
        ));
    }
    v
}

/// A block of `n` from `rng`, as a closure for [`drive`].
fn splits(mut rng: Rng) -> impl FnMut() -> usize {
    move || rng.block()
}

fn split_invariance(kind: EffectKind) {
    let mut rng = Rng::new(0x5b11_7000 + kind as u64);
    let mut failures = Vec::new();
    for (label, spec) in specs(kind, 6, &mut rng) {
        for keyed in [false, true] {
            if keyed && !kind.takes_sidechain() {
                continue;
            }
            let mut x = Input::music(48_000, keyed, kind.takes_notes(), &mut rng);
            let tail = Input::silence(24_000, keyed);
            for (v, t) in [
                (&mut x.l, &tail.l),
                (&mut x.r, &tail.r),
                (&mut x.kl, &tail.kl),
                (&mut x.kr, &tail.kr),
            ] {
                v.extend_from_slice(t);
            }
            let reference = drive(&mut unit(&spec), &x, || 512, |_, _, _| {});
            if !reference.0.iter().chain(&reference.1).all(|v| v.is_finite()) {
                failures.push(format!("{kind:?} {label} keyed {keyed}: not finite"));
                continue;
            }
            let runs: [(&str, Box<dyn FnMut() -> usize>); 5] = [
                ("blocks of 1", Box::new(|| 1)),
                ("blocks of 4096", Box::new(|| 4096)),
                ("blocks of 7", Box::new(|| 7)),
                ("random splits", Box::new(splits(rng.clone()))),
                ("random splits", Box::new(splits(Rng::new(rng.next_u64())))),
            ];
            for (name, block) in runs {
                let out = drive(&mut unit(&spec), &x, block, |_, _, _| {});
                if let Some(d) = first_difference(&reference, &out) {
                    failures.push(format!("{kind:?} {label} keyed {keyed}, {name}: {d}"));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "split invariance broken:\n{}\n(specs: see `specs` with the same seed)",
        failures.join("\n")
    );
}

/// Feed, silence, compare four twins. `Ok(Some(frames))`: it came to rest after that much
/// silence and every twin agreed; `Ok(None)`: no rest within `cap` frames of silence.
/// With `change`, the parameters move at the moment the signal returns.
fn twins(
    kind: EffectKind,
    spec: &EffectSpec,
    seed: u64,
    cap: usize,
    change: Option<&EffectSpec>,
) -> Result<Option<usize>, String> {
    let mut rng = Rng::new(seed);
    let keyed = kind.takes_sidechain() && rng.chance(0.5);
    let notes = kind.takes_notes();
    // A unit that rests is as good as a new one skipped as far, so where a new one never
    // comes to rest a long wait is wasted: wait a little, enough to catch a unit that
    // claims rest where a new one cannot.
    let mut probe = unit(spec);
    probe.skip(cap);
    let cap = if probe.at_rest() { cap } else { cap.min(48_000) };
    let history = Input::music(rng.range(2_400, 30_000), keyed, notes, &mut rng);
    let (mut a, mut b, mut d) = (unit(spec), unit(spec), unit(spec));
    let hist_seed = rng.next_u64();
    for u in [&mut a, &mut b, &mut d] {
        drive(u, &history, splits(Rng::new(hist_seed)), |_, _, _| {});
    }
    // Silence until the unit says it is at rest. The three twins are the same unit so far.
    let mut silent = 0;
    loop {
        if a.at_rest() {
            break;
        }
        if silent > cap {
            if let Some(why) = a.rest_blocker() {
                eprintln!("  ({kind:?}, seed {seed}: not at rest — {why})");
            }
            return Ok(None);
        }
        let n = rng.block();
        let z = Input::silence(n, keyed);
        for u in [&mut a, &mut b, &mut d] {
            drive(u, &z, || n, |_, _, _| {});
        }
        silent += n;
        if a.at_rest() != b.at_rest() || a.at_rest() != d.at_rest() {
            return Err("at_rest differs between identical twins".into());
        }
    }
    // a: keeps processing zeros, which must come out as zeros, at rest throughout.
    let k = rng.range(0, 30_000);
    let mut broke = None;
    drive(&mut a, &Input::silence(k, keyed), splits(Rng::new(rng.next_u64())), |u, l, r| {
        if broke.is_none() && !(l.iter().chain(r).all(|v| *v == 0.0) && u.at_rest()) {
            broke = Some(if u.at_rest() {
                "at rest, silence in gave sound out"
            } else {
                "at rest, silence in left it not at rest"
            });
        }
    });
    if let Some(e) = broke {
        return Err(e.into());
    }
    // d: skipped, told so. b: reset, as a wake may do. c: brand new. The last two are
    // skipped as far as a has run, so whatever counts samples agrees.
    let total = history.frames() + silent + k;
    d.skip(k);
    if !d.at_rest() {
        return Err("skip left the unit not at rest".into());
    }
    b.reset();
    b.skip(total);
    let mut c = unit(spec);
    c.skip(total);
    if !b.at_rest() || !c.at_rest() {
        return Err("a reset or a new unit, skipped, is not at rest".into());
    }
    if let Some(to) = change {
        for u in [&mut a, &mut b, &mut c, &mut d] {
            u.configure(to);
            if let Some(need) = u.take_request() {
                drop(u.supply(cantus_engine::EffectMemory::build(need, SAMPLE_RATE)));
            }
        }
    }
    let back = Input::music(rng.range(4_800, 40_000), keyed, notes, &mut rng);
    let out_a = drive(&mut a, &back, splits(Rng::new(rng.next_u64())), |_, _, _| {});
    let out_d = drive(&mut d, &back, splits(Rng::new(rng.next_u64())), |_, _, _| {});
    if let Some(e) = first_difference(&out_a, &out_d) {
        return Err(format!("the skipped twin differs after {k} frames of rest: {e}"));
    }
    for (name, u) in [("reset", &mut b), ("new", &mut c)] {
        let out = drive(u, &back, splits(Rng::new(rng.next_u64())), |_, _, _| {});
        if let Some(e) = first_difference(&out_a, &out) {
            if change.is_none() {
                return Err(format!("the {name} twin differs: {e}"));
            }
            // As good as new only while the parameters stay (a memory that outlives
            // silence may show once they move): reported, not required.
            eprintln!("  ({kind:?}: parameters moved at the wake: the {name} twin differs: {e})");
        }
    }
    Ok(Some(silent))
}

/// Whether a kind must come to rest, at its defaults and at the heavy settings, within
/// the test's silence (30 s): the kinds that implement `at_rest`, where it is reachable.
/// A kind that says `false` forever is allowed (the default), but then this table says
/// why in `docs/round21/at-rest.md`.
fn must_rest(kind: EffectKind) -> (bool, bool) {
    match kind {
        EffectKind::Delay => (true, false), // heavy: 0.9 feedback rings for ~11 minutes
        EffectKind::Reverb => (true, false), // heavy: size 1 rings for over a minute
        EffectKind::Stereo | EffectKind::Saturator => (true, true),
        EffectKind::Multiband | EffectKind::DeEsser | EffectKind::Maximizer => (true, true),
        EffectKind::Limiter => (true, false), // heavy: dithers (R10)
        EffectKind::Gate => (false, false), // keyed, a reset gate starts open
        _ => (false, false),
    }
}

fn at_rest_is_exact(kind: EffectKind) {
    let mut rng = Rng::new(0xa7_4e57 + kind as u64);
    let mut failures = Vec::new();
    let mut report = Vec::new();
    let (rest_default, rest_heavy) = must_rest(kind);
    for (label, spec) in specs(kind, 4, &mut rng) {
        let fixed = label == "default" || label == "heavy";
        let cap = if fixed { 30 * 48_000 } else { 4 * 48_000 };
        let must = (label == "default" && rest_default) || (label == "heavy" && rest_heavy);
        for trial in 0..3 {
            // The first trial as the tests run, the others as the engine's `process` will
            // (denormals flushed): exact in both, but some state only reaches zero in the
            // second, so rest is required only there.
            let flushed = trial != 0;
            let _flush = flushed.then(heavy::Flush::new);
            let change = (trial == 2).then(|| heavy::random_effect(kind, 0.5, &mut rng));
            let seed = rng.next_u64();
            match twins(kind, &spec, seed, cap, change.as_ref()) {
                Ok(Some(after)) => report.push(format!(
                    "{label} #{trial}: at rest after {:.2} s{}",
                    after as f32 / SAMPLE_RATE,
                    if flushed { " (FTZ)" } else { "" }
                )),
                Ok(None) => {
                    report.push(format!("{label} #{trial}: no rest (seed {seed})"));
                    if must && flushed {
                        failures.push(format!(
                            "{kind:?} {label} #{trial}: no rest within {} s of silence",
                            cap / 48_000
                        ));
                    }
                }
                Err(e) => failures.push(format!("{kind:?} {label} #{trial} (seed {seed}): {e}")),
            }
        }
    }
    eprintln!("{kind:?}:\n  {}", report.join("\n  "));
    if rest_default {
        assert!(
            unit(&EffectSpec::new(kind)).at_rest(),
            "{kind:?}: a new unit is at rest by definition"
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

macro_rules! per_kind {
    ($($name:ident => $kind:expr),* $(,)?) => {
        mod split {
            use super::*;
            $( #[test] fn $name() { split_invariance($kind) } )*
        }
        mod at_rest {
            use super::*;
            $( #[test] fn $name() { at_rest_is_exact($kind) } )*
        }
    };
}

per_kind! {
    eq => EffectKind::Eq,
    compressor => EffectKind::Compressor,
    limiter => EffectKind::Limiter,
    space => EffectKind::Space,
    multiband => EffectKind::Multiband,
    deesser => EffectKind::DeEsser,
    gate => EffectKind::Gate,
    saturator => EffectKind::Saturator,
    stereo => EffectKind::Stereo,
    maximizer => EffectKind::Maximizer,
    delay => EffectKind::Delay,
    reverb => EffectKind::Reverb,
}

#[test]
fn every_native_kind_is_checked() {
    // `per_kind!` lists twelve: a new kind must be added there.
    assert_eq!(EffectKind::ALL.len(), 12);
}
