//! What reaches the project from the audio editor (ADR 0096 §2, §7): a made edit as one
//! undo step (its file in the pool, the clip pointed at it, the file's marks moved with the
//! audio), files no one will ever play cleared away, and *Remove unused audio*.

use super::jobs::Applied;
use super::say;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use cantus_core::audio_edit::Note;
use cantus_core::{
    AudioRegion, AudioSource, AudioSourceId, ClipId, Command, Project, SourceMarks, TrackId,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The folder an edit's file goes in: the project's audio folder, or the session's scratch
/// until the project is saved (saving collects it, as it does a recording).
pub fn edit_dir(app: &CantusApp) -> PathBuf {
    // `Audio/<project file's name>`: one folder per project, as freeze keeps its renders
    // (several projects side by side share the folder they are in, and a cleanup of one
    // must never see another's files). Edits an older build wrote in `Audio/` itself stay
    // where their paths say, loaded as before, never cleaned.
    match (
        app.audio_base(),
        app.path.as_deref().and_then(Path::file_stem),
    ) {
        (Some(b), Some(stem)) => b.join("Audio").join(stem),
        (Some(b), None) => b.join("Audio"),
        _ => crate::app::data_dir().join("bounces"),
    }
}

/// When this run of Cantus began (first asked when the editor's state is made, at
/// start-up): a half-written file older than this is a cut session's, never one being
/// written now.
pub fn process_start() -> std::time::SystemTime {
    static START: std::sync::OnceLock<std::time::SystemTime> = std::sync::OnceLock::new();
    *START.get_or_init(std::time::SystemTime::now)
}

/// Whether `path` is a half-written file left before this run began.
fn stale_part(path: &Path) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .is_ok_and(|t| t < process_start())
}

/// Whether an agent's edit is being made (its file may be on its way, not yet pending).
pub fn agent_jobs(app: &CantusApp) -> bool {
    app.editor
        .agent_jobs
        .load(std::sync::atomic::Ordering::SeqCst)
        > 0
}

/// The undo label of an edit named `name` (`Edit::name`).
pub fn label_key(edit: &cantus_core::audio_edit::Edit) -> String {
    format!("cmd.editor.{}", edit.name())
}

/// A frame of the old take where it is in the new one, after edit `a` (`None`: cut away).
fn moved(a: &Applied, f: u64) -> Option<u64> {
    let (s, e) = (a.edited.used.start as u64, a.edited.used.end as u64);
    let d = a.edited.delta_frames;
    let n = a.buf.frames() as u64;
    let v = match a.name {
        _ if a.keeps_timing && d == 0 => Some(f),
        "delete" => Some(if f < s {
            f
        } else if f < e {
            s
        } else {
            f - (e - s)
        }),
        "trim" => (f >= s && f <= e).then(|| f - s),
        "paste" => {
            let m = a.edited.sel.len() as u64;
            Some(if f < s {
                f
            } else if f < e {
                s
            } else {
                f - (e - s) + m
            })
        }
        "insert_silence" => Some(if f < s { f } else { f + d.max(0) as u64 }),
        "reverse" => Some(if f >= s && f <= e { s + e - f } else { f }),
        _ => None,
    };
    v.map(|v| v.min(n))
}

/// The file's marks moved with the audio of edit `a` (cleared by *Remove silence*, whose
/// many cuts they would not survive whole).
pub fn moved_marks(marks: &SourceMarks, a: &Applied) -> SourceMarks {
    if a.name == "remove_silence" {
        return SourceMarks::default();
    }
    let mut out = SourceMarks::default();
    for m in &marks.markers {
        if let Some(frame) = moved(a, m.frame) {
            out.markers.push(cantus_core::SourceMarker {
                frame,
                name: m.name.clone(),
            });
        }
    }
    out.markers.sort_by_key(|m| m.frame);
    out.markers.dedup_by_key(|m| m.frame);
    let span = |x: u64, y: u64| -> Option<(u64, u64)> {
        let (p, q) = (moved(a, x)?, moved(a, y)?);
        let (p, q) = (p.min(q), p.max(q));
        (p < q).then_some((p, q))
    };
    for r in &marks.regions {
        if let Some((start, end)) = span(r.start, r.end) {
            out.regions.push(cantus_core::SourceRegion {
                start,
                end,
                name: r.name.clone(),
            });
        }
    }
    out.loop_points = marks.loop_points.and_then(|(x, y)| span(x, y));
    out
}

/// The commands that put a made edit in the project: its file in the pool (or the pool's
/// own, when the same bytes are already there), the clip pointed at it, the file's marks
/// moved with the audio, and the clip's length when it showed the whole take. Also says
/// whether the clip's pitch edits or warp markers were dropped. Shared with the agents.
pub fn commit_commands(
    app: &mut CantusApp,
    track: TrackId,
    clip: ClipId,
    from_source: AudioSourceId,
    applied: &Applied,
) -> Result<(Vec<Command>, bool), String> {
    let p = app.project();
    let c = p
        .track(track)
        .and_then(|t| t.clip(clip))
        .ok_or_else(|| t("editor.gone"))?
        .clone();
    let region = c.audio.clone().ok_or_else(|| t("editor.gone"))?;
    // A clip an ARA plug-in edits plays the plug-in's rendering: its audio is not this
    // window's to change (ADR 0097).
    if region.ara.is_some() {
        return Err(t("editor.ara_bound"));
    }
    if region.source != from_source {
        return Err(t("editor.stale"));
    }
    let old = p
        .audio_source(from_source)
        .cloned()
        .ok_or_else(|| t("editor.gone"))?;
    let old_marks = p
        .source_marks
        .get(&from_source)
        .cloned()
        .unwrap_or_default();
    let existing = p
        .audio_pool
        .iter()
        .find(|s| s.sha256.as_deref() == Some(applied.sha256.as_str()))
        .map(|s| s.id);
    let mut commands = Vec::new();
    let id = match existing {
        Some(id) => id,
        None => {
            // The project's folder as it is now (a Save As while the edit was made moved
            // it): the file goes there.
            let file = into_folder(app, &applied.file, applied.fresh)?;
            let id = AudioSourceId(app.bus.alloc_id());
            let base = app.audio_base().map(Path::to_path_buf);
            commands.push(Command::InsertAudioSource {
                source: AudioSource {
                    id,
                    name: tf("editor.source_name", &[("name", &old.name)]),
                    path: crate::audio_files::stored_path(base.as_deref(), &file),
                    sample_rate: applied.buf.rate,
                    channels: applied.buf.channels,
                    frames: applied.buf.frames() as u64,
                    sha256: Some(applied.sha256.clone()),
                    recorded: false,
                },
            });
            let marks = moved_marks(&old_marks, applied);
            if !marks.is_empty() {
                commands.push(Command::SetAudioSourceMarks {
                    id,
                    marks: Some(Box::new(marks)),
                });
            }
            id
        }
    };
    // The pitch editor's notes and a warp's markers are set on source frames: kept by an
    // edit that leaves every frame where it was, dropped by one that moves them.
    let keep = applied.keeps_timing && applied.edited.delta_frames == 0;
    let dropped = !keep && (region.pitch.is_some() || region.warp.is_some());
    let kept = AudioRegion {
        source: id,
        pitch: if keep { region.pitch.clone() } else { None },
        ara: None,
        warp: if keep { region.warp.clone() } else { None },
        ..region.clone()
    };
    commands.push(Command::SetClipAudio {
        track,
        clip,
        audio: Some(kept),
    });
    // A clip that showed the whole take shows the whole of the edited one.
    let same_length = applied.edited.delta_frames == 0;
    if !same_length
        && c.offset == cantus_core::Ticks::ZERO
        && c.link.is_none()
        && region.warp.is_none()
    {
        let seen =
            crate::audio_files::length_ticks(app.project(), c.start, old.frames, old.sample_rate);
        if c.length >= seen {
            let after = crate::audio_files::length_ticks(
                app.project(),
                c.start,
                applied.buf.frames() as u64,
                applied.buf.rate,
            );
            commands.push(Command::SetClipBounds {
                track,
                clip,
                start: c.start,
                length: after,
            });
        }
    }
    Ok((commands, dropped))
}

/// `file` in the project's edit folder as it is now: moved there (copied, the copy written
/// first) when the folder changed since it was written.
fn into_folder(app: &CantusApp, file: &Path, fresh: bool) -> Result<PathBuf, String> {
    let dir = edit_dir(app);
    if file.parent() == Some(dir.as_path()) {
        return Ok(file.to_path_buf());
    }
    let to = dir.join(file.file_name().ok_or("no file name")?);
    if !to.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        std::fs::copy(file, &to).map_err(|e| e.to_string())?;
    }
    if fresh {
        let _ = std::fs::remove_file(file);
    }
    Ok(to)
}

/// Take in a finished edit: one undo step, and what it did said.
pub fn finish_edit(app: &mut CantusApp, a: Applied) {
    let Some(s) = app.editor.session.as_ref() else {
        return discard(app, &a.file, a.fresh);
    };
    let (track, clip, from) = (s.track, s.clip, s.source);
    let (cmds, dropped) = match commit_commands(app, track, clip, from, &a) {
        Ok(c) => c,
        Err(m) => {
            discard(app, &a.file, a.fresh);
            return say(app, m, true);
        }
    };
    let source = cmds
        .iter()
        .find_map(|c| match c {
            Command::SetClipAudio { audio: Some(r), .. } => Some(r.source),
            _ => None,
        })
        .unwrap_or(from);
    if !app.exec(Command::Batch {
        label_key: format!("cmd.editor.{}", a.name),
        commands: cmds,
    }) {
        discard(app, &a.file, a.fresh);
        return say(app, t("editor.refused"), true);
    }
    let n = a.buf.frames();
    let mut status = tf(
        "editor.done",
        &[("what", &t(&format!("editor.op.{}", a.name)))],
    );
    match a.edited.note {
        Some(Note::Ceiling { reached_db }) => {
            status.push_str(&tf(
                "editor.note.ceiling",
                &[("db", &format!("{reached_db:.1}"))],
            ));
        }
        Some(Note::Clips { peak_db }) => {
            status.push_str(&tf(
                "editor.note.clips",
                &[("db", &format!("{peak_db:+.1}"))],
            ));
        }
        Some(Note::Clicks { count }) => {
            status.push_str(&tf("editor.note.clicks", &[("n", &count.to_string())]));
        }
        None => {}
    }
    if dropped {
        status.push_str(&t("editor.note.dropped"));
    }
    let warn = dropped || matches!(a.edited.note, Some(Note::Clips { .. }));
    if let Some(s) = app.editor.session.as_mut() {
        s.source = source;
        s.peaks = Some(a.peaks);
        s.buf = Some(a.buf.clone());
        let sel = a.edited.sel;
        s.sel = (!sel.is_empty() || a.edited.delta_frames != 0).then_some(sel.clamp(n));
        s.spec = None;
        s.detail = None;
        s.detail_asked = None;
        s.status = Some((status, warn));
    }
    super::dispatch(
        app,
        Some("editor.busy.picture"),
        super::jobs::Job::Picture { buf: a.buf },
    );
}

// --- files no one will play ------------------------------------------------------------

/// An agent's edit, planned: its file waits to see the edit made — the host may make it
/// now, ask the user first, or never — and goes if it never is.
pub struct Pending {
    pub file: PathBuf,
    /// The pool entry the edit adds (none: it reused one, and the file is that one's).
    pub source: Option<AudioSourceId>,
    pub fresh: bool,
    pub epoch: u64,
}

/// Settle the pending files: made (the source is in the pool), kept; still asked, waiting;
/// else gone.
pub fn settle_pending(app: &mut CantusApp) {
    if app.editor.pending.is_empty() {
        return;
    }
    let asking = app.ai.agents().is_some_and(|a| a.asks().next().is_some());
    let epoch = app.epoch;
    let pending = std::mem::take(&mut app.editor.pending);
    for p in pending {
        let made = p
            .source
            .is_some_and(|id| app.project().audio_source(id).is_some());
        if p.epoch == epoch && (made || p.source.is_none()) {
            continue;
        }
        if p.epoch == epoch && asking {
            app.editor.pending.push(p);
            continue;
        }
        discard(app, &p.file, p.fresh);
    }
}

/// Remove `file` when this edit wrote it and nothing — the pool, the history, the saved
/// project — refers to it.
pub fn discard(app: &CantusApp, file: &Path, fresh: bool) {
    if fresh && !referenced(app).contains(&key_of(file)) {
        let _ = std::fs::remove_file(file);
    }
}

/// How two paths to one file compare: resolved on disk where it exists.
fn key_of(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Every file the project could still play: its pool, every pool and source in its undo
/// and redo history, and the project as saved (and its autosave and backup) on disk.
pub fn referenced(app: &CantusApp) -> BTreeSet<PathBuf> {
    let base = app.audio_base().map(Path::to_path_buf);
    let mut out = BTreeSet::new();
    let mut add = |path: &str| {
        out.insert(key_of(&crate::waveform::resolve(base.as_deref(), path)));
    };
    let pool = |p: &Project, add: &mut dyn FnMut(&str)| {
        for s in &p.audio_pool {
            add(&s.path);
        }
    };
    pool(app.project(), &mut add);
    fn walk(c: &Command, add: &mut dyn FnMut(&str)) {
        match c {
            Command::Batch { commands, .. } => commands.iter().for_each(|c| walk(c, add)),
            Command::InsertAudioSource { source } => add(&source.path),
            Command::SetAudioSourcePath { path, .. } => add(path),
            Command::ReplaceProject { project } => {
                project.audio_pool.iter().for_each(|s| add(&s.path))
            }
            _ => {}
        }
    }
    for h in app.bus.history().iter().chain(app.bus.redo_history()) {
        walk(&h.command, &mut add);
        walk(&h.inverse, &mut add);
    }
    // Every project file in the folder — this one and any beside it — with its autosave and
    // backup: a file another project plays is never this one's to delete.
    let mut projects: Vec<PathBuf> = app.path.iter().cloned().collect();
    if let Some(dir) = base.as_deref() {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.flatten() {
                let p = e.path();
                let ext = p.extension().map(|x| x.to_string_lossy().to_lowercase());
                if ext.as_deref() == Some(cantus_project::FILE_EXTENSION) && !projects.contains(&p)
                {
                    projects.push(p);
                }
            }
        }
    }
    for path in &projects {
        for f in [
            path.to_path_buf(),
            cantus_project::autosave_path(path),
            cantus_project::backup_path(path),
        ] {
            if let Ok(l) = cantus_project::load(&f, false) {
                // A sibling's relative paths are relative to its own folder: the same one.
                pool(&l.project, &mut add);
            }
        }
    }
    // Files an agent's edit wrote and that wait to see it made.
    for p in &app.editor.pending {
        out.insert(key_of(&p.file));
    }
    out
}

/// Whether `name` is a file the editor writes: `edit-<16 hex>.wav`, or one half written.
pub fn is_edit_file(name: &str) -> bool {
    let stem = name
        .strip_suffix(".wav.part")
        .or_else(|| name.strip_suffix(".wav"));
    stem.and_then(|s| s.strip_prefix("edit-"))
        .is_some_and(|h| h.len() == 16 && h.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Half-written edit files (`edit-*.wav.part`) a cut session left in the edit folder.
pub fn tidy(app: &CantusApp) {
    if agent_jobs(app) || app.editor.busy() {
        return;
    }
    let Ok(dir) = std::fs::read_dir(edit_dir(app)) else {
        return;
    };
    for e in dir.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.ends_with(".wav.part") && is_edit_file(&name) && stale_part(&e.path()) {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

// --- a selection as a clip ----------------------------------------------------------

/// The selection's file (written by `act::to_new_clip`) on a new track, where the selection
/// plays now on the timeline, named after the clip: one undo step.
pub fn place_new_clip(app: &mut CantusApp, path: PathBuf, fresh: bool, from: usize) {
    let Some(s) = app.editor.session.as_ref() else {
        return discard(app, &path, fresh);
    };
    let (track, clip, source, name) = (s.track, s.clip, s.source, s.name.clone());
    let at = {
        let p = app.project();
        let c = p.track(track).and_then(|t| t.clip(clip));
        let src = p.audio_source(source);
        match (c, src) {
            (Some(c), Some(src)) => crate::pitch::ClipTime::of(p, c, src).tick(from as f64),
            _ => app.engine.position(),
        }
    };
    let onto = app
        .project()
        .tracks
        .iter()
        .position(|t| t.id == track)
        .map(|i| i + 1);
    let before: Vec<TrackId> = app.project().tracks.iter().map(|t| t.id).collect();
    if app.import_audio(
        std::slice::from_ref(&path),
        at.max(cantus_core::Ticks::ZERO),
        None,
    ) == 0
    {
        discard(app, &path, fresh);
        return say(app, t("editor.refused"), true);
    }
    let _ = onto;
    // Named after the clip, in the import's own undo step.
    let made = app
        .project()
        .tracks
        .iter()
        .find(|t| !before.contains(&t.id))
        .and_then(|t| t.clips.first().map(|c| (t.id, c.id)));
    if let (Some((t, c)), Some(key)) = (made, app.bus.open_gesture()) {
        app.exec_merging(
            Command::SetClipName {
                track: t,
                clip: c,
                name: tf("editor.part_name", &[("name", &name)]),
            },
            key,
        );
    }
    say(app, t("editor.new_clip"), false);
}

// --- Remove unused audio --------------------------------------------------------------

/// What *Remove unused audio* will do, shown before it does it.
#[derive(Clone, Debug, Default)]
pub struct Cleanup {
    /// Pool entries no clip, comp or session clip plays: (id, name, bytes).
    pub sources: Vec<(AudioSourceId, String, u64)>,
    /// Edit files on disk nothing refers to: (path, bytes).
    pub files: Vec<(PathBuf, u64)>,
}

impl Cleanup {
    pub fn bytes(&self) -> u64 {
        self.sources.iter().map(|s| s.2).sum::<u64>() + self.files.iter().map(|f| f.1).sum::<u64>()
    }
}

/// The pool entries nothing plays.
pub fn unused_sources(p: &Project) -> Vec<AudioSourceId> {
    p.audio_pool
        .iter()
        .map(|s| s.id)
        .filter(|id| {
            !p.tracks.iter().any(|t| {
                t.all_clips()
                    .chain(t.comps.iter().flat_map(|c| c.windows.iter()))
                    .any(|c| c.audio.as_ref().is_some_and(|a| a.source == *id))
            })
        })
        .collect()
}

/// The plan: unused pool entries, and edit files in the project's audio folder that
/// nothing refers to (only the editor's own files, never one a person put there; only in a
/// saved project's folder — the scratch folder is shared by every unsaved session).
pub fn plan_cleanup(app: &CantusApp) -> Cleanup {
    let p = app.project();
    let size = |path: &Path| std::fs::metadata(path).map_or(0, |m| m.len());
    let sources = unused_sources(p)
        .into_iter()
        .filter_map(|id| p.audio_source(id))
        .map(|s| (s.id, s.name.clone(), size(&app.audio_file(s))))
        .collect();
    let mut files = Vec::new();
    if app.audio_base().is_some() {
        let refs = referenced(app);
        if let Ok(dir) = std::fs::read_dir(edit_dir(app)) {
            for e in dir.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                // A half-written file only when it is a cut session's (older than this run).
                let part_ok = !name.ends_with(".part") || stale_part(&e.path());
                if is_edit_file(&name) && part_ok && !refs.contains(&key_of(&e.path())) {
                    files.push((e.path(), size(&e.path())));
                }
            }
        }
        files.sort();
    }
    Cleanup { sources, files }
}

/// The pool's part of a plan: one undo step that removes the unused entries.
pub fn cleanup_command(plan: &Cleanup) -> Option<Command> {
    (!plan.sources.is_empty()).then(|| Command::Batch {
        label_key: "cmd.editor.remove_unused".into(),
        commands: plan
            .sources
            .iter()
            .map(|(id, _, _)| Command::RemoveAudioSource { id: *id })
            .collect(),
    })
}

/// Open the dialog with the plan.
pub fn open_cleanup(app: &mut CantusApp) {
    app.editor.cleanup = Some(plan_cleanup(app));
}

/// Do the plan: the pool entries go in one undo step (their files stay: the history holds
/// them until it is cleared — run it again after reopening the project to delete those),
/// and the edit files nothing refers to are deleted. Returns (entries, files deleted).
pub fn run_cleanup(app: &mut CantusApp) -> (usize, usize) {
    let plan = plan_cleanup(app);
    let entries = plan.sources.len();
    if let Some(cmd) = cleanup_command(&plan) {
        if !app.exec(cmd) {
            return (0, 0);
        }
    }
    (entries, sweep(app))
}

/// Delete the edit files nothing refers to (after the pool changed); returns how many.
pub fn sweep(app: &CantusApp) -> usize {
    // Never while an edit (the window's or an agent's) may be writing its file.
    if app.audio_base().is_none() || app.editor.busy() || agent_jobs(app) {
        return 0;
    }
    plan_cleanup(app)
        .files
        .iter()
        .filter(|(p, _)| std::fs::remove_file(p).is_ok())
        .count()
}

/// "240 KB", "66 MB".
pub fn bytes_text(n: u64) -> String {
    if n >= 1 << 20 {
        format!("{:.1} MB", n as f64 / (1u64 << 20) as f64)
    } else {
        format!("{} KB", n.div_ceil(1024))
    }
}
