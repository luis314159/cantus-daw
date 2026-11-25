//! `editors` (round 42, ADR 0161): the pin key on a device window's title bar — before (the
//! title bar with no pin, as it was), after (the leaning pin; under the pointer; pinned) —
//! and the whole window through `CantusApp::assemble` with editors open beside the mixer:
//! before (one at a time: the second instrument replaced the first), after (Calor pinned,
//! Modulus and a Terminus open with it, each placed where it covers the others least).

use cantus_core::{EffectKind, InstrumentKind, InstrumentSpec, NativeSpec};
use cantus_engine::EngineHandle;
use cantus_ui::kit::shell::{Footer, Header, Preset, Shell};
use cantus_ui::kit::window_ctl::WindowCtl;
use cantus_ui::views::workspace_device as device;
use cantus_ui::CantusApp;
use egui::{Event, PointerButton, Pos2, Rect, Vec2};
use std::path::Path;

/// Frames for what opened to finish fading in.
const SETTLE: usize = 30;

struct Rig {
    app: CantusApp,
    ctx: egui::Context,
    size: Vec2,
    pointer: Pos2,
    t: f64,
    tex: cantus_ui::raster::Textures,
    shapes: Vec<egui::epaint::ClippedShape>,
    _dir: tempfile::TempDir,
}

impl Rig {
    fn new(size: Vec2) -> Rig {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"audio_setup":{"setup_done":true},"start_page_at_start":false,"plugin_scan_at_start":false}"#,
        )
        .unwrap();
        let files = cantus_ui::app::Files::read(dir.path(), Some(cantus_ui::i18n::locale()));
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let p = cantus_ui::welcome_project();
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
        let mut r = Rig {
            app,
            ctx,
            size,
            pointer: Pos2::new(size.x * 0.5, size.y - 4.0),
            t: 0.0,
            tex: Default::default(),
            shapes: Vec::new(),
            _dir: dir,
        };
        let tracks: Vec<_> = r.app.project().tracks.iter().map(|t| t.id).collect();
        for (t, kind) in tracks
            .iter()
            .zip([InstrumentKind::Analog, InstrumentKind::Fm])
        {
            r.app
                .set_instrument(*t, InstrumentSpec::Native(NativeSpec::new(kind)));
        }
        r.idle(6);
        r
    }

    fn frame(&mut self, mut events: Vec<Event>) {
        self.t += 1.0 / 60.0;
        events.insert(0, Event::PointerMoved(self.pointer));
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events,
            ..Default::default()
        };
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

    fn click(&mut self, p: Pos2) {
        self.pointer = p;
        self.idle(2);
        for pressed in [true, false] {
            self.frame(vec![Event::PointerButton {
                pos: p,
                button: PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            }]);
        }
        self.idle(4);
    }

    fn open(&mut self, n: usize) {
        let t = self.app.project().tracks[n].id;
        self.app.select_track(t);
        self.app.open_instrument_window(t);
        self.idle(SETTLE);
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

/// The title bar of an instrument's window, its right end (the window keys) cropped and
/// drawn at 2× so the pin reads: `pin` as the window shows it, `hover` the pointer on it.
fn title_bar(dir: &Path, name: &str, pin: Option<bool>, hover: bool) {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let size = Vec2::new(1040.0, 120.0);
    let shell = Shell::new(("shot-editors", 1u8));
    let ctl = WindowCtl {
        swap: Some("swap.tip.instrument"),
        minimise: true,
        maximised: Some(false),
        close: true,
        pin,
    };
    // Where the pin key sits: the fourth key from the right (close, maximise, minimise).
    let k = cantus_ui::kit::window_ctl::BUTTON_W;
    let at = Pos2::new(size.x - 3.5 * k, 14.0);
    let canvas = cantus_ui::raster::shoot_at(
        &ctx,
        size,
        2.0,
        12,
        None,
        |_| hover.then_some(at),
        |ui| {
            let header = Header {
                name: "Calor",
                context: "(Keys)",
                hue: cantus_ui::kit::Function::Source,
                preset: Some(Preset {
                    name: "Init",
                    modified: false,
                    favourite: false,
                }),
                ab: Some(cantus_ui::kit::shell::Ab::A),
                undo: Some((true, false)),
                views: None,
                views_w: 0.0,
                bypassed: None,
                help: false,
                name_w: 0.0,
                window: Some(ctl),
            };
            let footer = Footer {
                chips: &[],
                meter: None,
            };
            let _ = shell.show(ui, &header, &footer, |_, _, _| {});
        },
    );
    // The right half of the header, at 2×.
    let (x0, y0) = (((size.x - 420.0) * 2.0) as usize, 0usize);
    let (w, h) = ((420.0 * 2.0) as usize, (40.0 * 2.0) as usize);
    let mut pix = tiny_skia::Pixmap::new(w as u32, h as u32).unwrap();
    for (i, d) in pix.pixels_mut().iter_mut().enumerate() {
        let [r, g, b, a] = canvas.pixel(x0 + i % w, y0 + i / w);
        *d = tiny_skia::ColorU8::from_rgba(r, g, b, a).premultiply();
    }
    let path = dir.join(name);
    std::fs::write(&path, pix.encode_png().unwrap()).unwrap();
    println!("{}", path.display());
}

pub fn shots(dir: &Path) {
    // The whole app on a thread of its own: Windows' main thread's 1 MB stack is too small
    // for an assembled app's frame.
    let dir = dir.to_path_buf();
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || run(&dir))
        .unwrap()
        .join()
        .unwrap();
}

fn run(dir: &Path) {
    title_bar(dir, "editors-pin-before.png", None, false);
    title_bar(dir, "editors-pin-after.png", Some(false), false);
    title_bar(dir, "editors-pin-after-hover.png", Some(false), true);
    title_bar(dir, "editors-pin-after-pinned.png", Some(true), false);
    let size = Vec2::new(2560.0, 1400.0);
    // Before: one at a time, no pin — Modulus took Calor's place.
    let mut r = Rig::new(size);
    r.open(0);
    r.open(1);
    r.shoot(&dir.join("editors-several-before.png"));
    // After: Calor pinned, Modulus beside it, a Terminus opened from the mixer.
    let mut r = Rig::new(size);
    r.open(0);
    let a = r.app.project().tracks[0].id;
    let key = cantus_ui::views::inst::editor_id(a).value();
    if let Some(pin) = device::buttons(&r.ctx, key).and_then(|b| b.pin) {
        r.click(pin.center());
    }
    r.open(1);
    let strip = r.app.project().tracks[1].strip.unwrap();
    r.app.add_effect(strip, EffectKind::Limiter);
    let fx = r.app.project().strip(strip).unwrap().effects.last().unwrap().id;
    r.app.open_effect_editor(strip, fx);
    r.idle(SETTLE);
    r.pointer = Pos2::new(size.x * 0.5, size.y - 4.0);
    r.idle(SETTLE);
    r.shoot(&dir.join("editors-several-after.png"));
}
