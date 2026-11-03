//! External gear for agents (round 36, ADR 0094, 0095): add a unit to a track, ping it, read
//! the units and their state, read the control surfaces. They go through the same commands
//! and the same service the Settings page uses.

use crate::app::CantusApp;
use crate::external_service::{self as svc, Outcome};
use cantus_ai::tools_project::args::{self, Res, A};
use cantus_ai::{EditPlan, Said, ToolClass, ToolCx, ToolDef, ToolError, ToolRun};
use cantus_core::external_unit::{ExternalUnit, ExternalUnitId, MidiRoute, Pair, UnitKind};
use cantus_core::TrackId;
use serde_json::{json, Value};

#[rustfmt::skip]
pub static TOOLS: &[ToolDef] = &[
    ToolDef { name: "add_external_unit", title: "Add external gear to a track", description: "Make a track an external instrument (its notes go out of a MIDI port to a hardware synth; the synth's sound comes back on a pair of the audio interface's inputs) or give it an external effect (its audio goes out of a pair of outputs and comes back on a pair of inputs). Channels count from 1; a pair is its first channel (stereo unless mono=true). One unit of each kind per track. The unit has no latency measured yet: call ping_external_unit with the transport stopped. One undo step.", class: ToolClass::Edit, input_schema: add_schema, output_schema: None, run: ToolRun::Edit(add_unit) },
    ToolDef { name: "ping_external_unit", title: "Measure an external unit's latency", description: "The automatic latency ping: an effect plays a noise burst train out of its send pair and finds it on its return pair; an instrument is played a note (middle C, four times) and the first sound on its return is timed. Needs the transport stopped, the audio interface of the unit open and (for an instrument) a patch that makes sound; it plays sound out of the interface for about three seconds. It runs in the background: read list_external_units for the result (frames, milliseconds, jitter, or why it failed); a good result is stored in the project and used to keep the gear in time.", class: ToolClass::Transport, input_schema: ping_schema, output_schema: None, run: ToolRun::Host(ping_unit) },
    ToolDef { name: "list_external_units", title: "Read the external units", description: "Every external unit of the project: its track, kind, MIDI port and channel or send and return pairs (from 1), the audio interface it was set up with, whether it can play with what is open now (and why not), its measured latency in frames and milliseconds and the trim, and what its last ping came to.", class: ToolClass::Read, input_schema: empty, output_schema: None, run: ToolRun::Read(list_units) },
    ToolDef { name: "list_surfaces", title: "Read the control surfaces", description: "The Mackie Control, extender and HUI surfaces set up in this studio (Settings, MIDI): kind, ports, whether virtual, the bank they show and the last messages they exchanged. Surfaces are set up by the user in Settings; this reads them.", class: ToolClass::Read, input_schema: empty, output_schema: None, run: ToolRun::Read(list_surfaces) },
];

fn empty() -> Value {
    args::object(json!({}), &[])
}

fn add_schema() -> Value {
    args::object(
        json!({
            "track_id": {"type": "integer", "minimum": 0, "description": "The track."},
            "kind": {"enum": ["instrument", "effect"]},
            "name": {"type": "string", "maxLength": 64, "description": "A name for the gear (default: the track's)."},
            "midi_port": {"type": "string", "maxLength": 160, "description": "Instrument: the MIDI output port's name (as list_external_units or Settings shows it). Default: the first output."},
            "midi_channel": {"type": "integer", "minimum": 1, "maximum": 16, "default": 1},
            "send_channel": {"type": "integer", "minimum": 1, "maximum": 512, "default": 1, "description": "Effect: the first interface output channel of the send pair."},
            "return_channel": {"type": "integer", "minimum": 1, "maximum": 512, "default": 1, "description": "The first interface input channel of the return pair."},
            "mono": {"type": "boolean", "default": false, "description": "Send and return are one channel each instead of a stereo pair."}
        }),
        &["track_id", "kind"],
    )
}

fn ping_schema() -> Value {
    args::object(
        json!({"unit_id": {"type": "integer", "minimum": 0, "description": "The unit's id from list_external_units."}}),
        &["unit_id"],
    )
}

fn app_of<'a>(cx: &'a ToolCx<'_>) -> Res<&'a CantusApp> {
    cx.host
        .as_any()
        .downcast_ref::<CantusApp>()
        .ok_or_else(|| ToolError::Failed("this tool needs the Cantus window".into()))
}

fn add_unit(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let track: TrackId = args::track(cx.project(), a, "track_id")?;
    let kind = match a.opt_choice("kind", &["instrument", "effect"])? {
        Some("effect") => UnitKind::Effect,
        Some(_) => UnitKind::Instrument,
        None => return Err(ToolError::bad("kind", "instrument or effect")),
    };
    if cx.project().external_unit_of(track, kind).is_some() {
        return Err(ToolError::Refused(
            "the track already has external gear of that kind".into(),
        ));
    }
    let mono = a.opt_bool("mono")?.unwrap_or(false);
    let pair = |k: &str| -> Res<Pair> {
        let c = a.opt_u64(k)?.unwrap_or(1);
        if !(1..=512).contains(&c) {
            return Err(ToolError::bad(k, "1 to 512"));
        }
        Ok(Pair {
            first: (c - 1) as u16,
            stereo: !mono,
        })
    };
    let ret = pair("return_channel")?;
    let (device, port) = {
        let app = app_of(cx)?;
        let present = svc::present(app);
        (
            present.device.unwrap_or_default(),
            present.midi_outputs.first().cloned().unwrap_or_default(),
        )
    };
    let id = cx.project().next_id;
    cx.bus_mut().reserve_ids(id + 1);
    let id = ExternalUnitId(id);
    let channel = a.opt_u64("midi_channel")?.unwrap_or(1);
    if !(1..=16).contains(&channel) {
        return Err(ToolError::bad("midi_channel", "1 to 16"));
    }
    let mut unit = match kind {
        UnitKind::Instrument => ExternalUnit::instrument(
            id,
            track,
            MidiRoute {
                port: a.opt_str("midi_port")?.map_or(port, str::to_string),
                ordinal: 0,
                channel: (channel - 1) as u8,
            },
            ret,
        ),
        UnitKind::Effect => ExternalUnit::effect(id, track, pair("send_channel")?, ret),
    };
    unit.device = device;
    let track_name = cx
        .project()
        .track(track)
        .map(|t| t.name.clone())
        .unwrap_or_default();
    unit.name = a
        .opt_str("name")?
        .map_or_else(|| track_name.clone(), str::to_string);
    if !unit.is_valid() {
        return Err(ToolError::bad("unit", "a valid routing"));
    }
    let command = crate::external_service::add_commands(cx.project(), unit);
    Ok(EditPlan {
        command,
        said: Said::new("external.tool.add")
            .arg(
                "kind",
                crate::i18n::t(match kind {
                    UnitKind::Instrument => "external.kind.instrument",
                    UnitKind::Effect => "external.kind.effect",
                }),
            )
            .name("track", track_name),
        result: json!({"unit_id": id.0, "track_id": track.0, "follow": "ping_external_unit"}),
    })
}

fn ping_unit(cx: &mut ToolCx<'_>, a: &Value) -> Res<Value> {
    let id = ExternalUnitId(A(a).u64("unit_id")?);
    let app = crate::ai_tools::project_app::app_mut(cx)?;
    svc::start_ping(app, id).map_err(|why| ToolError::Refused(crate::i18n::t(why)))?;
    Ok(json!({"started": true, "unit_id": id.0, "follow": "list_external_units"}))
}

fn pair_json(p: Pair) -> Value {
    json!({"first_channel": p.first + 1, "stereo": p.stereo})
}

fn list_units(cx: &ToolCx<'_>, _: &Value) -> Res<Value> {
    let app = app_of(cx)?;
    let rate = app.engine.sample_rate;
    let units: Vec<Value> = app
        .project()
        .external_units
        .iter()
        .map(|u| {
            let state = svc::state(app, u);
            let last = match app.external.outcomes.get(&u.id) {
                Some(Outcome::Measuring) => json!({"state": "measuring"}),
                Some(Outcome::Done(r)) => json!({"state": "done", "frames": r.frames, "jitter_frames": r.jitter, "polarity_inverted": r.inverted}),
                Some(Outcome::Failed(e)) => json!({"state": "failed", "why": crate::i18n::t(e.key())}),
                None => Value::Null,
            };
            json!({
                "unit_id": u.id.0,
                "track_id": u.track.0,
                "name": u.name,
                "kind": u.kind.id(),
                "midi_port": u.midi.as_ref().map(|m| m.port.clone()),
                "midi_channel": u.midi.as_ref().map(|m| m.channel + 1),
                "send": u.send.map(pair_json),
                "return": pair_json(u.ret),
                "interface": u.device,
                "enabled": u.enabled,
                "available": state.is_ready(),
                "state": crate::i18n::t(state.key()),
                "measured": u.latency.as_ref().map(|m| json!({"frames": m.frames, "rate_hz": m.rate, "ms": m.ms()})),
                "trim_ms": u.trim_ms,
                "return_db": u.return_db,
                "latency_frames_now": u.latency_frames(rate),
                "last_ping": last,
            })
        })
        .collect();
    Ok(json!({ "units": units }))
}

fn list_surfaces(cx: &ToolCx<'_>, _: &Value) -> Res<Value> {
    let app = app_of(cx)?;
    let s = &app.surfaces;
    let devices: Vec<Value> = s
        .choices()
        .iter()
        .map(|c| {
            json!({
                "kind": c.kind,
                "input": c.input,
                "output": c.output,
                "virtual": c.virtual_unit,
            })
        })
        .collect();
    Ok(json!({
        "surfaces": devices,
        "bank_first_strip": s.set.as_ref().map(|x| x.bank + 1),
        "flip": s.set.as_ref().is_some_and(|x| x.flip),
        "messages_received": s.received,
        "messages_sent": s.sent,
        "log": s.log.iter().rev().take(12).rev().collect::<Vec<_>>(),
    }))
}
