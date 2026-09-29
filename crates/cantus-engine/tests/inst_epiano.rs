//! Lamina through the engine: the house rules at tolerance 0, pitch across the keyboard,
//! aliasing of the pickup and preamp on a high note, per-note modulation, the continuous
//! sustain pedal as a controller, and a CPU budget per voice.

mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::inst::epiano::{tel, Unit};
use cantus_engine::inst::{Controller, InstrumentUnit, NoteOn};
use cantus_engine::*;
use std::sync::Arc;

const SR: f32 = 48_000.0;

fn ep_project(values: &[(&str, f32)], notes: &[(f64, f64, u8, u8)]) -> Project {
    let mut p = Project::new("epiano");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Keys");
    let mut spec = NativeSpec::new(InstrumentKind::ElectricPiano);
    for (k, v) in values {
        assert!(spec.set(k, *v).is_some(), "{k}");
    }
    t.instrument = InstrumentSpec::Native(spec);
    let mut clip = p.make_clip("Play", Ticks::ZERO, Ticks::beats(12));
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

fn busy(engine: f32) -> Project {
    ep_project(
        &[
            ("engine", engine),
            ("trem.depth", 0.5),
            ("drive", 0.5),
            ("polyphony", 8.0),
        ],
        &[
            (0.0, 2.0, 40, 90),
            (0.0, 1.0, 64, 127),
            (0.25, 0.5, 67, 60),
            (0.5, 0.25, 71, 30),
            (1.0, 2.0, 64, 110),
            (1.5, 1.0, 76, 100),
            (1.5, 1.0, 79, 100),
            (1.5, 1.0, 83, 100),
            (1.5, 1.0, 86, 100),
            (1.75, 1.0, 88, 100),
            (2.0, 1.0, 91, 100),
        ],
    )
}

#[test]
fn a_busy_performance_is_identical_at_every_block_size() {
    for engine in [0.0, 1.0] {
        let p = busy(engine);
        let reference = render_at(&p, 512, 1.0);
        assert!(reference.iter().any(|x| *x != 0.0));
        for block in [1usize, 7, 64, 256, 4096] {
            assert_null(
                &reference,
                &render_at(&p, block, 1.0),
                0.0,
                &format!("engine {engine}, block {block}"),
            );
        }
        assert_null(&reference, &render_at(&p, 512, 1.0), 0.0, "a second render");
    }
}

#[test]
fn every_value_at_its_default_changes_no_sample() {
    let p = busy(0.0);
    let reference = render_at(&p, 256, 0.5);
    let mut q = p.clone();
    let spec = q.tracks[0].instrument.native_mut().unwrap();
    for info in InstrumentKind::ElectricPiano.params() {
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

/// Amplitude at `hz` (Goertzel, Hann window).
fn amp_at(x: &[f32], hz: f64) -> f64 {
    let n = x.len();
    let w = std::f64::consts::TAU * hz / SR as f64;
    let (mut re, mut im, mut norm) = (0.0, 0.0, 0.0);
    for (i, v) in x.iter().enumerate() {
        let win = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / n as f64).cos();
        re += *v as f64 * win * (w * i as f64).cos();
        im += *v as f64 * win * (w * i as f64).sin();
        norm += win;
    }
    2.0 * (re * re + im * im).sqrt() / norm
}

fn peak_hz(x: &[f32], guess: f64) -> f64 {
    let mut best = (guess, 0.0);
    for k in 0..=400 {
        let f = guess * (0.98 + 0.04 * k as f64 / 400.0);
        let a = amp_at(x, f);
        if a > best.1 {
            best = (f, a);
        }
    }
    let step = guess * 0.04 / 400.0;
    let mut fine = best;
    for k in 0..=100 {
        let f = best.0 - step + 2.0 * step * k as f64 / 100.0;
        let a = amp_at(x, f);
        if a > fine.1 {
            fine = (f, a);
        }
    }
    fine.0
}

#[test]
fn pitch_is_equal_temperament_across_the_keyboard() {
    let mut worst = 0.0f64;
    for engine in [0.0f32, 1.0] {
        for key in (33u8..=93).step_by(6) {
            let p = ep_project(
                &[
                    ("engine", engine),
                    ("spread", 0.0),
                    ("age", 0.0),
                    ("hammer.noise", -60.0),
                ],
                &[(0.0, 3.0, key, 90)],
            );
            let out = left(&render_at(&p, 256, 0.0));
            let want = 440.0 * 2f64.powf((key as f64 - 69.0) / 12.0);
            let hz = peak_hz(&out[9_600..57_600], want);
            let ct = 1200.0 * (hz / want).log2();
            worst = worst.max(ct.abs());
            assert!(ct.abs() < 0.5, "engine {engine} key {key}: {ct:+.3} ct");
        }
    }
    println!("lamina: worst pitch error {worst:.3} ct");
}

#[test]
fn a_high_note_at_full_velocity_keeps_its_aliases_down() {
    // B6 (1976 Hz) at full velocity, through the pickup and a driven preamp: any component
    // that is not a harmonic is an alias (or a decaying inharmonic mode, which the window
    // after the attack leaves out).
    for (engine, name) in [(0.0f32, "tine"), (1.0, "reed")] {
        let p = ep_project(
            &[
                ("engine", engine),
                ("spread", 0.0),
                ("age", 0.0),
                ("hammer.noise", -60.0),
                ("tine.bell", -40.0),
                ("reed.overtones", -40.0),
                ("drive", 0.6),
            ],
            &[(0.0, 4.0, 95, 127)],
        );
        let out = left(&render_at(&p, 256, 0.0));
        let n = 1 << 15;
        let x = &out[24_000..24_000 + n];
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
        let f0 = 440.0 * 2f64.powf((95.0 - 69.0) / 12.0);
        let b0 = (f0 * n as f64 / SR as f64).round() as usize;
        let fund = mag[b0 - 3..b0 + 4].iter().cloned().fold(0.0, f64::max);
        let mut worst = 0.0f64;
        for (k, m) in mag.iter().enumerate().skip(8) {
            let hz = k as f64 * SR as f64 / n as f64;
            let h = hz / f0;
            if (h - h.round()).abs() * f0 < 80.0 {
                continue;
            }
            worst = worst.max(*m);
        }
        let floor = 20.0 * (worst / fund).log10();
        println!("lamina: {name} alias floor at B6, full velocity: {floor:.1} dB");
        assert!(floor < -60.0, "{name}: aliases at {floor:.1} dB");
    }
}

#[test]
fn velocity_can_drive_a_parameter_note_by_note() {
    // vel → pickup level: a loud note gets louder still; a soft one does not move.
    let notes = [(0.0, 1.0, 60u8, 127u8), (2.0, 1.0, 60, 20)];
    let plain = ep_project(&[], &notes);
    let mut routed = plain.clone();
    let spec = routed.tracks[0].instrument.native_mut().unwrap();
    spec.set_text("mod1.src", Some("vel".into())).unwrap();
    spec.set_text("mod1.dst", Some("pickup.level".into()))
        .unwrap();
    spec.set("mod1.amt", 0.25).unwrap();
    let a = left(&render_at(&plain, 256, 0.0));
    let b = left(&render_at(&routed, 256, 0.0));
    let loud = 20.0 * (rms(&b[..24_000]) / rms(&a[..24_000])).log10();
    let soft = 20.0 * (rms(&b[48_000..72_000]) / rms(&a[48_000..72_000])).log10();
    assert!(
        loud > soft + 3.0,
        "loud note +{loud:.1} dB, soft note +{soft:.1} dB"
    );
}

fn unit() -> Unit {
    let mut u = Unit::new(SR);
    u.configure(&NativeSpec::new(InstrumentKind::ElectricPiano), &[]);
    u
}

fn play(u: &mut Unit, frames: usize) -> Vec<f32> {
    let mut l = vec![0.0; frames];
    let mut r = vec![0.0; frames];
    for (a, b) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
        u.process(a, b);
    }
    l
}

#[test]
fn the_sustain_pedal_arrives_as_a_controller_and_half_pedals() {
    let tail = |pedal: f32| {
        let mut u = unit();
        u.controller(Controller::Sustain, pedal);
        u.note_on(NoteOn {
            key: 57,
            velocity: 0.8,
            id: 1,
            channel: 0,
        });
        play(&mut u, 9_600);
        u.note_off(57, 1, 0.5);
        play(&mut u, 2_400);
        let t = u.telemetry();
        (rms(&play(&mut u, 9_600)), t[tel::PEDAL], t[tel::ENGAGEMENT])
    };
    let (up, p0, e0) = tail(0.0);
    let (half, _, eh) = tail(0.35);
    let (down, p1, e1) = tail(1.0);
    assert_eq!((p0, e0), (0.0, 1.0));
    assert_eq!((p1, e1), (1.0, 0.0));
    assert!(eh > 0.2 && eh < 0.8, "half pedal engagement {eh}");
    assert!(
        up < half && half < down,
        "tails: up {up}, half {half}, down {down}"
    );
}

#[test]
fn cpu_per_voice_stays_small() {
    // The best of five 2-second runs, scaled to 10 s: preemption on a shared machine can
    // only add time, so the minimum is the instrument's own cost.
    let time = |voices: usize| {
        let mut best = f64::INFINITY;
        let mut held = 0;
        for _ in 0..5 {
            let mut u = unit();
            for i in 0..voices {
                u.note_on(NoteOn {
                    key: 36 + (i * 7 % 60) as u8,
                    velocity: 0.9,
                    id: i as u64 + 1,
                    channel: 0,
                });
            }
            u.controller(Controller::Sustain, 1.0);
            held = u.active_voices();
            let mut l = vec![0.0; 256];
            let mut r = vec![0.0; 256];
            let start = std::time::Instant::now();
            for _ in 0..(2 * 48_000 / 256) {
                l.fill(0.0);
                r.fill(0.0);
                u.process(&mut l, &mut r);
            }
            best = best.min(start.elapsed().as_secs_f64());
        }
        (best * 5.0, held)
    };
    let (t20, n20) = time(20);
    let (t32, n32) = time(32);
    println!(
        "lamina: 10 s of audio with {n20} voices {t20:.3} s ({:.2} % of one core), {n32} voices {t32:.3} s",
        t20 / 10.0 * 100.0
    );
    assert_eq!(n20, 20);
    // A generous ceiling, as the house's budget tests keep: faster than real time on a loaded
    // machine in a dev build. It catches the pathologies (a mode sinking into denormals
    // made 13 voices take 26 s here once), not the load of a dozen builders.
    assert!(t32 < 10.0, "32 voices: {t32:.3} s for 10 s of audio");
}
