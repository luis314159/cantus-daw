//! Command bridge: turns model tool calls into ordinary [`Command`]s. An agent gets no
//! back door — it goes through the same bus, undo and audit as a mouse click (ADR 0004).

use crate::provider::{ToolCall, ToolSpec};
use cantus_core::{Command, CommandBus, EffectKind, FirmusParam, Project, Ticks, TrackId};
use serde_json::{json, Value};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("unknown tool {0:?}")]
    UnknownTool(String),
    #[error("missing or invalid argument {0:?}")]
    BadArgument(&'static str),
    #[error("track {0} not found")]
    TrackNotFound(u64),
}

/// The tools Cantus exposes to any agent (and, later, over MCP). Kept small on purpose:
/// every entry is a promise we have to keep forever.
pub fn tool_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "describe_project".into(),
            description: "Read-only summary of the open project: tempo, tracks, clips, note counts.".into(),
            input_schema: json!({ "type": "object", "properties": {} }),
        },
        ToolSpec {
            name: "set_tempo".into(),
            description: "Set the project tempo in beats per minute (20-999).".into(),
            input_schema: json!({
                "type": "object",
                "properties": { "bpm": { "type": "number" } },
                "required": ["bpm"]
            }),
        },
        ToolSpec {
            name: "set_tempo_point".into(),
            description: "Put a tempo change on the tempo track at a beat (a ramp when `ramp` is true: the tempo moves linearly from this point to the next). Beat 0 sets the song's starting tempo.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "beat": { "type": "number" },
                    "bpm": { "type": "number" },
                    "ramp": { "type": "boolean" }
                },
                "required": ["beat", "bpm"]
            }),
        },
        ToolSpec {
            name: "add_track".into(),
            description: "Create a new instrument track playing the built-in Firmus synth.".into(),
            input_schema: json!({
                "type": "object",
                "properties": { "name": { "type": "string" } },
                "required": ["name"]
            }),
        },
        ToolSpec {
            name: "add_notes".into(),
            description: "Add MIDI notes to a track. Positions/lengths are in beats (quarter notes) from the project start. A clip is created if the track has none.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "track_id": { "type": "integer" },
                    "notes": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "start_beats": { "type": "number" },
                                "length_beats": { "type": "number" },
                                "pitch": { "type": "integer", "minimum": 0, "maximum": 127 },
                                "velocity": { "type": "integer", "minimum": 1, "maximum": 127 }
                            },
                            "required": ["start_beats", "length_beats", "pitch"]
                        }
                    }
                },
                "required": ["track_id", "notes"]
            }),
        },
        ToolSpec {
            name: "set_track_gain".into(),
            description: "Set a track's fader in dB (-96 to 12).".into(),
            input_schema: json!({
                "type": "object",
                "properties": { "track_id": { "type": "integer" }, "gain_db": { "type": "number" } },
                "required": ["track_id", "gain_db"]
            }),
        },
        ToolSpec {
            name: "add_effect".into(),
            description: "Append an insert effect to a track. kind: \"delay\" or \"reverb\".".into(),
            input_schema: json!({
                "type": "object",
                "properties": { "track_id": { "type": "integer" }, "kind": { "type": "string", "enum": ["delay", "reverb"] } },
                "required": ["track_id", "kind"]
            }),
        },
        ToolSpec {
            name: "set_synth_param".into(),
            description: "Set a Firmus synth parameter on a track. Params: cutoff (Hz), resonance (0-1), amp_attack, amp_decay, amp_sustain, amp_release (s), osc_mix (0-1), gain (dB).".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "track_id": { "type": "integer" },
                    "param": { "type": "string" },
                    "value": { "type": "number" }
                },
                "required": ["track_id", "param", "value"]
            }),
        },
    ]
}

/// Human/model-readable project summary for the system prompt.
pub fn describe_project(project: &Project) -> String {
    let mut s = format!(
        "<project name=\"{}\" tempo_bpm=\"{}\" time_signature=\"{}\">\n",
        project.name, project.tempo_bpm, project.time_signature
    );
    // The tempo map (round 29, ADR 0060): every tempo point, metre change and marker.
    for p in &project.tempo_points {
        s.push_str(&format!(
            "  <tempo beat=\"{}\" bpm=\"{}\" ramp=\"{}\"/>
",
            p.tick.as_beats_f64(),
            p.bpm,
            p.ramp
        ));
    }
    for m in &project.metre_changes {
        s.push_str(&format!(
            "  <metre beat=\"{}\" signature=\"{}\"/>
",
            m.tick.as_beats_f64(),
            m.signature
        ));
    }
    for m in &project.markers {
        s.push_str(&format!(
            "  <marker beat=\"{}\" name=\"{}\"/>
",
            m.tick.as_beats_f64(),
            m.name
        ));
    }
    for t in &project.tracks {
        let notes: usize = t.clips.iter().map(|c| c.notes.len()).sum();
        s.push_str(&format!(
            "  <track id=\"{}\" name=\"{}\" gain_db=\"{:.1}\" pan=\"{:.2}\" muted=\"{}\" clips=\"{}\" notes=\"{}\"/>\n",
            t.id.0, t.name, t.gain_db, t.pan, t.muted, t.clips.len(), notes
        ));
    }
    s.push_str("</project>");
    s
}

/// Build the command for a tool call. `describe_project` returns `None` (read-only).
pub fn to_command(call: &ToolCall, bus: &mut CommandBus) -> Result<Option<Command>, BridgeError> {
    let a = &call.input;
    match call.name.as_str() {
        "describe_project" => Ok(None),
        "set_tempo" => {
            let bpm = a
                .get("bpm")
                .and_then(Value::as_f64)
                .ok_or(BridgeError::BadArgument("bpm"))?;
            Ok(Some(Command::SetTempo { bpm }))
        }
        "set_tempo_point" => {
            let beat = a
                .get("beat")
                .and_then(Value::as_f64)
                .filter(|b| b.is_finite() && *b >= 0.0)
                .ok_or(BridgeError::BadArgument("beat"))?;
            let bpm = a
                .get("bpm")
                .and_then(Value::as_f64)
                .filter(|b| b.is_finite())
                .ok_or(BridgeError::BadArgument("bpm"))?;
            let ramp = a.get("ramp").and_then(Value::as_bool).unwrap_or(false);
            Ok(Some(cantus_core::tempo::edit::add_tempo_point(
                bus.project(),
                Ticks::from_beats_f64(beat),
                bpm,
                ramp,
            )))
        }
        "add_track" => {
            let name = a
                .get("name")
                .and_then(Value::as_str)
                .ok_or(BridgeError::BadArgument("name"))?;
            let track = bus.new_track(name);
            let index = bus.project().tracks.len();
            Ok(Some(Command::InsertTrack {
                index,
                track: Box::new(track),
            }))
        }
        "add_notes" => {
            let track = track_id(a, bus.project())?;
            let items = a
                .get("notes")
                .and_then(Value::as_array)
                .ok_or(BridgeError::BadArgument("notes"))?;
            let mut parsed = Vec::with_capacity(items.len());
            for n in items {
                let start = n
                    .get("start_beats")
                    .and_then(Value::as_f64)
                    .ok_or(BridgeError::BadArgument("start_beats"))?;
                let len = n
                    .get("length_beats")
                    .and_then(Value::as_f64)
                    .ok_or(BridgeError::BadArgument("length_beats"))?;
                let pitch = n
                    .get("pitch")
                    .and_then(Value::as_u64)
                    .ok_or(BridgeError::BadArgument("pitch"))?;
                let vel = n.get("velocity").and_then(Value::as_u64).unwrap_or(100);
                if len <= 0.0 || start < 0.0 || pitch > 127 {
                    return Err(BridgeError::BadArgument("notes"));
                }
                parsed.push((
                    Ticks::from_beats_f64(start),
                    Ticks::from_beats_f64(len),
                    pitch as u8,
                    vel.clamp(1, 127) as u8,
                ));
            }
            let end = parsed
                .iter()
                .map(|(s, l, _, _)| *s + *l)
                .max()
                .unwrap_or(Ticks::ZERO);
            let existing = bus
                .project()
                .track(track)
                .and_then(|t| t.clips.first().cloned());
            let mut commands = Vec::new();
            let (clip_id, clip_start) = match existing {
                Some(c) => {
                    // Grow the clip if the new notes run past its end.
                    if c.start + c.length < end {
                        commands.push(Command::SetClipBounds {
                            track,
                            clip: c.id,
                            start: c.start,
                            length: (end - c.start).max(Ticks::beats(1)),
                        });
                    }
                    (c.id, c.start)
                }
                None => {
                    let clip = bus.new_clip("Clip", Ticks::ZERO, end.max(Ticks::beats(1)));
                    let id = clip.id;
                    commands.push(Command::InsertClip {
                        track,
                        clip: Box::new(clip),
                    });
                    (id, Ticks::ZERO)
                }
            };
            let notes = parsed
                .into_iter()
                .map(|(s, l, p, v)| bus.new_note(s - clip_start, l, p, v))
                .collect();
            commands.push(Command::InsertNotes {
                track,
                clip: clip_id,
                notes,
            });
            Ok(Some(Command::Batch {
                label_key: "cmd.ai_add_notes".into(),
                commands,
            }))
        }
        "set_track_gain" => {
            let id = track_id(a, bus.project())?;
            let gain_db = a
                .get("gain_db")
                .and_then(Value::as_f64)
                .ok_or(BridgeError::BadArgument("gain_db"))? as f32;
            Ok(Some(Command::SetTrackGain { id, gain_db }))
        }
        "add_effect" => {
            let track = track_id(a, bus.project())?;
            let kind = match a.get("kind").and_then(Value::as_str) {
                Some("delay") => EffectKind::Delay,
                Some("reverb") => EffectKind::Reverb,
                _ => return Err(BridgeError::BadArgument("kind")),
            };
            // Inserts live on the strip the track feeds. A track routed straight to the
            // master has nowhere to put one, which is reported rather than guessed.
            let strip = bus
                .project()
                .track(track)
                .and_then(|t| t.strip)
                .ok_or(BridgeError::BadArgument("track has no mixer strip"))?;
            let slot = bus.new_effect(kind);
            let index = bus
                .project()
                .strip(strip)
                .map(|s| s.effects.len())
                .unwrap_or(0);
            Ok(Some(Command::InsertEffect {
                strip,
                index,
                slot: Box::new(slot),
            }))
        }
        "set_synth_param" => {
            let track = track_id(a, bus.project())?;
            let name = a
                .get("param")
                .and_then(Value::as_str)
                .ok_or(BridgeError::BadArgument("param"))?;
            let value = a
                .get("value")
                .and_then(Value::as_f64)
                .ok_or(BridgeError::BadArgument("value"))? as f32;
            let param = match name {
                "cutoff" => FirmusParam::Cutoff,
                "resonance" => FirmusParam::Resonance,
                "amp_attack" => FirmusParam::AmpAttack,
                "amp_decay" => FirmusParam::AmpDecay,
                "amp_sustain" => FirmusParam::AmpSustain,
                "amp_release" => FirmusParam::AmpRelease,
                "osc_mix" => FirmusParam::OscMix,
                "gain" => FirmusParam::Gain,
                _ => return Err(BridgeError::BadArgument("param")),
            };
            Ok(Some(Command::SetInstrumentParam {
                track,
                param: param.into(),
                value,
            }))
        }
        other => Err(BridgeError::UnknownTool(other.to_string())),
    }
}

fn track_id(a: &Value, project: &Project) -> Result<TrackId, BridgeError> {
    let raw = a
        .get("track_id")
        .and_then(Value::as_u64)
        .ok_or(BridgeError::BadArgument("track_id"))?;
    let id = TrackId(raw);
    if project.track(id).is_none() {
        return Err(BridgeError::TrackNotFound(raw));
    }
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::Origin;

    #[test]
    fn agent_actions_are_ordinary_undoable_commands() {
        let mut bus = CommandBus::default();
        let call = ToolCall {
            name: "add_track".into(),
            input: json!({ "name": "Pad" }),
        };
        let cmd = to_command(&call, &mut bus).unwrap().unwrap();
        bus.execute(cmd, Origin::Ai).unwrap();
        let tid = bus.project().tracks[0].id.0;

        let call = ToolCall {
            name: "add_notes".into(),
            input: json!({ "track_id": tid, "notes": [
                { "start_beats": 0, "length_beats": 1, "pitch": 60 },
                { "start_beats": 1, "length_beats": 0.5, "pitch": 64, "velocity": 80 },
                { "start_beats": 4, "length_beats": 2, "pitch": 67 }
            ]}),
        };
        let cmd = to_command(&call, &mut bus).unwrap().unwrap();
        bus.execute(cmd, Origin::Ai).unwrap();
        let t = &bus.project().tracks[0];
        assert_eq!(t.clips.len(), 1);
        assert_eq!(t.clips[0].notes.len(), 3);
        assert_eq!(t.clips[0].length, Ticks::beats(6));

        // Adding more notes past the end grows the same clip.
        let call = ToolCall {
            name: "add_notes".into(),
            input: json!({ "track_id": tid, "notes": [{ "start_beats": 8, "length_beats": 1, "pitch": 72 }] }),
        };
        let cmd = to_command(&call, &mut bus).unwrap().unwrap();
        bus.execute(cmd, Origin::Ai).unwrap();
        assert_eq!(bus.project().tracks[0].clips[0].length, Ticks::beats(9));
        assert_eq!(bus.project().tracks[0].clips[0].notes.len(), 4);

        // Everything the agent did unwinds.
        bus.undo();
        bus.undo();
        bus.undo();
        assert!(bus.project().tracks.is_empty());
        assert!(bus.audit_log().iter().all(|e| e.origin == Origin::Ai));
    }

    #[test]
    fn rejects_bad_calls() {
        let mut bus = CommandBus::default();
        let bad = ToolCall {
            name: "explode".into(),
            input: json!({}),
        };
        assert!(matches!(
            to_command(&bad, &mut bus),
            Err(BridgeError::UnknownTool(_))
        ));
        let bad = ToolCall {
            name: "set_track_gain".into(),
            input: json!({ "track_id": 99, "gain_db": 0 }),
        };
        assert!(matches!(
            to_command(&bad, &mut bus),
            Err(BridgeError::TrackNotFound(99))
        ));
        let ro = ToolCall {
            name: "describe_project".into(),
            input: json!({}),
        };
        assert!(to_command(&ro, &mut bus).unwrap().is_none());
    }

    #[test]
    fn describe_is_xml_like_and_mentions_tracks() {
        let mut bus = CommandBus::default();
        let t = bus.new_track("Keys");
        bus.execute(
            Command::InsertTrack {
                index: 0,
                track: Box::new(t),
            },
            Origin::User,
        )
        .unwrap();
        let d = describe_project(bus.project());
        assert!(d.starts_with("<project"));
        assert!(d.contains("name=\"Keys\""));
    }

    /// Round 29 (ADR 0060): the agent sees the tempo map and can put a tempo point, an
    /// ordinary undoable command.
    #[test]
    fn the_agent_sees_the_tempo_map_and_sets_a_tempo_point() {
        let mut bus = CommandBus::default();
        let call = ToolCall {
            name: "set_tempo_point".into(),
            input: json!({ "beat": 8.0, "bpm": 140.0, "ramp": false }),
        };
        let cmd = to_command(&call, &mut bus).unwrap().unwrap();
        bus.execute(cmd, cantus_core::Origin::Ai).unwrap();
        assert_eq!(bus.project().tempo_points.len(), 2);
        assert_eq!(bus.project().tempo_points[1].bpm, 140.0);
        let text = describe_project(bus.project());
        assert!(text.contains("<tempo beat=\"8\" bpm=\"140\""), "{text}");
        assert!(bus.undo().is_some());
        assert!(bus.project().tempo_points.is_empty());
    }

    /// Review 2 (findings 8, 63): a ramp asked for from beat 0 ramps — an accelerando from
    /// the top to a point already there.
    #[test]
    fn the_agent_can_ramp_from_the_start() {
        let mut bus = CommandBus::default();
        for input in [
            json!({ "beat": 16.0, "bpm": 140.0, "ramp": false }),
            json!({ "beat": 0.0, "bpm": 90.0, "ramp": true }),
        ] {
            let call = ToolCall {
                name: "set_tempo_point".into(),
                input,
            };
            let cmd = to_command(&call, &mut bus).unwrap().unwrap();
            bus.execute(cmd, cantus_core::Origin::Ai).unwrap();
        }
        let p = bus.project();
        assert_eq!(p.tempo_bpm, 90.0);
        assert!(p.tempo_points[0].ramp, "{:?}", p.tempo_points);
        let map = p.tempo_map();
        assert!((map.bpm_at(cantus_core::Ticks::beats(8).0 as f64) - 115.0).abs() < 1e-9);
    }

    /// Review 3 (finding 55): the ramp asked for first, the point it ramps to after.
    #[test]
    fn the_agent_can_ramp_from_the_start_asked_first() {
        let mut bus = CommandBus::default();
        for input in [
            json!({ "beat": 0.0, "bpm": 90.0, "ramp": true }),
            json!({ "beat": 16.0, "bpm": 140.0, "ramp": false }),
        ] {
            let call = ToolCall {
                name: "set_tempo_point".into(),
                input,
            };
            let cmd = to_command(&call, &mut bus).unwrap().unwrap();
            bus.execute(cmd, cantus_core::Origin::Ai).unwrap();
        }
        let p = bus.project();
        assert_eq!(p.tempo_bpm, 90.0);
        assert!(p.tempo_points[0].ramp, "{:?}", p.tempo_points);
        let map = p.tempo_map();
        assert!((map.bpm_at(cantus_core::Ticks::beats(8).0 as f64) - 115.0).abs() < 1e-9);
    }
}
