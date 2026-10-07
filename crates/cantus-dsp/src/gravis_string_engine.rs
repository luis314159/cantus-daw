//! The strings of one bass together: fingering (which string plays a note), techniques,
//! hammer-ons, slides, the pickups' coils and the piezo. A child of `gravis_string.rs`.

use super::{fret_mm, Fingering, Guide, StringSettings, Technique, MAX_STRINGS};
use crate::gravis::voice::Rng;
use crate::math;

/// A second-order low-pass in `f32` (a pickup's coil and its tone control).
#[derive(Copy, Clone, Debug, Default)]
struct Coil {
    s1: f32,
    s2: f32,
}

impl Coil {
    #[inline]
    fn process(&mut self, x: f32, g: f32, k: f32) -> f32 {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let v3 = x - self.s2;
        let v1 = a1 * self.s1 + a2 * v3;
        let v2 = self.s2 + a2 * self.s1 + a3 * v3;
        self.s1 = 2.0 * v1 - self.s1;
        self.s2 = 2.0 * v2 - self.s2;
        if self.s1.abs() < 1e-20 {
            self.s1 = 0.0;
        }
        if self.s2.abs() < 1e-20 {
            self.s2 = 0.0;
        }
        v2
    }
}

/// Where a note is played, for the editor: which string, which fret.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Played {
    pub string: usize,
    pub fret: f32,
}

/// The strings of one bass, their pickups and the fretting hand.
#[derive(Debug)]
pub struct StringEngine {
    g: Vec<Guide>,
    coil: [Coil; 2],
    piezo_hp: (f32, f32),
    /// The index finger's fret.
    pub hand: f32,
    /// The string a mono voice is sounding.
    pub current: Option<usize>,
    /// The fret a fretted slide last crossed.
    slide_fret: i32,
    sr: f32,
}

impl StringEngine {
    pub fn new(sr: f32) -> Self {
        // Room for a loop down to 12 Hz.
        let need = (sr as f64 / 12.0 / 2.0) as usize + 32;
        let len = need.next_power_of_two();
        StringEngine {
            g: (0..MAX_STRINGS).map(|_| Guide::new(len)).collect(),
            coil: [Coil::default(); 2],
            piezo_hp: (0.0, 0.0),
            hand: 1.0,
            current: None,
            slide_fret: 0,
            sr,
        }
    }

    pub fn reset(&mut self) {
        for g in &mut self.g {
            g.clear();
        }
        self.coil = [Coil::default(); 2];
        self.piezo_hp = (0.0, 0.0);
        self.current = None;
        self.hand = 1.0;
    }

    /// Zero the pickups' filters (their last residue, once every string is still).
    pub fn zero_filters(&mut self) {
        self.coil = [Coil::default(); 2];
        self.piezo_hp = (0.0, 0.0);
    }

    /// Strings sounding.
    pub fn active(&self) -> usize {
        self.g.iter().filter(|g| g.active).count()
    }

    /// Whether nothing rings and the pickups' filters are empty.
    pub fn is_silent(&self) -> bool {
        self.g.iter().all(|g| !g.active)
            && self.coil.iter().all(|c| c.s1 == 0.0 && c.s2 == 0.0)
            && self.piezo_hp == (0.0, 0.0)
    }

    /// The fundamental of string `i` now, in hertz.
    pub fn f0(&self, i: usize) -> f64 {
        self.g[i].f0
    }

    /// What string `i` plays: its fret (fractional while sliding).
    pub fn played(&self, i: usize) -> Played {
        Played {
            string: i,
            fret: (self.g[i].note - self.g[i].open as f64) as f32,
        }
    }

    /// The vibrating length of string `i`, mm.
    pub fn vibrating_mm(&self, i: usize) -> f64 {
        self.g[i].vib_mm
    }

    pub fn note_of(&self, i: usize) -> f64 {
        self.g[i].note
    }

    pub fn is_held(&self, i: usize) -> bool {
        self.g[i].held
    }

    pub fn is_active(&self, i: usize) -> bool {
        self.g[i].active
    }

    /// The string for `note`. `prefer`: stay on this string if it reaches (legato).
    pub fn choose(&self, s: &StringSettings, note: f64, prefer: Option<usize>) -> usize {
        let n = s.strings.clamp(4, MAX_STRINGS);
        let max = if s.fretless { 24.0 } else { s.frets as f64 };
        let reach = |i: usize| {
            let f = note - s.open[i] as f64;
            (-0.01..=max + 0.01).contains(&f)
        };
        if let Some(p) = prefer.filter(|p| *p < n && reach(*p)) {
            return p;
        }
        let fret = |i: usize| note - s.open[i] as f64;
        let candidates = (0..n).filter(|i| reach(*i));
        let pick = match s.fingering {
            Fingering::Lowest => candidates.min_by(|a, b| fret(*a).total_cmp(&fret(*b))),
            Fingering::LowStrings => candidates.min(),
            Fingering::Position => {
                let hand = self.hand as f64;
                let cost = |i: usize| {
                    let f = fret(i);
                    if f < 0.5 {
                        0.0
                    } else if f < hand - 0.01 {
                        hand - f
                    } else if f > hand + 3.01 {
                        f - hand - 3.0
                    } else {
                        0.0
                    }
                };
                candidates.min_by(|a, b| {
                    cost(*a)
                        .total_cmp(&cost(*b))
                        .then(fret(*a).total_cmp(&fret(*b)))
                })
            }
        };
        pick.unwrap_or(if note < s.open[0] as f64 { 0 } else { n - 1 })
    }

    fn move_hand(&mut self, fret: f64) {
        if fret < 0.5 {
            return;
        }
        let f = fret as f32;
        if f < self.hand {
            self.hand = f;
        } else if f > self.hand + 3.0 {
            self.hand = f - 3.0;
        }
    }

    /// Pluck string `i` at `note`: the technique's excitation added to what rings.
    /// `soft`: a legato note on another string (a tap, not a full pluck).
    #[allow(clippy::too_many_arguments)]
    pub fn pluck(
        &mut self,
        s: &StringSettings,
        i: usize,
        note: f64,
        vel: f32,
        seed: u64,
        a4: f64,
        soft: bool,
    ) {
        let sr = self.sr as f64;
        let g = &mut self.g[i];
        if g.active {
            // The finger stops the string before plucking it again.
            g.damp(0.25);
        }
        g.open = s.open[i];
        g.held = true;
        g.released = false;
        g.tune(s, note, sr, a4, 0);
        let mut rng = Rng(seed);
        let hum = s.humanize;
        let jitter = 1.0 + 0.03 * hum as f64 * rng.bipolar() as f64;
        let level = if soft { 0.45 } else { 1.0 };
        let amp = vel * math::powf(10f32, hum * rng.bipolar() / 20.0) * level;
        let per_mm = g.half / g.vib_mm;
        let x = g.pluck_x * jitter;
        let hard = 1.3 - s.hardness as f64;
        match s.technique {
            Technique::Finger | Technique::PalmMute => {
                g.vfac = 0.35;
                g.level = 1.0;
                g.pluck(x, amp, 14.0 * hard * per_mm);
            }
            Technique::Pick => {
                g.vfac = 0.5;
                g.level = 1.0;
                g.pluck(x, 0.8 * amp, 3.0 * hard * per_mm);
            }
            Technique::Slap if vel >= s.pop_vel => {
                g.vfac = 1.0;
                g.level = super::POP_LEVEL;
                g.pluck(x, 3.5 * amp, 3.0 * per_mm);
            }
            Technique::Slap => {
                g.vfac = 1.0;
                g.level = super::THUMB_LEVEL;
                g.strike(x, 2.5 * amp, 12.0 * per_mm);
            }
        }
        let fret = note - s.open[i] as f64;
        self.slide_fret = fret.round() as i32;
        self.move_hand(fret);
    }

    /// Change the pitch of string `i` (a slide, a bend) over `ramp` samples.
    pub fn set_pitch(&mut self, s: &StringSettings, i: usize, note: f64, a4: f64, ramp: u32) {
        let sr = self.sr as f64;
        let g = &mut self.g[i];
        if !g.active {
            g.note = note;
            return;
        }
        g.tune(s, note, sr, a4, ramp);
    }

    /// Follow a slide: fret by fret on a fretted neck (a click at each fret the noise
    /// control allows), continuously on a fretless one.
    pub fn slide_to(&mut self, s: &StringSettings, i: usize, note: f64, a4: f64, ramp: u32) {
        let fret = note - s.open[i] as f64;
        let target = if s.fretless {
            note
        } else {
            s.open[i] as f64 + fret.round()
        };
        let f = fret.round() as i32;
        if !s.fretless && f != self.slide_fret {
            self.slide_fret = f;
            let g = &mut self.g[i];
            if g.active && s.noise > 0.0 {
                let x = g.half - 3.0;
                g.strike(x, 0.02 * s.noise, 2.0);
            }
        }
        if (self.g[i].note - target).abs() > 1e-9 {
            self.set_pitch(s, i, target, a4, ramp);
        }
        self.move_hand(fret);
    }

    /// A hammer-on or pull-off: the new length at once, and the finger's own small
    /// excitation (the hammer's strike at the fret; the pull-off's little pluck).
    pub fn hammer(&mut self, s: &StringSettings, i: usize, note: f64, a4: f64) {
        let up = note > self.g[i].note;
        self.set_pitch(s, i, note, a4, 0);
        let g = &mut self.g[i];
        if g.active {
            if up {
                let x = g.half - 3.0;
                g.strike(x, 0.15, 3.0);
            } else {
                let x = (g.half * 0.9).max(2.0);
                g.pluck(x, 0.25, 4.0);
            }
        }
        let fret = note - s.open[i] as f64;
        self.slide_fret = fret.round() as i32;
        self.move_hand(fret);
    }

    /// Let go of string `i`: the fingers damp it.
    pub fn release(&mut self, s: &StringSettings, i: usize, a4: f64) {
        let sr = self.sr as f64;
        let g = &mut self.g[i];
        g.held = false;
        if g.active && !g.released {
            g.released = true;
            let note = g.note;
            g.tune(s, note, sr, a4, 0);
        }
    }

    /// Back to held (a legato note arrived inside the window).
    pub fn resume(&mut self, s: &StringSettings, i: usize, a4: f64) {
        let sr = self.sr as f64;
        let g = &mut self.g[i];
        g.held = true;
        if g.released {
            g.released = false;
            let note = g.note;
            g.tune(s, note, sr, a4, 0);
        }
    }

    /// Settings changed: retune every sounding string in place.
    pub fn retune_all(&mut self, s: &StringSettings, a4: f64) {
        let sr = self.sr as f64;
        for g in &mut self.g {
            if g.active {
                let note = g.note;
                g.tune(s, note, sr, a4, 0);
            }
        }
    }

    /// A note in Strings mode: the fingering picks a string (a free one if it can), which
    /// is plucked; what that string was playing stops.
    #[allow(clippy::too_many_arguments)]
    pub fn poly_on(
        &mut self,
        s: &StringSettings,
        key: u8,
        id: u64,
        note: f64,
        vel: f32,
        seed: u64,
        a4: f64,
    ) -> usize {
        let n = s.strings.clamp(4, MAX_STRINGS);
        let mut i = self.choose(s, note, None);
        if self.g[i].held {
            let max = if s.fretless { 24.0 } else { s.frets as f64 };
            if let Some(j) = (0..n).find(|j| {
                let f = note - s.open[*j] as f64;
                !self.g[*j].held && (-0.01..=max + 0.01).contains(&f)
            }) {
                i = j;
            }
        }
        self.pluck(s, i, note, vel, seed, a4, false);
        self.g[i].key = key;
        self.g[i].id = id;
        i
    }

    /// Release the string sounding this note (Strings mode). Returns it.
    pub fn poly_off(&mut self, s: &StringSettings, key: u8, id: u64, a4: f64) -> Option<usize> {
        let i = (0..MAX_STRINGS).find(|i| {
            let g = &self.g[*i];
            g.held && g.key == key && g.id == id
        })?;
        self.release(s, i, a4);
        Some(i)
    }

    /// Release every held string.
    pub fn release_all(&mut self, s: &StringSettings, a4: f64) {
        for i in 0..MAX_STRINGS {
            if self.g[i].held {
                self.release(s, i, a4);
            }
        }
    }

    /// One sample of the bass's output.
    #[inline]
    pub fn next(&mut self, s: &StringSettings) -> f32 {
        let mut sum = [0.0f32; 3];
        let buzz = if s.fretless { 0.0 } else { s.buzz };
        let h = s.pu_height_mm.max(0.5);
        for g in &mut self.g {
            if g.active {
                let o = g.next(buzz, h);
                sum[0] += o[0];
                sum[1] += o[1];
                sum[2] += o[2];
            }
        }
        // The coils: a resonant low-pass each, loaded by the passive tone control.
        let (fc, q) = if s.humbucker {
            (3_200.0, 1.3)
        } else {
            (6_500.0, 1.0)
        };
        let tone = s.tone.clamp(0.0, 1.0);
        let fc = (fc * (0.12 + 0.88 * tone * tone)).min(0.45 * self.sr);
        let g = math::tan(std::f32::consts::PI * fc / self.sr);
        let k = 1.0 / (q * (0.5 + 0.5 * tone));
        let mut out = 0.0;
        for ((coil, x), level) in self.coil.iter_mut().zip(sum).zip(s.pu_level) {
            out += coil.process(x, g, k) * level;
        }
        if s.piezo > 0.0 || self.piezo_hp != (0.0, 0.0) {
            // A piezo under the saddle: the force at the bridge, bright and uncombed.
            let x = sum[2];
            let y = 0.99 * (self.piezo_hp.1 + x - self.piezo_hp.0);
            let y = if y.abs() < 1e-20 { 0.0 } else { y };
            self.piezo_hp = (if x.abs() < 1e-20 { 0.0 } else { x }, y);
            out += y * s.piezo * 0.5;
        }
        out
    }

    /// Where fret `k` sits, mm from the bridge, for the editor.
    pub fn fret_position(s: &StringSettings, k: f64) -> f64 {
        fret_mm(s.scale_mm as f64, k)
    }
}
