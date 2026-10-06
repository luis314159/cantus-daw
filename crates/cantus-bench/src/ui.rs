//! UI frame times for big sessions: the real application (`CantusApp::from_parts` around
//! a manual engine), driven frame by frame through `eframe::App::ui` the way eframe drives
//! it, with a 60-Hz block of audio before each frame so the transport plays and the
//! meters move — the harness of `crates/cantus-ui/tests/frame_budget.rs`, on the scaled
//! demo sessions. A frame's time is building it plus tessellating it (CPU); the GPU's
//! share is `research/ui/rendering-performance.md` §2.3's and is not measured here.

use crate::report::{Better, Metric};
use crate::stats::{median, quantile, Summary};
use crate::{sessions, Plan};
use cantus_core::Project;
use cantus_engine::{EngineHandle, Processor};
use cantus_ui::app::CentralView;
use cantus_ui::CantusApp;
use std::time::Instant;

struct Rig {
    app: CantusApp,
    processor: Processor,
    ctx: egui::Context,
    audio: Vec<f32>,
    frame: u64,
}

impl Rig {
    fn new(project: Project, view: CentralView) -> Rig {
        cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
        let (engine, processor) = EngineHandle::manual(48_000.0);
        let mut app = CantusApp::from_parts(Some(project), engine, false);
        app.central = view;
        app.sync_engine();
        if !app.engine.is_playing() {
            app.toggle_play();
        }
        Rig {
            app,
            processor,
            ctx,
            audio: vec![0.0; 800 * 2],
            frame: 0,
        }
    }

    /// One frame: build and tessellate, ms.
    fn step(&mut self) -> f64 {
        self.processor.process(&mut self.audio, 2);
        self.frame += 1;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1920.0, 1080.0),
            )),
            time: Some(self.frame as f64 / 60.0),
            predicted_dt: 1.0 / 60.0,
            ..Default::default()
        };
        let app = &mut self.app;
        let t0 = Instant::now();
        let mut out = self.ctx.run_ui(input, |root| {
            let mut frame = eframe::Frame::_new_kittest();
            eframe::App::ui(app, root, &mut frame);
        });
        let shapes = std::mem::take(&mut out.shapes);
        let prims = self.ctx.tessellate(shapes, out.pixels_per_point);
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        std::hint::black_box(prims.len());
        ms
    }
}

/// A scenario: its name, the project it opens, the view it shows.
type Case = (&'static str, Box<dyn Fn() -> Project>, CentralView);

/// The scenarios: the 64-track session in the mixer and in the piano roll (with the
/// arrangement), and the project the program opens on.
pub fn frames(plan: &Plan) -> Vec<Metric> {
    let mut out = Vec::new();
    let frames = if plan.quick { 60 } else { 200 };
    let cases: Vec<Case> = vec![
        (
            "s64.mixer",
            Box::new(|| sessions::scaled(64)),
            CentralView::Mixer,
        ),
        (
            "s64.pianoroll",
            Box::new(|| sessions::scaled(64)),
            CentralView::PianoRoll,
        ),
        (
            "startup.pianoroll",
            Box::new(cantus_ui::demos::startup_project),
            CentralView::PianoRoll,
        ),
    ];
    for (name, build, view) in cases {
        let (mut p50, mut p95) = (Vec::new(), Vec::new());
        for _ in 0..plan.runs {
            let mut rig = Rig::new(build(), view);
            for _ in 0..30 {
                rig.step();
            }
            let ms: Vec<f64> = (0..frames).map(|_| rig.step()).collect();
            p50.push(median(&ms));
            p95.push(quantile(&ms, 0.95));
        }
        out.push(Metric::new(
            format!("ui.{name}"),
            "ms/frame",
            Better::Lower,
            Summary::of(&p50),
        ));
        out.push(
            Metric::new(
                format!("ui.{name}.p95"),
                "ms/frame",
                Better::Lower,
                Summary::of(&p95),
            )
            .ungated(),
        );
    }
    out
}
