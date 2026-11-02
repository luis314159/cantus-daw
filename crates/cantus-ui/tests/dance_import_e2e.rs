//! Importing a character end to end (round 35, ADR 0089 §7), on the boot path: a dancer in
//! an untitled project wears an imported Lottie character (one undo step), the first save
//! carries it beside the project, the project opens again and the character draws; *Edit
//! moves…* reaches every dancer that uses it.
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::inst::dancer::CHARACTER_KEY;
use cantus_core::*;
use cantus_dance::import::{package, samples};
use cantus_dance::render::{self, Background, Stage};
use cantus_dance::{pose_at, DanceFrame, Move};
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::dance_import;
use cantus_ui::CantusApp;

fn data() -> &'static std::path::Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

fn assembled() -> CantusApp {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, _proc) = EngineHandle::manual(48_000.0);
    CantusApp::assemble(&ctx, files, None, engine, None, None, None, false)
}

fn dancer(app: &mut CantusApp, index: usize) -> TrackId {
    let track = app.project().tracks[index].id;
    assert!(app.exec(Command::SetInstrument {
        track,
        spec: Box::new(InstrumentSpec::Native(NativeSpec::new(
            InstrumentKind::Dancer
        ))),
    }));
    track
}

fn key(app: &CantusApp, track: TrackId) -> Option<String> {
    app.project()
        .track(track)?
        .instrument
        .native()?
        .text(CHARACTER_KEY)
        .map(str::to_string)
}

#[test]
fn import_use_save_reopen_and_it_draws() {
    let mut app = assembled();
    assert!(app.path.is_none(), "an untitled project");
    let track = dancer(&mut app, 0);
    let src = tempfile::tempdir().unwrap();
    samples::write(src.path(), &samples::dotlottie("Sunny")).unwrap();
    dance_import::begin(&mut app, track, src.path().join("Sunny.lottie"), None);
    dance_import::poll(&mut app, true);
    assert!(app.dance_import.card.is_some());
    let before = key(&app, track);
    assert!(dance_import::use_card(&mut app), "Use");
    // The copy is made off the interface's thread: the card waits, saying so.
    assert!(app.dance_import.card.as_ref().is_some_and(|c| c.busy()));
    assert_eq!(key(&app, track), before);
    dance_import::poll(&mut app, true);
    assert!(app.dance_import.card.is_none(), "closed once done");
    let k = key(&app, track).expect("a character");
    let rel0 = k.strip_prefix("file:").unwrap().to_string();
    assert!(
        rel0.starts_with("Characters/Sunny-") && rel0.ends_with("/Sunny.lottie"),
        "{k}"
    );
    let dir = package::folder_of(&rel0).unwrap();
    // The copy waits in Cantus's folder for untitled work.
    let untitled = dance_import::project_dir(&app);
    assert!(untitled.starts_with(data()));
    assert!(untitled.join(&dir).join("character.json").is_file());
    // One undo step.
    app.undo();
    assert_eq!(key(&app, track), before);
    app.redo();
    assert_eq!(key(&app, track).as_deref(), Some(k.as_str()));
    // The first save carries it beside the project.
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("song.cantus");
    assert!(app.start_save(&file));
    app.wait_for_save();
    assert!(folder.path().join(&rel0).is_file());
    assert!(folder.path().join(&dir).join("character.json").is_file());
    // Opened again: the key, and the character it names, drawn.
    let project = cantus_project::load(&file, true).expect("opens").project;
    let n = project.track(track).unwrap().instrument.native().unwrap();
    let rel = n
        .text(CHARACTER_KEY)
        .unwrap()
        .strip_prefix("file:")
        .unwrap()
        .to_string();
    let ch = cantus_dance::import::load(folder.path(), &rel).expect("loads");
    let f = DanceFrame {
        beats: 2.5,
        seconds: 1.25,
        playing: true,
        bar: 0,
        bar_start_beats: 0.0,
        ..DanceFrame::default()
    };
    let img = render::render(
        &Stage::new(200, 200, Background::Transparent),
        &[(ch.as_ref(), pose_at(&f, ch.as_ref(), 0.0))],
    )
    .unwrap();
    assert!(img.pixels().iter().filter(|p| p.alpha() > 0).count() > 1000);
    // Save As into another folder carries it too.
    let other = tempfile::tempdir().unwrap();
    let file2 = other.path().join("copy.cantus");
    assert!(app.start_save(&file2));
    app.wait_for_save();
    assert!(other.path().join(&rel0).is_file());
}

#[test]
fn edit_moves_reaches_every_dancer_using_the_character() {
    let mut app = assembled();
    let a = dancer(&mut app, 0);
    let b = dancer(&mut app, 1);
    let src = tempfile::tempdir().unwrap();
    samples::write(src.path(), &samples::aseprite("pal")).unwrap();
    for t in [a, b] {
        dance_import::begin(&mut app, t, src.path().join("pal.json"), None);
        dance_import::poll(&mut app, true);
        assert!(dance_import::use_card(&mut app));
        dance_import::poll(&mut app, true);
    }
    assert_eq!(
        key(&app, a),
        key(&app, b),
        "the same file, the same choices: one copy"
    );
    dance_import::edit_moves(&mut app, a);
    dance_import::poll(&mut app, true);
    let card = app.dance_import.card.as_mut().expect("the card, editing");
    assert!(card.edit.is_some());
    let mut s = card.settings.clone();
    s.mapping.moves[Move::Clap.index()] = None;
    card.set_settings(s);
    let before = key(&app, a);
    assert!(dance_import::use_card(&mut app), "Save");
    let dir = dance_import::project_dir(&app);
    // Review 3: a new copy; both dancers re-keyed to it in one undo step.
    assert_ne!(key(&app, a), before);
    assert_eq!(key(&app, a), key(&app, b));
    for t in [a, b] {
        let rel = key(&app, t)
            .unwrap()
            .strip_prefix("file:")
            .unwrap()
            .to_string();
        let ch = cantus_dance::import::load(&dir, &rel).unwrap();
        assert!(!ch.has_move(Move::Clap), "every dancer follows the edit");
    }
    app.undo();
    assert_eq!(key(&app, a), before, "one undo step");
    assert_eq!(key(&app, b), before);
    let old = before.unwrap().strip_prefix("file:").unwrap().to_string();
    assert!(
        cantus_dance::import::load(&dir, &old)
            .unwrap()
            .has_move(Move::Clap),
        "the old copy is there, unchanged"
    );
    let _ = package::FOLDER;
}

/// Review 3: two projects in one folder never collide — the other project's version of
/// the character (other moves) has a folder of its own, the save goes through, and theirs
/// is untouched.
#[test]
fn saving_beside_another_version_of_the_character_keeps_both() {
    let mut app = assembled();
    let track = dancer(&mut app, 2);
    let src = tempfile::tempdir().unwrap();
    samples::write(src.path(), &samples::dotlottie("Sunny")).unwrap();
    dance_import::begin(&mut app, track, src.path().join("Sunny.lottie"), None);
    dance_import::poll(&mut app, true);
    assert!(dance_import::use_card(&mut app));
    dance_import::poll(&mut app, true);
    let before = key(&app, track);
    let rel = before
        .clone()
        .unwrap()
        .strip_prefix("file:")
        .unwrap()
        .to_string();
    let folder = tempfile::tempdir().unwrap();
    let path = src.path().join("Sunny.lottie");
    let found = cantus_dance::import::inspect(&path).unwrap();
    let mut s = cantus_dance::import::guessed(&found);
    s.mapping.moves[Move::Clap.index()] = None;
    let licence = found.licence.clone();
    let theirs_rel = package::copy_into(folder.path(), &path, &found, &s, &licence).unwrap();
    assert_ne!(package::folder_of(&theirs_rel), package::folder_of(&rel));
    let theirs = std::fs::read(folder.path().join(&theirs_rel)).unwrap();
    let file = folder.path().join("mine.cantus");
    assert!(app.start_save(&file));
    app.wait_for_save();
    assert!(file.exists(), "saved");
    assert!(!app.status.error, "{}", app.status.text);
    assert_eq!(key(&app, track), before);
    assert!(folder.path().join(&rel).is_file());
    assert_eq!(
        std::fs::read(folder.path().join(&theirs_rel)).unwrap(),
        theirs
    );
}

/// The folder a copy of `file` made in the untitled folder, by the name it starts with.
fn untitled_folders(app: &CantusApp, prefix: &str) -> Vec<std::path::PathBuf> {
    let dir = dance_import::project_dir(app).join(package::FOLDER);
    std::fs::read_dir(dir)
        .map(|d| {
            d.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with(prefix))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn gone(app: &CantusApp, prefix: &str) -> bool {
    for _ in 0..200 {
        if untitled_folders(app, prefix).is_empty() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    false
}

/// Review 2, item 5: a copy belongs to the project and the dancer it was asked for — another
/// project coming in, *Cancel* while it copies, or the track removed: no key is set on
/// anything, and the folder the copy made is removed.
#[test]
fn a_copy_that_can_no_longer_be_used_is_removed() {
    for (case, prefix) in [
        ("project", "epochpal"),
        ("cancel", "cancelpal"),
        ("track", "trackpal"),
    ] {
        let mut app = assembled();
        let track = dancer(&mut app, 0);
        let src = tempfile::tempdir().unwrap();
        samples::write(src.path(), &samples::aseprite(prefix)).unwrap();
        dance_import::begin(
            &mut app,
            track,
            src.path().join(format!("{prefix}.json")),
            None,
        );
        dance_import::poll(&mut app, true);
        assert!(dance_import::use_card(&mut app));
        match case {
            "project" => {
                app.replace_project(cantus_ui::templates::Starter::Beat.build());
                dance_import::poll(&mut app, false);
                assert!(
                    app.dance_import.card.is_none(),
                    "the card goes with its project"
                );
            }
            "cancel" => dance_import::close(&mut app),
            _ => {
                assert!(app.exec(Command::RemoveTrack { id: track }));
                dance_import::poll(&mut app, true);
                assert!(app.dance_import.card.is_none());
                assert!(app.status.error);
            }
        }
        assert!(gone(&app, prefix), "{case}: the copy is removed");
        let set = app
            .project()
            .tracks
            .iter()
            .filter_map(|t| {
                t.instrument
                    .native()?
                    .text(CHARACTER_KEY)
                    .map(str::to_string)
            })
            .any(|k| k.contains(prefix));
        assert!(!set, "{case}: no key names it");
    }
}

/// Review 3, item 1: a copy that finishes after the project moved folders (an untitled
/// project's first save does not change its epoch) is carried to where the project is now
/// before its key is set.
#[test]
fn a_copy_finished_after_the_first_save_is_carried_beside_the_project() {
    let mut app = assembled();
    let track = dancer(&mut app, 0);
    let src = tempfile::tempdir().unwrap();
    samples::write(src.path(), &samples::aseprite("movedpal")).unwrap();
    dance_import::begin(&mut app, track, src.path().join("movedpal.json"), None);
    dance_import::poll(&mut app, true);
    assert!(dance_import::use_card(&mut app));
    let folder = tempfile::tempdir().unwrap();
    assert!(app.start_save(&folder.path().join("song.cantus")));
    app.wait_for_save();
    dance_import::poll(&mut app, true);
    let rel = key(&app, track)
        .expect("set")
        .strip_prefix("file:")
        .unwrap()
        .to_string();
    assert!(
        folder.path().join(&rel).is_file(),
        "the copy is beside the project"
    );
    assert!(cantus_dance::import::load(folder.path(), &rel).is_ok());
}

/// Review 3, item 6: a new import while a copy is in flight closes the card properly: the
/// copy is abandoned and the folder it made removed.
#[test]
fn a_new_import_abandons_a_copy_in_flight() {
    let mut app = assembled();
    let track = dancer(&mut app, 0);
    let src = tempfile::tempdir().unwrap();
    samples::write(src.path(), &samples::aseprite("leakpal")).unwrap();
    samples::write(src.path(), &samples::lottie("next")).unwrap();
    dance_import::begin(&mut app, track, src.path().join("leakpal.json"), None);
    dance_import::poll(&mut app, true);
    assert!(dance_import::use_card(&mut app));
    dance_import::begin(&mut app, track, src.path().join("next.json"), None);
    assert!(gone(&app, "leakpal"), "the abandoned copy is removed");
}
