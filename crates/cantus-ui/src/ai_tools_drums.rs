//! Pulsus III for agents (round 37, ADR 0100): a kit split to strips and merged back, a
//! chain per pad, a kit made from a folder or an SFZ file. Each is one ordinary command (or
//! one batch) — one undo step, the same the kit's menu, its pad card and a drop run.

use cantus_ai::tools_project::args::{self, plan, Res, A};
use cantus_ai::{EditPlan, Said, ToolClass, ToolCx, ToolDef, ToolError, ToolRun};
use cantus_core::{padout, Command, EffectId, InstrumentSpec, PadId, Project, TrackId};
use serde_json::{json, Value};

#[rustfmt::skip]
pub static TOOLS: &[ToolDef] = &[
    ToolDef { name: "split_kit_outputs", title: "Split a kit to strips", description: "Give every pad of a Pulsus kit that sounds a mixer strip of its own (named after the pad, right after the kit's strip, routed into it, its pan law a balance), so each pad has its own fader, inserts and sends; the kit sums to itself to the bit while the new strips are at unity. merge=true does the reverse: every pad back on the kit's strip, the strips it made removed (a pad's chain kept). One undo step either way.", class: ToolClass::Edit, input_schema: split_schema, output_schema: None, run: ToolRun::Edit(split_kit_outputs) },
    ToolDef { name: "set_pad_chain", title: "Edit a pad's chain", description: "Insert effects on one pad of a Pulsus kit, before the kit's sum: add puts an effect (eq, compressor, gate, saturator, delay, spatium — the reverb, also named reverb — … by kind, with its starting preset) at the end of the pad's chain (made with the first); remove takes an effect out by its id (the last takes the chain). A chain at rest costs nothing. Answers the pad's chain after the change. One undo step.", class: ToolClass::Edit, input_schema: chain_schema, output_schema: None, run: ToolRun::Edit(set_pad_chain) },
    ToolDef { name: "make_kit_from_folder", title: "Make a kit from a folder", description: "Make a Pulsus kit from a folder of audio files (or an SFZ drum kit file) inside the project's folder or a folder the user allowed: names in English or Spanish map to the General MIDI drum map (kick 36, snare 38, clap 39, hats 42/44/46 choking each other, toms, crash, ride, cowbell, percussion), numbered takes become a round robin, dynamics (soft/hard, pp/ff, vl1…) velocity layers, the rest by order; at most 512 sounds. On track_id's kit (its pads' strips kept; a track that is not a kit is refused), else on a new kit track. One undo step. Answers the pads (notes, names, layer counts) and what did not fit, by file name.", class: ToolClass::Files, input_schema: folder_schema, output_schema: None, run: ToolRun::Edit(make_kit_from_folder) },
];

fn split_schema() -> Value {
    args::object(
        json!({
            "track_id": args::s_id("The kit's track"),
            "merge": {"type": "boolean", "default": false, "description": "Merge the kit back instead."}
        }),
        &["track_id"],
    )
}

fn chain_schema() -> Value {
    args::object(
        json!({
            "track_id": args::s_id("The kit's track"),
            "pad": {"type": "integer", "minimum": 0, "maximum": 127, "description": "The pad, by its note (36 the kick on the General MIDI map)."},
            "add": {"type": "string", "description": "An effect kind to add at the end of the pad's chain (eq, compressor, gate, saturator, delay, spatium — the reverb, also named reverb —, …)."},
            "remove": args::s_id("An effect of the pad's chain to remove, by id")
        }),
        &["track_id", "pad"],
    )
}

fn folder_schema() -> Value {
    args::object(
        json!({
            "path": {"type": "string", "description": "A folder of audio files (three levels deep), or an .sfz file."},
            "track_id": args::s_id("A kit's track to load it on; left out, a new kit track")
        }),
        &["path"],
    )
}

/// The kit's track, refused when it is not a kit.
fn kit_track(p: &Project, a: A<'_>) -> Res<TrackId> {
    let t = args::track(p, a, "track_id")?;
    if p.track(t).and_then(|x| x.instrument.pulsus()).is_none() {
        return Err(ToolError::Refused(format!(
            "track {} is not a Pulsus kit",
            t.0
        )));
    }
    Ok(t)
}

fn refusal(r: padout::Refusal) -> ToolError {
    ToolError::Refused(
        match r {
            padout::Refusal::NotAKit => "not a Pulsus kit",
            padout::Refusal::Nothing => {
                "nothing to do: no pad left to split, none to merge, or no such effect"
            }
            padout::Refusal::NoPad => "no such pad",
            // r37:fixes
            padout::Refusal::NoRoom => {
                "no room: the engine plays at most 256 tracks (the project's and one per pad \
                 output) and 256 strips; remove tracks or strips, or split fewer pads"
            }
        }
        .into(),
    )
}

fn split_kit_outputs(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let t = kit_track(cx.project(), a)?;
    let merge = a.opt_bool("merge")?.unwrap_or(false);
    let name = cx
        .project()
        .track(t)
        .map(|x| x.name.clone())
        .unwrap_or_default();
    let cmd = if merge {
        padout::merge(cx.project(), t).map_err(refusal)?
    } else {
        let view = cx.project().clone();
        padout::split_with(&view, t, |n| cx.bus_mut().new_strip(n)).map_err(refusal)?
    };
    let said = if merge {
        Said::new("ai.tools.padout_merge")
    } else {
        Said::new("ai.tools.padout_split")
    };
    Ok(plan(
        cmd,
        said.name("name", args::clean(&name)),
        json!({"track_id": t.0, "merged": merge}),
    ))
}

fn set_pad_chain(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let t = kit_track(cx.project(), a)?;
    let pitch = a.u64("pad")?.min(127) as u8;
    let pad: PadId = cx
        .project()
        .track(t)
        .and_then(|x| x.instrument.pulsus())
        .and_then(|k| k.pads.iter().find(|p| p.pitch == pitch))
        .map(|p| p.id)
        .ok_or_else(|| ToolError::NotFound(format!("a pad on note {pitch}")))?;
    let cmd = match (a.opt_str("add")?, a.opt_u64("remove")?) {
        (Some(kind), None) => {
            let kind = cantus_ai::tools_project::args::effect_kind(kind)
                .filter(|k| *k != cantus_core::EffectKind::External)
                .ok_or_else(|| ToolError::bad("add", "is not an effect kind"))?;
            let mut slot = cx.bus_mut().new_effect(kind);
            cantus_core::starting::apply(&mut slot.spec);
            let view = cx.project().clone();
            let chain_name = |n: &str| crate::i18n::tf("padout.chain_name", &[("pad", &n)]);
            padout::chain_add(&view, t, pad, slot, |n| {
                cx.bus_mut().new_strip(chain_name(n))
            })
            .map_err(refusal)?
        }
        (None, Some(e)) => {
            padout::chain_remove(cx.project(), t, pad, EffectId(e)).map_err(refusal)?
        }
        _ => return Err(ToolError::bad("add", "give add or remove, one of them")),
    };
    let pad_name = cx
        .project()
        .track(t)
        .and_then(|x| x.instrument.pulsus())
        .and_then(|k| k.by_id(pad))
        .map(|p| p.name.clone())
        .unwrap_or_default();
    Ok(plan(
        cmd,
        Said::new("ai.tools.padout_chain").name("name", args::clean(&pad_name)),
        json!({"track_id": t.0, "pad": pitch}),
    ))
}

fn make_kit_from_folder(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let asked = std::path::PathBuf::from(a.str("path")?);
    // Only inside the project's folder and the folders the user allowed, resolved on disk;
    // every file the kit reads or names is judged the same way.
    let path = cantus_ai::resolve_touch(&*cx.host, &asked).ok_or_else(|| {
        ToolError::Refused(
            "that path is outside the project's folder and the folders the user allowed".into(),
        )
    })?;
    let onto = args::opt_track(cx.project(), a, "track_id")?;
    if let Some(t) = onto {
        if cx
            .project()
            .track(t)
            .and_then(|x| x.instrument.pulsus())
            .is_none()
        {
            return Err(ToolError::Refused(format!(
                "track {} is not a Pulsus kit: give no track_id for a new kit track",
                t.0
            )));
        }
    }
    let host = &*cx.host;
    let allow = |p: &std::path::Path| cantus_ai::may_touch(host, p);
    let sfz = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("sfz"));
    let (mut kit, log) = if sfz {
        let (zones, outside) =
            crate::padout_ui::sfz_zones_within(&path, &allow).map_err(ToolError::Failed)?;
        let (k, mut log) = cantus_core::pads::import::kit_from_zones(&zones);
        if outside > 0 {
            log.push(format!(
                "{outside} regions name a file outside the allowed folders: left out"
            ));
        }
        (k, log)
    } else if path.is_dir() {
        match crate::padout_ui::kit_files_within(&path, &allow) {
            crate::padout_ui::Scan::Files(files) => {
                cantus_core::pads::import::kit_from_files(&files)
            }
            crate::padout_ui::Scan::TooBig => {
                return Err(ToolError::Refused(
                    "too big to be a kit: more than 512 sounds, or 4 GB".into(),
                ))
            }
        }
    } else {
        return Err(ToolError::bad(
            "path",
            "is neither a folder nor an .sfz file",
        ));
    };
    if kit.pads.is_empty() {
        return Err(ToolError::Refused(
            "it holds no sound a kit can play".into(),
        ));
    }
    // What did not fit is said by file name (the import's log names no folder).
    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let pads: Vec<Value> = kit
        .pads
        .iter()
        .map(|p| json!({"note": p.pitch, "name": p.name, "layers": p.layers.len()}))
        .collect();
    let (cmd, track) = match onto {
        Some(t) => {
            if let Some(old) = cx.project().track(t).and_then(|x| x.instrument.pulsus()) {
                padout::carry_outputs(old, &mut kit);
            }
            // The bus removes what the old kit's pads leave unused (ADR 0100 §9).
            let cmd = Command::SetInstrument {
                track: t,
                spec: Box::new(InstrumentSpec::Pulsus(kit)),
            };
            (cmd, t)
        }
        None => {
            let mut t = cx.bus_mut().new_track(name.clone());
            t.instrument = InstrumentSpec::Pulsus(kit);
            let id = t.id;
            let index = cx.project().tracks.len();
            (
                Command::InsertTrack {
                    index,
                    track: Box::new(t),
                },
                id,
            )
        }
    };
    Ok(plan(
        cmd,
        Said::new("ai.tools.padout_kit").name("name", args::clean(&name)),
        json!({"track_id": track.0, "pads": pads, "not_kept": log}),
    ))
}
