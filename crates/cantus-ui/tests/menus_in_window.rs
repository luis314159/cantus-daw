//! A menu stays inside the window and its rows read whole (round 39, `el-controls`; the
//! two items `ux-b` left on the kit's menu): through `CantusApp::assemble` at 1600 × 900,
//! the track's menu — several lists in one popup, taller than the room under the track —
//! is opened the two ways a person opens it (the context-menu key on the selected track,
//! the right button on the track's head) and every popup on the screen must lie inside the
//! window, the menu scrolling where it is taller; and the kit's text probe must find no
//! word of it cut.

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
