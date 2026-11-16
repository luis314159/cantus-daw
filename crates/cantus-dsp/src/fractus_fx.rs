//! Fractus's processors, in series after the read: pitch jump, crush, filter sweep, gate,
//! delay throw. Each is faded in after its event's line and out before the next, over the
//! join; a processor's state starts again at its event's first sample (so it hears only
//! its event), except the pitch jump's window and the throw's echoes.

use super::read::rc;
use super::seq::Grid;
use super::{Ev, FractusSettings, Module};
use crate::math;
use crate::modfx_line::{ModLine, OnePole};

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PitchSettings {
    /// The jump, semitones (−24 … 24).
    pub semis: f32,
    /// The shifter's window, ms (20 … 100): longer is smoother, shorter tighter.
    pub window_ms: f32,
    /// Clean (two heads, one sounding at a time, a short splice) instead of the classic
    /// four-head warble (the default: the glitch pitch, every tap always sounding).
    pub clean: bool,
}

impl Default for PitchSettings {
    fn default() -> Self {
        PitchSettings {
            semis: 12.0,
            window_ms: 50.0,
            clean: false,
        }
    }
}

/// The clean style's splice, a share of its window: each head sounds alone for the rest.
const CLEAN_SPLICE: f64 = 0.2;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CrushSettings {
    /// Bits (1 … 24; 24 leaves the levels alone).
    pub bits: f32,
    /// The rate the signal is held at, Hz (100 … the sample rate: no hold).
    pub rate_hz: f32,
}

impl Default for CrushSettings {
    fn default() -> Self {
        CrushSettings {
            bits: 6.0,
            rate_hz: 6_000.0,
        }
    }
}

#[allow(clippy::enum_variant_names)] // the filters' own names
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum FilterKind {
    #[default]
    LowPass,
    HighPass,
    BandPass,
}

impl FilterKind {
    pub fn from_index(v: f32) -> FilterKind {
        match v.round() as i32 {
            1 => FilterKind::HighPass,
            2 => FilterKind::BandPass,
            _ => FilterKind::LowPass,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FilterSettings {
    pub kind: FilterKind,
    /// The cut-off at the event's start and end, Hz.
    pub from_hz: f32,
    pub to_hz: f32,
    /// 0 … 1.
    pub res: f32,
    /// −1 … 1: how the sweep bends (0: even in octaves).
    pub curve: f32,
}

impl Default for FilterSettings {
    fn default() -> Self {
        FilterSettings {
            kind: FilterKind::LowPass,
            from_hz: 16_000.0,
            to_hz: 300.0,
            res: 0.4,
            curve: 0.0,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct GateSettings {
    /// The gate's pulse, quarter notes; 0: the whole event shut.
    pub rate: f64,
    /// How much of each pulse is open (0.05 … 1).
    pub duty: f32,
    /// How far it shuts (0 … 1: silence).
    pub depth: f32,
}

impl Default for GateSettings {
    fn default() -> Self {
        GateSettings {
            rate: 0.125,
            duty: 0.5,
            depth: 1.0,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ThrowSettings {
    /// The echo's time, quarter notes (≤ 2).
    pub time: f64,
    /// 0 … 0.95.
    pub feedback: f32,
    /// The echoes' low-pass, Hz.
    pub tone_hz: f32,
    /// Echoes from side to side.
    pub pingpong: bool,
    /// The echoes' level, 0 … 1.
    pub level: f32,
}

impl Default for ThrowSettings {
    fn default() -> Self {
        ThrowSettings {
            time: 0.75,
            feedback: 0.45,
            tone_hz: 5_000.0,
            pingpong: true,
            level: 0.8,
        }
    }
}

/// The weight of an event's processor at `so`: its amount, in after its line and out
/// before its end, over `xf` (no more than half the event).
#[inline]
pub fn env(e: &Ev, so: i64, xf: u32) -> f32 {
    let t = so - e.start;
    let x = if e.end == i64::MAX {
        i64::from(xf)
    } else {
        i64::from(xf).min((e.end - e.start) / 2).max(1)
    };
    let xi = (x + 1) as f64;
    let a = if t < x { rc((t + 1) as f64 / xi) } else { 1.0 };
    let to_end = e.end.saturating_sub(so);
    let b = if to_end <= x {
        rc(to_end as f64 / xi)
    } else {
        1.0
    };
    e.amount * a.min(b)
}

/// The longest throw, seconds (a 1/2 note at 60 BPM).
pub const THROW_S: f64 = 2.05;
/// The longest pitch window, ms.
const PITCH_MAX_MS: f32 = 100.0;

#[derive(Clone, Debug)]
pub struct Chain {
    sr: f32,
    pitch: PitchSettings,
    crush: CrushSettings,
    filter: FilterSettings,
    gate: GateSettings,
    throw: ThrowSettings,
    pl: ModLine,
    pr: ModLine,
    /// The crush's held samples, its hold count and its event.
    held: [f32; 2],
    hold_n: i64,
    crush_ev: i64,
    /// The filter's state and its event.
    svf: [[f32; 2]; 2],
    filt_ev: i64,
    tl: ModLine,
    tr: ModLine,
    lp: [OnePole; 2],
    lp_g: f32,
}

impl Chain {
    pub fn new(sr: f32) -> Chain {
        let pmax = (PITCH_MAX_MS * 0.001 * sr) as usize + 8;
        let tmax = (THROW_S * f64::from(sr)) as usize + 8;
        Chain {
            sr,
            pitch: PitchSettings::default(),
            crush: CrushSettings::default(),
            filter: FilterSettings::default(),
            gate: GateSettings::default(),
            throw: ThrowSettings::default(),
            pl: ModLine::new(pmax),
            pr: ModLine::new(pmax),
            held: [0.0; 2],
            hold_n: -1,
            crush_ev: i64::MIN,
            svf: [[0.0; 2]; 2],
            filt_ev: i64::MIN,
            tl: ModLine::new(tmax),
            tr: ModLine::new(tmax),
            lp: [OnePole::default(); 2],
            lp_g: 1.0,
        }
    }

    pub fn memory_bytes(&self) -> usize {
        let line = |l: &ModLine| (l.max_delay() as usize + 4).next_power_of_two() * 4;
        line(&self.pl) + line(&self.pr) + line(&self.tl) + line(&self.tr)
    }

    pub fn set(&mut self, s: &FractusSettings, sr: f32) {
        self.sr = sr;
        self.pitch = PitchSettings {
            semis: s.pitch.semis.clamp(-24.0, 24.0),
            window_ms: s.pitch.window_ms.clamp(20.0, PITCH_MAX_MS),
            clean: s.pitch.clean,
        };
        self.crush = CrushSettings {
            bits: s.crush.bits.clamp(1.0, 24.0),
            rate_hz: s.crush.rate_hz.clamp(100.0, sr),
        };
        self.filter = FilterSettings {
            from_hz: s.filter.from_hz.clamp(20.0, 20_000.0),
            to_hz: s.filter.to_hz.clamp(20.0, 20_000.0),
            res: s.filter.res.clamp(0.0, 1.0),
            curve: s.filter.curve.clamp(-1.0, 1.0),
            ..s.filter
        };
        self.gate = GateSettings {
            rate: s.gate.rate.clamp(0.0, 4.0),
            duty: s.gate.duty.clamp(0.05, 1.0),
            depth: s.gate.depth.clamp(0.0, 1.0),
        };
        self.throw = ThrowSettings {
            time: s.throw.time.clamp(1.0 / 64.0, 2.0),
            feedback: s.throw.feedback.clamp(0.0, 0.95),
            tone_hz: s.throw.tone_hz.clamp(200.0, 20_000.0),
            level: s.throw.level.clamp(0.0, 1.0),
            ..s.throw
        };
        self.lp_g = OnePole::coef(self.throw.tone_hz, sr);
    }

    /// A seek: the processors that hear only their event start again.
    pub fn restart(&mut self) {
        self.held = [0.0; 2];
        self.hold_n = -1;
        self.crush_ev = i64::MIN;
        self.svf = [[0.0; 2]; 2];
        self.filt_ev = i64::MIN;
    }

    pub fn reset(&mut self) {
        self.restart();
        self.pl.reset();
        self.pr.reset();
        self.tl.reset();
        self.tr.reset();
        self.lp = [OnePole::default(); 2];
    }

    /// Nothing here could make a sound from silence.
    pub fn silent(&self) -> bool {
        self.pl.is_silent()
            && self.pr.is_silent()
            && self.tl.is_silent()
            && self.tr.is_silent()
            && self.lp.iter().all(OnePole::is_zero)
            && self.held.iter().all(|h| h.to_bits() == 0)
            && self.svf.iter().flatten().all(|v| v.to_bits() == 0)
    }

    /// One sample through the processors; `evs` are the lanes' events at `so`.
    #[inline]
    pub fn run(&mut self, x: [f32; 2], evs: &[Option<Ev>], so: i64, g: &Grid, xf: u32) -> [f32; 2] {
        let mut y = x;
        // Pitch: its window hears everything, so a jump has its past.
        self.pl.push(y[0]);
        self.pr.push(y[1]);
        if let Some(e) = evs[Module::Pitch.lane()] {
            if self.pitch.semis != 0.0 {
                let w = env(&e, so, xf);
                let p = self.shift(so - e.start);
                y = mix(y, p, w);
            }
        }
        if let Some(e) = evs[Module::Crush.lane()] {
            let w = env(&e, so, xf);
            let c = self.crush(&e, so, y);
            y = mix(y, c, w);
        } else if self.hold_n >= 0 {
            self.held = [0.0; 2];
            self.hold_n = -1;
            self.crush_ev = i64::MIN;
        }
        if let Some(e) = evs[Module::Filter.lane()] {
            let w = env(&e, so, xf);
            let f = self.filter(&e, so, y, g);
            y = mix(y, f, w);
        } else if self.filt_ev != i64::MIN {
            self.svf = [[0.0; 2]; 2];
            self.filt_ev = i64::MIN;
        }
        if let Some(e) = evs[Module::Gate.lane()] {
            let gain = 1.0 - self.gate.depth * env(&e, so, xf) * self.gate_shut(&e, so, g, xf);
            y = [y[0] * gain, y[1] * gain];
        }
        let send = match evs[Module::Throw.lane()] {
            Some(e) => {
                let w = env(&e, so, xf);
                [y[0] * w, y[1] * w]
            }
            None => [0.0; 2],
        };
        let echo = self.throw(send, g);
        if echo[0].to_bits() == 0 && echo[1].to_bits() == 0 {
            y
        } else {
            [
                y[0] + self.throw.level * echo[0],
                y[1] + self.throw.level * echo[1],
            ]
        }
    }

    /// A sample passing untouched: the pitch jump's window still hears it.
    #[inline]
    pub fn feed(&mut self, x: [f32; 2]) {
        self.pl.push(x[0]);
        self.pr.push(x[1]);
        self.tl.skip(1);
        self.tr.skip(1);
    }

    /// No echo rings.
    pub fn silent_tail(&self) -> bool {
        self.tl.is_silent() && self.tr.is_silent() && self.lp.iter().all(OnePole::is_zero)
    }

    /// `frames` of silence skipped at rest.
    pub fn skip(&mut self, frames: usize) {
        self.pl.skip(frames);
        self.pr.skip(frames);
        self.tl.skip(frames);
        self.tr.skip(frames);
    }

    /// The crush's hold, as a skip leaves it: the hold count of the event playing at `so`.
    pub fn skip_crush_to(&mut self, e: Option<Ev>, so: i64) {
        match e {
            Some(e) => {
                self.crush_ev = e.start;
                self.hold_n = self.hold_index(so - e.start);
                self.held = [0.0; 2];
            }
            None => {
                self.held = [0.0; 2];
                self.hold_n = -1;
                self.crush_ev = i64::MIN;
            }
        }
    }

    /// The filter as a skip leaves it.
    pub fn skip_filter_to(&mut self, e: Option<Ev>) {
        self.svf = [[0.0; 2]; 2];
        self.filt_ev = e.map_or(i64::MIN, |e| e.start);
    }

    /// The shifter, `t` samples into its event: four taps a quarter of the window apart,
    /// each sweeping its delay at `1 − ratio` and weighed by a Hann over it (four Hanns a
    /// quarter apart sum to two), so each tap is silent where its delay wraps and no two
    /// taps line up in phase for long.
    fn shift(&self, t: i64) -> [f32; 2] {
        let w = f64::from(self.pitch.window_ms) * 0.001 * f64::from(self.sr);
        let r = math::exp2(f64::from(self.pitch.semis) / 12.0);
        let ph = t as f64 * (1.0 - r) / w;
        let mut y = [0.0f32; 2];
        if self.pitch.clean {
            // Two heads half a window apart. Each rises over the splice from where its
            // delay wraps, sounds alone, and falls over the splice as the other rises
            // (sin² + cos²: the two weights sum to one), so for most of the window one
            // head plays — no comb between taps, no warble but at the splices.
            for k in 0..2 {
                let p = ph + 0.5 * k as f64;
                let p = p - p.floor();
                let h = if p < CLEAN_SPLICE {
                    let s = math::sin(std::f64::consts::FRAC_PI_2 * p / CLEAN_SPLICE);
                    s * s
                } else if p < 0.5 {
                    1.0
                } else if p < 0.5 + CLEAN_SPLICE {
                    let c = math::cos(std::f64::consts::FRAC_PI_2 * (p - 0.5) / CLEAN_SPLICE);
                    c * c
                } else {
                    0.0
                } as f32;
                if h > 0.0 {
                    let d = (p * w) as f32;
                    y[0] += h * self.pl.read(d);
                    y[1] += h * self.pr.read(d);
                }
            }
            return y;
        }
        for k in 0..4 {
            let p = ph + 0.25 * k as f64;
            let p = p - p.floor();
            let s = math::sin(std::f64::consts::PI * p);
            let h = (0.5 * s * s) as f32;
            let d = (p * w) as f32;
            y[0] += h * self.pl.read(d);
            y[1] += h * self.pr.read(d);
        }
        y
    }

    fn hold_index(&self, t: i64) -> i64 {
        if self.crush.rate_hz >= self.sr {
            t
        } else {
            (t as f64 * f64::from(self.crush.rate_hz) / f64::from(self.sr)).floor() as i64
        }
    }

    fn crush(&mut self, e: &Ev, so: i64, x: [f32; 2]) -> [f32; 2] {
        if e.start != self.crush_ev {
            self.crush_ev = e.start;
            self.hold_n = -1;
        }
        let h = self.hold_index(so - e.start);
        if h != self.hold_n {
            self.hold_n = h;
            self.held = x;
        }
        if self.crush.bits >= 24.0 {
            return self.held;
        }
        let q = math::exp2(1.0 - self.crush.bits);
        [
            (self.held[0] / q).round() * q,
            (self.held[1] / q).round() * q,
        ]
    }

    fn filter(&mut self, e: &Ev, so: i64, x: [f32; 2], g: &Grid) -> [f32; 2] {
        if e.start != self.filt_ev {
            self.filt_ev = e.start;
            self.svf = [[0.0; 2]; 2];
        }
        let span = if e.end == i64::MAX {
            4.0 * g.spb()
        } else {
            (e.end - e.start) as f64
        };
        let u = ((so - e.start) as f64 / span).clamp(0.0, 1.0);
        let bend = math::exp2(-2.0 * f64::from(self.filter.curve));
        let u = math::powf(u, bend);
        let (a, b) = (f64::from(self.filter.from_hz), f64::from(self.filter.to_hz));
        let fc = (a * math::powf(b / a, u)).min(0.45 * g.sr);
        let gg = math::tan(std::f64::consts::PI * fc / g.sr) as f32;
        let k = 2.0 - 1.9 * self.filter.res;
        let a1 = 1.0 / (1.0 + gg * (gg + k));
        let a2 = gg * a1;
        let a3 = gg * a2;
        let mut y = [0.0; 2];
        for (ch, out) in y.iter_mut().enumerate() {
            let [ic1, ic2] = self.svf[ch];
            let v3 = x[ch] - ic2;
            let v1 = a1 * ic1 + a2 * v3;
            let v2 = ic2 + a2 * ic1 + a3 * v3;
            self.svf[ch] = [2.0 * v1 - ic1, 2.0 * v2 - ic2];
            *out = match self.filter.kind {
                FilterKind::LowPass => v2,
                FilterKind::HighPass => x[ch] - k * v1 - v2,
                FilterKind::BandPass => k * v1,
            };
        }
        y
    }

    /// How shut the gate is at `so` (0 open … 1 shut): each pulse open for its duty, shut
    /// after, open again on the next pulse's line.
    fn gate_shut(&self, e: &Ev, so: i64, g: &Grid, xf: u32) -> f32 {
        if self.gate.rate <= 0.0 {
            return 1.0;
        }
        let len = (self.gate.rate * g.spb()).max(4.0);
        let t = so - e.start;
        let mut i = (t as f64 / len).floor() as i64;
        let at = |i: i64| (i as f64 * len).round() as i64;
        while at(i) > t {
            i -= 1;
        }
        while at(i + 1) <= t {
            i += 1;
        }
        let (p0, p1) = (at(i), at(i + 1));
        let plen = p1 - p0;
        let open = (f64::from(self.gate.duty) * plen as f64).round() as i64;
        let tau = t - p0;
        if open >= plen {
            return 0.0;
        }
        let x = i64::from(xf).min((plen - open) / 2).max(1);
        let xi = (x + 1) as f64;
        if tau < open {
            0.0
        } else if tau < open + x {
            rc((tau - open + 1) as f64 / xi)
        } else if p1 - (so - e.start) <= x {
            rc((p1 - t) as f64 / xi)
        } else {
            1.0
        }
    }

    fn throw(&mut self, send: [f32; 2], g: &Grid) -> [f32; 2] {
        let quiet = send[0].to_bits() == 0 && send[1].to_bits() == 0;
        if quiet && self.tl.is_silent() && self.tr.is_silent() {
            self.tl.skip(1);
            self.tr.skip(1);
            return [0.0; 2];
        }
        let d = ((self.throw.time * g.spb()) as f32 - 1.0).clamp(0.0, self.tl.max_delay());
        let y = [self.tl.read(d), self.tr.read(d)];
        let fb = self.throw.feedback;
        let back = [
            self.lp[0].run(y[0], self.lp_g),
            self.lp[1].run(y[1], self.lp_g),
        ];
        self.lp[0].settle(1e-20);
        self.lp[1].settle(1e-20);
        if self.throw.pingpong {
            let m = 0.5 * (send[0] + send[1]);
            self.tl.push(flush(m + fb * back[1]));
            self.tr.push(flush(fb * back[0]));
        } else {
            self.tl.push(flush(send[0] + fb * back[0]));
            self.tr.push(flush(send[1] + fb * back[1]));
        }
        y
    }
}

/// An echo below −160 dB (under any converter's floor) is silence, so the echoes end in
/// zeros and the unit rests.
pub const ECHO_FLOOR: f32 = 1e-8;

#[inline]
fn flush(x: f32) -> f32 {
    if x.abs() < ECHO_FLOOR {
        0.0
    } else {
        x
    }
}

#[inline]
fn mix(dry: [f32; 2], wet: [f32; 2], w: f32) -> [f32; 2] {
    if w >= 1.0 {
        wet
    } else {
        [
            dry[0] + (wet[0] - dry[0]) * w,
            dry[1] + (wet[1] - dry[1]) * w,
        ]
    }
}
