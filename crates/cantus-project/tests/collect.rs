//! Audio files on save (ADR 0025 §2): recordings move, imports copy, paths go relative,
//! and the saved project finds its files again.

use cantus_core::{AudioSource, AudioSourceId, Command, Project};
use cantus_project::collect::{self, FileAction};
use std::path::Path;

fn source(p: &mut Project, name: &str, path: &Path, recorded: bool) -> AudioSourceId {
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: name.into(),
        path: path.to_string_lossy().into_owned(),
        sample_rate: 48_000,
        channels: 1,
        frames: 4,
        sha256: None,
        recorded,
    });
    id
}

fn apply(p: &mut Project, paths: &[(AudioSourceId, String)]) {
    for (id, path) in paths {
        Command::SetAudioSourcePath {
            id: *id,
            path: path.clone(),
        }
        .apply(p)
        .expect("relink");
    }
}

#[test]
fn save_moves_a_recording_copies_an_import_and_writes_relative_paths() {
    let scratch = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let take = scratch.path().join("Vocal take 1.wav");
    let loop_ = elsewhere.path().join("Loop.wav");
    std::fs::write(&take, b"take").unwrap();
    std::fs::write(&loop_, b"loop").unwrap();
    let mut p = Project::empty("Song");
    let rec = source(&mut p, "Vocal take 1", &take, true);
    let imp = source(&mut p, "Loop", &loop_, false);

    let file = folder.path().join("Song.cantus");
    let plan = collect::plan(&p, None, &file);
    let actions: Vec<_> = plan.ops.iter().map(|o| o.action).collect();
    assert_eq!(actions, [FileAction::Move, FileAction::Copy]);
    let out = collect::execute(&p, &plan);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    apply(&mut p, &out.paths);

    assert!(!take.exists(), "the recording was moved");
    assert!(loop_.exists(), "the import's original stays");
    assert_eq!(p.audio_source(rec).unwrap().path, "Audio/Vocal take 1.wav");
    assert_eq!(p.audio_source(imp).unwrap().path, "Audio/Loop.wav");

    // Saved and reopened, the paths resolve to the files from the project's folder.
    cantus_project::save(&p, &file, "test").unwrap();
    let back = cantus_project::load(&file, true).unwrap().project;
    for s in &back.audio_pool {
        let at = collect::resolve(file.parent(), &s.path);
        assert!(at.is_file(), "{} resolves to {}", s.path, at.display());
    }
    assert_eq!(
        std::fs::read(folder.path().join("Audio/Vocal take 1.wav")).unwrap(),
        b"take"
    );

    // Saving again changes nothing: the files are home.
    let again = collect::plan(&back, file.parent(), &file);
    assert!(again.ops.iter().all(|o| o.action == FileAction::Keep));
    assert!(collect::execute(&back, &again).paths.is_empty());
}

#[test]
fn names_are_made_unique_and_one_file_is_copied_once() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let one = a.path().join("Kick.wav");
    let two = b.path().join("Kick.wav");
    std::fs::write(&one, b"one").unwrap();
    std::fs::write(&two, b"two").unwrap();
    let mut p = Project::empty("Song");
    let s1 = source(&mut p, "Kick", &one, false);
    let s2 = source(&mut p, "Kick", &two, false);
    let s3 = source(&mut p, "Kick again", &one, false);
    let file = folder.path().join("Song.cantus");
    let plan = collect::plan(&p, None, &file);
    let out = collect::execute(&p, &plan);
    assert!(out.errors.is_empty());
    apply(&mut p, &out.paths);
    assert_eq!(p.audio_source(s1).unwrap().path, "Audio/Kick.wav");
    assert_eq!(p.audio_source(s2).unwrap().path, "Audio/Kick (2).wav");
    assert_eq!(p.audio_source(s3).unwrap().path, "Audio/Kick.wav");
    assert_eq!(out.moved.len(), 2, "the shared file is copied once");
}

#[test]
fn save_as_to_another_folder_copies_the_audio_along() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(first.path().join("Audio")).unwrap();
    std::fs::write(first.path().join("Audio/Take.wav"), b"t").unwrap();
    let mut p = Project::empty("Song");
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "Take".into(),
        path: "Audio/Take.wav".into(),
        sample_rate: 48_000,
        channels: 1,
        frames: 1,
        sha256: None,
        recorded: true,
    });
    let file = second.path().join("Copy.cantus");
    let plan = collect::plan(&p, Some(first.path()), &file);
    // A recording inside the old project's folder belongs to that project: copied.
    assert_eq!(plan.ops[0].action, FileAction::Copy);
    let out = collect::execute(&p, &plan);
    assert!(out.errors.is_empty());
    assert!(first.path().join("Audio/Take.wav").exists());
    assert!(second.path().join("Audio/Take.wav").exists());
    // The relative path happens to be the same, and now means the new folder's file.
    assert!(out.paths.is_empty());
}

#[test]
fn a_missing_file_is_left_alone_and_kept_findable() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let mut p = Project::empty("Song");
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "Gone".into(),
        path: "Audio/Gone.wav".into(),
        sample_rate: 48_000,
        channels: 1,
        frames: 1,
        sha256: None,
        recorded: false,
    });
    // Same folder: nothing to do.
    let same = collect::plan(&p, Some(first.path()), &first.path().join("Song.cantus"));
    assert!(same.ops.is_empty());
    assert_eq!(collect::execute(&p, &same).paths, vec![]);
    // Another folder: the relative path is written absolute, to where it was expected.
    let plan = collect::plan(&p, Some(first.path()), &second.path().join("Song.cantus"));
    let out = collect::execute(&p, &plan);
    assert_eq!(out.paths.len(), 1);
    assert_eq!(
        Path::new(&out.paths[0].1),
        first.path().join("Audio/Gone.wav")
    );
}
