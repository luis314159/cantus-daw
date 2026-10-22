//! The menu bar's model, its shortcuts and its text edits, and the trackpad's gestures
//! reaching the views (round 30, ADR 0066) — all on every system.

use super::menu::{equivalent, events_for, model, Entry, Equivalent, Pick};
use crate::keymap::{Action, Binding, Keymap, Preset};
use egui::Key;

fn items(keymap: &Keymap) -> Vec<(String, Option<Equivalent>, Option<Pick>)> {
    model(keymap)
        .into_iter()
        .flat_map(|m| m.entries)
        .filter_map(|e| match e {
            Entry::Item { label, pick, key } => Some((label, key, Some(pick))),
            Entry::System { label, key, .. } => Some((label, key, None)),
            Entry::Separator => None,
        })
        .collect()
}

#[test]
fn the_bar_has_the_mac_menus_and_every_label_is_translated() {
    let map = Preset::Mac.keymap();
    let menus = model(&map);
    assert_eq!(menus.len(), 6, "Cantus, File, Edit, View, Window, Help");
    assert!(menus.iter().filter(|m| m.windows).count() == 1);
    assert!(menus.iter().filter(|m| m.help).count() == 1);
    for (label, _, _) in items(&map) {
        assert!(!label.is_empty());
        // A missing string comes back as its key, which always has a dot and no space.
        assert!(
            label.contains(' ') || !label.contains('.'),
            "untranslated: {label}"
        );
    }
}

#[test]
fn items_show_the_keymaps_own_keys_and_move_with_them() {
    let mut map = Preset::Mac.keymap();
    let key = |map: &Keymap, pick: Pick| {
        items(map)
            .into_iter()
            .find(|(_, _, p)| *p == Some(pick))
            .and_then(|(_, k, _)| k)
    };
    let s = key(&map, Pick::Action(Action::Save)).expect("Save has ⌘S");
    assert_eq!((s.chars.as_str(), s.command, s.shift), ("s", true, false));
    let comma = key(&map, Pick::Action(Action::OpenSettings)).expect("Settings has ⌘,");
    assert_eq!(comma.chars, ",");
    let q = key(&map, Pick::Quit).expect("Quit has ⌘Q");
    assert_eq!(q.chars, "q");
    // A remapped Save moves in the menu.
    map.bindings
        .insert(Action::Save, vec![Binding::cmd_shift(Key::W)]);
    let w = key(&map, Pick::Action(Action::Save)).expect("the new key");
    assert_eq!((w.chars.as_str(), w.shift), ("w", true));
    // No two items answer the same chord, and none takes a bare letter.
    let all: Vec<Equivalent> = items(&Preset::Mac.keymap())
        .into_iter()
        .filter_map(|(_, k, _)| k)
        .collect();
    for (i, a) in all.iter().enumerate() {
        assert!(
            a.command || a.alt || a.chars.chars().all(|c| c > '\u{F700}'),
            "{a:?}"
        );
        assert!(!all[i + 1..].contains(a), "{a:?} twice");
    }
}

#[test]
fn key_equivalents_are_appkits_characters() {
    let f5 = equivalent(&Binding::key(Key::F5)).expect("a bare function key");
    assert_eq!(f5.chars, "\u{F708}");
    let left = equivalent(&Binding::cmd(Key::ArrowLeft)).expect("⌘←");
    assert_eq!(left.chars, "\u{F702}");
    assert_eq!(equivalent(&Binding::cmd(Key::Plus)).unwrap().chars, "=");
    assert_eq!(equivalent(&Binding::key(Key::K)), None, "the views' own");
    assert_eq!(equivalent(&Binding::key(Key::ArrowLeft)), None);
    assert_eq!(equivalent(&Binding::cmd(Key::Escape)), None);
    let alt = equivalent(&Binding::alt(Key::M)).expect("⌥M");
    assert!(alt.alt && !alt.command);
}

#[test]
fn a_text_edit_becomes_the_events_its_key_makes() {
    assert_eq!(events_for(Action::Copy, None), vec![egui::Event::Copy]);
    assert_eq!(events_for(Action::Cut, None), vec![egui::Event::Cut]);
    assert_eq!(
        events_for(Action::Paste, Some("hola".into())),
        vec![egui::Event::Paste("hola".into())]
    );
    assert!(events_for(Action::Paste, None).is_empty());
    let undo = events_for(Action::Undo, None);
    assert_eq!(undo.len(), 2, "pressed, then released");
    match &undo[0] {
        egui::Event::Key {
            key: Key::Z,
            pressed: true,
            modifiers,
            ..
        } => assert!(modifiers.command && !modifiers.shift),
        e => panic!("{e:?}"),
    }
    match &events_for(Action::Redo, None)[0] {
        egui::Event::Key { modifiers, .. } => assert!(modifiers.command && modifiers.shift),
        e => panic!("{e:?}"),
    }
    // The queue a menu fills while a field is focused drains into the next pass.
    let mut raw = egui::RawInput::default();
    super::menu::inject(&mut raw);
    assert!(raw.events.is_empty());
}

/// A trackpad's pinch (egui's `Zoom`) zooms a view's time axis and its two-finger swipe
/// (a wheel in points) scrolls it — through the same reading of the wheel every editor
/// uses (`clips_chrome::wheel`).
#[test]
fn trackpad_pinch_and_swipe_reach_the_views() {
    let ctx = egui::Context::default();
    let run = |events: Vec<egui::Event>| {
        let mut got = None;
        let raw = egui::RawInput {
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            got = Some(crate::views::clips_chrome::wheel(ui));
        });
        out.textures_delta.clear();
        got.expect("a pass ran")
    };
    // egui folds a pass's input in at the start of the next: one pass to settle.
    let _ = run(Vec::new());
    let pinch = run(vec![egui::Event::Zoom(1.25)]);
    assert!((pinch.zoom_x - 1.25).abs() < 1e-4, "{}", pinch.zoom_x);
    assert_eq!(pinch.zoom_y, 1.0);
    let swipe = run(vec![egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, -30.0),
        modifiers: egui::Modifiers::NONE,
        phase: egui::TouchPhase::Move,
    }]);
    assert!(
        swipe.scroll.y != 0.0 || swipe.scroll.x != 0.0,
        "{:?}",
        swipe.scroll
    );
}

/// macOS keeps an application's files in `~/Library/Application Support/<name>`.
#[cfg(target_os = "macos")]
#[test]
fn data_lives_in_application_support() {
    if std::env::var_os("CANTUS_DATA_DIR").is_some() {
        return;
    }
    let dir = crate::app::data_dir();
    assert!(
        dir.ends_with("Library/Application Support/Cantus"),
        "{}",
        dir.display()
    );
}
