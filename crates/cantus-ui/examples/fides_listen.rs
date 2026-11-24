//! Fides's listening and fitting tool (round 41, ADR 0150): every factory preset rendered
//! through the real engine, as the preset browser auditions it (its role's phrase,
//! `preset_audition`), and as single notes across its range — WAV files for the owner's
//! ear — with its loudness against its role's target (`preset_measure`).
//!
//! `cargo run --release -p cantus-ui --example fides_listen -- <dir>` writes
//! `<dir>/NN-<preset>.wav` (the phrase) and `<dir>/NN-<preset>-notes.wav` (C2, C3, C4, C5,
//! C6 held two seconds each) and prints each preset's LUFS, its offset and its peaks.
//!
//! `… -- <dir> fit` also moves each preset's `gain` by its offset (five passes; never past a
//! phrase peak of −1.2 dBFS) and writes it into `src/views/inst/fides_presets.rs`.
//!
//! `… -- <dir> [fit] arcus` does the same for Arcus, the bowed string (round 42, ADR 0157):
//! its presets in `src/views/inst/arcus_presets.rs`, its notes across each instrument's own
//! range (the cello C2 … E4, the double bass E1 … G3). `tools/arcus/presets.sh` runs it.

use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec, Project, Snapshot, Ticks};
use cantus_engine::{render, RenderOptions};
use cantus_ui::preset_audition as aud;
use cantus_ui::preset_catalog::{entries, Payload, Unit};
use cantus_ui::preset_measure::{lufs, target};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const RATE: u32 = 48_000;

fn render_project(p: &Project, end: Ticks) -> Vec<f32> {
    let snap = Arc::new(Snapshot::compile(p, 0));
    render(
        snap,
        &RenderOptions {
            sample_rate: RATE,
            start: Ticks::ZERO,
            end: Some(end),
            tail_seconds: aud::TAIL,
            compensate_latency: true,
            ..Default::default()
        },
    )
    .expect("a render")
    .samples
}

fn write_wav(path: &Path, samples: &[f32]) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: RATE,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).expect("a wav");
    for x in samples {
        w.write_sample((x.clamp(-1.0, 1.0) * 8_388_607.0) as i32)
            .expect("a sample");
    }
    w.finalize().expect("the file");
}

fn peak_db(x: &[f32]) -> f64 {
    let p = x.iter().fold(0f32, |m, v| m.max(v.abs()));
    20.0 * (p.max(1e-9) as f64).log10()
}

/// The preset's phrase: its role's, at the audition's tempo.
fn phrase(spec: &NativeSpec, role: cantus_ui::preset_tags::Role) -> Vec<f32> {
    let p = aud::phrase_project(InstrumentSpec::Native(spec.clone()), role);
    render_project(&p, Ticks::beats((4.0 * aud::BARS) as i64))
}

/// Five notes across the keyboard, two seconds each (at 120 BPM a beat is half a second).
fn notes(spec: &NativeSpec, keys: [u8; 5]) -> Vec<f32> {
    let mut p = aud::phrase_project(
        InstrumentSpec::Native(spec.clone()),
        cantus_ui::preset_tags::Role::Keys,
    );
    let mut clip = p.make_clip("notes", Ticks::ZERO, Ticks::beats(20));
    for (i, key) in keys.iter().enumerate() {
        let at = Ticks::beats(4 * i as i64);
        clip.notes.push(p.make_note(at, Ticks::beats(3), *key, 100));
    }
    p.tracks[0].clips = vec![clip];
    render_project(&p, Ticks::beats(20))
}

/// Move `"gain" => x` inside the preset named `label` in the presets' source.
fn write_gain(src: &mut String, label: &str, gain: f32) {
    let Some(at) = src.find(&format!("\"{label}\"")) else {
        return;
    };
    let end = src[at + 1..]
        .find("preset!(")
        .map_or(src.len(), |e| at + 1 + e);
    let block = &src[at..end];
    let Some(g) = block.find("\"gain\" => ") else {
        return;
    };
    let from = at + g + "\"gain\" => ".len();
    let to = from
        + src[from..]
            .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
            .unwrap_or(0);
    src.replace_range(from..to, &format!("{gain:.1}"));
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = PathBuf::from(args.first().map_or("fides-listen", String::as_str));
    let fit = args.iter().any(|a| a == "fit");
    std::fs::create_dir_all(&dir).expect("the folder");
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let arcus = args.iter().any(|a| a == "arcus");
    let (unit, file, prefix) = if arcus {
        (
            InstrumentKind::BowedString,
            "arcus_presets.rs",
            "arcus.preset.",
        )
    } else {
        (
            InstrumentKind::Resonator,
            "fides_presets.rs",
            "fides.preset.",
        )
    };
    let unit = Unit::Instrument(unit);
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/views/inst")
        .join(file);
    let mut src = std::fs::read_to_string(&source).expect("the presets' source");
    println!("| # | Preset | Role | LUFS | Off (LU) | Phrase peak | Notes peak |");
    println!("|---|---|---|---|---|---|---|");
    for (i, e) in entries(unit, false).iter().enumerate() {
        let Payload::Native(spec) = &e.payload else {
            continue;
        };
        let mut spec = spec.clone();
        let role = e.role();
        let want = target(e);
        let mut out = phrase(&spec, role);
        let mut level = lufs(&out, 2, RATE as f32);
        if fit {
            for _ in 0..5 {
                let off = want - level;
                if off.abs() < 0.2 {
                    break;
                }
                let g = (spec.get("gain") + off as f32).clamp(-48.0, 12.0);
                spec.set("gain", (g * 10.0).round() / 10.0);
                out = phrase(&spec, role);
                level = lufs(&out, 2, RATE as f32);
            }
            // The fit stops where the phrase would pass −1.2 dBFS (the render check's
            // headroom): a preset with that much crest sits under its window by its dynamics.
            for _ in 0..3 {
                let p = peak_db(&out);
                if p <= -1.2 {
                    break;
                }
                let g = (spec.get("gain") - (p + 1.3) as f32).clamp(-48.0, 12.0);
                spec.set("gain", (g * 10.0).floor() / 10.0);
                out = phrase(&spec, role);
                level = lufs(&out, 2, RATE as f32);
            }
            write_gain(&mut src, &e.id, spec.get("gain"));
        }
        let name = e.id.trim_start_matches(prefix);
        write_wav(&dir.join(format!("{:02}-{name}.wav", i + 1)), &out);
        // Arcus's notes in each instrument's range (`inst` 1: the double bass).
        let keys = match (arcus, spec.get("inst") >= 0.5) {
            (false, _) => [36, 48, 60, 72, 84],
            (true, false) => [36, 43, 50, 57, 64],
            (true, true) => [28, 35, 40, 47, 55],
        };
        let n = notes(&spec, keys);
        write_wav(&dir.join(format!("{:02}-{name}-notes.wav", i + 1)), &n);
        println!(
            "| {} | {} | {:?} | {:.1} | {:+.1} | {:.1} | {:.1} |",
            i + 1,
            e.name,
            role,
            level,
            level - want,
            peak_db(&out),
            peak_db(&n)
        );
    }
    if fit {
        std::fs::write(&source, src).expect("the presets' source written");
        println!("gains written to {}", source.display());
    }
}
