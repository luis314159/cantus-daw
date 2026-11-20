//! One of Fragor's circuits alone (round 41, ADR 0152): Fragor's board (`cantus_dsp::fragor`)
//! with one slot, built with only that circuit — two circuits (a channel each) where Fragor
//! builds ten — and **the same bits as `Fragor` with only that slot filled**: every step below
//! is Fragor's, in its order, with the chain reduced to one circuit that is in or out
//! (`fx_fragor_pedal`'s tests hold it, at every setting, block size and rest).
//!
//! What Fragor does that one circuit never needs is left out: a footswitch per pedal (the
//! board has its own) and a reorder (one slot). What is kept: the gate, the input, the
//! oversampling (2×, 4×, 8×), mono, the dry path delayed by the latency for the mix, the
//! output, the 16-sample grid with its 20 ms glides, the 10 ms dip when the circuit comes in
//! or goes out (or the quality or mono change), and the rest after a second of silence.
//!
//! This copies Fragor's control flow rather than calling into it because `fragor.rs` is
//! nobody's this round (its golden hashes must not move); if Fragor's board ever changes,
//! the equality tests fail until this follows.

use cantus_dsp::fervor::os;
use cantus_dsp::fragor::{factor, gate, BoardSettings, Model, Pedal, FADE_MS, GLIDE_MS, UPDATE};
use cantus_dsp::fragor::{telemetry, SETTLE_S};
use cantus_dsp::math;

/// Fragor's longest dry delay (8× linear phase is 57 samples).
const DELAY_MAX: usize = 128;

/// One circuit's settings, in the units a guitarist reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// Whether the circuit is in (out: a wire).
    pub on: bool,
    /// Its knobs, 0 … 1, in `circuits::default_knobs`'s order.
    pub knobs: [f32; 3],
    /// The gate's threshold, dBFS; at or below `gate::OFF_DB` it is off.
    pub gate_db: f32,
    pub in_db: f32,
    pub out_db: f32,
    pub mix: f32,
    pub mono: bool,
    /// 2, 4 or 8.
    pub oversampling: usize,
}

impl Settings {
    /// The defaults: the circuit out, its knobs Fragor's defaults, the gate off, 0 dB.
    pub fn new(model: Model) -> Settings {
        let d = BoardSettings::default();
        Settings {
            on: false,
            knobs: d.knobs[model.index()],
            gate_db: d.gate_db,
            in_db: d.in_db,
            out_db: d.out_db,
            mix: d.mix,
            mono: d.mono,
            oversampling: d.oversampling,
        }
    }

    /// Fragor's board this is: the circuit alone in the first slot (or no slot filled), its
    /// footswitch on, the other circuits at their defaults.
    pub fn board(&self, model: Model) -> BoardSettings {
        let mut b = BoardSettings::default();
        b.chain[0] = self.on.then_some(model);
        b.knobs[model.index()] = self.knobs;
        b.gate_db = self.gate_db;
        b.in_db = self.in_db;
        b.out_db = self.out_db;
        b.mix = self.mix;
        b.mono = self.mono;
        b.oversampling = self.oversampling;
        b
    }

    fn gate_on(&self) -> bool {
        self.gate_db > gate::OFF_DB
    }

    /// Whether the wet path runs: the circuit in, or the gate on.
    fn engaged(&self) -> bool {
        self.on || self.gate_on()
    }

    fn latency(&self) -> usize {
        if self.engaged() {
            os::latency(factor(self.oversampling), false)
        } else {
            0
        }
    }
}

fn gain_of(db: f32) -> f32 {
    if db == 0.0 {
        1.0
    } else {
        math::db_to_gain(db)
    }
}

/// Glide `v` toward `to` by `a`; snap when within `eps`. Whether it moved.
fn glide(v: &mut f32, to: f32, a: f32, eps: f32) -> bool {
    if *v == to {
        return false;
    }
    *v += (to - *v) * a;
    if (*v - to).abs() < eps {
        *v = to;
    }
    true
}

/// A gain stepping linearly across a grid step.
#[derive(Copy, Clone, Debug, PartialEq)]
struct Ramp {
    v: f32,
    end: f32,
    step: f32,
}

impl Ramp {
    fn at(v: f32) -> Ramp {
        Ramp {
            v,
            end: v,
            step: 0.0,
        }
    }
    fn aim(&mut self, to: f32) {
        self.v = self.end;
        self.end = to;
        self.step = (to - self.v) / UPDATE as f32;
    }
    fn still(&self, to: f32) -> bool {
        self.v == to && self.end == to && self.step == 0.0
    }
}

#[derive(Copy, Clone, Debug, Default)]
struct Meter {
    in_peak: f32,
    out_peak: f32,
    pedal_peak: f32,
}

/// One circuit on Fragor's board, stereo.
#[derive(Clone, Debug)]
pub struct One {
    sr: f32,
    model: Model,
    target: Settings,
    /// The knobs as they sound (gliding toward the target's).
    knobs: [f32; 3],
    /// Whether the circuit is in the chain as it sounds, and whether it runs one circuit for
    /// both channels.
    chain: bool,
    mono: bool,
    factor: usize,
    latency: usize,
    /// The chain's share against the dry signal while the structure changes (1: the chain).
    dip: f32,
    /// −1 dipping toward a change, +1 coming back, 0 still.
    dip_dir: i8,
    fade_step: f32,
    /// The circuit, once per channel (the second idle in mono).
    pedals: Box<[Pedal; 2]>,
    os: [os::Oversampler; 2],
    gate: gate::Gate,
    gate_thr: f32,
    delay: [[f32; DELAY_MAX]; 2],
    pos: usize,
    grid: usize,
    alpha: f32,
    in_db: f32,
    out_db: f32,
    mix: f32,
    /// Gains interpolated across each grid step.
    gains: [Ramp; 3],
    engaged: bool,
    quiet_in: u32,
    quiet_wet: u32,
    settle_len: u32,
    /// The circuit rests at its operating point and the oversamplers are clear.
    idle: bool,
    tel: Meter,
    blocks: u32,
}

impl One {
    pub fn new(model: Model, sample_rate: f32) -> One {
        let sr = sample_rate.max(1.0);
        let target = Settings::new(model);
        let rate = sr as f64 * 4.0;
        let pedals = Box::new([Pedal::new(model, rate), Pedal::new(model, rate)]);
        let mut f = One {
            sr,
            model,
            knobs: target.knobs,
            chain: target.on,
            mono: target.mono,
            factor: 4,
            latency: 0,
            dip: 1.0,
            dip_dir: 0,
            fade_step: 1.0 / (FADE_MS * 0.001 * sr).max(1.0),
            pedals,
            os: [
                os::Oversampler::new(4, false),
                os::Oversampler::new(4, false),
            ],
            gate: gate::Gate::new(sr),
            gate_thr: 0.0,
            delay: [[0.0; DELAY_MAX]; 2],
            pos: 0,
            grid: 0,
            alpha: 1.0 - math::exp(-(UPDATE as f32) / (GLIDE_MS * 0.001 * sr)),
            in_db: 0.0,
            out_db: 0.0,
            mix: 1.0,
            gains: [Ramp::at(1.0); 3],
            engaged: false,
            quiet_in: 0,
            quiet_wet: 0,
            settle_len: (SETTLE_S * sr) as u32,
            idle: true,
            tel: Meter::default(),
            blocks: 0,
            target,
        };
        f.reset();
        f
    }

    pub fn model(&self) -> Model {
        self.model
    }

    /// New settings: they take effect at the next grid point. Allocation-free.
    pub fn set(&mut self, s: &Settings) {
        self.target = *s;
    }

    pub fn settings(&self) -> &Settings {
        &self.target
    }

    /// The delay the unit adds, in samples; follows the settings as soon as they are set.
    pub fn latency(&self) -> usize {
        self.target.latency()
    }

    /// Rebuild the circuits for an oversampling factor (allocation-free: they live in place).
    fn rebuild_rate(&mut self, factor: usize) {
        self.factor = factor;
        let rate = self.sr as f64 * factor as f64;
        for p in self.pedals.iter_mut() {
            *p = Pedal::new(self.model, rate);
        }
        for o in &mut self.os {
            o.configure(factor, false);
            o.reset();
        }
    }

    /// The circuit (when in the chain) with its knobs as they are now, at its operating point.
    fn rest_pedals(&mut self) {
        if !self.chain {
            return;
        }
        let k = self.knobs.map(|v| v as f64);
        for p in self.pedals.iter_mut() {
            p.set_knobs(k);
            p.snap();
        }
    }

    /// Fragor's `snap`: the settings taken at once (a start, a seek, a loop's wrap).
    pub fn snap(&mut self) {
        if !self.structure_is_target() {
            self.apply_structure();
        }
        self.dip = 1.0;
        self.dip_dir = 0;
        let t = self.target;
        self.knobs = t.knobs;
        self.in_db = t.in_db;
        self.out_db = t.out_db;
        self.mix = t.mix.clamp(0.0, 1.0);
        self.gains = [
            Ramp::at(gain_of(self.in_db)),
            Ramp::at(gain_of(self.out_db)),
            Ramp::at(self.mix),
        ];
        self.gate_thr = gain_of(t.gate_db);
        self.latency = t.latency();
        let engaged = t.engaged();
        if engaged && !self.engaged {
            self.come_to_rest();
        }
        self.engaged = engaged;
        if self.chain {
            let k = self.knobs.map(|v| v as f64);
            let idle = self.idle;
            for p in self.pedals.iter_mut() {
                p.set_knobs(k);
                if idle {
                    p.snap();
                }
            }
        }
    }

    /// Back to silence: every glide landed, the structure the target's, the circuit at rest.
    pub fn reset(&mut self) {
        let t = self.target;
        let f = factor(t.oversampling);
        if f != self.factor {
            self.rebuild_rate(f);
        }
        self.knobs = t.knobs;
        self.chain = t.on;
        self.mono = t.mono;
        self.latency = t.latency();
        self.dip = 1.0;
        self.dip_dir = 0;
        self.in_db = t.in_db;
        self.out_db = t.out_db;
        self.mix = t.mix.clamp(0.0, 1.0);
        self.gains = [
            Ramp::at(gain_of(self.in_db)),
            Ramp::at(gain_of(self.out_db)),
            Ramp::at(self.mix),
        ];
        self.gate_thr = gain_of(t.gate_db);
        self.engaged = t.engaged();
        self.come_to_rest();
        self.delay = [[0.0; DELAY_MAX]; 2];
        self.pos = 0;
        self.grid = 0;
        self.quiet_in = self.settle_len;
        self.quiet_wet = self.settle_len;
    }

    fn come_to_rest(&mut self) {
        self.rest_wet();
        self.gate.reset();
    }

    fn rest_wet(&mut self) {
        self.rest_pedals();
        for o in &mut self.os {
            o.reset();
        }
        self.idle = true;
    }

    /// No circuit sounds and no dip runs: the chain is the dry signal, delayed.
    fn chain_silent(&self) -> bool {
        self.dip_dir == 0 && !self.chain
    }

    fn structure_is_target(&self) -> bool {
        self.chain == self.target.on
            && self.mono == self.target.mono
            && self.factor == factor(self.target.oversampling)
    }

    fn apply_structure(&mut self) {
        let f = factor(self.target.oversampling);
        if f != self.factor {
            self.rebuild_rate(f);
        }
        self.chain = self.target.on;
        self.mono = self.target.mono;
        self.rest_pedals();
    }
}

impl One {
    /// A grid point: read the target, glide, rebuild what moved.
    fn update(&mut self) {
        if !self.structure_is_target() && self.dip_dir == 0 {
            if self.engaged {
                self.dip_dir = -1;
            } else {
                self.apply_structure();
            }
        }
        let a = self.alpha;
        let mut moved = false;
        for k in 0..3 {
            moved |= glide(&mut self.knobs[k], self.target.knobs[k], a, 1e-5);
        }
        if moved && self.chain {
            let kn = self.knobs.map(|v| v as f64);
            let idle = self.idle;
            for p in self.pedals.iter_mut() {
                p.set_knobs(kn);
                if idle {
                    p.snap();
                }
            }
        }
        glide(&mut self.in_db, self.target.in_db, a, 0.005);
        glide(&mut self.out_db, self.target.out_db, a, 0.005);
        glide(&mut self.mix, self.target.mix.clamp(0.0, 1.0), a, 1e-4);
        let (ig, og, mx) = (gain_of(self.in_db), gain_of(self.out_db), self.mix);
        self.gains[0].aim(ig);
        self.gains[1].aim(og);
        self.gains[2].aim(mx);
        self.gate_thr = gain_of(self.target.gate_db);
        let moving = self.dip_dir != 0 || !self.structure_is_target();
        let engaged = self.target.engaged() || moving;
        if engaged && !self.engaged {
            self.come_to_rest();
        }
        self.engaged = engaged;
        self.latency = if engaged {
            os::latency(self.factor, false)
        } else {
            0
        };
    }

    /// The structure's dip, one sample on.
    #[inline]
    fn move_dip(&mut self) {
        let st = self.fade_step;
        match self.dip_dir {
            -1 => {
                self.dip -= st;
                if self.dip <= 0.0 {
                    self.dip = 0.0;
                    self.apply_structure();
                    self.dip_dir = 1;
                }
            }
            1 => {
                self.dip += st;
                if self.dip >= 1.0 {
                    self.dip = 1.0;
                    self.dip_dir = 0;
                }
            }
            _ => {}
        }
    }

    /// The circuit on one channel, oversampled.
    #[inline]
    fn run_chain(&mut self, ch: usize, x: f32) -> f32 {
        let mut buf = [0.0f32; 8];
        self.os[ch].up(x, &mut buf);
        if self.chain {
            let p = &mut self.pedals[ch];
            let pk = &mut self.tel.pedal_peak;
            for b in buf.iter_mut().take(self.factor) {
                let y = p.tick(*b as f64);
                *pk = pk.max(y.abs() as f32);
                *b = y as f32;
            }
        }
        self.os[ch].down(&mut buf)
    }

    /// Write the dry delay; returns the delayed pair.
    #[inline]
    fn write_dry(&mut self, a: f32, b: f32) -> (f32, f32) {
        let p = self.pos;
        self.pos = (p + 1) % DELAY_MAX;
        let r = (p + DELAY_MAX - self.latency.min(DELAY_MAX - 1)) % DELAY_MAX;
        self.delay[0][p] = a;
        self.delay[1][p] = b;
        (self.delay[0][r], self.delay[1][r])
    }
}

impl One {
    /// Process a stereo block in place.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let n = left.len().min(right.len());
        self.tel = Meter::default();
        for i in 0..n {
            if self.grid == 0 {
                self.update();
            }
            self.grid = (self.grid + 1) % UPDATE;
            let (xl, xr) = (left[i], right[i]);
            let peak_in = xl.abs().max(xr.abs());
            self.tel.in_peak = self.tel.in_peak.max(peak_in);
            let silent = xl.to_bits() == 0 && xr.to_bits() == 0;
            let [ig, og, mx] = [self.gains[0].v, self.gains[1].v, self.gains[2].v];
            for g in &mut self.gains {
                g.v += g.step;
            }
            let (yl, yr) = if !self.engaged {
                self.write_dry(xl, xr);
                (xl * og, xr * og)
            } else {
                self.move_dip();
                let (dl, dr) = self.write_dry(xl, xr);
                let (mut a, mut b) = (xl * ig, xr * ig);
                if self.target.gate_on() {
                    let g = self.gate.tick(a.abs().max(b.abs()), self.gate_thr);
                    // Closed is +0.0, bit for bit (a negative sample times 0 is -0.0).
                    (a, b) = if g == 0.0 { (0.0, 0.0) } else { (a * g, b * g) };
                }
                if self.mono {
                    let m = 0.5 * (a + b);
                    (a, b) = (m, m);
                }
                let into_silent = a.to_bits() == 0 && b.to_bits() == 0;
                self.quiet_wet = if into_silent {
                    (self.quiet_wet + 1).min(self.settle_len)
                } else {
                    0
                };
                let (wl, wr) = if self.idle && into_silent {
                    (0.0, 0.0)
                } else {
                    self.idle = false;
                    if self.mono {
                        let w = self.run_chain(0, a);
                        (w, w)
                    } else {
                        (self.run_chain(0, a), self.run_chain(1, b))
                    }
                };
                // No circuit sounding and the gate off: the dry signal, exactly.
                let (wl, wr) = if self.chain_silent() && !self.target.gate_on() {
                    (dl, dr)
                } else {
                    let dip = self.dip;
                    if dip >= 1.0 {
                        (wl, wr)
                    } else {
                        (dl + dip * (wl - dl), dr + dip * (wr - dr))
                    }
                };
                let (ml, mr) = if mx >= 1.0 {
                    (wl, wr)
                } else {
                    (dl + mx * (wl - dl), dr + mx * (wr - dr))
                };
                if !self.idle && self.quiet_wet >= self.settle_len {
                    self.rest_wet();
                }
                (ml * og, mr * og)
            };
            left[i] = yl;
            right[i] = yr;
            self.tel.out_peak = self.tel.out_peak.max(yl.abs()).max(yr.abs());
            if silent {
                self.quiet_in = (self.quiet_in + 1).min(self.settle_len);
                if self.quiet_in == self.settle_len && !self.gate.is_clear() {
                    self.gate.reset();
                }
            } else {
                self.quiet_in = 0;
            }
        }
        self.blocks = (self.blocks + 1) % (1 << 24);
    }

    /// At rest (ADR 0031 section 5.1): Fragor's conditions for one circuit.
    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    /// The first condition that keeps the unit from resting (in Fragor's words).
    pub fn rest_blocker(&self) -> Option<&'static str> {
        let t = &self.target;
        if self.knobs != t.knobs {
            return Some("knobs gliding");
        }
        if !self.structure_is_target() || self.dip_dir != 0 || self.dip != 1.0 {
            return Some("chain changing");
        }
        if self.in_db != t.in_db || self.out_db != t.out_db || self.mix != t.mix.clamp(0.0, 1.0) {
            return Some("gains gliding");
        }
        let (ig, og, mx) = (gain_of(self.in_db), gain_of(self.out_db), self.mix);
        if !self.gains[0].still(ig) || !self.gains[1].still(og) || !self.gains[2].still(mx) {
            return Some("gains ramping");
        }
        if self.gate_thr != gain_of(t.gate_db) || self.engaged != t.engaged() {
            return Some("settings pending");
        }
        if !self.delay.iter().all(|d| cantus_dsp::rest::all_zero(d)) {
            return Some("dry delay");
        }
        if self.engaged && !(self.idle && self.gate.is_clear()) {
            return Some("pedals sounding");
        }
        None
    }

    /// Samples of silence skipped while at rest: the grid moves on.
    pub fn skip(&mut self, frames: usize) {
        self.grid = (self.grid + frames % UPDATE) % UPDATE;
    }

    /// Fragor's telemetry layout (`cantus_dsp::fragor::telemetry`), this circuit alone in the
    /// first slot: the editor and the board read a pedal as they read a one-slot Fragor.
    pub fn write_telemetry(&self, t: &mut [f32; 64]) {
        use telemetry::*;
        let db = |v: f32| {
            if v > 1e-9 {
                20.0 * math::log10(v)
            } else {
                -120.0
            }
        };
        let m = self.model.index();
        t[BLOCKS] = self.blocks as f32;
        t[IN_PEAK] = db(self.tel.in_peak);
        t[OUT_PEAK] = db(self.tel.out_peak);
        t[GATE] = self.gate.gain();
        t[GATE_OPEN] = self.gate.is_open() as u8 as f32;
        t[LATENCY] = self.latency as f32;
        t[SAMPLE_RATE] = self.sr;
        t[FACTOR] = self.factor as f32;
        t[ENGAGED] = self.engaged as u8 as f32;
        t[RESTING] = self.idle as u8 as f32;
        for i in 0..5 {
            t[PEDAL_PEAK + i] = if i == m {
                db(self.tel.pedal_peak)
            } else {
                -120.0
            };
            t[FADE + i] = 1.0;
        }
        t[IN_LINEAR] = self.tel.in_peak;
        t[CHAIN] = if self.chain { m as f32 + 1.0 } else { 0.0 };
        t[CHAIN + 1] = 0.0;
        t[CHAIN + 2] = 0.0;
    }
}
