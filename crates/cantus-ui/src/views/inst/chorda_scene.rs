//! Chorda's scene (round 37, ADR 0104): the piano in its recording room — the hero of the
//! Play view. It follows the instrument: the body and finish its look declares
//! (`cantus_core::inst::chorda_look`), the lid at the Lid control's angle, the microphones
//! where the perspective and the width put them, the room the Room controls make. It is a
//! picture only: it reads the spec, never writes the sound.
//!
//! **Baked by our own renderer.** The scene is built in 3-D from geometry (`piano.rs`,
//! `room.rs`, `gear.rs` into `geo.rs`'s triangles) and rendered on a worker thread by
//! `raster.rs` — depth buffer, per-pixel materials and light (`shade.rs`), shadows,
//! occlusion, reflections, depth of field, bloom — into an image shown as one textured quad.
//! No photograph and no stock asset: every pixel is computed by this code. While a control
//! glides, quick previews follow it; once it rests, the full bake replaces them. A frame at
//! rest paints the cached quad and allocates nothing.

#[path = "chorda_scene_gear.rs"]
mod gear;
#[path = "chorda_scene_geo.rs"]
pub mod geo;
#[path = "chorda_scene_mesh.rs"]
pub mod mesh;
#[path = "chorda_scene_piano.rs"]
mod piano;
#[path = "chorda_scene_raster.rs"]
pub mod raster;
#[path = "chorda_scene_room.rs"]
mod room;
#[path = "chorda_scene_shade.rs"]
mod shade;

use crate::kit::skin::scene as pal;
use cantus_core::inst::chorda_look::{Body, Finish, Look};
use cantus_core::NativeSpec;
use egui::epaint::{Mesh, Vertex};
use egui::{Color32, Pos2, Rect, Vec2};
use mesh::{lscale, v3, Camera};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

pub use raster::Quality;

/// What the picture shows: the look and the controls it follows.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Inputs {
    pub look: Look,
    pub lid: f32,
    pub persp: f32,
    pub width: f32,
    pub room: f32,
    pub size: f32,
    /// A dark theme's evening stage, or a light one's daylight.
    pub dark: bool,
}

impl Inputs {
    /// What `spec` shows (its look and its stage's controls).
    pub fn of(spec: &NativeSpec, dark: bool) -> Inputs {
        Inputs {
            look: cantus_core::inst::chorda_look::of(spec),
            lid: spec.get("lid"),
            persp: spec.get("persp"),
            width: spec.get("width"),
            room: spec.get("room"),
            size: spec.get("roomsize"),
            dark,
        }
    }

    fn controls(&self) -> [f32; 5] {
        [self.lid, self.persp, self.width, self.room, self.size]
    }

    fn with_controls(mut self, c: [f32; 5]) -> Inputs {
        [self.lid, self.persp, self.width, self.room, self.size] = c;
        self
    }
}

/// Where the scene's parts land (in points from the picture's corner), for tests.
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Probe {
    pub body: Option<Body>,
    /// The lid's far corner over the treble, the two main microphones, the room's corner.
    pub lid_tip: Pos2,
    pub mics: [Pos2; 2],
    pub corner: Pos2,
    pub room_mics: bool,
    pub digest: u64,
}

/// The finish's paint.
pub fn paint_of(f: Finish) -> pal::Paint {
    match f {
        Finish::Black => pal::BLACK,
        Finish::White => pal::WHITE,
        Finish::Walnut => pal::WALNUT,
        Finish::Rosewood => pal::ROSEWOOD,
    }
}

/// The camera on `p`, framing it in a picture `size` points large (origin at its corner):
/// a three-quarter view from the treble side and the front, a little above (a grand's lid
/// opens toward it), lower in a bigger room so its walls rise into the frame.
fn camera(p: &piano::Piano, size: Vec2, room_size: f32) -> Camera {
    let lower = 9.0 * room_size.clamp(0.0, 1.0).powi(2);
    let (az, el, dist) = if p.body == Body::Upright {
        (56f32.to_radians(), (17.0 - lower * 0.6).to_radians(), 6.2)
    } else {
        (31f32.to_radians(), (21.0 - lower).to_radians(), 8.6)
    };
    let target = p.centre();
    let eye = target + v3(az.cos() * el.cos(), el.sin(), az.sin() * el.cos()) * dist;
    let unit = Camera::look(eye, target, 1.0, Pos2::ZERO);
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for q in p.frame_points() {
        let s = unit.project(q).to_vec2();
        lo = lo.min(s);
        hi = hi.max(s);
    }
    let span = (hi - lo).max(Vec2::splat(1e-3));
    let k = (size.x * 0.64 / span.x).min(size.y * 0.70 / span.y);
    let mid = (lo + hi) * 0.5;
    let want = Pos2::new(size.x * 0.43, size.y * 0.57);
    Camera::look(eye, target, k, want - mid * k)
}

type Plan = ((f32, f32), (f32, f32));

/// The rug's rect under a piano.
fn rug_of(p: &piano::Piano) -> Plan {
    if p.body == Body::Upright {
        ((-0.5, -p.l + 0.05), (p.w + 0.6, 1.5))
    } else {
        ((-0.45, -p.l - 0.35), (p.w + 0.55, 1.15))
    }
}

/// Where the parts land, without building the scene (cheap: a few projections).
pub fn probe(i: &Inputs, size: Vec2) -> Probe {
    let p = piano::Piano::of(i.look.body);
    let cam = camera(&p, size, i.size);
    let room = room::Room::of(i.size, i.room, p.l);
    let pair = gear::main_pair(&p, i.persp, i.width, i.lid);
    let m = pair.mics();
    let lid_tip = cam.project(p.lid_tip(i.lid));
    let mics = [cam.project(m[0].0), cam.project(m[1].0)];
    let corner = cam.project(v3(room.x0, 0.0, room.z0));
    let room_mics = gear::room_pair(&p, &room).is_some();
    let mut digest = 0xcbf2_9ce4_8422_2325u64;
    let mut mix = |v: f32| {
        digest ^= v.to_bits() as u64;
        digest = digest.wrapping_mul(0x0100_0000_01b3);
    };
    for q in [lid_tip, mics[0], mics[1], corner] {
        mix(q.x);
        mix(q.y);
    }
    for c in i.controls() {
        mix(c);
    }
    mix(i.look.body as u8 as f32);
    mix(i.look.finish as u8 as f32);
    Probe {
        body: Some(p.body),
        lid_tip,
        mics,
        corner,
        room_mics,
        digest,
    }
}

/// The scene's light, from the theme, round the piano seen by `cam`.
fn stage_of(
    i: &Inputs,
    cam: &Camera,
    p: piano::Piano,
    room: room::Room,
    rug: Option<Plan>,
) -> shade::Stage {
    let l = if i.dark { pal::EVENING } else { pal::DAYLIGHT };
    let left = -cam.right;
    let toward = -cam.fwd;
    let up = v3(0.0, 1.0, 0.0);
    let centre = p.centre();
    // The key from the camera's side, high: it reaches under an open lid, as a
    // photographer's key does.
    let key_pos = centre - left * 1.2 + up * 3.6 + toward * 2.6;
    let key_aim = (centre - v3(0.0, 0.3, 0.0) - key_pos).norm();
    let soft_c = key_pos + up * 0.4 + left * 0.8;
    let su = key_aim.cross(up).norm() * 0.9;
    let sv = su.cross(key_aim).norm() * 0.55;
    shade::Stage {
        cam: *cam,
        key_pos,
        key_aim,
        key: lscale(l.key, 1.1),
        cone: (0.62, 0.9),
        sky: mesh::ladd(lscale(l.ambient, 1.25), lscale(l.sky, 0.25)),
        ground: lscale(mesh::ladd(l.ambient, l.ground), 0.55),
        rim_dir: (-toward * 0.7 + up * 0.55 - left * 0.35).norm(),
        rim: lscale(l.key, 0.3),
        soft: (soft_c, su, sv, lscale(l.softbox, 1.2)),
        haze: l.haze,
        haze_k: 0.004 + 0.03 * room.size,
        glow: l.glow,
        piano: p,
        room,
        finish: paint_of(i.look.finish),
        rug,
    }
}

/// Build the whole scene's geometry into `g` (the room left out of a mirror's pass).
fn geometry(g: &mut geo::Geo, i: &Inputs, p: &piano::Piano, room: &room::Room, rug: Option<Plan>) {
    if !g.mirror {
        room::build(g, room, rug);
    }
    if let Some(rp) = gear::room_pair(p, room) {
        gear::build(g, &rp, false);
    }
    if p.body == Body::Upright {
        piano::upright(g, p, i.lid);
    } else {
        piano::grand(g, p, i.lid);
    }
    let pair = gear::main_pair(p, i.persp, i.width, i.lid);
    gear::build(g, &pair, true);
}

/// How many triangles the scene of `i` has (the budget test's bound).
pub fn triangles(i: &Inputs, size: Vec2) -> usize {
    let p = piano::Piano::of(i.look.body);
    let cam = camera(&p, size, i.size);
    let room = room::Room::of(i.size, i.room, p.l);
    let mut g = geo::Geo::new(cam, 1.0);
    geometry(&mut g, i, &p, &room, room.has_rug().then(|| rug_of(&p)));
    g.tris.len()
}

/// The most samples one bake may hold (its peak memory is about 45–60 bytes a sample: some
/// 180 MB at this cap, whatever the window).
pub const MAX_SAMPLES: usize = 3_000_000;

/// The pixels a point and the samples a pixel side a bake of `size` points at `ppp` uses:
/// no side past `max_side` (the GPU's largest texture), no more than [`MAX_SAMPLES`]
/// samples (a larger picture is baked smaller and drawn stretched — on a multi-4K window).
pub fn bake_shape(size: Vec2, ppp: f32, q: Quality, max_side: usize) -> (f32, usize) {
    let longest = size.x.max(size.y).max(1.0);
    let mut ppp = ppp.min(max_side as f32 / longest);
    let area = (size.x * size.y).max(1.0);
    if area * ppp * ppp > MAX_SAMPLES as f32 {
        ppp = (MAX_SAMPLES as f32 / area).sqrt();
    }
    let px = area * ppp * ppp;
    let mut ss = q.ss.max(1);
    while ss > 1 && px * (ss * ss) as f32 > MAX_SAMPLES as f32 {
        ss -= 1;
    }
    (ppp, ss)
}

/// Render the scene of `i` into an image `size` points large at `ppp` pixels a point (as
/// [`bake_shape`] bounds it).
pub fn render(
    i: &Inputs,
    size: Vec2,
    ppp: f32,
    q: Quality,
    threads: usize,
    max_side: usize,
) -> egui::ColorImage {
    let (ppp, ss) = bake_shape(size, ppp, q, max_side);
    let q = Quality { ss, ..q };
    let (w, h) = (
        ((size.x * ppp).round() as usize).max(8),
        ((size.y * ppp).round() as usize).max(8),
    );
    let p = piano::Piano::of(i.look.body);
    let cpt = camera(&p, size, i.size);
    let cam = Camera {
        k: cpt.k * ppp,
        c: Pos2::new(cpt.c.x * ppp, cpt.c.y * ppp),
        ..cpt
    };
    let room = room::Room::of(i.size, i.room, p.l);
    let rug = room.has_rug().then(|| rug_of(&p));
    let st = stage_of(i, &cam, p, room, rug);
    let mut g = geo::Geo::new(cam, 1.0);
    geometry(&mut g, i, &p, &room, rug);
    let strength = room.mirror();
    let mirror = if q.full && strength > 0.05 {
        let mut mg = geo::Geo::new(cam.mirrored(), 1.0);
        mg.mirror = true;
        geometry(&mut mg, i, &p, &room, rug);
        Some(mg)
    } else {
        None
    };
    raster::bake(
        &g,
        mirror.as_ref().map(|m| (m, strength)),
        &st,
        &cam,
        w,
        h,
        q,
        threads,
    )
}

/// The full bake and the preview a gliding control gets (half the pixels, no extras).
pub const FULL: Quality = Quality { ss: 2, full: true };
pub const PREVIEW: Quality = Quality { ss: 1, full: false };

/// Bake on the calling thread (screenshots, which cannot wait for the worker).
static BLOCKING: AtomicBool = AtomicBool::new(false);

/// Make every scene bake on the thread that paints it (the CPU screenshots).
pub fn set_blocking(on: bool) {
    BLOCKING.store(on, Ordering::Relaxed);
}

fn threads() -> usize {
    std::thread::available_parallelism()
        .map_or(2, |n| n.get())
        .saturating_sub(1)
        .clamp(1, 8)
}

/// What one bake is for.
#[derive(Copy, Clone, Debug, PartialEq)]
struct Want {
    inputs: Inputs,
    px: [u32; 2],
    ppp_bits: u32,
    q: Quality,
}

impl Want {
    /// The picture's identity: what a landed bake must still show to be shown (a look
    /// undone while its bake was on the way must not land over the look now there).
    fn same_picture(&self, o: &Want) -> bool {
        self.inputs.look == o.inputs.look && self.inputs.dark == o.inputs.dark
    }

    fn size(&self) -> Vec2 {
        let wp = f32::from_bits(self.ppp_bits);
        Vec2::new(self.px[0] as f32 / wp, self.px[1] as f32 / wp)
    }
}

struct Job {
    gen: u64,
    want: Want,
    max_side: usize,
}

struct Worker {
    tx: Sender<Job>,
    rx: Receiver<(u64, egui::ColorImage)>,
}

/// For a test only: how long the worker waits before each bake (a bake made to land late).
#[doc(hidden)]
pub static TEST_BAKE_DELAY_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl Worker {
    /// The worker, waking `ctx` whenever a bake lands (the UI never polls by repainting).
    fn start(ctx: egui::Context) -> Option<Worker> {
        let (tx, jobs) = channel::<Job>();
        let (done, rx) = channel();
        std::thread::Builder::new()
            .name("chorda-scene".into())
            .spawn(move || {
                while let Ok(mut job) = jobs.recv() {
                    // Only the newest asked for.
                    while let Ok(newer) = jobs.try_recv() {
                        job = newer;
                    }
                    let delay = TEST_BAKE_DELAY_MS.load(Ordering::Relaxed);
                    if delay > 0 {
                        std::thread::sleep(std::time::Duration::from_millis(delay));
                    }
                    let w = job.want;
                    let ppp = f32::from_bits(w.ppp_bits);
                    let ppp = if w.q.full { ppp } else { ppp * 0.5 };
                    // A bake that fails never leaves the picture waiting: an empty image
                    // comes back in its place (and the log says why).
                    let img = std::panic::catch_unwind(|| {
                        render(&w.inputs, w.size(), ppp, w.q, threads(), job.max_side)
                    })
                    .unwrap_or_else(|_| {
                        log::error!("chorda scene: a bake failed");
                        egui::ColorImage::new([1, 1], vec![Color32::TRANSPARENT])
                    });
                    if done.send((job.gen, img)).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            })
            .ok()?;
        Some(Worker { tx, rx })
    }
}

/// The scene's cache: the baked picture, what it shows, what was asked for, the glide.
#[derive(Default)]
pub struct Scene {
    worker: Option<Worker>,
    tex: Option<egui::TextureHandle>,
    quad: Option<(Arc<Mesh>, Rect, egui::TextureId)>,
    shown: Option<[f32; 5]>,
    last: Option<f64>,
    /// What the texture shows; the bake on its way; the picture this frame wants (at the
    /// size it is painted at).
    showing: Option<Want>,
    asked: Option<(u64, Want)>,
    wanted: Option<Want>,
    size_seen: Option<([u32; 2], u32)>,
    gen: u64,
    pub probe: Probe,
    /// Bakes asked for (the budget test: none at rest) and pictures received.
    pub builds: u64,
    pub uploads: u64,
}

impl std::fmt::Debug for Scene {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scene")
            .field("probe", &self.probe)
            .field("builds", &self.builds)
            .field("uploads", &self.uploads)
            .field("showing", &self.showing)
            .field("asked", &self.asked)
            .field("wanted", &self.wanted)
            .finish()
    }
}

/// The glide's time constant (s): a control turned reaches its picture in about 0.3 s.
const GLIDE: f64 = 0.07;

impl Scene {
    /// Paint the scene for `target` into `rect` with `painter`; `now` the frame's time.
    /// Returns whether a control is still gliding (the caller asks for another frame).
    pub fn paint(&mut self, painter: &egui::Painter, rect: Rect, target: Inputs, now: f64) -> bool {
        let want = target.controls();
        let dt = self.last.map_or(1.0, |t| (now - t).clamp(0.0, 0.25));
        self.last = Some(now);
        let shown = match self.shown {
            None => want,
            Some(s) => {
                let k = (1.0 - (-dt / GLIDE).exp()) as f32;
                std::array::from_fn(|i| {
                    let d = want[i] - s[i];
                    if d.abs() < 0.002 {
                        want[i]
                    } else {
                        s[i] + d * k
                    }
                })
            }
        };
        let moving = shown != want;
        let inputs = target.with_controls(shown);
        if self.shown != Some(shown)
            || self.probe.body != Some(target.look.body)
            || self.probe.digest == 0
        {
            self.probe = probe(&inputs, rect.size());
        }
        self.shown = Some(shown);
        let ctx = painter.ctx();
        let max_side = ctx.input(|i| i.max_texture_side).max(64);
        let ppp = ctx.pixels_per_point();
        let wanted = Want {
            inputs,
            px: [
                (rect.width() * ppp).round() as u32,
                (rect.height() * ppp).round() as u32,
            ],
            ppp_bits: ppp.to_bits(),
            q: if moving { PREVIEW } else { FULL },
        };
        self.wanted = Some(wanted);
        // What to ask for: a picture where there is none comes as a preview first; a new
        // size or scale once it has held for a frame (a window's passing frame, laid out at
        // another size or before its scale has settled, asks for none) — and that next
        // frame is asked for, so the new size is baked without waiting for another event.
        let mut w = wanted;
        if self.showing.is_none() {
            w.q = PREVIEW;
        }
        let size = (wanted.px, wanted.ppp_bits);
        if let Some(have) = self.showing {
            if (have.px, have.ppp_bits) != size && self.size_seen != Some(size) {
                w.px = have.px;
                w.ppp_bits = have.ppp_bits;
                ctx.request_repaint();
            }
        }
        self.size_seen = Some(size);
        if BLOCKING.load(Ordering::Relaxed) {
            // The full bake of what is asked for (its size held a frame, as below).
            let full = Want { q: FULL, ..w };
            if self.showing != Some(full) {
                let wp = f32::from_bits(full.ppp_bits);
                let img = render(&inputs, full.size(), wp, FULL, threads(), max_side);
                self.upload(ctx, img);
                self.showing = Some(full);
                self.builds += 1;
            }
        } else {
            if self.worker.is_none() {
                self.worker = Worker::start(ctx.clone());
            }
            let mut got = None;
            if let Some(wk) = &self.worker {
                while let Ok(r) = wk.rx.try_recv() {
                    got = Some(r);
                }
            }
            if let Some((gen, img)) = got {
                if let Some((asked_gen, asked)) = self.asked {
                    if gen == asked_gen {
                        self.asked = None;
                        // A bake for another look than the one now wanted (undone while
                        // it was on its way) is dropped, never shown.
                        if asked.same_picture(&w) {
                            self.upload(ctx, img);
                            self.showing = Some(asked);
                        }
                    }
                }
            }
            // One bake in flight at a time: the next is asked for when the last has landed
            // (the worker wakes the UI then).
            if self.asked.is_none() && self.showing != Some(w) {
                if let Some(wk) = &self.worker {
                    self.gen += 1;
                    let job = Job {
                        gen: self.gen,
                        want: w,
                        max_side,
                    };
                    if wk.tx.send(job).is_ok() {
                        self.asked = Some((self.gen, w));
                        self.builds += 1;
                    }
                }
            }
        }
        if self.showing.is_some() {
            self.paint_quad(painter, rect);
        }
        moving
    }

    fn upload(&mut self, ctx: &egui::Context, img: egui::ColorImage) {
        match &mut self.tex {
            Some(t) => t.set(img, egui::TextureOptions::LINEAR),
            None => {
                self.tex = Some(ctx.load_texture("chorda-scene", img, egui::TextureOptions::LINEAR))
            }
        }
        self.uploads += 1;
    }

    fn paint_quad(&mut self, painter: &egui::Painter, rect: Rect) {
        let Some(tex) = &self.tex else {
            return;
        };
        let id = tex.id();
        if self.quad.as_ref().map(|q| (q.1, q.2)) != Some((rect, id)) {
            let white = Color32::from(egui::Rgba::WHITE);
            let corner = |p: Pos2, u: f32, v: f32| Vertex {
                pos: p,
                uv: Pos2::new(u, v),
                color: white,
            };
            let mesh = Mesh {
                indices: vec![0, 1, 2, 0, 2, 3],
                vertices: vec![
                    corner(rect.left_top(), 0.0, 0.0),
                    corner(rect.right_top(), 1.0, 0.0),
                    corner(rect.right_bottom(), 1.0, 1.0),
                    corner(rect.left_bottom(), 0.0, 1.0),
                ],
                texture_id: id,
            };
            self.quad = Some((Arc::new(mesh), rect, id));
        }
        if let Some((m, _, _)) = &self.quad {
            painter.add(egui::Shape::Mesh(m.clone()));
        }
    }

    /// Whether the picture shown is the one this frame wants, at its size and quality,
    /// with no bake on its way.
    pub fn ready(&self) -> bool {
        self.wanted.is_some() && self.showing == self.wanted && self.asked.is_none()
    }

    /// What the picture shown shows (its look and controls) and its pixels, if anything.
    pub fn showing(&self) -> Option<(Inputs, [u32; 2])> {
        self.showing.map(|w| (w.inputs, w.px))
    }
}

pub fn caption(i: &Inputs) -> String {
    use crate::i18n::{t, tf};
    let lid = if i.look.body == Body::Upright {
        if i.lid < 0.04 {
            t("chorda.scene.top_closed")
        } else {
            t("chorda.scene.top_open")
        }
    } else if i.lid < 0.04 {
        t("chorda.scene.lid_closed")
    } else if i.lid * piano::FULL_STICK <= 20f32.to_radians() {
        t("chorda.scene.lid_short")
    } else {
        t("chorda.scene.lid_full")
    };
    let mics = if i.persp < 0.18 {
        t("chorda.scene.mics_close")
    } else if i.persp < 0.6 {
        t("chorda.scene.mics_player")
    } else {
        t("chorda.scene.mics_far")
    };
    let room = match room::Room::of(i.size, i.room, 2.0).kind {
        room::Kind::Booth => t("chorda.scene.room_booth"),
        room::Kind::Studio => t("chorda.scene.room_studio"),
        room::Kind::Hall => t("chorda.scene.room_hall"),
        room::Kind::Church => t("chorda.scene.room_church"),
    };
    tf(
        "chorda.scene.caption",
        &[
            ("body", &t(i.look.body.label_key())),
            ("finish", &t(i.look.finish.label_key())),
            ("lid", &lid),
            ("mics", &mics),
            ("room", &room),
        ],
    )
}
