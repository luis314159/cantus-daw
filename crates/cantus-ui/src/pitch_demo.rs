//! The pitch editor's demo project (round 28, ADR 0054): a sung take corrected in the
//! editor, beside the take as sung — what to open to judge the editor by ear.
//!
//! [`write_demo`] writes, into a folder, `pitch-demo.cantus` and the take it edits,
//! `voice-take.wav` (the pitch base's synthetic voice — Tonus's demo line: eight notes of
//! A minor sung a little out of tune, late more and more, with vibrato, a slide, a breath
//! and an "s"). In the project, at 90 BPM:
//!
//! 1. **Voice · corrected** — the take with its pitch edits: every note pulled onto its
//!    note (*Correct pitch* 100 %), the drift halved, the notes quantised to the eighth.
//!    Double-click the clip: the pitch editor opens on it, *Original* compares.
//! 2. **Voice · as sung** (muted: solo it to compare) — the same take, untouched.
//! 3. **Voice · a third above** (muted) — *Double*: the corrected take a diatonic third
//!    higher, from the same file.
//! 4. **Melody (MIDI)** (muted) — *MIDI out*: the corrected notes on an instrument.
//! 5. **Your voice** — an audio track on input 1: arm it, record, double-click the take.

use crate::pitch::ClipTime;
use cantus_core::pitch::{edit, midi, PitchEdit};
use cantus_core::{AudioSource, AudioSourceId, Project, Ticks, TrackInput};
use std::path::{Path, PathBuf};

const SR: u32 = 48_000;
const BPM: f64 = 90.0;

/// The corrected edit of the take: correct pitch 100 %, drift halved, quantised to the
/// eighth note (the line is sung on the off-beats, late more and more; the clip at bar 1,
/// no slip).
pub fn corrected(take: &cantus_engine::pitch::TakeAnalysis) -> PitchEdit {
    let mut e = take.edit();
    let ids: Vec<u32> = e.notes.iter().map(|n| n.id).collect();
    edit::correct_pitch(&mut e, &ids, 1.0);
    edit::correct_drift(&mut e, &ids, 0.5);
    let time = time();
    let beat = Ticks::QUARTER.0 / 2;
    edit::quantise(&mut e, &ids, |f| {
        let t = time.tick(f as f64).0;
        let q = (t as f64 / beat as f64).round() as i64 * beat;
        time.frame(Ticks(q)).round().max(0.0) as u64
    });
    e
}

fn time() -> ClipTime {
    ClipTime {
        start: Ticks::ZERO,
        offset: Ticks::ZERO,
        slip: 0,
        rate: SR,
        bpm: BPM,
        reverse: false,
        end: Ticks::beats(1 << 20),
    }
}

/// The demo project over the take in `wav`.
pub fn project(wav: &Path, x: &[f32]) -> Project {
    let take = cantus_engine::pitch::analyse_take(&[x], SR);
    let fixed = corrected(&take);
    let mut p = Project::new("Pitch editor demo");
    p.tempo_bpm = BPM;
    p.metronome = false;
    let src = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id: src,
        name: "voice-take".into(),
        path: wav.to_string_lossy().into_owned(),
        sample_rate: SR,
        channels: 1,
        frames: x.len() as u64,
        sha256: None,
        recorded: true,
    });
    let beats = (x.len() as f64 / SR as f64 * BPM / 60.0).ceil() as i64;
    let voices: [(&str, Option<PitchEdit>, bool); 3] = [
        ("Voice \u{b7} corrected", Some(fixed.clone()), false),
        ("Voice \u{b7} as sung", None, true),
        (
            "Voice \u{b7} a third above",
            Some(midi::double(&fixed, midi::Interval::Third)),
            true,
        ),
    ];
    for (name, e, muted) in voices {
        let mut t = p.make_track(name);
        t.muted = muted;
        let mut c = p.make_audio_clip("take", src, Ticks::ZERO, Ticks::beats(beats));
        if let Some(a) = c.audio.as_mut() {
            a.pitch = e.map(Box::new);
        }
        t.clips.push(c);
        p.tracks.push(t);
    }
    // MIDI out: the corrected notes on the default instrument.
    let mut melody = p.make_track("Melody (MIDI)");
    melody.muted = true;
    let mut clip = p.make_clip("melody", Ticks::ZERO, Ticks::beats(beats));
    let tm = time();
    for n in midi::notes(&fixed) {
        let a = tm.tick(n.start as f64).max(Ticks::ZERO);
        let b = tm.tick(n.end as f64).max(a + Ticks(1));
        clip.notes.push(p.make_note(a, b - a, n.pitch, n.velocity));
    }
    melody.clips.push(clip);
    p.tracks.push(melody);
    let mut yours = p.make_track("Your voice");
    yours.input = Some(TrackInput::mono(0));
    p.tracks.push(yours);
    p
}

/// Write the take and the project into `dir`; returns the project's path.
pub fn write_demo(dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let wav = dir.join("voice-take.wav");
    let x = crate::views::fx::tonus::demo::voice_line();
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SR,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(&wav, spec).map_err(|e| e.to_string())?;
    for s in &x {
        w.write_sample(*s).map_err(|e| e.to_string())?;
    }
    w.finalize().map_err(|e| e.to_string())?;
    let p = project(&wav, &x);
    let path = dir.join("pitch-demo.cantus");
    cantus_project::save(&p, &path, env!("CARGO_PKG_VERSION")).map_err(|e| format!("{e:?}"))?;
    Ok(path)
}
