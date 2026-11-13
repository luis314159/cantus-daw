//! Lapsus, the time shaper (ADR 0132, provisional): Gross Beat's job — tape stops, half
//! time, stutters, reverse, scratches and gates — drawn as two curves over a cycle synced
//! to the song, with Ableton's Beat Repeat and Effectrix ideas on top.
//!
//! - **The time curve** gives, at each moment of the cycle, the **delay** behind the live
//!   signal in cycle lengths: flat 0 is the signal as it arrives (and, read as a whole
//!   sample, the same bits); a slope is a speed (`1 − slope`: half a 45° slope is half
//!   speed, 45° is still, steeper is reverse); a held step is a repeat. It reads the
//!   unit's own history of the input ([`read`]), interpolated without zipper.
//! - **The volume curve** multiplies what is read (gates, pumps, the tape's fading level),
//!   through a one-pole glide (`smooth`).
//! - **The cycle** is the pattern's length in quarter notes, **a function of the song
//!   sample** (the family's clock): bar *k* starts on the sample the engine puts bar *k*
//!   on, at every tempo — a tape stop ends on the bar to the sample. With `retrig`, a note
//!   starts the cycle where it lands.
//! - **Nothing before the cycle's start is read** (zeros), and everything that carries
//!   state across samples — the volume's glide, a crossfade, the jump detector — starts
//!   again at each cycle's first sample from the pattern alone: a render from bar 5 is the
//!   bars from 5 of the whole render, to the bit, whatever the music before.
//! - **Joins**: where the time curve jumps inside a cycle (a repeat, a step), the old read
//!   carries on at its speed and fades out over `xfade` while the new one fades in (an
//!   equal-gain raised cosine); where it jumps at the cycle's end, the fade is placed
//!   *before* the line, onto the next cycle's first read, so the next bar starts on its
//!   line at full weight. A pattern switched by a note or the slot fades the same way.
//! - **Notes** (`TAKES_NOTES`): note `key + k` plays slot `k` — switched until another
//!   note, or held while the key is down — and, with `retrig`, from its beginning.
//! - **Lookahead** (`ahead`): the whole output later by that much (reported as latency),
//!   so the time curve may read that far into the future and the sinc has its taps.
//!
//! The history is built off the audio thread ([`Lapsus::memory_need`] → [`Ring`]): one bar
//! of the bank's longest pattern at 60 BPM, plus the lookahead.

#[path = "lapsus_curve.rs"]
pub mod curve;
#[path = "lapsus_read.rs"]
pub mod read;
#[path = "lapsus_run.rs"]
mod run;

use crate::fluctus::{CurvePoint, ShapeFn};
use crate::modfx_clock::{Clock, Transport};
use crate::modfx_line::Glide;
pub use curve::{Curve, MAX_POINTS};
pub use read::{History, Ring};

/// Patterns in the bank.
pub const SLOTS: usize = 36;
/// The slowest tempo the history is sized for: a pattern read further back at a slower
/// tempo reads silence there.
pub const MIN_TEMPO: f64 = 60.0;
/// A step in the delay larger than this, in samples, beyond what the curve's speed
/// predicts, is a jump: a join's crossfade starts.
pub const JUMP: f64 = 0.5;
/// Notes waiting for their sample.
const MAX_NOTES: usize = 64;
/// How the volume's glide lands.
const SNAP: f32 = 1e-7;
/// The time curve's range, in cycle lengths.
pub const TIME_LO: f32 = -1.0;
pub const TIME_HI: f32 = 1.0;

/// What a note does.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum NoteMode {
    /// Notes are ignored.
    Off,
    /// A note switches the pattern until another note does.
    Switch,
    /// A note plays its pattern while it is held; the slot's own comes back on release.
    #[default]
    Hold,
}

impl NoteMode {
    pub fn from_index(v: f32) -> NoteMode {
        match v.round() as i32 {
            0 => NoteMode::Off,
            1 => NoteMode::Switch,
            _ => NoteMode::Hold,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LapsusSettings {
    /// The slot playing when no note chooses one, 0 … 35.
    pub slot: usize,
    /// The shaped signal against the dry, 0 … 1.
    pub mix: f32,
    pub xfade_ms: f32,
    pub smooth_ms: f32,
    pub notes: NoteMode,
    pub retrig: bool,
    /// The note of slot 0.
    pub key: u8,
    pub ahead_ms: f32,
    pub out_db: f32,
}

impl Default for LapsusSettings {
    fn default() -> Self {
        LapsusSettings {
            slot: 0,
            mix: 1.0,
            xfade_ms: 2.0,
            smooth_ms: 1.0,
            notes: NoteMode::Hold,
            retrig: false,
            key: 48,
            ahead_ms: 0.0,
            out_db: 0.0,
        }
    }
}

/// One pattern as the DSP plays it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Pattern {
    /// Quarter notes in a cycle.
    pub beats: f64,
    pub time: Curve,
    pub vol: Curve,
}

impl Pattern {
    /// Nothing moves: no delay, full level, a bar long.
    pub const THROUGH: Pattern = Pattern {
        beats: 4.0,
        time: Curve::flat(0.0),
        vol: Curve::flat(1.0),
    };

    /// A pattern from points (allocation-free).
    pub fn new(beats: f64, time: &[CurvePoint], vol: &[CurvePoint]) -> Pattern {
        let mut p = Pattern::THROUGH;
        p.beats = beats;
        p.time.set(time, TIME_LO, TIME_HI);
        p.vol.set(vol, 0.0, 1.0);
        p
    }

    /// Whether it changes nothing (a flat zero delay at full level).
    pub fn is_through(&self) -> bool {
        self.time.constant() == Some(0.0) && self.vol.constant() == Some(1.0)
    }
}

/// A note as Lapsus hears it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Note {
    On(u8),
    Off(u8),
    /// Every note stops (the transport stopped, sought or wrapped).
    AllOff,
}

/// The read that fades out where the time curve jumped: the old position carrying on at
/// its speed, at its level.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct Ghost {
    on: bool,
    d: f64,
    slope: f64,
    vol: f32,
    left: u32,
    len: u32,
}

/// The cycle holding a song sample, and what its end needs. A cache: every field is a
/// function of the pattern, the tempo, the origin and the sample.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct Cycle {
    valid: bool,
    b0: i64,
    b1: i64,
    e0: f64,
    len: f64,
    /// The next cycle's first read: its delay and level.
    d_first: f64,
    v_first: f32,
    /// Whether the time curve jumps at the cycle's end.
    wrap_jump: bool,
}

/// Lapsus's own telemetry, after the family's header (`modfx_clock::tel`).
pub mod t {
    use crate::modfx_clock::tel::OWN;
    /// The slot playing (0 … 35).
    pub const SLOT: usize = OWN;
    /// Where in the cycle (0 … 1) at the end of the last block.
    pub const PHASE: usize = OWN + 1;
    /// The time curve's value (cycle lengths) there.
    pub const DELAY: usize = OWN + 2;
    /// The volume's level there.
    pub const LEVEL: usize = OWN + 3;
    /// The read's speed against real time.
    pub const SPEED: usize = OWN + 4;
    /// Quarter notes in the cycle.
    pub const BEATS: usize = OWN + 5;
    /// 1 while a note chooses the pattern.
    pub const NOTE: usize = OWN + 6;
    /// The slot the knob sets.
    pub const KNOB: usize = OWN + 7;
    /// Seconds of history held.
    pub const HELD_S: usize = OWN + 8;
    /// 1 while a join's crossfade runs.
    pub const JOIN: usize = OWN + 9;
}

#[derive(Clone, Debug)]
pub struct Lapsus {
    sr: f32,
    set: LapsusSettings,
    shape: ShapeFn,
    bank: Box<[Pattern; SLOTS]>,
    clock: Clock,
    hist: History,
    notes: [(i64, Note); MAX_NOTES],
    note_n: usize,
    held: [u8; SLOTS],
    held_n: usize,
    latched: Option<u8>,
    origin: Option<i64>,
    active: usize,
    cyc: Cycle,
    /// The delay at the last two song samples evaluated: (sample, delay). A cache.
    dc: [(i64, f64); 2],
    glide: f32,
    kv: f32,
    ghost: Ghost,
    /// The output song sample of the last jump the detector found.
    jumped: i64,
    /// Something not of the pattern alone happened since the cycle began (a switch, a jump).
    dirty: bool,
    mix: Glide,
    out: Glide,
    ahead: usize,
    xf: u32,
    fv: u32,
    tel: [f32; 24],
    /// The last tick's phase, delay (lengths), level and speed, for the telemetry.
    last: [f32; 4],
    blocks: u32,
}

impl Lapsus {
    /// A unit whose segments take `shape` (the automation editor's `Shape::at`). Its bank is
    /// *Through* everywhere until [`Lapsus::set_pattern`]; its history is empty until
    /// [`Lapsus::supply`] (a read of the past is silence meanwhile; the live signal passes).
    pub fn new(sample_rate: f32, shape: ShapeFn) -> Lapsus {
        // The kernels, here, off the audio thread.
        let _ = read::kernels();
        let set = LapsusSettings::default();
        let mut u = Lapsus {
            sr: sample_rate,
            set,
            shape,
            bank: Box::new([Pattern::THROUGH; SLOTS]),
            clock: Clock::new(),
            hist: History::default(),
            notes: [(0, Note::AllOff); MAX_NOTES],
            note_n: 0,
            held: [0; SLOTS],
            held_n: 0,
            latched: None,
            origin: None,
            active: 0,
            cyc: Cycle::default(),
            dc: [(i64::MIN, 0.0); 2],
            glide: 1.0,
            kv: 1.0,
            ghost: Ghost::default(),
            jumped: i64::MIN,
            dirty: false,
            mix: Glide::new(1.0, 10.0, sample_rate, 1e-6),
            out: Glide::new(1.0, 10.0, sample_rate, 1e-6),
            ahead: 0,
            xf: 0,
            fv: 0,
            tel: [0.0; 24],
            last: [0.0, 0.0, 1.0, 1.0],
            blocks: 0,
        };
        u.set(&set);
        u.reset();
        u
    }

    pub fn settings(&self) -> &LapsusSettings {
        &self.set
    }

    pub fn pattern(&self, slot: usize) -> &Pattern {
        &self.bank[slot.min(SLOTS - 1)]
    }

    /// The slot playing now.
    pub fn active(&self) -> usize {
        self.active
    }

    pub fn latency(&self) -> usize {
        self.ahead
    }

    /// History frames the bank and the lookahead need: the longest pattern at
    /// [`MIN_TEMPO`], the lookahead and the longest kernel.
    pub fn need_frames(&self) -> usize {
        let beats = self.bank.iter().map(|p| p.beats).fold(0.0, f64::max);
        let past = (beats * 60.0 / MIN_TEMPO * f64::from(self.sr)).ceil() as usize;
        past + self.ahead + read::MAX_TAPS + 8
    }

    /// The history the settings need and the unit has not got.
    pub fn memory_need(&self) -> Option<usize> {
        let n = self.need_frames();
        (n > self.hist.frames()).then_some(n)
    }

    /// New history (built off the audio thread); the old one is handed back. What was
    /// heard before is not read any more.
    pub fn supply(&mut self, ring: Ring) -> Ring {
        let n = self.clock.stream(0);
        self.hist.supply(ring, n)
    }

    /// Allocates: the history the settings need, at once (tests, tools, offline).
    pub fn ensure_memory(&mut self) {
        if let Some(n) = self.memory_need() {
            drop(self.supply(Ring::new(n)));
        }
    }

    pub fn set(&mut self, s: &LapsusSettings) {
        let sr = f64::from(self.sr);
        let n = f64::from(s.smooth_ms.clamp(0.0, 20.0)) * 0.001 * sr;
        self.kv = if n < 1.0 {
            1.0
        } else {
            1.0 - crate::math::exp(-1.0 / n) as f32
        };
        self.fv = if n < 1.0 { 0 } else { (3.0 * n).round() as u32 };
        self.xf = (f64::from(s.xfade_ms.clamp(0.0, 20.0)) * 0.001 * sr).round() as u32;
        let ahead = (f64::from(s.ahead_ms.clamp(0.0, 250.0)) * 0.001 * sr).round() as usize;
        if ahead != self.ahead {
            self.ahead = ahead;
            self.invalidate();
        }
        self.mix.target = s.mix.clamp(0.0, 1.0);
        self.out.target = if s.out_db == 0.0 {
            1.0
        } else {
            crate::math::db_to_gain(s.out_db)
        };
        self.set = LapsusSettings {
            slot: s.slot.min(SLOTS - 1),
            ..*s
        };
        if s.notes == NoteMode::Off {
            self.held_n = 0;
            self.latched = None;
            self.origin = None;
        }
        self.choose_at(self.so_next());
    }

    /// Set slot `slot`'s pattern; a change to the one playing fades into it.
    pub fn set_pattern(&mut self, slot: usize, p: &Pattern) {
        let slot = slot.min(SLOTS - 1);
        if self.bank[slot] == *p {
            return;
        }
        if slot == self.active {
            self.start_ghost_at(self.so_next());
        }
        self.bank[slot] = *p;
        if slot == self.active {
            self.invalidate();
            self.dirty = true;
        }
    }

    /// Whether `points` would change slot `slot`'s curves or length (the engine's check).
    pub fn pattern_differs(&self, slot: usize, beats: f64, time: &[CurvePoint], vol: &[CurvePoint]) -> bool {
        let p = &self.bank[slot.min(SLOTS - 1)];
        p.beats != beats || !p.time.same_as(time, TIME_LO, TIME_HI) || !p.vol.same_as(vol, 0.0, 1.0)
    }

    pub fn set_transport(&mut self, t: &Transport) {
        let n = self.clock.stream(0);
        let (was, before, tempo) = (self.clock.playing(), self.clock.song(0), self.clock.tempo());
        self.clock.set(t);
        let started = t.playing && !was;
        let moved = t.playing && was && self.clock.tempo() == tempo && (t.samples - before).abs() > 2;
        if started || moved {
            // The past is another part of the song now: nothing before here is heard.
            self.hist.restart(n);
            self.ghost.on = false;
            self.invalidate();
            self.glide = self.level_now();
            self.dirty = true;
        } else if self.clock.tempo() != tempo {
            self.invalidate();
        }
    }

    /// A note `offset` samples into the next [`Lapsus::process`] call, acted on at its
    /// sample whatever the block size. No allocation; past [`MAX_NOTES`] waiting, the
    /// oldest acts at once.
    pub fn note(&mut self, offset: usize, note: Note) {
        if self.note_n == MAX_NOTES {
            let (_, oldest) = self.notes[0];
            self.notes.copy_within(1.., 0);
            self.note_n -= 1;
            self.apply(oldest, self.so_next());
        }
        self.notes[self.note_n] = (self.clock.stream(offset), note);
        self.note_n += 1;
    }

    pub fn reset(&mut self) {
        self.clock.reset();
        self.hist.clear(0);
        self.note_n = 0;
        self.held_n = 0;
        self.latched = None;
        self.origin = None;
        self.active = self.set.slot;
        self.ghost = Ghost::default();
        self.invalidate();
        self.glide = self.level_now();
        self.dirty = false;
        self.mix.jump(self.mix.target);
        self.out.jump(self.out.target);
        self.tel = [0.0; 24];
        self.blocks = 0;
    }

    /// At rest: everything a read could see is silence, no crossfade runs, no note chooses
    /// the pattern, the cycle has begun again since anything that is not the pattern's own
    /// (so the volume's glide is where a new unit's is), and the glides have landed.
    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    pub fn rest_blocker(&self) -> Option<&'static str> {
        if !self.hist.silent(self.clock.stream(0)) {
            Some("the history holds sound")
        } else if self.ghost.on {
            Some("a join is fading")
        } else if self.note_n > 0 || self.held_n > 0 || self.latched.is_some() || self.origin.is_some() {
            Some("a note chose the pattern")
        } else if self.dirty {
            Some("the cycle has not begun again")
        } else if !self.mix.still() || !self.out.still() {
            Some("a glide is moving")
        } else {
            None
        }
    }

    /// The output's song sample of the next sample processed (between blocks).
    fn so_next(&self) -> i64 {
        self.clock.song(0) - self.ahead as i64
    }

    /// The pattern a note or the slot chooses; switching at output song sample `so` fades
    /// into it.
    fn choose_at(&mut self, so: i64) {
        let want = match self.set.notes {
            NoteMode::Off => self.set.slot,
            NoteMode::Switch => self.latched.map_or(self.set.slot, usize::from),
            NoteMode::Hold => {
                if self.held_n > 0 {
                    usize::from(self.held[self.held_n - 1])
                } else {
                    self.set.slot
                }
            }
        };
        if want != self.active {
            self.start_ghost_at(so);
            self.active = want;
            self.invalidate();
            self.dirty = true;
        }
    }

    /// The cycle's cache and the delay's cache start again.
    fn invalidate(&mut self) {
        self.cyc.valid = false;
        self.dc = [(i64::MIN, 0.0); 2];
    }

    fn overridden(&self) -> bool {
        self.held_n > 0 || self.latched.is_some() || self.origin.is_some()
    }
}

#[cfg(test)]
#[path = "lapsus_tests.rs"]
mod tests;
