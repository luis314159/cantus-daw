//! Voice allocation shared by the native instruments (ADR 0023): poly with stealing, mono
//! and legato with a note stack and priority, and the sustain pedal. It decides *which*
//! voice plays a note; the instrument owns what a voice is.
//!
//! Deterministic: the choice depends only on the order of calls, never on time or on the
//! block size, which is what keeps an instrument's render identical at every block size.

/// The most voices an allocator can manage.
pub const MAX_VOICES: usize = 64;
/// Notes a mono voice remembers for returning to when the top note is released.
pub const NOTE_STACK: usize = 16;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum PlayMode {
    #[default]
    Poly,
    /// One voice, retriggered by every note.
    Mono,
    /// One voice; a note played while another is held glides without retriggering.
    Legato,
}

impl PlayMode {
    pub fn from_index(i: f32) -> PlayMode {
        match i.round() as i32 {
            1 => PlayMode::Mono,
            2 => PlayMode::Legato,
            _ => PlayMode::Poly,
        }
    }
}

/// Which held note a mono voice plays.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Priority {
    #[default]
    Last,
    Low,
    High,
}

impl Priority {
    pub fn from_index(i: f32) -> Priority {
        match i.round() as i32 {
            1 => Priority::Low,
            2 => Priority::High,
            _ => Priority::Last,
        }
    }
}

/// What the instrument should do after a note-on or a note-off.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Action {
    Nothing,
    /// Start `voice` on `key`. `stolen`: the voice was sounding and is taken over, so the
    /// instrument should declick it.
    Start {
        voice: usize,
        key: u8,
        id: u64,
        velocity: f32,
        stolen: bool,
    },
    /// Move the sounding `voice` to `key` without retriggering its envelopes (legato, or a
    /// mono voice returning to a note still held).
    Glide {
        voice: usize,
        key: u8,
        id: u64,
    },
    /// Release `voice`.
    Release {
        voice: usize,
    },
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct Slot {
    active: bool,
    released: bool,
    /// Held past its note-off by the sustain pedal.
    sustained: bool,
    key: u8,
    id: u64,
    /// Start order, for stealing the oldest.
    age: u64,
}

#[derive(Clone, Debug)]
pub struct Allocator {
    slots: [Slot; MAX_VOICES],
    polyphony: usize,
    mode: PlayMode,
    priority: Priority,
    /// Held notes, oldest first: (key, id, velocity).
    stack: [(u8, u64, f32); NOTE_STACK],
    stack_len: usize,
    sustain: bool,
    clock: u64,
    /// Voices taken over since the last [`take_stolen`](Self::take_stolen).
    stolen: u32,
}

impl Allocator {
    pub fn new(polyphony: usize) -> Self {
        Allocator {
            slots: [Slot::default(); MAX_VOICES],
            polyphony: polyphony.clamp(1, MAX_VOICES),
            mode: PlayMode::Poly,
            priority: Priority::Last,
            stack: [(0, 0, 0.0); NOTE_STACK],
            stack_len: 0,
            sustain: false,
            clock: 0,
            stolen: 0,
        }
    }

    pub fn set_polyphony(&mut self, n: usize) {
        self.polyphony = n.clamp(1, MAX_VOICES);
    }

    pub fn polyphony(&self) -> usize {
        match self.mode {
            PlayMode::Poly => self.polyphony,
            _ => 1,
        }
    }

    pub fn set_mode(&mut self, mode: PlayMode, priority: Priority) {
        self.mode = mode;
        self.priority = priority;
    }

    pub fn mode(&self) -> PlayMode {
        self.mode
    }

    /// Voices sounding or releasing, as the allocator knows them.
    pub fn active(&self) -> usize {
        self.slots.iter().filter(|s| s.active).count()
    }

    /// How many voices were stolen since the last call.
    pub fn take_stolen(&mut self) -> u32 {
        std::mem::take(&mut self.stolen)
    }

    /// Whether `voice` is sounding a note that is still held (not released).
    pub fn is_held(&self, voice: usize) -> bool {
        self.slots
            .get(voice)
            .is_some_and(|s| s.active && !s.released)
    }

    pub fn note_on(&mut self, key: u8, id: u64, velocity: f32) -> Action {
        self.clock += 1;
        if self.mode != PlayMode::Poly {
            return self.mono_on(key, id, velocity);
        }
        let n = self.polyphony;
        let voice = match self.slots[..n].iter().position(|s| !s.active) {
            Some(v) => v,
            None => {
                // Steal: the oldest released voice, else the oldest held one.
                let pick = |released: bool| {
                    self.slots[..n]
                        .iter()
                        .enumerate()
                        .filter(|(_, s)| s.released == released)
                        .min_by_key(|(_, s)| s.age)
                        .map(|(i, _)| i)
                };
                self.stolen += 1;
                pick(true).or_else(|| pick(false)).unwrap_or(0)
            }
        };
        let stolen = self.slots[voice].active;
        self.slots[voice] = Slot {
            active: true,
            released: false,
            sustained: false,
            key,
            id,
            age: self.clock,
        };
        Action::Start {
            voice,
            key,
            id,
            velocity,
            stolen,
        }
    }

    fn mono_on(&mut self, key: u8, id: u64, velocity: f32) -> Action {
        if self.stack_len == NOTE_STACK {
            self.stack.copy_within(1.., 0);
            self.stack_len -= 1;
        }
        self.stack[self.stack_len] = (key, id, velocity);
        self.stack_len += 1;
        let (k, i, v) = self.chosen();
        let s = self.slots[0];
        let held = s.active && !s.released;
        if held && s.id == i {
            return Action::Nothing; // a lower-priority note: the voice keeps its note
        }
        self.slots[0] = Slot {
            active: true,
            released: false,
            sustained: false,
            key: k,
            id: i,
            age: self.clock,
        };
        if held && self.mode == PlayMode::Legato {
            Action::Glide {
                voice: 0,
                key: k,
                id: i,
            }
        } else {
            Action::Start {
                voice: 0,
                key: k,
                id: i,
                velocity: v,
                stolen: s.active,
            }
        }
    }

    /// The held note the priority chooses.
    fn chosen(&self) -> (u8, u64, f32) {
        let held = &self.stack[..self.stack_len];
        let last = held[held.len() - 1];
        match self.priority {
            Priority::Last => last,
            Priority::Low => *held.iter().min_by_key(|n| n.0).unwrap_or(&last),
            Priority::High => *held.iter().max_by_key(|n| n.0).unwrap_or(&last),
        }
    }

    pub fn note_off(&mut self, key: u8, id: u64) -> Action {
        if self.mode != PlayMode::Poly {
            return self.mono_off(key, id);
        }
        let Some(v) = self
            .slots
            .iter()
            .position(|s| s.active && !s.released && s.id == id && s.key == key)
        else {
            return Action::Nothing;
        };
        if self.sustain {
            self.slots[v].sustained = true;
            return Action::Nothing;
        }
        self.slots[v].released = true;
        Action::Release { voice: v }
    }

    fn mono_off(&mut self, key: u8, id: u64) -> Action {
        let Some(p) = self.stack[..self.stack_len]
            .iter()
            .position(|n| n.0 == key && n.1 == id)
        else {
            return Action::Nothing;
        };
        self.stack.copy_within(p + 1..self.stack_len, p);
        self.stack_len -= 1;
        let s = self.slots[0];
        if !(s.active && !s.released && s.id == id) {
            return Action::Nothing; // a note that was held but not sounding
        }
        if self.stack_len > 0 {
            // Back to a note still held: glide there without a new attack.
            let (k, i, _) = self.chosen();
            self.slots[0].key = k;
            self.slots[0].id = i;
            return Action::Glide {
                voice: 0,
                key: k,
                id: i,
            };
        }
        if self.sustain {
            self.slots[0].sustained = true;
            return Action::Nothing;
        }
        self.slots[0].released = true;
        Action::Release { voice: 0 }
    }

    /// The sustain pedal. Lifting it releases every voice it was holding: their numbers
    /// are written into `released`, and the count returned.
    pub fn set_sustain(&mut self, down: bool, released: &mut [usize; MAX_VOICES]) -> usize {
        self.sustain = down;
        if down {
            return 0;
        }
        let mut n = 0;
        for (i, s) in self.slots.iter_mut().enumerate() {
            if s.active && s.sustained {
                s.sustained = false;
                s.released = true;
                released[n] = i;
                n += 1;
            }
        }
        n
    }

    /// The instrument says a voice has fallen silent (its release ended).
    pub fn voice_finished(&mut self, voice: usize) {
        if let Some(s) = self.slots.get_mut(voice) {
            *s = Slot::default();
        }
    }

    /// Forget every voice and held note (reset, panic).
    pub fn clear(&mut self) {
        self.slots = [Slot::default(); MAX_VOICES];
        self.stack_len = 0;
        self.sustain = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn voice(a: Action) -> usize {
        match a {
            Action::Start { voice, .. }
            | Action::Glide { voice, .. }
            | Action::Release { voice } => voice,
            Action::Nothing => usize::MAX,
        }
    }

    #[test]
    fn poly_fills_then_steals_the_oldest_released_first() {
        let mut a = Allocator::new(2);
        assert_eq!(voice(a.note_on(60, 1, 1.0)), 0);
        assert_eq!(voice(a.note_on(62, 2, 1.0)), 1);
        assert_eq!(a.note_off(60, 1), Action::Release { voice: 0 });
        match a.note_on(64, 3, 1.0) {
            Action::Start { voice, stolen, .. } => assert_eq!((voice, stolen), (0, true)),
            other => panic!("{other:?}"),
        }
        assert_eq!(a.take_stolen(), 1);
        // Both held now: the oldest held (62) goes.
        assert_eq!(voice(a.note_on(65, 4, 1.0)), 1);
    }

    #[test]
    fn two_notes_on_one_key_are_told_apart_by_id() {
        let mut a = Allocator::new(4);
        a.note_on(60, 1, 1.0);
        a.note_on(60, 2, 1.0);
        assert_eq!(a.note_off(60, 2), Action::Release { voice: 1 });
        assert_eq!(a.note_off(60, 1), Action::Release { voice: 0 });
    }

    #[test]
    fn legato_glides_and_returns_to_the_held_note() {
        let mut a = Allocator::new(8);
        a.set_mode(PlayMode::Legato, Priority::Last);
        assert!(matches!(
            a.note_on(48, 1, 1.0),
            Action::Start { voice: 0, .. }
        ));
        assert_eq!(
            a.note_on(55, 2, 1.0),
            Action::Glide {
                voice: 0,
                key: 55,
                id: 2
            }
        );
        assert_eq!(
            a.note_off(55, 2),
            Action::Glide {
                voice: 0,
                key: 48,
                id: 1
            }
        );
        assert_eq!(a.note_off(48, 1), Action::Release { voice: 0 });
    }

    #[test]
    fn low_priority_keeps_the_lowest_note() {
        let mut a = Allocator::new(1);
        a.set_mode(PlayMode::Mono, Priority::Low);
        a.note_on(48, 1, 1.0);
        assert_eq!(a.note_on(60, 2, 1.0), Action::Nothing);
        assert!(matches!(
            a.note_on(40, 3, 1.0),
            Action::Start { key: 40, .. }
        ));
    }

    #[test]
    fn the_pedal_holds_releases_until_it_lifts() {
        let mut a = Allocator::new(4);
        let mut out = [0; MAX_VOICES];
        a.set_sustain(true, &mut out);
        a.note_on(60, 1, 1.0);
        assert_eq!(a.note_off(60, 1), Action::Nothing);
        assert!(a.is_held(0));
        assert_eq!(a.set_sustain(false, &mut out), 1);
        assert_eq!(out[0], 0);
        assert!(!a.is_held(0));
    }
}
