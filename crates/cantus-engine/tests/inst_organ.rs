//! Organum through the engine: the house rules at tolerance 0, pitch across the keyboard,
//! aliasing of the preamp, the fixture's macro connection, controllers, and a CPU budget
//! that does not grow with the number of keys held.

mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_dsp::organum::wheels::{cents, gear_hz, wheel_of};
use cantus_engine::inst::organ::{tel, Unit};
use cantus_engine::inst::{Controller, InstrumentUnit, NoteOn};
use cantus_engine::*;
use std::sync::Arc;

const SR: f32 = 48_000.0;

fn organ_project(values: &[(&str, f32)], notes: &[(f64, f64, u8)]) -> Project {
    let mut p = Project::new("organ");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Organ");
    let mut spec = NativeSpec::new(InstrumentKind::Organ);
    for (k, v) in values {
        assert!(spec.set(k, *v).is_some(), "{k}");
    }
    t.instrument = InstrumentSpec::Native(spec);
    let mut clip = p.make_clip("Play", Ticks::ZERO, Ticks::beats(16));
    for (start, len, pitch) in notes {
        let n = p.make_note(
            Ticks((start * Ticks::QUARTER.0 as f64) as i64),
            Ticks((len * Ticks::QUARTER.0 as f64) as i64),
            *pitch,
            100,
        );
        clip.notes.push(n);
    }
    t.clips.push(clip);
    p.tracks.push(t);
    p
}

fn render_at(p: &Project, block: usize, tail: f64) -> Vec<f32> {
    let opts = RenderOptions {
        tail_seconds: tail,
        block_frames: block,
        ..Default::default()
    };
    render(Arc::new(Snapshot::compile(p, 1)), &opts)
        .unwrap()
        .samples
}

fn left(x: &[f32]) -> Vec<f32> {
    x.chunks(2).map(|c| c[0]).collect()
}

fn rms(x: &[f32]) -> f64 {
    (x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len().max(1) as f64).sqrt()
}

/// A performance that moves everything: a split with pedal, lower and upper, the
/// percussion retriggering, legato, chords.
fn busy() -> Project {
    organ_project(
        &[
            ("split.upper", 60.0),
            ("split.pedal", 48.0),
            ("perc", 1.0),
            ("rot.speed", 2.0),
            ("drive", 0.55),
        ],
        &[
            (0.0, 2.0, 36),
            (0.0, 3.5, 55),
            (0.0, 1.0, 72),
            (0.5, 1.0, 76),
            (1.5, 0.25, 79),
            (2.0, 2.0, 43),
            (2.5, 1.0, 84),
            (2.75, 1.0, 88),
        ],
    )
}

#[test]
fn a_busy_performance_is_identical_at_every_block_size() {
    let p = busy();
    let reference = render_at(&p, 512, 1.0);
    assert!(reference.iter().any(|x| *x != 0.0));
    for block in [1usize, 7, 64, 256, 4096] {
        assert_null(
            &reference,
            &render_at(&p, block, 1.0),
            0.0,
            &format!("block {block}"),
        );
    }
    assert_null(&reference, &render_at(&p, 512, 1.0), 0.0, "a second render");
}

#[test]
fn every_value_at_its_default_changes_no_sample() {
    let p = busy();
    let reference = render_at(&p, 256, 0.5);
    let mut q = p.clone();
    let spec = q.tracks[0].instrument.native_mut().unwrap();
    for info in InstrumentKind::Organ.params() {
        if !spec.is_stored(info.key) {
            spec.set(info.key, info.default);
        }
    }
    assert_null(
        &reference,
        &render_at(&q, 256, 0.5),
        0.0,
        "defaults written out",
    );
}

fn zc_hz(x: &[f32]) -> f64 {
    let mut first = None;
    let (mut last, mut count) = (0.0, 0usize);
    for i in 1..x.len() {
        if x[i - 1] < 0.0 && x[i] >= 0.0 {
            let t = (i - 1) as f64 + (-x[i - 1] as f64) / (x[i] as f64 - x[i - 1] as f64);
            if first.is_none() {
                first = Some(t);
            } else {
                count += 1;
            }
            last = t;
        }
    }
    count as f64 / ((last - first.unwrap()) / SR as f64)
}

#[test]
fn pitch_is_the_gear_pitch_across_the_keyboard() {
    let mut worst = 0.0f64;
    for key in (36u8..=96).step_by(5) {
        let p = organ_project(
            &[
                ("u1", 0.0),
                ("u2", 0.0),
                ("rot.out", 1.0),
                ("vib.upper", 0.0),
                ("drive", 0.0),
                ("leak", -80.0),
                ("leak.bar", -80.0),
            ],
            &[(0.0, 4.0, key)],
        );
        let out = left(&render_at(&p, 256, 0.0));
        // The steady part of the note: after the contacts settle, before they open.
        let hz = zc_hz(&out[4_800..91_200]);
        let err = cents(hz, gear_hz(wheel_of(key as i32)));
        worst = worst.max(err.abs());
        assert!(
            err.abs() < 0.05,
            "key {key}: {hz:.4} Hz is {err:+.4} ct off its gear"
        );
    }
    println!("organ: worst pitch error against the gear table {worst:.4} ct");
}

#[test]
fn the_preamp_keeps_its_aliases_down() {
    // C7 (2093 Hz) at full drive: every component that is not a harmonic is an alias.
    let p = organ_project(
        &[
            ("u1", 0.0),
            ("u2", 0.0),
            ("rot.out", 1.0),
            ("vib.upper", 0.0),
            ("drive", 1.0),
            ("leak", -80.0),
            ("leak.bar", -80.0),
            ("click", -60.0),
        ],
        &[(0.0, 4.0, 96)],
    );
    let out = left(&render_at(&p, 256, 0.0));
    let n = 1 << 15;
    let x = &out[9_600..9_600 + n];
    let mut re: Vec<f32> = x
        .iter()
        .enumerate()
        .map(|(i, v)| v * (0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / n as f32).cos()))
        .collect();
    let mut im = vec![0.0; n];
    cantus_dsp::spectrum::fft(&mut re, &mut im);
    let mag: Vec<f64> = (0..n / 2)
        .map(|k| ((re[k] as f64).powi(2) + (im[k] as f64).powi(2)).sqrt())
        .collect();
    let f0 = gear_hz(wheel_of(96));
    let bin = |hz: f64| hz * n as f64 / SR as f64;
    let fund = mag[bin(f0).round() as usize - 2..bin(f0).round() as usize + 3]
        .iter()
        .cloned()
        .fold(0.0, f64::max);
    let mut worst = 0.0f64;
    for (k, m) in mag.iter().enumerate().skip(8) {
        let hz = k as f64 * SR as f64 / n as f64;
        let h = hz / f0;
        if (h - h.round()).abs() * f0 < 60.0 {
            continue; // a harmonic (or its window skirt)
        }
        worst = worst.max(*m);
    }
    let floor = 20.0 * (worst / fund).log10();
    println!("organ: preamp alias floor at C7, full drive: {floor:.1} dB");
    assert!(
        floor < -60.0,
        "aliases at {floor:.1} dB against the fundamental"
    );
}

#[test]
fn the_fixtures_macro_drives_the_gain() {
    let notes = [(0.0, 2.0, 60u8), (0.0, 2.0, 64)];
    let plain = organ_project(&[("macro1", 0.5)], &notes);
    let mut routed = plain.clone();
    let spec = routed.tracks[0].instrument.native_mut().unwrap();
    spec.set_text("mod1.src", Some("macro1".into())).unwrap();
    spec.set_text("mod1.dst", Some("gain".into())).unwrap();
    spec.set("mod1.amt", 0.25).unwrap();
    let a = render_at(&plain, 256, 0.0);
    let b = render_at(&routed, 256, 0.0);
    // macro 0.5 × depth 0.25 × the gain's 60 dB range = +7.5 dB.
    let db = 20.0 * (rms(&b) / rms(&a)).log10();
    assert!(
        (db - 7.5).abs() < 0.01,
        "the connection moved the gain {db:.3} dB"
    );
}

fn unit_with(values: &[(&str, f32)]) -> Unit {
    let mut spec = NativeSpec::new(InstrumentKind::Organ);
    for (k, v) in values {
        spec.set(k, *v);
    }
    let mut u = Unit::new(SR);
    u.configure(&spec, &[]);
    u
}

fn play(u: &mut Unit, frames: usize) -> Vec<f32> {
    let mut l = vec![0.0; frames];
    let mut r = vec![0.0; frames];
    for (cl, cr) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
        u.process(cl, cr);
    }
    l
}

#[test]
fn the_mod_wheel_spins_the_rotor_up_and_expression_is_the_swell() {
    let mut u = unit_with(&[]);
    u.note_on(NoteOn {
        key: 60,
        velocity: 0.8,
        id: 1,
        channel: 0,
    });
    play(&mut u, 4_800);
    assert!((u.telemetry()[tel::HORN_RPM] - 40.0).abs() < 1.0);
    u.controller(Controller::ModWheel, 1.0);
    play(&mut u, 48_000);
    let t = u.telemetry();
    assert!(
        t[tel::HORN_RPM] > 400.0,
        "horn at {} rpm a second later",
        t[tel::HORN_RPM]
    );
    assert!(
        t[tel::DRUM_RPM] > 90.0 && t[tel::DRUM_RPM] < 200.0,
        "the drum is slower to spin up: {}",
        t[tel::DRUM_RPM]
    );
    assert_eq!(t[tel::SPEED], 2.0);
    let loud = rms(&play(&mut u, 9_600));
    u.controller(Controller::Expression, 0.25);
    play(&mut u, 4_800);
    let soft = rms(&play(&mut u, 9_600));
    assert!(
        soft < loud * 0.3,
        "the swell pedal at a quarter: {loud} → {soft}"
    );
}

#[test]
fn cpu_does_not_grow_with_the_keys_held() {
    // Ten seconds' worth at 48 kHz with 20 keys held, and with every key of both manuals and
    // the pedals: the same work (the wheels always turn; a key only changes gains).
    let time = |keys: &[(u8, u8)]| {
        let mut u = unit_with(&[
            ("split.upper", 60.0),
            ("split.pedal", 48.0),
            ("perc", 1.0),
            ("u5", 6.0),
            ("u9", 4.0),
        ]);
        for (i, (k, ch)) in keys.iter().enumerate() {
            u.note_on(NoteOn {
                key: *k,
                velocity: 1.0,
                id: i as u64 + 1,
                channel: *ch,
            });
        }
        // The best of five 2-second stretches, scaled to 10 s: preemption on a shared
        // machine can only add time.
        let mut l = vec![0.0; 256];
        let mut r = vec![0.0; 256];
        let mut best = f64::INFINITY;
        for _ in 0..5 {
            let start = std::time::Instant::now();
            for _ in 0..(2 * 48_000 / 256) {
                l.fill(0.0);
                r.fill(0.0);
                u.process(&mut l, &mut r);
            }
            best = best.min(start.elapsed().as_secs_f64());
        }
        best * 5.0
    };
    let twenty: Vec<(u8, u8)> = (0..20).map(|i| (60 + i as u8, 0)).collect();
    let all: Vec<(u8, u8)> = (36..=96u8)
        .map(|k| (k, 0))
        .chain((36..=96u8).map(|k| (k, 1)))
        .chain((36..=67u8).map(|k| (k, 2)))
        .collect();
    let one = time(&[(60, 0)]);
    let t20 = time(&twenty);
    let t_all = time(&all);
    println!(
        "organ: 10 s of audio with 1 key {:.3} s, 20 keys {:.3} s ({:.2} % of one core), {} keys {:.3} s",
        one,
        t20,
        t20 / 10.0 * 100.0,
        all.len(),
        t_all
    );
    // A generous ceiling for a loaded machine: 10 s of audio in under 5 s.
    assert!(t20 < 5.0, "20 keys: {t20:.3} s for 10 s of audio");
    // Constant cost: a loop over held keys would make 154 keys about 7 times 20 keys.
    assert!(
        t_all < t20 * 3.0 + 0.5,
        "all keys {t_all:.3} s against 20 keys {t20:.3} s"
    );
}

#[test]
fn a_loud_full_organ_keeps_headroom() {
    let mut chord = Vec::new();
    for (i, k) in [36u8, 43, 48, 55, 60, 64, 67, 72, 76, 79]
        .iter()
        .enumerate()
    {
        chord.push((i as f64 * 0.01, 3.0, *k));
    }
    let p = organ_project(
        &[
            ("u4", 8.0),
            ("u5", 8.0),
            ("u6", 8.0),
            ("u7", 8.0),
            ("u8", 8.0),
            ("u9", 8.0),
            ("drive", 1.0),
            ("perc", 1.0),
            ("lvl.upper", 6.0),
        ],
        &chord,
    );
    let out = render_at(&p, 256, 0.5);
    let peak = out.iter().fold(0f32, |m, x| m.max(x.abs()));
    println!(
        "organ: ten keys, every drawbar out, full drive: peak {:.2} dBFS",
        20.0 * peak.log10()
    );
    assert!(peak < 1.0, "peak {peak}");
}
