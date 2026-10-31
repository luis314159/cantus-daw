//! The project tools that need the application (round 34, builder `tools`, ADR 0082): the
//! transport and the selection, the unit catalogue (its names and presets live with the
//! editors), presets, comping, and export. Registered after `cantus-ai`'s own list by a
//! marked line in `ai_tools.rs`; the catalogue's resources by one in `ai_catalog.rs`.

use crate::app::CantusApp;
use cantus_ai::tools_project::args::{self, plan, s_id, Res, A};
use cantus_ai::tools_project::words;
use cantus_ai::{
    EditPlan, ResourceBody, ResourceDef, ResourceTemplateDef, Said, TemplateVars, ToolClass,
    ToolCx, ToolDef, ToolError, ToolRun,
};
use cantus_core::{Command, EffectKind, InstrumentSpec, TrackId};
use serde_json::{json, Value};

#[path = "agent_export.rs"]
pub mod export;
#[path = "agent_units.rs"]
pub mod units;

fn app_of<'a>(cx: &'a ToolCx<'_>) -> Res<&'a CantusApp> {
    cx.host
        .as_any()
        .downcast_ref::<CantusApp>()
        .ok_or_else(|| ToolError::Failed("this tool needs the Cantus window".into()))
}

pub fn app_mut<'a>(cx: &'a mut ToolCx<'_>) -> Res<&'a mut CantusApp> {
    cx.host
        .as_any_mut()
        .downcast_mut::<CantusApp>()
        .ok_or_else(|| ToolError::Failed("this tool needs the Cantus window".into()))
}

// --- transport and selection -------------------------------------------------------------

fn transport_schema() -> Value {
    args::object(
        json!({
            "action": {"enum": ["play", "stop", "locate"], "description": "play from the playhead, stop (a second stop returns to the start), or locate the playhead."},
            "at": args::s_pos("Where to put the playhead (locate; play may take it too)")
        }),
        &["action"],
    )
}

fn transport(cx: &mut ToolCx<'_>, a: &Value) -> Res<Value> {
    let a = A(a);
    let action = a
        .opt_choice("action", &["play", "stop", "locate"])?
        .ok_or_else(|| ToolError::bad("action", "is required"))?;
    let map = cx.project().tempo_map();
    let at = args::opt_pos(&map, a, "at")?;
    let app = app_mut(cx)?;
    // A take is the user's: an agent neither ends it nor moves the playhead under it
    // (review #11).
    if app.is_recording() {
        return Err(ToolError::Refused(
            "Cantus is recording: the transport is the user's until the take ends".into(),
        ));
    }
    match action {
        "locate" => app.seek(at.ok_or_else(|| ToolError::bad("at", "where to locate"))?),
        "play" => {
            if let Some(t) = at {
                app.seek(t);
            }
            if !app.engine.is_playing() {
                app.toggle_play();
            }
        }
        _ => {
            app.stop();
        }
    }
    let pos = at.unwrap_or_else(|| app.engine.position());
    Ok(
        json!({"playing": app.engine.is_playing(), "position": args::pos_json(&map, pos), "note": "transport is not an undo step"}),
    )
}

fn get_selection(cx: &ToolCx<'_>, _: &Value) -> Res<Value> {
    let app = app_of(cx)?;
    let p = app.project();
    let map = p.tempo_map();
    let export = &app.export;
    Ok(json!({
        "track_id": app.selected_track.map(|t| t.0),
        "clip_id": app.selected_clip.map(|c| c.0),
        "clips": app.selected_clips.iter().map(|r| json!({"track_id": r.0 .0, "clip_id": r.1 .0})).collect::<Vec<_>>(),
        "note_ids": app.selected_notes.iter().map(|n| n.0).collect::<Vec<_>>(),
        "strip_id": app.selected_strip.map(args::strip_json),
        "strips": app.selected_strips.iter().map(|s| s.0).collect::<Vec<_>>(),
        "time_selection": app.time_selection.map(|(a, b)| json!({"start": args::pos_json(&map, a), "end": args::pos_json(&map, b)})),
        "transport": {
            "playing": app.engine.is_playing(),
            "recording": app.is_recording(),
            "position": args::pos_json(&map, app.engine.position()),
        },
        "export": {
            "busy": export.busy(),
            "progress": export.progress().and_then(|(_, f)| f),
            "last": export.last.as_ref().map(|r| match r { Ok(s) => json!({"done": s}), Err(e) => json!({"failed": e}) }),
        },
    }))
}

// --- the catalogue -----------------------------------------------------------------------

fn get_unit_schema() -> Value {
    args::object(
        json!({"id": {"type": "string", "description": "A unit's id from list_units (\"aequalis\", \"chorda\", \"pulsus\"…), or a plain role (\"eq\", \"compressor\")."}}),
        &["id"],
    )
}

fn list_units(_: &ToolCx<'_>, _: &Value) -> Res<Value> {
    Ok(units::list())
}

fn get_unit(_: &ToolCx<'_>, a: &Value) -> Res<Value> {
    let id = A(a).str("id")?;
    units::entry(id)
        .ok_or_else(|| ToolError::NotFound(format!("a unit {id:?} (list_units names them)")))
}

// --- presets -----------------------------------------------------------------------------

fn device_args() -> Value {
    json!({
        "effect_id": s_id("An insert effect."),
        "track_id": s_id("A track, for its instrument."),
    })
}

fn list_presets_schema() -> Value {
    let mut p = device_args();
    p["unit"] = json!({"type": "string", "description": "Or a unit's id, without a device in the project."});
    args::object(p, &[])
}

/// The presets of a device's kind: factory ones with what they set, then the user's.
fn presets_of(kind: Kind) -> Vec<(String, Preset)> {
    match kind {
        Kind::Effect(k) => crate::views::fx::factory_presets(k)
            .iter()
            .map(|(label, v)| {
                (
                    crate::i18n::t_in(crate::i18n::Locale::En, label),
                    Preset::Effect(v.iter().map(|(a, b)| (a.to_string(), *b)).collect()),
                )
            })
            .chain(
                crate::views::fx::user_presets(k)
                    .into_iter()
                    .map(|(n, v)| (n, Preset::Effect(v.into_iter().collect()))),
            )
            .collect(),
        Kind::Native(k) => crate::views::inst::factory_presets(k)
            .iter()
            .map(|fp| {
                (
                    crate::i18n::t_in(crate::i18n::Locale::En, fp.label_key),
                    Preset::Native(crate::views::inst::preset_spec(k, fp)),
                )
            })
            .chain(
                crate::views::inst::user_presets(k)
                    .into_iter()
                    .map(|(n, s)| (n, Preset::Native(s))),
            )
            .collect(),
        Kind::Pulsus => vec![
            (
                "Default kit".into(),
                Preset::Kit(cantus_core::PulsusParams::default()),
            ),
            (
                "Studio kit".into(),
                Preset::Kit(cantus_core::PulsusParams::studio()),
            ),
        ],
    }
}

#[derive(Copy, Clone)]
enum Kind {
    Effect(EffectKind),
    Native(cantus_core::InstrumentKind),
    Pulsus,
}

enum Preset {
    Effect(Vec<(String, f32)>),
    Native(cantus_core::NativeSpec),
    Kit(cantus_core::PulsusParams),
}

fn kind_of(cx: &ToolCx<'_>, a: A<'_>) -> Res<Kind> {
    let p = cx.project();
    if a.has("effect_id") {
        let (host, id) = args::effect(p, a, "effect_id")?;
        let k = p.effect(host, id).expect("found").spec.kind;
        if k == EffectKind::External {
            return Err(ToolError::bad(
                "effect_id",
                "a hosted plug-in keeps its presets in its own window",
            ));
        }
        return Ok(Kind::Effect(k));
    }
    if a.has("track_id") {
        let t = args::track(p, a, "track_id")?;
        return match &p.track(t).expect("checked").instrument {
            InstrumentSpec::Native(n) => Ok(Kind::Native(n.kind)),
            InstrumentSpec::Pulsus(_) => Ok(Kind::Pulsus),
            _ => Err(ToolError::bad(
                "track_id",
                "Firmus and hosted instruments have no presets here",
            )),
        };
    }
    if let Some(u) = a.opt_str("unit")? {
        if let Some(k) = args::effect_kind(u) {
            return Ok(Kind::Effect(k));
        }
        return match args::instrument_choice(u) {
            Some(args::InstrumentChoice::Native(k)) => Ok(Kind::Native(k)),
            Some(args::InstrumentChoice::Pulsus) => Ok(Kind::Pulsus),
            _ => Err(ToolError::NotFound(format!("presets of {u:?}"))),
        };
    }
    Err(ToolError::bad(
        "effect_id",
        "give effect_id, track_id or unit",
    ))
}

fn list_presets(cx: &ToolCx<'_>, a: &Value) -> Res<Value> {
    let kind = kind_of(cx, A(a))?;
    let rows: Vec<Value> = presets_of(kind)
        .into_iter()
        .map(|(name, p)| match p {
            Preset::Effect(v) => json!({"name": name, "sets": Value::Object(v.into_iter().map(|(k, x)| (k, json!(x))).collect())}),
            Preset::Native(s) => json!({"name": name, "sets": Value::Object(s.params.iter().map(|(k, x)| (k.clone(), json!(x))).collect())}),
            Preset::Kit(k) => json!({"name": name, "pads": k.pads.iter().map(|p| json!({"note": p.pitch, "name": p.name})).collect::<Vec<_>>()}),
        })
        .collect();
    Ok(json!({"presets": rows}))
}

fn apply_preset_schema() -> Value {
    let mut p = device_args();
    p["preset"] = json!({"type": "string", "description": "The preset's name as list_presets gives it (any case)."});
    args::object(p, &["preset"])
}

fn apply_preset(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let kind = kind_of(cx, a)?;
    let wanted = a.str("preset")?.trim().to_lowercase();
    let (name, preset) = presets_of(kind)
        .into_iter()
        .find(|(n, _)| n.to_lowercase() == wanted)
        .ok_or_else(|| {
            ToolError::NotFound(format!("a preset {wanted:?} (list_presets names them)"))
        })?;
    let p = cx.project();
    let (command, target) = match preset {
        Preset::Effect(values) => {
            let (host, id) = args::effect(p, a, "effect_id")?;
            let mut slot = p.effect(host, id).expect("found").clone();
            let mut spec = cantus_core::EffectSpec::new(slot.spec.kind);
            for (k, v) in &values {
                let _ = spec.set(k, *v);
            }
            slot.spec = spec;
            (
                Command::ReplaceEffect {
                    strip: host,
                    effect: id,
                    slot: Box::new(slot),
                },
                words::effect(p, host, id),
            )
        }
        Preset::Native(target) => {
            let track = args::track(p, a, "track_id")?;
            let now = p
                .track(track)
                .and_then(|t| t.instrument.native())
                .expect("native")
                .clone();
            (
                inst_commands(track, &now, &target)?,
                words::instrument_of(p, track),
            )
        }
        Preset::Kit(kit) => {
            let track = args::track(p, a, "track_id")?;
            (
                Command::SetInstrument {
                    track,
                    spec: Box::new(InstrumentSpec::Pulsus(kit)),
                },
                words::kit_of(p, track),
            )
        }
    };
    let said = Said::new("ai.tools.apply_preset")
        .name("preset", args::clean(&name))
        .nested("target", target);
    Ok(plan(command, said, json!({"preset": name})))
}

/// The commands that make a native instrument's state equal `target`, as the editors'
/// preset menu does (`apply_inst_spec`): one step.
fn inst_commands(
    track: TrackId,
    now: &cantus_core::NativeSpec,
    target: &cantus_core::NativeSpec,
) -> Res<Command> {
    let mut c = Vec::new();
    for (k, v) in &target.params {
        if now.params.get(k) != Some(v) {
            c.push(Command::SetInstrumentValue {
                track,
                key: k.clone(),
                value: *v,
            });
        }
    }
    for k in now
        .params
        .keys()
        .filter(|k| !target.params.contains_key(*k))
    {
        c.push(Command::ClearInstrumentValue {
            track,
            key: k.clone(),
        });
    }
    for (k, v) in &target.text {
        if now.text.get(k) != Some(v) {
            c.push(Command::SetInstrumentText {
                track,
                key: k.clone(),
                text: Some(v.clone()),
            });
        }
    }
    for k in now.text.keys().filter(|k| !target.text.contains_key(*k)) {
        c.push(Command::SetInstrumentText {
            track,
            key: k.clone(),
            text: None,
        });
    }
    args::one(c, "cmd.ai.apply_preset")
        .map_err(|_| ToolError::bad("preset", "the instrument already has exactly this preset"))
}

// --- the table -----------------------------------------------------------------------------

fn empty() -> Value {
    args::object(json!({}), &[])
}

/// This builder's application tools, after `cantus_ai::tools_project::TOOLS`.
pub static TOOLS: &[ToolDef] = &[
    ToolDef { name: "transport", title: "Play, stop, locate", description: "Play from the playhead, stop (a second stop returns to the start) or locate the playhead at a position. Refused while Cantus records (the take is the user's). Not an undo step: it changes no part of the project.", class: ToolClass::Transport, input_schema: transport_schema, output_schema: None, run: ToolRun::Host(transport) },
    ToolDef { name: "get_selection", title: "Read the selection and transport", description: "What the user has selected (track, clips, notes, strips, a time range), whether the transport plays or records and where, and the state of an export started by export (busy, progress 0-1, how the last one ended).", class: ToolClass::Read, input_schema: empty, output_schema: None, run: ToolRun::Read(get_selection) },
    ToolDef { name: "list_units", title: "List every unit", description: "Every native instrument and effect: id, name, what it is for, how many parameters and presets — and the model's limits. get_unit gives one in full.", class: ToolClass::Read, input_schema: empty, output_schema: None, run: ToolRun::Read(list_units) },
    ToolDef { name: "get_unit", title: "Describe a unit", description: "One instrument or effect in full: every parameter's key, English name, group (Band 4…), unit, range, default, taper (linear/log/stepped) and a stepped one's choices by number and name; its factory presets and what they set; its starting preset; latency at 48 kHz; whether it takes a sidechain or hears notes; what it is for. Generated from the parameter tables — keys here are what set_effect, set_instrument_param and automate take.", class: ToolClass::Read, input_schema: get_unit_schema, output_schema: None, run: ToolRun::Read(get_unit) },
    ToolDef { name: "list_presets", title: "List presets", description: "The presets of a device (effect_id or track_id) or of a unit: factory presets with what they set, then the user's own. Pulsus has its kits.", class: ToolClass::Read, input_schema: list_presets_schema, output_schema: None, run: ToolRun::Read(list_presets) },
    ToolDef { name: "apply_preset", title: "Apply a preset", description: "Load a preset by name into an effect (effect_id) or a track's instrument (track_id): the device's settings are replaced — what the preset does not name goes back to its default, as the editors' preset menu does — so the user is asked. One undo step.", class: ToolClass::Destructive, input_schema: apply_preset_schema, output_schema: None, run: ToolRun::Edit(apply_preset) },
    ToolDef { name: "comp_takes", title: "Comp recorded takes", description: "Comping of a track's recorded takes (lanes 1…): swipe a range of a take lane into the comp, promote a whole lane or one take clip, switch to another alternate comp (A, B, C…), duplicate the comp, rename one. Tracks recorded together comp together. One undo step.", class: ToolClass::Edit, input_schema: export::comp_schema, output_schema: None, run: ToolRun::Edit(export::comp_takes) },
    ToolDef { name: "tidy_takes", title: "Tidy takes", description: "Destructive comping: delete an alternate comp, flatten the comp (the takes go, the comp stays as plain clips), or delete the take clips the comp does not use. One undo step the user confirms.", class: ToolClass::Destructive, input_schema: export::tidy_schema, output_schema: None, run: ToolRun::Edit(export::tidy_takes) },
    ToolDef { name: "export_audio", title: "Export audio or stems", description: "Render the song (or the loop, the time selection, or every strip as a stem) to WAV, FLAC, MP3 or Ogg, into the project's folder (a relative file name) or a folder the user allowed in Settings, AI. Refused until the user allows files, and always asked. It runs in the background: get_selection's export field says when it ends. Never overwrites unless overwrite=true.", class: ToolClass::Files, input_schema: export::export_audio_schema, output_schema: None, run: ToolRun::HostSaid { run: export::export_audio, said: export::export_audio_said } },
    ToolDef { name: "export_project", title: "Export MIDI or DAWproject", description: "Write the notes as a Standard MIDI File (the song, a track or a clip; as played or as written), or the whole project as a .dawproject other DAWs open, into the project's folder or a folder the user allowed. Refused until the user allows files, and always asked. Answers when the file is written (a long DAWproject as a job). Never overwrites unless overwrite=true.", class: ToolClass::Files, input_schema: export::export_project_schema, output_schema: None, run: ToolRun::Job(export::export_project) },
];

fn units_list(_: &ToolCx<'_>) -> Result<ResourceBody, ToolError> {
    serde_json::to_string_pretty(&units::list())
        .map(ResourceBody::Text)
        .map_err(|e| ToolError::Failed(e.to_string()))
}

fn unit_one(_: &ToolCx<'_>, vars: &TemplateVars) -> Result<ResourceBody, ToolError> {
    let id = vars.get("id").map(String::as_str).unwrap_or("");
    let v = units::entry(id).ok_or_else(|| ToolError::NotFound(format!("unit {id}")))?;
    serde_json::to_string_pretty(&v)
        .map(ResourceBody::Text)
        .map_err(|e| ToolError::Failed(e.to_string()))
}

pub static RESOURCES: &[ResourceDef] = &[ResourceDef {
    uri: "cantus://units",
    name: "units",
    title: "Every instrument and effect",
    description: "The catalogue of Cantus's own units: ids, names, what each is for, parameter and preset counts, the model's limits. cantus://units/{id} holds one in full (the same as get_unit).",
    mime_type: "application/json",
    read: units_list,
}];

pub static RESOURCE_TEMPLATES: &[ResourceTemplateDef] = &[ResourceTemplateDef {
    uri_template: "cantus://units/{id}",
    name: "unit",
    title: "One unit in full",
    description: "Every parameter of one instrument or effect (key, name, unit, range, default, taper, choices), its presets and starting preset, latency and uses. The same as get_unit.",
    mime_type: "application/json",
    read: unit_one,
}];
