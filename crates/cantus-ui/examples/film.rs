//! The launch video's film (round 41, `docs/round41/video/README.md` §1, `script.md`'s cut):
//! a director's script in code. Each scene opens its demo song in the real application —
//! `CantusApp::assemble`, the boot's path, over a data folder of its own, then *File ▸ Demo
//! songs* (`views::demos::request`) — plays it on the real engine stepped by hand
//! (`EngineHandle::manual`, `48 000 / fps` samples a frame), does what a person does with a
//! drawn cursor whose clicks are real pointer events, and writes every frame as a PNG and
//! the engine's own output as a WAV, so picture and sound share one clock.
//!
//! ```text
//! cargo run --release -p cantus-ui --example film -- --out <dir> [--scene a,b] [--fps 30]
//!     [--width 1920 --height 1080] [--scale 1] [--every N] [--limit N] [--no-camera]
//! ```
//!
//! - `--width/--height` are points, `--scale` pixels per point: the draft is 1920 × 1080
//!   points at 1.0; the final 3840 × 2160 is the same program at 2.0.
//! - `--every N` writes only every Nth PNG (to look at a scene quickly; the sound and the
//!   clock are the same), `--limit N` stops a scene after N frames.
//! - `--no-camera` writes the frames unframed; the camera's targets are in the scene's JSON
//!   either way.
//!
//! Out: `<dir>/<scene>/00000.png…`, `<dir>/<scene>.wav` (48 kHz, stereo, 32-bit float) and
//! `<dir>/<scene>.json` (the frames, the song, where it starts, the fps, the camera).
//!
//! The look is Slate Lapis Night in every scene. Onda (`builtin:onda`) is the only dancer,
//! held on *Bounce*, and shows only where the cut says; no other dancer is in any song used.
//!
//! Generated files never go in the repository: `tools/video/film/draft.sh` renders into
//! `F:\projects\video-work\film\`.

use cantus_core::inst::dancer::{self, Mood, Move};
use cantus_core::{InstrumentKind, StripId, Ticks, TrackId};
use cantus_engine::{EngineHandle, Processor};
use cantus_ui::app::{CantusApp, Files};
use cantus_ui::kit::appearance::Mode;
use cantus_ui::picker::PickChoice;
use cantus_ui::raster::{Canvas, Textures};
use egui::epaint::ClippedShape;
use egui::{Color32, Event, Modifiers, PointerButton, Pos2, Rect, Vec2};
use std::path::{Path, PathBuf};

#[path = "shots_common/studio.rs"]
mod studio;

/// The engine's rate.
const RATE: f64 = 48_000.0;
/// The look of every shot: Slate Lapis Night (the dark theme, the owner's choice).
const LOOK: &str = "slate_lapis_night";

// ---- options -----------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Opts {
    fps: u32,
    size: Vec2,
    scale: f32,
    out: PathBuf,
    scenes: Vec<String>,
    every: usize,
    limit: Option<usize>,
    camera: bool,
}

impl Opts {
    fn parse() -> Opts {
        let mut o = Opts {
            fps: 30,
            size: Vec2::new(1920.0, 1080.0),
            scale: 1.0,
            out: PathBuf::from(r"F:\projects\video-work\film"),
            scenes: Vec::new(),
            every: 1,
            limit: None,
            camera: true,
        };
        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut i = 0;
        while i < args.len() {
            let a = args[i].as_str();
            let v = args.get(i + 1).cloned().unwrap_or_default();
            let num =
                |s: &str| -> f32 { s.parse().unwrap_or_else(|_| panic!("{a} wants a number")) };
            match a {
                "--fps" => o.fps = num(&v) as u32,
                "--width" => o.size.x = num(&v),
                "--height" => o.size.y = num(&v),
                "--scale" => o.scale = num(&v),
                "--out" => o.out = PathBuf::from(&v),
                "--scene" => o.scenes.extend(v.split(',').map(str::to_string)),
                "--every" => o.every = (num(&v) as usize).max(1),
                "--limit" => o.limit = Some(num(&v) as usize),
                "--no-camera" => {
                    o.camera = false;
                    i += 1;
                    continue;
                }
                other => panic!("unknown option {other}"),
            }
            i += 2;
        }
        o
    }

    /// Samples a frame.
    fn spf(&self) -> usize {
        (RATE / self.fps as f64).round() as usize
    }
}

// ---- easing -----------------------------------------------------------------------------

/// Cubic in and out.
fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

fn lerp_rect(a: Rect, b: Rect, t: f32) -> Rect {
    Rect::from_min_max(a.min.lerp(b.min, t), a.max.lerp(b.max, t))
}

// ---- what a frame painted -----------------------------------------------------------------

/// One painted text: its words and the part of it that can be seen.
#[derive(Clone, Debug)]
struct Painted {
    text: String,
    seen: Rect,
}

fn walk(shape: &egui::Shape, clip: Rect, out: &mut Vec<Painted>) {
    match shape {
        egui::Shape::Text(t) => {
            let rect = t.galley.rect.translate(t.pos.to_vec2());
            out.push(Painted {
                text: t.galley.text().to_string(),
                seen: rect.intersect(clip),
            });
        }
        egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, out)),
        _ => {}
    }
}

// ---- the director --------------------------------------------------------------------------

/// A camera key: from frame `at`, the view eases to `rect` (points) over `frames`.
#[derive(Copy, Clone, Debug)]
struct Key {
    at: usize,
    frames: usize,
    rect: Rect,
}

/// What the interface's input is fed with, a block at a time, as a device would.
struct Feed {
    ring: rtrb::Producer<f32>,
    data: Vec<f32>,
    pos: usize,
}

/// One scene's app, engine, clock, cursor and camera.
struct Director {
    feed: Option<Feed>,
    app: CantusApp,
    ctx: egui::Context,
    proc: Processor,
    tex: Textures,
    o: Opts,
    name: String,
    dir: PathBuf,
    /// egui's clock, seconds.
    t: f64,
    /// Frames written (the scene's clock).
    frame: usize,
    /// Whether frames are being written and the sound kept.
    rolling: bool,
    /// Whether the engine runs during the frames that are not written (a pre-roll).
    engine_idle: bool,
    pointer: Pos2,
    cursor: bool,
    ripples: Vec<(usize, Pos2)>,
    audio: Vec<f32>,
    buf: Vec<f32>,
    camera: Vec<Key>,
    texts: Vec<Painted>,
    mods: Modifiers,
    meta: serde_json::Map<String, serde_json::Value>,
    /// The song's position when the first frame is written.
    start_tick: Option<Ticks>,
    /// The scene's length in frames (frames past it are not written).
    total: usize,
    /// Where the scene sits in the cut (seconds).
    cut: (f32, f32),
    /// Whether this scene was asked for (else its frames run unpainted: a later scene of the
    /// same take still starts where it should).
    wanted: bool,
    _data: PathBuf,
}

/// The block the engine runs in: the largest divisor of a frame's samples up to 512.
fn block_of(spf: usize) -> usize {
    (1..=512.min(spf))
        .rev()
        .find(|b| spf.is_multiple_of(*b))
        .unwrap_or(1)
}

impl Director {
    /// The application as the boot assembles it, over a fresh data folder of the scene's
    /// own, with `project` open (the start page off, the audio set up — a configured
    /// install), the real engine run by hand, in Slate Lapis Night, the demo interface on.
    fn new(o: &Opts, name: &str, cut: (f32, f32)) -> Director {
        let data = o.out.join("tmp").join(format!("data-{name}"));
        let _ = std::fs::remove_dir_all(&data);
        std::fs::create_dir_all(&data).expect("the scene's data folder");
        std::fs::write(
            data.join("settings.json"),
            r#"{"audio_setup":{"setup_done":true},"start_page_at_start":false,"plugin_scan_at_start":false}"#,
        )
        .expect("settings");
        // Everything the app keeps (caches, autosaves) goes in the scene's folder.
        std::env::set_var("CANTUS_DATA_DIR", &data);
        cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
        let files = Files::read(&data, Some(cantus_ui::i18n::Locale::En));
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let (engine, proc) = EngineHandle::manual(RATE);
        let mut app = CantusApp::assemble(
            &ctx,
            files,
            Some(cantus_ui::starter_project()),
            engine,
            None,
            None,
            None,
            false,
        );
        app.appearance.mode = Mode::Dark;
        app.appearance.assign(Mode::Dark, format!("preset:{LOOK}"));
        app.apply_appearance();
        let spf = o.spf();
        let block = block_of(spf);
        // The demo interface (no "no audio device" line), at the block the engine runs.
        studio::studio(&mut app, block as u32);
        let mut d = Director {
            feed: None,
            app,
            ctx,
            proc,
            tex: Textures::default(),
            o: o.clone(),
            name: String::new(),
            dir: PathBuf::new(),
            t: 0.0,
            frame: 0,
            rolling: false,
            engine_idle: true,
            // Off the window until a scene shows the cursor: nothing hovered, no hint.
            pointer: Pos2::new(-200.0, -200.0),
            cursor: false,
            ripples: Vec::new(),
            audio: Vec::new(),
            buf: vec![0.0; block * 2],
            camera: Vec::new(),
            texts: Vec::new(),
            mods: Modifiers::NONE,
            meta: serde_json::Map::new(),
            start_tick: None,
            total: 0,
            cut,
            wanted: false,
            _data: data,
        };
        d.begin(name, cut);
        d.idle(6);
        d
    }

    /// Start scene `name` (its place in the cut `cut`): its frames from 0, its sound, its
    /// camera at the whole screen.
    fn begin(&mut self, name: &str, cut: (f32, f32)) {
        self.name = name.to_string();
        self.cut = cut;
        self.total = ((cut.1 - cut.0) * self.o.fps as f32).round() as usize;
        if let Some(l) = self.o.limit {
            self.total = self.total.min(l);
        }
        self.wanted = self.o.scenes.is_empty() || self.o.scenes.iter().any(|s| s == name);
        self.dir = self.o.out.join(name);
        if self.wanted {
            let _ = std::fs::remove_dir_all(&self.dir);
            std::fs::create_dir_all(&self.dir).expect("the scene's frames folder");
        }
        self.frame = 0;
        self.audio.clear();
        self.camera.clear();
        self.ripples.clear();
        self.start_tick = None;
        let song = self.meta.get("song").cloned();
        self.meta = serde_json::Map::new();
        if let Some(s) = song {
            self.meta.insert("song".into(), s);
        }
    }

    /// End this scene and go straight on with `name`: the same take, the song playing on.
    fn next(&mut self, name: &str, cut: (f32, f32)) {
        self.close();
        self.begin(name, cut);
    }

    /// The screen in points.
    fn screen(&self) -> Rect {
        Rect::from_min_size(Pos2::ZERO, self.o.size)
    }

    /// Open demo `id` as *File ▸ Demo songs* opens it.
    fn open_demo(&mut self, id: &str) {
        let i = cantus_ui::demos::ALL
            .iter()
            .position(|d| d.id == id)
            .unwrap_or_else(|| panic!("no demo {id}"));
        cantus_ui::views::demos::request(&mut self.app, i);
        self.app.settings.stage.hidden = true;
        self.meta.insert(
            "song".into(),
            serde_json::Value::String(self.app.project().name.clone()),
        );
        self.idle(4);
        // The demo interface again: a project open resets nothing of it, but say so.
        self.app.set_status(String::new());
    }

    /// One frame with `events`. While rolling the engine renders the frame's samples first
    /// (kept), and the frame is written.
    fn step(&mut self, events: Vec<Event>) {
        if self.rolling && self.frame >= self.total {
            return;
        }
        let spf = self.o.spf();
        let block = self.buf.len() / 2;
        if self.rolling || self.engine_idle {
            for _ in 0..spf / block {
                if let Some(f) = self.feed.as_mut() {
                    for _ in 0..block {
                        let v = f.data.get(f.pos).copied().unwrap_or(0.0);
                        f.pos += 1;
                        let _ = f.ring.push(v);
                    }
                }
                self.proc.process(&mut self.buf, 2);
                if self.rolling {
                    self.audio.extend_from_slice(&self.buf);
                }
            }
        }
        if self.rolling && self.start_tick.is_none() {
            self.start_tick = Some(self.app.engine.position());
        }
        let mut ev = vec![
            Event::ModifiersChanged(self.mods),
            Event::PointerMoved(self.pointer),
        ];
        ev.extend(events);
        let mut input = egui::RawInput {
            screen_rect: Some(self.screen()),
            time: Some(self.t),
            predicted_dt: 1.0 / self.o.fps as f32,
            events: ev,
            ..Default::default()
        };
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(self.o.scale);
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| app.frame(ui));
        self.tex.update(&out.textures_delta);
        out.textures_delta.clear();
        self.texts.clear();
        for c in &out.shapes {
            walk(&c.shape, c.clip_rect, &mut self.texts);
        }
        self.t += 1.0 / self.o.fps as f64;
        if self.rolling {
            if self.wanted && self.frame.is_multiple_of(self.o.every) {
                self.write(out.shapes);
            }
            self.frame += 1;
        }
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(Vec::new());
        }
    }

    /// Frames until the scene's clock reads `seconds`.
    fn until(&mut self, seconds: f32) {
        let to = (seconds * self.o.fps as f32).round() as usize;
        while self.rolling && self.frame < to.min(self.total) {
            self.step(Vec::new());
        }
    }

    /// Start writing frames (the scene's 0).
    fn roll(&mut self) {
        self.rolling = true;
        // Clicks made while setting the scene up leave no ripple in it.
        self.ripples.clear();
    }

    /// Frames for `ms` milliseconds.
    fn frames_for(&self, ms: f32) -> usize {
        ((ms / 1000.0) * self.o.fps as f32).round().max(1.0) as usize
    }

    // ---- the cursor: drawn by the film, its clicks real events ----------------------------

    /// The cursor glides to `p` (cubic in and out) over `ms`, the pointer with it.
    fn glide(&mut self, p: Pos2, ms: f32) {
        self.cursor = true;
        let from = self.pointer;
        let n = self.frames_for(ms);
        for i in 1..=n {
            self.pointer = from.lerp(p, ease(i as f32 / n as f32));
            self.step(Vec::new());
        }
        self.pointer = p;
    }

    /// A move whose length sets its time: 250 ms for a short hop, 400 ms across the screen.
    fn go(&mut self, p: Pos2) {
        let d = (p - self.pointer).length();
        let ms = (250.0 + d / self.o.size.x * 300.0).min(400.0);
        self.glide(p, ms);
    }

    fn button(&mut self, pressed: bool, button: PointerButton) {
        let p = self.pointer;
        self.step(vec![Event::PointerButton {
            pos: p,
            button,
            pressed,
            modifiers: self.mods,
        }]);
    }

    /// A click where the cursor is, with its ripple.
    fn click(&mut self) {
        self.ripples.push((self.frame, self.pointer));
        self.button(true, PointerButton::Primary);
        self.button(false, PointerButton::Primary);
    }

    fn double_click(&mut self) {
        self.ripples.push((self.frame, self.pointer));
        self.button(true, PointerButton::Primary);
        self.button(false, PointerButton::Primary);
        self.step(Vec::new());
        self.ripples.push((self.frame, self.pointer));
        self.button(true, PointerButton::Primary);
        self.button(false, PointerButton::Primary);
    }

    /// Real wheel events over `ms`: `points` scrolled (positive takes a list down), eased, a
    /// part every frame (a flick on a mouse wheel or a trackpad).
    fn fling(&mut self, points: f32, ms: f32) {
        let n = self.frames_for(ms);
        let mut done = 0.0;
        for i in 1..=n {
            let to = points * ease(i as f32 / n as f32);
            let d = to - done;
            done = to;
            self.step(vec![Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -d),
                phase: egui::TouchPhase::Move,
                modifiers: self.mods,
            }]);
        }
    }

    /// In the open device picker: the cursor into its grid, the list wheeled down by `step`
    /// points at a time (at most `max`) until the tile labelled `name` is wholly in view, the
    /// tile hovered for `hold_ms`, the list wheeled back to where it was. Said in the JSON.
    fn show_tile(&mut self, name: &str, step: f32, max: f32, hold_ms: f32) {
        let panel = cantus_ui::views::picker::panel_rect(self.screen());
        let grid = Rect::from_min_max(
            Pos2::new(panel.left() + 210.0, panel.top() + 65.0),
            Pos2::new(panel.right() - 330.0, panel.bottom() - 60.0),
        );
        // The label sits under its tile's picture: in view when the picture is too.
        let seen = |d: &Director| {
            d.texts_all(name)
                .into_iter()
                .filter(|r| {
                    grid.contains(r.center())
                        && r.top() > grid.top() + 175.0
                        && r.bottom() < grid.bottom()
                })
                .min_by(|a, b| a.left().total_cmp(&b.left()))
        };
        self.glide(grid.center(), 250.0);
        let mut scrolled = 0.0;
        while seen(self).is_none() && scrolled < max {
            self.fling(step, 260.0);
            scrolled += step;
            self.idle(self.frames_for(80.0));
        }
        match seen(self) {
            Some(label) => {
                let over = Pos2::new(label.center().x, label.top() - 70.0);
                self.go(over);
                self.idle(self.frames_for(hold_ms));
                self.note(
                    &format!("shown_{name}"),
                    serde_json::json!([over.x, over.y, scrolled]),
                );
            }
            None => self.did(&format!(
                "{name}'s tile not brought into view within {max} points"
            )),
        }
        if scrolled > 0.0 {
            self.fling(-scrolled - step, 350.0);
            self.idle(self.frames_for(100.0));
        }
    }

    /// Press where the cursor is, move to `p` over `ms` (eased), let go.
    fn drag(&mut self, p: Pos2, ms: f32) {
        self.ripples.push((self.frame, self.pointer));
        self.button(true, PointerButton::Primary);
        self.glide(p, ms);
        self.button(false, PointerButton::Primary);
    }

    /// The arrow and the ripples, over everything, in points.
    fn cursor_shapes(&self) -> Vec<ClippedShape> {
        let mut shapes = Vec::new();
        let clip = self.screen();
        let fps = self.o.fps as f32;
        for &(at, p) in &self.ripples {
            let age = (self.frame as f32 - at as f32) / fps;
            if !(0.0..0.45).contains(&age) {
                continue;
            }
            let k = age / 0.45;
            let r = 6.0 + 18.0 * ease(k);
            let a = ((1.0 - k) * 150.0) as u8;
            shapes.push(ClippedShape {
                clip_rect: clip,
                shape: egui::Shape::circle_stroke(
                    p,
                    r,
                    egui::Stroke::new(2.0, Color32::from_rgba_unmultiplied(255, 255, 255, a)),
                ),
            });
        }
        if !self.cursor {
            return shapes;
        }
        // A plain arrow, 21 points tall: white, a fine dark edge, a soft shadow.
        const ARROW: [(f32, f32); 7] = [
            (0.0, 0.0),
            (0.0, 17.0),
            (4.0, 13.2),
            (6.9, 19.6),
            (9.6, 18.4),
            (6.8, 12.2),
            (12.2, 12.2),
        ];
        const TRIS: [[usize; 3]; 5] = [[0, 1, 2], [0, 2, 5], [0, 5, 6], [2, 3, 4], [2, 4, 5]];
        let at = |dx: f32, dy: f32| -> Vec<Pos2> {
            ARROW
                .iter()
                .map(|&(x, y)| self.pointer + Vec2::new(x + dx, y + dy))
                .collect()
        };
        let mesh = |pts: &[Pos2], c: Color32| {
            let mut m = egui::Mesh::default();
            for p in pts {
                m.colored_vertex(*p, c);
            }
            for t in TRIS {
                m.add_triangle(t[0] as u32, t[1] as u32, t[2] as u32);
            }
            egui::Shape::mesh(m)
        };
        let shadow = at(0.8, 1.4);
        shapes.push(ClippedShape {
            clip_rect: clip,
            shape: mesh(&shadow, Color32::from_black_alpha(70)),
        });
        let arrow = at(0.0, 0.0);
        shapes.push(ClippedShape {
            clip_rect: clip,
            shape: mesh(&arrow, Color32::from_rgb(250, 250, 248)),
        });
        shapes.push(ClippedShape {
            clip_rect: clip,
            shape: egui::Shape::closed_line(
                arrow,
                egui::Stroke::new(1.1, Color32::from_rgb(20, 22, 28)),
            ),
        });
        shapes
    }

    // ---- the camera: eased framings of the frame, written in the JSON ----------------------

    /// From now, ease to `rect` (points) over `ms`.
    fn frame_to(&mut self, rect: Rect, ms: f32) {
        let frames = self.frames_for(ms);
        self.camera.push(Key {
            at: self.frame,
            frames,
            rect,
        });
    }

    /// The screen's aspect at `zoom` round `centre`, kept inside the screen.
    fn framing(&self, centre: Pos2, zoom: f32) -> Rect {
        let s = self.o.size / zoom.max(1.0);
        let min = (centre - s / 2.0)
            .max(Pos2::ZERO)
            .min((self.o.size - s).to_pos2());
        Rect::from_min_size(min, s)
    }

    /// What the camera shows at frame `f` (points).
    fn view(&self, f: usize) -> Rect {
        let mut cur = self.screen();
        for k in &self.camera {
            if f < k.at {
                break;
            }
            let t = (f - k.at) as f32 / k.frames.max(1) as f32;
            if k.frames == 0 || t >= 1.0 {
                cur = k.rect;
            } else {
                return lerp_rect(cur, k.rect, ease(t));
            }
        }
        cur
    }

    fn write(&mut self, shapes: Vec<ClippedShape>) {
        let ppp = self.ctx.pixels_per_point();
        let mut shapes = shapes;
        shapes.extend(self.cursor_shapes());
        let prims = self.ctx.tessellate(shapes, ppp);
        let (w, h) = (
            (self.o.size.x * ppp).round() as usize,
            (self.o.size.y * ppp).round() as usize,
        );
        let bg = self.ctx.global_style().visuals.panel_fill;
        let mut c = Canvas::new(w, h, bg);
        c.paint(&prims, &self.tex, ppp);
        let path = self.dir.join(format!("{:05}.png", self.frame));
        let view = self.view(self.frame);
        if !self.o.camera || view == self.screen() {
            c.write_png(&path).expect("a frame is written");
            return;
        }
        // The framing, resampled (bilinear) to the whole frame.
        let src: Vec<[u8; 4]> = (0..w * h).map(|i| c.pixel(i % w, i / w)).collect();
        let mut pix = tiny_skia::Pixmap::new(w as u32, h as u32).expect("a frame");
        let (vx, vy) = (view.left() * ppp, view.top() * ppp);
        let (sx, sy) = (
            view.width() * ppp / w as f32,
            view.height() * ppp / h as f32,
        );
        let px = pix.pixels_mut();
        for y in 0..h {
            let fy = (vy + (y as f32 + 0.5) * sy - 0.5).clamp(0.0, (h - 1) as f32);
            let (y0, ty) = (fy.floor() as usize, fy.fract());
            let y1 = (y0 + 1).min(h - 1);
            for x in 0..w {
                let fx = (vx + (x as f32 + 0.5) * sx - 0.5).clamp(0.0, (w - 1) as f32);
                let (x0, tx) = (fx.floor() as usize, fx.fract());
                let x1 = (x0 + 1).min(w - 1);
                let (a, b, cc, d) = (
                    src[y0 * w + x0],
                    src[y0 * w + x1],
                    src[y1 * w + x0],
                    src[y1 * w + x1],
                );
                let ch = |k: usize| -> u8 {
                    let top = a[k] as f32 * (1.0 - tx) + b[k] as f32 * tx;
                    let bot = cc[k] as f32 * (1.0 - tx) + d[k] as f32 * tx;
                    (top * (1.0 - ty) + bot * ty).round() as u8
                };
                px[y * w + x] =
                    tiny_skia::ColorU8::from_rgba(ch(0), ch(1), ch(2), 255).premultiply();
            }
        }
        std::fs::write(&path, pix.encode_png().expect("png")).expect("a frame is written");
    }

    // ---- the scene's sound and notes ---------------------------------------------------

    fn note(&mut self, key: &str, v: serde_json::Value) {
        self.meta.insert(key.into(), v);
    }

    /// The last scene of a take ends.
    fn finish(mut self) {
        self.close();
    }

    /// The WAV and the JSON beside the frames.
    fn close(&mut self) {
        // Frames to the scene's length, whatever the script left.
        self.rolling = true;
        while self.frame < self.total {
            self.step(Vec::new());
        }
        if !self.wanted {
            return;
        }
        let cut = self.cut;
        let wav = self.o.out.join(format!("{}.wav", self.name));
        write_wav(&wav, &self.audio, RATE as u32);
        let ppp = self.o.scale;
        let px = |r: Rect| {
            serde_json::json!([
                (r.left() * ppp).round(),
                (r.top() * ppp).round(),
                (r.width() * ppp).round(),
                (r.height() * ppp).round()
            ])
        };
        let start = self.start_tick.unwrap_or(Ticks::ZERO);
        let start_s = self.app.project().tempo_map().seconds_at(start.0 as f64);
        let keys: Vec<serde_json::Value> = self
            .camera
            .iter()
            .map(|k| serde_json::json!({"from": k.at, "frames": k.frames, "rect_px": px(k.rect)}))
            .collect();
        let m = &mut self.meta;
        m.insert("scene".into(), self.name.clone().into());
        m.insert("fps".into(), self.o.fps.into());
        m.insert("frames".into(), self.total.into());
        m.insert("every".into(), self.o.every.into());
        m.insert("cut".into(), serde_json::json!([cut.0, cut.1]));
        m.insert(
            "points".into(),
            serde_json::json!([self.o.size.x, self.o.size.y]),
        );
        m.insert("scale".into(), ppp.into());
        m.insert("start_tick".into(), start.0.into());
        m.insert("start_seconds".into(), start_s.into());
        m.insert("sample_rate".into(), (RATE as u32).into());
        m.insert("samples".into(), (self.audio.len() / 2).into());
        m.insert("camera_applied".into(), self.o.camera.into());
        m.insert("camera".into(), serde_json::Value::Array(keys));
        let json = self.o.out.join(format!("{}.json", self.name));
        let text = serde_json::to_string_pretty(&self.meta).expect("json");
        std::fs::write(&json, text).expect("the scene's notes");
        println!(
            "{}: {} frames, {:.2} s of sound -> {}",
            self.name,
            self.total,
            self.audio.len() as f64 / 2.0 / RATE,
            self.dir.display()
        );
    }

    // ---- finding things the way a person does: by their words, by their hints ------------

    /// Where the words `s` are painted and seen (the topmost).
    fn text(&self, s: &str) -> Option<Rect> {
        self.texts
            .iter()
            .rev()
            .find(|p| p.text.trim() == s && p.seen.is_positive())
            .map(|p| p.seen)
    }

    /// [`Self::text`] inside `region`.
    fn text_in(&self, s: &str, region: Rect) -> Option<Rect> {
        self.texts
            .iter()
            .rev()
            .find(|p| p.text.trim() == s && p.seen.is_positive() && region.contains_rect(p.seen))
            .map(|p| p.seen)
    }

    /// The interactive widgets drawn last frame whose centre is inside `region`, smallest
    /// first.
    fn widgets_in(&self, region: Rect) -> Vec<Rect> {
        let mut v: Vec<Rect> = self.ctx.viewport(|vp| {
            vp.prev_pass
                .widgets
                .layers()
                .flat_map(|(_, ws)| ws.iter())
                .filter(|w| w.sense.interactive() && region.contains(w.interact_rect.center()))
                .map(|w| w.interact_rect)
                .collect()
        });
        v.sort_by(|a, b| a.area().total_cmp(&b.area()));
        v.dedup();
        v
    }

    /// The help line's name for the control under `p`: a frame that is not written, the
    /// clock and the engine held (only before the scene rolls).
    fn hint_at(&mut self, p: Pos2) -> Option<String> {
        assert!(!self.rolling, "probing while rolling");
        let (was, idle) = (self.pointer, self.engine_idle);
        self.engine_idle = false;
        self.pointer = p;
        self.step(Vec::new());
        let name = cantus_ui::kit::hint::current(&self.ctx).map(|h| h.name);
        self.pointer = was;
        self.engine_idle = idle;
        name
    }

    /// The control in `region` whose hint contains `want` (any case).
    fn control(&mut self, region: Rect, want: &str) -> Option<Rect> {
        let want = want.to_lowercase();
        for r in self.widgets_in(region) {
            if let Some(name) = self.hint_at(r.center()) {
                if name.to_lowercase().contains(&want) {
                    return Some(r);
                }
            }
        }
        None
    }

    /// Every control in `region` with its hint (printed when a scene cannot find one).
    fn hints_in(&mut self, region: Rect) -> Vec<(Rect, Option<String>)> {
        let mut out = Vec::new();
        for r in self.widgets_in(region) {
            let h = self.hint_at(r.center());
            out.push((r, h));
        }
        out
    }

    /// The device pictures a scene's picker shows made before the scene rolls: the picker
    /// opened as `purpose`, each of `only` asked for in order (`Thumbs::get`, the grid's own
    /// call; `None`: every instrument of the catalogue), and frames run — the picker's own
    /// frame pumps its captures and workers — until `Thumbs::idle` says all are in; then
    /// closed. The picker's atlas holds 28 pictures and keeps the first 28 it is given (no
    /// eviction: `Thumbs::upload` drops the rest), so the effects asked for are the ones
    /// the shot rests on, and the tiles a flick passes take what is left.
    fn warm_thumbs(
        &mut self,
        purpose: cantus_ui::picker::PickPurpose,
        only: Option<Vec<cantus_ui::views::picker_snap::Subject>>,
    ) {
        use cantus_ui::views::picker_snap::Subject;
        let idle = self.engine_idle;
        self.engine_idle = false;
        cantus_ui::views::picker::open(&mut self.app, purpose);
        self.idle(2);
        let subjects: Vec<Subject> = only.unwrap_or_else(|| {
            self.app
                .picker_view
                .catalog
                .iter()
                .filter_map(|d| match &d.choice {
                    PickChoice::Instrument(k) => Some(Subject::Instrument(*k)),
                    PickChoice::Pulsus => Some(Subject::Pulsus),
                    _ => None,
                })
                .collect()
        });
        for s in &subjects {
            let _ = self.app.picker_view.thumbs.get(*s);
        }
        let mut frames = 0;
        while (!self.app.picker_view.thumbs.idle() || frames < 2) && frames < 6_000 {
            self.step(Vec::new());
            frames += 1;
            if frames.is_multiple_of(20) {
                // The workers fill and shrink on threads: give them a moment.
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
        let ready = subjects
            .iter()
            .filter(|s| self.app.picker_view.thumbs.get(**s).is_some())
            .count();
        self.note(
            "thumbs_warmed",
            serde_json::json!({"asked": subjects.len(), "ready": ready, "frames": frames}),
        );
        eprintln!(
            "{}: {ready} of {} picker pictures ready after {frames} frames",
            self.name,
            subjects.len()
        );
        cantus_ui::views::picker::close(&mut self.app);
        self.idle(2);
        self.engine_idle = idle;
    }

    // ---- the song -------------------------------------------------------------------------

    /// The track whose name is `name` (a locale key's words).
    fn track(&self, key: &str) -> TrackId {
        let name = cantus_ui::i18n::t(key);
        self.app
            .project()
            .tracks
            .iter()
            .find(|t| t.name == name)
            .map(|t| t.id)
            .unwrap_or_else(|| panic!("{}: no track “{name}”", self.name))
    }

    fn strip_of(&self, track: TrackId) -> StripId {
        self.app
            .project()
            .track(track)
            .and_then(|t| t.strip)
            .expect("the track has a strip")
    }

    /// Bar `bar` (from 0) as ticks, in 4/4.
    fn bar(&self, bar: f64) -> Ticks {
        Ticks((bar * 4.0 * cantus_core::PPQ as f64).round() as i64)
    }

    /// The song from `at`, playing; the engine renders `pre` seconds before the first frame
    /// (the meters and the dancer as they are while the song runs).
    fn play_from(&mut self, at: Ticks, pre: f32) {
        let spf = self.o.spf() as f64;
        let bpm = self.app.project().tempo_bpm.max(1.0);
        let pre_ticks = pre as f64 * bpm / 60.0 * cantus_core::PPQ as f64;
        let from = Ticks((at.0 as f64 - pre_ticks).max(0.0).round() as i64);
        self.app.seek(from);
        self.app.toggle_play();
        self.engine_idle = true;
        let frames = (pre as f64 * RATE / spf).round() as usize;
        self.idle(frames);
    }

    /// Onda, the one dancer, on a dancer track of its own — added as a person adds one (the
    /// picker's *Saltator*), its character chosen, held on *Bounce* with a calm energy and a
    /// happy mood (never *Jump*, *Shy* or *Surprised*: those are its square), in the Stage
    /// (`view.mode` docked) or floating at `place` (x, y, size).
    fn add_onda(&mut self, docked: bool, place: (f32, f32, f32)) -> TrackId {
        let keep = self.app.selected_track;
        let ok = self
            .app
            .add_instrument_track(None, &PickChoice::Instrument(InstrumentKind::Dancer));
        assert!(ok, "a dancer track");
        let id = self.app.selected_track.expect("the new track is selected");
        self.app.close_instrument_window(id);
        self.app
            .set_inst_text(id, dancer::CHARACTER_KEY, Some("builtin:onda".into()));
        let mode = if docked { 1.0 } else { 0.0 };
        for (k, v) in [
            ("move", dancer::move_value(Move::Bounce)),
            ("energy", 0.45),
            ("mood", Mood::Happy.index() as f32),
            ("react", 0.0),
            ("x", place.0),
            ("y", place.1),
            ("size", place.2),
            ("view.mode", mode),
        ] {
            self.app.set_inst_value(id, k, v, false);
        }
        if let Some(k) = keep {
            self.app.select_track(k);
        }
        self.app.set_status(String::new());
        self.note("dancer", serde_json::json!({"character": "builtin:onda", "move": "Bounce", "energy": 0.45, "mood": "Happy"}));
        id
    }

    /// Every dancer the song brings (Neon Tide's Trino) taken out — its track removed and
    /// the strip it took left unnamed, as a person deletes a track — and said in the JSON:
    /// no other dancer reaches a frame.
    fn no_other_dancers(&mut self, except: Option<TrackId>) {
        let others: Vec<(TrackId, Option<StripId>)> = self
            .app
            .project()
            .tracks
            .iter()
            .filter(|t| dancer::is_dancer(&t.instrument) && Some(t.id) != except)
            .map(|t| (t.id, t.strip))
            .collect();
        for (t, strip) in &others {
            self.app.exec(cantus_core::Command::RemoveTrack { id: *t });
            if let Some(s) = strip {
                self.app.exec(cantus_core::Command::SetStripName {
                    id: *s,
                    name: String::new(),
                });
            }
        }
        self.note("other_dancers_removed", others.len().into());
    }

    /// The desk large: the arrangement folded, the strips wide (names across, inserts).
    fn big_desk(&mut self) {
        self.app.show_central(cantus_ui::app::CentralView::Mixer);
        self.app
            .workspace
            .toggle_fold(cantus_ui::views::workspace::Pane::Arrangement);
        cantus_ui::views::mixer::set_width_mode(
            &mut self.app,
            cantus_ui::views::mixer::layout::WidthMode::Wide,
        );
    }

    /// Where the words `s` are painted, every place, top to bottom of the paint order.
    fn texts_all(&self, s: &str) -> Vec<Rect> {
        self.texts
            .iter()
            .filter(|p| p.text.trim() == s && p.seen.is_positive())
            .map(|p| p.seen)
            .collect()
    }

    /// Press the button where the cursor is (with a ripple), hold `ms`, let go.
    fn press_hold(&mut self, ms: f32) {
        self.ripples.push((self.frame, self.pointer));
        self.button(true, PointerButton::Primary);
        self.idle(self.frames_for(ms));
        self.button(false, PointerButton::Primary);
    }

    /// The camera fits `r` (points) with a margin, at most `max` zoom.
    fn fit(&self, r: Rect, margin: f32, max: f32) -> Rect {
        let z = (self.o.size.x / (r.width() + 2.0 * margin))
            .min(self.o.size.y / (r.height() + 2.0 * margin))
            .clamp(1.0, max);
        self.framing(r.center(), z)
    }

    /// Where beat `beat` lies on the arrangement's timeline, and the middle of its rows.
    fn arrangement_at(&self, beat: f64) -> Option<Pos2> {
        let g = self.app.arrangement.geometry()?;
        let x = g.left + (beat as f32 - g.scroll_beats) * g.px_per_beat;
        let bottom = g
            .rows
            .last()
            .map_or(g.body_top + 200.0, |r| r.top + r.height);
        let y = (g.body_top + bottom.min(g.lanes_bottom)) / 2.0;
        Some(Pos2::new(x, y))
    }
}

impl Director {
    /// The rect of the window drawn on top (the last of the middle layers), if any.
    fn top_window(&self) -> Option<Rect> {
        self.ctx.memory(|m| {
            let seen = m.areas().visible_layer_ids();
            let all: Vec<egui::LayerId> = m.layer_ids().collect();
            all.into_iter()
                .rev()
                .filter(|l| l.order == egui::Order::Middle && seen.contains(l))
                .find_map(|l| m.area_rect(l.id))
        })
    }

    /// Say in the JSON what a scene did that a click could not (the call it made instead).
    fn did(&mut self, what: &str) {
        let at = self.frame as f64 / self.o.fps as f64;
        let list = self
            .meta
            .entry("actions")
            .or_insert_with(|| serde_json::Value::Array(Vec::new()));
        if let serde_json::Value::Array(v) = list {
            v.push(serde_json::json!({"t": at, "what": what}));
        }
        eprintln!("{} @{at:.2}s: {what}", self.name);
    }
}

// MARK: scenes

/// A stereo 32-bit float WAV.
fn write_wav(path: &Path, inter: &[f32], rate: u32) {
    let data = (inter.len() * 4) as u32;
    let mut b = Vec::with_capacity(44 + data as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&3u16.to_le_bytes()); // IEEE float
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * 8).to_le_bytes());
    b.extend_from_slice(&8u16.to_le_bytes());
    b.extend_from_slice(&32u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    for s in inter {
        b.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(path, b).expect("the scene's sound");
}

/// Shots 1 and 2, one take of *With All Your Heart* from its first bar: the arrangement
/// playing, a slow push-in on the playhead crossing a bar line (0:00–0:06); then the whole
/// window, a pull-back to the arrangement, the mixer and the browser, Onda in the Stage
/// (0:06–0:14).
fn take_open(o: &Opts) {
    let mut d = Director::new(o, "s1-open", (0.0, 6.0));
    d.open_demo("heart");
    d.no_other_dancers(None);
    let _onda = d.add_onda(true, (0.5, 0.9, 1.0));
    // Shot 1 shows no dancer: the Stage is off until shot 2.
    d.app.settings.stage.hidden = true;
    // The devices browser at the left (View ▸ Devices), as the script's shot 2 shows it.
    d.app
        .workspace
        .reveal(cantus_ui::views::workspace::Pane::Devices);
    d.idle(8);
    d.play_from(Ticks::ZERO, 0.0);
    d.roll();
    d.idle(1);
    // The playhead crosses bar 3 at about 4.1 s: the push-in ends on it.
    let bpm = d.app.project().tempo_bpm;
    let beat_at = |s: f64| s * bpm / 60.0;
    if let Some(p) = d.arrangement_at(beat_at(4.4)) {
        let r = d.framing(p, 1.3);
        d.frame_to(r, 5900.0);
        d.note("push_in_on", serde_json::json!([p.x, p.y]));
    }
    d.until(6.0);
    d.next("s2-window", (6.0, 14.0));
    d.app.settings.stage.hidden = false;
    d.app
        .workspace
        .reveal(cantus_ui::views::workspace::Pane::Stage);
    // The pull-back: from the arrangement's middle to the whole window.
    let c = d.screen().center();
    let r = d.framing(c, 1.35);
    d.camera.push(Key {
        at: 0,
        frames: 0,
        rect: r,
    });
    let full = d.screen();
    d.frame_to(full, 3200.0);
    d.until(8.0);
    d.finish();
}

/// Shot 3 (0:14–0:24): *Morning Light*'s chorus; the piano track's instrument picker, Chorda
/// chosen, Chorda's window over the song, its keys moving; a push-in on the keys.
fn take_piano(o: &Opts) {
    let mut d = Director::new(o, "s3-piano", (14.0, 24.0));
    d.open_demo("ballad");
    d.no_other_dancers(None);
    let piano = d.track("demos.track.piano");
    // Every instrument's picture made before the shot (Fides's among them).
    d.warm_thumbs(
        cantus_ui::picker::PickPurpose::ReplaceInstrument { track: piano },
        None,
    );
    d.app.select_track(piano);
    // Chorda's strings designed before the shot (the picker asks for them when it opens).
    cantus_engine::inst::chorda::warm(RATE as f32);
    d.idle(10);
    // Where the window's keys are: the window opened once before the shot (it opens in the
    // same place again), its keyboard found by its hint, closed.
    d.app.open_inst_editor(piano);
    d.idle(12);
    let hint = cantus_ui::i18n::t("ux.hint.keyboard");
    let head: String = hint.chars().take(18).collect();
    // Kept relative to the window: it may open elsewhere the second time.
    let keys = d
        .top_window()
        .and_then(|w| d.control(w, &head).map(|k| k.translate(-w.min.to_vec2())));
    d.note(
        "keys",
        keys.map_or(serde_json::Value::Null, |k| {
            serde_json::json!([k.left(), k.top(), k.width(), k.height()])
        }),
    );
    if keys.is_none() {
        eprintln!("s3: the keyboard was not found by its hint “{head}”");
    }
    d.app.close_inst_editor();
    d.idle(6);
    let chorus = d.bar(12.0);
    d.play_from(chorus, 1.5);
    d.roll();
    d.pointer = Pos2::new(d.o.size.x * 0.62, d.o.size.y * 0.42);
    d.cursor = true;
    d.until(0.4);
    // The piano's row in the arrangement: the cursor goes there and clicks it.
    let row = d
        .app
        .arrangement
        .geometry()
        .and_then(|g| {
            let i = d.app.project().tracks.iter().position(|t| t.id == piano)?;
            let r = g.rows.get(i)?;
            Some(Pos2::new(g.left - 60.0, r.top + r.height / 2.0))
        })
        .unwrap_or(Pos2::new(140.0, 300.0));
    d.go(row);
    d.click();
    // Its instrument's picker, as *Swap instrument* opens it.
    cantus_ui::views::picker::open(
        &mut d.app,
        cantus_ui::picker::PickPurpose::ReplaceInstrument { track: piano },
    );
    d.did("picker::open(ReplaceInstrument) for the piano track (its header's swap)");
    d.idle(d.frames_for(250.0));
    // Fides seen (the integrator, for the release): the list wheeled down until its tile is
    // in view, the tile hovered, the list wheeled back.
    d.show_tile("Fides", 260.0, 1_600.0, 700.0);
    // The tile, not the details pane's title: the leftmost "Chorda" in the picker.
    let tile = d
        .texts_all("Chorda")
        .into_iter()
        .min_by(|a, b| a.left().total_cmp(&b.left()));
    match tile {
        Some(t) => {
            d.go(t.center());
            d.idle(d.frames_for(200.0));
            d.double_click();
        }
        None => {
            d.did("no Chorda tile on screen: the picker closed by its call");
            cantus_ui::views::picker::close(&mut d.app);
        }
    }
    d.idle(d.frames_for(200.0));
    if d.app.picker.is_some() {
        cantus_ui::views::picker::close(&mut d.app);
    }
    // Chorda already plays the piano: the window opens as a double-click on the track does.
    d.app.open_inst_editor(piano);
    d.did("open_inst_editor(piano)");
    d.idle(d.frames_for(300.0));
    let win = d.top_window();
    if let Some(w) = win {
        d.note(
            "window",
            serde_json::json!([w.left(), w.top(), w.width(), w.height()]),
        );
    }
    // The keys as this window drew them: the long, flat control (its first-open place, kept
    // relative to the window, only if none is found).
    let drawn = win.and_then(|w| {
        d.widgets_in(w)
            .into_iter()
            .filter(|r| {
                w.contains_rect(*r)
                    && r.top() > w.center().y
                    && (400.0..900.0).contains(&r.width())
                    && (16.0..60.0).contains(&r.height())
                    && r.width() > 8.0 * r.height()
            })
            .max_by(|a, b| a.width().total_cmp(&b.width()))
    });
    let keys = drawn.or_else(|| keys.zip(win).map(|(k, w)| k.translate(w.min.to_vec2())));
    d.note(
        "keys_in_shot",
        keys.map_or(serde_json::Value::Null, |k| {
            serde_json::json!([k.left(), k.top(), k.width(), k.height()])
        }),
    );
    match keys {
        Some(k) => {
            // The push-in on the keys, and the cursor plays them on the beat: D, A, D, A
            // (74 BPM, from the chorus's first beat), each held a little under a beat.
            d.go(Pos2::new(k.center().x + 60.0, k.bottom() + 40.0));
            d.until(4.2);
            let r = d.framing(Pos2::new(k.center().x, k.center().y - 40.0), 1.5);
            d.frame_to(r, 2400.0);
            d.note(
                "push_in_on",
                serde_json::json!([k.center().x, k.center().y]),
            );
            let white = |note: u8| -> Pos2 {
                let map = [0, 0, 1, 1, 2, 3, 3, 4, 4, 5, 5, 6];
                let i = (note as usize / 12 - 3) * 7 + map[note as usize % 12];
                let w = k.width() / 36.0;
                Pos2::new(k.left() + (i as f32 + 0.5) * w, k.top() + 0.8 * k.height())
            };
            let beat = 60.0 / d.app.project().tempo_bpm as f32;
            for (n, b) in [(74u8, 7.0f32), (81, 8.0), (86, 9.0), (81, 10.0)] {
                let at = b * beat;
                let lead = 0.3;
                d.until(at - lead);
                d.glide(white(n), lead * 1000.0 - 40.0);
                d.until(at);
                d.press_hold(beat * 650.0);
            }
            d.go(Pos2::new(k.center().x + 140.0, k.bottom() + 50.0));
        }
        None => d.did("no keyboard rect: no keys played"),
    }
    d.until(10.0);
    d.finish();
}

/// The drum tracks of *Sunday Strut* (kick, snare, hats, toms) made one: every clip onto the
/// kick's track, renamed *Drums* — what a person does before loading one kit (each a
/// `MoveClipToTrack`, one undo step). Returns the drums' track.
fn one_drum_track(d: &mut Director) -> TrackId {
    let kick = d.track("demos.track.kick");
    let mut cmds = Vec::new();
    let mut others = Vec::new();
    for key in ["demos.track.snare", "demos.track.hats", "demos.track.toms"] {
        let t = d.track(key);
        others.push(t);
        let clips: Vec<_> = d
            .app
            .project()
            .track(t)
            .unwrap()
            .clips
            .iter()
            .map(|c| c.id)
            .collect();
        for c in clips {
            cmds.push(cantus_core::Command::MoveClipToTrack {
                from: t,
                to: kick,
                clip: c,
            });
        }
    }
    cmds.push(cantus_core::Command::SetTrackName {
        id: kick,
        name: "Drums".into(),
    });
    let ok = d.app.exec(cantus_core::Command::Batch {
        label_key: "cmd.move_clips".into(),
        commands: cmds,
    });
    assert!(ok, "the drums' clips moved");
    // The emptied tracks go (their strips stay, unused).
    for t in others {
        let i = d
            .app
            .project()
            .tracks
            .iter()
            .position(|x| x.id == t)
            .unwrap();
        let _ = i;
        d.app.exec(cantus_core::Command::RemoveTrack { id: t });
    }
    kick
}

/// Shot 4 (0:24–0:31): *Sunday Strut*'s verse played by Tympanum, loaded on its drum track
/// as a person loads it (the picker), *Each mic to its own strip* clicked in its mixer; the
/// kit view playing, then its mixer, every mic on a strip of its own.
fn take_drums(o: &Opts) {
    let mut d = Director::new(o, "s4-drums", (24.0, 31.0));
    d.open_demo("funk");
    d.no_other_dancers(None);
    let drums = one_drum_track(&mut d);
    // Tympanum chosen in the drum track's picker.
    cantus_ui::views::picker::open(
        &mut d.app,
        cantus_ui::picker::PickPurpose::ReplaceInstrument { track: drums },
    );
    d.idle(4);
    let key = d
        .app
        .picker_view
        .catalog
        .iter()
        .find(|x| x.choice == PickChoice::Instrument(InstrumentKind::AcousticDrums))
        .map(|x| x.key.clone())
        .expect("Tympanum in the catalogue");
    cantus_ui::views::picker::choose(&mut d.app, &key);
    d.idle(6);
    d.note("tympanum_notes", notes_check(&d, drums));
    d.app.select_track(drums);
    d.app.open_inst_editor(drums);
    d.app.inst.view = cantus_ui::views::inst::View::Mixer;
    d.idle(8);
    match cantus_ui::views::inst::tympanum::plan_rect(
        &d.ctx,
        drums,
        cantus_core::inst_out::OutputPlan::EachOwn,
    ) {
        Some(b) => {
            d.pointer = b.center();
            d.step(Vec::new());
            d.click();
            d.idle(6);
            d.note("each_mic_clicked", true.into());
        }
        None => panic!("s4: Each mic to its own strip was not drawn"),
    }
    d.app.inst.view = cantus_ui::views::inst::View::Play;
    d.idle(8);
    d.play_from(d.bar(4.0), 1.5);
    d.roll();
    d.cursor = true;
    d.pointer = Pos2::new(d.o.size.x * 0.7, d.o.size.y * 0.75);
    d.idle(1);
    // The window framed, a slow push-in.
    if let Some(w) = d.top_window() {
        let r = d.fit(w, 30.0, 1.35);
        d.camera.push(Key {
            at: 0,
            frames: 0,
            rect: d.fit(w, 120.0, 1.35),
        });
        d.frame_to(r, 6500.0);
        d.note(
            "window",
            serde_json::json!([w.left(), w.top(), w.width(), w.height()]),
        );
    }
    d.until(3.2);
    // To the instrument's mixer: its tab in the window's header.
    let w = d.top_window().unwrap_or(d.screen());
    let tab = d
        .text_in("Mixer", w)
        .or_else(|| d.text_in(&cantus_ui::i18n::t("inst.view.mixer"), w));
    match tab {
        Some(t) => {
            d.go(t.center());
            d.click();
        }
        None => {
            d.did("no Mixer tab found in the window: View::Mixer set");
            d.app.inst.view = cantus_ui::views::inst::View::Mixer;
        }
    }
    d.idle(4);
    if d.app.inst.view != cantus_ui::views::inst::View::Mixer {
        d.did("the tab's click did not switch: View::Mixer set");
        d.app.inst.view = cantus_ui::views::inst::View::Mixer;
    }
    d.until(7.0);
    d.finish();
}

/// Which of the drum track's pitches the Tympanum kit has a piece for.
fn notes_check(d: &Director, track: TrackId) -> serde_json::Value {
    let mut used = std::collections::BTreeSet::new();
    if let Some(t) = d.app.project().track(track) {
        for c in &t.clips {
            for n in &c.notes {
                used.insert(n.pitch);
            }
        }
    }
    let kit = cantus_core::inst::tympanum_kit::modelled_kit("studio");
    let keys: std::collections::BTreeSet<u8> = kit
        .map(|k| {
            k.pieces
                .iter()
                .flat_map(|p| p.articulations.iter().map(|a| a.key))
                .collect()
        })
        .unwrap_or_default();
    let missing: Vec<u8> = used.iter().copied().filter(|p| !keys.contains(p)).collect();
    eprintln!("s4: the song's drum pitches {used:?}; the kit's keys {keys:?}; missing {missing:?}");
    serde_json::json!({"used": used, "kit_keys": keys, "missing": missing})
}

/// Shot 5 (0:31–0:44): *Neon Tide*'s drop on the desk. The cursor clicks a strip's empty
/// insert slot, picks Cribrum (the auto filter) in the picker, its face opens with its
/// starting preset, and its cutoff knob is turned: the sound changes.
fn take_mixer(o: &Opts) {
    let mut d = Director::new(o, "s5-mixer", (31.0, 44.0));
    d.open_demo("house");
    d.no_other_dancers(None);
    d.big_desk();
    let target = d.track("demos.track.chords");
    let strip = d.strip_of(target);
    // Every effect's picture made before the shot (Nitor's among them).
    {
        use cantus_core::EffectKind as E;
        use cantus_ui::views::picker_snap::Subject;
        // The tiles the shot rests on: the list's top (EQ, Dynamics) where Cribrum is picked,
        // and its end (Stereo, Mastering, Utility) where Nitor is shown.
        let rest = [
            E::Eq,
            E::AutoFilter,
            E::ResonanceSuppressor,
            E::Compressor,
            E::Gate,
            E::DeEsser,
            E::Multiband,
            E::Limiter,
            E::Clipper,
            E::Transient,
            E::TransientSplit,
            E::BusCompressor,
            E::Ducker,
            E::Stereo,
            E::Maximizer,
            E::Reference,
            E::Enhancer,
            E::Analyzer,
            E::Utility,
            E::Repair,
        ];
        let subjects = rest.iter().filter_map(|k| Subject::of_effect(*k)).collect();
        d.warm_thumbs(
            cantus_ui::picker::PickPurpose::AddEffect { strip },
            Some(subjects),
        );
    }
    d.idle(10);
    // Where the face's cutoff knob sits in its window: Cribrum inserted once before the
    // shot, the knob found by its hint (the biggest control named so), then undone.
    d.app.add_picked_effect(
        strip,
        &PickChoice::Effect(cantus_core::EffectKind::AutoFilter),
    );
    d.idle(14);
    let mut cut_knob: Option<Rect> = None;
    if let Some(w) = d.top_window() {
        for r in d.widgets_in(w) {
            let name = d.hint_at(r.center()).unwrap_or_default().to_lowercase();
            if name.contains("cutoff")
                && r.width() < 120.0
                && r.height() < 120.0
                && cut_knob.is_none_or(|k| r.area() > k.area())
            {
                cut_knob = Some(r);
            }
        }
        cut_knob = cut_knob.map(|k| k.translate(-w.min.to_vec2()));
    }
    if cut_knob.is_none() {
        eprintln!("s5: no control named Cutoff in the face");
    }
    d.app.undo();
    d.idle(8);
    d.play_from(d.bar(16.0), 1.5);
    // Where the strip's first empty insert is: by its hint, in the strip's column.
    let name = cantus_ui::i18n::t("demos.track.chords");
    let col = d.text(&name).map(|r| {
        Rect::from_min_max(
            Pos2::new(r.center().x - 32.0, r.top() - 10.0),
            Pos2::new(r.center().x + 32.0, d.o.size.y),
        )
    });
    let slot = col.and_then(|c| {
        d.control(c, "insert")
            .or_else(|| d.control(c, "add an effect"))
            .or_else(|| d.control(c, "effect"))
    });
    if slot.is_none() {
        match col {
            Some(c) => eprintln!("s5: hints in the column: {:?}", d.hints_in(c)),
            None => eprintln!("s5: the strip's name “{name}” is not painted"),
        }
    }
    d.note(
        "slot",
        slot.map_or(serde_json::Value::Null, |s| {
            serde_json::json!([s.left(), s.top(), s.width(), s.height()])
        }),
    );
    d.roll();
    d.cursor = true;
    d.pointer = Pos2::new(d.o.size.x * 0.5, d.o.size.y * 0.45);
    d.until(1.4);
    match slot {
        Some(s) => {
            d.go(s.center());
            d.idle(d.frames_for(250.0));
            d.click();
        }
        None => {
            // The inspector's way: the strip selected, *Add an effect* under its inserts.
            d.app.select_strip(strip);
            d.idle(2);
            match d.text("Add an effect") {
                Some(a) => {
                    d.go(a.center());
                    d.idle(d.frames_for(200.0));
                    d.click();
                    d.did("the strip selected by its call; the inspector's Add an effect clicked");
                }
                None => d.did("no empty insert and no Add an effect on screen"),
            }
        }
    }
    d.idle(d.frames_for(300.0));
    if d.app.picker.is_none() {
        cantus_ui::views::picker::open(
            &mut d.app,
            cantus_ui::picker::PickPurpose::AddEffect { strip },
        );
        d.did("picker::open(AddEffect) on the chords' strip");
        d.idle(d.frames_for(300.0));
    }
    // Nitor seen (the integrator, for the release): the list wheeled down to the mastering
    // shelf until its tile is in view, the tile hovered, the list wheeled back to the top.
    d.show_tile("Nitor", 1_800.0, 8_000.0, 700.0);
    let tile = d
        .texts_all("Cribrum")
        .into_iter()
        .min_by(|a, b| a.left().total_cmp(&b.left()));
    match tile {
        Some(t) => {
            d.go(t.center());
            d.idle(d.frames_for(250.0));
            d.double_click();
        }
        None => {
            d.did("no Cribrum tile: add_picked_effect called");
            cantus_ui::views::picker::close(&mut d.app);
            d.app.add_picked_effect(
                strip,
                &PickChoice::Effect(cantus_core::EffectKind::AutoFilter),
            );
        }
    }
    d.idle(d.frames_for(500.0));
    let effect = d
        .app
        .project()
        .effects_of(strip)
        .and_then(|e| {
            e.iter()
                .find(|e| e.spec.kind == cantus_core::EffectKind::AutoFilter)
        })
        .map(|e| e.id);
    d.note("effect_added", effect.is_some().into());
    // The face: its window framed, then the cutoff knob (Filter A's, the lowest "Cutoff").
    d.until(5.0);
    if let Some(w) = d.top_window() {
        d.note(
            "face",
            serde_json::json!([w.left(), w.top(), w.width(), w.height()]),
        );
        let r = d.fit(w, 40.0, 1.5);
        d.frame_to(r, 1200.0);
    }
    let knob = d
        .top_window()
        .zip(cut_knob)
        .map(|(w, k)| k.translate(w.min.to_vec2()));
    match knob {
        Some(k) => {
            let at = k.center();
            d.go(at);
            d.idle(d.frames_for(300.0));
            // Down: the cutoff falls, the chords go dark; then part of the way back.
            d.drag(at + Vec2::new(0.0, 110.0), 2600.0);
            d.idle(d.frames_for(500.0));
            d.drag(at + Vec2::new(0.0, -50.0), 1600.0);
            d.note(
                "knob",
                serde_json::json!([at.x, at.y, k.width(), k.height()]),
            );
        }
        None => d.did("no Cutoff knob found in the face: no knob turned"),
    }
    d.until(11.6);
    let full = d.screen();
    d.frame_to(full, 1300.0);
    d.until(13.0);
    d.finish();
}

// Shot 7 (0:52–1:00): *With All Your Heart* into its first chorus on the desk, the master
/// selected, its chain in view; Speculum, the last insert, opened: the loudness meter
/// landing against the song's target.
fn take_master(o: &Opts) {
    let mut d = Director::new(o, "s7-master", (52.0, 60.0));
    d.open_demo("heart");
    d.no_other_dancers(None);
    d.add_onda(true, (0.5, 0.9, 1.0));
    d.app.settings.stage.hidden = true;
    d.app.show_central(cantus_ui::app::CentralView::Mixer);
    d.app.select_master();
    d.idle(10);
    d.play_from(d.bar(heard_bar(52.0)), 2.0);
    d.roll();
    d.cursor = true;
    d.pointer = Pos2::new(d.o.size.x * 0.45, d.o.size.y * 0.5);
    d.until(1.0);
    let speculum = cantus_ui::i18n::t("fx.name.analyzer");
    let row = d.text("Speculum").or_else(|| d.text(&speculum));
    let effect = d
        .app
        .project()
        .master
        .effects
        .iter()
        .find(|e| e.spec.kind == cantus_core::EffectKind::Analyzer)
        .map(|e| e.id);
    match row {
        Some(r) => {
            d.go(r.center());
            d.idle(d.frames_for(300.0));
            d.double_click();
        }
        None => d.did("no Speculum row on screen"),
    }
    d.idle(d.frames_for(400.0));
    if let Some(e) = effect {
        if d.top_window().is_none() {
            d.app.open_effect_editor(StripId::MASTER, e);
            d.did("open_effect_editor(master, Speculum)");
        }
    }
    d.idle(d.frames_for(400.0));
    if let Some(w) = d.top_window() {
        d.go(Pos2::new(w.right() + 30.0, w.center().y).min(d.screen().max - Vec2::splat(30.0)));
        let r = d.framing(
            w.center(),
            (d.o.size.x / (w.width() + 200.0)).clamp(1.0, 1.35),
        );
        d.frame_to(r, 2500.0);
    }
    d.until(8.0);
    d.finish();
}

/// Shot 6a (0:44–0:46.7): recording. *With All Your Heart*'s chorus with a guitar track
/// armed at the top, its input meter moving: the interface's input is fed, as a guitar
/// played into it would be, with the song's own nylon guitar rendered alone by the engine
/// beforehand (a sound Cantus made). The input is not monitored, so the song is heard once.
fn take_record(o: &Opts) {
    let cut = (44.0, 44.0 + 8.0 / 3.0);
    let mut d = Director::new(o, "s6a-record", cut);
    d.open_demo("heart");
    d.no_other_dancers(None);
    let guitar = d.track("demos.track.guitar");
    let at = d.bar(heard_bar(44.0));
    // The guitar alone, from a little before the shot to its end.
    let pre = 1.5f32;
    let p = d.app.project().clone();
    let bpm = p.tempo_bpm;
    let from = Ticks((at.0 as f64 - pre as f64 * bpm / 60.0 * cantus_core::PPQ as f64) as i64);
    let mut solo = p.clone();
    for t in solo.tracks.iter_mut() {
        if t.id != guitar {
            t.clips.clear();
        }
    }
    let seconds = pre as f64 + (cut.1 - cut.0) as f64 + 0.5;
    let mut snap = cantus_core::Snapshot::compile(&solo, 1);
    let _ = cantus_engine::SampleCache::new().attach_audio(&mut snap, RATE as u32);
    let opts = cantus_engine::RenderOptions {
        sample_rate: RATE as u32,
        start: from,
        end: Some(Ticks(
            from.0 + (seconds * bpm / 60.0 * cantus_core::PPQ as f64) as i64,
        )),
        tail_seconds: 0.0,
        block_frames: 512,
        ..Default::default()
    };
    let input: Vec<f32> = cantus_engine::render(std::sync::Arc::new(snap), &opts)
        .map(|r| {
            r.samples
                .chunks_exact(2)
                .map(|f| 0.5 * (f[0] + f[1]))
                .collect()
        })
        .unwrap_or_default();
    d.note("input_seconds", (input.len() as f64 / RATE).into());
    let (ring, consumer) = rtrb::RingBuffer::<f32>::new(RATE as usize);
    // The guitar's track, armed, its input the interface's first.
    let mut t = d.app.bus.new_track("Guitar take".to_string());
    t.input = Some(cantus_core::audio::TrackInput {
        monitor: cantus_core::audio::MonitorMode::On,
        ..cantus_core::audio::TrackInput::mono(0)
    });
    t.armed = true;
    t.color = d
        .app
        .project()
        .track(guitar)
        .map(|g| g.color)
        .unwrap_or(t.color);
    let id = t.id;
    d.app.exec(cantus_core::Command::InsertTrack {
        index: 0,
        track: Box::new(t),
    });
    d.app.select_track(id);
    // The song's own guitar part muted: the guitar heard is the one coming in.
    d.app.exec(cantus_core::Command::SetTrackMute {
        id: guitar,
        muted: true,
    });
    d.idle(6);
    // The input starts with the song: the pre-roll plays it from its start, fed a block at
    // a time (a backlog is shed, as from a device whose clock runs ahead).
    d.proc.attach_input(consumer, 1);
    d.feed = Some(Feed {
        ring,
        data: input,
        pos: 0,
    });
    d.play_from(at, pre);
    d.roll();
    d.cursor = true;
    d.pointer = Pos2::new(d.o.size.x * 0.55, d.o.size.y * 0.5);
    if let Some(g) = d.app.arrangement.geometry() {
        if let Some(r) = g.rows.first() {
            let c = Pos2::new(g.left * 0.6, r.top + r.height / 2.0);
            let fr = d.framing(Pos2::new(c.x + 300.0, c.y + 120.0), 1.5);
            d.camera.push(Key {
                at: 0,
                frames: 0,
                rect: fr,
            });
            let fr2 = d.framing(Pos2::new(c.x + 330.0, c.y + 130.0), 1.6);
            d.frame_to(fr2, 2600.0);
            d.note("armed_row", serde_json::json!([c.x, c.y]));
        }
    }
    d.until(cut.1 - cut.0);
    d.finish();
}

/// Shot 6b (0:46.7–0:49.3): the session view. *Neon Tide*'s drop clips taken into the
/// session (Live's drag from the arrangement: `slots_from_arrangement`), the Session pane in
/// front, the cursor launching a scene with a real click on its launcher.
fn take_session(o: &Opts) {
    let cut = (44.0 + 8.0 / 3.0, 44.0 + 16.0 / 3.0);
    let mut d = Director::new(o, "s6b-session", cut);
    d.open_demo("house");
    d.no_other_dancers(None);
    let (a, b) = (d.bar(16.0), d.bar(32.0));
    let (c, e) = (d.bar(40.0), d.bar(56.0));
    let p = d.app.project().clone();
    let mut clips = Vec::new();
    for (from, to) in [(a, b), (c, e)] {
        for t in &p.tracks {
            for k in &t.clips {
                if k.automation.is_none() && k.start >= from && k.start < to {
                    clips.push((t.id, k.id));
                }
            }
        }
    }
    let bus = &mut d.app.bus;
    let mut ids = || bus.alloc_id();
    let cmd = cantus_core::session::edit::slots_from_arrangement(&p, &clips, 0, &mut ids);
    match cmd {
        Some(c) => {
            d.app.exec(c);
        }
        None => d.did("no clips taken into the session"),
    }
    d.app.show_pane(cantus_ui::views::workspace::Pane::Session);
    d.idle(8);
    d.play_from(d.bar(8.0), 0.5);
    let scenes = d.app.project().session.scenes.len();
    d.note("scenes", scenes.into());
    // The first scene launched before the shot (playing), the next one by the cursor.
    if let Some(s0) = d.app.project().session.scenes.first().map(|s| s.id) {
        cantus_ui::views::session::launch_scene(&mut d.app, s0, 1.0);
        d.idle(2);
        cantus_ui::views::session::release_scene(&mut d.app, s0);
    }
    d.idle(d.frames_for(1500.0));
    d.roll();
    d.cursor = true;
    // The grid framed: its clip columns, the first rows.
    let geo = d.app.session_view.geo.clone();
    if let Some(g) = &geo {
        let n = g.tracks.len().min(10);
        let rows = g.scenes.len().min(4);
        if n > 0 && rows > 0 {
            let r = g.head(0).union(g.cell(n - 1, rows - 1));
            let fr = d.fit(r, 16.0, 1.6);
            d.camera.push(Key {
                at: 0,
                frames: 0,
                rect: fr,
            });
            d.note(
                "grid",
                serde_json::json!([r.left(), r.top(), r.width(), r.height()]),
            );
            d.pointer = Pos2::new(r.center().x, r.bottom() + 30.0);
        }
    }
    d.until(0.25);
    // Two clips of the second scene launched by their own launchers: the chords, then the
    // lead (Live's way: a click on a clip's triangle).
    if let Some(g) = &geo {
        let cols: Vec<(Option<usize>, f32)> =
            [("demos.track.chords", 0.4f32), ("demos.track.lead", 1.4)]
                .iter()
                .map(|(name, at)| {
                    let n = cantus_ui::i18n::t(name);
                    let i = g
                        .tracks
                        .iter()
                        .position(|t| d.app.project().track(*t).is_some_and(|x| x.name == n));
                    (i, *at)
                })
                .collect();
        for (i, at) in cols {
            match i {
                Some(i) if g.scenes.len() > 1 => {
                    let l = cantus_ui::views::session::paint::launcher_rect(g.cell(i, 1), g.k);
                    d.until(at);
                    d.go(l.center());
                    d.click();
                }
                _ => d.did("no column for a clip to launch"),
            }
        }
    }
    d.until(cut.1 - cut.0);
    d.finish();
}

/// The repository's test gain plug-in (CLAP), built by `tools/video/film/draft.sh` into
/// `target/test-plugins`: a hosted plug-in that is free to show.
fn test_gain() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    root.join("target")
        .join("test-plugins")
        .join("debug")
        .join(if cfg!(windows) {
            "cantus_test_clap.dll"
        } else {
            "libcantus_test_clap.so"
        })
}

/// Shot 6c (0:49.3–0:52): a hosted CLAP plug-in on the desk: the test gain loaded into a
/// strip's insert slot of *Neon Tide* through the picker's path (`add_picked_effect` with
/// the scanned plug-in), its slot named and marked as hosted; the cursor on it.
fn take_plugin(o: &Opts) {
    let cut = (44.0 + 16.0 / 3.0, 52.0);
    let mut d = Director::new(o, "s6c-plugin", cut);
    d.open_demo("house");
    d.no_other_dancers(None);
    d.app.show_central(cantus_ui::app::CentralView::Mixer);
    let lib = test_gain();
    let target = d.track("demos.track.lead");
    let strip = d.strip_of(target);
    if lib.exists() {
        let folder = d.o.out.join("tmp").join("plugins");
        let _ = std::fs::create_dir_all(&folder);
        let file = folder.join("Cantus Test Gain.clap");
        let _ = std::fs::copy(&lib, &file);
        use cantus_host::Backend;
        let plugins = cantus_host::clap::ClapBackend
            .scan(&file)
            .unwrap_or_default();
        let info = plugins.first().cloned();
        let (modified, size) = cantus_host::db::stamp(&file).unwrap_or((0, 0));
        d.app.plugins.db_path = None;
        d.app.plugins.db.put(cantus_host::DbEntry {
            path: file,
            format: cantus_host::Format::Clap,
            modified,
            size,
            status: cantus_host::ScanStatus::Ok,
            plugins,
        });
        d.app.plugins.default_isolation = cantus_core::PluginIsolation::Within;
        match info {
            Some(i) => {
                let ok = d
                    .app
                    .add_picked_effect(strip, &PickChoice::Hosted(Box::new(i)));
                d.note("plugin_added", ok.into());
                d.did("add_picked_effect(Hosted test gain) on the lead's strip");
            }
            None => d.did("the test gain did not scan"),
        }
    } else {
        d.did("no test plug-in built (target/test-plugins): the shot shows the desk only");
    }
    d.idle(10);
    // Its window, if it opened, closed: the slot is the picture.
    d.app.select_strip(strip);
    d.play_from(d.bar(24.0), 1.0);
    d.roll();
    d.cursor = true;
    d.pointer = Pos2::new(d.o.size.x * 0.5, d.o.size.y * 0.4);
    d.idle(2);
    let slot = d.text("Cantus Test Gain");
    match slot {
        Some(s) => {
            let fr = d.framing(s.center(), 1.6);
            d.frame_to(fr, 1800.0);
            d.go(Pos2::new(s.right() + 14.0, s.center().y + 6.0));
        }
        None => d.did("the plug-in's slot name is not on screen"),
    }
    d.until(cut.1 - cut.0);
    d.finish();
}

/// The soundtrack's edit (`soundtrack.rs`): *With All Your Heart* from bar 1, and from its
/// bar 12 (a 2-s crossfade there) 44 bars ahead, so its last chord rings at the film's end.
/// The Heart bar heard at film time `film_s`: what s6a, s7, s8 and s9 play and show.
fn heard_bar(film_s: f64) -> f64 {
    let bar = 4.0 * 60.0 / 116.0;
    let b = film_s / bar;
    if b >= 12.0 {
        b + 44.0
    } else {
        b
    }
}

/// Where `pane` is docked (as laid out last frame).
fn leaf_of(d: &Director, pane: cantus_ui::views::workspace::Pane) -> Option<Rect> {
    let dock = &d.app.workspace.dock;
    let path = dock.find_tab(&pane)?;
    dock.leaf(path.node_path()).ok().map(|l| l.rect)
}

/// Shot 8 (1:00–1:08): the bars the soundtrack plays there (the outro); a slow glide across the desk, ending
/// on Onda in the Stage, framed close.
fn take_desk(o: &Opts) {
    let mut d = Director::new(o, "s8-desk", (60.0, 68.0));
    d.open_demo("heart");
    d.no_other_dancers(None);
    d.add_onda(true, (0.5, 0.9, 1.0));
    d.app.settings.stage.hidden = false;
    d.big_desk();
    d.app
        .workspace
        .reveal(cantus_ui::views::workspace::Pane::Stage);
    d.idle(10);
    d.play_from(d.bar(heard_bar(60.0)), 1.5);
    d.roll();
    let s = d.screen();
    let start = d.framing(Pos2::new(s.width() * 0.2, s.height() * 0.45), 1.35);
    d.camera.push(Key {
        at: 0,
        frames: 0,
        rect: start,
    });
    let across = d.framing(Pos2::new(s.width() * 0.52, s.height() * 0.45), 1.35);
    d.frame_to(across, 4800.0);
    d.until(5.0);
    match leaf_of(&d, cantus_ui::views::workspace::Pane::Stage) {
        Some(st) => {
            let z = (s.height() / (st.height() + 80.0)).clamp(1.2, 2.0);
            let r = d.framing(st.center(), z);
            d.frame_to(r, 2200.0);
            d.note(
                "stage",
                serde_json::json!([st.left(), st.top(), st.width(), st.height()]),
            );
        }
        None => d.did("no Stage pane in the dock"),
    }
    d.until(8.0);
    d.finish();
}

/// Shot 9 (1:08–1:20): the song's ending behind the end card, which covers it; no dancer.
fn take_end(o: &Opts) {
    let mut d = Director::new(o, "s9-end", (68.0, 80.0));
    d.open_demo("heart");
    d.no_other_dancers(None);
    d.idle(10);
    // The bars the soundtrack plays there, the song's ending. No dancer (the owner, 2026-11-21).
    d.play_from(d.bar(heard_bar(68.0)), 1.0);
    d.roll();
    d.until(12.0);
    d.finish();
}

/// One frame of each song's window as it opens, to look at the layouts.
fn take_look(o: &Opts) {
    for id in ["heart", "ballad", "funk", "house"] {
        let mut d = Director::new(o, &format!("look-{id}"), (0.0, 1.0 / o.fps as f32));
        d.open_demo(id);
        d.no_other_dancers(None);
        d.play_from(d.bar(8.0), 1.0);
        d.roll();
        d.finish();
    }
}

type Take = fn(&Opts);

/// Every take, with the scenes it writes.
const TAKES: &[(&[&str], Take)] = &[
    (
        &["look-heart", "look-ballad", "look-funk", "look-house"],
        take_look,
    ),
    (&["s1-open", "s2-window"], take_open),
    (&["s3-piano"], take_piano),
    (&["s4-drums"], take_drums),
    (&["s5-mixer"], take_mixer),
    (&["s6a-record"], take_record),
    (&["s6b-session"], take_session),
    (&["s6c-plugin"], take_plugin),
    (&["s7-master"], take_master),
    (&["s8-desk"], take_desk),
    (&["s9-end"], take_end),
];

fn main() {
    let o = Opts::parse();
    // Temporary files on the work drive, never on the system's.
    let tmp = o.out.join("tmp").join("temp");
    std::fs::create_dir_all(&tmp).expect("the temporary folder");
    std::env::set_var("TEMP", &tmp);
    std::env::set_var("TMP", &tmp);
    // The scenes built on the painting thread: a frame cannot wait for a worker.
    cantus_ui::scene::set_blocking(true);
    // The whole app on a thread of its own: Windows' main thread's stack is too small for an
    // assembled app's frame.
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(move || {
            for (scenes, take) in TAKES {
                let wanted = if o.scenes.is_empty() {
                    !scenes[0].starts_with("look")
                } else {
                    scenes.iter().any(|s| o.scenes.iter().any(|w| w == s))
                };
                if wanted {
                    take(&o);
                }
            }
        })
        .expect("a thread")
        .join()
        .expect("the film");
}
