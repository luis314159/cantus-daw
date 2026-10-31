//! Audio to MIDI, measured (round 34, ADR 0086 §4): note F-measures by mir_eval's rules
//! (onset within 50 ms, pitch within 50 cents; "F" also asks the offset within 20 % or
//! 50 ms) on music rendered from known MIDI through our own instruments — melodies on each
//! and chords with a bass and a tune on the keyboards — and, when given, on a public set
//! (TRIOS: `<name>.wav` at 22 050 Hz beside `<name>.truth.json`, `{"notes": [[start, end,
//! pitch], …], "mono": bool}`). Melody mode's rule is compared there with the other rule
//! and with Cantus's own pitch tracker (the pitch editor's). Ignored: it needs the runtime
//! and Basic Pitch's graph.
//!
//! ```text
//! CANTUS_ORT_LIBRARY=…/onnxruntime.dll CANTUS_BASIC_PITCH=…/nmp.onnx \
//! CANTUS_TRANSCRIBE_SET=…/trios22   (optional) \
//! cargo test -p cantus-ui --release --test transcribe_accuracy -- --ignored --nocapture
//! ```

mod ml_support;

use cantus_core::inst::{InstrumentKind, NativeSpec};
use cantus_core::*;
use cantus_dsp::onset::eval::{self, Score};
use cantus_ml::client::{ClientConfig, MlClient, Outcome};
use cantus_ml::spec::{Device, RuntimeSpec};
use cantus_ml::transcribe::melody;
use cantus_ml::transcribe::spec::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

const BPM: f64 = 120.0;
const SR: u32 = 22_050;

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[(self.next() % v.len() as u64) as usize]
    }
}

/// `(start beat, length beats, pitch, velocity)`.
type Part = Vec<(f64, f64, u8, u8)>;

/// A melody of `n` notes on the C major scale within `lo..=hi`: a random walk of steps and
/// small leaps, lengths of a quarter to a dotted half, played legato (90 %).
fn melody_part(seed: u64, n: usize, lo: u8, hi: u8) -> Part {
    let scale = [0u8, 2, 4, 5, 7, 9, 11];
    let mut r = Lcg(seed);
    let degrees: Vec<u8> = (lo..=hi).filter(|p| scale.contains(&(p % 12))).collect();
    let mut i = degrees.len() / 2;
    let mut at = 0.5;
    let mut out = Vec::new();
    for _ in 0..n {
        let step = r.pick(&[-2i32, -1, -1, 1, 1, 2, 3, -3]);
        i = (i as i32 + step).clamp(0, degrees.len() as i32 - 1) as usize;
        let len = r.pick(&[0.5, 0.5, 1.0, 1.0, 1.5, 2.0]);
        out.push((at, len * 0.9, degrees[i], r.pick(&[70u8, 85, 100, 110])));
        at += len;
    }
    out
}

/// Chords for `bars`: a triad or a seventh each half bar in the middle register, a bass
/// note under each, a tune of quarters and eighths over them.
fn chords_part(seed: u64, bars: usize) -> Part {
    let mut r = Lcg(seed);
    let roots = [48u8, 53, 55, 57, 50, 52];
    let mut out = Vec::new();
    for half in 0..bars * 2 {
        let at = half as f64 * 2.0;
        let root = r.pick(&roots);
        let shape: &[u8] = r.pick(&[&[0u8, 4, 7][..], &[0, 3, 7], &[0, 4, 7, 11], &[0, 3, 7, 10]]);
        for s in shape {
            out.push((at, 1.9, root + 12 + s, 80));
        }
        out.push((at, 1.9, root - 12, 95));
        let mut t = at;
        while t < at + 2.0 - 1e-9 {
            let len = r.pick(&[0.5, 1.0]);
            let p = root + 24 + r.pick(&[0u8, 2, 4, 7, 9, 12]);
            out.push((t, len * 0.9, p, 105));
            t += len;
        }
    }
    out
}

/// `part` played by `kind`, rendered mono at 22 050 Hz, and its truth in seconds.
fn render_part(kind: InstrumentKind, part: &Part) -> (Vec<f32>, Vec<eval::Note>) {
    let mut p = Project {
        tempo_bpm: BPM,
        ..Project::default()
    };
    let mut t = p.make_track("Part");
    t.instrument = InstrumentSpec::Native(NativeSpec::new(kind));
    let end = part.iter().map(|n| n.0 + n.1).fold(0.0, f64::max);
    let mut c = p.make_clip("Part", Ticks::ZERO, Ticks::beats(end.ceil() as i64 + 1));
    let q = Ticks::QUARTER.0 as f64;
    for &(at, len, pitch, vel) in part {
        let n = p.make_note(
            Ticks((at * q).round() as i64),
            Ticks((len * q).round() as i64),
            pitch,
            vel,
        );
        c.notes.push(n);
    }
    t.clips.push(c);
    p.tracks.push(t);
    let r = cantus_engine::render(
        Arc::new(Snapshot::compile(&p, 1)),
        &cantus_engine::RenderOptions {
            sample_rate: SR,
            channels: 2,
            tail_seconds: 1.0,
            compensate_latency: true,
            ..Default::default()
        },
    )
    .expect("render");
    let x: Vec<f32> = r.samples.chunks(2).map(|c| 0.5 * (c[0] + c[1])).collect();
    let spb = 60.0 / BPM;
    let truth = part
        .iter()
        .map(|&(at, len, pitch, _)| eval::Note {
            onset: at * spb,
            offset: (at + len) * spb,
            pitch: pitch as f64,
        })
        .collect();
    (x, truth)
}

fn write(path: &Path, x: &[f32]) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SR,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for v in x {
        w.write_sample(*v).unwrap();
    }
    w.finalize().unwrap();
}

fn notes_of(n: &[cantus_ml::transcribe::spec::NoteEvent]) -> Vec<eval::Note> {
    n.iter()
        .map(|n| eval::Note {
            onset: n.start,
            offset: n.end,
            pitch: n.pitch as f64,
        })
        .collect()
}

/// Cantus's own tracker (the pitch editor's analysis) on a WAV: its notes, seconds.
fn tracker(x: &[f32]) -> Vec<eval::Note> {
    let a = cantus_engine::pitch::analyse_take(&[x], SR);
    cantus_core::pitch::midi::notes(&a.edit())
        .iter()
        .map(|n| eval::Note {
            onset: n.start as f64 / SR as f64,
            offset: n.end as f64 / SR as f64,
            pitch: n.pitch as f64,
        })
        .collect()
}

struct Row {
    name: String,
    mono: bool,
    poly: (Score, Score),
    line: Option<(Score, Score, Score, Score)>,
}

#[test]
#[ignore = "needs the runtime and Basic Pitch's graph (see the file's header)"]
fn note_f_measures_on_our_instruments_and_a_public_set() {
    let lib = PathBuf::from(std::env::var_os("CANTUS_ORT_LIBRARY").expect("CANTUS_ORT_LIBRARY"));
    let graph = PathBuf::from(std::env::var_os("CANTUS_BASIC_PITCH").expect("CANTUS_BASIC_PITCH"));
    let dir = tempfile::tempdir().unwrap();
    // The material: (name, wav, truth, monophonic).
    let mut set: Vec<(String, PathBuf, Vec<eval::Note>, bool)> = Vec::new();
    let mono_parts = [
        (InstrumentKind::Wavetable, "unda", 60u8, 84u8),
        (InstrumentKind::Analog, "calor", 55, 79),
        (InstrumentKind::Fm, "modulus", 60, 84),
        (InstrumentKind::Bass, "gravis", 28, 52),
        (InstrumentKind::ElectricPiano, "lamina", 52, 79),
        (InstrumentKind::Organ, "organum", 52, 79),
        (InstrumentKind::Chorda, "chorda", 48, 84),
    ];
    for (i, (kind, name, lo, hi)) in mono_parts.iter().enumerate() {
        let (x, truth) = render_part(*kind, &melody_part(100 + i as u64, 48, *lo, *hi));
        let path = dir.path().join(format!("{name}_melody.wav"));
        write(&path, &x);
        set.push((format!("{name} melody"), path, truth, true));
    }
    for (i, kind) in [
        InstrumentKind::Chorda,
        InstrumentKind::ElectricPiano,
        InstrumentKind::Organ,
        InstrumentKind::Analog,
        InstrumentKind::Wavetable,
    ]
    .iter()
    .enumerate()
    {
        let (x, truth) = render_part(*kind, &chords_part(200 + i as u64, 12));
        let name = format!("{kind:?} chords").to_lowercase();
        let path = dir.path().join(format!("{}.wav", name.replace(' ', "_")));
        write(&path, &x);
        set.push((name, path, truth, false));
    }
    if let Some(public) = std::env::var_os("CANTUS_TRANSCRIBE_SET").map(PathBuf::from) {
        let mut wavs: Vec<PathBuf> = std::fs::read_dir(&public)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "wav"))
            .filter(|p| p.with_extension("truth.json").is_file())
            .collect();
        wavs.sort();
        for w in wavs {
            let t: serde_json::Value =
                serde_json::from_slice(&std::fs::read(w.with_extension("truth.json")).unwrap())
                    .unwrap();
            let truth = t["notes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|n| eval::Note {
                    onset: n[0].as_f64().unwrap(),
                    offset: n[1].as_f64().unwrap(),
                    pitch: n[2].as_f64().unwrap(),
                })
                .collect();
            let name = w.file_stem().unwrap().to_string_lossy().to_string();
            set.push((name, w, truth, t["mono"].as_bool().unwrap_or(false)));
        }
    }
    let mut c = MlClient::new(ClientConfig::new(ml_support::helper_exe()));
    let mut rows = Vec::new();
    for (name, wav, truth, mono) in &set {
        let args = TranscribeArgs {
            mode: Mode::Polyphonic,
            input: wav.clone(),
            out: dir.path().join("out.json"),
            runtime: Some(RuntimeSpec {
                library: lib.clone(),
                threads: 0,
                device: Device::Cpu,
                sha256: None,
            }),
            model: Some(NoteModel {
                graph: graph.clone(),
                family: NoteFamily::BasicPitch,
                check: Vec::new(),
            }),
            notes: NoteSettings::default(),
            drums: DrumSettings::default(),
        };
        let id = c.start("transcribe", serde_json::to_value(&args).unwrap());
        let r: TranscribeResult = match c.wait(id, Duration::from_secs(900)) {
            Some(Outcome::Done(v)) => serde_json::from_value(v).unwrap(),
            other => panic!("{name}: {other:?}"),
        };
        let t: Transcription = serde_json::from_slice(&std::fs::read(&r.file).unwrap()).unwrap();
        let found = notes_of(&t.notes);
        let both = |f: &[eval::Note]| {
            (
                eval::notes(truth, f, None),
                eval::notes(truth, f, Some(0.2)),
            )
        };
        let poly = both(&found);
        let line = mono.then(|| {
            let shortest = 0.1277;
            let l = both(&notes_of(&melody::line(&t.notes, shortest)));
            let h = both(&notes_of(&melody::highest(&t.notes, shortest)));
            let x = cantus_ml::transcribe::read_mono(wav).unwrap().0;
            let own = both(&tracker(&x));
            (l.0, h.0, own.0, own.1)
        });
        let row = Row {
            name: name.clone(),
            mono: *mono,
            poly,
            line,
        };
        print_row(&row);
        rows.push(row);
    }
    let sum = |f: &dyn Fn(&Row) -> Option<Score>| {
        Score::sum(&rows.iter().filter_map(f).collect::<Vec<_>>())
    };
    println!("--");
    for (label, ours) in [("rendered", true), ("public", false)] {
        let pick = |r: &Row| r.name.contains(' ') == ours;
        let p = sum(&|r| pick(r).then_some(r.poly.0));
        let po = sum(&|r| pick(r).then_some(r.poly.1));
        let l = sum(&|r| (pick(r) && r.mono).then(|| r.line.unwrap().0));
        let h = sum(&|r| (pick(r) && r.mono).then(|| r.line.unwrap().1));
        let o = sum(&|r| (pick(r) && r.mono).then(|| r.line.unwrap().2));
        let pm = sum(&|r| (pick(r) && r.mono).then_some(r.poly.0));
        println!(
            "{label}: polyphonic F(no offset) {:.3} F {:.3} over {} notes · on the melodies: polyphonic {:.3}, line {:.3}, highest {:.3}, own tracker {:.3}",
            p.f, po.f, p.truth, pm.f, l.f, h.f, o.f
        );
    }
}

fn print_row(r: &Row) {
    let mut s = format!(
        "{:28} Fno {:.3} (P {:.3} R {:.3}) F {:.3}",
        r.name, r.poly.0.f, r.poly.0.precision, r.poly.0.recall, r.poly.1.f
    );
    if let Some((l, h, o, _)) = r.line {
        s += &format!(" · line {:.3} highest {:.3} own {:.3}", l.f, h.f, o.f);
    }
    let _ = r.mono;
    println!("{s}");
}
