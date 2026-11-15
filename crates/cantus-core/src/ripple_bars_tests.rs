//! r40:ripple — the section drag's four landings and the ripple mode (ADR 0116 §3, §4).

use super::*;
use crate::ripple::tests::{bar, song, with_ids};
use crate::ripple_mode::{rippled, Follow, RippleMode};

fn exec(p: &Project, c: Command) -> Project {
    let mut q = p.clone();
    c.apply(&mut q).expect("the drag applies");
    q
}

fn drag(p: &mut Project, from: i64, to: i64, dest: i64, copy: bool, insert: bool) -> Project {
    let how = Landing { copy, insert };
    let cmd = with_ids(p, |q, ids| {
        move_bars(q, bar(from), bar(to), bar(dest), how, ids)
    })
    .expect("a drag");
    assert_eq!(cmd.label_key(), how.label());
    exec(p, cmd)
}

fn spans(p: &Project, track: usize) -> Vec<(Ticks, Ticks)> {
    p.tracks[track]
        .clips
        .iter()
        .map(|c| (c.start, c.end()))
        .collect()
}

#[test]
fn a_still_drop_is_no_command() {
    let mut p = song();
    let none = |p: &mut Project, dest, how| {
        with_ids(p, |q, ids| {
            move_bars(q, bar(8), bar(16), bar(dest), how, ids)
        })
        .is_none()
    };
    assert!(none(&mut p, 8, Landing::default()));
    let insert = Landing {
        copy: false,
        insert: true,
    };
    assert!(none(&mut p, 12, insert), "inside its own bars");
    assert!(none(&mut p, 16, insert), "at its own end");
}

#[test]
fn a_drag_moves_the_bars_and_replaces_where_they_land() {
    // The chorus (bars 8–16) dropped at bar 0: the verse's bars 0–8 replaced, 8–16 silent.
    let mut p = song();
    let chorus = p.sections[1].id;
    let q = drag(&mut p, 8, 16, 0, false, false);
    // Track A: the clip at 8–10 now at 0–2; the clip of 2–6 was replaced.
    assert_eq!(spans(&q, 0), vec![(bar(0), bar(2))]);
    assert_eq!(
        q.tracks[0].clips[0].id, p.tracks[0].clips[1].id,
        "the same clip moved"
    );
    assert!(
        spans(&q, 1).is_empty(),
        "B's clips in bars 0–8 replaced by nothing"
    );
    assert_eq!(q.section(chorus).map(|s| s.start), Some(bar(0)));
    assert!(
        q.section(p.sections[0].id).is_none(),
        "the verse was replaced"
    );
    assert_eq!(q.markers[0].tick, bar(0), "its marker went with it");
    assert_eq!(q.tempo_points, p.tempo_points, "the clock stays");
}

#[test]
fn ctrl_copies_the_bars_over_where_they_land() {
    let mut p = song();
    let q = drag(&mut p, 8, 16, 16, true, false);
    assert_eq!(spans(&q, 0)[..2], spans(&p, 0)[..], "the originals stay");
    let copy = q.tracks[0]
        .clips
        .iter()
        .find(|c| c.start == bar(16))
        .unwrap();
    assert_ne!(copy.id, p.tracks[0].clips[1].id);
    assert_eq!(copy.notes.len(), p.tracks[0].clips[1].notes.len());
    assert!(q
        .sections
        .iter()
        .any(|s| s.name == "Chorus" && s.start == bar(16)));
    assert_eq!(q.markers.len(), 2, "the marker copied");
}

#[test]
fn shift_moves_the_bars_in_and_the_song_moves_on() {
    // The chorus (8–16) taken out and put in at bar 4: what was at 4–8 now follows it.
    let mut p = song();
    let q = drag(&mut p, 8, 16, 4, false, true);
    // Put in inside the verse: the verse is cut around it, its rest a section of its own.
    let names: Vec<(&str, Ticks, Ticks)> = q
        .sections
        .iter()
        .map(|s| (s.name.as_str(), s.start, s.end()))
        .collect();
    assert!(names.contains(&("Verse", bar(0), bar(4))), "{names:?}");
    assert!(names.contains(&("Verse", bar(12), bar(16))), "{names:?}");
    let chorus = q.sections.iter().find(|s| s.name == "Chorus").unwrap();
    assert_eq!((chorus.start, chorus.length), (bar(4), bar(8)));
    // Track A: 2–4 kept; the chorus clip (8–10) at 4–6; the verse's 4–6 now at 12–14.
    assert_eq!(
        spans(&q, 0),
        vec![(bar(2), bar(4)), (bar(4), bar(6)), (bar(12), bar(14))]
    );
}

#[test]
fn ctrl_shift_copies_the_bars_in_and_the_song_grows() {
    let mut p = song();
    let q = drag(&mut p, 8, 16, 16, true, true);
    let end = |p: &Project| {
        p.tracks
            .iter()
            .flat_map(|t| &t.clips)
            .map(|c| c.end())
            .max()
    };
    assert_eq!(end(&q), Some(bar(18)), "the copy of 8–10 at 16–18");
    assert_eq!(q.sections.iter().filter(|s| s.name == "Chorus").count(), 2);
}

#[test]
fn the_ripple_mode_moves_what_follows_on_the_track_or_on_every_track() {
    let p = song();
    let (a, b) = (p.tracks[0].id, p.tracks[1].id);
    let first = p.tracks[0].clips[0].clone(); // bars 2–6
    let gone = vec![Command::RemoveClip {
        track: a,
        clip: first.id,
    }];
    let follow = [Follow {
        track: a,
        at: first.end(),
        delta: Ticks::ZERO - first.length,
        exclude: Vec::new(),
    }];
    let run = |mode| {
        let mut p = p.clone();
        let cmd = with_ids(&mut p, |q, ids| {
            rippled(q, gone.clone(), "x", mode, &follow, ids)
        })
        .unwrap();
        exec(&p, cmd)
    };
    let off = run(RippleMode::Off);
    assert_eq!(
        spans(&off, 0),
        vec![(bar(8), bar(10))],
        "off: nothing follows"
    );
    let track = run(RippleMode::Track);
    assert_eq!(
        spans(&track, 0),
        vec![(bar(4), bar(6))],
        "its track closes up"
    );
    assert_eq!(spans(&track, 1), spans(&p, 1), "the other track stays");
    assert_eq!(track.markers, p.markers);
    let all = run(RippleMode::All);
    assert_eq!(spans(&all, 0), vec![(bar(4), bar(6))]);
    // B: the clip at bar 0 stays (before), the one at bar 5 started inside the closed
    // stretch and is left where it was (nothing is cut in the ripple mode).
    assert_eq!(spans(&all, 1), spans(&p, 1));
    assert_eq!(
        all.markers[0].tick,
        bar(4),
        "the song's marks move on every track"
    );
    let _ = b;
}

// Review mid-round 40, B-1: a section's bars carry the mixer's cues in them, in every one of
// the four landings, and what they land on loses its own.
#[test]
fn the_bars_carry_their_cues_and_replace_the_ones_where_they_land() {
    use crate::ripple::tests::with_cues;
    let at = |p: &Project| {
        let mut v: Vec<Ticks> = p.mix.cues.iter().map(|c| c.at).collect();
        v.sort();
        v
    };
    // Cues at bars 1 and 4 (in the verse, 0–8) and 9 (in the chorus, 8–16).
    let mut p = song();
    let ids = with_cues(&mut p);
    // The chorus dropped at bar 0, replacing: its cue at 9 lands at 1; the verse's go.
    let q = drag(&mut p, 8, 16, 0, false, false);
    assert_eq!(at(&q), vec![bar(1)]);
    assert_eq!(q.mix.cues[0].id, ids[0], "the same cue moved");
    // Copied: the chorus keeps its cue and a copy lands at 1.
    let q = drag(&mut p, 8, 16, 0, true, false);
    assert_eq!(at(&q), vec![bar(1), bar(9)]);
    assert!(
        q.mix.cues.iter().any(|c| !ids.contains(&c.id)),
        "a cue of its own"
    );
    // Moved in before the verse: the song rearranged, every cue with its bars.
    let q = drag(&mut p, 8, 16, 0, false, true);
    assert_eq!(at(&q), vec![bar(1), bar(9), bar(12)]);
    // Copied in: the song longer by the chorus, its cue twice.
    let q = drag(&mut p, 8, 16, 0, true, true);
    assert_eq!(at(&q), vec![bar(1), bar(9), bar(12), bar(17)]);
}

// The ripple mode's every-track follow moves the cues after the edit too.
#[test]
fn every_tracks_ripple_moves_the_cues_after_the_edit() {
    use crate::ripple::tests::with_cues;
    let mut p = song();
    let ids = with_cues(&mut p);
    let track = p.tracks[0].id;
    let follow = Follow {
        track,
        at: bar(6),
        delta: bar(2),
        exclude: Vec::new(),
    };
    for (mode, moved) in [(RippleMode::All, bar(11)), (RippleMode::Track, bar(9))] {
        let cmd = with_ids(&mut p, |q, ids| {
            rippled(
                q,
                Vec::new(),
                "cmd.move_clips",
                mode,
                std::slice::from_ref(&follow),
                ids,
            )
        })
        .expect("a ripple");
        let q = exec(&p, cmd);
        let at = q.mix.cues.iter().find(|c| c.id == ids[0]).unwrap().at;
        assert_eq!(at, moved, "{mode:?}");
    }
}
