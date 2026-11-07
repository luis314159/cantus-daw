//! The stage over the workspace (round 35, ADR 0091 §1), through the real application's
//! frame (`CantusApp::frame`, what `cantus_ui::run`'s window draws): a floating dancer is
//! see-through over the workspace at physical pixels, pinned on top it is in the
//! foreground, docked it dances on the Stage tab, hidden it is nowhere and costs nothing;
//! it redraws only when its pose changed and no faster than its rate; the frame and grips
//! show on hover.
//!
//! The dancers are real dancer tracks (the welcome song's first track turned into one),
//! posed by the dancer's live feed (`crate::dance_live`) — from the transport here, the
//! test's engine having no device.

use cantus_core::{Command, InstrumentKind, InstrumentSpec, NativeSpec, Ticks, TrackId};
use cantus_engine::EngineHandle;
use cantus_ui::app::CantusApp;
use cantus_ui::i18n::{default_thread_locale, Locale};
use cantus_ui::stage::{Surface, ViewMode};
use cantus_ui::views::workspace::Pane;
use cantus_ui::views::workspace_ticker as ticker;
use cantus_ui::welcome_project;
use egui::{Order, Pos2, Rect, Vec2};

fn app() -> (CantusApp, egui::Context, tempfile::TempDir) {
    default_thread_locale(Locale::En);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let dir = tempfile::tempdir().unwrap();
    let mut a = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    a.settings_path = dir.path().join("settings.json");
    a.layouts_path = dir.path().join("layouts.json");
    a.keymap_path = dir.path().join("keymap.toml");
    a.appearance_path = dir.path().join("appearance.json");
    a.attach(&ctx);
    (a, ctx, dir)
}

/// The welcome song's first track as a dancer standing at `x`, 90 % down, shown as
/// `mode` (on top or not); a track already a dancer just takes the numbers.
fn dancer(a: &mut CantusApp, mode: ViewMode, top: bool, x: f32) -> TrackId {
    let track = a.project().tracks[0].id;
    let is_dancer = a
        .project()
        .track(track)
        .and_then(|t| t.instrument.native())
        .is_some_and(|n| n.kind == InstrumentKind::Dancer);
    if !is_dancer {
        assert!(a.exec(Command::SetInstrument {
            track,
            spec: Box::new(InstrumentSpec::Native(NativeSpec::new(
                InstrumentKind::Dancer
            ))),
        }));
    }
    for (k, v) in [
        ("x", x),
        ("y", 0.9),
        ("view.mode", mode.value()),
        ("view.top", if top { 1.0 } else { 0.0 }),
    ] {
        a.set_inst_value(track, k, v, false);
    }
    a.sync_engine();
    track
}

fn input(t: f64, pointer: Option<Pos2>) -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))),
        time: Some(t),
        predicted_dt: 1.0 / 60.0,
        events: pointer.map(egui::Event::PointerMoved).into_iter().collect(),
        ..Default::default()
    }
}

fn frame(a: &mut CantusApp, ctx: &egui::Context, t: f64, pointer: Option<Pos2>) {
    ctx.run_ui(input(t, pointer), |ui| a.frame(ui))
        .textures_delta
        .clear();
}

/// Frames from `t0`, `dt` apart, `n` of them; the song moves `beats_per_s` while playing.
fn frames(a: &mut CantusApp, ctx: &egui::Context, t0: f64, dt: f64, n: usize, playing: bool) {
    for i in 0..n {
        let t = t0 + dt * i as f64;
        if playing {
            let at = Ticks((t * 2.0 * cantus_core::PPQ as f64) as i64);
            a.engine.set_heard_for_tests(at, true);
        }
        frame(a, ctx, t, None);
    }
}

#[test]
fn the_stage_is_drawn_from_the_applications_frame() {
    let (mut a, ctx, _dir) = app();
    frame(&mut a, &ctx, 0.0, None);
    assert!(a.stage.passes > 0, "the frame reaches the stage");
    assert_eq!(a.stage.textures(), 0, "no dancer, no texture");
    let track = dancer(&mut a, ViewMode::Floating, false, 0.7);
    frames(&mut a, &ctx, 0.1, 0.05, 3, false);
    assert_eq!(a.stage.textures(), 1);
    assert!(a.stage.body(track, Surface::Float).is_some());
}

#[test]
fn a_floating_dancer_is_see_through_over_the_workspace_at_physical_pixels() {
    let mut sizes = Vec::new();
    for ppp in [1.0f32, 2.0] {
        let (mut a, ctx, _dir) = app();
        ctx.set_pixels_per_point(ppp);
        let track = dancer(&mut a, ViewMode::Floating, false, 0.7);
        frames(&mut a, &ctx, 0.0, 0.1, 4, false);
        let p = a.stage.pixmap(track, Surface::Float).expect("drawn");
        assert_eq!(p.pixel(0, 0).unwrap().alpha(), 0, "a corner is see-through");
        let w = p.width() - 1;
        assert_eq!(p.pixel(w, 0).unwrap().alpha(), 0);
        let opaque = p.pixels().iter().filter(|c| c.alpha() == 255).count();
        assert!(
            opaque > 1000,
            "the dancer is drawn ({opaque} opaque pixels)"
        );
        let body = a.stage.body(track, Surface::Float).unwrap();
        // The body stands at 70 % across, its feet at 90 % down the workspace.
        assert!(body.center().x > 1600.0 * 0.6 && body.center().x < 1600.0 * 0.8);
        sizes.push((p.width(), p.height(), body));
    }
    let ((w1, h1, b1), (w2, h2, b2)) = (sizes[0], sizes[1]);
    // Drawn again at twice the pixels, never stretched: the same body in points.
    assert!((b1.height() - b2.height()).abs() < 1.5, "{b1:?} {b2:?}");
    // Each box is rounded up to a bucket, so the 100 % one may carry up to a bucket of
    // slack that doubles at 200 % (Fuga, round 37: 481 → 512 at 100 %, 960 at 200 %).
    let bucket = cantus_ui::stage::geom::BUCKET;
    assert!(
        w2 + 2 * bucket >= 2 * w1 && w2 <= 2 * w1 + bucket,
        "{w1} → {w2}"
    );
    assert!(
        h2 + 2 * bucket >= 2 * h1 && h2 <= 2 * h1 + bucket,
        "{h1} → {h2}"
    );
}

#[test]
fn pinned_on_top_it_is_in_the_foreground_else_among_the_panels() {
    for top in [false, true] {
        let (mut a, ctx, _dir) = app();
        let track = dancer(&mut a, ViewMode::Floating, top, 0.5);
        frames(&mut a, &ctx, 0.0, 0.1, 4, false);
        let body = a.stage.body(track, Surface::Float).unwrap();
        let layer = ctx
            .layer_id_at(body.center())
            .expect("a layer under the dancer");
        let want = if top {
            Order::Foreground
        } else {
            Order::Middle
        };
        assert_eq!(layer.order, want, "top = {top}");
    }
}

#[test]
fn docked_it_dances_on_the_stage_tab_and_hidden_nowhere() {
    let (mut a, ctx, _dir) = app();
    a.workspace.reveal(Pane::Stage);
    let track = dancer(&mut a, ViewMode::Docked, false, 0.5);
    frames(&mut a, &ctx, 0.0, 0.1, 4, false);
    assert!(a.stage.pixmap(track, Surface::Dock).is_some(), "on the tab");
    assert!(
        a.stage.pixmap(track, Surface::Float).is_none(),
        "not floating"
    );
    let body = a.stage.body(track, Surface::Dock).unwrap();
    let inspector_x = 1600.0 * 0.6;
    assert!(
        body.center().x > inspector_x,
        "under the inspector: {body:?}"
    );

    let _ = dancer(&mut a, ViewMode::Hidden, false, 0.5);
    let before = a.stage.redraws;
    frames(&mut a, &ctx, 1.0, 0.1, 4, false);
    assert_eq!(a.stage.textures(), 0, "hidden: no texture kept");
    assert_eq!(a.stage.redraws, before, "hidden: nothing drawn");
}

#[test]
fn it_redraws_only_when_its_pose_changed_and_no_faster_than_its_rate() {
    let (mut a, ctx, _dir) = app();
    let _ = dancer(&mut a, ViewMode::Floating, false, 0.6);
    frames(&mut a, &ctx, 0.0, 0.1, 2, false);
    // Stopped, frames at 240 a second for a second: the breath at its idle rate.
    let r0 = a.stage.redraws;
    frames(&mut a, &ctx, 1.0, 1.0 / 240.0, 240, false);
    let idle = a.stage.redraws - r0;
    let cap = cantus_ui::stage::IDLE_RATE as u64 + 2;
    assert!(idle >= 8 && idle <= cap, "{idle} redraws a second stopped");
    // The same moment twice: nothing to draw.
    let r1 = a.stage.redraws;
    frame(&mut a, &ctx, 3.0, None);
    frame(&mut a, &ctx, 3.0, None);
    assert!(a.stage.redraws - r1 <= 1);
    // Playing, frames at 144 a second: at most 30 (what a video plays at).
    let r2 = a.stage.redraws;
    frames(&mut a, &ctx, 5.0, 1.0 / 144.0, 144, true);
    let playing = a.stage.redraws - r2;
    let cap = cantus_ui::stage::PLAY_RATE as u64 + 2;
    assert!(
        playing >= 20 && playing <= cap,
        "{playing} redraws a second playing"
    );
}

#[test]
fn stopped_it_asks_for_its_breath_and_nothing_hidden_or_minimised() {
    let (mut a, ctx, _dir) = app();
    frames(&mut a, &ctx, 0.0, 0.5, 4, false);
    let _ = dancer(&mut a, ViewMode::Floating, false, 0.6);
    frames(&mut a, &ctx, 3.0, 0.5, 3, false);
    let with = ticker::last(&ctx);
    let breath = std::time::Duration::from_secs_f64(1.0 / cantus_ui::stage::IDLE_RATE);
    assert!(
        with.requested
            .is_some_and(|d| d <= breath + std::time::Duration::from_millis(2)),
        "the idle breath asks for its next frame: {with:?}"
    );
    let _ = dancer(&mut a, ViewMode::Hidden, false, 0.6);
    frames(&mut a, &ctx, 6.0, 0.5, 3, false);
    // Nothing soon: what is left is the autosave's wake-up, a minute away.
    let quiet =
        |d: Option<std::time::Duration>| d.is_none_or(|d| d > std::time::Duration::from_secs(1));
    assert!(
        quiet(ticker::last(&ctx).requested),
        "hidden asks nothing: {:?}",
        ticker::last(&ctx)
    );

    let _ = dancer(&mut a, ViewMode::Floating, false, 0.6);
    frames(&mut a, &ctx, 9.0, 0.5, 2, false);
    let mut minimised = input(11.0, None);
    minimised
        .viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .minimized = Some(true);
    let before = a.stage.redraws;
    ctx.run_ui(minimised, |ui| a.frame(ui))
        .textures_delta
        .clear();
    assert!(
        quiet(ticker::last(&ctx).requested),
        "minimised asks nothing: {:?}",
        ticker::last(&ctx)
    );
    assert!(a.stage.redraws <= before + 1);
}

#[test]
fn its_frame_shows_on_hover_and_lingers_a_moment() {
    let (mut a, ctx, _dir) = app();
    let track = dancer(&mut a, ViewMode::Floating, false, 0.5);
    frames(&mut a, &ctx, 0.0, 0.1, 3, false);
    let body = a.stage.body(track, Surface::Float).unwrap();
    assert_eq!(a.stage.hovered(0.3), None);
    frame(&mut a, &ctx, 0.4, Some(body.center()));
    frame(&mut a, &ctx, 0.45, Some(body.center()));
    assert_eq!(a.stage.hovered(0.45), Some((track, Surface::Float)));
    // Away: it lingers, then goes.
    frame(&mut a, &ctx, 0.5, Some(Pos2::new(5.0, 300.0)));
    assert!(a.stage.hovered(0.6).is_some());
    frame(&mut a, &ctx, 1.5, Some(Pos2::new(5.0, 300.0)));
    assert_eq!(a.stage.hovered(1.5), None);
}

#[test]
fn the_stage_tab_goes_under_the_inspector_and_keeps_its_place_in_a_layout() {
    let (mut a, ctx, _dir) = app();
    a.workspace.reveal(Pane::Stage);
    frames(&mut a, &ctx, 0.0, 0.1, 2, false);
    let rect_of = |a: &CantusApp, pane: Pane| {
        a.workspace
            .dock
            .iter_all_nodes()
            .find_map(|(_, n)| match n {
                egui_dock::Node::Leaf(l) if l.tabs.contains(&pane) => Some(l.rect),
                _ => None,
            })
            .unwrap()
    };
    let stage = rect_of(&a, Pane::Stage);
    let inspector = rect_of(&a, Pane::Inspector);
    assert!(
        (stage.left() - inspector.left()).abs() < 2.0,
        "{stage:?} {inspector:?}"
    );
    assert!(stage.top() >= inspector.bottom() - 2.0, "below it");
    let v = cantus_ui::layout::dock_to_value(&a.workspace.dock).unwrap();
    let back: egui_dock::DockState<Pane> = serde_json::from_value(v).unwrap();
    assert!(back.find_tab(&Pane::Stage).is_some());
}

fn frame_events(a: &mut CantusApp, ctx: &egui::Context, t: f64, events: Vec<egui::Event>) {
    let mut input = input(t, None);
    input.events = events;
    ctx.run_ui(input, |ui| a.frame(ui)).textures_delta.clear();
}

fn x_of(a: &CantusApp, track: TrackId) -> f32 {
    a.project()
        .track(track)
        .and_then(|t| t.instrument.native())
        .map(|n| n.get("x"))
        .unwrap()
}

#[test]
fn dragging_a_dancer_moves_it_in_one_undo_step() {
    let (mut a, ctx, _dir) = app();
    let track = dancer(&mut a, ViewMode::Floating, false, 0.5);
    frames(&mut a, &ctx, 0.0, 0.1, 3, false);
    let body = a.stage.body(track, Surface::Float).unwrap();
    let from = body.center();
    let press = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    frame_events(&mut a, &ctx, 1.0, vec![egui::Event::PointerMoved(from)]);
    frame_events(&mut a, &ctx, 1.02, vec![press(from, true)]);
    for i in 1..=8 {
        let p = from + egui::vec2(20.0 * i as f32, 0.0);
        frame_events(
            &mut a,
            &ctx,
            1.02 + i as f64 * 0.02,
            vec![egui::Event::PointerMoved(p)],
        );
    }
    let to = from + egui::vec2(160.0, 0.0);
    frame_events(&mut a, &ctx, 1.3, vec![press(to, false)]);
    let x = x_of(&a, track);
    assert!(x > 0.55, "it moved right ({x})");
    a.undo();
    assert!(
        (x_of(&a, track) - 0.5).abs() < 1e-6,
        "one undo step brings it back"
    );
}

#[test]
fn its_pill_docks_it_and_hides_it() {
    let (mut a, ctx, _dir) = app();
    let track = dancer(&mut a, ViewMode::Floating, false, 0.5);
    frames(&mut a, &ctx, 0.0, 0.1, 3, false);
    let body = a.stage.body(track, Surface::Float).unwrap();
    // Hover shows the pill above the body; its second button docks (pin, dock, hide).
    frame_events(
        &mut a,
        &ctx,
        1.0,
        vec![egui::Event::PointerMoved(body.center())],
    );
    frame_events(
        &mut a,
        &ctx,
        1.05,
        vec![egui::Event::PointerMoved(body.center())],
    );
    let mode = |a: &CantusApp| {
        a.project()
            .track(track)
            .and_then(|t| t.instrument.native())
            .map(|n| n.get("view.mode"))
            .unwrap()
    };
    // Found by the chrome's own layout: dock is the button 24 pt left of the last.
    let s = cantus_ui::kit::Style::current();
    let chrome = cantus_ui::stage::chrome::layout(
        &ctx,
        body,
        "Keys",
        &[
            cantus_ui::stage::chrome::Button::Pin,
            cantus_ui::stage::chrome::Button::Dock,
            cantus_ui::stage::chrome::Button::Hide,
        ],
        &s,
        Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0)),
    );
    let dock = chrome.buttons[1].1.center();
    let click = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    frame_events(&mut a, &ctx, 1.1, vec![egui::Event::PointerMoved(dock)]);
    frame_events(&mut a, &ctx, 1.15, vec![click(dock, true)]);
    frame_events(&mut a, &ctx, 1.2, vec![click(dock, false)]);
    assert_eq!(mode(&a), 1.0, "docked");
    assert!(a.workspace.contains(Pane::Stage), "the Stage tab opened");
}
