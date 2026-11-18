//! The window holds still once it has settled (GitHub issue #1, "GUI flickering": the whole
//! interface kept flickering along both axes on a fresh portable install, every start;
//! ADR 0146). Driven the way the desktop starts — `CantusApp::assemble`
//! with the files of a fresh data folder — and fed as egui-winit feeds the real window:
//! every size in points is the window's physical size divided by **the zoom in effect
//! before the pass** times the system's scale (`egui_winit::pixels_per_point` and
//! `update_viewport_info` read `Context::zoom_factor()` before `run`; a zoom set during a
//! pass becomes active at the start of the next one). The monitor's size included: that is
//! what the interface's screen-following scale (ADR 0111 §7) read as if it were in points
//! of the system's scale only, so the zoom it chose changed the width it measured, which
//! changed the zoom — 1.0 ↔ 1.25 every other frame on a 1 920-px screen at 100 %.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::CantusApp;
use egui::{pos2, vec2, Pos2, Rect, Vec2, ViewportId};

/// One screen and the window on it, in physical pixels — what winit reports.
#[derive(Clone, Copy, Debug)]
struct Screen {
    monitor: Vec2,
    native: f32,
    maximized: bool,
}

impl Screen {
    /// The window's inner size in physical pixels: the work area when maximised (the
    /// taskbar's 48 points off the bottom), else 1 280 × 800 points of the system's scale.
    fn window_px(&self) -> Vec2 {
        if self.maximized {
            vec2(self.monitor.x, self.monitor.y - 48.0 * self.native)
        } else {
            vec2(1280.0, 800.0) * self.native
        }
    }
}

/// What one frame produced: the scale it was drawn at, the commands it sent the window,
/// where its words were painted.
struct Drawn {
    ppp: f32,
    commands: Vec<egui::ViewportCommand>,
    texts: Vec<(String, Rect)>,
    bar: Option<Rect>,
}

struct Rig {
    ctx: egui::Context,
    app: CantusApp,
    frame: u64,
    _dir: tempfile::TempDir,
}

impl Rig {
    /// A fresh installation's files (the scale follows the screen), assembled as the boot
    /// does; `look` adjusts the settings before (the top bar's variants).
    fn new(look: impl FnOnce(&mut cantus_ui::settings::Settings)) -> Rig {
        cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", dir.path());
        let mut files = Files::read(dir.path(), Some(cantus_ui::i18n::Locale::En));
        assert!(
            files.settings.ui_scale_follows_screen,
            "a fresh installation follows the screen"
        );
        // The first-run audio setup stays shut: its list fills in when this machine's
        // device scan arrives, whenever that is — content arriving, not the window moving.
        files.settings.audio_setup.setup_done = true;
        look(&mut files.settings);
        let ctx = egui::Context::default();
        let app = CantusApp::assemble(
            &ctx,
            files,
            Some(cantus_ui::welcome_project()),
            EngineHandle::dummy(),
            None,
            None,
            None,
            false,
        );
        Rig {
            ctx,
            app,
            frame: 0,
            _dir: dir,
        }
    }

    /// The input egui-winit would give this frame (`take_egui_input` and
    /// `update_viewport_info`), from the zoom in effect before the pass.
    fn input(&self, s: Screen) -> egui::RawInput {
        let ppp = self.ctx.zoom_factor() * s.native;
        let win = s.window_px() / ppp;
        let mut raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, win)),
            time: Some(self.frame as f64 / 60.0),
            predicted_dt: 1.0 / 60.0,
            ..Default::default()
        };
        let vp = raw.viewports.entry(ViewportId::ROOT).or_default();
        vp.native_pixels_per_point = Some(s.native);
        vp.monitor_size = Some(s.monitor / ppp);
        let at = if s.maximized {
            Pos2::ZERO
        } else {
            pos2(200.0, 120.0)
        };
        vp.inner_rect = Some(Rect::from_min_size(at, win));
        vp.outer_rect = Some(Rect::from_min_size(at, win));
        vp.maximized = Some(s.maximized);
        vp.minimized = Some(false);
        vp.fullscreen = Some(false);
        vp.focused = Some(true);
        raw
    }

    fn step(&mut self, s: Screen) -> Drawn {
        self.frame += 1;
        let input = self.input(s);
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| app.frame(ui));
        let mut texts = Vec::new();
        for c in &out.shapes {
            if let egui::epaint::Shape::Text(t) = &c.shape {
                texts.push((t.galley.text().to_string(), t.visual_bounding_rect()));
            }
        }
        let commands = out
            .viewport_output
            .get(&ViewportId::ROOT)
            .map(|v| v.commands.clone())
            .unwrap_or_default();
        out.textures_delta.clear();
        let bar = cantus_ui::views::transport::last_layout(&self.ctx).map(|(r, _)| r);
        Drawn {
            ppp: out.pixels_per_point,
            commands,
            texts,
            bar,
        }
    }
}

/// The screens a laptop or a desk is likely to have, at every scale Windows offers up
/// to 200 %, maximised and restored.
fn screens() -> Vec<Screen> {
    let mut v = Vec::new();
    for monitor in [
        vec2(1920.0, 1080.0),
        vec2(1920.0, 1200.0),
        vec2(2560.0, 1600.0),
        vec2(2880.0, 1800.0),
        vec2(3840.0, 2160.0),
    ] {
        for native in [1.0, 1.25, 1.5, 1.75, 2.0] {
            for maximized in [true, false] {
                v.push(Screen {
                    monitor,
                    native,
                    maximized,
                });
            }
        }
    }
    v
}

/// Frames a screen gets to settle in, and the frames after that must all be alike.
const SETTLE: usize = 24;
const STILL: usize = 12;

/// Every screen in turn (as a window carried from one monitor to the next): once settled,
/// no frame sends the window a command, the scale holds, and every word painted in two
/// consecutive frames is painted at the same place.
fn holds_still(rig: &mut Rig, label: &str) {
    let mut failures = Vec::new();
    for s in screens() {
        let mut ppps = Vec::new();
        for _ in 0..SETTLE {
            ppps.push(rig.step(s).ppp);
        }
        let mut last = rig.step(s);
        for i in 0..STILL {
            let now = rig.step(s);
            let mut why = Vec::new();
            if now.ppp != last.ppp {
                why.push(format!("scale {} -> {}", last.ppp, now.ppp));
            }
            if !now.commands.is_empty() {
                why.push(format!("commands {:?}", now.commands));
            }
            if now.bar != last.bar {
                why.push(format!("top bar {:?} -> {:?}", last.bar, now.bar));
            }
            let moved = now
                .texts
                .iter()
                .filter(|(t, r)| {
                    // Empty galleys (a caret's, a field's placeholder) carry no words.
                    !t.trim().is_empty()
                        && last.texts.iter().any(|(t0, _)| t0 == t)
                        && !last.texts.iter().any(|(t0, r0)| t0 == t && r0 == r)
                })
                .map(|(t, _)| t.clone())
                .take(4)
                .collect::<Vec<_>>();
            if !moved.is_empty() {
                why.push(format!("moved {moved:?}"));
            }
            if !why.is_empty() {
                failures.push(format!(
                    "{label} {s:?}, still frame {i}: {} (scales while settling: {ppps:?})",
                    why.join("; ")
                ));
                break;
            }
            last = now;
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} screens never held still:\n{}",
        failures.len(),
        screens().len(),
        failures.join("\n")
    );
}

#[test]
fn a_fresh_install_holds_still_on_every_screen_and_scale() {
    let mut rig = Rig::new(|_| {});
    holds_still(&mut rig, "default bar");
    // The top bar's variants (round 40, the owner's look answers): the automation arm and
    // the typing keyboard's key on the bar.
    let mut rig = Rig::new(|s| {
        s.look.topbar_automation = true;
        s.look.topbar_typing = true;
    });
    holds_still(&mut rig, "bar with automation and typing");
}

/// The scale a screen settles on is the rule's (ADR 0111 §7), not the zoom it happened to
/// be measured at: 125 % on a 1 920-px screen at the system's 100 %, 100 % at its 125 %.
#[test]
fn the_followed_scale_is_the_rules_whatever_zoom_it_was_measured_at() {
    let mut rig = Rig::new(|_| {});
    for (monitor, native, want) in [
        (vec2(1920.0, 1080.0), 1.0, 1.25),
        (vec2(1920.0, 1080.0), 1.25, 1.0),
        (vec2(2560.0, 1440.0), 1.0, 1.75),
        (vec2(2880.0, 1800.0), 2.0, 0.9),
        (vec2(3840.0, 2160.0), 1.5, 1.25),
    ] {
        let s = Screen {
            monitor,
            native,
            maximized: true,
        };
        for _ in 0..SETTLE {
            rig.step(s);
        }
        let ppp = rig.step(s).ppp;
        assert_eq!(
            rig.ctx.zoom_factor(),
            want,
            "{monitor:?} at {native}: drawn at {ppp}"
        );
        assert!((ppp - want * native).abs() < 1e-4);
    }
}

/// `--scale` (and `--safe-graphics`, issue #1): the scale asked for is set through the
/// boot's `apply_flags`, the screen is not followed after it, the settings keep their own.
#[test]
fn a_pinned_scale_holds_and_the_screen_is_not_followed() {
    let mut rig = Rig::new(|_| {});
    let mut o = cantus_ui::LaunchOptions::default();
    o.graphics.scale = Some(1.5);
    o.apply_flags(&mut rig.app);
    let s = Screen {
        monitor: vec2(1920.0, 1080.0),
        native: 1.0,
        maximized: true,
    };
    let mut ppps = Vec::new();
    for _ in 0..SETTLE {
        ppps.push(rig.step(s).ppp);
    }
    assert!(ppps[4..].iter().all(|p| *p == 1.5), "{ppps:?}");
    assert_eq!(rig.app.settings.ui_scale, 1.0, "never the settings'");
    // Carried to another screen: still the pinned scale.
    let other = Screen {
        monitor: vec2(3840.0, 2160.0),
        native: 1.5,
        maximized: true,
    };
    for _ in 0..SETTLE {
        rig.step(other);
    }
    assert_eq!(rig.ctx.zoom_factor(), 1.5);
    let mut safe = cantus_ui::LaunchOptions::default();
    safe.graphics.safe = true;
    assert_eq!(safe.graphics.scale(), Some(1.0));
}
