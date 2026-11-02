//! Lottie's document as our model, read once with every count bounded.

use super::super::image::{self, PixelBudget};
use super::super::limits::{self, Budget, Limits};
use super::super::{paths, ImportError, Note};
use super::prop::{has_expression, num, nums, PathProp, Prop};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use tiny_skia::Pixmap;

/// A transform (a layer's `ks`, a group's `tr`, a repeater's).
#[derive(Clone, Debug, PartialEq)]
pub struct Xform {
    pub anchor: Prop,
    pub position: Prop,
    /// A split position (`p.s`): x and y animated apart.
    pub split: Option<(Prop, Prop)>,
    pub scale: Prop,
    pub rotation: Prop,
    pub skew: Prop,
    pub skew_axis: Prop,
    pub opacity: Prop,
    /// A repeater's start and end opacity.
    pub start_opacity: Prop,
    pub end_opacity: Prop,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    pub width: Prop,
    pub cap: u8,
    pub join: u8,
    pub miter: f32,
    /// Dashes: (`d`ash or `g`ap value) in order, and the offset.
    pub dashes: Vec<Prop>,
    pub dash_offset: Option<Prop>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Gradient {
    /// 1 linear, 2 radial.
    pub kind: u8,
    pub stops: usize,
    pub colors: Prop,
    pub start: Prop,
    pub end: Prop,
    pub highlight: Prop,
    pub angle: Prop,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Group(Vec<Shape>, Xform),
    Path(PathProp),
    Rect {
        p: Prop,
        s: Prop,
        r: Prop,
    },
    Ellipse {
        p: Prop,
        s: Prop,
    },
    Star {
        star: bool,
        p: Prop,
        points: Prop,
        rotation: Prop,
        outer: Prop,
        inner: Prop,
    },
    Fill {
        color: Prop,
        opacity: Prop,
        even_odd: bool,
    },
    Stroke {
        color: Prop,
        opacity: Prop,
        stroke: Stroke,
    },
    GradientFill {
        g: Gradient,
        opacity: Prop,
        even_odd: bool,
    },
    GradientStroke {
        g: Gradient,
        opacity: Prop,
        stroke: Stroke,
    },
    Trim {
        start: Prop,
        end: Prop,
        offset: Prop,
        individually: bool,
    },
    Repeater {
        copies: Prop,
        offset: Prop,
        above: bool,
        tr: Xform,
    },
    RoundCorners(Prop),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Mask {
    /// `a` add, `s` subtract, `i` intersect, `n` none (and the rest: as add).
    pub mode: char,
    pub inverted: bool,
    pub path: PathProp,
    pub opacity: Prop,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Content {
    Shapes(Vec<Shape>),
    Null,
    Precomp { asset: usize, w: f32, h: f32 },
    Image(usize),
    Solid { color: [f32; 3], w: f32, h: f32 },
    Text(Box<Text>),
}

/// A text layer drawn from the glyph shapes the file carries.
#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    pub text: String,
    pub size: f32,
    pub font: String,
    pub fill: [f32; 3],
    pub justify: u8,
    pub tracking: f32,
    pub line_height: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub index: Option<i64>,
    pub parent: Option<i64>,
    pub content: Content,
    pub ip: f32,
    pub op: f32,
    pub st: f32,
    pub stretch: f32,
    pub ks: Xform,
    pub masks: Vec<Mask>,
    /// Matted by the layer above it (or `tp`): 1 alpha, 2 inverted alpha, 3 luma, 4
    /// inverted luma.
    pub matte: u8,
    pub matte_parent: Option<i64>,
    /// A matte source: not drawn itself.
    pub is_matte: bool,
    pub hidden: bool,
    pub blend: u8,
    pub time_remap: Option<Prop>,
}

/// A glyph: its character, font, advance (font units at size 100) and shapes.
#[derive(Clone, Debug, PartialEq)]
pub struct Glyph {
    pub ch: String,
    pub font: String,
    pub advance: f32,
    pub shapes: Vec<Shape>,
}

pub struct Doc {
    pub w: f32,
    pub h: f32,
    pub fr: f32,
    pub ip: f32,
    pub op: f32,
    pub layers: Vec<Layer>,
    pub precomps: Vec<Vec<Layer>>,
    pub images: Vec<Option<Pixmap>>,
    pub markers: Vec<(String, f32, f32)>,
    pub glyphs: Vec<Glyph>,
    pub name: String,
    pub author: String,
    pub notes: Vec<Note>,
    /// Paint calls a frame makes, about (what "heavy" is decided from).
    pub paints: usize,
    pub has_mattes: bool,
}

/// Where a document's images come from: a folder, or a package's entries.
pub enum Files<'a> {
    Folder(&'a Path),
    Package(&'a HashMap<String, Vec<u8>>),
}

struct Reader<'a> {
    limits: &'a Limits,
    budget: Budget,
    pixels: PixelBudget,
    expressions: usize,
    ignored: Vec<&'static str>,
    paints: usize,
    files: Files<'a>,
}

impl Reader<'_> {
    fn prop(&mut self, v: &Value, key: &str, default: &[f32]) -> Result<Prop, ImportError> {
        let p = v.get(key);
        if p.is_some_and(has_expression) {
            self.expressions += 1;
        }
        Prop::parse(p, default, &mut self.budget, self.limits)
    }

    fn ignore(&mut self, what: &'static str) {
        if !self.ignored.contains(&what) {
            self.ignored.push(what);
        }
    }

    fn xform(&mut self, v: &Value) -> Result<Xform, ImportError> {
        let p = &v["p"];
        let split =
            if p["s"].as_bool() == Some(true) || (p.get("x").is_some() && p.get("k").is_none()) {
                Some((self.prop(p, "x", &[0.0])?, self.prop(p, "y", &[0.0])?))
            } else {
                None
            };
        // A 3D layer's rotation: `rz` (or `or`'s z) turns it; `rx`/`ry` are ignored.
        let rotation = if v.get("r").is_some() {
            self.prop(v, "r", &[0.0])?
        } else if v.get("rz").is_some() {
            if v.get("rx").is_some() || v.get("ry").is_some() {
                self.ignore("dance_import.note.three_d");
            }
            self.prop(v, "rz", &[0.0])?
        } else {
            Prop::Static(vec![0.0])
        };
        Ok(Xform {
            anchor: self.prop(v, "a", &[0.0, 0.0])?,
            position: if split.is_some() {
                Prop::Static(vec![0.0, 0.0])
            } else {
                self.prop(v, "p", &[0.0, 0.0])?
            },
            split,
            scale: self.prop(v, "s", &[100.0, 100.0])?,
            rotation,
            skew: self.prop(v, "sk", &[0.0])?,
            skew_axis: self.prop(v, "sa", &[0.0])?,
            opacity: self.prop(v, "o", &[100.0])?,
            start_opacity: self.prop(v, "so", &[100.0])?,
            end_opacity: self.prop(v, "eo", &[100.0])?,
        })
    }

    fn stroke(&mut self, v: &Value) -> Result<Stroke, ImportError> {
        let mut dashes = Vec::new();
        let mut dash_offset = None;
        for d in v["d"].as_array().into_iter().flatten() {
            let p = self.prop(d, "v", &[0.0])?;
            match d["n"].as_str() {
                Some("o") => dash_offset = Some(p),
                _ => dashes.push(p),
            }
        }
        Ok(Stroke {
            width: self.prop(v, "w", &[1.0])?,
            cap: v["lc"].as_u64().unwrap_or(2) as u8,
            join: v["lj"].as_u64().unwrap_or(2) as u8,
            miter: num(&v["ml"]).max(1.0),
            dashes,
            dash_offset,
        })
    }

    fn gradient(&mut self, v: &Value) -> Result<Gradient, ImportError> {
        let g = &v["g"];
        Ok(Gradient {
            kind: v["t"].as_u64().unwrap_or(1) as u8,
            stops: (g["p"].as_u64().unwrap_or(0) as usize).min(256),
            colors: self.prop(g, "k", &[])?,
            start: self.prop(v, "s", &[0.0, 0.0])?,
            end: self.prop(v, "e", &[0.0, 0.0])?,
            highlight: self.prop(v, "h", &[0.0])?,
            angle: self.prop(v, "a", &[0.0])?,
        })
    }

    fn shapes(&mut self, list: &Value, depth: usize) -> Result<Vec<Shape>, ImportError> {
        if depth > self.limits.json_depth {
            return Err(ImportError::TooLarge("shape groups nested too deep".into()));
        }
        let mut out = Vec::new();
        for it in list.as_array().into_iter().flatten() {
            self.budget
                .take("shape items", 1, self.limits.shape_items)?;
            if it["hd"].as_bool() == Some(true) {
                continue;
            }
            let ty = it["ty"].as_str().unwrap_or("");
            let shape = match ty {
                "gr" => {
                    let items = &it["it"];
                    let tr = items
                        .as_array()
                        .into_iter()
                        .flatten()
                        .find(|x| x["ty"] == "tr")
                        .cloned()
                        .unwrap_or(Value::Null);
                    Shape::Group(self.shapes(items, depth + 1)?, self.xform(&tr)?)
                }
                "sh" => {
                    if it["ks"].get("x").is_some_and(Value::is_string) {
                        self.expressions += 1;
                    }
                    Shape::Path(PathProp::parse(&it["ks"], &mut self.budget, self.limits)?)
                }
                "rc" => Shape::Rect {
                    p: self.prop(it, "p", &[0.0, 0.0])?,
                    s: self.prop(it, "s", &[0.0, 0.0])?,
                    r: self.prop(it, "r", &[0.0])?,
                },
                "el" => Shape::Ellipse {
                    p: self.prop(it, "p", &[0.0, 0.0])?,
                    s: self.prop(it, "s", &[0.0, 0.0])?,
                },
                "sr" => {
                    if it.get("os").is_some() || it.get("is").is_some() {
                        let round =
                            |v: &Value| nums(&v["k"]).first().copied().unwrap_or(0.0) != 0.0;
                        if round(&it["os"]) || round(&it["is"]) {
                            self.ignore("dance_import.note.star_roundness");
                        }
                    }
                    Shape::Star {
                        star: it["sy"].as_u64().unwrap_or(1) == 1,
                        p: self.prop(it, "p", &[0.0, 0.0])?,
                        points: self.prop(it, "pt", &[5.0])?,
                        rotation: self.prop(it, "r", &[0.0])?,
                        outer: self.prop(it, "or", &[0.0])?,
                        inner: self.prop(it, "ir", &[0.0])?,
                    }
                }
                "fl" => {
                    self.paints += 1;
                    Shape::Fill {
                        color: self.prop(it, "c", &[0.0, 0.0, 0.0, 1.0])?,
                        opacity: self.prop(it, "o", &[100.0])?,
                        even_odd: it["r"].as_u64() == Some(2),
                    }
                }
                "st" => {
                    self.paints += 1;
                    Shape::Stroke {
                        color: self.prop(it, "c", &[0.0, 0.0, 0.0, 1.0])?,
                        opacity: self.prop(it, "o", &[100.0])?,
                        stroke: self.stroke(it)?,
                    }
                }
                "gf" => {
                    self.paints += 1;
                    Shape::GradientFill {
                        g: self.gradient(it)?,
                        opacity: self.prop(it, "o", &[100.0])?,
                        even_odd: it["r"].as_u64() == Some(2),
                    }
                }
                "gs" => {
                    self.paints += 1;
                    Shape::GradientStroke {
                        g: self.gradient(it)?,
                        opacity: self.prop(it, "o", &[100.0])?,
                        stroke: self.stroke(it)?,
                    }
                }
                "tm" => Shape::Trim {
                    start: self.prop(it, "s", &[0.0])?,
                    end: self.prop(it, "e", &[100.0])?,
                    offset: self.prop(it, "o", &[0.0])?,
                    individually: it["m"].as_u64() == Some(2),
                },
                "rp" => {
                    let tr = it.get("tr").cloned().unwrap_or(Value::Null);
                    Shape::Repeater {
                        copies: self.prop(it, "c", &[1.0])?,
                        offset: self.prop(it, "o", &[0.0])?,
                        above: it["m"].as_u64() == Some(2),
                        tr: self.xform(&tr)?,
                    }
                }
                "rd" => Shape::RoundCorners(self.prop(it, "r", &[0.0])?),
                "tr" => continue,
                "mm" => {
                    self.ignore("dance_import.note.merge_paths");
                    continue;
                }
                "pb" | "tw" | "zz" | "op" => {
                    self.ignore("dance_import.note.path_effects");
                    continue;
                }
                _ => continue,
            };
            out.push(shape);
        }
        Ok(out)
    }

    fn layers(
        &mut self,
        list: &Value,
        assets: &HashMap<String, usize>,
        images: &HashMap<String, usize>,
    ) -> Result<Vec<Layer>, ImportError> {
        let mut out = Vec::new();
        for l in list.as_array().into_iter().flatten() {
            self.budget.take("layers", 1, self.limits.layers)?;
            let ty = l["ty"].as_u64().unwrap_or(99);
            let content = match ty {
                4 => Content::Shapes(self.shapes(&l["shapes"], 0)?),
                3 => Content::Null,
                0 => match l["refId"].as_str().and_then(|r| assets.get(r)) {
                    Some(&asset) => Content::Precomp {
                        asset,
                        w: num(&l["w"]),
                        h: num(&l["h"]),
                    },
                    None => Content::Null,
                },
                2 => match l["refId"].as_str().and_then(|r| images.get(r)) {
                    Some(&i) => Content::Image(i),
                    None => Content::Null,
                },
                1 => Content::Solid {
                    color: hex(l["sc"].as_str().unwrap_or("#000000")),
                    w: num(&l["sw"]),
                    h: num(&l["sh"]),
                },
                5 => match self.text(l) {
                    Some(t) => Content::Text(Box::new(t)),
                    None => Content::Null,
                },
                6 => {
                    self.ignore("dance_import.note.audio");
                    Content::Null
                }
                _ => Content::Null,
            };
            if l["ef"].as_array().is_some_and(|e| !e.is_empty()) {
                self.ignore("dance_import.note.effects");
            }
            let mut masks = Vec::new();
            for m in l["masksProperties"].as_array().into_iter().flatten() {
                self.budget.take("masks", 1, self.limits.masks)?;
                if num(&m["f"]["k"]) != 0.0 || num(&m["x"]["k"]) != 0.0 {
                    self.ignore("dance_import.note.mask_feather");
                }
                masks.push(Mask {
                    mode: m["mode"]
                        .as_str()
                        .and_then(|s| s.chars().next())
                        .unwrap_or('a'),
                    inverted: m["inv"].as_bool().unwrap_or(false),
                    path: PathProp::parse(&m["pt"], &mut self.budget, self.limits)?,
                    opacity: self.prop(m, "o", &[100.0])?,
                });
            }
            let blend = l["bm"].as_u64().unwrap_or(0).min(255) as u8;
            if blend == 17 {
                self.ignore("dance_import.note.blend");
            }
            let matte = l["tt"].as_u64().unwrap_or(0).min(4) as u8;
            out.push(Layer {
                index: l["ind"].as_i64(),
                parent: l["parent"].as_i64(),
                content,
                ip: num(&l["ip"]),
                op: l["op"].as_f64().map(|_| num(&l["op"])).unwrap_or(1e7),
                st: num(&l["st"]),
                stretch: l["sr"]
                    .as_f64()
                    .map(|_| num(&l["sr"]))
                    .filter(|s| *s != 0.0)
                    .unwrap_or(1.0),
                ks: self.xform(&l["ks"])?,
                masks,
                matte,
                matte_parent: l["tp"].as_i64(),
                is_matte: l["td"].as_u64() == Some(1),
                hidden: l["hd"].as_bool() == Some(true),
                blend,
                time_remap: match l.get("tm") {
                    Some(_) => Some(self.prop(l, "tm", &[0.0])?),
                    None => None,
                },
            });
        }
        Ok(out)
    }

    fn text(&mut self, l: &Value) -> Option<Text> {
        let d = &l["t"]["d"]["k"];
        let s = d
            .as_array()
            .and_then(|k| k.first())
            .map(|k| &k["s"])
            .unwrap_or(&Value::Null);
        if l["t"]["a"].as_array().is_some_and(|a| !a.is_empty()) {
            self.ignore("dance_import.note.text_animators");
        }
        let text = s["t"].as_str()?;
        Some(Text {
            text: limits::name(
                text,
                &Limits {
                    name_bytes: 4096,
                    ..*self.limits
                },
            ),
            size: num(&s["s"]).clamp(0.0, 10_000.0),
            font: s["f"].as_str().unwrap_or("").to_string(),
            fill: {
                let c = nums(&s["fc"]);
                [
                    c.first().copied().unwrap_or(0.0),
                    c.get(1).copied().unwrap_or(0.0),
                    c.get(2).copied().unwrap_or(0.0),
                ]
            },
            justify: s["j"].as_u64().unwrap_or(0) as u8,
            tracking: num(&s["tr"]),
            line_height: num(&s["lh"]),
        })
    }

    fn image(&mut self, a: &Value) -> Option<Pixmap> {
        let p = a["p"].as_str()?;
        let bytes = if let Some(data) = image::data_uri(p) {
            match data {
                Ok(b) => b,
                Err(_) => {
                    self.ignore("dance_import.note.image_unreadable");
                    return None;
                }
            }
        } else if a["e"].as_u64() == Some(1) {
            self.ignore("dance_import.note.image_unreadable");
            return None;
        } else {
            let dir = a["u"].as_str().unwrap_or("");
            if dir.contains("://") || p.contains("://") {
                self.ignore("dance_import.note.image_url");
                return None;
            }
            let rel = format!("{dir}{p}");
            let read = match &self.files {
                Files::Folder(folder) => paths::read_inside(folder, &rel, self.limits.image_bytes),
                Files::Package(entries) => {
                    // A package names its images from its root (`/images/x.png`).
                    paths::relative_parts(rel.trim_start_matches('/')).and_then(|parts| {
                        let key = parts.join("/");
                        let last = parts.last().cloned().unwrap_or_default();
                        entries
                            .get(&key)
                            .or_else(|| entries.get(&format!("images/{last}")))
                            .or_else(|| entries.get(&format!("i/{last}")))
                            .cloned()
                            .ok_or_else(|| ImportError::Invalid(format!("no {key} in the package")))
                    })
                }
            };
            match read {
                Ok(b) => b,
                Err(_) => {
                    self.ignore("dance_import.note.image_unreadable");
                    return None;
                }
            }
        };
        match image::decode(&bytes, self.limits, &mut self.pixels) {
            Ok(img) => Some(img),
            Err(_) => {
                self.ignore("dance_import.note.image_unreadable");
                None
            }
        }
    }
}

/// `#rrggbb` as 0…1.
pub fn hex(s: &str) -> [f32; 3] {
    let s = s.trim_start_matches('#');
    let c = |i: usize| {
        s.get(i..i + 2)
            .and_then(|h| u8::from_str_radix(h, 16).ok())
            .map(|v| f32::from(v) / 255.0)
            .unwrap_or(0.0)
    };
    [c(0), c(2), c(4)]
}

/// The relative paths of the images a document names outside itself (to copy with it).
pub fn image_files(v: &Value) -> Vec<String> {
    v["assets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|a| a.get("p").is_some() && a.get("layers").is_none())
        .filter(|a| a["e"].as_u64() != Some(1))
        .filter_map(|a| {
            let p = a["p"].as_str()?;
            if p.starts_with("data:") || p.contains("://") {
                return None;
            }
            let rel = format!("{}{}", a["u"].as_str().unwrap_or(""), p);
            paths::relative_parts(&rel)
                .ok()
                .map(|parts| parts.join("/"))
        })
        .collect()
}

/// Read a document.
pub fn read(v: &Value, files: Files<'_>, limits: &Limits) -> Result<Doc, ImportError> {
    let mut r = Reader {
        limits,
        budget: Budget::new(),
        pixels: PixelBudget::default(),
        expressions: 0,
        ignored: Vec::new(),
        paints: 0,
        files,
    };
    let fr = num(&v["fr"]);
    let fr = if fr.is_finite() && fr > 0.0 {
        fr.clamp(1.0, 240.0)
    } else {
        30.0
    };
    let (ip, op) = (num(&v["ip"]), num(&v["op"]));
    if op.partial_cmp(&ip) != Some(std::cmp::Ordering::Greater) {
        return Err(ImportError::Invalid(
            "the animation has no length (op ≤ ip)".into(),
        ));
    }
    if f64::from(op - ip) > limits.timeline_frames {
        return Err(ImportError::TooLarge(format!(
            "a timeline of {} frames, at most {}",
            op - ip,
            limits.timeline_frames
        )));
    }
    // Assets: precomps get an index first (layers name them), images are decoded.
    let mut asset_ids = HashMap::new();
    let mut image_ids = HashMap::new();
    let assets = v["assets"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let mut images = Vec::new();
    for a in assets {
        let Some(id) = a["id"].as_str() else { continue };
        // Two assets of one id: the first is kept (a layer names one), and said.
        if asset_ids.contains_key(id) || image_ids.contains_key(id) {
            r.ignore("dance_import.note.duplicate_ids");
            continue;
        }
        if a.get("layers").is_some() {
            let n = asset_ids.len();
            asset_ids.insert(id.to_string(), n);
        } else if a.get("p").is_some() {
            image_ids.insert(id.to_string(), images.len());
            images.push(r.image(a));
        }
    }
    let mut precomps = vec![Vec::new(); asset_ids.len()];
    let mut seen = std::collections::HashSet::new();
    for a in assets {
        if let (Some(id), Some(_)) = (a["id"].as_str(), a.get("layers")) {
            if !seen.insert(id) {
                continue;
            }
            if let Some(&n) = asset_ids.get(id) {
                precomps[n] = r.layers(&a["layers"], &asset_ids, &image_ids)?;
            }
        }
    }
    let layers = r.layers(&v["layers"], &asset_ids, &image_ids)?;
    check_precomps(&layers, &precomps, limits)?;
    let marker_count = v["markers"].as_array().map(Vec::len).unwrap_or(0);
    r.budget.take("markers", marker_count, limits.markers)?;
    let markers = v["markers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let name = m["cm"].as_str()?;
            Some((
                limits::name(name.trim(), limits),
                num(&m["tm"]),
                num(&m["dr"]).max(0.0),
            ))
        })
        .collect();
    let mut glyphs = Vec::new();
    for c in v["chars"].as_array().into_iter().flatten() {
        let shapes = r.shapes(&c["data"]["shapes"], 0)?;
        glyphs.push(Glyph {
            ch: c["ch"].as_str().unwrap_or("").to_string(),
            font: c["fFamily"].as_str().unwrap_or("").to_string(),
            advance: num(&c["w"]),
            shapes,
        });
    }
    let has_text = layers
        .iter()
        .chain(precomps.iter().flatten())
        .any(|l| matches!(l.content, Content::Text(_)));
    if has_text && glyphs.is_empty() {
        r.ignore("dance_import.note.text_without_glyphs");
    }
    let mut notes: Vec<Note> = r
        .ignored
        .iter()
        .map(|k| Note::new(k, String::new()))
        .collect();
    if r.expressions > 0 {
        notes.insert(
            0,
            Note::new("dance_import.note.expressions", r.expressions.to_string()),
        );
    }
    let has_mattes = layers
        .iter()
        .chain(precomps.iter().flatten())
        .any(|l| l.matte != 0 || !l.masks.is_empty());
    Ok(Doc {
        w: num(&v["w"]).max(1.0),
        h: num(&v["h"]).max(1.0),
        fr,
        ip,
        op,
        layers,
        precomps,
        images,
        markers,
        glyphs,
        name: limits::name(v["nm"].as_str().unwrap_or(""), limits),
        author: limits::name(v["meta"]["a"].as_str().unwrap_or(""), limits),
        notes,
        paints: r.paints,
        has_mattes,
    })
}

/// Layers drawn in one frame, counting every precomposition's layers each time it is used:
/// bounded, so a file reusing precomps inside precomps cannot multiply a frame's work.
pub const LAYER_INSTANCES: usize = 20_000;

/// Precomps nested at most `precomp_depth` deep, none containing itself, and at most
/// [`LAYER_INSTANCES`] layers drawn in a frame.
fn check_precomps(
    top: &[Layer],
    precomps: &[Vec<Layer>],
    limits: &Limits,
) -> Result<(), ImportError> {
    fn walk(
        layers: &[Layer],
        precomps: &[Vec<Layer>],
        stack: &mut Vec<usize>,
        count: &mut usize,
        limits: &Limits,
    ) -> Result<(), ImportError> {
        for l in layers {
            *count += 1;
            if *count > LAYER_INSTANCES {
                return Err(ImportError::TooLarge(format!(
                    "more than {LAYER_INSTANCES} layers drawn in a frame"
                )));
            }
            if let Content::Precomp { asset, .. } = l.content {
                if stack.contains(&asset) {
                    return Err(ImportError::Invalid(
                        "a precomposition contains itself".into(),
                    ));
                }
                if stack.len() >= limits.precomp_depth {
                    return Err(ImportError::TooLarge(format!(
                        "precompositions nested more than {} deep",
                        limits.precomp_depth
                    )));
                }
                stack.push(asset);
                walk(&precomps[asset], precomps, stack, count, limits)?;
                stack.pop();
            }
        }
        Ok(())
    }
    walk(top, precomps, &mut Vec::new(), &mut 0, limits)
}
