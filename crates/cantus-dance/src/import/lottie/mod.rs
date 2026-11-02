//! Lottie (ADR 0089 §2): our own player over tiny-skia — no dependency, every size
//! bounded, expressions never run (the keyframes play; the card says so). `.lottie`
//! packages (a zip: a manifest, animations, images) are read from their central
//! directory with the entries' sizes checked before anything is inflated.

pub mod draw;
pub mod geom;
pub mod model;
pub mod prop;

use super::imported::Source;
use super::limits::{self, Limits};
use super::mapping::Clip;
use super::raster::Filter;
use super::{Found, ImportError, Note};
use crate::character::{Bounds, Licence};
use crate::render::Canvas;
use model::{Doc, Files};
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use tiny_skia::Pixmap;

/// Whether a JSON document is a Lottie animation.
pub fn is_lottie(v: &Value) -> bool {
    v["layers"].is_array() && (v.get("v").is_some() || v.get("fr").is_some())
}

/// A Lottie animation as a source of clips.
pub struct Lottie {
    doc: Doc,
    clips: Vec<Clip>,
    /// Each clip's first frame.
    starts: Vec<f32>,
    boxed: [f32; 4],
    heavy: bool,
}

/// Paint calls above which frames are cached at the size shown (decided from the file).
pub const HEAVY_PAINTS: usize = 120;

impl Lottie {
    pub fn new(doc: Doc) -> Lottie {
        let fr = doc.fr;
        let mut clips = Vec::new();
        let mut starts = Vec::new();
        let mut marks: Vec<&(String, f32, f32)> = doc
            .markers
            .iter()
            .filter(|m| m.1 >= doc.ip && m.1 < doc.op && !m.0.is_empty())
            .collect();
        marks.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        for (k, m) in marks.iter().enumerate() {
            let next = marks.get(k + 1).map(|n| n.1).unwrap_or(doc.op);
            let frames = if m.2 > 0.0 {
                m.2.min(doc.op - m.1)
            } else {
                next - m.1
            };
            if frames <= 0.0 {
                continue;
            }
            clips.push(Clip {
                name: m.0.clone(),
                seconds: frames / fr,
                frames: frames.round().max(1.0) as u32,
            });
            starts.push(m.1);
        }
        if clips.is_empty() {
            let frames = doc.op - doc.ip;
            clips.push(Clip {
                name: if doc.name.is_empty() {
                    "all".into()
                } else {
                    doc.name.clone()
                },
                seconds: frames / fr,
                frames: frames.round().max(1.0) as u32,
            });
            starts.push(doc.ip);
        }
        let heavy = doc.paints > HEAVY_PAINTS || doc.has_mattes;
        let mut l = Lottie {
            doc,
            clips,
            starts,
            boxed: [0.0; 4],
            heavy,
        };
        l.boxed = l.measure();
        l
    }

    /// The box every clip draws in (sampled), or the composition's.
    fn measure(&self) -> [f32; 4] {
        let mut b = [
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ];
        for (k, c) in self.clips.iter().enumerate() {
            let n = 16;
            for s in 0..=n {
                let f = self.starts[k] + c.seconds * self.doc.fr * s as f32 / n as f32;
                let mut frame = draw::Frame::new(&self.doc, 0.0, Filter::Bilinear);
                let mut target = draw::Target::Bounds(&mut b);
                frame.layers(
                    &self.doc.layers,
                    f.min(self.doc.op - 1e-3),
                    tiny_skia::Transform::identity(),
                    1.0,
                    None,
                    &mut target,
                    0,
                );
            }
        }
        let ok = b.iter().all(|v| v.is_finite()) && b[2] > b[0] && b[3] > b[1];
        if ok {
            b
        } else {
            [0.0, 0.0, self.doc.w, self.doc.h]
        }
    }

    pub fn doc(&self) -> &Doc {
        &self.doc
    }

    /// Draw the composition's frame `f` onto `pixmap` through `t` (tests and previews).
    pub fn draw_frame(&self, f: f32, t: tiny_skia::Transform, hue: f32, pixmap: &mut Pixmap) {
        let mut frame = draw::Frame::new(&self.doc, hue, Filter::Bilinear);
        let mut target = draw::Target::Pixels(pixmap);
        frame.layers(&self.doc.layers, f, t, 1.0, None, &mut target, 0);
    }
}

impl Source for Lottie {
    fn format(&self) -> &'static str {
        "Lottie"
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

    fn draw(&self, clip: usize, t: f32, filter: Filter, canvas: &mut Canvas) {
        let Some(start) = self.starts.get(clip) else {
            return;
        };
        let f = (start + t * self.doc.fr).min(self.doc.op - 1e-3);
        let mut frame = draw::Frame::new(&self.doc, canvas.hue, filter);
        let mut target = draw::Target::Pixels(canvas.pixmap);
        frame.layers(
            &self.doc.layers,
            f,
            canvas.transform,
            1.0,
            None,
            &mut target,
            0,
        );
    }

    fn heavy(&self) -> bool {
        self.heavy
    }

    fn frame_rate(&self) -> f32 {
        self.doc.fr
    }
}

fn found(
    doc: Doc,
    files: Vec<String>,
    extra: Vec<Note>,
    licence: Licence,
    author: String,
) -> Found {
    let mut notes = extra;
    notes.extend(doc.notes.iter().cloned());
    let facts = vec![
        ("dance_import.fact.layers", doc.layers.len().to_string()),
        ("dance_import.fact.frame_rate", format!("{}", doc.fr)),
        (
            "dance_import.fact.length",
            format!("{:.2} s", (doc.op - doc.ip) / doc.fr),
        ),
        ("dance_import.fact.size", format!("{} × {}", doc.w, doc.h)),
    ];
    let name = doc.name.clone();
    let author = if author.is_empty() {
        doc.author.clone()
    } else {
        author
    };
    Found {
        source: Box::new(Lottie::new(doc)),
        name,
        author,
        licence,
        notes,
        facts,
        files,
        default_filter: Filter::Bilinear,
    }
}

/// A Lottie JSON beside its images in `folder`.
pub fn read(v: &Value, folder: &Path, limits: &Limits) -> Result<Found, ImportError> {
    let files = model::image_files(v);
    let doc = model::read(v, Files::Folder(folder), limits)?;
    let licence = stated_licence(v);
    Ok(found(doc, files, Vec::new(), licence, String::new()))
}

/// A licence a Lottie file states in its metadata (`meta.k` keywords or `meta.d`
/// description naming a licence Cantus recognises).
fn stated_licence(v: &Value) -> Licence {
    for key in ["k", "d", "license", "licence"] {
        let text = match &v["meta"][key] {
            Value::String(s) => s.clone(),
            Value::Array(a) => a
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" "),
            _ => continue,
        };
        if let Some(id) = super::package::recognise(&text).or_else(|| spdx_in(&text)) {
            return Licence::Stated {
                text: id.to_string(),
                source: "meta".into(),
            };
        }
    }
    Licence::Unknown
}

fn spdx_in(text: &str) -> Option<&'static str> {
    let t = text.to_ascii_uppercase();
    ["CC0-1.0", "CC-BY-SA-4.0", "CC-BY-4.0", "MIT", "APACHE-2.0"]
        .into_iter()
        .find(|id| t.contains(id))
        .map(|id| match id {
            "APACHE-2.0" => "Apache-2.0",
            other => other,
        })
}

/// A `.lottie` package (a zip): its manifest's first animation and the images it holds.
pub fn read_package(bytes: &[u8], limits: &Limits) -> Result<Found, ImportError> {
    if bytes.len() as u64 > limits.zip_bytes {
        return Err(ImportError::TooLarge(format!(
            "a package of {} bytes, at most {}",
            bytes.len(),
            limits.zip_bytes
        )));
    }
    let bad = |e: zip::result::ZipError| ImportError::Invalid(format!(".lottie: {e}"));
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(bad)?;
    if zip.len() > limits.zip_entries {
        return Err(ImportError::TooLarge(format!(
            "{} entries in the package, at most {}",
            zip.len(),
            limits.zip_entries
        )));
    }
    // What it says it holds, before anything is inflated.
    let mut declared = 0u64;
    let mut names = Vec::new();
    for i in 0..zip.len() {
        let e = zip.by_index_raw(i).map_err(bad)?;
        declared = declared.saturating_add(e.size());
        if let Some(name) = e.enclosed_name() {
            let name = name.to_string_lossy().replace('\\', "/");
            if !e.is_dir() {
                names.push((i, name, e.size()));
            }
        }
    }
    if declared > limits.zip_inflated {
        return Err(ImportError::TooLarge(format!(
            "{declared} bytes once unpacked, at most {}",
            limits.zip_inflated
        )));
    }
    let mut read_entry = |i: usize, size: u64| -> Result<Vec<u8>, ImportError> {
        let e = zip.by_index(i).map_err(bad)?;
        let mut out = Vec::with_capacity(size.min(limits.zip_inflated) as usize);
        e.take(size.saturating_add(1)).read_to_end(&mut out)?;
        if out.len() as u64 > size {
            return Err(ImportError::Invalid("an entry larger than it says".into()));
        }
        Ok(out)
    };
    let mut notes = Vec::new();
    let manifest = match names.iter().find(|n| n.1 == "manifest.json") {
        Some(&(i, _, size)) => {
            let b = read_entry(i, size)?;
            limits::parse_json(&b, limits).unwrap_or(Value::Null)
        }
        None => Value::Null,
    };
    let anims: Vec<&(usize, String, u64)> = names
        .iter()
        .filter(|n| {
            (n.1.starts_with("animations/") || n.1.starts_with("a/")) && n.1.ends_with(".json")
        })
        .collect();
    let wanted = manifest["initial"]["animation"]
        .as_str()
        .or_else(|| manifest["activeAnimationId"].as_str())
        .or_else(|| manifest["animations"][0]["id"].as_str());
    let pick = wanted
        .and_then(|id| anims.iter().find(|n| n.1.ends_with(&format!("/{id}.json"))))
        .or_else(|| anims.first())
        .ok_or_else(|| ImportError::Invalid("the package holds no animation".into()))?;
    if anims.len() > 1 {
        notes.push(Note::new(
            "dance_import.note.first_animation",
            (anims.len() - 1).to_string(),
        ));
    }
    if names
        .iter()
        .any(|n| n.1.starts_with("s/") || n.1.starts_with("states/"))
    {
        notes.push(Note::new("dance_import.note.state_machines", String::new()));
    }
    if names
        .iter()
        .any(|n| n.1.starts_with("t/") || n.1.starts_with("themes/"))
    {
        notes.push(Note::new("dance_import.note.themes", String::new()));
    }
    let (pi, psize) = (pick.0, pick.2);
    let json = read_entry(pi, psize)?;
    let v = limits::parse_json(&json, limits)?;
    if !is_lottie(&v) {
        return Err(ImportError::Invalid(
            "the package's animation is not Lottie".into(),
        ));
    }
    // Its images, by their path in the package.
    let mut entries = HashMap::new();
    let mut image_bytes = 0u64;
    for (i, name, size) in &names {
        if name.starts_with("images/") || name.starts_with("i/") {
            image_bytes += size;
            if image_bytes > limits.zip_inflated {
                break;
            }
            entries.insert(name.clone(), read_entry(*i, *size)?);
        }
    }
    let doc = model::read(&v, Files::Package(&entries), limits)?;
    let mut licence = stated_licence(&v);
    if licence == Licence::Unknown {
        for key in ["license", "licence"] {
            if let Some(text) = manifest[key].as_str().filter(|s| !s.trim().is_empty()) {
                licence = Licence::Stated {
                    text: super::package::recognise(text)
                        .map(str::to_string)
                        .unwrap_or_else(|| limits::name(text.trim(), limits)),
                    source: "manifest.json".into(),
                };
            }
        }
    }
    let author = manifest["author"].as_str().unwrap_or("").to_string();
    Ok(found(
        doc,
        Vec::new(),
        notes,
        licence,
        limits::name(&author, limits),
    ))
}
