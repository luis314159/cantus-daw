//! Several screens look right on every screen (ADR 0148; the owner, 2026-11-17: *"make
//! sure it looks excellent on different screens"*). Through `CantusApp::assemble`, run as
//! eframe runs windows (`windows_rig`), fed as egui-winit feeds a maximised window on each:
//!
//! - **One screen**, every common kind — 1 366 × 768 at 100 %, 1 920 × 1 080 at 100 / 125 /
//!   150 %, 2 560 × 1 440 at 100 / 150 %, 2 880 × 1 800 at 200 %, 3 840 × 2 160 at 150 /
//!   200 %, the ultrawide 3 440 × 1 440, a portrait 1 080 × 1 920: the main window draws at
//!   exactly the follow rule's scale times the system's (no fractional blur from a wrong
//!   scale), every word inside the window, and the top bar's words whole (the kit's probe
//!   notes no word dropped there).
//! - **Mixed pairs and three screens** — the laptop at 200 % with a 1 080p screen at 100 %,
//!   a 4K at 150 % with a 1 080p at 100 %, three screens: the mixer in a window of its own on
//!   the other screen draws at that screen's own scale, its words inside it and whole, its
//!   bar's groups whole; the window's menu opens inside that window, beside its key.

mod windows_rig;
use cantus_ui::screen_scale::for_screen;
use cantus_ui::screens::ScreenKey;
use cantus_ui::views::workspace::Pane;
use cantus_ui::windows::Target;
use egui::{PointerButton, ViewportId};
use windows_rig::*;

fn screens_one() -> Vec<ScreenKey> {
    vec![
        screen("A", 1366, 768, 0, 0, 1.0),
        screen("B", 1920, 1080, 0, 0, 1.0),
        screen("C", 1920, 1080, 0, 0, 1.25),
        screen("D", 1920, 1080, 0, 0, 1.5),
        screen("E", 2560, 1440, 0, 0, 1.0),
        screen("F", 2560, 1440, 0, 0, 1.5),
        screen("G", 2880, 1800, 0, 0, 2.0),
        screen("H", 3840, 2160, 0, 0, 1.5),
        screen("I", 3840, 2160, 0, 0, 2.0),
        screen("J", 3440, 1440, 0, 0, 1.0),
        screen("K", 1080, 1920, 0, 0, 1.0),
    ]
}

/// The scale a window on `s` must draw at: the follow rule's interface scale for the
/// screen times the system's.
fn expected_ppp(s: &ScreenKey) -> f32 {
    for_screen(s.size[0] as f32, s.scale) * s.scale
}

/// Every word of viewport `id`'s last pass inside its window.
fn words_inside(r: &Rig, id: ViewportId, what: &str) {
    let size = r.passes[&id].size;
    for (w, rect, clip) in r.texts(id) {
        if !clip.intersects(rect) {
            continue; // scrolled away
        }
        assert!(
            rect.min.x >= -0.5
                && rect.min.y >= -0.5
                && rect.max.x <= size.x + 0.5
                && rect.max.y <= size.y + 0.5,
            "{what}: {w:?} at {rect:?} outside the window's {size:?}"
        );
    }
}

/// The words the kit dropped (not even an ellipsis fit) during `f`, inside `region`.
fn dropped(f: impl FnOnce(), region: egui::Rect) -> Vec<String> {
    cantus_ui::kit::text::probe::start();
    f();
    cantus_ui::kit::text::probe::take()
        .into_iter()
        .filter(|c| c.how == "dropped" && region.intersects(c.rect))
        .map(|c| format!("{:?} in {:?}", c.full, c.rect))
        .collect()
}

#[test]
fn the_main_window_on_every_kind_of_screen() {
    run(|| {
        for s in screens_one() {
            let mut r = Rig::new(vec![s.clone()]);
            r.idle(8);
            let ppp = r.passes[&ViewportId::ROOT].ppp;
            assert!(
                (ppp - expected_ppp(&s)).abs() < 1e-3,
                "{}×{} at {}: drawn at {ppp}, the rule says {}",
                s.size[0],
                s.size[1],
                s.scale,
                expected_ppp(&s)
            );
            words_inside(&r, ViewportId::ROOT, &format!("{s:?}"));
            let bar = egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(r.passes[&ViewportId::ROOT].size.x, 44.0),
            );
            let cut = dropped(|| r.root(Vec::new()), bar);
            assert!(cut.is_empty(), "{s:?}: the top bar dropped {cut:?}");
        }
    });
}

/// The desks of two and three screens: (screens, the screen the mixer's window goes to).
fn desks() -> Vec<(Vec<ScreenKey>, usize)> {
    vec![
        (vec![laptop(), external()], 1),
        (
            vec![
                screen("4K", 3840, 2160, 0, 0, 1.5),
                screen("HD", 1920, 1080, 3840, 540, 1.0),
            ],
            1,
        ),
        (
            vec![
                screen("M", 2560, 1440, 0, 0, 1.0),
                screen("L", 1920, 1080, -1920, 180, 1.0),
                screen("P", 1080, 1920, 2560, -240, 1.0),
            ],
            2,
        ),
    ]
}

#[test]
fn a_window_of_its_own_on_each_desk_draws_at_its_screens_scale() {
    run(|| {
        for (screens, target) in desks() {
            let mut r = Rig::new(screens.clone());
            r.app().reveal_pane(Pane::Mixer);
            r.idle(6);
            let ctx = r.ctx.clone();
            r.app().move_pane(
                &ctx,
                Pane::Mixer,
                Target::NewWindow {
                    screen: Some(target),
                },
            );
            r.idle(8);
            let id = r.windows()[0];
            let s = &screens[target];
            let main = &screens[0];
            // The window's own scale: the rule for its screen, measured against the main
            // window's (the user's own scale for the main window carries over; here none).
            let want = for_screen(s.size[0] as f32, s.scale) * s.scale;
            let ppp = r.passes[&id].ppp;
            assert!(
                (ppp - want).abs() < 1e-3,
                "{s:?}: drawn at {ppp}, want {want}"
            );
            let mp = r.passes[&ViewportId::ROOT].ppp;
            assert!(
                (mp - expected_ppp(main)).abs() < 1e-3,
                "{main:?}: main at {mp}"
            );
            words_inside(&r, id, &format!("the mixer's window on {s:?}"));
            words_inside(
                &r,
                ViewportId::ROOT,
                &format!("the main window on {main:?}"),
            );
            // Its bar's words whole.
            let bar =
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(r.passes[&id].size.x, 44.0));
            let cut = dropped(|| r.child(id, Vec::new()), bar);
            assert!(cut.is_empty(), "{s:?}: the window's bar dropped {cut:?}");
            // Its menu opens in it, under its key, inside the window.
            let key = egui::Pos2::new(8.0 + 16.0, 22.0);
            r.click(id, key, PointerButton::Primary);
            r.child(id, Vec::new());
            let rows = r.text_in(id, "Always on top");
            assert_eq!(
                rows.len(),
                1,
                "{s:?}: the window's menu drawn in its window"
            );
            let size = r.passes[&id].size;
            assert!(rows[0].max.x <= size.x && rows[0].max.y <= size.y, "{s:?}");
            assert!(rows[0].min.y > 30.0, "{s:?}: under the bar");
            assert!(
                r.text_in(ViewportId::ROOT, "Always on top").is_empty(),
                "{s:?}: not in the main window"
            );
        }
    });
}

#[test]
fn the_screens_map_shows_every_screen_and_sends_a_window_across() {
    run(|| {
        let mut r = Rig::new(vec![laptop(), external()]);
        r.app().reveal_pane(Pane::Mixer);
        r.idle(4);
        let ctx = r.ctx.clone();
        r.app()
            .move_pane(&ctx, Pane::Mixer, Target::NewWindow { screen: Some(1) });
        r.idle(6);
        r.app().windows.map_open = true;
        r.app().windows.host = ViewportId::ROOT;
        r.idle(10);
        assert_eq!(r.text_in(ViewportId::ROOT, "Screens and windows").len(), 1);
        assert_eq!(r.text_in(ViewportId::ROOT, "Screen 1 · DISPLAY1").len(), 1);
        assert_eq!(r.text_in(ViewportId::ROOT, "Screen 2 · DISPLAY2").len(), 1);
        assert_eq!(r.text_in(ViewportId::ROOT, "Main window").len(), 1);
        words_inside(&r, ViewportId::ROOT, "the screens map");
        // Its drag's end: the mixer's window to the laptop's screen.
        let key = r.app().windows.list[0].key;
        r.app().send_window_to_screen(key, 0);
        r.idle(4);
        let p = r.app().windows.list[0].place.clone().unwrap();
        assert_eq!(p.screen.size, [2880, 1800], "sent to the laptop");
        // Esc closes it.
        r.key(ViewportId::ROOT, egui::Key::Escape, egui::Modifiers::NONE);
        r.idle(2);
        assert!(!r.app().windows.map_open);
    });
}
