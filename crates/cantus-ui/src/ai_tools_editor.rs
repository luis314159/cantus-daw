//! The audio editor for agents (round 36, ADR 0096): `edit_audio`, `denoise_clip` and
//! `remove_unused_audio`. The edits run as jobs — the take read, edited and written off the
//! interface thread, through the same functions the window runs — and end in one undo step
//! planned back on the interface thread: a new content-addressed file, the clip pointed at
//! it, the original untouched. A file written for an edit that is never made (a declined
//! card, another project meanwhile) is removed. Positions are seconds in the clip's own file.

use crate::app::CantusApp;
use crate::editor::{self, commit, jobs};
use cantus_ai::tools_project::args::{self, Res, A};
use cantus_ai::{EditPlan, Job, JobDone, Said, ToolClass, ToolCx, ToolDef, ToolError, ToolRun};
use cantus_core::audio::FadeCurve;
use cantus_core::audio_edit::{silence, AudioBuf, Edit, Joins, Level, Sel};
use cantus_core::{AudioSourceId, ClipId, Command, TrackId};
use cantus_dsp::denoise::Params;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

#[rustfmt::skip]
pub static TOOLS: &[ToolDef] = &[
    ToolDef { name: "edit_audio", title: "Edit an audio clip's take", description: "Destructive edits on an audio clip's file, as the audio editor does: delete or trim a range, silence it, fade in or out, gain, normalize (peak, rms or lufs, with a peak ceiling for rms and lufs), invert, reverse, remove DC, remove silence, insert silence, find and repair clicks (declick), rebuild a short range from the audio around it (repair, 20 ms at most), or turn a time-frequency region (start/end seconds and lo_hz/hi_hz) up or down. Joins are made at zero crossings with a 3 ms crossfade. The result is a NEW file (the original stays) the clip then plays: one undo step. Positions are seconds in the clip's own file, not the timeline; no range means the whole take for gain, normalize, invert, reverse, dc_remove, remove_silence, declick and the fades. The clip's pitch edits and warp markers are kept by gain-like edits (gain, normalize, invert, dc_remove, fades, silence, attenuate, declick, repair) and dropped by reverse, paste and every edit that changes the length (delete, trim, remove_silence, insert_silence). Runs in the background (a job). Takes up to 30 minutes.", class: ToolClass::Edit, input_schema: edit_schema, output_schema: None, run: ToolRun::Job(edit_audio) },
    ToolDef { name: "denoise_clip", title: "Reduce noise in an audio clip", description: "Noise reduction by profile, as the audio editor does: learn what the noise sounds like from a noise-only stretch (noise_start_seconds..noise_end_seconds of the clip's file, at least 0.1 s), then subtract it with spectral subtraction (a range, or the whole take; a range meets the rest through 10 ms crossfades). amount 0 changes nothing (refused as such); 1 is the whole reduction (reduction_db at most). The result is a NEW file the clip plays: one undo step, the original kept, the clip's pitch edits and warp markers kept. Runs in the background (a job).", class: ToolClass::Edit, input_schema: denoise_schema, output_schema: None, run: ToolRun::Job(denoise_clip) },
    ToolDef { name: "remove_unused_audio", title: "Remove unused audio", description: "Remove the project's audio files no clip, comp or session clip plays from the pool (one undo step), and delete from the project's audio folder the audio editor's files (edit-*.wav) that nothing refers to: not the project, not its undo history, not the project as saved. Files of the entries this removes stay until the undo history no longer holds them (run it again after reopening the project). Only the project's own editor folder is cleaned, never a file another project in the folder refers to, and never while an edit is being made. Always asked first, with the sizes. Answers what it removed.", class: ToolClass::Destructive, input_schema: empty, output_schema: None, run: ToolRun::HostSaid { run: remove_unused_audio, said: clean_said } },
];

const OPS: &[&str] = &[
    "delete",
    "trim",
    "silence",
    "insert_silence",
    "fade_in",
    "fade_out",
    "gain",
    "normalize",
    "invert",
    "reverse",
    "dc_remove",
    "remove_silence",
    "attenuate",
    "declick",
    "repair",
];

fn empty() -> Value {
    args::object(json!({}), &[])
}

fn edit_schema() -> Value {
    args::object(
        json!({
            "clip": args::s_id("The audio clip"),
            "operation": {"enum": OPS, "description": "What to do."},
            "start_seconds": {"type": "number", "minimum": 0, "description": "The range's start, seconds into the clip's file."},
            "end_seconds": {"type": "number", "minimum": 0, "description": "The range's end, seconds into the file."},
            "gain_db": {"type": "number", "minimum": -96, "maximum": 96, "description": "gain, and attenuate (negative turns the region down)."},
            "level": {"enum": ["peak", "rms", "lufs"], "default": "peak", "description": "normalize: what is measured."},
            "target_db": {"type": "number", "minimum": -96, "maximum": 24, "default": -1, "description": "normalize: dBFS (peak, rms) or LUFS."},
            "ceiling_db": {"type": "number", "minimum": -24, "maximum": 0, "description": "normalize rms or lufs: the gain stops where the peak reaches this (dBFS). Without it, peaks may pass 0 dBFS (the answer says so)."},
            "curve": {"enum": ["linear", "equal_power", "exponential", "logarithmic", "s_curve"], "default": "linear", "description": "fade_in, fade_out."},
            "threshold_db": {"type": "number", "minimum": -120, "maximum": 0, "default": -50, "description": "remove_silence: below this a block is silent."},
            "min_ms": {"type": "number", "minimum": 10, "maximum": 60000, "default": 300, "description": "remove_silence: shortest quiet stretch cut."},
            "keep_ms": {"type": "number", "minimum": 0, "maximum": 10000, "default": 80, "description": "remove_silence: quiet left in each cut."},
            "lo_hz": {"type": "number", "minimum": 0, "description": "attenuate: the region's lowest frequency."},
            "hi_hz": {"type": "number", "minimum": 0, "description": "attenuate: its highest (above lo_hz)."},
            "seconds": {"type": "number", "exclusiveMinimum": 0, "maximum": 600, "description": "insert_silence: how long."},
            "sensitivity": {"type": "number", "minimum": 0, "maximum": 1, "default": 0.5, "description": "declick: higher finds fainter clicks."}
        }),
        &["clip", "operation"],
    )
}

fn denoise_schema() -> Value {
    args::object(
        json!({
            "clip": args::s_id("The audio clip"),
            "noise_start_seconds": {"type": "number", "minimum": 0, "description": "Where a noise-only stretch starts, seconds into the file."},
            "noise_end_seconds": {"type": "number", "minimum": 0, "description": "Where it ends (at least 0.1 s later)."},
            "amount": {"type": "number", "minimum": 0, "maximum": 1, "default": 1},
            "reduction_db": {"type": "number", "minimum": 0, "maximum": 80, "default": 24, "description": "The most any frequency is turned down."},
            "sensitivity": {"type": "number", "minimum": 0.5, "maximum": 8, "default": 3, "description": "How many times the noise's mean is subtracted."},
            "smoothing": {"type": "number", "minimum": 0, "maximum": 1, "default": 0.5},
            "start_seconds": {"type": "number", "minimum": 0, "description": "Clean only from here (default the whole take)."},
            "end_seconds": {"type": "number", "minimum": 0}
        }),
        &["clip", "noise_start_seconds", "noise_end_seconds"],
    )
}

fn curve(name: &str) -> FadeCurve {
    FadeCurve::ALL
        .into_iter()
        .find(|c| c.id() == name)
        .unwrap_or_default()
}

/// What a job needs of the clip, read on the interface thread.
/// Counts an agent's edit as being made from when it is planned until its edit is planned
/// back (or it fails): the editor tidies and sweeps nothing meanwhile.
struct JobGuard(Arc<std::sync::atomic::AtomicUsize>);

impl JobGuard {
    fn new(n: &Arc<std::sync::atomic::AtomicUsize>) -> Self {
        n.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        JobGuard(n.clone())
    }
}

impl Drop for JobGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

struct Target {
    _guard: JobGuard,
    track: TrackId,
    clip: ClipId,
    source: AudioSourceId,
    name: String,
    path: PathBuf,
    rate: f64,
    frames: usize,
    dir: PathBuf,
    epoch: u64,
}

fn target(cx: &mut ToolCx<'_>, a: A<'_>) -> Res<Target> {
    let (track, clip) = {
        let (t, c) = args::clip(cx.project(), a, "clip")?;
        (t, c.clone())
    };
    let region = clip
        .audio
        .as_ref()
        .ok_or_else(|| ToolError::bad("clip", "that clip is not an audio clip"))?;
    if region.ara.is_some() {
        return Err(ToolError::Refused(
            "an ARA plug-in edits that clip: unbind the ARA plug-in first".into(),
        ));
    }
    let app = crate::ai_tools::project_app::app_mut(cx)?;
    let src = app
        .project()
        .audio_source(region.source)
        .cloned()
        .ok_or_else(|| ToolError::Failed("the clip's file is not in the project".into()))?;
    if src.frames as f64 / src.sample_rate.max(1) as f64 > editor::MAX_SECONDS {
        return Err(ToolError::Refused(
            "the take is longer than 30 minutes: edit it in the window".into(),
        ));
    }
    let guard = JobGuard::new(&app.editor.agent_jobs);
    Ok(Target {
        track,
        _guard: guard,
        clip: clip.id,
        source: region.source,
        name: clip.name.clone(),
        path: app.audio_file(&src),
        rate: src.sample_rate.max(1) as f64,
        frames: src.frames as usize,
        dir: commit::edit_dir(app),
        epoch: app.epoch,
    })
}

/// A range in seconds as frames (`None`: the whole take, when none is given and `whole`).
fn range(a: A<'_>, rate: f64, frames: usize, whole: bool) -> Res<Option<Sel>> {
    let at = |s: f64| ((s * rate).round().max(0.0) as usize).min(frames);
    match (a.opt_f64("start_seconds")?, a.opt_f64("end_seconds")?) {
        (None, None) if whole => Ok(None),
        (None, None) => Err(ToolError::bad(
            "start_seconds",
            "this operation needs a range (start_seconds and end_seconds)",
        )),
        (s, e) => {
            let (s, e) = (at(s.unwrap_or(0.0)), at(e.unwrap_or(frames as f64 / rate)));
            if e <= s {
                return Err(ToolError::bad("end_seconds", "must be after start_seconds"));
            }
            Ok(Some(Sel::new(s, e)))
        }
    }
}

type MakeEdit = Box<dyn FnOnce(&AudioBuf) -> Res<Edit> + Send>;

/// The job: read, edit (`make` turns the take into the edit, on the job's thread), write;
/// then the edit planned back on the interface thread.
fn job(t: Target, sel: Option<Sel>, said: Said, make: MakeEdit) -> Res<Job> {
    Ok(Job {
        said: said.clone(),
        work: Box::new(move |jcx| {
            let buf = Arc::new(jobs::read(&t.path).map_err(ToolError::Failed)?);
            if (jcx.cancelled)() {
                return Err(ToolError::Cancelled);
            }
            let sel = sel.unwrap_or(Sel::whole(buf.frames()));
            let edit = make(&buf)?;
            let applied = jobs::apply(buf, sel, &edit, &Joins::default(), &t.dir).map_err(|e| {
                if e == jobs::UNCHANGED {
                    ToolError::Refused("that edit would change nothing".into())
                } else {
                    ToolError::bad("operation", e)
                }
            })?;
            let answer = json!({
                "frames": applied.buf.frames(),
                "seconds": applied.buf.frames() as f64 / applied.buf.rate as f64,
                "changed_seconds": applied.edited.delta_frames as f64 / applied.buf.rate as f64,
                "note": applied.edited.note.map(|n| format!("{n:?}")),
            });
            Ok(JobDone::Then(
                answer,
                Box::new(move |cx: &mut ToolCx<'_>| plan_back(cx, t, applied, said)),
            ))
        }),
    })
}

/// On the interface thread: the edit's commands, its file waiting to see it made.
fn plan_back(cx: &mut ToolCx<'_>, t: Target, applied: jobs::Applied, said: Said) -> Res<EditPlan> {
    let app: &mut CantusApp = crate::ai_tools::project_app::app_mut(cx)?;
    if app.epoch != t.epoch {
        commit::discard(app, &applied.file, applied.fresh);
        return Err(ToolError::Refused(
            "another project was opened while the edit was made".into(),
        ));
    }
    let (commands, dropped) =
        match commit::commit_commands(app, t.track, t.clip, t.source, &applied) {
            Ok(c) => c,
            Err(e) => {
                commit::discard(app, &applied.file, applied.fresh);
                return Err(ToolError::Refused(e));
            }
        };
    let inserted = commands.iter().find_map(|c| match c {
        Command::InsertAudioSource { source } => Some(source.id),
        _ => None,
    });
    app.editor.pending.push(commit::Pending {
        file: applied.file.clone(),
        source: inserted,
        fresh: applied.fresh,
        epoch: t.epoch,
    });
    Ok(EditPlan {
        command: Command::Batch {
            label_key: format!("cmd.editor.{}", applied.name),
            commands,
        },
        said,
        result: json!({
            "file": applied.file.display().to_string(),
            "sha256": applied.sha256,
            "frames": applied.buf.frames(),
            "seconds": applied.buf.frames() as f64 / applied.buf.rate as f64,
            "changed_frames": applied.edited.delta_frames,
            "pitch_and_warp_dropped": dropped,
        }),
    })
}

fn said(what: &str, clip: &str) -> Said {
    Said::new("editor.tool.said")
        .arg("what", crate::i18n::t(&format!("editor.op.{what}")))
        .name("clip", clip.to_string())
}

fn edit_audio(cx: &mut ToolCx<'_>, v: &Value) -> Res<Job> {
    let a = A(v);
    let op = a
        .opt_choice("operation", OPS)?
        .ok_or_else(|| ToolError::bad("operation", "is required"))?;
    let t = target(cx, a)?;
    let whole_ok = !matches!(
        op,
        "delete" | "trim" | "silence" | "attenuate" | "insert_silence" | "repair"
    );
    let sel = if op == "insert_silence" {
        let at = (a.opt_f64("start_seconds")?.unwrap_or(0.0) * t.rate)
            .round()
            .max(0.0) as usize;
        Some(Sel::new(at.min(t.frames), at.min(t.frames)))
    } else {
        range(a, t.rate, t.frames, whole_ok)?
    };
    let db = a.opt_in("gain_db", -96.0, 96.0)?.map(|v| v as f32);
    let edit = match op {
        "delete" => Edit::Delete,
        "trim" => Edit::Trim,
        "silence" => Edit::Silence,
        "insert_silence" => {
            let s = a
                .opt_f64("seconds")?
                .ok_or_else(|| ToolError::bad("seconds", "is required for insert_silence"))?;
            if !(s > 0.0 && s <= 600.0) {
                return Err(ToolError::bad("seconds", "more than 0 and at most 600"));
            }
            Edit::InsertSilence {
                frames: (s * t.rate).round().max(1.0) as usize,
            }
        }
        "fade_in" => Edit::FadeIn {
            curve: curve(a.opt_str("curve")?.unwrap_or("linear")),
        },
        "fade_out" => Edit::FadeOut {
            curve: curve(a.opt_str("curve")?.unwrap_or("linear")),
        },
        "gain" => Edit::Gain {
            db: db.ok_or_else(|| ToolError::bad("gain_db", "is required for gain"))?,
        },
        "normalize" => Edit::Normalize {
            level: match a
                .opt_choice("level", &["peak", "rms", "lufs"])?
                .unwrap_or("peak")
            {
                "rms" => Level::Rms,
                "lufs" => Level::Lufs,
                _ => Level::Peak,
            },
            target_db: a.opt_in("target_db", -96.0, 24.0)?.unwrap_or(-1.0) as f32,
            ceiling_db: a.opt_in("ceiling_db", -24.0, 0.0)?.map(|v| v as f32),
        },
        "invert" => Edit::Invert,
        "reverse" => Edit::Reverse,
        "dc_remove" => Edit::DcRemove,
        "remove_silence" => {
            let d = silence::Settings::default();
            Edit::RemoveSilence(silence::Settings {
                threshold_db: a
                    .opt_in("threshold_db", -120.0, 0.0)?
                    .map_or(d.threshold_db, |v| v as f32),
                min_ms: a
                    .opt_in("min_ms", 10.0, 60_000.0)?
                    .map_or(d.min_ms, |v| v as f32),
                keep_ms: a
                    .opt_in("keep_ms", 0.0, 10_000.0)?
                    .map_or(d.keep_ms, |v| v as f32),
            })
        }
        "declick" => Edit::Declick {
            sensitivity: a.opt_in("sensitivity", 0.0, 1.0)?.map_or(0.5, |v| v as f32),
        },
        "repair" => Edit::Repair,
        _ => {
            let lo = a.f64("lo_hz")?;
            let hi = a.f64("hi_hz")?;
            if lo >= hi {
                return Err(ToolError::bad("hi_hz", "must be above lo_hz"));
            }
            if hi > t.rate / 2.0 {
                return Err(ToolError::bad(
                    "hi_hz",
                    "at most half the file's sample rate",
                ));
            }
            Edit::Attenuate {
                lo_hz: lo as f32,
                hi_hz: hi as f32,
                gain_db: db
                    .ok_or_else(|| ToolError::bad("gain_db", "is required for attenuate"))?,
            }
        }
    };
    let s = said(op, &t.name);
    job(t, sel, s, Box::new(move |_| Ok(edit)))
}

fn denoise_clip(cx: &mut ToolCx<'_>, v: &Value) -> Res<Job> {
    let a = A(v);
    let t = target(cx, a)?;
    let at = |s: f64| ((s * t.rate).round().max(0.0) as usize).min(t.frames);
    let (n0, n1) = (
        at(a.f64("noise_start_seconds")?),
        at(a.f64("noise_end_seconds")?),
    );
    if n1 <= n0 {
        return Err(ToolError::bad(
            "noise_end_seconds",
            "must be after noise_start_seconds",
        ));
    }
    let d = Params::default();
    let params = Params {
        amount: a.opt_in("amount", 0.0, 1.0)?.map_or(d.amount, |x| x as f32),
        reduction_db: a
            .opt_in("reduction_db", 0.0, 80.0)?
            .map_or(d.reduction_db, |x| x as f32),
        sensitivity: a
            .opt_in("sensitivity", 0.5, 8.0)?
            .map_or(d.sensitivity, |x| x as f32),
        smoothing: a
            .opt_in("smoothing", 0.0, 1.0)?
            .map_or(d.smoothing, |x| x as f32),
    };
    if params.amount <= 0.0 {
        return Err(ToolError::Refused("amount 0 would change nothing".into()));
    }
    let sel = range(a, t.rate, t.frames, true)?;
    let s = said("denoise", &t.name);
    job(
        t,
        sel,
        s,
        Box::new(move |buf: &AudioBuf| {
            let profile = jobs::learn(buf, Sel::new(n0, n1))
                .map_err(|e| ToolError::bad("noise_end_seconds", e))?;
            Ok(Edit::Denoise { profile, params })
        }),
    )
}

fn clean_said(cx: &ToolCx<'_>, _: &Value) -> Said {
    let (n, f, size) = cx
        .host
        .as_any()
        .downcast_ref::<CantusApp>()
        .map(|app| {
            let plan = commit::plan_cleanup(app);
            (
                plan.sources.len(),
                plan.files.len(),
                commit::bytes_text(plan.bytes()),
            )
        })
        .unwrap_or((0, 0, String::new()));
    Said::new("editor.tool.said_clean")
        .arg("n", n)
        .arg("f", f)
        .arg("size", size)
}

/// Asked first (the class asks, with the sizes): the unused entries go in one undo step and
/// the editor's files nothing refers to are deleted — never while an edit is being made.
fn remove_unused_audio(cx: &mut ToolCx<'_>, _: &Value) -> Res<Value> {
    use cantus_mcp::AgentHost;
    let app: &mut CantusApp = crate::ai_tools::project_app::app_mut(cx)?;
    let plan = commit::plan_cleanup(app);
    if plan.sources.is_empty() && plan.files.is_empty() {
        return Err(ToolError::Refused("nothing is unused".into()));
    }
    if let Some(command) = commit::cleanup_command(&plan) {
        app.execute(command, None).map_err(ToolError::Refused)?;
    }
    let deleted = commit::sweep(app);
    Ok(json!({
        "removed": plan.sources.iter().map(|(_, n, b)| json!({"name": n, "bytes": b})).collect::<Vec<_>>(),
        "files_deleted": deleted,
        "bytes": plan.bytes(),
        "note": "the removed entries' files stay until the undo history no longer holds them",
    }))
}
