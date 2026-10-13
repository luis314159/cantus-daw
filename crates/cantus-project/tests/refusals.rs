//! Files the store must refuse cleanly (round 22, soak): a damaged project, one cut off at
//! any byte, one from a newer Cantus, something that is not a project at all — each named
//! for what it is, and none of them touched by the attempt. A save over a damaged file
//! keeps the last good backup; an autosave whose folder cannot be written goes to the
//! fallback folder instead of failing silently.

use cantus_core::{Project, Ticks};
use cantus_project::{
    autosave, backup_path, corrupt_path, load, save, Problem, StoreError, SCHEMA_VERSION,
};
use std::fs;
use std::path::Path;

fn demo() -> Project {
    let mut p = Project::new("Refusals");
    let mut t = p.make_track("Lead");
    let mut c = p.make_clip("c", Ticks::ZERO, Ticks::beats(4));
    for i in 0..8 {
        c.notes
            .push(p.make_note(Ticks(i * 480), Ticks::QUARTER, 60 + i as u8, 100));
    }
    t.clips.push(c);
    p.tracks.push(t);
    p
}

fn problem(path: &Path) -> Problem {
    match load(path, true) {
        Ok(_) => panic!("{} loaded", path.display()),
        Err(e) => e.problem(),
    }
}

#[test]
fn a_file_cut_off_anywhere_is_called_truncated_and_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let good = dir.path().join("good.cantus");
    save(&demo(), &good, "t").unwrap();
    let bytes = fs::read(&good).unwrap();
    let cut = dir.path().join("cut.cantus");
    // Every cut point in the first 256 bytes, then a stride through the rest.
    let points: Vec<usize> = (0..bytes.len().min(256))
        .chain((256..bytes.len()).step_by(211))
        .collect();
    for n in points {
        fs::write(&cut, &bytes[..n]).unwrap();
        let p = problem(&cut);
        // A cut inside a string or a number can still read as complete JSON up to there
        // only at the very end (the closing brace); anything shorter is incomplete.
        assert!(
            p == Problem::Truncated || p == Problem::Damaged,
            "cut at {n}: {p:?}"
        );
        if n < bytes.len() - 2 {
            assert_eq!(p, Problem::Truncated, "cut at {n}");
        }
        assert_eq!(
            fs::read(&cut).unwrap(),
            &bytes[..n],
            "the file is not touched"
        );
    }
    // An empty file — what a power cut during a careless save leaves — is truncated too.
    fs::write(&cut, b"").unwrap();
    assert_eq!(problem(&cut), Problem::Truncated);
}

#[test]
fn damaged_newer_and_foreign_files_are_each_named() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.cantus");
    save(&demo(), &path, "t").unwrap();
    let text = fs::read_to_string(&path).unwrap();

    // Bytes flipped in the middle: no longer JSON.
    let mut bad = text.clone().into_bytes();
    let mid = bad.len() / 2;
    bad[mid..mid + 8].copy_from_slice(b"\x00\x01garb\"{");
    fs::write(&path, &bad).unwrap();
    assert_eq!(problem(&path), Problem::Damaged);

    // Not text at all.
    fs::write(&path, [0xffu8, 0xfe, 0x00, 0x81, 0x9f, 0xc3]).unwrap();
    assert_eq!(problem(&path), Problem::Damaged);

    // Well-formed JSON with the wrong shape inside.
    let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
    v["project"]["tracks"] = serde_json::Value::from("not a list");
    fs::write(&path, serde_json::to_string(&v).unwrap()).unwrap();
    // Its hash no longer matches; opened anyway (as *Open* offers), it is damaged.
    assert_eq!(problem(&path), Problem::Integrity);
    assert_eq!(
        load(&path, false).err().unwrap().problem(),
        Problem::Damaged
    );

    // A newer schema: refused with both numbers, the file left as it is.
    let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
    v["schema_version"] = serde_json::Value::from(SCHEMA_VERSION + 3);
    let newer = serde_json::to_string_pretty(&v).unwrap();
    fs::write(&path, &newer).unwrap();
    assert_eq!(
        problem(&path),
        Problem::Newer {
            found: SCHEMA_VERSION + 3,
            supported: SCHEMA_VERSION
        }
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), newer);

    // Some other program's JSON.
    fs::write(&path, r#"{"name": "a package.json", "version": "1.0.0"}"#).unwrap();
    assert_eq!(problem(&path), Problem::NotAProject);

    // A missing file is an I/O problem, not a damaged one.
    let missing = dir.path().join("nope.cantus");
    assert!(matches!(problem(&missing), Problem::Io(_)));

    // A hand edit that keeps the JSON valid but breaks the hash.
    let edited = text.replacen("\"Lead\"", "\"Lead (edited)\"", 1);
    fs::write(&path, edited).unwrap();
    assert_eq!(problem(&path), Problem::Integrity);
    assert!(
        load(&path, false).is_ok(),
        "and it opens when the user says so"
    );
}

#[test]
fn a_save_over_a_damaged_file_keeps_the_last_good_backup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.cantus");
    let mut a = demo();
    save(&a, &path, "t").unwrap();
    a.tempo_bpm = 97.0;
    save(&a, &path, "t").unwrap();
    // The backup now holds the first save; the file the second. The file is damaged.
    let good_backup = fs::read(backup_path(&path)).unwrap();
    fs::write(&path, b"{ \"format\": \"cantus-project\", \"sche").unwrap();
    a.tempo_bpm = 101.0;
    let r = save(&a, &path, "t").unwrap();
    assert_eq!(r.backup, Some(backup_path(&path)));
    assert_eq!(
        fs::read(backup_path(&path)).unwrap(),
        good_backup,
        "the good backup survived"
    );
    assert_eq!(
        fs::read(corrupt_path(&path)).unwrap(),
        b"{ \"format\": \"cantus-project\", \"sche",
        "the damaged file is kept aside, not deleted"
    );
    assert_eq!(load(&path, true).unwrap().project.tempo_bpm, 101.0);
}

#[test]
fn an_autosave_that_cannot_be_written_beside_the_project_goes_to_the_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("Song");
    fs::create_dir_all(&folder).unwrap();
    let path = folder.join("song.cantus");
    save(&demo(), &path, "t").unwrap();
    let before = fs::read(&path).unwrap();
    // The project's folder goes away and a file takes its name (a drive unplugged and
    // something else mounted there): nothing can be written beside the project.
    fs::remove_dir_all(&folder).unwrap();
    fs::write(&folder, b"not a folder").unwrap();
    let fallback = dir.path().join("autosaves");
    let r = autosave(&demo(), Some(&path), &fallback, "t").unwrap();
    assert!(r.fallback.is_some(), "the reason is reported");
    assert!(r.path.starts_with(&fallback));
    assert!(r
        .path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("song"));
    assert_eq!(load(&r.path, true).unwrap().project, demo());
    // And a real save there fails visibly, leaving nothing half-made.
    fs::remove_file(&folder).unwrap();
    fs::create_dir_all(&folder).unwrap();
    fs::write(&path, &before).unwrap();
    let tmp = folder.join("song.cantus.tmp");
    fs::create_dir_all(&tmp).unwrap(); // the temporary file's name is taken by a folder
    let e = save(&demo(), &path, "t").unwrap_err();
    assert!(matches!(e, StoreError::Io(_)), "{e}");
    assert_eq!(fs::read(&path).unwrap(), before, "the project is untouched");
}
