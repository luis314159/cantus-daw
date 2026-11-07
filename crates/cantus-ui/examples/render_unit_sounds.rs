//! The website's preset excerpts (round 37, ADR 0101 §8): for every unit's card, a few of its
//! factory presets rendered by the program on what the preset browser auditions them on —
//! an instrument on its role's phrase, an effect on Cantus's own source for its role (the
//! dry source too, to compare) — at the presets' own levels (the render check holds them
//! within ±2 LU of their role), with no processing after, as MP3. It writes the files into
//! `<web>/assets/units/sounds/` and their list into `<web>/src/units_sounds.rs`.
//!
//! `cargo run --release -p cantus-ui --example render_unit_sounds -- web`

use cantus_core::{Snapshot, Ticks};
use cantus_engine::{render, RenderOptions};
use cantus_export::job::Control;
use cantus_export::settings::Tags;
use cantus_export::{export_rendered, Audio, ExportSettings, Format, Mp3Mode, Rate};
use cantus_ui::i18n::{t_in, Locale};
use cantus_ui::preset_audition::{self as aud, Target};
use cantus_ui::preset_catalog::{entries, units, Entry, Unit};
use cantus_ui::preset_tags::Role;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const RATE: u32 = 48_000;

/// The card a unit's excerpts go on (`web/src/units_data.rs`'s `file`).
fn card(u: Unit) -> String {
    match u {
        Unit::Instrument(k) => match k {
            cantus_core::InstrumentKind::Wavetable => "unda",
            cantus_core::InstrumentKind::Analog => "calor",
            cantus_core::InstrumentKind::Fm => "modulus",
            cantus_core::InstrumentKind::Sampler => "exemplar",
            cantus_core::InstrumentKind::Bass => "gravis",
            cantus_core::InstrumentKind::ElectricPiano => "lamina",
            cantus_core::InstrumentKind::Organ => "organum",
            cantus_core::InstrumentKind::Chorda => "chorda",
            cantus_core::InstrumentKind::Dancer => "saltator",
        }
        .to_string(),
        Unit::Kit => "pulsus".into(),
        Unit::Effect(_) => t_in(Locale::En, u.name_key())
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_lowercase(),
    }
}

/// Up to `n` presets of a unit, each a role not heard yet before a second of any role, then
/// styles not heard yet: the range of the unit, not the first `n` of its list.
fn pick(all: &[Entry], n: usize) -> Vec<&Entry> {
    // An init patch is where a sound starts, not one to show.
    let all: Vec<&Entry> = all.iter().filter(|e| !e.id.ends_with(".init")).collect();
    let all = &all[..];
    let mut out: Vec<&Entry> = Vec::new();
    let mut roles = Vec::new();
    let mut styles = Vec::new();
    for &e in all {
        if out.len() < n && !roles.contains(&e.role()) {
            roles.push(e.role());
            styles.extend(e.styles.iter().copied());
            out.push(e);
        }
    }
    for &e in all {
        if out.len() >= n {
            break;
        }
        if !out.iter().any(|o| o.id == e.id) && e.styles.iter().any(|s| !styles.contains(s)) {
            styles.extend(e.styles.iter().copied());
            out.push(e);
        }
    }
    for &e in all {
        if out.len() >= n {
            break;
        }
        if !out.iter().any(|o| o.id == e.id) {
            out.push(e);
        }
    }
    out
}

fn mp3(samples: Vec<f32>, path: &Path, title: &str) -> u64 {
    let s = ExportSettings {
        format: Format::Mp3,
        mp3: Mp3Mode::Cbr(96),
        rate: Rate::Hz(44_100),
        ..Default::default()
    };
    let tags = Tags {
        title: title.into(),
        artist: "Cantus".into(),
        album: "Factory presets".into(),
        ..Default::default()
    };
    let audio = Audio {
        rate: RATE,
        channels: 2,
        samples,
    };
    export_rendered(audio, &s, &tags, path, &Control::new()).expect("the file is written");
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

fn render_prepared(p: &aud::Prepared) -> Vec<f32> {
    let snap = Arc::new(Snapshot::compile(&p.project, 0));
    render(
        snap,
        &RenderOptions {
            sample_rate: RATE,
            start: p.start,
            end: Some(p.end),
            tail_seconds: aud::TAIL,
            compensate_latency: true,
            ..Default::default()
        },
    )
    .expect("it renders")
    .samples
}

/// "Lead · House, Pop" in one language.
fn what(e: &Entry, l: Locale) -> String {
    let mut s = t_in(l, e.role().key());
    if !e.styles.is_empty() {
        s.push_str(" — ");
        s.push_str(
            &e.styles
                .iter()
                .map(|x| t_in(l, x.key()))
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    s
}

fn lit(s: &str) -> String {
    format!("{s:?}")
}

fn main() {
    let web = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "web".into()));
    let dir = web.join("assets/units/sounds");
    std::fs::create_dir_all(&dir).expect("the folder");
    cantus_ui::i18n::set_thread_locale(Locale::En);
    let empty = cantus_core::Project::new("none");
    let mut total = 0u64;
    let mut code = String::from(
        "//! Every unit card's preset excerpts (`assets/units/sounds/`): generated by\n\
         //! `cargo run --release -p cantus-ui --example render_unit_sounds -- web` (round 37,\n\
         //! ADR 0101), rendered by Cantus on what its preset browser plays them on, at the\n\
         //! presets' own levels, with no processing after. Do not edit by hand.\n\n\
         /// One excerpt: its file (without `.mp3`), the preset's name and what it is for, each\n\
         /// `(English, Spanish)`.\n\
         pub struct Sound {\n    pub file: &'static str,\n    pub name: (&'static str, &'static str),\n    pub what: (&'static str, &'static str),\n    /// An effect's dry source (`SOURCES`); empty for an instrument.\n    pub on: &'static str,\n}\n\n\
         /// The excerpts of each card, by its `file`.\npub const SOUNDS: &[(&str, &[Sound])] = &[\n",
    );
    for u in units() {
        if !u.audible() || matches!(u, Unit::Effect(cantus_core::EffectKind::Reverb)) {
            continue;
        }
        let all = entries(u, false);
        let n = if u.is_instrument() { 4 } else { 3 };
        let c = card(u);
        let _ = writeln!(code, "    ({}, &[", lit(&c));
        for e in pick(&all, n) {
            let p = aud::prepare(&empty, Target::None, e, None, aud::TEMPO);
            let stem = format!("{c}-{}", e.id.rsplit('.').next().unwrap_or("preset"));
            let name_en = t_in(Locale::En, &e.id);
            total += mp3(
                render_prepared(&p),
                &dir.join(format!("{stem}.mp3")),
                &name_en,
            );
            let _ = writeln!(
                code,
                "        Sound {{ file: {}, name: ({}, {}), what: ({}, {}), on: {} }},",
                lit(&stem),
                lit(&name_en),
                lit(&t_in(Locale::Es, &e.id)),
                lit(&what(e, Locale::En)),
                lit(&what(e, Locale::Es)),
                lit(match (u.is_instrument(), e.role()) {
                    (true, _) => "",
                    (false, Role::Drums) => "drums",
                    (false, Role::Vocal) => "vocal",
                    _ => "creative",
                })
            );
        }
        code.push_str("    ]),\n");
    }
    code.push_str("];\n\n/// The dry sources the effects are heard on, by role (`source-<role>.mp3`).\npub const SOURCES: &[(&str, (&str, &str))] = &[\n");
    for role in [Role::Drums, Role::Vocal, Role::Creative] {
        let p = aud::source_project(role);
        let prepared = aud::Prepared {
            project: p,
            start: Ticks::ZERO,
            end: Ticks::from_beats_f64(aud::BARS * 4.0),
            on: aud::HeardOn::Source(role),
            needs_engine: false,
            fallback: None,
            restyle: None, // r37:piano
        };
        let stem = format!("source-{}", role.id());
        total += mp3(
            render_prepared(&prepared),
            &dir.join(format!("{stem}.mp3")),
            "Dry source",
        );
        let key = match role {
            Role::Drums => "preset.browser.source.drums",
            Role::Vocal => "preset.browser.source.vocal",
            _ => "preset.browser.source.band",
        };
        let _ = writeln!(
            code,
            "    ({}, ({}, {})),",
            lit(role.id()),
            lit(&t_in(Locale::En, key)),
            lit(&t_in(Locale::Es, key))
        );
    }
    code.push_str("];\n");
    std::fs::write(web.join("src/units_sounds.rs"), code).expect("the list");
    println!("{} KB of excerpts in {}", total / 1024, dir.display());
}
