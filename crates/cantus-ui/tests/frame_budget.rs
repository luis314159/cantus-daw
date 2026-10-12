//! The frame budget (ADR 0024 §13, `research/ui/rendering-performance.md` §8.2), held
//! by a test.
//!
//! **Harness.** The real application — `CantusApp::from_parts` around
//! `EngineHandle::manual` — with its `Processor` run for one 60-Hz block of audio before
//! every UI frame, so the transport plays and the meters move. Frames are driven through
//! `eframe::App::ui`, the whole window, exactly as eframe drives it; the mixer alone is
//! measured too, inside a central panel, because the rest of the window belongs to other
//! views.
//!
//! **Deterministic assertions**, run by every `cargo test`: heap allocations and bytes a
//! frame (a counting `#[global_allocator]`, counting only the thread under measurement),
//! the strips the mixer laid out, triangles and draw calls after tessellation, and that an
//! idle desk asks for no repaint. They do not depend on the machine.
//!
//! **Timing** (`#[ignore]`; `cargo test --release -p cantus-ui --test frame_budget --
//! --ignored --nocapture`): the median and 95th percentile of build + tessellate over 200
//! frames, asserted at twice the budget — a ceiling for CI noise, not the target.

use cantus_core::{Color, Project, Send, Ticks};
use cantus_engine::{EngineHandle, Processor};
use cantus_ui::{views, welcome_project, CantusApp};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::time::{Duration, Instant};

// ---- a counting allocator --------------------------------------------------------------

struct Counting;

thread_local! {
    static ON: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<u64> = const { Cell::new(0) };
    static BYTES: Cell<u64> = const { Cell::new(0) };
}

fn note(size: usize) {
    let _ = ON.try_with(|on| {
        if on.get() {
            let _ = COUNT.try_with(|c| c.set(c.get() + 1));
            let _ = BYTES.try_with(|b| b.set(b.get() + size as u64));
        }
    });
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note(new_size);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Run `f` and return what it allocated on this thread: (allocations, bytes).
fn counted<R>(f: impl FnOnce() -> R) -> (R, u64, u64) {
    COUNT.with(|c| c.set(0));
    BYTES.with(|b| b.set(0));
    ON.with(|on| on.set(true));
    let r = f();
    ON.with(|on| on.set(false));
    (r, COUNT.with(Cell::get), BYTES.with(Cell::get))
}

// ---- the scenarios ---------------------------------------------------------------------

/// The welcome project grown to `n` strips, each fed by a playing track with a 16-note
/// clip (`rendering-performance.md` §2's "mixer 64"); a send and a muted strip besides.
fn grown(n: usize) -> Project {
    let mut p = welcome_project();
    while p.strips.len() < n {
        let s = p.make_strip("");
        p.strips.push(s);
    }
    let mut i = 0usize;
    while p.tracks.len() < n.min(p.strips.len()) {
        i += 1;
        let name = format!("Track {i}");
        let mut t = p.make_track(name.clone());
        t.color = Color::PALETTE[(i * 5) % Color::PALETTE.len()];
        let mut clip = p.make_clip(name, Ticks::ZERO, Ticks::beats(16));
        for k in 0..16 {
            let pitch = 48 + ((k * 7 + i * 3) % 24) as u8;
            let note = p.make_note(Ticks::beats(k as i64), Ticks::QUARTER, pitch, 96);
            clip.notes.push(note);
        }
        t.clips.push(clip);
        if t.strip.is_none() {
            break;
        }
        p.tracks.push(t);
    }
    p.strips[4].muted = true;
    let to = p.strips[p.strips.len() - 2].id;
    p.strips[1].sends.push(Send::new(to));
    p
}

/// What a scenario needs between frames.
struct Rig {
    app: CantusApp,
    processor: Processor,
    ctx: egui::Context,
    audio: Vec<f32>,
    frame: u64,
    size: egui::Vec2,
    /// Drive the whole window (`eframe::App::ui`), or the mixer alone in a central panel.
    whole: bool,
    /// Where the pointer rests (a hovered cable), if anywhere.
    pointer: Option<egui::Pos2>,
}

/// One frame's measurements.
#[derive(Copy, Clone, Debug, Default)]
struct Frame {
    allocs: u64,
    bytes: u64,
    build: Duration,
    tess: Duration,
    triangles: usize,
    draws: usize,
    strips: usize,
    repaint: Duration,
}

impl Rig {
    fn new(n: usize, whole: bool, size: egui::Vec2) -> Rig {
        Rig::with(grown(n), whole, size)
    }

    fn with(project: Project, whole: bool, size: egui::Vec2) -> Rig {
        cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
        let (engine, processor) = EngineHandle::manual(48_000.0);
        let mut app = CantusApp::from_parts(Some(project), engine, false);
        app.central = cantus_ui::app::CentralView::Mixer;
        let pick = app.project().strips[2].id;
        app.select_strip(pick);
        app.sync_engine();
        Rig {
            app,
            processor,
            ctx,
            audio: vec![0.0; 800 * 2],
            frame: 0,
            size,
            whole,
            pointer: None,
        }
    }

    fn play(&mut self) {
        if !self.app.engine.is_playing() {
            self.app.toggle_play();
        }
    }

    fn stop(&mut self) {
        if self.app.engine.is_playing() {
            self.app.toggle_play();
        }
    }

    /// One 60-Hz frame: a block of audio, then the UI, then tessellation.
    fn step(&mut self) -> Frame {
        self.processor.process(&mut self.audio, 2);
        self.frame += 1;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
            time: Some(self.frame as f64 / 60.0),
            predicted_dt: 1.0 / 60.0,
            events: self
                .pointer
                .map(egui::Event::PointerMoved)
                .into_iter()
                .collect(),
            ..Default::default()
        };
        let whole = self.whole;
        let app = &mut self.app;
        let ctx = self.ctx.clone();
        let t0 = Instant::now();
        let (out, allocs, bytes) = counted(|| {
            ctx.run_ui(input, |root| {
                if whole {
                    let mut frame = eframe::Frame::_new_kittest();
                    eframe::App::ui(app, root, &mut frame);
                } else {
                    egui::CentralPanel::default().show(root, |ui| views::mixer::show(app, ui));
                }
            })
        });
        let t1 = Instant::now();
        let mut out = out;
        let repaint = out
            .viewport_output
            .values()
            .map(|v| v.repaint_delay)
            .min()
            .unwrap_or(Duration::MAX);
        let shapes = std::mem::take(&mut out.shapes);
        let prims = self.ctx.tessellate(shapes, out.pixels_per_point);
        let t2 = Instant::now();
        out.textures_delta.clear();
        let triangles = prims
            .iter()
            .map(|p| match &p.primitive {
                egui::epaint::Primitive::Mesh(m) => m.indices.len() / 3,
                _ => 0,
            })
            .sum();
        Frame {
            allocs,
            bytes,
            build: t1 - t0,
            tess: t2 - t1,
            triangles,
            draws: prims.len(),
            strips: strips_laid_out(&self.app),
            repaint,
        }
    }

    /// `warm` frames to settle, then `n` measured.
    fn run(&mut self, warm: usize, n: usize) -> Vec<Frame> {
        for _ in 0..warm {
            self.step();
        }
        (0..n).map(|_| self.step()).collect()
    }
}

fn strips_laid_out(app: &CantusApp) -> usize {
    app.mixer.strips_laid_out()
}

fn median(mut v: Vec<u64>) -> u64 {
    v.sort_unstable();
    v[v.len() / 2]
}

fn summary(name: &str, frames: &[Frame]) -> (u64, u64, usize, usize, usize) {
    let allocs = median(frames.iter().map(|f| f.allocs).collect());
    let bytes = median(frames.iter().map(|f| f.bytes).collect());
    let tris = frames.iter().map(|f| f.triangles).max().unwrap_or(0);
    let draws = frames.iter().map(|f| f.draws).max().unwrap_or(0);
    let strips = frames.iter().map(|f| f.strips).max().unwrap_or(0);
    eprintln!(
        "{name}: {allocs} allocations, {} KB a frame (median); {tris} triangles, {draws} \
         draw calls, {strips} strips laid out (worst)",
        bytes / 1024
    );
    (allocs, bytes, tris, draws, strips)
}

// ---- deterministic budgets (every `cargo test`) ----------------------------------------

const HD: egui::Vec2 = egui::vec2(1920.0, 1080.0);

/// The mixer alone, 64 strips playing at 1080p: what the desk itself costs.
#[test]
fn mixer64_alone_stays_in_budget() {
    let mut rig = Rig::new(64, false, HD);
    rig.play();
    let frames = rig.run(20, 30);
    let (allocs, bytes, tris, draws, strips) = summary("mixer64, desk alone", &frames);
    assert!(allocs <= MIXER_ALLOCS, "{allocs} allocations a frame");
    assert!(bytes <= 64 * 1024, "{bytes} bytes a frame");
    assert!(tris <= 120_000, "{tris} triangles");
    assert!(draws <= 16, "{draws} draw calls");
    // Only what shows is laid out: at 1080p the compact desk shows nearly all 64…
    let room = (rig.app.mixer.desk_width() / 24.0).ceil() as usize + 1 + 3;
    assert!(strips <= room, "{strips} strips laid out, room for {room}");
    // …the wide one about half.
    cantus_ui::views::mixer::set_width_mode(
        &mut rig.app,
        cantus_ui::views::mixer::layout::WidthMode::Wide,
    );
    let frames = rig.run(5, 10);
    let (allocs, _, _, _, strips) = summary("mixer64 wide, desk alone", &frames);
    let room = (rig.app.mixer.desk_width() / 48.0).ceil() as usize + 1 + 3;
    assert!(
        strips <= room && strips < 64,
        "{strips} strips laid out, room for {room}"
    );
    assert!(allocs <= MIXER_ALLOCS, "{allocs} allocations a frame");
}

/// Sixteen strips: the same, and the desk costs no more than at 64.
#[test]
fn mixer16_alone_stays_in_budget() {
    let mut rig = Rig::new(16, false, HD);
    rig.play();
    let frames = rig.run(20, 30);
    let (allocs, bytes, _, draws, strips) = summary("mixer16, desk alone", &frames);
    assert!(allocs <= MIXER_ALLOCS, "{allocs} allocations a frame");
    assert!(bytes <= 64 * 1024, "{bytes} bytes a frame");
    assert!(draws <= 16, "{draws} draw calls");
    assert!(strips <= 16, "{strips}");
}

/// The whole window with the 64-strip desk, arrangement and inspector: ADR 0024 §13's
/// scenario. The rest of the window is other views' (migrated in parallel); the ceiling
/// here is what the window must never exceed again.
#[test]
fn mixer64_window_stays_in_budget() {
    let mut rig = Rig::new(64, true, HD);
    rig.play();
    let frames = rig.run(20, 30);
    let (allocs, bytes, tris, draws, _) = summary("mixer64, whole window", &frames);
    assert!(allocs <= WINDOW_ALLOCS, "{allocs} allocations a frame");
    assert!(bytes <= WINDOW_BYTES, "{bytes} bytes a frame");
    assert!(tris <= 120_000, "{tris} triangles");
    // The docked workspace (ADR 0024 §5) gives each pane, tab bar and splitter its own clip
    // region, and egui starts a new draw call per clip region: 73 measured with the dock
    // in, against 64 before it. 96 keeps the margin; the GPU cost is what matters, and it is
    // held separately.
    assert!(draws <= 96, "{draws} draw calls");
}

/// Nothing moving: the desk asks for no frame at all.
#[test]
fn an_idle_desk_requests_no_repaint() {
    let mut rig = Rig::new(16, false, HD);
    // Never played: every meter is at silence from the first frame.
    let frames = rig.run(5, 5);
    for f in &frames {
        assert_eq!(f.repaint, Duration::MAX, "the idle desk asked for a frame");
    }
    // Played, then stopped: the meters fall for a while, at most 60 times a second,
    // then the desk goes quiet.
    rig.play();
    rig.run(30, 0);
    rig.stop();
    let falling = rig.step();
    assert!(
        falling.repaint >= Duration::from_millis(15) || falling.repaint == Duration::MAX,
        "falling meters repaint at 60 Hz at most, not at the monitor's rate: {:?}",
        falling.repaint
    );
    let later = rig.run(240, 3);
    for f in &later {
        assert_eq!(
            f.repaint,
            Duration::MAX,
            "the landed desk still asks for frames"
        );
    }
}

// ---- timing (release, `--ignored`) -----------------------------------------------------

fn timing(name: &str, rig: &mut Rig, budget_ms: f64) {
    let frames = rig.run(30, 200);
    let mut ms: Vec<f64> = frames
        .iter()
        .map(|f| (f.build + f.tess).as_secs_f64() * 1e3)
        .collect();
    ms.sort_by(f64::total_cmp);
    let p50 = ms[ms.len() / 2];
    let p95 = ms[ms.len() * 95 / 100];
    summary(name, &frames);
    eprintln!("{name}: build + tessellate p50 {p50:.2} ms, p95 {p95:.2} ms");
    assert!(p50 <= budget_ms * 2.0, "{name}: p50 {p50:.2} ms");
}

#[test]
#[ignore = "timing: run in release with --ignored"]
fn mixer64_window_timing() {
    let mut rig = Rig::new(64, true, HD);
    rig.play();
    timing("mixer64, whole window", &mut rig, 4.0);
}

#[test]
#[ignore = "timing: run in release with --ignored"]
fn mixer16_window_timing() {
    let mut rig = Rig::new(16, true, HD);
    rig.play();
    timing("mixer16, whole window", &mut rig, 2.5);
}

#[test]
#[ignore = "timing: run in release with --ignored"]
fn mixer64_alone_timing() {
    let mut rig = Rig::new(64, false, HD);
    rig.play();
    timing("mixer64, desk alone", &mut rig, 4.0);
}

#[test]
#[ignore = "timing: run in release with --ignored"]
fn mixer16_alone_timing() {
    let mut rig = Rig::new(16, false, HD);
    rig.play();
    timing("mixer16, desk alone", &mut rig, 2.5);
}

/// The desk's own share of the §13 budget of 1,500 allocations a window.
const MIXER_ALLOCS: u64 = 100;
/// The whole window as measured after the desk's migration (3,490 allocations, 1.2 MB;
/// before: 28,778 and 5.6 MB), the rest being views not yet migrated; only ever lowered.
const WINDOW_ALLOCS: u64 = 4_500;
const WINDOW_BYTES: u64 = 1536 * 1024;

// ---- the cable band (round 25, ADR 0041 §12, design §6 and test 14) ---------------------

/// The 64-strip desk wired as design §6 asks: 128 sends and 16 sidechain keys, every
/// route from a lower strip to a higher one (no loops).
fn wired(n: usize) -> Project {
    let mut p = grown(n);
    let ids: Vec<_> = p.strips.iter().map(|s| s.id).collect();
    let mut sends = 0;
    'all: for step in [3usize, 11, 29] {
        for i in 0..n {
            if sends == 128 {
                break 'all;
            }
            if i + step < n && p.strips[i].send_to(ids[i + step]).is_none() {
                p.strips[i].sends.push(Send::new(ids[i + step]));
                sends += 1;
            }
        }
    }
    assert_eq!(sends, 128);
    for k in 0..16 {
        let host = 20 + k;
        let mut fx = p.make_effect(cantus_core::EffectKind::Compressor);
        fx.sidechain = Some(ids[host - 5]);
        p.strips[host].effects.push(fx);
    }
    p
}

fn cable_view(rig: &mut Rig, view: views::mixer::layout::CableView) {
    let mut p = rig.app.mixer.prefs();
    p.cables = view;
    rig.app.mixer.set_prefs(p);
}

/// Design test 14: *All* on 64 strips with 128 sends and 16 keys adds no allocation and no
/// draw call to the desk in steady state; *Selected* draws only the selection's cables
/// and *Off* none; the band stays idle when nothing moves.
#[test]
fn the_cable_band_costs_no_allocation_and_no_draw_call() {
    use views::mixer::layout::CableView;
    let mut rig = Rig::with(wired(64), false, HD);
    rig.play();
    cable_view(&mut rig, CableView::Off);
    let off = rig.run(20, 30);
    let (off_allocs, _, _, off_draws, _) = summary("mixer64 wired, cables off", &off);
    assert_eq!(rig.app.mixer.cables_drawn(), 0);
    cable_view(&mut rig, CableView::All);
    let all = rig.run(20, 30);
    let (allocs, bytes, tris, draws, _) = summary("mixer64 wired, cables all", &all);
    let drawn = rig.app.mixer.cables_drawn();
    eprintln!("cables drawn in All: {drawn}");
    assert!(drawn >= 100, "{drawn} cables drawn in All");
    assert!(
        allocs <= off_allocs,
        "All allocates {allocs} a frame against {off_allocs} with the band off"
    );
    assert!(
        allocs <= MIXER_ALLOCS && bytes <= 64 * 1024,
        "{allocs}, {bytes} B"
    );
    assert_eq!(draws, off_draws, "the band paints into the desk's own mesh");
    assert!(tris <= 120_000, "{tris} triangles");
    // Selected: only the cables into and out of the selection (strip 3 here), and its
    // master route.
    cable_view(&mut rig, CableView::Selected);
    rig.run(3, 1);
    let sel = rig.app.project().strips[2].id;
    let mine = rig
        .app
        .project()
        .strips
        .iter()
        .filter(|s| s.id == sel)
        .map(|s| s.sends.len() + 1)
        .sum::<usize>()
        + rig
            .app
            .project()
            .strips
            .iter()
            .filter(|s| s.send_to(sel).is_some() || s.output == Some(sel))
            .count();
    assert_eq!(rig.app.mixer.cables_drawn(), mine);
    for d in rig.app.mixer.cables() {
        assert!(d.route.from == sel || d.route.to == sel, "{d:?}");
    }
}

/// The wired desk at rest, cables on: no frame is asked for.
#[test]
fn a_wired_idle_desk_requests_no_repaint() {
    let mut rig = Rig::with(wired(64), false, HD);
    cable_view(&mut rig, views::mixer::layout::CableView::All);
    for f in &rig.run(5, 5) {
        assert_eq!(f.repaint, Duration::MAX, "the idle band asked for a frame");
    }
}

#[test]
#[ignore = "timing: run in release with --ignored"]
fn mixer64_wired_all_timing() {
    let mut rig = Rig::with(wired(64), false, HD);
    cable_view(&mut rig, views::mixer::layout::CableView::All);
    rig.play();
    timing("mixer64 wired, cables all, desk alone", &mut rig, 4.0);
}

/// The wired desk with every look a send can take: every fourth send pre-fader, every
/// fifth switched off, and doubled signals (a strip that also routes its output where it
/// sends).
fn wired_every_look(n: usize) -> Project {
    let mut p = wired(n);
    let mut k = 0;
    for st in &mut p.strips {
        for send in &mut st.sends {
            if k % 4 == 0 {
                send.tap = cantus_core::SendTap::Pre;
            }
            if k % 5 == 0 {
                send.enabled = false;
            }
            k += 1;
        }
    }
    // Doubled: the output goes where the first send goes, on every eighth strip.
    for i in (0..n).step_by(8) {
        if let Some(to) = p.strips[i].sends.first().map(|s| s.to) {
            p.strips[i].output = Some(to);
        }
    }
    p
}

fn max_allocs(frames: &[Frame]) -> (u64, u64) {
    (
        frames.iter().map(|f| f.allocs).max().unwrap_or(0),
        frames.iter().map(|f| f.bytes).max().unwrap_or(0),
    )
}

fn fastest(frames: &[Frame]) -> Duration {
    frames.iter().map(|f| f.build).min().unwrap_or_default()
}

/// Reviewers' finding 23: the band's cost held in Wide (pills drawn), with pre, off and
/// doubled sends, a hovered cable and edge chips on screen, by the **worst** frame's
/// allocations (not the median):
/// - at rest, *All* allocates no more than *Off* in any frame;
/// - with a cable hovered (its pill lit, its words in the hint bar), no more than one
///   allocation over the same desk with a fader hovered instead (hovering any control
///   costs the hint bar's own ~17 allocations; the cable's second line of words is the one
///   more);
/// - the build time within a documented tolerance of the plain desk's: the fastest of 30
///   frames with *All* at most 1.6 × the fastest with *Off*, plus 2 ms (the fastest frame,
///   so a busy test machine does not decide it).
#[test]
fn the_band_in_wide_with_every_look_costs_no_allocation() {
    use views::mixer::layout::CableView;
    let mut rig = Rig::with(wired_every_look(64), false, HD);
    views::mixer::set_width_mode(&mut rig.app, views::mixer::layout::WidthMode::Wide);
    rig.play();
    cable_view(&mut rig, CableView::Off);
    // The pointer is reported every frame in every phase (an event costs egui its own
    // allocation): here, away from the desk.
    let away = egui::pos2(-100.0, -100.0);
    rig.pointer = Some(away);
    let off = rig.run(20, 30);
    let (off_max, _) = max_allocs(&off);
    let fader = views::mixer::control_rect(
        &rig.ctx,
        Some(rig.app.project().strips[5].id),
        views::mixer::Control::Fader,
    )
    .expect("a fader on screen")
    .center();
    rig.pointer = Some(fader);
    let (hover_max, _) = max_allocs(&rig.run(5, 30));
    rig.pointer = Some(away);
    cable_view(&mut rig, CableView::All);
    let calm = rig.run(5, 30);
    let (calm_max, calm_bytes) = max_allocs(&calm);
    assert!(
        calm_max <= off_max,
        "at rest, every look: {calm_max} allocations in the worst frame against {off_max} off"
    );
    assert!(calm_bytes <= 64 * 1024, "{calm_bytes} B");
    // A lit cable hovered (its pill and halo), and the selection's far ends as chips.
    let d = *rig
        .app
        .mixer
        .cables()
        .iter()
        .find(|d| d.lit && d.chips == [false, false])
        .expect("a lit cable on screen");
    let path = cantus_ui::kit::cable::path(d.from, d.to, d.depth);
    rig.pointer = Some(path.run_start + egui::vec2(3.0, 0.0));
    let all = rig.run(20, 30);
    let (all_max, all_bytes) = max_allocs(&all);
    let cables = rig.app.mixer.cables();
    assert!(
        cables.iter().any(|d| d.chips != [false, false]),
        "edge chips on screen"
    );
    assert!(!rig.app.mixer.cable_pills().is_empty(), "pills drawn");
    eprintln!(
        "wide wired, every look: off {off_max}, fader hovered {hover_max}, all at rest {calm_max}, cable hovered {all_max} allocations (worst frame, {all_bytes} B); fastest build off {:?}, all {:?}",
        fastest(&off),
        fastest(&all)
    );
    assert!(
        all_max <= hover_max + 1,
        "a hovered cable: {all_max} allocations against {hover_max} for a hovered fader"
    );
    assert!(all_bytes <= 64 * 1024, "{all_bytes} B");
    let budget = fastest(&off).mul_f32(1.6) + Duration::from_millis(2);
    assert!(
        fastest(&all) <= budget,
        "the band costs too much: {:?} against {:?} off",
        fastest(&all),
        fastest(&off)
    );
}
