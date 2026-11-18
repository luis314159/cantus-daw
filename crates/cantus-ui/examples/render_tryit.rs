//! The website's "Try it" (fx side round, `docs/round-fx/site.md`): each guitar unit's factory
//! presets on one take, dry or wet, with the editor's picture for each preset.
//!
//! ```text
//! cargo run --release -p cantus-ui --example render_tryit -- <SampleData> <web> <shots> [unit…] [--presets N] [--takes <dir>]
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
//! Presets are each unit's factory presets in their order, without the init patch, those for
//! drums, voice, buses and the master left out (the take is a guitar), and those that need
//! notes from a track (Fractus's played lanes) — up to `--presets` (4) a unit.
//!
//! **The units a guitar does not suit** ([`OWN`]: Loquax, Lenis, Vinculum, Cedo, Purus) are
//! heard on a take of their own from `--takes <dir>` (not in the repository; gathered from the
//! side round's listening sets): `voice.flac` (Britton Rea's *Gettysburg Address*, public
//! domain), `mix.wav` (Cantus's Pulsus drums and Gravis bass under the CC0 guitar, 90 BPM),
//! `pad.wav` (a synthetic pad, 124 BPM) and `faults.wav` (the CC0 guitar with a synthetic hum,
//! buzz, clicks and crackle). Each names its presets; Loquax's synthesiser plays the
//! progression's chords as notes; each has its own dry take, at the same loudness.

use cantus_core::{EffectId, EffectKind, EffectSlot};
use cantus_engine::fx::{FxNote, SongPosition};
use cantus_engine::{EffectMemory, EffectUnit, EngineHandle};
use cantus_export::job::Control;
use cantus_export::settings::Tags;
use cantus_export::{export_rendered, Audio, ExportSettings, Format, Mp3Mode, Rate};
use cantus_ui::i18n::{t_in, Locale};
use cantus_ui::preset_catalog::{entries, Entry, Payload, Unit};
use cantus_ui::preset_tags::Role;
use cantus_ui::raster::shoot;
use cantus_ui::{views, welcome_project, CantusApp};
use std::path::{Path, PathBuf};

const RATE: u32 = 48_000;
const BPM: f64 = 90.0;
const BARS: usize = 8;
/// Every take's loudness, LUFS (the site's stand-ins were made at the same).
const TARGET: f64 = -19.3;
/// No take's peak above this, dBFS (MP3 adds a little on decoding).
const CEILING: f64 = -1.5;
/// The pictures' width, points.
const WIDTH: f32 = 1200.0;

/// The units, in the panel's order: (the site's name, the kind, `demo_editor`'s name).
const UNITS: &[(&str, EffectKind, &str)] = &[
    ("fragor", EffectKind::Pedal, "fragor"),
    ("tabula", EffectKind::Board, "tabula"),
    ("saltus", EffectKind::PitchShift, "saltus"),
    ("arca", EffectKind::Amp, "arca"),
    ("rota", EffectKind::Rotary, "rota"),
    ("cribrum", EffectKind::AutoFilter, "cribrum"),
    ("vetus", EffectKind::LoFi, "vetus"),
    ("lapsus", EffectKind::TimeShaper, "lapsus"),
    ("caelum", EffectKind::Shimmer, "caelum"),
    ("fractus", EffectKind::Glitch, "fractus"),
    ("phasis", EffectKind::Phaser, "phasis"),
];

/// A unit heard on a take of its own (`--takes`).
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

const VOICE: (&str, &str) = (
    "Voice: Britton Rea, public domain (Wikimedia Commons)",
    "Voz: Britton Rea, dominio público (Wikimedia Commons)",
);
const GUITAR: (&str, &str) = (
    "Guitar: Carbonthief, CC0 (Strummed Guitar Chords, DI)",
    "Guitarra: Carbonthief, CC0 (Strummed Guitar Chords, DI)",
);

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
        file: "mix.wav",
        bpm: BPM,
        looped: true,
        chords: false,
        presets: &["glue", "mix_glue", "opto_smooth", "valve_master"],
        source: (
            "A small mix: Cantus's own drums and bass under the CC0 guitar, 90 BPM",
            "Una pequeña mezcla: la batería y el bajo de Cantus bajo la guitarra CC0, 90 BPM",
        ),
        credit: (
            "Guitar: Carbonthief, CC0 (Strummed Guitar Chords, DI); drums and bass: Cantus's Pulsus and Gravis",
            "Guitarra: Carbonthief, CC0 (Strummed Guitar Chords, DI); batería y bajo: Pulsus y Gravis de Cantus",
        ),
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
            "The CC0 guitar with a synthetic mains hum (50 Hz, drifting), a buzz, clicks and crackle",
            "La guitarra CC0 con un zumbido de red sintético (50 Hz, que se desvía), un zumbido, clics y crepitar",
        ),
        credit: GUITAR,
        artist: "Cantus (guitar: Carbonthief, CC0)",
    },
];

/// Loquax's notes: A minor, F, C, G (the progression), a chord a bar.
const CHORD_NOTES: [[u8; 3]; 4] = [[57, 60, 64], [53, 57, 60], [48, 52, 55], [55, 59, 62]];

/// The chord of each bar (`refs/audio/README.md`).
const CHORDS: [&str; 4] = [
    "Sample07 (A Minor)",
    "Sample15 (F Major)",
    "Sample22 (C Major)",
    "Sample17 (G Major)",
];

/// One strum, stereo interleaved at 48 kHz (the `.flac`, or a `.wav` made from it where the
/// decoder cannot read the FLAC — Carbonthief's are 32-bit).
fn strum(dir: &Path, stem: &str) -> Vec<f32> {
    let d = ["flac", "wav"]
        .iter()
        .find_map(|ext| {
            let p = dir.join(format!("{stem}.{ext}"));
            p.exists()
                .then(|| cantus_engine::SampleCache::decode_source(&p).ok())
                .flatten()
        })
        .unwrap_or_else(|| panic!("{stem}: not in {}", dir.display()));
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

/// The eight bars, two strums a bar, each faded over 20 ms before the next, peak −6 dBFS.
fn progression(dir: &Path) -> (Vec<f32>, Vec<f32>) {
    let beat = (60.0 / BPM * f64::from(RATE)) as usize;
    let n = BARS * 4 * beat;
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

/// The take through `slot` on the engine's own unit, the song playing from bar 1, its
/// latency taken off (aligned with the dry take).
fn through(
    l: &[f32],
    r: &[f32],
    slot: &EffectSlot,
    bpm: f64,
    notes: &[(usize, FxNote)],
) -> (Vec<f32>, Vec<f32>) {
    let mut u = EffectUnit::new(slot, RATE as f32);
    u.configure(&slot.spec);
    if let Some(need) = u.memory_need() {
        drop(u.supply(EffectMemory::build(need, RATE as f32)));
    }
    let lat = u.latency();
    let (mut ol, mut or) = (l.to_vec(), r.to_vec());
    ol.extend(std::iter::repeat_n(0.0, lat));
    or.extend(std::iter::repeat_n(0.0, lat));
    let mut at = 0usize;
    while at < ol.len() {
        let n = 480.min(ol.len() - at);
        for (when, note) in notes.iter().filter(|(w, _)| (at..at + n).contains(w)) {
            u.note(when - at, *note);
        }
        u.set_position(&SongPosition {
            playing: true,
            tempo_bpm: bpm,
            tempo_now: bpm,
            beats: at as f64 * bpm / 60.0 / f64::from(RATE),
            samples: at as i64,
            ..Default::default()
        });
        u.process(&mut ol[at..at + n], &mut or[at..at + n]);
        at += n;
    }
    (ol[lat..].to_vec(), or[lat..].to_vec())
}

/// Bars 5 to 8.
fn second_half(x: &[f32]) -> Vec<f32> {
    bars_5_to_8(x, BPM)
}

/// Bars 5 to 8 at `bpm`.
fn bars_5_to_8(x: &[f32], bpm: f64) -> Vec<f32> {
    let half = BARS / 2 * 4 * (60.0 / bpm * f64::from(RATE)) as usize;
    x[half..(2 * half).min(x.len())].to_vec()
}

fn lufs(l: &[f32], r: &[f32]) -> f64 {
    let mut m = cantus_dsp::terminus::loudness::LoudnessMeter::new(RATE as f32);
    m.process(l, r);
    m.integrated().unwrap_or(-120.0)
}

fn peak_db(l: &[f32], r: &[f32]) -> f64 {
    let p = l
        .iter()
        .chain(r)
        .fold(0f32, |m, v| m.max(v.abs()))
        .max(1e-9);
    20.0 * cantus_dsp::math::log10(f64::from(p))
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
    )
}

/// [`mp3`] with its artist tag.
fn mp3_by(l: &[f32], r: &[f32], gain_db: f64, path: &Path, title: &str, artist: &str) -> u64 {
    let g = cantus_dsp::math::db_to_gain(gain_db as f32);
    let samples: Vec<f32> = l
        .iter()
        .zip(r)
        .flat_map(|(a, b)| [*a * g, *b * g])
        .collect();
    let s = ExportSettings {
        format: Format::Mp3,
        mp3: Mp3Mode::Cbr(96),
        rate: Rate::Hz(RATE),
        ..Default::default()
    };
    let tags = Tags {
        title: title.into(),
        artist: artist.into(),
        album: "Try it".into(),
        ..Default::default()
    };
    let audio = Audio {
        rate: RATE,
        channels: 2,
        samples,
    };
    export_rendered(audio, &s, &tags, path, &Control::new()).expect("the take is written");
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

type Values = std::collections::BTreeMap<String, f32>;

/// The values a factory preset loads, when it is one the guitar can show.
fn guitar_preset(e: &Entry) -> Option<&Values> {
    let Payload::Effect(values) = &e.payload else {
        return None;
    };
    let not_guitar = [Role::Drums, Role::Vocal, Role::Bus, Role::Master];
    if e.id.ends_with(".init") || not_guitar.contains(&e.role()) {
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

/// The app with the unit's editor open, fed as playing, the preset loaded as the browser
/// loads it; and the slot as it is then.
fn app_with(demo: &str, kind: EffectKind, values: &Values) -> (CantusApp, EffectSlot) {
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor(demo);
    views::fx::demo_feed(&mut app, kind);
    let (strip, effect, _) = app.fx.open.expect("the editor is open");
    app.apply_fx_params(
        strip,
        effect,
        values,
        cantus_ui::preset_browser::APPLY_LABEL,
    );
    app.fx.pos = Some(egui::pos2(0.0, 0.0));
    let mut slot = app
        .project()
        .effect(strip, effect)
        .cloned()
        .expect("the slot");
    // Heard alone on the take: no key from another track, no notes, all wet.
    slot.id = EffectId(1);
    slot.bypass = false;
    slot.mix = 1.0;
    slot.sidechain = None;
    slot.midi_from = None;
    (app, slot)
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

/// A take from `--takes`, stereo at 48 kHz, split into its channels.
fn take(dir: &Path, file: &str) -> (Vec<f32>, Vec<f32>) {
    let p = dir.join(file);
    let d = cantus_engine::SampleCache::decode_source(&p)
        .unwrap_or_else(|e| panic!("{}: {e:?}", p.display()));
    let ch = d.channels as usize;
    let mut x: Vec<f32> = d.frames.to_vec();
    if d.rate != RATE {
        x = cantus_engine::samples::resample::resample(&x, ch, d.rate, RATE);
    }
    if ch == 1 {
        (x.clone(), x)
    } else {
        (
            x.iter().step_by(ch).copied().collect(),
            x.iter().skip(1).step_by(ch).copied().collect(),
        )
    }
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
fn render_own(own: &Own, takes: &Path, out: &Path, shots: &Path, n_presets: usize) -> u64 {
    let (mut l, mut r) = take(takes, own.file);
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
    let dry_gain = (TARGET - dry_lufs).min(CEILING - peak_db(&dl, &dr));
    let mut total = mp3_by(
        &dl,
        &dr,
        dry_gain,
        &dir.join("dry.mp3"),
        "The dry take",
        own.artist,
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
        let gain = (TARGET - loud).min(CEILING - peak_db(&wl, &wr));
        let name_en = t_in(Locale::En, &e.id);
        total += mp3_by(
            &wl,
            &wr,
            gain,
            &dir.join(format!("{id}.mp3")),
            &name_en,
            own.artist,
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
                && (*i == 0 || !["--presets", "--takes"].contains(&args[i - 1].as_str()))
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
    cantus_ui::i18n::set_thread_locale(Locale::En);

    let (l, r) = progression(&samples);
    let (dl, dr) = (second_half(&l), second_half(&r));
    let dry_lufs = lufs(&dl, &dr);
    let out = web.join("assets/tryit");
    std::fs::create_dir_all(&out).expect("assets/tryit");
    let dry_gain = (TARGET - dry_lufs).min(CEILING - peak_db(&dl, &dr));
    let mut total = mp3(&dl, &dr, dry_gain, &out.join("guitar.mp3"), "The DI guitar");

    for (name, kind, demo) in UNITS {
        if !only.is_empty() && !only.iter().any(|o| o == name) {
            continue;
        }
        let dir = out.join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("the unit's folder");
        let pics = shots.join(name);
        std::fs::create_dir_all(&pics).expect("the shots' folder");
        let all = entries(Unit::Effect(*kind), false);
        let chosen: Vec<(&Entry, &Values)> = all
            .iter()
            .filter_map(|e| guitar_preset(e).map(|v| (e, v)))
            .take(n_presets)
            .collect();
        let mut presets = Vec::new();
        for (e, values) in chosen {
            let (_, slot) = app_with(demo, *kind, values);
            let (wl, wr) = through(&l, &r, &slot, BPM, &[]);
            let (wl, wr) = (second_half(&wl), second_half(&wr));
            let loud = lufs(&wl, &wr);
            let gain = (TARGET - loud).min(CEILING - peak_db(&wl, &wr));
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
            total += render_own(own, &takes, &out, &shots, n_presets);
        }
    }
    println!("{} KB of takes in {}", total / 1024, out.display());
}
