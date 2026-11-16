//! Fractus, the glitch sequencer and grain cloud (wave A of the effects side round; name
//! provisional — [`NAME`]). Effectrix's and Stutter Edit's job — repeats, reverses, tape
//! stops, gates, crushes, filter sweeps, pitch jumps, delay throws and shuffled slices,
//! each painted on its own lane of a step sequencer synced to the song — with a granular
//! cloud beside it (Portal's and Granulator's job) that can be painted in too. Lapsus is a
//! time curve; Fractus is a *sequence of events*, each module's own.
//!
//! - **The grid**: steps of 1/4 … 1/64 (triplets too) over 1 … 4 bars of 4/4, with swing
//!   (the odd steps late by up to half a step). A step's line is the sample the engine puts
//!   that beat on: `round(beats · 60/tempo · fs)` from the song's start — exact at every
//!   tempo, a function of the song sample alone.
//! - **Events**: a lane holds blocks — a *start* step and the *hold* steps after it — and a
//!   block is one event from its start's line to the line after its last step. Each start
//!   has an amount (the module's mix for that event) and a probability, drawn from a hash
//!   of the seed, the lane and the step's absolute index from the song's start: the same
//!   song gives the same rolls, offline or live, from bar 1 or from bar 5. The cycle's line
//!   always breaks a block.
//! - **Performance**: held notes (`key + lane`) play a lane's module from the note's sample
//!   (or the next line, quantised) until the note is released, its velocity the amount.
//!   While the transport is stopped the steps rest and only notes play. Bars are 4/4
//!   from the song's start (as Lapsus's).
//! - **The read modules** (repeat, reverse, tape stop, shuffle) say, at every sample, where
//!   in the past to read; the newest event wins. The history is read with Lapsus's reader
//!   (a whole sample is that sample to the bit; a line, a Hermite, a 32-tap Kaiser sinc by
//!   distance, anti-aliased by speed). Repeats read a capture of their own slice, so a
//!   repeat held for minutes still repeats its first slice.
//! - **The processors** (pitch jump, crush, filter sweep, gate, delay throw) work in series
//!   on what was read, each faded in and out by its own event.
//! - **The cloud** reads grains from its own buffer of the input — position, spray, size
//!   5 … 500 ms, density, pitch ±24 st quantised to a scale, spread, reverse, window,
//!   freeze — every draw a hash of the seed and the grain's number on the song's grid. Its
//!   level runs in parallel; its lane swaps the chain for the cloud.
//! - **Joins**: an event's effect starts on its first sample and is gone by the next line,
//!   to the sample: what ends fades out over the last `xfade` *before* the line (towards
//!   whatever plays after it), what jumps inside an event (a repeat's restart, a slice)
//!   fades from the old read carrying on at its speed into the new one (a raised cosine).
//! - **Latency 0**; at the defaults (nothing painted, no cloud) the output is the input, to
//!   the bit; at rest when everything a read could hear is silence.
//!
//! **Determinism**: every choice (which step plays, every roll, every grain's start, pitch,
//! pan and direction) is a function of the song sample and the seed. The sound of a render
//! from a cycle's line equals the whole render's from there once every read reaches only
//! audio after the line: at once for repeats, gates, crushes, filter sweeps and tape stops
//! (they read only their own event), after a slice for reverses and shuffles, after the
//! window for pitch jumps, after the echoes for throws and after the grains' reach for the
//! cloud ([`Fractus::reach`]).

#[path = "fractus_cloud.rs"]
pub mod cloud;
#[path = "fractus_fx.rs"]
mod fx;
#[path = "fractus_read.rs"]
mod read;
#[path = "fractus_run.rs"]
mod run;
#[path = "fractus_seq.rs"]
mod seq;

use crate::lapsus::read::{History, Ring};
use crate::modfx_clock::{Clock, Transport};
use crate::modfx_line::Glide;
pub use cloud::{Cloud, CloudSettings, GrainWindow, Scale};
pub use fx::ThrowSettings;
pub use fx::{CrushSettings, FilterKind, FilterSettings, GateSettings, PitchSettings};
pub use read::{RepeatSettings, ReverseSettings, ShuffleSettings, TapeKind, TapeSettings};

/// The unit's name: one constant, provisional.
pub const NAME: &str = "Fractus";
/// Lanes, one per module.
pub const LANES: usize = 10;
/// The longest sequence: 4 bars of 1/64.
pub const MAX_STEPS: usize = 256;
pub const MAX_BARS: u8 = 4;
/// The slowest tempo the buffers are sized for (slower reads silence beyond them).
pub const MIN_TEMPO: f64 = 60.0;
/// Seconds of input the read modules may reach back.
pub const HISTORY_S: f64 = 8.0;
/// Seconds a repeat's capture holds (a 1/2 note at 60 BPM).
pub const CAPTURE_S: f64 = 2.05;
/// Notes waiting for their sample.
const MAX_NOTES: usize = 64;

/// The modules, in lane order: the four that read the past first.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Module {
    Repeat = 0,
    Reverse = 1,
    TapeStop = 2,
    Shuffle = 3,
    Pitch = 4,
    Crush = 5,
    Filter = 6,
    Gate = 7,
    Throw = 8,
    Cloud = 9,
}

impl Module {
    pub const ALL: [Module; LANES] = [
        Module::Repeat,
        Module::Reverse,
        Module::TapeStop,
        Module::Shuffle,
        Module::Pitch,
        Module::Crush,
        Module::Filter,
        Module::Gate,
        Module::Throw,
        Module::Cloud,
    ];
    /// The modules that choose where to read.
    pub const READS: [Module; 4] = [
        Module::Repeat,
        Module::Reverse,
        Module::TapeStop,
        Module::Shuffle,
    ];

    pub fn lane(self) -> usize {
        self as usize
    }

    pub fn from_lane(lane: usize) -> Option<Module> {
        Module::ALL.get(lane).copied()
    }

    /// A stable key for the model and the locale files (`fractus.module.{key}`).
    pub fn key(self) -> &'static str {
        match self {
            Module::Repeat => "repeat",
            Module::Reverse => "reverse",
            Module::TapeStop => "tape",
            Module::Shuffle => "shuffle",
            Module::Pitch => "pitch",
            Module::Crush => "crush",
            Module::Filter => "filter",
            Module::Gate => "gate",
            Module::Throw => "throw",
            Module::Cloud => "cloud",
        }
    }
}

/// The step lengths offered, in quarter notes: 1/4, 1/8, 1/8T, 1/16, 1/16T, 1/32, 1/64.
pub const STEP_BEATS: [f64; 7] = [1.0, 0.5, 1.0 / 3.0, 0.25, 1.0 / 6.0, 0.125, 0.0625];
/// The index of 1/16 in [`STEP_BEATS`].
pub const STEP_SIXTEENTH: usize = 3;

/// Note values a module's size may take, in quarter notes (`fractus.note.{i}`).
pub const NOTE_BEATS: [f64; 14] = [
    2.0,        // 1/2
    1.5,        // 1/4·
    1.0,        // 1/4
    0.75,       // 1/8·
    2.0 / 3.0,  // 1/4T
    0.5,        // 1/8
    0.375,      // 1/16·
    1.0 / 3.0,  // 1/8T
    0.25,       // 1/16
    1.0 / 6.0,  // 1/16T
    0.125,      // 1/32
    1.0 / 12.0, // 1/32T
    0.0625,     // 1/64
    0.03125,    // 1/128
];

/// The beats of note value `index` (clamped into [`NOTE_BEATS`]).
pub fn note_beats(index: f32) -> f64 {
    let i = if index.is_finite() {
        index.round().clamp(0.0, (NOTE_BEATS.len() - 1) as f32) as usize
    } else {
        8
    };
    NOTE_BEATS[i]
}

/// What a step holds.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum StepKind {
    #[default]
    Off,
    /// An event starts here.
    Start,
    /// The event before goes on through this step.
    Hold,
}

/// One step of one lane.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Step {
    pub kind: StepKind,
    /// The module's mix for the event that starts here, 0 … 1.
    pub amount: f32,
    /// The chance the event that starts here plays, 0 … 1.
    pub prob: f32,
}

impl Step {
    pub const OFF: Step = Step {
        kind: StepKind::Off,
        amount: 1.0,
        prob: 1.0,
    };
    pub const HOLD: Step = Step {
        kind: StepKind::Hold,
        amount: 1.0,
        prob: 1.0,
    };
    pub const fn start(amount: f32, prob: f32) -> Step {
        Step {
            kind: StepKind::Start,
            amount,
            prob,
        }
    }
    pub const ON: Step = Step::start(1.0, 1.0);
}

impl Default for Step {
    fn default() -> Self {
        Step::OFF
    }
}

/// Every lane's steps.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Pattern {
    pub lanes: [[Step; MAX_STEPS]; LANES],
}

impl Pattern {
    pub const EMPTY: Pattern = Pattern {
        lanes: [[Step::OFF; MAX_STEPS]; LANES],
    };

    /// Paint a block on `lane`: a start at `from` and holds through `len` steps.
    pub fn block(&mut self, m: Module, from: usize, len: usize, amount: f32, prob: f32) {
        let l = &mut self.lanes[m.lane()];
        for (k, s) in l.iter_mut().enumerate().skip(from).take(len.max(1)) {
            *s = if k == from {
                Step::start(amount, prob)
            } else {
                Step::HOLD
            };
        }
    }

    pub fn is_empty(&self) -> bool {
        self.lanes
            .iter()
            .all(|l| l.iter().all(|s| s.kind == StepKind::Off))
    }

    pub fn lane_empty(&self, m: Module, steps: usize) -> bool {
        self.lanes[m.lane()][..steps.min(MAX_STEPS)]
            .iter()
            .all(|s| s.kind == StepKind::Off)
    }
}

/// What triggers the modules.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Trigger {
    /// The painted steps.
    #[default]
    Sequence,
    /// Held notes only.
    Notes,
    /// Both; a held note replaces its lane's steps while it is down.
    Both,
}

impl Trigger {
    pub fn from_index(v: f32) -> Trigger {
        match v.round() as i32 {
            1 => Trigger::Notes,
            2 => Trigger::Both,
            _ => Trigger::Sequence,
        }
    }
    fn steps(self) -> bool {
        self != Trigger::Notes
    }
    fn notes(self) -> bool {
        self != Trigger::Sequence
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FractusSettings {
    /// A step's length, quarter notes ([`STEP_BEATS`]).
    pub step: f64,
    /// Bars in a cycle, 1 … 4.
    pub bars: u8,
    /// The odd steps late by up to half a step, 0 … 1.
    pub swing: f32,
    /// Joins, ms (0.5 … 10).
    pub xfade_ms: f32,
    pub trigger: Trigger,
    /// A note waits for the next line.
    pub quantise: bool,
    /// The note of lane 0 (Repeat); the next nine lanes on the keys above.
    pub key: u8,
    /// Every roll's seed.
    pub seed: u32,
    /// The processed signal against the dry, 0 … 1.
    pub mix: f32,
    pub out_db: f32,
    pub repeat: RepeatSettings,
    pub reverse: ReverseSettings,
    pub tape: TapeSettings,
    pub shuffle: ShuffleSettings,
    pub pitch: PitchSettings,
    pub crush: CrushSettings,
    pub filter: FilterSettings,
    pub gate: GateSettings,
    pub throw: ThrowSettings,
    pub cloud: CloudSettings,
}

impl Default for FractusSettings {
    fn default() -> Self {
        FractusSettings {
            step: 0.25,
            bars: 1,
            swing: 0.0,
            xfade_ms: 2.0,
            trigger: Trigger::Sequence,
            quantise: true,
            key: 48,
            seed: 1,
            mix: 1.0,
            out_db: 0.0,
            repeat: RepeatSettings::default(),
            reverse: ReverseSettings::default(),
            tape: TapeSettings::default(),
            shuffle: ShuffleSettings::default(),
            pitch: PitchSettings::default(),
            crush: CrushSettings::default(),
            filter: FilterSettings::default(),
            gate: GateSettings::default(),
            throw: ThrowSettings::default(),
            cloud: CloudSettings::default(),
        }
    }
}

impl FractusSettings {
    /// Steps in a cycle.
    pub fn steps(&self) -> usize {
        let per_bar = (4.0 / self.step).round() as usize;
        (per_bar * usize::from(self.bars.clamp(1, MAX_BARS))).clamp(1, MAX_STEPS)
    }
}

/// A note as Fractus hears it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Note {
    /// Pitch and velocity (1 … 127).
    On(u8, u8),
    Off(u8),
    /// Every note stops (the transport stopped, sought or wrapped).
    AllOff,
}

/// Fractus's own telemetry, after the family's header (`modfx_clock::tel`).
pub mod t {
    use crate::modfx_clock::tel::OWN;
    /// The step playing (0 … steps − 1) at the end of the last block.
    pub const STEP: usize = OWN;
    /// Steps in the cycle.
    pub const STEPS: usize = OWN + 1;
    /// Where in the cycle (0 … 1).
    pub const PHASE: usize = OWN + 2;
    /// The lanes sounding, a bit each (lane 0 = 1).
    pub const ACTIVE: usize = OWN + 3;
    /// The lanes a held note plays, a bit each.
    pub const NOTES: usize = OWN + 4;
    /// Grains sounding.
    pub const GRAINS: usize = OWN + 5;
    /// 1 while the cloud is frozen.
    pub const FROZEN: usize = OWN + 6;
    /// The read module playing, lane + 1 (0: the live signal).
    pub const READ: usize = OWN + 7;
    /// Up to [`SHOWN`] grains: where each reads (0 now … 1 the cloud's reach) and its
    /// pitch (−1 … 1 for ±24 st), in pairs.
    pub const GRAIN0: usize = OWN + 8;
    pub const SHOWN: usize = 16;
    /// The end of Fractus's numbers.
    pub const END: usize = GRAIN0 + 2 * SHOWN;
}

/// A sounding event: from its start's line to its end (exclusive), song samples.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct Ev {
    pub start: i64,
    pub end: i64,
    pub amount: f32,
    /// The event's own seed (its start step's absolute index, or its note's sample).
    pub seed: u64,
}

#[derive(Clone, Debug)]
pub struct Fractus {
    sr: f32,
    set: FractusSettings,
    pat: Box<Pattern>,
    clock: Clock,
    hist: History,
    cap: History,
    /// The stream sample last written into the capture.
    cap_head: i64,
    seq: seq::Seq,
    reader: read::Reader,
    fx: fx::Chain,
    cloud: Cloud,
    notes: [(i64, Note); MAX_NOTES],
    note_n: usize,
    mix: Glide,
    out: Glide,
    xf: u32,
    /// The lanes sounding and the read playing at the last sample processed.
    act: u32,
    read_lane: u8,
    tel: [f32; 64],
    blocks: u32,
}

impl Fractus {
    /// A unit at `sample_rate` with its buffers (this allocates: build it off the audio
    /// thread). Nothing painted: a wire.
    pub fn new(sample_rate: f32) -> Fractus {
        let _ = crate::lapsus::read::kernels();
        let sr = f64::from(sample_rate);
        let mut hist = History::default();
        drop(hist.supply(Ring::new((HISTORY_S * sr) as usize + 256), 0));
        let mut cap = History::default();
        drop(cap.supply(Ring::new((CAPTURE_S * sr) as usize + 256), 0));
        let set = FractusSettings::default();
        let mut u = Fractus {
            sr: sample_rate,
            set,
            pat: Box::new(Pattern::EMPTY),
            clock: Clock::new(),
            hist,
            cap,
            cap_head: -1,
            seq: seq::Seq::new(),
            reader: read::Reader::new(),
            fx: fx::Chain::new(sample_rate),
            cloud: Cloud::new(sample_rate),
            notes: [(0, Note::AllOff); MAX_NOTES],
            note_n: 0,
            mix: Glide::new(1.0, 10.0, sample_rate, 1e-6),
            out: Glide::new(1.0, 10.0, sample_rate, 1e-6),
            xf: 96,
            act: 0,
            read_lane: 0,
            tel: [0.0; 64],
            blocks: 0,
        };
        u.set(&set);
        u.reset();
        u
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    pub fn settings(&self) -> &FractusSettings {
        &self.set
    }

    pub fn pattern(&self) -> &Pattern {
        &self.pat
    }

    /// Latency: none.
    pub fn latency(&self) -> usize {
        0
    }

    /// Bytes of buffers this unit holds (they scale with the sample rate).
    pub fn memory_bytes(&self) -> usize {
        4 * 2 * (self.hist.frames() + self.cap.frames())
            + self.cloud.memory_bytes()
            + self.fx.memory_bytes()
    }

    pub fn set(&mut self, s: &FractusSettings) {
        let sr = f64::from(self.sr);
        let step = if s.step.is_finite() && s.step > 0.0 {
            s.step.clamp(STEP_BEATS[6], 1.0)
        } else {
            0.25
        };
        let s = FractusSettings {
            step,
            bars: s.bars.clamp(1, MAX_BARS),
            swing: s.swing.clamp(0.0, 1.0),
            xfade_ms: s.xfade_ms.clamp(0.5, 10.0),
            mix: s.mix.clamp(0.0, 1.0),
            ..*s
        };
        if s.step != self.set.step || s.bars != self.set.bars || s.swing != self.set.swing {
            self.seq.invalidate();
        }
        if s.seed != self.set.seed {
            self.seq.invalidate();
        }
        self.xf = (f64::from(s.xfade_ms) * 0.001 * sr).round().max(1.0) as u32;
        self.mix.target = s.mix;
        self.out.target = if s.out_db == 0.0 {
            1.0
        } else {
            crate::math::db_to_gain(s.out_db.clamp(-60.0, 24.0))
        };
        if !s.trigger.notes() {
            self.seq.notes_off_now();
            self.note_n = 0;
        }
        self.fx.set(&s, self.sr);
        self.cloud.set(&s.cloud, s.seed);
        self.set = s;
        self.cloud_enable();
    }

    /// Replace the whole pattern (no allocation).
    pub fn set_pattern(&mut self, p: &Pattern) {
        if *self.pat != *p {
            *self.pat = *p;
            self.seq.invalidate();
            self.cloud_enable();
        }
    }

    /// One step of one lane.
    pub fn set_step(&mut self, m: Module, k: usize, s: Step) {
        if k < MAX_STEPS && self.pat.lanes[m.lane()][k] != s {
            self.pat.lanes[m.lane()][k] = s;
            self.seq.invalidate();
            self.cloud_enable();
        }
    }

    fn cloud_enable(&mut self) {
        let painted = !self.pat.lane_empty(Module::Cloud, self.set.steps());
        let on = self.set.cloud.level > 0.0
            || self.set.cloud.freeze
            || (painted && self.set.trigger.steps())
            || self.set.trigger.notes();
        self.cloud.enable(on);
    }

    pub fn set_transport(&mut self, t: &Transport) {
        let n = self.clock.stream(0);
        let (was, before, tempo) = (self.clock.playing(), self.clock.song(0), self.clock.tempo());
        self.clock.set(t);
        let started = t.playing && !was;
        let moved =
            t.playing && was && self.clock.tempo() == tempo && (t.samples - before).abs() > 2;
        if started || moved {
            // Another part of the song: nothing before here is heard.
            self.hist.restart(n);
            self.cap.restart(n);
            self.reader.restart();
            self.fx.restart();
            self.cloud.restart(self.clock.song(0));
            self.seq.invalidate();
        } else if self.clock.tempo() != tempo {
            self.seq.invalidate();
        }
    }

    /// A note `offset` samples into the next [`Fractus::process`] call, acted on at its
    /// sample whatever the block size. No allocation; past [`MAX_NOTES`] waiting, the
    /// oldest acts at once.
    pub fn note(&mut self, offset: usize, note: Note) {
        if !self.set.trigger.notes() {
            return;
        }
        if self.note_n == MAX_NOTES {
            let (_, oldest) = self.notes[0];
            self.notes.copy_within(1.., 0);
            self.note_n -= 1;
            let so = self.clock.song(0);
            self.apply_note(oldest, so);
        }
        self.notes[self.note_n] = (self.clock.stream(offset), note);
        self.note_n += 1;
    }

    pub fn reset(&mut self) {
        self.clock.reset();
        self.hist.clear(0);
        self.cap.clear(0);
        self.cap_head = -1;
        self.note_n = 0;
        self.seq.reset();
        self.reader.restart();
        self.fx.reset();
        self.cloud.reset();
        self.mix.jump(self.mix.target);
        self.out.jump(self.out.target);
        self.tel = [0.0; 64];
        self.blocks = 0;
    }

    /// At rest: everything a read could hear is silence, no note plays, nothing fades and
    /// the glides have landed — so a skip is what processing silence would have done.
    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    pub fn rest_blocker(&self) -> Option<&'static str> {
        let n = self.clock.stream(0);
        if !self.hist.silent(n) {
            Some("the history holds sound")
        } else if !self.cap.silent(self.cap_head + 1) {
            Some("a repeat's capture holds sound")
        } else if self.note_n > 0 || self.seq.notes_held() {
            Some("a note plays")
        } else if self.reader.fading() {
            Some("a join fades")
        } else if !self.fx.silent() {
            Some("a processor holds sound")
        } else if !self.cloud.silent() {
            Some("the cloud holds sound")
        } else if !self.mix.still() || !self.out.still() {
            Some("a glide is moving")
        } else {
            None
        }
    }

    /// How far back, in samples, a read may reach before its event's start with these
    /// settings at `tempo`: a render from a cycle's line equals the whole render's from
    /// there after this many samples (0: at once).
    pub fn reach(&self, tempo: f64) -> usize {
        let spb = 60.0 / tempo.max(1.0) * f64::from(self.sr);
        let steps = self.set.steps();
        let used = |m: Module| !self.pat.lane_empty(m, steps);
        let mut r = 0.0f64;
        if used(Module::Reverse) {
            let s = if self.set.reverse.slice > 0.0 {
                self.set.reverse.slice
            } else {
                self.set.step
            };
            r = r.max(s * spb + 64.0);
        }
        if used(Module::Shuffle) {
            let s = &self.set.shuffle;
            r = r.max(s.slice * spb * f64::from(s.range.max(1)) + 64.0);
        }
        if used(Module::Pitch) {
            r = r.max(f64::from(self.set.pitch.window_ms) * 0.001 * f64::from(self.sr) + 8.0);
        }
        if used(Module::Throw) {
            // The echoes ring until they are flushed to zero, 160 dB down.
            let t = &self.set.throw;
            let fb = f64::from(t.feedback.clamp(0.0, 0.95)).max(1e-6);
            let echoes = (-8.0 / crate::math::log10(fb)).ceil().min(1_000.0);
            r = r.max(t.time * spb * (echoes + 1.0));
        }
        if self.cloud.enabled() {
            r = r.max(self.cloud.reach() as f64);
        }
        r.ceil() as usize
    }

    /// The telemetry: the family's header and Fractus's own ([`t`]).
    pub fn telemetry(&self, out: &mut [f32; 64]) {
        out.copy_from_slice(&self.tel);
    }
}

#[cfg(test)]
#[path = "fractus_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "fractus_tests2.rs"]
mod tests2;
