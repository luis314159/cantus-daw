//! Every area of the workspace resizable, movable, collapsible and restorable, its shape
//! kept between sessions, and every overflowing area scrollable (round 39, ADR 0110) —
//! through `CantusApp::assemble`, with the pointer: a splitter dragged, a tab's menu, a
//! double-click on a tab, the window closed and Cantus opened again over the same folder.

mod paths_rig;

use cantus_ui::views::workspace::Pane;
use egui::{pos2, vec2, Key, Rect};
use paths_rig::*;

fn session(dir: &std::path::Path) -> Rig {
    let mut r = Rig::in_dir(dir, Some(cantus_ui::welcome_project()));
    r.idle(3);
    r
}

/// The splitter between the arrangement's leaf and the inspector's: its x.
fn inspector_split_x(r: &Rig) -> f32 {
    r.leaf_of(Pane::Inspector).expect("the inspector").left() - 1.0
}

/// A tab's menu row, chosen with the pointer.
fn tab_menu(r: &mut Rig, tab: Pane, row: &str) {
    let at = r.tab(tab).center();
    r.right_click(at);
    let at = r.must(row).center();
    r.click(at);
}

#[test]
fn a_dragged_splitter_is_kept_when_cantus_opens_again() {
    let dir = tempfile::tempdir().unwrap();
    let wide = {
        let mut r = session(dir.path());
        let before = r.leaf_of(Pane::Inspector).unwrap().width();
        let x = inspector_split_x(&r);
        r.drag(pos2(x, 400.0), pos2(x - 120.0, 400.0));
        let after = r.leaf_of(Pane::Inspector).unwrap().width();
        assert!(
            (after - before - 120.0).abs() < 6.0,
            "the drag widened it: {before} → {after}"
        );
        r.close_window();
        after
    };
    let r = session(dir.path());
    let again = r.leaf_of(Pane::Inspector).unwrap().width();
    assert!((again - wide).abs() < 2.0, "kept: {wide} → {again}");
}

#[test]
fn an_area_collapses_to_its_tab_and_expands_back_where_it_was_across_sessions() {
    let dir = tempfile::tempdir().unwrap();
    let open_h = {
        let mut r = session(dir.path());
        let h = r.leaf_of(Pane::Mixer).unwrap().height();
        tab_menu(&mut r, Pane::Mixer, "Collapse area");
        r.idle(2);
        assert!(r.app.workspace.is_folded(Pane::Mixer));
        let folded = r.leaf_of(Pane::Mixer).unwrap().height();
        assert!(folded < 40.0, "down to its tab bar: {folded}");
        let arr = r.leaf_of(Pane::Arrangement).unwrap().height();
        assert!(arr > h, "the arrangement took the room: {arr}");
        r.shot("panels-folded");
        r.close_window();
        h
    };
    let mut r = session(dir.path());
    assert!(r.app.workspace.is_folded(Pane::Mixer), "still folded");
    assert!(r.leaf_of(Pane::Mixer).unwrap().height() < 40.0);
    tab_menu(&mut r, Pane::Mixer, "Expand area");
    r.idle(2);
    let back = r.leaf_of(Pane::Mixer).unwrap().height();
    assert!(
        (back - open_h).abs() < 3.0,
        "back as it was: {open_h} → {back}"
    );
}

#[test]
fn a_side_area_folds_to_a_rail_that_a_click_opens() {
    let tmp = tempfile::tempdir().unwrap();
    let mut r = session(tmp.path());
    let w = r.leaf_of(Pane::Inspector).unwrap().width();
    tab_menu(&mut r, Pane::Inspector, "Collapse area");
    r.idle(2);
    let rail = r.leaf_of(Pane::Inspector).unwrap();
    assert!(rail.width() < 40.0, "a rail: {}", rail.width());
    r.shot("panels-rail");
    r.click(pos2(rail.center().x, rail.top() + 60.0));
    r.idle(2);
    assert!(!r.app.workspace.is_folded(Pane::Inspector));
    let back = r.leaf_of(Pane::Inspector).unwrap().width();
    assert!((back - w).abs() < 3.0, "back as it was: {w} → {back}");
}

#[test]
fn a_double_click_on_a_tab_maximises_its_area_and_gives_the_rest_back() {
    let tmp = tempfile::tempdir().unwrap();
    let mut r = session(tmp.path());
    let before = r.leaf_of(Pane::Mixer).unwrap();
    let at = r.tab(Pane::Mixer).center();
    r.double_click(at);
    r.idle(2);
    assert_eq!(r.app.workspace.maximised(), Some(Pane::Mixer));
    let big = r.leaf_of(Pane::Mixer).unwrap();
    // r39:lo-arrange — the whole workspace less the docks' row (28) the others fold to.
    assert!(big.height() > before.height() * 1.7, "{before:?} → {big:?}");
    assert!(
        r.leaf_of(Pane::Arrangement).is_none(),
        "the others step aside"
    );
    r.shot("panels-maximised");
    // The key does the same: Ctrl+Alt+Enter gives them back.
    r.key(Key::Enter, CTRL | egui::Modifiers::ALT);
    assert_eq!(r.app.workspace.maximised(), None);
    let back = r.leaf_of(Pane::Mixer).unwrap();
    assert!((back.height() - before.height()).abs() < 3.0);
    assert!(r.leaf_of(Pane::Arrangement).is_some() && r.leaf_of(Pane::Inspector).is_some());
}

#[test]
fn an_area_floats_from_its_tab_and_docks_back_and_floats_again_next_session() {
    let dir = tempfile::tempdir().unwrap();
    {
        let mut r = session(dir.path());
        let float = cantus_ui::i18n::t("workspace.menu.float");
        tab_menu(&mut r, Pane::Mixer, &float);
        assert!(r.app.workspace.is_floating(Pane::Mixer));
        r.close_window();
    }
    let mut r = session(dir.path());
    assert!(r.app.workspace.is_floating(Pane::Mixer), "kept floating");
    let dock = cantus_ui::i18n::t("workspace.menu.dock");
    tab_menu(&mut r, Pane::Mixer, &dock);
    assert!(!r.app.workspace.is_floating(Pane::Mixer));
}

/// Every area that holds more than it shows scrolls: shown in a small window, a wheel over
/// it (with Shift, for what overflows at the right) moves what is seen.
#[test]
fn every_overflowing_area_scrolls() {
    let tmp = tempfile::tempdir().unwrap();
    let mut r = session(tmp.path());
    // r39:lo-mixer — the inspector's sections fold to one row each (des-mixer), so it fits
    // the window as it starts: open them, as a user does, so it holds more than it shows.
    for key in [
        "kit.inspector.routing",
        "inspector.sidechain",
        "rec.recording",
        "inspector.automation",
        "inspector.level",
        "freeze.section",
    ] {
        cantus_ui::views::inspector::unfold(&r.ctx, key);
    }
    // A track chosen: its sheet, then its channel's.
    if let Some(t) = r.app.project().tracks.first().map(|t| t.id) {
        r.app.select_track(t);
    }
    let mut stuck = Vec::new();
    let mut overflowing = Vec::new();
    for pane in [
        Pane::Arrangement,
        Pane::Mixer,
        Pane::PianoRoll,
        Pane::ChannelRack,
        Pane::Inspector,
        Pane::Browser,
        Pane::Devices,
        Pane::Routing,
        Pane::Controllers,
        Pane::History,
    ] {
        r.app.show_pane(pane);
        r.idle(4);
        let Some(leaf) = r.leaf_of(pane) else {
            stuck.push(format!("{pane:?}: not shown"));
            continue;
        };
        let body = Rect::from_min_max(leaf.min + vec2(0.0, 30.0), leaf.max);
        for (shift, edge) in [(false, "bottom"), (true, "right")] {
            // Words of this area cut by its own edge: seen inside it, painted past it.
            let cut = |r: &Rig| -> Vec<String> {
                r.texts
                    .iter()
                    .filter(|p| p.seen.is_positive() && body.contains(p.seen.center()))
                    .filter(|p| {
                        !p.elided
                            && if shift {
                                p.rect.right() > p.seen.right() + 1.0
                            } else {
                                p.rect.bottom() > p.seen.bottom() + 1.0
                            }
                    })
                    .map(|p| format!("{:?}", p.text))
                    .collect()
            };
            let words = cut(&r);
            if words.is_empty() {
                continue;
            }
            let seen = |r: &Rig| -> Vec<(String, i32, i32)> {
                r.texts
                    .iter()
                    .filter(|p| p.seen.is_positive() && body.contains(p.seen.center()))
                    .map(|p| (p.text.clone(), p.rect.min.x as i32, p.rect.min.y as i32))
                    .collect()
            };
            let before = seen(&r);
            let m = if shift {
                egui::Modifiers::SHIFT
            } else {
                egui::Modifiers::NONE
            };
            r.wheel(body.center(), -4.0, m);
            println!(
                "{pane:?} overflows at its {edge} ({} words cut)",
                words.len()
            );
            overflowing.push((pane, shift));
            if seen(&r) == before {
                r.shot(&format!("panels-stuck-{pane:?}"));
                stuck.push(format!(
                    "{pane:?} overflows at its {edge} and does not scroll: {words:?}"
                ));
            }
        }
    }
    assert!(stuck.is_empty(), "{}", stuck.join("\n"));
    // Not a test of nothing: the inspector holds more than the default window shows. Where
    // its last row ends whole at the edge (no word cut), a wheel over it still moves it.
    if !overflowing.contains(&(Pane::Inspector, false)) {
        r.app.show_pane(Pane::Inspector);
        r.idle(4);
        let leaf = r.leaf_of(Pane::Inspector).expect("the inspector shown");
        let body = Rect::from_min_max(leaf.min + vec2(0.0, 30.0), leaf.max);
        let seen = |r: &Rig| -> Vec<(String, i32)> {
            r.texts
                .iter()
                .filter(|p| p.seen.is_positive() && body.contains(p.seen.center()))
                .map(|p| (p.text.clone(), p.rect.min.y as i32))
                .collect()
        };
        let before = seen(&r);
        r.wheel(body.center(), -4.0, egui::Modifiers::NONE);
        assert_ne!(seen(&r), before, "the inspector holds more and scrolls");
    }
}
