//! The website's preset excerpts (round 37, ADR 0101 §8): for every unit's card, a few of its
//! factory presets rendered by the program on what the preset browser auditions them on —
//! an instrument on its role's phrase, an effect on Cantus's own source for its role (the
//! dry source too, to compare) — at the presets' own levels (the render check holds them
//! within ±2 LU of their role), with no processing after, as MP3. It writes the files into
//! `<web>/assets/units/sounds/` and their list into `<web>/src/units_sounds.rs`.
//!
//! `cargo run --release -p cantus-ui --example render_unit_sounds -- web`
//!
//! **On recordings** (`docs/round-fx/sounds.md`): with `--guitar <SampleData> --takes <dir>
//! --content <packs>`, an effect is heard on a recording instead — Carbonthief's DI guitar
//! (CC0), Gogodze Phu Vol. II's recorded drums (CC0) played by Tympanum, Britton Rea's voice
//! (public domain), the Salamander grand (CC BY 3.0) played by Chorda's recorded source, a pad
//! of Calor's or a small mix of the drums, Gravis and the guitar — chosen by the preset's
//! role and the unit ([`source_for`]), each excerpt and its dry take matched to one loudness
//! by a gain alone, the source's credit beside them. A unit with a Try it manifest
//! (`<web>/assets/tryit/<unit>.json`; run `render_tryit` first) lists those takes instead.
//! `--effects` renders the effects only and keeps every other card's excerpts as they are.
//!
//! `-- web chorda` (any card names after the folder) renders those cards' excerpts alone, puts
//! their blocks into `units_sounds.rs` (a new card in its place in the picker's order) and
//! leaves every other card's block and the dry sources as they are (a unit re-voiced or a new
//! card: its files only; with `--guitar`/`--takes`, the recordings an effect's card is heard
//! on are still made).

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
#[path = "site_takes/shared.rs"]
#[allow(dead_code)] // Try it's generator uses the rest
mod shared;

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
            cantus_core::InstrumentKind::Resonator => "fides",        // r41:contract
            cantus_core::InstrumentKind::BowedString => "arcus",      // r42:contract
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

/// Units heard on the synthetic sources still: a pitch corrector and a vibrato need a sung,
/// pitched line, which none of the recordings is.
const SYNTHETIC: &[&str] = &["tonus", "vibrans"];
/// Units whose natural source is a guitar (a preset for drums or a voice still goes to
/// those): the delay, the tremolo, the gate and the attack shapers.
const GUITAR_UNITS: &[&str] = &["delay", "tremor", "porta", "impetus", "discrimen"];
/// Reverbs, heard on a recorded piano (the space shows on its decays).
const PIANO_UNITS: &[&str] = &["spatium", "aula"];
/// Effects made for synthesisers (chorus, flanger, ring and frequency shifting), heard on a
/// pad of Cantus's own.
const PAD_UNITS: &[&str] = &["concentus", "flexus", "anulus", "translatio"];
/// Units with no excerpt: Comparatio matches loudness to a reference, which a short excerpt
/// matched to one loudness cannot show honestly; its card shows its window.
const SILENT: &[&str] = &["comparatio"];

type Pair = (String, String);

/// A recorded source: its id (`Sound::on`), the dry file, what it is and its credit.
struct Real {
    id: &'static str,
    file: String,
    name: Pair,
    credit: Pair,
    /// Eight bars at 90 BPM, as the effect is fed.
    take: shared::Stereo,
    /// The excerpt is bars 5 to 8 (a loop's second half), else bars 1 to 4 (a voice from
    /// its first word).
    second_half: bool,
}

/// Which recording a preset is heard on, or `None` for the synthetic source of its role.
fn source_for(card: &str, e: &Entry) -> Option<&'static str> {
    if SYNTHETIC.contains(&card) {
        return None;
    }
    Some(match e.role() {
        Role::Drums => "kit",
        Role::Vocal => "voice",
        _ if GUITAR_UNITS.contains(&card) || e.id.contains("guitar") => "guitar",
        _ if PIANO_UNITS.contains(&card) => "piano",
        _ if PAD_UNITS.contains(&card) => "pad",
        _ => "mix",
    })
}

/// The loudness each recording is fed at, LUFS: where such a track sits in a production — a
/// drum bus peaking near full scale, a pre-master mix, a voice, a piano and a pad as tracked —
/// so dynamics meet the level they act on. (Cantus's own audition sources sit 8 dB down, where
/// a compressor or a clipper does not act at all.)
const FED_DRUMS: f64 = -18.0;
const FED_MIX: f64 = -16.0;
const FED_TRACK: f64 = -20.0;

/// Below this, an excerpt differs from its dry take too little to hear (dB, the gain-matched
/// difference against the dry take): it is left off the card.
const AUDIBLE_DB: f64 = -30.0;

fn cut(x: &[f32], second_half: bool) -> Vec<f32> {
    if second_half {
        shared::bars_5_to_8(x, shared::BPM)
    } else {
        let n = shared::eight_bars() / 2;
        x[..n.min(x.len())].to_vec()
    }
}

fn pair(a: &str, b: &str) -> Pair {
    (a.to_string(), b.to_string())
}

/// Calor's *Velvet pad*, the factory patch.
fn calor_pad() -> cantus_core::NativeSpec {
    entries(Unit::Instrument(cantus_core::InstrumentKind::Analog), false)
        .into_iter()
        .find_map(|e| match e.payload {
            Payload::Native(spec) if e.id == "calor.preset.velvet_pad" => Some(spec),
            _ => None,
        })
        .expect("Calor's velvet pad")
}

/// The recordings, each with its dry take written as `<dir>/source-<id>.mp3` (the guitar's
/// is Try it's, `/tryit/guitar.mp3`).
fn reals(guitar_dir: &Path, takes: &Path, dir: &Path, total: &mut u64) -> Vec<Real> {
    let guitar = shared::progression(guitar_dir);
    let mut voice = shared::take(&takes.join("voice.flac"));
    voice.0.resize(shared::eight_bars(), 0.0);
    voice.1.resize(shared::eight_bars(), 0.0);
    shared::set_lufs(&mut voice.0, &mut voice.1, FED_TRACK);
    let mut piano = shared::take(&takes.join("piano.flac"));
    piano.0.resize(shared::eight_bars(), 0.0);
    piano.1.resize(shared::eight_bars(), 0.0);
    shared::set_lufs(&mut piano.0, &mut piano.1, FED_TRACK);
    let out = vec![
        Real {
            id: "guitar",
            file: "/tryit/guitar".into(),
            name: pair(
                "four chords (Am, F, C, G at 90 BPM) on a DI electric guitar",
                "cuatro acordes (Lam, Fa, Do, Sol a 90 BPM) en una guitarra eléctrica DI",
            ),
            credit: pair(shared::GUITAR.0, shared::GUITAR.1),
            take: guitar.clone(),
            second_half: true,
        },
        Real {
            id: "kit",
            file: "source-kit".into(),
            name: pair(
                "a recorded drum kit, rock at 90 BPM",
                "una batería grabada, rock a 90 BPM",
            ),
            credit: pair(shared::DRUMS.0, shared::DRUMS.1),
            take: shared::drums(FED_DRUMS),
            second_half: true,
        },
        Real {
            id: "voice",
            file: "source-voice".into(),
            name: pair("a spoken voice", "una voz hablada"),
            credit: pair(shared::VOICE.0, shared::VOICE.1),
            take: voice,
            second_half: false,
        },
        Real {
            id: "piano",
            file: "source-piano".into(),
            name: pair("a recorded grand piano", "un piano de cola grabado"),
            credit: pair(shared::PIANO.0, shared::PIANO.1),
            take: piano,
            second_half: false,
        },
        Real {
            id: "pad",
            file: "source-pad".into(),
            name: pair(
                "a synthesiser pad holding Am, F, C, G at 90 BPM",
                "un pad de sintetizador sosteniendo Lam, Fa, Do, Sol a 90 BPM",
            ),
            credit: pair(shared::PAD.0, shared::PAD.1),
            take: shared::pad(calor_pad(), FED_TRACK),
            second_half: true,
        },
        Real {
            id: "mix",
            file: "source-mix".into(),
            name: pair(
                "a small mix: recorded drums, bass and the DI guitar",
                "una pequeña mezcla: batería grabada, bajo y la guitarra DI",
            ),
            credit: pair(shared::MIX.0, shared::MIX.1),
            take: shared::mix(&guitar, FED_MIX),
            second_half: true,
        },
    ];
    for r in &out {
        println!(
            "source-{}: fed at {:.1} LUFS",
            r.id,
            shared::lufs(&r.take.0, &r.take.1)
        );
        if r.file.starts_with('/') {
            continue;
        }
        let (l, rr) = (cut(&r.take.0, r.second_half), cut(&r.take.1, r.second_half));
        let g = shared::site_gain(&l, &rr);
        *total += shared::mp3_by(
            &l,
            &rr,
            g,
            &dir.join(format!("{}.mp3", r.file)),
            "Dry source",
            &format!("Cantus ({})", r.credit.0),
            "Factory presets",
        );
    }
    out
}

/// A preset on a recording: the excerpt, matched to the dry take's loudness by a gain alone;
/// its size (0 when it is too close to the dry take to hear, and so not written), the level
/// the preset itself left against the dry and how far apart they are (dB).
fn on_real(
    kind: EffectKind,
    e: &Entry,
    real: &Real,
    path: &Path,
    title: &str,
) -> Option<(u64, f64, f64)> {
    let Payload::Effect(values) = &e.payload else {
        return None;
    };
    let mut scratch = cantus_core::Project::new("scratch");
    let mut slot = scratch.make_effect(kind);
    slot.spec.params = aud::effect_params_after(&slot.spec, values);
    slot.mix = 1.0;
    let (wl, wr) = shared::through(&real.take.0, &real.take.1, &slot, shared::BPM, &[]);
    let (wl, wr) = (cut(&wl, real.second_half), cut(&wr, real.second_half));
    let dl = cut(&real.take.0, real.second_half);
    let dr = cut(&real.take.1, real.second_half);
    let level = shared::lufs(&wl, &wr) - shared::lufs(&dl, &dr);
    let g = shared::site_gain(&wl, &wr);
    // How far the excerpt is from the dry take, both at the site's loudness.
    let (gw, gd) = (
        cantus_dsp::math::db_to_gain(g as f32),
        cantus_dsp::math::db_to_gain(shared::site_gain(&dl, &dr) as f32),
    );
    let (mut diff, mut dry) = (0f64, 0f64);
    for i in 0..wl.len().min(dl.len()) {
        for (w, d) in [(wl[i], dl[i]), (wr[i], dr[i])] {
            diff += f64::from(w * gw - d * gd).powi(2);
            dry += f64::from(d * gd).powi(2);
        }
    }
    let apart = 10.0 * cantus_dsp::math::log10((diff / dry.max(1e-12)).max(1e-12));
    if apart < AUDIBLE_DB {
        return Some((0, level, apart));
    }
    let size = shared::mp3_by(
        &wl,
        &wr,
        g,
        path,
        title,
        &format!("Cantus ({})", real.credit.0),
        "Factory presets",
    );
    Some((size, level, apart))
}

/// The excerpts already in `units_sounds.rs`, by card (each block's text).
fn kept_blocks(web: &Path) -> std::collections::BTreeMap<String, String> {
    let text = std::fs::read_to_string(web.join("src/units_sounds.rs")).unwrap_or_default();
    blocks(&text).into_iter().collect()
}

/// The cards' blocks of a `units_sounds.rs` text, in its order.
fn blocks(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut cur: Option<(String, String)> = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("    (\"") {
            if line.ends_with("&[") {
                let name = rest.split('"').next().unwrap_or_default().to_string();
                cur = Some((name, format!("{line}\n")));
                continue;
            }
        }
        if let Some((_, body)) = cur.as_mut() {
            body.push_str(line);
            body.push('\n');
            if line == "    ])," {
                out.push(cur.take().expect("a block"));
            }
        }
    }
    out
}

/// `-- web nitor` (r41): the named cards' new blocks put into `units_sounds.rs` — each in
/// its place, a new card after the card before it in the picker's order — and every other
/// card's block, the dry sources and their list left as they are. A named card heard on a
/// source the list does not hold yet stops it: render every card then.
fn splice(web: &Path, code: &str, order: &[String], used: &[Used]) {
    let path = web.join("src/units_sounds.rs");
    let read = std::fs::read_to_string(&path).expect("units_sounds.rs");
    // A checkout with `core.autocrlf` has CRLF lines: read as LF, written back as it was.
    let crlf = read.contains("\r\n");
    let text = read.replace("\r\n", "\n");
    const HEAD: &str = "pub const SOUNDS: &[(&str, &[Sound])] = &[\n";
    const TAIL: &str = "];\n\n/// The dry sources";
    let (head, rest) = text.split_at(text.find(HEAD).expect("SOUNDS") + HEAD.len());
    let (_, tail) = rest.split_at(rest.find(TAIL).expect("SOURCES"));
    for (id, ..) in used {
        assert!(
            tail.contains(&format!("    ({},", lit(id))),
            "the source {id} is not in units_sounds.rs yet: render every card"
        );
    }
    let mut all = blocks(&text);
    for (name, body) in blocks(code) {
        if let Some(b) = all.iter_mut().find(|(n, _)| *n == name) {
            b.1 = body;
            continue;
        }
        let before = order
            .iter()
            .position(|c| *c == name)
            .map(|i| &order[..i])
            .unwrap_or_default();
        let at = all
            .iter()
            .rposition(|(n, _)| before.contains(n))
            .map_or(0, |i| i + 1);
        all.insert(at, (name, body));
    }
    let middle: String = all.into_iter().map(|(_, b)| b).collect();
    let mut out = format!("{head}{middle}{tail}");
    if crlf {
        out = out.replace('\n', "\r\n");
    }
    std::fs::write(&path, out).expect("the list");
}

/// A Try it manifest's source: `on`, its dry file, what it is, its credit.
struct TrySource {
    on: String,
    dry: String,
    name: Pair,
    credit: Pair,
}

/// A Try it take: its file, the preset's name and what it is for.
type TryTake = (String, Pair, Pair);

/// A Try it manifest's takes as the card's excerpts: its source and each preset's take.
fn tryit(web: &Path, card: &str) -> Option<(TrySource, Vec<TryTake>)> {
    let text = std::fs::read_to_string(web.join(format!("assets/tryit/{card}.json"))).ok()?;
    let m: serde_json::Value = serde_json::from_str(&text).ok()?;
    let two = |v: &serde_json::Value| {
        pair(
            v[0].as_str().unwrap_or_default(),
            v[1].as_str().unwrap_or_default(),
        )
    };
    if m["origin"] != "presets" {
        return None;
    }
    let dry = m["dry"].as_str()?.trim_end_matches(".mp3").to_string();
    let on = if dry == "/tryit/guitar" {
        "guitar".to_string()
    } else {
        format!("tryit-{card}")
    };
    let said = if m["dry_source"].is_array() {
        &m["dry_source"]
    } else {
        &m["source"]
    };
    let (mut en, mut es) = two(said);
    for s in [&mut en, &mut es] {
        // The page says "Without the effect · <source>": a lower-case start reads on.
        if let Some(c) = s.chars().next() {
            *s = c.to_lowercase().collect::<String>() + &s[c.len_utf8()..];
        }
    }
    let presets = m["presets"]
        .as_array()?
        .iter()
        .map(|p| {
            (
                p["wet"]
                    .as_str()
                    .unwrap_or_default()
                    .trim_end_matches(".mp3")
                    .to_string(),
                two(&p["name"]),
                two(&p["what"]),
            )
        })
        .collect();
    let source = TrySource {
        on,
        dry,
        name: (en, es),
        credit: two(&m["credit"]),
    };
    Some((source, presets))
}

/// A dry source heard: (id, file, what it is, its credit, matched by loudness).
type Used = (String, String, Pair, Pair, bool);

fn use_source(used: &mut Vec<Used>, id: &str, file: &str, name: Pair, credit: Pair, matched: bool) {
    if !used.iter().any(|u| u.0 == id) {
        used.push((id.into(), file.into(), name, credit, matched));
    }
}

fn line(code: &mut String, file: &str, name: &Pair, what: &Pair, on: &str) {
    let _ = writeln!(
        code,
        "        snd!({}, ({}, {}), ({}, {}), {}),",
        lit(file),
        lit(&name.0),
        lit(&name.1),
        lit(&what.0),
        lit(&what.1),
        lit(on)
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let opt = |k: &str| {
        args.iter()
            .position(|a| a == k)
            .and_then(|i| args.get(i + 1))
            .map(PathBuf::from)
    };
    let flags = ["--guitar", "--takes", "--content"];
    let web = args
        .iter()
        .enumerate()
        .find(|(i, a)| !a.starts_with("--") && (*i == 0 || !flags.contains(&args[i - 1].as_str())))
        .map(|(_, a)| PathBuf::from(a))
        .unwrap_or_else(|| PathBuf::from("web"));
    let effects_only = args.iter().any(|a| a == "--effects");
    // Cards named after the folder (`-- web chorda`): those alone, the list and the sources
    // untouched.
    let only: Vec<String> = args
        .iter()
        .enumerate()
        .filter(|(i, a)| {
            !a.starts_with("--") && (*i == 0 || !flags.contains(&args[i - 1].as_str()))
        })
        .map(|(_, a)| a.clone())
        .skip(1)
        .collect();
    let dir = web.join("assets/units/sounds");
    std::fs::create_dir_all(&dir).expect("the folder");
    cantus_ui::i18n::set_thread_locale(Locale::En);
    if let Some(root) = opt("--content") {
        cantus_ui::tympanum_load::install(root);
    }
    let empty = cantus_core::Project::new("none");
    let mut total = 0u64;
    let kept = if effects_only {
        kept_blocks(&web)
    } else {
        Default::default()
    };
    let reals = match (opt("--guitar"), opt("--takes")) {
        (Some(g), Some(t)) => reals(&g, &t, &dir, &mut total),
        _ => Vec::new(),
    };
    let mut used: Vec<Used> = Vec::new();
    let mut code = String::from(
        "//! Every unit card's preset excerpts (`assets/units/sounds/`, and Try it's takes):\n\
         //! generated by `cargo run --release -p cantus-ui --example render_unit_sounds -- web`\n\
         //! (round 37, ADR 0101; `docs/round-fx/sounds.md`), rendered by Cantus with no\n\
         //! processing after. Do not edit by hand.\n\n\
         /// One excerpt: its file (without `.mp3`; under `/units/sounds/` unless it starts with\n\
         /// `/`), the preset's name and what it is for, each `(English, Spanish)`. Written as\n\
         /// `snd!(file, name, what, on)` (`main.rs`): the WebAssembly leaves the name and what\n\
         /// out and reads them from `/unit-text.json`.\n\
         #[cfg_attr(target_arch = \"wasm32\", allow(dead_code))]\n\
         pub struct Sound {\n    pub file: &'static str,\n    pub name: (&'static str, &'static str),\n    pub what: (&'static str, &'static str),\n    /// An effect's dry source (`SOURCES`); empty for an instrument.\n    pub on: &'static str,\n}\n\n\
         /// The excerpts of each card, by its `file`.\npub const SOUNDS: &[(&str, &[Sound])] = &[\n",
    );
    for u in units() {
        if !u.audible() || matches!(u, Unit::Effect(cantus_core::EffectKind::Reverb)) {
            continue;
        }
        let c = card(u);
        if SILENT.contains(&c.as_str()) {
            continue;
        }
        if !only.is_empty() && !only.contains(&c) {
            continue;
        }
        if effects_only && !matches!(u, Unit::Effect(_)) {
            match kept.get(&c) {
                Some(b) => code.push_str(b),
                None => panic!("{c}: no excerpts to keep in units_sounds.rs"),
            }
            continue;
        }
        let all = entries(u, false);
        let n = if u.is_instrument() { 4 } else { 3 };
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
        // A unit with a Try it manifest: its takes, on its source.
        if let Some((src, presets)) = tryit(&web, &c).filter(|_| !reals.is_empty()) {
            for (file, name, what) in &presets {
                line(&mut code, file, name, what, &src.on);
            }
            use_source(&mut used, &src.on, &src.dry, src.name, src.credit, true);
            code.push_str("    ]),\n");
            continue;
        }
        // The presets in `pick`'s order of range; one too close to its dry take to hear is
        // passed over for the next.
        let mut written = 0;
        for e in pick(&all, all.len()) {
            if written == n {
                break;
            }
            let stem = format!("{c}-{}", e.id.rsplit('.').next().unwrap_or("preset"));
            let name_en = t_in(Locale::En, &e.id);
            let path = dir.join(format!("{stem}.mp3"));
            let real = match (u, source_for(&c, e)) {
                (Unit::Effect(_), Some(id)) => reals.iter().find(|r| r.id == id),
                _ => None,
            };
            let heard = match (u, real) {
                (Unit::Effect(kind), Some(r)) => {
                    on_real(kind, e, r, &path, &name_en).map(|x| (x, r))
                }
                _ => None,
            };
            let on = match heard {
                Some(((0, level, apart), r)) => {
                    println!(
                        "{stem}: on {} LEFT OUT, {apart:.1} dB from the dry take ({level:+.1} dB as the preset leaves it)",
                        r.id
                    );
                    let _ = std::fs::remove_file(&path);
                    continue;
                }
                Some(((size, level, apart), r)) => {
                    total += size;
                    println!(
                        "{stem}: on {} ({apart:.1} dB from the dry take, {level:+.1} dB as the preset leaves it)",
                        r.id
                    );
                    use_source(
                        &mut used,
                        r.id,
                        &r.file,
                        r.name.clone(),
                        r.credit.clone(),
                        true,
                    );
                    r.id
                }
                None => {
                    let p = aud::prepare(&empty, Target::None, e, None, aud::TEMPO);
                    total += mp3(render_prepared(&p), &path, &name_en);
                    match (u.is_instrument(), e.role()) {
                        (true, _) => "",
                        (false, Role::Drums) => "drums",
                        (false, Role::Vocal) => "vocal",
                        _ => "creative",
                    }
                }
            };
            let name = pair(&name_en, &t_in(Locale::Es, &e.id));
            let what = pair(&what(e, Locale::En), &what(e, Locale::Es));
            line(&mut code, &stem, &name, &what, on);
            written += 1;
        }
        code.push_str("    ]),\n");
    }
    if !only.is_empty() {
        let order: Vec<String> = units().into_iter().map(card).collect();
        splice(&web, &code, &order, &used);
        println!(
            "{} KB of excerpts for {:?} in {}",
            total / 1024,
            only,
            dir.display()
        );
        return;
    }
    code.push_str(
        "];\n\n/// The dry sources the effects are heard on: (id, file as `Sound::file`, what it is, the\n\
         /// recording's credit — empty for Cantus's own —, whether each excerpt is matched to its\n\
         /// loudness by a gain alone rather than left at the preset's own level).\n\
         #[allow(clippy::type_complexity)]\n\
         pub const SOURCES: &[(&str, &str, (&str, &str), (&str, &str), bool)] = &[\n",
    );
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
        let name = pair(&t_in(Locale::En, key), &t_in(Locale::Es, key));
        use_source(&mut used, role.id(), &stem, name, pair("", ""), false);
    }
    for (id, file, name, credit, matched) in &used {
        let _ = writeln!(
            code,
            "    ({}, {}, ({}, {}), ({}, {}), {matched}),",
            lit(id),
            lit(file),
            lit(&name.0),
            lit(&name.1),
            lit(&credit.0),
            lit(&credit.1)
        );
    }
    code.push_str("];\n");
    std::fs::write(web.join("src/units_sounds.rs"), code).expect("the list");
    println!("{} KB of excerpts in {}", total / 1024, dir.display());
}
