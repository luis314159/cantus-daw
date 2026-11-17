//! Lapsus on a real guitar (fx side round, ADR 0129): the owner's reference
//! progression — Am, F, C, G, one chord a bar at 90 BPM, struck on beats 1 and 3, eight
//! bars — built from DI strums read with Cantus's own decoder and resampled to 48 kHz with
//! its resampler, then played through Lapsus's engine unit on the song's clock and written
//! as FLAC beside the dry guitar (`00-guitar-di.flac`, peak −6 dBFS), each processed file
//! level-matched to it within 1 dB (RMS; its peak kept under −1 dBFS). The samples are not
//! in the repository: pass their folder (Carbonthief's FLACs are 32-bit, which Cantus's
//! FLAC decoder does not read yet: pass a folder of WAVs made from them).
//!
//! ```text
//! cargo run -p cantus-ui --example render_lapsus_guitar -- <samples dir> <out dir>
//! ```

use cantus_core::{EffectKind, EffectSpec};
use cantus_engine::fx::{FxUnit, SongPosition};
use cantus_export::job::Control;
use cantus_export::settings::Tags;
use cantus_export::{export_rendered, Audio, ExportSettings, Format};
use std::path::{Path, PathBuf};

const RATE: u32 = 48_000;

/// A sound example: its file, its title, Lapsus's settings.
type Example = (&'static str, &'static str, &'static [(&'static str, f32)]);
const BPM: f64 = 90.0;
const BARS: usize = 8;

/// The chord of each bar (`refs/audio/README.md`).
const CHORDS: [&str; 4] = [
    "Sample07 (A Minor)",
    "Sample15 (F Major)",
    "Sample22 (C Major)",
    "Sample17 (G Major)",
];

/// One strum, stereo interleaved at 48 kHz: the `.flac`, or a `.wav` made from it.
fn strum(dir: &Path, stem: &str) -> Vec<f32> {
    let d = ["flac", "wav"]
        .iter()
        .find_map(|ext| {
            let p = dir.join(format!("{stem}.{ext}"));
            p.exists()
                .then(|| cantus_engine::SampleCache::decode_source(&p).ok())
                .flatten()
        })
        .expect("decoded");
    let ch = d.channels as usize;
    let mut x: Vec<f32> = d.frames.to_vec();
    if d.rate != RATE {
        x = cantus_engine::samples::resample::resample(&x, ch, d.rate, RATE);
    }
    if ch == 1 {
        x.iter().flat_map(|v| [*v, *v]).collect()
    } else {
        x
    }
}

/// The progression: two strums a bar, each faded over 20 ms before the next.
fn progression(dir: &Path) -> (Vec<f32>, Vec<f32>) {
    let beat = (60.0 / BPM * f64::from(RATE)) as usize;
    let n = BARS * 4 * beat + RATE as usize;
    let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
    let fade = (0.02 * f64::from(RATE)) as usize;
    let strums: Vec<Vec<f32>> = CHORDS.iter().map(|c| strum(dir, c)).collect();
    for bar in 0..BARS {
        for half in 0..2 {
            let at = (bar * 4 + half * 2) * beat;
            let len = 2 * beat;
            let s = &strums[bar % 4];
            for i in 0..len.min(s.len() / 2) {
                let g = if i + fade >= len {
                    (len - i) as f32 / fade as f32
                } else {
                    1.0
                };
                l[at + i] += s[2 * i] * g;
                r[at + i] += s[2 * i + 1] * g;
            }
        }
    }
    let peak = l
        .iter()
        .chain(&r)
        .fold(0f32, |m, v| m.max(v.abs()))
        .max(1e-9);
    let g = cantus_dsp::math::db_to_gain(-6.0) / peak;
    for v in l.iter_mut().chain(r.iter_mut()) {
        *v *= g;
    }
    (l, r)
}

fn rms(l: &[f32], r: &[f32]) -> f32 {
    let s: f64 = l
        .iter()
        .chain(r)
        .map(|v| f64::from(*v) * f64::from(*v))
        .sum();
    (s / (l.len() + r.len()) as f64).sqrt() as f32
}

/// Through Lapsus's engine unit with `params`, the song playing from bar 1.
fn through(l: &[f32], r: &[f32], params: &[(&str, f32)]) -> (Vec<f32>, Vec<f32>) {
    let mut spec = EffectSpec::new(EffectKind::TimeShaper);
    for (k, v) in params {
        spec.set(k, *v).unwrap_or_else(|| panic!("{k}"));
    }
    let mut u = cantus_engine::fx::lapsus::Unit::new(RATE as f32);
    u.configure(&spec);
    if let Some(need) = u.memory_need() {
        drop(u.supply(cantus_engine::EffectMemory::build(need, RATE as f32)));
    }
    u.reset();
    let (mut ol, mut or) = (l.to_vec(), r.to_vec());
    let mut at = 0usize;
    while at < ol.len() {
        let n = 480.min(ol.len() - at);
        let beats = at as f64 * BPM / 60.0 / f64::from(RATE);
        u.set_position(&SongPosition {
            playing: true,
            tempo_bpm: BPM,
            tempo_now: BPM,
            beats,
            samples: at as i64,
            ..Default::default()
        });
        u.process(&mut ol[at..at + n], &mut or[at..at + n], None);
        at += n;
    }
    (ol, or)
}

fn write(l: &[f32], r: &[f32], path: &Path, title: &str) {
    let samples: Vec<f32> = l.iter().zip(r).flat_map(|(a, b)| [*a, *b]).collect();
    let s = ExportSettings {
        format: Format::Flac,
        ..Default::default()
    };
    let tags = Tags {
        title: title.into(),
        artist: "Cantus (guitar: Carbonthief, CC0)".into(),
        album: "Lapsus on a guitar".into(),
        ..Default::default()
    };
    export_rendered(
        Audio {
            rate: RATE,
            channels: 2,
            samples,
        },
        &s,
        &tags,
        path,
        &Control::new(),
    )
    .expect("written");
    println!("{}", path.display());
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let src = PathBuf::from(args.first().expect("the samples' folder"));
    let dir = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/sounds/lapsus-guitar"));
    std::fs::create_dir_all(&dir).expect("output dir");
    let (l, r) = progression(&src);
    let dry_rms = rms(&l, &r);
    write(&l, &r, &dir.join("00-guitar-di.flac"), "The DI guitar, dry");
    // (file, title, settings): slots are 0-based (time 4 is index 3).
    let examples: [Example; 6] = [
        (
            "01-tape-stop-quarter",
            "Tape stop ¼ with Fade the last beat: each bar's last beat winds down onto the line",
            &[("tslot", 3.0), ("vslot", 21.0)],
        ),
        (
            "02-tape-stop-bar",
            "Tape stop over the whole bar with its fade",
            &[("tslot", 1.0), ("vslot", 23.0)],
        ),
        (
            "03-half-time",
            "Half time ½: every half bar, its first beat at half speed (an octave down)",
            &[("tslot", 7.0)],
        ),
        (
            "04-stutter-roll",
            "Roll build: a beat, then eighths, sixteenths and thirty-seconds",
            &[("tslot", 16.0)],
        ),
        (
            "05-stutter-gated",
            "Stutter 1/16 under a 1/16 gate",
            &[("tslot", 14.0), ("vslot", 3.0)],
        ),
        (
            "06-reverse-half",
            "Reverse ½: each bar's second half runs its first back",
            &[("tslot", 18.0)],
        ),
    ];
    for (name, title, params) in examples {
        let (mut ol, mut or) = through(&l, &r, params);
        let mut g = dry_rms / rms(&ol, &or).max(1e-9);
        let peak = ol.iter().chain(&or).fold(0f32, |m, v| m.max(v.abs()));
        let ceiling = cantus_dsp::math::db_to_gain(-1.0);
        if peak * g > ceiling {
            g = ceiling / peak;
        }
        for v in ol.iter_mut().chain(or.iter_mut()) {
            *v *= g;
        }
        println!(
            "  {name}: gain {:+.1} dB, RMS {:+.2} dB against the dry",
            20.0 * g.log10(),
            20.0 * (rms(&ol, &or) / dry_rms).log10()
        );
        write(&ol, &or, &dir.join(format!("{name}.flac")), title);
    }
    let readme = "Lapsus on a guitar (Cantus, fx side round)\n\n\
Guitar: Carbonthief, CC0 (Strummed Guitar Chords, the Renoise forum's library).\n\
Am - F - C - G at 90 BPM, struck on beats 1 and 3, eight bars; 00-guitar-di.flac is the dry DI.\n\
Each processed file is level-matched to it (RMS, peak under -1 dBFS).\n";
    std::fs::write(dir.join("README.md"), readme).expect("readme");
}
