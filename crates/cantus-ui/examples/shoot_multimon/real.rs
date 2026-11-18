//! `multimon-real`: the pictures of several screens **as built** (ADR 0148, phase 2) — no
//! mock. The application through `CantusApp::assemble`, its windows run as eframe runs them
//! (viewports not embedded; the main window's pass, then each window's deferred callback in
//! a pass of its own with its own input, the application lent to it and its input given its
//! screen's scale as `boot::Shell` does), each pass rasterised on the CPU.

use cantus_engine::EngineHandle;
use cantus_ui::screens::ScreenKey;
use cantus_ui::views::workspace::Pane;
use cantus_ui::windows::Target;
use cantus_ui::CantusApp;
use egui::{Event, Pos2, Rect, Vec2, ViewportId};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const TASKBAR: f32 = 48.0;

struct Desk {
    app: Option<Box<CantusApp>>,
    ctx: egui::Context,
    screens: Vec<ScreenKey>,
    tex: cantus_ui::raster::Textures,
    shapes: HashMap<ViewportId, (Vec<egui::epaint::ClippedShape>, f32, Vec2)>,
    cbs: HashMap<ViewportId, Arc<egui::DeferredViewportUiCallback>>,
    t: f64,
    _dir: tempfile::TempDir,
}

impl Desk {
    fn new(screens: Vec<ScreenKey>) -> Desk {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"audio_setup":{"setup_done":true},"start_page_at_start":false,"plugin_scan_at_start":false,"ui_scale_follows_screen":true}"#,
        )
        .unwrap();
        let files = cantus_ui::app::Files::read(dir.path(), Some(cantus_ui::i18n::locale()));
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let p = cantus_ui::demos::house_without_dancer();
        let mut app = Box::new(CantusApp::assemble(
            &ctx,
            files,
            Some(p),
            EngineHandle::dummy(),
            None,
            None,
            None,
            false,
        ));
        crate::studio::studio(&mut app, 256);
        app.windows.fed_screens = Some(screens.clone());
        Desk {
            app: Some(app),
            ctx,
            screens,
            tex: Default::default(),
            shapes: HashMap::new(),
            cbs: HashMap::new(),
            t: 0.0,
            _dir: dir,
        }
    }

    fn input(&self, id: ViewportId, s: &ScreenKey, events: Vec<Event>) -> egui::RawInput {
        let ppp = self.ctx.zoom_factor() * s.scale;
        let size = Vec2::new(s.size[0] as f32, s.size[1] as f32 - TASKBAR * s.scale) / ppp;
        let at = Pos2::new(s.pos[0] as f32, s.pos[1] as f32) / ppp;
        let mut raw = egui::RawInput {
            viewport_id: id,
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events,
            ..Default::default()
        };
        let vp = raw.viewports.entry(id).or_default();
        vp.native_pixels_per_point = Some(s.scale);
        vp.monitor_size = Some(Vec2::new(s.size[0] as f32, s.size[1] as f32) / ppp);
        vp.inner_rect = Some(Rect::from_min_size(at, size));
        vp.outer_rect = Some(Rect::from_min_size(at, size));
        vp.maximized = Some(true);
        vp.focused = Some(true);
        raw
    }

    fn keep(&mut self, id: ViewportId, mut out: egui::FullOutput, size: Vec2) {
        self.tex.update(&out.textures_delta);
        out.textures_delta.clear();
        if id == ViewportId::ROOT {
            self.cbs.retain(|k, _| out.viewport_output.contains_key(k));
            for (vid, v) in &out.viewport_output {
                if let Some(cb) = &v.viewport_ui_cb {
                    self.cbs.insert(*vid, cb.clone());
                }
            }
        }
        self.shapes
            .insert(id, (out.shapes, out.pixels_per_point, size));
    }

    fn frame(&mut self, root_events: Vec<Event>) {
        self.t += 1.0 / 60.0;
        let s = self.screens[0].clone();
        let raw = self.input(ViewportId::ROOT, &s, root_events);
        let size = raw.screen_rect.unwrap().size();
        let mut app = self.app.take().unwrap();
        let out = self.ctx.run_ui(raw, |ui| app.frame(ui));
        self.app = Some(app);
        self.keep(ViewportId::ROOT, out, size);
        let wins: Vec<(ViewportId, Option<ScreenKey>)> = {
            let app = self.app.as_ref().unwrap();
            app.windows
                .list
                .iter()
                .map(|w| (w.id(), w.place.as_ref().map(|p| p.screen.clone())))
                .collect()
        };
        for (id, screen) in wins {
            let Some(cb) = self.cbs.get(&id).cloned() else {
                continue;
            };
            let s = screen
                .and_then(|k| self.screens.iter().find(|s| s.pos == k.pos).cloned())
                .unwrap_or_else(|| self.screens[0].clone());
            let mut raw = self.input(id, &s, Vec::new());
            let mut app = self.app.take().unwrap();
            cantus_ui::windows_pass::adjust_input(&mut app, &self.ctx, &mut raw);
            let size = raw.screen_rect.unwrap().size();
            cantus_ui::windows_pass::lend(app);
            let out = self.ctx.run_ui(raw, |ui| cb(ui));
            self.app = Some(cantus_ui::windows_pass::take_back().unwrap());
            self.keep(id, out, size);
        }
    }

    fn shoot(&self, id: ViewportId, path: &Path) {
        let Some((shapes, ppp, size)) = self.shapes.get(&id) else {
            return;
        };
        let prims = self.ctx.tessellate(shapes.clone(), *ppp);
        let bg = self.ctx.global_style().visuals.panel_fill;
        let mut c = cantus_ui::raster::Canvas::new(
            (size.x * ppp).round() as usize,
            (size.y * ppp).round() as usize,
            bg,
        );
        c.paint(&prims, &self.tex, *ppp);
        let _ = c.write_png(path);
        println!("{}", path.display());
    }

    fn text_at(&self, id: ViewportId, words: &str) -> Option<Rect> {
        fn walk(s: &egui::Shape, words: &str, out: &mut Option<Rect>) {
            match s {
                egui::Shape::Text(t) if t.galley.text() == words => {
                    out.get_or_insert(t.galley.rect.translate(t.pos.to_vec2()));
                }
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, words, out)),
                _ => {}
            }
        }
        let mut out = None;
        for c in &self.shapes.get(&id)?.0 {
            walk(&c.shape, words, &mut out);
        }
        out
    }
}

pub fn shots(dir: &Path) {
    let dir = dir.to_path_buf();
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || run(&dir))
        .unwrap()
        .join()
        .unwrap();
}

fn run(dir: &Path) {
    let lang = match cantus_ui::i18n::locale() {
        cantus_ui::i18n::Locale::Es => "es",
        _ => "en",
    };
    let out = |name: &str| -> PathBuf { dir.join(format!("{name}-{lang}.png")) };
    let laptop = ScreenKey::new(r"\\.\DISPLAY1", [2880, 1800], [0, 0], 2.0);
    let external = ScreenKey::new(r"\\.\DISPLAY2", [1920, 1080], [2880, 0], 1.0);
    let mut d = Desk::new(vec![laptop, external]);
    {
        let app = d.app.as_mut().unwrap();
        let beats = app.project().content_end().0 as f32 / cantus_core::Ticks::beats(1).0 as f32;
        app.arrangement.px_per_beat = 1250.0 / beats.max(1.0);
        app.reveal_pane(Pane::Mixer);
    }
    for _ in 0..10 {
        d.frame(Vec::new());
    }
    // The tab's menu on the mixer's tab, as built.
    if let Some(tab) = d.text_at(
        ViewportId::ROOT,
        &cantus_ui::i18n::t("workspace.pane.mixer"),
    ) {
        let press = |pressed| Event::PointerButton {
            pos: tab.center(),
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: Default::default(),
        };
        d.frame(vec![Event::PointerMoved(tab.center())]);
        d.frame(vec![press(true)]);
        d.frame(vec![press(false)]);
        for _ in 0..12 {
            d.frame(Vec::new());
        }
        d.shoot(ViewportId::ROOT, &out("10-real-tab-menu"));
        d.frame(vec![Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }]);
    }
    // The mixer on the other screen, its strips Large.
    {
        let ctx = d.ctx.clone();
        let app = d.app.as_mut().unwrap();
        use cantus_ui::views::mixer::layout::WidthMode;
        cantus_ui::views::mixer::set_width_mode(app, WidthMode::from_index(2));
        app.move_pane(&ctx, Pane::Mixer, Target::NewWindow { screen: Some(1) });
        app.reveal_pane(Pane::PianoRoll);
    }
    for _ in 0..16 {
        d.frame(Vec::new());
    }
    let main_png = out("11-real-main-laptop-200");
    d.shoot(ViewportId::ROOT, &main_png);
    let id = d.app.as_ref().unwrap().windows.list.first().map(|w| w.id());
    if let Some(id) = id {
        let win_png = out("12-real-mixer-window-100");
        d.shoot(id, &win_png);
        crate::multimon_scene::desk(&main_png, &win_png, &out("13-real-desk"));
    }
}
