//! The machine-readable catalogue of every unit (round 34, builder `tools`, ADR 0082):
//! each native instrument and effect with its parameters — keys, English names, groups,
//! ranges, units, tapers, defaults, the names of a stepped one's choices — its factory
//! presets and what they set, its starting preset, latency, what it is for and the
//! model's limits. **Generated from the parameter tables** (`cantus_core::fx::*`,
//! `cantus_core::inst::*`, the pads), the names from the English locale the editors use;
//! the test fails when a kind or a parameter is missing.

use crate::controllers_words::effect_choice_words;
use crate::i18n::{t_in, Locale};
use cantus_ai::tools_project::args::{effect_names, native_id, INSTRUMENTS};
use cantus_core::pads::{KitField, PadField, PadVoice};
use cantus_core::starting::{start_of, Neutral, Start};
use cantus_core::{
    EffectKind, EffectParamInfo, EffectSlot, EffectSpec, FirmusParam, InstrumentKind,
};
use serde_json::{json, Value};

fn en(key: &str) -> String {
    t_in(Locale::En, key)
}

/// What each effect is for, in a sentence a model can choose by (exhaustive: a new kind
/// does not compile until it is described).
pub fn effect_use(kind: EffectKind) -> &'static str {
    use EffectKind as K;
    match kind {
        K::Eq => "Parametric EQ, 24 bands (bell, shelves, cuts, notch, tilt), stereo/mid/side, zero-latency, natural or linear phase, a dynamic mode per band. Tone shaping, cleaning lows, carving masking frequencies, mastering EQ.",
        K::Compressor => "Compressor with styles (clean, vintage, opto, FET, bus…), sidechain from another strip and a filtered key. Control dynamics, glue a bus, duck with a sidechain.",
        K::Limiter => "Brickwall limiter with true peak and lookahead. The last unit on the master or a safety on a bus.",
        K::Space => "Algorithmic reverb with rooms, halls, plates and a decay EQ. A send return for space, or an insert for character.",
        K::Multiband => "Multiband dynamics (up to six bands): compress or expand each band. Mastering glue, de-mud, tame a resonant band.",
        K::DeEsser => "De-esser: tames sibilance (4–10 kHz) of vocals and cymbals, split-band or wideband.",
        K::Gate => "Gate and expander, keyed from its own signal, another strip or a track's notes. Tighten drums, rhythmic gating.",
        K::Saturator => "Saturation and colour: tape, tube, overdrive, a drawn waveshaper, oversampled. Warmth, grit, harmonics, loudness without peaks.",
        K::Stereo => "Stereo control: width, mid/side balance, mono bass below a frequency, Haas widening, panning. Mono compatibility, a wider chorus.",
        K::Maximizer => "Mastering maximizer: multiband dynamics into a limiter, loud and clean. The last unit on the master for streaming loudness.",
        K::Delay => "Tempo delay with feedback, tone and ping-pong. Echoes, slapback, throws on a send.",
        K::Reverb => "The simple reverb (older projects); Spatium is the full one.",
        K::External => "A hosted CLAP/VST3 plug-in, chosen by the user in Cantus.",
        K::Chorus => "Chorus (Juno-style, Dimension-style, modern): width and motion on pads, keys, guitars, a doubled voice.",
        K::Flanger => "Through-zero flanger: jet sweeps, synced sweeps on leads and drums.",
        K::Phaser => "Phaser (Phase 90 and more): swirling motion on guitars, keys, pads.",
        K::Tremolo => "Tremolo and auto-pan, synced to the song: pulsing amplitude or a moving stereo image.",
        K::Vibrato => "Vibrato and tape wow/flutter: pitch motion, lo-fi drift.",
        K::RingMod => "Ring modulator: metallic, bell-like, inharmonic tones; a shimmer at low mix.",
        K::FreqShift => "Frequency shifter: barber-pole risers, detuned stereo, inharmonic movement.",
        K::VolumeShaper => "Volume shaper (LFO Tool style): sidechain-style pumping on quarters, gating rhythms, synced to the song.",
        K::Clipper => "Clipper with true peak: shave peaks before a limiter, drive drums, a soft master clip.",
        K::Transient => "Transient shaper: more or less attack and sustain — punchier kicks, softer snares.",
        K::TransientSplit => "Transient splitter: separate gain and processing of the attack and the body.",
        K::Analyzer => "Analyser (spectrum, loudness, stereo): passes its sound through unchanged; for looking.",
        K::AutoTune => "Real-time pitch correction to a key and scale, from natural to hard-tuned, with created vibrato.",
    }
}

pub fn instrument_use(id: &str) -> &'static str {
    INSTRUMENTS
        .iter()
        .find(|(i, ..)| *i == id)
        .map(|(_, _, _, w)| *w)
        .unwrap_or("")
}

fn taper(i: &EffectParamInfo) -> &'static str {
    if i.stepped {
        "stepped"
    } else if i.logarithmic {
        "log"
    } else {
        "linear"
    }
}

/// `b3.gain` → `Band 4`: a banded key's group, numbered from 1 as the editors show it.
pub fn group_of(key: &str) -> Option<String> {
    let (head, _) = key.split_once('.')?;
    let letters: String = head
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    let digits = &head[letters.len()..];
    let n: usize = digits.parse().ok()?;
    let word = match letters.as_str() {
        "b" => "Band",
        "sc" => "Key band",
        "d" => "Decay band",
        "e" => "Post band",
        "p" => "Point",
        "osc" => "Oscillator",
        "env" => "Envelope",
        "lfo" => "LFO",
        "mod" => "Matrix slot",
        "macro" => "Macro",
        "z" => "Zone",
        "op" => "Operator",
        "filter" | "f" => "Filter",
        other => other,
    };
    Some(format!("{word} {}", n + 1))
}

fn row(key: &str, i: &EffectParamInfo, choices: Option<Vec<String>>) -> Value {
    let mut v = json!({
        "key": key, "name": en(i.label_key), "unit": i.unit,
        "min": i.min, "max": i.max, "default": i.default, "taper": taper(i),
    });
    if let Some(g) = group_of(key) {
        v["group"] = json!(g);
    }
    if let Some(c) = choices.filter(|c| !c.is_empty()) {
        let lo = i.min.round() as i64;
        v["choices"] = json!(c
            .into_iter()
            .enumerate()
            .map(|(k, name)| json!({"value": lo + k as i64, "name": name}))
            .collect::<Vec<_>>());
    }
    v
}

fn latency_at_48k(spec: EffectSpec) -> usize {
    let slot = EffectSlot {
        id: cantus_core::EffectId(1),
        bypass: false,
        spec,
        mix: 1.0,
        sidechain: None,
        midi_from: None,
    };
    cantus_engine::effects::EffectUnit::new(&slot, 48_000.0).latency()
}

fn sets(values: &[(&str, f32)]) -> Value {
    Value::Object(
        values
            .iter()
            .map(|(k, v)| (k.to_string(), json!(v)))
            .collect(),
    )
}

/// The full entry of an effect kind.
pub fn effect_entry(kind: EffectKind) -> Value {
    let (id, name) = effect_names(kind);
    let params: Vec<Value> = kind
        .params()
        .iter()
        .map(|i| row(i.key, i, effect_choice_words(kind, i.key, Locale::En)))
        .collect();
    let presets: Vec<Value> = crate::views::fx::factory_presets(kind)
        .iter()
        .map(|(label, values)| json!({"name": en(label), "sets": sets(values)}))
        .collect();
    let user: Vec<String> = crate::views::fx::user_presets(kind)
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    let starting = match start_of(kind) {
        Start::Preset(p) => json!({"preset": en(p.label_key), "sets": sets(p.params)}),
        Start::Neutral(n) => json!({"neutral": match n {
            Neutral::Flat => "flat by design: the curve or image is yours to make",
            Neutral::Sounds => "its defaults already do the job audibly",
            Neutral::MustNotSurprise => "a safety at the end of a chain: changes nothing until asked",
            Neutral::Measures => "it only measures",
            Neutral::Hosted => "a hosted plug-in's own initial state",
        }}),
    };
    json!({
        "id": id, "name": name, "type": "effect", "what": effect_use(kind),
        "sidechain": kind.takes_sidechain(), "hears_notes": kind.takes_notes(),
        "latency_samples_at_48k": latency_at_48k(cantus_core::starting::starting_spec(kind)),
        "starting_preset": starting,
        "params": params, "presets": presets, "user_presets": user,
        "limits": {"per_strip": cantus_core::model::MAX_INSERTS},
    })
}

fn firmus_rows() -> Vec<Value> {
    FirmusParam::ALL
        .iter()
        .map(|p| {
            let i = p.info();
            let info = EffectParamInfo {
                key: "",
                label_key: i.label_key,
                min: i.min,
                max: i.max,
                default: i.default,
                unit: i.unit,
                logarithmic: i.logarithmic,
                stepped: i.stepped,
            };
            let key = cantus_ai::tools_project::json::firmus_key(*p);
            let waves = matches!(p, FirmusParam::Osc1Wave | FirmusParam::Osc2Wave).then(|| {
                crate::inst_choices::firmus_waves()
                    .iter()
                    .map(|w| w.show_in(Locale::En))
                    .collect()
            });
            row(&key, &info, waves)
        })
        .collect()
}

fn pulsus_rows() -> Vec<Value> {
    let mut out = Vec::new();
    for f in KitField::ALL {
        let (label, min, max, default) = f.range();
        out.push(json!({"key": format!("kit.{}", cantus_ai::tools_project::json::snake(&format!("{f:?}"))),
            "name": en(label), "min": min, "max": max, "default": default, "taper": "stepped"}));
    }
    for voice in PadVoice::ALL {
        for f in PadField::for_voice(voice) {
            let i = f.info(voice);
            let field = cantus_ai::tools_project::json::snake(&format!("{f:?}"));
            out.push(json!({
                "key": format!("pad<note>.{field}"), "voice": en(voice.label_key()), "name": en(i.label_key),
                "unit": i.unit, "min": i.min, "max": i.max, "default": i.default,
                "taper": if i.stepped { "stepped" } else if i.logarithmic { "log" } else { "linear" },
            }));
        }
    }
    out
}

/// The full entry of an instrument, by its id (`unda`, `chorda`, `firmus`, `pulsus`…).
pub fn instrument_entry(id: &str) -> Option<Value> {
    let (_, name, choice, what) = INSTRUMENTS.iter().find(|(i, ..)| *i == id)?;
    use cantus_ai::tools_project::args::InstrumentChoice as C;
    let (params, presets): (Vec<Value>, Vec<Value>) = match choice {
        C::Native(kind) => (
            kind.params()
                .iter()
                .map(|i| {
                    let words = crate::inst_choices::words(*kind, i.key)
                        .map(|w| w.iter().map(|x| x.show_in(Locale::En)).collect());
                    let mut r = row(i.key, i, words);
                    r["automatable"] = json!(kind.automatable(i.key));
                    r
                })
                .collect(),
            crate::views::inst::factory_presets(*kind)
                .iter()
                .map(|p| json!({"name": en(p.label_key), "tags": p.tags, "sets": sets(p.values),
                    "text": Value::Object(p.text.iter().map(|(k, v)| (k.to_string(), json!(v))).collect())}))
                .chain(crate::views::inst::user_presets(*kind).into_iter().map(|(n, _)| json!({"name": n, "user": true})))
                .collect(),
        ),
        C::Firmus => (firmus_rows(), Vec::new()),
        C::Pulsus => (
            pulsus_rows(),
            vec![json!({"name": "Default kit", "what": "seven pads on the General MIDI drum map"}),
                 json!({"name": "Studio kit", "what": "the full Pulsus II kit"})],
        ),
    };
    let mut v = json!({
        "id": id, "name": name, "type": "instrument", "what": what,
        "params": params, "presets": presets,
    });
    if let C::Native(kind) = choice {
        v["playable"] = match range_words(*kind) {
            Some(r) => json!(r),
            None => json!("the whole MIDI range"),
        };
        if *kind == InstrumentKind::Sampler {
            v["note"] = json!("silent until an instrument (SFZ, Decent Sampler, SF2) is loaded into it by the user or from a file in the project's folder");
        }
        let _ = native_id(*kind);
    }
    if id == "pulsus" {
        v["note"] = json!("pads are addressed by their note: pad36 is the kick on the General MIDI map; read get_track for a kit's pads and names");
    }
    Some(v)
}

fn range_words(kind: InstrumentKind) -> Option<String> {
    let spec = cantus_core::InstrumentSpec::Native(cantus_core::NativeSpec::new(kind));
    cantus_ai::tools_project::music::playable(&spec).map(|(_, s)| s)
}

/// Every unit, short: what list_units answers and `cantus://units` holds.
pub fn list() -> Value {
    let mut out: Vec<Value> = INSTRUMENTS
        .iter()
        .map(|(id, name, _, what)| json!({"id": id, "name": name, "type": "instrument", "what": what}))
        .collect();
    for kind in EffectKind::ALL
        .into_iter()
        .filter(|k| *k != EffectKind::External)
    {
        let (id, name) = effect_names(kind);
        out.push(json!({"id": id, "name": name, "type": "effect", "what": effect_use(kind),
            "params": kind.params().len(), "presets": crate::views::fx::factory_presets(kind).len()}));
    }
    json!({"units": out, "limits": cantus_ai::tools_project::json::limits_json()})
}

/// One unit by id, or `None`.
pub fn entry(id: &str) -> Option<Value> {
    instrument_entry(id).or_else(|| {
        cantus_ai::tools_project::args::effect_kind(id)
            .filter(|k| *k != EffectKind::External)
            .map(effect_entry)
    })
}
