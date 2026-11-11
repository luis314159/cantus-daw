//! r39:arrange — the arrangement's structure (ADR 0113): each edit and its exact inverse,
//! and the ones refused.

use super::*;
use crate::{AutoMode, AutoTarget, Command, CommandBus, Origin, Project};

fn bar(n: i64) -> Ticks {
    Ticks(Ticks::beats(4).0 * n)
}

fn section(p: &mut Project, start: i64, len: i64, name: &str) -> Section {
    Section {
        id: SectionId(p.alloc_id()),
        start: bar(start),
        length: bar(len),
        name: name.into(),
        color: None,
    }
}

fn run(bus: &mut CommandBus, c: ArrangeCommand) -> bool {
    bus.execute(Command::Arrange(c), Origin::User).is_ok()
}

fn bus_with_tracks(n: usize) -> CommandBus {
    let mut bus = CommandBus::default();
    for i in 0..n {
        let mut p = bus.project().clone();
        let t = p.make_track(format!("T{i}"));
        bus.reserve_ids(p.next_id);
        bus.execute(
            Command::InsertTrack {
                index: i,
                track: Box::new(t),
            },
            Origin::User,
        )
        .unwrap();
    }
    bus
}

#[test]
fn sections_are_added_moved_renamed_and_removed_and_each_undoes_exactly() {
    let mut bus = CommandBus::default();
    let start = bus.project().clone();
    let mut p = bus.project().clone();
    let verse = section(&mut p, 0, 8, "Verse");
    let chorus = section(&mut p, 8, 8, "Chorus");
    bus.reserve_ids(p.next_id);
    assert!(run(
        &mut bus,
        ArrangeCommand::InsertSection {
            index: 0,
            section: verse.clone()
        }
    ));
    assert!(run(
        &mut bus,
        ArrangeCommand::InsertSection {
            index: 1,
            section: chorus.clone()
        }
    ));
    assert_eq!(bus.project().section_at(bar(9)).unwrap().name, "Chorus");
    let mut moved = chorus.clone();
    moved.start = bar(16);
    moved.name = "Drop".into();
    assert!(run(&mut bus, ArrangeCommand::SetSection { section: moved }));
    assert!(bus.project().section_at(bar(9)).is_none());
    assert_eq!(bus.project().section(chorus.id).unwrap().end(), bar(24));
    assert!(run(
        &mut bus,
        ArrangeCommand::RemoveSection { id: verse.id }
    ));
    assert_eq!(bus.project().sections.len(), 1);
    let end = bus.project().clone();
    while bus.undo().is_some() {}
    let mut back = bus.project().clone();
    back.next_id = start.next_id;
    assert_eq!(back, start);
    while bus.redo().is_some() {}
    assert_eq!(bus.project().sections, end.sections);
}

#[test]
fn a_section_that_would_overlap_another_or_has_no_length_is_refused() {
    let mut bus = CommandBus::default();
    let mut p = bus.project().clone();
    let a = section(&mut p, 0, 8, "A");
    let b = section(&mut p, 4, 8, "B");
    let mut empty = section(&mut p, 20, 1, "E");
    empty.length = Ticks::ZERO;
    bus.reserve_ids(p.next_id);
    assert!(run(
        &mut bus,
        ArrangeCommand::InsertSection {
            index: 0,
            section: a.clone()
        }
    ));
    let before = bus.project().clone();
    assert!(!run(
        &mut bus,
        ArrangeCommand::InsertSection {
            index: 1,
            section: b
        }
    ));
    assert!(!run(
        &mut bus,
        ArrangeCommand::InsertSection {
            index: 1,
            section: empty
        }
    ));
    // Its own span may be set again (it does not overlap itself).
    let mut longer = a.clone();
    longer.length = bar(12);
    assert!(run(
        &mut bus,
        ArrangeCommand::SetSection { section: longer }
    ));
    bus.undo();
    assert_eq!(bus.project(), &before);
}

#[test]
fn a_folder_gathers_tracks_folds_and_is_removed_only_when_empty() {
    let mut bus = bus_with_tracks(3);
    let start = bus.project().clone();
    let (a, b) = (start.tracks[0].id, start.tracks[1].id);
    let mut p = bus.project().clone();
    let folder = TrackFolder {
        id: FolderId(p.alloc_id()),
        name: "Drums".into(),
        color: crate::Color::PALETTE[0],
        open: true,
    };
    bus.reserve_ids(p.next_id);
    assert!(run(
        &mut bus,
        ArrangeCommand::InsertFolder {
            index: 0,
            folder: folder.clone()
        }
    ));
    for t in [a, b] {
        assert!(run(
            &mut bus,
            ArrangeCommand::SetTrackFolder {
                track: t,
                folder: Some(folder.id)
            }
        ));
    }
    assert_eq!(bus.project().folder_members(folder.id).count(), 2);
    let mut closed = folder.clone();
    closed.open = false;
    assert!(run(&mut bus, ArrangeCommand::SetFolder { folder: closed }));
    assert!(!bus.project().folder(folder.id).unwrap().open);
    // Not while it holds tracks; and no track joins a folder that is not there.
    assert!(!run(
        &mut bus,
        ArrangeCommand::RemoveFolder { id: folder.id }
    ));
    assert!(!run(
        &mut bus,
        ArrangeCommand::SetTrackFolder {
            track: a,
            folder: Some(FolderId(999_999))
        }
    ));
    let empty = Command::Batch {
        label_key: "cmd.arrange.remove_folder".into(),
        commands: vec![
            Command::Arrange(ArrangeCommand::SetTrackFolder {
                track: a,
                folder: None,
            }),
            Command::Arrange(ArrangeCommand::SetTrackFolder {
                track: b,
                folder: None,
            }),
            Command::Arrange(ArrangeCommand::RemoveFolder { id: folder.id }),
        ],
    };
    assert!(bus.execute(empty, Origin::User).is_ok());
    assert!(bus.project().folders.is_empty());
    while bus.undo().is_some() {}
    // The three tracks were inserted by the bus too, and undo removed them: what is left
    // has no folder and no member.
    let back = bus.project().clone();
    assert!(back.folders.is_empty());
    assert!(back.tracks.iter().all(|t| t.folder.is_none()));
}

#[test]
fn a_lane_has_its_own_mode_over_its_tracks_and_gives_it_back() {
    let mut bus = bus_with_tracks(1);
    let t = bus.project().tracks[0].id;
    let gain = AutoTarget::TrackGain(t);
    let pan = AutoTarget::TrackPan(t);
    let before = bus.project().clone();
    assert!(run(
        &mut bus,
        ArrangeCommand::SetLaneMode {
            track: t,
            target: gain.clone(),
            mode: Some(AutoMode::Touch)
        }
    ));
    let tr = bus.project().track(t).unwrap();
    assert_eq!(lane_mode(tr, &gain), AutoMode::Touch);
    assert_eq!(lane_mode(tr, &pan), AutoMode::Read, "the track's own");
    assert!(run(
        &mut bus,
        ArrangeCommand::SetLaneMode {
            track: t,
            target: gain.clone(),
            mode: Some(AutoMode::Write)
        }
    ));
    assert!(run(
        &mut bus,
        ArrangeCommand::SetLaneMode {
            track: t,
            target: gain.clone(),
            mode: None
        }
    ));
    assert!(bus.project().track(t).unwrap().lane_modes.is_empty());
    while bus.project() != &before && bus.undo().is_some() {}
    assert_eq!(bus.project(), &before);
    assert!(!run(
        &mut bus,
        ArrangeCommand::SetLaneMode {
            track: crate::TrackId(424_242),
            target: gain,
            mode: Some(AutoMode::Latch)
        }
    ));
}

#[test]
fn an_older_project_reads_with_no_sections_no_folders_and_no_lane_modes() {
    let p = Project::new("old");
    let mut v = serde_json::to_value(&p).unwrap();
    assert!(
        v.get("sections").is_none() && v.get("folders").is_none(),
        "not written"
    );
    v.as_object_mut().unwrap().remove("sections");
    let back: Project = serde_json::from_value(v).unwrap();
    assert!(back.sections.is_empty() && back.folders.is_empty());
}

#[test]
fn a_tracks_own_icon_is_set_cleared_and_undone_and_a_bad_id_is_refused() {
    let mut bus = bus_with_tracks(1);
    let t = bus.project().tracks[0].id;
    let before = bus.project().clone();
    let set = |g: Option<&str>| ArrangeCommand::SetTrackGlyph {
        track: t,
        glyph: g.map(str::to_string),
    };
    assert!(run(&mut bus, set(Some("kick"))));
    assert_eq!(
        bus.project().track(t).unwrap().glyph.as_deref(),
        Some("kick")
    );
    assert!(run(&mut bus, set(None)));
    assert!(!run(&mut bus, set(Some("Bad Id!"))));
    assert!(!run(&mut bus, set(Some(""))));
    while bus.undo().is_some() && bus.project() != &before {}
    assert_eq!(bus.project(), &before);
}
