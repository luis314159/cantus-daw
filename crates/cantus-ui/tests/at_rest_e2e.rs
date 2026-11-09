//! Cantus at rest, end to end (round 38, the scorecard's *at rest* row): the app
//! `CantusApp::assemble` puts together on the demo it opens with, on a real processor driven
//! block by block, left alone with the transport stopped.
//!
//! - Its desk rests: every strip with inserts and the master chain stop running over the
//!   silence (ADR 0031 §5.1, generalised from the pad chains of ADR 0100) — and resting
//!   changes no sample: a twin that never rests plays the same bits through silence, play,
//!   stop, the tails dying away and play again.
//! - Its dancer holds still once nothing has happened for a while (`dance_live::Doze`): the
//!   window then asks for no frame at all, and the next thing that happens wakes it.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::Project;
use cantus_engine::{EngineHandle, Processor};
use cantus_ui::app::Files;
use cantus_ui::views::workspace_ticker as ticker;
use cantus_ui::CantusApp;
use std::path::Path;

const RATE: f64 = 48_000.0;
/// One frame of the window, in samples: 60 frames a second.
const FRAME: usize = 800;

fn data() -> &'static Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        // A configured copy: no first-run audio setup (as the scorecard starts it).
        std::fs::write(
            d.path().join("settings.json"),
            r#"{"audio_setup":{"setup_done":true}}"#,
        )
        .unwrap();
        d
    })
    .path()
}

struct Rig {
    app: CantusApp,
    proc_: Processor,
    ctx: egui::Context,
    t: f64,
    out: Vec<f32>,
}

fn assembled(project: Project) -> Rig {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let (engine, proc_) = EngineHandle::manual(RATE);
    let app = CantusApp::assemble(&ctx, files, Some(project), engine, None, None, None, false);
    let mut r = Rig {
        app,
        proc_,
        ctx,
        t: 0.0,
        out: Vec::new(),
    };
    // The demo's samples in before anything is heard, so two rigs hear the same.
    let since = std::time::Instant::now();
    while r.app.engine.samples.loading() && since.elapsed().as_secs() < 60 {
        r.frame(None);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    r.out.clear();
    r
}

impl Rig {
    /// One block of audio and one frame, with the pointer at `pointer` if given.
    fn frame(&mut self, pointer: Option<egui::Pos2>) {
        let mut buf = vec![0f32; FRAME * 2];
        self.proc_.process(&mut buf, 2);
        self.out.extend_from_slice(&buf);
        self.t += 1.0 / 60.0;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(1600.0, 900.0),
            )),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events: pointer.map(egui::Event::PointerMoved).into_iter().collect(),
            ..Default::default()
        };
        let app = &mut self.app;
        self.ctx
            .run_ui(input, |ui| app.frame(ui))
            .textures_delta
            .clear();
    }

    /// `seconds` of the app left alone.
    fn idle(&mut self, seconds: f64) {
        for _ in 0..(seconds * 60.0).round() as usize {
            self.frame(None);
        }
    }
}

/// Strips that carry inserts in the project, and whether the master has any.
fn chains(p: &Project) -> (u64, bool) {
    let strips = p.strips.iter().filter(|s| !s.effects.is_empty()).count() as u64;
    (strips, !p.master.effects.is_empty())
}

#[test]
fn the_demos_desk_rests_in_silence() {
    let demo = cantus_ui::demos::startup_project();
    let (strips, master) = chains(&demo);
    assert!(
        strips >= 8 && master,
        "the demo has a desk: {strips} chains"
    );
    let mut r = assembled(demo);
    r.idle(1.0);
    let before = *r.proc_.stats();
    r.idle(1.0);
    let after = *r.proc_.stats();
    let passes = after.blocks_processed - before.blocks_processed;
    let names = |id: cantus_core::StripId| {
        r.app
            .project()
            .strip(id)
            .map_or("master".to_string(), |s| s.name.clone())
    };
    let blockers: Vec<String> = r
        .proc_
        .rest_blockers()
        .into_iter()
        .map(|(s, i, why)| format!("{} insert {i}: {why}", names(s)))
        .collect();
    assert!(blockers.is_empty(), "inserts not at rest: {blockers:?}");
    let rests = after.strip_rests - before.strip_rests;
    assert_eq!(
        rests,
        passes * strips,
        "every chain rests every pass while nothing plays: {rests} rests in {passes} passes \
         of {strips} chains"
    );
    assert_eq!(after.master_rests - before.master_rests, passes);
}

#[test]
fn resting_changes_no_sample_of_the_demo() {
    let run = |never_rest: bool| {
        let mut r = assembled(cantus_ui::demos::startup_project());
        r.proc_.set_never_rest(never_rest);
        r.idle(0.5);
        r.app.engine.play();
        r.idle(3.0);
        r.app.engine.stop();
        // The tails die away and the desk comes to rest again…
        r.idle(6.0);
        let rested = r.proc_.stats().strip_rests;
        // …and wakes for the song.
        r.app.engine.play();
        r.idle(2.0);
        (r.out, rested, r.proc_.stats().master_rests)
    };
    let (resting, rests, master_rests) = run(false);
    let (running, none, no_master) = run(true);
    assert!(rests > 0 && master_rests > 0, "the desk rested");
    assert_eq!((none, no_master), (0, 0), "the twin never rested");
    assert_eq!(resting.len(), running.len());
    assert!(resting.iter().any(|x| *x != 0.0), "the demo sounded");
    if let Some(i) = resting
        .iter()
        .zip(&running)
        .position(|(a, b)| a.to_bits() != b.to_bits())
    {
        panic!(
            "sample {i} ({:.3} s) differs: {:e} resting, {:e} running",
            i as f64 / 2.0 / RATE,
            resting[i],
            running[i]
        );
    }
}

#[test]
fn the_demos_dancer_holds_still_and_the_window_asks_for_nothing() {
    // The excerpt with the song's dancer, as a user who added one has it.
    let mut r = assembled(cantus_ui::demos::startup_project_with_dancer());
    r.app.dance_live.doze.after = 0.3;
    r.frame(Some(egui::pos2(400.0, 300.0)));
    r.idle(0.2);
    assert!(
        !r.app.dance_live.doze.holding(),
        "it breathes while things happen"
    );
    let breath = std::time::Duration::from_secs_f64(1.0 / cantus_ui::stage::IDLE_RATE);
    assert!(
        ticker::last(&r.ctx)
            .requested
            .is_some_and(|d| d <= breath + std::time::Duration::from_millis(2)),
        "the breath asks for its next frame: {:?}",
        ticker::last(&r.ctx)
    );
    // Nothing happens: it holds still, and nothing is asked for soon.
    std::thread::sleep(std::time::Duration::from_millis(400));
    r.idle(0.5);
    assert!(r.app.dance_live.doze.holding(), "it holds still");
    let redraws = r.app.stage.redraws;
    r.idle(0.5);
    assert_eq!(
        r.app.stage.redraws, redraws,
        "a dancer holding still draws nothing"
    );
    let quiet =
        |d: Option<std::time::Duration>| d.is_none_or(|d| d > std::time::Duration::from_secs(1));
    assert!(
        quiet(ticker::last(&r.ctx).requested),
        "at rest the window asks for no frame: {:?}",
        ticker::last(&r.ctx)
    );
    // The pointer over the window wakes it, from the pose it held.
    let held = r.app.dance_live.wall();
    r.frame(Some(egui::pos2(420.0, 310.0)));
    assert!(!r.app.dance_live.doze.holding(), "the pointer wakes it");
    assert!(
        (r.app.dance_live.wall() - held).abs() < 0.1,
        "it goes on from the pose it held"
    );
    r.idle(0.2);
    assert!(ticker::last(&r.ctx)
        .requested
        .is_some_and(|d| d <= breath + std::time::Duration::from_millis(2)));
    // A hover on the dancer wakes it too, and once the pointer has left and the frame has
    // faded it holds still again: a hover is not "something happening" for ever.
    let dancer = r
        .app
        .project()
        .tracks
        .iter()
        .find(|t| {
            t.instrument
                .native()
                .is_some_and(|n| n.kind == cantus_core::InstrumentKind::Dancer)
        })
        .expect("the demo's dancer")
        .id;
    let body = r
        .app
        .stage
        .body(dancer, cantus_ui::stage::Surface::Float)
        .expect("drawn over the workspace");
    r.frame(Some(body.center()));
    r.frame(Some(body.center()));
    r.frame(Some(egui::pos2(3.0, 3.0)));
    // The frame's fade (egui's clock) and the breath's while (the wall clock) both pass.
    r.idle(1.0);
    std::thread::sleep(std::time::Duration::from_millis(400));
    r.idle(0.3);
    assert!(
        r.app.dance_live.doze.holding(),
        "after a hover it holds still again"
    );
}

/// The owner's decision (2026-11-08): nothing dances at start. What the boot opens when it is
/// given no project — the demo — through `CantusApp::assemble`: no dancer track, no dancer
/// drawn, no texture, and once the start's own frames are drawn the window asks for none.
#[test]
fn a_fresh_start_shows_no_dancer_and_spends_no_frames_on_one() {
    let mut r = assembled(cantus_ui::demos::startup_project());
    assert!(
        !r.app.project().tracks.iter().any(|t| t
            .instrument
            .native()
            .is_some_and(|n| n.kind == cantus_core::InstrumentKind::Dancer)),
        "the song Cantus opens on has no dancer"
    );
    assert!(!r.app.settings.stage.companion_open, "no companion window");
    r.idle(1.0);
    assert!(
        cantus_ui::dance_live::DanceLive::dancers(&r.app).is_empty(),
        "nobody dances"
    );
    assert_eq!(r.app.stage.textures(), 0, "no dancer drawn");
    assert_eq!(r.app.stage.redraws, 0, "no frame spent on a dancer");
    let quiet =
        |d: Option<std::time::Duration>| d.is_none_or(|d| d > std::time::Duration::from_secs(1));
    assert!(
        quiet(ticker::last(&r.ctx).requested),
        "a fresh start at rest asks for no frame: {:?}",
        ticker::last(&r.ctx)
    );
}

/// Round 38's review: a dancer played live from a MIDI keyboard while the transport is
/// stopped is something happening. Once it holds still, a note on its track (no pointer, no
/// key on the computer's keyboard) wakes it, and the breath asks for its frames again.
#[test]
fn a_note_played_live_wakes_a_dancer_that_holds_still() {
    let mut r = assembled(cantus_ui::demos::startup_project_with_dancer());
    r.app.dance_live.doze.after = 0.3;
    r.frame(Some(egui::pos2(400.0, 300.0)));
    std::thread::sleep(std::time::Duration::from_millis(400));
    r.idle(0.5);
    assert!(r.app.dance_live.doze.holding(), "it holds still");
    let dancer = r
        .app
        .project()
        .tracks
        .iter()
        .find(|t| {
            t.instrument
                .native()
                .is_some_and(|n| n.kind == cantus_core::InstrumentKind::Dancer)
        })
        .expect("the excerpt's dancer")
        .id;
    r.app
        .engine
        .note_on(dancer, cantus_core::inst::dancer::BASE_KEY + 2, 100);
    r.idle(0.1);
    assert!(
        !r.app.dance_live.doze.holding(),
        "a note on its track wakes it"
    );
    let breath = std::time::Duration::from_secs_f64(1.0 / cantus_ui::stage::IDLE_RATE);
    assert!(
        ticker::last(&r.ctx)
            .requested
            .is_some_and(|d| d <= breath + std::time::Duration::from_millis(2)),
        "awake, it asks for its next frame: {:?}",
        ticker::last(&r.ctx)
    );
    // Let go and left alone, it holds still again.
    r.app
        .engine
        .note_off(dancer, cantus_core::inst::dancer::BASE_KEY + 2);
    r.idle(0.2);
    std::thread::sleep(std::time::Duration::from_millis(400));
    r.idle(0.5);
    assert!(
        r.app.dance_live.doze.holding(),
        "the note over, it holds still again"
    );
}

/// Round 38's review: a knob turned on a resting desk. Every strip's first insert has its
/// first continuous parameter moved through `CantusApp::exec` while the transport is stopped
/// and the desk rests; then the song plays. A twin that never rests hears the same bits — a
/// parameter's glide is not lost, nor started late, by resting.
#[test]
fn a_knob_turned_while_the_desk_rests_changes_nothing_heard() {
    let run = |never_rest: bool| {
        let mut r = assembled(cantus_ui::demos::startup_project());
        r.proc_.set_never_rest(never_rest);
        r.idle(1.0);
        let rests_before = r.proc_.stats().strip_rests;
        let turns: Vec<cantus_core::Command> = r
            .app
            .project()
            .strips
            .iter()
            .filter_map(|s| {
                let slot = s.effects.first()?;
                let info = slot
                    .spec
                    .kind
                    .params()
                    .iter()
                    .find(|p| !p.stepped && p.max > p.min)?;
                Some(cantus_core::Command::SetEffectParam {
                    strip: s.id,
                    effect: slot.id,
                    key: info.key.to_string(),
                    value: info.min + (info.max - info.min) * 0.37,
                })
            })
            .collect();
        assert!(turns.len() >= 4, "knobs to turn: {}", turns.len());
        for cmd in turns {
            assert!(r.app.exec(cmd), "the knob turns");
        }
        r.idle(1.0);
        let rested = r.proc_.stats().strip_rests - rests_before;
        r.app.engine.play();
        r.idle(2.0);
        (r.out, rested)
    };
    let (resting, rests) = run(false);
    let (running, none) = run(true);
    assert!(rests > 0, "the desk rested around the turns");
    assert_eq!(none, 0, "the twin never rested");
    assert_eq!(resting.len(), running.len());
    if let Some(i) = resting
        .iter()
        .zip(&running)
        .position(|(a, b)| a.to_bits() != b.to_bits())
    {
        panic!(
            "sample {i} ({:.3} s) differs: {:e} resting, {:e} running",
            i as f64 / 2.0 / RATE,
            resting[i],
            running[i]
        );
    }
}
