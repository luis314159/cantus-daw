//! Resources (what a client attaches as context without a call) and prompts (what the user
//! picks in the client), worded in the interface's language by the host.

use super::json::{mixer_json, project_json};
use super::read::{history_json, track_detail};
use crate::provider::Role;
use crate::tool::{
    PromptArg, PromptDef, PromptMessage, ResourceBody, ResourceDef, ResourceTemplateDef, Said,
    TemplateVars, ToolCx, ToolError,
};
use cantus_core::{ClipId, TrackId};
use serde_json::{json, Value};

fn text(v: Value) -> Result<ResourceBody, ToolError> {
    serde_json::to_string_pretty(&v)
        .map(ResourceBody::Text)
        .map_err(|e| ToolError::Failed(e.to_string()))
}

fn project(cx: &ToolCx<'_>) -> Result<ResourceBody, ToolError> {
    text(project_json(cx.project(), cx.bus().revision()))
}

fn mixer(cx: &ToolCx<'_>) -> Result<ResourceBody, ToolError> {
    text(mixer_json(cx.project()))
}

fn history(cx: &ToolCx<'_>) -> Result<ResourceBody, ToolError> {
    text(history_json(cx.bus(), 100))
}

fn id_of(vars: &TemplateVars) -> Result<u64, ToolError> {
    vars.get("id")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| ToolError::NotFound("an id".into()))
}

fn track(cx: &ToolCx<'_>, vars: &TemplateVars) -> Result<ResourceBody, ToolError> {
    let id = TrackId(id_of(vars)?);
    cx.project()
        .track(id)
        .ok_or_else(|| ToolError::NotFound(format!("track {}", id.0)))?;
    text(track_detail(cx.project(), id))
}

fn clip_notes(cx: &ToolCx<'_>, vars: &TemplateVars) -> Result<ResourceBody, ToolError> {
    let id = id_of(vars)?;
    super::args::find_clip(cx.project(), ClipId(id))
        .ok_or_else(|| ToolError::NotFound(format!("clip {id}")))?;
    text(super::read::get_clip_notes(
        cx,
        &json!({"clip_id": id, "limit": 4096}),
    )?)
}

pub static RESOURCES: &[ResourceDef] = &[
    ResourceDef {
        uri: "cantus://project",
        name: "project",
        title: "The open project",
        description: "Tempo, metre, key, loop, markers, the tempo map, every track (id, instrument, strip, level), scenes and the model's limits. The same as get_project.",
        mime_type: "application/json",
        read: project,
    },
    ResourceDef {
        uri: "cantus://mixer",
        name: "mixer",
        title: "The mixer",
        description: "Every strip with its level, pan, width, routing, sends and inserts, and the master. The same as get_mixer.",
        mime_type: "application/json",
        read: mixer,
    },
    ResourceDef {
        uri: "cantus://history",
        name: "history",
        title: "The undo history",
        description: "The last 100 steps that can be undone and redone, and who made each (user or agent).",
        mime_type: "application/json",
        read: history,
    },
];

pub static RESOURCE_TEMPLATES: &[ResourceTemplateDef] = &[
    ResourceTemplateDef {
        uri_template: "cantus://tracks/{id}",
        name: "track",
        title: "A track",
        description: "One track: its clips (with positions), instrument, modulators and session slots. The same as get_track.",
        mime_type: "application/json",
        read: track,
    },
    ResourceTemplateDef {
        uri_template: "cantus://clips/{id}/notes",
        name: "clip_notes",
        title: "A clip's notes",
        description: "Every note of a clip (or every point of an automation clip) in song positions. The same as get_clip_notes.",
        mime_type: "application/json",
        read: clip_notes,
    },
];

fn arg(args: &Value, k: &str) -> String {
    args.get(k)
        .and_then(Value::as_str)
        .map(|s| s.chars().take(200).collect())
        .unwrap_or_default()
}

fn user(said: Said) -> Vec<PromptMessage> {
    vec![PromptMessage {
        role: Role::User,
        said,
    }]
}

fn explain_mix(_: &ToolCx<'_>, _: &Value) -> Result<Vec<PromptMessage>, ToolError> {
    Ok(user(Said::new("ai.prompt.explain_mix.text")))
}

fn write_part(_: &ToolCx<'_>, a: &Value) -> Result<Vec<PromptMessage>, ToolError> {
    Ok(user(
        Said::new("ai.prompt.write_part.text")
            .arg("track", arg(a, "track"))
            .arg("bars", arg(a, "bars"))
            .arg("style", arg(a, "style")),
    ))
}

fn arrange_intro(_: &ToolCx<'_>, a: &Value) -> Result<Vec<PromptMessage>, ToolError> {
    Ok(user(
        Said::new("ai.prompt.arrange_intro.text")
            .arg("bars", arg(a, "bars"))
            .arg("style", arg(a, "style")),
    ))
}

fn tidy(_: &ToolCx<'_>, _: &Value) -> Result<Vec<PromptMessage>, ToolError> {
    Ok(user(Said::new("ai.prompt.tidy_project.text")))
}

fn teach(_: &ToolCx<'_>, a: &Value) -> Result<Vec<PromptMessage>, ToolError> {
    Ok(user(
        Said::new("ai.prompt.teach_unit.text").arg("unit", arg(a, "unit")),
    ))
}

pub static PROMPTS: &[PromptDef] = &[
    PromptDef {
        name: "explain_my_mix",
        title: "ai.prompt.explain_mix.title",
        description: "ai.prompt.explain_mix.description",
        arguments: &[],
        get: explain_mix,
    },
    PromptDef {
        name: "write_a_part",
        title: "ai.prompt.write_part.title",
        description: "ai.prompt.write_part.description",
        arguments: &[
            PromptArg {
                name: "track",
                description: "ai.prompt.arg.track",
                required: true,
            },
            PromptArg {
                name: "bars",
                description: "ai.prompt.arg.bars",
                required: false,
            },
            PromptArg {
                name: "style",
                description: "ai.prompt.arg.style",
                required: false,
            },
        ],
        get: write_part,
    },
    PromptDef {
        name: "arrange_an_intro",
        title: "ai.prompt.arrange_intro.title",
        description: "ai.prompt.arrange_intro.description",
        arguments: &[
            PromptArg {
                name: "bars",
                description: "ai.prompt.arg.bars",
                required: false,
            },
            PromptArg {
                name: "style",
                description: "ai.prompt.arg.style",
                required: false,
            },
        ],
        get: arrange_intro,
    },
    PromptDef {
        name: "tidy_the_project",
        title: "ai.prompt.tidy_project.title",
        description: "ai.prompt.tidy_project.description",
        arguments: &[],
        get: tidy,
    },
    PromptDef {
        name: "teach_me_this_unit",
        title: "ai.prompt.teach_unit.title",
        description: "ai.prompt.teach_unit.description",
        arguments: &[PromptArg {
            name: "unit",
            description: "ai.prompt.arg.unit",
            required: true,
        }],
        get: teach,
    },
];
