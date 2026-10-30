//! The session view's messages and events (round 33, ADR 0076): what the view (the
//! launcher, keyboard, MIDI learn and controller grids) tells the engine, and what it is
//! told back — every type `Copy`, small and fixed-size.
//!
//! **Telling.** [`SessionMsg`] goes as `EngineMessage::Session` through
//! `EngineHandle::send`, which keeps it for the session's **conductor**
//! (`cantus_ui::session_play`) — it never reaches the audio thread: the conductor plans the
//! performance (`cantus_core::session::play`) and the engine plays it as a projection, the
//! song with what plays written into its arrangement (ADR 0076 §6–7). Slots and scenes are
//! named by id. A launch takes effect at the clip's quantisation boundary (`Quantize`,
//! resolved against the session's global one), sample-accurately — the first sample of the
//! clip lands on the boundary's sample — and no sooner than the conductor's lead after the
//! playhead. While the transport is stopped a launch starts it, at the playhead.
//!
//! **Hearing back.** [`SessionEvent`]s come from `EngineHandle::take_session_events`, told
//! only when something changes (never per frame), with the tick the change takes effect at:
//! a slot queued, playing, stopping, stopped, recording; a scene's launch state; whether a
//! track follows the arrangement (Live's *Back to Arrangement*); arrangement record. A view
//! that (re)connects ([`SessionMsg::Resend`]) is told the whole state again. Where a
//! playing clip is within itself is not streamed: the event that started it says where and
//! from which content tick ([`SlotPhase::Playing`], [`SlotPhase::Stopping`]), and
//! [`SlotPhase::content_at`] gives the content tick heard at any transport tick.

use cantus_core::session::{SceneId, SessionClip, SlotRef};
use cantus_core::{Ticks, TrackId};

/// How many session events a view should be ready to take in one frame (a scene launched
/// on 256 tracks is 512 changes). The handle keeps them in a list, so none is lost:
/// [`SessionEvent::Overflow`] is kept in the API for a view that bounds its own queue.
pub const MAX_SESSION_EVENTS: usize = 1024;

/// View → engine (`EngineMessage::Session`).
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum SessionMsg {
    /// A slot's launcher pressed, at `velocity` (0…1: a pad's; the mouse and keys give 1).
    /// A clip launches by its mode (trigger, gate, toggle, repeat); an empty slot with its
    /// stop button stops its track; one without does nothing.
    Launch { slot: SlotRef, velocity: f32 },
    /// A slot's launcher released: a gate clip stops (at its quantisation), a repeating one
    /// stops retriggering. Nothing for the other modes.
    Release { slot: SlotRef },
    /// A scene's launcher pressed: every track's slot in its row launched (clips, and the
    /// empty slots' stop buttons), its tempo and metre applied at the same boundary.
    LaunchScene { scene: SceneId, velocity: f32 },
    /// A scene's launcher released (its gate clips stop).
    ReleaseScene { scene: SceneId },
    /// A track's session clip stopped at the global quantisation (its stop button). The
    /// track stays silent until a launch or *back to arrangement*.
    StopTrack { track: TrackId },
    /// Every track's session clip stopped at the global quantisation.
    StopAll,
    /// The track (every track: `None`) plays its arrangement again, from where the
    /// transport is — its session clip stopped at once with the engine's click-free fade.
    BackToArrangement { track: Option<TrackId> },
    /// Record into a slot: an empty one gets a new clip (MIDI or audio as the track is
    /// armed), recorded from the next boundary (after the count-in when the transport is
    /// stopped); a looping MIDI clip that plays is overdubbed. Launching the slot (or
    /// [`Self::StopRecording`]) ends it at the bar line, and the clip plays on.
    Record { slot: SlotRef },
    /// End a recording into a slot at the next bar line (the fixed length ends it by
    /// itself).
    StopRecording { slot: SlotRef },
    /// *Arrangement record*: every launch, stop and follow action while it is on is written
    /// into the arrangement as it was heard (the view commits it, one undo step).
    SetArrangementRecord(bool),
    /// Send the whole state again (a view that connected late or lost events).
    Resend,
}

/// What a slot is doing, as the view draws it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SlotPhase {
    /// Nothing (an empty slot, or a clip that does not play).
    Stopped,
    /// Launched; starts at `at` (the boundary). Blinks until then.
    Queued { at: Ticks },
    /// Playing since timeline tick `since`, from content tick `from` (its start marker,
    /// or where legato took over).
    Playing { since: Ticks, from: Ticks },
    /// Playing (since `since`, from content tick `from`, as [`Self::Playing`]), and will
    /// stop at `at`.
    Stopping {
        since: Ticks,
        from: Ticks,
        at: Ticks,
    },
    /// Waiting to record from `at` (a count-in or a boundary).
    RecordQueued { at: Ticks },
    /// Recording since `since`.
    Recording { since: Ticks },
}

impl SlotPhase {
    /// The content tick of `clip` heard at transport tick `tick` while it plays (`None`
    /// before it starts, after a clip that does not loop has ended, or in another phase):
    /// the playing position the view draws, read off one event. A stopping clip plays on
    /// until its stop.
    pub fn content_at(&self, clip: &SessionClip, tick: Ticks) -> Option<Ticks> {
        let (since, from) = match *self {
            SlotPhase::Playing { since, from } => (since, from),
            SlotPhase::Stopping { since, from, at } if tick < at => (since, from),
            _ => return None,
        };
        clip.content_from(from, tick - since)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::session::edit::new_note_clip;

    /// Where a playing — or stopping — clip is, read off its event; nothing once stopped.
    #[test]
    fn the_playing_position_is_read_off_the_event() {
        let mut n = 0;
        let mut ids = || {
            n += 1;
            n
        };
        let bar = Ticks::beats(4);
        let c = new_note_clip("c", bar, &mut ids);
        let playing = SlotPhase::Playing {
            since: bar,
            from: Ticks::ZERO,
        };
        assert_eq!(playing.content_at(&c, bar * 2 + Ticks(5)), Some(Ticks(5)));
        let stopping = SlotPhase::Stopping {
            since: bar,
            from: Ticks::ZERO,
            at: bar * 3,
        };
        assert_eq!(stopping.content_at(&c, bar * 2 + Ticks(5)), Some(Ticks(5)));
        assert_eq!(stopping.content_at(&c, bar * 3), None);
        assert_eq!(SlotPhase::Stopped.content_at(&c, bar), None);
    }
}

/// A scene's launch button, as the view draws it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ScenePhase {
    Idle,
    /// Launched; its row starts at `at`.
    Queued {
        at: Ticks,
    },
    /// Its row was launched and plays.
    Playing {
        since: Ticks,
    },
}

/// Engine → view (`EngineEvent::Session`): one change, at the tick it takes effect.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum SessionEvent {
    Slot {
        slot: SlotRef,
        phase: SlotPhase,
    },
    Scene {
        scene: SceneId,
        phase: ScenePhase,
    },
    /// Whether a track plays its arrangement (`true`) or the session holds it — playing a
    /// clip or stopped by one — (`false`: Live's *Back to Arrangement* lit).
    Track {
        track: TrackId,
        arrangement: bool,
    },
    /// A follow action was taken: `from`'s clip handed over to `to` (or stopped: `None`) at
    /// `at`. For the view's history and the performance record.
    Followed {
        from: SlotRef,
        to: Option<SlotRef>,
        at: Ticks,
    },
    /// A scene's tempo (and metre) took effect at `at`.
    Tempo {
        at: Ticks,
        bpm: f64,
        metre: Option<cantus_core::TimeSignature>,
    },
    ArrangementRecord(bool),
    /// The engine had more changes than its ring held between two frames: ask
    /// [`SessionMsg::Resend`].
    Overflow,
}
