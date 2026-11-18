//! A desk of screens for the several-screens tests (ADR 0148): the application through
//! `CantusApp::assemble`, run **as eframe runs windows** — egui's viewports not embedded;
//! the main window's pass, then each window of its own's deferred callback in a pass of its
//! own with its own `RawInput` (`glow_integration::run_ui_and_paint`), the application lent
//! to it as `boot::Shell` lends it and its input given its scale as the shell's
//! `raw_input_hook` gives it (`windows_pass::adjust_input`). Each window is fed as
//! egui-winit feeds a window on its screen: sizes in points are its physical pixels over its
//! system's scale times egui's zoom. Repaint requests are recorded as eframe hears them (the
//! context's repaint callback).

#![allow(dead_code)]

use cantus_engine::EngineHandle;
use cantus_ui::screens::ScreenKey;
use cantus_ui::CantusApp;
use egui::{Event, Pos2, Rect, Vec2, ViewportId};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The taskbar a maximised window leaves, in points of its screen.
pub const TASKBAR: f32 = 48.0;

pub fn screen(name: &str, w: u32, h: u32, x: i32, y: i32, scale: f32) -> ScreenKey {
    ScreenKey::new(name, [w, h], [x, y], scale)
}

/// The issue-#1 reporter's laptop: 2 880 × 1 800 at 200 %.
pub fn laptop() -> ScreenKey {
    screen(r"\\.\DISPLAY1", 2880, 1800, 0, 0, 2.0)
}

/// A 1 920 × 1 080 screen at 100 % to the laptop's right.
pub fn external() -> ScreenKey {
    screen(r"\\.\DISPLAY2", 1920, 1080, 2880, 0, 1.0)
}

/// What one pass drew.
#[derive(Default)]
pub struct Pass {
    pub shapes: Vec<egui::epaint::ClippedShape>,
    pub ppp: f32,
    pub size: Vec2,
}

pub struct Rig {
    pub app: Option<Box<CantusApp>>,
    pub ctx: egui::Context,
    pub t: f64,
    pub screens: Vec<ScreenKey>,
    /// The screen the main window is maximised on.
    pub main_screen: usize,
    pub passes: HashMap<ViewportId, Pass>,
    pub cbs: HashMap<ViewportId, Arc<egui::DeferredViewportUiCallback>>,
    pub pointers: HashMap<ViewportId, Pos2>,
    /// Every repaint asked for since last cleared, as eframe hears it.
    pub asked: Arc<Mutex<Vec<(ViewportId, Duration)>>>,
    pub dir: PathBuf,
    /// The engine's processor, driven a block per main-window pass.
    pub proc_: cantus_engine::Processor,
    _tmp: Option<tempfile::TempDir>,
}

/// A data folder of the test binary's own, configured (no first-run setup, no start page).
pub fn data() -> &'static Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

pub fn configure(dir: &Path) {
    let path = dir.join("settings.json");
    if !path.exists() {
        std::fs::write(
            &path,
            r#"{"audio_setup":{"setup_done":true},"start_page_at_start":false,"plugin_scan_at_start":false,"ui_scale_follows_screen":true}"#,
        )
        .unwrap();
    }
}

impl Rig {
    /// The application over a fresh configuration folder, on `screens` (the main window
    /// maximised on the first).
    pub fn new(screens: Vec<ScreenKey>) -> Rig {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let mut r = Rig::in_dir(&dir, screens, Some(cantus_ui::welcome_project()));
        r._tmp = Some(tmp);
        r
    }

    /// The application over the configuration folder `dir` (its layouts file read).
    pub fn in_dir(
        dir: &Path,
        screens: Vec<ScreenKey>,
        project: Option<cantus_core::Project>,
    ) -> Rig {
        let _ = data();
        configure(dir);
        let files = cantus_ui::app::Files::read(dir, Some(cantus_ui::i18n::Locale::En));
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let asked = Arc::new(Mutex::new(Vec::new()));
        {
            let asked = asked.clone();
            ctx.set_request_repaint_callback(move |info| {
                asked.lock().unwrap().push((info.viewport_id, info.delay));
            });
        }
        let (engine, proc_) = EngineHandle::manual(48_000.0);
        let mut app = Box::new(CantusApp::assemble(
            &ctx, files, project, engine, None, None, None, false,
        ));
        app.windows.fed_screens = Some(screens.clone());
        Rig {
            app: Some(app),
            ctx,
            t: 0.0,
            screens,
            main_screen: 0,
            passes: HashMap::new(),
            cbs: HashMap::new(),
            pointers: HashMap::new(),
            asked,
            dir: dir.to_path_buf(),
            proc_,
            _tmp: None,
        }
    }

    pub fn app(&mut self) -> &mut CantusApp {
        self.app.as_mut().expect("the app is here between passes")
    }

    /// Plug or unplug screens: the next pass reads them.
    pub fn set_screens(&mut self, screens: Vec<ScreenKey>) {
        self.screens = screens.clone();
        let app = self.app();
        app.windows.fed_screens = Some(screens);
        app.windows.stale = true;
    }

    /// The input egui-winit gives a window on screen `s`, with `events`: maximised, or at the
    /// place it was given when that is not maximised (a device's own window: fx/final).
    fn input(&self, id: ViewportId, s: &ScreenKey, events: Vec<Event>) -> egui::RawInput {
        let zoom = self.ctx.zoom_factor();
        let native = s.scale;
        let ppp = zoom * native;
        let placed = (id != ViewportId::ROOT)
            .then(|| {
                let app = self.app.as_ref()?;
                let i = app.windows.index_of(id)?;
                app.windows.list[i].place.clone()
            })
            .flatten()
            .filter(|p| !p.maximised);
        let (window, corner) = match &placed {
            Some(p) => (
                Vec2::new(p.rect.w as f32, p.rect.h as f32),
                Pos2::new(p.rect.x as f32, p.rect.y as f32),
            ),
            None => (
                Vec2::new(s.size[0] as f32, s.size[1] as f32 - TASKBAR * native),
                Pos2::new(s.pos[0] as f32, s.pos[1] as f32),
            ),
        };
        let size = window / ppp;
        let at = corner / ppp;
        let rect = Rect::from_min_size(Pos2::ZERO, size);
        let mut raw = egui::RawInput {
            viewport_id: id,
            screen_rect: Some(rect),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events,
            ..Default::default()
        };
        let vp = raw.viewports.entry(id).or_default();
        vp.native_pixels_per_point = Some(native);
        vp.monitor_size = Some(Vec2::new(s.size[0] as f32, s.size[1] as f32) / ppp);
        vp.inner_rect = Some(Rect::from_min_size(at, size));
        vp.outer_rect = Some(Rect::from_min_size(at, size));
        vp.maximized = Some(placed.is_none());
        vp.minimized = Some(false);
        vp.focused = Some(true);
        raw
    }

    /// The screen window `id` is on (its place's; the main window's otherwise).
    fn screen_of(&self, id: ViewportId) -> ScreenKey {
        if id != ViewportId::ROOT {
            if let Some(app) = self.app.as_ref() {
                if let Some(i) = app.windows.index_of(id) {
                    if let Some(p) = &app.windows.list[i].place {
                        if let Some(s) = self.screens.iter().find(|s| s.pos == p.screen.pos) {
                            return s.clone();
                        }
                    }
                }
            }
        }
        self.screens[self.main_screen].clone()
    }

    fn keep(&mut self, id: ViewportId, out: &mut egui::FullOutput, size: Vec2) {
        let shapes = std::mem::take(&mut out.shapes);
        self.passes.insert(
            id,
            Pass {
                shapes,
                ppp: out.pixels_per_point,
                size,
            },
        );
        out.textures_delta.clear();
        if id == ViewportId::ROOT {
            self.cbs.retain(|k, _| out.viewport_output.contains_key(k));
            for (vid, v) in &out.viewport_output {
                if let Some(cb) = &v.viewport_ui_cb {
                    self.cbs.insert(*vid, cb.clone());
                }
            }
        }
    }

    /// The main window's pass with `events`.
    pub fn root(&mut self, mut events: Vec<Event>) {
        // A frame's worth of audio (60 frames a second at 48 kHz) before the frame.
        let mut buf = vec![0f32; 800 * 2];
        self.proc_.process(&mut buf, 2);
        self.t += 1.0 / 60.0;
        let s = self.screens[self.main_screen].clone();
        if let Some(p) = self.pointers.get(&ViewportId::ROOT) {
            events.insert(0, Event::PointerMoved(*p));
        }
        let raw = self.input(ViewportId::ROOT, &s, events);
        let size = raw.screen_rect.unwrap().size();
        let mut app = self.app.take().unwrap();
        let mut out = self.ctx.run_ui(raw, |ui| app.frame(ui));
        self.app = Some(app);
        self.keep(ViewportId::ROOT, &mut out, size);
    }

    /// Window `id`'s pass with `events` (its callback, the app lent to it).
    pub fn child(&mut self, id: ViewportId, mut events: Vec<Event>) {
        let Some(cb) = self.cbs.get(&id).cloned() else {
            return;
        };
        self.t += 1.0 / 240.0;
        let s = self.screen_of(id);
        if let Some(p) = self.pointers.get(&id) {
            events.insert(0, Event::PointerMoved(*p));
        }
        // The test speaks in the window's own points (what its last pass drew); egui-winit
        // gives the shell points of egui's zoom, which `adjust_input` turns into the window's:
        // positions are handed over in egui-winit's units.
        let k = self
            .app
            .as_ref()
            .and_then(|a| a.windows.index_of(id).map(|i| a.windows.list[i].factor))
            .unwrap_or(1.0);
        let events = events
            .into_iter()
            .map(|e| match e {
                Event::PointerMoved(p) => Event::PointerMoved((p.to_vec2() * k).to_pos2()),
                Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers,
                } => Event::PointerButton {
                    pos: (pos.to_vec2() * k).to_pos2(),
                    button,
                    pressed,
                    modifiers,
                },
                e => e,
            })
            .collect();
        let mut raw = self.input(id, &s, events);
        let mut app = self.app.take().unwrap();
        cantus_ui::windows_pass::adjust_input(&mut app, &self.ctx, &mut raw);
        let size = raw.screen_rect.unwrap().size();
        cantus_ui::windows_pass::lend(app);
        let mut out = self.ctx.run_ui(raw, |ui| cb(ui));
        self.app = Some(cantus_ui::windows_pass::take_back().expect("given back"));
        self.keep(id, &mut out, size);
    }

    /// The windows of their own, by viewport.
    pub fn windows(&self) -> Vec<ViewportId> {
        let app = self.app.as_ref().unwrap();
        app.windows.list.iter().map(|w| w.id()).collect()
    }

    /// A frame of the desk: the main window's pass, then every window's.
    pub fn frame(&mut self) {
        self.root(Vec::new());
        for id in self.windows() {
            self.child(id, Vec::new());
        }
    }

    pub fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.frame();
        }
    }

    /// Where `words` were painted whole in viewport `id`'s last pass.
    pub fn text_in(&self, id: ViewportId, words: &str) -> Vec<Rect> {
        fn walk(s: &egui::Shape, clip: Rect, words: &str, out: &mut Vec<Rect>) {
            match s {
                egui::Shape::Text(t) => {
                    let r = t.galley.rect.translate(t.pos.to_vec2());
                    if t.galley.text() == words && clip.contains_rect(r) {
                        out.push(r);
                    }
                }
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, words, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        if let Some(p) = self.passes.get(&id) {
            for c in &p.shapes {
                walk(&c.shape, c.clip_rect, words, &mut out);
            }
        }
        out
    }

    /// Every text painted in viewport `id`'s last pass, with its rect and its clip.
    pub fn texts(&self, id: ViewportId) -> Vec<(String, Rect, Rect)> {
        fn walk(s: &egui::Shape, clip: Rect, out: &mut Vec<(String, Rect, Rect)>) {
            match s {
                egui::Shape::Text(t) => {
                    let r = t.galley.rect.translate(t.pos.to_vec2());
                    out.push((t.galley.text().to_string(), r, clip));
                }
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        if let Some(p) = self.passes.get(&id) {
            for c in &p.shapes {
                walk(&c.shape, c.clip_rect, &mut out);
            }
        }
        out
    }

    /// A click of `button` at `p` in viewport `id` (moved there first, a pass each).
    pub fn click(&mut self, id: ViewportId, p: Pos2, button: egui::PointerButton) {
        self.pointers.insert(id, p);
        let ev = |pressed| Event::PointerButton {
            pos: p,
            button,
            pressed,
            modifiers: Default::default(),
        };
        self.pass_in(id, vec![]);
        self.pass_in(id, vec![ev(true)]);
        self.pass_in(id, vec![ev(false)]);
    }

    /// A drag with the primary button from `from` to `to` in viewport `id` (the pointer
    /// keeps reporting outside the window while the button is held, as the system's mouse
    /// capture gives it).
    pub fn drag(&mut self, id: ViewportId, from: Pos2, to: Pos2) {
        let button = |pos, pressed| Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        self.pointers.insert(id, from);
        self.pass_in(id, vec![]);
        self.pass_in(id, vec![button(from, true)]);
        for k in 1..=12 {
            let p = from + (to - from) * (k as f32 / 12.0);
            self.pointers.insert(id, p);
            self.pass_in(id, vec![]);
        }
        self.pass_in(id, vec![button(to, false)]);
        self.pointers.remove(&id);
    }

    /// A key pressed and released in viewport `id`.
    pub fn key(&mut self, id: ViewportId, key: egui::Key, modifiers: egui::Modifiers) {
        let ev = |pressed| Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers,
        };
        // egui-winit tells the modifiers as their own event, before the key.
        self.pass_in(id, vec![Event::ModifiersChanged(modifiers), ev(true)]);
        self.pass_in(
            id,
            vec![ev(false), Event::ModifiersChanged(egui::Modifiers::NONE)],
        );
    }

    /// One pass of viewport `id` with `events` (the main window's or a window's).
    pub fn pass_in(&mut self, id: ViewportId, events: Vec<Event>) {
        if id == ViewportId::ROOT {
            self.root(events);
        } else {
            self.child(id, events);
        }
    }

    pub fn clear_asked(&self) {
        self.asked.lock().unwrap().clear();
    }

    /// Whether a frame was asked for viewport `id` sooner than `within`.
    pub fn was_asked(&self, id: ViewportId, within: Duration) -> bool {
        self.asked
            .lock()
            .unwrap()
            .iter()
            .any(|(v, d)| *v == id && *d < within)
    }
}

/// The app's whole frames on a thread with room: an assembled app's frame wants more stack
/// than a test thread's 2 MB.
pub fn run(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}
