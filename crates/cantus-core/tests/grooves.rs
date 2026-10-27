//! Grooves and note effects in the model (round 31, ADR 0069): a groove at no amount, or a
//! straight swing, compiles the events a clip without one compiles; a swing moves every
//! second step of every repetition; a linked copy shares its groove; *Commit* writes what
//! was heard; a baked clip's events pass the note effects; a track coming in with a chain
//! another track holds is refused.

use cantus_core::groove::*;
use cantus_core::notefx::{NoteEffect, NoteFxId, NoteFxType};
use cantus_core::*;

/// One track, one clip of 16ths (C D E F …) over a bar, looping over two bars.
fn song() -> (Project, TrackId, ClipId) {
    let mut p = Project::new("grooves");
    let mut t = p.make_track("Lead");
    let mut c = p.make_clip("16ths", Ticks::ZERO, Ticks::beats(8));
    for k in 0..16 {
        let n = p.make_note(Ticks::SIXTEENTH * k, Ticks(120), 60 + (k % 12) as u8, 100);
        c.notes.push(n);
    }
    c.content_length = Some(Ticks::beats(4));
    let (tid, cid) = (t.id, c.id);
    t.clips.push(c);
    p.tracks.push(t);
    (p, tid, cid)
}

fn events(p: &Project) -> Vec<NoteEvent> {
    Snapshot::compile(p, 1).tracks[0].events.clone()
}

fn with_groove(mut p: Project, g: Option<ClipGroove>) -> Project {
    p.tracks[0].clips[0].groove = g;
    p
}

#[test]
fn no_amount_or_a_straight_swing_compiles_as_no_groove() {
    let (p, _, _) = song();
    let plain = events(&p);
    let sw = GrooveSource::Swing {
        grid: SwingGrid::Sixteenths,
        percent: 66,
    };
    let zero = ClipGroove {
        source: sw,
        timing: 0.0,
        velocity: 0.0,
        length: 0.0,
    };
    assert_eq!(events(&with_groove(p.clone(), Some(zero))), plain);
    let straight = ClipGroove::new(GrooveSource::Swing {
        grid: SwingGrid::Eighths,
        percent: 50,
    });
    assert_eq!(events(&with_groove(p.clone(), Some(straight))), plain);
    // A pooled groove that is gone plays the clip as written.
    let gone = ClipGroove::new(GrooveSource::Pool { id: GrooveId(999) });
    assert_eq!(events(&with_groove(p, Some(gone))), plain);
}

#[test]
fn a_swing_moves_every_second_sixteenth_of_every_repetition() {
    let (p, _, _) = song();
    let swung = with_groove(
        p,
        Some(ClipGroove::new(GrooveSource::Swing {
            grid: SwingGrid::Sixteenths,
            percent: 66,
        })),
    );
    let ons: Vec<i64> = events(&swung)
        .iter()
        .filter(|e| e.is_on())
        .map(|e| e.tick.0)
        .collect();
    assert_eq!(ons.len(), 32, "two repetitions of 16");
    for (i, t) in ons.iter().enumerate() {
        let grid = i as i64 * 240;
        let want = if i % 2 == 1 { grid + 77 } else { grid };
        assert_eq!(*t, want, "note {i}");
    }
}

#[test]
fn a_linked_copy_shares_the_groove_and_commit_writes_what_was_heard() {
    let (p, tid, cid) = song();
    let mut bus = CommandBus::new(p);
    let src = bus.project().tracks[0].clips[0].clone();
    let mut next = bus.project().next_id;
    let mut ids = || {
        let id = next;
        next += 1;
        id
    };
    let mut linker = cantus_core::links::Linker::new();
    let (pre, copy) = linker.copy(tid, &src, Ticks::beats(16), &mut ids);
    let copy_id = copy.id;
    let mut commands = pre;
    commands.push(Command::InsertClip {
        track: tid,
        clip: Box::new(copy),
    });
    bus.reserve_ids(next);
    bus.execute(
        Command::Batch {
            label_key: "cmd.duplicate_clips".into(),
            commands,
        },
        Origin::User,
    )
    .unwrap();
    let g = ClipGroove {
        velocity: 0.7,
        ..ClipGroove::new(GrooveSource::Swing {
            grid: SwingGrid::Eighths,
            percent: 62,
        })
    };
    bus.execute(
        Command::SetClipGroove {
            track: tid,
            clip: cid,
            groove: Some(g),
        },
        Origin::User,
    )
    .unwrap();
    let copy = bus.project().tracks[0].clip(copy_id).unwrap();
    assert_eq!(copy.groove, Some(g), "the sibling shares it");
    // Commit on a clip that does not loop: its notes become what was heard.
    let mut flat = bus.project().clone();
    flat.tracks[0].clips.retain(|c| c.id == cid);
    flat.tracks[0].clips[0].content_length = None;
    flat.tracks[0].clips[0].length = Ticks::beats(4);
    flat.tracks[0].clips[0].link = None;
    let heard = events(&flat);
    let notes = committed(&flat.tracks[0].clips[0], &flat.grooves);
    flat.tracks[0].clips[0].notes = notes;
    flat.tracks[0].clips[0].groove = None;
    assert_eq!(
        events(&flat),
        heard,
        "committed, the clip plays what it played"
    );
}

#[test]
fn a_baked_clip_s_events_pass_the_note_effects() {
    let (mut p, _, _) = song();
    let mut other = p.make_clip("baked", Ticks::beats(8), Ticks::beats(4));
    other
        .notes
        .push(p.make_note(Ticks::ZERO, Ticks::QUARTER, 72, 90));
    other.skip_note_fx = true;
    p.tracks[0].clips.push(other);
    let s = Snapshot::compile(&p, 1);
    let t = &s.tracks[0];
    assert_eq!(t.direct.len(), t.events.len());
    for (e, d) in t.events.iter().zip(&t.direct) {
        assert_eq!(*d, e.tick >= Ticks::beats(8), "{e:?}");
    }
    // Without a baked clip the flags are not carried at all.
    let (q, _, _) = song();
    assert!(Snapshot::compile(&q, 1).tracks[0].direct.is_empty());
}

#[test]
fn a_track_cannot_bring_a_chain_id_another_track_holds() {
    let (mut p, tid, _) = song();
    let id = NoteFxId(p.alloc_id());
    let mut bus = CommandBus::new(p);
    bus.execute(
        Command::SetTrackNoteFx {
            track: tid,
            chain: vec![NoteEffect::new(id, NoteFxType::Arp)],
        },
        Origin::User,
    )
    .unwrap();
    let mut t = bus.new_track("Copy");
    t.note_fx = vec![NoteEffect::new(id, NoteFxType::Chord)];
    let refused = bus.execute(
        Command::InsertTrack {
            index: 1,
            track: Box::new(t.clone()),
        },
        Origin::User,
    );
    assert!(refused.is_err());
    // Under ids of its own it comes in.
    let mut next = 10_000u64;
    cantus_core::notefx::renew_ids(&mut t.note_fx, || {
        next += 1;
        next
    });
    bus.reserve_ids(next + 1);
    bus.execute(
        Command::InsertTrack {
            index: 1,
            track: Box::new(t),
        },
        Origin::User,
    )
    .unwrap();
    assert_eq!(bus.project().tracks.len(), 2);
}

#[test]
fn commit_is_refused_when_a_loop_is_not_whole_groove_periods() {
    let (mut p, _, _) = song();
    // A one-bar groove from a played part, on a clip that loops every three beats.
    let hits: Vec<(Ticks, f32, Ticks)> = (0..8)
        .map(|k| {
            (
                Ticks(k * 480 + if k % 2 == 1 { 60 } else { 0 }),
                100.0,
                Ticks(100),
            )
        })
        .collect();
    let id = GrooveId(p.alloc_id());
    p.grooves
        .push(Groove::extract(id, "Bar".into(), &hits, Ticks::beats(4), Ticks::EIGHTH).unwrap());
    let c = &mut p.tracks[0].clips[0];
    c.groove = Some(ClipGroove::new(GrooveSource::Pool { id }));
    c.content_length = Some(Ticks::beats(3));
    assert!(!committed_exactly(&p));
    // Two bars: every pass has the same feel, and the commit plays what was heard.
    p.tracks[0].clips[0].content_length = Some(Ticks::beats(8));
    assert!(committed_exactly(&p));
    let heard = events(&p);
    let notes = committed(&p.tracks[0].clips[0], &p.grooves);
    let mut q = p.clone();
    q.tracks[0].clips[0].notes = notes;
    q.tracks[0].clips[0].groove = None;
    assert_eq!(events(&q), heard);
}

fn committed_exactly(p: &Project) -> bool {
    commit_is_exact(&p.tracks[0].clips[0], &p.grooves)
}

/// The song with a groove of its pool, a bar on 8ths, on its clip.
fn pooled() -> (Project, TrackId, ClipId, GrooveId) {
    let (mut p, t, c) = song();
    let id = GrooveId(p.alloc_id());
    let mut g = Groove::swing(SwingGrid::Eighths, 62);
    g.id = id;
    g.name = "Mine".into();
    p.grooves.push(g);
    p.tracks[0].clips[0].groove = Some(ClipGroove::new(GrooveSource::Pool { id }));
    (p, t, c, id)
}

#[test]
fn a_clip_comes_in_only_with_a_groove_the_pool_holds() {
    let (p, t, _, _) = pooled();
    let mut bus = CommandBus::new(p);
    let mut c = bus.new_clip("x", Ticks::beats(16), Ticks::beats(4));
    c.groove = Some(ClipGroove::new(GrooveSource::Pool {
        id: GrooveId(424_242),
    }));
    let refused = bus.execute(
        Command::InsertClip {
            track: t,
            clip: Box::new(c.clone()),
        },
        Origin::User,
    );
    assert!(refused.is_err(), "a groove not in the pool");
    c.groove = Some(ClipGroove {
        timing: 9.0,
        ..ClipGroove::new(GrooveSource::Swing {
            grid: SwingGrid::Eighths,
            percent: 60,
        })
    });
    let refused = bus.execute(
        Command::InsertClip {
            track: t,
            clip: Box::new(c),
        },
        Origin::User,
    );
    assert!(refused.is_err(), "amounts out of range");
}

fn paste(p: &Project, board: &cantus_core::clipboard::Clipboard, same: bool) -> Project {
    let mut q = p.clone();
    let mut next = q.next_id;
    let mut ids = || {
        let id = next;
        next += 1;
        id
    };
    let pasted = cantus_core::clipboard::paste_clips(
        &q,
        board,
        same,
        0,
        Ticks::beats(32),
        &Default::default(),
        false,
        &mut ids,
    );
    q.next_id = next;
    for c in pasted.commands {
        c.apply(&mut q).expect("the paste applies");
    }
    q
}

#[test]
fn a_pasted_clip_brings_its_groove_or_finds_it() {
    use cantus_core::clipboard::{copy_clips, ClipOrigin, Clipboard};
    let (p, t, c, id) = pooled();
    let board = Clipboard::new(ClipOrigin::default(), &p, copy_clips(&p, &[(t, c)]));
    assert_eq!(board.grooves.len(), 1);
    // Same project, the groove still there: the paste plays it by its id.
    let q = paste(&p, &board, true);
    assert_eq!(q.grooves.len(), 1);
    assert!(q.tracks[0]
        .clips
        .iter()
        .all(|x| x.groove.map(|g| g.source) == Some(GrooveSource::Pool { id })));
    // The groove removed since (the original set to none first): brought back, fresh id.
    let mut gone = p.clone();
    gone.tracks[0].clips[0].groove = None;
    gone.grooves.clear();
    let q = paste(&gone, &board, true);
    assert_eq!(q.grooves.len(), 1);
    let pasted = q.tracks[0]
        .clips
        .iter()
        .find(|x| x.groove.is_some())
        .unwrap();
    assert_eq!(
        pasted.groove.unwrap().source,
        GrooveSource::Pool {
            id: q.grooves[0].id
        }
    );
    assert_eq!(q.grooves[0].points, p.grooves[0].points);
    // Another project whose pool has another groove under the same id: not that one.
    let mut other = song().0;
    while other.next_id <= id.0 {
        other.alloc_id();
    }
    let mut clash = Groove::swing(SwingGrid::Sixteenths, 75);
    clash.id = id;
    other.grooves.push(clash);
    let q = paste(&other, &board, false);
    assert_eq!(q.grooves.len(), 2);
    let pasted = q.tracks[0]
        .clips
        .iter()
        .find(|x| x.groove.is_some())
        .unwrap();
    let GrooveSource::Pool { id: got } = pasted.groove.unwrap().source else {
        panic!("pooled");
    };
    assert_ne!(got, id, "a fresh id, not the clashing groove");
    assert_eq!(
        q.grooves.iter().find(|g| g.id == got).unwrap().points,
        p.grooves[0].points
    );
}
