//! A recorded take's **MPE notes** (ADR 0080): the engine reports what an MPE controller
//! plays while recording ([`ExprInput`]): each note's on and off and every value, named by
//! its member channel, key and the tick its note-on was heard at. The take pairs them by
//! that name — two notes of one key on two channels (a LinnStrument's same pitch twice, a
//! slide onto a held key) stay two notes with their own lengths and curves (review fix 7)
//! — and hands each note, when it closes, its curves: points at their ticks from the
//! note's start, thinned to what an ear tells apart (a cent of bend, a thousandth).

use crate::recording::RecordedNote;
use cantus_core::model::note_expr::{ops, ExprKind, ExprPoint, NoteExpr, MAX_POINTS};
use cantus_core::{LoopRegion, Ticks};
use cantus_engine::inst::mpe::{ExprInput, MpeIn};

/// A note-off that comes back before its note-on (a loop's wrap) ends at the loop's end,
/// or this long.
const FALLBACK: Ticks = Ticks::SIXTEENTH;

#[derive(Debug)]
struct Open {
    channel: u8,
    pitch: u8,
    start: Ticks,
    velocity: u8,
    pass: u32,
    expr: NoteExpr,
}

/// The notes being played on an MPE controller, until they close.
#[derive(Debug, Default)]
pub struct MpeTake {
    open: Vec<Open>,
    done: Vec<RecordedNote>,
}

impl MpeTake {
    pub fn clear(&mut self) {
        self.open.clear();
        self.done.clear();
    }

    /// Notes still sounding.
    pub fn open(&self) -> usize {
        self.open.len()
    }

    /// A frame's events, in the order they were played; `pass` the loop pass being played,
    /// `wrapped` whether the playhead wrapped this frame: the events in the loop's first half
    /// then belong to the next pass, as the recorder splits the plain notes (review D2).
    pub fn ingest(
        &mut self,
        inputs: &[ExprInput],
        pass: u32,
        loop_region: Option<&LoopRegion>,
        wrapped: bool,
    ) {
        let mid = loop_region.map(|l| l.start + (l.end - l.start) / 2);
        let (mut pass, mut pending_wrap) = (pass, wrapped);
        for e in inputs {
            if pending_wrap && mid.is_some_and(|m| e.tick < m) {
                pass += 1;
                pending_wrap = false;
            }
            let at = |o: &Open| o.channel == e.channel && o.pitch == e.pitch && o.start == e.start;
            match e.what {
                MpeIn::On { velocity } => {
                    // A note still open on that channel ends where the next begins.
                    if let Some(i) = self.open.iter().position(|o| o.channel == e.channel) {
                        let o = self.open.swap_remove(i);
                        self.done.push(close(o, e.tick, 0, loop_region));
                    }
                    self.open.push(Open {
                        channel: e.channel,
                        pitch: e.pitch,
                        start: e.start,
                        velocity,
                        pass,
                        expr: NoteExpr::default(),
                    });
                }
                MpeIn::Off { release } => {
                    if let Some(i) = self.open.iter().position(at) {
                        let o = self.open.swap_remove(i);
                        self.done.push(close(o, e.tick, release, loop_region));
                    }
                }
                MpeIn::Value { kind, value } => {
                    let Some(o) = self.open.iter_mut().find(|o| at(o)) else {
                        continue;
                    };
                    let rel = (e.tick - e.start).0.max(0);
                    let c = o.expr.curve_mut(kind);
                    match c.last_mut() {
                        // Two values at one tick: the later stands.
                        Some(last) if last.0 >= rel => last.1 = value,
                        _ => {
                            if c.len() < MAX_POINTS * 4 {
                                c.push(ExprPoint(rel, value));
                            }
                        }
                    }
                }
            }
        }
    }

    /// The notes closed since the last call.
    pub fn take_done(&mut self) -> Vec<RecordedNote> {
        std::mem::take(&mut self.done)
    }

    /// Recording stopped at `at`: every note still sounding closes there.
    pub fn stop(&mut self, at: Ticks) -> Vec<RecordedNote> {
        let open = std::mem::take(&mut self.open);
        let mut out = self.take_done();
        out.extend(open.into_iter().map(|o| close(o, at, 0, None)));
        out
    }
}

fn close(o: Open, off: Ticks, release: u8, loop_region: Option<&LoopRegion>) -> RecordedNote {
    let length = if off > o.start {
        off - o.start
    } else {
        match loop_region {
            Some(l) if l.end > o.start => l.end - o.start,
            _ => FALLBACK,
        }
    };
    let mut e = o.expr;
    for k in ExprKind::ALL {
        let c = e.curve_mut(k);
        *c = ops::thin(c, ops::default_tolerance(k));
    }
    RecordedNote {
        start: o.start,
        length: length.max(Ticks(1)),
        pitch: o.pitch,
        velocity: o.velocity.clamp(1, 127),
        pass: o.pass,
        release: release.min(127),
        expr: e.normalised().boxed(),
    }
}

#[cfg(test)]
mod tests {
    // Review D2: see `a_wrapped_frame_puts_the_downbeat_note_in_the_next_pass`.
    use super::*;
    use cantus_core::TrackId;

    fn input(tick: i64, channel: u8, pitch: u8, start: i64, what: MpeIn) -> ExprInput {
        ExprInput {
            track: TrackId(1),
            tick: Ticks(tick),
            channel,
            pitch,
            start: Ticks(start),
            what,
        }
    }

    fn bend(x: f64) -> MpeIn {
        MpeIn::Value {
            kind: ExprKind::Bend,
            value: ExprKind::Bend.store(x),
        }
    }

    /// Review fix 7: two notes of one key on two member channels, overlapping, stay two
    /// notes — each its own length and its own curve.
    #[test]
    fn two_notes_of_one_key_on_two_channels_stay_apart() {
        let mut take = MpeTake::default();
        take.ingest(
            &[
                input(100, 1, 60, 100, MpeIn::On { velocity: 90 }),
                input(100, 1, 60, 100, bend(1.0)),
                input(200, 2, 60, 200, MpeIn::On { velocity: 80 }),
                input(200, 2, 60, 200, bend(-2.0)),
                input(300, 1, 60, 100, MpeIn::Off { release: 10 }),
                input(500, 2, 60, 200, MpeIn::Off { release: 20 }),
            ],
            0,
            None,
            false,
        );
        let done = take.take_done();
        assert_eq!(done.len(), 2);
        let (a, b) = (&done[0], &done[1]);
        assert_eq!(
            (a.start, a.length, a.velocity, a.release),
            (Ticks(100), Ticks(200), 90, 10)
        );
        assert_eq!(
            (b.start, b.length, b.velocity, b.release),
            (Ticks(200), Ticks(300), 80, 20)
        );
        assert_eq!(
            a.expr.as_deref().unwrap().value_at(ExprKind::Bend, 0),
            Some(1.0)
        );
        assert_eq!(
            b.expr.as_deref().unwrap().value_at(ExprKind::Bend, 0),
            Some(-2.0)
        );
        assert_eq!(take.open(), 0);
    }

    /// A straight played line is thinned to its ends; a held note closes at the stop.
    #[test]
    fn a_held_note_closes_at_the_stop_with_its_thinned_curve() {
        let mut take = MpeTake::default();
        let mut evs = vec![input(0, 3, 64, 0, MpeIn::On { velocity: 100 })];
        for i in 0..=10 {
            evs.push(input(i * 10, 3, 64, 0, bend(i as f64 * 0.3)));
        }
        take.ingest(&evs, 2, None, false);
        let notes = take.stop(Ticks(400));
        assert_eq!(notes.len(), 1);
        assert_eq!((notes[0].pass, notes[0].length), (2, Ticks(400)));
        let b = &notes[0].expr.as_deref().unwrap().bend;
        assert_eq!(
            b,
            &vec![ExprPoint(0, 0), ExprPoint(100, ExprKind::Bend.store(3.0))]
        );
    }
}

#[cfg(test)]
mod wrap_tests {
    use super::*;
    use cantus_core::TrackId;

    /// Review D2: in a frame where the playhead wrapped, a note played at the loop's start
    /// belongs to the next pass (as the recorder puts the plain notes), not the one ending.
    #[test]
    fn a_wrapped_frame_puts_the_downbeat_note_in_the_next_pass() {
        let ev = |tick: i64, what| ExprInput {
            track: TrackId(1),
            tick: Ticks(tick),
            channel: 1,
            pitch: 60,
            start: Ticks(tick.min(1900)),
            what,
        };
        let lp = LoopRegion {
            start: Ticks(0),
            end: Ticks(1920),
        };
        let mut take = MpeTake::default();
        // Before the wrap, a note in the loop's second half; after it, one on the downbeat.
        take.ingest(
            &[
                ev(1900, MpeIn::On { velocity: 90 }),
                ev(1910, MpeIn::Off { release: 0 }),
                ev(0, MpeIn::On { velocity: 90 }),
            ],
            3,
            Some(&lp),
            true,
        );
        let notes = take.stop(Ticks(400));
        let late = notes.iter().find(|n| n.start == Ticks(1900)).unwrap();
        let downbeat = notes.iter().find(|n| n.start == Ticks(0)).unwrap();
        assert_eq!((late.pass, downbeat.pass), (3, 4));
    }
}
