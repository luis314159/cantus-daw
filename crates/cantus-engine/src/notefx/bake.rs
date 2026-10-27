//! *Bake to notes* (ADR 0069): a chain run offline over a clip's events, in ticks, giving
//! the notes it plays — what the engine plays while the transport rolls, each note on the
//! tick it starts (a swung step rounded to the nearest tick). Used by the bake command and
//! by MIDI export when it writes what is played.

use super::{Chain, Due, Msg, Now};
use cantus_core::notefx::NoteEffect;
use cantus_core::scale::Key;
use cantus_core::{NoteEvent, Ticks};

/// A note on the timeline.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BakedNote {
    pub start: Ticks,
    pub length: Ticks,
    pub pitch: u8,
    pub velocity: u8,
    /// Its note-off's release velocity (0 where none was given: a chain that never let
    /// go of it, or a note with none).
    pub release: u8,
}

/// The notes `events` (sorted, as a snapshot holds them) play through `chain` in `key`:
/// what playback plays from them — each note to its own end, a gate past 100 % or a swung
/// last step past `end` included. A latched arpeggio (*Hold*) keeps stepping after the keys
/// are up, as in playback, until `hold_until` (where the track's next note would replace it;
/// the caller says), then rings out. `direct[i]` (when given) says event `i` passes the
/// chain. Sorted by start, then pitch.
pub fn bake_until(
    chain: &[NoteEffect],
    key: Option<Key>,
    events: &[NoteEvent],
    direct: &[bool],
    end: Ticks,
    hold_until: Ticks,
) -> Vec<BakedNote> {
    let mut c = Chain::new();
    c.configure(chain, key);
    let active = c.is_active();
    let now = |t: f64| Now {
        rolling: true,
        pos: t,
        tps: 1.0,
    };
    // Open notes: (tag, pitch, start, velocity).
    let mut open: Vec<(u64, u8, f64, u8)> = Vec::new();
    let mut out: Vec<BakedNote> = Vec::new();
    let mut close = |t: f64, m: &Msg, open: &mut Vec<(u64, u8, f64, u8)>| {
        if m.on {
            open.push((m.tag, m.pitch, t, m.velocity));
        } else if let Some(i) = open
            .iter()
            .position(|(tag, p, _, _)| *tag == m.tag && *p == m.pitch)
        {
            let (_, pitch, start, velocity) = open.remove(i);
            let release = m.release;
            let s = start.round() as i64;
            let e = (t.round() as i64).max(s + 1);
            out.push(BakedNote {
                start: Ticks(s),
                length: Ticks(e - s),
                pitch,
                velocity,
                release,
            });
        }
    };
    let mut drain = |c: &mut Chain, open: &mut Vec<(u64, u8, f64, u8)>| {
        let taken: Vec<(f64, Msg)> = c.take().to_vec();
        for (t, m) in &taken {
            close(*t, m, open);
        }
    };
    for (i, e) in events.iter().enumerate() {
        let t = e.tick.0 as f64;
        let m = Msg {
            on: e.is_on(),
            pitch: e.pitch,
            velocity: e.velocity,
            release: e.release,
            tag: e.note.0,
            ordinal: e.ordinal,
            live: false,
        };
        // What is due before this tick fires first; what is due on it, after every event on
        // it is in (the engine's order inside a sample).
        let first_here = i == 0 || events[i - 1].tick != e.tick;
        let last_here = events.get(i + 1).is_none_or(|x| x.tick != e.tick);
        if first_here {
            c.run_due(now(t), |d: Due| d.pos < t);
            drain(&mut c, &mut open);
        }
        if !active || direct.get(i).copied().unwrap_or(false) {
            c.out_push(t, m);
        } else {
            c.input(now(t), m);
        }
        drain(&mut c, &mut open);
        if last_here {
            c.run_due(now(t), |d: Due| d.pos <= t);
            drain(&mut c, &mut open);
        }
    }
    let end_t = end.0 as f64;
    c.run_due(now(end_t), |d: Due| d.pos < end_t);
    drain(&mut c, &mut open);
    // A latch steps on to where the next note would replace it …
    let hold_t = hold_until.max(end).0 as f64;
    c.run_due(now(hold_t), |d: Due| d.pos < hold_t);
    drain(&mut c, &mut open);
    // … then lets go: what sounds rings to its own end (a gate's tail, a swung step).
    c.release_holds();
    let mut last = hold_t;
    for _ in 0..4096 {
        let Some(d) = c.next_due() else { break };
        last = last.max(d.pos);
        c.run_due(now(d.pos), |x: Due| x.pos <= d.pos);
        drain(&mut c, &mut open);
    }
    // Whatever a chain never let go of ends there.
    let last = last.round() as i64;
    for (_, pitch, start, velocity) in std::mem::take(&mut open) {
        let s = start.round() as i64;
        if s >= last {
            continue;
        }
        out.push(BakedNote {
            start: Ticks(s),
            length: Ticks((last - s).max(1)),
            pitch,
            velocity,
            release: 0,
        });
    }
    out.sort_by_key(|n| (n.start, n.pitch, n.length));
    out
}

/// [`bake_until`] with no latch past `end`: what a chain plays from `events` that stop
/// there (MIDI export's hook, the engine's tests).
pub fn bake(
    chain: &[NoteEffect],
    key: Option<Key>,
    events: &[NoteEvent],
    direct: &[bool],
    end: Ticks,
) -> Vec<BakedNote> {
    bake_until(chain, key, events, direct, end, end)
}
