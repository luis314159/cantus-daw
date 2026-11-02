//! The import card (round 35, ADR 0089 §7–8): a file is checked off the interface's
//! thread, the card holds what it is and the choices made, a broken file is said in words,
//! the licence comes from the file or the person, and the project's characters are listed.

use cantus_dance::import::{package, samples};
use cantus_dance::{Licence, Move};
use cantus_engine::EngineHandle;
use cantus_ui::dance_import;
use cantus_ui::CantusApp;

fn app() -> CantusApp {
    let project = cantus_ui::templates::Starter::Beat.build();
    CantusApp::from_parts(Some(project), EngineHandle::dummy(), false)
}

#[test]
fn a_lottie_file_opens_the_card_with_its_moves_guessed() {
    let d = tempfile::tempdir().unwrap();
    samples::write(d.path(), &samples::dotlottie("Sunny")).unwrap();
    let mut a = app();
    let track = a.project().tracks[0].id;
    dance_import::begin(&mut a, track, d.path().join("Sunny.lottie"), None);
    assert!(
        dance_import::checking(&a),
        "read off the interface's thread"
    );
    dance_import::poll(&mut a, true);
    let card = a.dance_import.card.as_ref().expect("the card");
    assert_eq!(card.format, "Lottie");
    assert_eq!(card.clips.len(), 16);
    assert_eq!(card.author, "Cantus");
    assert!(matches!(card.licence(), Licence::Stated { .. }));
    for m in Move::ALL {
        assert_eq!(card.settings.mapping.moves[m.index()], Some(m.index()));
    }
    assert!(dance_import::preview(&a).is_some());
}

#[test]
fn the_licence_is_the_files_or_the_persons_choice() {
    let d = tempfile::tempdir().unwrap();
    samples::write(d.path(), &samples::aseprite("pal")).unwrap();
    let mut a = app();
    let track = a.project().tracks[0].id;
    dance_import::begin(&mut a, track, d.path().join("pal.json"), None);
    dance_import::poll(&mut a, true);
    let card = a.dance_import.card.as_mut().unwrap();
    assert_eq!(
        card.licence(),
        Licence::Unknown,
        "nothing stated, nothing chosen"
    );
    card.choice = Some(1);
    assert_eq!(
        card.licence(),
        Licence::Stated {
            text: "CC0-1.0".into(),
            source: package::CHOSEN.into()
        }
    );
    let other = package::CHOICES
        .iter()
        .position(|c| c.0 == "other")
        .unwrap();
    card.choice = Some(other);
    assert_eq!(card.licence(), Licence::Unknown, "other, with no words");
    card.other = "Free for streams, by the artist".into();
    assert!(matches!(card.licence(), Licence::Stated { text, .. } if text.starts_with("Free")));
}

#[test]
fn changing_a_move_changes_the_preview() {
    let d = tempfile::tempdir().unwrap();
    samples::write(d.path(), &samples::aseprite("pal")).unwrap();
    let mut a = app();
    let track = a.project().tracks[0].id;
    dance_import::begin(&mut a, track, d.path().join("pal.json"), None);
    dance_import::poll(&mut a, true);
    let card = a.dance_import.card.as_mut().unwrap();
    let mut s = card.settings.clone();
    s.mapping.moves[Move::Kick.index()] = None;
    card.set_settings(s);
    use cantus_dance::Character;
    assert!(!card.preview.has_move(Move::Kick));
    assert!(card.preview.has_move(Move::Clap));
}

#[test]
fn a_broken_file_is_said_in_words() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("broken.json");
    std::fs::write(
        &p,
        br#"{"layers": [], "v": "5", "fr": 30, "ip": 0, "op": 0}"#,
    )
    .unwrap();
    let mut a = app();
    let track = a.project().tracks[0].id;
    dance_import::begin(&mut a, track, p, None);
    dance_import::poll(&mut a, true);
    assert!(a.dance_import.card.is_none());
    let f = a.dance_import.failed.as_ref().expect("the failure");
    assert_eq!(f.key, "dance_import.error.invalid");
    let words = dance_import::view::failed_words(f);
    assert!(words.contains("length"), "{words}");
    let p = d.path().join("notes.txt");
    std::fs::write(&p, b"hello").unwrap();
    dance_import::begin(&mut a, track, p, None);
    dance_import::poll(&mut a, true);
    assert_eq!(
        a.dance_import.failed.as_ref().unwrap().key,
        "dance_import.error.unsupported"
    );
}

#[test]
fn an_untitled_project_keeps_its_characters_in_cantus_folder() {
    let a = app();
    assert!(a.path.is_none());
    let dir = dance_import::project_dir(&a);
    assert!(dir.ends_with("untitled"), "{}", dir.display());
}

#[test]
fn the_projects_characters_are_listed_by_name() {
    let src = tempfile::tempdir().unwrap();
    samples::write(src.path(), &samples::aseprite("zed")).unwrap();
    samples::write(src.path(), &samples::lottie("Amy")).unwrap();
    let project = tempfile::tempdir().unwrap();
    for f in ["zed.json", "Amy.json"] {
        let p = src.path().join(f);
        let found = cantus_dance::import::inspect(&p).unwrap();
        let s = cantus_dance::import::guessed(&found);
        let l = found.licence.clone();
        package::copy_into(project.path(), &p, &found, &s, &l).unwrap();
    }
    let listed = dance_import::listed_in(project.path());
    let names: Vec<&str> = listed.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, vec!["Amy", "zed"]);
    assert!(listed[0].rel.starts_with("Characters/Amy-") && listed[0].rel.ends_with("/Amy.json"));
    assert_eq!(listed[0].format, "Lottie");
}

#[test]
fn the_macs_file_menu_has_import_character_after_import_audio() {
    use cantus_ui::macos::menu::{model, Entry, Pick};
    let map = cantus_ui::keymap::Preset::Mac.keymap();
    let menus = model(&map);
    let file = &menus[1];
    let picks: Vec<Pick> = file
        .entries
        .iter()
        .filter_map(|e| match e {
            Entry::Item { pick, .. } => Some(*pick),
            _ => None,
        })
        .collect();
    let at = picks.iter().position(|p| *p == Pick::ImportAudio).unwrap();
    assert_eq!(picks.get(at + 1), Some(&Pick::ImportCharacter));
    let label = file.entries.iter().find_map(|e| match e {
        Entry::Item {
            label,
            pick: Pick::ImportCharacter,
            ..
        } => Some(label.clone()),
        _ => None,
    });
    assert_eq!(label, Some(cantus_ui::i18n::t("dance_import.menu")));
    // Chosen with no dancer selected: said in words, nothing opened.
    let mut a = app();
    a.selected_track = Some(a.project().tracks[0].id);
    let ctx = egui::Context::default();
    cantus_ui::macos::menu::run(&mut a, &ctx, Pick::ImportCharacter);
    assert_eq!(
        a.status.text,
        cantus_ui::i18n::t("dance_import.menu.needs_dancer")
    );
    assert!(!dance_import::checking(&a) && a.dance_import.card.is_none());
}
