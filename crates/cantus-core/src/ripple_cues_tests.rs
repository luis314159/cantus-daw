//! R2-B2: every time edit keeps the scene each piece of music plays under — on random songs
//! with random cues (whole-desk and scoped, snapshots of a few channels or all, a
//! recall-safe channel), after insert and delete time and the four section drags. Where
//! the music went is derived here on its own (stretches of the new song and where each
//! began in the old), not from the builders' pieces, so a mistake there shows.

use super::*;
use crate::mix_snap::{capture, MixSnapshot};
use crate::ripple::tests::{bar, with_ids};
use crate::ripple::{delete_time, insert_time};
use crate::ripple_bars::{move_bars, Landing};

const END: i64 = i64::MAX / 4;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn coin(&mut self) -> bool {
        self.below(2) == 0
    }
}

/// A song of four channels (one recall-safe now and then), three snapshots of a few
/// channels each (the master in some), and up to seven cues on beats of the first 12 bars.
fn song(r: &mut Rng) -> Project {
    let mut p = Project::new("Cues");
    for n in 0..4 {
        let mut s = p.make_strip(format!("ch {n}"));
        s.recall_safe = n == 3 && r.coin();
        p.strips.push(s);
    }
    let strips: Vec<StripId> = p.strips.iter().map(|s| s.id).collect();
    let mut snaps = Vec::new();
    for n in 0..3 {
        let scope: Vec<StripId> = if r.coin() {
            Vec::new()
        } else {
            strips.iter().copied().filter(|_| r.coin()).collect()
        };
        let id = MixSnapId(p.alloc_id());
        let state = capture(&p, &scope, r.below(3) > 0);
        p.mix.snapshots.push(MixSnapshot {
            id,
            name: format!("S{n}"),
            state,
        });
        snaps.push(id);
    }
    for _ in 0..1 + r.below(7) {
        let scope: Vec<StripId> = if r.coin() {
            Vec::new()
        } else {
            strips.iter().copied().filter(|_| r.coin()).collect()
        };
        let id = MixCueId(p.alloc_id());
        p.mix.cues.push(MixCue {
            id,
            at: Ticks(Ticks::beats(1).0 * r.below(48) as i64),
            snapshot: snaps[r.below(3) as usize],
            scope,
        });
    }
    p
}

/// `(new start, old start, length)`: what of the old song each stretch of the new plays.
type Segs = Vec<(i64, i64, i64)>;

/// `segs` with the new stretch `[a, b)` taken out (its music gone; nothing moves).
fn cut(segs: Segs, a: i64, b: i64) -> Segs {
    let mut out = Vec::new();
    for (n, o, l) in segs {
        if n < a {
            out.push((n, o, l.min(a - n)));
        }
        if n + l > b {
            let k = (b - n).max(0);
            out.push((n + k, o + k, l - k));
        }
    }
    out
}

/// `segs` with `len` of new time opened at `at`.
fn open(segs: Segs, at: i64, len: i64) -> Segs {
    let mut out = Vec::new();
    for (n, o, l) in segs {
        if n >= at {
            out.push((n + len, o, l));
        } else if n + l > at {
            out.push((n, o, at - n));
            out.push((at + len, o + (at - n), l - (at - n)));
        } else {
            out.push((n, o, l));
        }
    }
    out
}

/// `segs` with `[from, to)` deleted: what followed closes up.
fn close(segs: Segs, from: i64, to: i64) -> Segs {
    let mut out = Vec::new();
    for (n, o, l) in cut(segs, from, to) {
        out.push(if n >= to {
            (n - (to - from), o, l)
        } else {
            (n, o, l)
        });
    }
    out
}

/// The scene each stretch plays, before and after, at every place it could change: the
/// stretch's start and every cue inside it, old or new.
fn holds(before: &Project, after: &Project, segs: &Segs, what: &str) {
    for &(n, o, l) in segs {
        let mut at: Vec<i64> = vec![0];
        at.extend(before.mix.cues.iter().map(|c| c.at.0 - o));
        at.extend(after.mix.cues.iter().map(|c| c.at.0 - n));
        for d in at.into_iter().filter(|d| *d >= 0 && *d < l) {
            let (y, x) = (Ticks(n + d), Ticks(o + d));
            let (now, was) = (scene_at(after, y), scene_at(before, x));
            // Every channel a cue had set for this music plays that cue's snapshot again.
            // One no cue had set yet played the desk's own values; where the music now
            // lies after a cue, that cue's (a cue cannot unset a channel; ADR 0116 §2).
            let wrong: Vec<_> = was
                .iter()
                .filter(|(ch, s)| now.get(*ch) != Some(*s))
                .map(|(ch, s)| (*ch, *s, now.get(ch).copied()))
                .collect();
            assert!(
                wrong.is_empty(),
                "{what}: new {y:?} (old {x:?}) plays another scene: {wrong:?}
before {:?}
after {:?}",
                before.mix.cues,
                after.mix.cues
            );
            // And where the music had no cue before, it has none now unless one came
            // before its new place.
            if was.is_empty() && y.0 == n {
                let earlier = after.mix.cues.iter().any(|c| c.at < y);
                assert!(
                    now.is_empty() || earlier,
                    "{what}: a cue made for music that had none"
                );
            }
        }
    }
}

#[test]
fn every_time_edit_keeps_the_scene_each_piece_of_music_plays_under() {
    let mut r = Rng(0x9e37_79b9_7f4a_7c15);
    let beat = Ticks::beats(1).0;
    for round in 0..3000 {
        let mut p = song(&mut r);
        let a = beat * r.below(44) as i64;
        let len = beat * (1 + r.below(16) as i64);
        let kind = r.below(6);
        let (cmd, segs, what) = match kind {
            0 => (
                with_ids(&mut p, |p, ids| insert_time(p, Ticks(a), Ticks(len), ids)),
                open(vec![(0, 0, END)], a, len),
                "insert time",
            ),
            1 => (
                with_ids(&mut p, |p, ids| {
                    delete_time(p, Ticks(a), Ticks(a + len), ids)
                }),
                close(vec![(0, 0, END)], a, a + len),
                "delete time",
            ),
            _ => {
                let how = Landing {
                    copy: kind >= 4,
                    insert: kind % 2 == 1,
                };
                let dest = beat * r.below(48) as i64;
                let land = how.lands_at(Ticks(a), Ticks(a + len), Ticks(dest)).0;
                let base = match (how.insert, how.copy) {
                    (false, false) => cut(cut(vec![(0, 0, END)], a, a + len), land, land + len),
                    (false, true) => cut(vec![(0, 0, END)], land, land + len),
                    (true, false) => open(close(vec![(0, 0, END)], a, a + len), land, len),
                    (true, true) => open(vec![(0, 0, END)], land, len),
                };
                let mut segs = base;
                segs.push((land, a, len));
                (
                    with_ids(&mut p, |p, ids| {
                        move_bars(p, Ticks(a), Ticks(a + len), Ticks(dest), how, ids)
                    }),
                    segs,
                    how.label(),
                )
            }
        };
        let Some(cmd) = cmd else {
            continue;
        };
        let mut q = p.clone();
        let inverse = cmd.apply(&mut q).expect("the edit applies");
        holds(
            &p,
            &q,
            &segs,
            &format!("round {round}, {what} at {a} for {len}"),
        );
        // One undo step gives the song back.
        inverse.apply(&mut q).expect("its inverse applies");
        assert_eq!(q.mix, p.mix, "round {round}: {what} undone");
    }
}

// The reviewer's case: deleting time whose last cue is inside the range, with no cue at its
// end — what follows keeps the scene that cue gave it.
#[test]
fn deleting_time_keeps_the_scene_of_what_follows() {
    let mut p = Project::new("Cues");
    let s = p.make_strip("ch");
    p.strips.push(s);
    let (a, b) = (MixSnapId(p.alloc_id()), MixSnapId(p.alloc_id()));
    for id in [a, b] {
        let state = capture(&p, &[], true);
        p.mix.snapshots.push(MixSnapshot {
            id,
            name: "S".into(),
            state,
        });
    }
    for (at, snap) in [(bar(1), a), (bar(5), b)] {
        let id = MixCueId(p.alloc_id());
        p.mix.cues.push(MixCue {
            id,
            at,
            snapshot: snap,
            scope: Vec::new(),
        });
    }
    let cmd = with_ids(&mut p, |p, ids| delete_time(p, bar(4), bar(8), ids)).unwrap();
    let mut q = p.clone();
    cmd.apply(&mut q).unwrap();
    assert_eq!(
        scene_at(&q, bar(4)),
        scene_at(&p, bar(8)),
        "the chorus's scene kept"
    );
    assert_eq!(
        scene_at(&q, bar(3)),
        scene_at(&p, bar(3)),
        "before the range, as it was"
    );
    assert_eq!(
        q.mix.cues.len(),
        2,
        "one cue gone, one cue of its own at the seam"
    );
}

// R2-B2, the every-track ripple: a clip deleted on one track closes its time on every
// track; a cue inside it that another track still plays over stays with that music, and
// what moved up starts under the scene it had.
#[test]
fn every_track_ripple_keeps_a_cue_another_track_still_plays_and_the_moved_musics_scene() {
    use crate::ripple_mode::{rippled, Follow, RippleMode};
    for still in [true, false] {
        let mut p = Project::new("Ripple");
        let (a, b, c) = (
            MixSnapId(p.alloc_id()),
            MixSnapId(p.alloc_id()),
            MixSnapId(p.alloc_id()),
        );
        for id in [a, b, c] {
            let state = capture(&p, &[], true);
            p.mix.snapshots.push(MixSnapshot {
                id,
                name: "S".into(),
                state,
            });
        }
        let mut cue_ids = Vec::new();
        for (at, snap) in [(bar(1), a), (bar(5), b), (bar(10), c)] {
            let id = MixCueId(p.alloc_id());
            cue_ids.push(id);
            p.mix.cues.push(MixCue {
                id,
                at,
                snapshot: snap,
                scope: Vec::new(),
            });
        }
        let mut one = p.make_track("One");
        let gone = p.make_clip("gone", bar(4), bar(4));
        let gone_id = gone.id;
        one.clips.push(gone);
        let one_id = one.id;
        p.tracks.push(one);
        if still {
            let mut two = p.make_track("Two");
            let long = p.make_clip("long", bar(0), bar(16));
            two.clips.push(long);
            p.tracks.push(two);
        }
        let follow = Follow {
            track: one_id,
            at: bar(8),
            delta: Ticks::ZERO - bar(4),
            exclude: Vec::new(),
        };
        let gesture = vec![Command::RemoveClip {
            track: one_id,
            clip: gone_id,
        }];
        let cmd = with_ids(&mut p, |q, ids| {
            rippled(
                q,
                gesture,
                "cmd.delete_clips",
                RippleMode::All,
                &[follow],
                ids,
            )
        })
        .expect("a ripple");
        let mut q = p.clone();
        cmd.apply(&mut q).unwrap();
        let at = |id| q.mix.cues.iter().find(|x| x.id == id).map(|x| x.at);
        assert_eq!(at(cue_ids[2]), Some(bar(6)), "the later cue moved up");
        assert_eq!(
            at(cue_ids[1]),
            still.then_some(bar(5)),
            "the cue inside: kept where the other track plays ({still})"
        );
        assert_eq!(
            scene_at(&q, bar(4)),
            scene_at(&p, bar(8)),
            "what moved up starts under its scene ({still})"
        );
        assert_eq!(
            scene_at(&q, bar(3)),
            scene_at(&p, bar(3)),
            "before, as it was"
        );
    }
}
