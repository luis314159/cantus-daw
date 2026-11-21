//! Nitor's presets for the owner's ears (round 41, ADR 0151): each factory preset rendered
//! as the preset browser auditions it (on Cantus's own source for its role, `preset_audition`)
//! beside the dry source, **both brought to one loudness** (−18 LUFS integrated, a gain
//! alone), as 32-bit float WAV pairs: `NN-<preset>-dry.wav` and `NN-<preset>-wet.wav`, and a
//! `README.txt` with each preset's change in loudness before the matching (what the level
//! match left) and its peak.
//!
//! `cargo run --release -p cantus-ui --example nitor_listen -- <out-dir>` // r41:nitor

use cantus_core::{EffectKind, Project, Snapshot, Ticks};
use cantus_engine::{render, RenderOptions};
use cantus_ui::preset_audition::{self as aud, Target};
use cantus_ui::preset_catalog::{entries, Unit};
use cantus_ui::preset_measure::lufs;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The loudness each pair is brought to, LUFS.
const TARGET: f64 = -18.0;

fn render_stretch(p: &Project, start: Ticks, end: Ticks) -> (Vec<f32>, u32) {
    let snap = Arc::new(Snapshot::compile(p, 0));
    let r = render(
        snap,
        &RenderOptions {
            start,
            end: Some(end),
            tail_seconds: aud::TAIL,
            compensate_latency: true,
            ..Default::default()
        },
    )
    .expect("render");
    (r.samples, r.sample_rate)
}

fn write_wav(path: &Path, x: &[f32], sr: u32) {
    let mut b = Vec::with_capacity(44 + x.len() * 4);
    let data = (x.len() * 4) as u32;
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&3u16.to_le_bytes()); // IEEE float
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&sr.to_le_bytes());
    b.extend_from_slice(&(sr * 8).to_le_bytes());
    b.extend_from_slice(&8u16.to_le_bytes());
    b.extend_from_slice(&32u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    for v in x {
        b.extend_from_slice(&v.to_le_bytes());
    }
    std::fs::write(path, b).expect("write");
}

fn gain_to(x: &mut [f32], from: f64) -> f64 {
    let g = 10f64.powf((TARGET - from) / 20.0) as f32;
    for v in x.iter_mut() {
        *v *= g;
    }
    20.0 * f64::from(g).log10()
}

fn peak_db(x: &[f32]) -> f64 {
    20.0 * f64::from(x.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-9)).log10()
}

fn main() {
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/nitor-listen"));
    std::fs::create_dir_all(&dir).expect("output dir");
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let empty = Project::new("none");
    let mut readme = String::from(
        "Nitor's factory presets, each beside its dry source, both at -18 LUFS (a gain alone).\n\
         change: the preset's loudness over the dry before that gain (the level match's work);\n\
         peak: the wet file's sample peak after it.\n\n",
    );
    for (i, e) in entries(Unit::Effect(EffectKind::Enhancer), false)
        .iter()
        .enumerate()
    {
        let prepared = aud::prepare(&empty, Target::None, e, None, aud::TEMPO);
        let (mut wet, sr) = render_stretch(&prepared.project, prepared.start, prepared.end);
        let dry_p = aud::source_project(e.role());
        let (mut dry, _) = render_stretch(
            &dry_p,
            Ticks::ZERO,
            Ticks::from_beats_f64(aud::BARS * 4.0),
        );
        let (lw, ld) = (lufs(&wet, 2, sr as f32), lufs(&dry, 2, sr as f32));
        gain_to(&mut wet, lw);
        gain_to(&mut dry, ld);
        let short = e.id.trim_start_matches("preset.nitor.");
        write_wav(&dir.join(format!("{:02}-{short}-dry.wav", i + 1)), &dry, sr);
        write_wav(&dir.join(format!("{:02}-{short}-wet.wav", i + 1)), &wet, sr);
        let _ = writeln!(
            readme,
            "{:02} {:<34} {:<16} change {:+.2} LU   peak {:.1} dBFS",
            i + 1,
            e.name,
            format!("{:?}", e.role()),
            lw - ld,
            peak_db(&wet)
        );
        println!("{:02} {} {:+.2} LU", i + 1, e.name, lw - ld);
    }
    std::fs::write(dir.join("README.txt"), readme).expect("readme");
    println!("{}", dir.display());
}
