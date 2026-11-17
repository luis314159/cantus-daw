//! **Saltus**, the pitch unit (ADR 0132) — Latin *saltus*, the leap: a pitch
//! shifter, a harmoniser, an octaver and a whammy in one, each a way of moving the pitch
//! of what it hears, and each also a pedal on Tabula's board.
//!
//! ## The four modes, the two engines
//!
//! | Mode | What it does | Engine | Latency at 48 kHz |
//! |---|---|---|---|
//! | **Shift** | two voices, each moved by a fixed interval (semitones and cents) | mono: PSOLA, formants kept · poly: phase vocoder | 128 · 4 095 |
//! | **Harmony** | two voices a number of *scale degrees* from the note heard — in a key and scale, in the chord held on MIDI, or singing the notes held | PSOLA | 128 |
//! | **Octave** | one and two octaves down and one up — from a divider that follows the note (no latency), or clean by PSOLA | divider · PSOLA | 0 · 128 |
//! | **Whammy** | one voice swept by a pedal between a heel and a toe interval, glided — or pressed to the toe while a MIDI note is held | mono: PSOLA · poly: phase vocoder | 128 · 4 095 |
//!
//! - **Mono** ([`crate::psola::Shifter`], ADR 0053): the pitch base's streaming tracker
//!   ([`crate::pitch::PitchTracker`]) hears the note and pitch-synchronous grains move it,
//!   keeping its formants (or letting them follow, or moving them by the *throat*). For a
//!   voice, a lead line, a bass: anything that plays one note at a time.
//! - **Poly** ([`poly::Poly`]): a streaming phase-locked vocoder (4 096 points at 48 kHz,
//!   hops of 512) that moves every partial at once, the spectral envelope kept by the true
//!   envelope (the stretcher's). For chords and whole mixes; it costs 85 ms.
//! - **The divider** ([`octave::Divider`]): a comparator on the note's fundamental (behind
//!   a low-pass the tracker keeps just above it), and sub-octave sines locked to its edges,
//!   carrying the note's level; the octave up is the fundamental rectified. Nothing waits
//!   for an analysis: latency zero, as Boss's OC-2 and its descendants.
//!
//! **The latency is fixed by the settings** — a function of the mode, the engine and the
//! octave's style ([`latency_for`]), never of the audio. The dry path is delayed to match.
//!
//! ## The null
//!
//! At the defaults — Shift, mono, voice 1 at 0 semitones and 0 cents at full level and
//! centred, voice 2 silent, no dry, formants kept, throat 0, output 0 dB — the shifter is
//! never engaged and the output is the input delayed by 128 samples at 48 kHz, **bit for
//! bit**. A level of exactly 100 % is not multiplied, a pan of 0 is not applied, a level of
//! 0 is not added, and sums start at −0.0 (the additive identity of floats), so nothing
//! touches a sample that passes.
//!
//! ## At rest
//!
//! Exactly as new: the running engine at rest (the tracker, the shifters, the vocoder or
//! the divider), the dry line empty, no note held that the output could hear, every glide
//! landed. [`Saltus::skip`] advances the counters only. An engine that is not running is
//! reset and brought to the count when it is chosen, so it starts as a new unit's would.

use crate::math;
use crate::pitch::PitchTracker;
use crate::rest::ZeroRun;
use crate::tonus::Scale;

pub use crate::tonus::NoteEvent;

#[path = "saltus_octave.rs"]
pub mod octave;
#[path = "saltus_poly.rs"]
pub mod poly;
#[path = "saltus_voice.rs"]
pub mod voice;

#[cfg(test)]
#[path = "saltus_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "saltus_tests_measure.rs"]
mod tests_measure;
#[cfg(test)]
#[path = "saltus_tests_poly.rs"]
mod tests_poly;

use voice::{Keys, Voices};

/// What Saltus does (the parameter `mode`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// Two voices at fixed intervals.
    #[default]
    Shift,
    /// Two voices in scale degrees from the note heard.
    Harmony,
    /// Octaves down and up.
    Octave,
    /// One voice swept by the pedal.
    Whammy,
}

impl Mode {
    pub const ALL: [Mode; 4] = [Mode::Shift, Mode::Harmony, Mode::Octave, Mode::Whammy];

    pub fn from_index(i: usize) -> Mode {
        Mode::ALL.get(i).copied().unwrap_or_default()
    }
}

/// How the Shift and Whammy modes move the pitch (the parameter `engine`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Engine {
    /// One note at a time, formants kept, 2.7 ms.
    #[default]
    Mono,
    /// Chords and mixes, by phase vocoder, 85 ms.
    Poly,
}

impl Engine {
    pub fn from_index(i: usize) -> Engine {
        if i == 1 {
            Engine::Poly
        } else {
            Engine::Mono
        }
    }
}

/// How the Octave mode makes its octaves (the parameter `ostyle`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum OctaveStyle {
    /// The divider: no latency, the analog octaver's character.
    #[default]
    Analog,
    /// By PSOLA: the note itself an octave away, 2.7 ms.
    Clean,
}

impl OctaveStyle {
    pub fn from_index(i: usize) -> OctaveStyle {
        if i == 1 {
            OctaveStyle::Clean
        } else {
            OctaveStyle::Analog
        }
    }
}

/// Where the Harmony mode's intervals are counted (the parameter `hsrc`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum HarmonySource {
    /// In the key and scale: a degree is a step of the scale.
    #[default]
    Scale,
    /// In the chord held on MIDI: a degree is a step to the next note of the chord; with
    /// nothing held the voices are silent.
    Chord,
    /// The notes held on MIDI, sung: voice 1 the newest, voice 2 the one before; the
    /// singer's own vibrato carried.
    Melody,
}

impl HarmonySource {
    pub fn from_index(i: usize) -> HarmonySource {
        match i {
            1 => HarmonySource::Chord,
            2 => HarmonySource::Melody,
            _ => HarmonySource::Scale,
        }
    }
}

/// One of the two voices.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct VoiceSettings {
    /// The Shift mode's interval, semitones (whole: `st1`/`st2`).
    pub semitones: f32,
    /// A fine offset, cents, in Shift and Harmony (`ct1`/`ct2`).
    pub cents: f32,
    /// The Harmony mode's interval, scale degrees (0 unison, ±7 an octave in a
    /// seven-note scale).
    pub degree: i32,
    /// Its level, 0 … 1 (linear).
    pub level: f32,
    /// Its balance, −1 (left) … +1 (right).
    pub pan: f32,
}

/// Saltus's settings, as the DSP wants them. The defaults are the null (see the module
/// docs); the other modes' defaults are musical, since they are heard only when chosen.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SaltusSettings {
    pub mode: Mode,
    pub engine: Engine,
    pub octave_style: OctaveStyle,
    pub harmony_source: HarmonySource,
    pub voices: [VoiceSettings; 2],
    /// The dry signal's level, 0 … 1.
    pub dry: f32,
    /// The key, 0 = C … 11 = B, and the scale (Tonus's thirteen).
    pub key: u8,
    pub scale: Scale,
    /// Keep the formants where they are (true) or let them move with the pitch.
    pub formants: bool,
    /// The formant shift, semitones.
    pub throat: f32,
    /// The whammy's pedal, 0 (heel) … 1 (toe), its two intervals (semitones), and the
    /// glide the pedal is followed with, ms.
    pub pedal: f32,
    pub heel: f32,
    pub toe: f32,
    pub glide_ms: f32,
    /// The whammy played by the notes held on MIDI: while a note is held the pedal goes to
    /// the toe (gliding there), and back to where `pedal` sets it when every note is
    /// released — the momentary pedal.
    pub whammy_notes: bool,
    /// The octaves' levels, 0 … 1: one down, two down, one up.
    pub sub1: f32,
    pub sub2: f32,
    pub up1: f32,
    /// The reference pitch of A4, Hz (where the key's notes are).
    pub a4: f32,
    /// The output gain, dB.
    pub out_db: f32,
}

impl Default for SaltusSettings {
    fn default() -> Self {
        let voice = |degree, level| VoiceSettings {
            semitones: 0.0,
            cents: 0.0,
            degree,
            level,
            pan: 0.0,
        };
        SaltusSettings {
            mode: Mode::Shift,
            engine: Engine::Mono,
            octave_style: OctaveStyle::Analog,
            harmony_source: HarmonySource::Scale,
            voices: [voice(2, 1.0), voice(4, 0.0)],
            dry: 0.0,
            key: 0,
            scale: Scale::Major,
            formants: true,
            throat: 0.0,
            pedal: 0.0,
            heel: 0.0,
            toe: 12.0,
            glide_ms: 40.0,
            whammy_notes: false,
            sub1: 1.0,
            sub2: 0.0,
            up1: 0.0,
            a4: 440.0,
            out_db: 0.0,
        }
    }
}

/// Which engine a setting runs.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Path {
    /// The tracker and the shifters.
    Psola,
    /// The phase vocoder.
    Poly,
    /// The tracker and the divider.
    Divider,
}

/// The engine `s` runs.
pub fn path_of(s: &SaltusSettings) -> Path {
    match s.mode {
        Mode::Octave if s.octave_style == OctaveStyle::Analog => Path::Divider,
        Mode::Octave | Mode::Harmony => Path::Psola,
        Mode::Shift | Mode::Whammy if s.engine == Engine::Poly => Path::Poly,
        Mode::Shift | Mode::Whammy => Path::Psola,
    }
}

/// The shifter's latency at `sample_rate` (as [`crate::psola::Shifter::latency`]).
pub fn psola_latency(sample_rate: f32) -> usize {
    let sr = sample_rate.max(8_000.0);
    ((crate::psola::LATENCY_48K as f32 * sr / 48_000.0).round() as usize).max(1)
}

/// The latency of settings `s` at `sample_rate`, samples: fixed by the mode, the engine and
/// the octave's style.
pub fn latency_for(s: &SaltusSettings, sample_rate: f32) -> usize {
    match path_of(s) {
        Path::Divider => 0,
        Path::Psola => psola_latency(sample_rate),
        Path::Poly => poly::Poly::latency_at(sample_rate),
    }
}

/// The whammy's interval at pedal `g` (0 … 1), semitones.
#[inline]
pub fn whammy_interval(s: &SaltusSettings, g: f32) -> f32 {
    s.heel + (s.toe - s.heel) * g
}

/// A running sum that is the additive identity until a term arrives: −0.0 plus `x` is `x`
/// for every `x`, +0.0 and −0.0 included.
#[derive(Copy, Clone, Debug)]
pub struct Sum {
    l: f32,
    r: f32,
    any: bool,
}

impl Default for Sum {
    fn default() -> Self {
        Sum {
            l: -0.0,
            r: -0.0,
            any: false,
        }
    }
}

impl Sum {
    /// Add `(l, r)` at `level` and balance `pan`; a level of 1 and a pan of 0 add the
    /// samples themselves, a level of 0 adds nothing.
    #[inline]
    pub fn add(&mut self, l: f32, r: f32, level: f32, pan: f32) {
        if level == 0.0 {
            return;
        }
        let (a, b) = if level == 1.0 {
            (l, r)
        } else {
            (l * level, r * level)
        };
        let (a, b) = if pan == 0.0 {
            (a, b)
        } else {
            let (gl, gr) = balance(pan);
            (a * gl, b * gr)
        };
        self.l += a;
        self.r += b;
        self.any = true;
    }

    /// Another sum's terms added to this one.
    #[inline]
    pub fn merge(&mut self, o: Sum) {
        if o.any {
            self.l += o.l;
            self.r += o.r;
            self.any = true;
        }
    }

    /// The sum, +0.0 where nothing was added.
    #[inline]
    pub fn get(&self) -> (f32, f32) {
        if self.any {
            (self.l, self.r)
        } else {
            (0.0, 0.0)
        }
    }
}

/// The balance law: the centre is unity on both sides; a side turns down the other.
#[inline]
pub fn balance(pan: f32) -> (f32, f32) {
    let p = pan.clamp(-1.0, 1.0);
    ((1.0 - p).min(1.0), (1.0 + p).min(1.0))
}

/// The dry line: long enough for the vocoder's latency.
#[derive(Clone, Debug)]
struct DryLine {
    ring: [Vec<f32>; 2],
    zero: [ZeroRun; 2],
    n: u64,
}

impl DryLine {
    fn new(len: usize) -> Self {
        let len = (len + 1).next_power_of_two();
        DryLine {
            ring: [vec![0.0; len], vec![0.0; len]],
            zero: [ZeroRun::FULL; 2],
            n: 0,
        }
    }

    fn reset(&mut self) {
        for r in &mut self.ring {
            r.fill(0.0);
        }
        self.zero = [ZeroRun::FULL; 2];
        self.n = 0;
    }

    fn at_rest(&self) -> bool {
        let len = self.ring[0].len();
        self.zero[0].covers(len) && self.zero[1].covers(len)
    }

    /// Write one sample and read the one `delay` behind it (zeros before the start).
    #[inline]
    fn tick(&mut self, l: f32, r: f32, delay: usize) -> (f32, f32) {
        let mask = self.ring[0].len() - 1;
        let w = self.n as usize & mask;
        self.ring[0][w] = l;
        self.ring[1][w] = r;
        self.zero[0].push(l);
        self.zero[1].push(r);
        self.n += 1;
        let t = self.n as i64 - 1 - delay as i64;
        if t < 0 {
            return (0.0, 0.0);
        }
        let k = t as usize & mask;
        (self.ring[0][k], self.ring[1][k])
    }
}

/// MIDI notes one `process` call queues (Tonus's figure: the exported plug-in's block).
pub const MAX_EVENTS: usize = crate::tonus::MAX_EVENTS;

/// The pedal's glide lands within this (a millionth of its travel).
const PEDAL_SNAP: f32 = 1e-6;

/// The pitch unit. Allocates in `new` only.
#[derive(Clone, Debug)]
pub struct Saltus {
    sample_rate: f32,
    s: SaltusSettings,
    path: Path,
    latency: usize,
    out_gain: f32,
    /// Samples processed and skipped since reset: an engine chosen later is brought here.
    count: u64,
    tracker: PitchTracker,
    voices: Voices,
    poly: poly::Poly,
    divider: octave::Divider,
    dry: DryLine,
    keys: Keys,
    events: [(usize, NoteEvent); MAX_EVENTS],
    n_events: usize,
    /// The whammy's pedal, glided, and its coefficient.
    pedal: f32,
    k_pedal: f32,
    /// The newest tracker frame's level, dBFS.
    level_db: f32,
    /// Whether a sample has been processed since reset: before, the pedal is where it is
    /// set, not gliding there.
    started: bool,
}

impl Saltus {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(8_000.0);
        let poly = poly::Poly::new(sr);
        let dry = DryLine::new(poly.latency().max(psola_latency(sr)));
        let s = SaltusSettings::default();
        let mut u = Saltus {
            sample_rate: sr,
            s,
            path: path_of(&s),
            latency: latency_for(&s, sr),
            out_gain: 1.0,
            count: 0,
            tracker: PitchTracker::new(sr),
            voices: Voices::new(sr),
            poly,
            divider: octave::Divider::new(sr),
            dry,
            keys: Keys::default(),
            events: [(0, NoteEvent::AllOff); MAX_EVENTS],
            n_events: 0,
            pedal: 0.0,
            k_pedal: 1.0,
            level_db: -120.0,
            started: false,
        };
        u.set(&s);
        u
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    pub fn settings(&self) -> &SaltusSettings {
        &self.s
    }

    /// The fixed delay of the current settings, samples ([`latency_for`]).
    pub fn latency(&self) -> usize {
        self.latency
    }

    /// The engine running.
    pub fn path(&self) -> Path {
        self.path
    }

    pub fn set(&mut self, s: &SaltusSettings) {
        let old = self.path;
        self.s = *s;
        self.path = path_of(s);
        self.latency = latency_for(s, self.sample_rate);
        self.out_gain = if s.out_db == 0.0 {
            1.0
        } else {
            math::db_to_gain(s.out_db.clamp(-60.0, 24.0))
        };
        self.k_pedal = if s.glide_ms <= 0.0 {
            1.0
        } else {
            1.0 - math::exp(-1.0 / (s.glide_ms * 0.001 * self.sample_rate))
        };
        if self.path != old {
            self.enter(self.path);
        }
        if !self.started {
            self.pedal = self.pedal_target();
        }
        self.voices.retarget(&self.s, &self.keys);
    }

    /// Bring the engine `p` uses to a new one's state at the current count.
    fn enter(&mut self, p: Path) {
        let n = self.count as usize;
        match p {
            Path::Psola => {
                self.tracker.reset();
                self.tracker.skip(n);
                self.voices.reset();
                self.voices.skip(n);
            }
            Path::Divider => {
                self.tracker.reset();
                self.tracker.skip(n);
                self.divider.reset();
            }
            Path::Poly => {
                self.poly.reset();
                self.poly.skip(n);
            }
        }
        self.level_db = -120.0;
    }

    pub fn reset(&mut self) {
        self.count = 0;
        self.tracker.reset();
        self.voices.reset();
        self.poly.reset();
        self.divider.reset();
        self.dry.reset();
        self.keys.clear();
        self.n_events = 0;
        self.pedal = self.pedal_target();
        self.level_db = -120.0;
        self.started = false;
        self.voices.retarget(&self.s, &self.keys);
    }

    /// Whether the held notes can reach the output (the Harmony mode on MIDI, the whammy
    /// played by notes).
    fn hears_notes(&self) -> bool {
        (self.s.mode == Mode::Harmony && self.s.harmony_source != HarmonySource::Scale)
            || self.whammy_by_notes()
    }

    /// Whether the notes held play the whammy's pedal.
    fn whammy_by_notes(&self) -> bool {
        self.s.mode == Mode::Whammy && self.s.whammy_notes
    }

    /// Where the whammy's pedal is going: the toe while a note plays it, else where it is
    /// set.
    pub fn pedal_target(&self) -> f32 {
        if self.whammy_by_notes() && !self.keys.is_empty() {
            1.0
        } else {
            self.s.pedal.clamp(0.0, 1.0)
        }
    }

    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    pub fn rest_blocker(&self) -> Option<&'static str> {
        if !self.dry.at_rest() {
            return Some("the dry line holds sound");
        }
        if self.n_events > 0 || (self.hears_notes() && !self.keys.is_empty()) {
            return Some("a note is held");
        }
        if self.pedal != self.pedal_target() {
            return Some("the pedal is moving");
        }
        match self.path {
            Path::Psola => {
                if !self.tracker.at_rest() {
                    Some("the tracker hears sound")
                } else {
                    self.voices.rest_blocker()
                }
            }
            Path::Divider => {
                if !self.tracker.at_rest() {
                    Some("the tracker hears sound")
                } else if !self.divider.at_rest() {
                    Some("the octaves are still sounding")
                } else {
                    None
                }
            }
            Path::Poly => (!self.poly.at_rest()).then_some("the vocoder is still sounding"),
        }
    }

    /// `frames` samples of silence went by unprocessed, at rest.
    pub fn skip(&mut self, frames: usize) {
        self.count += frames as u64;
        self.dry.n += frames as u64;
        match self.path {
            Path::Psola => {
                self.tracker.skip(frames);
                self.voices.skip(frames);
            }
            Path::Divider => self.tracker.skip(frames),
            Path::Poly => self.poly.skip(frames),
        }
    }

    /// A note from the slot's MIDI source, `offset` samples into the next `process`; past
    /// the queue's room the queued ones act at once (their timing lost, never a note).
    pub fn note(&mut self, offset: usize, e: NoteEvent) {
        if self.n_events == MAX_EVENTS {
            for i in 0..MAX_EVENTS {
                let queued = self.events[i].1;
                self.apply(queued);
            }
            self.n_events = 0;
        }
        self.events[self.n_events] = (offset, e);
        self.n_events += 1;
    }

    fn apply(&mut self, e: NoteEvent) {
        self.keys.apply(e);
        self.voices.retarget(&self.s, &self.keys);
    }

    /// Process a stereo block in place.
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let mut ev = 0;
        for i in 0..n {
            while ev < self.n_events && self.events[ev].0 <= i {
                let e = self.events[ev].1;
                self.apply(e);
                ev += 1;
            }
            let (a, b) = self.tick(l[i], r[i]);
            l[i] = a;
            r[i] = b;
        }
        while ev < self.n_events {
            let e = self.events[ev].1;
            self.apply(e);
            ev += 1;
        }
        self.n_events = 0;
    }

    /// The whammy's pedal, one sample on.
    #[inline]
    fn glide_pedal(&mut self) {
        let t = self.pedal_target();
        if self.pedal != t {
            let v = self.pedal + (t - self.pedal) * self.k_pedal;
            // Landed within the snap, or stalled (the step under half an ulp).
            self.pedal = if (t - v).abs() < PEDAL_SNAP || v == self.pedal {
                t
            } else {
                v
            };
        }
    }

    #[inline]
    fn tick(&mut self, l: f32, r: f32) -> (f32, f32) {
        self.count += 1;
        self.started = true;
        let (dl, dr) = self.dry.tick(l, r, self.latency);
        self.glide_pedal();
        let mut sum = Sum::default();
        sum.add(dl, dr, self.s.dry, 0.0);
        match self.path {
            Path::Psola => {
                if let Some(f) = self.tracker.push(0.5 * (l + r)) {
                    self.level_db = f.level_db;
                    self.voices.frame(&f, &self.s, &self.keys);
                }
                let whammy = whammy_interval(&self.s, self.pedal);
                sum.merge(self.voices.tick(l, r, &self.s, whammy));
            }
            Path::Divider => {
                if let Some(f) = self.tracker.push(0.5 * (l + r)) {
                    self.level_db = f.level_db;
                    self.divider.guide(&f);
                }
                let o = self.divider.tick(0.5 * (l + r));
                let s = &self.s;
                for (v, level) in [(o.sub1, s.sub1), (o.sub2, s.sub2), (o.up1, s.up1)] {
                    sum.add(v, v, level, 0.0);
                }
            }
            Path::Poly => {
                let whammy = whammy_interval(&self.s, self.pedal);
                let plan = poly::plan(&self.s, whammy);
                sum.merge(self.poly.tick(l, r, &plan));
            }
        }
        let (a, b) = sum.get();
        if self.out_gain == 1.0 {
            (a, b)
        } else {
            (a * self.out_gain, b * self.out_gain)
        }
    }

    // ---- What the editor reads ----------------------------------------------------

    /// The note heard now (MIDI against the reference), when one is.
    pub fn detected(&self) -> Option<f32> {
        match self.path {
            Path::Psola => self.voices.detected(),
            Path::Divider => self.divider.detected(self.s.a4),
            Path::Poly => None,
        }
    }

    /// Voice `i`'s shift now, semitones, while it sounds (in the clean octave, the
    /// octaves: 0 one down, 1 two down, 2 one up).
    pub fn voice_shift(&self, i: usize) -> Option<f32> {
        match self.path {
            Path::Psola => self.voices.shift(i),
            Path::Poly => self.poly.shift(i),
            Path::Divider => None,
        }
    }

    /// The whammy's pedal as it is followed, 0 … 1.
    pub fn pedal_now(&self) -> f32 {
        self.pedal
    }

    /// The input level of the newest tracker frame, dBFS.
    pub fn level_db(&self) -> f32 {
        self.level_db
    }

    /// The divider's octaves' levels now (one down, two down, one up), linear.
    pub fn octave_levels(&self) -> [f32; 3] {
        self.divider.levels()
    }

    /// The pitch classes held on MIDI (bit 0 = C).
    pub fn held_classes(&self) -> u16 {
        self.keys.classes()
    }
}
