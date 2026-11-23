//! r40:scene-px — the photographic renderer (the owner's decision of 2026-11-12: the
//! instruments' pictures are configurable, *Drawn* — the scene module's projected vectors,
//! the default — or *Photographic*, this). It draws **the designer's scenes**: the scene
//! module's subject (Chorda's pianos in every body, finish and lid; Tympanum's kit and its
//! microphones), its rooms with their floor, walls and rug, its camera fitted exactly as the
//! vector picture fits it, its moods (a dark theme's evening, a light one's daylight) — as
//! closed meshes rendered per sample (`raster.rs`) under a photographer's light (`light.rs`).
//! ADR 0104's per-pixel renderer grown up; the bake runs on the scene's worker, never the
//! UI thread.

#[path = "chorda_scene_px_bowed.rs"]
pub mod bowed;
#[path = "chorda_scene_px_gear.rs"]
pub mod gear;
#[path = "chorda_scene_px_geo.rs"]
pub mod geo;
#[path = "chorda_scene_px_kit.rs"]
pub mod kit;
#[path = "chorda_scene_px_light.rs"]
pub mod light;
#[path = "chorda_scene_px_mat.rs"]
pub mod mat;
#[path = "chorda_scene_px_model.rs"]
pub mod model;
#[path = "chorda_scene_px_piano.rs"]
pub mod piano;
#[path = "chorda_scene_px_pool.rs"]
pub mod pool;
#[path = "chorda_scene_px_raster.rs"]
pub mod raster;
#[path = "chorda_scene_px_room.rs"]
pub mod room;
#[path = "chorda_scene_px_shade.rs"]
pub mod shade;
#[path = "chorda_scene_px_trace.rs"]
pub mod trace;
#[path = "chorda_scene_px_upright.rs"]
pub mod upright;

pub use raster::{Quality, Stats};

use super::mesh;
use crate::kit::skin::scene::{self as pal, Mood};
use crate::scene::compose::{Frame, KIT_FRAME, PIANO_FRAME};
use crate::scene::{finish_mat, Model, PixelBackend, Room, SceneView, Subject};
use cantus_core::inst::chorda_look::Body;
use egui::{Pos2, Vec2};
use light::{Key, Panel, Rig};
use mat::lin3;
use mesh::{lscale, v3, Camera, V3};

/// The quality a picture is baked at: two samples a pixel side, every effect.
pub const FULL: Quality = Quality {
    ss: 2,
    full: true,
    shadow: 2048,
    trace: true,
};

/// A quick look while a control glides: one sample, no occlusion, mirror or bloom.
pub const PREVIEW: Quality = Quality {
    ss: 1,
    full: false,
    shadow: 1024,
    trace: false,
};

/// The box a frame fits the instrument into (points).
fn fit_box(f: &Frame, size: Vec2) -> (f32, f32, f32, f32) {
    let [l, t, r, b] = f.inset;
    (l, t, (size.x - l - r).max(8.0), (size.y - t - b).max(8.0))
}

/// The scene module's camera (cm, points) as this renderer's (m, pixels).
fn camera_of(c: &crate::scene::geo::Camera, ppp: f32) -> Camera {
    Camera {
        eye: geo::of(c.pos),
        right: v3(c.r.x, c.r.y, c.r.z),
        up: v3(c.u.x, c.u.y, c.u.z),
        fwd: v3(c.f.x, c.f.y, c.f.z),
        k: c.s * ppp,
        c: Pos2::new(c.ox * ppp, c.oy * ppp),
        flip: false,
    }
}

/// The mood the scene module lights a room with.
fn mood(dark: bool, stage: bool) -> Mood {
    match (dark, stage) {
        (true, false) => pal::MOOD_EVENING,
        (false, false) => pal::MOOD_DAYLIGHT,
        (true, true) => pal::MOOD_STAGE_EVENING,
        (false, true) => pal::MOOD_STAGE_DAYLIGHT,
    }
}

/// Where the light stands for a subject.
struct Place {
    /// The subject's middle, the key softbox, where its spot falls.
    mid: V3,
    key: V3,
    focus: V3,
    /// In focus up to this far from the eye (m).
    far: f32,
}

/// The photographer's light round a subject.
fn rig(room: Room, evening: bool, m: &Mood, pl: &Place, eye: V3, built: room::Built) -> Rig {
    let stage = room == Room::Stage;
    let kc = lin3(m.key_col);
    let (mid, key_pos, focus) = (pl.mid, pl.key, pl.focus);
    let dist = (focus - key_pos).len();
    let spot = m.spot_r / 100.0 * if stage { 1.2 } else { 2.16 };
    let cone = (
        (spot * 1.9 / dist).atan().cos(),
        (spot * 0.45 / dist).atan().cos(),
    );
    let key = Key {
        pos: key_pos,
        aim: (focus - key_pos).norm(),
        focus,
        cone,
        e0: lscale(kc, m.key * 0.85),
        size: 0.55,
    };
    let white = if evening {
        [1.0, 0.95, 0.88]
    } else {
        [1.0, 1.0, 1.0]
    };
    let warm = [1.0, 0.86, 0.68];
    let cool = [0.78, 0.86, 1.0];
    let s = if stage { 1.0 } else { 0.8 };
    let up = v3(0.0, 1.0, 0.0);
    let back = v3(mid.x - eye.x, 0.0, mid.z - eye.z).norm();
    let side = v3(-back.z, 0.0, back.x);
    let toward_cam = -back;
    // A softbox placed by its angle round the instrument from the camera (0 behind the
    // camera, 180 behind the instrument; positive to the camera's right), its distance
    // from the instrument's middle, its centre's height, its width and height (m).
    let strip = |deg: f32, d: f32, y: f32, w: f32, h: f32, k: f32, soft: f32| {
        let a = deg.to_radians();
        let dir = toward_cam * a.cos() + side * a.sin();
        let c = v3(mid.x + dir.x * d, y, mid.z + dir.z * d);
        Panel::toward(
            c,
            v3(mid.x, y * 0.6, mid.z),
            w,
            h,
            up,
            lscale(white, k * s),
            0.0,
        )
        .soft(soft)
        .core(0.35)
    };
    let mut panels = vec![
        // The key's own face.
        Panel::toward(
            key_pos,
            focus,
            1.3,
            0.9,
            v3(1.0, 0.0, 0.0),
            lscale(kc, m.key * 22.0),
            0.0,
        ),
        // A broad overhead softbox: the line along every top edge, the sheen on the lid.
        Panel::toward(
            mid + v3(0.4, 3.0, 1.6),
            mid + v3(0.4, 0.0, 1.0),
            4.8,
            0.5,
            v3(1.0, 0.0, 0.0),
            lscale(white, 7.0 * s),
            0.02,
        ),
        // Floor-standing strips out of frame, close enough that the lacquered sides (which
        // see the floor) catch them: the long crisp bars down the case, along chrome and
        // bronze; a large soft one in front that the desk and the fallboard show.
        strip(-95.0, 3.0, 1.4, 1.6, 1.6, 4.0, 1.0),
        strip(130.0, 2.4, 1.2, 0.3, 3.0, 16.0, 0.1),
        strip(45.0, 2.4, 1.2, 0.25, 3.0, 18.0, 0.1),
        // A cool strip behind, opposite the camera: the rim on the far edges.
        Panel::toward(
            mid + back * 4.2 - side * 1.2 + v3(0.0, 1.6, 0.0),
            mid,
            0.6,
            3.0,
            up,
            lscale(cool, 6.0 * s),
            0.0,
        ),
        // A broad warm wash high over the stage, seen in no reflection: the floor and the
        // instrument's surroundings out of the dark.
        Panel::toward(
            mid + v3(0.0, 6.0, 0.5),
            mid,
            8.0,
            8.0,
            v3(1.0, 0.0, 0.0),
            lscale(warm, 0.6 * s),
            1.0,
        )
        .unseen(),
        // A warm bounce board low on the bass side: the lid's underside and the case pick it up.
        Panel::toward(
            mid + v3(-2.6, 0.5, 0.4),
            mid + v3(0.0, 0.6, 0.0),
            2.0,
            1.2,
            up,
            lscale(warm, 1.6 * s),
            0.12,
        ),
    ];
    if !stage {
        // A room is lit all round, not only by the key's pool: a broad soft light from the
        // camera's side, seen in no reflection, that shows its walls and floor.
        let c = v3(mid.x, 2.2, mid.z) + toward_cam * 4.5;
        let k = if evening { 0.45 } else { 2.0 };
        panels
            .push(Panel::toward(c, c - toward_cam, 10.0, 5.0, up, lscale(white, k), 1.0).unseen());
    }
    panels.extend(built.panels);
    let amb = m.amb * if evening { 0.55 } else { 0.42 };
    let sky = lscale(lin3(m.amb_col), amb);
    let ground = lscale(lin3([0.62, 0.55, 0.48]), amb * 0.5);
    // Above a stage is its dark fly tower; a room's ceiling sits in its own shade.
    // A stage's fly tower is dark by day too (round 40: a grey void washed the Paper
    // stage out); a room's ceiling sits in its own shade.
    let void = if stage {
        lscale(lin3([0.30, 0.27, 0.24]), if evening { 0.012 } else { 0.06 })
    } else {
        lscale(sky, 0.35)
    };
    Rig {
        key,
        key_panel: Some(0),
        panels,
        sky,
        ground,
        void,
        rims: vec![(
            (back + up * 0.6 - side * 0.3).norm(),
            lscale(cool, if evening { 0.35 } else { 0.25 }),
        )],
        lamps: built.lamps,
        room: built.env,
        plate: None,
        haze: lscale(lin3(m.haze), 0.6),
        haze_k: (if evening { 1.0 } else { 0.3 })
            * match room {
                Room::Stage => 0.05,
                Room::Booth => 0.0,
                Room::Studio => 0.02,
                Room::Hall => 0.06,
                Room::Church => 0.07,
            },
        evening,
        exposure: if evening { 1.25 } else { 1.15 },
        focus_far: pl.far,
        eye,
    }
}

/// How far from the eye the farthest of `pts` (cm) stands, and a margin.
fn far_of(cam: &Camera, pts: &[crate::scene::geo::V3]) -> f32 {
    pts.iter()
        .map(|&p| (geo::of(p) - cam.eye).dot(cam.fwd))
        .fold(0.0f32, f32::max)
        + 0.6
}

/// At most this many samples a bake (a bake's peak is about 30 bytes a sample on top of
/// its scene): a larger picture drops to one sample a pixel side first, then is baked
/// smaller (the caller draws it stretched).
pub const MAX_SAMPLES: usize = 3_200_000;
/// No side past this (the GPU's texture limit on the smallest machines).
pub const MAX_SIDE: usize = 8192;

/// The pixels a bake of `size` points at `ppp` makes, and the quality it can afford.
pub fn shape(size: Vec2, ppp: f32, q: Quality) -> (usize, usize, Quality) {
    let (mut w, mut h) = (
        (size.x * ppp).round().max(8.0),
        (size.y * ppp).round().max(8.0),
    );
    let side = w.max(h);
    if side > MAX_SIDE as f32 {
        w *= MAX_SIDE as f32 / side;
        h *= MAX_SIDE as f32 / side;
    }
    let mut q = q;
    while q.ss > 1 && (w * h) as usize * q.ss * q.ss > MAX_SAMPLES {
        q.ss -= 1;
    }
    if (w * h) as usize > MAX_SAMPLES {
        let k = (MAX_SAMPLES as f32 / (w * h)).sqrt();
        w *= k;
        h *= k;
    }
    ((w.floor() as usize).max(8), (h.floor() as usize).max(8), q)
}

/// The picture of `view` `size` points large at `ppp` pixels a point (within the bounds of
/// [`shape`]), and what it cost; `threads` bake it.
pub fn render(
    view: &SceneView,
    size: Vec2,
    ppp: f32,
    q: Quality,
    threads: usize,
) -> (egui::ColorImage, Stats) {
    let (w, h, q) = shape(size, ppp, q);
    render_px(view, size, w, h, q, threads)
}

/// The picture of `view` (`size` points) baked `w` × `h` pixels, with no bound (pictures for
/// the documents: a 3× crop).
pub fn render_px(
    view: &SceneView,
    size: Vec2,
    w: usize,
    h: usize,
    q: Quality,
    threads: usize,
) -> (egui::ColorImage, Stats) {
    bake_view(view, size, w, h, q, threads, None)
}

/// The bake of `view`, its camera fitted as the scene module fits it — or `fitted`, the
/// scene module's own (a model's: its framing, whatever the sheet asked for).
#[allow(clippy::too_many_arguments)]
fn bake_view(
    view: &SceneView,
    size: Vec2,
    w: usize,
    h: usize,
    q: Quality,
    threads: usize,
    fitted: Option<&crate::scene::geo::Camera>,
) -> (egui::ColorImage, Stats) {
    let stage = view.room == crate::scene::Room::Stage;
    let m = mood(view.dark, stage);
    let ppp = w as f32 / size.x.max(1.0);
    let mut g = geo::Geo::default();
    match &view.subject {
        Subject::Piano {
            look,
            lid,
            persp,
            width,
            room_level,
            mics,
        } => {
            use crate::scene::piano as sp;
            let upright = look.body == Body::Upright;
            let full = if upright {
                sp::UPRIGHT_LID
            } else {
                sp::GRAND_LID
            };
            let mut sc = sp::camera(look.body, view.room);
            sc.fit(
                &sp::points(look.body, full),
                fit_box(&PIANO_FRAME, size),
                PIANO_FRAME.margin,
                PIANO_FRAME.anchor,
            );
            if let Some(c) = fitted {
                sc = *c;
            }
            let cam = camera_of(&sc, ppp);
            let cz = if upright { -20.0 } else { -90.0 };
            let built = room::build(
                &mut g,
                view.room,
                (0.0, cz),
                view.look,
                sc.pos.z,
                view.dark,
                m.glow,
            );
            let split = g.tris.len();
            let fin = finish_mat(look.finish);
            piano::build(&mut g, look.body, &fin, lid.clamp(0.0, 1.0) * full);
            if *mics {
                let km = kit::KitMats::new(&mut g, &fin);
                let gm = gear::GearMats::new(&mut g, km);
                gear::piano(
                    &mut g,
                    &gm,
                    *persp,
                    *width,
                    *room_level,
                    upright,
                    built.env.z0 * 100.0,
                );
            }
            let pl = Place {
                mid: if upright {
                    geo::cm(0.0, 70.0, -25.0)
                } else {
                    geo::cm(0.0, 80.0, -0.42 * sp::length(look.body))
                },
                key: if upright {
                    geo::cm(140.0, 380.0, 220.0)
                } else {
                    geo::cm(320.0, 300.0, 40.0)
                },
                focus: geo::cm(0.0, 0.0, if upright { -10.0 } else { -90.0 }),
                far: far_of(&cam, &sp::points(look.body, full)),
            };
            let mut r = rig(view.room, view.dark, &m, &pl, cam.eye, built);
            if !upright && *lid > 0.03 {
                let o = sp::outline(look.body, 6);
                r.plate = Some(light::Plate {
                    y: (sp::RIM_HI - 7.0) / 100.0,
                    outline: o
                        .iter()
                        .map(|&(x, z)| (x * 0.94 / 100.0, (z * 0.97 - 2.0) / 100.0))
                        .collect(),
                });
            }
            raster::bake(&g, split, &r, &cam, w, h, q, threads)
        }
        Subject::Kit {
            def, finish, mics, ..
        } => {
            use crate::scene::kit as sk;
            let pieces = sk::of_def(def);
            let el = match view.room {
                crate::scene::Room::Stage => 15.0,
                crate::scene::Room::Hall => 18.0,
                _ => 22.0,
            };
            let mut sc = crate::scene::geo::Camera::new(
                crate::scene::geo::v(-4.0, 80.0, 0.0),
                16.0,
                el,
                600.0,
            );
            sc.fit(
                &sk::points(&pieces),
                fit_box(&KIT_FRAME, size),
                KIT_FRAME.margin,
                KIT_FRAME.anchor,
            );
            if let Some(c) = fitted {
                sc = *c;
            }
            let cam = camera_of(&sc, ppp);
            let built = room::build(
                &mut g,
                view.room,
                (-8.0, -4.0),
                view.look,
                sc.pos.z,
                view.dark,
                m.glow,
            );
            let split = g.tris.len();
            let fin = finish_mat(*finish);
            kit::build(&mut g, &pieces, &fin);
            if *mics {
                let km = kit::KitMats::new(&mut g, &fin);
                let gm = gear::GearMats::new(&mut g, km);
                gear::kit(&mut g, &gm, &crate::scene::kit_mics(def));
            }
            let pl = Place {
                mid: geo::cm(-6.0, 80.0, -4.0),
                key: geo::cm(120.0, 400.0, 220.0),
                focus: geo::cm(-6.0, 0.0, -4.0),
                far: far_of(&cam, &sk::points(&pieces)),
            };
            let r = rig(view.room, view.dark, &m, &pl, cam.eye, built);
            raster::bake(&g, split, &r, &cam, w, h, q, threads)
        }
        Subject::Bowed { instrument } => {
            // r42:arcus-look — Arcus's cello or double bass, framed as the Drawn picture
            // frames it.
            use crate::scene::bowed_draw as bd;
            let b = *instrument;
            let d = b.dims();
            let mut sc = bd::camera(b, view.room);
            let pts = bd::points(b);
            let fr = bd::frame_for(size);
            sc.fit(&pts, fit_box(&fr, size), fr.margin, fr.anchor);
            if let Some(c) = fitted {
                sc = *c;
            }
            let cam = camera_of(&sc, ppp);
            let mid = crate::scene::bowed::Pose::of(b).at(0.0, d.body * 0.5, 0.0);
            let built = room::build(
                &mut g,
                view.room,
                (mid.x, mid.z),
                view.look,
                sc.pos.z,
                view.dark,
                m.glow,
            );
            let split = g.tris.len();
            bowed::build(&mut g, b);
            let pl = Place {
                mid: geo::cm(mid.x, mid.y, mid.z),
                key: geo::cm(mid.x - 110.0, 360.0, mid.z + 200.0),
                focus: geo::cm(mid.x, 0.0, mid.z),
                far: far_of(&cam, &pts),
            };
            let r = rig(view.room, view.dark, &m, &pl, cam.eye, built);
            raster::bake(&g, split, &r, &cam, w, h, q, threads)
        }
    }
}

/// How many parts a bake is split into: the bake pool's threads (at least two, so a bake
/// always runs on the pool, below normal priority, never on the caller).
pub fn threads() -> usize {
    pool::workers().max(2)
}

/// The picture of the scene module's `model` (its captured primitives, camera, light and
/// room) at `ppp` pixels a point, within the bounds of [`shape`].
pub fn render_model(
    model: &Model,
    ppp: f32,
    q: Quality,
    threads: usize,
) -> (egui::ColorImage, Stats) {
    let (w, h, q) = shape(model.size, ppp, q);
    let ppp = w as f32 / model.size.x.max(1.0);
    let cam = camera_of(&model.cam, ppp);
    let evening = model.evening;
    let m = mood(evening, model.room == Room::Stage);
    let mut g = geo::Geo::default();
    let f = model.light.focus;
    let built = room::build(
        &mut g,
        model.room,
        (f.x, f.z),
        model.look,
        model.cam.pos.z,
        evening,
        m.glow,
    );
    let split = g.tris.len();
    model::build(&mut g, model);
    let (mut lo, mut hi) = (
        v3(f32::MAX, f32::MAX, f32::MAX),
        v3(f32::MIN, f32::MIN, f32::MIN),
    );
    for t in &g.tris[split..] {
        for v in &t.v {
            lo = v3(lo.x.min(v.p.x), lo.y.min(v.p.y), lo.z.min(v.p.z));
            hi = v3(hi.x.max(v.p.x), hi.y.max(v.p.y), hi.z.max(v.p.z));
        }
    }
    let mid = if lo.x <= hi.x {
        (lo + hi) * 0.5
    } else {
        geo::of(f)
    };
    let far = [lo, hi, v3(lo.x, lo.y, hi.z), v3(hi.x, lo.y, lo.z)]
        .iter()
        .map(|&p| (p - cam.eye).dot(cam.fwd))
        .fold(0.0f32, f32::max)
        + 0.6;
    let pl = Place {
        mid,
        key: geo::of(model.light.key_pos),
        focus: geo::of(f),
        far,
    };
    let r = rig(model.room, evening, &m, &pl, cam.eye, built);
    raster::bake(&g, split, &r, &cam, w, h, q, threads)
}

/// The photographic renderer as the scene module's per-pixel backend (*Instrument
/// pictures: Photographic*): registered once at start-up.
pub struct Photographic;

impl PixelBackend for Photographic {
    /// While a control glides (`quick`), the preview: a sample a pixel, no occlusion,
    /// mirror, traced reflection or bloom; at rest, the full bake.
    fn render(&self, model: &Model, ppp: f32, quick: bool) -> egui::ColorImage {
        if quick {
            return render_of(model, ppp, PREVIEW, threads()).0;
        }
        let key = recent_key(model, ppp);
        if let Some(img) = recent(key) {
            return img;
        }
        let img = render_of(model, ppp, FULL, threads()).0;
        keep(key, &img);
        img
    }
}

/// How many full pictures are kept, newest first: going back to a look just seen (a theme,
/// a finish, a room toggled and toggled back) shows it again with no bake.
pub const RECENT: usize = 3;

static RECENT_BAKES: std::sync::Mutex<Vec<(u64, egui::ColorImage)>> =
    std::sync::Mutex::new(Vec::new());

/// What a picture depends on, hashed: the subject, the room and its look, the mood, the
/// camera, the size and the pixels a point.
fn recent_key(model: &Model, ppp: f32) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    format!(
        "{:?}|{:?}|{:?}|{}|{:?}|{:?}|{}",
        model.subject, model.room, model.look, model.evening, model.cam, model.size, ppp
    )
    .hash(&mut h);
    if model.subject.is_none() {
        format!("{:?}", model.prims).hash(&mut h);
    }
    h.finish()
}

fn recent(key: u64) -> Option<egui::ColorImage> {
    let mut g = RECENT_BAKES.lock().ok()?;
    let i = g.iter().position(|(k, _)| *k == key)?;
    let e = g.remove(i);
    let img = e.1.clone();
    g.insert(0, e);
    Some(img)
}

fn keep(key: u64, img: &egui::ColorImage) {
    if let Ok(mut g) = RECENT_BAKES.lock() {
        g.retain(|(k, _)| *k != key);
        g.insert(0, (key, img.clone()));
        g.truncate(RECENT);
    }
}

/// How many full pictures are kept now (tests).
pub fn recent_count() -> usize {
    RECENT_BAKES.lock().map(|g| g.len()).unwrap_or(0)
}

/// The picture of `model`: from its subject (this renderer's own closed meshes of the
/// designer's instrument) when it carries one, from its captured primitives otherwise.
pub fn render_of(model: &Model, ppp: f32, q: Quality, threads: usize) -> (egui::ColorImage, Stats) {
    match &model.subject {
        Some(subject) => {
            let view = SceneView {
                subject: subject.clone(),
                room: model.room,
                look: model.look,
                dark: model.evening,
                selected: None,
                hovered: None,
                sounding: Vec::new(),
                accent: egui::Color32::WHITE,
                ink: egui::Color32::WHITE,
            };
            let (w, h, q) = shape(model.size, ppp, q);
            bake_view(&view, model.size, w, h, q, threads, Some(&model.cam))
        }
        None => render_model(model, ppp, q, threads),
    }
}

/// Register the photographic renderer with the scene module (at start-up; again is a no-op).
pub fn register() -> bool {
    crate::scene::backend::register_pixel(std::sync::Arc::new(Photographic))
}
