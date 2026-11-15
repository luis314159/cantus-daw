//! r40:ripple — the property the owner asked for (ADR 0116): on random songs, inserting time
//! and deleting the same range gives the song back to the tick; and every time edit and
//! section drag undoes exactly through the bus.
//!
//! The songs hold note clips (some looping, some linked, some on take lanes), automation
//! clips, audio clips (forward, reversed, faded), markers, sections, metre changes on bar
//! lines, a tempo track (steps and ramps), the mixer's cues, the loop and the punch. Two
//! limits are the model's, said where they are generated: a looping clip cut where no repeat
//! begins is written out (the model has no loop phase), so loops are cut on their repeats
//! here; and an unwarped audio window under a tempo track that changes over its hidden part
//! is re-laid in seconds, so audio songs have no tempo track here (its own test holds it).

use cantus_core::mix_snap::{MixCue, MixCueId, MixSnapId, MixSnapshot, MixState};
use cantus_core::ripple::{delete_time, insert_time};
use cantus_core::ripple_bars::{move_bars, Landing};
use cantus_core::tempo::{Marker, MarkerId, MetreChange, TempoPoint};
use cantus_core::*;

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn chance(&mut self, one_in: u64) -> bool {
        self.below(one_in) == 0
    }
}

fn bar(n: i64) -> Ticks {
    Ticks(Ticks::beats(4).0 * n)
}

fn with_ids<R>(p: &mut Project, f: impl FnOnce(&Project, &mut dyn FnMut() -> u64) -> R) -> R {
    let mut next = p.next_id;
    let out = f(p, &mut || {
        let id = next;
        next += 1;
        id
    });
    p.next_id = next;
    out
}

/// A random song (see the module's docs for its two limits). `audio`: audio clips, and no
/// tempo track.
fn random_song(rng: &mut Lcg, audio: bool) -> Project {
    let mut p = Project::empty("random");
    let source_id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id: source_id,
        name: "take".into(),
        path: "take.wav".into(),
        sample_rate: 48_000,
        channels: 2,
        frames: 48_000 * 600,
        sha256: None,
        recorded: false,
    });
    let source = p.audio_pool[0].id;
    let link = LinkId(p.alloc_id());
    let mut linked: Option<Clip> = None;
    for ti in 0..1 + rng.below(4) {
        let mut t = p.make_track(format!("T{ti}"));
        let mut at = Ticks(rng.below(4) as i64 * Ticks::beats(1).0);
        for _ in 0..rng.below(6) {
            let len = Ticks((1 + rng.below(24) as i64) * Ticks::beats(1).0 / 2);
            let kind = rng.below(6);
            let mut c = match kind {
                0 if audio => {
                    let mut c = p.make_audio_clip("a", source, at, len);
                    c.offset = Ticks(rng.below(8) as i64 * 240);
                    if let Some(a) = c.audio.as_mut() {
                        a.reverse = rng.chance(4);
                        a.fade_in = Ticks(rng.below(3) as i64 * 60);
                        a.fade_out = Ticks(rng.below(3) as i64 * 60);
                    }
                    c
                }
                1 => p.make_automation_clip(
                    "auto",
                    AutoTarget::TrackGain(t.id),
                    at,
                    len,
                    vec![
                        AutoPoint::new(Ticks::ZERO, 0.2),
                        AutoPoint::new(Ticks::beats(3), 0.9),
                    ],
                ),
                _ => {
                    let mut c = p.make_clip("n", at, len);
                    for i in 0..1 + rng.below(6) as i64 {
                        let n = p.make_note(Ticks(i * 300), Ticks(200), 50 + i as u8, 90);
                        c.notes.push(n);
                    }
                    c.offset = Ticks(rng.below(4) as i64 * 120);
                    if kind == 2 && rng.chance(2) {
                        // A loop of one bar, on the bar: cut on its repeats (see above).
                        c.start = bar(c.start.0 / bar(1).0 + 1);
                        c.length = bar(1 + rng.below(3) as i64);
                        c.offset = Ticks::ZERO;
                        c.content_length = Some(bar(1));
                    }
                    c
                }
            };
            if kind == 3 {
                c.lane = 1;
            }
            if kind == 4 && c.audio.is_none() && c.automation.is_none() {
                // The same clip again, linked (ADR 0037).
                match &linked {
                    Some(l) => {
                        let id = c.id;
                        c = Clip {
                            id,
                            start: c.start,
                            length: c.length,
                            ..l.clone()
                        };
                    }
                    None => {
                        c.link = Some(link);
                        linked = Some(c.clone());
                    }
                }
            }
            at = c.end() + Ticks(rng.below(3) as i64 * 480);
            t.clips.push(c);
        }
        t.clips.sort_by_key(|c| (c.start, c.id));
        p.tracks.push(t);
    }
    let mut s = Ticks::ZERO;
    for i in 0..rng.below(4) {
        let len = bar(1 + rng.below(8) as i64);
        let start = s + bar(rng.below(3) as i64);
        let id = SectionId(p.alloc_id());
        p.sections.push(Section {
            id,
            start,
            length: len,
            name: format!("S{i}"),
            color: None,
        });
        s = start + len;
    }
    for i in 0..rng.below(4) {
        let id = MarkerId(p.alloc_id());
        p.markers.push(Marker {
            id,
            tick: Ticks(rng.below(64) as i64 * 480),
            name: format!("M{i}"),
            color: None,
        });
    }
    p.markers.sort_by_key(|m| (m.tick, m.id));
    // The mixer's cues (review mid-round 40, B-1), in the order they were taken.
    let cues = rng.below(4);
    if cues > 0 {
        let snapshot = MixSnapId(p.alloc_id());
        p.mix.snapshots.push(MixSnapshot {
            id: snapshot,
            name: "Scene".into(),
            state: MixState::default(),
        });
        for _ in 0..cues {
            let id = MixCueId(p.alloc_id());
            p.mix.cues.push(MixCue {
                id,
                at: Ticks(rng.below(64) as i64 * 480),
                snapshot,
                scope: Vec::new(),
            });
        }
    }
    if rng.chance(2) {
        let a = Ticks(rng.below(32) as i64 * 480);
        p.loop_region = Some(LoopRegion {
            start: a,
            end: a + Ticks((1 + rng.below(16) as i64) * 480),
        });
        p.loop_enabled = rng.chance(2);
    }
    if rng.chance(2) {
        let a = Ticks(rng.below(32) as i64 * 480);
        p.recording.punch = Some(LoopRegion {
            start: a,
            end: a + Ticks((1 + rng.below(16) as i64) * 480),
        });
        p.recording.punch_enabled = rng.chance(2);
    }
    if rng.chance(2) {
        // Metre changes on bar lines, each a new metre (none repeats the one before it).
        let sigs = [TimeSignature::new(4, 4), TimeSignature::new(3, 4)];
        let mut m = vec![MetreChange {
            tick: Ticks::ZERO,
            signature: sigs[0],
        }];
        let mut tick = Ticks::ZERO;
        for i in 1..1 + rng.below(3) as usize {
            tick += bar(1 + rng.below(6) as i64);
            m.push(MetreChange {
                tick,
                signature: sigs[i % 2],
            });
        }
        p.metre_changes = m;
    }
    if !audio && rng.chance(2) {
        // A tempo track with no point the music does not need: each a new tempo.
        let mut pts = Vec::new();
        let mut tick = Ticks::ZERO;
        for i in 0..1 + rng.below(4) as usize {
            pts.push(TempoPoint {
                tick,
                bpm: 90.0 + 13.0 * i as f64 + rng.below(7) as f64,
                ramp: rng.chance(2),
            });
            tick += Ticks((1 + rng.below(24) as i64) * 480);
        }
        p.tempo_bpm = pts[0].bpm;
        p.tempo_points = pts;
    }
    p
}

/// A random edit point: on a bar line most of the time (where loops repeat), else anywhere
/// on the eighth-note grid when no clip loops.
fn random_point(rng: &mut Lcg, p: &Project) -> Ticks {
    let loops = p
        .tracks
        .iter()
        .any(|t| t.clips.iter().any(|c| c.content_length.is_some()));
    if loops || rng.chance(2) {
        bar(rng.below(12) as i64)
    } else {
        Ticks(rng.below(96) as i64 * 480)
    }
}

#[test]
fn inserting_then_deleting_the_same_range_is_the_identity_to_the_tick() {
    let mut cases = 0;
    for seed in 1..=400u64 {
        let mut rng = Lcg(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(7));
        let audio = rng.chance(2);
        let mut p = random_song(&mut rng, audio);
        let at = random_point(&mut rng, &p);
        let len = if rng.chance(2) {
            bar(1 + rng.below(4) as i64)
        } else {
            Ticks((1 + rng.below(16) as i64) * 240)
        };
        let len = if p
            .tracks
            .iter()
            .any(|t| t.clips.iter().any(|c| c.content_length.is_some()))
        {
            bar(len.0 / bar(1).0 + 1)
        } else {
            len
        };
        let original = p.clone();
        let Some(ins) = with_ids(&mut p, |q, ids| insert_time(q, at, len, ids)) else {
            // Nothing at or after the point: an insert there changes nothing (no command).
            let reaches = |t: Ticks| t >= at;
            let moves = p
                .tracks
                .iter()
                .any(|t| t.clips.iter().any(|c| c.end() > at))
                || p.markers.iter().any(|m| reaches(m.tick))
                || p.mix.cues.iter().any(|c| reaches(c.at))
                || p.sections.iter().any(|s| s.end() > at)
                || p.loop_region.is_some_and(|r| r.end > at)
                || p.recording.punch.is_some_and(|r| r.end > at)
                || p.tempo_points
                    .iter()
                    .any(|q| q.tick >= at && q.tick > Ticks::ZERO)
                || p.metre_changes
                    .iter()
                    .any(|m| m.tick >= at && m.tick > Ticks::ZERO);
            assert!(!moves, "seed {seed}: an insert at {at:?} was refused");
            continue;
        };
        let mut q = p.clone();
        ins.apply(&mut q).expect("the insert applies");
        let del = with_ids(&mut q, |r, ids| delete_time(r, at, at + len, ids)).expect("a delete");
        let mut r = q.clone();
        del.apply(&mut r).expect("the delete applies");
        r.next_id = original.next_id;
        assert_eq!(
            r, original,
            "seed {seed}: insert {len:?} at {at:?} then delete it"
        );
        cases += 1;
    }
    assert!(cases > 300, "{cases} of 400 songs had something to move");
}

/// The kinds a sequence draws from: insert, delete, and the four section drags.
const KINDS: usize = 6;

#[test]
fn time_edits_and_section_drags_undo_and_redo_whole() {
    let mut done = [0usize; KINDS];
    for seed in 1..=120u64 {
        let mut rng = Lcg(seed.wrapping_mul(0x2545_F491_4F6C_DD1D).wrapping_add(3));
        let audio = rng.chance(2);
        let song = random_song(&mut rng, audio);
        let mut bus = CommandBus::new(song.clone());
        for _ in 0..8 {
            let mut scratch = bus.project().clone();
            let k = rng.below(KINDS as u64) as usize;
            let a = Ticks(rng.below(64) as i64 * 240);
            let b = a + Ticks((1 + rng.below(32) as i64) * 240);
            let dest = Ticks(rng.below(64) as i64 * 240);
            let cmd = with_ids(&mut scratch, |p, ids| match k {
                0 => insert_time(p, a, b - a, ids),
                1 => delete_time(p, a, b, ids),
                _ => {
                    let how = Landing {
                        copy: k == 3 || k == 5,
                        insert: k >= 4,
                    };
                    move_bars(p, a, b, dest, how, ids)
                }
            });
            let Some(cmd) = cmd else {
                continue;
            };
            bus.reserve_ids(scratch.next_id);
            if bus.execute(cmd, Origin::User).is_ok() {
                done[k] += 1;
            }
        }
        let mut end = bus.project().clone();
        while bus.undo().is_some() {}
        let mut back = bus.project().clone();
        back.next_id = song.next_id;
        assert_eq!(back, song, "seed {seed}: undo all");
        while bus.redo().is_some() {}
        let mut again = bus.project().clone();
        again.next_id = 0;
        end.next_id = 0;
        assert_eq!(again, end, "seed {seed}: redo all");
    }
    assert!(done.iter().all(|&n| n > 10), "every kind ran: {done:?}");
}
