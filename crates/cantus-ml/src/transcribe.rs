//! The `transcribe` job (ADR 0086): an audio file into notes (Basic Pitch, polyphonic or
//! one line) or into drum hits (Cantus's own detector), written to a file
//! (`transcribe_spec::Transcription`).
//!
//! The note path is Basic Pitch's `predict` step for step: the file mixed to mono at
//! 22 050 Hz, preceded by half the windows' overlap of silence, cut into windows of 43 844
//! samples a hop of 36 164 apart (the last zero-padded), each window through the network,
//! the outputs joined with half the overlap dropped at each side and trimmed to the file,
//! then notes made from them ([`notes`]), their bends, and the frames turned into seconds
//! by Basic Pitch's own alignment.

#[path = "transcribe_drums.rs"]
pub mod drums_model;
#[path = "transcribe_fixture.rs"]
pub mod fixture;
#[path = "transcribe_melody.rs"]
pub mod melody;
#[path = "transcribe_notes.rs"]
pub mod notes;
#[path = "transcribe_spec.rs"]
pub mod spec;

use crate::jobs::{self, Ctx, Stop};
use crate::proto::JobError;
use crate::runtime;
use notes::{FrameNote, NoteParams, Posteriors, CONTOURS, NOTES, SAMPLE_RATE, WINDOW};
use spec::{
    DrumClass, DrumHit, Mode, NoteEvent, NoteFamily, NoteSettings, TranscribeArgs,
    TranscribeResult, Transcription,
};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::time::Instant;

/// Basic Pitch's graph: its input and its outputs, as `basic_pitch/inference.py` names them.
const INPUT: &str = "serving_default_input_2:0";
const OUT_NOTE: &str = "StatefulPartitionedCall:1";
const OUT_ONSET: &str = "StatefulPartitionedCall:2";
const OUT_CONTOUR: &str = "StatefulPartitionedCall:0";

/// A WAV file mixed to mono (`librosa.load(mono=True)`: the channels' mean in `float32`),
/// and its rate.
pub fn read_mono(path: &Path) -> Result<(Vec<f32>, u32), JobError> {
    // A block at a time, so a long file costs its mono samples and no more.
    let mut input = crate::audio::Input::open(path)?;
    let n = input.frames as usize;
    let mut mono = Vec::with_capacity(n);
    const BLOCK: usize = 1 << 16;
    let (mut l, mut r) = (vec![0f32; BLOCK], vec![0f32; BLOCK]);
    let mut at = 0usize;
    while at < n {
        let k = BLOCK.min(n - at);
        input.read(at as u64, &mut l[..k], &mut r[..k])?;
        if input.channels == 1 {
            mono.extend_from_slice(&l[..k]);
        } else {
            mono.extend(l[..k].iter().zip(&r[..k]).map(|(a, b)| (a + b) / 2.0));
        }
        at += k;
    }
    Ok((mono, input.sample_rate))
}

pub fn job(args: serde_json::Value, ctx: &mut Ctx) -> Result<serde_json::Value, Stop> {
    let a: TranscribeArgs = jobs::args(args)?;
    let t0 = Instant::now();
    ctx.report.progress(0.0, "read");
    let (x, rate) = read_mono(&a.input)?;
    let duration = x.len() as f64 / rate.max(1) as f64;
    ctx.report.check()?;
    let mut out = Transcription {
        mode: a.mode,
        duration,
        ..Default::default()
    };
    let (inferences, network_seconds, device) = match a.mode {
        Mode::Drums => {
            ctx.report.progress(0.05, "drums");
            let s = a.drums.sensitivity;
            if !(0.0..=1.0).contains(&s) {
                return Err(JobError::BadArgs(format!("sensitivity {s} is not in 0–1")).into());
            }
            match &a.model {
                // Onsets and Frames Drums, and the detector's hi-hats.
                Some(m) if m.family == NoteFamily::OafDrums => {
                    if rate != drums_model::RATE {
                        return Err(JobError::Input(format!(
                            "{} is at {rate} Hz; the drum model wants 44 100 Hz",
                            a.input.display()
                        ))
                        .into());
                    }
                    let rt = a.runtime.as_ref().ok_or_else(|| {
                        JobError::BadArgs("the drum model needs its runtime".into())
                    })?;
                    let cancel = ctx.report.cancel_flag();
                    let report = &mut ctx.report;
                    let t = Instant::now();
                    let (mut model, dev) =
                        ctx.runtime
                            .model(rt, &m.graph, &m.check, &mut |l, msg| report.log(l, msg))?;
                    let p = drums_model::probs(&mut model, &x, &cancel, &mut |done| {
                        report.progress(0.05 + 0.8 * done, "listen")
                    })?;
                    let net = t.elapsed().as_secs_f64();
                    report.progress(0.9, "drums");
                    // Each factorisation step says it is alive: an hour's detector runs
                    // longer than the client waits for a silent helper.
                    let report = std::cell::RefCell::new(report);
                    let alive = || {
                        report.borrow_mut().progress(0.9, "drums");
                        cancel.load(Ordering::Relaxed)
                    };
                    out.hits =
                        drums_model::hybrid(&x, rate, s, &p, &alive).ok_or(Stop::Cancelled)?;
                    (p.frames.div_ceil(60 * 100) as u32, net, dev.to_string())
                }
                Some(_) => {
                    return Err(
                        JobError::BadArgs("drums take the drum model or none".into()).into(),
                    )
                }
                None => {
                    let cancel = ctx.report.cancel_flag();
                    let report = std::cell::RefCell::new(&mut ctx.report);
                    let alive = || {
                        report.borrow_mut().progress(0.05, "drums");
                        cancel.load(Ordering::Relaxed)
                    };
                    out.hits = drums_until(&x, rate, s, &alive).ok_or(Stop::Cancelled)?;
                    (0, 0.0, "dsp".to_string())
                }
            }
        }
        Mode::Polyphonic | Mode::Melody => {
            if rate != SAMPLE_RATE {
                return Err(JobError::Input(format!(
                    "{} is at {rate} Hz; the note model wants {SAMPLE_RATE} Hz",
                    a.input.display()
                ))
                .into());
            }
            check_settings(&a.notes)?;
            let (post, inferences, net, device) = infer(&a, &x, ctx)?;
            ctx.report.progress(0.95, "notes");
            out.notes = events(&post, &a.notes, a.mode);
            (inferences, net, device)
        }
    };
    ctx.report.check()?;
    let json = serde_json::to_vec(&out).map_err(|e| JobError::Output(e.to_string()))?;
    write_atomic(&a.out, &json)?;
    ctx.report.progress(1.0, "write");
    jobs::result(&TranscribeResult {
        mode: a.mode,
        file: a.out.clone(),
        notes: out.notes.len(),
        hits: out.hits.len(),
        duration,
        inferences,
        network_seconds,
        seconds: t0.elapsed().as_secs_f64(),
        device,
    })
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), JobError> {
    let err = |e: std::io::Error| JobError::Output(format!("{}: {e}", path.display()));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(err)?;
    }
    let tmp = path.with_extension("part");
    std::fs::write(&tmp, bytes).map_err(err)?;
    std::fs::rename(&tmp, path).map_err(err)
}

fn check_settings(s: &NoteSettings) -> Result<(), Stop> {
    let unit = |v: f64| (0.0..=1.0).contains(&v);
    if !unit(s.onset_threshold) || !unit(s.frame_threshold) {
        return Err(JobError::BadArgs("thresholds are 0–1".into()).into());
    }
    if !(0.0..=10_000.0).contains(&s.min_note_ms) {
        return Err(JobError::BadArgs("the shortest note is 0–10 000 ms".into()).into());
    }
    for hz in [s.min_hz, s.max_hz].into_iter().flatten() {
        if !(1.0..=30_000.0).contains(&hz) {
            return Err(JobError::BadArgs(format!("{hz} Hz is not a frequency limit")).into());
        }
    }
    Ok(())
}

/// Every window through the network, joined: the file's posteriorgrams.
fn infer(
    a: &TranscribeArgs,
    x: &[f32],
    ctx: &mut Ctx,
) -> Result<(Posteriors, u32, f64, String), Stop> {
    let model = a
        .model
        .as_ref()
        .ok_or_else(|| JobError::BadArgs("the note modes need a model".into()))?;
    let count = notes::window_count(x.len());
    // Each window's kept frames are joined as it comes, then the window is let go: a long
    // file holds its posteriorgrams once, never its windows besides.
    let keep = notes::kept_frames(x.len());
    let mut joined: [Vec<f32>; 3] = [
        Vec::with_capacity(keep * NOTES),
        Vec::with_capacity(keep * NOTES),
        Vec::with_capacity(keep * CONTOURS),
    ];
    let mut buf = vec![0f32; WINDOW];
    let cancel = ctx.report.cancel_flag();
    let mut net = 0.0f64;
    let report = &mut ctx.report;
    report.progress(0.02, "load");
    let device = match &model.family {
        NoteFamily::Fixture { delay_ms, crash_at } => {
            if !ctx.test_hooks {
                return Err(JobError::BadArgs("fixture models are for tests".into()).into());
            }
            for k in 0..count {
                if cancel.load(Ordering::Relaxed) {
                    return Err(Stop::Cancelled);
                }
                if *crash_at == Some(k) {
                    std::process::abort();
                }
                std::thread::sleep(std::time::Duration::from_millis(*delay_ms));
                notes::window(x, k, &mut buf);
                let t = Instant::now();
                let (n, o, c) = fixture::run(&buf);
                net += t.elapsed().as_secs_f64();
                notes::join(&mut joined[0], &n, NOTES, keep);
                notes::join(&mut joined[1], &o, NOTES, keep);
                notes::join(&mut joined[2], &c, CONTOURS, keep);
                report.progress(0.05 + 0.9 * (k + 1) as f32 / count as f32, "listen");
            }
            "fixture".to_string()
        }
        NoteFamily::OafDrums => {
            return Err(JobError::BadArgs("the drum model is for drums".into()).into());
        }
        NoteFamily::BasicPitch => {
            let rt = a
                .runtime
                .as_ref()
                .ok_or_else(|| JobError::BadArgs("Basic Pitch needs its runtime".into()))?;
            let (mut m, dev) =
                ctx.runtime
                    .model(rt, &model.graph, &model.check, &mut |l, msg| {
                        report.log(l, msg)
                    })?;
            for k in 0..count {
                report.check()?;
                notes::window(x, k, &mut buf);
                let t = Instant::now();
                let outs = m.run(
                    &[runtime::Input {
                        name: INPUT,
                        shape: &[1, WINDOW, 1],
                        data: &buf,
                    }],
                    &cancel,
                )?;
                net += t.elapsed().as_secs_f64();
                let take = |name: &str, bins: usize| -> Result<Vec<f32>, Stop> {
                    let o = outs.iter().find(|o| o.name == name).ok_or_else(|| {
                        JobError::Model(format!("the graph has no output {name}"))
                    })?;
                    if o.data.len() != notes::WINDOW_FRAMES * bins {
                        return Err(JobError::Model(format!(
                            "{name} has shape {:?}, not [1, 172, {bins}]",
                            o.shape
                        ))
                        .into());
                    }
                    Ok(o.data.clone())
                };
                notes::join(&mut joined[0], &take(OUT_NOTE, NOTES)?, NOTES, keep);
                notes::join(&mut joined[1], &take(OUT_ONSET, NOTES)?, NOTES, keep);
                notes::join(
                    &mut joined[2],
                    &take(OUT_CONTOUR, CONTOURS)?,
                    CONTOURS,
                    keep,
                );
                report.progress(0.05 + 0.9 * (k + 1) as f32 / count as f32, "listen");
            }
            dev.to_string()
        }
    };
    let [note, onset, contour] = joined;
    let post = Posteriors {
        frames: note.len() / NOTES,
        note,
        onset,
        contour,
    };
    Ok((post, count as u32, net, device))
}

/// The notes, seconds, as Basic Pitch's `model_output_to_notes` gives them (sorted by
/// start, then pitch); one line of them in melody mode.
pub fn events(post: &Posteriors, s: &NoteSettings, mode: Mode) -> Vec<NoteEvent> {
    let p = NoteParams {
        onset_threshold: s.onset_threshold,
        frame_threshold: s.frame_threshold,
        min_note_frames: notes::min_note_frames(s.min_note_ms),
        min_hz: s.min_hz,
        max_hz: s.max_hz,
        infer_onsets: s.infer_onsets,
        melodia_trick: s.melodia_trick,
    };
    let found: Vec<FrameNote> = notes::notes(post, &p);
    let bends = if s.bends {
        notes::pitch_bends(&post.contour, &found)
    } else {
        vec![Vec::new(); found.len()]
    };
    let mut out: Vec<NoteEvent> = found
        .iter()
        .zip(bends)
        .map(|(n, b)| NoteEvent {
            start: notes::frame_time(n.start),
            end: notes::frame_time(n.end),
            pitch: n.pitch,
            velocity: notes::velocity(n.amplitude).clamp(1, 127) as u8,
            amplitude: n.amplitude,
            bends: if b.iter().any(|v| *v != 0) {
                b
            } else {
                Vec::new()
            },
            curve: Vec::new(),
        })
        .collect();
    out.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.pitch.cmp(&b.pitch)));
    if mode == Mode::Melody {
        let shortest = p.min_note_frames as f64 * notes::FFT_HOP as f64 / SAMPLE_RATE as f64;
        out = melody::line(&out, shortest);
    }
    out
}

/// The drum hits (Cantus's detector, `cantus_dsp::onset::drums`), with their GM keys.
pub fn drums(x: &[f32], rate: u32, sensitivity: f32) -> Vec<DrumHit> {
    drums_until(x, rate, sensitivity, &|| false).unwrap_or_default()
}

/// [`drums`] that stops when `cancelled` says so (asked each factorisation iteration).
pub fn drums_until(
    x: &[f32],
    rate: u32,
    sensitivity: f32,
    cancelled: &dyn Fn() -> bool,
) -> Option<Vec<DrumHit>> {
    use cantus_dsp::onset::drums as d;
    let hits = d::transcribe_until(
        x,
        rate as f32,
        sensitivity,
        &d::Tuning::default(),
        cancelled,
    )?;
    Some(
        hits.into_iter()
            .map(|h| DrumHit {
                time: h.time,
                class: match h.class {
                    d::Class::Kick => DrumClass::Kick,
                    d::Class::Snare => DrumClass::Snare,
                    d::Class::Toms => DrumClass::Toms,
                    d::Class::HiHat => DrumClass::HiHat,
                    d::Class::Cymbals => DrumClass::Cymbals,
                },
                key: detector_key(h.kind),
                velocity: h.velocity,
            })
            .collect(),
    )
}

/// The studio kit's pad for each kind the detector hears (it has a crash, 49, and a ride,
/// 51: no bell).
pub fn detector_key(k: cantus_dsp::onset::drums::Kind) -> u8 {
    use cantus_dsp::onset::drums::Kind;
    match k {
        Kind::Kick => 36,
        Kind::Snare => 38,
        Kind::LowTom => 45,
        Kind::MidTom => 47,
        Kind::HighTom => 50,
        Kind::ClosedHat => 42,
        Kind::OpenHat => 46,
        Kind::Crash => 49,
        Kind::Ride => 51,
    }
}
