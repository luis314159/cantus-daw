//! r40:ripple — insert and delete time (ADR 0116): each part of the song through an edit,
//! the edit's exact inverse, and the property the owner asked for: inserting time and
//! deleting the same range gives the song back, to the tick, on random songs.

use super::*;
use crate::arrange::{Section, SectionId};
use crate::model::{Clip, LoopRegion, Project};
use crate::tempo::{Marker, MarkerId, MetreChange, TempoPoint};
use crate::time::TimeSignature;
use crate::{AudioSourceId, CommandBus, Origin};

pub(crate) fn bar(n: i64) -> Ticks {
    Ticks(Ticks::beats(4).0 * n)
}

/// An id counter over a project (what the app's `with_ids` hands the builders).
pub(crate) fn with_ids<R>(p: &mut Project, f: impl FnOnce(&Project, Ids) -> R) -> R {
    let mut next = p.next_id;
    let out = f(p, &mut || {
        let id = next;
        next += 1;
        id
    });
    p.next_id = next;
    out
}

pub(crate) fn note_clip(p: &mut Project, start: Ticks, len: Ticks) -> Clip {
    let mut c = p.make_clip("n", start, len);
    for i in 0..(len.0 / Ticks::beats(1).0).max(1) {
        let n = p.make_note(Ticks::beats(i), Ticks(240), 60 + (i % 12) as u8, 100);
        c.notes.push(n);
    }
    c
}

/// Two tracks: a clip across bar 4 (bars 2–6), one at bar 8, a marker at bar 8, a section
/// across bar 4 (the verse, bars 0–8) and one after (the chorus, 8–16), the loop over bars
/// 3–5, the punch over 9–10.
pub(crate) fn song() -> Project {
    let mut p = Project::empty("ripple");
    let mut a = p.make_track("A");
    let across = note_clip(&mut p, bar(2), bar(4));
    let later = note_clip(&mut p, bar(8), bar(2));
    a.clips = vec![across, later];
    let mut b = p.make_track("B");
    b.clips = vec![
        note_clip(&mut p, bar(0), bar(1)),
        note_clip(&mut p, bar(5), bar(1)),
    ];
    p.tracks = vec![a, b];
    p.markers = vec![Marker {
        id: MarkerId(p.alloc_id()),
        tick: bar(8),
        name: "Vox in".into(),
        color: None,
    }];
    for (s, l, n) in [(0, 8, "Verse"), (8, 8, "Chorus")] {
        let id = SectionId(p.alloc_id());
        p.sections.push(Section {
            id,
            start: bar(s),
            length: bar(l),
            name: n.into(),
            color: None,
        });
    }
    p.loop_region = Some(LoopRegion {
        start: bar(3),
        end: bar(5),
    });
    p.recording.punch = Some(LoopRegion {
        start: bar(9),
        end: bar(10),
    });
    p
}

fn exec(p: &Project, c: Command) -> Project {
    let mut q = p.clone();
    c.apply(&mut q).expect("the edit applies");
    q
}

fn starts(p: &Project, track: usize) -> Vec<(Ticks, Ticks)> {
    p.tracks[track]
        .clips
        .iter()
        .map(|c| (c.start, c.length))
        .collect()
}

#[test]
fn inserting_time_splits_what_runs_across_and_moves_everything_after() {
    let mut p = song();
    let cmd = with_ids(&mut p, |p, ids| insert_time(p, bar(4), bar(2), ids)).unwrap();
    assert_eq!(cmd.label_key(), INSERT_LABEL);
    let q = exec(&p, cmd);
    // Track A: bars 2–4 stay, 4–6 is now 6–8 (the same notes, the window going on), 8 → 10.
    assert_eq!(
        starts(&q, 0),
        vec![(bar(2), bar(2)), (bar(6), bar(2)), (bar(10), bar(2))]
    );
    let right = &q.tracks[0].clips[1];
    assert_eq!(
        right.offset,
        bar(2),
        "the right half shows what came after the cut"
    );
    assert_eq!(right.notes, p.tracks[0].clips[0].notes, "no note moved");
    assert_eq!(starts(&q, 1), vec![(bar(0), bar(1)), (bar(7), bar(1))]);
    assert_eq!(q.markers[0].tick, bar(10));
    // The verse holds the new bars; the chorus moves.
    assert_eq!(
        (q.sections[0].start, q.sections[0].length),
        (bar(0), bar(10))
    );
    assert_eq!(
        (q.sections[1].start, q.sections[1].length),
        (bar(10), bar(8))
    );
    // The loop across the cut grows; the punch after it moves.
    assert_eq!(
        q.loop_region,
        Some(LoopRegion {
            start: bar(3),
            end: bar(7)
        })
    );
    assert_eq!(
        q.recording.punch,
        Some(LoopRegion {
            start: bar(11),
            end: bar(12)
        })
    );
}

#[test]
fn deleting_time_takes_what_is_inside_and_closes_the_gap() {
    let mut p = song();
    let cmd = with_ids(&mut p, |p, ids| delete_time(p, bar(4), bar(6), ids)).unwrap();
    assert_eq!(cmd.label_key(), DELETE_LABEL);
    let q = exec(&p, cmd);
    // A: 2–4 kept, 4–6 gone; the clip at 8 now at 6.
    assert_eq!(starts(&q, 0), vec![(bar(2), bar(2)), (bar(6), bar(2))]);
    // B: the clip in bar 5 was inside: gone.
    assert_eq!(starts(&q, 1), vec![(bar(0), bar(1))]);
    assert_eq!(q.markers[0].tick, bar(6));
    assert_eq!(
        (q.sections[0].start, q.sections[0].length),
        (bar(0), bar(6))
    );
    assert_eq!(
        (q.sections[1].start, q.sections[1].length),
        (bar(6), bar(8))
    );
    assert_eq!(
        q.loop_region,
        Some(LoopRegion {
            start: bar(3),
            end: bar(4)
        })
    );
    // A marker inside deleted time goes with it; a loop wholly inside is cleared and off.
    let mut p = song();
    p.loop_enabled = true;
    let cmd = with_ids(&mut p, |p, ids| delete_time(p, bar(3), bar(9), ids)).unwrap();
    let q = exec(&p, cmd);
    assert!(q.markers.is_empty());
    assert_eq!(q.loop_region, None);
    assert!(!q.loop_enabled);
    assert_eq!(
        q.recording.punch,
        Some(LoopRegion {
            start: bar(3),
            end: bar(4)
        })
    );
}

#[test]
fn nothing_to_do_is_no_command() {
    let mut p = song();
    assert!(with_ids(&mut p, |p, ids| insert_time(p, bar(4), Ticks::ZERO, ids)).is_none());
    assert!(with_ids(&mut p, |p, ids| delete_time(p, bar(4), bar(4), ids)).is_none());
    assert!(with_ids(&mut p, |p, ids| delete_time(p, bar(5), bar(4), ids)).is_none());
}

#[test]
fn each_edit_undoes_exactly_through_the_bus() {
    let p = song();
    let mut bus = CommandBus::new(p.clone());
    for k in 0..4 {
        let mut scratch = bus.project().clone();
        let cmd = with_ids(&mut scratch, |q, ids| match k {
            0 => insert_time(q, bar(4), bar(2), ids),
            1 => delete_time(q, bar(1), bar(3), ids),
            2 => insert_time(q, Ticks::ZERO, bar(1), ids),
            _ => delete_time(q, Ticks::ZERO, bar(1), ids),
        })
        .unwrap();
        bus.reserve_ids(scratch.next_id);
        bus.execute(cmd, Origin::User).unwrap();
    }
    let end = bus.project().clone();
    while bus.undo().is_some() {}
    let mut back = bus.project().clone();
    back.next_id = p.next_id;
    assert_eq!(back, p);
    while bus.redo().is_some() {}
    let (mut again, mut end) = (bus.project().clone(), end);
    again.next_id = 0;
    end.next_id = 0;
    assert_eq!(again, end);
}

#[test]
fn the_tempo_track_moves_and_a_cut_ramp_plays_as_it_did() {
    let mut p = song();
    p.tempo_bpm = 100.0;
    p.tempo_points = vec![
        TempoPoint {
            tick: Ticks::ZERO,
            bpm: 100.0,
            ramp: true,
        },
        TempoPoint {
            tick: bar(8),
            bpm: 140.0,
            ramp: false,
        },
        TempoPoint {
            tick: bar(12),
            bpm: 90.0,
            ramp: false,
        },
    ];
    let before = p.tempo_map();
    let cmd = with_ids(&mut p, |p, ids| insert_time(p, bar(4), bar(2), ids)).unwrap();
    let q = exec(&p, cmd);
    let after = q.tempo_map();
    // Before the cut: the same; the new bars hold the ramp's tempo at bar 4; after them the
    // ramp goes on to 140 at bar 10, and 90 holds from bar 14.
    for b in [0, 1, 2, 3] {
        let t = bar(b).0 as f64;
        assert_eq!(after.bpm_at(t), before.bpm_at(t));
    }
    let at4 = before.bpm_at(bar(4).0 as f64);
    assert!((at4 - 120.0).abs() < 1e-9);
    assert_eq!(after.bpm_at(bar(5).0 as f64), after.bpm_at(bar(4).0 as f64));
    for b in [4, 5, 6, 7, 8, 12, 13] {
        let (t0, t1) = (bar(b).0 as f64, (bar(b) + bar(2)).0 as f64);
        assert!(
            (after.bpm_at(t1) - before.bpm_at(t0)).abs() < 1e-9,
            "bar {b}"
        );
    }
    // The seconds after the new bars are the old ones plus the new bars' length.
    let gap = after.seconds_between(bar(4), bar(6));
    for b in [6, 8, 9, 12, 15] {
        let old = before.seconds_at(bar(b).0 as f64);
        let new = after.seconds_at((bar(b) + bar(2)).0 as f64);
        assert!(
            (new - (old + gap)).abs() < 1e-9,
            "bar {b}: {new} vs {old} + {gap}"
        );
    }
    // Deleted again: the tempo track as it was, to the bit.
    let mut q = q;
    let cmd = with_ids(&mut q, |q, ids| delete_time(q, bar(4), bar(6), ids)).unwrap();
    let r = exec(&q, cmd);
    assert_eq!(r.tempo_points, p.tempo_points);
    assert_eq!(r.tempo_bpm, p.tempo_bpm);
}

#[test]
fn metre_changes_move_and_a_deleted_change_hands_its_metre_on() {
    let mut p = song();
    let three = TimeSignature::new(3, 4);
    p.metre_changes = vec![
        MetreChange {
            tick: Ticks::ZERO,
            signature: p.time_signature,
        },
        MetreChange {
            tick: bar(4),
            signature: three,
        },
    ];
    let cmd = with_ids(&mut p, |p, ids| insert_time(p, bar(4), bar(2), ids)).unwrap();
    let q = exec(&p, cmd);
    assert_eq!(q.metre_changes[1].tick, bar(6));
    // Deleting over the change: the 3/4 goes on from where the cut closes.
    let cmd = with_ids(&mut p, |p, ids| delete_time(p, bar(3), bar(5), ids)).unwrap();
    let q = exec(&p, cmd);
    assert_eq!(
        q.metre_changes[1],
        MetreChange {
            tick: bar(3),
            signature: three
        }
    );
    // From the song's start: the metre at tick 0 is the one that was at the range's end.
    let cmd = with_ids(&mut p, |p, ids| delete_time(p, Ticks::ZERO, bar(5), ids)).unwrap();
    let q = exec(&p, cmd);
    assert_eq!(q.time_signature, three);
    assert_eq!(q.metre_changes.len(), 1);
}

#[test]
fn a_looping_clip_cut_between_repeats_keeps_playing_its_loop() {
    let mut p = song();
    let mut c = note_clip(&mut p, bar(0), bar(1));
    c.length = bar(4);
    c.content_length = Some(bar(1));
    let id = c.id;
    p.tracks[1].clips = vec![c.clone()];
    let cmd = with_ids(&mut p, |p, ids| insert_time(p, bar(2), bar(1), ids)).unwrap();
    let q = exec(&p, cmd);
    let clips = &q.tracks[1].clips;
    assert_eq!(clips.len(), 2);
    assert_eq!((clips[0].id, clips[0].length), (id, bar(2)));
    assert_eq!((clips[1].start, clips[1].length), (bar(3), bar(2)));
    assert_eq!(clips[1].content_length, Some(bar(1)));
    // And deleting the bar heals it into the one loop it was.
    let mut q = q;
    let cmd = with_ids(&mut q, |q, ids| delete_time(q, bar(2), bar(3), ids)).unwrap();
    let r = exec(&q, cmd);
    assert_eq!(r.tracks[1].clips, vec![c]);
}

#[test]
fn an_audio_clip_keeps_its_fades_at_its_outer_edges_and_heals_whole() {
    let mut p = song();
    let source = AudioSourceId(p.alloc_id());
    p.audio_pool.push(crate::audio::AudioSource {
        id: source,
        name: "take".into(),
        path: "take.wav".into(),
        sample_rate: 48_000,
        channels: 2,
        frames: 48_000 * 60,
        sha256: None,
        recorded: false,
    });
    let mut c = p.make_audio_clip("a", source, bar(1), bar(4));
    if let Some(a) = c.audio.as_mut() {
        a.fade_in = Ticks::beats(1);
        a.fade_out = Ticks::beats(2);
    }
    p.tracks[1].clips = vec![c.clone()];
    let cmd = with_ids(&mut p, |p, ids| insert_time(p, bar(3), bar(2), ids)).unwrap();
    let q = exec(&p, cmd);
    let (l, r) = (&q.tracks[1].clips[0], &q.tracks[1].clips[1]);
    assert_eq!(l.audio.as_ref().unwrap().fade_out, Ticks::ZERO);
    assert_eq!(r.audio.as_ref().unwrap().fade_in, Ticks::ZERO);
    assert_eq!(r.audio.as_ref().unwrap().fade_out, Ticks::beats(2));
    let mut q = q;
    let cmd = with_ids(&mut q, |q, ids| delete_time(q, bar(3), bar(5), ids)).unwrap();
    assert_eq!(exec(&q, cmd).tracks[1].clips, vec![c]);
}

#[test]
fn delete_time_does_not_glue_pieces_that_do_not_continue_each_other() {
    let mut p = song();
    let cmd = with_ids(&mut p, |p, ids| delete_time(p, bar(3), bar(4), ids)).unwrap();
    let q = exec(&p, cmd);
    // The clip of bars 2–6 lost bar 3: two pieces, the second showing what came after.
    let a = &q.tracks[0].clips;
    assert_eq!((a[0].start, a[0].length), (bar(2), bar(1)));
    assert_eq!(
        (a[1].start, a[1].length, a[1].offset),
        (bar(3), bar(2), bar(2))
    );
}

#[test]
fn the_takes_and_the_alternate_comps_move_with_the_song() {
    let mut p = song();
    let mut take = note_clip(&mut p, bar(6), bar(1));
    take.lane = 1;
    p.tracks[1].clips.push(take);
    let mut kept = note_clip(&mut p, bar(6), bar(1));
    kept.comp = true;
    let audio_free = crate::comp::Comp {
        name: "A".into(),
        windows: Vec::new(),
    };
    let alt = crate::comp::Comp {
        name: "B".into(),
        windows: vec![kept],
    };
    p.tracks[1].comps = vec![audio_free, alt];
    p.tracks[1].active_comp = 0;
    let cmd = with_ids(&mut p, |p, ids| insert_time(p, bar(4), bar(2), ids)).unwrap();
    let q = exec(&p, cmd);
    let take = q.tracks[1].clips.iter().find(|c| c.lane == 1).unwrap();
    assert_eq!(take.start, bar(8));
    assert_eq!(q.tracks[1].comps[1].windows[0].start, bar(8));
}

/// A snapshot and cues at bars 1, 4 and 9 (the mixer's scenes of the song, ADR 0093).
pub(crate) fn with_cues(p: &mut Project) -> Vec<crate::mix_snap::MixCueId> {
    use crate::mix_snap::{MixCue, MixCueId, MixSnapId, MixSnapshot, MixState};
    let snap = MixSnapId(p.alloc_id());
    p.mix.snapshots.push(MixSnapshot {
        id: snap,
        name: "Scene".into(),
        state: MixState::default(),
    });
    let mut ids = Vec::new();
    for b in [9, 1, 4] {
        let id = MixCueId(p.alloc_id());
        p.mix.cues.push(MixCue {
            id,
            at: bar(b),
            snapshot: snap,
            scope: Vec::new(),
        });
        ids.push(id);
    }
    ids
}

fn cue_at(p: &Project, id: crate::mix_snap::MixCueId) -> Option<Ticks> {
    p.mix.cues.iter().find(|c| c.id == id).map(|c| c.at)
}

// Review mid-round 40, B-1: the mixer's cues move with the music as the markers do.
#[test]
fn the_mixers_cues_move_with_the_time_and_go_with_deleted_time() {
    let mut p = song();
    let cues = with_cues(&mut p);
    let ins = with_ids(&mut p, |q, ids| insert_time(q, bar(4), bar(2), ids)).unwrap();
    let q = exec(&p, ins.clone());
    assert_eq!(cue_at(&q, cues[0]), Some(bar(11)), "after: moved on");
    assert_eq!(cue_at(&q, cues[1]), Some(bar(1)), "before: kept");
    assert_eq!(cue_at(&q, cues[2]), Some(bar(6)), "at the point: moved on");
    let order: Vec<_> = q.mix.cues.iter().map(|c| c.id).collect();
    assert_eq!(order, cues, "the list keeps its order");
    // Deleting bars 3–5: the cue at bar 4 goes, the one at 9 moves back.
    let del = with_ids(&mut p, |q, ids| delete_time(q, bar(3), bar(5), ids)).unwrap();
    let r = exec(&p, del.clone());
    assert_eq!(cue_at(&r, cues[2]), None, "inside: gone");
    assert_eq!(cue_at(&r, cues[0]), Some(bar(7)));
    assert_eq!(cue_at(&r, cues[1]), Some(bar(1)));
    // Each undoes exactly.
    for c in [ins, del] {
        let mut s = p.clone();
        let inv = c.apply(&mut s).unwrap();
        inv.apply(&mut s).unwrap();
        assert_eq!(s, p);
    }
}
