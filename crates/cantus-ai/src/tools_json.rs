//! The project as an agent reads it: plain JSON with ids, positions in beats and
//! bars:beats, values in their units. Names, marker texts and clip names are the user's
//! data: they are given as JSON strings, never read as instructions.

use super::args::{beats, effect_names, instrument_id, instrument_name, pos_json, strip_json};
use cantus_core::automation::range::range_of;
use cantus_core::tempo::TempoMap;
use cantus_core::{
    Clip, EffectSlot, FirmusParam, InstrumentSpec, MixerStrip, Note, Project, Ticks, Track,
};
use serde_json::{json, Value};

/// `"OscMix"` → `"osc_mix"`: a stable key from an enum variant's name (they are written to
/// project files, so they never change).
pub fn snake(camel: &str) -> String {
    let mut out = String::with_capacity(camel.len() + 4);
    for (i, c) in camel.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

pub fn firmus_key(p: FirmusParam) -> String {
    snake(&format!("{p:?}"))
}

/// A number rounded for reading (four significant decimals is plenty for a knob).
pub fn num(x: f32) -> Value {
    let v = (x as f64 * 10_000.0).round() / 10_000.0;
    json!(v)
}

pub fn color_hex(c: cantus_core::Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

pub fn key_json(p: &Project) -> Value {
    match p.key {
        Some(k) => json!({
            "root": k.root_name(),
            "scale": serde_json::to_value(k.scale).unwrap_or(Value::Null),
        }),
        None => Value::Null,
    }
}

/// What a clip holds.
pub fn clip_kind(c: &Clip) -> &'static str {
    if c.automation.is_some() {
        "automation"
    } else if c.audio.is_some() {
        "audio"
    } else {
        "notes"
    }
}

pub fn clip_json(p: &Project, map: &TempoMap, c: &Clip) -> Value {
    let mut v = json!({
        "id": c.id.0,
        "name": c.name,
        "kind": clip_kind(c),
        "start": pos_json(map, c.start),
        "end": pos_json(map, c.end()),
        "length_beats": beats(c.length),
        "muted": c.muted,
    });
    if c.automation.is_none() && c.audio.is_none() {
        v["notes"] = json!(c.notes.len());
    }
    if c.offset != Ticks::ZERO {
        v["offset_beats"] = json!(beats(c.offset));
    }
    if let Some(l) = c.content_length {
        v["loop_beats"] = json!(beats(l));
    }
    if c.lane != 0 {
        v["take_lane"] = json!(c.lane);
    }
    if let Some(a) = &c.automation {
        v["target"] = target_json(p, &a.target);
        v["points"] = json!(a.points.len());
    }
    v
}

/// An automation or modulation target as an agent names it.
pub fn target_json(p: &Project, t: &cantus_core::AutoTarget) -> Value {
    use cantus_core::AutoTarget as T;
    let mut v = match t {
        T::TrackGain(id) => json!({"track_id": id.0, "param": "gain_db"}),
        T::TrackPan(id) => json!({"track_id": id.0, "param": "pan"}),
        T::StripGain(id) => json!({"strip_id": id.0, "param": "gain_db"}),
        T::StripPan(id) => json!({"strip_id": id.0, "param": "pan"}),
        T::StripWidth(id) => json!({"strip_id": id.0, "param": "width"}),
        T::Send { strip, to } => {
            json!({"strip_id": strip.0, "param": "send", "to": strip_json(*to)})
        }
        T::MasterGain => json!({"strip_id": "master", "param": "gain_db"}),
        T::MasterPan => json!({"strip_id": "master", "param": "pan"}),
        T::MasterWidth => json!({"strip_id": "master", "param": "width"}),
        T::Effect { effect, key } => json!({"effect_id": effect.0, "param": key}),
        T::Instrument { track, key } => json!({"track_id": track.0, "param": key}),
        T::Firmus { track, param } => json!({"track_id": track.0, "param": firmus_key(*param)}),
        T::Plugin { effect, id } => json!({"effect_id": effect.0, "plugin_param": id}),
        T::PluginInstrument { track, id } => json!({"track_id": track.0, "plugin_param": id}),
        T::ModAmount(m) => json!({"modulator_id": m.0, "param": "amount"}),
        T::NoteFx { track, effect, key } => {
            json!({"track_id": track.0, "note_effect_id": effect.0, "param": key})
        }
    };
    if let Some(r) = range_of(p, t) {
        v["min"] = num(r.min);
        v["max"] = num(r.max);
    }
    v
}

pub fn note_json(map: &TempoMap, clip: &Clip, n: &Note) -> Value {
    let at = clip.start - clip.offset + n.start;
    let mut v = json!({
        "id": n.id.0,
        "start": pos_json(map, at),
        "length_beats": beats(n.length),
        "pitch": n.pitch,
        "name": cantus_core::harmony::pitch_name(n.pitch),
        "velocity": n.velocity,
    });
    if n.muted {
        v["muted"] = json!(true);
    }
    v
}

pub fn instrument_json(spec: &InstrumentSpec) -> Value {
    json!({"kind": instrument_id(spec), "name": instrument_name(spec)})
}

pub fn track_summary(p: &Project, t: &Track) -> Value {
    let notes: usize = t.clips.iter().map(|c| c.notes.len()).sum();
    json!({
        "id": t.id.0,
        "name": t.name,
        "instrument": instrument_json(&t.instrument),
        "strip_id": t.strip.map(|s| json!(s.0)).unwrap_or(json!("master")),
        "gain_db": num(t.gain_db),
        "pan": num(t.pan),
        "muted": t.muted,
        "soloed": t.soloed,
        "clips": t.clips.len(),
        "notes": notes,
        "frozen": t.frozen.is_some(),
        "session_clips": t.slots.iter().filter(|s| s.clip.is_some()).count(),
        "color": color_hex(t.color),
    })
    .tap(|v| {
        if !p.tracks.is_empty() {
            v["index"] = json!(p.track_index(t.id).unwrap_or(0));
        }
    })
}

/// A small helper so builders read top to bottom.
pub trait Tap: Sized {
    fn tap(self, f: impl FnOnce(&mut Self)) -> Self;
}

impl Tap for Value {
    fn tap(mut self, f: impl FnOnce(&mut Self)) -> Self {
        f(&mut self);
        self
    }
}

pub fn effect_entry(slot: &EffectSlot) -> Value {
    let (id, name) = effect_names(slot.spec.kind);
    let name = slot
        .spec
        .external
        .as_ref()
        .map(|e| e.name.clone())
        .unwrap_or_else(|| name.to_string());
    let mut v = json!({
        "id": slot.id.0,
        "kind": id,
        "name": name,
        "bypass": slot.bypass,
        "mix": num(slot.mix),
    });
    if let Some(sc) = slot.sidechain {
        v["sidechain_strip_id"] = json!(sc.0);
    }
    v
}

pub fn strip_entry(p: &Project, s: &MixerStrip) -> Value {
    let tracks: Vec<u64> = p.tracks_on_strip(s.id).map(|t| t.id.0).collect();
    json!({
        "id": s.id.0,
        "number": p.strip_number(s.id),
        "name": s.name,
        "gain_db": num(s.gain_db),
        "pan": num(s.pan),
        "width": num(s.width),
        "muted": s.muted,
        "soloed": s.soloed,
        "phase_invert": s.phase_invert,
        "fx_enabled": s.fx_enabled,
        "output": s.output.map(|o| json!(o.0)).unwrap_or(json!("master")),
        "sends": s.sends.iter().map(|x| json!({
            "to": x.to.0,
            "level": num(x.level),
            "level_db": num(if x.level > 0.0 { 20.0 * x.level.log10() } else { -96.0 }),
            "enabled": x.enabled,
            "pre_fader": !x.tap.is_post(),
        })).collect::<Vec<_>>(),
        "effects": s.effects.iter().map(effect_entry).collect::<Vec<_>>(),
        "tracks": tracks,
        "frozen": s.frozen.is_some(),
    })
}

pub fn mixer_json(p: &Project) -> Value {
    json!({
        "strips": p.strips.iter().map(|s| strip_entry(p, s)).collect::<Vec<_>>(),
        "master": {
            "gain_db": num(p.master.gain_db),
            "pan": num(p.master.pan),
            "width": num(p.master.width),
            "phase_invert": p.master.phase_invert,
            "fx_enabled": p.master.fx_enabled,
            "effects": p.master.effects.iter().map(effect_entry).collect::<Vec<_>>(),
        },
        "groups": p.strip_groups.iter().map(|g| json!({
            "id": g.id.0, "name": g.name,
            "strips": p.strips_in_group(g.id).map(|s| s.id.0).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "pan_law": p.pan_law.id(),
        "limits": limits_json(),
    })
}

/// What the model can and cannot set, stated up front.
pub fn limits_json() -> Value {
    json!({
        "gain_db": [cantus_core::model::MIN_GAIN_DB, cantus_core::model::MAX_GAIN_DB],
        "pan": [-1.0, 1.0],
        "width": [0.0, cantus_core::model::MAX_WIDTH],
        "inserts_per_strip": cantus_core::model::MAX_INSERTS,
        "sends_per_strip": cantus_core::model::MAX_SENDS,
        "modulators_per_host": cantus_core::modulator::MAX_PER_HOST,
        "routes_per_modulator": cantus_core::modulator::MAX_ROUTES,
        "notes_per_call": super::args::MAX_NOTES,
        "automation_points_per_call": super::args::MAX_POINTS,
        "song_horizon_beats": 1_000_000,
        "tempo_bpm": [cantus_core::time::MIN_BPM, cantus_core::time::MAX_BPM],
    })
}

pub fn project_json(p: &Project, revision: u64) -> Value {
    let map = p.tempo_map();
    let end = p.content_end();
    let (last_bar, _, _) = map.bbt(end);
    json!({
        "name": p.name,
        "revision": revision,
        "tempo_bpm": p.tempo_bpm,
        "time_signature": p.time_signature.to_string(),
        "key": key_json(p),
        "end": pos_json(&map, end),
        "bars": if end == Ticks::ZERO { 0 } else { last_bar - i64::from(map.bar_start(end) == end) },
        "loop": p.loop_region.as_ref().map(|l| json!({
            "start": pos_json(&map, l.start), "end": pos_json(&map, l.end), "enabled": p.loop_enabled,
        })),
        "tempo_points": p.tempo_points.iter().map(|t| json!({
            "at": pos_json(&map, t.tick), "bpm": t.bpm, "ramp": t.ramp,
        })).collect::<Vec<_>>(),
        "metre_changes": p.metre_changes.iter().map(|m| json!({
            "at": pos_json(&map, m.tick), "time_signature": m.signature.to_string(),
        })).collect::<Vec<_>>(),
        "markers": p.markers.iter().map(|m| json!({
            "id": m.id.0, "at": pos_json(&map, m.tick), "name": m.name,
        })).collect::<Vec<_>>(),
        "tracks": p.tracks.iter().map(|t| track_summary(p, t)).collect::<Vec<_>>(),
        "strips": p.strips.len(),
        "master_effects": p.master.effects.len(),
        "scenes": p.session.scenes.iter().map(|s| json!({"id": s.id.0, "name": s.name})).collect::<Vec<_>>(),
        "limits": limits_json(),
    })
}
