//! A menu stays inside the window and its rows read whole (round 39, `el-controls`; the
//! two items `ux-b` left on the kit's menu): through `CantusApp::assemble` at 1600 × 900,
//! the track's menu — several lists in one popup, taller than the room under the track —
//! is opened the two ways a person opens it (the context-menu key on the selected track,
//! the right button on the track's head) and every popup on the screen must lie inside the
//! window, the menu scrolling where it is taller; and the kit's text probe must find no
//! word of it cut. The bar's View menu too: whole where it fits, scrolling where it does not.

mod paths_rig;

use cantus_ui::kit::text::probe;
use egui::{Key, Pos2, Rect};
use paths_rig::*;

/// Every popup shown now, with its rect.
fn popups(r: &Rig) -> Vec<Rect> {
    let layers = r.ctx.memory(|m| m.areas().visible_layer_ids());
    layers
        .into_iter()
        .filter(|l| l.order == egui::Order::Foreground)
        .filter_map(|l| r.ctx.memory(|m| m.area_rect(l.id)))
        .filter(|rect| rect.width() > 40.0 && rect.height() > 40.0)
        .collect()
}

fn inside(rect: Rect) -> bool {
    let screen = Rect::from_min_size(Pos2::ZERO, SIZE).expand(0.5);
    screen.contains_rect(rect)
}

fn rig() -> Rig {
    let mut r = Rig::new(cantus_ui::welcome_project());
    r.idle(3);
    r
}

/// The words cut in the frame just drawn, inside `region`.
fn cuts_in(r: &mut Rig, region: Rect) -> Vec<String> {
    probe::start();
    r.idle(1);
    probe::take()
        .into_iter()
        .filter(|c| region.intersects(c.rect))
        .map(|c| format!("{:?} → {:?}", c.full, c.shown))
        .collect()
}

#[test]
fn the_track_menu_opens_inside_the_window_and_reads_whole() {
    let mut r = rig();
    // By the keys: Shift+F10 on the selected track.
    r.app.clear_clip_selection();
    r.app.selected_clip = None;
    r.app.focus_arrangement();
    r.key(Key::F10, SHIFT);
    r.idle(3);
    r.shot("track-menu-in-window-keys");
    let shown = popups(&r);
    assert!(!shown.is_empty(), "the track's menu opened");
    for p in &shown {
        assert!(inside(*p), "a popup runs off the 1600 × 900 window: {p:?}");
    }
    let menu = shown.iter().fold(Rect::NOTHING, |a, b| a.union(*b));
    let cut = cuts_in(&mut r, menu);
    assert!(cut.is_empty(), "words cut in the track's menu: {cut:?}");
    // The keys walk to its last row, and the menu still lies inside the window.
    let (_, n) = cantus_ui::menu_keys::last(&r.ctx).expect("a menu the keys walk");
    for _ in 0..n {
        r.key(Key::ArrowDown, NONE);
    }
    r.idle(2);
    for p in popups(&r) {
        assert!(
            inside(p),
            "walked to its foot, a popup runs off the window: {p:?}"
        );
    }
    r.key(Key::Escape, NONE);
    r.idle(2);
    // By the pointer: the right button on the first track's head, in the arrangement.
    let name = r.app.project().tracks[0].name.clone();
    let leaf = r
        .leaf_of(cantus_ui::views::workspace::Pane::Arrangement)
        .expect("the arrangement is shown");
    let head = r.must_in(&name, leaf);
    r.right_click(head.center());
    r.idle(3);
    r.shot("track-menu-in-window-pointer");
    let shown = popups(&r);
    assert!(
        !shown.is_empty(),
        "the right button opened the track's menu"
    );
    for p in &shown {
        assert!(inside(*p), "a popup runs off the 1600 × 900 window: {p:?}");
    }
}

/// The bar's View menu (the owner's screen, 2026-11-11: it ran off the foot of the window and
/// nothing scrolled; its rows were egui's check boxes and buttons, two heights and boxed
/// entries). Opened by its word, as a person does: one kit menu, whole where the window has
/// room for it — its first frame no longer caps it at egui's 400-pt sizing pass — a check
/// switching its area and closing the menu; in a window 520 pt tall it is taller than the
/// window, lies inside it all the same and scrolls to its last row.
#[test]
fn the_view_menu_shows_whole_where_it_fits_and_scrolls_where_it_does_not() {
    use cantus_ui::views::workspace::Pane;
    let mut r = rig();
    let word = r.must("View");
    r.click(word.center());
    r.idle(3);
    r.shot("view-menu-whole");
    let shown = popups(&r);
    assert!(!shown.is_empty(), "the View menu opened");
    for p in &shown {
        assert!(inside(*p), "a popup runs off the 1600 × 900 window: {p:?}");
    }
    let menu = shown.iter().fold(Rect::NOTHING, |a, b| a.union(*b));
    assert!(
        menu.height() > 600.0,
        "the menu is {} pt tall: capped by the sizing pass",
        menu.height()
    );
    // Whole: its last row is painted inside it, no word cut.
    r.must_in("Engine statistics", menu);
    let cut = cuts_in(&mut r, menu);
    assert!(cut.is_empty(), "words cut in the View menu: {cut:?}");
    // A check switches its area and closes the menu, as every entry does.
    let had = r.app.workspace.contains(Pane::History);
    let row = r.must_in("History", menu);
    r.click(row.center());
    r.idle(2);
    assert_ne!(
        r.app.workspace.contains(Pane::History),
        had,
        "History switched"
    );
    assert!(popups(&r).is_empty(), "the menu closed");

    // A window 520 pt tall (a laptop's browser-sized window, or the interface zoomed): the
    // menu is taller than it.
    r.size = egui::vec2(1600.0, 520.0);
    r.idle(3);
    let screen = r.ctx.content_rect().expand(0.5);
    let word = r.must("View");
    r.click(word.center());
    r.idle(3);
    r.shot("view-menu-short");
    let shown = popups(&r);
    assert!(!shown.is_empty(), "the View menu opened in a short window");
    for p in &shown {
        assert!(
            screen.contains_rect(*p),
            "in a short window a popup runs off it {screen:?}: {p:?}"
        );
    }
    let menu = shown.iter().fold(Rect::NOTHING, |a, b| a.union(*b));
    let last_seen = |r: &Rig| {
        r.text("Engine statistics")
            .is_some_and(|t| menu.contains_rect(t))
    };
    assert!(
        !last_seen(&r),
        "taller than the window, its foot waits below"
    );
    // The wheel over it walks to its foot, still inside the window.
    for _ in 0..30 {
        if last_seen(&r) {
            break;
        }
        r.wheel(menu.center(), -3.0, NONE);
        r.idle(1);
    }
    assert!(last_seen(&r), "scrolled, its last row shows");
    for p in popups(&r) {
        assert!(
            screen.contains_rect(p),
            "scrolled, a popup runs off the window: {p:?}"
        );
    }
}
