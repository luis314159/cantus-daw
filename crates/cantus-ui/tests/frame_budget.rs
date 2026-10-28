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
        cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
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
        // Quiet before measuring: no instrument's samples still loading beside the
        // window (their progress rebuilds the status line on whichever frame they land).
        let t = Instant::now();
        while self.app.engine.samples.progress().is_some() && t.elapsed().as_secs() < 20 {
            std::thread::sleep(Duration::from_millis(5));
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
    // Round 31 (ADR 0068) added the top bar's automation arm, a mode badge on each header and
    // the inspector's Automation row: 120 108 measured, against 120 000 before.
    assert!(tris <= 121_000, "{tris} triangles");
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

/// The worst frame's allocations and bytes, but for one: GitHub's Windows runner shows a
/// single odd frame now and then (71 against 36, 39 against 36 — never reproduced on a
/// desk machine), while a cost that comes back comes back on more than one frame. So the
/// second-worst frame is what these budgets compare (the worst, with fewer than two).
fn max_allocs(frames: &[Frame]) -> (u64, u64) {
    let second = |mut v: Vec<u64>| {
        v.sort_unstable();
        v.iter().rev().nth(1).or(v.last()).copied().unwrap_or(0)
    };
    (
        second(frames.iter().map(|f| f.allocs).collect()),
        second(frames.iter().map(|f| f.bytes).collect()),
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
///   costs egui's own ~16 allocations — its hit test and interaction lists, and with debug
///   assertions one plugin-list clone per widget under the pointer; the band is the one
///   widget more under it — see `a_hovered_desk_control_costs_only_what_egui_charges_for_a_hover`
///   and `the_hint_bar_costs_no_allocation`, round 26);
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

// ---- round 26: what hovering a control costs (BUILDING.md mixer item 5) ----------------

/// Round 26: the hint bar itself allocates nothing. A control writes its hint into the
/// slot's reused buffers (`kit::hint::set`) and the bar copies it into its own
/// (`views::hint`); the words are laid out once and cached. Measured by the worst of 30
/// warm frames painting the bar with a hovered hint (a name, a value, the capability
/// icons), with a live value (and the pill by the pointer), and with nothing hovered (the
/// idle words): all three allocate the same. (Round 25 credited the hint bar with the
/// ~17 allocations a hovered desk control costs; they are egui's, below.)
#[test]
fn the_hint_bar_costs_no_allocation() {
    use cantus_ui::kit::hint::{self, Caps};
    use std::fmt::Write as _;
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
    let bar = egui::Rect::from_min_size(egui::pos2(10.0, 4.0), egui::vec2(420.0, 34.0));
    let mut frame = 0u64;
    // `bar`: paint the hint bar and the value pill — or, the control experiment, one plain
    // shape where the bar would be (what egui charges for anything painted there).
    // `layer`: a bare shape on a layer of the tooltip order too, as the pill's.
    let mut run = |hovered: Option<(&str, &str, bool)>,
                   down: bool,
                   bar_on: bool,
                   layer: bool,
                   n: usize|
     -> u64 {
        let mut worst = 0;
        for i in 0..20 + n {
            frame += 1;
            let p = egui::pos2(200.0, 200.0);
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, HD)),
                time: Some(frame as f64 / 60.0),
                predicted_dt: 1.0 / 60.0,
                events: vec![
                    egui::Event::PointerMoved(p),
                    egui::Event::PointerButton {
                        pos: p,
                        button: egui::PointerButton::Primary,
                        pressed: down,
                        modifiers: Default::default(),
                    },
                ],
                ..Default::default()
            };
            let (out, allocs, _) = counted(|| {
                ctx.run_ui(input, |ui| {
                    if let Some((name, value, live)) = hovered {
                        hint::set(ui.ctx(), name, Caps::PARAM, live, |b| b.write_str(value));
                    }
                    if bar_on {
                        let s = cantus_ui::kit::Style::current();
                        cantus_ui::kit::with_brush(ui.painter(), |b| {
                            views::hint::paint_panel(
                                b,
                                bar,
                                &s,
                                "Song for the Lord",
                                Some("workspace.mixer.hint"),
                            );
                        });
                        views::hint::value_pill(ui.ctx());
                    } else {
                        ui.painter().rect_filled(bar, 3.0, egui::Color32::DARK_GRAY);
                    }
                    if layer {
                        let id = egui::Id::new("bare-tooltip-layer");
                        ui.ctx()
                            .layer_painter(egui::LayerId::new(egui::Order::Tooltip, id))
                            .rect_filled(bar, 2.0, egui::Color32::GRAY);
                    }
                })
            });
            let mut out = out;
            out.textures_delta.clear();
            if i >= 20 {
                worst = worst.max(allocs);
            }
        }
        worst
    };
    // Review finding 8: against an absolute control, not the bar itself — a frame with
    // one plain shape where the bar is, nothing of ours painted.
    let control = run(None, false, false, false, 30);
    let idle = run(None, false, true, false, 30);
    let hovered = run(
        Some(("Volume", "\u{2212}6.0 dB", false)),
        false,
        true,
        false,
        30,
    );
    // A drag: the button held in both, the value live and its pill in one, a plain shape
    // and a bare shape on a tooltip layer of its own in the other.
    let held = run(None, true, false, true, 30);
    let live = run(
        Some(("Volume", "\u{2212}4.5 dB", true)),
        true,
        true,
        false,
        30,
    );
    eprintln!(
        "hint bar: control {control}, idle {idle}, hovered {hovered}; control held {held}, live value and pill {live} allocations (worst frame)"
    );
    assert!(
        idle <= control,
        "the idle bar: {idle} allocations against {control} for a plain shape"
    );
    assert!(
        hovered <= control,
        "a hovered hint: {hovered} allocations against {control} for a plain shape"
    );
    // The pill adds its own shapes to the frame, and egui gathers every layer's shapes into
    // one list (`GraphicLayers::drain` appends them): crossing a power of two regrows
    // that list once — egui's, not the pill's; nothing else may come with it.
    assert!(
        live <= held + 1,
        "a live value and its pill: {live} against {held}"
    );
}

/// Round 26: what hovering a desk control costs is egui's own price for hovering a
/// widget, and nothing of ours. A bare egui frame shaped like the desk's (a central
/// panel, the desk's area, and a control under the pointer among neighbours, as a fader
/// sits among the strip's parts) is the control experiment: hovered against away, it pays
/// for egui's hit test and interaction lists (and, in builds with debug assertions, its
/// `on_widget_under_pointer` plugin hook, one list clone per widget under the pointer).
/// The 64-strip desk with a fader hovered may cost no more over the same desk at rest
/// than that.
#[test]
fn a_hovered_desk_control_costs_only_what_egui_charges_for_a_hover() {
    let away = egui::pos2(-100.0, -100.0);
    // The desk: 64 strips, Wide, a fader hovered.
    let mut rig = Rig::new(64, false, HD);
    views::mixer::set_width_mode(&mut rig.app, views::mixer::layout::WidthMode::Wide);
    rig.play();
    rig.pointer = Some(away);
    let (rest, _) = max_allocs(&rig.run(20, 30));
    let fader = views::mixer::control_rect(
        &rig.ctx,
        Some(rig.app.project().strips[5].id),
        views::mixer::Control::Fader,
    )
    .expect("a fader on screen")
    .center();
    rig.pointer = Some(fader);
    let (hover, _) = max_allocs(&rig.run(10, 30));
    // The control: the same nesting in bare egui, a widget under the pointer among
    // neighbours.
    let ctx = egui::Context::default();
    let mut frame = 0u64;
    let mut bare = |at: egui::Pos2| -> u64 {
        let mut worst = 0;
        for i in 0..40 {
            frame += 1;
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, HD)),
                time: Some(frame as f64 / 60.0),
                predicted_dt: 1.0 / 60.0,
                events: vec![egui::Event::PointerMoved(at)],
                ..Default::default()
            };
            let (out, allocs, _) = counted(|| {
                ctx.run_ui(input, |root| {
                    egui::CentralPanel::default().show(root, |ui| {
                        let full = ui.available_rect_before_wrap();
                        ui.allocate_rect(full, egui::Sense::hover());
                        for k in 0..12 {
                            let r = egui::Rect::from_center_size(
                                fader
                                    + egui::vec2(
                                        (k % 4) as f32 * 9.0 - 13.5,
                                        (k / 4) as f32 * 9.0 - 9.0,
                                    ),
                                egui::vec2(8.0, 8.0),
                            );
                            ui.interact(
                                r,
                                egui::Id::new(("bare", k)),
                                egui::Sense::click_and_drag(),
                            );
                        }
                    });
                })
            });
            let mut out = out;
            out.textures_delta.clear();
            if i >= 10 {
                worst = worst.max(allocs);
            }
        }
        worst
    };
    let bare_rest = bare(away);
    let bare_hover = bare(fader);
    eprintln!(
        "hover: desk {rest} -> {hover} (+{}), bare egui {bare_rest} -> {bare_hover} (+{}) allocations (worst frame)",
        hover.saturating_sub(rest),
        bare_hover.saturating_sub(bare_rest)
    );
    assert!(
        hover.saturating_sub(rest) <= bare_hover.saturating_sub(bare_rest),
        "a hovered fader costs {} allocations over the desk at rest; egui's own hover costs {}",
        hover.saturating_sub(rest),
        bare_hover.saturating_sub(bare_rest)
    );
}

// ---- round 26, second review: a hovered bus with many inputs (finding 5) ---------------

/// 64 strips; strip 6 a bus fed by 32 sends (strips 21 to 52), All cables, Wide.
fn bus_desk() -> Project {
    let mut p = grown(64);
    let bus = p.strips[5].id;
    for i in 20..52 {
        p.strips[i].sends.push(Send::new(bus));
    }
    p
}

fn hover_fader(rig: &mut Rig, index: usize) -> Vec<Frame> {
    let id = rig.app.project().strips[index].id;
    let at = views::mixer::control_rect(&rig.ctx, Some(id), views::mixer::Control::Fader)
        .expect("the fader on screen")
        .center();
    rig.pointer = Some(at);
    rig.run(10, 30)
}

/// Second review, finding 5: hovering a bus with 32 inputs lights 32 cables that the
/// placement fits around the selection's — at no allocation over hovering a plain strip
/// (the placement keeps its scratch lists), and (timing, release) well under 0.3 ms more.
/// A placement that allocates does so on every frame; one frame over the plain strip's
/// worst is forgiven (GitHub's Windows runner showed a single such frame, 71 against 36,
/// that no local machine reproduces), a second is not.
#[test]
fn a_hovered_bus_with_many_inputs_costs_no_allocation() {
    use views::mixer::layout::CableView;
    let mut rig = Rig::with(bus_desk(), false, HD);
    views::mixer::set_width_mode(&mut rig.app, views::mixer::layout::WidthMode::Wide);
    cable_view(&mut rig, CableView::All);
    rig.play();
    rig.run(20, 0);
    let (plain, _) = max_allocs(&hover_fader(&mut rig, 6));
    let frames = hover_fader(&mut rig, 5);
    let (bus, _) = max_allocs(&frames);
    let over: Vec<u64> = frames
        .iter()
        .map(|f| f.allocs)
        .filter(|&a| a > plain)
        .collect();
    let bus_id = rig.app.project().strips[5].id;
    let into = rig
        .app
        .mixer
        .cables()
        .iter()
        .filter(|d| d.route.to == bus_id)
        .count();
    let lit = rig.app.mixer.cables().iter().filter(|d| d.lit).count();
    eprintln!(
        "hovered: plain strip {plain}, bus {bus} allocations (worst frame); {into} cables into the bus, {lit} lit"
    );
    assert!(into >= 32, "the bus's inputs are drawn: {into}");
    assert!(
        lit >= 3,
        "as many lit (with their marks) as the band holds apart: {lit}"
    );
    assert!(
        rig.app
            .mixer
            .cables()
            .iter()
            .filter(|d| d.route.to == bus_id)
            .all(|d| d.lit || d.yielded),
        "every input reads as the bus's: lit, or in its colour without a mark"
    );
    assert!(
        over.len() <= 1,
        "a hovered bus: {bus} allocations against {plain}, {} frames over it: {over:?}",
        over.len()
    );
}

#[test]
#[ignore = "timing: run in release with --ignored"]
fn a_hovered_bus_with_many_inputs_timing() {
    use views::mixer::layout::CableView;
    let mut rig = Rig::with(bus_desk(), false, HD);
    views::mixer::set_width_mode(&mut rig.app, views::mixer::layout::WidthMode::Wide);
    cable_view(&mut rig, CableView::All);
    rig.play();
    rig.run(20, 0);
    let plain = fastest(&hover_fader(&mut rig, 6));
    let bus = fastest(&hover_fader(&mut rig, 5));
    eprintln!("fastest build: plain strip hovered {plain:?}, bus hovered {bus:?}");
    assert!(
        bus <= plain + Duration::from_micros(300),
        "a hovered bus costs {:?} more",
        bus.saturating_sub(plain)
    );
}

/// The hosted cases one at a time: `plugins::demo_setup` copies the test plug-in into one
/// shared folder, and a copy made while another case loads it can leave it unreadable.
fn hosted_serial() -> std::sync::MutexGuard<'static, ()> {
    static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    ONE.lock().unwrap_or_else(|e| e.into_inner())
}

/// The test CLAP plug-in, built from source where the host's tests build it.
fn test_clap() -> std::path::PathBuf {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let target = root.join("target").join("test-plugins");
    let status =
        std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args(["build", "-p", "cantus-test-clap", "--target-dir"])
            .arg(&target)
            .current_dir(&root)
            .status()
            .expect("cargo");
    assert!(status.success());
    target.join("debug").join(if cfg!(windows) {
        "cantus_test_clap.dll"
    } else if cfg!(target_os = "macos") {
        "libcantus_test_clap.dylib"
    } else {
        "libcantus_test_clap.so"
    })
}

/// Round 29, review 2 of ADR 0062, #30: with a hosted plug-in loaded and a controller
/// mapped to one of its parameters, an idle frame's controllers (the filter's check, the
/// feedback's, the clocks) allocate nothing — a hosted parameter is looked up, not copied.
#[test]
fn an_idle_frame_with_a_hosted_mapping_costs_no_allocation() {
    use cantus_core::controller::*;
    let (engine, _processor) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::from_parts(Some(cantus_ui::starter_project()), engine, false);
    app.midi_port_names = vec!["Keys".into()];
    let _one = hosted_serial();
    cantus_ui::views::plugins::demo_setup(&mut app, &test_clap(), true);
    let effect = app
        .fx
        .open
        .map(|(_, e, _)| e)
        .expect("the test plug-in inserted");
    let target = MappingTarget::Param(cantus_core::AutoTarget::Plugin { effect, id: 0 });
    let mut m = Mapping::new(
        MappingId(1),
        MidiSource {
            port: PortRef {
                name: "Keys".into(),
                ordinal: 0,
            },
            channel: Some(0),
            control: Control::Cc(20),
        },
        target.clone(),
        MapMode::Absolute,
    );
    m.plugin = target.plugin_of(app.project());
    assert!(app.add_mapping(cantus_ui::controllers::Scope::Project, m));
    assert!(app
        .hosted_param_info(&cantus_core::AutoTarget::Plugin { effect, id: 0 })
        .is_some());
    // Published once; then idle.
    app.controller_input();
    app.controller_input();
    let (_, allocs, bytes) = counted(|| {
        for _ in 0..10 {
            app.controller_input();
        }
    });
    eprintln!("an idle controller frame with a hosted mapping: {allocs} allocations, {bytes} B");
    assert_eq!(allocs, 0, "{allocs} allocations in ten idle frames");
}

/// Round 29, review 3 of ADR 0062, #51: an editor's stepped controls (its switches,
/// choices, lamps) offer MIDI learn, and with the learn mode off, nothing mapped and no
/// menu open the offer builds nothing. A warm frame of the Compressor's editor and of the
/// Wavetable's Edit view, nothing mapped, stays inside the allocations measured with the
/// offers idle; and the offers alone, called over a drawn control, allocate nothing.
#[test]
fn an_editors_idle_stepped_controls_cost_no_allocation() {
    use cantus_core::{EffectKind, InstrumentKind, InstrumentSpec, NativeSpec};
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
    let size = egui::vec2(1600.0, 1000.0);
    let mut frame = 0u64;
    let mut measure = |app: &mut CantusApp, show: &mut dyn FnMut(&mut CantusApp, &mut egui::Ui)| {
        let mut counts = Vec::new();
        for i in 0..30 {
            frame += 1;
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                time: Some(frame as f64 / 60.0),
                predicted_dt: 1.0 / 60.0,
                ..Default::default()
            };
            let (out, allocs, _) = counted(|| ctx.run_ui(input, |ui| show(app, ui)));
            let mut out = out;
            out.textures_delta.clear();
            if i >= 20 {
                counts.push(allocs);
            }
        }
        median(counts)
    };
    // The Compressor's editor.
    let (engine, _p) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::from_parts(Some(cantus_ui::starter_project()), engine, false);
    let s0 = app.project().tracks[0].strip.unwrap();
    app.add_effect(s0, EffectKind::Compressor);
    let fx = app.project().strip(s0).unwrap().effects.last().unwrap().id;
    cantus_ui::views::fx::demo_setup(&mut app, s0, fx, EffectKind::Compressor);
    app.open_effect_editor(s0, fx);
    let comp = measure(&mut app, &mut |a, ui| {
        let ctx = ui.ctx().clone();
        cantus_ui::views::comp_editor::window(a, &ctx);
        cantus_ui::views::fx::window(a, &ctx);
    });
    // The Wavetable's Edit view.
    let (engine, _p) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::from_parts(Some(cantus_ui::starter_project()), engine, false);
    let track = app.project().tracks[0].id;
    app.set_instrument(
        track,
        InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Wavetable)),
    );
    cantus_ui::views::inst::demo_setup(&mut app, track, InstrumentKind::Wavetable);
    app.open_inst_editor(track);
    app.inst.view = cantus_ui::views::inst::View::Edit;
    let wt = measure(&mut app, &mut |a, ui| {
        cantus_ui::views::inst::window_in(a, ui)
    });
    eprintln!("a warm editor frame, nothing mapped: Compressor {comp}, Wavetable {wt} allocations");
    assert!(
        comp <= COMP_EDITOR_ALLOCS,
        "Compressor: {comp} allocations a frame"
    );
    assert!(
        wt <= WAVETABLE_EDITOR_ALLOCS,
        "Wavetable: {wt} allocations a frame"
    );
    // The offer alone over a drawn switch: nothing.
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        ..Default::default()
    };
    let mut alone = u64::MAX;
    let _ = ctx.run_ui(input, |ui| {
        let resp = ui.interact(
            egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(40.0, 20.0)),
            egui::Id::new("budget-switch"),
            egui::Sense::click(),
        );
        let (_, n, _) = counted(|| {
            cantus_ui::controllers::stepped_inst_of(
                &mut app,
                ui,
                track,
                InstrumentKind::Wavetable,
                "osc1.on",
                &resp,
            )
        });
        alone = n;
    });
    assert_eq!(alone, 0, "an idle stepped offer allocated {alone} times");
}

/// A warm frame of the Compressor's editor, nothing mapped: 191 measured with the stepped
/// offers idle, 209 with them building their targets (round 29, review 3 of ADR 0062, #51).
const COMP_EDITOR_ALLOCS: u64 = 200;
/// The Wavetable's Edit view: 1 494 measured idle, 1 646 with the offers building.
const WAVETABLE_EDITOR_ALLOCS: u64 = 1_560;

/// Round 29, review 4 of ADR 0062, #27, #56, #63: the pane's word for a hosted choice asks
/// the plug-in for the one step it shows — not for each of its steps, every cell, every
/// frame. The test plug-in's fault selector has 25 steps.
#[test]
fn a_hosted_choices_word_asks_for_one_step() {
    use cantus_core::controller::MappingTarget;
    let (engine, _processor) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::from_parts(Some(cantus_ui::starter_project()), engine, false);
    let _one = hosted_serial();
    cantus_ui::views::plugins::demo_setup(&mut app, &test_clap(), true);
    let effect = app
        .fx
        .open
        .map(|(_, e, _)| e)
        .expect("the test plug-in inserted");
    let target = MappingTarget::Param(cantus_core::AutoTarget::Plugin { effect, id: 4 });
    let steps = app
        .hosted_param_info(&cantus_core::AutoTarget::Plugin { effect, id: 4 })
        .and_then(|i| i.step_count())
        .expect("a stepped parameter");
    assert!(steps > 16, "{steps}");
    let _ = cantus_ui::views::controllers::value_words(&app, &target, 0.5);
    let (word, allocs, _) =
        counted(|| cantus_ui::views::controllers::value_words(&app, &target, 3.0 / steps as f32));
    eprintln!("a hosted choice's word: {allocs} allocations ({word})");
    assert_eq!(word, "3");
    assert!(
        allocs <= 8,
        "{allocs} allocations: one step asked, not {steps}"
    );
}

// ---- a hosted plug-in of 500 parameters in Cantus's view (ADR 0073) ---------------------

/// 500 parameters in 25 groups of 20 — a big synth's pages — the last of each a meter (a
/// read-only parameter, as plug-ins report a level or a gain reduction).
fn five_hundred() -> Vec<cantus_host::ParamInfo> {
    five_hundred_in(20)
}

/// 500 parameters in groups of `per_group` (500: one page, one card — only its rows in
/// view may be drawn).
fn five_hundred_in(per_group: u32) -> Vec<cantus_host::ParamInfo> {
    (0..500u32)
        .map(|i| cantus_host::ParamInfo {
            id: 1000 + i,
            name: format!("Param {}", i + 1),
            group: format!("Module {}", i / per_group + 1),
            min: 0.0,
            max: 1.0,
            default: 0.5,
            stepped: false,
            automatable: true,
            hidden: false,
            read_only: i % 20 == 19,
            steps: 0,
        })
        .collect()
}

/// What one scrolled frame of the view cost and drew.
struct HostedFrame {
    build: Duration,
    tess: Duration,
    allocs: u64,
    drawn: usize,
    meters: usize,
    text: String,
}

/// The test gain's Cantus view with 500 parameters, scrolled by the wheel every frame from
/// the sixth on: `n` frames.
fn hosted500_frames(n: usize) -> Vec<HostedFrame> {
    hosted_frames(n, five_hundred())
}

/// [`hosted500_frames`] with the parameters given.
fn hosted_frames(n: usize, params: Vec<cantus_host::ParamInfo>) -> Vec<HostedFrame> {
    use egui::{pos2, vec2, Event, Modifiers, MouseWheelUnit, TouchPhase};
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let (engine, _processor) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::from_parts(Some(cantus_ui::starter_project()), engine, false);
    let _one = hosted_serial();
    cantus_ui::views::plugins::demo_setup(&mut app, &test_clap(), true);
    let effect = app
        .fx
        .open
        .map(|(_, e, _)| e)
        .expect("the Cantus view opened");
    app.plugins.set_params_for_tests(effect, params);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
    let size = vec2(1600.0, 1000.0);
    let at = pos2(800.0, 640.0);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let mut events = vec![Event::PointerMoved(at)];
        if i >= 5 {
            events.push(Event::MouseWheel {
                unit: MouseWheelUnit::Point,
                delta: vec2(0.0, -120.0),
                phase: TouchPhase::Move,
                modifiers: Modifiers::NONE,
            });
        }
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            time: Some(i as f64 / 60.0),
            predicted_dt: 1.0 / 60.0,
            events,
            ..Default::default()
        };
        let t0 = Instant::now();
        let (mut full, allocs, _) = counted(|| {
            ctx.run_ui(input, |ui| {
                let c = ui.ctx().clone();
                views::fx::window(&mut app, &c);
            })
        });
        let build = t0.elapsed();
        full.textures_delta.clear();
        let text = hosted_text(&full.shapes);
        let t1 = Instant::now();
        let _ = ctx.tessellate(full.shapes, full.pixels_per_point);
        let tess = t1.elapsed();
        let (drawn, meters) = cantus_ui::views::plugin_view::drawn(&app);
        out.push(HostedFrame {
            build,
            tess,
            allocs,
            drawn,
            meters,
            text,
        });
    }
    out
}

fn hosted_text(shapes: &[egui::epaint::ClippedShape]) -> String {
    fn walk(shape: &egui::Shape, out: &mut String) {
        match shape {
            egui::Shape::Text(t) => {
                out.push_str(t.galley.text());
                out.push_str(" | ");
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = String::new();
    for s in shapes {
        walk(&s.shape, &mut out);
    }
    out
}

/// The cells a frame of the view may draw: what fits in its window (a few rows of nine)
/// and the rows half in view at its edges — never the 500.
const HOSTED_MAX_DRAWN: usize = 96;
/// A scrolled frame's allocations, median: 161 measured on Windows (round 31, ADR 0073).
const HOSTED500_ALLOCS: u64 = 200;

/// ADR 0073 (BACKLOG §6h point 2): Cantus's view of a 500-parameter plug-in scrolls from
/// its first group to its last drawing only the rows in view — its cells, its meters — and
/// a scrolled frame allocates within its measured bound.
#[test]
fn a_500_parameter_plugin_scrolls_drawing_only_the_rows_in_view() {
    let frames = hosted500_frames(160);
    let warm = &frames[5..];
    for (i, f) in warm.iter().enumerate() {
        assert!(f.drawn > 0, "frame {i} drew no cell");
        assert!(
            f.drawn <= HOSTED_MAX_DRAWN,
            "frame {i} drew {} cells of 500",
            f.drawn
        );
    }
    assert!(frames[5].text.contains("MODULE 1 |"), "{}", frames[5].text);
    assert!(
        warm.iter().any(|f| f.text.contains("MODULE 25 |")),
        "the wheel reached the last group"
    );
    assert!(warm.iter().any(|f| f.meters > 0), "its meters drawn");
    let mut allocs: Vec<u64> = warm.iter().map(|f| f.allocs).collect();
    allocs.sort_unstable();
    let median = allocs[allocs.len() / 2];
    let mut ms: Vec<f64> = warm
        .iter()
        .map(|f| (f.build + f.tess).as_secs_f64() * 1e3)
        .collect();
    ms.sort_by(f64::total_cmp);
    let most = warm.iter().map(|f| f.drawn).max().unwrap_or(0);
    eprintln!(
        "500 parameters, scrolled: at most {most} cells drawn, {median} allocations (median), \
         build + tessellate p50 {:.2} ms (this build)",
        ms[ms.len() / 2]
    );
    assert!(
        median <= HOSTED500_ALLOCS,
        "{median} allocations a scrolled frame"
    );
}

#[test]
#[ignore = "timing: run in release with --ignored"]
fn a_500_parameter_plugin_scroll_timing() {
    let frames = hosted500_frames(240);
    let mut ms: Vec<f64> = frames[20..]
        .iter()
        .map(|f| (f.build + f.tess).as_secs_f64() * 1e3)
        .collect();
    ms.sort_by(f64::total_cmp);
    let p50 = ms[ms.len() / 2];
    let p95 = ms[ms.len() * 95 / 100];
    eprintln!("500 parameters, scrolled: build + tessellate p50 {p50:.2} ms, p95 {p95:.2} ms");
    // The editor alone, as the mixer's 64 strips alone: 4 ms, asserted at twice that.
    assert!(p50 <= 8.0, "p50 {p50:.2} ms");
}

/// The cells a frame may draw of one card of 500: its rows in view — the card is in view
/// all along, so only the culling of its rows keeps this bound.
const HOSTED_ONE_CARD_MAX_DRAWN: usize = 72;

/// Review of ADR 0073, #6: the 500 parameters in **one** page — one card taller than the
/// window — scrolled from its first row to its last: only the rows in view are drawn.
#[test]
fn one_page_of_500_parameters_draws_only_its_rows_in_view() {
    let frames = hosted_frames(160, five_hundred_in(500));
    let warm = &frames[5..];
    for (i, f) in warm.iter().enumerate() {
        assert!(f.drawn > 0, "frame {i} drew no cell");
        assert!(
            f.drawn <= HOSTED_ONE_CARD_MAX_DRAWN,
            "frame {i} drew {} cells of 500",
            f.drawn
        );
    }
    assert!(
        warm.iter().any(|f| f.text.contains("| PARAM 500 |")),
        "the wheel reached the last row"
    );
    let most = warm.iter().map(|f| f.drawn).max().unwrap_or(0);
    eprintln!("one page of 500, scrolled: at most {most} cells drawn");
}
