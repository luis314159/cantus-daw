//! Command bridge: turns model tool calls into ordinary [`Command`]s. An agent gets no
//! back door — it goes through the same bus, undo and audit as a mouse click (ADR 0004).
//!
//! Since round 34 (ADR 0082) the tools themselves are the one table in
//! [`crate::tools_project::TOOLS`]: this bridge is the assistant panel's door to it —
//! [`tool_specs`] lists the table, [`to_command`] plans a call against a bare bus.

use crate::provider::{ToolCall, ToolSpec};
use crate::tool::{ToolClass, ToolCx, ToolError, ToolHost, ToolRun};
use cantus_core::{Command, CommandBus, Project};
use std::any::Any;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("unknown tool {0:?}")]
    UnknownTool(String),
    #[error("missing or invalid argument {0:?}")]
    BadArgument(String),
    #[error("{0} not found")]
    NotFound(String),
    #[error("{0}")]
    Tool(String),
}

impl From<ToolError> for BridgeError {
    fn from(e: ToolError) -> Self {
        match e {
            ToolError::BadArgument { name, reason } => {
                BridgeError::BadArgument(format!("{name}: {reason}"))
            }
            ToolError::NotFound(what) => BridgeError::NotFound(what),
            other => BridgeError::Tool(other.to_string()),
        }
    }
}

/// A host over a bare bus: what the panel, the bridge and the tests plan against.
#[derive(Default)]
pub struct BareHost {
    pub bus: CommandBus,
    /// The project's folder, when the project has one.
    pub dir: Option<PathBuf>,
}

impl ToolHost for BareHost {
    fn bus(&self) -> &CommandBus {
        &self.bus
    }
    fn bus_mut(&mut self) -> &mut CommandBus {
        &mut self.bus
    }
    fn project_dir(&self) -> Option<PathBuf> {
        self.dir.clone()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// The tools Cantus exposes to any agent: the project's table, the same the MCP server
/// lists (every entry is a promise we keep forever).
pub fn tool_specs() -> Vec<ToolSpec> {
    crate::tools_project::TOOLS
        .iter()
        .map(|t| ToolSpec {
            name: t.name.into(),
            description: t.description.into(),
            input_schema: (t.input_schema)(),
        })
        .collect()
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

/// A planned call: the command, the class the user's permission is asked by, and what it
/// says — a caller must not execute it without deciding by `class` (review #18).
#[derive(Debug)]
pub struct Planned {
    pub command: Command,
    pub class: ToolClass,
    pub said: crate::tool::Said,
}

/// Plan a tool call: `None` for a reading tool (`describe_project`, the old name of
/// `get_project`, included). The command is never executed here, and comes with its class.
pub fn to_command(call: &ToolCall, bus: &mut CommandBus) -> Result<Option<Planned>, BridgeError> {
    if call.name == "describe_project" {
        return Ok(None);
    }
    let tool = crate::tools_project::TOOLS
        .iter()
        .find(|t| t.name == call.name)
        .ok_or_else(|| BridgeError::UnknownTool(call.name.clone()))?;
    let ToolRun::Edit(plan) = tool.run else {
        return match tool.class {
            ToolClass::Read => Ok(None),
            _ => Err(BridgeError::Tool(format!("{} is not an edit", tool.name))),
        };
    };
    // The tool plans against the host's bus: lend it ours for the call.
    let mut host = BareHost {
        bus: std::mem::take(bus),
        dir: None,
    };
    let planned = plan(&mut ToolCx { host: &mut host }, &call.input);
    *bus = host.bus;
    let planned = planned?;
    Ok(Some(Planned {
        command: planned.command,
        class: tool.class,
        said: planned.said,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::{Origin, Ticks};
    use serde_json::json;

    #[test]
    fn agent_actions_are_ordinary_undoable_commands() {
        let mut bus = CommandBus::default();
        let call = ToolCall {
            name: "add_track".into(),
            input: json!({ "name": "Pad" }),
        };
        let cmd = to_command(&call, &mut bus).unwrap().unwrap().command;
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
        let cmd = to_command(&call, &mut bus).unwrap().unwrap().command;
        bus.execute(cmd, Origin::Ai).unwrap();
        let t = &bus.project().tracks[0];
        assert_eq!(t.clips.len(), 1);
        assert_eq!(t.clips[0].notes.len(), 3);
        // A new clip is whole bars around the notes (round 34).
        assert_eq!(t.clips[0].length, Ticks::beats(8));

        // Notes under the clip go into it; past it, into a new clip of their own bars.
        let call = ToolCall {
            name: "add_notes".into(),
            input: json!({ "track_id": tid, "notes": [{ "start_beats": 8, "length_beats": 1, "pitch": 72 }] }),
        };
        let cmd = to_command(&call, &mut bus).unwrap().unwrap().command;
        bus.execute(cmd, Origin::Ai).unwrap();
        assert_eq!(bus.project().tracks[0].clips.len(), 2);
        assert_eq!(bus.project().tracks[0].clips[1].start, Ticks::beats(8));
        assert_eq!(bus.project().tracks[0].clips[1].notes.len(), 1);

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
            name: "set_track".into(),
            input: json!({ "track_id": 99, "gain_db": 0 }),
        };
        assert!(matches!(
            to_command(&bad, &mut bus),
            Err(BridgeError::NotFound(_))
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
        let cmd = to_command(&call, &mut bus).unwrap().unwrap().command;
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
            let cmd = to_command(&call, &mut bus).unwrap().unwrap().command;
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
            let cmd = to_command(&call, &mut bus).unwrap().unwrap().command;
            bus.execute(cmd, cantus_core::Origin::Ai).unwrap();
        }
        let p = bus.project();
        assert_eq!(p.tempo_bpm, 90.0);
        assert!(p.tempo_points[0].ramp, "{:?}", p.tempo_points);
        let map = p.tempo_map();
        assert!((map.bpm_at(cantus_core::Ticks::beats(8).0 as f64) - 115.0).abs() < 1e-9);
    }

    /// Review 5 (finding 28): a point past the song's horizon is refused; a call that says
    /// nothing of the ramp keeps the point's own (a new start tempo keeps the accelerando).
    #[test]
    fn a_far_point_is_refused_and_an_unsaid_ramp_is_kept() {
        let mut bus = CommandBus::default();
        let call = |bus: &mut CommandBus, input: serde_json::Value| {
            let call = ToolCall {
                name: "set_tempo_point".into(),
                input,
            };
            to_command(&call, bus).map(|c| {
                bus.execute(c.unwrap().command, cantus_core::Origin::Ai)
                    .unwrap();
            })
        };
        assert!(call(&mut bus, json!({ "beat": 1e9, "bpm": 140.0 })).is_err());
        call(&mut bus, json!({ "beat": 0.0, "bpm": 90.0, "ramp": true })).unwrap();
        call(&mut bus, json!({ "beat": 16.0, "bpm": 140.0 })).unwrap();
        call(&mut bus, json!({ "beat": 0.0, "bpm": 100.0 })).unwrap();
        let p = bus.project();
        assert_eq!(p.tempo_points[0].bpm, 100.0);
        assert!(
            p.tempo_points[0].ramp,
            "the accelerando kept: {:?}",
            p.tempo_points
        );
    }

    /// Review #18: a planned call carries its class, so a panel cannot execute a removal
    /// without the user's say.
    #[test]
    fn a_planned_call_carries_its_class() {
        let mut bus = CommandBus::default();
        let t = bus.new_track("Pad");
        let id = t.id.0;
        bus.execute(
            Command::InsertTrack {
                index: 0,
                track: Box::new(t),
            },
            Origin::User,
        )
        .unwrap();
        let call = ToolCall {
            name: "remove_items".into(),
            input: json!({"track_ids": [id]}),
        };
        let planned = to_command(&call, &mut bus).unwrap().unwrap();
        assert_eq!(planned.class, ToolClass::Destructive);
        assert_eq!(planned.said.key, "ai.tools.remove_items");
        let call = ToolCall {
            name: "set_track".into(),
            input: json!({"track_id": id, "gain_db": -3}),
        };
        assert_eq!(
            to_command(&call, &mut bus).unwrap().unwrap().class,
            ToolClass::Edit
        );
    }
}
