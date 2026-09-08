//! MIDI recording: turns the engine's timestamped [`NoteInput`] events into finished notes.
//!
//! The recorder is pure state, no I/O, so the whole take logic is unit-testable. The app
//! commits the notes it returns through ordinary commands (one merged undo step per take).

use cantus_core::{LoopRegion, Ticks};
use cantus_engine::NoteInput;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingNote {
    pub pitch: u8,
    pub velocity: u8,
    /// Absolute tick of the note-on.
    pub start: Ticks,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordedNote {
    /// Absolute tick.
    pub start: Ticks,
    pub length: Ticks,
    pub pitch: u8,
    pub velocity: u8,
}

#[derive(Debug, Default)]
pub struct Recorder {
    recording: bool,
    pending: Vec<PendingNote>,
    /// Increments per take so each take merges into its own undo step.
    take: u64,
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

    pub fn start(&mut self) {
        if !self.recording {
            self.recording = true;
            self.take += 1;
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
            })
            .collect()
    }

    /// Feed engine events; returns the notes that completed.
    pub fn ingest(
        &mut self,
        inputs: &[NoteInput],
        loop_region: Option<&LoopRegion>,
    ) -> Vec<RecordedNote> {
        let mut done = Vec::new();
        if !self.recording {
            return done;
        }
        for ev in inputs {
            if ev.velocity > 0 {
                // Retrigger of a held pitch: close the previous one first.
                if let Some(i) = self.pending.iter().position(|p| p.pitch == ev.pitch) {
                    let p = self.pending.remove(i);
                    done.push(close(p, ev.tick, loop_region));
                }
                self.pending.push(PendingNote {
                    pitch: ev.pitch,
                    velocity: ev.velocity,
                    start: ev.tick,
                });
            } else if let Some(i) = self.pending.iter().position(|p| p.pitch == ev.pitch) {
                let p = self.pending.remove(i);
                done.push(close(p, ev.tick, loop_region));
            }
        }
        done
    }
}

fn close(p: PendingNote, off: Ticks, loop_region: Option<&LoopRegion>) -> RecordedNote {
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
                velocity: 100
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
}
