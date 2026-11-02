//! DragonBones (ADR 0089 §6): the JSON skeleton (5.x) and its atlas — bones, slots, image
//! and mesh displays (weighted to bones), deform and z-order timelines, IK, colours,
//! nested armatures — solved per frame and drawn through our rasteriser.

use super::image::{self, PixelBudget};
use super::imported::Source;
use super::limits::{self, Budget, Limits};
use super::mapping::Clip;
use super::raster::{self, Filter, Region, Tint, Vertex};
use super::{paths, Found, ImportError, Note};
use crate::character::{Bounds, Licence};
use crate::render::Canvas;
use cantus_dsp::math;
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;
use tiny_skia::{Pixmap, Transform};

/// Whether a JSON document is a DragonBones skeleton.
pub fn is_skeleton(v: &Value) -> bool {
    v["armature"].is_array()
}

fn num(v: &Value) -> f32 {
    super::lottie::prop::num(v)
}

fn nums(v: &Value) -> Vec<f32> {
    super::lottie::prop::nums(v)
}

/// A local transform: x, y, rotation-skew angles (degrees), scale.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Xf {
    pub x: f32,
    pub y: f32,
    pub sk_x: f32,
    pub sk_y: f32,
    pub sc_x: f32,
    pub sc_y: f32,
}

impl Xf {
    pub const IDENTITY: Xf = Xf {
        x: 0.0,
        y: 0.0,
        sk_x: 0.0,
        sk_y: 0.0,
        sc_x: 1.0,
        sc_y: 1.0,
    };

    fn parse(v: &Value) -> Xf {
        let or = |k: &str, d: f32| v.get(k).map(num).unwrap_or(d);
        Xf {
            x: or("x", 0.0),
            y: or("y", 0.0),
            sk_x: or("skX", 0.0),
            sk_y: or("skY", 0.0),
            sc_x: or("scX", 1.0),
            sc_y: or("scY", 1.0),
        }
    }

    pub fn matrix(&self) -> Transform {
        let rad = std::f32::consts::PI / 180.0;
        let (sy, cy) = math::sin_cos(self.sk_y * rad);
        let (sx, cx) = math::sin_cos(self.sk_x * rad);
        Transform::from_row(
            self.sc_x * cy,
            self.sc_x * sy,
            -self.sc_y * sx,
            self.sc_y * cx,
            self.x,
            self.y,
        )
    }
}

/// A timeline's key: when (frames), how long, its easing, its values.
#[derive(Clone, Debug, PartialEq)]
struct Key {
    at: f32,
    duration: f32,
    /// `None`: no tween (held); `Some(e)`: DragonBones' easing number.
    ease: Option<f32>,
    curve: Option<[f32; 4]>,
    v: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq, Default)]
struct Timeline {
    keys: Vec<Key>,
}

impl Timeline {
    fn parse(
        list: &Value,
        values: impl Fn(&Value) -> Vec<f32>,
        budget: &mut Budget,
        limits: &Limits,
    ) -> Result<Timeline, ImportError> {
        let frames = list.as_array().map(Vec::as_slice).unwrap_or(&[]);
        budget.take("keyframes", frames.len(), limits.keyframes)?;
        let mut at = 0.0f32;
        let mut keys = Vec::with_capacity(frames.len());
        for f in frames {
            let duration = f.get("duration").map(num).unwrap_or(1.0).max(0.0);
            let curve = {
                let c = nums(&f["curve"]);
                (c.len() >= 4).then(|| [c[0], c[1], c[2], c[3]])
            };
            keys.push(Key {
                at,
                duration,
                ease: f
                    .get("tweenEasing")
                    .and_then(Value::as_f64)
                    .map(|e| e as f32)
                    .or(curve.map(|_| 0.0)),
                curve,
                v: values(f),
            });
            at += duration;
        }
        Ok(Timeline { keys })
    }

    /// The value at frame `t`: the key it is in, eased towards the next (an angle takes
    /// the short way round).
    fn at(&self, t: f32, angle: bool) -> Option<Vec<f32>> {
        let n = self.keys.partition_point(|k| k.at <= t);
        let k = self.keys.get(n.checked_sub(1)?)?;
        let next = self.keys.get(n);
        let (Some(e), Some(next)) = (k.ease, next) else {
            return Some(k.v.clone());
        };
        if k.duration <= 0.0 {
            return Some(k.v.clone());
        }
        let p = ((t - k.at) / k.duration).clamp(0.0, 1.0);
        let p = match k.curve {
            Some([x1, y1, x2, y2]) => super::lottie::prop::ease(p, (x1, y1), (x2, y2)),
            None => db_ease(p, e),
        };
        Some(
            k.v.iter()
                .zip(&next.v)
                .enumerate()
                .map(|(i, (a, b))| {
                    let mut d = b - a;
                    if angle && i == 0 {
                        d = (d + 180.0).rem_euclid(360.0) - 180.0;
                    }
                    a + d * p
                })
                .collect(),
        )
    }
}

/// DragonBones' easing number: 0 linear, below 0 easing in, above 0 easing out, 2 both.
pub fn db_ease(p: f32, e: f32) -> f32 {
    if e == 0.0 {
        p
    } else if e >= 2.0 {
        p * p * (3.0 - 2.0 * p)
    } else if e > 0.0 {
        let out = 1.0 - (1.0 - p) * (1.0 - p);
        p + (out - p) * e.min(1.0)
    } else {
        let inn = p * p;
        p + (inn - p) * (-e).min(1.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Bone {
    name: String,
    parent: Option<usize>,
    setup: Xf,
    length: f32,
}

#[derive(Clone, Debug, PartialEq)]
enum Display {
    Image {
        tex: usize,
        xf: Xf,
    },
    Mesh {
        tex: usize,
        name: String,
        /// Slot space (unweighted) or armature space at bind time (weighted).
        vertices: Vec<[f32; 2]>,
        uvs: Arc<Vec<[f32; 2]>>,
        triangles: Arc<Vec<[u32; 3]>>,
        /// Per vertex: (bone, weight) pairs; empty for a mesh on its slot's bone.
        weights: Vec<Vec<(usize, f32)>>,
        /// The inverse bind matrix of each bone, by bone index.
        bind: Vec<Option<Transform>>,
    },
    Armature(usize),
    None,
}

#[derive(Clone, Debug, PartialEq)]
struct Slot {
    name: String,
    bone: usize,
    display_index: i32,
    color: [f32; 4],
    displays: Vec<Display>,
}

#[derive(Clone, Debug, PartialEq)]
struct Ik {
    bone: usize,
    target: usize,
    two: bool,
    bend_positive: bool,
    weight: f32,
}

#[derive(Clone, Debug, PartialEq, Default)]
struct Animation {
    name: String,
    duration: f32,
    /// Per bone: translate, rotate, scale.
    bones: Vec<(usize, Timeline, Timeline, Timeline)>,
    displays: Vec<(usize, Timeline)>,
    colors: Vec<(usize, Timeline)>,
    /// (slot, display index, offsets timeline).
    deforms: Vec<(usize, usize, Timeline)>,
    z_order: Timeline,
}

#[derive(Clone, Debug, PartialEq)]
struct Armature {
    name: String,
    frame_rate: f32,
    bones: Vec<Bone>,
    slots: Vec<Slot>,
    iks: Vec<Ik>,
    animations: Vec<Animation>,
}

/// A texture of the atlas: where it is and how it was trimmed.
#[derive(Clone, Debug, PartialEq)]
struct Tex {
    name: String,
    region: Region,
    rotated: bool,
    /// The trimmed image's size and its place in its untrimmed frame.
    w: f32,
    h: f32,
    frame_x: f32,
    frame_y: f32,
    frame_w: f32,
    frame_h: f32,
}

pub struct DragonBones {
    armatures: Vec<Armature>,
    main: usize,
    textures: Vec<Tex>,
    atlas: Arc<Pixmap>,
    clips: Vec<Clip>,
    boxed: [f32; 4],
    /// How deep armatures nest (refused past `Limits::armature_depth` when read).
    max_depth: usize,
    /// The work one frame does (`weigh`): what bounds the box's sampling.
    work: u64,
}

/// The atlas beside a skeleton: `<name>_tex.json`, or the one `*_tex.json` in the folder,
/// or `texture.json`.
fn find_atlas(v: &Value, folder: &Path) -> Result<String, ImportError> {
    let name = v["name"].as_str().unwrap_or("");
    let mut candidates = Vec::new();
    if !name.is_empty() {
        candidates.push(format!("{name}_tex.json"));
    }
    if let Ok(dir) = std::fs::read_dir(folder) {
        let mut found: Vec<String> = dir
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.ends_with("_tex.json"))
            .collect();
        found.sort();
        if found.len() == 1 {
            candidates.extend(found);
        }
    }
    candidates.push("texture.json".into());
    candidates
        .into_iter()
        .find(|c| paths::resolve(folder, c).is_ok_and(|p| p.is_file()))
        .ok_or_else(|| ImportError::Invalid("no atlas beside the skeleton (…_tex.json)".into()))
}

fn read_atlas(
    tex_json: &Value,
    atlas: &Pixmap,
    budget: &mut Budget,
    limits: &Limits,
) -> Result<Vec<Tex>, ImportError> {
    let mut out = Vec::new();
    for t in tex_json["SubTexture"].as_array().into_iter().flatten() {
        budget.take("frames", 1, limits.frames)?;
        let (x, y, w, h) = (
            num(&t["x"]),
            num(&t["y"]),
            num(&t["width"]),
            num(&t["height"]),
        );
        let rotated = t["rotated"].as_bool().unwrap_or(false);
        let (aw, ah) = if rotated { (h, w) } else { (w, h) };
        let inside = x >= 0.0
            && y >= 0.0
            && w > 0.0
            && h > 0.0
            && x + aw <= atlas.width() as f32
            && y + ah <= atlas.height() as f32;
        if !inside {
            return Err(ImportError::Invalid(format!(
                "the texture \"{}\" lies outside its image",
                limits::name(t["name"].as_str().unwrap_or(""), limits)
            )));
        }
        out.push(Tex {
            name: limits::name(t["name"].as_str().unwrap_or(""), limits),
            region: Region {
                x0: x as i32,
                y0: y as i32,
                x1: (x + aw) as i32,
                y1: (y + ah) as i32,
            },
            rotated,
            w,
            h,
            frame_x: t.get("frameX").map(num).unwrap_or(0.0),
            frame_y: t.get("frameY").map(num).unwrap_or(0.0),
            frame_w: t.get("frameWidth").map(num).unwrap_or(w),
            frame_h: t.get("frameHeight").map(num).unwrap_or(h),
        });
    }
    Ok(out)
}

struct Reader<'a> {
    limits: &'a Limits,
    budget: Budget,
    notes: Vec<Note>,
}

impl Reader<'_> {
    fn note(&mut self, key: &'static str) {
        if !self.notes.iter().any(|n| n.key == key) {
            self.notes.push(Note::new(key, String::new()));
        }
    }

    fn armature(
        &mut self,
        a: &Value,
        names: &[String],
        textures: &[Tex],
    ) -> Result<Armature, ImportError> {
        let limits = self.limits;
        let list = a["bone"].as_array().map(Vec::as_slice).unwrap_or(&[]);
        self.budget.take("bones", list.len(), limits.bones)?;
        let bone_names: Vec<String> = list
            .iter()
            .map(|b| b["name"].as_str().unwrap_or("").to_string())
            .collect();
        let bone_of = |n: &str| bone_names.iter().position(|b| b == n);
        let mut bones = Vec::with_capacity(list.len());
        for b in list {
            if b["inheritRotation"].as_bool() == Some(false)
                || b["inheritScale"].as_bool() == Some(false)
                || b["inheritTranslation"].as_bool() == Some(false)
            {
                self.note("dance_import.note.bone_inherit");
            }
            bones.push(Bone {
                name: limits::name(b["name"].as_str().unwrap_or(""), limits),
                parent: b["parent"].as_str().and_then(bone_of),
                setup: Xf::parse(&b["transform"]),
                length: num(&b["length"]),
            });
        }
        if bones.is_empty() {
            return Err(ImportError::Invalid("an armature without bones".into()));
        }
        // A tree: following parents from any bone reaches a root.
        for start in 0..bones.len() {
            let mut at = start;
            let mut steps = 0;
            while let Some(p) = bones[at].parent {
                at = p;
                steps += 1;
                if steps > bones.len() {
                    return Err(ImportError::Invalid(
                        "the bones' parents make a loop".into(),
                    ));
                }
            }
        }
        let slot_list = a["slot"].as_array().map(Vec::as_slice).unwrap_or(&[]);
        self.budget.take("slots", slot_list.len(), limits.slots)?;
        let mut slots = Vec::with_capacity(slot_list.len());
        for s in slot_list {
            let c = &s["color"];
            let pct = |k: &str| c.get(k).map(num).unwrap_or(100.0).clamp(0.0, 100.0) / 100.0;
            if ["aO", "rO", "gO", "bO"]
                .iter()
                .any(|k| c.get(*k).is_some_and(|v| num(v) != 0.0))
            {
                self.note("dance_import.note.color_offsets");
            }
            slots.push(Slot {
                name: limits::name(s["name"].as_str().unwrap_or(""), limits),
                bone: s["parent"].as_str().and_then(bone_of).unwrap_or(0),
                display_index: s["displayIndex"].as_i64().unwrap_or(0).clamp(-1, 10_000) as i32,
                color: [pct("rM"), pct("gM"), pct("bM"), pct("aM")],
                displays: Vec::new(),
            });
        }
        // The skin named "" or "default", else the first.
        let skins = a["skin"].as_array().map(Vec::as_slice).unwrap_or(&[]);
        let skin = skins
            .iter()
            .find(|s| {
                s["name"]
                    .as_str()
                    .is_none_or(|n| n.is_empty() || n == "default")
            })
            .or(skins.first());
        if skins.len() > 1 {
            self.note("dance_import.note.first_skin");
        }
        if let Some(skin) = skin {
            for ss in skin["slot"].as_array().into_iter().flatten() {
                let Some(si) = ss["name"]
                    .as_str()
                    .and_then(|n| slots.iter().position(|s: &Slot| s.name == n))
                else {
                    continue;
                };
                let mut displays = Vec::new();
                for d in ss["display"].as_array().into_iter().flatten() {
                    displays.push(self.display(d, names, textures, &bone_of)?);
                }
                slots[si].displays = displays;
            }
        }
        let mut iks = Vec::new();
        for ik in a["ik"].as_array().into_iter().flatten() {
            let (Some(bone), Some(target)) = (
                ik["bone"].as_str().and_then(bone_of),
                ik["target"].as_str().and_then(bone_of),
            ) else {
                continue;
            };
            let two = ik["chain"].as_u64().unwrap_or(0) >= 1 && bones[bone].parent.is_some();
            iks.push(Ik {
                bone,
                target,
                two,
                bend_positive: ik["bendPositive"].as_bool().unwrap_or(true),
                weight: ik.get("weight").map(num).unwrap_or(1.0).clamp(0.0, 1.0),
            });
        }
        let anim_list = a["animation"].as_array().map(Vec::as_slice).unwrap_or(&[]);
        self.budget
            .take("animations", anim_list.len(), limits.animations)?;
        let mut animations = Vec::new();
        for an in anim_list {
            animations.push(self.animation(an, &bone_of, &slots)?);
        }
        Ok(Armature {
            name: limits::name(a["name"].as_str().unwrap_or(""), limits),
            frame_rate: a
                .get("frameRate")
                .map(num)
                .filter(|f| *f > 0.0)
                .unwrap_or(24.0)
                .clamp(1.0, 240.0),
            bones,
            slots,
            iks,
            animations,
        })
    }

    fn display(
        &mut self,
        d: &Value,
        names: &[String],
        textures: &[Tex],
        bone_of: &dyn Fn(&str) -> Option<usize>,
    ) -> Result<Display, ImportError> {
        let limits = self.limits;
        let tex_of = |d: &Value| {
            let n = d["path"].as_str().or(d["name"].as_str()).unwrap_or("");
            textures.iter().position(|t| t.name == n)
        };
        Ok(match d["type"].as_str().unwrap_or("image") {
            "image" => match tex_of(d) {
                Some(tex) => Display::Image {
                    tex,
                    xf: Xf::parse(&d["transform"]),
                },
                None => {
                    self.note("dance_import.note.missing_texture");
                    Display::None
                }
            },
            "mesh" => {
                let Some(tex) = tex_of(d) else {
                    self.note("dance_import.note.missing_texture");
                    return Ok(Display::None);
                };
                let v = nums(&d["vertices"]);
                let n = v.len() / 2;
                self.budget.take("mesh vertices", n, limits.mesh_vertices)?;
                let vertices: Vec<[f32; 2]> = v.chunks_exact(2).map(|p| [p[0], p[1]]).collect();
                let uv = nums(&d["uvs"]);
                let uvs: Vec<[f32; 2]> = uv.chunks_exact(2).map(|p| [p[0], p[1]]).collect();
                let tri = nums(&d["triangles"]);
                self.budget
                    .take("mesh triangles", tri.len() / 3, limits.mesh_triangles)?;
                let triangles: Vec<[u32; 3]> = tri
                    .chunks_exact(3)
                    .map(|t| [t[0] as u32, t[1] as u32, t[2] as u32])
                    .filter(|t| t.iter().all(|i| (*i as usize) < n))
                    .collect();
                if uvs.len() != n {
                    return Err(ImportError::Invalid(
                        "a mesh whose uvs and vertices differ".into(),
                    ));
                }
                // Weights: per vertex a count, then (bone, weight) pairs.
                let w = nums(&d["weights"]);
                let mut weights = Vec::new();
                let mut bind: Vec<Option<Transform>> = Vec::new();
                if !w.is_empty() {
                    let mut k = 0usize;
                    for _ in 0..n {
                        let count = w.get(k).copied().unwrap_or(0.0).max(0.0) as usize;
                        k += 1;
                        let mut pairs = Vec::new();
                        for _ in 0..count.min(8) {
                            let (Some(b), Some(wt)) = (w.get(k), w.get(k + 1)) else {
                                break;
                            };
                            pairs.push((*b as usize, *wt));
                            k += 2;
                        }
                        k += 2 * count.saturating_sub(8);
                        weights.push(pairs);
                    }
                    let bp = nums(&d["bonePose"]);
                    for c in bp.chunks_exact(7) {
                        let b = c[0] as usize;
                        if b >= 100_000 {
                            continue;
                        }
                        if bind.len() <= b {
                            bind.resize(b + 1, None);
                        }
                        bind[b] = Transform::from_row(c[1], c[2], c[3], c[4], c[5], c[6]).invert();
                    }
                    // Vertices are in the slot's bind space; to the armature's.
                    let sp = nums(&d["slotPose"]);
                    if sp.len() >= 6 {
                        let t = Transform::from_row(sp[0], sp[1], sp[2], sp[3], sp[4], sp[5]);
                        let mut vs = vertices.clone();
                        for p in &mut vs {
                            let (x, y) = raster::map(t, p[0], p[1]);
                            *p = [x, y];
                        }
                        return Ok(Display::Mesh {
                            tex,
                            name: limits::name(d["name"].as_str().unwrap_or(""), limits),
                            vertices: vs,
                            uvs: Arc::new(uvs),
                            triangles: Arc::new(triangles),
                            weights,
                            bind,
                        });
                    }
                }
                Display::Mesh {
                    tex,
                    name: limits::name(d["name"].as_str().unwrap_or(""), limits),
                    vertices,
                    uvs: Arc::new(uvs),
                    triangles: Arc::new(triangles),
                    weights,
                    bind,
                }
            }
            "armature" => match d["path"]
                .as_str()
                .or(d["name"].as_str())
                .and_then(|n| names.iter().position(|a| a == n))
            {
                Some(a) => Display::Armature(a),
                None => Display::None,
            },
            "boundingBox" => Display::None,
            _ => {
                let _ = bone_of;
                Display::None
            }
        })
    }
}

impl Reader<'_> {
    fn animation(
        &mut self,
        an: &Value,
        bone_of: &dyn Fn(&str) -> Option<usize>,
        slots: &[Slot],
    ) -> Result<Animation, ImportError> {
        let limits = self.limits;
        let mut a = Animation {
            name: limits::name(an["name"].as_str().unwrap_or(""), limits),
            duration: num(&an["duration"]).max(0.0),
            ..Animation::default()
        };
        for b in an["bone"].as_array().into_iter().flatten() {
            let Some(bi) = b["name"].as_str().and_then(bone_of) else {
                continue;
            };
            let (tr, ro, sc) = if b.get("frame").is_some() {
                // The older form: one timeline of whole transforms.
                let f = &b["frame"];
                (
                    Timeline::parse(
                        f,
                        |k| {
                            let x = Xf::parse(&k["transform"]);
                            vec![x.x, x.y]
                        },
                        &mut self.budget,
                        limits,
                    )?,
                    Timeline::parse(
                        f,
                        |k| {
                            let x = Xf::parse(&k["transform"]);
                            vec![x.sk_y, x.sk_x - x.sk_y]
                        },
                        &mut self.budget,
                        limits,
                    )?,
                    Timeline::parse(
                        f,
                        |k| {
                            let x = Xf::parse(&k["transform"]);
                            vec![x.sc_x, x.sc_y]
                        },
                        &mut self.budget,
                        limits,
                    )?,
                )
            } else {
                (
                    Timeline::parse(
                        &b["translateFrame"],
                        |k| {
                            vec![
                                k.get("x").map(num).unwrap_or(0.0),
                                k.get("y").map(num).unwrap_or(0.0),
                            ]
                        },
                        &mut self.budget,
                        limits,
                    )?,
                    Timeline::parse(
                        &b["rotateFrame"],
                        |k| {
                            vec![
                                k.get("rotate").map(num).unwrap_or(0.0),
                                k.get("skew").map(num).unwrap_or(0.0),
                            ]
                        },
                        &mut self.budget,
                        limits,
                    )?,
                    Timeline::parse(
                        &b["scaleFrame"],
                        |k| {
                            vec![
                                k.get("x").map(num).unwrap_or(1.0),
                                k.get("y").map(num).unwrap_or(1.0),
                            ]
                        },
                        &mut self.budget,
                        limits,
                    )?,
                )
            };
            a.bones.push((bi, tr, ro, sc));
        }
        let slot_of = |n: &str| slots.iter().position(|s| s.name == n);
        for s in an["slot"].as_array().into_iter().flatten() {
            let Some(si) = s["name"].as_str().and_then(slot_of) else {
                continue;
            };
            if s.get("displayFrame").is_some() {
                let t = Timeline::parse(
                    &s["displayFrame"],
                    |k| vec![k.get("value").map(num).unwrap_or(0.0)],
                    &mut self.budget,
                    limits,
                )?;
                a.displays.push((
                    si,
                    Timeline {
                        keys: t
                            .keys
                            .into_iter()
                            .map(|mut k| {
                                k.ease = None;
                                k
                            })
                            .collect(),
                    },
                ));
            }
            if s.get("colorFrame").is_some() {
                let pct = |c: &Value, k: &str| {
                    c.get(k).map(num).unwrap_or(100.0).clamp(0.0, 100.0) / 100.0
                };
                let t = Timeline::parse(
                    &s["colorFrame"],
                    |k| {
                        let c = &k["value"];
                        vec![pct(c, "rM"), pct(c, "gM"), pct(c, "bM"), pct(c, "aM")]
                    },
                    &mut self.budget,
                    limits,
                )?;
                a.colors.push((si, t));
            }
        }
        let deforms = an.get("ffd").or(an.get("deform")).and_then(Value::as_array);
        for d in deforms.into_iter().flatten() {
            let Some(si) = d["slot"].as_str().and_then(slot_of) else {
                continue;
            };
            let name = d["name"].as_str().unwrap_or("");
            let Some((di, n)) = slots[si]
                .displays
                .iter()
                .enumerate()
                .find_map(|(i, x)| match x {
                    Display::Mesh {
                        name: m, vertices, ..
                    } if m == name => Some((i, vertices.len())),
                    _ => None,
                })
            else {
                continue;
            };
            let t = Timeline::parse(
                &d["frame"],
                |k| {
                    let mut out = vec![0.0f32; n * 2];
                    let off = k.get("offset").map(num).unwrap_or(0.0).max(0.0) as usize;
                    for (i, v) in nums(&k["vertices"]).into_iter().enumerate() {
                        if let Some(o) = out.get_mut(off + i) {
                            *o = v;
                        }
                    }
                    out
                },
                &mut self.budget,
                limits,
            )?;
            a.deforms.push((si, di, t));
        }
        if let Some(z) = an["zOrder"]["frame"].as_array() {
            let t = Timeline::parse(
                &Value::Array(z.clone()),
                |k| nums(&k["zOrder"]),
                &mut self.budget,
                limits,
            )?;
            a.z_order = Timeline {
                keys: t
                    .keys
                    .into_iter()
                    .map(|mut k| {
                        k.ease = None;
                        k
                    })
                    .collect(),
            };
        }
        if an["ik"].as_array().is_some_and(|i| !i.is_empty()) {
            self.note("dance_import.note.ik_timelines");
        }
        if an["frame"].as_array().is_some_and(|f| !f.is_empty()) {
            self.note("dance_import.note.events");
        }
        if a.duration <= 0.0 {
            let longest = a
                .bones
                .iter()
                .flat_map(|(_, x, y, z)| [x, y, z])
                .map(|t| t.keys.last().map(|k| k.at + k.duration).unwrap_or(0.0))
                .fold(0.0f32, f32::max);
            a.duration = longest.max(1.0);
        }
        Ok(a)
    }
}

/// Read a skeleton, its atlas and the atlas's image from `folder`.
pub fn read(v: &Value, folder: &Path, limits: &Limits) -> Result<Found, ImportError> {
    let version = v["version"].as_str().unwrap_or("");
    let major = version
        .split('.')
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);
    if major != 0 && major < 5 {
        return Err(ImportError::Invalid(format!(
            "DragonBones {version}: Cantus reads 5.x — open it in DragonBones or LoongBones and export again"
        )));
    }
    let atlas_name = find_atlas(v, folder)?;
    let tex_bytes = paths::read_inside(folder, &atlas_name, limits.json_bytes)?;
    let tex_json = limits::parse_json(&tex_bytes, limits)?;
    let image_name = tex_json["imagePath"]
        .as_str()
        .ok_or_else(|| ImportError::Invalid("the atlas names no image (imagePath)".into()))?
        .to_string();
    let img_bytes = paths::read_inside(folder, &image_name, limits.image_bytes)?;
    let atlas = image::decode(&img_bytes, limits, &mut PixelBudget::default())?;
    let mut r = Reader {
        limits,
        budget: Budget::new(),
        notes: Vec::new(),
    };
    let textures = read_atlas(&tex_json, &atlas, &mut r.budget, limits)?;
    let list = v["armature"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    if list.is_empty() {
        return Err(ImportError::Invalid("the skeleton has no armature".into()));
    }
    let names: Vec<String> = list
        .iter()
        .map(|a| a["name"].as_str().unwrap_or("").to_string())
        .collect();
    let mut armatures = Vec::new();
    for a in list {
        armatures.push(r.armature(a, &names, &textures)?);
    }
    // Nested armatures: no loop, at most `armature_depth` deep, and at most
    // `ARMATURE_INSTANCES` drawn in a frame — counted per armature once (memoised), before
    // anything is walked.
    let weight = weigh(&armatures, limits)?;
    let main = &armatures[0];
    let clips: Vec<Clip> = main
        .animations
        .iter()
        .map(|a| Clip {
            name: a.name.clone(),
            seconds: (a.duration / main.frame_rate).max(1e-3),
            frames: a.duration.round().max(1.0) as u32,
        })
        .collect();
    let facts = vec![
        ("dance_import.fact.bones", main.bones.len().to_string()),
        ("dance_import.fact.slots", main.slots.len().to_string()),
        (
            "dance_import.fact.frame_rate",
            format!("{}", main.frame_rate),
        ),
        (
            "dance_import.fact.image",
            format!("{} × {}", atlas.width(), atlas.height()),
        ),
    ];
    let mut db = DragonBones {
        armatures,
        main: 0,
        textures,
        atlas: Arc::new(atlas),
        clips: if clips.is_empty() {
            vec![Clip {
                name: "pose".into(),
                seconds: 1.0,
                frames: 1,
            }]
        } else {
            clips
        },
        boxed: [0.0; 4],
        max_depth: limits.armature_depth,
        work: weight.work,
    };
    db.boxed = db.measure();
    let name = v["name"].as_str().unwrap_or("").to_string();
    Ok(Found {
        source: Box::new(db),
        name: limits::name(&name, limits),
        author: String::new(),
        licence: Licence::Unknown,
        notes: r.notes,
        facts,
        files: vec![atlas_name, image_name],
        default_filter: Filter::Bilinear,
    })
}

/// Armatures drawn in one frame at most, counting each nested armature every time a slot
/// can show it (the most a slot's displays can give).
pub const ARMATURE_INSTANCES: u64 = 2_000;

/// What one armature costs a frame, nested ones included: armatures drawn, work (a slot, an
/// image's four corners, a mesh's vertices and triangles — each instance its own), and how
/// deep it nests.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Weight {
    pub instances: u64,
    pub work: u64,
    pub height: usize,
}

/// The weight of the main armature, every armature worked out once (memoised) before
/// anything is walked; refused in words past the depth (one limit, wherever an armature is
/// reached from), the instances or the frame's work, or for a loop.
fn weigh(arms: &[Armature], limits: &Limits) -> Result<Weight, ImportError> {
    fn of(
        arms: &[Armature],
        i: usize,
        stack: &mut Vec<usize>,
        memo: &mut [Option<Weight>],
        limits: &Limits,
    ) -> Result<Weight, ImportError> {
        if let Some(w) = memo[i] {
            return Ok(w);
        }
        if stack.contains(&i) {
            return Err(ImportError::Invalid("an armature shows itself".into()));
        }
        if stack.len() > limits.armature_depth {
            return Err(too_deep(limits));
        }
        stack.push(i);
        // Posing it: every bone solved once, and again for each IK constraint (a chain of
        // two solves twice) — per instance, like what it draws (review 3).
        let arm = &arms[i];
        let solves: u64 = 1 + arm
            .iks
            .iter()
            .map(|k| if k.two { 2 } else { 1 })
            .sum::<u64>();
        let mut w = Weight {
            instances: 1,
            work: (arm.bones.len() as u64).saturating_mul(solves),
            height: 0,
        };
        for s in &arms[i].slots {
            let (mut work, mut inst) = (0u64, 0u64);
            for disp in &s.displays {
                match disp {
                    Display::Image { .. } => work = work.max(4),
                    Display::Mesh {
                        vertices,
                        triangles,
                        ..
                    } => work = work.max((vertices.len() + triangles.len()) as u64),
                    Display::Armature(c) => {
                        let child = of(arms, *c, stack, memo, limits)?;
                        work = work.max(child.work);
                        inst = inst.max(child.instances);
                        w.height = w.height.max(child.height + 1);
                    }
                    Display::None => {}
                }
            }
            w.work = w.work.saturating_add(1 + work);
            w.instances = w.instances.saturating_add(inst);
            if w.instances > ARMATURE_INSTANCES {
                return Err(ImportError::TooLarge(format!(
                    "more than {ARMATURE_INSTANCES} armatures drawn in a frame"
                )));
            }
            if w.work > limits.frame_work {
                return Err(ImportError::TooLarge(format!(
                    "more than {} slots, corners, vertices and triangles drawn in a frame",
                    limits.frame_work
                )));
            }
        }
        if w.height > limits.armature_depth {
            return Err(too_deep(limits));
        }
        stack.pop();
        memo[i] = Some(w);
        Ok(w)
    }
    let mut memo = vec![None; arms.len()];
    of(arms, 0, &mut Vec::new(), &mut memo, limits)
}

fn too_deep(limits: &Limits) -> ImportError {
    ImportError::TooLarge(format!(
        "armatures nested more than {} deep",
        limits.armature_depth
    ))
}

/// The work the box's sampling may do at load, in all (see [`Weight::work`]).
pub const MEASURE_WORK: u64 = 20_000_000;

/// How the box is sampled within [`MEASURE_WORK`]: how many animations (spread evenly) and
/// how many moments of each (2…17, or the start alone) — never more work in all than the
/// bound, beyond the one walk a file always gets (review 3: no floor overriding it).
pub fn measure_plan(work: u64, animations: usize) -> (usize, usize) {
    let anims = animations.max(1);
    let walks = (MEASURE_WORK / work.max(1)).max(1) as usize;
    if walks >= anims * 2 {
        (anims, (walks / anims).min(17))
    } else {
        (walks.min(anims), 1)
    }
}

/// What a frame draws, before pixels.
enum Prim {
    Quad {
        tex: usize,
        corners: [(f32, f32); 4],
        tint: [f32; 4],
    },
    Mesh {
        tex: usize,
        positions: Vec<(f32, f32)>,
        uvs: Arc<Vec<[f32; 2]>>,
        triangles: Arc<Vec<[u32; 3]>>,
        tint: [f32; 4],
    },
}

impl DragonBones {
    /// Every bone's world transform for `anim` at `frame`, with IK applied.
    fn pose(&self, arm: &Armature, anim: Option<&Animation>, frame: f32) -> Vec<Transform> {
        let mut local: Vec<Xf> = arm.bones.iter().map(|b| b.setup).collect();
        if let Some(a) = anim {
            for (bi, tr, ro, sc) in &a.bones {
                let Some(x) = local.get_mut(*bi) else {
                    continue;
                };
                if let Some(v) = tr.at(frame, false) {
                    x.x += v.first().copied().unwrap_or(0.0);
                    x.y += v.get(1).copied().unwrap_or(0.0);
                }
                if let Some(v) = ro.at(frame, true) {
                    let r = v.first().copied().unwrap_or(0.0);
                    let skew = v.get(1).copied().unwrap_or(0.0);
                    x.sk_y += r;
                    x.sk_x += r + skew;
                }
                if let Some(v) = sc.at(frame, false) {
                    x.sc_x *= v.first().copied().unwrap_or(1.0);
                    x.sc_y *= v.get(1).copied().unwrap_or(1.0);
                }
            }
        }
        let mut world = solve(arm, &local);
        for ik in &arm.iks {
            apply_ik(arm, ik, &mut local, &mut world);
        }
        world
    }

    fn walk(
        &self,
        ai: usize,
        anim: usize,
        frame: f32,
        base: Transform,
        depth: usize,
        out: &mut Vec<Prim>,
    ) {
        let Some(arm) = self.armatures.get(ai) else {
            return;
        };
        if depth > self.max_depth {
            return;
        }
        let a = arm.animations.get(anim);
        let world = self.pose(arm, a, frame);
        // The draw order, through the z-order timeline.
        let n = arm.slots.len();
        let mut order: Vec<usize> = (0..n).collect();
        if let Some(z) = a.and_then(|a| a.z_order.at(frame, false)) {
            order = z_order(n, &z);
        }
        for si in order {
            let slot = &arm.slots[si];
            let mut di = slot.display_index;
            let mut tint = slot.color;
            if let Some(a) = a {
                if let Some((_, t)) = a.displays.iter().find(|(s, _)| *s == si) {
                    if let Some(v) = t.at(frame, false) {
                        di = v.first().copied().unwrap_or(0.0) as i32;
                    }
                }
                if let Some((_, t)) = a.colors.iter().find(|(s, _)| *s == si) {
                    if let Some(v) = t.at(frame, false) {
                        for (k, c) in tint.iter_mut().enumerate() {
                            *c = v.get(k).copied().unwrap_or(1.0).clamp(0.0, 1.0);
                        }
                    }
                }
            }
            let Some(display) = usize::try_from(di).ok().and_then(|d| slot.displays.get(d)) else {
                continue;
            };
            let bone_world = base.pre_concat(world.get(slot.bone).copied().unwrap_or_default());
            match display {
                Display::Image { tex, xf } => {
                    let Some(t) = self.textures.get(*tex) else {
                        continue;
                    };
                    let m = bone_world.pre_concat(xf.matrix());
                    let (x0, y0) = (-t.frame_w * 0.5 - t.frame_x, -t.frame_h * 0.5 - t.frame_y);
                    let p = |x: f32, y: f32| raster::map(m, x0 + x, y0 + y);
                    out.push(Prim::Quad {
                        tex: *tex,
                        corners: [p(0.0, 0.0), p(t.w, 0.0), p(t.w, t.h), p(0.0, t.h)],
                        tint,
                    });
                }
                Display::Mesh {
                    tex,
                    vertices,
                    uvs,
                    triangles,
                    weights,
                    bind,
                    ..
                } => {
                    let offsets = a
                        .and_then(|a| {
                            a.deforms.iter().find(|(s, d, _)| {
                                *s == si && usize::try_from(di).is_ok_and(|di| di == *d)
                            })
                        })
                        .and_then(|(_, _, t)| t.at(frame, false));
                    let positions: Vec<(f32, f32)> = vertices
                        .iter()
                        .enumerate()
                        .map(|(k, v)| {
                            let (dx, dy) = offsets
                                .as_ref()
                                .map(|o| {
                                    (
                                        o.get(2 * k).copied().unwrap_or(0.0),
                                        o.get(2 * k + 1).copied().unwrap_or(0.0),
                                    )
                                })
                                .unwrap_or((0.0, 0.0));
                            let (x, y) = (v[0] + dx, v[1] + dy);
                            match weights.get(k).filter(|w| !w.is_empty()) {
                                None => raster::map(bone_world, x, y),
                                Some(w) => {
                                    let (mut sx, mut sy) = (0.0, 0.0);
                                    for (b, wt) in w {
                                        let (Some(bw), Some(Some(inv))) =
                                            (world.get(*b), bind.get(*b))
                                        else {
                                            continue;
                                        };
                                        let m = base.pre_concat(*bw).pre_concat(*inv);
                                        let (px, py) = raster::map(m, x, y);
                                        sx += px * wt;
                                        sy += py * wt;
                                    }
                                    (sx, sy)
                                }
                            }
                        })
                        .collect();
                    out.push(Prim::Mesh {
                        tex: *tex,
                        positions,
                        uvs: Arc::clone(uvs),
                        triangles: Arc::clone(triangles),
                        tint,
                    });
                }
                Display::Armature(child) => {
                    let c = &self.armatures[*child];
                    let len = c
                        .animations
                        .first()
                        .map(|a| a.duration)
                        .unwrap_or(1.0)
                        .max(1.0);
                    let f = (frame / arm.frame_rate * c.frame_rate).rem_euclid(len);
                    self.walk(*child, 0, f, bone_world, depth + 1, out);
                }
                Display::None => {}
            }
        }
    }

    /// Atlas pixels of a texture's (u, v) in 0…1.
    fn uv(&self, t: &Tex, u: f32, v: f32) -> (f32, f32) {
        let r = t.region;
        if t.rotated {
            // Stored a quarter turn clockwise.
            (r.x0 as f32 + (1.0 - v) * t.h, r.y0 as f32 + u * t.w)
        } else {
            (r.x0 as f32 + u * t.w, r.y0 as f32 + v * t.h)
        }
    }

    fn measure(&self) -> [f32; 4] {
        let mut b = [
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ];
        let main = &self.armatures[self.main];
        let anims = main.animations.len().max(1);
        let (sampled, per) = measure_plan(self.work, anims);
        let stride = (anims / sampled.max(1)).max(1);
        for ai in (0..anims).step_by(stride).take(sampled) {
            let dur = main.animations.get(ai).map(|a| a.duration).unwrap_or(1.0);
            for s in 0..per {
                let mut prims = Vec::new();
                self.walk(
                    self.main,
                    ai,
                    dur * s as f32 / per.max(2).saturating_sub(1) as f32,
                    Transform::identity(),
                    0,
                    &mut prims,
                );
                for p in &prims {
                    let pts: Vec<(f32, f32)> = match p {
                        Prim::Quad { corners, .. } => corners.to_vec(),
                        Prim::Mesh { positions, .. } => positions.clone(),
                    };
                    for (x, y) in pts {
                        if x.is_finite() && y.is_finite() {
                            b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
                        }
                    }
                }
            }
        }
        if b[0].is_finite() && b[2] > b[0] && b[3] > b[1] {
            b
        } else {
            [-50.0, -100.0, 50.0, 0.0]
        }
    }
}

/// World transforms from local ones, parents first (any order in the file).
fn solve(arm: &Armature, local: &[Xf]) -> Vec<Transform> {
    let n = arm.bones.len();
    let mut world: Vec<Option<Transform>> = vec![None; n];
    fn get(
        i: usize,
        arm: &Armature,
        local: &[Xf],
        world: &mut Vec<Option<Transform>>,
        hops: usize,
    ) -> Transform {
        if let Some(t) = world[i] {
            return t;
        }
        let own = local[i].matrix();
        let t = match arm.bones[i].parent {
            Some(p) if hops < arm.bones.len() => {
                get(p, arm, local, world, hops + 1).pre_concat(own)
            }
            _ => own,
        };
        world[i] = Some(t);
        t
    }
    (0..n).map(|i| get(i, arm, local, &mut world, 0)).collect()
}

fn origin(t: &Transform) -> (f32, f32) {
    (t.tx, t.ty)
}

fn angle_of(t: &Transform) -> f32 {
    math::atan2(t.ky, t.sx) * 180.0 / std::f32::consts::PI
}

fn wrap(d: f32) -> f32 {
    (d + 180.0).rem_euclid(360.0) - 180.0
}

/// Turn `bone` (and its parent, for a chain of two) so its tip reaches the target.
fn apply_ik(arm: &Armature, ik: &Ik, local: &mut [Xf], world: &mut Vec<Transform>) {
    let (tx, ty) = origin(&world[ik.target]);
    let w = ik.weight;
    let child = ik.bone;
    if !ik.two {
        let (ox, oy) = origin(&world[child]);
        let want = math::atan2(ty - oy, tx - ox) * 180.0 / std::f32::consts::PI;
        let d = wrap(want - angle_of(&world[child])) * w;
        local[child].sk_x += d;
        local[child].sk_y += d;
        *world = solve(arm, local);
        return;
    }
    let Some(parent) = arm.bones[child].parent else {
        return;
    };
    let (px, py) = origin(&world[parent]);
    let (cx, cy) = origin(&world[child]);
    let l1 = ((cx - px) * (cx - px) + (cy - py) * (cy - py)).sqrt();
    let cw = &world[child];
    let l2 = arm.bones[child].length * (cw.sx * cw.sx + cw.ky * cw.ky).sqrt();
    let (dx, dy) = (tx - px, ty - py);
    let d = (dx * dx + dy * dy)
        .sqrt()
        .clamp((l1 - l2).abs() + 1e-3, (l1 + l2 - 1e-3).max(1e-3));
    if l1 <= 1e-4 {
        return;
    }
    let cos = ((l1 * l1 + d * d - l2 * l2) / (2.0 * l1 * d)).clamp(-1.0, 1.0);
    let bend = math::acos(cos) * 180.0 / std::f32::consts::PI;
    let base = math::atan2(dy, dx) * 180.0 / std::f32::consts::PI;
    let want_parent = if ik.bend_positive {
        base - bend
    } else {
        base + bend
    };
    let now_parent = math::atan2(cy - py, cx - px) * 180.0 / std::f32::consts::PI;
    let dp = wrap(want_parent - now_parent) * w;
    local[parent].sk_x += dp;
    local[parent].sk_y += dp;
    *world = solve(arm, local);
    let (cx, cy) = origin(&world[child]);
    let want_child = math::atan2(ty - cy, tx - cx) * 180.0 / std::f32::consts::PI;
    let dc = wrap(want_child - angle_of(&world[child])) * w;
    local[child].sk_x += dc;
    local[child].sk_y += dc;
    *world = solve(arm, local);
}

/// The draw order from a z-order key: (slot, offset) pairs moving slots up or down.
fn z_order(n: usize, pairs: &[f32]) -> Vec<usize> {
    let mut order: Vec<Option<usize>> = vec![None; n];
    let mut unchanged = Vec::with_capacity(n);
    let mut next = 0usize;
    for p in pairs.chunks_exact(2) {
        let (slot, offset) = (p[0] as i64, p[1] as i64);
        if slot < 0 || slot as usize >= n {
            continue;
        }
        while next < slot as usize {
            unchanged.push(next);
            next += 1;
        }
        next = next.max(slot as usize + 1);
        let to = slot + offset;
        if to >= 0 && (to as usize) < n && order[to as usize].is_none() {
            order[to as usize] = Some(slot as usize);
        } else {
            unchanged.push(slot as usize);
        }
    }
    while next < n {
        unchanged.push(next);
        next += 1;
    }
    let mut rest = unchanged.into_iter();
    let out: Vec<usize> = order
        .into_iter()
        .map(|o| o.or_else(|| rest.next()).unwrap_or(0))
        .collect();
    // A malformed key could repeat a slot: then keep the file's order.
    let mut seen = vec![false; n];
    if out.iter().all(|&i| !std::mem::replace(&mut seen[i], true)) {
        out
    } else {
        (0..n).collect()
    }
}

impl Source for DragonBones {
    fn format(&self) -> &'static str {
        "DragonBones"
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
        self.armatures[self.main].frame_rate
    }

    fn draw(&self, clip: usize, t: f32, filter: Filter, canvas: &mut Canvas) {
        let main = &self.armatures[self.main];
        let mut prims = Vec::new();
        self.walk(
            self.main,
            clip,
            t * main.frame_rate,
            canvas.transform,
            0,
            &mut prims,
        );
        let atlas = &self.atlas;
        for p in prims {
            match p {
                Prim::Quad { tex, corners, tint } => {
                    let Some(t) = self.textures.get(tex) else {
                        continue;
                    };
                    let uv = [
                        self.uv(t, 0.0, 0.0),
                        self.uv(t, 1.0, 0.0),
                        self.uv(t, 1.0, 1.0),
                        self.uv(t, 0.0, 1.0),
                    ];
                    let v = |k: usize| Vertex {
                        x: corners[k].0,
                        y: corners[k].1,
                        u: uv[k].0,
                        v: uv[k].1,
                    };
                    let tint = Tint::rgba(tint).hue(canvas.hue);
                    raster::triangle(
                        canvas.pixmap,
                        atlas,
                        [v(0), v(1), v(2)],
                        t.region,
                        filter,
                        tint,
                    );
                    raster::triangle(
                        canvas.pixmap,
                        atlas,
                        [v(0), v(2), v(3)],
                        t.region,
                        filter,
                        tint,
                    );
                }
                Prim::Mesh {
                    tex,
                    positions,
                    uvs,
                    triangles,
                    tint,
                } => {
                    let Some(t) = self.textures.get(tex) else {
                        continue;
                    };
                    let uvs: Vec<(f32, f32)> = uvs.iter().map(|p| self.uv(t, p[0], p[1])).collect();
                    raster::mesh(
                        canvas.pixmap,
                        atlas,
                        &positions,
                        &uvs,
                        triangles.as_slice(),
                        t.region,
                        filter,
                        Tint::rgba(tint).hue(canvas.hue),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn z_order_moves_slots_and_refuses_nonsense() {
        assert_eq!(z_order(4, &[]), vec![0, 1, 2, 3]);
        // Slot 0 moved up two places.
        assert_eq!(z_order(4, &[0.0, 2.0]), vec![1, 2, 0, 3]);
        assert_eq!(z_order(3, &[9.0, 1.0, -1.0, 1.0]), vec![0, 1, 2]);
    }

    #[test]
    fn easing_numbers() {
        assert_eq!(db_ease(0.5, 0.0), 0.5);
        assert!(db_ease(0.25, 1.0) > 0.25);
        assert!(db_ease(0.25, -1.0) < 0.25);
        assert!((db_ease(0.5, 2.0) - 0.5).abs() < 1e-6);
    }
}
