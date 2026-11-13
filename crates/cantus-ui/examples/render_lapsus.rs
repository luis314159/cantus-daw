//! Lapsus's sound examples (fx side round, ADR 0132 provisional): Cantus's own audition
//! sources (`preset_audition::source_project`) played eight bars through Lapsus on the
//! master, rendered offline by the engine, written as FLAC with the dry source beside each.
//!
//! ```text
//! cargo run --release -p cantus-ui --example render_lapsus -- <out-dir>
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

fn with_lapsus(mut p: Project, params: &[(&str, f32)]) -> Project {
    let mut fx = p.make_effect(EffectKind::TimeShaper);
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
    println!("  peak {:+.1} dBFS", 20.0 * peak.max(1e-9).log10());
    let s = ExportSettings {
        format: Format::Flac,
        ..Default::default()
    };
    let tags = Tags {
        title: title.into(),
        artist: "Cantus".into(),
        album: "Lapsus — sound examples".into(),
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
        .unwrap_or_else(|| PathBuf::from("target/sounds/lapsus"));
    std::fs::create_dir_all(&dir).expect("output dir");
    let bpm = 120.0;
    // (file, source, title, slot (0-based), other settings)
    let examples: [(&str, Role, &str, f32, &[(&str, f32)]); 6] = [
        (
            "1-tape-stop-drums",
            Role::Drums,
            "Tape stop ¼: three beats as they come, the fourth stopping on the bar",
            3.0,
            &[],
        ),
        (
            "2-half-time-drums",
            Role::Drums,
            "Half time ½: every half bar, its first beat at half speed",
            7.0,
            &[],
        ),
        (
            "3-roll-build-drums",
            Role::Drums,
            "Roll build: a beat, then eighths, sixteenths and thirty-seconds of each",
            15.0,
            &[],
        ),
        (
            "4-reverse-bus",
            Role::Bus,
            "Reverse ½: the second half bar runs the first back",
            18.0,
            &[],
        ),
        (
            "5-scratch-drums",
            Role::Drums,
            "Baby scratch: back and forth on every eighth",
            24.0,
            &[("mix", 0.85)],
        ),
        (
            "6-backspin-bus",
            Role::Bus,
            "Backspin: the last beat spun back hard, coming to rest on the bar",
            23.0,
            &[],
        ),
    ];
    for (name, role, title, slot, more) in examples {
        let dry = source(role, bpm);
        render_to(&dry, &dir.join(format!("{name}-dry.flac")), title);
        let mut params = vec![("slot", slot)];
        params.extend_from_slice(more);
        let wet = with_lapsus(dry, &params);
        render_to(&wet, &dir.join(format!("{name}.flac")), title);
    }
}
