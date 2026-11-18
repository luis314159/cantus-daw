//! `issue1-menus`: GitHub issue #1's follow-up — the menus on a 2 880 × 1 800 screen at the
//! system's scale `scale` (Windows' 200 % is `2`), through `CantusApp::assemble`, fed as
//! egui-winit feeds a maximised window (sizes in points are the pixels over the system's
//! scale times the zoom): a mixer strip's right-click menu with *Add effect ▸* and, scrolled
//! to it, *Send to ▸* hovered; an instrument's preset list opened from its well.

use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec};
use cantus_engine::EngineHandle;
use cantus_ui::views::mixer::{self, Control};
use cantus_ui::views::workspace::Pane;
use cantus_ui::CantusApp;
use egui::{Event, PointerButton, Pos2, Rect, Vec2};
use std::path::Path;

/// Frames for what opened to finish fading in (half a second at 60 Hz): a picture taken
/// sooner shows a popup half-transparent that is opaque on the screen.
const SETTLE: usize = 30;

struct Rig {
    app: CantusApp,
    ctx: egui::Context,
    native: f32,
    window: Vec2,
    pointer: Pos2,
    t: f64,
    tex: cantus_ui::raster::Textures,
    shapes: Vec<egui::epaint::ClippedShape>,
    size: Vec2,
    _dir: tempfile::TempDir,
}

impl Rig {
    fn new(native: f32) -> Rig {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"audio_setup":{"setup_done":true},"start_page_at_start":false,"plugin_scan_at_start":false}"#,
        )
        .unwrap();
        let files = cantus_ui::app::Files::read(dir.path(), Some(cantus_ui::i18n::Locale::En));
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let mut p = cantus_ui::welcome_project();
        let mut t = p.make_track("Lead");
        let strip = p.make_strip("Lead");
        t.strip = Some(strip.id);
        p.strips.push(strip);
        t.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Wavetable));
        p.tracks.push(t);
        let app = CantusApp::assemble(
            &ctx,
            files,
            Some(p),
            EngineHandle::dummy(),
            None,
            None,
            None,
            false,
        );
        let monitor = Vec2::new(2880.0, 1800.0);
        let mut r = Rig {
            app,
            ctx,
            native,
            window: Vec2::new(monitor.x, monitor.y - 48.0 * native),
            pointer: Pos2::new(400.0, 300.0),
            t: 0.0,
            tex: Default::default(),
            shapes: Vec::new(),
            size: Vec2::ZERO,
            _dir: dir,
        };
        r.idle(6);
        r
    }

    fn frame(&mut self, mut events: Vec<Event>) {
        self.t += 1.0 / 60.0;
        let ppp = self.ctx.zoom_factor() * self.native;
        self.size = self.window / ppp;
        let rect = Rect::from_min_size(Pos2::ZERO, self.size);
        events.insert(0, Event::PointerMoved(self.pointer));
        let mut input = egui::RawInput {
            screen_rect: Some(rect),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events,
            ..Default::default()
        };
        let vp = input.viewports.entry(egui::ViewportId::ROOT).or_default();
        vp.native_pixels_per_point = Some(self.native);
        vp.monitor_size = Some(Vec2::new(2880.0, 1800.0) / ppp);
        vp.inner_rect = Some(rect);
        vp.outer_rect = Some(rect);
        vp.maximized = Some(true);
        vp.focused = Some(true);
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| app.frame(ui));
        self.tex.update(&out.textures_delta);
        out.textures_delta.clear();
        self.shapes = out.shapes;
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.frame(Vec::new());
        }
    }

    fn press(&mut self, p: Pos2, button: PointerButton) {
        self.pointer = p;
        self.idle(1);
        for pressed in [true, false] {
            self.frame(vec![Event::PointerButton {
                pos: p,
                button,
                pressed,
                modifiers: Default::default(),
            }]);
        }
        self.idle(SETTLE);
    }

    fn hover(&mut self, p: Pos2) {
        self.pointer = p;
        self.idle(SETTLE);
    }

    fn wheel(&mut self, p: Pos2) {
        self.pointer = p;
        self.frame(vec![Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0.0, -1.0),
            phase: egui::TouchPhase::Move,
            modifiers: Default::default(),
        }]);
        self.idle(2);
    }

    /// Where `words` were painted whole inside `region`.
    fn text_in(&self, words: &str, region: Rect) -> Option<Rect> {
        fn walk(s: &egui::Shape, clip: Rect, words: &str, region: Rect, out: &mut Option<Rect>) {
            match s {
                egui::Shape::Text(t) => {
                    let r = t.galley.rect.translate(t.pos.to_vec2());
                    if t.galley.text() == words && region.contains_rect(r) && clip.contains_rect(r)
                    {
                        *out = Some(r);
                    }
                }
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, words, region, out)),
                _ => {}
            }
        }
        let mut out = None;
        for c in &self.shapes {
            walk(&c.shape, c.clip_rect, words, region, &mut out);
        }
        out
    }

    fn area(&self, id: egui::Id) -> Option<Rect> {
        self.ctx.memory(|m| m.area_rect(id))
    }

    fn shoot(&self, path: &Path) {
        let ppp = self.ctx.pixels_per_point();
        let prims = self.ctx.tessellate(self.shapes.clone(), ppp);
        let bg = self.ctx.global_style().visuals.panel_fill;
        let mut c = cantus_ui::raster::Canvas::new(
            (self.size.x * ppp).round() as usize,
            (self.size.y * ppp).round() as usize,
            bg,
        );
        c.paint(&prims, &self.tex, ppp);
        let _ = c.write_png(path);
        println!("{}", path.display());
    }
}

pub fn shots(dir: &Path, scale: f32) {
    // The whole app on a thread of its own: Windows' main thread's 1 MB stack is too small
    // for an assembled app's frame.
    let dir = dir.to_path_buf();
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || run(&dir, scale))
        .unwrap()
        .join()
        .unwrap();
}

fn run(dir: &Path, scale: f32) {
    let tag = format!("{:.0}", scale * 100.0);
    // The mixer: a strip's menu, Add effect ▸ hovered, then Send to ▸.
    let mut r = Rig::new(scale);
    r.app.workspace.reveal(Pane::Mixer);
    r.idle(4);
    let strip = r.app.project().strips[1].id;
    let head = mixer::control_rect(&r.ctx, Some(strip), Control::Head).expect("a strip head");
    r.press(head.center(), PointerButton::Secondary);
    let menu = egui::Id::new(("mixer-menu", 1u64));
    let top = r.area(menu).expect("the strip menu");
    for (key, name) in [
        ("effect.add", "add-effect"),
        ("mixer.menu.send_to", "send-to"),
    ] {
        let words = cantus_ui::i18n::t(key);
        let mut row = None;
        for _ in 0..40 {
            let top = r.area(menu).unwrap_or(top);
            row = r.text_in(&words, top);
            if row.is_some() {
                break;
            }
            r.wheel(top.center());
        }
        let Some(row) = row else {
            eprintln!("{words} not found in the strip menu");
            continue;
        };
        r.hover(row.center());
        r.shoot(&dir.join(format!("mixer-sub-{name}-{tag}.png")));
    }
    // An instrument's preset list, opened from its well.
    let mut r = Rig::new(scale);
    let track = r.app.project().tracks.last().unwrap().id;
    r.app.open_inst_editor(track);
    r.idle(6);
    let shell = cantus_ui::kit::shell::Shell::new(("inst", track.0)).id;
    if let Some(well) = r
        .ctx
        .read_response(shell.with(("part", 1usize)))
        .map(|x| x.rect)
    {
        r.press(well.center(), PointerButton::Primary);
        r.shoot(&dir.join(format!("inst-presets-{tag}.png")));
    } else {
        eprintln!("the preset well was not drawn");
    }
}
