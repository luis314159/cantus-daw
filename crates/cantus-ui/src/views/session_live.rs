//! What plays in the session, as the engine (session's conductor, `crate::session_play`)
//! tells it (ADR 0076's events, ADR 0077): every slot's phase, every scene's, which tracks
//! the session holds, arrangement record — folded from `SessionEvent`s once a frame, never
//! polled. A press shows at once: the slot (or scene) pressed reads as queued until the
//! conductor's word on it arrives — a press takes effect at least its lead ahead of the
//! playhead — or, unconfirmed, for half a second.

use cantus_core::session::{SceneId, SlotRef};
use cantus_core::{Ticks, TrackId};
use cantus_engine::messages::{ScenePhase, SessionEvent, SlotPhase};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// How long a press shows as queued with no word from the conductor (a refused press: a
/// frozen track's clip).
pub const PRESS_SHOWN: Duration = Duration::from_millis(500);

/// The session's state as last heard.
#[derive(Debug, Default)]
pub struct Live {
    /// Every slot that is not stopped.
    slots: HashMap<SlotRef, SlotPhase>,
    scenes: HashMap<SceneId, ScenePhase>,
    /// The tracks the session holds (playing a clip, or stopped by one): their
    /// arrangement is silent until *back to arrangement*.
    held: Vec<TrackId>,
    /// Presses not yet answered: shown as queued from their tick.
    pressed: Vec<(SlotRef, Ticks, Instant)>,
    pressed_scenes: Vec<(SceneId, Ticks, Instant)>,
    /// Arrangement record, as the conductor has it.
    pub arrangement_record: bool,
    /// The engine lost events: the view asks for the whole state again.
    pub overflowed: bool,
    /// Whether any event was ever heard.
    pub heard: bool,
}

impl Live {
    pub fn phase(&self, s: SlotRef) -> SlotPhase {
        match self.slots.get(&s) {
            Some(p) => *p,
            None => match self.pressed.iter().find(|(x, _, _)| *x == s) {
                Some((_, at, _)) => SlotPhase::Queued { at: *at },
                None => SlotPhase::Stopped,
            },
        }
    }

    pub fn scene_phase(&self, id: SceneId) -> ScenePhase {
        match self.scenes.get(&id) {
            Some(p) => *p,
            None => match self.pressed_scenes.iter().find(|(x, _, _)| *x == id) {
                Some((_, at, _)) => ScenePhase::Queued { at: *at },
                None => ScenePhase::Idle,
            },
        }
    }

    /// A slot's launcher pressed at transport tick `now`: queued until the conductor says.
    pub fn press(&mut self, s: SlotRef, now: Ticks) {
        if !self.slots.contains_key(&s) {
            self.pressed.retain(|(x, _, _)| *x != s);
            self.pressed.push((s, now, Instant::now()));
        }
    }

    pub fn press_scene(&mut self, id: SceneId, now: Ticks) {
        if !self.scenes.contains_key(&id) {
            self.pressed_scenes.retain(|(x, _, _)| *x != id);
            self.pressed_scenes.push((id, now, Instant::now()));
        }
    }

    /// Presses the conductor never answered let go (once a frame).
    pub fn expire(&mut self, now: Instant) {
        self.pressed
            .retain(|(_, _, t)| now.saturating_duration_since(*t) < PRESS_SHOWN);
        self.pressed_scenes
            .retain(|(_, _, t)| now.saturating_duration_since(*t) < PRESS_SHOWN);
    }

    /// Whether the session holds `track` (Live's *Back to Arrangement* lit).
    pub fn holds(&self, track: TrackId) -> bool {
        self.held.contains(&track)
    }

    /// Whether any track is held.
    pub fn holds_any(&self) -> bool {
        !self.held.is_empty()
    }

    /// The slots playing now (stopping ones too): what *capture scene* copies.
    pub fn playing(&self) -> Vec<SlotRef> {
        let mut v: Vec<SlotRef> = self
            .slots
            .iter()
            .filter(|(_, p)| matches!(p, SlotPhase::Playing { .. } | SlotPhase::Stopping { .. }))
            .map(|(s, _)| *s)
            .collect();
        v.sort_by_key(|s| (s.track, s.scene));
        v
    }

    /// The slot of `track` that plays (or records) now, with its phase.
    pub fn playing_on(&self, track: TrackId) -> Option<(SlotRef, SlotPhase)> {
        self.slots
            .iter()
            .filter(|(s, _)| s.track == track)
            .find(|(_, p)| {
                matches!(
                    p,
                    SlotPhase::Playing { .. }
                        | SlotPhase::Stopping { .. }
                        | SlotPhase::Recording { .. }
                )
            })
            .map(|(s, p)| (*s, *p))
    }

    /// Whether anything is queued, playing or recording.
    pub fn busy(&self) -> bool {
        !self.slots.is_empty() || self.scenes.values().any(|p| *p != ScenePhase::Idle)
    }

    /// Whether a light somewhere blinks (a queued slot or scene, a press unanswered): the
    /// view keeps drawing.
    pub fn blinking(&self) -> bool {
        !self.pressed.is_empty()
            || !self.pressed_scenes.is_empty()
            || self.slots.values().any(|p| {
                matches!(
                    p,
                    SlotPhase::Queued { .. }
                        | SlotPhase::RecordQueued { .. }
                        | SlotPhase::Stopping { .. }
                )
            })
            || self
                .scenes
                .values()
                .any(|p| matches!(p, ScenePhase::Queued { .. }))
    }

    /// Forget everything (another project, a Resend coming).
    pub fn clear(&mut self) {
        self.slots.clear();
        self.scenes.clear();
        self.held.clear();
        self.pressed.clear();
        self.pressed_scenes.clear();
    }

    /// One event, in order.
    pub fn apply(&mut self, ev: SessionEvent) {
        self.heard = true;
        match ev {
            SessionEvent::Slot { slot, phase } => {
                self.pressed.retain(|(x, _, _)| *x != slot);
                if phase == SlotPhase::Stopped {
                    self.slots.remove(&slot);
                } else {
                    if matches!(
                        phase,
                        SlotPhase::Playing { .. } | SlotPhase::Recording { .. }
                    ) {
                        // One clip a track: another that played there has handed over.
                        self.slots.retain(|s, p| {
                            s.track != slot.track
                                || *s == slot
                                || !matches!(
                                    p,
                                    SlotPhase::Playing { .. } | SlotPhase::Stopping { .. }
                                )
                        });
                    }
                    self.slots.insert(slot, phase);
                }
            }
            SessionEvent::Scene { scene, phase } => {
                self.pressed_scenes.retain(|(x, _, _)| *x != scene);
                if phase == ScenePhase::Idle {
                    self.scenes.remove(&scene);
                } else {
                    self.scenes.insert(scene, phase);
                }
            }
            SessionEvent::Track { track, arrangement } => {
                self.held.retain(|t| *t != track);
                if !arrangement {
                    self.held.push(track);
                }
            }
            SessionEvent::ArrangementRecord(on) => self.arrangement_record = on,
            SessionEvent::Overflow => self.overflowed = true,
            SessionEvent::Followed { .. } | SessionEvent::Tempo { .. } => {}
        }
    }
}
