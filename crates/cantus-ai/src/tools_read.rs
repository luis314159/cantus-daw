//! The reading tools: the project, a track, a clip's notes or points, the mixer, a device's
//! values and the history. Each answers at once; none changes anything.

use super::args::{self, beats, pos_json, s_id, strip_json, Res, A};
use super::json::{
    clip_json, effect_entry, firmus_key, mixer_json, note_json, num, project_json, snake,
    target_json, track_summary,
};
use crate::tool::{ToolCx, ToolError};
use cantus_core::automation::range::range_of;
use cantus_core::pads::PadField;
use cantus_core::{EffectSlot, FirmusParam, InstrumentSpec, Origin, Project, Ticks, TrackId};
use serde_json::{json, Value};

pub fn empty_schema() -> Value {
    args::object(json!({}), &[])
}

pub fn get_project(cx: &ToolCx<'_>, _: &Value) -> Res<Value> {
    Ok(project_json(cx.project(), cx.bus().revision()))
}

pub fn track_schema() -> Value {
    args::object(
        json!({"track_id": s_id("The track (get_project lists them).")}),
        &["track_id"],
    )
}

pub fn get_track(cx: &ToolCx<'_>, a: &Value) -> Res<Value> {
    let p = cx.project();
    let id = args::track(p, A(a), "track_id")?;
    Ok(track_detail(p, id))
}

pub fn track_detail(p: &Project, id: TrackId) -> Value {
    let map = p.tempo_map();
    let t = p.track(id).expect("checked");
    let mut v = track_summary(p, t);
    v["clips"] = json!(t
        .clips
        .iter()
        .map(|c| clip_json(p, &map, c))
        .collect::<Vec<_>>());
    v["modulators"] = json!(t
        .modulators
        .iter()
        .map(|m| modulator_json(p, m))
        .collect::<Vec<_>>());
    v["note_effects"] = json!(t.note_fx.len());
    v["session_slots"] = json!(t
        .slots
        .iter()
        .filter_map(|s| s.clip.as_ref().map(|c| json!({
            "scene_id": s.scene.0,
            "clip_name": c.clip.name,
            "length_beats": beats(c.clip.length),
            "notes": c.clip.notes.len(),
        })))
        .collect::<Vec<_>>());
    if let InstrumentSpec::Pulsus(kit) = &t.instrument {
        v["pads"] = json!(kit
            .pads
            .iter()
            .map(|pad| json!({"note": pad.pitch, "name": pad.name, "voice": format!("{:?}", pad.voice)}))
            .collect::<Vec<_>>());
    }
    v
}

pub fn modulator_json(p: &Project, m: &cantus_core::Modulator) -> Value {
    json!({
        "id": m.id.0,
        "name": m.name,
        "kind": snake(&format!("{:?}", m.source.kind())),
        "source": serde_json::to_value(&m.source).unwrap_or(Value::Null),
        "amount": num(m.amount),
        "muted": m.muted,
        "routes": m.routes.iter().map(|r| json!({
            "target": target_json(p, &r.target), "depth": num(r.depth),
        })).collect::<Vec<_>>(),
    })
}

pub fn clip_notes_schema() -> Value {
    args::object(
        json!({
            "clip_id": s_id("The clip (get_track lists them)."),
            "from": args::s_pos("Only notes starting here or later"),
            "to": args::s_pos("Only notes starting before here"),
            "limit": {"type": "integer", "minimum": 1, "maximum": 4096, "default": 2000,
                "description": "The most notes (or points) to return."}
        }),
        &["clip_id"],
    )
}

pub fn get_clip_notes(cx: &ToolCx<'_>, a: &Value) -> Res<Value> {
    let p = cx.project();
    let a = A(a);
    let map = p.tempo_map();
    let (track, c) = args::clip(p, a, "clip_id")?;
    let from = args::opt_pos(&map, a, "from")?.unwrap_or(Ticks(i64::MIN));
    let to = args::opt_pos(&map, a, "to")?.unwrap_or(Ticks(i64::MAX));
    let limit = a.opt_u64("limit")?.unwrap_or(2000).clamp(1, 4096) as usize;
    let origin = c.start - c.offset;
    let mut v = json!({"clip": clip_json(p, &map, c), "track_id": track.0});
    if let Some(auto) = &c.automation {
        let range = range_of(p, &auto.target);
        let pts: Vec<Value> = auto
            .points
            .iter()
            .filter(|pt| (from..to).contains(&(origin + pt.tick)))
            .take(limit)
            .map(|pt| {
                let mut x = json!({
                    "at": pos_json(&map, origin + pt.tick),
                    "position": num(pt.value),
                    "shape": snake(&format!("{:?}", pt.shape)),
                    "tension": num(pt.tension),
                });
                if let Some(r) = range {
                    x["value"] = num(r.value(pt.value));
                }
                x
            })
            .collect();
        v["points"] = json!(pts);
        return Ok(v);
    }
    let total = c.notes.len();
    let notes: Vec<Value> = c
        .notes
        .iter()
        .filter(|n| (from..to).contains(&(origin + n.start)))
        .take(limit)
        .map(|n| note_json(&map, c, n))
        .collect();
    v["truncated"] = json!(notes.len() < total && notes.len() == limit);
    v["notes"] = json!(notes);
    Ok(v)
}

pub fn get_mixer(cx: &ToolCx<'_>, _: &Value) -> Res<Value> {
    Ok(mixer_json(cx.project()))
}

pub fn device_schema() -> Value {
    args::object(
        json!({
            "effect_id": s_id("An insert effect (get_mixer lists them). Give this or track_id."),
            "track_id": s_id("A track, for its instrument. Give this or effect_id."),
            "all": {"type": "boolean", "default": false,
                "description": "Every parameter, not only those moved from their defaults (an EQ has hundreds)."}
        }),
        &[],
    )
}

pub fn get_device(cx: &ToolCx<'_>, a: &Value) -> Res<Value> {
    let p = cx.project();
    let a = A(a);
    let all = a.opt_bool("all")?.unwrap_or(false);
    if a.has("effect_id") {
        let (host, id) = args::effect(p, a, "effect_id")?;
        let slot = p.effect(host, id).expect("found");
        let mut v = effect_entry(slot);
        v["strip_id"] = strip_json(host);
        v["params"] = effect_params(slot, all);
        return Ok(v);
    }
    if a.has("track_id") {
        let id = args::track(p, a, "track_id")?;
        let t = p.track(id).expect("checked");
        let mut v = super::json::instrument_json(&t.instrument);
        v["track_id"] = json!(id.0);
        v["params"] = instrument_params(&t.instrument, all);
        return Ok(v);
    }
    Err(ToolError::bad("effect_id", "give effect_id or track_id"))
}

fn param_row(key: &str, value: f32, info: &cantus_core::EffectParamInfo) -> Value {
    json!({
        "key": key, "value": num(value), "unit": info.unit,
        "min": num(info.min), "max": num(info.max), "default": num(info.default),
        "stepped": info.stepped,
    })
}

pub fn effect_params(slot: &EffectSlot, all: bool) -> Value {
    if let Some(ext) = &slot.spec.external {
        return json!({
            "hosted": {"name": ext.name, "vendor": ext.vendor, "id": ext.id},
            "values_by_plugin_id": ext.params.iter().map(|(k, v)| (k.to_string(), json!(v))).collect::<serde_json::Map<_, _>>(),
        });
    }
    let rows: Vec<Value> = slot
        .spec
        .kind
        .params()
        .iter()
        .map(|i| (i, slot.spec.get(i.key)))
        .filter(|(i, v)| all || *v != i.default)
        .map(|(i, v)| param_row(i.key, v, i))
        .collect();
    json!(rows)
}

pub fn instrument_params(spec: &InstrumentSpec, all: bool) -> Value {
    match spec {
        InstrumentSpec::Native(n) => {
            let mut rows: Vec<Value> = n
                .kind
                .params()
                .iter()
                .map(|i| (i, n.get(i.key)))
                .filter(|(i, v)| all || *v != i.default)
                .map(|(i, v)| param_row(i.key, v, i))
                .collect();
            for (k, t) in &n.text {
                rows.push(json!({"key": k, "text": t}));
            }
            json!(rows)
        }
        InstrumentSpec::Firmus(f) => json!(FirmusParam::ALL
            .iter()
            .map(|pp| (pp.info(), pp.get(f)))
            .filter(|(i, v)| all || *v != i.default)
            .map(|(i, v)| json!({
                "key": firmus_key(i.param), "value": num(v), "unit": i.unit,
                "min": num(i.min), "max": num(i.max), "default": num(i.default), "stepped": i.stepped,
            }))
            .collect::<Vec<_>>()),
        InstrumentSpec::Pulsus(kit) => {
            let mut rows = Vec::new();
            for f in cantus_core::pads::KitField::ALL {
                let (_, min, max, default) = f.range();
                let v = f.get(kit);
                if all || v != default {
                    rows.push(json!({
                        "key": format!("kit.{}", snake(&format!("{f:?}"))), "value": v,
                        "min": min, "max": max, "default": default, "stepped": true,
                    }));
                }
            }
            for pad in &kit.pads {
                for f in PadField::ALL.into_iter().filter(|f| f.applies(pad)) {
                    let i = f.info(pad.voice);
                    let v = f.get(pad);
                    if all || v != i.default {
                        rows.push(json!({
                            "key": pad_key(pad.pitch, f), "pad": pad.name, "value": num(v),
                            "unit": i.unit, "min": num(i.min), "max": num(i.max),
                            "default": num(i.default), "stepped": i.stepped,
                        }));
                    }
                }
            }
            json!(rows)
        }
        InstrumentSpec::External(e) => json!({
            "hosted": {"name": e.name, "vendor": e.vendor, "id": e.id},
            "values_by_plugin_id": e.params.iter().map(|(k, v)| (k.to_string(), json!(v))).collect::<serde_json::Map<_, _>>(),
        }),
    }
}

/// A Pulsus pad's field as a key: `pad36.decay`.
pub fn pad_key(note: u8, f: PadField) -> String {
    format!("pad{note}.{}", snake(&format!("{f:?}")))
}

pub fn history_schema() -> Value {
    args::object(
        json!({"limit": {"type": "integer", "minimum": 1, "maximum": 200, "default": 20,
            "description": "How many of the most recent steps."}}),
        &[],
    )
}

fn origin_name(o: Origin) -> &'static str {
    match o {
        Origin::User => "user",
        Origin::Script => "script",
        Origin::Ai => "assistant",
        Origin::Mcp => "agent",
        Origin::System => "system",
    }
}

/// `cmd.set_track_gain` → `set track gain`.
pub fn label_words(key: &str) -> String {
    key.trim_start_matches("cmd.").replace(['_', '.'], " ")
}

pub fn get_history(cx: &ToolCx<'_>, a: &Value) -> Res<Value> {
    let limit = A(a).opt_u64("limit")?.unwrap_or(20).clamp(1, 200) as usize;
    Ok(history_json(cx.bus(), limit))
}

pub fn history_json(bus: &cantus_core::CommandBus, limit: usize) -> Value {
    let row = |e: &cantus_core::HistoryEntry| json!({"what": label_words(&e.label_key), "label_key": e.label_key, "by": origin_name(e.origin)});
    json!({
        "revision": bus.revision(),
        "undo": bus.history().iter().rev().take(limit).map(row).collect::<Vec<_>>(),
        "redo": bus.redo_history().iter().rev().take(limit).map(row).collect::<Vec<_>>(),
    })
}
