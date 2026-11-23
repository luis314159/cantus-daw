//! Round 42 (ADR 0155): the project's tuning in every pitched instrument, through the real
//! engine (a snapshot compiled from a project, rendered offline and live).
//!
//! - **Pitch**: a tuned key's measured pitch is its table's within 0.1 cent — the same key
//!   rendered at the default and in Werckmeister III in E flat at A415, its strongest partial
//!   measured in both (a phase-slope estimate), the ratio against the table's offset. The
//!   ratio cancels whatever each instrument adds of its own (drift, inharmonicity, trims).
//! - **The default**: `tuning_null.rs` dumps every instrument's render at the default for
//!   `tools/test/ab-compare.sh` against the contract's code; here, a track that plays `Equal`
//!   in a tuned project is the untuned render to the bit, and unpitched instruments ignore a
//!   tuning (its keyboard map's unmapped keys too).
//! - **Block sizes** at tolerance 0 and **offline = real time**, with a tuning set.

use cantus_core::tuning::{KeyboardMap, Temperament, TrackTuning, Tuning, TuningSystem};
use cantus_core::*;
use cantus_dsp::math;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::*;
use std::sync::Arc;

const SR: u32 = 48_000;

/// The round's tuning: Werckmeister III in E flat at A415 (fixtures/v80's).
pub fn werck3_e_flat_415() -> Tuning {
    Tuning {
        a4_hz: 415.0,
        system: TuningSystem::Temperament {
            id: Temperament::WerckmeisterIii,
            key: 3,
            follow_song_key: false,
        },
        map: None,
    }
}

/// What a key is set to play: a native kind (with settings) or Firmus (`None`).
#[derive(Clone)]
struct Inst {
    name: &'static str,
    kind: Option<InstrumentKind>,
    set: &'static [(&'static str, f32)],
}

fn pitched() -> Vec<Inst> {
    let k = |name, kind, set| Inst {
        name,
        kind: Some(kind),
        set,
    };
    vec![
        Inst {
            name: "firmus",
            kind: None,
            set: &[],
        },
        k("calor", InstrumentKind::Analog, &[]),
        k("unda", InstrumentKind::Wavetable, &[]),
        k("modulus", InstrumentKind::Fm, &[]),
        k("gravis", InstrumentKind::Bass, &[]),
        k("gravis-string", InstrumentKind::Bass, &[("engine", 2.0)]),
        k(
            "gravis-strings",
            InstrumentKind::Bass,
            &[("engine", 2.0), ("mode", 2.0)],
        ),
        k("lamina", InstrumentKind::ElectricPiano, &[]),
        k("organum", InstrumentKind::Organ, &[]),
        k("exemplar", InstrumentKind::Sampler, &[]),
        k("chorda", InstrumentKind::Chorda, &[]),
        k("fides", InstrumentKind::Resonator, &[]),
    ]
}

/// One track playing `keys` one after another at 60 BPM — each held 1.5 s, one every 2 s —
/// in `tuning`.
fn project(inst: &Inst, keys: &[u8], tuning: Tuning) -> Project {
    let mut p = Project::new("tuning");
    p.tempo_bpm = 60.0;
    p.tuning = tuning;
    let mut t = p.make_track(inst.name);
    t.strip = None;
    if let Some(kind) = inst.kind {
        let mut spec = NativeSpec::new(kind);
        for (k, v) in inst.set {
            spec.set(k, *v);
            assert_eq!(spec.get(k), *v, "{}: {k}", inst.name);
        }
        t.instrument = InstrumentSpec::Native(spec);
    }
    let len = Ticks::beats(2 * keys.len() as i64);
    let mut clip = p.make_clip("notes", Ticks::ZERO, len);
    for (i, &key) in keys.iter().enumerate() {
        let n = p.make_note(
            Ticks::beats(2 * i as i64),
            Ticks::beats(1) + Ticks::EIGHTH,
            key,
            100,
        );
        clip.notes.push(n);
    }
    t.clips.push(clip);
    p.tracks.push(t);
    p
}

fn offline(p: &Project, block: usize) -> Vec<f32> {
    let opts = RenderOptions {
        sample_rate: SR,
        tail_seconds: 0.25,
        block_frames: block,
        ..Default::default()
    };
    render(Arc::new(Snapshot::compile(p, 1)), &opts)
        .expect("render")
        .samples
}

/// The left channel of an interleaved stereo render.
fn left(x: &[f32]) -> Vec<f64> {
    x.chunks_exact(2).map(|f| f64::from(f[0])).collect()
}

/// The Hann-windowed spectrum of `x[start..start + n]` at `hz` (one bin, any frequency).
fn bin(x: &[f64], start: usize, n: usize, hz: f64) -> (f64, f64) {
    let w = std::f64::consts::TAU * hz / f64::from(SR);
    let (mut re, mut im) = (0.0, 0.0);
    for i in 0..n {
        let win = 0.5 - 0.5 * math::cos(std::f64::consts::TAU * i as f64 / n as f64);
        let v = x[start + i] * win;
        let (s, c) = math::sin_cos(w * i as f64);
        re += v * c;
        im -= v * s;
    }
    (re, im)
}

const N: usize = 32_768;

/// r42 fix-engine (review reach 10): [`bin`]'s magnitude for the coarse search, faster — the
/// window computed once and the oscillator turned by one complex multiplication a sample
/// (its drift over 32 768 samples is ~1e-11 rad, nothing to an arg-max). The fine measure
/// keeps `bin`'s exact sines.
fn coarse_power(x: &[f64], start: usize, hz: f64) -> f64 {
    static WINDOW: std::sync::OnceLock<Vec<f64>> = std::sync::OnceLock::new();
    let win = WINDOW.get_or_init(|| {
        (0..N)
            .map(|i| 0.5 - 0.5 * math::cos(std::f64::consts::TAU * i as f64 / N as f64))
            .collect()
    });
    let (s, c) = math::sin_cos(std::f64::consts::TAU * hz / f64::from(SR));
    let (mut or, mut oi) = (1.0f64, 0.0f64);
    let (mut re, mut im) = (0.0, 0.0);
    for (v, w) in x[start..start + N].iter().zip(win) {
        let v = v * w;
        re += v * or;
        im -= v * oi;
        let next = or * c - oi * s;
        oi = or * s + oi * c;
        or = next;
    }
    re * re + im * im
}

/// The strongest frequency within ±2.5 % of `near` (a coarse search), then its phase slope
/// over two hops (256 and 4 096 samples): a steady partial's frequency to a millionth.
fn partial(x: &[f64], start: usize, near: f64) -> (f64, f64) {
    let mut best = (0.0, near);
    for i in 0..=500 {
        let f = near * (0.975 + 0.05 * i as f64 / 500.0);
        let m = coarse_power(x, start, f);
        if m > best.0 {
            best = (m, f);
        }
    }
    let mut f = best.1;
    for hop in [256usize, 4096, 8192] {
        let (a_re, a_im) = bin(x, start, N, f);
        let (b_re, b_im) = bin(x, start + hop, N, f);
        let d = math::atan2(b_im, b_re) - math::atan2(a_im, a_re);
        let want = std::f64::consts::TAU * f * hop as f64 / f64::from(SR);
        let mut r = (d - want) % std::f64::consts::TAU;
        if r > std::f64::consts::PI {
            r -= std::f64::consts::TAU;
        }
        if r < -std::f64::consts::PI {
            r += std::f64::consts::TAU;
        }
        f += r / (std::f64::consts::TAU * hop as f64 / f64::from(SR));
    }
    (best.0, f)
}

/// The note at `index`'s strongest harmonic (1 … 6) near its 12-TET fundamental, and its
/// frequency.
fn strongest(x: &[f64], index: usize, key: u8) -> (usize, f64) {
    let start = (index * 2 * SR as usize) + (SR as usize) * 3 / 10;
    let f0 = 440.0 * math::exp2((f64::from(key) - 69.0) / 12.0);
    (1..=6usize)
        .filter(|h| f0 * *h as f64 * 1.03 < f64::from(SR) * 0.45)
        .map(|h| {
            let (m, f) = partial(x, start, f0 * h as f64);
            (m, h, f)
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, h, f)| (h, f))
        .unwrap()
}

/// Harmonic `h` of the note at `index`, near `hz`.
fn harmonic(x: &[f64], index: usize, hz: f64) -> f64 {
    let start = (index * 2 * SR as usize) + (SR as usize) * 3 / 10;
    partial(x, start, hz).1
}

const KEYS: [u8; 4] = [45, 57, 63, 70];

/// What makes a partial beat — a second oscillator detuned, drift, a chorus, a piano's
/// unison, a tonewheel's leakage — switched off for the measure: a beating partial has no
/// single frequency to read to a tenth of a cent. The tuning's path is the same.
fn steady(name: &str) -> &'static [(&'static str, f32)] {
    match name {
        "calor" => &[
            ("chorus.mix", 0.0),
            ("analog.drift", 0.0),
            ("analog.detune", 0.0),
            ("mix.osc2", 0.0),
            ("mix.osc3", 0.0),
        ],
        // One 8' drawbar: the 16' wheel's octave sits on the 8' (a gear-tuned octave is not
        // quite 2:1, so the two beat).
        "organum" => &[
            ("u1", 0.0),
            ("u2", 0.0),
            ("vib.upper", 0.0),
            ("vib.lower", 0.0),
            ("rot.speed", 0.0),
            ("leak", -80.0),
            ("leak.bar", -80.0),
        ],
        "chorda" => &[
            ("unison", 0.0),
            ("coupling", 0.0),
            ("sympathy", 0.0),
            ("phantom", 0.0),
        ],
        _ => &[],
    }
}

fn steady_project(inst: &Inst, keys: &[u8], tuning: Tuning) -> Project {
    let mut p = project(inst, keys, tuning);
    match &mut p.tracks[0].instrument {
        InstrumentSpec::Native(spec) => {
            for (k, v) in steady(inst.name) {
                assert!(spec.set(k, *v).is_some(), "{}: {k}", inst.name);
            }
        }
        i => {
            if let Some(f) = i.firmus_mut() {
                f.osc2_detune_cents = 0.0;
            }
        }
    }
    p
}

#[test]
fn every_pitched_instrument_plays_each_key_at_its_table_s_pitch() {
    let tuning = werck3_e_flat_415();
    let table = tuning.offsets();
    let mut worst = 0.0f64;
    let mut failed = Vec::new();
    for inst in pitched() {
        let plain = left(&offline(
            &steady_project(&inst, &KEYS, Tuning::default()),
            256,
        ));
        let tuned = left(&offline(&steady_project(&inst, &KEYS, tuning.clone()), 256));
        for (i, &key) in KEYS.iter().enumerate() {
            // The piano's first partial: its higher ones meet the body's modes, which do
            // not move with the tuning (as a real soundboard's do not).
            let (h, f_plain) = if inst.name == "chorda" {
                let f0 = 440.0 * math::exp2((f64::from(key) - 69.0) / 12.0);
                (1, harmonic(&plain, i, f0))
            } else {
                strongest(&plain, i, key)
            };
            let want = f64::from(table.at(key));
            let f_tuned = harmonic(&tuned, i, f_plain * math::exp2(want / 12.0));
            let got = 12.0 * math::log2(f_tuned / f_plain);
            let err = (got - want) * 100.0;
            worst = worst.max(err.abs());
            println!(
                "{:15} key {key:3}: harmonic {h}, {f_plain:9.3} → {f_tuned:9.3} Hz, \
                 {:+8.3} c for {:+8.3} c ({err:+.4} c)",
                inst.name,
                got * 100.0,
                want * 100.0
            );
            if err.abs() >= 0.1 {
                failed.push(format!("{} key {key}: {err:+.4} c", inst.name));
            }
        }
    }
    println!("worst: {worst:.4} c");
    assert!(
        failed.is_empty(),
        "off their table by 0.1 c or more: {failed:?}"
    );
}

#[test]
fn a_track_that_plays_equal_in_a_tuned_project_is_the_untuned_render_to_the_bit() {
    for inst in pitched().into_iter().filter(|i| i.name != "chorda") {
        let plain = offline(&project(&inst, &KEYS[..2], Tuning::default()), 256);
        let mut p = project(&inst, &KEYS[..2], werck3_e_flat_415());
        p.tracks[0].tuning = TrackTuning::Equal;
        assert_null(&plain, &offline(&p, 256), 0.0, inst.name);
    }
}

#[test]
fn unpitched_instruments_ignore_the_tuning_even_its_unmapped_keys() {
    // A map that leaves every key but 60 unmapped: a drum kit still plays every pad.
    let mut t = werck3_e_flat_415();
    t.map = Some(KeyboardMap {
        size: 12,
        first_key: 0,
        last_key: 127,
        middle_key: 60,
        reference_key: 60,
        reference_hz: 261.625_565_3,
        octave_degree: 12,
        mapping: vec![Some(0)],
    });
    let drums = [36u8, 38, 42];
    for kind in [None, Some(InstrumentKind::AcousticDrums)] {
        let make = |tuning: Tuning| {
            let mut p = Project::new("drums");
            p.tuning = tuning;
            let mut tr = p.make_track("kit");
            tr.strip = None;
            tr.instrument = match kind {
                Some(k) => InstrumentSpec::Native(NativeSpec::new(k)),
                None => InstrumentSpec::Pulsus(Default::default()),
            };
            let mut clip = p.make_clip("hits", Ticks::ZERO, Ticks::beats(4));
            for (i, &k) in drums.iter().enumerate() {
                let n = p.make_note(Ticks::beats(i as i64), Ticks::QUARTER, k, 110);
                clip.notes.push(n);
            }
            tr.clips.push(clip);
            p.tracks.push(tr);
            p
        };
        let a = offline(&make(Tuning::default()), 256);
        let b = offline(&make(t.clone()), 256);
        assert_null(&a, &b, 0.0, &format!("{kind:?}"));
    }
}

#[test]
fn an_unmapped_key_sounds_nothing_and_the_mapped_ones_play() {
    let t = Tuning {
        map: Some(KeyboardMap {
            size: 12,
            first_key: 0,
            last_key: 127,
            middle_key: 60,
            reference_key: 69,
            reference_hz: 440.0,
            octave_degree: 12,
            // Only the white keys: C D E F G A B.
            mapping: vec![
                Some(0),
                None,
                Some(2),
                None,
                Some(4),
                Some(5),
                None,
                Some(7),
                None,
                Some(9),
                None,
                Some(11),
            ],
        }),
        ..Tuning::default()
    };
    let firmus = &pitched()[0];
    let black = offline(&project(firmus, &[61, 63], t.clone()), 256);
    assert!(black.iter().all(|&s| s == 0.0), "unmapped keys are silent");
    let white = offline(&project(firmus, &[60, 62], t), 256);
    let plain = offline(&project(firmus, &[60, 62], Tuning::default()), 256);
    assert_null(&plain, &white, 0.0, "a map of 12-TET's own keys");
}

#[test]
fn a_tuned_render_is_the_same_at_every_block_size() {
    let t = werck3_e_flat_415();
    for inst in pitched() {
        let p = project(&inst, &KEYS[1..3], t.clone());
        let reference = offline(&p, 512);
        assert!(reference.iter().any(|&s| s != 0.0), "{} sounds", inst.name);
        for block in [1usize, 64, 1000] {
            assert_null(
                &reference,
                &offline(&p, block),
                0.0,
                &format!("{} at block {block}", inst.name),
            );
        }
    }
}

/// The live engine over `p` as the application drives it (`fides_input.rs`'s rig): units,
/// automation and the plan before the snapshot, compensation answered; `external` the
/// MTS-ESP link it is handed first.
fn live(
    p: &Project,
    frames: usize,
    external: Option<Arc<dyn cantus_engine::tuning::ExternalTuning>>,
) -> Vec<f32> {
    live_with(p, frames, external, false, |_| {})
}

/// [`live`], r42 fix-engine: `replace` swaps every instrument for a fresh one of its kind
/// once the snapshot is installed (a plug-in's restart, an isolation move); `each` is told
/// the frames played before every block.
fn live_with(
    p: &Project,
    frames: usize,
    external: Option<Arc<dyn cantus_engine::tuning::ExternalTuning>>,
    replace: bool,
    mut each: impl FnMut(usize),
) -> Vec<f32> {
    let snap = Arc::new(Snapshot::compile(p, 1));
    let instruments = cantus_engine::inst::units_for(&snap, SR as f32, |_, _| false);
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64 + instruments.len());
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(8192);
    let mut proc = Processor::without_midi(f64::from(SR), rx_engine, tx_engine);
    if let Some(e) = external {
        tx.push(EngineMessage::SetExternalTuning(e)).ok();
    }
    for unit in instruments {
        tx.push(EngineMessage::AddInstrument(unit)).ok();
    }
    tx.push(EngineMessage::Automation(AutomationRuntime::build(&snap)))
        .ok();
    let plan = cantus_engine::graph::compile(&snap);
    tx.push(EngineMessage::SetSnapshotPlanned(snap.clone(), plan)).ok();
    proc.settle();
    while let Ok(ev) = rx.pop() {
        if let EngineEvent::PdcNeed { count, delay } = ev {
            tx.push(EngineMessage::PdcLines(cantus_engine::pdc::lines_for(
                count, delay,
            )))
            .ok();
            proc.settle();
        }
    }
    if replace {
        for unit in cantus_engine::inst::units_for(&snap, SR as f32, |_, _| false) {
            tx.push(EngineMessage::ReplaceInstrument(unit)).ok();
        }
        proc.settle();
        while rx.pop().is_ok() {}
    }
    tx.push(EngineMessage::Seek(Ticks::ZERO)).ok();
    tx.push(EngineMessage::Play).ok();
    let sizes = [128usize, 333, 512, 96, 1024, 17, 256, 1];
    let mut out = Vec::with_capacity(frames * 2);
    let mut i = 0;
    while out.len() / 2 < frames {
        each(out.len() / 2);
        let n = sizes[i % sizes.len()].min(frames - out.len() / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        proc.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while rx.pop().is_ok() {}
    }
    out
}

#[test]
fn a_tuned_offline_render_equals_real_time() {
    let t = werck3_e_flat_415();
    // Chorda's keyboard is designed offline on the spot; live it arrives from the designer
    // (its own tests hold that path) — here the instruments whose live start is immediate.
    for inst in pitched().into_iter().filter(|i| i.name != "chorda") {
        let p = project(&inst, &KEYS[1..3], t.clone());
        let off = offline(&p, 256);
        let frames = off.len() / 2 - SR as usize / 8;
        let on = live(&p, frames, None);
        assert_null(&off[..frames * 2], &on, 0.0, inst.name);
    }
}

/// A master that publishes a fixed table: the MTS-ESP link's stand-in.
#[derive(Debug)]
struct Master(cantus_core::tuning::TuningTable);

impl cantus_engine::tuning::ExternalTuning for Master {
    fn read(&self, out: &mut cantus_core::tuning::TuningTable) -> bool {
        *out = self.0;
        true
    }
}

#[test]
fn a_track_that_follows_the_mts_esp_master_plays_its_table_live_and_offline() {
    let table = werck3_e_flat_415().offsets();
    let external = Tuning {
        system: TuningSystem::External,
        ..Tuning::default()
    };
    let firmus = &pitched()[0];
    let p = project(firmus, &KEYS, external);
    // The same notes in the table's own tuning: what the master's table must sound as.
    let want = offline(&project(firmus, &KEYS, werck3_e_flat_415()), 256);
    let frames = want.len() / 2 - SR as usize / 8;
    let heard = live(&p, frames, Some(Arc::new(Master(table))));
    assert_null(&want[..frames * 2], &heard, 0.0, "live, read each block");
    // Offline: the table its snapshot was compiled with (the handle reads the master then).
    let mut snap = Snapshot::compile(&p, 1);
    cantus_engine::tuning::fill_external(&mut snap, &Master(table));
    let opts = RenderOptions {
        sample_rate: SR,
        tail_seconds: 0.25,
        block_frames: 256,
        ..Default::default()
    };
    let off = render(Arc::new(snap), &opts).unwrap().samples;
    assert_null(&want, &off, 0.0, "offline, the compile's table");
    // No master: equal temperament.
    let none = live(&p, frames, None);
    let plain = offline(&project(firmus, &KEYS, Tuning::default()), 256);
    assert_null(&plain[..frames * 2], &none, 0.0, "no master");
}

/// r42 fix-engine (review correctness 5): a unit swapped in for its track's (a plug-in's
/// restart, an isolation move) plays the track's tuning at once. The control: untuned, the
/// swap changes nothing; tuned, the swapped unit sounds as the one it replaced.
#[test]
fn a_unit_swapped_in_plays_its_tracks_tuning() {
    let calor = pitched()
        .into_iter()
        .find(|i| i.kind == Some(InstrumentKind::Analog))
        .expect("Calor");
    for t in [Tuning::default(), werck3_e_flat_415()] {
        let p = project(&calor, &KEYS[1..3], t.clone());
        let frames = 4 * SR as usize;
        let kept = live(&p, frames, None);
        let swapped = live_with(&p, frames, None, true, |_| {});
        assert_null(&kept, &swapped, 0.0, &format!("swapped, {:?}", t.system));
    }
}

/// A master that leaves when told (an `AtomicBool`).
#[derive(Debug)]
struct Leaving(cantus_core::tuning::TuningTable, Arc<std::sync::atomic::AtomicBool>);

impl cantus_engine::tuning::ExternalTuning for Leaving {
    fn read(&self, out: &mut cantus_core::tuning::TuningTable) -> bool {
        if self.1.load(std::sync::atomic::Ordering::Relaxed) {
            return false;
        }
        *out = self.0;
        true
    }
}

/// r42 fix-engine (review correctness 8): when the master goes, a live track that followed
/// it plays the table it was compiled with again (equal temperament at its diapason) — the
/// notes after it went sound as the untuned song's, to the bit.
#[test]
fn a_track_whose_master_goes_plays_its_compiled_table_again() {
    let firmus = &pitched()[0];
    let external = Tuning {
        system: TuningSystem::External,
        ..Tuning::default()
    };
    let p = project(firmus, &KEYS, external);
    let plain = offline(&project(firmus, &KEYS, Tuning::default()), 256);
    let frames = plain.len() / 2 - SR as usize / 8;
    let gone = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let master = Leaving(werck3_e_flat_415().offsets(), gone.clone());
    let heard = live_with(&p, frames, Some(Arc::new(master)), false, |at| {
        if at >= SR as usize {
            gone.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    });
    // The first key, under the master: tuned (it is not the plain song's).
    let first = ..SR as usize;
    assert!(heard[first.clone()] != plain[first], "played the master's table first");
    // From 4 s (the third key, long after the master left): the untuned song's.
    let from = 4 * SR as usize * 2;
    assert_null(&plain[from..frames * 2], &heard[from..], 0.0, "the master gone");
}

#[test]
fn chorda_designs_each_key_at_its_table_s_offset_and_reuses_the_design() {
    use cantus_engine::inst::chorda;
    let spec = NativeSpec::new(InstrumentKind::Chorda);
    let knobs = chorda::tuning_of(&chorda::values_of(&spec));
    let table = werck3_e_flat_415().offsets();
    // The default: the same tuning, so the same design (its key, to the bit).
    let same = chorda::with_offsets(knobs, &cantus_core::tuning::TuningTable::EQUAL.0);
    assert_eq!(
        chorda::key_of(same, SR as f32),
        chorda::key_of(knobs, SR as f32)
    );
    let tuned = chorda::with_offsets(knobs, &table.0);
    assert_ne!(
        chorda::key_of(tuned, SR as f32),
        chorda::key_of(knobs, SR as f32)
    );
    let a = chorda::ensure(knobs, SR as f32);
    let b = chorda::ensure(tuned, SR as f32);
    let mut worst = 0.0f64;
    for (ka, kb) in a.keys.iter().zip(b.keys.iter()) {
        let want = f64::from(table.at(ka.key)) * 100.0;
        let got = 1200.0 * math::log2(kb.f1 / ka.f1);
        worst = worst.max((got - want).abs());
        assert!(
            (got - want).abs() < 0.002,
            "key {}: {got:.4} c for {want:.4}",
            ka.key
        );
    }
    println!("Chorda's designed keys: worst {worst:.5} c from the table");
}

/// The owner's rule for Chorda's diapason (round 42): in a song with no tuning its own `a4`
/// is the diapason, as in every older song; in a tuned one the tuning's is, and its `a4` is
/// not used (`cantus_core::inst::chorda::effective_a4`, which the editor reads too).
#[test]
fn chorda_s_own_a4_is_its_diapason_only_where_the_track_plays_no_tuning() {
    use cantus_core::inst::chorda::effective_a4;
    let at = |a4: &'static [(&'static str, f32)]| Inst {
        name: "chorda",
        kind: Some(InstrumentKind::Chorda),
        set: a4,
    };
    let (c432, c440) = (at(&[("a4", 432.0)]), at(&[]));
    // A song with no tuning: 432, measured (its first partial against Chorda at 440).
    let f432 = left(&offline(
        &steady_project(&c432, &[69], Tuning::default()),
        256,
    ));
    let f440 = left(&offline(
        &steady_project(&c440, &[69], Tuning::default()),
        256,
    ));
    let a = harmonic(&f432, 0, 432.0);
    let b = harmonic(&f440, 0, 440.0);
    let got = 1200.0 * math::log2(a / b);
    let want = 1200.0 * math::log2(432.0 / 440.0);
    println!("Chorda at a4 = 432: {a:.4} Hz against {b:.4} Hz ({got:+.4} c for {want:+.4} c)");
    assert!((got - want).abs() < 0.1, "{got:+.4} c for {want:+.4} c");
    let mut spec = NativeSpec::new(InstrumentKind::Chorda);
    spec.set("a4", 432.0);
    assert_eq!(effective_a4(&spec, &Tuning::default()), 432.0);
    // A tuned song: the tuning's diapason; Chorda's own a4 changes nothing, to the bit.
    let t = werck3_e_flat_415();
    assert_eq!(effective_a4(&spec, &t), 415.0);
    let a = offline(&project(&c432, &KEYS[1..3], t.clone()), 256);
    let b = offline(&project(&c440, &KEYS[1..3], t), 256);
    assert_null(&b, &a, 0.0, "Chorda's a4 under a tuning");
}
