//! `beta14-look`: the owner's two see-through lists of beta.14, through
//! `CantusApp::assemble`, in Slate Paper (his light look) and Slate Night: the audio
//! popover's *Input device* list, and Settings ▸ Audio's *Output device* list over his
//! machine's devices (four ASIO drivers, a microphone array); then the desk in each fader
//! cap. A 1 600 × 900 window at the system's `scale`.

use cantus_engine::{DeviceInfo, DriverInfo, EngineHandle, InputDeviceInfo};
use cantus_ui::audio_setup::{demo, Hit};
use cantus_ui::kit::appearance::Mode;
use cantus_ui::views::workspace_settings::{self, SettingsPage};
use cantus_ui::CantusApp;
use egui::{Event, PointerButton, Pos2, Rect, Vec2};
use std::path::Path;

/// Frames for what opened to finish fading in.
const SETTLE: usize = 30;

struct Rig {
    app: CantusApp,
    ctx: egui::Context,
    native: f32,
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
        let app = CantusApp::assemble(
            &ctx,
            files,
            Some(cantus_ui::starter_project()),
            EngineHandle::dummy(),
            None,
            None,
            None,
            false,
        );
        let mut r = Rig {
            app,
            ctx,
            native,
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
        self.size = Vec2::new(1600.0, 900.0);
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
        self.idle(1);
        for pressed in [true, false] {
            self.frame(vec![Event::PointerButton {
                pos: p,
                button: PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            }]);
        }
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

    fn look(&mut self, preset: &str, light: bool) {
        self.app.appearance.mode = if light { Mode::Light } else { Mode::Dark };
        if light {
            self.app.appearance.light.base = format!("preset:{preset}");
        } else {
            self.app.appearance.dark.base = format!("preset:{preset}");
        }
        self.app.apply_appearance();
        self.idle(4);
    }

    /// Where `words` are painted whole (the topmost).
    fn text(&self, words: &str) -> Option<Rect> {
        fn walk(s: &egui::Shape, clip: Rect, words: &str, out: &mut Option<Rect>) {
            match s {
                egui::Shape::Text(t) => {
                    let r = t.galley.rect.translate(t.pos.to_vec2());
                    if t.galley.text().trim() == words && clip.contains_rect(r) {
                        *out = Some(r);
                    }
                }
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, words, out)),
                _ => {}
            }
        }
        let mut out = None;
        for c in &self.shapes {
            walk(&c.shape, c.clip_rect, words, &mut out);
        }
        out
    }
}

/// The owner's machine as his Settings ▸ Audio shows it.
fn owner_scan() -> cantus_ui::views::settings_audio::Scan {
    let out = |n: &str, d: bool| DeviceInfo {
        name: n.into(),
        is_default: d,
    };
    cantus_ui::views::settings_audio::Scan {
        driver: None,
        drivers: vec![
            DriverInfo {
                name: "WASAPI".into(),
                is_default: true,
            },
            DriverInfo {
                name: "ASIO".into(),
                is_default: false,
            },
        ],
        outputs: vec![
            out("ASIO4ALL v2", true),
            out("FL Studio ASIO", false),
            out("FlexASIO", false),
        ],
        inputs: vec![InputDeviceInfo {
            name: "Microphone Array (AMD Audio Device)".into(),
            channels: 2,
            is_default: true,
        }],
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
    let _ = std::fs::create_dir_all(dir);
    for (preset, light) in [("slate_paper", true), ("slate_night", false)] {
        // The audio popover's Input device list.
        let mut r = Rig::new(scale);
        r.look(preset, light);
        demo::setup(&mut r.app);
        r.app.audio_setup.popover = true;
        r.idle(SETTLE);
        match r.app.audio_setup.geo.rect(&Hit::InputField) {
            Some(f) => {
                r.click(f.center());
                r.shoot(&dir.join(format!("popover-input-{preset}.png")));
            }
            None => eprintln!("the popover's Input device field was not drawn"),
        }
        // Settings ▸ Audio's Output device list.
        let mut r = Rig::new(scale);
        r.look(preset, light);
        r.app.settings_page.audio.scan = Some(owner_scan());
        workspace_settings::open(&mut r.app, SettingsPage::Audio);
        r.idle(SETTLE);
        let words = cantus_ui::i18n::t("devices.output");
        match r.text(&words) {
            Some(label) => {
                // The field is the row's value, to the label's right.
                let at = Pos2::new(label.right() + 260.0, label.center().y);
                r.click(at);
                r.shoot(&dir.join(format!("settings-output-{preset}.png")));
            }
            None => eprintln!("“{words}” was not drawn"),
        }
        // The desk in each fader cap.
        let mut r = Rig::new(scale);
        r.look(preset, light);
        r.app
            .workspace
            .reveal(cantus_ui::views::workspace::Pane::Mixer);
        for c in cantus_ui::kit::fader::FaderCap::ALL {
            r.app.appearance.fader = c;
            r.app.apply_appearance();
            r.idle(6);
            r.shoot(&dir.join(format!("desk-faders-{c:?}-{preset}.png")));
        }
    }
}
