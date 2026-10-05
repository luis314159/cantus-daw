//! Saving with audio, reopening, and a missing file found again.

use super::{app, empty_app, import, wav};
use crate::support::{ctx, paint};
use cantus_core::{AudioRegion, AudioSource, AudioSourceId, Command, Project, Ticks};
use cantus_ui::views;
use cantus_ui::waveform::Lookup;
use std::path::Path;
use std::time::Duration;

#[test]
fn save_moves_a_recording_copies_an_import_and_reopening_finds_them() {
    let scratch = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let folder = tempfile::tempdir().unwrap();
    let take = scratch.path().join("Take 1.wav");
    let loop_ = elsewhere.path().join("Loop.wav");
    wav(&take, 48_000, 1, 4800, |_, _| 0.3);
    wav(&loop_, 48_000, 2, 4800, |_, _| 0.2);
    let mut a = empty_app();
    let (_, imported) = import(&mut a, &loop_);
    // A recording, as the recorder hands it over (ADR 0025 §2: absolute, recorded).
    let id = AudioSourceId(a.bus.alloc_id());
    let track = a.project().tracks[0].id;
    let clip = {
        let mut c = a.bus.new_clip("Take 1", Ticks::ZERO, Ticks::beats(1));
        c.audio = Some(AudioRegion::of(id));
        c
    };
    a.exec(Command::Batch {
        label_key: "cmd.insert_audio_source".into(),
        commands: vec![
            Command::InsertAudioSource {
                source: AudioSource {
                    id,
                    name: "Take 1".into(),
                    path: take.to_string_lossy().into_owned(),
                    sample_rate: 48_000,
                    channels: 1,
                    frames: 4800,
                    sha256: None,
                    recorded: true,
                },
            },
            Command::InsertClip {
                track,
                clip: Box::new(clip),
            },
        ],
    });
    let file = folder.path().join("Song.cantus");
    a.save_to_path(&file);
    assert!(!a.status.error, "{}", a.status.text);
    assert!(!take.exists(), "the recording moved into the project");
    assert!(loop_.exists(), "the import's original stays where it was");
    let paths: Vec<String> = a
        .project()
        .audio_pool
        .iter()
        .map(|s| s.path.clone())
        .collect();
    assert!(paths.contains(&"Audio/Take 1.wav".to_string()), "{paths:?}");
    assert!(paths.contains(&"Audio/Loop.wav".to_string()), "{paths:?}");
    let _ = imported;

    // Reopened by another session, every source is found from the project's folder.
    let mut b = app(Project::empty("x"));
    b.open(&file);
    assert_eq!(b.project().audio_pool.len(), 2);
    for s in b.project().audio_pool.clone() {
        let at = b.audio_file(&s);
        assert!(at.is_file(), "{}", at.display());
        let _ = b.peaks.lookup(&s, b.audio_base(), None);
    }
    assert!(b.peaks.wait_idle(Duration::from_secs(20)));
    for s in b.project().audio_pool.clone() {
        assert!(
            matches!(b.peaks.lookup(&s, b.audio_base(), None), Lookup::Ready(_)),
            "{} reads",
            s.path
        );
    }
}

fn missing_project(path: &str) -> (Project, AudioSourceId) {
    let mut p = Project::empty("Missing");
    let mut t = p.make_track("Vocal");
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "Vocal".into(),
        path: path.into(),
        sample_rate: 48_000,
        channels: 1,
        frames: 96_000,
        sha256: None,
        recorded: false,
    });
    let mut c = p.make_audio_clip("Vocal", id, Ticks::ZERO, Ticks::beats(4));
    c.audio.as_mut().unwrap().gain_db = -3.0;
    t.clips.push(c);
    p.tracks.push(t);
    (p, id)
}

#[test]
fn a_missing_file_shows_as_missing_and_relinks() {
    let dir = tempfile::tempdir().unwrap();
    let gone = dir.path().join("gone.wav");
    let (p, id) = missing_project(&gone.to_string_lossy());
    let mut a = app(p);
    let ctx = ctx();
    let text = paint(&ctx, egui::vec2(1200.0, 400.0), 3, |ui| {
        views::arrangement::show(&mut a, ui)
    });
    assert!(text.contains("MISSING"), "{text}");
    assert!(text.contains("gone.wav"), "{text}");

    // Found again: the path follows, the waveform reads, undo puts it back.
    let found = dir.path().join("found.wav");
    wav(&found, 48_000, 1, 96_000, |_, _| 0.25);
    assert!(a.relink_audio(id, &found));
    let s = a.project().audio_source(id).unwrap().clone();
    assert_eq!(Path::new(&s.path), found);
    assert!(a.peaks.wait_idle(Duration::from_secs(20)));
    assert!(matches!(
        a.peaks.lookup(&s, a.audio_base(), None),
        Lookup::Ready(_)
    ));
    let text = paint(&ctx, egui::vec2(1200.0, 400.0), 3, |ui| {
        views::arrangement::show(&mut a, ui)
    });
    assert!(!text.contains("MISSING"), "{text}");
    a.undo();
    assert_eq!(Path::new(&a.project().audio_source(id).unwrap().path), gone);
}

#[test]
fn the_inspector_shows_an_audio_clip() {
    let (p, _) = missing_project("Z:/nowhere/Vocal.wav");
    let mut a = app(p);
    let (t, c) = (a.project().tracks[0].id, a.project().tracks[0].clips[0].id);
    a.select_clip(t, c);
    let ctx = ctx();
    let text = paint(&ctx, egui::vec2(420.0, 900.0), 3, |ui| {
        views::inspector::show(&mut a, ui)
    });
    for want in [
        "Audio clip",
        "Clip gain",
        "Fade in",
        "Fade out",
        "Locate file",
        "-3.0",
    ] {
        let (text, want) = (text.to_lowercase(), want.to_lowercase());
        assert!(text.contains(&want), "{want} in {text}");
    }
}
