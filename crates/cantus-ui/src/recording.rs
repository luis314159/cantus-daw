//! MIDI recording: turns the engine's timestamped [`NoteInput`] events into finished notes.
//!
//! The recorder is pure state, no I/O, so the whole take logic is unit-testable. The app
//! keeps the notes it returns for the pass and, when the pass ends, turns them into clips
//! through `record_pass` (one undo step per pass).
//!
//! **Loop passes.** While the transport loops, every note is tagged with the pass it was
//! played in. The application tells the recorder when the playhead wrapped this frame
//! ([`Recorder::ingest_pass`]); the events of that frame are split at the middle of the
//! loop, which is far from both ends of a frame's worth of time.

use cantus_core::{LoopRegion, Ticks};
use cantus_engine::NoteInput;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingNote {
    pub pitch: u8,
    pub velocity: u8,
    /// Absolute tick of the note-on.
    pub start: Ticks,
    /// The loop pass it was played in (0 without a loop).
    pub pass: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordedNote {
    /// Absolute tick.
    pub start: Ticks,
    pub length: Ticks,
    pub pitch: u8,
    pub velocity: u8,
    /// The loop pass it was played in (0 without a loop).
    pub pass: u32,
    /// The note-off's release velocity (0 none given, ADR 0068).
    pub release: u8,
}

/// A controller played while recording (ADR 0068): where it was heard, what it drives and
/// its position (0…1, the bend's centre 0.5) — a recorded clip's lanes are made of these.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RecordedControl {
    pub tick: Ticks,
    pub kind: cantus_core::LaneKind,
    pub value: f32,
    /// The loop pass it was played in.
    pub pass: u32,
}

#[derive(Debug, Default)]
pub struct Recorder {
    recording: bool,
    pending: Vec<PendingNote>,
    /// Increments per take so each take merges into its own undo step.
    take: u64,
    /// The loop pass being played.
    pass: u32,
}

/// A note-off that arrives with no matching note-on (or with a tick before the note-on
/// because the loop wrapped) closes the note with this length.
const FALLBACK_LENGTH: Ticks = Ticks::SIXTEENTH;

impl Recorder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_recording(&self) -> bool {
        self.recording
    }

    pub fn take(&self) -> u64 {
        self.take
    }

    pub fn pending(&self) -> &[PendingNote] {
        &self.pending
    }

    /// The loop pass being played (0 until the playhead first wraps).
    pub fn pass(&self) -> u32 {
        self.pass
    }

    pub fn start(&mut self) {
        if !self.recording {
            self.recording = true;
            self.take += 1;
            self.pass = 0;
            self.pending.clear();
        }
    }

    /// Stop and close every held note at `at`.
    pub fn stop(&mut self, at: Ticks) -> Vec<RecordedNote> {
        self.recording = false;
        let held = std::mem::take(&mut self.pending);
        held.into_iter()
            .map(|p| RecordedNote {
                start: p.start,
                length: (at - p.start).max(FALLBACK_LENGTH),
                pitch: p.pitch,
                velocity: p.velocity,
                pass: p.pass,
                release: 0,
            })
            .collect()
    }

    /// Feed engine events; returns the notes that completed.
    pub fn ingest(
        &mut self,
        inputs: &[NoteInput],
        loop_region: Option<&LoopRegion>,
    ) -> Vec<RecordedNote> {
        self.ingest_pass(inputs, loop_region, false)
    }

    /// [`Self::ingest`], told whether the playhead wrapped around the loop since the last
    /// call. The frame's events before the wrap (in the loop's second half) belong to the
    /// pass that is ending, the ones after it (in the first half) to the next.
    pub fn ingest_pass(
        &mut self,
        inputs: &[NoteInput],
        loop_region: Option<&LoopRegion>,
        wrapped: bool,
    ) -> Vec<RecordedNote> {
        let mut done = Vec::new();
        if !self.recording {
            return done;
        }
        let mid = loop_region.map(|l| l.start + (l.end - l.start) / 2);
        let mut pending_wrap = wrapped;
        for ev in inputs {
            if pending_wrap && mid.is_some_and(|m| ev.tick < m) {
                self.pass += 1;
                pending_wrap = false;
            }
            if ev.velocity > 0 {
                // Retrigger of a held pitch: close the previous one first.
                if let Some(i) = self.pending.iter().position(|p| p.pitch == ev.pitch) {
                    let p = self.pending.remove(i);
                    done.push(close(p, ev.tick, 0, loop_region));
                }
                self.pending.push(PendingNote {
                    pitch: ev.pitch,
                    velocity: ev.velocity,
                    start: ev.tick,
                    pass: self.pass,
                });
            } else if let Some(i) = self.pending.iter().position(|p| p.pitch == ev.pitch) {
                let p = self.pending.remove(i);
                done.push(close(p, ev.tick, ev.release, loop_region));
            }
        }
        if pending_wrap {
            self.pass += 1;
        }
        done
    }

    /// The controllers of a frame (ADR 0068), each tagged with the loop pass it was played
    /// in by the rule [`Self::ingest_pass`] splits the notes by. Call it before the notes of
    /// the same frame: it reads the pass without moving it on.
    pub fn ingest_controls(
        &self,
        inputs: &[cantus_engine::ControlInput],
        loop_region: Option<&LoopRegion>,
        wrapped: bool,
    ) -> Vec<RecordedControl> {
        if !self.recording {
            return Vec::new();
        }
        let mid = loop_region.map(|l| l.start + (l.end - l.start) / 2);
        let mut pass = self.pass;
        let mut pending_wrap = wrapped;
        inputs
            .iter()
            .map(|c| {
                if pending_wrap && mid.is_some_and(|m| c.tick < m) {
                    pass += 1;
                    pending_wrap = false;
                }
                RecordedControl {
                    tick: c.tick,
                    kind: c.kind,
                    value: c.value,
                    pass,
                }
            })
            .collect()
    }
}

fn close(
    p: PendingNote,
    off: Ticks,
    release: u8,
    loop_region: Option<&LoopRegion>,
) -> RecordedNote {
    let length = if off > p.start {
        off - p.start
    } else {
        // The playhead wrapped: end the note at the loop end.
        match loop_region {
            Some(l) if l.end > p.start => l.end - p.start,
            _ => FALLBACK_LENGTH,
        }
    };
    RecordedNote {
        start: p.start,
        length: length.max(Ticks(1)),
        pitch: p.pitch,
        velocity: p.velocity,
        pass: p.pass,
        release,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::TrackId;

    fn ev(tick: i64, pitch: u8, vel: u8) -> NoteInput {
        NoteInput {
            track: TrackId(1),
            tick: Ticks(tick),
            pitch,
            velocity: vel,
            release: 0,
        }
    }

    #[test]
    fn pairs_on_and_off() {
        let mut r = Recorder::new();
        assert!(
            r.ingest(&[ev(0, 60, 100)], None).is_empty(),
            "not recording yet"
        );
        r.start();
        let out = r.ingest(&[ev(100, 60, 100), ev(150, 64, 90), ev(580, 60, 0)], None);
        assert_eq!(
            out,
            vec![RecordedNote {
                start: Ticks(100),
                length: Ticks(480),
                pitch: 60,
                velocity: 100,
                pass: 0,
                release: 0,
            }]
        );
        assert_eq!(r.pending().len(), 1);
        let rest = r.stop(Ticks(1000));
        assert_eq!(rest[0].pitch, 64);
        assert_eq!(rest[0].length, Ticks(850));
        assert!(!r.is_recording());
    }

    #[test]
    fn loop_wrap_closes_at_loop_end() {
        let mut r = Recorder::new();
        r.start();
        let l = LoopRegion {
            start: Ticks::ZERO,
            end: Ticks::beats(4),
        };
        let out = r.ingest(&[ev(3600, 60, 100), ev(50, 60, 0)], Some(&l));
        assert_eq!(out[0].length, Ticks(3840 - 3600));
    }

    #[test]
    fn retrigger_and_stray_off() {
        let mut r = Recorder::new();
        r.start();
        let out = r.ingest(&[ev(0, 60, 100), ev(200, 60, 110), ev(999, 72, 0)], None);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].length, Ticks(200));
        assert_eq!(r.pending()[0].velocity, 110);
        assert_eq!(r.take(), 1);
        r.stop(Ticks(300));
        r.start();
        assert_eq!(r.take(), 2);
    }

    /// A frame in which the playhead wrapped: what was played before the wrap (late in
    /// the loop) stays in the old pass, what came after it starts the next one.
    #[test]
    fn a_wrap_splits_the_frame_at_the_middle_of_the_loop() {
        let mut r = Recorder::new();
        r.start();
        let l = LoopRegion {
            start: Ticks::ZERO,
            end: Ticks::beats(8),
        };
        let a = r.ingest_pass(&[ev(960, 60, 100), ev(1200, 60, 0)], Some(&l), false);
        assert_eq!(a[0].pass, 0);
        // Late in pass 0, then the wrap, then early in pass 1 — all in one frame.
        let b = r.ingest_pass(
            &[
                ev(7000, 62, 100),
                ev(7300, 62, 0),
                ev(100, 64, 100),
                ev(300, 64, 0),
            ],
            Some(&l),
            true,
        );
        assert_eq!(
            b.iter().map(|n| (n.pitch, n.pass)).collect::<Vec<_>>(),
            vec![(62, 0), (64, 1)]
        );
        assert_eq!(r.pass(), 1);
        // A wrap in a frame with no events still counts.
        r.ingest_pass(&[], Some(&l), true);
        assert_eq!(r.pass(), 2);
        // A note held over the wrap keeps the pass it began in.
        r.ingest_pass(&[ev(7600, 65, 90)], Some(&l), false);
        let c = r.ingest_pass(&[ev(50, 65, 0)], Some(&l), true);
        assert_eq!((c[0].pass, c[0].length), (2, Ticks(7680 - 7600)));
        assert_eq!(r.pass(), 3);
    }
}
