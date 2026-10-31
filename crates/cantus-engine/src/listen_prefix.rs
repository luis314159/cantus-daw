//! When a checkpoint still holds (round 34, ADR 0083): two snapshots that play the same
//! until a tick leave the engine in the same state there, so a checkpoint taken at that
//! tick under one serves the other.
//!
//! [`same_until`] compares the two with everything that only happens at or after the tick
//! taken out — notes, controller and expression events, audio clips that start there or
//! later, automation pieces that begin after it (the piece playing *at* the tick stays: its
//! value there was read before the checkpoint; one that begins on the tick counts by its value
//! there) — and everything else as it is: an effect's
//! setting, a fader, the routing, the tempo map, a modulator, all of which sound from the
//! start. Conservative by construction: what it does not know how to cut it compares
//! whole, so a difference it cannot place makes the checkpoint fail, never pass.

use cantus_core::{Snapshot, Ticks};

/// `s` as far as the engine has played it when it reaches `tick` (see the module notes).
fn until(s: &Snapshot, tick: Ticks) -> Snapshot {
    let mut s = s.clone();
    s.revision = 0;
    s.content_end = Ticks::ZERO;
    for t in &mut s.tracks {
        // Events and the flags beside them, cut together.
        let keep = t.events.partition_point(|e| e.tick < tick);
        t.events.truncate(keep);
        if t.direct.len() > keep {
            t.direct.truncate(keep);
        }
        t.controls.retain(|c| c.tick < tick);
        t.expr.retain(|e| e.tick < tick);
        t.audio.retain(|a| a.start < tick);
    }
    for lane in &mut s.automation {
        lane.segs.retain(|g| g.at <= tick);
        // A piece that begins exactly at the tick has been read there only: by its value
        // at its start, whatever it does after.
        if let Some(g) = lane.segs.last_mut().filter(|g| g.at == tick) {
            let v = g.pos(tick.0 as f64);
            *g = cantus_core::automation::compile::AutoSeg {
                at: g.at,
                t0: g.at,
                t1: g.at,
                v0: v,
                v1: v,
                shape: cantus_core::automation::Shape::Hold,
                tension: 0.0,
            };
        }
    }
    s.scene_tempos.retain(|(at, _)| *at < tick);
    s
}

/// Whether `a` and `b` play the same until `tick` (exclusive): a checkpoint at `tick`
/// taken under one is a checkpoint of the other.
pub fn same_until(a: &Snapshot, b: &Snapshot, tick: Ticks) -> bool {
    until(a, tick) == until(b, tick)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::Project;

    fn song() -> Project {
        let mut p = Project::new("prefix");
        let mut t = p.make_track("Lead");
        let mut c = p.make_clip("A", Ticks::ZERO, Ticks::beats(16));
        for i in 0..16 {
            let n = p.make_note(Ticks::beats(i), Ticks::QUARTER, 60 + (i % 5) as u8, 100);
            c.notes.push(n);
        }
        t.clips.push(c);
        p.tracks.push(t);
        p
    }

    fn snap(p: &Project) -> Snapshot {
        Snapshot::compile(p, 7)
    }

    #[test]
    fn a_change_after_the_tick_keeps_the_checkpoint_and_one_before_does_not() {
        let p = song();
        let at = Ticks::beats(8);
        let mut later = p.clone();
        later.tracks[0].clips[0].notes[10].pitch = 40;
        assert!(same_until(&snap(&p), &snap(&later), at));
        // A note exactly at the tick has not played yet.
        let mut on = p.clone();
        on.tracks[0].clips[0].notes[8].velocity = 20;
        assert!(same_until(&snap(&p), &snap(&on), at));
        let mut earlier = p.clone();
        earlier.tracks[0].clips[0].notes[3].pitch = 40;
        assert!(!same_until(&snap(&p), &snap(&earlier), at));
        // A setting sounds from the start: never the same.
        let mut fader = p.clone();
        let s = fader.tracks[0].strip.unwrap();
        fader.strip_mut(s).unwrap().gain_db = -2.0;
        assert!(!same_until(&snap(&p), &snap(&fader), at));
        // The revision alone is no difference.
        assert!(same_until(
            &Snapshot::compile(&p, 1),
            &Snapshot::compile(&p, 9),
            at
        ));
    }

    #[test]
    fn a_note_that_starts_before_the_tick_and_ends_after_it_counts_by_its_start() {
        let p = song();
        let at = Ticks::beats(8) + Ticks::EIGHTH;
        // Note 8 starts at beat 8 and sounds over the tick: its end moving later is a
        // change after the tick (its note-off), its start moving is not.
        let mut longer = p.clone();
        longer.tracks[0].clips[0].notes[8].length = Ticks::beats(2);
        assert!(same_until(&snap(&p), &snap(&longer), at));
        let mut shorter = p.clone();
        shorter.tracks[0].clips[0].notes[8].length = Ticks(10);
        assert!(!same_until(&snap(&p), &snap(&shorter), at));
    }
}
