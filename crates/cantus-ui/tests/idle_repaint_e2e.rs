//! An idle window asks egui for no frame (round 39, r39:lo-rest; ADR 0106 §6): through
//! `CantusApp::assemble` on the demo song the program opens with, driven as eframe drives it
//! (`eframe::App::ui`), the transport stopped and nobody touching it. Once the start-up
//! settles, the window's own `repaint_delay` — what egui will wait, whoever asked — stays
//! `Duration::MAX` frame after frame: not only the ticker's reasons, every caller of
//! `request_repaint` (round 39's el-cables found the window drawing every frame at rest).
//!
//! Held at 1920 × 1080 with the default layout, at 125 %, and with the mixer's cables shown
//! (Flow's gleam moves only while something plays).
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_engine::{EngineHandle, Processor};
use cantus_ui::app::{CentralView, Files};
use cantus_ui::views::mixer::layout::CableView;
use cantus_ui::CantusApp;
use egui::{vec2, Pos2, Rect};
use std::path::Path;
use std::time::Duration;

struct Rig {
    app: CantusApp,
    ctx: egui::Context,
    proc_: Processor,
    t: f64,
    /// Events for the next frame only (a pointer moved, a key), as a window sends them.
    events: Vec<egui::Event>,
    /// The window's size in physical pixels and its scale.
    size: egui::Vec2,
    ppp: f32,
}

fn boot(dir: &Path, size: egui::Vec2, ppp: f32) -> Rig {
    std::fs::write(
        dir.join("settings.json"),
        r#"{"audio_setup":{"setup_done":true}}"#,
    )
    .unwrap();
    std::env::set_var("CANTUS_DATA_DIR", dir);
    let files = Files::read(dir, Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let (engine, proc_) = EngineHandle::manual(48_000.0);
    let app = CantusApp::assemble(
        &ctx,
        files,
        Some(cantus_ui::demos::startup_project()),
        engine,
        None,
        None,
        None,
        false,
    );
    let mut r = Rig {
        app,
        ctx,
        proc_,
        t: 0.0,
        events: Vec::new(),
        size,
        ppp,
    };
    // The demo's samples in, as a user waits for the window to come up.
    let since = std::time::Instant::now();
    while r.app.engine.samples.loading() && since.elapsed().as_secs() < 60 {
        r.frame();
        std::thread::sleep(Duration::from_millis(5));
    }
    r
}

impl Rig {
    /// One frame as eframe runs it; what egui will wait before the next, and who asked.
    fn frame(&mut self) -> (Duration, Vec<String>) {
        self.t += 1.0 / 60.0;
        let mut audio = vec![0f32; 800 * 2];
        self.proc_.process(&mut audio, 2);
        let mut viewports = egui::ViewportIdMap::default();
        viewports.insert(
            egui::ViewportId::ROOT,
            egui::ViewportInfo {
                native_pixels_per_point: Some(self.ppp),
                ..Default::default()
            },
        );
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size / self.ppp)),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            viewports,
            events: std::mem::take(&mut self.events),
            ..Default::default()
        };
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| {
            let mut frame = eframe::Frame::_new_kittest();
            eframe::App::ui(app, ui, &mut frame);
        });
        out.textures_delta.clear();
        let delay = out
            .viewport_output
            .values()
            .map(|v| v.repaint_delay)
            .min()
            .unwrap_or(Duration::MAX);
        let causes = self
            .ctx
            .repaint_causes()
            .iter()
            .map(|c| c.to_string())
            .collect();
        (delay, causes)
    }

    /// Let the start-up settle (fonts, first layouts, fades, the verse's greeting), then
    /// `n` frames that must each ask for nothing soon. A far deadline (the autosave's, a
    /// clock's minute) is not a frame.
    fn assert_quiet(&mut self, what: &str, n: usize) {
        for _ in 0..240 {
            self.frame();
        }
        self.app.bus.mark_saved();
        let mut asked = Vec::new();
        for i in 0..n {
            let (delay, causes) = self.frame();
            if delay < Duration::from_secs(1) {
                asked.push(format!("frame {i}: {delay:?} at {causes:?}"));
            }
        }
        assert!(
            asked.is_empty(),
            "{what}: an idle window asked for {} of {n} frames:\n{}",
            asked.len(),
            asked.iter().take(8).cloned().collect::<Vec<_>>().join("\n")
        );
    }
}

#[test]
fn an_idle_window_asks_egui_for_no_frame() {
    let dir = tempfile::tempdir().unwrap();
    // The default layout at 1920 × 1080, at 100 % and at 125 %.
    let mut r = boot(dir.path(), vec2(1920.0, 1080.0), 1.0);
    r.assert_quiet("1920 × 1080 at 100 %", 120);
    r.ppp = 1.25;
    r.assert_quiet("1920 × 1080 at 125 %", 120);
    // The pointer comes to rest over the arrangement: once its hover has been drawn (a
    // tooltip's wait included), nothing more.
    r.events
        .push(egui::Event::PointerMoved(egui::pos2(700.0, 300.0)));
    r.assert_quiet("the pointer resting over the arrangement", 120);
    r.events.push(egui::Event::PointerGone);
    // The mixer in the middle with every cable drawn: the gleam waits for the transport.
    r.app.show_central(CentralView::Mixer);
    let mut p = r.app.mixer.prefs();
    p.cables = CableView::All;
    r.app.mixer.set_prefs(p);
    r.assert_quiet("the mixer with its cables", 120);
    // Each editor in front, the browser and its packs.
    r.app.show_central(CentralView::PianoRoll);
    r.assert_quiet("the piano roll", 60);
    r.app.show_central(CentralView::ChannelRack);
    r.assert_quiet("the channel rack", 60);
    r.app
        .toggle_pane(cantus_ui::views::workspace::Pane::Browser);
    r.assert_quiet("the browser", 60);
    r.app.samples.window.open = true;
    r.assert_quiet("the sound packs", 60);
    r.app.samples.window.open = false;
    // Playing asks for frames (the rig sees a request when there is one).
    r.app.toggle_play();
    let (delay, _) = r.frame();
    let (delay2, _) = r.frame();
    r.app.stop();
    assert!(
        delay.min(delay2) < Duration::from_secs(1),
        "a playing window asks for its frames"
    );
}

/// What round 39's el-cables saw — the window asking for a frame every frame at rest —
/// was its rig's: it sent `ModifiersChanged` with every frame, and egui draws again after
/// any input event. A window sends it only when a modifier changes.
#[test]
fn an_input_event_every_frame_is_what_asked_for_frames() {
    let dir = tempfile::tempdir().unwrap();
    let mut r = boot(dir.path(), vec2(1920.0, 1080.0), 1.25);
    r.assert_quiet("at rest", 30);
    let mut asked = 0;
    for _ in 0..30 {
        r.events
            .push(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
        let (delay, _) = r.frame();
        asked += usize::from(delay == Duration::ZERO);
    }
    assert_eq!(asked, 30, "an event each frame: a frame after each");
    r.assert_quiet("the events stop", 30);
}
