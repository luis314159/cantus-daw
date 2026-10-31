//! Removing the user's material — tracks, clips, notes, effects, strips, sends, modulators,
//! markers, tempo points, scenes and session clips — in one tool of the destructive class,
//! so the user is always asked and one undo brings everything back.

use super::args::{self, plan, s_ids, Res, A};
use super::words;
use crate::tool::{EditPlan, Said, ToolCx, ToolError};
use cantus_core::session::{SceneId, SessionCommand, SlotRef};
use cantus_core::{ClipId, Command, ModulatorId, NoteId, StripId, TrackId};
use serde_json::{json, Value};

pub fn remove_schema() -> Value {
    args::object(
        json!({
            "track_ids": s_ids("Tracks (with their clips and automation)."),
            "clip_ids": s_ids("Arrangement clips (notes, audio or automation)."),
            "notes": {"type": "array", "description": "Notes of a clip.", "items": {"type": "object", "additionalProperties": false, "required": ["clip_id", "note_ids"],
                "properties": {"clip_id": {"type": "integer", "minimum": 1}, "note_ids": s_ids("The notes.")}}},
            "effect_ids": s_ids("Insert effects, on any strip or the master."),
            "strip_ids": s_ids("Mixer strips nothing feeds any more (tracks removed in the same call do not count)."),
            "sends": {"type": "array", "items": {"type": "object", "additionalProperties": false, "required": ["strip_id", "to"],
                "properties": {"strip_id": {"type": "integer", "minimum": 1}, "to": {"type": "integer", "minimum": 1}}}},
            "modulator_ids": s_ids("Modulators."),
            "modulator_routes": {"type": "array", "description": "Routes of a modulator, by what they drive.", "items": {"type": "object", "additionalProperties": false, "required": ["modulator_id", "target"],
                "properties": {"modulator_id": {"type": "integer", "minimum": 1}, "target": super::modulation::target_schema()}}},
            "marker_ids": s_ids("Markers."),
            "tempo_points_at": {"type": "array", "items": args::s_pos("A tempo point's position (not the first)"), "description": "Tempo points."},
            "scene_ids": s_ids("Session scenes (with their row of clips)."),
            "session_slots": {"type": "array", "items": {"type": "object", "additionalProperties": false, "required": ["track_id", "scene_id"],
                "properties": {"track_id": {"type": "integer", "minimum": 1}, "scene_id": {"type": "integer", "minimum": 1}}}, "description": "Session clips, by their slot."}
        }),
        &[],
    )
}

fn ids(a: A<'_>, k: &str) -> Res<Vec<u64>> {
    match a.opt_array(k)? {
        None => Ok(Vec::new()),
        Some(v) => v
            .iter()
            .map(|x| {
                x.as_u64()
                    .ok_or_else(|| ToolError::bad(k, "ids are numbers"))
            })
            .collect(),
    }
}

pub fn remove_items(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let map = p.tempo_map();
    let mut cmds = Vec::new();
    let mut counts = serde_json::Map::new();
    let mut count = |k: &str, n: usize| {
        if n > 0 {
            counts.insert(k.to_string(), json!(n));
        }
    };
    let tracks: Vec<TrackId> = ids(a, "track_ids")?
        .into_iter()
        .map(|i| {
            p.track(TrackId(i))
                .map(|_| TrackId(i))
                .ok_or_else(|| ToolError::NotFound(format!("track {i}")))
        })
        .collect::<Res<_>>()?;
    let gone = |t: TrackId| tracks.contains(&t);
    let mut said_items: Vec<Said> = tracks
        .iter()
        .map(|t| {
            Said::new("ai.phrase.rm.track").name("track", args::clean(&words::track_name(p, *t)))
        })
        .collect();
    // Notes first, then what holds them.
    let mut notes = 0;
    if let Some(list) = a.opt_array("notes")? {
        for (i, n) in list.iter().enumerate() {
            let na = A(n);
            let (track, c) = args::clip(p, na, "clip_id")?;
            let want = ids(na, "note_ids")?;
            if let Some(missing) = want.iter().find(|id| c.note(NoteId(**id)).is_none()) {
                return Err(ToolError::NotFound(format!(
                    "note {missing} in clip {} (notes[{i}])",
                    c.id.0
                )));
            }
            notes += want.len();
            said_items.push(
                Said::new("ai.phrase.rm.notes")
                    .nested("n", words::count(want.len(), "notes"))
                    .nested("clip", words::clip(p, &map, track, c)),
            );
            if !gone(track) {
                cmds.push(Command::RemoveNotes {
                    track,
                    clip: c.id,
                    ids: want.into_iter().map(NoteId).collect(),
                });
            }
        }
    }
    count("notes", notes);
    let mut sends = 0;
    if let Some(list) = a.opt_array("sends")? {
        for s in list {
            let sa = A(s);
            let from = args::strip(p, sa, "strip_id")?;
            let to = StripId(sa.u64("to")?);
            if p.strip(from).and_then(|x| x.send_to(to)).is_none() {
                return Err(ToolError::NotFound(format!(
                    "a send from strip {} to strip {}",
                    from.0, to.0
                )));
            }
            sends += 1;
            said_items.push(words::send(p, from, to));
            cmds.push(Command::RemoveStripSend { id: from, to });
        }
    }
    count("sends", sends);
    let effects = ids(a, "effect_ids")?;
    for e in &effects {
        let host = p
            .effect_host(cantus_core::EffectId(*e))
            .ok_or_else(|| ToolError::NotFound(format!("effect {e}")))?;
        said_items.push(words::effect(p, host, cantus_core::EffectId(*e)));
        cmds.push(Command::RemoveEffect {
            strip: host,
            effect: cantus_core::EffectId(*e),
        });
    }
    count("effects", effects.len());
    let mut routes_gone = 0;
    // Each modulator edited once, with every route this call removes (fix review 3 #3).
    let mut edited: Vec<cantus_core::Modulator> = Vec::new();
    if let Some(list) = a.opt_array("modulator_routes")? {
        for (i, r) in list.iter().enumerate() {
            let ra = A(r);
            let mid = cantus_core::ModulatorId(ra.u64("modulator_id")?);
            let (h, m) = p
                .all_modulators()
                .find(|(_, x)| x.id == mid)
                .ok_or_else(|| ToolError::NotFound(format!("modulator {}", mid.0)))?;
            let target = super::modulation::target_of(
                p,
                ra.req("target")?,
                &format!("modulator_routes[{i}].target"),
            )?;
            if !m.routes.iter().any(|x| x.target == target) {
                return Err(ToolError::NotFound(format!(
                    "a route of modulator {} to that target",
                    mid.0
                )));
            }
            let at = match edited.iter().position(|x| x.id == mid) {
                Some(at) => at,
                None => {
                    edited.push(m.clone());
                    edited.len() - 1
                }
            };
            let kept = &mut edited[at];
            if !kept.routes.iter().any(|x| x.target == target) {
                return Err(ToolError::bad(
                    format!("modulator_routes[{i}]"),
                    "that route is named twice",
                ));
            }
            kept.routes.retain(|x| x.target != target);
            routes_gone += 1;
            said_items.push(
                Said::new("ai.phrase.rm.route")
                    .nested("target", words::target(p, &target))
                    .nested("modulator", words::modulator(p, mid))
                    .nested("host", words::mod_host_of(p, h)),
            );
        }
    }
    for m in edited {
        cmds.push(Command::SetModulator {
            modulator: Box::new(m),
        });
    }
    count("modulator_routes", routes_gone);
    let mods = ids(a, "modulator_ids")?;
    for m in &mods {
        let (h, x) = p
            .all_modulators()
            .find(|(_, x)| x.id.0 == *m)
            .ok_or_else(|| ToolError::NotFound(format!("modulator {m}")))?;
        let _ = x;
        said_items.push(
            Said::new("ai.phrase.rm.modulator")
                .nested("modulator", words::modulator(p, ModulatorId(*m)))
                .nested("host", words::mod_host_of(p, h)),
        );
        cmds.push(Command::RemoveModulator {
            id: ModulatorId(*m),
        });
    }
    count("modulators", mods.len());
    let mut slots = 0;
    if let Some(list) = a.opt_array("session_slots")? {
        for s in list {
            let sa = A(s);
            let track = args::track(p, sa, "track_id")?;
            let scene = SceneId(sa.u64("scene_id")?);
            p.track(track)
                .and_then(|t| t.slot_clip(scene))
                .ok_or_else(|| {
                    ToolError::NotFound(format!(
                        "a clip in track {}'s slot of scene {}",
                        track.0, scene.0
                    ))
                })?;
            slots += 1;
            let sname = p
                .session
                .scene(scene)
                .map(|s| s.name.clone())
                .unwrap_or_default();
            said_items.push(
                Said::new("ai.phrase.rm.session_clip")
                    .name("track", args::clean(&words::track_name(p, track)))
                    .name("scene", args::clean(&sname)),
            );
            cmds.push(Command::Session(SessionCommand::RemoveSlotClip {
                slot: SlotRef { track, scene },
            }));
        }
    }
    count("session_clips", slots);
    let clips = ids(a, "clip_ids")?;
    for c in &clips {
        let (track, clip) = args::find_clip(p, ClipId(*c))
            .ok_or_else(|| ToolError::NotFound(format!("clip {c}")))?;
        said_items
            .push(Said::new("ai.phrase.rm.clip").nested("clip", words::clip(p, &map, track, clip)));
        if !gone(track) {
            cmds.push(Command::RemoveClip {
                track,
                clip: ClipId(*c),
            });
        }
    }
    count("clips", clips.len());
    let scenes = ids(a, "scene_ids")?;
    for s in &scenes {
        let sc = p
            .session
            .scene(SceneId(*s))
            .ok_or_else(|| ToolError::NotFound(format!("scene {s}")))?;
        said_items.push(Said::new("ai.phrase.rm.scene").name("scene", args::clean(&sc.name)));
        cmds.push(Command::Session(SessionCommand::RemoveScene {
            id: SceneId(*s),
        }));
    }
    count("scenes", scenes.len());
    let markers = ids(a, "marker_ids")?;
    for m in &markers {
        let mk = p
            .markers
            .iter()
            .find(|x| x.id.0 == *m)
            .ok_or_else(|| ToolError::NotFound(format!("marker {m}")))?;
        said_items.push(Said::new("ai.phrase.rm.marker").name("marker", args::clean(&mk.name)));
        cmds.push(Command::RemoveMarker {
            id: cantus_core::tempo::MarkerId(*m),
        });
    }
    count("markers", markers.len());
    let mut points: Vec<usize> = Vec::new();
    if let Some(list) = a.opt_array("tempo_points_at")? {
        for v in list {
            let at = args::pos_of(&map, v, "tempo_points_at")?;
            let i = p
                .tempo_points
                .iter()
                .position(|t| t.tick == at)
                .filter(|i| *i > 0)
                .ok_or_else(|| {
                    ToolError::NotFound(format!(
                        "a tempo point (not the first) at {}",
                        args::bar_beat_text(&map, at)
                    ))
                })?;
            points.push(i);
            said_items
                .push(Said::new("ai.phrase.rm.tempo_point").nested("at", words::pos(&map, at)));
        }
    }
    // Last first, so each index still names its point.
    points.sort_unstable();
    points.dedup();
    count("tempo_points", points.len());
    for i in points.into_iter().rev() {
        cmds.push(Command::RemoveTempoPoint { index: i });
    }
    for t in &tracks {
        cmds.push(Command::RemoveTrack { id: *t });
    }
    count("tracks", tracks.len());
    let strips = ids(a, "strip_ids")?;
    for s in &strips {
        let id = StripId(*s);
        p.strip(id)
            .ok_or_else(|| ToolError::NotFound(format!("strip {s}")))?;
        let fed: Vec<u64> = p
            .tracks_on_strip(id)
            .filter(|t| !gone(t.id))
            .map(|t| t.id.0)
            .collect();
        let by_strips: Vec<u64> = p
            .strips
            .iter()
            .filter(|x| {
                !strips.contains(&x.id.0)
                    && (x.feeds().any(|to| to == id) || x.keys().any(|k| k == id))
            })
            .map(|x| x.id.0)
            .collect();
        if !fed.is_empty() || !by_strips.is_empty() {
            return Err(ToolError::bad(
                "strip_ids",
                format!("strip {s} is still fed by tracks {fed:?} and strips {by_strips:?}: route them elsewhere first, or remove them in this call"),
            ));
        }
        said_items.push(Said::new("ai.phrase.rm.strip").nested("strip", words::strip(p, id)));
        cmds.push(Command::RemoveStrip { id });
    }
    count("strips", strips.len());
    let said = Said::new("ai.tools.remove_items").nested("what", words::list(said_items));
    Ok(plan(
        args::one(cmds, "cmd.ai.remove_items")
            .map_err(|_| ToolError::bad("track_ids", "say what to remove"))?,
        said,
        json!({"removed": counts}),
    ))
}
