//! The tuning (round 42, ADR 0155; r42 fix2-model, review 9): read the project's tuning or a
//! track's, and set either — equal temperament, a named temperament in a key (or following
//! the song's key), a Scala scale from its file's text (and a `.kbm`'s), the MTS-ESP
//! master's, the diapason — through `SetProjectTuning` and `SetTrackTuning`, one undo step,
//! refused by the commands' own check (`tuning_cmd::check`) as the window is.

use super::args::{self, plan, Res, A};
use super::words;
use crate::tool::{EditPlan, Said, ToolCx, ToolError};
use cantus_core::scale::ROOT_NAMES;
use cantus_core::tuning::{
    Temperament, TrackTuning, Tuning, TuningSystem, A4_MAX_HZ, A4_MIN_HZ,
};
use cantus_core::tuning_scala;
use cantus_core::Command;
use serde_json::{json, Value};

/// A temperament's id as the tools name it (its name in the song file).
fn temperament_id(t: Temperament) -> String {
    serde_json::to_value(t)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn temperament_ids() -> Vec<String> {
    Temperament::ALL.iter().map(|t| temperament_id(*t)).collect()
}

/// A tuning as the tools read it.
pub fn tuning_json(t: &Tuning) -> Value {
    let mut v = json!({
        "a4_hz": t.a4_hz,
        "is_default": t.is_default(),
    });
    match &t.system {
        TuningSystem::Equal => v["system"] = json!("equal"),
        TuningSystem::Temperament {
            id,
            key,
            follow_song_key,
        } => {
            v["system"] = json!("temperament");
            v["temperament"] = json!(temperament_id(*id));
            v["key"] = json!(ROOT_NAMES[usize::from(key % 12)]);
            v["follow_song_key"] = json!(follow_song_key);
        }
        TuningSystem::Scale(s) => {
            v["system"] = json!("scale");
            v["scale"] = json!({
                "description": s.description,
                "count": s.degrees.len(),
                "degrees_cents": s.degrees.iter().take(128).map(|d| (d.cents() * 1000.0).round() / 1000.0).collect::<Vec<_>>(),
                "scl": tuning_scala::write_scl(s),
            });
        }
        TuningSystem::External => v["system"] = json!("external"),
    }
    if let Some(m) = &t.map {
        v["keyboard_map"] = json!({
            "size": m.size,
            "first_key": m.first_key,
            "last_key": m.last_key,
            "middle_key": m.middle_key,
            "reference_key": m.reference_key,
            "reference_hz": m.reference_hz,
            "octave_degree": m.octave_degree,
            "unmapped": m.mapping.iter().filter(|e| e.is_none()).count(),
            "kbm": tuning_scala::write_kbm(m),
        });
    }
    v
}

/// What a tuning plays: C4 … B4's cents from 12-TET at A440, and A4's frequency (an
/// unmapped key is `null`). `External` without a master reads as equal temperament at its
/// diapason (the application reads the master's table; this crate has no link to it).
fn played_json(t: &Tuning) -> Value {
    let table = t.offsets();
    let cents: Vec<Value> = (60..72u8)
        .map(|k| {
            let o = f64::from(table.at(k));
            if o.is_nan() {
                Value::Null
            } else {
                json!((o * 100_000.0).round() / 1000.0)
            }
        })
        .collect();
    let a4 = f64::from(table.at(69));
    json!({
        "cents_c4_to_b4": cents,
        "a4_hz": if a4.is_nan() { Value::Null } else { json!(cantus_core::tuning_mts::pitch_hz(69.0 + a4)) },
    })
}

pub fn get_tuning_schema() -> Value {
    args::object(
        json!({
            "track_id": {"type": "integer", "minimum": 0, "description": "A track: how it is tuned (follow the project's, equal temperament, or its own) and what it plays. Left out: the project's tuning."}
        }),
        &[],
    )
}

pub fn get_tuning(cx: &ToolCx<'_>, a: &Value) -> Res<Value> {
    let p = cx.project();
    let song_key = p.key.map(|k| k.root);
    match args::opt_track(p, A(a), "track_id")? {
        None => Ok(json!({
            "tuning": tuning_json(&p.tuning),
            "plays": played_json(&p.tuning.resolved_for_key(song_key)),
        })),
        Some(id) => {
            let t = p.track(id).expect("checked");
            let plays = t.tuning.resolve(&p.tuning).resolved_for_key(song_key);
            let mut v = json!({
                "track_id": id.0,
                "mode": match t.tuning {
                    TrackTuning::Follow => "follow",
                    TrackTuning::Equal => "equal",
                    TrackTuning::Own { .. } => "own",
                },
                "plays": played_json(&plays),
                "plays_tuning": tuning_json(&plays),
            });
            if let TrackTuning::Own { tuning } = &t.tuning {
                v["tuning"] = tuning_json(tuning);
            }
            Ok(v)
        }
    }
}

pub fn set_tuning_schema() -> Value {
    args::object(
        json!({
            "track_id": {"type": "integer", "minimum": 0, "description": "Set this track's tuning; left out, the project's (every track that follows it)."},
            "mode": {"type": "string", "enum": ["follow", "equal", "own"], "description": "A track only: follow the project's tuning, play equal temperament at A440 whatever the project says (a drum bus, a sampled loop), or its own (the default when a system, a diapason or a Scala text is given)."},
            "system": {"type": "string", "enum": ["equal", "temperament", "scale", "external"], "description": "Equal temperament, a named temperament (`temperament`, `key`), a Scala scale (`scala`), or the MTS-ESP master's table (another program's). Left out: the current system, changed by the other fields."},
            "temperament": {"type": "string", "enum": temperament_ids(), "description": "The historical or just temperament, rotated so its best key sits on `key`."},
            "key": {"type": ["string", "integer"], "description": "The temperament's key: a note name (C, F#, Eb …) or 0–11 (0 = C)."},
            "follow_song_key": {"type": "boolean", "description": "The temperament follows the song's key (set_song): a change of key rotates it."},
            "a4_hz": {"type": "number", "minimum": A4_MIN_HZ, "maximum": A4_MAX_HZ, "description": "The diapason, A4 in Hz (440 modern, 415 baroque, 432, 442, 443 …). With a keyboard map it scales the map's reference."},
            "scala": {"type": "string", "maxLength": 200000, "description": "A Scala .scl file's text (or an Ableton .ascl's, whose @ABL REFERENCE_PITCH is its reference): the song keeps the scale, never a path. With no `kbm`, degree 0 sits on middle C (key 60) at C4 of 12-TET, as Scala and Surge read a scale."},
            "kbm": {"type": "string", "maxLength": 200000, "description": "A Scala .kbm keyboard map's text, with `scala` (or over the current scale): which key plays which degree, `x` for a key that sounds nothing, and the reference key's frequency."}
        }),
        &[],
    )
}

fn key_of(v: &Value) -> Res<u8> {
    if let Some(n) = v.as_u64() {
        return u8::try_from(n)
            .ok()
            .filter(|k| *k < 12)
            .ok_or_else(|| ToolError::bad("key", "is 0 (C) to 11 (B)"));
    }
    let s = v
        .as_str()
        .ok_or_else(|| ToolError::bad("key", "a note name or 0 to 11"))?;
    cantus_core::harmony::chord::parse_pc(s)
        .filter(|(_, rest)| rest.is_empty())
        .map(|(pc, _)| pc)
        .ok_or_else(|| ToolError::bad("key", format!("{s:?} is not a note name")))
}

fn scala_refused(k: &'static str, e: &tuning_scala::ScalaError) -> ToolError {
    ToolError::bad(k, e.to_string())
}

pub fn set_tuning(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let p = cx.project();
    let a = A(a);
    let track = args::opt_track(p, a, "track_id")?;
    let mode = a.opt_choice("mode", &["follow", "equal", "own"])?;
    if track.is_none() && mode.is_some() {
        return Err(ToolError::bad("mode", "is a track's (give track_id)"));
    }
    let shaping = ["system", "temperament", "key", "follow_song_key", "a4_hz", "scala", "kbm"]
        .iter()
        .any(|k| a.has(k));
    // The tuning the fields change: the project's, or what the track plays now.
    let now = match track {
        None => p.tuning.clone(),
        Some(id) => p.track(id).expect("checked").tuning.resolve(&p.tuning).into_owned(),
    };
    if let (Some(id), Some(m @ ("follow" | "equal"))) = (track, mode) {
        if shaping {
            return Err(ToolError::bad("mode", "follow and equal take no tuning fields"));
        }
        let (tuning, key) = if m == "follow" {
            (TrackTuning::Follow, "ai.tools.set_track_tuning_follow")
        } else {
            (TrackTuning::Equal, "ai.tools.set_track_tuning_equal")
        };
        let said = Said::new(key).name("track", words::track_name(p, id));
        return Ok(plan(
            Command::SetTrackTuning { track: id, tuning },
            said,
            json!({"track_id": id.0, "mode": m}),
        ));
    }
    if !shaping {
        return Err(ToolError::bad(
            "system",
            "say what to set: a system, a temperament, a key, a4_hz, a Scala text or a mode",
        ));
    }
    let mut tu = now;
    if let Some(text) = a.opt_str("scala")? {
        let scale =
            tuning_scala::read_scl(text.as_bytes()).map_err(|e| scala_refused("scala", &e))?;
        let ascl = tuning_scala::read_ascl_map(text.as_bytes(), &scale)
            .map_err(|e| scala_refused("scala", &e))?;
        tu.system = TuningSystem::Scale(scale);
        tu.map = ascl;
    } else if let Some(s) = a.opt_choice("system", &["equal", "temperament", "scale", "external"])? {
        tu.system = match (s, &tu.system) {
            ("equal", _) => TuningSystem::Equal,
            ("external", _) => TuningSystem::External,
            ("scale", TuningSystem::Scale(_)) => tu.system.clone(),
            ("scale", _) => {
                return Err(ToolError::bad("scala", "a Scala scale needs its file's text"))
            }
            (_, TuningSystem::Temperament { .. }) => tu.system.clone(),
            _ => TuningSystem::Temperament {
                id: Temperament::WerckmeisterIii,
                key: 0,
                follow_song_key: false,
            },
        };
        if !matches!(tu.system, TuningSystem::Scale(_)) {
            // A keyboard map belongs to a scale (the window's rule, ADR 0155 §6).
            tu.map = None;
        }
    }
    if let Some(name) = a.opt_choice("temperament", &temperament_ids().iter().map(String::as_str).collect::<Vec<_>>())? {
        let id = Temperament::ALL
            .into_iter()
            .find(|t| temperament_id(*t) == name)
            .expect("in the enum");
        let (key, follow) = match tu.system {
            TuningSystem::Temperament {
                key,
                follow_song_key,
                ..
            } => (key, follow_song_key),
            _ => (0, false),
        };
        tu.system = TuningSystem::Temperament {
            id,
            key,
            follow_song_key: follow,
        };
        tu.map = None;
    }
    if a.has("key") || a.has("follow_song_key") {
        let TuningSystem::Temperament {
            key,
            follow_song_key,
            ..
        } = &mut tu.system
        else {
            return Err(ToolError::bad("key", "a key belongs to a temperament"));
        };
        if let Some(v) = a.get("key") {
            *key = key_of(v)?;
        }
        if let Some(f) = a.opt_bool("follow_song_key")? {
            *follow_song_key = f;
        }
    }
    if let Some(text) = a.opt_str("kbm")? {
        if !matches!(tu.system, TuningSystem::Scale(_)) {
            return Err(ToolError::bad("kbm", "a keyboard map belongs to a Scala scale"));
        }
        tu.map = Some(tuning_scala::read_kbm(text.as_bytes()).map_err(|e| scala_refused("kbm", &e))?);
    }
    // A map read with the scale sounds as written: the diapason goes back to 440 then (the
    // window's import), unless one is given.
    if (a.has("kbm") || (a.has("scala") && tu.map.is_some())) && !a.has("a4_hz") {
        tu.a4_hz = cantus_core::tuning::A4_DEFAULT_HZ;
    }
    if let Some(hz) = a.opt_in("a4_hz", A4_MIN_HZ, A4_MAX_HZ)? {
        tu.a4_hz = hz;
    }
    cantus_core::tuning_cmd::check(&tu).map_err(|e| ToolError::bad("tuning", e.to_string()))?;
    // The system in the host's words (the tuning area's names), the diapason as a number.
    let system = match &tu.system {
        TuningSystem::Equal => Said::new("tuning.system.equal_name"),
        TuningSystem::External => Said::new("tuning.system.external_name"),
        TuningSystem::Temperament { id, key, .. } => Said::new("ai.phrase.temperament_in")
            .nested("temperament", Said::new(id.name_key()))
            .arg("key", ROOT_NAMES[usize::from(key % 12)]),
        TuningSystem::Scale(s) => Said::new("ai.phrase.scale_of")
            .name("scale", args::clean(&s.description))
            .arg("n", s.degrees.len()),
    };
    let said = match track {
        None => Said::new("ai.tools.set_tuning"),
        Some(id) => Said::new("ai.tools.set_track_tuning").name("track", words::track_name(p, id)),
    }
    .nested("system", system)
    .nested("a4", words::num(tu.a4_hz));
    let result = tuning_json(&tu);
    let cmd = match track {
        None => Command::SetProjectTuning {
            tuning: Box::new(tu),
        },
        Some(id) => Command::SetTrackTuning {
            track: id,
            tuning: TrackTuning::Own { tuning: tu },
        },
    };
    Ok(plan(cmd, said, json!({"tuning": result})))
}
