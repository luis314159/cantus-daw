//! Agent tools: Sectio, Exemplar's slicer (builder piano, round 37, ADR 0099) —
//! `slice_sample` (a loop cut into an Exemplar, one slice per key from C1) and
//! `dump_slices` (the slices as a clip that replays the loop). Each one undo step.

use crate::sectio_ui;
use cantus_ai::tool::{EditPlan, Said, ToolClass, ToolCx, ToolError, ToolRun};
use cantus_ai::ToolDef;
use cantus_core::sectio::{SliceMode, Slicing};
use cantus_core::{Command, InstrumentKind, InstrumentSpec, Ticks, TrackId};
use serde_json::{json, Value};
use std::path::PathBuf;

pub static TOOLS: &[ToolDef] = &[
    ToolDef {
        name: "slice_sample",
        title: "Slice a loop",
        description: "Cut an audio loop (WAV, AIFF, FLAC, OGG) inside the project's folder or a folder the user allowed into slices played by Exemplar, one slice per key from C1 (MIDI 36) up, each at its own pitch: at the file's own markers (WAV cue points; an ACID file's beats also give its tempo), else at its transients (sensitivity 0..1: higher keeps fainter hits), on a beat grid (grid_steps per beat at the loop's tempo), or as one slice to cut by hand. With track_id the track's instrument becomes the sliced Exemplar (its clips stay); without, a new track is made. When the tempo is known the points are put where the dump replays the file exactly. At most 64 slices. One undo step. Answers the track, the tempo and every slice's start (frames and seconds).",
        class: ToolClass::Files,
        input_schema: slice_schema,
        output_schema: None,
        run: ToolRun::Edit(slice_sample),
    },
    ToolDef {
        name: "dump_slices",
        title: "Dump the slices",
        description: "Write a sliced Exemplar's slices to a new clip on its track: slice i on key C1+i at the tick its start falls on at the loop's tempo, until the next — played at the loop's tempo it replays the file sample for sample; at another tempo the slices follow the grid. at_bar (1-based, default 1) places the clip. One undo step. Answers the clip and its notes.",
        class: ToolClass::Edit,
        input_schema: dump_schema,
        output_schema: None,
        run: ToolRun::Edit(dump_slices),
    },
];

fn slice_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "file": {"type": "string", "description": "The loop's path on this computer."},
            "track_id": {"type": "integer", "minimum": 1, "description": "The track whose instrument becomes the sliced Exemplar; omitted, a new track."},
            "mode": {"type": "string", "enum": ["markers", "transients", "grid", "manual"], "description": "markers (default: the file's own, else transients), transients, grid or manual (one slice)."},
            "sensitivity": {"type": "number", "minimum": 0, "maximum": 1, "description": "Transients: 0 keeps the strongest only, 1 the faintest too (default 0.5)."},
            "grid_steps": {"type": "integer", "enum": [1, 2, 4, 8], "description": "Grid: slices per beat (default 4: sixteenths)."}
        },
        "required": ["file"],
        "additionalProperties": false
    })
}

fn dump_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "track_id": {"type": "integer", "minimum": 1, "description": "A track whose instrument is a sliced Exemplar."},
            "at_bar": {"type": "integer", "minimum": 1, "description": "Where the clip starts (1-based bar, default 1)."}
        },
        "required": ["track_id"],
        "additionalProperties": false
    })
}

fn bad(name: &str, why: impl Into<String>) -> ToolError {
    ToolError::bad(name, why)
}

fn slices_json(s: &Slicing) -> Value {
    let rate = s.rate.max(1) as f64;
    json!(s
        .points
        .iter()
        .enumerate()
        .map(|(i, p)| json!({"slice": i, "key": 36 + i, "frame": p, "seconds": *p as f64 / rate}))
        .collect::<Vec<_>>())
}

fn slice_sample(cx: &mut ToolCx<'_>, a: &Value) -> Result<EditPlan, ToolError> {
    let file = a
        .get("file")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("file", "a path"))?;
    // Only inside the project's folder and the folders the user allowed, resolved on disk
    // (a relative path, `..` or a link never steps out); the resolved path is what is read
    // and what the zones keep.
    let path = cantus_ai::resolve_touch(&*cx.host, &PathBuf::from(file)).ok_or_else(|| {
        ToolError::Refused(
            "that path is outside the project's folder and the folders the user allowed".into(),
        )
    })?;
    if !path.is_file() {
        return Err(bad("file", "no such file"));
    }
    let file = &*path.to_string_lossy().into_owned();
    let mode = match a.get("mode").and_then(Value::as_str).unwrap_or("markers") {
        "markers" => SliceMode::Markers,
        "transients" => SliceMode::Transients,
        "grid" => SliceMode::Grid,
        "manual" => SliceMode::Manual,
        _ => return Err(bad("mode", "markers, transients, grid or manual")),
    };
    let sens = a.get("sensitivity").and_then(Value::as_f64).unwrap_or(0.5) as f32;
    let grid = a.get("grid_steps").and_then(Value::as_u64).unwrap_or(4) as u8;
    let tempo = cx.project().tempo_bpm;
    // An agent reads English whatever the window's language (its refusals included).
    let _en = crate::i18n::scoped_thread_locale(crate::i18n::Locale::En);
    let s = sectio_ui::analyse_full(&path, mode, sens, grid.clamp(1, 8), tempo)
        .map(|a| a.slicing)
        .map_err(|e| ToolError::Failed(format!("the loop could not be sliced: {e}")))?;
    let name = path
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Sliced loop".into());
    let (command, track) = match a.get("track_id").and_then(Value::as_u64) {
        Some(id) => {
            let track = TrackId(id);
            let base = cx
                .project()
                .track(track)
                .ok_or_else(|| bad("track_id", "no such track"))?
                .instrument
                .native()
                .filter(|n| n.kind == InstrumentKind::Sampler)
                .cloned();
            let spec = sectio_ui::spec_of(file, &s, base.as_ref());
            (
                Command::SetInstrument {
                    track,
                    spec: Box::new(InstrumentSpec::Native(spec)),
                },
                track,
            )
        }
        None => {
            let mut tr = cx.bus_mut().new_track(name.clone());
            tr.instrument = InstrumentSpec::Native(sectio_ui::spec_of(file, &s, None));
            let id = tr.id;
            let index = cx.project().tracks.len();
            (
                Command::InsertTrack {
                    index,
                    track: Box::new(tr),
                },
                id,
            )
        }
    };
    Ok(EditPlan {
        command: Command::Batch {
            label_key: "cmd.slice_sample".into(),
            commands: vec![command],
        },
        said: Said::new("slicer.tool.slice").name("file", name),
        result: json!({
            "track_id": track.0,
            "mode": match s.mode { SliceMode::Markers => "markers", SliceMode::Transients => "transients", SliceMode::Grid => "grid", SliceMode::Manual => "manual" },
            "bpm": s.bpm,
            "slices": slices_json(&s),
        }),
    })
}

fn dump_slices(cx: &mut ToolCx<'_>, a: &Value) -> Result<EditPlan, ToolError> {
    let track = TrackId(
        a.get("track_id")
            .and_then(Value::as_u64)
            .ok_or_else(|| bad("track_id", "a track"))?,
    );
    let tr = cx
        .project()
        .track(track)
        .ok_or_else(|| bad("track_id", "no such track"))?;
    let s = tr
        .instrument
        .native()
        .and_then(Slicing::of)
        .ok_or_else(|| {
            bad(
                "track_id",
                "its instrument is not a sliced Exemplar (slice_sample first)",
            )
        })?;
    let bar = a
        .get("at_bar")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .clamp(1, 100_000) as i64;
    let at = Ticks(cx.project().time_signature.ticks_per_bar().0 * (bar - 1));
    let notes = sectio_ui::dump_notes_at(&s, cx.project().tempo_bpm);
    if notes.is_empty() {
        return Err(ToolError::Refused(
            "the loop has no tempo and the song none to lend it: nothing to dump".into(),
        ));
    }
    let end = notes.iter().map(|n| n.0 + n.1).max().unwrap_or(0).max(1);
    let mut clip = cx
        .bus_mut()
        .new_clip(crate::i18n::t("slicer.dump_name"), at, Ticks(end));
    for (tick, len, key, vel) in &notes {
        let n = cx.bus_mut().new_note(Ticks(*tick), Ticks(*len), *key, *vel);
        clip.notes.push(n);
    }
    let id = clip.id;
    Ok(EditPlan {
        command: Command::Batch {
            label_key: "cmd.slice_dump".into(),
            commands: vec![Command::InsertClip {
                track,
                clip: Box::new(clip),
            }],
        },
        said: Said::new("slicer.tool.dump").arg("n", notes.len().to_string()),
        result: json!({
            "clip_id": id.0,
            "notes": notes.iter().map(|n| json!({"tick": n.0, "length": n.1, "key": n.2})).collect::<Vec<_>>(),
        }),
    })
}
