//! Speculum in the real window (ADR 0056, ADR 0024 §13): three analysers at once — one
//! floating, two docked as tabs of the workspace — on a playing engine, frame after frame
//! at 60 Hz.
//!
//! **Deterministic, every `cargo test`**: all three are watched by the engine (three scope
//! rings), and **no frame is dropped** — each analyser measures exactly the frames the
//! engine played between two points, 800 a frame; triangles and draw calls stay bounded.
//!
//! **Timing** (`#[ignore]`; `cargo test --release -p cantus-ui --test speculum_budget --
//! --ignored --nocapture`): build + tessellate of the window with the three, median and
//! 95th percentile over 200 frames, within the 16.7 ms of a 60-Hz frame (the view's own
//! budget, ADR 0024 §13, is 8 ms — the median is asserted against it).

use cantus_core::{EffectId, EffectKind, Project, StripId};
use cantus_engine::{EngineHandle, Processor};
use cantus_ui::views::fx::speculum;
use cantus_ui::{welcome_project, CantusApp};
use std::time::{Duration, Instant};

const FRAME: usize = 800;

struct Rig {
    app: CantusApp,
    processor: Processor,
    ctx: egui::Context,
    audio: Vec<f32>,
    frame: u64,
}

/// Measurements of one frame.
struct Frame {
    time: Duration,
    triangles: usize,
    draws: usize,
}

impl Rig {
    fn new(project: Project) -> Rig {
        cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        cantus_ui::theme::apply(&cantus_ui::theme_family::cantus_night(), &ctx);
        let (engine, processor) = EngineHandle::manual(48_000.0);
        let mut app = CantusApp::from_parts(Some(project), engine, false);
        app.sync_engine();
        Rig {
            app,
            processor,
            ctx,
            audio: vec![0.0; FRAME * 2],
            frame: 0,
        }
    }

    fn step(&mut self) -> Frame {
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
        let time = t0.elapsed();
        out.textures_delta.clear();
        let triangles = prims
            .iter()
            .map(|p| match &p.primitive {
                egui::epaint::Primitive::Mesh(m) => m.indices.len() / 3,
                _ => 0,
            })
            .sum();
        Frame {
            time,
            triangles,
            draws: prims.len(),
        }
    }
}

/// The welcome project with three Speculums on the master chain (where they always run:
/// the strip path is checked frame for frame in the engine's `fx_speculum`): the first
/// floats (overview), the other two are docked (spectrum, loudness).
fn three() -> (Rig, [(StripId, EffectId); 3]) {
    let mut p = welcome_project();
    let mut ids = Vec::new();
    let master = p.make_effect(EffectKind::Analyzer);
    ids.push((StripId::MASTER, master.id));
    p.master.effects.push(master);
    for i in 0..2 {
        let mut slot = p.make_effect(EffectKind::Analyzer);
        slot.spec.set("view", if i == 0 { 1.0 } else { 5.0 });
        // The master's own: it always runs, so the frames counted are every frame played.
        ids.push((StripId::MASTER, slot.id));
        p.master.effects.push(slot);
    }
    let mut rig = Rig::new(p);
    let (s, fx) = ids[0];
    rig.app.open_effect_editor(s, fx);
    for &(s, fx) in &ids[1..] {
        speculum::dock(&mut rig.app, s, fx);
    }
    if !rig.app.engine.is_playing() {
        rig.app.toggle_play();
    }
    (rig, [ids[0], ids[1], ids[2]])
}

/// Three analysers at once, every frame measured by each: none lost at 60 frames a second.
#[test]
fn three_analysers_at_once_lose_no_frame() {
    let (mut rig, ids) = three();
    for _ in 0..30 {
        rig.step();
    }
    assert_eq!(rig.app.engine.watched_scopes().count(), 3, "three rings");
    let before: Vec<u64> = ids
        .iter()
        .map(|(_, fx)| speculum::measured(&rig.app, *fx).expect("shown"))
        .collect();
    let n = 60;
    let mut worst = (0usize, 0usize);
    for _ in 0..n {
        let f = rig.step();
        worst = (worst.0.max(f.triangles), worst.1.max(f.draws));
    }
    for (i, (_, fx)) in ids.iter().enumerate() {
        let after = speculum::measured(&rig.app, *fx).unwrap();
        // Every frame played, once — but for the chunk still being filled at each end
        // (fewer than 64 frames).
        let got = (after - before[i]) as i64;
        assert!(
            (got - (n * FRAME) as i64).abs() < 64,
            "analyser {i}: {got} frames measured of {}",
            n * FRAME
        );
    }
    // What the three add to the same window without them.
    let mut bare = Rig::new(welcome_project());
    bare.app.toggle_play();
    let mut base = (0usize, 0usize);
    for _ in 0..40 {
        let f = bare.step();
        base = (base.0.max(f.triangles), base.1.max(f.draws));
    }
    eprintln!(
        "three analysers: {} triangles, {} draw calls (worst); the window without them: {}, {}",
        worst.0, worst.1, base.0, base.1
    );
    assert!(
        worst.0 <= base.0 + 60_000,
        "{} triangles over {}",
        worst.0,
        base.0
    );
    assert!(
        worst.1 <= base.1 + 40,
        "{} draw calls over {}",
        worst.1,
        base.1
    );
}

/// The frame budget, timed (release).
#[test]
#[ignore]
fn three_analysers_draw_within_the_frame_budget() {
    let (mut rig, _) = three();
    for _ in 0..60 {
        rig.step();
    }
    let mut times: Vec<Duration> = (0..200).map(|_| rig.step().time).collect();
    times.sort_unstable();
    let median = times[times.len() / 2];
    let p95 = times[times.len() * 95 / 100];
    eprintln!("the window with three analysers: median {median:?}, 95th percentile {p95:?}");
    let budget = if cfg!(debug_assertions) { 120.0 } else { 8.0 };
    assert!(median.as_secs_f64() * 1e3 < budget, "median {median:?}");
    assert!(
        p95.as_secs_f64() * 1e3 < 2.0 * budget.max(8.0),
        "p95 {p95:?}"
    );
}
