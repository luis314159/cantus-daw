//! The website's "Try it" (fx side round, `docs/round-fx/site.md`): each guitar unit's factory
//! presets on one take, dry or wet, with the editor's picture for each preset.
//!
//! ```text
//! cargo run --release -p cantus-ui --example render_tryit -- <SampleData> <web> <shots> [unit…] [--presets N] [--takes <dir>] [--content <packs>]
//! python web/tools/tryit_pictures.py <shots>
//! ```
//!
//! - **The take**: Carbonthief's DI strums (CC0, `refs/audio/carbonthief/x/SampleData`) as
//!   Am, F, C, G, a chord a bar at 90 BPM, struck on beats 1 and 3, eight bars — the
//!   progression every guitar example of the round uses. Each preset runs the eight bars on
//!   the engine's own unit (the song playing from bar 1, its latency taken off) and the site
//!   keeps bars 5 to 8: the loop's end runs into the tail its own start carries, so the loop
//!   joins without a seam.
//! - **The level**: every take (the dry one too) is brought to `TARGET` LUFS by a gain alone,
//!   lowered as one where a peak would pass `CEILING`; the manifest says what level the preset
//!   itself left (`level_db`, the wet take against the dry before the gains). MP3, CBR 96
//!   kbit/s at 48 kHz, the size of the site's other excerpts.
//! - **The pictures**: the editor as the preset browser leaves it (`apply_fx_params`, one undo
//!   step) and fed as playing (`demo_feed`), 1200 points wide, in Slate and Paper, English and
//!   Spanish: `<shots>/<unit>/<preset>-{slate,paper}-{en,es}.png`, made WebP by
//!   `web/tools/tryit_pictures.py`.
//! - **The manifest**: `<web>/assets/tryit/<unit>.json` (what `web/src/tryit.rs` reads), and the
//!   takes in `<web>/assets/tryit/<unit>/`; the dry take is shared, `<web>/assets/tryit/guitar.mp3`.
//!
//! Presets are chosen by hand for each unit's range on a guitar ([`UNITS`]; clean, crunch and
//! high gain for Arca, overdrive, distortion, fuzz and muff for Fragor…,
//! `docs/round-fx/sounds.md`) — up to `--presets` (4) a unit. Each must be a factory preset a
//! guitar can show: not the init patch, not one for drums, voice, buses or the master, not one
//! that needs notes from a track (Fractus's played lanes).
//!
//! **The units a guitar does not suit** ([`OWN`]: Loquax, Lenis, Vinculum, Cedo, Purus) are
//! heard on a take of their own from `--takes <dir>` (not in the repository; gathered from the
//! side round's listening sets): `voice.flac` (Britton Rea's *Gettysburg Address*, public
//! domain), the small mix built here (`shared::mix`: Tympanum on Gogodze Phu Vol. II's
//! recorded kit, CC0, read from `--content <packs>`, Gravis on the roots and the CC0 guitar),
//! `pad.wav` (a synthetic pad, 124 BPM) and `faults.wav` (the CC0 guitar with a synthetic hum,
//! buzz, clicks and crackle). Each names its presets; Loquax's synthesiser plays the
//! progression's chords as notes; each has its own dry take, at the same loudness.

#[path = "site_takes/shared.rs"]
#[allow(dead_code)] // the cards' generator uses the rest
mod shared;

use cantus_core::EffectKind;
use cantus_engine::fx::FxNote;
use cantus_ui::i18n::{t_in, Locale};
use cantus_ui::preset_catalog::{entries, Entry, Payload, Unit};
use cantus_ui::preset_tags::Role;
use cantus_ui::raster::shoot;
use cantus_ui::views;
use shared::{
    app_with, bars_5_to_8, lufs, mp3_by, progression, site_gain, through, Values, BARS, BPM,
    GUITAR, MIX, RATE, VOICE,
};
use std::path::{Path, PathBuf};

/// The pictures' width, points.
const WIDTH: f32 = 1200.0;

/// The units, in the panel's order: (the site's name, the kind, `demo_editor`'s name, the
/// presets heard, by the last part of their id — chosen by hand for the unit's range on a
/// guitar, `docs/round-fx/sounds.md`).
const UNITS: &[(&str, EffectKind, &str, &[&str])] = &[
    (
        "fragor",
        EffectKind::Pedal,
        "fragor",
        &["green_push", "rat_rhythm", "vintage_fuzz", "muff_sustain"],
    ),
    (
        "tabula",
        EffectKind::Board,
        "tabula",
        &["guitar_rig", "blues_crunch", "fuzz_lead", "ambient_swells"],
    ),
    (
        "saltus",
        EffectKind::PitchShift,
        "saltus",
        &["twelve_string", "octaves_both", "wide_doubler", "dive_bomb"],
    ),
    (
        "arca",
        EffectKind::Amp,
        "arca",
        &[
            "chime_jangle",
            "british_crunch",
            "full_stack",
            "tight_rhythm",
        ],
    ),
    (
        "rota",
        EffectKind::Rotary,
        "rota",
        &["guitar", "chorale", "tremolo", "open_back"],
    ),
    (
        "cribrum",
        EffectKind::AutoFilter,
        "cribrum",
        &["cry_wah", "funk_wah", "bar_sweep", "talking"],
    ),
    (
        "vetus",
        EffectKind::LoFi,
        "vetus",
        &["worn_cassette", "lofi_hip_hop", "warped_tape", "eight_bit"],
    ),
    (
        "lapsus",
        EffectKind::TimeShaper,
        "lapsus",
        &["half_time", "tape_stop", "reverse_half", "baby_scratch"],
    ),
    (
        "caelum",
        EffectKind::Shimmer,
        "caelum",
        &[
            "guitar_ambience",
            "octave_shimmer",
            "fifth_cathedral",
            "bloom_pad",
        ],
    ),
    (
        "fractus",
        EffectKind::Glitch,
        "fractus",
        &[
            "reverse_build",
            "gated_sixteenths",
            "trance_gate",
            "grain_pad",
        ],
    ),
    (
        "phasis",
        EffectKind::Phaser,
        "phasis",
        &[
            "phase90_script",
            "small_stone",
            "lamp_chorus",
            "synced_eighths",
        ],
    ),
];

/// A unit heard on a take of its own (`--takes`; `@mix` is built, `shared::mix`).
struct Own {
    name: &'static str,
    kind: EffectKind,
    demo: &'static str,
    /// The take's file in `--takes`.
    file: &'static str,
    bpm: f64,
    /// Eight bars, the site looping bars 5 to 8; else the whole take, once.
    looped: bool,
    /// The progression's chords played as notes (a vocoder's synthesiser).
    chords: bool,
    /// The presets heard, by the last part of their id, in this order.
    presets: &'static [&'static str],
    source: (&'static str, &'static str),
    credit: (&'static str, &'static str),
    /// The MP3s' artist tag.
    artist: &'static str,
}

const OWN: &[Own] = &[
    Own {
        name: "loquax",
        kind: EffectKind::Vocoder,
        demo: "loquax",
        file: "voice.flac",
        bpm: BPM,
        looped: false,
        chords: true,
        presets: &["robot", "classic", "choir", "deep_robot"],
        source: (
            "A spoken voice (the Gettysburg Address read by Britton Rea, public domain); the synthesiser plays Am, F, C, G at 90 BPM",
            "Una voz hablada (el discurso de Gettysburg leído por Britton Rea, dominio público); el sintetizador toca Lam, Fa, Do, Sol a 90 BPM",
        ),
        credit: VOICE,
        artist: "Cantus (voice: Britton Rea, public domain)",
    },
    Own {
        name: "lenis",
        kind: EffectKind::ResonanceSuppressor,
        demo: "lenis",
        file: "voice.flac",
        bpm: BPM,
        looped: false,
        chords: false,
        presets: &["podcast_voice", "harsh_vocal", "sibilance", "gentle"],
        source: (
            "A spoken voice (the Gettysburg Address read by Britton Rea, public domain)",
            "Una voz hablada (el discurso de Gettysburg leído por Britton Rea, dominio público)",
        ),
        credit: VOICE,
        artist: "Cantus (voice: Britton Rea, public domain)",
    },
    Own {
        name: "vinculum",
        kind: EffectKind::BusCompressor,
        demo: "vinculum",
        file: "@mix",
        bpm: BPM,
        looped: true,
        chords: false,
        presets: &["glue", "mix_glue", "opto_smooth", "valve_master"],
        source: (
            "A small mix: recorded drums, Cantus's bass and the CC0 guitar, Am, F, C, G at 90 BPM",
            "Una pequeña mezcla: batería grabada, el bajo de Cantus y la guitarra CC0, Lam, Fa, Do, Sol a 90 BPM",
        ),
        credit: MIX,
        artist: "Cantus (guitar: Carbonthief, CC0)",
    },
    Own {
        name: "cedo",
        kind: EffectKind::Ducker,
        demo: "cedo",
        file: "pad.wav",
        bpm: 124.0,
        looped: true,
        chords: false,
        presets: &["classic_pump", "eighths_pump", "triplet_breathe", "trance_gate"],
        source: (
            "A synthetic pad at 124 BPM, ducked by the song's beat (no key routed)",
            "Un pad sintético a 124 BPM, atenuado por el pulso de la canción (sin clave enviada)",
        ),
        credit: ("Pad: synthetic, made by Cantus", "Pad: sintético, hecho por Cantus"),
        artist: "Cantus",
    },
    Own {
        name: "purus",
        kind: EffectKind::Repair,
        demo: "purus",
        file: "faults.wav",
        bpm: BPM,
        looped: true,
        chords: false,
        presets: &["mains_50", "guitar_amp", "vinyl_crackle", "strong"],
        source: (
            "The CC0 guitar with a mains hum (50 Hz, drifting), a buzz, clicks and crackle, synthetic, added to show the repair",
            "La guitarra CC0 con un zumbido de red (50 Hz, que se desvía), un zumbido, clics y crepitar, sintéticos, añadidos para mostrar la reparación",
        ),
        credit: GUITAR,
        artist: "Cantus (guitar: Carbonthief, CC0)",
    },
];

/// Loquax's notes: A minor, F, C, G (the progression), a chord a bar.
const CHORD_NOTES: [[u8; 3]; 4] = [[57, 60, 64], [53, 57, 60], [48, 52, 55], [55, 59, 62]];

/// Bars 5 to 8.
fn second_half(x: &[f32]) -> Vec<f32> {
    bars_5_to_8(x, BPM)
}

/// The take at `gain_db`, as the site's MP3.
fn mp3(l: &[f32], r: &[f32], gain_db: f64, path: &Path, title: &str) -> u64 {
    mp3_by(
        l,
        r,
        gain_db,
        path,
        title,
        "Cantus (guitar: Carbonthief, CC0)",
        "Try it",
    )
}

/// The values a factory preset loads, when it is one the guitar can show.
fn guitar_preset(e: &Entry) -> Option<&Values> {
    let not_guitar = [Role::Drums, Role::Vocal, Role::Bus, Role::Master];
    if not_guitar.contains(&e.role()) {
        return None;
    }
    playable(e)
}

/// The values of a factory preset the take can play: not the init patch, no lanes played by
/// a track's notes (a hand-picked preset may name another role first: a guitar suits it too).
fn playable(e: &Entry) -> Option<&Values> {
    let Payload::Effect(values) = &e.payload else {
        return None;
    };
    if e.id.ends_with(".init") {
        return None;
    }
    // Lanes played by a track's notes: the take has none.
    if e.unit == Unit::Effect(EffectKind::Glitch) && values.get("trig").is_some_and(|v| *v >= 0.5) {
        return None;
    }
    Some(values)
}

/// "Lead — House, Pop" in one language (as the preset browser says it).
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

fn context(dark: bool) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let theme = if dark {
        cantus_ui::theme::slate_night()
    } else {
        cantus_ui::theme::slate_paper()
    };
    cantus_ui::theme::apply(&theme, &ctx);
    ctx
}

fn shoot_preset(dir: &Path, stem: &str, demo: &str, kind: EffectKind, values: &Values) {
    let k = WIDTH / views::fx::frame::designed_size(kind).x;
    for lang in [Locale::En, Locale::Es] {
        for dark in [true, false] {
            cantus_ui::i18n::set_locale(lang);
            let ctx = context(dark);
            let (mut app, _) = app_with(demo, kind, values);
            views::fx::frame::shell_of(kind).set_scale(&ctx, k);
            let size = views::fx::frame::designed_size(kind) * k;
            let path = dir.join(format!(
                "{stem}-{}-{}.png",
                if dark { "slate" } else { "paper" },
                if lang == Locale::En { "en" } else { "es" }
            ));
            shoot(&ctx, size, 1.0, 8, Some(&path), |ui| {
                let ctx = ui.ctx().clone();
                views::fx::window(&mut app, &ctx);
            });
        }
    }
    cantus_ui::i18n::set_locale(Locale::En);
}

/// A take from `--takes`, stereo at 48 kHz, split into its channels (`@mix`: the small mix).
fn take(dir: &Path, file: &str, guitar: &(Vec<f32>, Vec<f32>)) -> (Vec<f32>, Vec<f32>) {
    if file == "@mix" {
        return shared::mix(guitar, -20.0);
    }
    shared::take(&dir.join(file))
}

/// The progression's chords as notes over `frames`, a chord a bar at `bpm`.
fn chord_notes(frames: usize, bpm: f64) -> Vec<(usize, FxNote)> {
    let bar = (4.0 * 60.0 / bpm * f64::from(RATE)) as usize;
    let mut v = Vec::new();
    for (b, at) in (0..frames).step_by(bar).enumerate() {
        let chord = CHORD_NOTES[b % 4];
        for pitch in chord {
            v.push((
                at,
                FxNote::On {
                    pitch,
                    velocity: 100,
                },
            ));
        }
        let end = (at + bar - 1).min(frames.saturating_sub(1));
        for pitch in chord {
            v.push((end, FxNote::Off { pitch }));
        }
    }
    v
}

/// One of [`OWN`]: its dry take, each named preset's take and pictures, and its manifest.
fn render_own(
    own: &Own,
    takes: &Path,
    guitar: &(Vec<f32>, Vec<f32>),
    out: &Path,
    shots: &Path,
    n_presets: usize,
) -> u64 {
    let (mut l, mut r) = take(takes, own.file, guitar);
    if own.looped {
        let n = BARS * 4 * (60.0 / own.bpm * f64::from(RATE)) as usize;
        l.resize(n, 0.0);
        r.resize(n, 0.0);
    }
    let cut = |x: &[f32]| {
        if own.looped {
            bars_5_to_8(x, own.bpm)
        } else {
            x.to_vec()
        }
    };
    let notes = if own.chords {
        chord_notes(l.len(), own.bpm)
    } else {
        Vec::new()
    };
    let dir = out.join(own.name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the unit's folder");
    let pics = shots.join(own.name);
    std::fs::create_dir_all(&pics).expect("the shots' folder");
    let (dl, dr) = (cut(&l), cut(&r));
    let dry_lufs = lufs(&dl, &dr);
    let dry_gain = site_gain(&dl, &dr);
    let mut total = mp3_by(
        &dl,
        &dr,
        dry_gain,
        &dir.join("dry.mp3"),
        "The dry take",
        own.artist,
        "Try it",
    );
    println!("{}: dry ({:.1} LUFS after)", own.name, dry_lufs + dry_gain);
    let all = entries(Unit::Effect(own.kind), false);
    let mut presets = Vec::new();
    for id in own.presets.iter().take(n_presets) {
        let (e, values) = all
            .iter()
            .find_map(|e| match &e.payload {
                Payload::Effect(v) if e.id.rsplit('.').next() == Some(id) => Some((e, v)),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{}: no preset {id}", own.name));
        let (_, slot) = app_with(own.demo, own.kind, values);
        let (wl, wr) = through(&l, &r, &slot, own.bpm, &notes);
        let (wl, wr) = (cut(&wl), cut(&wr));
        let loud = lufs(&wl, &wr);
        let gain = site_gain(&wl, &wr);
        let name_en = t_in(Locale::En, &e.id);
        total += mp3_by(
            &wl,
            &wr,
            gain,
            &dir.join(format!("{id}.mp3")),
            &name_en,
            own.artist,
            "Try it",
        );
        shoot_preset(&pics, id, own.demo, own.kind, values);
        presets.push(serde_json::json!({
            "id": id,
            "name": [name_en, t_in(Locale::Es, &e.id)],
            "what": [what(e, Locale::En), what(e, Locale::Es)],
            "wet": format!("/tryit/{}/{id}.mp3", own.name),
            "level_db": ((loud - dry_lufs) * 10.0).round() / 10.0,
            "pic": format!("/tryit/{}/{id}", own.name),
        }));
        println!(
            "{}: {name_en} ({:+.1} dB as the preset leaves it, {:.1} LUFS after)",
            own.name,
            loud - dry_lufs,
            loud + gain
        );
    }
    let manifest = serde_json::json!({
        "unit": own.name,
        "origin": "presets",
        "note": [
            "Rendered by Cantus from its factory presets on one take, each matched to one loudness by a gain alone.",
            "Renderizado por Cantus desde sus presets de fábrica sobre una toma, cada uno igualado a una sonoridad solo con una ganancia."
        ],
        "source": [own.source.0, own.source.1],
        "credit": [own.credit.0, own.credit.1],
        "loop": own.looped,
        "dry": format!("/tryit/{}/dry.mp3", own.name),
        "presets": presets,
    });
    let text = serde_json::to_string_pretty(&manifest).expect("json") + "\n";
    std::fs::write(out.join(format!("{}.json", own.name)), text).expect("the manifest");
    total
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let n_presets = args
        .iter()
        .position(|a| a == "--presets")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(4usize);
    let mut pos = args
        .iter()
        .enumerate()
        .filter(|(i, a)| {
            !a.starts_with("--")
                && (*i == 0
                    || !["--presets", "--takes", "--content"].contains(&args[i - 1].as_str()))
        })
        .map(|(_, a)| a.clone());
    let samples = PathBuf::from(pos.next().expect("the SampleData folder"));
    let web = PathBuf::from(pos.next().unwrap_or_else(|| "web".into()));
    let shots = PathBuf::from(pos.next().unwrap_or_else(|| "target/shots/tryit".into()));
    let only: Vec<String> = pos.collect();
    let takes = args
        .iter()
        .position(|a| a == "--takes")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);
    // The installed packs Tympanum's recorded kit is read from (the small mix's drums).
    if let Some(root) = args
        .iter()
        .position(|a| a == "--content")
        .and_then(|i| args.get(i + 1))
    {
        cantus_ui::tympanum_load::install(PathBuf::from(root));
    }
    cantus_ui::i18n::set_thread_locale(Locale::En);

    let (l, r) = progression(&samples);
    let (dl, dr) = (second_half(&l), second_half(&r));
    let dry_lufs = lufs(&dl, &dr);
    let out = web.join("assets/tryit");
    std::fs::create_dir_all(&out).expect("assets/tryit");
    let dry_gain = site_gain(&dl, &dr);
    let mut total = mp3(&dl, &dr, dry_gain, &out.join("guitar.mp3"), "The DI guitar");

    for (name, kind, demo, picks) in UNITS {
        if !only.is_empty() && !only.iter().any(|o| o == name) {
            continue;
        }
        let dir = out.join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("the unit's folder");
        let pics = shots.join(name);
        std::fs::create_dir_all(&pics).expect("the shots' folder");
        let all = entries(Unit::Effect(*kind), false);
        // The hand-picked presets in their order; a unit without picks, its first guitar ones.
        let mut chosen: Vec<(&Entry, &Values)> = picks
            .iter()
            .map(|id| {
                all.iter()
                    .find(|e| e.id.rsplit('.').next() == Some(id))
                    .and_then(|e| playable(e).map(|v| (e, v)))
                    .unwrap_or_else(|| panic!("{name}: no guitar preset {id}"))
            })
            .collect();
        if chosen.is_empty() {
            chosen = all
                .iter()
                .filter_map(|e| guitar_preset(e).map(|v| (e, v)))
                .collect();
        }
        chosen.truncate(n_presets);
        let mut presets = Vec::new();
        for (e, values) in chosen {
            let (_, slot) = app_with(demo, *kind, values);
            let (wl, wr) = through(&l, &r, &slot, BPM, &[]);
            let (wl, wr) = (second_half(&wl), second_half(&wr));
            let loud = lufs(&wl, &wr);
            let gain = site_gain(&wl, &wr);
            let stem = e.id.rsplit('.').next().unwrap_or("preset").to_string();
            let name_en = t_in(Locale::En, &e.id);
            total += mp3(&wl, &wr, gain, &dir.join(format!("{stem}.mp3")), &name_en);
            shoot_preset(&pics, &stem, demo, *kind, values);
            presets.push(serde_json::json!({
                "id": stem,
                "name": [name_en, t_in(Locale::Es, &e.id)],
                "what": [what(e, Locale::En), what(e, Locale::Es)],
                "wet": format!("/tryit/{name}/{stem}.mp3"),
                "level_db": ((loud - dry_lufs) * 10.0).round() / 10.0,
                "pic": format!("/tryit/{name}/{stem}"),
            }));
            println!(
                "{name}: {name_en} ({:+.1} dB as the preset leaves it, {:.1} LUFS after)",
                loud - dry_lufs,
                loud + gain
            );
        }
        let manifest = serde_json::json!({
            "unit": name,
            "origin": "presets",
            "note": [
                "Rendered by Cantus from its factory presets on one take, each matched to one loudness by a gain alone.",
                "Renderizado por Cantus desde sus presets de fábrica sobre una toma, cada uno igualado a una sonoridad solo con una ganancia."
            ],
            "source": [
                "Four chords (Am, F, C, G at 90 BPM) on Carbonthief's DI guitar",
                "Cuatro acordes (Lam, Fa, Do, Sol a 90 BPM) en la guitarra DI de Carbonthief"
            ],
            "credit": [
                "Guitar: Carbonthief, CC0 (Strummed Guitar Chords, DI)",
                "Guitarra: Carbonthief, CC0 (Strummed Guitar Chords, DI)"
            ],
            "loop": true,
            "dry": "/tryit/guitar.mp3",
            "presets": presets,
        });
        let text = serde_json::to_string_pretty(&manifest).expect("json") + "\n";
        std::fs::write(out.join(format!("{name}.json")), text).expect("the manifest");
    }
    if let Some(takes) = takes {
        for own in OWN {
            if !only.is_empty() && !only.iter().any(|o| o == own.name) {
                continue;
            }
            total += render_own(
                own,
                &takes,
                &(l.clone(), r.clone()),
                &out,
                &shots,
                n_presets,
            );
        }
    }
    println!("{} KB of takes in {}", total / 1024, out.display());
}
