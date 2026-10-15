//! Tonus's demo project (round 28): a sung line through three Tonus settings and a track
//! for the owner's own voice — what to open to judge Tonus by ear.
//!
//! [`write_demo`] writes, into a folder, `tonus-demo.cantus` and the line it plays,
//! `voice-line.wav` (the pitch base's synthetic voice: A minor, sung a little out of tune,
//! with vibrato, a slide, a breath and an "s"). In the project, at 90 BPM:
//!
//! 1. **Voice · natural** — the line through Tonus in A minor at 50 ms, humanised.
//! 2. **Voice · hard tune** (muted: solo it to compare) — the same line at 0 ms.
//! 3. **Voice · MIDI melody** (muted) — the same line sung to the melody of track 4.
//! 4. **Melody (MIDI)** — the notes Tonus follows in track 3 (muted: notes only).
//! 5. **Your voice** — an audio track with input 1 and Tonus (natural) on its strip: arm
//!    it, monitor, sing, record; or drop a recorded vocal on it.

use cantus_core::{
    AudioSource, AudioSourceId, EffectKind, Project, Ticks, TrackId, TrackInput,
};
use std::path::{Path, PathBuf};

const SR: u32 = 48_000;
const BPM: f64 = 90.0;

/// The demo's sung line: eight notes of A minor, each a little off, with what a real take
/// has between them. Mono, 48 kHz.
pub fn voice_line() -> Vec<f32> {
    use cantus_dsp::pitch::testkit::{self, Vowel};
    let sr = SR as f32;
    let off = |hz: f32, c: f32| hz * cantus_dsp::math::exp2(c / 1200.0);
    // (Hz, cents off, seconds, vibrato cents)
    let notes: [(f32, f32, f32, f32); 8] = [
        (220.00, 28.0, 0.62, 0.0),
        (261.63, -32.0, 0.62, 25.0),
        (329.63, 18.0, 0.62, 0.0),
        (293.66, -22.0, 0.62, 20.0),
        (261.63, 35.0, 0.62, 0.0),
        (246.94, -25.0, 0.62, 30.0),
        (220.00, 15.0, 0.95, 35.0),
        (196.00, -30.0, 1.0, 40.0),
    ];
    let vowels = [Vowel::A, Vowel::I, Vowel::A, Vowel::U, Vowel::A, Vowel::I, Vowel::A, Vowel::U];
    let mut x = vec![0.0f32; (0.33 * sr) as usize];
    for (i, ((hz, c, secs, vib), v)) in notes.into_iter().zip(vowels).enumerate() {
        let f = off(hz, c);
        let line = if vib > 0.0 {
            testkit::voice(sr, secs, v, 0.35, testkit::vibrato(f, vib, 5.3))
        } else if i == 2 {
            // A slide up into the note, as singers do.
            testkit::voice(sr, secs, v, 0.35, testkit::glide(f * 0.94, f, 0.12))
        } else {
            testkit::voice(sr, secs, v, 0.35, move |_| f)
        };
        x.extend(line);
        match i {
            3 => x.extend(testkit::breath(sr, 0.18, 0.03, 11)),
            5 => x.extend(testkit::sibilant(sr, 0.1, 0.12, 13)),
            _ => x.extend(std::iter::repeat_n(0.0, (0.06 * sr) as usize)),
        }
    }
    x.extend(std::iter::repeat_n(0.0, (0.5 * sr) as usize));
    x
}

/// The demo project over the line in `wav` (`frames` long).
pub fn project(wav: &Path, frames: u64) -> Project {
    let mut p = Project::new("Tonus demo");
    p.tempo_bpm = BPM;
    p.metronome = false;
    let src = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id: src,
        name: "voice-line".into(),
        path: wav.to_string_lossy().into_owned(),
        sample_rate: SR,
        channels: 1,
        frames,
        sha256: None,
        recorded: false,
    });
    let beats = (frames as f64 / SR as f64 * BPM / 60.0).ceil() as i64;
    // The melody track first: the MIDI voice needs its id.
    let mut melody = p.make_track("Melody (MIDI)");
    melody.muted = true;
    let mut clip = p.make_clip("melody", Ticks::ZERO, Ticks::beats(beats));
    // The line's notes, a third higher where the singer went A C E D C B A G.
    let line: [(f64, f64, u8); 8] = [
        (0.5, 1.0, 60),
        (1.5, 1.0, 64),
        (2.5, 1.0, 67),
        (3.5, 1.0, 65),
        (4.5, 1.0, 64),
        (5.5, 1.0, 62),
        (6.5, 1.5, 60),
        (8.0, 1.5, 59),
    ];
    for (at, len, pitch) in line {
        let n = p.make_note(
            Ticks((at * Ticks::QUARTER.0 as f64) as i64),
            Ticks((len * Ticks::QUARTER.0 as f64) as i64 - 20),
            pitch,
            100,
        );
        clip.notes.push(n);
    }
    melody.clips.push(clip);
    let melody_id: TrackId = melody.id;

    let tonus = |p: &mut Project, values: &[(&str, f32)], midi: Option<TrackId>| {
        let mut fx = p.make_effect(EffectKind::AutoTune);
        for (k, v) in values {
            fx.spec.set(k, *v);
        }
        fx.midi_from = midi;
        fx
    };
    let natural: &[(&str, f32)] = &[("key", 9.0), ("scale", 2.0), ("speed", 50.0), ("human", 0.3)];
    let voices: [(&str, &[(&str, f32)], Option<TrackId>, bool); 3] = [
        ("Voice \u{b7} natural", natural, None, false),
        (
            "Voice \u{b7} hard tune",
            &[("key", 9.0), ("scale", 2.0), ("speed", 0.0)],
            None,
            true,
        ),
        (
            "Voice \u{b7} MIDI melody",
            &[("src", 2.0), ("speed", 10.0)],
            Some(melody_id),
            true,
        ),
    ];
    for (name, values, midi, muted) in voices {
        let mut t = p.make_track(name);
        t.muted = muted;
        let c = p.make_audio_clip("voice line", src, Ticks::ZERO, Ticks::beats(beats));
        t.clips.push(c);
        let strip = t.strip;
        p.tracks.push(t);
        let fx = tonus(&mut p, values, midi);
        if let Some(s) = strip.and_then(|s| p.strip_mut(s)) {
            s.effects.push(fx);
        }
    }
    p.tracks.push(melody);
    let mut yours = p.make_track("Your voice");
    yours.input = Some(TrackInput::mono(0));
    let strip = yours.strip;
    p.tracks.push(yours);
    let fx = tonus(&mut p, natural, None);
    if let Some(s) = strip.and_then(|s| p.strip_mut(s)) {
        s.effects.push(fx);
    }
    p
}

/// Write the line and the project into `dir`; returns the project's path.
pub fn write_demo(dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let wav = dir.join("voice-line.wav");
    let x = voice_line();
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
    let p = project(&wav, x.len() as u64);
    let path = dir.join("tonus-demo.cantus");
    cantus_project::save(&p, &path, env!("CARGO_PKG_VERSION")).map_err(|e| format!("{e:?}"))?;
    Ok(path)
}
