//! `export_video` for agents (round 35, ADR 0088): the dancers as a video with the song,
//! through the export the dialog runs — the same render, timelines, encoders and checks.
//! Files class: refused until the user allows files, always asked; it runs in the
//! background and `get_video_export` says when it ends.

use crate::app::CantusApp;
use crate::video_export::{self as vx, Back, Format, Quality, Range, Shape, VideoSettings};
use cantus_ai::tools_project::args::{self, Res, A};
use cantus_ai::{Said, ToolClass, ToolCx, ToolDef, ToolError, ToolRun};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

#[rustfmt::skip]
pub static TOOLS: &[ToolDef] = &[
    ToolDef { name: "export_video", title: "Export a dancing video", description: "Render the song (or the loop, the time selection) with every dancer of the project as a video: MP4 (H.264 + AAC, for every platform), WebM (AV1 + Vorbis, royalty-free, slower), MOV with alpha or a folder of PNG frames for a video editor. Shapes: vertical 1080x1920, square 1080x1080, wide 1920x1080. Into the project's folder (a relative file name) or a folder the user allowed in Settings, AI. Refused until the user allows files, and always asked. Runs in the background: get_video_export says when it ends. Never overwrites unless overwrite=true.", class: ToolClass::Files, input_schema: schema, output_schema: None, run: ToolRun::HostSaid { run: export_video, said } },
    ToolDef { name: "get_video_export", title: "Read the video export", description: "Whether a video export runs, how far (0-1) and its phase, and how the last one ended (the file, its size, or the error).", class: ToolClass::Read, input_schema: empty, output_schema: None, run: ToolRun::Read(get_video_export) },
];

fn empty() -> Value {
    args::object(json!({}), &[])
}

fn schema() -> Value {
    args::object(
        json!({
            "file": {"type": "string", "minLength": 1, "maxLength": 260, "description": "The video file (for png, the folder made for the frames): a file name (or a path under the project's folder), or an absolute path inside a folder the user allowed in Settings, AI. The extension is added when missing."},
            "format": {"enum": ["mp4", "webm", "mov", "png"], "default": "mp4"},
            "shape": {"enum": ["vertical", "square", "wide"], "default": "vertical"},
            "fps": {"enum": [30, 60], "default": 30},
            "quality": {"enum": ["good", "high", "best"], "default": "high"},
            "background": {"enum": ["stage", "transparent", "green", "blue", "magenta", "black"], "default": "stage", "description": "transparent only with mov and png."},
            "title": {"type": "boolean", "default": true, "description": "The song's name and tempo at the foot of the picture."},
            "range": {"enum": ["song", "loop", "selection"], "default": "song"},
            "overwrite": {"type": "boolean", "default": false}
        }),
        &["file"],
    )
}

/// The dialog's choices from the arguments (the last ones for what is left out).
fn settings(app: &CantusApp, a: A<'_>) -> Res<VideoSettings> {
    let mut d = app.settings.video.clone();
    if let Some(f) = a.opt_choice("format", &["mp4", "webm", "mov", "png"])? {
        d.format = match f {
            "webm" => Format::Webm,
            "mov" => Format::Mov,
            "png" => Format::Png,
            _ => Format::Mp4,
        };
    }
    if let Some(s) = a.opt_choice("shape", &["vertical", "square", "wide"])? {
        d.shape = match s {
            "square" => Shape::Square,
            "wide" => Shape::Landscape,
            _ => Shape::Vertical,
        };
    }
    if let Some(f) = a.opt_u64("fps")? {
        if f != 30 && f != 60 {
            return Err(ToolError::bad("fps", "30 or 60"));
        }
        d.fps = f as u32;
    }
    if let Some(q) = a.opt_choice("quality", &["good", "high", "best"])? {
        d.quality = match q {
            "good" => Quality::Good,
            "best" => Quality::Best,
            _ => Quality::High,
        };
    }
    if let Some(b) = a.opt_choice(
        "background",
        &["stage", "transparent", "green", "blue", "magenta", "black"],
    )? {
        d.background = match b {
            "transparent" => Back::Transparent,
            "green" => Back::Green,
            "blue" => Back::Blue,
            "magenta" => Back::Magenta,
            "black" => Back::Black,
            _ => Back::Stage,
        };
    }
    if d.background == Back::Transparent && !d.format.alpha() {
        return Err(ToolError::bad(
            "background",
            "transparent needs format mov or png",
        ));
    }
    if let Some(t) = a.opt_bool("title")? {
        d.title = t;
    }
    if let Some(r) = a.opt_choice("range", &["song", "loop", "selection"])? {
        d.range = match r {
            "loop" => Range::Loop,
            "selection" => Range::Selection,
            _ => Range::Song,
        };
    }
    Ok(d)
}

/// Where the file goes: under the project's folder, or in a folder the user allowed.
fn target(cx: &ToolCx<'_>, file: &str, d: &VideoSettings) -> Res<PathBuf> {
    let p = Path::new(file);
    let path = if p.is_absolute() {
        p.to_path_buf()
    } else {
        cx.host
            .project_dir()
            .ok_or_else(|| ToolError::Refused("the project is not saved yet, so it has no folder: save it first, or give an absolute path in a folder the user allowed".into()))?
            .join(p)
    };
    let path = vx::with_extension(&path, d);
    cantus_ai::resolve_touch(&*cx.host, &path).ok_or_else(|| {
        ToolError::Refused(format!(
            "{} is outside the project's folder and the folders the user allowed",
            path.display()
        ))
    })
}

fn said(cx: &ToolCx<'_>, a: &Value) -> Said {
    let a = A(a);
    let file = a.opt_str("file").ok().flatten().unwrap_or("?");
    let Some(app) = cx.host.as_any().downcast_ref::<CantusApp>() else {
        return Said::new("video.tool.said")
            .arg("format", "?")
            .name("file", file);
    };
    let Ok(d) = settings(app, a) else {
        return Said::new("video.tool.said")
            .arg("format", "?")
            .name("file", file);
    };
    let path = target(cx, file, &d).unwrap_or_else(|_| PathBuf::from(file));
    let shown = args::clean_path(&path.display().to_string());
    let key = if path.exists() {
        "video.tool.said_replace"
    } else {
        "video.tool.said"
    };
    Said::new(key)
        .arg("format", crate::i18n::t(d.format.key()))
        .name("file", shown)
}

fn export_video(cx: &mut ToolCx<'_>, a: &Value) -> Res<Value> {
    let a = A(a);
    let overwrite = a.opt_bool("overwrite")?.unwrap_or(false);
    let d = {
        let app = cx
            .host
            .as_any()
            .downcast_ref::<CantusApp>()
            .ok_or_else(|| ToolError::Failed("this tool needs the Cantus window".into()))?;
        settings(app, a)?
    };
    let path = target(cx, a.str("file")?, &d)?;
    match std::fs::symlink_metadata(&path) {
        Ok(m) if m.file_type().is_symlink() => {
            return Err(ToolError::Refused(format!(
                "{} is a link: not written through",
                path.display()
            )))
        }
        Ok(_) if !overwrite => {
            return Err(ToolError::Refused(format!(
                "{} exists: give overwrite=true to replace",
                path.display()
            )))
        }
        _ => {}
    }
    if d.format == Format::Png && path.exists() {
        return Err(ToolError::Refused(format!(
            "{} exists: a sequence is never written into a folder that is there",
            path.display()
        )));
    }
    let app = crate::ai_tools::project_app::app_mut(cx)?;
    vx::start(app, &path, &d, overwrite).map_err(ToolError::Refused)?;
    let (w, h) = d.size();
    Ok(json!({
        "started": true,
        "file": path.display().to_string(),
        "format": format!("{:?}", d.format).to_lowercase(),
        "width": w, "height": h, "fps": d.fps,
        "follow": "get_video_export"
    }))
}

fn get_video_export(cx: &ToolCx<'_>, _: &Value) -> Res<Value> {
    let app = cx
        .host
        .as_any()
        .downcast_ref::<CantusApp>()
        .ok_or_else(|| ToolError::Failed("this tool needs the Cantus window".into()))?;
    let running = app.video.progress().map(|(phase, f, left)| {
        json!({"phase": crate::i18n::t(phase), "progress": f, "seconds_left": left})
    });
    let last = match &app.video.last {
        Some(Ok(m)) => json!({"ok": true, "said": m}),
        Some(Err(m)) => json!({"ok": false, "error": m}),
        None => Value::Null,
    };
    Ok(json!({"busy": app.video.busy(), "running": running, "last": last}))
}
