//! The session view (ADR 0076): scenes, slots and how their clips launch and follow.

use super::args::{self, plan, s_id, Res, A, MAX_NOTES};
use super::notes::pitch_of;
use super::notes2::copy_clip;
use super::words;
use crate::tool::{EditPlan, Said, ToolCx, ToolError};
use cantus_core::session::{
    FollowAction, FollowActions, FollowAfter, LaunchMode, Quantize, Scene, SceneId, SessionClip,
    SessionCommand, SlotRef,
};
use cantus_core::{ClipId, Command, Ticks, TimeSignature};
use serde_json::{json, Value};

pub const QUANTIZE: [&str; 15] = [
    "global", "none", "8_bars", "4_bars", "2_bars", "bar", "1/2", "1/2t", "1/4", "1/4t", "1/8",
    "1/8t", "1/16", "1/16t", "1/32",
];
const MODES: [&str; 4] = ["trigger", "gate", "toggle", "repeat"];
const FOLLOW: [&str; 9] = [
    "none", "stop", "again", "previous", "next", "first", "last", "any", "other",
];

pub fn session_schema() -> Value {
    let follow = json!({"type": "object", "additionalProperties": false, "description": "Follow actions (Live's): after the time (or the clip's own length when `after_bars` is left out, `count` times), action a or b by chance.",
    "properties": {
        "on": {"type": "boolean", "default": true},
        "a": {"type": "string", "description": format!("One of {}, or \"jump:<scene_id>\".", FOLLOW.join(", "))},
        "b": {"type": "string"},
        "chance_a": {"type": "integer", "minimum": 0, "maximum": 100, "default": 100},
        "after_bars": {"type": "number", "exclusiveMinimum": 0, "maximum": 512},
        "count": {"type": "integer", "minimum": 1, "maximum": 999, "default": 1}
    }});
    args::object(
        json!({
            "add_scene": {"type": "object", "additionalProperties": false, "description": "Add a scene (a row of the grid).",
                "properties": {
                    "name": {"type": "string", "maxLength": 80},
                    "index": {"type": "integer", "minimum": 0},
                    "tempo_bpm": {"type": "number", "minimum": 20, "maximum": 999, "description": "Launching it sets this tempo."},
                    "time_signature": {"type": "string", "description": "Launching it sets this metre (\"3/4\")."}
                }},
            "scene": {"type": "object", "additionalProperties": false, "required": ["scene_id"], "description": "Change a scene.",
                "properties": {
                    "scene_id": {"type": "integer", "minimum": 1},
                    "name": {"type": "string", "maxLength": 80},
                    "tempo_bpm": {"type": ["number", "null"], "minimum": 20, "maximum": 999, "description": "null: launching leaves the tempo."},
                    "time_signature": {"type": ["string", "null"]},
                    "follow": follow.clone()
                }},
            "slot": {"type": "object", "additionalProperties": false, "required": ["track_id", "scene_id"], "description": "Fill a slot or set how its clip launches.",
                "properties": {
                    "track_id": {"type": "integer", "minimum": 1},
                    "scene_id": {"type": "integer", "minimum": 0, "description": "The scene; 0 for the scene added in this same call (add_scene)."},
                    "from_clip_id": s_id("Copy this arrangement clip into the (empty) slot."),
                    "length_beats": {"type": "number", "exclusiveMinimum": 0, "maximum": 4096, "description": "A new empty-or-filled note clip of this length in the (empty) slot."},
                    "notes": {"type": "array", "maxItems": MAX_NOTES, "description": "Notes of the new clip, positions in beats from the clip's start.",
                        "items": {"type": "object", "additionalProperties": false, "required": ["start_beats", "length_beats", "pitch"],
                            "properties": {"start_beats": {"type": "number", "minimum": 0}, "length_beats": {"type": "number", "exclusiveMinimum": 0},
                                "pitch": {"type": ["integer", "string"]}, "velocity": {"type": "integer", "minimum": 1, "maximum": 127}}}},
                    "loop": {"type": "boolean", "default": true, "description": "The new clip loops over its length."},
                    "launch": {"type": "object", "additionalProperties": false, "properties": {
                        "mode": {"enum": MODES}, "quantize": {"enum": QUANTIZE},
                        "legato": {"type": "boolean"}, "velocity": {"type": "number", "minimum": 0, "maximum": 1}}},
                    "follow": follow
                }}
        }),
        &[],
    )
}

fn metre(s: &str) -> Res<TimeSignature> {
    let bad = || {
        ToolError::bad(
            "time_signature",
            format!("{s:?} is not a metre such as 3/4"),
        )
    };
    let (n, d) = s.split_once('/').ok_or_else(bad)?;
    let sig = TimeSignature::new(
        n.trim().parse().map_err(|_| bad())?,
        d.trim().parse().map_err(|_| bad())?,
    );
    if cantus_core::session::metre_is_valid(sig) {
        Ok(sig)
    } else {
        Err(bad())
    }
}

fn follow_action(s: &str) -> Res<FollowAction> {
    if let Some(id) = s.strip_prefix("jump:") {
        return Ok(FollowAction::Jump(SceneId(
            id.parse()
                .map_err(|_| ToolError::bad("follow", "jump:<scene_id>"))?,
        )));
    }
    Ok(match s {
        "none" => FollowAction::None,
        "stop" => FollowAction::Stop,
        "again" => FollowAction::Again,
        "previous" => FollowAction::Previous,
        "next" => FollowAction::Next,
        "first" => FollowAction::First,
        "last" => FollowAction::Last,
        "any" => FollowAction::Any,
        "other" => FollowAction::Other,
        _ => {
            return Err(ToolError::bad(
                "follow",
                format!("{s:?} is not a follow action"),
            ))
        }
    })
}

fn follow_of(v: &Value, beats_per_bar: Ticks) -> Res<FollowActions> {
    let a = A(v);
    let mut f = FollowActions::off();
    f.on = a.opt_bool("on")?.unwrap_or(true);
    if let Some(x) = a.opt_str("a")? {
        f.a = follow_action(x)?;
    }
    if let Some(x) = a.opt_str("b")? {
        f.b = follow_action(x)?;
    }
    f.chance_a = a.opt_in("chance_a", 0.0, 100.0)?.unwrap_or(100.0) as u8;
    f.count = a.opt_in("count", 1.0, 999.0)?.unwrap_or(1.0) as u16;
    f.after = match a.opt_in("after_bars", 0.0, 512.0)? {
        Some(b) if b > 0.0 => FollowAfter::Time(Ticks((beats_per_bar.0 as f64 * b).round() as i64)),
        _ => FollowAfter::Linked,
    };
    if !f.is_valid() {
        return Err(ToolError::bad("follow", "not valid follow actions"));
    }
    Ok(f)
}

pub fn set_session(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let bar = cx.project().time_signature.ticks_per_bar();
    let mut cmds = Vec::new();
    let mut result = serde_json::Map::new();
    let mut what: Vec<Said> = Vec::new();
    let mut new_scene: Option<SceneId> = None;
    let mut new_scene_name = String::new();
    if let Some(v) = a.get("add_scene") {
        let sa = A(v);
        let id = SceneId(cx.bus_mut().alloc_id());
        let p = cx.project();
        let sname = sa.opt_name("name", 80)?.unwrap_or_default();
        new_scene_name = sname.clone();
        let mut scene = Scene::new(id, sname);
        scene.tempo = sa.opt_in("tempo_bpm", 20.0, 999.0)?;
        scene.metre = sa.opt_str("time_signature")?.map(metre).transpose()?;
        let len = p.session.scenes.len();
        let index = sa.opt_u64("index")?.map_or(len, |i| (i as usize).min(len));
        cmds.push(Command::Session(SessionCommand::InsertScene {
            index,
            scene: Box::new(scene),
            slots: Vec::new(),
        }));
        result.insert("scene_id".into(), json!(id.0));
        what.push(
            Said::new("ai.phrase.session.add_scene").name("scene", args::clean(&new_scene_name)),
        );
        new_scene = Some(id);
    }
    if let Some(v) = a.get("scene") {
        let sa = A(v);
        let id = SceneId(sa.u64("scene_id")?);
        let p = cx.project();
        let old_name = p
            .session
            .scene(id)
            .ok_or_else(|| ToolError::NotFound(format!("scene {}", id.0)))?
            .name
            .clone();
        if let Some(n) = sa.opt_name("name", 80)? {
            cmds.push(Command::Session(SessionCommand::SetSceneName {
                id,
                name: n,
            }));
        }
        if sa.0.get("tempo_bpm").is_some() {
            cmds.push(Command::Session(SessionCommand::SetSceneTempo {
                id,
                tempo: sa.opt_in("tempo_bpm", 20.0, 999.0)?,
            }));
        }
        if sa.0.get("time_signature").is_some() {
            cmds.push(Command::Session(SessionCommand::SetSceneMetre {
                id,
                metre: sa.opt_str("time_signature")?.map(metre).transpose()?,
            }));
        }
        if let Some(f) = sa.get("follow") {
            cmds.push(Command::Session(SessionCommand::SetSceneFollow {
                id,
                follow: follow_of(f, bar)?,
            }));
        }
        what.push(Said::new("ai.phrase.session.scene").name("scene", args::clean(&old_name)));
    }
    if let Some(v) = a.get("slot") {
        let sa = A(v);
        let p = cx.project();
        let track = args::track(p, sa, "track_id")?;
        let raw = sa.u64("scene_id")?;
        let scene = match (raw, new_scene) {
            (0, Some(s)) => s,
            (0, None) => {
                return Err(ToolError::bad(
                    "slot.scene_id",
                    "0 means the scene added in this call, and none was",
                ))
            }
            (n, _) => {
                let s = SceneId(n);
                p.session
                    .scene(s)
                    .ok_or_else(|| ToolError::NotFound(format!("scene {n}")))?;
                s
            }
        };
        let slot = SlotRef { track, scene };
        let t = p.track(track).expect("checked");
        let existing = t.slot_clip(scene).cloned();
        let tname = t.name.clone();
        let scene_name = p
            .session
            .scene(scene)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| new_scene_name.clone());
        let slot_words = |key: &'static str| {
            Said::new(key)
                .name("track", args::clean(&tname))
                .name("scene", args::clean(&scene_name))
        };
        let mut sc: Option<SessionClip> = None;
        if let Some(cid) = sa.opt_u64("from_clip_id")? {
            let (_, c) = args::find_clip(p, ClipId(cid))
                .ok_or_else(|| ToolError::NotFound(format!("clip {cid}")))?;
            let c = c.clone();
            let mut k = copy_clip(cx, &c, Ticks::ZERO);
            k.lane = 0;
            sc = Some(SessionClip::new(k));
        } else if let Some(lb) = sa.opt_in("length_beats", 0.0, 4096.0)? {
            let len = Ticks::from_beats_f64(lb);
            let mut clip = cx.bus_mut().new_clip("Clip", Ticks::ZERO, len);
            if sa.opt_bool("loop")?.unwrap_or(true) {
                clip.content_length = Some(len);
            }
            if let Some(items) = sa.opt_array("notes")? {
                if items.len() > MAX_NOTES {
                    return Err(ToolError::bad(
                        "slot.notes",
                        format!("at most {MAX_NOTES} notes"),
                    ));
                }
                for (i, n) in items.iter().enumerate() {
                    let na = A(n);
                    let start = Ticks::from_beats_f64(na.f64_in("start_beats", 0.0, lb)?);
                    let l = Ticks::from_beats_f64(na.f64_in("length_beats", 0.0, 4096.0)?)
                        .max(Ticks(1));
                    let pitch = pitch_of(na.req("pitch")?, &format!("slot.notes[{i}].pitch"))?;
                    let vel = na.opt_in("velocity", 1.0, 127.0)?.unwrap_or(100.0) as u8;
                    let note = cx.bus_mut().new_note(start, l, pitch, vel);
                    clip.notes.push(note);
                }
                clip.notes.sort_by_key(|n| (n.start, n.pitch, n.id));
            }
            sc = Some(SessionClip::new(clip));
        }
        if let Some(mut c) = sc {
            if existing.is_some() {
                return Err(ToolError::bad(
                    "slot",
                    "the slot holds a clip: remove it first with remove_items (session_slots)",
                ));
            }
            if let Some(l) = sa.get("launch") {
                c.launch = launch_of(l, c.launch)?;
            }
            if let Some(f) = sa.get("follow") {
                c.follow = follow_of(f, bar)?;
            }
            result.insert("clip_id".into(), json!(c.clip.id.0));
            cmds.push(Command::Session(SessionCommand::InsertSlotClip {
                slot,
                clip: Box::new(c),
            }));
            what.push(slot_words("ai.phrase.session.fill"));
        } else {
            let c = existing.ok_or_else(|| {
                ToolError::bad(
                    "slot",
                    "the slot is empty: give from_clip_id or length_beats",
                )
            })?;
            if let Some(l) = sa.get("launch") {
                cmds.push(Command::Session(SessionCommand::SetSlotLaunch {
                    slot,
                    launch: launch_of(l, c.launch)?,
                }));
            }
            if let Some(f) = sa.get("follow") {
                cmds.push(Command::Session(SessionCommand::SetSlotFollow {
                    slot,
                    follow: follow_of(f, bar)?,
                }));
            }
            what.push(slot_words("ai.phrase.session.launch"));
        }
        result.insert("track".into(), json!(tname));
    }
    let said = Said::new("ai.tools.set_session").nested("what", words::list(what));
    Ok(plan(
        args::one(cmds, "cmd.ai.set_session")?,
        said,
        Value::Object(result),
    ))
}

fn launch_of(
    v: &Value,
    mut l: cantus_core::session::LaunchSettings,
) -> Res<cantus_core::session::LaunchSettings> {
    let a = A(v);
    if let Some(m) = a.opt_choice("mode", &MODES)? {
        l.mode = LaunchMode::ALL[MODES.iter().position(|x| *x == m).expect("listed")];
    }
    if let Some(q) = a.opt_choice("quantize", &QUANTIZE)? {
        l.quantize = Quantize::ALL[QUANTIZE.iter().position(|x| *x == q).expect("listed")];
    }
    if let Some(x) = a.opt_bool("legato")? {
        l.legato = x;
    }
    if let Some(x) = a.opt_in("velocity", 0.0, 1.0)? {
        l.velocity = x as f32;
    }
    Ok(l)
}
