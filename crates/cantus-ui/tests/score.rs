//! The score through the application (round 32, ADR 0075): the staff under the piano roll
//! and the Score pane draw and cache their engraving, a click selects with the piano
//! roll's selection, the keys edit on the staff (each an undoable command), the settings
//! are commands, MusicXML and PDF go out and MusicXML comes back in as a clip, keymap 13
//! gives an older keymap Alt+S, and a 200-bar part draws within the frame budget.

mod support;

use cantus_core::notation::{ReadingGrid, ScoreSettings, StaffChoice};
use cantus_core::{Command, Key, Note, Project, ScaleKind, Ticks};
use cantus_ui::keymap::Action;
use cantus_ui::score::Scope;
use cantus_ui::views::workspace::Pane;
use cantus_ui::CantusApp;

const Q: i64 = 960;

/// A project with one track and one clip holding a C major scale of quarters.
fn project(bars: i64, per_bar: i64) -> Project {
    let mut p = Project::new("Score");
    p.key = Some(Key::new(0, ScaleKind::Major));
    let mut t = p.make_track("Flute");
    let mut c = p.make_clip("Line", Ticks(0), Ticks(bars * 4 * Q));
    let scale = [60u8, 62, 64, 65, 67, 69, 71, 72];
    let step = 4 * Q / per_bar;
    for i in 0..bars * per_bar {
        let n = p.make_note(Ticks(i * step), Ticks(step), scale[(i % 8) as usize], 90);
        c.notes.push(n);
    }
    t.clips.push(c);
    p.tracks.push(t);
    p
}

fn select_first(app: &mut CantusApp) -> (cantus_core::TrackId, cantus_core::ClipId) {
    let t = app.project().tracks.last().unwrap().id;
    let c = app.project().tracks.last().unwrap().clips[0].id;
    app.selected_track = Some(t);
    app.selected_clip = Some(c);
    (t, c)
}

#[test]
fn the_split_and_the_pane_draw_their_engraving() {
    let mut app = support::app(project(4, 4));
    let (t, c) = select_first(&mut app);
    assert!(cantus_ui::score::action(&mut app, Action::ShowScore));
    assert!(app.score.split_open);
    let ctx = support::ctx();
    support::paint(&ctx, egui::vec2(1200.0, 700.0), 2, |ui| {
        cantus_ui::views::piano_roll::show(&mut app, ui);
    });
    assert!(
        app.score.drawn_pass > 0,
        "the staff under the roll was drawn"
    );
    let notes = app.project().track(t).unwrap().clip(c).unwrap().notes.len();
    let heads = app
        .score
        .split_cache
        .as_ref()
        .map(|k| {
            k.engraved.pages[0]
                .list
                .items
                .iter()
                .filter(|i| {
                    matches!(
                        i,
                        cantus_score::Item::Glyph {
                            glyph: cantus_score::Glyph::NoteheadBlack,
                            ..
                        }
                    )
                })
                .count()
        })
        .unwrap_or(0);
    assert_eq!(heads, notes);
    // The pane, from the View menu's action.
    assert!(cantus_ui::score::action(&mut app, Action::ViewScore));
    assert!(app.workspace.contains(Pane::Score));
    support::paint(&ctx, egui::vec2(900.0, 600.0), 2, |ui| {
        cantus_ui::views::score_view::show(&mut app, ui);
    });
    assert!(app.score.pane_cache.is_some());
}

#[test]
fn the_settings_are_commands_with_an_undo() {
    let mut app = support::app(project(2, 4));
    let (t, _) = select_first(&mut app);
    let s = ScoreSettings {
        staves: StaffChoice::Grand,
        transpose: 2,
        dynamics: true,
        grid: ReadingGrid::EighthTriplet,
        name: Some("Flauta".into()),
        ..Default::default()
    };
    app.set_track_score(t, s.clone());
    assert_eq!(app.project().track(t).unwrap().score, s);
    let score = cantus_ui::score::score_of(&app, Scope::Track(t));
    assert_eq!(score.parts[0].name, "Flauta");
    assert_eq!(score.parts[0].staves.len(), 2);
    assert_eq!(score.parts[0].key.fifths, 2, "in B♭: written in D");
    app.undo();
    assert!(app.project().track(t).unwrap().score.is_default());
}

#[test]
fn keys_edit_on_the_staff() {
    let mut app = support::app(project(1, 4));
    let (t, c) = select_first(&mut app);
    let first = app.project().track(t).unwrap().clip(c).unwrap().notes[0].clone();
    app.selected_notes = vec![first.id];
    // A step up in C major: C → D; an octave: D → D5.
    app.score_step(1, false);
    let pitch = |app: &CantusApp| {
        app.project()
            .track(t)
            .unwrap()
            .clip(c)
            .unwrap()
            .note(first.id)
            .unwrap()
            .pitch
    };
    assert_eq!(pitch(&app), 62);
    app.score_step(1, true);
    assert_eq!(pitch(&app), 74);
    app.undo();
    app.undo();
    assert_eq!(pitch(&app), 60);
    // Note input: N, a half note, then G — after the selected note, the octave nearest.
    cantus_ui::views::score_bar::toggle_entry(&mut app, t, c);
    let mut e = app.score.entry.unwrap();
    assert_eq!(e.at, Q);
    e.dur = cantus_score::model::Dur::new(1, 0);
    app.score.entry = Some(e);
    assert!(app.score_enter(4));
    let clip = app.project().track(t).unwrap().clip(c).unwrap();
    let n: &Note = clip
        .notes
        .iter()
        .find(|n| n.start == Ticks(Q) && n.length == Ticks(2 * Q))
        .unwrap();
    assert_eq!(
        n.pitch, 55,
        "G3: a fourth below C4 is nearer than a fifth above"
    );
    assert_eq!(app.score.entry.unwrap().at, 3 * Q);
    // Delete it again.
    app.selected_notes = vec![n.id];
    app.score_delete();
    assert!(!app
        .project()
        .track(t)
        .unwrap()
        .clip(c)
        .unwrap()
        .notes
        .iter()
        .any(|m| m.length == Ticks(2 * Q)));
}

#[test]
fn musicxml_and_pdf_go_out_and_musicxml_comes_back_in() {
    let mut app = support::app(project(2, 8));
    let (t, c) = select_first(&mut app);
    let dir = tempfile::tempdir().unwrap();
    let xml = dir.path().join("line.musicxml");
    assert!(app.export_musicxml_to(Scope::Clip(t, c), &xml));
    let pdf = dir.path().join("line.pdf");
    assert!(app.export_pdf_to(Scope::Track(t), cantus_score::Paper::A4, &pdf));
    assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF-"));
    let before = app.project().tracks.len();
    assert!(app.import_musicxml_from(&xml));
    assert_eq!(app.project().tracks.len(), before + 1);
    let orig: Vec<(Ticks, Ticks, u8)> = app
        .project()
        .track(t)
        .unwrap()
        .clip(c)
        .unwrap()
        .notes
        .iter()
        .map(|n| (n.start, n.length, n.pitch))
        .collect();
    let back = app.project().tracks.last().unwrap();
    let got: Vec<(Ticks, Ticks, u8)> = back.clips[0]
        .notes
        .iter()
        .map(|n| (n.start, n.length, n.pitch))
        .collect();
    assert_eq!(got, orig);
    // One undo step takes the import away.
    app.undo();
    assert_eq!(app.project().tracks.len(), before);
}

#[test]
fn keymap_13_gives_an_older_keymap_show_score() {
    let old = "version = 12\npreset = \"cantus\"\n";
    let map = cantus_ui::keymap::Keymap::from_toml(old).expect("reads");
    assert!(!map.bindings_for(Action::ShowScore).is_empty());
    assert!(!map.bindings_for(Action::ViewScore).is_empty());
    assert_eq!(cantus_ui::keymap::FILE_VERSION, 13);
}

#[test]
fn a_200_bar_part_draws_within_the_frame_budget() {
    let mut app = support::app(project(200, 8));
    select_first(&mut app);
    app.score.split_open = true;
    let ctx = support::ctx();
    // The first frame engraves (once per edit); the frames after only paint.
    let t0 = std::time::Instant::now();
    let painted = support::paint(&ctx, egui::vec2(1400.0, 800.0), 1, |ui| {
        cantus_ui::views::piano_roll::show(&mut app, ui);
    });
    let first = t0.elapsed();
    // The first frame engraves *and* paints, as in the app (the music font is in from
    // boot), so the frames timed below are cached ones, not a late first paint.
    assert!(
        painted
            .chars()
            .any(|c| ('\u{E000}'..='\u{F8FF}').contains(&c)),
        "the first frame painted no music glyph"
    );
    let mut worst = std::time::Duration::ZERO;
    for _ in 0..5 {
        let t = std::time::Instant::now();
        support::paint(&ctx, egui::vec2(1400.0, 800.0), 1, |ui| {
            cantus_ui::views::piano_roll::show(&mut app, ui);
        });
        worst = worst.max(t.elapsed());
    }
    println!("200 bars: first frame {first:?}, then at most {worst:?}");
    assert!(
        first.as_secs_f64() < 2.0,
        "engraving 200 bars took {first:?}"
    );
    assert!(worst.as_secs_f64() < 0.030, "a cached frame took {worst:?}");
    let _ = Command::SetProjectName {
        name: String::new(),
    };
}

#[test]
fn the_fonts_the_app_starts_with_hold_the_music_font() {
    // The boot path installs `font_fallback::definitions` (`design::install_fonts`): the
    // music family is in it, with Bravura's bytes, so the shipped app draws the score.
    // (`font_fallback::install_for_window` is the boot stage's; it enables the music font.)
    cantus_ui::font_fallback::enable_music();
    let defs = cantus_ui::font_fallback::definitions();
    let fam = defs
        .families
        .get(&cantus_ui::design::family_music())
        .expect("the music family");
    let key = fam.first().expect("a face");
    assert_eq!(defs.font_data[key].font.len(), cantus_score::BRAVURA.len());
    let w = cantus_score::text::TextMeasure::new(Some(cantus_ui::design::score_faces())).width(
        "Score",
        2.0,
        cantus_score::text::Face::Regular,
    );
    assert!(w > 0.0);
}

/// Feed one key press through `global_shortcuts`, as a keyboard would.
fn press(ctx: &egui::Context, a: &mut CantusApp, key: egui::Key, modifiers: egui::Modifiers) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1200.0, 800.0),
        )),
        events: vec![
            egui::Event::ModifiersChanged(modifiers),
            egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            },
        ],
        ..Default::default()
    };
    ctx.run_ui(input, |ui| {
        // The staff was drawn in this very pass (its keys work only while on screen).
        a.score.drawn_pass = ui.ctx().cumulative_pass_nr();
        a.global_shortcuts(ui.ctx());
    })
    .textures_delta
    .clear();
}

/// Review, finding 4: the staff takes its keys with exactly their modifiers — Shift+↑
/// still transposes an octave, Alt+C still toggles the count-in during note input, and
/// Esc outside note input still clears the selection.
#[test]
fn the_staff_takes_only_its_own_keys() {
    let mut app = support::app(project(1, 4));
    let (t, c) = select_first(&mut app);
    let first = app.project().track(t).unwrap().clip(c).unwrap().notes[0].clone();
    app.selected_notes = vec![first.id];
    app.score.focused = true;
    let ctx = support::ctx();
    press(&ctx, &mut app, egui::Key::ArrowUp, egui::Modifiers::SHIFT);
    let pitch = app
        .project()
        .track(t)
        .unwrap()
        .clip(c)
        .unwrap()
        .note(first.id)
        .unwrap()
        .pitch;
    assert_eq!(pitch, 72, "Shift+Up transposes an octave");
    cantus_ui::views::score_bar::toggle_entry(&mut app, t, c);
    let before = app.project().recording.count_in_bars;
    press(&ctx, &mut app, egui::Key::C, egui::Modifiers::ALT);
    assert_ne!(
        app.project().recording.count_in_bars,
        before,
        "Alt+C toggles the count-in"
    );
    let notes = app.project().track(t).unwrap().clip(c).unwrap().notes.len();
    assert_eq!(notes, 4, "Alt+C entered no note");
    app.score.entry = None;
    app.selected_notes = vec![first.id];
    press(&ctx, &mut app, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(app.selected_notes.is_empty(), "Esc clears the selection");
}

/// Review, finding 5: each settings change is its own undo step.
#[test]
fn each_settings_change_is_one_undo_step() {
    use cantus_core::notation::ClefChoice;
    let mut app = support::app(project(1, 4));
    let (t, _) = select_first(&mut app);
    let mut s = app.project().track(t).unwrap().score.clone();
    s.clef = ClefChoice::Bass;
    app.set_track_score(t, s.clone());
    s.clef = ClefChoice::Alto;
    app.set_track_score(t, s);
    app.undo();
    assert_eq!(app.project().track(t).unwrap().score.clef, ClefChoice::Bass);
}

/// Review, finding 2: the file printing hands to the system is named only with safe
/// characters, whatever the track is called (a shell never sees `& ^ % ( )`).
#[test]
fn the_print_file_name_is_safe() {
    for name in [
        "a&b^c%d",
        "x&calc&",
        "Rock&Roll (live)",
        "../../evil",
        "ñandú",
    ] {
        let n = cantus_ui::score::print_name(name);
        assert!(
            n.chars()
                .all(|c| c.is_ascii_alphanumeric() || " _.-".contains(c)),
            "{name} → {n}"
        );
        assert!(n.ends_with(".pdf") && !n.contains(".."), "{n}");
    }
}

/// Review, finding 7: note input stays inside the clip's window, and a transposed part
/// takes its letters as written.
#[test]
fn note_input_stays_in_the_window_and_reads_written_letters() {
    let mut app = support::app(project(1, 4));
    let (t, c) = select_first(&mut app);
    cantus_ui::views::score_bar::toggle_entry(&mut app, t, c);
    let end = app.project().track(t).unwrap().clip(c).unwrap().end().0;
    let mut e = app.score.entry.unwrap();
    e.at = end;
    app.score.entry = Some(e);
    let before = app.project().track(t).unwrap().clip(c).unwrap().notes.len();
    assert!(!app.score_enter(0), "nothing past the window");
    assert_eq!(
        app.project().track(t).unwrap().clip(c).unwrap().notes.len(),
        before
    );
    // A B-flat part: the D typed is the written D, a concert C.
    let mut s = app.project().track(t).unwrap().score.clone();
    s.transpose = 2;
    app.set_track_score(t, s);
    cantus_ui::views::score_bar::toggle_entry(&mut app, t, c);
    let mut e = app.score.entry.unwrap();
    e.at = 0;
    e.last = 67;
    app.score.entry = Some(e);
    assert!(app.score_enter(1));
    let id = app.selected_notes[0];
    let pitch = app
        .project()
        .track(t)
        .unwrap()
        .clip(c)
        .unwrap()
        .note(id)
        .unwrap()
        .pitch;
    assert_eq!(pitch % 12, 0, "written D in B-flat sounds C (got {pitch})");
}

/// Review, finding 9: MusicXML into a song with no notes brings its tempo, metres and
/// key; into one with notes, the song keeps its own.
#[test]
fn an_import_into_an_empty_song_brings_its_tempo_metre_and_key() {
    let mut src = support::app(project(2, 4));
    let (t, _) = select_first(&mut src);
    let dir = tempfile::tempdir().unwrap();
    let xml = dir.path().join("x.musicxml");
    src.exec(Command::SetTempo { bpm: 133.0 });
    src.exec(Command::SetProjectKey {
        key: Some(Key::new(7, ScaleKind::Major)),
    });
    assert!(src.export_musicxml_to(Scope::Track(t), &xml));
    let mut empty = support::app(Project::new("Empty"));
    assert!(empty.import_musicxml_from(&xml));
    assert_eq!(empty.project().tempo_bpm, 133.0);
    assert_eq!(empty.project().key, Some(Key::new(7, ScaleKind::Major)));
    // A song with notes keeps its own.
    let mut full = support::app(project(1, 4));
    let bpm = full.project().tempo_bpm;
    assert!(full.import_musicxml_from(&xml));
    assert_eq!(full.project().tempo_bpm, bpm);
}

#[test]
fn the_default_paper_follows_the_region() {
    use cantus_score::Paper;
    assert_eq!(cantus_ui::score::paper_for_region("en-US"), Paper::LETTER);
    assert_eq!(
        cantus_ui::score::paper_for_region("es_MX.UTF-8"),
        Paper::LETTER
    );
    assert_eq!(cantus_ui::score::paper_for_region("es-ES"), Paper::A4);
    assert_eq!(cantus_ui::score::paper_for_region(""), Paper::A4);
    assert_eq!(
        cantus_ui::score::region_of_reg_output(
            "\r\nHKEY_CURRENT_USER\\Control Panel\\International\r\n    LocaleName    REG_SZ    en-CA\r\n"
        ),
        Some("en-CA".to_string())
    );
}

/// Review 2, finding 2: the print command carries the file's path as data (an
/// environment variable), never spliced into PowerShell's command — a folder with a ’ or
/// a ' in its name prints as it is.
#[test]
fn the_print_command_never_holds_the_path() {
    let path = std::path::Path::new(r"C:\Users\O’Brien's\Temp\cantus-score-1-x.pdf");
    let c = cantus_ui::score::print_command(path);
    assert!(c.args.iter().all(|a| !a.contains("Brien")), "{:?}", c.args);
    let (k, v) = c.env.first().expect("the path as data");
    assert!(c.args.iter().any(|a| a.contains(&format!("$env:{k}"))));
    assert_eq!(std::path::Path::new(v), path);
}
