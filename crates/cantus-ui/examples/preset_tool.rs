//! The preset library's tool (round 37, ADR 0101), on `crates/cantus-ui/presets/`:
//!
//! - `export` — every unit's presets written in code (and the tags its data file gives
//!   them) written as that unit's data file; the step that moved the arrays into data.
//! - `fit [unit…]` — each factory preset rendered on its role's material and its level
//!   brought to its window's centre: an instrument's `gain`, an effect's output (`out`,
//!   `gain` or `makeup`, whichever it has), rewritten in its file. Five passes (a unit
//!   that saturates after its gain needs more than one). Prints what it moved.
//!
//! `cargo run --release -p cantus-ui --example preset_tool -- <command> [unit…]`

use cantus_core::{EffectKind, InstrumentKind, Project, Snapshot, Ticks};
use cantus_engine::{render, RenderOptions};
use cantus_ui::preset_audition::{self as aud, Target};
use cantus_ui::preset_catalog::{entries, Entry, Unit};
use cantus_ui::preset_data::{self, DataPreset, File, FileUnit, TagRecord};
use cantus_ui::preset_tags::Role;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("presets")
}

/// The units whose presets are the library's (Chorda's are piano's, the dancer's are
/// dances, Pulsus's kits are drums').
fn library_units() -> Vec<FileUnit> {
    let mut out: Vec<FileUnit> = InstrumentKind::ALL
        .into_iter()
        .filter(|k| !matches!(k, InstrumentKind::Chorda | InstrumentKind::Dancer))
        .map(FileUnit::Instrument)
        .collect();
    out.extend(
        EffectKind::ALL
            .into_iter()
            .filter(|k| *k != EffectKind::External)
            .map(FileUnit::Effect),
    );
    out
}

fn unit_of(u: FileUnit) -> Unit {
    match u {
        FileUnit::Instrument(k) => Unit::Instrument(k),
        FileUnit::Effect(k) => Unit::Effect(k),
        FileUnit::Kits => Unit::Kit,
    }
}

fn header(u: FileUnit) -> String {
    let name = cantus_ui::i18n::t_in(cantus_ui::i18n::Locale::En, unit_of(u).name_key());
    format!(
        "{name}: factory presets (ADR 0101). Names are locale keys (the unit's locale file).\n\
         `cargo run --release -p cantus-ui --example preset_tool -- fit` sets the levels;\n\
         the tests check every key, value, style and role (`preset_data`, `preset_render`)."
    )
}

fn read(u: FileUnit) -> File {
    let path = root().join(preset_data::path_of(u));
    match std::fs::read_to_string(&path) {
        Ok(t) => preset_data::parse(&t).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
        Err(_) => File::default(),
    }
}

fn save(u: FileUnit, f: &File) {
    let path = root().join(preset_data::path_of(u));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, preset_data::write(f, &header(u))).unwrap();
    println!("wrote {}", path.display());
}

/// A preset written in code: its key, values, texts and tags.
type CodePreset = (
    &'static str,
    Vec<(String, f32)>,
    Vec<(String, String)>,
    Vec<String>,
);

/// The code presets of a unit as data, with the styles and roles the catalogue reads for
/// them; the data file's own presets after them, its tags dropped (they are in the presets
/// now).
fn export() {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    for u in library_units() {
        let unit = unit_of(u);
        let all = entries(unit, false);
        let old = read(u);
        let mut f = File::default();
        let code: Vec<CodePreset> = match u {
            FileUnit::Instrument(k) => cantus_ui::views::inst::code_presets(k)
                .iter()
                .map(|p| {
                    (
                        p.label_key,
                        p.values.iter().map(|(a, b)| (a.to_string(), *b)).collect(),
                        p.text
                            .iter()
                            .map(|(a, b)| (a.to_string(), b.to_string()))
                            .collect(),
                        p.tags.iter().map(|t| t.to_string()).collect(),
                    )
                })
                .collect(),
            FileUnit::Effect(k) => cantus_ui::views::fx::code_presets(k)
                .iter()
                .map(|(l, v)| {
                    (
                        *l,
                        v.iter().map(|(a, b)| (a.to_string(), *b)).collect(),
                        Vec::new(),
                        Vec::new(),
                    )
                })
                .collect(),
            FileUnit::Kits => Vec::new(),
        };
        for (key, values, text, tags) in code {
            let e = all.iter().find(|e| e.id == key).expect("in the catalogue");
            f.preset.push(DataPreset {
                key: key.to_string(),
                note: String::new(),
                styles: e.styles.iter().map(|s| s.id().to_string()).collect(),
                roles: e.roles.iter().map(|r| r.id().to_string()).collect(),
                values: values.into_iter().collect::<BTreeMap<_, _>>(),
                text: text.into_iter().collect::<BTreeMap<_, _>>(),
                tags: tags
                    .into_iter()
                    .filter(|t| !t.starts_with("preset.style.") && !t.starts_with("preset.role."))
                    .collect(),
            });
        }
        f.preset.extend(old.preset);
        let _: Vec<TagRecord> = old.tag;
        save(u, &f);
    }
}

// ---- fit ------------------------------------------------------------------------------------

/// The key a unit's level is set with, and its range.
fn level_key(u: FileUnit) -> Option<(&'static str, f32, f32)> {
    match u {
        FileUnit::Instrument(k) => k.param("gain").map(|i| ("gain", i.min, i.max)),
        // Aula's output is `level` (dB), the only effect with that key.
        FileUnit::Effect(k) => ["out", "gain", "makeup", "level"]
            .into_iter()
            .find_map(|key| {
                k.params()
                    .iter()
                    .find(|i| i.key == key)
                    .map(|i| (i.key, i.min, i.max))
            }),
        FileUnit::Kits => None,
    }
}

fn render_lufs(e: &Entry) -> f64 {
    let empty = Project::new("none");
    let p = aud::prepare(&empty, Target::None, e, None, aud::TEMPO);
    let snap = Arc::new(Snapshot::compile(&p.project, 0));
    let r = render(
        snap,
        &RenderOptions {
            start: p.start,
            end: Some(p.end),
            tail_seconds: aud::TAIL,
            compensate_latency: true,
            ..Default::default()
        },
    )
    .expect("it renders");
    cantus_ui::preset_measure::lufs(&r.samples, r.channels as usize, r.sample_rate as f32)
}

/// The peak of an instrument preset on the headroom test's dense riff, dBFS.
fn riff_peak_db(spec: &cantus_core::NativeSpec) -> f64 {
    let p = aud::riff_project(spec);
    let snap = Arc::new(Snapshot::compile(&p, 0));
    let r = render(
        snap,
        &RenderOptions {
            tail_seconds: 1.0,
            ..Default::default()
        },
    )
    .expect("it renders");
    20.0 * f64::from(r.peak.max(1e-9)).log10()
}

fn dry(role: Role) -> f64 {
    let p = aud::source_project(role);
    let snap = Arc::new(Snapshot::compile(&p, 0));
    let r = render(
        snap,
        &RenderOptions {
            end: Some(Ticks::from_beats_f64(aud::BARS * 4.0)),
            tail_seconds: aud::TAIL,
            ..Default::default()
        },
    )
    .expect("it renders");
    cantus_ui::preset_measure::lufs(&r.samples, r.channels as usize, r.sample_rate as f32)
}

fn fit(only: &[String]) {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    for u in library_units() {
        let name = format!("{u:?}");
        if !only.is_empty()
            && !only
                .iter()
                .any(|o| name.to_lowercase().contains(&o.to_lowercase()))
        {
            continue;
        }
        let Some((key, lo, hi)) = level_key(u) else {
            println!("{name}: no level key, left as it is");
            continue;
        };
        if matches!(
            u,
            FileUnit::Effect(EffectKind::Limiter | EffectKind::Maximizer | EffectKind::Clipper)
        ) {
            continue;
        }
        let mut f = read(u);
        for i in 0..f.preset.len() {
            if cantus_ui::preset_measure::by_design(&f.preset[i].key).is_some() {
                continue;
            }
            for _pass in 0..5 {
                let p = f.preset[i].clone();
                let e = entry_of(u, &p);
                let want = cantus_ui::preset_measure::target(&e);
                let got = if e.unit.is_instrument() {
                    render_lufs(&e)
                } else {
                    render_lufs(&e) - dry(e.role())
                };
                let off = (want - got) as f32;
                // How far an instrument may still rise before its velocity-127 headroom runs out
                // (the headroom test's dense riff under `HEADROOM_DB`).
                let room = match &e.payload {
                    cantus_ui::preset_catalog::Payload::Native(spec) => {
                        (cantus_ui::preset_measure::HEADROOM_DB - riff_peak_db(spec)) as f32
                    }
                    _ => f32::INFINITY,
                };
                if (off.abs() < 0.3 && room >= 0.0) || got < -60.0 {
                    break;
                }
                let now = p
                    .values
                    .get(key)
                    .copied()
                    .unwrap_or_else(|| default_of(u, key));
                let next = (now + off).min(now + room).clamp(lo, hi);
                let next = (next * 10.0).round() / 10.0;
                println!(
                    "{name} {}: {got:.1} → want {want:.1}: {key} {now} → {next}",
                    p.key
                );
                if next == now {
                    break;
                }
                f.preset[i].values.insert(key.to_string(), next);
            }
        }
        save(u, &f);
    }
}

fn default_of(u: FileUnit, key: &str) -> f32 {
    match u {
        FileUnit::Instrument(k) => k.param(key).map(|i| i.default).unwrap_or(0.0),
        FileUnit::Effect(k) => k
            .params()
            .iter()
            .find(|i| i.key == key)
            .map(|i| i.default)
            .unwrap_or(0.0),
        FileUnit::Kits => 0.0,
    }
}

/// The catalogue's entry for a data preset as it stands in the file being fitted.
fn entry_of(u: FileUnit, p: &DataPreset) -> Entry {
    cantus_ui::preset_catalog::entry_of_data(unit_of(u), p)
}

/// Every unit's parameters, one line each: key, range, default, unit, name (what presets are
/// written with).
fn params() {
    use cantus_ui::i18n::{t_in, Locale::En};
    for u in library_units() {
        let (name, list): (String, &[cantus_core::EffectParamInfo]) = match u {
            FileUnit::Instrument(k) => (t_in(En, k.name_key()), k.params()),
            FileUnit::Effect(k) => (t_in(En, k.name_key()), k.params()),
            FileUnit::Kits => continue,
        };
        println!("## {name} ({u:?})");
        for i in list {
            println!(
                "{} [{}..{}] ={} {} {}{}",
                i.key,
                i.min,
                i.max,
                i.default,
                i.unit,
                t_in(En, i.label_key),
                if i.stepped { " (steps)" } else { "" }
            );
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("export") => export(),
        Some("fit") => fit(&args[1..]),
        Some("params") => params(),
        _ => eprintln!("preset_tool export | fit [unit…]"),
    }
}
