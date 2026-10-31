//! The `transcribe` job through the real helper process (ADR 0086), with the fixture
//! network (Basic Pitch's contract, a tuned filter bank inside) for the note modes and
//! Cantus's own detector for drums: tones become their notes at their times across the
//! windows' joins, melody mode keeps one line, drums need no model, a cancel stops it and
//! leaves no file, a crash is reported, wrong input refused, the fixture refused outside
//! tests.

// Sample loops over index ranges read clearer than iterator chains here.
#![allow(clippy::needless_range_loop)]

use cantus_dsp::onset::eval;
use cantus_ml::client::{ClientConfig, Failure, MlClient, Outcome};
use cantus_ml::proto::JobError;
use cantus_ml::transcribe::spec::*;
use std::path::{Path, PathBuf};
use std::time::Duration;

const SR: u32 = 22_050;

fn client(hooks: bool) -> MlClient {
    let mut c = ClientConfig::new(env!("CARGO_BIN_EXE_cantus-ml"));
    c.test_hooks = hooks;
    MlClient::new(c)
}

fn write(path: &Path, rate: u32, x: &[f32]) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for v in x {
        w.write_sample(*v).unwrap();
    }
    w.finalize().unwrap();
}

/// Sine tones: `(start s, end s, midi)`, with 5 ms ramps.
fn tones(notes: &[(f64, f64, u8)], seconds: f64) -> Vec<f32> {
    let n = (seconds * SR as f64) as usize;
    let mut x = vec![0.0f32; n];
    for &(a, b, m) in notes {
        let hz = 440.0 * 2f64.powf((m as f64 - 69.0) / 12.0);
        let (i0, i1) = ((a * SR as f64) as usize, ((b * SR as f64) as usize).min(n));
        for i in i0..i1 {
            let t = (i - i0) as f64 / SR as f64;
            let ramp = (t / 0.005)
                .min(1.0)
                .min((i1 - i) as f64 / SR as f64 / 0.005);
            x[i] += (0.15 * ramp * (std::f64::consts::TAU * hz * t).sin()) as f32;
        }
    }
    x
}

fn args(mode: Mode, input: PathBuf, out: PathBuf, family: NoteFamily) -> TranscribeArgs {
    TranscribeArgs {
        mode,
        input,
        out,
        runtime: None,
        model: Some(NoteModel {
            graph: PathBuf::from("fixture"),
            family,
            check: Vec::new(),
        }),
        notes: NoteSettings::default(),
        drums: DrumSettings::default(),
    }
}

fn run(c: &mut MlClient, a: &TranscribeArgs) -> Outcome {
    let id = c.start("transcribe", serde_json::to_value(a).unwrap());
    c.wait(id, Duration::from_secs(120)).expect("ended")
}

fn done(o: Outcome) -> TranscribeResult {
    match o {
        Outcome::Done(v) => serde_json::from_value(v).unwrap(),
        other => panic!("{other:?}"),
    }
}

fn read(path: &Path) -> Transcription {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn fixture() -> NoteFamily {
    NoteFamily::Fixture {
        delay_ms: 0,
        crash_at: None,
    }
}

#[test]
fn tones_become_their_notes_across_the_windows_joins() {
    let dir = tempfile::tempdir().unwrap();
    // Seven seconds: five windows. A chord, a line, a note held across the first join
    // (1.64 s) and one across the second (3.28 s).
    let truth = [
        (0.20, 1.00, 60),
        (0.20, 1.00, 64),
        (0.20, 1.00, 67),
        (1.20, 2.40, 72),
        (2.60, 4.00, 69),
        (4.20, 4.60, 71),
        (4.80, 5.30, 74),
        (5.50, 6.60, 65),
    ];
    let input = dir.path().join("in.wav");
    write(&input, SR, &tones(&truth, 7.0));
    let out = dir.path().join("notes.json");
    let mut c = client(true);
    let r = done(run(
        &mut c,
        &args(Mode::Polyphonic, input, out.clone(), fixture()),
    ));
    assert_eq!(r.inferences, 5);
    assert_eq!(r.device, "fixture");
    assert!((r.duration - 7.0).abs() < 1e-3);
    let t = read(&out);
    assert_eq!(t.notes.len(), r.notes);
    let found: Vec<eval::Note> = t
        .notes
        .iter()
        .map(|n| eval::Note {
            onset: n.start,
            offset: n.end,
            pitch: n.pitch as f64,
        })
        .collect();
    let want: Vec<eval::Note> = truth
        .iter()
        .map(|&(a, b, p)| eval::Note {
            onset: a,
            offset: b,
            pitch: p as f64,
        })
        .collect();
    let s = eval::notes(&want, &found, None);
    assert_eq!(s.f, 1.0, "{s:?}\n{:#?}", t.notes);
    let s = eval::notes(&want, &found, Some(0.2));
    assert!(s.f >= 0.85, "with offsets: {s:?}\n{:#?}", t.notes);
    assert!(t.notes.iter().all(|n| (1..=127).contains(&n.velocity)));
    // Sine tones hold their pitch: no bends.
    assert!(t.notes.iter().all(|n| n.bends.is_empty()), "{:?}", t.notes);
}

#[test]
fn melody_mode_keeps_one_line() {
    let dir = tempfile::tempdir().unwrap();
    // A line over a held chord: the chord is quieter, the line wins where they overlap.
    let mut x = tones(&[(0.2, 0.8, 72), (0.9, 1.6, 74), (1.7, 2.6, 76)], 3.0);
    for (a, b) in x.iter_mut().zip(tones(&[(0.2, 2.6, 55)], 3.0)) {
        *a += 0.3 * b;
    }
    let input = dir.path().join("in.wav");
    write(&input, SR, &x);
    let out = dir.path().join("notes.json");
    let mut c = client(true);
    done(run(
        &mut c,
        &args(Mode::Melody, input.clone(), out.clone(), fixture()),
    ));
    let line = read(&out).notes;
    for w in line.windows(2) {
        assert!(w[0].end <= w[1].start + 1e-9, "{line:#?}");
    }
    let pitches: Vec<u8> = line.iter().map(|n| n.pitch).collect();
    assert!(pitches.windows(3).any(|w| w == [72, 74, 76]), "{line:#?}");
    // Polyphonic mode on the same file has the chord's note too.
    let out2 = dir.path().join("poly.json");
    done(run(
        &mut c,
        &args(Mode::Polyphonic, input, out2.clone(), fixture()),
    ));
    assert!(read(&out2).notes.iter().any(|n| n.pitch == 55));
}

#[test]
fn drums_need_no_model_and_take_any_rate() {
    let dir = tempfile::tempdir().unwrap();
    let rate = 44_100u32;
    let mut x = vec![0.0f32; 4 * rate as usize];
    let mut seed = 1u64;
    for k in 0..8 {
        let at = (0.25 + 0.5 * k as f64) * rate as f64;
        let at = at as usize;
        // A kick: a falling sine; a hat between kicks: high noise.
        for i in 0..(0.3 * rate as f64) as usize {
            let t = i as f64 / rate as f64;
            let f = 50.0 + 100.0 * (-t / 0.03).exp();
            let ph = std::f64::consts::TAU * (50.0 * t + 100.0 * 0.03 * (1.0 - (-t / 0.03).exp()));
            let _ = f;
            if let Some(v) = x.get_mut(at + i) {
                *v += (0.8 * ph.sin() * (-t / 0.15).exp()) as f32;
            }
        }
        let hat = at + rate as usize / 4;
        let mut prev = 0.0f32;
        for i in 0..(0.05 * rate as f64) as usize {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let n = ((seed >> 40) as f32 / (1u64 << 23) as f32) - 1.0;
            let hp = n - prev;
            prev = n;
            let t = i as f32 / rate as f32;
            if let Some(v) = x.get_mut(hat + i) {
                *v += 0.3 * hp * (-t / 0.015).exp();
            }
        }
    }
    let input = dir.path().join("drums.wav");
    write(&input, rate, &x);
    let out = dir.path().join("hits.json");
    let mut a = args(Mode::Drums, input, out.clone(), fixture());
    a.model = None;
    let mut c = client(false);
    let r = done(run(&mut c, &a));
    assert_eq!((r.device.as_str(), r.inferences), ("dsp", 0));
    let t = read(&out);
    let kicks: Vec<f64> = t
        .hits
        .iter()
        .filter(|h| h.class == DrumClass::Kick)
        .map(|h| h.time)
        .collect();
    let hats: Vec<f64> = t
        .hits
        .iter()
        .filter(|h| h.class == DrumClass::HiHat)
        .map(|h| h.time)
        .collect();
    let want_k: Vec<f64> = (0..8).map(|k| 0.25 + 0.5 * k as f64).collect();
    let want_h: Vec<f64> = want_k.iter().map(|k| k + 0.25).collect();
    assert_eq!(eval::onsets(&want_k, &kicks, 0.05).f, 1.0, "{:?}", t.hits);
    assert!(eval::onsets(&want_h, &hats, 0.05).f >= 0.9, "{:?}", t.hits);
    assert!(t
        .hits
        .iter()
        .filter(|h| h.class == DrumClass::Kick)
        .all(|h| h.key == 36));
}

#[test]
fn a_cancel_stops_the_job_and_leaves_no_file() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.wav");
    write(&input, SR, &tones(&[(0.1, 9.0, 60)], 10.0));
    let out = dir.path().join("notes.json");
    let mut c = client(true);
    let a = args(
        Mode::Polyphonic,
        input,
        out.clone(),
        NoteFamily::Fixture {
            delay_ms: 400,
            crash_at: None,
        },
    );
    let id = c.start("transcribe", serde_json::to_value(&a).unwrap());
    std::thread::sleep(Duration::from_millis(600));
    let t0 = std::time::Instant::now();
    c.cancel(id);
    let o = c.wait(id, Duration::from_secs(30)).unwrap();
    assert_eq!(o, Outcome::Cancelled);
    assert!(t0.elapsed() < Duration::from_secs(3));
    assert!(!out.exists());
}

#[test]
fn a_crash_inside_the_job_is_reported_as_one() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.wav");
    write(&input, SR, &tones(&[(0.1, 3.0, 60)], 4.0));
    let mut c = client(true);
    let a = args(
        Mode::Polyphonic,
        input,
        dir.path().join("n.json"),
        NoteFamily::Fixture {
            delay_ms: 0,
            crash_at: Some(1),
        },
    );
    assert!(matches!(
        run(&mut c, &a),
        Outcome::Failed(Failure::Crashed(_))
    ));
}

#[test]
fn wrong_input_and_settings_are_refused_and_the_fixture_needs_tests() {
    let dir = tempfile::tempdir().unwrap();
    let at44 = dir.path().join("44.wav");
    write(&at44, 44_100, &vec![0.0; 44_100]);
    let mut c = client(true);
    let o = run(
        &mut c,
        &args(Mode::Polyphonic, at44, dir.path().join("a.json"), fixture()),
    );
    assert!(
        matches!(o, Outcome::Failed(Failure::Job(JobError::Input(_)))),
        "{o:?}"
    );
    let ok = dir.path().join("ok.wav");
    write(&ok, SR, &vec![0.0; 22_050]);
    let mut a = args(
        Mode::Polyphonic,
        ok.clone(),
        dir.path().join("b.json"),
        fixture(),
    );
    a.notes.onset_threshold = 1.5;
    assert!(matches!(
        run(&mut c, &a),
        Outcome::Failed(Failure::Job(JobError::BadArgs(_)))
    ));
    let mut a = args(
        Mode::Polyphonic,
        ok.clone(),
        dir.path().join("c.json"),
        fixture(),
    );
    a.model = None;
    assert!(matches!(
        run(&mut c, &a),
        Outcome::Failed(Failure::Job(JobError::BadArgs(_)))
    ));
    let missing = args(
        Mode::Polyphonic,
        dir.path().join("none.wav"),
        dir.path().join("d.json"),
        fixture(),
    );
    assert!(matches!(
        run(&mut c, &missing),
        Outcome::Failed(Failure::Job(JobError::Input(_)))
    ));
    // Silence is no notes, not an error.
    let r = done(run(
        &mut c,
        &args(
            Mode::Polyphonic,
            ok.clone(),
            dir.path().join("e.json"),
            fixture(),
        ),
    ));
    assert_eq!(r.notes, 0);
    let mut plain = client(false);
    let o = run(
        &mut plain,
        &args(Mode::Polyphonic, ok, dir.path().join("f.json"), fixture()),
    );
    assert!(
        matches!(o, Outcome::Failed(Failure::Job(JobError::BadArgs(_)))),
        "{o:?}"
    );
}

#[test]
fn a_long_drum_take_keeps_the_helper_alive() {
    // Three minutes of kicks: the detector's factorisation runs for seconds on it, longer
    // than this client waits for a silent helper. Each of its steps says it is alive, so
    // the job is not taken for hung (an hour's take on a laptop runs for minutes).
    let dir = tempfile::tempdir().unwrap();
    let rate = 44_100u32;
    let seconds = 180usize;
    let mut x = vec![0.0f32; seconds * rate as usize];
    for k in 0..seconds * 2 {
        let at = (0.25 + 0.5 * k as f64) * rate as f64;
        for i in 0..(0.3 * rate as f64) as usize {
            let t = i as f64 / rate as f64;
            let ph = std::f64::consts::TAU * (50.0 * t + 100.0 * 0.03 * (1.0 - (-t / 0.03).exp()));
            if let Some(v) = x.get_mut(at as usize + i) {
                *v += (0.8 * ph.sin() * (-t / 0.15).exp()) as f32;
            }
        }
    }
    let input = dir.path().join("long.wav");
    write(&input, rate, &x);
    let mut a = args(Mode::Drums, input, dir.path().join("hits.json"), fixture());
    a.model = None;
    let mut cfg = ClientConfig::new(env!("CARGO_BIN_EXE_cantus-ml"));
    cfg.silence = Duration::from_millis(1_500);
    let mut c = MlClient::new(cfg);
    let t0 = std::time::Instant::now();
    let id = c.start("transcribe", serde_json::to_value(&a).unwrap());
    let o = c.wait(id, Duration::from_secs(300)).expect("ended");
    let took = t0.elapsed();
    let r = done(o);
    assert!(r.hits >= seconds * 2 * 9 / 10, "{} hits", r.hits);
    eprintln!("three minutes of drums in {took:?}");
}
