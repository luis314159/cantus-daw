//! Tabula on a real guitar (fx side round, wave 3, ADR 0131): the owner's reference
//! progression — Am, F, C, G, one chord a bar at 90 BPM, struck on beats 1 and 3, eight bars —
//! from Carbonthief's DI strums (CC0), through the engine's own board (each pedal the unit
//! itself) and written as FLAC beside the dry guitar (`00-guitar-di.flac`, peak −6 dBFS),
//! each processed file level-matched to it within 1 dB by the board's own output.
//!
//! ```text
//! cargo run -p cantus-ui --example render_tabula_guitar -- <WAV folder> <out dir>
//! ```
//!
//! The rig (the starting preset), a parallel A/B split, and a macro swept over the eight bars
//! (its drive and its echo rising together).

use cantus_core::fx::tabula::{guitar_rig, Board, Macro, MacroMap, Path, Pedal, PedalId};
use cantus_core::{EffectId, EffectKind, EffectSlot, EffectSpec};
use cantus_export::job::Control;
use cantus_export::settings::Tags;
use cantus_export::{export_rendered, Audio, ExportSettings, Format};
use std::path::{Path as FsPath, PathBuf};

const RATE: u32 = 48_000;

const BPM: f64 = 90.0;
const BARS: usize = 8;

/// The chord of each bar (`README.md`).
const CHORDS: [&str; 4] = [
    "Sample07 (A Minor)",
    "Sample15 (F Major)",
    "Sample22 (C Major)",
    "Sample17 (G Major)",
];

/// One strum, stereo interleaved at 48 kHz: the `.flac`, or a `.wav` made from it where the
/// decoder cannot read the FLAC (Carbonthief's are 32-bit, which `claxon` does not decode).
fn strum(dir: &FsPath, stem: &str) -> Vec<f32> {
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
fn progression(dir: &FsPath) -> (Vec<f32>, Vec<f32>) {
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

/// The guitar through the board `spec` (its macros applied block by block when `sweep`
/// moves macro 1 from 0 to 1 over the whole take), on the engine's own unit.
fn through(l: &[f32], r: &[f32], spec: &EffectSpec, sweep: bool) -> (Vec<f32>, Vec<f32>) {
    let mut spec = spec.clone();
    cantus_core::fx::tabula::ensure_macro_keys(&mut spec);
    cantus_core::fx::tabula::apply_macros(&mut spec);
    let slot = EffectSlot {
        id: EffectId(1),
        bypass: false,
        mix: 1.0,
        sidechain: None,
        midi_from: None,
        spec: spec.clone(),
    };
    let mut u = cantus_engine::EffectUnit::new(&slot, RATE as f32);
    let (mut ol, mut or) = (l.to_vec(), r.to_vec());
    let lat = u.latency();
    ol.extend(std::iter::repeat_n(0.0, lat));
    or.extend(std::iter::repeat_n(0.0, lat));
    let mut at = 0usize;
    while at < ol.len() {
        let n = 480.min(ol.len() - at);
        if sweep {
            spec.set("m1", at as f32 / l.len() as f32);
            cantus_core::fx::tabula::apply_macros(&mut spec);
            u.configure(&spec);
        }
        u.process(&mut ol[at..at + n], &mut or[at..at + n]);
        at += n;
    }
    // Aligned with the dry guitar: the board's latency taken off.
    (ol[lat..].to_vec(), or[lat..].to_vec())
}

fn write(l: &[f32], r: &[f32], path: &FsPath, title: &str) {
    let samples: Vec<f32> = l.iter().zip(r).flat_map(|(a, b)| [*a, *b]).collect();
    let s = ExportSettings {
        format: Format::Flac,
        ..Default::default()
    };
    let tags = Tags {
        title: title.into(),
        artist: "Cantus (guitar: Carbonthief, CC0)".into(),
        album: "Tabula on a guitar".into(),
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

fn pedal(id: u16, kind: EffectKind, path: Path, kv: &[(&str, f32)]) -> Pedal {
    let mut p = Pedal::new(PedalId(id), EffectSpec::new(kind));
    p.path = path;
    for (k, v) in kv {
        p.spec.set(k, *v).unwrap_or_else(|| panic!("{k}"));
    }
    p
}

fn board(pedals: Vec<Pedal>, macros: Vec<Macro>) -> EffectSpec {
    let mut s = EffectSpec::new(EffectKind::Board);
    s.board = Some(Box::new(Board { pedals, macros }));
    s
}

fn main() {
    let mut args = std::env::args().skip(1);
    let samples = PathBuf::from(args.next().expect("the WAV folder"));
    let out = PathBuf::from(
        args.next()
            .unwrap_or_else(|| "target/sounds/tabula-guitar".into()),
    );
    std::fs::create_dir_all(&out).expect("out dir");
    let (l, r) = progression(&samples);
    write(&l, &r, &out.join("00-guitar-di.flac"), "The DI guitar");
    let dry = rms(&l, &r);
    let split = board(
        vec![
            pedal(
                0,
                EffectKind::Pedal,
                Path::Pre,
                &[
                    ("s1", 1.0),
                    ("od.drive", 0.45),
                    ("od.tone", 0.6),
                    ("od.level", 0.55),
                ],
            ),
            pedal(
                1,
                EffectKind::Delay,
                Path::A,
                &[("time_ms", 500.0), ("feedback", 0.4), ("mix", 1.0)],
            ),
            pedal(
                2,
                EffectKind::Chorus,
                Path::B,
                &[("mode", 0.0), ("mix", 0.5), ("depth", 0.7)],
            ),
        ],
        Vec::new(),
    );
    let mut swept = guitar_rig();
    swept.board.as_mut().unwrap().macros = vec![Macro {
        name: Some("Drive and echo".into()),
        maps: vec![
            MacroMap {
                pedal: PedalId(0),
                key: "od.drive".into(),
                from: 0.05,
                to: 0.9,
            },
            MacroMap {
                pedal: PedalId(2),
                key: "mix".into(),
                from: 0.0,
                to: 0.45,
            },
            MacroMap {
                pedal: PedalId(2),
                key: "feedback".into(),
                from: 0.1,
                to: 0.55,
            },
        ],
    }];
    let examples: [(&str, &str, EffectSpec, bool); 3] = [
        (
            "01-guitar-rig",
            "The guitar rig (the starting preset): the green overdrive low, the chorus, the delay, the spring",
            guitar_rig(),
            false,
        ),
        (
            "02-ab-split",
            "A parallel split: the overdrive, then the delay alone on lane A beside the chorus on lane B, half and half",
            split,
            false,
        ),
        (
            "03-macro-sweep",
            "The rig with one macro swept from 0 to 100 % over the eight bars: the drive and the echo rising together",
            swept,
            true,
        ),
    ];
    let mut readme = String::from(
        "# Tabula on a guitar

Guitar: Carbonthief, CC0 (*Strummed Guitar Chords*, DI).

         The owner's reference progression: Am - F - C - G, one chord a bar at 90 BPM, struck
         on beats 1 and 3, eight bars. `00-guitar-di.flac` is the dry guitar (peak -6 dBFS);
         each other file is the board on it, level-matched to the dry guitar within 1 dB by the
         board's own output, its latency taken off so it lines up with the dry file:

",
    );
    for (name, title, spec, sweep) in examples {
        let (wl, wr) = through(&l, &r, &spec, sweep);
        let db = 20.0 * cantus_dsp::math::log10(dry / rms(&wl, &wr).max(1e-9));
        let mut s = spec.clone();
        s.set("out", db.clamp(-24.0, 24.0));
        let (wl, wr) = through(&l, &r, &s, sweep);
        let off = 20.0 * cantus_dsp::math::log10(rms(&wl, &wr) / dry);
        println!("  {name}: out {db:+.1} dB, {off:+.2} dB from the dry level");
        write(&wl, &wr, &out.join(format!("{name}.flac")), title);
        readme.push_str(&format!(
            "- `{name}`: {title}
"
        ));
    }
    std::fs::write(out.join("README.md"), readme).expect("readme");
}
