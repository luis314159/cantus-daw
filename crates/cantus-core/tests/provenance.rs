//! A file's provenance beside the pool (schema 48, ADR 0085): set and cleared with exact
//! inverses, refused for a file not in the pool, and removed with its source — the
//! source's undo bringing both back.

use cantus_core::{AudioSource, AudioSourceId, Command, CommandBus, Origin, Provenance};

fn source(id: u64) -> AudioSource {
    AudioSource {
        id: AudioSourceId(id),
        name: format!("vocals {id}"),
        path: format!("Audio/Stems/vocals {id}.wav"),
        sample_rate: 44_100,
        channels: 2,
        frames: 44_100,
        sha256: None,
        recorded: false,
    }
}

fn card() -> Provenance {
    Provenance {
        kind: "separated".into(),
        model: "htdemucs".into(),
        model_name: "HT-Demucs — 4 stems".into(),
        model_version: 1,
        licence: "MIT".into(),
        date: "2026-10-30".into(),
        from: "Song".into(),
        part: "vocals".into(),
    }
}

#[test]
fn provenance_is_set_cleared_and_undone_exactly() {
    let mut bus = CommandBus::default();
    let id = AudioSourceId(bus.alloc_id());
    let before = bus.project().clone();
    bus.execute(
        Command::Batch {
            label_key: "cmd.separate_stems".into(),
            commands: vec![
                Command::InsertAudioSource {
                    source: source(id.0),
                },
                Command::SetAudioProvenance {
                    id,
                    provenance: Some(Box::new(card())),
                },
            ],
        },
        Origin::User,
    )
    .unwrap();
    assert_eq!(bus.project().provenance.get(&id), Some(&card()));
    bus.undo();
    assert_eq!(
        bus.project(),
        &before,
        "one undo step takes the file and its card"
    );
    bus.redo();
    assert_eq!(bus.project().provenance.get(&id), Some(&card()));
    // Not in the pool: refused.
    assert!(bus
        .execute(
            Command::SetAudioProvenance {
                id: AudioSourceId(9_999),
                provenance: Some(Box::new(card())),
            },
            Origin::User,
        )
        .is_err());
}

#[test]
fn removing_a_source_removes_its_card_and_undo_brings_both_back() {
    let mut bus = CommandBus::default();
    let id = AudioSourceId(bus.alloc_id());
    bus.execute(
        Command::InsertAudioSource {
            source: source(id.0),
        },
        Origin::User,
    )
    .unwrap();
    bus.execute(
        Command::SetAudioProvenance {
            id,
            provenance: Some(Box::new(card())),
        },
        Origin::User,
    )
    .unwrap();
    let with = bus.project().clone();
    bus.execute(Command::RemoveAudioSource { id }, Origin::User)
        .unwrap();
    assert!(bus.project().audio_source(id).is_none());
    assert!(
        bus.project().provenance.is_empty(),
        "no card for a file not there"
    );
    bus.undo();
    assert_eq!(bus.project(), &with);
}

/// A paste that brings a file into another project brings its card (the clipboard
/// carries it); a paste that reuses a file adds none.
#[test]
fn a_pasted_file_keeps_its_card() {
    use cantus_core::clipboard::{self, ClipOrigin, Clipboard, SourcePlan};
    use cantus_core::Ticks;
    use std::collections::HashMap;
    let mut bus = CommandBus::default();
    let track = bus.new_track("Vocals");
    let tid = track.id;
    let id = AudioSourceId(bus.alloc_id());
    let mut clip = bus.new_clip("vocals", Ticks::ZERO, Ticks::beats(4));
    clip.audio = Some(cantus_core::AudioRegion::of(id));
    let cid = clip.id;
    bus.execute(
        Command::Batch {
            label_key: "cmd.separate_stems".into(),
            commands: vec![
                Command::InsertTrack {
                    index: 0,
                    track: Box::new(track),
                },
                Command::InsertAudioSource {
                    source: source(id.0),
                },
                Command::SetAudioProvenance {
                    id,
                    provenance: Some(Box::new(card())),
                },
                Command::InsertClip {
                    track: tid,
                    clip: Box::new(clip),
                },
            ],
        },
        Origin::User,
    )
    .unwrap();
    let board = Clipboard::new(
        ClipOrigin::default(),
        bus.project(),
        clipboard::copy_clips(bus.project(), &[(tid, cid)]),
    );
    assert_eq!(board.provenance.get(&id), Some(&card()));
    // Into another project: a new file, its card with it.
    let mut other = CommandBus::default();
    let t2 = other.new_track("Here");
    let t2id = t2.id;
    other
        .execute(
            Command::InsertTrack {
                index: 0,
                track: Box::new(t2),
            },
            Origin::User,
        )
        .unwrap();
    let fresh = AudioSourceId(other.alloc_id() + 100);
    let mut plans = HashMap::new();
    plans.insert(id, SourcePlan::Insert(source(fresh.0)));
    let mut n = fresh.0 + 1;
    let out = clipboard::paste_clips(
        other.project(),
        &board,
        false,
        0,
        Ticks::ZERO,
        &plans,
        false,
        &mut || {
            n += 1;
            n
        },
    );
    assert!(out.commands.iter().any(|c| matches!(c,
        Command::SetAudioProvenance { id: i, provenance: Some(p) } if *i == fresh && **p == card())));
    other.reserve_ids(n + 1);
    other
        .execute(
            Command::Batch {
                label_key: "cmd.paste_clips".into(),
                commands: out.commands,
            },
            Origin::User,
        )
        .unwrap();
    assert_eq!(other.project().provenance.get(&fresh), Some(&card()));
    let _ = t2id;
    // Reused: nothing added.
    plans.insert(id, SourcePlan::Reuse(id));
    let out = clipboard::paste_clips(
        bus.project(),
        &board,
        true,
        0,
        Ticks::ZERO,
        &plans,
        false,
        &mut || {
            n += 1;
            n
        },
    );
    assert!(!out
        .commands
        .iter()
        .any(|c| matches!(c, Command::SetAudioProvenance { .. })));
}
