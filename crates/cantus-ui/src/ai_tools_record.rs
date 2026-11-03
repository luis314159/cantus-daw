//! The recording mode for agents (round 36, ADR 0092): the same preparation the dialog
//! runs — the plan, its size, the freezes — and the same leave. `prepare_to_record` asks
//! like the dialog does (with `dry_run` it only answers what it would do and how big it
//! is); it runs in the background and `get_recording_mode` says when the mode is on.

use crate::app::CantusApp;
use crate::record_mode::{self as rm, Options, Phase};
use cantus_ai::tools_project::args::{self, Res, A};
use cantus_ai::{ToolClass, ToolCx, ToolDef, ToolError, ToolRun};
use cantus_core::freeze::FreezeTarget;
use cantus_core::record_mode::{self as plan_of, Why};
use serde_json::{json, Value};

#[rustfmt::skip]
pub static TOOLS: &[ToolDef] = &[
    ToolDef { name: "prepare_to_record", title: "Prepare to record", description: "Enter the recording mode: render every strip that feeds the master (after its effects and automation, before its fader and pan) to its own file, so while recording only audio is read and summed and the buffer can go lower without dropouts. Armed strips stay live; the master stays live. It does not shorten the interface's round trip. Runs in the background (a minute or more for a long song): get_recording_mode says when it is on. While it is on, every edit that changes the sound is refused ('Recording mode: leave it to edit'), but in the armed tracks; arming a track whose strip is baked brings that strip live again (no render made again). Refused while Link, MIDI clock or time code is followed or a session scene plays. leave_recording_mode gives the project back. Neither is an undo step: the mode is the session's. With dry_run=true nothing is changed: it answers what would be rendered, what stays live and the disk the renders take.", class: ToolClass::Edit, input_schema: prepare_schema, output_schema: None, run: ToolRun::Host(prepare_to_record) },
    ToolDef { name: "leave_recording_mode", title: "Leave the recording mode", description: "Leave the recording mode: what it baked plays live again (no undo step: the mode is the session's). The rendered files are kept by default, so entering again renders only what changed; discard_files=true deletes them. Refused while a take is being recorded.", class: ToolClass::Edit, input_schema: leave_schema, output_schema: None, run: ToolRun::Host(leave_recording_mode) },
    ToolDef { name: "get_recording_mode", title: "Read the recording mode", description: "Whether the recording mode is off, preparing (how far) or on, and for the mode on: the strips baked and the ones left live with the reason, the buffer, the measured round trip, the CPU before and since, the disk the renders take, and whether the mix controls are live or locked.", class: ToolClass::Read, input_schema: empty, output_schema: None, run: ToolRun::Read(get_recording_mode) },
];

fn empty() -> Value {
    args::object(json!({}), &[])
}

fn prepare_schema() -> Value {
    args::object(
        json!({
            "master": {"enum": ["live", "baked"], "default": "live", "description": "live: the master chain runs on the sum. baked: the whole mix (but what stays live) is one render, the lightest; nothing of the master or the mix moves."},
            "mix_controls": {"enum": ["live", "locked"], "default": "live", "description": "live: each baked strip's fader, pan, width, mute and solo still work at the sum (a strip that sends post-fader into a return keeps that fader locked: the return's render holds it). locked: nothing of the mix moves."},
            "on_failure": {"enum": ["stay_live", "stop"], "default": "stay_live", "description": "A strip that cannot be rendered (a hosted plug-in that cannot render offline) stays live and is named, or stops the preparation and bakes nothing."},
            "dry_run": {"type": "boolean", "default": false, "description": "Change nothing: answer the plan and its size."}
        }),
        &[],
    )
}

fn leave_schema() -> Value {
    args::object(
        json!({
            "discard_files": {"type": "boolean", "default": false, "description": "Delete the rendered files too."}
        }),
        &[],
    )
}

fn app_of<'a>(cx: &'a ToolCx<'_>) -> Res<&'a CantusApp> {
    cx.host
        .as_any()
        .downcast_ref::<CantusApp>()
        .ok_or_else(|| ToolError::Failed("this tool needs the Cantus window".into()))
}

fn name_of(app: &CantusApp, t: FreezeTarget) -> String {
    crate::freeze::name_of(app.project(), t)
}

fn why_word(w: Why) -> &'static str {
    match w {
        Why::Armed(_) => "armed",
        Why::Monitoring(_) => "monitoring",
        Why::Soloed => "soloed",
        Why::Dancer => "dancer",
        Why::External(_) => "external",
        Why::CueSolo(_) => "cue_solo",
    }
}

fn kind_word(t: FreezeTarget) -> &'static str {
    match t {
        FreezeTarget::Strip(_) => "strip",
        FreezeTarget::Track(_) => "track",
    }
}

fn prepare_to_record(cx: &mut ToolCx<'_>, a: &Value) -> Res<Value> {
    let a = A(a);
    let live = a
        .opt_choice("mix_controls", &["live", "locked"])?
        .unwrap_or("live")
        == "live";
    let stop = a
        .opt_choice("on_failure", &["stay_live", "stop"])?
        .unwrap_or("stay_live")
        == "stop";
    let dry = a.opt_bool("dry_run")?.unwrap_or(false);
    let master_live = a
        .opt_choice("master", &["live", "baked"])?
        .unwrap_or("live")
        == "live";
    let options = Options {
        mix_live: live && master_live,
        master_live,
        stop_on_failure: stop,
    };
    if dry {
        let app = app_of(cx)?;
        if let Some(why) = rm::cannot_start(app) {
            return Err(ToolError::Refused(why));
        }
        let plan = plan_of::plan(app.project());
        let est = rm::estimate(app, &plan, options);
        return Ok(json!({
            "dry_run": true,
            "bake": plan.bake.iter().map(|t| json!({"kind": kind_word(*t), "name": name_of(app, *t)})).collect::<Vec<_>>(),
            "stay_live": plan.live.iter().map(|(t, w)| json!({"kind": kind_word(*t), "name": name_of(app, *t), "why": why_word(*w)})).collect::<Vec<_>>(),
            "already_frozen": plan.already.len(),
            "renders_on_disk": est.reused,
            "disk_new_bytes_at_most": est.bytes_new,
            "disk_all_bytes_at_most": est.bytes_all,
            "strips_with_a_locked_mix": est.locked_mix,
            "seconds_each": est.seconds,
            "note": "It does not shorten the interface's round trip: it lets the buffer go lower without dropouts."
        }));
    }
    let app = crate::ai_tools::project_app::app_mut(cx)?;
    app.record_mode_begin(options).map_err(ToolError::Refused)?;
    Ok(json!({"started": true, "follow": "get_recording_mode"}))
}

fn leave_recording_mode(cx: &mut ToolCx<'_>, a: &Value) -> Res<Value> {
    let discard = A(a).opt_bool("discard_files")?.unwrap_or(false);
    let app = crate::ai_tools::project_app::app_mut(cx)?;
    if !app.freeze.record.on() {
        return Err(ToolError::Refused("the recording mode is not on".into()));
    }
    app.record_mode_leave(discard).map_err(ToolError::Refused)?;
    Ok(json!({"left": true, "files": if discard { "discarded" } else { "kept" }}))
}

fn get_recording_mode(cx: &ToolCx<'_>, _: &Value) -> Res<Value> {
    let app = app_of(cx)?;
    let s = &app.freeze.record;
    let cpu = |v: Option<f32>| {
        v.map(|x| json!((x * 100.0).round() / 100.0))
            .unwrap_or(Value::Null)
    };
    Ok(match &s.phase {
        Phase::Idle => json!({"state": "off"}),
        Phase::Preparing(p) => {
            let running = app
                .freeze
                .progress()
                .map(|(n, f)| json!({"name": n, "progress": f}));
            json!({
                "state": "preparing",
                "total": p.total,
                "queued": app.freeze.queued(),
                "rendering": running,
            })
        }
        Phase::Active(a) => {
            let st = app.engine.stats();
            let rate = app.engine.sample_rate.max(1.0);
            json!({
                "state": "on",
                "baked": a.baked.iter().map(|t| json!({"kind": kind_word(*t), "name": name_of(app, *t)})).collect::<Vec<_>>(),
                "stay_live": a.live.iter().map(|(t, w)| json!({"kind": kind_word(*t), "name": name_of(app, *t), "why": why_word(*w)})).collect::<Vec<_>>(),
                "could_not_bake": a.failed.iter().map(|(t, why)| json!({"name": name_of(app, *t), "why": why})).collect::<Vec<_>>(),
                "mix_controls": if a.options.mix_live { "live" } else { "locked" },
                "master": if a.master.is_some() { "baked" } else { "live" },
                "buffer_frames": st.block_frames,
                "buffer_ms": st.block_frames as f64 * 1000.0 / rate,
                "round_trip_ms": rm::round_trip(app).map(|(f, _)| f as f64 * 1000.0 / rate),
                "round_trip_measured": rm::round_trip(app).is_some_and(|(_, m)| m),
                "cpu_before": cpu(s.cpu_before()),
                "cpu_now": cpu(s.cpu_now()),
                "disk_bytes": a.bytes,
                "seconds_on": a.since.elapsed().as_secs(),
            })
        }
    })
}
