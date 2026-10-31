//! What every project tool shares: reading arguments (with errors the model can correct
//! itself from), positions in bars:beats and beats, ids, the names of instruments and
//! effects, the schema snippets that describe them, and the JSON that answers with them.

use crate::tool::{EditPlan, Said, ToolError};
use cantus_core::tempo::{TempoMap, HORIZON};
use cantus_core::{
    ClipId, Command, EffectId, EffectKind, InstrumentKind, InstrumentSpec, Project, StripId, Ticks,
    TrackId,
};
use serde_json::{json, Map, Value};

pub type Res<T> = Result<T, ToolError>;

/// The most notes one call may write or change, and the most points one lane may get.
pub const MAX_NOTES: usize = 4096;
pub const MAX_POINTS: usize = 2048;

/// The arguments of a call.
#[derive(Copy, Clone)]
pub struct A<'a>(pub &'a Value);

impl<'a> A<'a> {
    /// The value of `k`, `None` when absent or `null`.
    pub fn get(&self, k: &str) -> Option<&'a Value> {
        self.0.get(k).filter(|v| !v.is_null())
    }

    pub fn has(&self, k: &str) -> bool {
        self.get(k).is_some()
    }

    pub fn req(&self, k: &str) -> Res<&'a Value> {
        self.get(k).ok_or_else(|| ToolError::bad(k, "is required"))
    }

    pub fn opt_f64(&self, k: &str) -> Res<Option<f64>> {
        match self.get(k) {
            None => Ok(None),
            Some(v) => v
                .as_f64()
                .filter(|x| x.is_finite())
                .map(Some)
                .ok_or_else(|| ToolError::bad(k, "must be a number")),
        }
    }

    pub fn f64(&self, k: &str) -> Res<f64> {
        self.opt_f64(k)?
            .ok_or_else(|| ToolError::bad(k, "is required"))
    }

    /// A number in `lo..=hi`, refused (not clamped) outside it: the model learns the range.
    pub fn opt_in(&self, k: &str, lo: f64, hi: f64) -> Res<Option<f64>> {
        match self.opt_f64(k)? {
            Some(x) if !(lo..=hi).contains(&x) => {
                Err(ToolError::bad(k, format!("must be {lo} to {hi}, not {x}")))
            }
            other => Ok(other),
        }
    }

    pub fn f64_in(&self, k: &str, lo: f64, hi: f64) -> Res<f64> {
        self.opt_in(k, lo, hi)?
            .ok_or_else(|| ToolError::bad(k, "is required"))
    }

    pub fn opt_u64(&self, k: &str) -> Res<Option<u64>> {
        match self.get(k) {
            None => Ok(None),
            Some(v) => v
                .as_u64()
                .map(Some)
                .ok_or_else(|| ToolError::bad(k, "must be a whole number ≥ 0")),
        }
    }

    pub fn u64(&self, k: &str) -> Res<u64> {
        self.opt_u64(k)?
            .ok_or_else(|| ToolError::bad(k, "is required"))
    }

    pub fn opt_bool(&self, k: &str) -> Res<Option<bool>> {
        match self.get(k) {
            None => Ok(None),
            Some(v) => v
                .as_bool()
                .map(Some)
                .ok_or_else(|| ToolError::bad(k, "must be true or false")),
        }
    }

    pub fn opt_str(&self, k: &str) -> Res<Option<&'a str>> {
        match self.get(k) {
            None => Ok(None),
            Some(v) => v
                .as_str()
                .map(Some)
                .ok_or_else(|| ToolError::bad(k, "must be text")),
        }
    }

    pub fn str(&self, k: &str) -> Res<&'a str> {
        self.opt_str(k)?
            .ok_or_else(|| ToolError::bad(k, "is required"))
    }

    /// A name: text of 1…`max` characters, trimmed.
    pub fn opt_name(&self, k: &str, max: usize) -> Res<Option<String>> {
        match self.opt_str(k)? {
            None => Ok(None),
            Some(s) => {
                let s = s.trim();
                if s.is_empty() || s.chars().count() > max {
                    Err(ToolError::bad(k, format!("must be 1 to {max} characters")))
                } else {
                    Ok(Some(s.to_string()))
                }
            }
        }
    }

    pub fn opt_array(&self, k: &str) -> Res<Option<&'a Vec<Value>>> {
        match self.get(k) {
            None => Ok(None),
            Some(v) => v
                .as_array()
                .map(Some)
                .ok_or_else(|| ToolError::bad(k, "must be a list")),
        }
    }

    pub fn array(&self, k: &str) -> Res<&'a Vec<Value>> {
        self.opt_array(k)?
            .ok_or_else(|| ToolError::bad(k, "is required"))
    }

    pub fn opt_object(&self, k: &str) -> Res<Option<&'a Map<String, Value>>> {
        match self.get(k) {
            None => Ok(None),
            Some(v) => v
                .as_object()
                .map(Some)
                .ok_or_else(|| ToolError::bad(k, "must be an object")),
        }
    }

    /// One of `choices` (exact, lower case).
    pub fn opt_choice(&self, k: &str, choices: &[&str]) -> Res<Option<&'a str>> {
        match self.opt_str(k)? {
            None => Ok(None),
            Some(s) if choices.contains(&s) => Ok(Some(s)),
            Some(s) => Err(ToolError::bad(
                k,
                format!("{s:?} is not one of {}", choices.join(", ")),
            )),
        }
    }
}

// --- ids ------------------------------------------------------------------------------

pub fn track(p: &Project, a: A<'_>, k: &str) -> Res<TrackId> {
    let id = TrackId(a.u64(k)?);
    p.track(id)
        .map(|_| id)
        .ok_or_else(|| ToolError::NotFound(format!("track {}", id.0)))
}

pub fn opt_track(p: &Project, a: A<'_>, k: &str) -> Res<Option<TrackId>> {
    if a.has(k) {
        track(p, a, k).map(Some)
    } else {
        Ok(None)
    }
}

/// A clip anywhere in the arrangement, and the track holding it.
pub fn find_clip(p: &Project, id: ClipId) -> Option<(TrackId, &cantus_core::Clip)> {
    p.tracks.iter().find_map(|t| t.clip(id).map(|c| (t.id, c)))
}

pub fn clip<'p>(p: &'p Project, a: A<'_>, k: &str) -> Res<(TrackId, &'p cantus_core::Clip)> {
    let id = ClipId(a.u64(k)?);
    find_clip(p, id).ok_or_else(|| ToolError::NotFound(format!("clip {}", id.0)))
}

/// A strip: its id, or `"master"`.
pub fn strip_of(p: &Project, v: &Value, k: &str) -> Res<StripId> {
    if v.as_str() == Some("master") {
        return Ok(StripId::MASTER);
    }
    let id = StripId(
        v.as_u64()
            .ok_or_else(|| ToolError::bad(k, "a strip id, or \"master\""))?,
    );
    p.strip(id)
        .map(|_| id)
        .ok_or_else(|| ToolError::NotFound(format!("strip {}", id.0)))
}

pub fn strip(p: &Project, a: A<'_>, k: &str) -> Res<StripId> {
    strip_of(p, a.req(k)?, k)
}

/// An effect by id, and the strip (or the master) it is on.
pub fn effect(p: &Project, a: A<'_>, k: &str) -> Res<(StripId, EffectId)> {
    let id = EffectId(a.u64(k)?);
    p.effect_host(id)
        .map(|host| (host, id))
        .ok_or_else(|| ToolError::NotFound(format!("effect {}", id.0)))
}

/// A strip's id as an agent reads it: a number, or `"master"`.
pub fn strip_json(id: StripId) -> Value {
    if id.is_master() {
        json!("master")
    } else {
        json!(id.0)
    }
}

// --- positions ------------------------------------------------------------------------

/// A position: a number of beats (quarter notes) from the song's start, or `"bar:beat"`
/// (`"9:1"`, `"9:2.5"`, `"9"`), the beat counted in the metre's own beat, both from 1.
pub fn pos_of(map: &TempoMap, v: &Value, k: &str) -> Res<Ticks> {
    let t = if let Some(b) = v.as_f64() {
        if !(b.is_finite() && b >= 0.0) {
            return Err(ToolError::bad(k, "beats must be 0 or more"));
        }
        Ticks::from_beats_f64(b)
    } else if let Some(s) = v.as_str() {
        bar_beat(map, s).ok_or_else(|| {
            ToolError::bad(
                k,
                format!(
                    "{s:?} is not \"bar:beat\" (bar and beat from 1, e.g. \"9:1\" or \"9:2.5\")"
                ),
            )
        })?
    } else {
        return Err(ToolError::bad(k, "a number of beats, or \"bar:beat\""));
    };
    if t > HORIZON {
        return Err(ToolError::bad(
            k,
            "is past the song's horizon (1 000 000 beats)",
        ));
    }
    Ok(t)
}

fn bar_beat(map: &TempoMap, s: &str) -> Option<Ticks> {
    let s = s.trim();
    let (bar, beat) = match s.split_once(':') {
        Some((b, x)) => (b.trim().parse::<i64>().ok()?, x.trim().parse::<f64>().ok()?),
        None => (s.parse::<i64>().ok()?, 1.0),
    };
    if !(1..=1_000_000).contains(&bar) || !beat.is_finite() || beat < 1.0 {
        return None;
    }
    let start = map.bar_tick(bar);
    let sig = map.signature_at(start);
    if beat >= sig.numerator as f64 + 1.0 {
        return None;
    }
    let beat_len = sig.ticks_per_beat().0 as f64;
    Some(Ticks(start.0 + ((beat - 1.0) * beat_len).round() as i64))
}

pub fn pos(map: &TempoMap, a: A<'_>, k: &str) -> Res<Ticks> {
    pos_of(map, a.req(k)?, k)
}

pub fn opt_pos(map: &TempoMap, a: A<'_>, k: &str) -> Res<Option<Ticks>> {
    a.get(k).map(|v| pos_of(map, v, k)).transpose()
}

/// A length from `at`: a number of beats (quarter notes), or `"N bars"` / `"N beats"`
/// (the metre's beats), counted through metre changes.
pub fn len_of(map: &TempoMap, at: Ticks, v: &Value, k: &str) -> Res<Ticks> {
    let t = if let Some(b) = v.as_f64() {
        if !(b.is_finite() && b > 0.0) {
            return Err(ToolError::bad(k, "beats must be more than 0"));
        }
        Ticks::from_beats_f64(b)
    } else if let Some(s) = v.as_str() {
        let s = s.trim().to_ascii_lowercase();
        let (n, unit) = s
            .split_once(' ')
            .map(|(n, u)| (n.trim(), u.trim()))
            .unwrap_or((s.as_str(), "beats"));
        let n: f64 = n
            .parse()
            .ok()
            .filter(|n: &f64| n.is_finite() && *n > 0.0 && *n <= 100_000.0)
            .ok_or_else(|| ToolError::bad(k, format!("{s:?}: a count, then bars or beats")))?;
        match unit {
            "bar" | "bars" => {
                let whole = n.floor() as i64;
                let (bar, _, _) = map.bbt(at);
                let from = map.bar_tick(bar);
                let end = map.bar_tick(bar + whole);
                let frac = n - whole as f64;
                let extra = (map.bar_len_at(end).0 as f64 * frac).round() as i64;
                Ticks(end.0 - from.0 + extra)
            }
            "beat" | "beats" => Ticks((map.beat_len_at(at).0 as f64 * n).round() as i64),
            _ => {
                return Err(ToolError::bad(
                    k,
                    format!("{s:?}: the unit is bars or beats"),
                ))
            }
        }
    } else {
        return Err(ToolError::bad(k, "a number of beats, or \"N bars\""));
    };
    if t.0 <= 0 || t > HORIZON {
        return Err(ToolError::bad(
            k,
            "must be more than 0 and within the song's horizon",
        ));
    }
    Ok(t)
}

pub fn len(map: &TempoMap, at: Ticks, a: A<'_>, k: &str) -> Res<Ticks> {
    len_of(map, at, a.req(k)?, k)
}

pub fn opt_len(map: &TempoMap, at: Ticks, a: A<'_>, k: &str) -> Res<Option<Ticks>> {
    a.get(k).map(|v| len_of(map, at, v, k)).transpose()
}

/// `"9:2.5"` for a tick.
pub fn bar_beat_text(map: &TempoMap, t: Ticks) -> String {
    let (bar, beat, tick) = map.bbt(t);
    let beat_len = map.beat_len_at(t).0 as f64;
    let b = beat as f64 + tick as f64 / beat_len;
    format!("{bar}:{}", trim_num(b))
}

/// A number written as short as it reads: `2`, `2.5`, `2.333`.
pub fn trim_num(x: f64) -> String {
    let s = format!("{:.3}", x);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".into()
    } else {
        s.to_string()
    }
}

/// A position as an agent reads it: beats and bars:beats.
pub fn pos_json(map: &TempoMap, t: Ticks) -> Value {
    json!({"beats": beats(t), "bar_beat": bar_beat_text(map, t)})
}

/// Ticks as beats (quarter notes), rounded to a thousandth.
pub fn beats(t: Ticks) -> f64 {
    (t.as_beats_f64() * 1000.0).round() / 1000.0
}

// --- names of instruments and effects ---------------------------------------------------

/// What a track can host, by the name an agent uses.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InstrumentChoice {
    Firmus,
    Pulsus,
    Native(InstrumentKind),
}

/// Every instrument: its id for agents, its product name, what it is.
pub const INSTRUMENTS: &[(&str, &str, InstrumentChoice, &str)] = &[
    (
        "unda",
        "Unda",
        InstrumentChoice::Native(InstrumentKind::Wavetable),
        "wavetable / hybrid synth: pads, leads, plucks, basses, vocal-like formant tables",
    ),
    (
        "calor",
        "Calor",
        InstrumentChoice::Native(InstrumentKind::Analog),
        "analog-modelling polysynth: warm pads, brass, sync leads",
    ),
    (
        "modulus",
        "Modulus",
        InstrumentChoice::Native(InstrumentKind::Fm),
        "FM and additive synth: bells, electric pianos, metallic and glassy tones",
    ),
    (
        "exemplar",
        "Exemplar",
        InstrumentChoice::Native(InstrumentKind::Sampler),
        "multisample sampler (SFZ, Decent Sampler, SF2); silent until an instrument is loaded",
    ),
    (
        "gravis",
        "Gravis",
        InstrumentChoice::Native(InstrumentKind::Bass),
        "bass: 808, modelled electric bass, synth bass",
    ),
    (
        "lamina",
        "Lamina",
        InstrumentChoice::Native(InstrumentKind::ElectricPiano),
        "modelled electric piano (tines, reeds)",
    ),
    (
        "organum",
        "Organum",
        InstrumentChoice::Native(InstrumentKind::Organ),
        "tonewheel organ with drawbars and a rotary speaker",
    ),
    (
        "chorda",
        "Chorda",
        InstrumentChoice::Native(InstrumentKind::Chorda),
        "modelled grand piano with a sustain pedal parameter (`pedal`, 0 up … 1 down)",
    ),
    (
        "firmus",
        "Firmus",
        InstrumentChoice::Firmus,
        "the simple built-in two-oscillator subtractive synth",
    ),
    (
        "pulsus",
        "Pulsus",
        InstrumentChoice::Pulsus,
        "synthesized drum kit: pads on General MIDI drum notes (kick 36, snare 38, closed hat 42…)",
    ),
];

pub fn instrument_choice(name: &str) -> Option<InstrumentChoice> {
    INSTRUMENTS
        .iter()
        .find(|(id, ..)| *id == name)
        .map(|(_, _, c, _)| *c)
}

/// The id of a native kind (every kind is in [`INSTRUMENTS`]: the match is exhaustive).
pub fn native_id(kind: InstrumentKind) -> &'static str {
    match kind {
        InstrumentKind::Wavetable => "unda",
        InstrumentKind::Analog => "calor",
        InstrumentKind::Fm => "modulus",
        InstrumentKind::Sampler => "exemplar",
        InstrumentKind::Bass => "gravis",
        InstrumentKind::ElectricPiano => "lamina",
        InstrumentKind::Organ => "organum",
        InstrumentKind::Chorda => "chorda",
    }
}

/// The id of what a track hosts.
pub fn instrument_id(spec: &InstrumentSpec) -> &'static str {
    match spec {
        InstrumentSpec::Firmus(_) => "firmus",
        InstrumentSpec::Pulsus(_) => "pulsus",
        InstrumentSpec::Native(n) => native_id(n.kind),
        InstrumentSpec::External(_) => "hosted",
    }
}

pub fn instrument_name(spec: &InstrumentSpec) -> String {
    match spec {
        InstrumentSpec::External(e) => e.name.clone(),
        other => {
            let id = instrument_id(other);
            INSTRUMENTS
                .iter()
                .find(|(i, ..)| *i == id)
                .map(|(_, n, ..)| n.to_string())
                .unwrap_or_default()
        }
    }
}

/// An effect kind's id for agents and its product name (exhaustive: a new kind fails to
/// compile until it is named here).
pub fn effect_names(kind: EffectKind) -> (&'static str, &'static str) {
    use EffectKind as K;
    match kind {
        K::Eq => ("aequalis", "Aequalis"),
        K::Compressor => ("comprimo", "Comprimo"),
        K::Limiter => ("terminus", "Terminus"),
        K::Space => ("spatium", "Spatium"),
        K::Multiband => ("multiplex", "Multiplex"),
        K::DeEsser => ("sibilus", "Sibilus"),
        K::Gate => ("porta", "Porta"),
        K::Saturator => ("fervor", "Fervor"),
        K::Stereo => ("latitudo", "Latitudo"),
        K::Maximizer => ("culmen", "Culmen"),
        K::Delay => ("delay", "Delay"),
        K::Reverb => ("reverb", "Reverb"),
        K::External => ("hosted", "Hosted plug-in"),
        K::Chorus => ("concentus", "Concentus"),
        K::Flanger => ("flexus", "Flexus"),
        K::Phaser => ("phasis", "Phasis"),
        K::Tremolo => ("tremor", "Tremor"),
        K::Vibrato => ("vibrans", "Vibrans"),
        K::RingMod => ("anulus", "Anulus"),
        K::FreqShift => ("translatio", "Translatio"),
        K::VolumeShaper => ("fluctus", "Fluctus"),
        K::Clipper => ("tonsor", "Tonsor"),
        K::Transient => ("impetus", "Impetus"),
        K::TransientSplit => ("discrimen", "Discrimen"),
        K::Analyzer => ("speculum", "Speculum"),
        K::AutoTune => ("tonus", "Tonus"),
    }
}

/// A native effect kind by its id (or its plain English role: `eq`, `compressor`, …).
pub fn effect_kind(name: &str) -> Option<EffectKind> {
    let alias = match name {
        "eq" => Some(EffectKind::Eq),
        "compressor" => Some(EffectKind::Compressor),
        "limiter" => Some(EffectKind::Limiter),
        "multiband" => Some(EffectKind::Multiband),
        "deesser" | "de_esser" => Some(EffectKind::DeEsser),
        "gate" => Some(EffectKind::Gate),
        "saturator" => Some(EffectKind::Saturator),
        "stereo" => Some(EffectKind::Stereo),
        "maximizer" => Some(EffectKind::Maximizer),
        "chorus" => Some(EffectKind::Chorus),
        "flanger" => Some(EffectKind::Flanger),
        "phaser" => Some(EffectKind::Phaser),
        "tremolo" => Some(EffectKind::Tremolo),
        "vibrato" => Some(EffectKind::Vibrato),
        "ring_mod" => Some(EffectKind::RingMod),
        "freq_shift" => Some(EffectKind::FreqShift),
        "volume_shaper" => Some(EffectKind::VolumeShaper),
        "clipper" => Some(EffectKind::Clipper),
        "transient" => Some(EffectKind::Transient),
        "transient_split" => Some(EffectKind::TransientSplit),
        "analyzer" => Some(EffectKind::Analyzer),
        "autotune" | "auto_tune" => Some(EffectKind::AutoTune),
        _ => None,
    };
    alias.or_else(|| {
        EffectKind::ALL
            .into_iter()
            .find(|k| *k != EffectKind::External && effect_names(*k).0 == name)
    })
}

/// Every native effect id, for schemas.
pub fn effect_ids() -> Vec<&'static str> {
    EffectKind::ALL
        .into_iter()
        .filter(|k| *k != EffectKind::External)
        .map(|k| effect_names(k).0)
        .collect()
}

pub fn instrument_ids() -> Vec<&'static str> {
    INSTRUMENTS.iter().map(|(id, ..)| *id).collect()
}

// --- schema snippets --------------------------------------------------------------------

pub const POS_DOC: &str = "a position: beats (quarter notes) from the song's start as a number, or \"bar:beat\" text with bar and beat from 1 (\"9:1\", \"9:2.5\"; the beat is the metre's)";
pub const LEN_DOC: &str =
    "a length: beats (quarter notes) as a number, or \"N bars\" / \"N beats\" text";

pub fn s_pos(what: &str) -> Value {
    json!({"type": ["number", "string"], "description": format!("{what} — {POS_DOC}.")})
}

pub fn s_len(what: &str) -> Value {
    json!({"type": ["number", "string"], "description": format!("{what} — {LEN_DOC}.")})
}

pub fn s_id(what: &str) -> Value {
    json!({"type": "integer", "minimum": 1, "description": what})
}

pub fn s_strip(what: &str) -> Value {
    json!({"type": ["integer", "string"], "description": format!("{what}: a strip id (get_mixer), or \"master\".")})
}

pub fn s_ids(what: &str) -> Value {
    json!({"type": "array", "items": {"type": "integer", "minimum": 1}, "description": what})
}

/// An object schema that refuses unknown arguments.
pub fn object(properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

// --- plans ------------------------------------------------------------------------------

/// One command, or several as one undo step labelled `label_key`.
pub fn one(mut commands: Vec<Command>, label_key: &str) -> Res<Command> {
    match commands.len() {
        0 => Err(ToolError::bad("arguments", "nothing to change was given")),
        1 => Ok(commands.pop().expect("one")),
        _ => Ok(Command::Batch {
            label_key: label_key.to_string(),
            commands,
        }),
    }
}

pub fn plan(command: Command, said: Said, result: Value) -> EditPlan {
    EditPlan {
        command,
        said,
        result,
    }
}

/// A user's text (a name, a marker) for a sentence's quoted name: newlines and other
/// control characters become spaces (review #17), at most 80 characters. It is data, never
/// an instruction; the host puts it in its language's quotes.
pub fn clean_path(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// [`clean_path`] for a name: also at most 80 characters, one space between words.
pub fn clean(s: &str) -> String {
    let one_line: String = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(80)
        .collect();
    one_line.split_whitespace().collect::<Vec<_>>().join(" ")
}
