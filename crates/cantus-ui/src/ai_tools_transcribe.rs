//! Agent tools: audio to MIDI (builder transcribe, ADR 0086).
//!
//! `audio_to_midi` converts an audio clip, as *Convert to MIDI…* does: a job (the clip's
//! audio written and the model run off the interface's thread) that ends in one edit — a
//! new instrument track under the clip's with a MIDI clip aligned to it — planned back on
//! the interface's thread and asked for like any edit. The models are never downloaded by
//! an agent: one the user has not installed is refused, with where to install it.

use crate::app::CantusApp;
use crate::transcribe_ui::{self as tr, Grid, Instrument, Settings};
use cantus_ai::{EditPlan, Job, JobDone, Said, ToolClass, ToolCx, ToolDef, ToolError, ToolRun};
use cantus_core::{ClipId, TrackId};
use cantus_ml::client::{Outcome, Update};
use cantus_ml::transcribe::spec::{DrumClass, Mode, TranscribeResult, Transcription};
use serde_json::{json, Value};
use std::time::Duration;

pub static TOOLS: &[ToolDef] = &[ToolDef {
    name: "audio_to_midi",
    title: "Convert audio to MIDI",
    description: "Transcribe an audio clip into notes on a new instrument track under the clip's track, \
in a MIDI clip aligned to the audio clip (one undo step). Modes: \"melody\" (one line, for a voice or a \
solo instrument), \"polyphonic\" (every note, chords included; Spotify's Basic Pitch) and \"drums\" \
(kick, snare, toms, hi-hat, cymbals on General MIDI keys, played by Pulsus). Only the arrangement's \
audio clips are converted: a session clip is refused (it has no place on the timeline for the MIDI clip \
to line up with). Notes keep the timing they \
were played with unless `quantize` is given. Drums are reliable for kick, snare and hi-hat; toms and \
cymbals are written by default only when the drum model is installed. The note model must be installed \
by the user (Convert to MIDI… on an audio clip): this tool never downloads.",
    class: ToolClass::Edit,
    input_schema,
    output_schema: Some(output_schema),
    run: ToolRun::Job(prepare),
}];

fn input_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "track": {"type": "integer", "description": "The audio clip's track id."},
            "clip": {"type": "integer", "description": "The audio clip's id."},
            "mode": {"type": "string", "enum": ["melody", "polyphonic", "drums"], "default": "polyphonic"},
            "sensitivity": {"type": "number", "minimum": 0, "maximum": 1, "default": 0.5,
                "description": "Polyphonic and drums only (melody's tracker decides by itself): 0 keeps only the clearest notes, 1 the faintest; 0.5 is the model's own default."},
            "shortest_ms": {"type": "number", "minimum": 10, "maximum": 2000, "default": 127.7,
                "description": "Notes shorter than this are dropped (melody and polyphonic)."},
            "bends": {"type": "boolean", "default": false,
                "description": "Keep each note's pitch bends as a per-note bend curve."},
            "quantize": {"type": "string", "enum": ["off", "1/8", "1/16", "1/32"], "default": "off"},
            "instrument": {"type": "string", "enum": ["unda", "exemplar"], "default": "unda"},
            "separate_first": {"type": "boolean", "default": false,
                "description": "Drums only: take the drums out of a mix with HT-Demucs first (when the user installed it); much better on full mixes."},
            "classes": {"type": "array", "items": {"type": "string",
                "enum": ["kick", "snare", "toms", "hihat", "cymbals"]},
                "description": "Drums only: the classes written. Default: kick, snare and hi-hat, and toms and cymbals when the drum model is installed."}
        },
        "required": ["track", "clip"],
        "additionalProperties": false
    })
}

fn output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "track": {"type": "integer", "description": "The new track's id."},
            "clip": {"type": "integer", "description": "The new MIDI clip's id."},
            "notes": {"type": "integer"},
            "mode": {"type": "string"},
            "seconds": {"type": "number", "description": "How long the model took."}
        },
        "required": ["track", "clip", "notes", "mode"]
    })
}

fn class_name(c: DrumClass) -> &'static str {
    match c {
        DrumClass::Kick => "kick",
        DrumClass::Snare => "snare",
        DrumClass::Toms => "toms",
        DrumClass::HiHat => "hihat",
        DrumClass::Cymbals => "cymbals",
    }
}

/// The arguments as settings (`drum_model`: whether the drum model is ready).
pub fn settings(args: &Value, drum_model: bool) -> Result<Settings, ToolError> {
    let mut s = Settings::default();
    if let Some(m) = args.get("mode") {
        let m = m
            .as_str()
            .ok_or_else(|| ToolError::bad("mode", "a string"))?;
        s.mode = tr::parse_mode(m)
            .ok_or_else(|| ToolError::bad("mode", "melody, polyphonic or drums"))?;
    }
    if let Some(v) = args.get("sensitivity") {
        let v = v.as_f64().filter(|v| (0.0..=1.0).contains(v));
        s.sensitivity = v.ok_or_else(|| ToolError::bad("sensitivity", "0 to 1"))? as f32;
    }
    if let Some(v) = args.get("shortest_ms") {
        let v = v.as_f64().filter(|v| (10.0..=2000.0).contains(v));
        s.shortest_ms = v.ok_or_else(|| ToolError::bad("shortest_ms", "10 to 2000"))? as f32;
    }
    if let Some(v) = args.get("bends") {
        s.bends = v
            .as_bool()
            .ok_or_else(|| ToolError::bad("bends", "true or false"))?;
    }
    if let Some(v) = args.get("quantize") {
        s.quantize = v
            .as_str()
            .and_then(Grid::parse)
            .ok_or_else(|| ToolError::bad("quantize", "off, 1/8, 1/16 or 1/32"))?;
    }
    if let Some(v) = args.get("instrument") {
        s.instrument = match v.as_str() {
            Some("unda") => Instrument::Unda,
            Some("exemplar") => Instrument::Exemplar,
            _ => return Err(ToolError::bad("instrument", "unda or exemplar")),
        };
    }
    if let Some(v) = args.get("separate_first") {
        s.separate = v
            .as_bool()
            .ok_or_else(|| ToolError::bad("separate_first", "true or false"))?;
    }
    s.classes = tr::default_classes(drum_model);
    if let Some(v) = args.get("classes") {
        let list = v
            .as_array()
            .ok_or_else(|| ToolError::bad("classes", "a list"))?;
        s.classes = [false; 5];
        for c in list {
            let name = c.as_str().unwrap_or_default();
            let class = DrumClass::ALL
                .into_iter()
                .find(|d| class_name(*d) == name)
                .ok_or_else(|| ToolError::bad("classes", format!("{name:?} is not a class")))?;
            s.classes[tr::class_index(class)] = true;
        }
        if !s.classes.iter().any(|c| *c) {
            return Err(ToolError::bad("classes", "at least one"));
        }
    }
    Ok(s)
}

fn id(args: &Value, name: &str) -> Result<u64, ToolError> {
    args.get(name)
        .and_then(Value::as_u64)
        .ok_or_else(|| ToolError::bad(name, "an id (integer)"))
}

fn prepare(cx: &mut ToolCx<'_>, args: &Value) -> Result<Job, ToolError> {
    let (track, clip) = (TrackId(id(args, "track")?), ClipId(id(args, "clip")?));
    let app = cx
        .host
        .as_any_mut()
        .downcast_mut::<CantusApp>()
        .ok_or_else(|| ToolError::Failed("audio_to_midi needs the application".into()))?;
    // The arrangement's clips only (`Track::clip` finds session clips too): a session clip
    // has no place on the timeline for the MIDI clip to line up with.
    let t = app.project().track(track);
    if t.is_some_and(|t| t.clips.iter().all(|c| c.id != clip) && t.clip(clip).is_some()) {
        return Err(ToolError::Refused(format!(
            "clip {} on track {} is a session clip: audio_to_midi converts the arrangement's \
             clips, whose place on the timeline the MIDI clip lines up with",
            clip.0, track.0
        )));
    }
    if t.and_then(|t| t.clips.iter().find(|c| c.id == clip))
        .is_none()
    {
        return Err(ToolError::NotFound(format!(
            "clip {} on track {}",
            clip.0, track.0
        )));
    }
    let drum_model = app.ml().models().ready(tr::DRUM_MODEL).is_some();
    let s = settings(args, drum_model)?;
    let src = tr::source(app, track, clip).map_err(ToolError::Refused)?;
    if let Some(m) = tr::run::needed(s.mode) {
        if app.ml().models().ready(m).is_none() {
            return Err(ToolError::Refused(
                "the note model (Basic Pitch) is not installed: the user installs it from \
                 Convert to MIDI… on an audio clip"
                    .into(),
            ));
        }
    }
    tr::sweep();
    let dir = tr::WorkDir::new("agent-").map_err(ToolError::Failed)?;
    let job_args = tr::job_args(
        app.ml().models(),
        &s,
        dir.join("input.wav"),
        dir.join("notes.json"),
    )
    .map_err(ToolError::Failed)?;
    let sep = if s.mode == Mode::Drums && s.separate {
        let a = tr::separate_args(app.ml().models(), job_args.input.clone(), dir.join("stems"));
        Some(a.ok_or_else(|| {
            ToolError::Refused(
                "HT-Demucs is not installed: the user installs it with Separate stems… on an audio clip"
                    .into(),
            )
        })?)
    } else {
        None
    };
    let starter = app.ml().starter();
    let said = said(&src, s.mode);
    // The helper counts as in use from here until the job ends — between its helper jobs
    // too (reading the take, separating then transcribing): a model is not removed under
    // it (round 34's integrated review #1). The counter is the service's, every agent tool's.
    let held = app.ml().hold_ml_agent(); // r34:stems — the shared ml_agents counter
    Ok(Job {
        said: said.clone(),
        work: Box::new(move |jcx| {
            let r = work(jcx, &src, &s, job_args, sep, &starter, said);
            drop(dir);
            drop(held);
            r
        }),
    })
}

/// The worker's part: the clip's audio written, the helper's job run and followed, the
/// edit planned back on the interface's thread.
fn work(
    jcx: &cantus_ai::JobCx<'_>,
    src: &tr::Source,
    s: &Settings,
    mut args: cantus_ml::transcribe::spec::TranscribeArgs,
    sep: Option<cantus_ml::spec::SeparateArgs>,
    starter: &crate::ml_service::MlStarter,
    said: Said,
) -> Result<JobDone, ToolError> {
    (jcx.progress)(0.02);
    if s.mode == Mode::Melody {
        // Cantus's own tracker, here: no model, no helper.
        let t = tr::melody(src, s, jcx.cancelled).map_err(ToolError::Failed)?;
        return Ok(then(src, s, t, 0.0, said));
    }
    tr::write_input(src, tr::input_rate(s.mode), &args.input, jcx.cancelled)
        .map_err(ToolError::Failed)?;
    if (jcx.cancelled)() {
        return Err(ToolError::Cancelled);
    }
    let mut span = (0.05, 0.95);
    if let Some(a) = sep {
        // The drums out of the mix first; then the drum stem is what is transcribed.
        let o = follow(
            jcx,
            starter,
            "separate",
            serde_json::to_value(&a).unwrap_or_default(),
            (0.05, 0.6),
        )?;
        let r: cantus_ml::spec::SeparateResult = match o {
            Outcome::Done(v) => {
                serde_json::from_value(v).map_err(|e| ToolError::Failed(e.to_string()))?
            }
            Outcome::Cancelled => return Err(ToolError::Cancelled),
            Outcome::Failed(f) => return Err(ToolError::Failed(f.to_string())),
        };
        args.input = r
            .stems
            .into_iter()
            .find(|f| f.name == "drums")
            .map(|f| f.path)
            .ok_or_else(|| ToolError::Failed("the separation gave no drums".into()))?;
        span = (0.6, 0.95);
    }
    let outcome = follow(
        jcx,
        starter,
        "transcribe",
        serde_json::to_value(&args).unwrap_or_default(),
        span,
    )?;
    let res: TranscribeResult = match outcome {
        Outcome::Done(v) => {
            serde_json::from_value(v).map_err(|e| ToolError::Failed(e.to_string()))?
        }
        Outcome::Cancelled => return Err(ToolError::Cancelled),
        Outcome::Failed(f) => return Err(ToolError::Failed(f.to_string())),
    };
    let t: Transcription = std::fs::read(&res.file)
        .map_err(|e| e.to_string())
        .and_then(|b| serde_json::from_slice(&b).map_err(|e| e.to_string()))
        .map_err(ToolError::Failed)?;
    Ok(then(src, s, t, res.seconds, said))
}

/// Run helper job `name` and follow it to its end, its progress mapped into `span`.
fn follow(
    jcx: &cantus_ai::JobCx<'_>,
    starter: &crate::ml_service::MlStarter,
    name: &str,
    args: Value,
    span: (f32, f32),
) -> Result<Outcome, ToolError> {
    let job = starter.start(name, args);
    loop {
        if (jcx.cancelled)() {
            starter.cancel(job.id);
        }
        match job.updates.recv_timeout(Duration::from_millis(100)) {
            Ok(Update::Progress { done, .. }) => (jcx.progress)(span.0 + (span.1 - span.0) * done),
            Ok(Update::Ended(_, o)) => {
                starter.forget(job.id);
                return Ok(o);
            }
            Ok(_) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err(ToolError::Failed(
                    "the model's helper stopped answering".into(),
                ))
            }
        }
    }
}

/// The job's end: what was found, and the edit planned back on the interface's thread.
fn then(src: &tr::Source, s: &Settings, t: Transcription, seconds: f64, said: Said) -> JobDone {
    let found = json!({
        "mode": tr::mode_name(s.mode),
        "found": t.notes.len() + t.hits.len(),
        "seconds": seconds,
    });
    let (src, s) = (src.clone(), s.clone());
    JobDone::Then(
        found,
        Box::new(move |cx: &mut ToolCx<'_>| {
            let p = tr::plan(cx.bus_mut(), &src, &s, &t).map_err(ToolError::Failed)?;
            if p.notes == 0 {
                return Err(ToolError::Failed("no notes were found in the clip".into()));
            }
            Ok(EditPlan {
                command: p.command,
                said,
                result: json!({
                    "track": p.track.0,
                    "clip": p.clip.0,
                    "notes": p.notes,
                    "mode": tr::mode_name(s.mode),
                    "seconds": seconds,
                }),
            })
        }),
    )
}

/// What the confirmation, the history and the activity view say: the clip and its track
/// by name, and the mode — "Convert “Vox.wav” on “Lead” to MIDI (melody) on a new track".
pub fn said(src: &tr::Source, mode: Mode) -> Said {
    Said::new("ai.transcribe.said")
        .name("clip", &src.clip_name)
        .name("track", &src.track_name)
        .nested(
            "mode",
            Said::new(match mode {
                Mode::Melody => "transcribe.mode.melody.word",
                Mode::Polyphonic => "transcribe.mode.poly.word",
                Mode::Drums => "transcribe.mode.drums.word",
            }),
        )
}
