//! Comping a track's takes and exporting the project, for agents (round 34, ADR 0082): the
//! same builders and the same export paths the interface uses.

use super::app_mut;
use crate::app::CantusApp;
use cantus_ai::tools_project::args::{self, plan, s_id, Res, A};
use cantus_ai::tools_project::words;
use cantus_ai::{EditPlan, Said, ToolCx, ToolError, Touch};
use cantus_core::{ClipId, Command, Project, TrackId};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Build commands the way `CantusApp::exec_built` does: ids handed out from the project's
/// counter, reserved on the bus afterwards.
fn built(
    cx: &mut ToolCx<'_>,
    label: &str,
    f: impl FnOnce(&Project, crate::clips::Ids) -> Vec<Command>,
) -> Res<Command> {
    let mut next = cx.project().next_id;
    let commands = {
        let mut ids = || {
            let id = next;
            next += 1;
            id
        };
        f(cx.project(), &mut ids)
    };
    cx.bus_mut().reserve_ids(next);
    if commands.is_empty() {
        return Err(ToolError::bad(
            "op",
            "nothing to change: is there a take there?",
        ));
    }
    Ok(Command::Batch {
        label_key: label.to_string(),
        commands,
    })
}

fn comp_index(p: &Project, track: TrackId, v: &Value, k: &str) -> Res<u8> {
    let t = p.track(track).expect("checked");
    let n = t.comps.len().max(1);
    let i = if let Some(i) = v.as_u64() {
        i as usize
    } else if let Some(s) = v.as_str() {
        let c = s.trim().to_ascii_uppercase();
        let ch = c
            .chars()
            .next()
            .filter(|c| c.is_ascii_uppercase() && c.len_utf8() == 1 && s.trim().len() == 1);
        ch.map(|c| (c as u8 - b'A') as usize)
            .ok_or_else(|| ToolError::bad(k, "a comp's letter (A, B…) or number from 0"))?
    } else {
        return Err(ToolError::bad(k, "a comp's letter or number"));
    };
    if i >= n {
        return Err(ToolError::bad(k, format!("the track has {n} comp(s)")));
    }
    Ok(i as u8)
}

pub fn comp_schema() -> Value {
    args::object(
        json!({
            "track_id": s_id("The track whose takes to comp."),
            "op": {"enum": ["swipe", "promote_lane", "promote_clip", "switch", "duplicate", "rename"]},
            "lane": {"type": "integer", "minimum": 1, "maximum": 255, "description": "swipe, promote_lane: the take lane (1 is the first under the playing lane)."},
            "from": args::s_pos("swipe: the range's start"),
            "to": args::s_pos("swipe: its end"),
            "clip_id": s_id("promote_clip: the take clip."),
            "comp": {"type": ["string", "integer"], "description": "switch, rename: the alternate comp, by letter (A, B…) or number from 0."},
            "name": {"type": "string", "minLength": 1, "maxLength": 40}
        }),
        &["track_id", "op"],
    )
}

pub fn comp_takes(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let map = p.tempo_map();
    let track = args::track(p, a, "track_id")?;
    let tw = args::clean(&words::track_name(p, track));
    let op = a
        .opt_choice(
            "op",
            &[
                "swipe",
                "promote_lane",
                "promote_clip",
                "switch",
                "duplicate",
                "rename",
            ],
        )?
        .ok_or_else(|| ToolError::bad("op", "is required"))?;
    let lane = || -> Res<u8> { Ok(a.f64_in("lane", 1.0, 255.0)? as u8) };
    let (cmd, words) = match op {
        "swipe" => {
            let (l, from, to) = (
                lane()?,
                args::pos(&map, a, "from")?,
                args::pos(&map, a, "to")?,
            );
            if to <= from {
                return Err(ToolError::bad("to", "must be after from"));
            }
            let (b0, b1) = words::bar_numbers(&map, from, to);
            let words = Said::new("ai.tools.comp.swipe")
                .arg("lane", l)
                .name("track", tw.clone())
                .bars("bars", b0, b1);
            (
                built(cx, "cmd.comp_swipe", |p, ids| {
                    crate::comping::swipe(p, track, l, from, to, ids)
                })?,
                words,
            )
        }
        "promote_lane" => {
            let l = lane()?;
            (
                built(cx, "cmd.comp_promote", |p, ids| {
                    crate::comping::promote_lane(p, track, l, ids)
                })?,
                Said::new("ai.tools.comp.promote_lane")
                    .arg("lane", l)
                    .name("track", tw.clone()),
            )
        }
        "promote_clip" => {
            let clip = ClipId(a.u64("clip_id")?);
            let c = p
                .track(track)
                .and_then(|t| t.clip(clip))
                .ok_or_else(|| ToolError::NotFound(format!("clip {} on that track", clip.0)))?;
            let words = Said::new("ai.tools.comp.promote_clip")
                .nested("clip", words::clip(p, &map, track, c));
            (
                built(cx, "cmd.comp_promote", |p, ids| {
                    crate::comping::promote_clip(p, track, clip, ids)
                })?,
                words,
            )
        }
        "switch" => {
            let to = comp_index(p, track, a.req("comp")?, "comp")?;
            let group = crate::comping::comp_group(p, track);
            let c = built(cx, "cmd.comp_switch", |p, _| {
                group
                    .iter()
                    .flat_map(|g| crate::comping::switch_comp(p, *g, to))
                    .collect()
            })?;
            (
                c,
                Said::new("ai.tools.comp.switch")
                    .name("track", tw.clone())
                    .arg("comp", cantus_core::comp::letter(to as usize)),
            )
        }
        "duplicate" => (
            built(cx, "cmd.comp_duplicate", |p, ids| {
                crate::comping::duplicate_comp(p, track, ids)
            })?,
            Said::new("ai.tools.comp.duplicate").name("track", tw.clone()),
        ),
        _ => {
            let which = comp_index(p, track, a.req("comp")?, "comp")?;
            let name = a
                .opt_name("name", 40)?
                .ok_or_else(|| ToolError::bad("name", "is required"))?;
            let words = Said::new("ai.tools.comp.rename")
                .arg("comp", cantus_core::comp::letter(which as usize))
                .name("track", tw.clone())
                .name("name", args::clean(&name));
            (
                built(cx, "cmd.comp_rename", |p, _| {
                    crate::comping::rename_comp(p, track, which, &name)
                })?,
                words,
            )
        }
    };
    Ok(plan(cmd, words, json!({"track_id": track.0, "op": op})))
}

pub fn tidy_schema() -> Value {
    args::object(
        json!({
            "track_id": s_id("The track."),
            "op": {"enum": ["delete_comp", "flatten", "delete_unused_takes"]},
            "comp": {"type": ["string", "integer"], "description": "delete_comp: by letter or number."}
        }),
        &["track_id", "op"],
    )
}

pub fn tidy_takes(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let track = args::track(p, a, "track_id")?;
    let tw = args::clean(&words::track_name(p, track));
    let op = a
        .opt_choice("op", &["delete_comp", "flatten", "delete_unused_takes"])?
        .ok_or_else(|| ToolError::bad("op", "is required"))?;
    let (cmd, words) = match op {
        "delete_comp" => {
            let which = comp_index(p, track, a.req("comp")?, "comp")?;
            (
                built(cx, "cmd.comp_delete", |p, _| {
                    crate::comping::delete_comp(p, track, which)
                })?,
                Said::new("ai.tools.comp.delete")
                    .arg("comp", cantus_core::comp::letter(which as usize))
                    .name("track", tw.clone()),
            )
        }
        "flatten" => (
            built(cx, "cmd.comp_flatten", |p, _| {
                crate::comping::flatten(p, track)
            })?,
            Said::new("ai.tools.comp.flatten").name("track", tw.clone()),
        ),
        _ => (
            built(cx, "cmd.delete_unused_takes", |p, _| {
                crate::comping::delete_unused_takes(p, track)
            })?,
            Said::new("ai.tools.comp.delete_unused").name("track", tw.clone()),
        ),
    };
    Ok(plan(cmd, words, json!({"track_id": track.0, "op": op})))
}

// --- export --------------------------------------------------------------------------------

fn file_schema(what: &str) -> Value {
    json!({"type": "string", "minLength": 1, "maxLength": 260, "description": format!("{what}: a file name (or a path under the project's folder), or an absolute path inside a folder the user allowed in Settings, AI. The extension is added when missing.")})
}

pub fn export_audio_schema() -> Value {
    args::object(
        json!({
            "format": {"enum": ["wav", "flac", "mp3", "ogg"]},
            "file": file_schema("The audio file (for stems, the folder they go in)"),
            "scope": {"enum": ["song", "loop", "selection", "stems"], "default": "song", "description": "The whole song, the loop region, the time selection, or every strip as a stem."},
            "overwrite": {"type": "boolean", "default": false}
        }),
        &["format", "file"],
    )
}

pub fn export_project_schema() -> Value {
    args::object(
        json!({
            "format": {"enum": ["midi", "dawproject"]},
            "file": file_schema("The file"),
            "track_id": s_id("MIDI: only this track (left out: every track)."),
            "clip_id": s_id("MIDI: only this clip (with its track_id)."),
            "as_written": {"type": "boolean", "default": false, "description": "MIDI: the notes as written, without grooves and note effects."},
            "overwrite": {"type": "boolean", "default": false}
        }),
        &["format", "file"],
    )
}

/// Where `file` goes: the project's folder for a relative name, else an allowed folder —
/// resolved as `resolve_touch` does, so no path an agent gives steps outside them. `ext`
/// is added when missing (none for a folder of stems).
fn target(cx: &ToolCx<'_>, file: &str, ext: &str) -> Res<PathBuf> {
    let p = Path::new(file);
    let mut path = if p.is_absolute() {
        p.to_path_buf()
    } else {
        cx.host
            .project_dir()
            .ok_or_else(|| ToolError::Refused("the project is not saved yet, so it has no folder: save it first, or give an absolute path in a folder the user allowed".into()))?
            .join(p)
    };
    if !ext.is_empty()
        && path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
            != Some(ext)
    {
        let mut s = path.into_os_string();
        s.push(format!(".{ext}"));
        path = PathBuf::from(s);
    }
    cantus_ai::resolve_touch(&*cx.host, &path).ok_or_else(|| {
        ToolError::Refused(format!(
            "{} is outside the project's folder and the folders the user allowed",
            path.display()
        ))
    })
}

fn refused_open(path: &Path, e: std::io::Error) -> ToolError {
    if e.kind() == std::io::ErrorKind::AlreadyExists {
        ToolError::Refused(format!(
            "{} exists: give overwrite=true to replace it",
            path.display()
        ))
    } else {
        ToolError::Refused(format!("{}: {e}", path.display()))
    }
}

/// The number in the next part's name (`<file>.<process>-<n>.part`).
static PART_NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// How many part names that are taken [`write_whole`] passes over before it gives up.
const PART_TRIES: u32 = 64;

/// Write a whole file or none (fix review #3): into a new `.part` beside it, opened through
/// `open_touch`, flushed and synced, then put in place — renamed over the old one with
/// `overwrite`, else linked so that anything that appeared there meanwhile is refused. On
/// any failure the part goes and the target is as it was.
fn write_whole(
    roots: &dyn cantus_ai::ToolHost,
    path: &Path,
    overwrite: bool,
    f: impl FnOnce(&mut std::io::BufWriter<std::fs::File>) -> std::io::Result<()>,
) -> Res<PathBuf> {
    use std::io::Write;
    // A part of its own each time (fix review 3 #6): one a crash left behind never blocks —
    // nor one of an earlier process with the same id (Windows reuses them, fix review 4 #4):
    // a part that is there already is passed over for the next name.
    let mut tries = 0;
    let (file, real_part) = loop {
        let n = PART_NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut name = path.file_name().unwrap_or_default().to_os_string();
        name.push(format!(".{}-{n}.part", std::process::id()));
        let part = path.with_file_name(name);
        match cantus_ai::open_touch(roots, &part, Touch::CreateNew) {
            Ok(opened) => break opened,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && tries < PART_TRIES => {
                tries += 1;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(ToolError::Refused(format!(
                    "{}: no new part file could be made beside it ({PART_TRIES} were taken)",
                    path.display()
                )))
            }
            Err(e) => return Err(refused_open(&part, e)),
        }
    };
    let target = real_part.with_file_name(path.file_name().unwrap_or_default());
    let done = (|| -> std::io::Result<()> {
        let mut w = std::io::BufWriter::new(file);
        f(&mut w)?;
        w.flush()?;
        w.into_inner().map_err(|e| e.into_error())?.sync_all()?;
        match std::fs::symlink_metadata(&target) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "a link is there",
                ))
            }
            Ok(_) if !overwrite => return Err(std::io::ErrorKind::AlreadyExists.into()),
            _ => {}
        }
        if overwrite {
            std::fs::rename(&real_part, &target)
        } else {
            // A link that fails if anything appeared, or a copy into a new file where the
            // file system has no links (fix review 3 #2, #7).
            cantus_export::job::place_new(&real_part, &target)
        }
    })();
    match done {
        Ok(()) => Ok(target),
        Err(e) => {
            let _ = std::fs::remove_file(&real_part);
            Err(refused_open(&target, e))
        }
    }
}

/// The roots a worker may write in, taken from the host when the job is prepared: the job
/// opens its file through `open_touch` against them (mcp's fourth review), so a link or a
/// file put there after the plan is caught when the file is opened.
struct Roots {
    bus: cantus_core::CommandBus,
    roots: Vec<PathBuf>,
    dir: Option<PathBuf>,
}

impl cantus_ai::ToolHost for Roots {
    fn bus(&self) -> &cantus_core::CommandBus {
        &self.bus
    }
    fn bus_mut(&mut self) -> &mut cantus_core::CommandBus {
        &mut self.bus
    }
    fn project_dir(&self) -> Option<PathBuf> {
        self.dir.clone()
    }
    fn file_roots(&self) -> Vec<PathBuf> {
        self.roots.clone()
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// MIDI or DAWproject: prepared at once (nothing happens until the user allows it), the
/// file opened and written on a worker; the answer when it is written.
pub fn export_project(cx: &mut ToolCx<'_>, a: &Value) -> Res<cantus_ai::Job> {
    let a = A(a);
    let format = a
        .opt_choice("format", &["midi", "dawproject"])?
        .ok_or_else(|| ToolError::bad("format", "is required"))?;
    let overwrite = a.opt_bool("overwrite")?.unwrap_or(false);
    let ext = if format == "midi" {
        "mid"
    } else {
        "dawproject"
    };
    let path = target(cx, a.str("file")?, ext)?;
    if path.exists() && !overwrite {
        return Err(refused_open(
            &path,
            std::io::ErrorKind::AlreadyExists.into(),
        ));
    }
    let shown = path.display().to_string();
    let said = Said::new(if path.exists() {
        "ai.tools.export_project_replace"
    } else {
        "ai.tools.export_project"
    })
    .arg(
        "format",
        if format == "midi" {
            "MIDI"
        } else {
            "DAWproject"
        },
    )
    .name("file", args::clean_path(&shown));
    let roots = Roots {
        bus: cantus_core::CommandBus::default(),
        roots: cx.host.file_roots(),
        dir: cx.host.project_dir(),
    };
    let work: cantus_ai::JobWork = if format == "midi" {
        let p = cx.project();
        let track = args::opt_track(p, a, "track_id")?;
        let scope = match (track, a.opt_u64("clip_id")?) {
            (Some(t), Some(c)) => {
                p.track(t)
                    .and_then(|x| x.clip(ClipId(c)))
                    .ok_or_else(|| ToolError::NotFound(format!("clip {c} on track {}", t.0)))?;
                cantus_project::midi::Scope::Clip {
                    track: t,
                    clip: ClipId(c),
                }
            }
            (Some(t), None) => cantus_project::midi::Scope::Track(t),
            (None, Some(_)) => {
                return Err(ToolError::bad("track_id", "a clip is given with its track"))
            }
            (None, None) => cantus_project::midi::Scope::Song,
        };
        let notes = if a.opt_bool("as_written")?.unwrap_or(false) {
            cantus_project::midi::Notes::Written
        } else {
            cantus_project::midi::Notes::Played
        };
        let hook = crate::notefx_ui::midi_hook;
        let bytes = cantus_project::midi::export_with(
            p,
            &cantus_project::midi::ExportOptions {
                scope,
                ppq: cantus_project::midi::DEFAULT_PPQ,
                notes,
            },
            Some(&hook),
        )
        .map_err(|e| ToolError::Failed(e.to_string()))?;
        Box::new(move |_| {
            use std::io::Write;
            let real = write_whole(&roots, &path, overwrite, |w| w.write_all(&bytes))?;
            Ok(cantus_ai::JobDone::Answer(
                json!({"file": real.display().to_string(), "bytes": bytes.len()}),
            ))
        })
    } else {
        let project = cx.project().clone();
        let base = cx
            .host
            .as_any()
            .downcast_ref::<crate::app::CantusApp>()
            .and_then(|app| app.audio_base().map(Path::to_path_buf));
        Box::new(move |_| {
            let real = write_whole(&roots, &path, overwrite, |w| {
                cantus_project::dawproject::export_into(
                    &project,
                    crate::app::APP_VERSION,
                    base.as_deref(),
                    w,
                )
                .map(|_| ())
            })?;
            Ok(cantus_ai::JobDone::Answer(
                json!({"file": real.display().to_string()}),
            ))
        })
    };
    Ok(cantus_ai::Job { said, work })
}

/// What an audio export would write: the file, or each stem's file in the folder.
fn audio_outputs(app: &CantusApp, path: &Path, d: &cantus_export::ExportSettings) -> Vec<PathBuf> {
    if d.scope == cantus_export::Scope::Stems {
        let specs =
            crate::views::interop::stem_specs(app.project(), d.stems.tracks, d.stems.master);
        cantus_export::stems::output_names(&specs, d)
            .into_iter()
            .map(|n| path.join(n))
            .collect()
    } else {
        vec![path.to_path_buf()]
    }
}

/// The dialog's settings for an agent's export.
fn audio_settings(
    app: &CantusApp,
    a: A<'_>,
) -> Res<(cantus_export::ExportSettings, &'static str, bool)> {
    use cantus_export::{Format, Scope};
    let format = a
        .opt_choice("format", &["wav", "flac", "mp3", "ogg"])?
        .ok_or_else(|| ToolError::bad("format", "is required"))?;
    let scope = a
        .opt_choice("scope", &["song", "loop", "selection", "stems"])?
        .unwrap_or("song");
    let mut d = app.settings.export.clone();
    d.tags.title = app.project().name.clone();
    d.format = match format {
        "wav" => Format::Wav,
        "flac" => Format::Flac,
        "ogg" => Format::Ogg,
        _ => Format::Mp3,
    };
    d.scope = match scope {
        "loop" => Scope::Loop,
        "selection" => Scope::Selection,
        "stems" => Scope::Stems,
        _ => Scope::Song,
    };
    let stems = scope == "stems";
    let format: &'static str = match format {
        "wav" => "wav",
        "flac" => "flac",
        "ogg" => "ogg",
        _ => "mp3",
    };
    Ok((d, format, stems))
}

/// The card's sentence for `export_audio` (review #4): the file, and whether it replaces one.
pub fn export_audio_said(cx: &ToolCx<'_>, a: &Value) -> Said {
    let a = A(a);
    let Some(app) = cx.host.as_any().downcast_ref::<CantusApp>() else {
        return Said::new("ai.tools.export_audio")
            .arg("format", "?")
            .name("file", "?");
    };
    let Ok((d, format, stems)) = audio_settings(app, a) else {
        return Said::new("ai.tools.export_audio")
            .arg("format", "?")
            .name("file", "?");
    };
    let file = a.opt_str("file").ok().flatten().unwrap_or("");
    let path =
        target(cx, file, if stems { "" } else { format }).unwrap_or_else(|_| PathBuf::from(file));
    let shown = args::clean_path(&path.display().to_string());
    let there = audio_outputs(app, &path, &d)
        .iter()
        .filter(|p| p.exists())
        .count();
    match (stems, there) {
        (true, 0) => Said::new("ai.tools.export_stems").name("folder", shown),
        (true, n) => Said::new("ai.tools.export_stems_replace")
            .name("folder", shown)
            .nested("n", words::count(n, "files")),
        (false, 0) => Said::new("ai.tools.export_audio")
            .arg("format", format.to_uppercase())
            .name("file", shown),
        (false, _) => Said::new("ai.tools.export_audio_replace")
            .arg("format", format.to_uppercase())
            .name("file", shown),
    }
}

/// Audio and stems through the export the dialog runs (its waiting for samples and
/// plug-ins, its hosted render instances, its all-or-nothing `.part` and rename), started at
/// once; get_selection follows it. Nothing is created or truncated before the export writes
/// (fix review #2): each output is checked where it really is — not a link, absent unless
/// `overwrite` — and without leave to overwrite the exporter puts each file in place by a
/// link that fails if anything has appeared there meanwhile.
pub fn export_audio(cx: &mut ToolCx<'_>, a: &Value) -> Res<Value> {
    let a = A(a);
    let overwrite = a.opt_bool("overwrite")?.unwrap_or(false);
    let app = cx
        .host
        .as_any()
        .downcast_ref::<CantusApp>()
        .ok_or_else(|| ToolError::Failed("this tool needs the Cantus window".into()))?;
    let (mut d, format, stems) = audio_settings(app, a)?;
    let path = target(cx, a.str("file")?, if stems { "" } else { format })?;
    let outputs = audio_outputs(app, &path, &d);
    let mut there = Vec::new();
    for out in &outputs {
        let real = cantus_ai::resolve_touch(&*cx.host, out).ok_or_else(|| {
            ToolError::Refused(format!("{} is outside the folders allowed", out.display()))
        })?;
        match std::fs::symlink_metadata(&real) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(ToolError::Refused(format!(
                    "{} is a link: not written through",
                    real.display()
                )))
            }
            Ok(_) => there.push(real.display().to_string()),
            Err(_) => {}
        }
    }
    if !there.is_empty() && !overwrite {
        return Err(ToolError::Refused(format!(
            "{} exist(s): give overwrite=true to replace",
            there.join(", ")
        )));
    }
    d.no_replace = !overwrite;
    if stems {
        std::fs::create_dir_all(&path)
            .map_err(|e| ToolError::Failed(format!("{}: {e}", path.display())))?;
    }
    let app = app_mut(cx)?;
    if !crate::views::export::start_with(app, &path, Some(d)) {
        return Err(ToolError::Failed(app.status.text.clone()));
    }
    Ok(
        json!({"file": path.display().to_string(), "files": outputs.len(), "started": true,
        "follow": "get_selection's export field says when it ends"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fix review #3: a write that fails half-way leaves no half file and no part, and the
    /// old file as it was.
    #[test]
    fn a_failed_write_leaves_the_old_file_whole() {
        let dir = std::env::temp_dir().join(format!("cantus-write-whole-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let roots = Roots {
            bus: cantus_core::CommandBus::default(),
            roots: vec![dir.clone()],
            dir: Some(dir.clone()),
        };
        let target = dir.join("song.dawproject");
        let fail = |w: &mut std::io::BufWriter<std::fs::File>| -> std::io::Result<()> {
            use std::io::Write;
            w.write_all(b"PK half an archive")?;
            Err(std::io::Error::other("the disk is full"))
        };
        assert!(write_whole(&roots, &target, false, fail).is_err());
        assert!(!target.exists(), "no half file");
        std::fs::write(&target, b"the user's").unwrap();
        assert!(write_whole(&roots, &target, true, fail).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"the user's");
        assert!(
            std::fs::read_dir(&dir).unwrap().all(|e| !e
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".part")),
            "no part left by a failed write"
        );
        // A part a crash left behind does not block the next write (fix review 3 #6).
        std::fs::write(dir.join("song.dawproject.part"), b"stale").unwrap();
        let ok = write_whole(&roots, &target, true, |w| {
            use std::io::Write;
            w.write_all(b"PK whole")
        })
        .unwrap();
        assert_eq!(std::fs::read(&ok).unwrap(), b"PK whole");
        let fresh = dir.join("fresh.mid");
        write_whole(&roots, &fresh, false, |w| {
            use std::io::Write;
            w.write_all(b"MThd")
        })
        .unwrap();
        assert_eq!(std::fs::read(&fresh).unwrap(), b"MThd");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Fix review 4 #4: parts left by an earlier process with this one's id (Windows reuses
    /// them) are passed over and left as they are; the write never asks for `overwrite`.
    #[test]
    fn parts_left_under_this_process_id_never_block_a_write() {
        let dir = std::env::temp_dir().join(format!("cantus-part-reuse-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let roots = Roots {
            bus: cantus_core::CommandBus::default(),
            roots: vec![dir.clone()],
            dir: Some(dir.clone()),
        };
        let target = dir.join("song.mid");
        let n = PART_NEXT.load(std::sync::atomic::Ordering::Relaxed);
        let stale: Vec<_> = (n..n + 8)
            .map(|k| dir.join(format!("song.mid.{}-{k}.part", std::process::id())))
            .collect();
        for p in &stale {
            std::fs::write(p, b"an old process's").unwrap();
        }
        let written = write_whole(&roots, &target, false, |w| {
            use std::io::Write;
            w.write_all(b"MThd")
        })
        .unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(std::fs::read(&written).unwrap(), b"MThd");
        for p in &stale {
            assert_eq!(
                std::fs::read(p).unwrap(),
                b"an old process's",
                "{}",
                p.display()
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
