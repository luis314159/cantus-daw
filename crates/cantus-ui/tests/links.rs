//! Linked clips in the app (ADR 0037): Duplicate, the Alt-drag and paste follow the
//! "copies are linked" setting; Make unique, Link to and Select linked from the actions the
//! menus and keys run; an edit through one copy reaches all of them; glue keeps every note.

mod support;

use cantus_core::*;
use cantus_ui::keymap::{Action, Binding, Keymap};
use cantus_ui::{starter_project, CantusApp};
use support::{app, ctx};

fn clip_of(a: &CantusApp, r: (TrackId, ClipId)) -> Clip {
    a.project().track(r.0).unwrap().clip(r.1).unwrap().clone()
}

fn normalised(p: &Project) -> Project {
    let mut p = p.clone();
    p.next_id = 0;
    p
}

fn first(a: &CantusApp) -> (TrackId, ClipId) {
    (a.project().tracks[0].id, a.project().tracks[0].clips[0].id)
}

/// The clip Duplicate just made (the selection after it).
fn duplicate(a: &mut CantusApp, r: (TrackId, ClipId)) -> (TrackId, ClipId) {
    a.focus_arrangement();
    a.select_clip(r.0, r.1);
    a.run_action(Action::Duplicate);
    let sel = a.clip_selection();
    assert_eq!(sel.len(), 1);
    assert_ne!(sel[0], r);
    sel[0]
}

#[test]
fn duplicate_makes_the_same_clip_and_an_edit_through_the_copy_reaches_the_original() {
    let mut a = app(starter_project());
    assert!(a.settings.link_copies, "copies are linked by default");
    let orig = first(&a);
    let copy = duplicate(&mut a, orig);
    let (o, c) = (clip_of(&a, orig), clip_of(&a, copy));
    assert!(o.link.is_some());
    assert_eq!(o.link, c.link);
    assert_eq!(c.start, o.end());
    // Transpose every note of the copy, as the piano roll does with the copy open.
    a.select_clip(copy.0, copy.1);
    a.focus_piano_roll();
    a.selected_notes = c.notes.iter().map(|n| n.id).collect();
    a.transpose_selection(12);
    let (o2, c2) = (clip_of(&a, orig), clip_of(&a, copy));
    assert_eq!(o2.notes, c2.notes);
    for (before, after) in o.notes.iter().zip(&o2.notes) {
        assert_eq!(after.pitch, before.pitch + 12);
    }
}

#[test]
fn with_linking_off_duplicate_makes_a_clip_of_its_own() {
    let mut a = app(starter_project());
    a.settings.link_copies = false;
    let orig = first(&a);
    let copy = duplicate(&mut a, orig);
    let (o, c) = (clip_of(&a, orig), clip_of(&a, copy));
    assert_eq!((o.link, c.link), (None, None));
    assert!(c.notes.iter().all(|n| o.note(n.id).is_none()));
}

#[test]
fn make_unique_detaches_the_selection_in_one_undo_step() {
    let mut a = app(starter_project());
    let orig = first(&a);
    let copy = duplicate(&mut a, orig);
    let before = normalised(a.project());
    a.select_clip(copy.0, copy.1);
    a.run_action(Action::MakeUnique);
    assert_eq!(clip_of(&a, copy).link, None);
    assert_eq!(a.bus.undo_label_key(), Some("cmd.make_unique"));
    // Its edits are its own now.
    a.rename_clip(copy, "B part".into());
    assert_ne!(clip_of(&a, orig).name, "B part");
    a.undo();
    a.undo();
    assert_eq!(normalised(a.project()), before);
}

#[test]
fn link_to_makes_the_other_selected_clips_the_edited_clip() {
    let mut a = app(starter_project());
    let orig = first(&a);
    // A second clip of its own, with other notes.
    a.settings.link_copies = false;
    let other = duplicate(&mut a, orig);
    let n = clip_of(&a, other).notes[0].clone();
    a.exec(Command::SetNotes {
        track: other.0,
        clip: other.1,
        notes: vec![Note { pitch: 20, ..n }],
    });
    assert_ne!(clip_of(&a, other).notes, clip_of(&a, orig).notes);
    let before = normalised(a.project());
    // The edited clip (the piano roll's) is the original; the other joins it.
    a.select_clip(orig.0, orig.1);
    a.set_clip_selection(vec![orig, other]);
    a.selected_clip = Some(orig.1);
    a.run_action(Action::LinkClips);
    let (o, x) = (clip_of(&a, orig), clip_of(&a, other));
    assert!(o.link.is_some());
    assert_eq!(o.link, x.link);
    assert_eq!(o.notes, x.notes);
    assert_eq!(a.bus.undo_label_key(), Some("cmd.link_clips"));
    a.undo();
    assert_eq!(normalised(a.project()), before);
}

#[test]
fn select_linked_selects_every_copy() {
    let mut a = app(starter_project());
    let orig = first(&a);
    let b = duplicate(&mut a, orig);
    let c = duplicate(&mut a, b);
    a.select_clip(orig.0, orig.1);
    a.run_action(Action::SelectLinked);
    let mut sel = a.clip_selection();
    sel.sort();
    let mut all = vec![orig, b, c];
    all.sort();
    assert_eq!(sel, all);
}

/// Two linked copies side by side glue into one clip holding both copies' notes; the
/// third copy keeps its own.
#[test]
fn glue_of_linked_copies_keeps_every_note_and_leaves_the_rest_of_the_group() {
    let mut a = app(starter_project());
    let orig = first(&a);
    let b = duplicate(&mut a, orig);
    let c = duplicate(&mut a, b);
    let notes = clip_of(&a, orig).notes.len();
    let third = clip_of(&a, c);
    a.set_clip_selection(vec![orig, b]);
    a.run_action(Action::GlueClips);
    let glued = clip_of(&a, orig);
    assert_eq!(
        glued.notes.len(),
        2 * notes,
        "a copy's notes were taken for duplicates"
    );
    assert_eq!(glued.link, None, "the glued clip has content of its own");
    let after = clip_of(&a, c);
    assert_eq!(after.notes, third.notes);
    let ids: std::collections::HashSet<NoteId> = glued.notes.iter().map(|n| n.id).collect();
    assert_eq!(
        ids.len(),
        glued.notes.len(),
        "note ids are unique in the glued clip"
    );
}

/// A paste follows the setting too: linked, it joins the copied clip's group; with
/// linking off, it is a clip of its own. The clipboard text is version 2.
#[test]
fn paste_follows_the_setting() {
    let mut a = app(starter_project());
    let ctx = ctx();
    let orig = first(&a);
    a.focus_arrangement();
    a.select_clip(orig.0, orig.1);
    let frame = |a: &mut CantusApp, events: Vec<egui::Event>| -> egui::FullOutput {
        let mut ev = vec![egui::Event::ModifiersChanged(egui::Modifiers::COMMAND)];
        ev.extend(events);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            events: ev,
            time: Some(ctx.input(|i| i.time) + 1.0 / 60.0),
            predicted_dt: 1.0 / 60.0,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| a.frame(ui));
        out.textures_delta.clear();
        out
    };
    let out = frame(&mut a, vec![egui::Event::Copy]);
    let text = out
        .platform_output
        .commands
        .iter()
        .find_map(|c| match c {
            egui::OutputCommand::CopyText(t) => Some(t.clone()),
            _ => None,
        })
        .expect("copied");
    assert!(text.starts_with("{\"format\":\"cantus-clip/2\""));
    a.seek(Ticks::beats(16));
    frame(&mut a, vec![egui::Event::Paste(text.clone())]);
    let pasted = a.clip_selection()[0];
    assert_ne!(pasted, orig);
    assert!(clip_of(&a, orig).link.is_some());
    assert_eq!(clip_of(&a, pasted).link, clip_of(&a, orig).link);
    a.settings.link_copies = false;
    a.seek(Ticks::beats(32));
    frame(&mut a, vec![egui::Event::Paste(text)]);
    let unique = a.clip_selection()[0];
    assert_eq!(clip_of(&a, unique).link, None);
}

#[test]
fn the_link_keys_and_labels_are_there() {
    let map = Keymap::default();
    assert!(map
        .bindings_for(Action::MakeUnique)
        .contains(&Binding::alt(egui::Key::U)));
    assert!(map
        .bindings_for(Action::LinkClips)
        .contains(&Binding::alt(egui::Key::L)));
    assert!(map.conflicts().is_empty(), "{:?}", map.conflicts());
    use cantus_ui::i18n::{set_locale, t, Locale};
    let keys = [
        "cmd.make_unique",
        "cmd.link_clip",
        "cmd.link_clips",
        "cmd.set_clip_content",
        "action.make-unique",
        "action.link-clips",
        "action.select-linked",
        "clip.make_unique",
        "clip.link_here",
        "clip.select_linked",
        "clip.link_key",
        "links.copies",
        "links.status.made_unique",
        "links.status.not_linked",
        "links.status.select_two",
        "links.status.nothing_to_link",
        "links.status.linked",
        "links.status.copies_linked",
        "links.status.copies_unique",
    ];
    set_locale(Locale::En);
    let english: Vec<String> = keys.iter().map(|k| t(k)).collect();
    set_locale(Locale::Es);
    for (k, en) in keys.iter().zip(&english) {
        assert_ne!(en, k, "{k} has no English text");
        assert_ne!(&t(k), en, "{k} has no Spanish text");
    }
    set_locale(Locale::En);
    // Every command's label resolves: the core's own keys for the new commands.
    let cmd = Command::SetClipLink {
        track: TrackId(1),
        clip: ClipId(2),
        link: None,
    };
    assert_ne!(t(cmd.label_key()), cmd.label_key());
}

/// The toggle in the arrangement's menu flips the setting and keeps it.
#[test]
fn the_setting_is_switched_and_kept() {
    let mut a = app(starter_project());
    let dir = std::env::temp_dir().join(format!("cantus-links-{}", std::process::id()));
    a.settings_path = dir.join("settings.json");
    a.toggle_link_copies();
    assert!(!a.settings.link_copies);
    let kept = cantus_ui::settings::Settings::load(&a.settings_path);
    assert!(!kept.link_copies);
    a.toggle_link_copies();
    assert!(a.settings.link_copies);
    let _ = std::fs::remove_dir_all(dir);
}

/// What `track` plays: every note-on, as (tick, pitch, velocity), in order.
fn heard(a: &CantusApp, track: TrackId) -> Vec<(Ticks, u8, u8)> {
    let mut out: Vec<_> = Snapshot::compile(a.project(), 1)
        .track(track)
        .unwrap()
        .events
        .iter()
        .filter(|e| e.is_on())
        .map(|e| (e.tick, e.pitch, e.velocity))
        .collect();
    out.sort_unstable();
    out
}

/// A track with one four-beat clip holding a note on each beat (pitches 60…63).
fn pattern(linking: bool) -> (CantusApp, (TrackId, ClipId)) {
    let mut p = Project::new("glue");
    let mut t = p.make_track("Lead");
    let mut c = p.make_clip("Riff", Ticks::ZERO, Ticks::beats(4));
    for i in 0..4 {
        c.notes
            .push(p.make_note(Ticks::beats(i), Ticks::QUARTER, 60 + i as u8, 90));
    }
    let r = (t.id, c.id);
    t.clips.push(c);
    p.tracks.push(t);
    let mut a = app(p);
    a.settings.link_copies = linking;
    (a, r)
}

/// Glue edited halves of a split (no link anywhere): the left half's edit also reached
/// its hidden copy of the right half's notes. The glued clip plays what was heard — the
/// right half's notes once, as they sounded — never each of them twice.
#[test]
fn glue_of_edited_split_halves_plays_what_was_heard() {
    let (mut a, r) = pattern(false);
    a.focus_arrangement();
    a.select_clip(r.0, r.1);
    a.seek(Ticks::beats(2));
    a.run_action(Action::SplitClip);
    let right = a
        .project()
        .track(r.0)
        .unwrap()
        .clips
        .iter()
        .find(|c| c.id != r.1)
        .unwrap()
        .id;
    // Every note of the left half (hidden ones too) one velocity louder.
    let notes: Vec<Note> = clip_of(&a, r)
        .notes
        .iter()
        .map(|n| Note {
            velocity: 100,
            ..n.clone()
        })
        .collect();
    assert!(a.exec(Command::SetNotes {
        track: r.0,
        clip: r.1,
        notes,
    }));
    let before = heard(&a, r.0);
    assert_eq!(before.len(), 4);
    a.set_clip_selection(vec![r, (r.0, right)]);
    a.run_action(Action::GlueClips);
    assert_eq!(a.project().track(r.0).unwrap().clips.len(), 1);
    assert_eq!(heard(&a, r.0), before, "glue changed what is heard");
}

/// Glue a linked copy trimmed from the left onto the clip before it: the copy's hidden
/// notes (before its window) must not land on the first clip's own notes.
#[test]
fn glue_of_a_left_trimmed_linked_copy_plays_what_was_heard() {
    let (mut a, r) = pattern(true);
    let copy = duplicate(&mut a, r);
    assert!(clip_of(&a, copy).link.is_some());
    // Trim the copy's left edge two beats in: it shows content beats 2 and 3 only.
    let c = clip_of(&a, copy);
    let cmds = cantus_ui::clips::set_window(
        copy.0,
        &c,
        c.start + Ticks::beats(2),
        c.length - Ticks::beats(2),
    );
    assert!(a.exec(Command::Batch {
        label_key: "cmd.trim".into(),
        commands: cmds,
    }));
    // Close the gap so the copy starts where the first clip ends.
    let c = clip_of(&a, copy);
    assert!(a.exec(Command::SetClipBounds {
        track: copy.0,
        clip: copy.1,
        start: Ticks::beats(4),
        length: c.length,
    }));
    let before = heard(&a, r.0);
    assert_eq!(before.len(), 6);
    a.set_clip_selection(vec![r, copy]);
    a.run_action(Action::GlueClips);
    assert_eq!(heard(&a, r.0), before, "glue changed what is heard");
}

/// The piano roll's Duplicate (Ctrl+B on notes) past the end of a linked copy grows every
/// copy that showed what it showed, so the new notes are heard in each; one undo step.
#[test]
fn duplicating_notes_past_a_linked_copy_grows_every_copy() {
    let (mut a, r) = pattern(true);
    let copy = duplicate(&mut a, r);
    let before = normalised(a.project());
    a.select_clip(copy.0, copy.1);
    a.focus_piano_roll();
    a.selected_notes = clip_of(&a, copy).notes.iter().map(|n| n.id).collect();
    a.duplicate_selection();
    let (o, c) = (clip_of(&a, r), clip_of(&a, copy));
    assert_eq!(c.length, Ticks::beats(8));
    assert_eq!(o.length, c.length, "the original grew with the copy");
    assert_eq!(heard(&a, r.0).len(), 16, "eight notes in each copy");
    a.undo();
    assert_eq!(normalised(a.project()), before);
}

/// The clip menu's "Link selection to this clip" shows Alt+L beside it, so the key must
/// reach the same clip: a right-click on a selected clip makes it the edited clip.
#[test]
fn the_clip_menu_and_its_key_link_to_the_clip_right_clicked() {
    let (mut a, r) = pattern(false);
    let b = duplicate(&mut a, r);
    // b's notes differ from the original's.
    let notes: Vec<Note> = clip_of(&a, b)
        .notes
        .iter()
        .map(|n| Note {
            pitch: n.pitch + 12,
            ..n.clone()
        })
        .collect();
    assert!(a.exec(Command::SetNotes {
        track: b.0,
        clip: b.1,
        notes,
    }));
    let wanted = clip_of(&a, r).notes;
    // The original and b selected, b the edited clip (as after a Shift-click on it).
    a.set_clip_selection(vec![b, r]);
    assert_eq!(a.selected_clip, Some(b.1));
    // Right-click the original, close the menu, press Alt+L.
    cantus_ui::views::arrangement::clip_menu_opened(&mut a, r.0, r.1);
    assert_eq!(a.clip_selection().len(), 2, "the selection is kept");
    assert_eq!(a.selected_clip, Some(r.1));
    a.run_action(Action::LinkClips);
    let (o, c) = (clip_of(&a, r), clip_of(&a, b));
    assert_eq!(o.notes, wanted, "the clip right-clicked keeps its notes");
    assert_eq!(c.notes, wanted);
    assert!(o.link.is_some() && o.link == c.link);
}

/// A project whose link group no longer matches (a file edited by hand) opens with the odd
/// clip unlinked — and the status line says so, since editing it no longer changes the
/// copies it was linked to.
#[test]
fn opening_a_project_with_a_repaired_group_says_so() {
    let (mut a, r) = pattern(true);
    let copy = duplicate(&mut a, r);
    let mut broken = a.project().clone();
    let t = broken.track_mut(copy.0).unwrap();
    t.clip_mut(copy.1).unwrap().name = "edited by hand".into();
    let dir = std::env::temp_dir().join(format!("cantus-links-open-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("broken.cantus");
    cantus_project::save(&broken, &path, "test").unwrap();
    a.open(&path);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(clip_of(&a, copy).link.is_none());
    assert_eq!(a.status.text, cantus_ui::links::repaired_text(1));
    assert!(a.status.text.contains("unlinked"), "{}", a.status.text);
}
