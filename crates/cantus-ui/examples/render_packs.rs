//! Cantus's own sound packs (round 37, ADR 0101 §7), rendered by the program from its own
//! instruments and their factory presets — ours, given away as CC0:
//!
//! - `cantus-one-shots/`: one note of every library instrument preset, by role
//!   (`Bass/`, `Lead/`, `Keys/`, `Pad/`, `Pluck/`, `FX/`), 2.5 s, 44.1 kHz, 24-bit stereo;
//! - `cantus-loops/`: two-bar loops at 120 BPM — each library instrument preset marked for
//!   it on its role's phrase, and every factory kit on its groove — cut so they loop without
//!   a seam (the middle of three passes: the tail of the one before is in it).
//!
//! Every file is peak-normalised to −1 dBFS, as a sample pack's are (the presets' own levels
//! are a mixing level, quiet for a file dropped on a pad); one gain per file, so a loop
//! still loops.
//!
//! `cargo run --release -p cantus-ui --example render_packs -- <out_dir>`; then
//! `cargo run -p cantus-content --example pack_folder -- <out_dir>/<pack>` zips each pack and
//! prints its manifest entry (size and SHA-256). Nothing it writes belongs in the repository.

use cantus_core::{Project, Snapshot, Ticks};
use cantus_engine::{render, RenderOptions};
use cantus_ui::demos::song::{colors, Feel, Song};
use cantus_ui::demos::theory::Ev;
use cantus_ui::preset_audition::{self as aud};
use cantus_ui::preset_catalog::{entries, Entry, Payload, Unit};
use cantus_ui::preset_tags::Role;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const RATE: u32 = 44_100;

/// The styles whose presets get a loop.
const LOOP_STYLES: [&str; 7] = ["trap", "house", "lofi", "pop", "latin", "rnb", "ambient"];

fn role_folder(r: Role) -> &'static str {
    match r {
        Role::Bass => "Bass",
        Role::Lead => "Lead",
        Role::Pad => "Pad",
        Role::Pluck => "Pluck",
        Role::Fx => "FX",
        Role::Drums => "Drums",
        _ => "Keys",
    }
}

/// A file name from a preset's English name: letters, digits, spaces and dashes.
fn safe(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn unit_name(u: Unit) -> String {
    cantus_ui::i18n::t_in(cantus_ui::i18n::Locale::En, u.name_key())
}

/// The peak every file is brought to, dBFS.
const PEAK_DB: f32 = -1.0;

fn write_wav(samples: &[f32], path: &Path) {
    let peak = samples.iter().fold(0f32, |m, x| m.max(x.abs()));
    let gain = if peak > 1e-6 {
        10f32.powf(PEAK_DB / 20.0) / peak
    } else {
        1.0
    };
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: RATE,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for s in samples {
        let v = ((s * gain).clamp(-1.0, 1.0) * 8_388_607.0).round() as i32;
        w.write_sample(v).unwrap();
    }
    w.finalize().unwrap();
}

fn render_range(p: &Project, start: Ticks, end: Ticks, tail: f64) -> Vec<f32> {
    let snap = Arc::new(Snapshot::compile(p, 0));
    render(
        snap,
        &RenderOptions {
            sample_rate: RATE,
            start,
            end: Some(end),
            tail_seconds: tail,
            compensate_latency: true,
            ..Default::default()
        },
    )
    .expect("it renders")
    .samples
}

/// One note (a chord for a pad) of `e`, 1.5 s held and 1 s of release.
fn one_shot(e: &Entry) -> Vec<f32> {
    let spec = match &e.payload {
        Payload::Native(s) => cantus_core::InstrumentSpec::Native(s.clone()),
        _ => unreachable!(),
    };
    let evs: Vec<Ev> = match e.role() {
        Role::Bass => vec![Ev::new(0.0, 3.0, 36, 100)],
        Role::Pad => [48u8, 55, 60, 63]
            .iter()
            .map(|&n| Ev::new(0.0, 3.0, n, 96))
            .collect(),
        Role::Keys => [48u8, 55, 60, 63]
            .iter()
            .map(|&n| Ev::new(0.0, 3.0, n, 100))
            .collect(),
        _ => vec![Ev::new(0.0, 3.0, 60, 100)],
    };
    let mut s = Song::new("one-shot", 120.0, 1);
    let part = s.part("shot", spec, colors::KEYS, cantus_core::TrackIcon::Keys);
    s.feel(part, Feel::TIGHT);
    s.clip(part, "shot", 0.0, 1.0, &evs);
    let p = s.finish();
    let mut out = render_range(&p, Ticks::ZERO, Ticks::from_beats_f64(3.0), 1.0);
    out.truncate((RATE as f64 * 2.5) as usize * 2);
    fade_out(&mut out, 0.05);
    out
}

fn fade_out(s: &mut [f32], secs: f64) {
    let n = (RATE as f64 * secs) as usize;
    let frames = s.len() / 2;
    for i in 0..n.min(frames) {
        let g = i as f32 / n as f32;
        let f = frames - 1 - i;
        s[f * 2] *= g;
        s[f * 2 + 1] *= g;
    }
}

/// Two bars of `p`'s phrase cut from the middle of three passes: it loops seamlessly.
fn seamless(make: impl Fn(f64) -> Project) -> Vec<f32> {
    let p = make(3.0);
    let bar = Ticks::from_beats_f64(aud::BARS * 4.0);
    let all = render_range(&p, Ticks::ZERO, bar + bar + bar, 0.0);
    let per = (RATE as f64 * aud::BARS * 4.0 * 60.0 / aud::TEMPO).round() as usize * 2;
    all[per..(2 * per).min(all.len())].to_vec()
}

/// A project playing `spec` on `evs` repeated `passes` times (two bars each).
fn repeated(spec: cantus_core::InstrumentSpec, evs: &[Ev], passes: f64) -> Project {
    let mut s = Song::new("loop", aud::TEMPO, 1);
    let part = s.part("loop", spec, colors::KEYS, cantus_core::TrackIcon::Keys);
    s.feel(part, Feel::TIGHT);
    for i in 0..passes as usize {
        s.clip(part, "loop", i as f64 * aud::BARS, aud::BARS, evs);
    }
    s.finish()
}

fn main() {
    let out = PathBuf::from(std::env::args().nth(1).expect("an output folder"));
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let library = [
        cantus_core::InstrumentKind::Wavetable,
        cantus_core::InstrumentKind::Analog,
        cantus_core::InstrumentKind::Fm,
        cantus_core::InstrumentKind::Sampler,
        cantus_core::InstrumentKind::Bass,
        cantus_core::InstrumentKind::ElectricPiano,
        cantus_core::InstrumentKind::Organ,
    ];
    let shots = out.join("cantus-one-shots");
    let loops = out.join("cantus-loops");
    let (mut n_shots, mut n_loops) = (0, 0);
    for kind in library {
        let unit = Unit::Instrument(kind);
        let mut loops_here = 0;
        for e in entries(unit, false) {
            let name = format!("{} - {}", unit_name(unit), safe(&e.name));
            let path = shots
                .join(role_folder(e.role()))
                .join(format!("{name}.wav"));
            write_wav(&one_shot(&e), &path);
            n_shots += 1;
            // Loops for the presets made for the styles producers loop most, ten per
            // instrument (the pack stays a reasonable download).
            let loopable = e.styles.iter().any(|s| LOOP_STYLES.contains(&s.id()));
            if !loopable || loops_here >= 10 {
                continue;
            }
            loops_here += 1;
            let Payload::Native(spec) = &e.payload else {
                continue;
            };
            let spec = cantus_core::InstrumentSpec::Native(spec.clone());
            let evs = aud::phrase(e.role());
            let l = seamless(|n| repeated(spec.clone(), &evs, n));
            let path = loops
                .join(role_folder(e.role()))
                .join(format!("{name} - 120 BPM.wav"));
            write_wav(&l, &path);
            n_loops += 1;
        }
    }
    for e in entries(Unit::Kit, false) {
        let Payload::Kit(kit) = &e.payload else {
            continue;
        };
        let evs = aud::kit_phrase(kit);
        let spec = cantus_core::InstrumentSpec::Pulsus(kit.clone());
        let l = seamless(|n| repeated(spec.clone(), &evs, n));
        let path = loops
            .join("Drums")
            .join(format!("Pulsus - {} - 120 BPM.wav", safe(&e.name)));
        write_wav(&l, &path);
        n_loops += 1;
    }
    for dir in [&shots, &loops] {
        std::fs::write(dir.join("LICENSE.txt"), LICENCE).unwrap();
    }
    println!("{n_shots} one-shots in {}", shots.display());
    println!("{n_loops} loops in {}", loops.display());
}

const LICENCE: &str = "Cantus sound packs\n\
\n\
Every file here was rendered by Cantus (https://cantusdaw.com) from its own instruments\n\
and factory presets, with no recorded material from anyone else.\n\
\n\
Luis Angel Almazan, the author of Cantus, dedicates these sounds to the public domain under\n\
the Creative Commons CC0 1.0 Universal Public Domain Dedication:\n\
https://creativecommons.org/publicdomain/zero/1.0/legalcode\n\
\n\
You may copy, modify, distribute and use them, commercially or not, without asking and\n\
without credit. A credit to Cantus is welcome, never required.\n";
