//! Sprite sheets (ADR 0089 §6): Aseprite's JSON (hash or array, `frameTags` with their
//! direction and repeat, frame durations) and TexturePacker's (hash or array, rotated and
//! trimmed frames, `animations` lists or numbered names), with one PNG atlas beside them.

use super::image::{self, PixelBudget};
use super::imported::Source;
use super::limits::{self, Budget, Limits};
use super::mapping::{normalise, Clip};
use super::raster::{self, Filter, Region, Tint, Vertex};
use super::{paths, Found, ImportError, Note};
use crate::character::{Bounds, Licence};
use crate::render::Canvas;
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;
use tiny_skia::Pixmap;

/// One frame: where it is in the atlas and where it sits on the sprite's canvas.
#[derive(Clone, Debug, PartialEq)]
struct Frame {
    name: String,
    /// The atlas rectangle as stored (a rotated frame's is turned).
    region: Region,
    rotated: bool,
    /// The trimmed image's size (unrotated) and its place on the sprite's canvas.
    w: f32,
    h: f32,
    ox: f32,
    oy: f32,
    seconds: f32,
}

pub struct Sheet {
    format: &'static str,
    atlas: Arc<Pixmap>,
    frames: Vec<Frame>,
    clips: Vec<Clip>,
    /// Per clip: its frames and where each starts (seconds into the clip).
    seqs: Vec<Vec<(usize, f32)>>,
    /// The box the art fills on the sprite's canvas: left, top, right, bottom.
    boxed: [f32; 4],
    fps: f32,
}

/// The pace of a sheet that gives no durations (TexturePacker): twelve frames a second.
pub const DEFAULT_FPS: f32 = 12.0;

fn num(v: &Value) -> f32 {
    v.as_f64()
        .filter(|f| f.is_finite())
        .map(|f| f as f32)
        .unwrap_or(0.0)
}

fn rect(v: &Value) -> (f32, f32, f32, f32) {
    (num(&v["x"]), num(&v["y"]), num(&v["w"]), num(&v["h"]))
}

/// Whether a JSON document is a sprite sheet's.
pub fn is_sheet(v: &Value) -> bool {
    v.get("frames")
        .is_some_and(|f| f.is_object() || f.is_array())
        && v["meta"].is_object()
}

fn format_of(v: &Value) -> &'static str {
    let app = v["meta"]["app"].as_str().unwrap_or("").to_ascii_lowercase();
    if app.contains("aseprite") || app.contains("libresprite") || v["meta"]["frameTags"].is_array()
    {
        "Aseprite"
    } else {
        "TexturePacker"
    }
}

/// Read a sheet from its JSON (already parsed) and the atlas in `folder`.
pub fn read(v: &Value, folder: &Path, limits: &Limits) -> Result<Found, ImportError> {
    let format = format_of(v);
    let mut notes = Vec::new();
    let mut budget = Budget::new();
    let image_name = v["meta"]["image"]
        .as_str()
        .ok_or_else(|| ImportError::Invalid("the sheet names no image (meta.image)".into()))?;
    let bytes = paths::read_inside(folder, image_name, limits.image_bytes)?;
    let atlas = image::decode(&bytes, limits, &mut PixelBudget::default())?;
    let region_ok = |x: f32, y: f32, w: f32, h: f32| {
        x >= 0.0
            && y >= 0.0
            && w > 0.0
            && h > 0.0
            && x + w <= atlas.width() as f32
            && y + h <= atlas.height() as f32
    };
    // The frames, in the file's order (hash keys keep it: serde_json preserves order).
    let entries: Vec<(String, &Value)> = match &v["frames"] {
        Value::Object(m) => m.iter().map(|(k, f)| (k.clone(), f)).collect(),
        Value::Array(a) => a
            .iter()
            .map(|f| (f["filename"].as_str().unwrap_or("").to_string(), f))
            .collect(),
        _ => Vec::new(),
    };
    let mut frames = Vec::new();
    for (name, f) in entries {
        budget.take("frames", 1, limits.frames)?;
        let (x, y, w, h) = rect(&f["frame"]);
        let rotated = f["rotated"].as_bool().unwrap_or(false);
        // A rotated frame keeps its upright size in `frame`; the atlas holds it turned.
        let (aw, ah) = if rotated { (h, w) } else { (w, h) };
        if !region_ok(x, y, aw, ah) {
            return Err(ImportError::Invalid(format!(
                "the frame \"{}\" lies outside its image",
                limits::name(&name, limits)
            )));
        }
        let trimmed = f["trimmed"].as_bool().unwrap_or(false);
        let (ox, oy) = if trimmed || f.get("spriteSourceSize").is_some() {
            let (sx, sy, _, _) = rect(&f["spriteSourceSize"]);
            (sx, sy)
        } else {
            (0.0, 0.0)
        };
        let source = &f["sourceSize"];
        let (sw, sh) = (num(&source["w"]), num(&source["h"]));
        let seconds = match f["duration"].as_f64() {
            Some(ms) if ms.is_finite() && ms > 0.0 => (ms / 1000.0).min(60.0) as f32,
            _ => 1.0 / DEFAULT_FPS,
        };
        frames.push((
            Frame {
                name: limits::name(&name, limits),
                region: Region {
                    x0: x as i32,
                    y0: y as i32,
                    x1: (x + aw) as i32,
                    y1: (y + ah) as i32,
                },
                rotated,
                w,
                h,
                ox,
                oy,
                seconds,
            },
            (sw.max(ox + w), sh.max(oy + h)),
        ));
    }
    if frames.is_empty() {
        return Err(ImportError::Invalid("the sheet has no frames".into()));
    }
    let frames: Vec<Frame> = frames.into_iter().map(|(f, _)| f).collect();
    // The box the art fills: the union of every frame's opaque pixels on the sprite's
    // canvas (so the character stands on its lowest pixel, not on an empty margin).
    let mut boxed = [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];
    for f in &frames {
        if let Some([x0, y0, x1, y1]) = opaque_box(&atlas, f) {
            boxed = [
                boxed[0].min(x0),
                boxed[1].min(y0),
                boxed[2].max(x1),
                boxed[3].max(y1),
            ];
        }
    }
    if !boxed[0].is_finite() {
        boxed = [0.0, 0.0, 1.0, 1.0];
    }
    let has_durations = matches!(&v["frames"], Value::Object(m) if m.values().any(|f| f.get("duration").is_some()))
        || matches!(&v["frames"], Value::Array(a) if a.iter().any(|f| f.get("duration").is_some()));
    if !has_durations {
        notes.push(Note::new(
            "dance_import.note.no_durations",
            format!("{DEFAULT_FPS}"),
        ));
    }
    if v["meta"]["slices"]
        .as_array()
        .is_some_and(|s| !s.is_empty())
    {
        notes.push(Note::new("dance_import.note.slices", String::new()));
    }
    if v["meta"]["related_multi_packs"]
        .as_array()
        .is_some_and(|s| !s.is_empty())
    {
        notes.push(Note::new("dance_import.note.multipack", String::new()));
    }

    // The clips.
    let mut named: Vec<(String, Vec<usize>)> = Vec::new();
    if let Some(tags) = v["meta"]["frameTags"].as_array().filter(|t| !t.is_empty()) {
        for t in tags {
            budget.take("tags", 1, limits.tags)?;
            let n = frames.len();
            let from = (t["from"].as_u64().unwrap_or(0) as usize).min(n - 1);
            let to = (t["to"].as_u64().unwrap_or(0) as usize).clamp(from, n - 1);
            let fwd: Vec<usize> = (from..=to).collect();
            let back: Vec<usize> = fwd.iter().rev().copied().collect();
            let inner = |s: &[usize]| s[1..s.len().saturating_sub(1).max(1)].to_vec();
            let mut seq = match t["direction"].as_str().unwrap_or("forward") {
                "reverse" => back.clone(),
                "pingpong" if fwd.len() > 2 => [fwd.clone(), inner(&back)].concat(),
                "pingpong_reverse" if fwd.len() > 2 => [back.clone(), inner(&fwd)].concat(),
                "pingpong" => fwd.clone(),
                "pingpong_reverse" => back.clone(),
                _ => fwd.clone(),
            };
            // `repeat` ("0" or absent: once per cycle; n: n times in one move).
            let repeat = t["repeat"]
                .as_str()
                .and_then(|s| s.parse::<usize>().ok())
                .or_else(|| t["repeat"].as_u64().map(|r| r as usize))
                .unwrap_or(0)
                .clamp(1, 16);
            let once = seq.clone();
            for _ in 1..repeat {
                seq.extend_from_slice(&once);
            }
            budget.take("frames", seq.len(), limits.frames * 4)?;
            let name = limits::name(t["name"].as_str().unwrap_or("tag"), limits);
            named.push((name, seq));
        }
    } else if let Some(anims) = v["animations"].as_object().filter(|a| !a.is_empty()) {
        for (name, list) in anims {
            budget.take("tags", 1, limits.tags)?;
            let seq: Vec<usize> = list
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|f| f.as_str())
                .filter_map(|f| frames.iter().position(|fr| fr.name == f))
                .collect();
            budget.take("frames", seq.len(), limits.frames * 4)?;
            if !seq.is_empty() {
                named.push((limits::name(name, limits), seq));
            }
        }
    } else {
        // Numbered names: `walk_01.png`, `walk_02.png`… share the clip `walk`.
        for (i, f) in frames.iter().enumerate() {
            let stem = f.name.rsplit_once('.').map(|(s, _)| s).unwrap_or(&f.name);
            let key = normalise(stem);
            match named.iter_mut().find(|(k, _)| normalise(k) == key) {
                Some((_, seq)) => seq.push(i),
                None => {
                    budget.take("tags", 1, limits.tags)?;
                    let display =
                        stem.trim_end_matches(|c: char| c.is_ascii_digit() || "_- .".contains(c));
                    named.push((
                        if display.is_empty() { stem } else { display }.to_string(),
                        vec![i],
                    ));
                }
            }
        }
        if named.len() == frames.len() && frames.len() > 1 {
            // Every frame its own name: one clip of all of them.
            named = vec![("all".to_string(), (0..frames.len()).collect())];
        }
    }
    let mut clips = Vec::new();
    let mut seqs = Vec::new();
    for (name, seq) in named {
        let mut at = 0.0f32;
        let timed: Vec<(usize, f32)> = seq
            .iter()
            .map(|&i| {
                let s = at;
                at += frames[i].seconds;
                (i, s)
            })
            .collect();
        clips.push(Clip {
            name,
            seconds: at.max(1e-3),
            frames: seq.len() as u32,
        });
        seqs.push(timed);
    }
    let fps = if has_durations {
        let total: f32 = frames.iter().map(|f| f.seconds).sum();
        (frames.len() as f32 / total.max(1e-3)).clamp(1.0, 120.0)
    } else {
        DEFAULT_FPS
    };
    let facts = vec![
        ("dance_import.fact.frames", frames.len().to_string()),
        (
            "dance_import.fact.image",
            format!("{} × {}", atlas.width(), atlas.height()),
        ),
    ];
    let sheet = Sheet {
        format,
        atlas: Arc::new(atlas),
        frames,
        clips,
        seqs,
        boxed,
        fps,
    };
    let author = String::new();
    let default_filter = if format == "Aseprite" {
        Filter::Nearest
    } else {
        Filter::Bilinear
    };
    Ok(Found {
        source: Box::new(sheet),
        name: String::new(),
        author,
        licence: Licence::Unknown,
        notes,
        facts,
        files: vec![image_name.to_string()],
        default_filter,
    })
}

/// A frame's opaque pixels as a box on the sprite's canvas, if it has any.
fn opaque_box(atlas: &Pixmap, f: &Frame) -> Option<[f32; 4]> {
    let r = f.region;
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for y in r.y0..r.y1 {
        for x in r.x0..r.x1 {
            if atlas
                .pixel(x as u32, y as u32)
                .is_some_and(|p| p.alpha() > 0)
            {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    if x1 <= x0 {
        return None;
    }
    // Atlas → sprite: upright, or a quarter turn back for a rotated frame
    // (atlas u = x0 + h − sy, v = y0 + sx).
    let (ax0, ay0, ax1, ay1) = (
        (x0 - r.x0) as f32,
        (y0 - r.y0) as f32,
        (x1 - r.x0) as f32,
        (y1 - r.y0) as f32,
    );
    let (sx0, sy0, sx1, sy1) = if f.rotated {
        (ay0, f.h - ax1, ay1, f.h - ax0)
    } else {
        (ax0, ay0, ax1, ay1)
    };
    Some([f.ox + sx0, f.oy + sy0, f.ox + sx1, f.oy + sy1])
}

impl Sheet {
    /// The frame clip `clip` shows at `t` seconds.
    fn frame_at(&self, clip: usize, t: f32) -> Option<&Frame> {
        let seq = self.seqs.get(clip)?;
        let i = match seq
            .binary_search_by(|(_, s)| s.partial_cmp(&t).unwrap_or(std::cmp::Ordering::Less))
        {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        seq.get(i).and_then(|(f, _)| self.frames.get(*f))
    }
}

impl Source for Sheet {
    fn format(&self) -> &'static str {
        self.format
    }

    fn clips(&self) -> &[Clip] {
        &self.clips
    }

    fn bounds(&self) -> Bounds {
        let [left, top, right, bottom] = self.boxed;
        Bounds {
            left,
            top,
            right,
            bottom,
        }
    }

    fn frame_rate(&self) -> f32 {
        self.fps
    }

    fn draw(&self, clip: usize, t: f32, filter: Filter, canvas: &mut Canvas) {
        let Some(f) = self.frame_at(clip, t) else {
            return;
        };
        let atlas = &self.atlas;
        let tint = Tint::NONE.hue(canvas.hue);
        let m = canvas.transform;
        let p = |x: f32, y: f32| raster::map(m, f.ox + x, f.oy + y);
        let r = f.region;
        let (x0, y0, x1, y1) = (r.x0 as f32, r.y0 as f32, r.x1 as f32, r.y1 as f32);
        // The sprite's corners (upright) and where each samples the atlas.
        let uv = if f.rotated {
            // Stored a quarter turn clockwise: the top-left corner is at the atlas's top
            // right.
            [(x1, y0), (x1, y1), (x0, y1), (x0, y0)]
        } else {
            [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
        };
        let xy = [p(0.0, 0.0), p(f.w, 0.0), p(f.w, f.h), p(0.0, f.h)];
        let v = |k: usize| Vertex {
            x: xy[k].0,
            y: xy[k].1,
            u: uv[k].0,
            v: uv[k].1,
        };
        raster::triangle(canvas.pixmap, atlas, [v(0), v(1), v(2)], r, filter, tint);
        raster::triangle(canvas.pixmap, atlas, [v(0), v(2), v(3)], r, filter, tint);
    }
}
