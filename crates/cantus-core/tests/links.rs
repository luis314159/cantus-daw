//! Linked clips (ADR 0037): copies that are the same clip. Every rule is checked through a
//! real `CommandBus`, so "undo restores the project" is the bus's own answer.

use cantus_core::clipboard;
use cantus_core::links::{self, ClipContent, Linker};
use cantus_core::*;

struct Counter(u64);
impl Counter {
    fn of(bus: &CommandBus) -> Self {
        Counter(bus.project().next_id)
    }
    fn take(&mut self) -> u64 {
        let id = self.0;
        self.0 += 1;
        id
    }
}

fn run(bus: &mut CommandBus, label: &str, commands: Vec<Command>) {
    bus.execute(
        Command::Batch {
            label_key: label.into(),
            commands,
        },
        Origin::User,
    )
    .expect("the batch applies");
}

fn normalised(p: &Project) -> Project {
    let mut p = p.clone();
    p.next_id = 0;
    p
}

fn clip(bus: &CommandBus, t: TrackId, c: ClipId) -> Clip {
    bus.project().track(t).unwrap().clip(c).unwrap().clone()
}

/// Two tracks; a four-note clip `a` on the first.
fn fixture() -> (CommandBus, Vec<TrackId>, ClipId) {
    let mut bus = CommandBus::default();
    let mut tracks = Vec::new();
    for i in 0..2 {
        let t = bus.new_track(format!("t{i}"));
        tracks.push(t.id);
        bus.execute(
            Command::InsertTrack {
                index: i,
                track: Box::new(t),
            },
            Origin::User,
        )
        .unwrap();
    }
    let mut a = bus.new_clip("a", Ticks::beats(0), Ticks::beats(4));
    for i in 0..4 {
        a.notes
            .push(bus.new_note(Ticks::beats(i), Ticks::QUARTER, 60 + i as u8, 90));
    }
    let ca = a.id;
    bus.execute(
        Command::InsertClip {
            track: tracks[0],
            clip: Box::new(a),
        },
        Origin::User,
    )
    .unwrap();
    (bus, tracks, ca)
}

/// A linked copy of `(t, c)` at `start` on `to`, through the bus; returns its id.
fn linked_copy(bus: &mut CommandBus, t: TrackId, c: ClipId, to: TrackId, start: Ticks) -> ClipId {
    let src = clip(bus, t, c);
    let mut ids = Counter::of(bus);
    let (mut commands, copy) = Linker::new().copy(t, &src, start, &mut || ids.take());
    let id = copy.id;
    commands.push(Command::InsertClip {
        track: to,
        clip: Box::new(copy),
    });
    while bus.project().next_id < ids.0 {
        bus.new_note(Ticks::ZERO, Ticks::QUARTER, 60, 100);
    }
    run(bus, "cmd.duplicate_clips", commands);
    id
}

#[test]
fn a_linked_copy_is_the_same_clip_and_an_edit_through_either_reaches_both() {
    let (mut bus, tracks, a) = fixture();
    let before = normalised(bus.project());
    let b = linked_copy(&mut bus, tracks[0], a, tracks[1], Ticks::beats(8));
    let (ca, cb) = (clip(&bus, tracks[0], a), clip(&bus, tracks[1], b));
    assert!(ca.link.is_some());
    assert_eq!(ca.link, cb.link);
    assert!(links::same_content(&ca, &cb));
    assert_eq!(links::count(bus.project(), ca.link.unwrap()), 2);
    assert_eq!(
        links::siblings(bus.project(), (tracks[1], b)),
        vec![(tracks[0], a)]
    );

    // Notes added, moved and removed through the copy land in the original.
    let n = bus.new_note(Ticks::beats(2), Ticks::EIGHTH, 72, 100);
    let nid = n.id;
    bus.execute(
        Command::InsertNotes {
            track: tracks[1],
            clip: b,
            notes: vec![n],
        },
        Origin::User,
    )
    .unwrap();
    assert!(clip(&bus, tracks[0], a).note(nid).is_some());
    let mut moved = clip(&bus, tracks[1], b).notes[0].clone();
    moved.pitch = 40;
    let mid = moved.id;
    bus.execute(
        Command::SetNotes {
            track: tracks[1],
            clip: b,
            notes: vec![moved],
        },
        Origin::User,
    )
    .unwrap();
    assert_eq!(clip(&bus, tracks[0], a).note(mid).unwrap().pitch, 40);
    bus.execute(
        Command::RemoveNotes {
            track: tracks[0],
            clip: a,
            ids: vec![nid],
        },
        Origin::User,
    )
    .unwrap();
    assert!(clip(&bus, tracks[1], b).note(nid).is_none());
    assert!(links::same_content(
        &clip(&bus, tracks[0], a),
        &clip(&bus, tracks[1], b)
    ));
    links::check(bus.project()).unwrap();

    // Name and colour are shared too.
    bus.execute(
        Command::SetClipName {
            track: tracks[1],
            clip: b,
            name: "Chorus".into(),
        },
        Origin::User,
    )
    .unwrap();
    bus.execute(
        Command::SetClipColor {
            track: tracks[0],
            clip: a,
            color: Some(Color::PALETTE[4]),
        },
        Origin::User,
    )
    .unwrap();
    assert_eq!(clip(&bus, tracks[0], a).name, "Chorus");
    assert_eq!(clip(&bus, tracks[1], b).color, Some(Color::PALETTE[4]));

    // Undo walks every step back, on both clips: the copy, three note edits, two renames.
    for _ in 0..6 {
        bus.undo().expect("a step to undo");
    }
    assert_eq!(normalised(bus.project()), before);
}

#[test]
fn where_each_copy_sits_stays_its_own() {
    let (mut bus, tracks, a) = fixture();
    let b = linked_copy(&mut bus, tracks[0], a, tracks[0], Ticks::beats(8));
    let t = tracks[0];
    for cmd in [
        Command::SetClipBounds {
            track: t,
            clip: b,
            start: Ticks::beats(9),
            length: Ticks::beats(2),
        },
        Command::SetClipOffset {
            track: t,
            clip: b,
            offset: Ticks::beats(1),
        },
        Command::SetClipLoop {
            track: t,
            clip: b,
            content_length: Some(Ticks::beats(1)),
        },
        Command::SetClipMute {
            track: t,
            clip: b,
            muted: true,
        },
        Command::SetClipLane {
            track: t,
            clip: b,
            lane: 1,
        },
    ] {
        bus.execute(cmd, Origin::User).unwrap();
    }
    let ca = clip(&bus, t, a);
    assert_eq!(
        (
            ca.start,
            ca.length,
            ca.offset,
            ca.content_length,
            ca.muted,
            ca.lane
        ),
        (Ticks::ZERO, Ticks::beats(4), Ticks::ZERO, None, false, 0)
    );
    assert_eq!(ca.link, clip(&bus, t, b).link);
}

/// An audio clip's settings — gain, fades and their curves, reverse — are shared.
#[test]
fn audio_copies_share_their_settings() {
    let (mut bus, tracks, _) = fixture();
    let src = AudioSource {
        id: AudioSourceId(bus.project().next_id + 100),
        name: "loop".into(),
        path: "C:/nowhere/loop.wav".into(),
        sample_rate: 48_000,
        channels: 2,
        frames: 48_000 * 8,
        sha256: None,
        recorded: false,
    };
    let sid = src.id;
    bus.execute(Command::InsertAudioSource { source: src }, Origin::User)
        .unwrap();
    let mut au = bus.new_clip("drums", Ticks::beats(16), Ticks::beats(4));
    au.audio = Some(AudioRegion::of(sid));
    let aid = au.id;
    bus.execute(
        Command::InsertClip {
            track: tracks[1],
            clip: Box::new(au),
        },
        Origin::User,
    )
    .unwrap();
    let b = linked_copy(&mut bus, tracks[1], aid, tracks[1], Ticks::beats(24));
    let mut region = clip(&bus, tracks[1], b).audio.unwrap();
    region.gain_db = -6.0;
    region.fade_in = Ticks::QUARTER;
    region.fade_out_curve = FadeCurve::SCurve;
    region.reverse = true;
    bus.execute(
        Command::SetClipAudio {
            track: tracks[1],
            clip: b,
            audio: Some(region.clone()),
        },
        Origin::User,
    )
    .unwrap();
    assert_eq!(clip(&bus, tracks[1], aid).audio, Some(region));
    // The snapshot plays both with the new settings, each at its own place.
    let snap = Snapshot::compile(bus.project(), 1);
    let ts = snap.track(tracks[1]).unwrap();
    assert_eq!(ts.audio.len(), 2);
    for a in &ts.audio {
        assert!(a.reverse);
    }
    bus.undo();
    assert_eq!(clip(&bus, tracks[1], aid).audio, Some(AudioRegion::of(sid)));
}

#[test]
fn make_unique_detaches_one_copy_and_undo_links_it_again() {
    let (mut bus, tracks, a) = fixture();
    let b = linked_copy(&mut bus, tracks[0], a, tracks[1], Ticks::beats(8));
    let c = linked_copy(&mut bus, tracks[0], a, tracks[1], Ticks::beats(16));
    let link = clip(&bus, tracks[0], a).link.unwrap();
    assert_eq!(links::count(bus.project(), link), 3);
    let cmds = links::make_unique(bus.project(), &[(tracks[1], b)]);
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].label_key(), "cmd.make_unique");
    run(&mut bus, "cmd.make_unique", cmds);
    assert_eq!(clip(&bus, tracks[1], b).link, None);
    assert_eq!(links::count(bus.project(), link), 2);
    // Its content is its own now: an edit through it reaches nobody, and the others'
    // edits do not reach it.
    bus.execute(
        Command::SetClipName {
            track: tracks[1],
            clip: b,
            name: "solo".into(),
        },
        Origin::User,
    )
    .unwrap();
    assert_eq!(clip(&bus, tracks[0], a).name, "a");
    bus.execute(
        Command::SetClipName {
            track: tracks[1],
            clip: c,
            name: "both".into(),
        },
        Origin::User,
    )
    .unwrap();
    assert_eq!(clip(&bus, tracks[0], a).name, "both");
    assert_eq!(clip(&bus, tracks[1], b).name, "solo");
    // Undo the renames and the detach: linked again, and an edit reaches it again.
    bus.undo();
    bus.undo();
    bus.undo();
    assert_eq!(clip(&bus, tracks[1], b).link, Some(link));
    links::check(bus.project()).unwrap();
}

#[test]
fn link_to_gives_a_clip_the_target_content_and_leaves_its_old_group_alone() {
    let (mut bus, tracks, a) = fixture();
    // `x` and `y` are a group of their own with other notes.
    let mut x = bus.new_clip("x", Ticks::beats(8), Ticks::beats(2));
    x.notes
        .push(bus.new_note(Ticks::ZERO, Ticks::QUARTER, 30, 50));
    let xid = x.id;
    bus.execute(
        Command::InsertClip {
            track: tracks[1],
            clip: Box::new(x),
        },
        Origin::User,
    )
    .unwrap();
    let y = linked_copy(&mut bus, tracks[1], xid, tracks[1], Ticks::beats(12));
    let old = clip(&bus, tracks[1], xid);
    let before = normalised(bus.project());
    let mut ids = Counter::of(&bus);
    let cmds = links::link_to(
        bus.project(),
        &[(tracks[1], xid)],
        (tracks[0], a),
        &mut || ids.take(),
    );
    while bus.project().next_id < ids.0 {
        bus.new_note(Ticks::ZERO, Ticks::QUARTER, 60, 100);
    }
    run(&mut bus, "cmd.link_clips", cmds);
    let (ca, cx, cy) = (
        clip(&bus, tracks[0], a),
        clip(&bus, tracks[1], xid),
        clip(&bus, tracks[1], y),
    );
    assert!(ca.link.is_some());
    assert_eq!(cx.link, ca.link);
    assert!(links::same_content(&ca, &cx));
    // Its window stays its own.
    assert_eq!((cx.start, cx.length), (Ticks::beats(8), Ticks::beats(2)));
    // Its old sibling keeps the old content and the old group.
    assert!(links::same_content(&cy, &old));
    assert_eq!(cy.link, old.link);
    links::check(bus.project()).unwrap();
    bus.undo();
    assert_eq!(normalised(bus.project()), before);
}

#[test]
fn a_clip_that_does_not_match_a_group_is_refused_into_it() {
    let (mut bus, tracks, a) = fixture();
    let b = linked_copy(&mut bus, tracks[0], a, tracks[1], Ticks::beats(8));
    let link = clip(&bus, tracks[0], a).link;
    let mut other = bus.new_clip("other", Ticks::beats(20), Ticks::beats(4));
    other.link = link;
    let before = normalised(bus.project());
    assert!(bus
        .execute(
            Command::InsertClip {
                track: tracks[0],
                clip: Box::new(other.clone()),
            },
            Origin::User,
        )
        .is_err());
    other.link = None;
    let oid = other.id;
    bus.execute(
        Command::InsertClip {
            track: tracks[0],
            clip: Box::new(other),
        },
        Origin::User,
    )
    .unwrap();
    assert!(bus
        .execute(
            Command::SetClipLink {
                track: tracks[0],
                clip: oid,
                link,
            },
            Origin::User,
        )
        .is_err());
    bus.undo();
    assert_eq!(normalised(bus.project()), before);
    // A removed sibling comes back into its group on undo.
    bus.execute(
        Command::RemoveClip {
            track: tracks[1],
            clip: b,
        },
        Origin::User,
    )
    .unwrap();
    bus.undo();
    assert_eq!(clip(&bus, tracks[1], b).link, link);
}

#[test]
fn set_clip_content_replaces_everything_shared_on_every_sibling() {
    let (mut bus, tracks, a) = fixture();
    let b = linked_copy(&mut bus, tracks[0], a, tracks[1], Ticks::beats(8));
    let before = normalised(bus.project());
    let content = ClipContent {
        name: "new".into(),
        color: Some(Color::PALETTE[1]),
        notes: vec![bus.new_note(Ticks::ZERO, Ticks::QUARTER, 50, 70)],
        audio: None,
        automation: None,
    };
    bus.execute(
        Command::SetClipContent {
            track: tracks[0],
            clip: a,
            content: Box::new(content.clone()),
        },
        Origin::User,
    )
    .unwrap();
    assert_eq!(ClipContent::of(&clip(&bus, tracks[1], b)), content);
    bus.undo();
    assert_eq!(normalised(bus.project()), before);
}

/// The engine plays a linked copy exactly as an unlinked copy of the same notes.
#[test]
fn the_snapshot_plays_linked_copies_at_their_own_places() {
    let (mut bus, tracks, a) = fixture();
    let b = linked_copy(&mut bus, tracks[0], a, tracks[0], Ticks::beats(8));
    bus.execute(
        Command::SetClipOffset {
            track: tracks[0],
            clip: b,
            offset: Ticks::beats(1),
        },
        Origin::User,
    )
    .unwrap();
    let events = |p: &Project| -> Vec<(Ticks, u8, u8)> {
        Snapshot::compile(p, 1)
            .track(tracks[0])
            .unwrap()
            .events
            .iter()
            .map(|e| (e.tick, e.pitch, e.velocity))
            .collect()
    };
    // The same arrangement with an independent copy.
    let mut unlinked = bus.project().clone();
    for c in &mut unlinked.tracks[0].clips {
        c.link = None;
    }
    assert_eq!(events(bus.project()), events(&unlinked));
    // An edit through the copy is heard in the original too.
    let mut n = clip(&bus, tracks[0], b).notes[0].clone();
    n.velocity = 11;
    bus.execute(
        Command::SetNotes {
            track: tracks[0],
            clip: b,
            notes: vec![n],
        },
        Origin::User,
    )
    .unwrap();
    let heard: Vec<_> = events(bus.project())
        .into_iter()
        .filter(|e| e.2 == 11)
        .collect();
    // Beat 0 in the original; the copy's window starts a beat into the content, so its
    // first note (content beat 0) is outside it.
    assert_eq!(heard, vec![(Ticks::ZERO, 60, 11)]);
}

#[test]
fn copying_one_clip_twice_in_a_batch_makes_one_group() {
    let (bus, tracks, a) = fixture();
    let src = clip(&bus, tracks[0], a);
    let mut ids = Counter::of(&bus);
    let mut linker = Linker::new();
    let (pre1, c1) = linker.copy(tracks[0], &src, Ticks::beats(8), &mut || ids.take());
    let (pre2, c2) = linker.copy(tracks[0], &src, Ticks::beats(16), &mut || ids.take());
    assert_eq!(pre1.len(), 1);
    assert!(pre2.is_empty());
    assert_eq!(c1.link, c2.link);
}

#[test]
fn repair_takes_a_mismatching_member_out_of_its_group() {
    let (mut bus, tracks, a) = fixture();
    let b = linked_copy(&mut bus, tracks[0], a, tracks[1], Ticks::beats(8));
    let mut p = bus.project().clone();
    p.track_mut(tracks[1]).unwrap().clip_mut(b).unwrap().name = "edited by hand".into();
    assert!(links::check(&p).is_err());
    assert_eq!(links::repair(&mut p), 1);
    links::check(&p).unwrap();
    assert_eq!(p.track(tracks[1]).unwrap().clip(b).unwrap().link, None);
    assert!(p.track(tracks[0]).unwrap().clip(a).unwrap().link.is_some());
}

/// Splitting a linked note clip keeps both halves in the group; splitting a linked audio
/// clip with fades takes both halves out, and its siblings keep their fades.
#[test]
fn a_split_keeps_the_group_unless_it_changes_the_content() {
    let (mut bus, tracks, a) = fixture();
    let b = linked_copy(&mut bus, tracks[0], a, tracks[1], Ticks::beats(8));
    let link = clip(&bus, tracks[0], a).link;
    let mut ids = Counter::of(&bus);
    let cmds = clipboard::split(
        bus.project(),
        tracks[1],
        &clip(&bus, tracks[1], b),
        Ticks::beats(10),
        &mut || ids.take(),
    )
    .unwrap();
    run(&mut bus, "cmd.split_clip", cmds);
    assert_eq!(links::count(bus.project(), link.unwrap()), 3);
    links::check(bus.project()).unwrap();

    let src = AudioSource {
        id: AudioSourceId(bus.project().next_id + 500),
        name: "v".into(),
        path: "C:/nowhere/v.wav".into(),
        sample_rate: 48_000,
        channels: 1,
        frames: 48_000 * 30,
        sha256: None,
        recorded: false,
    };
    let sid = src.id;
    bus.execute(Command::InsertAudioSource { source: src }, Origin::User)
        .unwrap();
    let mut au = bus.new_clip("v", Ticks::beats(32), Ticks::beats(4));
    let mut region = AudioRegion::of(sid);
    region.fade_in = Ticks::QUARTER;
    region.fade_out = Ticks::QUARTER;
    au.audio = Some(region.clone());
    let aid = au.id;
    bus.execute(
        Command::InsertClip {
            track: tracks[0],
            clip: Box::new(au),
        },
        Origin::User,
    )
    .unwrap();
    let sib = linked_copy(&mut bus, tracks[0], aid, tracks[0], Ticks::beats(40));
    let mut ids = Counter::of(&bus);
    let cmds = clipboard::split(
        bus.project(),
        tracks[0],
        &clip(&bus, tracks[0], aid),
        Ticks::beats(34),
        &mut || ids.take(),
    )
    .unwrap();
    run(&mut bus, "cmd.split_clip", cmds);
    assert_eq!(clip(&bus, tracks[0], aid).link, None);
    assert_eq!(clip(&bus, tracks[0], sib).audio, Some(region));
    links::check(bus.project()).unwrap();
}

fn paste(
    bus: &mut CommandBus,
    sel: &[(TrackId, ClipId)],
    same_origin: bool,
    linked: bool,
) -> Vec<(TrackId, ClipId)> {
    use clipboard::{ClipOrigin, Clipboard};
    let board = Clipboard::new(
        ClipOrigin::default(),
        bus.project(),
        clipboard::copy_clips(bus.project(), sel),
    );
    paste_board(bus, &board, same_origin, linked)
}

fn paste_board(
    bus: &mut CommandBus,
    board: &clipboard::Clipboard,
    same_origin: bool,
    linked: bool,
) -> Vec<(TrackId, ClipId)> {
    let mut ids = Counter::of(bus);
    let out = clipboard::paste_clips(
        bus.project(),
        board,
        same_origin,
        1,
        Ticks::beats(32),
        &std::collections::HashMap::new(),
        linked,
        &mut || ids.take(),
    );
    while bus.project().next_id < ids.0 {
        bus.new_note(Ticks::ZERO, Ticks::QUARTER, 60, 100);
    }
    run(bus, "cmd.paste_clips", out.commands);
    out.made
}

#[test]
fn a_linked_paste_is_the_clip_it_was_copied_from() {
    let (mut bus, tracks, a) = fixture();
    let before = normalised(bus.project());
    let made = paste(&mut bus, &[(tracks[0], a)], true, true);
    let (ca, cp) = (clip(&bus, tracks[0], a), clip(&bus, made[0].0, made[0].1));
    assert!(ca.link.is_some(), "the original joins a new group");
    assert_eq!(ca.link, cp.link);
    assert!(links::same_content(&ca, &cp));
    // Pasting again joins the same group: three members.
    let again = paste(&mut bus, &[(tracks[0], a)], true, true);
    assert_eq!(clip(&bus, again[0].0, again[0].1).link, ca.link);
    assert_eq!(links::count(bus.project(), ca.link.unwrap()), 3);
    bus.undo();
    bus.undo();
    assert_eq!(normalised(bus.project()), before, "one undo per paste");
}

#[test]
fn a_paste_is_unique_when_it_cannot_be_the_same_clip() {
    let (mut bus, tracks, a) = fixture();
    // Linking off: a copy of its own, fresh note ids.
    let made = paste(&mut bus, &[(tracks[0], a)], true, false);
    let cp = clip(&bus, made[0].0, made[0].1);
    assert_eq!(cp.link, None);
    assert_eq!(clip(&bus, tracks[0], a).link, None);
    assert!(cp
        .notes
        .iter()
        .all(|n| clip(&bus, tracks[0], a).note(n.id).is_none()));
    // From another project (or another run): nothing here to be the same clip as.
    let made = paste(&mut bus, &[(tracks[0], a)], false, true);
    assert_eq!(clip(&bus, made[0].0, made[0].1).link, None);
    // The original changed since the copy: the copy is what was copied, on its own.
    let board = clipboard::Clipboard::new(
        clipboard::ClipOrigin::default(),
        bus.project(),
        clipboard::copy_clips(bus.project(), &[(tracks[0], a)]),
    );
    bus.execute(
        Command::SetClipName {
            track: tracks[0],
            clip: a,
            name: "changed".into(),
        },
        Origin::User,
    )
    .unwrap();
    let made = paste_board(&mut bus, &board, true, true);
    let cp = clip(&bus, made[0].0, made[0].1);
    assert_eq!(cp.link, None);
    assert_eq!(cp.name, "a");
}

/// A copy of a member pastes into its group even after the member itself was deleted.
#[test]
fn a_linked_paste_finds_the_group_when_its_original_is_gone() {
    let (mut bus, tracks, a) = fixture();
    let b = linked_copy(&mut bus, tracks[0], a, tracks[1], Ticks::beats(8));
    let link = clip(&bus, tracks[0], a).link;
    let board = clipboard::Clipboard::new(
        clipboard::ClipOrigin::default(),
        bus.project(),
        clipboard::copy_clips(bus.project(), &[(tracks[1], b)]),
    );
    bus.execute(
        Command::RemoveClip {
            track: tracks[1],
            clip: b,
        },
        Origin::User,
    )
    .unwrap();
    let made = paste_board(&mut bus, &board, true, true);
    assert_eq!(clip(&bus, made[0].0, made[0].1).link, link);
    links::check(bus.project()).unwrap();
}

/// What an edit through a sibling costs (ADR 0037's measurement): one note moved in a
/// 2 000-note pattern, alone and with 63 linked copies; and the snapshot, which compiles a
/// linked copy exactly as an unlinked one. `cargo test -p cantus-core --test links
/// measure -- --ignored --nocapture`.
#[test]
#[ignore = "a measurement, not a check"]
fn measure_an_edit_through_a_sibling() {
    use std::time::Instant;
    for copies in [0usize, 63] {
        let (mut bus, tracks, a) = fixture();
        let mut big = clip(&bus, tracks[0], a);
        big.notes.clear();
        for i in 0..2000 {
            big.notes
                .push(bus.new_note(Ticks(i * 48), Ticks(40), (i % 60) as u8 + 30, 90));
        }
        bus.execute(
            Command::SetClipContent {
                track: tracks[0],
                clip: a,
                content: Box::new(ClipContent::of(&big)),
            },
            Origin::User,
        )
        .unwrap();
        for k in 0..copies {
            linked_copy(
                &mut bus,
                tracks[0],
                a,
                tracks[1],
                Ticks::beats(8 * (k as i64 + 1)),
            );
        }
        let n = clip(&bus, tracks[0], a).notes[1000].clone();
        let runs = 200;
        let t0 = Instant::now();
        for r in 0..runs {
            let moved = Note {
                pitch: 40 + (r % 20) as u8,
                ..n.clone()
            };
            bus.execute(
                Command::SetNotes {
                    track: tracks[0],
                    clip: a,
                    notes: vec![moved],
                },
                Origin::User,
            )
            .unwrap();
        }
        let edit = t0.elapsed().as_secs_f64() / runs as f64;
        let t1 = Instant::now();
        for _ in 0..20 {
            let _ = Snapshot::compile(bus.project(), 1);
        }
        let snap = t1.elapsed().as_secs_f64() / 20.0;
        println!(
            "{} clips in the group: an edit {:.1} µs, a snapshot {:.2} ms",
            copies + 1,
            edit * 1e6,
            snap * 1e3
        );
    }
}

/// Every note event of `track`, tag included.
fn note_events(p: &Project, track: TrackId) -> Vec<(Ticks, u8, u8, u64)> {
    Snapshot::compile(p, 1)
        .track(track)
        .unwrap()
        .events
        .iter()
        .map(|e| (e.tick, e.pitch, e.velocity, e.note.0))
        .collect()
}

/// The voice tags (what pairs a note-off with its note-on, and what a seeded instrument
/// draws its randomness from) never depend on the link: joining or leaving a group
/// changes no event — so no sample — of a clip whose content did not change. The clip
/// that first held the notes (the lowest id) keeps its notes' own ids; a later clip that
/// plays the same note ids on the same track is told apart, in the low 31 bits too (the
/// part a CLAP or VST3 note id keeps).
#[test]
fn voice_tags_do_not_depend_on_the_link_and_survive_31_bits() {
    let (mut bus, tracks, a) = fixture();
    let solo = note_events(bus.project(), tracks[0]);
    // A copy that overlaps the original by three beats, on the same track.
    let b = linked_copy(&mut bus, tracks[0], a, tracks[0], Ticks::beats(1));
    let linked = note_events(bus.project(), tracks[0]);
    let raw: Vec<u64> = clip(&bus, tracks[0], a)
        .notes
        .iter()
        .map(|n| n.id.0)
        .collect();
    // The original's own events are exactly what they were before the copy.
    for e in &solo {
        assert!(linked.contains(e), "{e:?} changed when the copy was linked");
    }
    // The copy's are told apart from the original's, also when cut to 31 bits.
    let ons: Vec<u64> = linked.iter().filter(|e| e.2 > 0).map(|e| e.3).collect();
    assert_eq!(ons.len(), 8);
    let mut low: Vec<u64> = ons.iter().map(|t| t & 0x7FFF_FFFF).collect();
    low.sort_unstable();
    low.dedup();
    assert_eq!(low.len(), 8, "31-bit tags collide: {ons:?}");
    assert_eq!(ons.iter().filter(|t| raw.contains(t)).count(), 4);
    // Make unique (both copies): not one event changes.
    let unique = links::make_unique(bus.project(), &[(tracks[0], a), (tracks[0], b)]);
    run(&mut bus, "cmd.make_unique", unique);
    assert!(clip(&bus, tracks[0], b).link.is_none());
    assert_eq!(note_events(bus.project(), tracks[0]), linked);
    // The same copy on another track plays its notes' own ids, as the original does.
    let c = linked_copy(&mut bus, tracks[0], a, tracks[1], Ticks::beats(0));
    let other: Vec<u64> = note_events(bus.project(), tracks[1])
        .iter()
        .map(|e| e.3)
        .collect();
    assert!(other.iter().all(|t| raw.contains(t)), "{c}: {other:?}");
}

/// A track inserted whole must not bring two clips that name one group with different
/// content (an MCP call, a future duplicate-track): the guard compares the track's clips
/// with each other as well as with the project.
#[test]
fn an_inserted_track_whose_own_clips_disagree_on_a_group_is_refused() {
    let (mut bus, _tracks, _a) = fixture();
    let mut t = bus.new_track("new");
    let link = Some(LinkId(bus.project().next_id + 1000));
    let mut x = bus.new_clip("x", Ticks::ZERO, Ticks::beats(4));
    x.notes
        .push(bus.new_note(Ticks::ZERO, Ticks::QUARTER, 60, 90));
    let mut y = Clip {
        id: ClipId(bus.project().next_id + 2000),
        start: Ticks::beats(8),
        ..x.clone()
    };
    x.link = link;
    y.link = link;
    y.notes[0].pitch = 61;
    t.clips = vec![x, y.clone()];
    let before = normalised(bus.project());
    let insert = |t: &Track| Command::InsertTrack {
        index: 2,
        track: Box::new(t.clone()),
    };
    assert!(bus.execute(insert(&t), Origin::Mcp).is_err());
    assert_eq!(normalised(bus.project()), before);
    // The same content in both is a group, and is taken.
    t.clips[1].notes[0].pitch = 60;
    bus.execute(insert(&t), Origin::Mcp).unwrap();
    links::check(bus.project()).unwrap();
}

/// A content edit that grows the copy being edited (a paste or a duplicate of notes past
/// its end, an overdub) grows every sibling that showed what it showed, in the same batch,
/// so the new notes are heard in every copy; a sibling trimmed by hand keeps its window.
#[test]
fn a_grown_copy_grows_the_siblings_that_showed_what_it_showed() {
    let (mut bus, tracks, a) = fixture();
    let b = linked_copy(&mut bus, tracks[0], a, tracks[1], Ticks::beats(8));
    let c = linked_copy(&mut bus, tracks[0], a, tracks[1], Ticks::beats(16));
    // c is trimmed by hand to its first two beats.
    bus.execute(
        Command::SetClipBounds {
            track: tracks[1],
            clip: c,
            start: Ticks::beats(16),
            length: Ticks::beats(2),
        },
        Origin::User,
    )
    .unwrap();
    let before = normalised(bus.project());
    // Paste a note at content beat 6 into b: it grows to 7 beats.
    let target = clip(&bus, tracks[1], b);
    let note = bus.new_note(Ticks::ZERO, Ticks::QUARTER, 72, 90);
    let mut ids = Counter::of(&bus);
    let (mut commands, _) =
        clipboard::paste_notes(tracks[1], &target, &[note], Ticks::beats(6), &mut || {
            ids.take()
        });
    commands.extend(links::follow_window(
        bus.project(),
        (tracks[1], b),
        &commands,
    ));
    while bus.project().next_id < ids.0 {
        bus.new_note(Ticks::ZERO, Ticks::QUARTER, 60, 100);
    }
    run(&mut bus, "cmd.paste", commands);
    let (ca, cb, cc) = (
        clip(&bus, tracks[0], a),
        clip(&bus, tracks[1], b),
        clip(&bus, tracks[1], c),
    );
    assert!(cb.length > Ticks::beats(6));
    assert_eq!(
        (ca.start, ca.length, ca.offset),
        (Ticks::ZERO, cb.length, cb.offset)
    );
    assert_eq!(
        cc.length,
        Ticks::beats(2),
        "a copy trimmed by hand keeps its window"
    );
    // The new note is heard in a and b, not in c.
    let heard: Vec<Ticks> = Snapshot::compile(bus.project(), 1)
        .tracks
        .iter()
        .flat_map(|t| t.events.iter())
        .filter(|e| e.is_on() && e.pitch == 72)
        .map(|e| e.tick)
        .collect();
    assert_eq!(heard.len(), 2, "{heard:?}");
    links::check(bus.project()).unwrap();
    bus.undo();
    assert_eq!(normalised(bus.project()), before);
}
