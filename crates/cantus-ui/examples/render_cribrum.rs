//! Cribrum's sound examples (fx side round, ADR 0130 provisional): Cantus's own audition
//! sources (`preset_audition::source_project`) played eight bars through Cribrum on the
//! master, rendered offline by the engine, written as FLAC with the dry source beside each.
//!
//! ```text
//! cargo run --release -p cantus-ui --example render_cribrum -- <out-dir>
//! ```

use cantus_core::{EffectKind, Project, Snapshot, Ticks};
use cantus_engine::{render, RenderOptions};
use cantus_export::job::Control;
use cantus_export::settings::Tags;
use cantus_export::{export_rendered, Audio, ExportSettings, Format};
use cantus_ui::preset_audition as aud;
use cantus_ui::preset_tags::Role;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const RATE: u32 = 48_000;
const BARS: i64 = 8;

/// The source for `role`, its two bars played four times.
fn source(role: Role, bpm: f64) -> Project {
    let mut p = aud::source_project_at(role, bpm);
    for t in 0..p.tracks.len() {
        let first: Vec<_> = p.tracks[t].clips.clone();
        for rep in 1..(BARS / aud::BARS as i64) {
            for c in &first {
                let mut c = c.clone();
                c.id = cantus_core::ClipId(p.alloc_id());
                c.start = c.start + Ticks::beats(4 * aud::BARS as i64 * rep);
                p.tracks[t].clips.push(c);
            }
        }
    }
    p
}

fn with_cribrum(mut p: Project, params: &[(&str, f32)]) -> Project {
    let mut fx = p.make_effect(EffectKind::AutoFilter);
    for (k, v) in params {
        fx.spec.set(k, *v).unwrap_or_else(|| panic!("{k}"));
    }
    p.master.effects.push(fx);
    p
}

fn render_to(p: &Project, path: &Path, title: &str) {
    let samples = render(
        Arc::new(Snapshot::compile(p, 0)),
        &RenderOptions {
            sample_rate: RATE,
            start: Ticks::ZERO,
            end: Some(Ticks::beats(4 * BARS)),
            tail_seconds: 1.0,
            compensate_latency: true,
            ..Default::default()
        },
    )
    .expect("it renders")
    .samples;
    let peak = samples.iter().fold(0f32, |m, v| m.max(v.abs()));
    let rms = (samples.iter().map(|v| v * v).sum::<f32>() / samples.len().max(1) as f32).sqrt();
    println!(
        "  peak {:+.1} dBFS, RMS {:+.1} dBFS",
        20.0 * peak.max(1e-9).log10(),
        20.0 * rms.max(1e-9).log10()
    );
    let s = ExportSettings {
        format: Format::Flac,
        ..Default::default()
    };
    let tags = Tags {
        title: title.into(),
        artist: "Cantus".into(),
        album: "Cribrum — sound examples".into(),
        ..Default::default()
    };
    let audio = Audio {
        rate: RATE,
        channels: 2,
        samples,
    };
    export_rendered(audio, &s, &tags, path, &Control::new()).expect("the file is written");
    println!("{}", path.display());
}

fn main() {
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/sounds/cribrum"));
    std::fs::create_dir_all(&dir).expect("output dir");
    let bpm = 120.0;
    let examples: [(&str, Role, &str, Vec<(&str, f32)>); 4] = [
        (
            "1-sweep",
            Role::Bus,
            "A two-bar sweep: a resonant ladder, 24 dB/oct, a synced saw rising",
            vec![
                ("mix", 1.0),
                ("a.model", 1.0),
                ("a.slope", 1.0),
                ("a.cut", 700.0),
                ("a.res", 0.72),
                ("os", 1.0),
                ("lfo.div", 2.0),
                ("lfo.shape", 3.0),
                ("lfo.edge", 0.1),
                ("lfo.cut", 2.6),
                ("out", 2.0),
            ],
        ),
        (
            "2-drums-synced-lfo",
            Role::Drums,
            "Drums through a band-pass wobbling on the eighths, the sides apart",
            vec![
                ("mix", 1.0),
                ("a.mode", 1.0),
                ("a.cut", 1_100.0),
                ("a.res", 0.55),
                ("lfo.div", 11.0),
                ("lfo.shape", 1.0),
                ("lfo.cut", 1.8),
                ("lfo.stereo", 90.0),
                ("out", 6.0),
            ],
        ),
        (
            "3-vowel-talk",
            Role::Vocal,
            "A synth lead talking: the vowel filter moving A-E-I-O-U on the half bar",
            vec![
                ("mix", 1.0),
                ("a.mode", 5.0),
                ("a.vowel", 2.0),
                ("a.res", 0.45),
                ("lfo.div", 5.0),
                ("lfo.shape", 0.0),
                ("lfo.vowel", 2.0),
                ("out", 3.0),
            ],
        ),
        (
            "4-envelope-wah",
            Role::Bus,
            "The follower's wah: the band opens the filter as it plays",
            vec![
                ("mix", 1.0),
                ("a.model", 2.0),
                ("a.cut", 250.0),
                ("a.res", 0.6),
                ("env.att", 3.0),
                ("env.rel", 120.0),
                ("env.gain", 6.0),
                ("env.cut", 4.0),
                ("out", 2.0),
            ],
        ),
    ];
    for (name, role, title, params) in examples {
        let dry = source(role, bpm);
        render_to(&dry, &dir.join(format!("{name}-dry.flac")), title);
        let wet = with_cribrum(dry, &params);
        render_to(&wet, &dir.join(format!("{name}.flac")), title);
    }
}
