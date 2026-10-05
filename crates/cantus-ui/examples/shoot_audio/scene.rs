//! `shoot <dir> audio`: the arrangement with audio clips — a drum loop with fades, a vocal
//! with clip gain and curved fades, a stereo pad tall enough to show both channels, a
//! reversed bass, and a guitar whose file is missing — and the inspector on the vocal.
//! The audio is synthesised here into a temporary folder; nothing is read from the
//! machine.

use cantus_core::{AudioRegion, AudioSource, AudioSourceId, Color, FadeCurve, Project, Ticks};
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::{views, CantusApp};
use std::f32::consts::TAU;
use std::path::Path;
use std::time::Duration;

const RATE: u32 = 48_000;

fn noise(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed as f32 / u32::MAX as f32) * 2.0 - 1.0
}

fn write(path: &Path, channels: u16, frames: usize, f: impl Fn(usize, usize) -> f32) {
    let spec = hound::WavSpec {
        channels,
        sample_rate: RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).expect("wav");
    for i in 0..frames {
        for c in 0..channels as usize {
            let v = f(i, c).clamp(-1.0, 1.0);
            w.write_sample((v * 32767.0) as i16).expect("sample");
        }
    }
    w.finalize().expect("finalize");
}

/// Four bars of drums at 120 BPM: a kick on every beat, a snare on two and four, hats.
fn drums(path: &Path) {
    let beat = RATE as usize / 2;
    let frames = beat * 16;
    let hats: Vec<f32> = {
        let mut seed = 9u32;
        (0..frames).map(|_| noise(&mut seed)).collect()
    };
    write(path, 1, frames, |i, _| {
        let in_beat = i % beat;
        let t = in_beat as f32 / RATE as f32;
        let kick = (TAU * (50.0 + 90.0 * (-t * 30.0).exp()) * t).sin() * (-t * 9.0).exp() * 0.9;
        let n = i / beat;
        let snare = if n % 2 == 1 {
            hats[i] * (-t * 22.0).exp() * 0.55
        } else {
            0.0
        };
        let eighth = i % (beat / 2);
        let th = eighth as f32 / RATE as f32;
        let hat = hats[(i * 7) % frames] * (-th * 70.0).exp() * 0.18;
        kick + snare + hat
    });
}

/// A sung phrase: syllables of a harmonic tone with vibrato, rests between them.
fn vocal(path: &Path) {
    let frames = RATE as usize * 8;
    let notes = [
        (0.0, 0.9, 220.0),
        (1.0, 1.6, 247.0),
        (2.2, 3.4, 262.0),
        (4.0, 5.2, 247.0),
        (5.6, 7.6, 220.0),
    ];
    write(path, 1, frames, |i, _| {
        let t = i as f32 / RATE as f32;
        let Some(&(a, b, f)) = notes.iter().find(|(a, b, _)| t >= *a && t < *b) else {
            return 0.0;
        };
        let u = (t - a) / (b - a);
        let env = (u * 12.0).min(1.0) * (1.0 - u).powf(0.6) * (0.75 + 0.25 * (TAU * 3.0 * t).sin());
        let vib = f * (1.0 + 0.012 * (TAU * 5.5 * t).sin());
        let ph = TAU * vib * t;
        (ph.sin() + 0.45 * (2.0 * ph).sin() + 0.2 * (3.0 * ph).sin()) * env * 0.6
    });
}

/// A stereo pad: two detuned chords that breathe against each other, left and right.
fn pad(path: &Path) {
    let frames = RATE as usize * 8;
    write(path, 2, frames, |i, c| {
        let t = i as f32 / RATE as f32;
        let swell = 0.5 - 0.5 * (TAU * t / 8.0).cos();
        let lfo = 0.6 + 0.4 * (TAU * (0.5 + c as f32 * 0.21) * t).sin();
        let chord = [130.8, 164.8, 196.0, 246.9]
            .iter()
            .map(|f| (TAU * f * (1.0 + c as f32 * 0.003) * t).sin())
            .sum::<f32>();
        chord * swell * lfo * 0.22
    });
}

/// A plucked bass line.
fn bass(path: &Path) {
    let beat = RATE as usize / 2;
    let line = [55.0, 55.0, 65.4, 73.4, 55.0, 49.0, 55.0, 82.4];
    write(path, 1, beat * 8, |i, _| {
        let n = i / beat;
        let t = (i % beat) as f32 / RATE as f32;
        let f = line[n % line.len()];
        let saw = 2.0 * ((f * t) % 1.0) - 1.0;
        saw * (-t * 5.0).exp() * 0.6
    });
}

fn source(p: &mut Project, name: &str, path: &Path, channels: u8, frames: u64) -> AudioSourceId {
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: name.into(),
        path: path.to_string_lossy().into_owned(),
        sample_rate: RATE,
        channels,
        frames,
        sha256: None,
        recorded: false,
    });
    id
}

/// The project of the shot, its files written into `dir`.
pub fn project(dir: &Path) -> Project {
    let mut p = cantus_ui::starter_project();
    p.tempo_bpm = 120.0;
    let files = [
        ("Drum loop", "drums.wav", 1u8, RATE as u64 * 8),
        ("Lead vocal", "vocal.wav", 1, RATE as u64 * 8),
        ("Pad", "pad.wav", 2, RATE as u64 * 8),
        ("Bass", "bass.wav", 1, RATE as u64 * 4),
    ];
    drums(&dir.join("drums.wav"));
    vocal(&dir.join("vocal.wav"));
    pad(&dir.join("pad.wav"));
    bass(&dir.join("bass.wav"));
    let colors = [
        Color::PALETTE[3],
        Color::PALETTE[5],
        Color::PALETTE[7],
        Color::PALETTE[1],
        Color::PALETTE[6],
    ];
    for (n, (name, file, ch, frames)) in files.iter().enumerate() {
        let id = source(&mut p, name, &dir.join(file), *ch, *frames);
        let mut t = p.make_track(*name);
        t.color = colors[n % colors.len()];
        if n == 2 {
            t.height = 2.0;
        }
        let len = Ticks::beats((*frames / (RATE as u64 / 2)) as i64);
        let mut c = p.make_audio_clip(*name, id, Ticks::beats(if n == 3 { 4 } else { 0 }), len);
        let a: &mut AudioRegion = c.audio.as_mut().expect("audio");
        match n {
            0 => {
                a.fade_in = Ticks::beats(1);
                a.fade_out = Ticks::beats(2);
                a.fade_out_curve = FadeCurve::Exponential;
            }
            1 => {
                a.gain_db = -4.5;
                a.fade_in = Ticks::new(480);
                a.fade_in_curve = FadeCurve::SCurve;
                a.fade_out = Ticks::beats(3);
                a.fade_out_curve = FadeCurve::EqualPower;
            }
            2 => {
                a.fade_in = Ticks::beats(6);
                a.fade_in_curve = FadeCurve::Logarithmic;
            }
            _ => a.reverse = true,
        }
        t.clips.push(c);
        p.tracks.push(t);
    }
    // A guitar take whose file is gone.
    let id = source(
        &mut p,
        "Guitar DI",
        &dir.join("missing").join("Guitar DI.wav"),
        1,
        RATE as u64 * 6,
    );
    let mut t = p.make_track("Guitar");
    t.color = Color::PALETTE[2];
    let mut c = p.make_audio_clip("Guitar DI", id, Ticks::beats(8), Ticks::beats(12));
    c.audio.as_mut().expect("audio").gain_db = 2.0;
    t.clips.push(c);
    p.tracks.push(t);
    p
}

/// Write the shots into `out`.
pub fn shots(out: &Path, scale: f32) {
    let tmp = std::env::temp_dir().join(format!("cantus-shoot-audio-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).expect("temp dir");
    let themes = [
        ("slate", cantus_ui::theme::slate()),
        ("paper", cantus_ui::theme::paper()),
    ];
    // A fresh context per image: the rasteriser learns the font atlas from the frames it
    // is given, so a context that already sent its atlas would draw boxes for letters.
    let context = |theme: &cantus_ui::theme::Theme| {
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        cantus_ui::theme::apply(theme, &ctx);
        ctx
    };
    for (tn, theme) in &themes {
        let mut app = CantusApp::from_parts(Some(project(&tmp)), EngineHandle::dummy(), false);
        // Ask for every source's peaks, and wait for the worker.
        for s in app.project().audio_pool.clone() {
            let _ = app.peaks.lookup(&s, app.audio_base(), None);
        }
        app.peaks.wait_idle(Duration::from_secs(30));
        let vocal = app
            .project()
            .tracks
            .iter()
            .find(|t| t.name == "Lead vocal")
            .map(|t| (t.id, t.clips[0].id));
        if let Some((t, c)) = vocal {
            app.select_clip(t, c);
        }
        app.arrangement.px_per_beat = 34.0;
        app.arrangement.track_height = 52.0;
        let path = out.join(format!("audio-arrangement-{tn}.png"));
        let ctx = context(theme);
        shoot(
            &ctx,
            egui::vec2(1500.0, 640.0),
            scale,
            4,
            Some(&path),
            |root| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(root, |ui| views::arrangement::show(&mut app, ui));
            },
        );
        println!("wrote {}", path.display());
        let path = out.join(format!("audio-inspector-{tn}.png"));
        let ctx = context(theme);
        shoot(
            &ctx,
            egui::vec2(340.0, 1000.0),
            scale,
            4,
            Some(&path),
            |root| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(root, |ui| views::inspector::show(&mut app, ui));
            },
        );
        println!("wrote {}", path.display());
    }
    let _ = std::fs::remove_dir_all(&tmp);
}
