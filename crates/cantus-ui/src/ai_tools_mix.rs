//! The desk's studio tools for agents (round 36, ADR 0093): VCA faders and who follows
//! them, mixer snapshots and their recall, cues, the channels a recall leaves alone. Each
//! one is one ordinary command (or one batch): undoable in one step, the same the mixer's
//! own buttons run.

use cantus_ai::tools_project::args::{self, plan, Res, A};
use cantus_ai::{EditPlan, Said, ToolCx, ToolDef, ToolError, ToolRun};
use cantus_core::mix_cmd::MixCommand;
use cantus_core::mix_snap::{capture, MixCue, MixCueId, MixSnapId, MixSnapshot};
use cantus_core::vca::StripKind;
use cantus_core::{Command, Project, StripId, Ticks, PPQ};
use serde_json::{json, Value};

#[rustfmt::skip]
pub static TOOLS: &[ToolDef] = &[
    ToolDef { name: "create_vca", title: "Create a VCA fader", description: "Add a VCA to the mixer: a fader with no audio whose level (dB) scales the gain stage of the strips that follow it, and whose mute and solo are theirs. The members' own faders and automation stay as they are (the VCA adds to them in decibels); a VCA at 0 dB changes nothing. Optionally name the strips that follow it. A VCA can follow another VCA.", class: cantus_ai::ToolClass::Edit, input_schema: create_schema, output_schema: None, run: ToolRun::Edit(create_vca) },
    ToolDef { name: "assign_vca", title: "Assign strips to a VCA", description: "Make strips follow a VCA (or stop following it), or make every strip of a mixer group follow it (group_id; vca_id left out clears the group's). A strip may follow several VCAs: their decibels add, each counted once. Refused for a cycle or for something that is not a VCA.", class: cantus_ai::ToolClass::Edit, input_schema: assign_schema, output_schema: None, run: ToolRun::Edit(assign_vca) },
    ToolDef { name: "take_snapshot", title: "Take a mixer snapshot", description: "Store the desk's state under a name: each strip's fader, pan, width, polarity, mute, solo, swap, inserts switch, EQ, each existing send's level, switch and tap, and each insert's bypass, dry/wet and native parameters; the master's. Structure (routing, which inserts exist, a hosted plug-in's own state) is not part of it. strips narrows it to some channels.", class: cantus_ai::ToolClass::Edit, input_schema: take_schema, output_schema: None, run: ToolRun::Edit(take_snapshot) },
    ToolDef { name: "recall_snapshot", title: "Recall a mixer snapshot", description: "Put a snapshot's state back on the desk as one undo step: all channels, or only strips; the master when master=true; channels marked recall-safe sit out a recall of everything unless named in strips or include_safe=true. Never creates, removes or re-routes anything.", class: cantus_ai::ToolClass::Edit, input_schema: recall_schema, output_schema: None, run: ToolRun::Edit(recall_snapshot) },
    ToolDef { name: "get_mix_snapshots", title: "Read the mixer snapshots", description: "The mixer snapshots (id, name, how many channels, whether the master is in it), their cues on the timeline, the VCAs and who follows them, and which strips are recall-safe.", class: cantus_ai::ToolClass::Read, input_schema: empty, output_schema: None, run: ToolRun::Read(get_mix_snapshots) },
    ToolDef { name: "remove_snapshot", title: "Remove a mixer snapshot", description: "Delete a snapshot. Refused while a cue plays it (remove the cue first).", class: cantus_ai::ToolClass::Destructive, input_schema: snapshot_only, output_schema: None, run: ToolRun::Edit(remove_snapshot) },
    ToolDef { name: "add_mix_cue", title: "Cue a snapshot on the timeline", description: "Recall a snapshot at a bar, as a scene: from there its faders, pans, widths, send levels and native effect parameters hold (until another cue or the next automation clip of that control starts; a clip playing across the bar is silenced from the cue on), and its mutes, solos, polarity, swap, insert switches, bypasses and strip EQ are switched with a 16-sample fade. Only what differs from the desk is written. Position in beats from the song's start (4 per bar in 4/4).", class: cantus_ai::ToolClass::Edit, input_schema: cue_schema, output_schema: None, run: ToolRun::Edit(add_mix_cue) },
    ToolDef { name: "remove_mix_cue", title: "Remove a snapshot cue", description: "Remove a cue (get_mix_snapshots lists them).", class: cantus_ai::ToolClass::Destructive, input_schema: cue_only, output_schema: None, run: ToolRun::Edit(remove_mix_cue) },
    ToolDef { name: "set_strip_safe", title: "Mark a strip safe", description: "recall_safe: a recall of everything leaves the strip alone (a recall naming it still reaches it). solo_safe: a solo elsewhere does not silence the strip (a click, a reference), unless it is muted. Give either or both.", class: cantus_ai::ToolClass::Edit, input_schema: safe_schema, output_schema: None, run: ToolRun::Edit(set_strip_safe) },
];

fn empty() -> Value {
    args::object(json!({}), &[])
}

fn create_schema() -> Value {
    args::object(
        json!({
            "name": {"type": "string", "minLength": 1, "maxLength": 80, "description": "The VCA's name (\"Drums VCA\")."},
            "members": args::s_ids("Strips (ids from get_mixer) that follow it."),
            "gain_db": {"type": "number", "minimum": -96, "maximum": 12, "description": "Its fader, dB (default 0: no change)."}
        }),
        &["name"],
    )
}

fn assign_schema() -> Value {
    args::object(
        json!({
            "vca_id": args::s_id("The VCA (get_mix_snapshots lists them)."),
            "members": args::s_ids("Strips to make follow it."),
            "release": {"type": "boolean", "default": false, "description": "Make `members` stop following it instead."},
            "group_id": args::s_id("A mixer group: every strip in it follows the VCA (vca_id left out: the group follows none).")
        }),
        &[],
    )
}

fn take_schema() -> Value {
    args::object(
        json!({
            "name": {"type": "string", "minLength": 1, "maxLength": 80},
            "strips": args::s_ids("Only these strips (default every strip)."),
            "master": {"type": "boolean", "default": true, "description": "Include the master."}
        }),
        &["name"],
    )
}

fn recall_schema() -> Value {
    args::object(
        json!({
            "snapshot_id": args::s_id("The snapshot (get_mix_snapshots)."),
            "strips": args::s_ids("Only these strips (default all, but the recall-safe ones)."),
            "master": {"type": "boolean", "default": true},
            "include_safe": {"type": "boolean", "default": false}
        }),
        &["snapshot_id"],
    )
}

fn snapshot_only() -> Value {
    args::object(
        json!({"snapshot_id": args::s_id("The snapshot.")}),
        &["snapshot_id"],
    )
}

fn cue_schema() -> Value {
    args::object(
        json!({
            "snapshot_id": args::s_id("The snapshot to cue."),
            "at_beats": {"type": "number", "minimum": 0, "description": "Where, in beats from the start of the song."},
            "strips": args::s_ids("Only these strips (default all).")
        }),
        &["snapshot_id", "at_beats"],
    )
}

fn cue_only() -> Value {
    args::object(
        json!({"cue_id": args::s_id("The cue (get_mix_snapshots).")}),
        &["cue_id"],
    )
}

fn safe_schema() -> Value {
    args::object(
        json!({"strip_id": args::s_id("The strip."), "recall_safe": {"type": "boolean"}, "solo_safe": {"type": "boolean"}}),
        &["strip_id"],
    )
}

/// An array of strip ids that exist.
fn strips(p: &Project, a: A<'_>, k: &str) -> Res<Vec<StripId>> {
    let Some(v) = a.opt_array(k)? else {
        return Ok(Vec::new());
    };
    v.iter()
        .map(|x| {
            let id = StripId(x.as_u64().ok_or_else(|| ToolError::bad(k, "strip ids"))?);
            p.strip(id)
                .map(|_| id)
                .ok_or_else(|| ToolError::NotFound(format!("strip {}", id.0)))
        })
        .collect()
}

fn vca_of(p: &Project, id: u64) -> Res<StripId> {
    let id = StripId(id);
    match p.strip(id) {
        None => Err(ToolError::NotFound(format!("strip {}", id.0))),
        Some(s) if s.kind != StripKind::Vca => Err(ToolError::bad(
            "vca_id",
            format!("strip {} is not a VCA", id.0),
        )),
        Some(_) => Ok(id),
    }
}

fn snap_of(p: &Project, a: A<'_>, k: &str) -> Res<MixSnapId> {
    let id = MixSnapId(a.u64(k)?);
    p.mix
        .snapshot(id)
        .map(|_| id)
        .ok_or_else(|| ToolError::NotFound(format!("snapshot {}", id.0)))
}

fn with_vca(p: &Project, s: StripId, vca: StripId, on: bool) -> Command {
    let mut v = p.strip(s).map(|x| x.vcas.clone()).unwrap_or_default();
    v.retain(|x| *x != vca);
    if on {
        v.push(vca);
    }
    Command::Mix(MixCommand::SetStripVcas { id: s, vcas: v })
}

fn create_vca(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let name = a
        .opt_name("name", 80)?
        .ok_or_else(|| ToolError::bad("name", "is required"))?;
    let members = strips(cx.project(), a, "members")?;
    let gain = a.opt_in("gain_db", -96.0, 12.0)?.unwrap_or(0.0) as f32;
    let index = cx.project().strips.len();
    let mut s = cx.bus_mut().new_strip(name.clone());
    s.kind = StripKind::Vca;
    s.gain_db = gain;
    let id = s.id;
    let mut cmds = vec![Command::InsertStrip {
        index,
        strip: Box::new(s),
    }];
    let p = cx.project();
    for m in &members {
        // The new strip is not in the project yet: its list is built here.
        let mut v = p.strip(*m).map(|x| x.vcas.clone()).unwrap_or_default();
        v.push(id);
        cmds.push(Command::Mix(MixCommand::SetStripVcas { id: *m, vcas: v }));
    }
    Ok(plan(
        args::one(cmds, "cmd.mix.create_vca")?,
        Said::new("ai.tools.mix_create_vca").name("name", args::clean(&name)),
        json!({"vca_id": id.0, "members": members.iter().map(|m| m.0).collect::<Vec<_>>()}),
    ))
}

fn assign_vca(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let vca = a.opt_u64("vca_id")?.map(|v| vca_of(p, v)).transpose()?;
    if a.has("group_id") {
        let g = cantus_core::GroupId(a.u64("group_id")?);
        if p.strip_group(g).is_none() {
            return Err(ToolError::NotFound(format!("group {}", g.0)));
        }
        return Ok(plan(
            Command::Mix(MixCommand::SetGroupVca { id: g, vca }),
            Said::new("ai.tools.mix_assign_group"),
            json!({"group_id": g.0, "vca_id": vca.map(|v| v.0)}),
        ));
    }
    let vca = vca.ok_or_else(|| ToolError::bad("vca_id", "is required without group_id"))?;
    let members = strips(p, a, "members")?;
    let release = a.opt_bool("release")?.unwrap_or(false);
    let cmds: Vec<Command> = members
        .iter()
        .map(|m| with_vca(p, *m, vca, !release))
        .collect();
    Ok(plan(
        args::one(cmds, "cmd.mix.set_vcas")?,
        Said::new(if release {
            "ai.tools.mix_release_vca"
        } else {
            "ai.tools.mix_assign_vca"
        }),
        json!({"vca_id": vca.0, "members": members.iter().map(|m| m.0).collect::<Vec<_>>()}),
    ))
}

fn take_snapshot(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let name = a
        .opt_name("name", 80)?
        .ok_or_else(|| ToolError::bad("name", "is required"))?;
    let scope = strips(cx.project(), a, "strips")?;
    let master = a.opt_bool("master")?.unwrap_or(true);
    let state = capture(cx.project(), &scope, master);
    let channels = state.strips.len();
    let id = MixSnapId(cx.bus_mut().alloc_id());
    let index = cx.project().mix.snapshots.len();
    Ok(plan(
        Command::Mix(MixCommand::InsertSnapshot {
            index,
            snapshot: Box::new(MixSnapshot {
                id,
                name: name.clone(),
                state,
            }),
        }),
        Said::new("ai.tools.mix_take_snapshot").name("name", args::clean(&name)),
        json!({"snapshot_id": id.0, "channels": channels, "master": master}),
    ))
}

fn recall_snapshot(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let id = snap_of(p, a, "snapshot_id")?;
    let name = p
        .mix
        .snapshot(id)
        .map(|s| s.name.clone())
        .unwrap_or_default();
    let scope = strips(p, a, "strips")?;
    Ok(plan(
        Command::Mix(MixCommand::Recall {
            id,
            scope,
            master: a.opt_bool("master")?.unwrap_or(true),
            include_safe: a.opt_bool("include_safe")?.unwrap_or(false),
        }),
        Said::new("ai.tools.mix_recall").name("name", args::clean(&name)),
        json!({"snapshot_id": id.0}),
    ))
}

fn remove_snapshot(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let p = cx.project();
    let id = snap_of(p, A(a), "snapshot_id")?;
    let name = p
        .mix
        .snapshot(id)
        .map(|s| s.name.clone())
        .unwrap_or_default();
    Ok(plan(
        Command::Mix(MixCommand::RemoveSnapshot { id }),
        Said::new("ai.tools.mix_remove_snapshot").name("name", args::clean(&name)),
        json!({"removed": id.0}),
    ))
}

fn add_mix_cue(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let snapshot = snap_of(p, a, "snapshot_id")?;
    let beats = a
        .opt_in("at_beats", 0.0, 100_000.0)?
        .ok_or_else(|| ToolError::bad("at_beats", "is required"))?;
    let scope = strips(p, a, "strips")?;
    let index = p.mix.cues.len();
    let id = MixCueId(cx.bus_mut().alloc_id());
    Ok(plan(
        Command::Mix(MixCommand::InsertCue {
            index,
            cue: MixCue {
                id,
                at: Ticks((beats * PPQ as f64).round() as i64),
                snapshot,
                scope,
            },
        }),
        Said::new("ai.tools.mix_add_cue"),
        json!({"cue_id": id.0, "at_beats": beats}),
    ))
}

fn remove_mix_cue(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let id = MixCueId(A(a).u64("cue_id")?);
    if !cx.project().mix.cues.iter().any(|c| c.id == id) {
        return Err(ToolError::NotFound(format!("cue {}", id.0)));
    }
    Ok(plan(
        Command::Mix(MixCommand::RemoveCue { id }),
        Said::new("ai.tools.mix_remove_cue"),
        json!({"removed": id.0}),
    ))
}

fn set_strip_safe(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let id = args::strip(p, a, "strip_id")?;
    if id.is_master() {
        return Err(ToolError::bad("strip_id", "the master has no such setting"));
    }
    let mut cmds = Vec::new();
    if let Some(safe) = a.opt_bool("recall_safe")? {
        cmds.push(Command::Mix(MixCommand::SetRecallSafe { id, safe }));
    }
    if let Some(safe) = a.opt_bool("solo_safe")? {
        cmds.push(Command::Mix(MixCommand::SetSoloSafe { id, safe }));
    }
    Ok(plan(
        args::one(cmds, "cmd.mix.set_safe")?,
        Said::new("ai.tools.mix_set_safe"),
        json!({"strip_id": id.0}),
    ))
}

fn get_mix_snapshots(cx: &ToolCx<'_>, _: &Value) -> Result<Value, ToolError> {
    let p = cx.project();
    Ok(json!({
        "snapshots": p.mix.snapshots.iter().map(|s| json!({
            "id": s.id.0, "name": s.name,
            "channels": s.state.strips.len(), "master": s.state.master.is_some(),
        })).collect::<Vec<_>>(),
        "cues": p.mix.cues.iter().map(|c| json!({
            "id": c.id.0, "snapshot_id": c.snapshot.0,
            "at_beats": c.at.0 as f64 / PPQ as f64,
            "strips": c.scope.iter().map(|s| s.0).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "vcas": p.strips.iter().filter(|s| s.kind == StripKind::Vca).map(|s| json!({
            "id": s.id.0, "name": s.name, "gain_db": s.gain_db,
            "members": p.vca_members(s.id).iter().map(|m| m.0).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "group_vcas": p.mix.group_vcas.iter().map(|(g, v)| json!({"group_id": g.0, "vca_id": v.0})).collect::<Vec<_>>(),
        "recall_safe": p.strips.iter().filter(|s| s.recall_safe).map(|s| s.id.0).collect::<Vec<_>>(),
        "solo_safe": p.strips.iter().filter(|s| s.solo_safe).map(|s| s.id.0).collect::<Vec<_>>(),
    }))
}
