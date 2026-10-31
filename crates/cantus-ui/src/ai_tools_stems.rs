//! Agent tools: stem separation (builder stems, ADR 0085).
//!
//! `separate_stems` is a job: prepared on the interface's thread (the clips, the model —
//! which must be installed already: its licence is the user's to accept, in *Settings ▸
//! Models* — and the folders), separated on a worker through the application's one helper,
//! and ended by an edit planned back on the interface's thread, as *Separate stems…* plans
//! it: new tracks under a group bus, aligned to the sample, the originals muted — one undo
//! step, under the same permission as any edit.

use crate::app::CantusApp;
use crate::clips::ClipRef;
use crate::stems_ui::{self, run, Card, Target};
use cantus_ai::tool::{EditPlan, Job, JobDone, Said, ToolClass, ToolCx, ToolError, ToolRun};
use cantus_ai::ToolDef;
use cantus_core::{ClipId, TrackId};
use serde_json::{json, Value};

pub static TOOLS: &[ToolDef] = &[ToolDef {
    name: "separate_stems",
    title: "Separate stems",
    description: "Separate audio into its parts with a model that runs on this computer: HT-Demucs splits a song into drums, bass, other and vocals (model \"htdemucs\"), or adds guitar and piano (\"htdemucs-6s\", whose piano is weak). Give a track (every audio clip on it) or audio clips, all on one track and in the arrangement (clips on several tracks, or session clips, are refused: separate one track at a time). The parts land as new audio tracks right under the track, on a new mixer bus in a group named after the track, each clip on the same spot as the clip it came from, sample for sample, with its fades, crossfades, comp seams and mute; the part strips and the bus start above 0 dB (+3.01 dB each under the -3 dB pan law) so the parts sum to the original's level under the project's pan law, and changing the pan law later trims them again; the original clips are muted, not deleted — one undo step. Each new file records the model, its version and licence. Quality \"fast\" takes about a third of real time on a laptop processor; \"best\" averages four passes (four times slower, a little cleaner). The model must already be installed (Settings > Models: its licence is the user's to accept); if not, this answers with an error saying so. A long job: it answers with a job handle (wait_job).",
    class: ToolClass::Edit,
    input_schema: schema,
    output_schema: Some(result_schema),
    run: ToolRun::Job(separate_stems),
}];

fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "track_id": {"type": "integer", "minimum": 1, "description": "A track: every audio clip on it (get_project lists them)."},
            "clip_ids": {"type": "array", "items": {"type": "integer", "minimum": 1}, "description": "Audio clips to separate, instead of a track: all on one track, arrangement clips only (not session clips)."},
            "model": {"type": "string", "enum": ["htdemucs", "htdemucs-6s"], "description": "4 parts (default) or 6."},
            "quality": {"type": "string", "enum": ["fast", "best"], "description": "fast (default) or best (four passes averaged)."}
        },
        "additionalProperties": false
    })
}

fn result_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "track_ids": {"type": "array", "items": {"type": "integer"}, "description": "The new tracks, one per part."},
            "bus_strip_id": {"type": "integer", "description": "The mixer strip they feed (it and the part strips start above 0 dB, matching the pan law)."},
            "parts": {"type": "array", "items": {"type": "string"}},
            "muted_clip_ids": {"type": "array", "items": {"type": "integer"}, "description": "The original clips, now muted."}
        },
        "required": ["track_ids", "bus_strip_id", "parts", "muted_clip_ids"]
    })
}

fn bad(name: &str, why: impl Into<String>) -> ToolError {
    ToolError::bad(name, why)
}

/// The clips the arguments name.
fn target(app: &CantusApp, a: &Value) -> Result<Target, ToolError> {
    let p = app.project();
    if let Some(t) = a.get("track_id") {
        let id = TrackId(t.as_u64().ok_or_else(|| bad("track_id", "a track id"))?);
        if p.track(id).is_none() {
            return Err(ToolError::NotFound(format!("track {}", id.0)));
        }
        if a.get("clip_ids").is_some() {
            return Err(bad("clip_ids", "a track or clips, not both"));
        }
        return Ok(Target::Track(id));
    }
    let ids = a
        .get("clip_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("track_id", "give track_id or clip_ids"))?;
    let mut clips: Vec<ClipRef> = Vec::new();
    for v in ids {
        let id = ClipId(v.as_u64().ok_or_else(|| bad("clip_ids", "clip ids"))?);
        // The arrangement's clips only: a session clip has no place in time to lay the
        // parts at (review 2 #14).
        let (t, clip) = p
            .tracks
            .iter()
            .find_map(|t| t.clips.iter().find(|c| c.id == id).map(|c| (t, c)))
            .ok_or_else(|| ToolError::NotFound(format!("arrangement clip {}", id.0)))?;
        if clip.audio.is_none() {
            return Err(bad(
                "clip_ids",
                format!("clip {} is not an audio clip", id.0),
            ));
        }
        clips.push((t.id, id));
    }
    let target = Target::Clips(clips);
    if target.spans_tracks(p) {
        return Err(bad(
            "clip_ids",
            "clips on one track: each track's parts go under a bus of their own",
        ));
    }
    Ok(target)
}

fn separate_stems(cx: &mut ToolCx<'_>, a: &Value) -> Result<Job, ToolError> {
    let app = cx
        .host
        .as_any_mut()
        .downcast_mut::<CantusApp>()
        .ok_or_else(|| ToolError::Failed("this host cannot run models".into()))?;
    let target = target(app, a)?;
    let clips = target.clips(app.project());
    if clips.is_empty() {
        return Err(bad("track_id", "it holds no audio clip"));
    }
    let model_id = a
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("htdemucs")
        .to_string();
    if !stems_ui::view::MODELS.iter().any(|(m, _)| *m == model_id) {
        return Err(bad("model", "htdemucs or htdemucs-6s"));
    }
    if stems_ui::models::removing(app, &model_id) {
        return Err(ToolError::Refused(format!(
            "the model {model_id} is being removed"
        )));
    }
    let Some(model) = stems_ui::models::ready(app, &model_id) else {
        return Err(ToolError::Refused(format!(
            "the model {model_id} is not installed: the user installs it, and accepts its licence, in Settings > Models"
        )));
    };
    prepare_with_model(app, a, model)
}

/// The job with `model` (installed and ready): the clips, the folders, the work and the
/// edit. Public for the tests, which give it the stand-in network.
pub fn prepare_with_model(
    app: &mut CantusApp,
    a: &Value,
    model: cantus_content::models::ReadyModel,
) -> Result<Job, ToolError> {
    let target = target(app, a)?;
    let clips = target.clips(app.project());
    if clips.is_empty() {
        return Err(bad("track_id", "it holds no audio clip"));
    }
    let shifts = match a.get("quality").and_then(Value::as_str) {
        None | Some("fast") => 0,
        Some("best") => stems_ui::BEST_SHIFTS,
        Some(_) => return Err(bad("quality", "fast or best")),
    };
    let said = said(app, &clips, model.sources.len());
    // The folders are chosen here and made on the worker: a job denied, or never run,
    // leaves nothing behind (integration review #2).
    let mut sources = run::plan_sources(app, &clips, &model).map_err(ToolError::Failed)?;
    let device = stems_ui::device(app, &model.id);
    let ml = app.ml.starter();
    // The helper and the models count as in use from here until the job ends — its edit
    // made or not (review 3 #1; one counter for every agent's model job).
    let hold = app.ml.hold_ml_agent();
    let scratch = crate::app::data_dir().join("cache").join("stems");
    run::check_space(&sources, &scratch, model.sources.len(), shifts).map_err(ToolError::Failed)?;
    let words = target.words(app.project());
    let bus_name = target.bus_name(app.project());
    Ok(Job {
        said: said.clone(),
        work: Box::new(move |jcx| {
            run::make_dirs(&mut sources).map_err(ToolError::Failed)?;
            // Removed unless the edit is planned (then they wait to see it made).
            let leftovers = run::Leftovers::new(sources.iter().map(|s| s.out.clone()).collect());
            let done = run::separate_blocking(
                &ml,
                &sources,
                &model,
                shifts,
                device,
                &scratch,
                jcx.progress,
                jcx.cancelled,
            );
            let results = match done {
                Ok(r) => r,
                Err(e) => {
                    drop(leftovers);
                    return Err(if (jcx.cancelled)() {
                        ToolError::Cancelled
                    } else {
                        ToolError::Failed(e)
                    });
                }
            };
            let card = Card {
                id: model.id.clone(),
                name: model.name.clone(),
                version: model.version,
                licence: model.licence.clone(),
            };
            Ok(JobDone::Then(
                json!({"separated": words}),
                Box::new(move |cx: &mut ToolCx<'_>| {
                    let _hold = hold;
                    // The project's audio folder as it is now — a Save As during the job
                    // moved it (integration review #4) — and the project it belongs to.
                    let (base, epoch) = match cx.host.as_any_mut().downcast_mut::<CantusApp>() {
                        Some(app) => (
                            app.audio_base().map(std::path::Path::to_path_buf),
                            Some(app.epoch),
                        ),
                        None => (None, None),
                    };
                    // The clips still there (an edit meanwhile may have removed one).
                    let clips: Vec<ClipRef> = clips
                        .into_iter()
                        .filter(|c| crate::audio_clips::region(cx.project(), *c).is_some())
                        .collect();
                    let part = |p: &str| run::part_name(p);
                    let planned = stems_ui::plan(
                        cx.bus_mut(),
                        &clips,
                        &results,
                        &card,
                        base.as_deref(),
                        &stems_ui::today(),
                        &part,
                        &bus_name,
                    )
                    .map_err(ToolError::Failed)?;
                    let parts: Vec<String> = results
                        .first()
                        .map(|r| r.stems.iter().map(|s| s.part.clone()).collect())
                        .unwrap_or_default();
                    // Planned: the folders wait to see the edit made — the host may make it
                    // now, ask first, or refuse it — and go if it never is.
                    if let (Some(app), Some(epoch)) =
                        (cx.host.as_any_mut().downcast_mut::<CantusApp>(), epoch)
                    {
                        let added = run::inserted_sources(&planned.command);
                        app.stems.pending.push(leftovers.pending(added, epoch));
                    } else {
                        // A host that is not the application: its folders are its own.
                        let _ = leftovers.pending(Vec::new(), 0);
                    }
                    Ok(EditPlan {
                        command: planned.command,
                        said,
                        result: json!({
                            "track_ids": planned.tracks.iter().map(|t| t.0).collect::<Vec<_>>(),
                            "bus_strip_id": planned.bus.0,
                            "parts": parts,
                            "muted_clip_ids": clips.iter().map(|c| c.1 .0).collect::<Vec<_>>(),
                        }),
                    })
                }),
            ))
        }),
    })
}

/// What the card and the history say: "Separate “Song.wav” on “Mix” into 4 stems under a
/// new bus" — the target named in the user's words.
fn said(app: &CantusApp, clips: &[ClipRef], parts: usize) -> Said {
    let p = app.project();
    let track = clips
        .first()
        .and_then(|c| p.track(c.0))
        .map(|t| t.name.clone())
        .unwrap_or_default();
    let mut names: Vec<String> = clips
        .iter()
        .filter_map(|c| crate::audio_clips::region(p, *c))
        .filter_map(|(_, a)| p.audio_source(a.source).map(|s| s.name.clone()))
        .collect();
    names.dedup();
    let said = Said::new("stems.ai.separate");
    let said = match names.as_slice() {
        // The file's name as the user gave it: data, quoted, never words of Cantus's.
        [one] => said.name("what", one.clone()),
        many => said.nested("what", Said::new("stems.ai.files").arg("n", many.len())),
    };
    said.name("track", track).arg("n", parts)
}
