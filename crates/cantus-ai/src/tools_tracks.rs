//! Tracks and their instruments: add one (with a mixer strip of its own), set its name,
//! colour, level, pan, mute, solo, place and routing, swap its instrument, set the
//! instrument's parameters by key in their units.

use super::args::{self, plan, s_id, InstrumentChoice, Res, A};
use super::json::{firmus_key, snake};
use super::words;
use crate::tool::{EditPlan, Said, ToolCx, ToolError};
use cantus_core::pads::{KitField, PadField};
use cantus_core::{
    Color, Command, FirmusParam, InstrumentParam, InstrumentSpec, NativeSpec, Project,
    PulsusParams, TrackId,
};
use serde_json::{json, Map, Value};

/// A fresh instrument of `choice`, as the instrument picker makes it.
pub fn fresh_instrument(choice: InstrumentChoice) -> InstrumentSpec {
    match choice {
        InstrumentChoice::Firmus => InstrumentSpec::default(),
        InstrumentChoice::Pulsus => InstrumentSpec::Pulsus(PulsusParams::default()),
        InstrumentChoice::Native(k) => InstrumentSpec::Native(
            cantus_core::model::note_expr::defaults::with_routes(NativeSpec::new(k)),
        ),
    }
}

/// `base`, or `base 2`, `base 3`… — the first a track does not have.
pub fn unique_track_name(p: &Project, base: &str) -> String {
    if !p.tracks.iter().any(|t| t.name == base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|n| !p.tracks.iter().any(|t| &t.name == n))
        .expect("a free name")
}

fn kind_schema() -> Value {
    let doc: Vec<String> = args::INSTRUMENTS
        .iter()
        .map(|(id, name, _, what)| format!("{id} ({name}: {what})"))
        .collect();
    json!({"enum": args::instrument_ids(), "description": format!("The instrument: {}.", doc.join("; "))})
}

pub fn add_track_schema() -> Value {
    args::object(
        json!({
            "kind": kind_schema(),
            "name": {"type": "string", "minLength": 1, "maxLength": 80, "description": "The track's name; left out, the instrument's (made unique)."},
            "index": {"type": "integer", "minimum": 0, "description": "Where in the track list (0 is the top); left out, the end."},
            "color": {"type": "string", "pattern": "^#[0-9a-fA-F]{6}$", "description": "\"#rrggbb\"; left out, the palette's next."}
        }),
        &[],
    )
}

pub fn add_track(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let kind = a
        .opt_choice("kind", &args::instrument_ids())?
        .unwrap_or("firmus");
    let choice = args::instrument_choice(kind).expect("checked");
    let spec = fresh_instrument(choice);
    let base = a
        .opt_name("name", 80)?
        .unwrap_or_else(|| args::instrument_name(&spec));
    let name = unique_track_name(cx.project(), &base);
    let color = a
        .opt_str("color")?
        .map(|c| parse_color(c, "color"))
        .transpose()?;
    let len = cx.project().tracks.len();
    let index = a.opt_u64("index")?.map_or(len, |i| (i as usize).min(len));
    let bus = cx.bus_mut();
    let mut track = bus.new_track(name.clone());
    track.instrument = spec;
    if let Some(c) = color {
        track.color = c;
    }
    let id = track.id;
    let mut cmds = Vec::new();
    // Inserts live on a strip: a track with none would have nowhere to put an effect.
    let strip = match track.strip {
        Some(s) => s,
        None => {
            let s = bus.new_strip(name.clone());
            let sid = s.id;
            let at = bus.project().strips.len();
            cmds.push(Command::InsertStrip {
                index: at,
                strip: Box::new(s),
            });
            track.strip = Some(sid);
            sid
        }
    };
    // The strip first, then the track that feeds it.
    cmds.push(Command::InsertTrack {
        index,
        track: Box::new(track),
    });
    let said = Said::new("ai.tools.add_track")
        .arg("kind", kind)
        .name("name", args::clean(&name));
    let result = json!({"track_id": id.0, "strip_id": strip.0, "name": name, "instrument": kind, "index": index});
    Ok(plan(args::one(cmds, "cmd.ai.add_track")?, said, result))
}

pub fn parse_color(s: &str, k: &str) -> Res<Color> {
    let h = s.trim().trim_start_matches('#');
    let bad = || ToolError::bad(k, format!("{s:?} is not \"#rrggbb\""));
    if h.len() != 6 {
        return Err(bad());
    }
    let n = u32::from_str_radix(h, 16).map_err(|_| bad())?;
    Ok(Color {
        r: (n >> 16) as u8,
        g: (n >> 8) as u8,
        b: n as u8,
    })
}

pub fn set_track_schema() -> Value {
    args::object(
        json!({
            "track_id": s_id("The track."),
            "name": {"type": "string", "minLength": 1, "maxLength": 80},
            "color": {"type": "string", "pattern": "^#[0-9a-fA-F]{6}$", "description": "\"#rrggbb\"."},
            "gain_db": {"type": "number", "minimum": -96, "maximum": 12, "description": "The track's own level in dB (before its strip's fader), -96 to +12."},
            "pan": {"type": "number", "minimum": -1, "maximum": 1, "description": "The track's own pan, -1 left … 1 right."},
            "muted": {"type": "boolean"},
            "soloed": {"type": "boolean"},
            "index": {"type": "integer", "minimum": 0, "description": "Move the track to this place in the list (0 is the top)."},
            "strip_id": args::s_strip("Route the track into this strip (\"master\": straight to the master, with no inserts)")
        }),
        &["track_id"],
    )
}

pub fn set_track(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let id = args::track(p, a, "track_id")?;
    let t = p.track(id).expect("checked");
    let mut cmds = Vec::new();
    let mut done = Map::new();
    if let Some(name) = a.opt_name("name", 80)? {
        done.insert("name".into(), json!(name));
        cmds.push(Command::SetTrackName { id, name });
    }
    if let Some(c) = a.opt_str("color")? {
        let color = parse_color(c, "color")?;
        done.insert("color".into(), json!(c));
        cmds.push(Command::SetTrackColor { id, color });
    }
    if let Some(g) = a.opt_in("gain_db", -96.0, 12.0)? {
        done.insert("gain_db".into(), json!(g));
        cmds.push(Command::SetTrackGain {
            id,
            gain_db: g as f32,
        });
    }
    if let Some(pan) = a.opt_in("pan", -1.0, 1.0)? {
        done.insert("pan".into(), json!(pan));
        cmds.push(Command::SetTrackPan {
            id,
            pan: pan as f32,
        });
    }
    if let Some(m) = a.opt_bool("muted")? {
        done.insert("muted".into(), json!(m));
        cmds.push(Command::SetTrackMute { id, muted: m });
    }
    if let Some(s) = a.opt_bool("soloed")? {
        done.insert("soloed".into(), json!(s));
        cmds.push(Command::SetTrackSolo { id, soloed: s });
    }
    if let Some(i) = a.opt_u64("index")? {
        let index = (i as usize).min(p.tracks.len() - 1);
        done.insert("index".into(), json!(index));
        cmds.push(Command::MoveTrack { id, index });
    }
    if let Some(v) = a.get("strip_id") {
        let s = args::strip_of(p, v, "strip_id")?;
        let strip = (!s.is_master()).then_some(s);
        done.insert("strip_id".into(), args::strip_json(s));
        if t.strip != strip {
            cmds.push(Command::SetTrackStrip { id, strip });
        }
    }
    let said = Said::new("ai.tools.set_track")
        .name("name", args::clean(&t.name))
        .nested("what", words::fields(done.keys()));
    done.insert("track_id".into(), json!(id.0));
    Ok(plan(
        args::one(cmds, "cmd.ai.set_track")?,
        said,
        Value::Object(done),
    ))
}

pub fn set_instrument_schema() -> Value {
    args::object(
        json!({"track_id": s_id("The track."), "kind": kind_schema()}),
        &["track_id", "kind"],
    )
}

/// Swap a track's instrument for a fresh one (its settings are lost: destructive).
pub fn set_instrument(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let id = args::track(p, a, "track_id")?;
    let kind = a
        .opt_choice("kind", &args::instrument_ids())?
        .ok_or_else(|| ToolError::bad("kind", "is required"))?;
    let t = p.track(id).expect("checked");
    let spec = fresh_instrument(args::instrument_choice(kind).expect("checked"));
    let old_name = args::instrument_name(&t.instrument);
    let new_name = args::instrument_name(&spec);
    let mut cmds = vec![Command::SetInstrument {
        track: id,
        spec: Box::new(spec),
    }];
    // A track named after its instrument takes the new one's name, as *Swap* does.
    // Only the instrument's own name, or it with a number (`Unda 2`): a name the user gave
    // stays (review #15).
    let renamed = t.name == old_name
        || t.name
            .strip_prefix(&format!("{old_name} "))
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
    if renamed && t.name != new_name {
        cmds.push(Command::SetTrackName {
            id,
            name: unique_track_name(p, &new_name),
        });
    }
    let said = Said::new("ai.tools.set_instrument")
        .name("name", args::clean(&t.name))
        .arg("kind", kind);
    Ok(plan(
        args::one(cmds, "cmd.ai.set_instrument")?,
        said,
        json!({"track_id": id.0, "instrument": kind}),
    ))
}

pub fn instrument_param_schema() -> Value {
    args::object(
        json!({
            "track_id": s_id("The track whose instrument to set."),
            "params": {"type": "object", "minProperties": 1, "maxProperties": 256,
                "additionalProperties": {"type": ["number", "string", "boolean"]},
                "description": "Keys and values in the parameter's own unit (Hz, dB, s, ms, st, 0-1 …), as get_unit or get_device lists them; a stepped parameter takes its choice's number; true/false for on/off switches. Native instruments use their keys (\"filter.cutoff\", \"pedal\"); Firmus its keys (\"cutoff\", \"amp_release\"); Pulsus \"pad<note>.<field>\" (\"pad36.decay\", \"pad38.tune\"), \"pad<note>.name\" (text) and \"kit.<field>\"; a hosted plug-in its parameter ids (\"1234\") in the plug-in's own range. Text keys (names) take text; a sample's path only to an existing file inside the project's folder."},
            "param": {"type": "string", "description": "One key (with `value`), kept for older callers; prefer `params`."},
            "value": {"type": ["number", "string", "boolean"]}
        }),
        &["track_id"],
    )
}

fn number(v: &Value, k: &str) -> Res<f32> {
    match v {
        Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
        Value::Number(n) => n
            .as_f64()
            .filter(|x| x.is_finite())
            .map(|x| x as f32)
            .ok_or_else(|| ToolError::bad(k, "must be a finite number")),
        _ => Err(ToolError::bad(k, "must be a number")),
    }
}

fn in_range(k: &str, v: f32, min: f32, max: f32) -> Res<f32> {
    if v < min - 1e-6 || v > max + 1e-6 {
        return Err(ToolError::bad(
            k,
            format!("must be {min} to {max}, not {v}"),
        ));
    }
    Ok(v.clamp(min, max))
}

/// The command that sets `key` of `track`'s instrument to `v`, checked against its range.
pub fn instrument_value_command(
    p: &Project,
    track: TrackId,
    key: &str,
    v: &Value,
    host: &dyn crate::tool::ToolHost,
) -> Res<Command> {
    let t = p.track(track).expect("checked");
    match &t.instrument {
        InstrumentSpec::Native(n) => {
            if n.kind.is_text_key(key) {
                let mut text = v
                    .as_str()
                    .ok_or_else(|| ToolError::bad(key, "takes text"))?
                    .to_string();
                if n.kind.is_asset_key(key) {
                    text = inside(host, &text, key)?;
                }
                return Ok(Command::SetInstrumentText {
                    track,
                    key: key.to_string(),
                    text: Some(text),
                });
            }
            let info = n.kind.param(key).ok_or_else(|| {
                ToolError::bad(
                    key,
                    format!(
                        "{} has no parameter {key:?} (get_unit lists them)",
                        args::native_id(n.kind)
                    ),
                )
            })?;
            let value = in_range(key, number(v, key)?, info.min, info.max)?;
            Ok(Command::SetInstrumentValue {
                track,
                key: key.to_string(),
                value,
            })
        }
        InstrumentSpec::Firmus(_) => {
            let param = FirmusParam::ALL
                .into_iter()
                .find(|f| firmus_key(*f) == key)
                .ok_or_else(|| {
                    ToolError::bad(key, "Firmus has no such parameter (get_unit lists them)")
                })?;
            let i = param.info();
            let value = in_range(key, number(v, key)?, i.min, i.max)?;
            Ok(Command::SetInstrumentParam {
                track,
                param: param.into(),
                value,
            })
        }
        InstrumentSpec::Pulsus(kit) => {
            if let Some(field) = key.strip_prefix("kit.") {
                let f = KitField::ALL
                    .into_iter()
                    .find(|f| snake(&format!("{f:?}")) == field)
                    .ok_or_else(|| {
                        ToolError::bad(key, "no such kit field (version, seed, hat_cc, hat_invert)")
                    })?;
                let (_, min, max, _) = f.range();
                let value = in_range(key, number(v, key)?, min, max)?;
                return Ok(Command::SetInstrumentParam {
                    track,
                    param: InstrumentParam::PulsusKit(f),
                    value,
                });
            }
            let (pad_part, field) = key
                .split_once('.')
                .filter(|(p, _)| p.starts_with("pad"))
                .ok_or_else(|| {
                    ToolError::bad(
                        key,
                        "a Pulsus key is \"pad<note>.<field>\" or \"kit.<field>\"",
                    )
                })?;
            let note: u8 = pad_part[3..]
                .parse()
                .map_err(|_| ToolError::bad(key, "the pad is its note: \"pad36.decay\""))?;
            let pad = kit
                .pads
                .iter()
                .find(|x| x.pitch == note)
                .ok_or_else(|| ToolError::NotFound(format!("a pad on note {note}")))?;
            if field == "name" {
                let name = v
                    .as_str()
                    .ok_or_else(|| ToolError::bad(key, "takes text"))?;
                return Ok(Command::SetPadName {
                    track,
                    pad: pad.id,
                    name: name.chars().take(40).collect(),
                });
            }
            let f = PadField::ALL
                .into_iter()
                .find(|f| snake(&format!("{f:?}")) == field)
                .ok_or_else(|| ToolError::bad(key, "no such pad field (get_unit lists them)"))?;
            if !f.applies(pad) {
                return Err(ToolError::bad(
                    key,
                    format!("this pad's voice ({:?}) has no {field}", pad.voice),
                ));
            }
            let i = f.info(pad.voice);
            let value = in_range(key, number(v, key)?, i.min, i.max)?;
            Ok(Command::SetInstrumentParam {
                track,
                param: InstrumentParam::Pulsus {
                    pad: pad.id,
                    field: f,
                },
                value,
            })
        }
        InstrumentSpec::External(_) => {
            let id: u32 = key.parse().map_err(|_| {
                ToolError::bad(key, "a hosted plug-in's parameters are its numeric ids")
            })?;
            Ok(Command::SetInstrumentPluginParam {
                track,
                id,
                value: Some(number(v, key)? as f64),
            })
        }
    }
}

/// A file a tool may point an instrument at: an existing file inside the project's folder
/// (a relative path is the folder's). The answer is the path as it really is on disk —
/// the one the instrument is given.
pub fn inside(host: &dyn crate::tool::ToolHost, path: &str, k: &str) -> Res<String> {
    let p = std::path::Path::new(path);
    let full = match (p.is_absolute(), host.project_dir()) {
        (true, _) => p.to_path_buf(),
        (false, Some(d)) => d.join(p),
        (false, None) => {
            return Err(ToolError::Refused(format!(
                "{k}: the project is not saved yet, so a relative path has no folder to start from"
            )))
        }
    };
    // The project's folder only (review #12): an edit may not reach the folders the user
    // allowed for file tools, which ask for leave of their own.
    let dir = host
        .project_dir()
        .and_then(|d| std::fs::canonicalize(d).ok());
    let real = std::fs::canonicalize(&full).ok();
    match (dir, real) {
        (Some(d), Some(r)) if r.starts_with(&d) && r.is_file() => Ok(r.display().to_string()),
        _ => Err(ToolError::Refused(format!(
            "{k}: only an existing file inside the project's folder"
        ))),
    }
}

pub fn set_instrument_param(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let host: &dyn crate::tool::ToolHost = &*cx.host;
    let p = host.bus().project();
    let track = args::track(p, a, "track_id")?;
    let mut pairs: Vec<(String, Value)> = match a.opt_object("params")? {
        Some(m) => m.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        None => Vec::new(),
    };
    if let Some(k) = a.opt_str("param")? {
        pairs.push((k.to_string(), a.req("value")?.clone()));
    }
    if pairs.is_empty() || pairs.len() > 256 {
        return Err(ToolError::bad("params", "give 1 to 256 keys"));
    }
    let mut cmds = Vec::with_capacity(pairs.len());
    for (k, v) in &pairs {
        cmds.push(instrument_value_command(p, track, k, v, host)?);
    }
    let t = p.track(track).expect("checked");
    let said = Said::new("ai.tools.set_instrument_param")
        .name("name", args::clean(&t.name))
        .nested(
            "keys",
            words::list(pairs.iter().map(|(k, _)| words::key(k)).collect()),
        );
    let set: Map<String, Value> = pairs.into_iter().collect();
    Ok(plan(
        args::one(cmds, "cmd.ai.set_instrument_param")?,
        said,
        json!({"track_id": track.0, "set": set}),
    ))
}
