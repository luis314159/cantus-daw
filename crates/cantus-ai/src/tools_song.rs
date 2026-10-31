//! The song's frame: tempo and tempo points, metre, key, loop, name, markers.

use super::args::{self, plan, s_id, s_pos, Res, A};
use super::words;
use crate::tool::{EditPlan, Said, ToolCx, ToolError};
use cantus_core::tempo::{edit, Marker, MarkerId};
use cantus_core::time::{MAX_BPM, MIN_BPM};
use cantus_core::{Command, Key, LoopRegion, ScaleKind, Ticks, TimeSignature};
use serde_json::{json, Value};

fn bpm_schema() -> Value {
    json!({"type": "number", "minimum": MIN_BPM, "maximum": MAX_BPM, "description": "Beats (quarter notes) per minute, 20 to 999."})
}

pub fn tempo_schema() -> Value {
    args::object(json!({"bpm": bpm_schema()}), &["bpm"])
}

pub fn set_tempo(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let bpm = A(a).f64_in("bpm", MIN_BPM, MAX_BPM)?;
    let cmd = edit::set_tempo_at(cx.project(), Ticks::ZERO, bpm);
    Ok(plan(
        cmd,
        Said::new("ai.tools.set_tempo").nested("bpm", words::num(bpm)),
        json!({"tempo_bpm": bpm}),
    ))
}

pub fn tempo_point_schema() -> Value {
    args::object(
        json!({
            "at": s_pos("Where the tempo changes"),
            "beat": {"type": "number", "minimum": 0, "maximum": 1_000_000, "description": "The same as `at` in beats (kept for older callers)."},
            "bpm": bpm_schema(),
            "ramp": {"type": "boolean", "description": "Move linearly from this point's tempo to the next point's (an accelerando or ritardando). Left out, a point already there keeps its own."}
        }),
        &[],
    )
}

pub fn set_tempo_point(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let map = p.tempo_map();
    let at = match (a.get("at"), a.get("beat")) {
        (Some(v), _) => args::pos_of(&map, v, "at")?,
        (None, Some(v)) => args::pos_of(&map, v, "beat")?,
        (None, None) => return Err(ToolError::bad("at", "is required")),
    };
    let bpm = a.f64_in("bpm", MIN_BPM, MAX_BPM)?;
    // Unsaid, the point's own ramp stays (round 29's review 5).
    let ramp = match a.opt_bool("ramp")? {
        Some(r) => r,
        None => p
            .tempo_points
            .iter()
            .find(|t| t.tick == at)
            .is_some_and(|t| t.ramp),
    };
    let cmd = edit::add_tempo_point(p, at, bpm, ramp);
    Ok(plan(
        cmd,
        Said::new("ai.tools.set_tempo_point")
            .nested("bpm", words::num(bpm))
            .nested("at", words::pos(&map, at)),
        json!({"at": args::pos_json(&map, at), "bpm": bpm, "ramp": ramp}),
    ))
}

pub fn song_schema() -> Value {
    let scales: Vec<Value> = ScaleKind::ALL
        .iter()
        .map(|s| serde_json::to_value(s).unwrap_or(Value::Null))
        .collect();
    args::object(
        json!({
            "name": {"type": "string", "minLength": 1, "maxLength": 120, "description": "The project's name."},
            "time_signature": {"type": "string", "pattern": "^[0-9]{1,2}/(1|2|4|8|16|32)$", "description": "A metre such as \"4/4\", \"3/4\", \"6/8\", \"7/8\"."},
            "time_signature_at_bar": {"type": "integer", "minimum": 1, "default": 1, "description": "The bar the metre starts at (1: the song's own; later bars add a metre change there)."},
            "key": {"type": ["object", "null"], "description": "The project's key, which the piano roll and lint_music use; null clears it.",
                "properties": {
                    "root": {"type": "string", "description": "The tonic: C, C#, Db, D … B."},
                    "scale": {"enum": scales}
                },
                "required": ["root", "scale"], "additionalProperties": false},
            "loop": {"type": ["object", "null"], "description": "The loop region; null removes it.",
                "properties": {"start": s_pos("Its start"), "end": s_pos("Its end"), "enabled": {"type": "boolean", "default": true}},
                "required": ["start", "end"], "additionalProperties": false},
            "metronome": {"type": "boolean", "description": "The click on or off."}
        }),
        &[],
    )
}

fn parse_metre(s: &str) -> Option<TimeSignature> {
    let (n, d) = s.trim().split_once('/')?;
    let n: u8 = n.trim().parse().ok()?;
    let d: u8 = d.trim().parse().ok()?;
    ((1..=32).contains(&n) && [1, 2, 4, 8, 16, 32].contains(&d)).then(|| TimeSignature::new(n, d))
}

pub fn set_song(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let map = p.tempo_map();
    let mut cmds = Vec::new();
    let mut done = serde_json::Map::new();
    if let Some(name) = a.opt_name("name", 120)? {
        done.insert("name".into(), json!(name));
        cmds.push(Command::SetProjectName { name });
    }
    if let Some(s) = a.opt_str("time_signature")? {
        let sig = parse_metre(s).ok_or_else(|| {
            ToolError::bad(
                "time_signature",
                format!("{s:?} is not a metre such as 3/4 or 6/8"),
            )
        })?;
        let bar = a.opt_u64("time_signature_at_bar")?.unwrap_or(1).max(1);
        if bar == 1 {
            cmds.push(edit::set_metre_at(p, Ticks::ZERO, sig));
        } else {
            cmds.push(Command::SetMetreAt {
                tick: map.bar_tick(bar as i64),
                signature: sig,
            });
        }
        done.insert(
            "time_signature".into(),
            json!({"signature": sig.to_string(), "from_bar": bar}),
        );
    }
    if a.0.get("key").is_some() {
        let key = match a.get("key") {
            None => None,
            Some(k) => {
                let root = k["root"]
                    .as_str()
                    .ok_or_else(|| ToolError::bad("key.root", "is required"))?;
                let (pc, rest) = cantus_core::harmony::chord::parse_pc(root)
                    .filter(|(_, r)| r.is_empty())
                    .ok_or_else(|| {
                        ToolError::bad("key.root", format!("{root:?} is not a note name"))
                    })?;
                let _ = rest;
                let scale: ScaleKind = serde_json::from_value(k["scale"].clone())
                    .map_err(|_| ToolError::bad("key.scale", "is not a scale (see the schema)"))?;
                Some(Key::new(pc, scale))
            }
        };
        done.insert(
            "key".into(),
            json!(key.map(|k| format!("{} {:?}", k.root_name(), k.scale))),
        );
        cmds.push(Command::SetProjectKey { key });
    }
    if a.0.get("loop").is_some() {
        match a.get("loop") {
            None => cmds.push(Command::SetLoopRegion { region: None }),
            Some(l) => {
                let la = A(l);
                let start = args::pos(&map, la, "start")?;
                let end = args::pos(&map, la, "end")?;
                if end <= start {
                    return Err(ToolError::bad("loop.end", "must be after loop.start"));
                }
                cmds.push(Command::SetLoopRegion {
                    region: Some(LoopRegion { start, end }),
                });
                cmds.push(Command::SetLoopEnabled {
                    enabled: la.opt_bool("enabled")?.unwrap_or(true),
                });
                done.insert(
                    "loop".into(),
                    json!({"start": args::pos_json(&map, start), "end": args::pos_json(&map, end)}),
                );
            }
        }
    }
    if let Some(on) = a.opt_bool("metronome")? {
        cmds.push(Command::SetMetronome { enabled: on });
        done.insert("metronome".into(), json!(on));
    }
    let said = Said::new("ai.tools.set_song").nested("what", words::fields(done.keys()));
    Ok(plan(
        args::one(cmds, "cmd.ai.set_song")?,
        said,
        Value::Object(done),
    ))
}

pub fn marker_schema() -> Value {
    args::object(
        json!({
            "marker_id": s_id("A marker to move or rename (get_project lists them); left out, a new marker is made."),
            "at": s_pos("Where the marker is"),
            "name": {"type": "string", "minLength": 1, "maxLength": 80, "description": "Its name (\"Chorus\")."}
        }),
        &[],
    )
}

pub fn set_marker(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let map = cx.project().tempo_map();
    let at = args::opt_pos(&map, a, "at")?;
    let name = a.opt_name("name", 80)?;
    if let Some(raw) = a.opt_u64("marker_id")? {
        let old = cx
            .project()
            .markers
            .iter()
            .find(|m| m.id.0 == raw)
            .cloned()
            .ok_or_else(|| ToolError::NotFound(format!("marker {raw}")))?;
        if at.is_none() && name.is_none() {
            return Err(ToolError::bad("at", "give at or name to change"));
        }
        let marker = Marker {
            tick: at.unwrap_or(old.tick),
            name: name.unwrap_or(old.name.clone()),
            ..old
        };
        let said = Said::new("ai.tools.set_marker")
            .name("name", args::clean(&marker.name))
            .nested("at", words::pos(&map, marker.tick));
        let result =
            json!({"marker_id": raw, "at": args::pos_json(&map, marker.tick), "name": marker.name});
        return Ok(plan(Command::SetMarker { marker }, said, result));
    }
    let at = at.ok_or_else(|| ToolError::bad("at", "is required for a new marker"))?;
    let name = name.ok_or_else(|| ToolError::bad("name", "is required for a new marker"))?;
    let id = MarkerId(cx.bus_mut().alloc_id());
    let said = Said::new("ai.tools.add_marker")
        .name("name", args::clean(&name))
        .nested("at", words::pos(&map, at));
    let result = json!({"marker_id": id.0, "at": args::pos_json(&map, at), "name": name});
    Ok(plan(
        Command::AddMarker {
            marker: Marker {
                id,
                tick: at,
                name,
                color: None,
            },
        },
        said,
        result,
    ))
}
