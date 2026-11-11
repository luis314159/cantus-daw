//! Keyboard reach (round 39, ADR 0110), through `CantusApp::assemble`: every action of the
//! menus and of the main context menus is reachable without the mouse — a shortcut of the
//! keymap or an entry of the command palette, found under the same words — Esc closes
//! what opened last, and the keyboard moves between the workspace's areas in reading
//! order with the focused one lit.
//!
//! The menus are opened as a person opens them and their rows read off the screen: a row
//! nobody can reach from the keyboard fails here by name.

mod paths_rig;

use cantus_ui::keymap::Action;
use cantus_ui::views::workspace::Pane;
use egui::{pos2, Key, Pos2};
use paths_rig::*;
use std::collections::BTreeSet;

/// Where the pointer rests while a menu is read: over nothing that changes the help line.
const REST: Pos2 = Pos2::new(1250.0, 600.0);

/// The words of a row as a name: no "…", no shortcut after a wide gap, no check mark.
fn name_of(row: &str) -> String {
    let row = row.split("    ").next().unwrap_or(row);
    row.trim()
        .trim_end_matches('…')
        .trim_end_matches("...")
        .trim()
        .to_lowercase()
}

/// Every name the keyboard reaches: the palette's entries (which hold every action of the
/// keymap, the areas, windows, Settings pages and devices) and every action's own name.
fn reachable(r: &mut Rig) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for e in cantus_ui::views::palette::entries(&mut r.app) {
        out.insert(name_of(&e.name));
        for a in &e.also {
            out.insert(name_of(a));
        }
    }
    for a in Action::ALL {
        out.insert(name_of(&cantus_ui::i18n::t(&a.label_key())));
    }
    out
}

/// One row of a menu as painted: its words, whether they were cut short, and whether a
/// shortcut is painted on the same line.
#[derive(Clone, Debug)]
struct Row {
    label: String,
    elided: bool,
    keyed: bool,
}

/// Whether `s` is a shortcut as the menus print one ("Ctrl+Shift+P", "F5", "0").
fn is_shortcut(s: &str) -> bool {
    cantus_ui::keymap::Binding::parse(s.trim()).is_some()
}

/// The rows a frame paints that the frame before it, with the pointer in the same place,
/// did not: what `open` drew (a menu), grouped into lines.
fn opened_by(r: &mut Rig, open: impl FnOnce(&mut Rig)) -> Vec<Row> {
    r.pointer = REST;
    r.idle(3);
    let before: BTreeSet<(String, [i32; 4])> = r.texts.iter().map(key).collect();
    open(r);
    r.pointer = REST;
    cantus_ui::kit::inspect::set_enabled(&r.ctx, true);
    r.idle(3);
    // The kit's menus say where they are: only their words, when there are any.
    let menus: Vec<egui::Rect> = cantus_ui::kit::inspect::items(&r.ctx)
        .iter()
        .filter(|i| i.part == "menu")
        .map(|i| i.rect)
        .collect();
    cantus_ui::kit::inspect::set_enabled(&r.ctx, false);
    let mut new: Vec<&Painted> = r
        .texts
        .iter()
        .filter(|p| p.seen.is_positive() && !p.text.trim().is_empty())
        .filter(|p| !before.contains(&key(p)))
        .filter(|p| menus.is_empty() || menus.iter().any(|m| m.contains(p.seen.center())))
        .collect();
    new.sort_by(|a, b| {
        a.rect
            .center()
            .y
            .total_cmp(&b.rect.center().y)
            .then(a.rect.left().total_cmp(&b.rect.left()))
    });
    let mut rows: Vec<Row> = Vec::new();
    let mut line: Vec<&Painted> = Vec::new();
    let flush = |line: &mut Vec<&Painted>, rows: &mut Vec<Row>| {
        if line.is_empty() {
            return;
        }
        let keyed = line
            .iter()
            .any(|p| is_shortcut(&p.text) || p.text.split("    ").nth(1).is_some_and(is_shortcut));
        if let Some(l) = line.iter().find(|p| !is_shortcut(&p.text)) {
            rows.push(Row {
                label: l.text.clone(),
                elided: l.elided,
                keyed,
            });
        }
        line.clear();
    };
    for p in new {
        if line
            .first()
            .is_some_and(|f| (f.rect.center().y - p.rect.center().y).abs() > 4.0)
        {
            flush(&mut line, &mut rows);
        }
        line.push(p);
    }
    flush(&mut line, &mut rows);
    rows
}

fn key(p: &Painted) -> (String, [i32; 4]) {
    let r = p.rect;
    (
        p.text.clone(),
        [
            r.min.x as i32,
            r.min.y as i32,
            r.max.x as i32,
            r.max.y as i32,
        ],
    )
}

/// Close whatever menu is open.
fn close(r: &mut Rig) {
    r.key(Key::Escape, NONE);
    r.idle(2);
}

/// Rows that are not actions: a submenu's heading (its rows are read when it opens),
/// headings and values of the song (numbers, single marks).
fn is_heading(row: &str) -> bool {
    const SUBMENUS: [&str; 1] = ["layout"];
    let n = name_of(row);
    SUBMENUS.contains(&n.as_str())
        || row.chars().all(|c| c.is_ascii_digit() || c.is_whitespace())
        || row.chars().count() <= 1
}

/// The rows no shortcut or palette entry reaches.
fn unreachable(rows: &[Row], reach: &BTreeSet<String>) -> Vec<String> {
    rows.iter()
        .filter(|r| !r.keyed && !is_heading(&r.label))
        .filter(|r| {
            let n = name_of(&r.label);
            !(reach.contains(&n) || r.elided && reach.iter().any(|x| x.starts_with(&n)))
        })
        .map(|r| r.label.clone())
        .collect()
}

fn rig() -> Rig {
    let mut r = Rig::new(cantus_ui::welcome_project());
    r.idle(3);
    r
}

/// Every row of the top bar's menus (File, Edit, Clip, View, Options, Help).
#[test]
fn every_row_of_the_menus_is_reachable_from_the_keyboard() {
    let mut r = rig();
    let reach = reachable(&mut r);
    let mut missing = Vec::new();
    // Round 39 (ADR 0111): the menus are title case ("File", no longer "FILE").
    for word in ["File", "Edit", "Clip", "View", "Options", "Help"] {
        let at = r.must(word).center();
        let rows = opened_by(&mut r, |r| r.click(at));
        assert!(rows.len() >= 2, "the {word} menu opened: {rows:?}");
        for m in unreachable(&rows, &reach) {
            missing.push(format!("{word} ▸ {m}"));
        }
        close(&mut r);
    }
    assert!(
        missing.is_empty(),
        "{} menu rows have no key and no palette entry:\n{}",
        missing.len(),
        missing.join("\n")
    );
}

/// The empty timeline's menu, opened with the pointer (no key opens it: it is about a
/// place, not a selection): every row has a key or a palette entry.
#[test]
fn every_row_of_the_timeline_menu_is_reachable_from_the_keyboard() {
    let mut r = rig();
    let reach = reachable(&mut r);
    let g = r.app.arrangement.geometry().unwrap();
    let empty = pos2(1000.0, g.rows[2].top + 10.0);
    let rows = opened_by(&mut r, |r| r.right_click(empty));
    assert!(rows.len() >= 5, "the timeline menu opened: {rows:?}");
    let missing = unreachable(&rows, &reach);
    assert!(
        missing.is_empty(),
        "rows with no key and no palette entry: {missing:?}"
    );
}

/// Walk the menu the keyboard just opened: ↓ visits every row that acts, once each.
fn walk(r: &mut Rig, what: &str) -> usize {
    r.idle(2);
    let (_, n) = cantus_ui::menu_keys::last(&r.ctx).unwrap_or_else(|| panic!("{what}: no menu"));
    assert!(n >= 2, "{what}: {n} rows");
    let mut seen = BTreeSet::new();
    for _ in 0..n {
        r.key(Key::ArrowDown, NONE);
        let (at, m) = cantus_ui::menu_keys::last(&r.ctx).expect("still open");
        assert_eq!(m, n, "{what}: the same menu");
        seen.insert(at.expect("a row is lit"));
    }
    assert_eq!(seen.len(), n, "{what}: every row once");
    n
}

/// The context menus a selection has open from the keyboard — Shift+F10 (the platform's
/// context-menu key) on the selected clip and track and on the focused area's tab, and the
/// palette's *Open the menu of the selection* on the mixer's strip — and every row is
/// reached with the arrows; Enter runs the lit one.
#[test]
fn every_context_menu_opens_from_the_keyboard_and_its_rows_are_walked() {
    let mut r = rig();
    // The selected clip's menu.
    r.app.focus_arrangement();
    assert!(
        !r.app.clip_selection().is_empty(),
        "the welcome song has a clip selected"
    );
    r.key(Key::F10, SHIFT);
    assert!(
        r.text(&cantus_ui::i18n::t("clip.split")).is_some(),
        "the clip's menu opened"
    );
    let clip = walk(&mut r, "clip");
    r.key(Key::ArrowDown, NONE);
    r.shot("clip-menu-by-keys");
    r.key(Key::Escape, NONE);
    r.idle(2);
    assert!(
        r.text(&cantus_ui::i18n::t("clip.split")).is_none(),
        "Esc closed the clip's menu"
    );
    // The selected track's menu, no clip selected.
    r.app.clear_clip_selection();
    r.app.selected_clip = None;
    r.app.focus_arrangement();
    r.key(Key::F10, SHIFT);
    r.shot("track-menu-by-keys");
    let track = walk(&mut r, "track");
    assert!(
        track >= 12,
        "the track's menu walks as one over its lists: {track}"
    );
    r.key(Key::Escape, NONE);
    r.idle(2);
    // The inspector's tab menu: the keyboard moves there, Shift+F10 opens it, Enter runs.
    while r.app.workspace.focused_area() != Some(Pane::Inspector) {
        r.key(Key::F6, CTRL);
    }
    r.key(Key::F10, SHIFT);
    assert!(
        r.text("Maximise area").is_some(),
        "the inspector's tab menu opened"
    );
    let tab = walk(&mut r, "tab");
    // To "Maximise area", then Enter.
    r.key(Key::Home, NONE);
    r.key(Key::ArrowDown, NONE);
    r.key(Key::ArrowDown, NONE);
    r.key(Key::Enter, NONE);
    assert_eq!(
        r.app.workspace.maximised(),
        Some(Pane::Inspector),
        "Enter ran the row"
    );
    r.key(Key::Enter, CTRL | egui::Modifiers::ALT);
    // The mixer strip's menu from the palette.
    while r.app.workspace.focused_area() != Some(Pane::Mixer) {
        r.key(Key::F6, CTRL);
    }
    let s = r.app.project().tracks[1].strip.unwrap();
    r.app.select_strip(s);
    r.key(Key::P, ctrl_shift());
    r.type_text("menu of the selection");
    r.key(Key::Enter, NONE);
    let strip = walk(&mut r, "mixer strip");
    println!("rows walked: clip {clip}, track {track}, tab {tab}, strip {strip}");
}

/// Esc closes what opened last: Settings, then the palette over it — one Esc closes the
/// palette and leaves Settings, the next closes Settings; the selection is not cleared
/// while either is open.
#[test]
fn esc_closes_what_opened_last() {
    let mut r = rig();
    let selected = r.app.selected_clip;
    assert!(selected.is_some());
    r.key(Key::Comma, CTRL);
    assert!(r.app.show_settings);
    r.key(Key::P, ctrl_shift());
    assert!(r.app.workflow.palette.is_open());
    r.key(Key::Escape, NONE);
    assert!(
        !r.app.workflow.palette.is_open(),
        "the palette, opened last, closed"
    );
    assert!(r.app.show_settings, "Settings stays");
    r.key(Key::Escape, NONE);
    assert!(!r.app.show_settings, "then Settings");
    assert_eq!(
        r.app.selected_clip, selected,
        "nothing else took the two Escs"
    );
}

/// A tab's menu over the palette's absence: Esc closes the menu and nothing else.
#[test]
fn esc_closes_a_tabs_menu_and_keeps_the_selection() {
    let mut r = rig();
    let selected = r.app.selected_clip;
    let tab = r.tab(Pane::Arrangement).center();
    r.right_click(tab);
    assert!(r.text("Maximise area").is_some(), "the tab's menu is open");
    r.shot("tab-menu");
    r.key(Key::Escape, NONE);
    assert!(r.text("Maximise area").is_none(), "closed");
    assert_eq!(r.app.selected_clip, selected);
}

/// Ctrl+F6 walks the areas in reading order — arrangement, inspector, the lower zone —
/// the focused tab lit, and the arrows' focus follows to the mixer.
#[test]
fn the_keyboard_moves_between_the_areas_in_reading_order() {
    let mut r = rig();
    let order: Vec<Pane> = r
        .app
        .workspace
        .areas_in_order()
        .iter()
        .map(|a| a.0)
        .collect();
    assert_eq!(
        order,
        vec![Pane::Arrangement, Pane::Inspector, Pane::Mixer],
        "left to right, top to bottom"
    );
    let mut seen = Vec::new();
    for _ in 0..order.len() {
        r.key(Key::F6, CTRL);
        seen.push(r.app.workspace.focused_area().unwrap());
    }
    let mut sorted = seen.clone();
    sorted.sort_by_key(|p| order.iter().position(|o| o == p));
    sorted.dedup();
    assert_eq!(sorted.len(), order.len(), "every area once: {seen:?}");
    // Back to the mixer: M and S act on its strips.
    while r.app.workspace.focused_area() != Some(Pane::Mixer) {
        r.key(Key::F6, ctrl_shift());
    }
    assert_eq!(r.app.focus, cantus_ui::app::EditFocus::Mixer);
}
