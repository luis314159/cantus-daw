//! The website's preset excerpts (round 37, ADR 0101 §8): for every unit's card, a few of its
//! factory presets rendered by the program on what the preset browser auditions them on —
//! an instrument on its role's phrase, an effect on Cantus's own source for its role (the
//! dry source too, to compare) — at the presets' own levels (the render check holds them
//! within ±2 LU of their role), with no processing after, as MP3. It writes the files into
//! `<web>/assets/units/sounds/` and their list into `<web>/src/units_sounds.rs`.
//!
//! `cargo run --release -p cantus-ui --example render_unit_sounds -- web`
//!
//! `-- web chorda` (any card names after the folder) renders those cards' excerpts alone and
//! leaves the list and the dry sources as they are (a unit re-voiced: its card's files only).

use cantus_core::{EffectKind, Snapshot, Ticks};
use cantus_engine::{render, RenderOptions};
use cantus_export::job::Control;
use cantus_export::settings::Tags;
use cantus_export::{export_rendered, Audio, ExportSettings, Format, Mp3Mode, Rate};
use cantus_ui::demos::song::{colors, Feel, Song};
use cantus_ui::demos::theory::Ev;
use cantus_ui::i18n::{t_in, Locale};
use cantus_ui::preset_audition::{self as aud, Target};
use cantus_ui::preset_catalog::{entries, units, Entry, Payload, Unit};
use cantus_ui::preset_tags::{Role, Style};
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
            cantus_core::InstrumentKind::AcousticDrums => "tympanum", // r40:p3
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

/// fix/final-d — Tympanum's grooves: (file stem, modelled kit, BPM, style, events in beats
/// over four bars on the General MIDI drum keys the modelled kits play).
fn grooves() -> Vec<(&'static str, &'static str, f64, Style, Vec<Ev>)> {
    let ev = |at: f64, key: u8, vel: u8| Ev::new(at, 0.1, key, vel);
    // Rock: eighths on the hat, the kick on one and three and the "and" of three, a crash
    // opening the phrase and a tom fill closing it.
    let mut rock = vec![ev(0.0, 49, 112)];
    for bar in 0..4 {
        let b = f64::from(bar) * 4.0;
        for k in [0.0, 2.0, 2.5] {
            rock.push(ev(b + k, 36, 110));
        }
        for s in [1.0, 3.0] {
            if !(bar == 3 && s == 3.0) {
                rock.push(ev(b + s, 38, 112));
            }
        }
        for i in 0..8 {
            if bar == 3 && i >= 6 {
                continue;
            }
            rock.push(ev(
                b + f64::from(i) * 0.5,
                42,
                if i % 2 == 0 { 96 } else { 70 },
            ));
        }
    }
    for (i, key) in [48u8, 48, 45, 45, 43, 43, 43, 36].iter().enumerate() {
        rock.push(ev(15.0 + i as f64 * 0.125, *key, 100 - (i as u8) * 4));
    }
    // Funk: sixteenths on the hat, accented on the eighths, ghost notes on the snare, the
    // kick pushed off the beat, the hat opened on the last eighth and shut by the foot.
    let mut funk = Vec::new();
    for bar in 0..4 {
        let b = f64::from(bar) * 4.0;
        for k in [0.0, 0.75, 2.5, 3.25] {
            funk.push(ev(b + k, 36, 108));
        }
        for s in [1.0, 3.0] {
            funk.push(ev(b + s, 38, 114));
        }
        for g in [0.5, 1.75, 2.25, 3.75] {
            funk.push(ev(b + g, 38, 30));
        }
        for i in 0..14 {
            let v = if i % 4 == 0 {
                92
            } else if i % 2 == 0 {
                72
            } else {
                50
            };
            funk.push(ev(b + f64::from(i) * 0.25, 42, v));
        }
        funk.push(ev(b + 3.5, 46, 90));
    }
    for bar in 1..4 {
        funk.push(ev(f64::from(bar) * 4.0, 44, 70));
    }
    // Swing: the ride's spang-a-lang (triplet swing) played light, the hat's foot on two and
    // four, the kick feathered on the quarters, the snare comping.
    let mut swing = Vec::new();
    for bar in 0..4 {
        let b = f64::from(bar) * 4.0;
        for (at, v) in [
            (0.0, 70),
            (1.0, 76),
            (1.0 + 2.0 / 3.0, 52),
            (2.0, 70),
            (3.0, 76),
            (3.0 + 2.0 / 3.0, 52),
        ] {
            swing.push(ev(b + at, 51, v));
        }
        for f in [1.0, 3.0] {
            swing.push(ev(b + f, 44, 88));
        }
        for q in 0..4 {
            swing.push(ev(b + f64::from(q), 36, 50));
        }
    }
    for (at, v) in [
        (2.0 + 2.0 / 3.0, 72),
        (5.0 + 2.0 / 3.0, 64),
        (7.0 + 2.0 / 3.0, 80),
        (10.0 + 2.0 / 3.0, 70),
        (14.0, 88),
        (15.0 + 2.0 / 3.0, 100),
    ] {
        swing.push(ev(at, 38, v));
    }
    swing.push(ev(15.0 + 2.0 / 3.0, 36, 90));
    // Boom bap: a swung sixteenth hat, the kick lazy around one, the snare hard on two and
    // four.
    let mut boom = Vec::new();
    for bar in 0..4 {
        let b = f64::from(bar) * 4.0;
        for k in [0.0, 0.75, 1.75, 2.5] {
            boom.push(ev(b + k, 36, 112));
        }
        for s in [1.0, 3.0] {
            boom.push(ev(b + s, 38, 120));
        }
        for i in 0..8 {
            boom.push(ev(
                b + f64::from(i) * 0.5,
                42,
                if i % 2 == 0 { 88 } else { 60 },
            ));
            boom.push(ev(b + f64::from(i) * 0.5 + 1.0 / 3.0, 42, 40));
        }
    }
    vec![
        ("rock", "studio", 104.0, Style::Rock, rock),
        ("funk", "studio", 98.0, Style::RnB, funk),
        ("swing", "jazz", 150.0, Style::JazzSoul, swing),
        ("boom_bap", "studio", 88.0, Style::TrapHipHop, boom),
    ]
}

/// Where Tympanum's grooves sit, LUFS integrated (Pulsus's excerpts: -19 to -21).
const TYMPANUM_LUFS: f64 = -21.5;

/// A factory preset's values, by its id.
fn factory_values(kind: EffectKind, id: &str) -> std::collections::BTreeMap<String, f32> {
    entries(Unit::Effect(kind), false)
        .into_iter()
        .find_map(|e| match e.payload {
            Payload::Effect(v) if e.id == id => Some(v),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no preset {id}"))
}

/// Integrated loudness (LUFS) and true peak (dBTP) of an interleaved stereo render.
fn loudness(x: &[f32]) -> (f64, f64) {
    let l: Vec<f32> = x.iter().step_by(2).copied().collect();
    let r: Vec<f32> = x.iter().skip(1).step_by(2).copied().collect();
    let mut m = cantus_dsp::terminus::loudness::LoudnessMeter::new(RATE as f32);
    m.process(&l, &r);
    (
        m.integrated().unwrap_or(-120.0),
        f64::from(m.true_peak_db()),
    )
}

/// fix/final-d — Tympanum's excerpts: each modelled kit (instant, no download; never a
/// recorded kit, ADRs 0030/0042) in its generated voice on a groove, rendered as the presets
/// are, through Cantus's own Vinculum, Tonsor and Terminus on the kit's strip (each caption says so).
fn tympanum(card: &str, dir: &Path, code: &mut String) -> u64 {
    use cantus_core::inst::tympanum::KIT_KEY;
    use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec, TrackIcon};
    let mut total = 0;
    for (stem, kit, bpm, style, evs) in grooves() {
        // The groove as an engineer would print a real kit: on the kit's strip, Vinculum's
        // *Mix bus glue* (the VCA at 2:1, 10 ms attack: the hits keep their crack), Tonsor's
        // *Drum bus* (the hits' first instants clipped 6 dB, oversampled, the level matched:
        // what a modelled kit's sharpest spikes need, so the limiter after hardly works) into
        // Terminus *Drums* (its fast drum-bus style, 80 ms release; ceiling -1 dBTP, true peak),
        // Terminus's input gain the one level set (no processing after the render).
        let prepared = |lim_gain: f32, limit: bool| {
            let mut spec = NativeSpec::new(InstrumentKind::AcousticDrums);
            spec.set_text(KIT_KEY, Some(format!("model:{kit}")))
                .expect("a kit");
            let mut s = Song::new("tympanum", bpm, 1);
            let part = s.part(
                "Drums",
                InstrumentSpec::Native(spec),
                colors::KICK,
                TrackIcon::Drums,
            );
            s.feel(part, Feel::TIGHT);
            s.clip(part, "groove", 0.0, 4.0, &evs);
            let mut project = s.finish();
            let strip = project.tracks[0].strip.expect("the kit's strip");
            let mut chain = Vec::new();
            for (kind, id, set) in [
                (EffectKind::BusCompressor, "preset.vinculum.mix_glue", None),
                (EffectKind::Clipper, "preset.tonsor.drum_bus", None),
                (EffectKind::Limiter, "lim.preset.drums", Some(lim_gain)),
            ] {
                let mut values = factory_values(kind, id);
                if let Some(g) = set {
                    values.insert("gain".into(), g);
                    values.insert("ceiling".into(), -1.0);
                    values.insert("tp".into(), 1.0);
                }
                let mut slot = project.make_effect(kind);
                slot.spec.params = aud::effect_params_after(&slot.spec, &values);
                slot.bypass = set.is_some() && !limit;
                chain.push(slot);
            }
            project
                .strip_mut(strip)
                .expect("the strip")
                .effects
                .extend(chain);
            aud::Prepared {
                project,
                start: Ticks::ZERO,
                end: Ticks::from_beats_f64(16.0),
                on: aud::HeardOn::Phrase(Role::Drums, bpm),
                needs_engine: false,
                fallback: None,
                restyle: None,
            }
        };
        // Terminus's input gain found by measuring: each groove near TYMPANUM_LUFS (a kit's
        // groove on the site sits with Pulsus's, -19 to -21), its peaks held by the ceiling.
        let mut gain = 0.0f32;
        let (dry_lufs, dry_tp) = loudness(&render_prepared(&prepared(0.0, true)));
        for _ in 0..6 {
            let (now, _) = loudness(&render_prepared(&prepared(gain, true)));
            if (now - TYMPANUM_LUFS).abs() < 0.15 {
                break;
            }
            gain = (gain + (TYMPANUM_LUFS - now) as f32).clamp(0.0, 30.0);
        }
        let p = prepared(gain, true);
        let (after, tp_after) = loudness(&render_prepared(&p));
        // What Terminus takes off the loudest hit: the true peak it would pass at that gain.
        let (_, tp_open) = loudness(&render_prepared(&prepared(gain, false)));
        println!(
            "tympanum-{stem}: {dry_lufs:.1} LUFS {dry_tp:+.1} dBTP through Vinculum, Tonsor, Terminus +{gain:.1} dB -> {after:.1} LUFS {tp_after:.1} dBTP (its loudest hit {:.1} dB down)",
            tp_open + f64::from(gain) - tp_after
        );
        let kit_key = format!("tympanum.kit.{kit}");
        let groove = |l: Locale| match (stem, l) {
            ("rock", Locale::En) => "rock groove",
            ("rock", _) => "ritmo de rock",
            ("funk", Locale::En) => "funk groove",
            ("funk", _) => "ritmo de funk",
            ("swing", Locale::En) => "swing on the ride",
            ("swing", _) => "swing en el ride",
            (_, Locale::En) => "boom bap",
            _ => "boom bap",
        };
        let name = |l: Locale| format!("{} · {}", t_in(l, &kit_key), groove(l));
        let chain = |l: Locale| match l {
            Locale::En => "through Vinculum, Tonsor and Terminus on its strip",
            _ => "por Vinculum, Tonsor y Terminus en su canal",
        };
        let what = |l: Locale| {
            format!(
                "{} — {} · {}",
                t_in(l, Role::Drums.key()),
                t_in(l, style.key()),
                chain(l)
            )
        };
        let file = format!("{card}-{stem}");
        total += mp3(
            render_prepared(&p),
            &dir.join(format!("{file}.mp3")),
            &name(Locale::En),
        );
        let _ = writeln!(
            code,
            "        snd!({}, ({}, {}), ({}, {}), {}),",
            lit(&file),
            lit(&name(Locale::En)),
            lit(&name(Locale::Es)),
            lit(&what(Locale::En)),
            lit(&what(Locale::Es)),
            lit("")
        );
    }
    total
}

fn main() {
    let web = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "web".into()));
    // Cards named after the folder: those alone, the list and the sources untouched.
    let only: Vec<String> = std::env::args().skip(2).collect();
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
         /// `(English, Spanish)`. Written as `snd!(file, name, what, on)` (`main.rs`): the\n\
         /// WebAssembly leaves the name and what out and reads them from `/unit-text.json`.\n\
         #[cfg_attr(target_arch = \"wasm32\", allow(dead_code))]\n\
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
        if !only.is_empty() && !only.contains(&c) {
            continue;
        }
        let _ = writeln!(code, "    ({}, &[", lit(&c));
        // fix/final-d — Tympanum has no preset list: its card plays its modelled kits.
        if matches!(
            u,
            Unit::Instrument(cantus_core::InstrumentKind::AcousticDrums)
        ) {
            total += tympanum(&c, &dir, &mut code);
            code.push_str("    ]),\n");
            continue;
        }
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
                "        snd!({}, ({}, {}), ({}, {}), {}),",
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
    if !only.is_empty() {
        println!(
            "{} KB of excerpts for {:?} in {}",
            total / 1024,
            only,
            dir.display()
        );
        return;
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
