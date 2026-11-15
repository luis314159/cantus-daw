//! Saltus's mono voices: the shifters, and what each of them is asked to do — a fixed
//! interval (Shift), the whammy's (Whammy), an octave (the clean Octave), or a number of
//! scale degrees from the note heard (Harmony).
//!
//! **Harmony** keeps the singer's intonation: the note heard is snapped to the scale (with
//! Tonus's hysteresis, [`scale::HYSTERESIS`]) only to *count* the degrees; the voice is moved
//! by the whole number of semitones between that note and the one `degree` steps away, so
//! it follows every bend and vibrato of the line in parallel. A change of interval (the line
//! crossing from a major to a minor third) glides over 12 ms. On MIDI, the degrees count
//! through the chord held (*Chord*), or the voices sing the notes held, carrying the
//! singer's own movement around his centre (*Melody*).

use super::{HarmonySource, Mode, OctaveStyle, SaltusSettings, Sum};
use crate::math;
use crate::pitch::PitchFrame;
use crate::psola::{Control, Shifter};
use crate::tonus::scale::{self, Allowed, NoteState};
use crate::tonus::NoteEvent;

/// The shifters: two voices, or the clean octave's three.
pub const LANES: usize = 3;
/// A harmony interval's change glides this long, seconds.
const HARMONY_GLIDE_S: f32 = 0.012;
/// A voice opening or closing (a chord released, the clean octave's note ending), seconds.
const GATE_S: f32 = 0.005;
/// The centre of the sung pitch (the Melody source carries what moves around it), Hz.
const CENTRE_HZ: f32 = 2.0;
/// Below this level (dBFS) the unvoiced input is silence: no formants to move.
const SILENCE_DB: f32 = -90.0;
/// Glides land within this.
const SNAP: f32 = 1e-6;

/// The notes held on the slot's MIDI source, oldest first.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Keys {
    notes: [u8; 32],
    len: usize,
}

impl Default for Keys {
    fn default() -> Self {
        Keys {
            notes: [0; 32],
            len: 0,
        }
    }
}

impl Keys {
    pub fn apply(&mut self, e: NoteEvent) {
        match e {
            NoteEvent::On(p) => {
                self.off(p);
                if self.len == self.notes.len() {
                    self.notes.copy_within(1.., 0);
                    self.len -= 1;
                }
                self.notes[self.len] = p;
                self.len += 1;
            }
            NoteEvent::Off(p) => self.off(p),
            NoteEvent::AllOff => self.clear(),
        }
    }

    fn off(&mut self, p: u8) {
        if let Some(i) = self.notes[..self.len].iter().position(|&n| n == p) {
            self.notes.copy_within(i + 1..self.len, i);
            self.len -= 1;
        }
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The `i`-th newest note held (0: the newest).
    pub fn newest(&self, i: usize) -> Option<u8> {
        (i < self.len).then(|| self.notes[self.len - 1 - i])
    }

    /// The pitch classes held, as twelve bits (bit 0 = C).
    pub fn classes(&self) -> u16 {
        self.notes[..self.len]
            .iter()
            .fold(0u16, |m, &n| m | (1 << (n % 12)))
    }
}

/// The note `degree` steps of a scale away from `n` (a note of it): the scale is `degrees`
/// (semitones above `key`, ascending, within the octave).
pub fn degree_step(n: i32, degree: i32, key: i32, degrees: &[u8]) -> i32 {
    let m = degrees.len() as i32;
    if m == 0 {
        return n;
    }
    let rel = n - key;
    let (oct, pc) = (rel.div_euclid(12), rel.rem_euclid(12));
    // The degree at or below the note (it is one of the scale's when chosen by `choose`).
    let i = degrees.iter().rposition(|&d| d as i32 <= pc).unwrap_or(0) as i32;
    let below = degrees[i as usize] as i32;
    let a = oct * m + i + degree;
    key + 12 * a.div_euclid(m) + degrees[a.rem_euclid(m) as usize] as i32 + (pc - below)
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct Lane {
    /// The interval wanted and the one applied, semitones.
    target: f32,
    shift: f32,
    /// Whether the voice should sound, and how much it does (0 … 1, glided).
    open: bool,
    gate: f32,
}

/// The mono voices. Allocates in `new` only.
#[derive(Clone, Debug)]
pub struct Voices {
    shifters: [Shifter; LANES],
    voiced: bool,
    p: f32,
    p_c: f32,
    period: f32,
    level_db: f32,
    /// The note heard, snapped to the scale or the chord (Harmony).
    note: Option<i32>,
    lanes: [Lane; LANES],
    k_centre: f32,
    k_harmony: f32,
    k_gate: f32,
    /// Input samples since reset (skips included), the shifters' latency, and the
    /// period's peak: the newest committed and the one being looked for.
    clock: u64,
    latency: u64,
    peak: Option<u64>,
    window: u64,
    cand: (u64, f32),
}

impl Voices {
    pub fn new(sample_rate: f32) -> Self {
        let shifter = Shifter::new(sample_rate);
        let sr = shifter.sample_rate();
        let hop = crate::pitch::Design::new(sr).hop as f32;
        Voices {
            shifters: [shifter.clone(), shifter.clone(), shifter],
            voiced: false,
            p: 0.0,
            p_c: 0.0,
            period: 0.0,
            level_db: -120.0,
            note: None,
            lanes: [Lane::default(); LANES],
            k_centre: 1.0 - math::exp(-2.0 * core::f32::consts::PI * CENTRE_HZ * hop / sr),
            k_harmony: 1.0 - math::exp(-1.0 / (HARMONY_GLIDE_S * sr)),
            k_gate: 1.0 / (GATE_S * sr),
            clock: 0,
            latency: super::psola_latency(sr) as u64,
            peak: None,
            window: 0,
            cand: (0, 0.0),
        }
    }

    pub fn reset(&mut self) {
        for s in &mut self.shifters {
            s.reset();
        }
        self.clock = 0;
        self.unvoice();
        self.level_db = -120.0;
        for l in &mut self.lanes {
            *l = Lane::default();
        }
    }

    /// After [`Self::retarget`] on a reset unit: each gate where it is going.
    fn snap_gates(&mut self) {
        for l in &mut self.lanes {
            l.gate = if l.open { 1.0 } else { 0.0 };
        }
    }

    pub fn skip(&mut self, frames: usize) {
        for s in &mut self.shifters {
            s.skip(frames);
        }
        self.clock += frames as u64;
    }

    /// The input's peak in each period while voiced; whether the shifters' output time
    /// (`latency` behind the newest sample) falls on the lattice of those peaks.
    #[inline]
    fn mark(&mut self, m: f32) -> bool {
        let now = self.clock;
        self.clock += 1;
        if !self.voiced || self.period <= 1.0 {
            return false;
        }
        let a = m.abs();
        if a > self.cand.1 {
            self.cand = (now, a);
        }
        if (now - self.window) as f32 >= self.period {
            self.peak = Some(self.cand.0);
            self.window = now;
            self.cand = (now, 0.0);
        }
        let Some(p) = self.peak else {
            return false;
        };
        let t = now as f64 - self.latency as f64;
        (t - p as f64).rem_euclid(self.period as f64) < 1.0
    }

    pub fn rest_blocker(&self) -> Option<&'static str> {
        if self.shifters.iter().any(|s| !s.at_rest()) {
            Some("a shifter is still sounding")
        } else if self.voiced {
            Some("a note is being moved")
        } else if self
            .lanes
            .iter()
            .any(|l| l.shift != l.target || l.gate != if l.open { 1.0 } else { 0.0 })
        {
            Some("a voice is gliding")
        } else {
            None
        }
    }

    fn unvoice(&mut self) {
        self.voiced = false;
        self.p = 0.0;
        self.p_c = 0.0;
        self.period = 0.0;
        self.note = None;
        self.peak = None;
        self.window = self.clock;
        self.cand = (self.clock, 0.0);
        // Harmony's intervals belong to the note: none without one.
        for l in &mut self.lanes {
            l.target = 0.0;
            l.shift = 0.0;
        }
    }

    /// A frame from the tracker.
    pub fn frame(&mut self, f: &PitchFrame, s: &SaltusSettings, keys: &Keys) {
        self.level_db = f.level_db;
        if !f.voiced || f.f0 <= 0.0 {
            if self.voiced {
                self.unvoice();
            }
            self.retarget(s, keys);
            return;
        }
        let p = 69.0 + 12.0 * math::log2(f.f0 / s.a4.max(1.0));
        let onset = !self.voiced;
        if onset {
            self.voiced = true;
            self.p_c = p;
        } else {
            self.p_c += (p - self.p_c) * self.k_centre;
        }
        self.p = p;
        self.period = f.period;
        self.retarget(s, keys);
        if onset {
            for l in &mut self.lanes {
                l.shift = l.target;
            }
        }
    }

    /// Which voices sound and, in Harmony, the intervals: after a frame, a note on MIDI or
    /// a change of settings.
    pub fn retarget(&mut self, s: &SaltusSettings, keys: &Keys) {
        let fresh = self.shifters[0].at_rest() && !self.voiced;
        for (v, l) in self.lanes.iter_mut().enumerate() {
            l.open = match s.mode {
                Mode::Harmony if v < 2 => match s.harmony_source {
                    HarmonySource::Scale => true,
                    HarmonySource::Chord => !keys.is_empty(),
                    HarmonySource::Melody => keys.newest(v).is_some(),
                },
                Mode::Octave if s.octave_style == OctaveStyle::Clean => self.voiced,
                Mode::Harmony | Mode::Shift | Mode::Whammy => v < 2,
                Mode::Octave => false,
            };
        }
        if fresh {
            self.snap_gates();
        }
        if s.mode != Mode::Harmony || !self.voiced {
            return;
        }
        let cents = |v: usize| s.voices[v].cents * 0.01;
        match s.harmony_source {
            HarmonySource::Scale => {
                let allowed = scale::resolve(s.scale, s.key, &[NoteState::Scale; 12]);
                self.note = scale::choose(self.p, &allowed, self.note).map(|x| x.0);
                if let Some(n) = self.note {
                    let degrees = s.scale.degrees();
                    for v in 0..2 {
                        let t = degree_step(n, s.voices[v].degree, s.key as i32 % 12, degrees);
                        self.lanes[v].target = (t - n) as f32 + cents(v);
                    }
                }
            }
            HarmonySource::Chord => {
                let bits = keys.classes();
                let mut allowed = [Allowed::Removed; 12];
                let mut degrees = [0u8; 12];
                let mut m = 0;
                for (pc, a) in allowed.iter_mut().enumerate() {
                    if bits & (1 << pc) != 0 {
                        *a = Allowed::Target;
                        degrees[m] = pc as u8;
                        m += 1;
                    }
                }
                self.note = if m == 0 {
                    None
                } else {
                    scale::choose(self.p, &allowed, self.note).map(|x| x.0)
                };
                for v in 0..2 {
                    self.lanes[v].target = match self.note {
                        Some(n) => {
                            (degree_step(n, s.voices[v].degree, 0, &degrees[..m]) - n) as f32
                                + cents(v)
                        }
                        None => self.lanes[v].target,
                    };
                }
            }
            HarmonySource::Melody => {
                for v in 0..2 {
                    if let Some(k) = keys.newest(v) {
                        self.lanes[v].target = k as f32 - self.p_c + cents(v);
                    }
                }
            }
        }
    }

    /// The lane's level and balance for settings `s`.
    fn mix(s: &SaltusSettings, k: usize) -> (f32, f32) {
        match s.mode {
            Mode::Octave => ([s.sub1, s.sub2, s.up1][k], 0.0),
            Mode::Whammy if k > 0 => (0.0, 0.0),
            _ if k < 2 => (s.voices[k].level, s.voices[k].pan),
            _ => (0.0, 0.0),
        }
    }

    /// One stereo sample through every shifter; the voices' sum (the dry is not in it).
    #[inline]
    pub fn tick(&mut self, l: f32, r: f32, s: &SaltusSettings, whammy: f32) -> Sum {
        let throat = if s.throat == 0.0 {
            1.0
        } else {
            math::exp2(s.throat / 12.0)
        };
        let gated = matches!(s.mode, Mode::Harmony | Mode::Octave);
        let on_mark = self.mark(0.5 * (l + r));
        let mut sum = Sum::default();
        for k in 0..LANES {
            let (level, pan) = Self::mix(s, k);
            let lane = &mut self.lanes[k];
            // The interval: fixed by the settings, or Harmony's, glided.
            match s.mode {
                Mode::Shift if k < 2 => {
                    lane.target = s.voices[k].semitones + s.voices[k].cents * 0.01;
                    lane.shift = lane.target;
                }
                Mode::Whammy if k == 0 => {
                    lane.target = whammy;
                    lane.shift = whammy;
                }
                Mode::Octave => {
                    lane.target = [-12.0, -24.0, 12.0][k];
                    lane.shift = lane.target;
                }
                Mode::Harmony if lane.shift != lane.target => {
                    let v = lane.shift + (lane.target - lane.shift) * self.k_harmony;
                    lane.shift = if (lane.target - v).abs() < SNAP || v == lane.shift {
                        lane.target
                    } else {
                        v
                    };
                }
                _ => {}
            }
            if gated {
                let t = if lane.open { 1.0 } else { 0.0 };
                if lane.gate < t {
                    lane.gate = (lane.gate + self.k_gate).min(1.0);
                } else if lane.gate > t {
                    lane.gate = (lane.gate - self.k_gate).max(0.0);
                }
            } else {
                lane.gate = if lane.open { 1.0 } else { 0.0 };
            }
            let gain = if lane.gate == 1.0 {
                level
            } else {
                level * lane.gate
            };
            let shift = lane.shift;
            let ratio = if shift == 0.0 {
                1.0
            } else {
                math::exp2(shift / 12.0)
            };
            let formant = if s.formants { throat } else { throat * ratio };
            let sounds = gain > 0.0;
            let mut engage = sounds
                && if self.voiced {
                    shift != 0.0 || formant != 1.0
                } else {
                    formant != 1.0 && self.level_db > SILENCE_DB
                };
            // A voiced shift starts on the period's peak: the grains' lattice is anchored
            // where the first one falls, and the formants are kept best with the grains
            // centred on the pulses (1 dB of envelope error there, 6 dB half a period off).
            if engage && self.voiced && self.shifters[k].mix() == 0.0 {
                engage = on_mark;
            }
            let c = Control {
                period: if self.voiced { self.period } else { 0.0 },
                ratio,
                formant,
                engage,
            };
            let (a, b) = self.shifters[k].tick(l, r, &c);
            sum.add(a, b, gain, pan);
        }
        sum
    }

    /// The note heard now, MIDI.
    pub fn detected(&self) -> Option<f32> {
        self.voiced.then_some(self.p)
    }

    /// Lane `i`'s interval while it sounds.
    pub fn shift(&self, i: usize) -> Option<f32> {
        self.lanes
            .get(i)
            .filter(|l| l.gate > 0.0 && self.voiced)
            .map(|l| l.shift)
    }
}
