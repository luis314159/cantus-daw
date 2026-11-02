//! A character in a project (ADR 0089 §7): the copy, `character.json`, the loader, one
//! mapping shared by every dancer that uses the file, and carrying it to a new folder.

use cantus_dance::import::{self, package, samples, Mapping};
use cantus_dance::{Licence, Move};
use std::path::Path;

fn source_folder() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    samples::write(dir.path(), &samples::aseprite("hero")).unwrap();
    std::fs::write(
        dir.path().join("LICENSE.txt"),
        "Creative Commons Attribution 4.0 International\n\nhttps://creativecommons.org/licenses/by/4.0/\n",
    )
    .unwrap();
    let json = dir.path().join("hero.json");
    (dir, json)
}

/// The folder of a key path (`Characters/<name>-<hash>`).
fn folder(rel: &str) -> String {
    package::folder_of(rel).unwrap()
}

fn imported(project: &Path, src: &Path) -> String {
    let found = import::inspect(src).unwrap();
    let settings = import::guessed(&found);
    let licence = found.licence.clone();
    package::copy_into(project, src, &found, &settings, &licence).unwrap()
}

#[test]
fn use_copies_the_file_its_atlas_and_its_licence_beside_a_package() {
    let (_s, src) = source_folder();
    let project = tempfile::tempdir().unwrap();
    let rel = imported(project.path(), &src);
    // Its folder: the name and the hash of all its content (review 3).
    let dir = project.path().join(folder(&rel));
    let hash = package::read_package(&dir).unwrap().hash;
    assert_eq!(rel, format!("Characters/hero-{}/hero.json", &hash[..8]));
    assert_eq!(package::folder_hash(&dir), Some(hash));
    for f in ["hero.json", "hero.png", "LICENSE.txt", "character.json"] {
        assert!(dir.join(f).is_file(), "{f} copied");
    }
    let p = package::read_package(&dir).unwrap();
    assert_eq!(p.format, "Aseprite");
    assert_eq!(p.sha256.len(), 64);
    assert_eq!(
        p.licence.licence(),
        Licence::Stated {
            text: "CC-BY-4.0".into(),
            source: "LICENSE.txt".into()
        }
    );
    let ch = import::load(project.path(), &rel).unwrap();
    assert_eq!(ch.info().id, format!("file:{rel}"));
    assert_eq!(ch.info().licence, p.licence.licence());
    assert!(ch.has_move(Move::Clap));
}

#[test]
fn the_same_file_twice_is_one_copy_and_another_mapping_is_another() {
    let (_s, src) = source_folder();
    let project = tempfile::tempdir().unwrap();
    let a = imported(project.path(), &src);
    let b = imported(project.path(), &src);
    assert_eq!(a, b, "the same file with the same choices: one copy");
    let found = import::inspect(&src).unwrap();
    let mut s = import::guessed(&found);
    s.mapping.moves[Move::Clap.index()] = Some(Move::Wave.index());
    let c = package::copy_into(project.path(), &src, &found, &s, &found.licence.clone()).unwrap();
    assert_ne!(folder(&c), folder(&a), "other choices: another folder");
    assert!(c.starts_with("Characters/hero-"));
}

#[test]
fn two_dancers_sharing_a_file_both_follow_edit_moves() {
    let (_s, src) = source_folder();
    let project = tempfile::tempdir().unwrap();
    let rel = imported(project.path(), &src);
    let first = import::load(project.path(), &rel).unwrap();
    let second = import::load(project.path(), &rel).unwrap();
    assert!(
        std::sync::Arc::ptr_eq(&first, &second),
        "one character for both"
    );
    assert!(first.has_move(Move::Clap));
    // Edit moves…: Clap gets no clip of its own.
    let dir = project.path().join(folder(&rel));
    let before = std::fs::read(dir.join("character.json")).unwrap();
    let mut settings = package::read_package(&dir).unwrap().settings;
    settings.mapping.moves[Move::Clap.index()] = None;
    // Review 3: a new package beside it; the folder at `rel` is never changed.
    let edited = package::with_settings(project.path(), &rel, &settings).unwrap();
    assert_ne!(edited, rel);
    assert_eq!(std::fs::read(dir.join("character.json")).unwrap(), before);
    assert!(!import::load(project.path(), &edited)
        .unwrap()
        .has_move(Move::Clap));
    assert!(import::load(project.path(), &rel)
        .unwrap()
        .has_move(Move::Clap));
    // The same edit again lands in the same folder; no edit at all keeps the key.
    assert_eq!(
        package::with_settings(project.path(), &rel, &settings).unwrap(),
        edited
    );
    let same = package::read_package(&dir).unwrap().settings;
    assert_eq!(
        package::with_settings(project.path(), &rel, &same).unwrap(),
        rel
    );
}

#[test]
fn a_file_placed_by_hand_loads_with_guessed_moves() {
    let project = tempfile::tempdir().unwrap();
    let dir = project.path().join("Characters/hand");
    samples::write(&dir, &samples::aseprite("hand")).unwrap();
    let ch = import::load(project.path(), "Characters/hand/hand.json").unwrap();
    assert!(ch.has_move(Move::Jump));
    assert_eq!(ch.info().licence, Licence::Unknown);
}

#[test]
fn a_project_cannot_point_outside_its_folder() {
    let project = tempfile::tempdir().unwrap();
    for bad in [
        "../x.json",
        "/etc/x.json",
        "C:/x.json",
        "Characters/../../x.json",
        "http://x/y.json",
    ] {
        assert!(import::load(project.path(), bad).is_err(), "{bad}");
    }
    assert!(import::load(project.path(), "Characters/missing/m.json").is_err());
}

#[test]
fn carrying_copies_the_folders_a_project_uses() {
    let (_s, src) = source_folder();
    let old = tempfile::tempdir().unwrap();
    let rel = imported(old.path(), &src);
    let new = tempfile::tempdir().unwrap();
    let carried = package::carry(old.path(), new.path(), std::slice::from_ref(&rel)).unwrap();
    assert_eq!(carried.copied, vec![folder(&rel)]);
    let ch = import::load(new.path(), &rel).unwrap();
    assert!(ch.has_move(Move::Wave));
    // Carried again: nothing to do; the same folder: nothing to do.
    let again = package::carry(old.path(), new.path(), std::slice::from_ref(&rel)).unwrap();
    assert!(again.copied.is_empty());
    let same = package::carry(new.path(), new.path(), &[rel]).unwrap();
    assert!(same.copied.is_empty());
}

#[test]
fn licences_are_recognised_and_say_what_they_ask() {
    use package::{licence_asks, recognise};
    assert_eq!(recognise("CC0 1.0 Universal"), Some("CC0-1.0"));
    assert_eq!(
        recognise("Attribution-ShareAlike 4.0 International"),
        Some("CC-BY-SA-4.0")
    );
    assert_eq!(
        recognise("Attribution-NonCommercial 4.0"),
        Some("CC-BY-NC-4.0")
    );
    assert_eq!(
        recognise("Permission is hereby granted, free of charge, to any"),
        Some("MIT")
    );
    assert_eq!(recognise("my own words"), None);
    let st = |t: &str| Licence::Stated {
        text: t.into(),
        source: "x".into(),
    };
    assert_eq!(
        licence_asks(&st("CC-BY-4.0")),
        vec!["dance_import.asks.credit"]
    );
    assert_eq!(
        licence_asks(&st("CC-BY-NC-SA-4.0")),
        vec![
            "dance_import.asks.credit",
            "dance_import.asks.share_alike",
            "dance_import.asks.non_commercial"
        ]
    );
    assert!(licence_asks(&st("CC0-1.0")).is_empty());
    assert_eq!(
        licence_asks(&Licence::Unknown),
        vec!["dance_import.asks.unknown"]
    );
    let _ = Mapping::guess(&[]);
}

#[test]
fn folder_names_are_plain() {
    assert_eq!(package::folder_name("My Hero!"), "My_Hero");
    assert_eq!(package::folder_name("../.."), "character");
    assert_eq!(package::folder_name("señor"), "señor");
}

#[test]
fn importing_a_copy_reads_the_choices_kept_beside_it() {
    let (_s, src) = source_folder();
    let project = tempfile::tempdir().unwrap();
    let rel = imported(project.path(), &src);
    let dir = project.path().join(folder(&rel));
    let mut settings = package::read_package(&dir).unwrap().settings;
    settings.mapping.moves[Move::Wave.index()] = None;
    let rel = package::with_settings(project.path(), &rel, &settings).unwrap();
    let ch = import::import(&project.path().join(&rel)).unwrap();
    assert!(!ch.has_move(Move::Wave), "the mapping kept with the copy");
    assert!(matches!(ch.info().licence, Licence::Stated { .. }));
    // The file where it came from (no package beside it) is guessed.
    assert!(import::import(&src).unwrap().has_move(Move::Wave));
}

/// Review 1, item 9: Windows's reserved names never become a folder.
#[test]
fn reserved_windows_names_are_mapped() {
    for n in [
        "con", "CON", "aux", "nul", "prn", "com1", "LPT9", "COM¹", "lpt³", "com²",
    ] {
        let f = package::folder_name(n);
        assert_ne!(f.to_ascii_lowercase(), n.to_ascii_lowercase(), "{n}");
        assert_eq!(f, format!("{n}-character"));
    }
    assert_eq!(package::folder_name("console"), "console");
}

/// Review 3: folders are content-addressed — every file of the package and the choices.
/// The same file and choices make one folder; a recoloured atlas or other moves another;
/// a carry beside a folder of that name holding the same content shares it, beside one a
/// hand edited goes to a fresh name (the key follows), and a missing source is said.
#[test]
fn folders_are_named_by_all_their_content_and_carried_by_it() {
    let (_s, src) = source_folder();
    let ours = tempfile::tempdir().unwrap();
    let rel = imported(ours.path(), &src);
    assert_eq!(
        imported(ours.path(), &src),
        rel,
        "the same content: one folder"
    );
    let dir = ours.path().join(folder(&rel));
    assert_eq!(
        package::folder_hash(&dir),
        Some(package::read_package(&dir).unwrap().hash)
    );
    // The same JSON, its atlas recoloured: another folder.
    let recoloured = tempfile::tempdir().unwrap();
    samples::write(recoloured.path(), &samples::aseprite("hero")).unwrap();
    let png = recoloured.path().join("hero.png");
    let mut img = tiny_skia::Pixmap::load_png(&png).unwrap();
    for p in img.data_mut().chunks_exact_mut(4) {
        p.swap(0, 2);
    }
    img.save_png(&png).unwrap();
    let other = imported(ours.path(), &recoloured.path().join("hero.json"));
    assert_ne!(folder(&other), folder(&rel));
    // Carried beside the very same content: shared, nothing copied, no key moves.
    let theirs = tempfile::tempdir().unwrap();
    imported(theirs.path(), &src);
    let c = package::carry(ours.path(), theirs.path(), std::slice::from_ref(&rel)).unwrap();
    assert!(c.copied.is_empty() && c.renamed.is_empty());
    // Beside a folder of that name a hand edited: ours goes to a fresh name, theirs stays.
    let edited = tempfile::tempdir().unwrap();
    imported(edited.path(), &src);
    let theirs_png = edited.path().join(folder(&rel)).join("hero.png");
    std::fs::write(&theirs_png, b"not what it was").unwrap();
    let c = package::carry(ours.path(), edited.path(), std::slice::from_ref(&rel)).unwrap();
    assert_eq!(c.renamed.len(), 1);
    let (_, moved) = &c.renamed[0];
    assert!(import::load(edited.path(), moved)
        .unwrap()
        .has_move(Move::Clap));
    assert_eq!(std::fs::read(&theirs_png).unwrap(), b"not what it was");
    // A character not where the project keeps it is said, not skipped silently.
    let empty = tempfile::tempdir().unwrap();
    let c = package::carry(empty.path(), theirs.path(), std::slice::from_ref(&rel)).unwrap();
    assert_eq!(c.missing, vec![folder(&rel)]);
}
