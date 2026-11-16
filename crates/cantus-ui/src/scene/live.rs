//! A scene on screen: its picture built and drawn on a worker whenever what it shows
//! changes (the newest asked wins; a stale build is dropped), uploaded once, painted as one
//! quad at the screen's pixels; the selection and the pointer's outline drawn over it from
//! the same build's outlines (a cached mesh); the live controls (the lid, Perspective,
//! Width, the room pair's level) gliding to where they are set. A scene at rest asks for no
//! repaint and allocates nothing.

use super::compose::Built;
use super::display::Part;
use super::{build, draw, pick_in, Picked, SceneView, Subject, Want};
use egui::epaint::{Mesh, PathShape, PathStroke, Tessellator, Vertex};
use egui::{Color32, Pos2, Rect, Shape, Vec2};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

/// Build on the thread that paints (the CPU screenshots, which cannot wait).
static BLOCKING: AtomicBool = AtomicBool::new(false);

/// Make every scene build on the thread that paints it.
pub fn set_blocking(on: bool) {
    BLOCKING.store(on, Ordering::Relaxed);
}

/// The picture of a build at `ppp` pixels a point: the per-pixel backend's when the build
/// kept its model for one, the vector picture's otherwise.
pub fn to_image(b: &Built, ppp: f32, quick: bool) -> egui::ColorImage {
    if let (Some(m), Some(px)) = (&b.model, super::backend::pixel()) {
        return px.render(m, ppp, quick);
    }
    let ground = super::light::Light::new(
        b.evening,
        false,
        super::geo::V3::default(),
        super::geo::V3::default(),
    )
    .m
    .sky;
    match draw::render(&b.list, b.size, ppp, ground, b.gloss) {
        Some(pix) => {
            let size = [pix.width() as usize, pix.height() as usize];
            egui::ColorImage::from_rgba_premultiplied(size, pix.data())
        }
        None => egui::ColorImage::new([1, 1], vec![Color32::TRANSPARENT]),
    }
}

struct Job {
    gen: u64,
    want: Want,
}

type Landed = (u64, Arc<Built>, egui::ColorImage, f64);

/// r40:fix-f (review of 40a27bb7, finding 6) — one worker thread builds every scene's
/// pictures (it used to be one OS thread per scene, never freed while the scene stayed
/// cached): each scene hands it jobs with its own reply channel; the newest job of a scene
/// wins, and a scene dropped meanwhile is skipped.
struct Shared {
    alive: Arc<AtomicBool>,
    job: Job,
    reply: Sender<Landed>,
    ctx: egui::Context,
}

static POOL: std::sync::Mutex<Option<Sender<Shared>>> = std::sync::Mutex::new(None);
static WORKERS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static THREADS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// How many scenes hold a place in the shared worker, and how many worker threads run
/// (0 or 1): what the tests measure against a baseline.
pub fn workers() -> (usize, usize) {
    (
        WORKERS.load(Ordering::Relaxed),
        THREADS.load(Ordering::Relaxed),
    )
}

fn pool() -> Option<Sender<Shared>> {
    let mut g = POOL.lock().ok()?;
    if let Some(tx) = g.as_ref() {
        return Some(tx.clone());
    }
    let (tx, jobs) = channel::<Shared>();
    std::thread::Builder::new()
        .name("scene".into())
        .spawn(move || {
            THREADS.fetch_add(1, Ordering::Relaxed);
            while let Ok(first) = jobs.recv() {
                // The newest job of each scene, in the order the scenes asked.
                let mut queue = vec![first];
                while let Ok(more) = jobs.try_recv() {
                    queue.push(more);
                }
                let mut take: Vec<Shared> = Vec::with_capacity(queue.len());
                for j in queue {
                    match take.iter_mut().find(|t| Arc::ptr_eq(&t.alive, &j.alive)) {
                        Some(t) => *t = j,
                        None => take.push(j),
                    }
                }
                for j in take {
                    if !j.alive.load(Ordering::Relaxed) {
                        continue;
                    }
                    let t0 = std::time::Instant::now();
                    let got = std::panic::catch_unwind(|| {
                        let b = build(&j.job.want);
                        let img = to_image(&b, j.job.want.ppp, j.job.want.quick);
                        (Arc::new(b), img)
                    });
                    let Ok((b, img)) = got else {
                        log::error!("scene: a build failed");
                        continue;
                    };
                    let ms = t0.elapsed().as_secs_f64() * 1e3;
                    if j.reply.send((j.job.gen, b, img, ms)).is_ok() {
                        j.ctx.request_repaint();
                    }
                }
            }
            THREADS.fetch_sub(1, Ordering::Relaxed);
        })
        .ok()?;
    *g = Some(tx.clone());
    Some(tx)
}

/// One scene's place in the shared worker.
struct Worker {
    ctx: egui::Context,
    alive: Arc<AtomicBool>,
    pool: Sender<Shared>,
    reply: Sender<Landed>,
    rx: Receiver<Landed>,
}

impl Worker {
    fn start(ctx: egui::Context) -> Option<Worker> {
        let pool = pool()?;
        let (reply, rx) = channel();
        WORKERS.fetch_add(1, Ordering::Relaxed);
        Some(Worker {
            ctx,
            alive: Arc::new(AtomicBool::new(true)),
            pool,
            reply,
            rx,
        })
    }

    fn send(&self, job: Job) -> bool {
        self.pool
            .send(Shared {
                alive: self.alive.clone(),
                job,
                reply: self.reply.clone(),
                ctx: self.ctx.clone(),
            })
            .is_ok()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Relaxed);
        WORKERS.fetch_sub(1, Ordering::Relaxed);
    }
}

fn c32(c: crate::kit::skin::scene::C3) -> Color32 {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgb(b(c[0]), b(c[1]), b(c[2]))
}

/// The halo drawn under an outline of `accent`: dark or light, whichever stands further
/// from it — the outline then reads at its contrast with the halo on any picture.
pub fn halo_for(accent: Color32) -> Color32 {
    use crate::kit::color::contrast;
    use crate::kit::skin::scene::ink;
    let (d, l) = (c32(ink::HALO_DARK), c32(ink::HALO_LIGHT));
    if contrast(accent, d) >= contrast(accent, l) {
        d
    } else {
        l
    }
}

/// The glide's time constant (s): a control turned reaches its picture in about 0.3 s.
const GLIDE: f64 = 0.07;

/// What the overlay shows (its mesh is made again only when this changes).
#[derive(Clone, Debug, PartialEq)]
struct OverlayKey {
    gen: u64,
    rect: Rect,
    selected: Option<Picked>,
    hovered: Option<Picked>,
    accent: Color32,
    ink: Color32,
    ppp: f32,
}

/// One scene's cache.
#[derive(Default)]
pub struct Scene {
    worker: Option<Worker>,
    shown: Option<(u64, Want, Arc<Built>)>,
    asked: Option<(u64, Want)>,
    gen: u64,
    tex: Option<egui::TextureHandle>,
    quad: Option<(Arc<Mesh>, Rect, egui::TextureId)>,
    overlay: Option<(OverlayKey, Arc<Mesh>)>,
    glide: Option<[f32; 4]>,
    last: Option<f64>,
    size_seen: Option<(Vec2, u32)>,
    /// Where the picture was last painted (screen points).
    rect: Option<Rect>,
    /// Builds asked for (none at rest), pictures received, the last build's time (ms).
    pub builds: u64,
    pub uploads: u64,
    pub last_ms: f64,
}

impl std::fmt::Debug for Scene {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scene")
            .field("builds", &self.builds)
            .field("uploads", &self.uploads)
            .field("asked", &self.asked.as_ref().map(|a| a.0))
            .finish()
    }
}

fn controls(s: &Subject) -> Option<[f32; 4]> {
    match s {
        Subject::Piano {
            lid,
            persp,
            width,
            room_level,
            ..
        } => Some([*lid, *persp, *width, *room_level]),
        Subject::Kit { .. } => None,
    }
}

fn with_controls(s: &mut Subject, c: [f32; 4]) {
    if let Subject::Piano {
        lid,
        persp,
        width,
        room_level,
        ..
    } = s
    {
        [*lid, *persp, *width, *room_level] = c;
    }
}

impl Scene {
    /// Paint `view` into `rect`. Returns whether a control still glides (the caller asks
    /// for another frame); a build on its way wakes the UI itself when it lands.
    pub fn paint(&mut self, painter: &egui::Painter, rect: Rect, view: &SceneView) -> bool {
        let ctx = painter.ctx();
        let ppp = ctx.pixels_per_point();
        let now = ctx.input(|i| i.time);
        // The picture on the screen's pixel grid.
        let min = Pos2::new(
            (rect.min.x * ppp).round() / ppp,
            (rect.min.y * ppp).round() / ppp,
        );
        let px = Vec2::new(
            (rect.width() * ppp).round().max(1.0),
            (rect.height() * ppp).round().max(1.0),
        );
        let rect = Rect::from_min_size(min, px / ppp);
        self.rect = Some(rect);
        let mut target = view.subject.clone();
        let moving = self.glide_to(&mut target, now);
        let mut want = Want {
            subject: target,
            room: view.room,
            look: view.look,
            dark: view.dark,
            size: rect.size(),
            ppp,
            backend: super::backend::current(),
            frame: None,
            quick: false,
        };
        // A per-pixel picture is baked quick while a control glides, whole once it rests
        // (the vector picture is the same either way: no second build for it).
        want.quick = moving && want.backend == super::backend::SceneBackend::PerPixel;
        self.land(ctx);
        // A new size is asked for once it has held a frame (a window's passing frame asks
        // for none), and that next frame is asked for at once.
        let size = (rect.size(), ppp.to_bits());
        let mut ask = want.clone();
        if let Some((_, have, _)) = &self.shown {
            if (have.size, have.ppp.to_bits()) != size && self.size_seen != Some(size) {
                ask.size = have.size;
                ask.ppp = have.ppp;
                ctx.request_repaint();
            }
        }
        self.size_seen = Some(size);
        let showing = self.shown.as_ref().map(|s| &s.1);
        if showing != Some(&ask) {
            if BLOCKING.load(Ordering::Relaxed) {
                let t0 = std::time::Instant::now();
                let b = build(&ask);
                let img = to_image(&b, ask.ppp, ask.quick);
                self.gen += 1;
                self.builds += 1;
                self.last_ms = t0.elapsed().as_secs_f64() * 1e3;
                self.upload(ctx, img);
                self.shown = Some((self.gen, ask, Arc::new(b)));
            } else if self.asked.is_none() {
                if self.worker.is_none() {
                    self.worker = Worker::start(ctx.clone());
                }
                if let Some(wk) = &self.worker {
                    self.gen += 1;
                    if wk.send(Job {
                        gen: self.gen,
                        want: ask.clone(),
                    }) {
                        self.asked = Some((self.gen, ask));
                        self.builds += 1;
                    }
                }
            }
        }
        self.paint_quad(painter, rect);
        self.paint_overlay(painter, rect, view, ppp);
        moving
    }

    /// Ease the live controls toward `target`'s; returns whether they still move.
    fn glide_to(&mut self, target: &mut Subject, now: f64) -> bool {
        let Some(want) = controls(target) else {
            self.glide = None;
            return false;
        };
        let dt = self.last.map_or(1.0, |t| (now - t).clamp(0.0, 0.25));
        self.last = Some(now);
        let shown = match self.glide {
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
        self.glide = Some(shown);
        with_controls(target, shown);
        shown != want
    }

    /// Take what the worker built, if it is the build asked for.
    fn land(&mut self, ctx: &egui::Context) {
        let mut got = None;
        if let Some(wk) = &self.worker {
            while let Ok(r) = wk.rx.try_recv() {
                got = Some(r);
            }
        }
        if let Some((gen, b, img, ms)) = got {
            if let Some((asked_gen, asked)) = self.asked.take() {
                if gen == asked_gen {
                    self.last_ms = ms;
                    self.upload(ctx, img);
                    self.shown = Some((gen, asked, b));
                } else {
                    self.asked = Some((asked_gen, asked));
                }
            }
        }
    }

    fn upload(&mut self, ctx: &egui::Context, img: egui::ColorImage) {
        match &mut self.tex {
            Some(t) => t.set(img, egui::TextureOptions::LINEAR),
            None => self.tex = Some(ctx.load_texture("scene", img, egui::TextureOptions::LINEAR)),
        }
        self.uploads += 1;
    }

    fn paint_quad(&mut self, painter: &egui::Painter, rect: Rect) {
        let Some(tex) = &self.tex else {
            return;
        };
        let id = tex.id();
        if self.quad.as_ref().map(|q| (q.1, q.2)) != Some((rect, id)) {
            let corner = |p: Pos2, u: f32, v: f32| Vertex {
                pos: p,
                uv: Pos2::new(u, v),
                color: Color32::WHITE,
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
            painter.add(Shape::Mesh(m.clone()));
        }
    }

    /// The part a pick stands for in the build shown.
    fn part_of(&self, p: Picked) -> Option<Part> {
        let (_, w, b) = self.shown.as_ref()?;
        match p {
            Picked::Piece { piece, .. } => Some(Part::Piece(piece)),
            Picked::Lid => Some(Part::Lid),
            Picked::Mic(o) => b
                .mics
                .iter()
                .map(|m| m.0)
                .find(|&part| super::mic_output(w, part) == Some(o)),
        }
    }

    fn paint_overlay(&mut self, painter: &egui::Painter, rect: Rect, view: &SceneView, ppp: f32) {
        let Some(gen) = self.shown.as_ref().map(|s| s.0) else {
            return;
        };
        if view.selected.is_none() && view.hovered.is_none() {
            return;
        }
        let key = OverlayKey {
            gen,
            rect,
            selected: view.selected,
            hovered: view.hovered,
            accent: view.accent,
            ink: view.ink,
            ppp,
        };
        if self.overlay.as_ref().map(|o| &o.0) != Some(&key) {
            let mut shapes: Vec<Shape> = Vec::new();
            let off = rect.min.to_vec2();
            let b = self.shown.as_ref().map(|s| s.2.clone());
            if let Some(b) = b {
                for (pick, sel) in [(view.hovered, false), (view.selected, true)] {
                    if !sel && view.hovered == view.selected {
                        continue;
                    }
                    let Some(part) = pick.and_then(|p| self.part_of(p)) else {
                        continue;
                    };
                    let col = if sel { view.accent } else { view.ink };
                    let halo = halo_for(col).gamma_multiply(0.85);
                    if let Some(&(_, s)) = b.mics.iter().find(|m| m.0 == part) {
                        let c = s + off;
                        if sel {
                            shapes.push(Shape::circle_filled(c, 13.0, col.gamma_multiply(0.18)));
                        }
                        shapes.push(Shape::circle_stroke(
                            c,
                            12.0,
                            (if sel { 1.6 } else { 1.1 }, col),
                        ));
                        continue;
                    }
                    let Some((_, _, o)) = b.outlines.iter().find(|o| o.0 == part) else {
                        continue;
                    };
                    let pts: Vec<Pos2> = o.iter().map(|p| *p + off).collect();
                    if sel {
                        shapes.push(Shape::Path(PathShape::closed_line(
                            pts.clone(),
                            PathStroke::new(6.0, col.gamma_multiply(0.28)),
                        )));
                        shapes.push(Shape::Path(PathShape::closed_line(
                            pts,
                            PathStroke::new(1.6, col),
                        )));
                    } else {
                        shapes.push(Shape::Path(PathShape::closed_line(
                            pts.clone(),
                            PathStroke::new(2.8, halo.gamma_multiply(0.7)),
                        )));
                        shapes.push(Shape::Path(PathShape::closed_line(
                            pts,
                            PathStroke::new(1.1, col.gamma_multiply(0.8)),
                        )));
                    }
                }
            }
            let mut t = Tessellator::new(
                ppp,
                egui::epaint::TessellationOptions::default(),
                [1, 1],
                Vec::new(),
            );
            let mut mesh = Mesh::default();
            for s in shapes {
                t.tessellate_shape(s, &mut mesh);
            }
            self.overlay = Some((key, Arc::new(mesh)));
        }
        if let Some((_, m)) = &self.overlay {
            painter.add(Shape::Mesh(m.clone()));
        }
    }

    /// What the scene shown has at `point` (screen points), from its own build.
    pub fn pick(&self, rect: Rect, point: Pos2) -> Option<Picked> {
        let (_, w, b) = self.shown.as_ref()?;
        pick_in(w, b, (point - rect.min).to_pos2())
    }

    /// Where a tag of `p` stands in the picture shown (screen points; `rect` as painted),
    /// from the build shown: nothing is built.
    pub fn anchor(&self, rect: Rect, p: Picked) -> Option<Pos2> {
        let (_, w, b) = self.shown.as_ref()?;
        super::anchor_in(w, b, p).map(|q| q + rect.min.to_vec2())
    }

    /// What the picture shown shows: its subject (the controls as drawn) and its room.
    pub fn showing(&self) -> Option<(&Subject, super::Room)> {
        self.shown.as_ref().map(|s| (&s.1.subject, s.1.room))
    }

    /// Where the picture was last painted.
    pub fn rect(&self) -> Option<Rect> {
        self.rect
    }

    /// A point on the screen where the scene shown has `want` (for tests and the help:
    /// the first point of a grid over its outline that picks it).
    pub fn point_on(&self, want: Picked) -> Option<Pos2> {
        let rect = self.rect?;
        let part = self.part_of(want)?;
        let b = self.built()?;
        let pts: Vec<Pos2> = match b.mics.iter().find(|m| m.0 == part) {
            Some(&(_, c)) => vec![c],
            None => b.outlines.iter().find(|o| o.0 == part)?.2.clone(),
        };
        let (x0, y0, x1, y1) = super::geo::bbox(&pts);
        for j in 1..12 {
            for i in 1..12 {
                let p = Pos2::new(
                    x0 + (x1 - x0) * i as f32 / 12.0,
                    y0 + (y1 - y0) * j as f32 / 12.0,
                );
                let at = p + rect.min.to_vec2();
                let same = match (self.pick(rect, at), want) {
                    (Some(Picked::Piece { piece: a, .. }), Picked::Piece { piece: b, .. }) => {
                        a == b
                    }
                    (got, w) => got == Some(w),
                };
                if same {
                    return Some(at);
                }
            }
        }
        None
    }

    /// Whether the picture shown is the one last asked for, with no build on its way.
    pub fn ready(&self) -> bool {
        self.shown.is_some() && self.asked.is_none()
    }

    /// Whether a build is on its way (the picker's thumbnail waits for it to land).
    pub fn building(&self) -> bool {
        self.asked.is_some()
    }

    /// The build shown (its outlines and the mics' places, for a caller's labels).
    pub fn built(&self) -> Option<&Built> {
        self.shown.as_ref().map(|s| &*s.2)
    }

    /// The lid's new value (0 … 1) after the pointer moved `dy` points down while dragging
    /// it from `lid`: the motion projected onto the hinge's rotation, snapping to the short
    /// and the full stick within 2°.
    pub fn drag_lid(&self, lid: f32, dy: f32, upright: bool) -> f32 {
        let full = if upright {
            super::piano::UPRIGHT_LID
        } else {
            super::piano::GRAND_LID
        };
        let len = self
            .built()
            .and_then(|b| b.outlines.iter().find(|o| o.0 == Part::Lid))
            .map(|o| {
                let (x0, y0, x1, y1) = super::geo::bbox(&o.2);
                (x1 - x0).max(y1 - y0)
            })
            .unwrap_or(200.0)
            .max(20.0);
        let ang = (lid.clamp(0.0, 1.0) * full).to_radians();
        let next = (ang - dy / (len * ang.cos().max(0.2)))
            .to_degrees()
            .clamp(0.0, full);
        let snapped = [super::piano::SHORT_STICK, full]
            .into_iter()
            .find(|s| (next - s).abs() < 2.0)
            .unwrap_or(next);
        snapped / full
    }
}
