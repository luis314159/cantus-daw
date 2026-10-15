//! Chorda through the engine: it plays, every parameter at its default changes no sample,
//! an offline render waits for the tuning it is asked for (and plays it), the soft pedal
//! and the sustain pedal arrive as controllers, and the unit at rest is as good as new.

use cantus_dsp::math;

mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::inst::chorda::{tel, Unit, SOFT_CC};
use cantus_engine::inst::{Controller, InstrumentUnit, NoteOn};
use cantus_engine::*;
use std::sync::Arc;

const SR: f32 = 48_000.0;

fn piano_project(values: &[(&str, f32)], notes: &[(f64, f64, u8, u8)]) -> Project {
    let mut p = Project::new("chorda");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Piano");
    let mut spec = NativeSpec::new(InstrumentKind::Chorda);
    for (k, v) in values {
        assert!(spec.set(k, *v).is_some(), "{k}");
    }
    t.instrument = InstrumentSpec::Native(spec);
    let mut clip = p.make_clip("Play", Ticks::ZERO, Ticks::beats(8));
    for (start, len, pitch, vel) in notes {
        let n = p.make_note(
            Ticks((start * Ticks::QUARTER.0 as f64) as i64),
            Ticks((len * Ticks::QUARTER.0 as f64) as i64),
            *pitch,
            *vel,
        );
        clip.notes.push(n);
    }
    t.clips.push(clip);
    p.tracks.push(t);
    p
}

fn render_of(p: &Project, tail: f64) -> Vec<f32> {
    let snap = Arc::new(Snapshot::compile(p, 1));
    render(
        snap,
        &RenderOptions {
            tail_seconds: tail,
            ..Default::default()
        },
    )
    .unwrap()
    .samples
}

const PHRASE: [(f64, f64, u8, u8); 5] = [
    (0.0, 2.0, 48, 90),
    (0.0, 2.0, 55, 80),
    (0.5, 1.0, 64, 100),
    (1.0, 1.0, 67, 70),
    (2.0, 1.0, 72, 110),
];

#[test]
fn it_plays_finite_and_in_headroom() {
    let out = render_of(&piano_project(&[], &PHRASE), 1.0);
    assert!(out.iter().all(|x| x.is_finite()));
    let peak = out.iter().fold(0f32, |m, x| m.max(x.abs()));
    assert!(peak > 0.01 && peak < 1.0, "peak {peak}");
}

#[test]
fn every_parameter_at_its_default_changes_no_sample() {
    let reference = render_of(&piano_project(&[], &PHRASE), 0.5);
    for info in cantus_core::inst::chorda::PARAMS {
        let got = render_of(&piano_project(&[(info.key, info.default)], &PHRASE), 0.5);
        assert_null(&reference, &got, 0.0, info.key);
    }
}

/// The frequency of the strongest component near `hz` (zero crossings of a band around it
/// are too coarse for cents; a DFT search is enough for a few cents).
fn frequency_near(x: &[f64], fs: f64, hz: f64) -> f64 {
    let mut best = (0.0, hz);
    let mut f = hz * 0.98;
    while f < hz * 1.02 {
        let (mut re, mut im) = (0.0, 0.0);
        for (i, v) in x.iter().enumerate() {
            let (s, c) = math::sin_cos(std::f64::consts::TAU * f * i as f64 / fs);
            re += v * c;
            im += v * s;
        }
        let m = re * re + im * im;
        if m > best.0 {
            best = (m, f);
        }
        f += hz * 0.0002;
    }
    best.1
}

#[test]
fn an_offline_render_waits_for_its_tuning_and_plays_it() {
    // A4 at 442 Hz: a tuning nobody designed before this test, designed while the render
    // waits.
    let p = piano_project(&[("a4", 442.0)], &[(0.0, 4.0, 69, 100)]);
    let out = render_of(&p, 0.0);
    let left: Vec<f64> = out
        .chunks(2)
        .map(|f| f[0] as f64)
        .skip(4_800)
        .take(24_000)
        .collect();
    let f = frequency_near(&left, SR as f64, 442.0);
    let cents = 1200.0 * math::log2(f / 442.0);
    assert!(cents.abs() < 3.0, "A4 at {f:.2} Hz ({cents:+.1} cents)");
}

fn on(key: u8, velocity: f32, id: u64) -> NoteOn {
    NoteOn {
        key,
        velocity,
        id,
        channel: 0,
    }
}

fn play(u: &mut Unit, frames: usize) -> Vec<f32> {
    let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
    for (a, b) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
        u.process(a, b);
    }
    l
}

#[test]
fn the_pedals_arrive_as_controllers() {
    let mut u = Unit::new(SR);
    u.controller(Controller::Sustain, 1.0);
    u.controller(Controller::Cc(SOFT_CC), 1.0);
    let t = u.telemetry();
    assert_eq!((t[tel::PEDAL], t[tel::SOFT]), (1.0, 1.0));
    assert!(!u.at_rest(), "a held pedal is not rest");
    u.controller(Controller::Sustain, 0.0);
    u.controller(Controller::Cc(SOFT_CC), 0.0);
    assert!(u.at_rest());
    assert_eq!(u.telemetry()[tel::DESIGNED], 1.0);
}

#[test]
fn at_rest_it_is_as_good_as_new() {
    let mut a = Unit::new(SR);
    a.note_on(on(72, 0.8, 1));
    play(&mut a, 4_800);
    a.note_off(72, 1, 0.0);
    let mut n = 0;
    while !a.at_rest() {
        play(&mut a, 4_800);
        n += 1;
        assert!(n < 200, "it never came to rest");
    }
    let mut b = Unit::new(SR);
    b.skip((n + 1) * 4_800);
    for u in [&mut a, &mut b] {
        u.note_on(on(60, 0.6, 7));
    }
    let (x, y) = (play(&mut a, 9_600), play(&mut b, 9_600));
    assert_null(&x, &y, 0.0, "the rested unit against a new one");
}

fn peak_db(x: &[f32]) -> f64 {
    let p = x.iter().fold(0f32, |m, v| m.max(v.abs())) as f64;
    20.0 * math::log10(p.max(1e-12))
}

#[test]
fn its_levels_leave_headroom_and_a_middle_c_is_heard() {
    // Both hands fortissimo, the pedal down: under −2 dBFS at the defaults.
    let mut u = Unit::new(SR);
    u.controller(Controller::Sustain, 1.0);
    for (i, k) in [21u8, 33, 45, 52, 57, 64, 69, 76, 88, 96]
        .iter()
        .enumerate()
    {
        u.note_on(on(*k, 1.0, i as u64 + 1));
    }
    let loud = peak_db(&play(&mut u, 48_000));
    // A mezzo-forte middle C: clearly heard, well under the top.
    let mut u = Unit::new(SR);
    u.note_on(on(60, 0.6, 1));
    let mf = peak_db(&play(&mut u, 24_000));
    eprintln!("ff both hands with pedal {loud:.1} dBFS, mf middle C {mf:.1} dBFS");
    assert!(loud < -2.0, "{loud}");
    assert!((-30.0..-12.0).contains(&mf), "{mf}");
}

#[test]
#[ignore]
fn print_the_keys_levels() {
    for k in [21u8, 28, 33, 40, 45, 52, 57, 64, 69, 76, 81, 88, 96, 108] {
        let mut row = String::new();
        for v in [0.2f32, 0.5, 0.8, 1.0] {
            let mut u = Unit::new(SR);
            u.note_on(on(k, v, 1));
            let x = play(&mut u, 24_000);
            let rms =
                (x.iter().map(|s| (*s as f64) * (*s as f64)).sum::<f64>() / x.len() as f64).sqrt();
            row += &format!(
                "  v{v}: pk {:6.1} rms {:6.1}",
                peak_db(&x),
                20.0 * math::log10(rms.max(1e-12))
            );
        }
        eprintln!("key {k:3}{row}");
    }
}

/// The project played through the processor as the audio callback plays it (units sent
/// before the snapshot, blocks of changing size).
fn real_time(p: &Project, frames: usize) -> Vec<f32> {
    let snap = Arc::new(Snapshot::compile(p, 1));
    let (mut tx, rx) = rtrb::RingBuffer::<EngineMessage>::new(256);
    let (etx, mut erx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc_ = Processor::without_midi(SR as f64, rx, etx);
    for unit in cantus_engine::inst::units_for(&snap, SR, |_, _| false) {
        tx.push(EngineMessage::AddInstrument(unit)).unwrap();
    }
    tx.push(EngineMessage::SetSnapshot(snap)).unwrap();
    proc_.settle();
    while let Ok(ev) = erx.pop() {
        if let EngineEvent::PdcNeed { count, delay } = ev {
            tx.push(EngineMessage::PdcLines(pdc::lines_for(count, delay)))
                .unwrap();
            proc_.settle();
        }
    }
    tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let sizes = [128usize, 333, 512, 96, 1024, 17];
    let mut out = Vec::with_capacity(frames * 2);
    let mut i = 0;
    while out.len() / 2 < frames {
        let n = sizes[i % sizes.len()].min(frames - out.len() / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        proc_.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while erx.pop().is_ok() {}
    }
    out
}

#[test]
fn offline_render_equals_real_time() {
    // The reference tuning, and one designed before playing (in real time a tuning still
    // being designed plays the reference one until it is ready; offline waits for it).
    cantus_engine::inst::chorda::ensure(
        cantus_dsp::chorda::design::Tuning {
            a4: 442.0,
            stretch: 1.0,
            stiffness: 1.0,
        },
        SR,
    );
    for values in [vec![], vec![("a4", 442.0), ("hardness", 0.4)]] {
        let p = piano_project(&values, &PHRASE);
        let offline = render_of(&p, 0.5);
        let live = real_time(&p, offline.len() / 2);
        assert_null(
            &offline,
            &live,
            0.0,
            &format!("real time against offline {values:?}"),
        );
    }
}
