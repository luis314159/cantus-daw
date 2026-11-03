//! ARA 2 for agents (round 36, ADR 0097): the ARA plug-ins found and the clips bound to
//! them, binding a clip to one, letting it go. Each binding is one undo step, as in the
//! clip's menu; the plug-in renders the clip inside Cantus's own process (not isolated).

use crate::app::CantusApp;
use cantus_ai::tools_project::args::{self, Res, A};
use cantus_ai::{EditPlan, Said, ToolClass, ToolCx, ToolDef, ToolError, ToolRun};
use cantus_core::ara::AraClip;
use cantus_core::Command;
use cantus_host::ara::{plugin_ref, AraStatus};
use serde_json::{json, Value};

#[rustfmt::skip]
pub static TOOLS: &[ToolDef] = &[
    ToolDef { name: "list_ara_plugins", title: "List ARA plug-ins", description: "The ARA 2 plug-ins found by the plug-in scan (pitch, time and spectral editors that see a whole recording: id, name, vendor) and every audio clip bound to one, with how it stands: live (the plug-in renders it) or why it plays its source (not installed, failed, stopped after a crash or hang, ARA off, loading, no source, on a take lane). ARA plug-ins run inside Cantus, not isolated.", class: ToolClass::Read, input_schema: empty, output_schema: None, run: ToolRun::Read(list_ara_plugins) },
    ToolDef { name: "bind_ara_plugin", title: "Bind a clip to an ARA plug-in", description: "Bind an audio clip (clip_id) to an ARA 2 plug-in (plugin: its name or id, from list_ara_plugins): the plug-in then renders the clip, which plays its render in place of its file. Refused on a reversed clip or one with a warp or pitch edits (the plug-in does its own pitch and time). A fresh binding: the plug-in starts clean. The plug-in runs inside Cantus, not isolated. One undo step.", class: ToolClass::Edit, input_schema: bind_schema, output_schema: None, run: ToolRun::Edit(bind_ara_plugin) },
    ToolDef { name: "unbind_ara_plugin", title: "Let a clip go of its ARA plug-in", description: "Let an audio clip (clip_id) go of the ARA plug-in it is bound to: it plays its file again. Undo binds it again with what the plug-in had done to it. One undo step.", class: ToolClass::Edit, input_schema: clip_schema, output_schema: None, run: ToolRun::Edit(unbind_ara_plugin) },
];

fn empty() -> Value {
    args::object(json!({}), &[])
}

fn clip_schema() -> Value {
    args::object(
        json!({"clip_id": {"type": "integer", "minimum": 0}}),
        &["clip_id"],
    )
}

fn bind_schema() -> Value {
    args::object(
        json!({
            "clip_id": {"type": "integer", "minimum": 0},
            "plugin": {"type": "string", "minLength": 1, "maxLength": 200, "description": "The plug-in's name or id, as list_ara_plugins gives it."}
        }),
        &["clip_id", "plugin"],
    )
}

fn app<'a>(cx: &'a ToolCx<'_>) -> Res<&'a CantusApp> {
    cx.host
        .as_any()
        .downcast_ref::<CantusApp>()
        .ok_or_else(|| ToolError::Failed("this tool needs the Cantus window".into()))
}

fn status_word(s: Option<&AraStatus>) -> Value {
    match s {
        Some(AraStatus::Live) => json!("live"),
        Some(AraStatus::NotInstalled) | None => json!("not_installed"),
        Some(AraStatus::Failed(e)) => json!({"failed": e}),
        Some(AraStatus::NoSource) => json!("no_source"),
        Some(AraStatus::OnTakeLane) => json!("take_lane"),
        Some(AraStatus::Loading) => json!("loading"),
        Some(AraStatus::Stopped(e)) => json!({"stopped": e}),
        Some(AraStatus::Off) => json!("off"),
    }
}

fn list_ara_plugins(cx: &ToolCx<'_>, _: &Value) -> Res<Value> {
    let app = app(cx)?;
    let plugins: Vec<Value> = app
        .ara_plugins()
        .iter()
        .map(|p| json!({"id": p.id, "name": p.name, "vendor": p.vendor}))
        .collect();
    let mut bound = Vec::new();
    for t in &app.project().tracks {
        for c in &t.clips {
            let Some(a) = c.audio.as_ref().and_then(|a| a.ara.as_ref()) else {
                continue;
            };
            bound.push(json!({
                "clip_id": c.id.0,
                "track_id": t.id.0,
                "plugin": a.plugin.name,
                "plugin_id": a.plugin.id,
                "status": status_word(app.plugins.ara.status(c.id)),
            }));
        }
    }
    Ok(json!({"plugins": plugins, "bound_clips": bound, "in_process": true}))
}

fn bind_ara_plugin(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let wanted = a.str("plugin")?.trim().to_string();
    let (track, clip_id, has_audio) = {
        let (t, c) = args::clip(cx.project(), a, "clip_id")?;
        (t, c.id, c.audio.is_some())
    };
    if !has_audio {
        return Err(ToolError::bad("clip_id", "an audio clip"));
    }
    let app = app(cx)?;
    if !app.ara_can_bind((track, clip_id)) {
        return Err(ToolError::Refused(
            "the clip has a warp or pitch edits: remove them first".into(),
        ));
    }
    let info = app
        .ara_plugins()
        .into_iter()
        .find(|p| p.id.eq_ignore_ascii_case(&wanted) || p.name.eq_ignore_ascii_case(&wanted))
        .ok_or_else(|| {
            ToolError::NotFound(format!(
                "an ARA plug-in {wanted:?} (list_ara_plugins names them)"
            ))
        })?;
    let factory = cantus_host::ara::factory_id(&info).map_err(ToolError::Refused)?;
    let mut ara = AraClip::new(plugin_ref(&info, &factory));
    // A fresh binding: the plug-in starts clean on the clip.
    ara.binding = cx.bus_mut().alloc_id();
    Ok(EditPlan {
        command: Command::SetClipAra {
            track,
            clip: clip_id,
            ara: Some(Box::new(ara)),
        },
        said: Said::new("ara.tool.bind").name("plugin", args::clean(&info.name)),
        result: json!({"clip_id": clip_id.0, "plugin": info.name, "in_process": true}),
    })
}

fn unbind_ara_plugin(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let (track, clip) = args::clip(cx.project(), a, "clip_id")?;
    let name = clip
        .audio
        .as_ref()
        .and_then(|r| r.ara.as_ref())
        .map(|r| r.plugin.name.clone())
        .ok_or_else(|| ToolError::bad("clip_id", "a clip bound to an ARA plug-in"))?;
    let id = clip.id;
    Ok(EditPlan {
        command: Command::SetClipAra {
            track,
            clip: id,
            ara: None,
        },
        said: Said::new("ara.tool.unbind").name("plugin", args::clean(&name)),
        result: json!({"clip_id": id.0, "plugin": name}),
    })
}
