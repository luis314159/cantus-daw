//! Playback snapshots: an immutable, pre-sorted view of the project that the audio thread
//! can read without locks (ADR 0004/0005).
//!
//! The UI compiles a new snapshot after every command and hands an `Arc<Snapshot>` to the
//! engine. The engine never mutates it.

use crate::effect::EffectSlot;
use crate::instrument::InstrumentSpec;
use crate::model::*;
use crate::time::{Ticks, TimeSignature};

/// A note-on (`velocity > 0`) or note-off (`velocity == 0`) at an absolute tick.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct NoteEvent {
    pub tick: Ticks,
    pub pitch: Pitch,
    pub velocity: u8,
    pub note: NoteId,
}

impl NoteEvent {
    pub fn is_on(&self) -> bool {
        self.velocity > 0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrackSnapshot {
    pub id: TrackId,
    pub name: String,
    /// Linear gain (fader × instrument-independent).
    pub gain: f32,
    pub pan: f32,
    /// 0 mono, 1 as recorded, 2 twice as wide.
    pub width: f32,
    pub phase_invert: bool,
    /// Mute/solo already resolved.
    pub audible: bool,
    pub instrument: InstrumentSpec,
    /// Insert chain in processing order (bypassed slots included, flagged).
    pub effects: Vec<EffectSlot>,
    /// Sorted by `(tick, off-before-on)`.
    pub events: Vec<NoteEvent>,
    /// How many lanes the instrument offers. Carried so the meter and the stats can say
    /// something about a kit without the audio thread reasoning about pads.
    pub lane_count: usize,
    /// Decoded audio for each pad of a kit, aligned with its pads; `None` for a pad that
    /// is not a sample or whose file could not be read.
    ///
    /// **Empty after [`Snapshot::compile`].** This crate does no I/O, so the loader in the
    /// engine fills it in before the snapshot is sent — the same snapshot then serves
    /// real-time playback and offline render, which is what keeps the two bit-identical.
    pub samples: Vec<Option<std::sync::Arc<crate::pads::SampleData>>>,
}

impl TrackSnapshot {
    /// Index of the first event at or after `tick` (binary search).
    pub fn first_event_at_or_after(&self, tick: Ticks) -> usize {
        self.events.partition_point(|e| e.tick < tick)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub revision: u64,
    pub tempo_bpm: f64,
    pub time_signature: TimeSignature,
    pub loop_region: Option<LoopRegion>,
    pub loop_enabled: bool,
    pub metronome: bool,
    pub master_gain: f32,
    pub master_pan: f32,
    pub master_width: f32,
    pub master_phase_invert: bool,
    /// Resolved once here so the audio thread never has to know which law is in force —
    /// it reads two multipliers, the same as it always did.
    pub pan_law: PanLaw,
    pub tracks: Vec<TrackSnapshot>,
    pub content_end: Ticks,
}

impl Snapshot {
    pub fn compile(project: &Project, revision: u64) -> Snapshot {
        let tracks = project
            .tracks
            .iter()
            .map(|t| TrackSnapshot {
                id: t.id,
                name: t.name.clone(),
                gain: db_to_linear(t.gain_db),
                pan: t.pan,
                width: t.width,
                phase_invert: t.phase_invert,
                audible: project.track_audible(t),
                instrument: t.instrument.clone(),
                effects: t.effects.clone(),
                events: compile_events(t),
                lane_count: t.instrument.lanes().count(),
                samples: Vec::new(),
            })
            .collect();
        Snapshot {
            revision,
            tempo_bpm: project.tempo_bpm,
            time_signature: project.time_signature,
            loop_region: if project.loop_enabled {
                project.loop_region.clone()
            } else {
                None
            },
            loop_enabled: project.loop_enabled,
            metronome: project.metronome,
            master_gain: db_to_linear(project.master.gain_db),
            master_pan: project.master.pan,
            master_width: project.master.width,
            master_phase_invert: project.master.phase_invert,
            pan_law: project.pan_law,
            tracks,
            content_end: project.content_end(),
        }
    }

    pub fn track(&self, id: TrackId) -> Option<&TrackSnapshot> {
        self.tracks.iter().find(|t| t.id == id)
    }
}

fn compile_events(track: &Track) -> Vec<NoteEvent> {
    let mut events = Vec::new();
    // Reused across every note so compiling a project of 20 000 notes allocates once.
    let mut at = Vec::new();
    // A muted *pad* is resolved here, exactly as a muted track is: the audio thread is
    // handed a snapshot with the events already gone, rather than a kit it has to consult
    // per note. One place decides what sounds.
    let muted_pads = track.instrument.pulsus().map(|k| {
        k.pads
            .iter()
            .filter(|p| p.muted)
            .map(|p| p.pitch)
            .collect::<Vec<_>>()
    });
    for clip in &track.clips {
        if clip.muted {
            continue;
        }
        for n in &clip.notes {
            if n.muted {
                continue;
            }
            if muted_pads
                .as_ref()
                .is_some_and(|pads| pads.contains(&n.pitch))
            {
                continue;
            }
            // Where the note is *heard*, which is not where it is stored: a clip is a
            // window onto its content, and a looping clip shows the same note more than
            // once (`Clip::timeline_positions`).
            clip.timeline_positions(n.start, &mut at);
            for on in at.iter().copied() {
                let mut off = on + n.length;
                // A note never sounds past the clip, nor past the start of the next
                // repetition — otherwise a looped clip would stack its own tails.
                off = off.min(clip.end());
                if let Some(period) = clip.period() {
                    off = off.min(on + period);
                }
                if off <= on {
                    continue;
                }
                events.push(NoteEvent {
                    tick: on,
                    pitch: n.pitch,
                    velocity: n.velocity.max(1),
                    note: n.id,
                });
                events.push(NoteEvent {
                    tick: off,
                    pitch: n.pitch,
                    velocity: 0,
                    note: n.id,
                });
            }
        }
    }
    // Offs before ons at the same tick so a retriggered pitch restarts cleanly.
    events.sort_by_key(|e| (e.tick, e.is_on(), e.pitch, e.note));
    events
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_are_sorted_offs_first_and_clamped_to_clip() {
        let mut p = Project::default();
        let mut t = p.make_track("t");
        let mut c = p.make_clip("c", Ticks::beats(4), Ticks::beats(2));
        // Note that overhangs the clip end.
        c.notes
            .push(p.make_note(Ticks::beats(1), Ticks::beats(4), 60, 100));
        // Note starting exactly where the previous one ends... but the previous is clamped.
        c.notes
            .push(p.make_note(Ticks::ZERO, Ticks::beats(1), 60, 90));
        // Note outside the clip: ignored.
        c.notes
            .push(p.make_note(Ticks::beats(3), Ticks::beats(1), 62, 90));
        t.clips.push(c);
        p.tracks.push(t);

        let s = Snapshot::compile(&p, 7);
        let ev = &s.tracks[0].events;
        assert_eq!(ev.len(), 4);
        assert_eq!((ev[0].tick, ev[0].velocity), (Ticks::beats(4), 90));
        // At beat 5: off of first note, then on of second.
        assert_eq!((ev[1].tick, ev[1].velocity), (Ticks::beats(5), 0));
        assert_eq!((ev[2].tick, ev[2].velocity), (Ticks::beats(5), 100));
        // Clamped to clip end (beat 6), not beat 9.
        assert_eq!((ev[3].tick, ev[3].velocity), (Ticks::beats(6), 0));
        assert_eq!(s.tracks[0].first_event_at_or_after(Ticks::beats(5)), 1);
        assert_eq!(s.content_end, Ticks::beats(6));
        assert_eq!(s.revision, 7);
    }

    #[test]
    fn solo_resolves_audibility() {
        let mut p = Project::default();
        let a = p.make_track("a");
        let mut b = p.make_track("b");
        b.soloed = true;
        p.tracks.push(a);
        p.tracks.push(b);
        let s = Snapshot::compile(&p, 0);
        assert!(!s.tracks[0].audible);
        assert!(s.tracks[1].audible);
    }
}
