//! Fragor: a pedalboard of distortion and fuzz — five pedals modelled from their circuits
//! (the TS808, the RAT, the DS-1, the Fuzz Face and the Big Muff π), up to three of them in
//! a chain in any order, a noise gate before them, oversampled.
//!
//! ## The signal path
//!
//! ```text
//! in ─┬─ × input ─ gate ─ ↑os ─ pedal ─ pedal ─ pedal ─ ↓os ──┐
//!     └─ delay (the latency) ────────────────────── dry ─ mix ─┴─ × output
//! ```
//!
//! - **The pedals** are circuits ([`circuits`]), solved each oversampled sample by the DK
//!   method ([`dk`]): the op-amp's frequency-dependent gain into the diodes, the transistor
//!   stages, the tone stacks — what the schematic does, not a curve that resembles it. The
//!   signal is in volts: 1.0 at the input is one volt at the pedal's jack (a guitar's
//!   pickup gives some hundreds of millivolts).
//! - **The chain**: three slots, each holding one of the five pedals or nothing; a pedal
//!   is in at most one slot. Each pedal has its footswitch: off, it is a wire (a 10 ms
//!   crossfade, no click) and it rests. A new order dips the chain to the dry signal for
//!   10 ms on each side of the change.
//! - **Oversampling** ([`crate::fervor::os`], linear phase): 2×, 4× (the default) or 8×.
//!   The circuits run at the high rate — their harmonics are filtered there instead of
//!   folding back, and the trapezoidal rule's frequency warping stays far above the band.
//! - **The gate** ([`gate`]) closes on what falls below its threshold, before the pedals.
//!
//! ## On a grid, and the null
//!
//! Settings are read every [`UPDATE`] samples counted from the start of the stream, and
//! the knobs glide (20 ms) — so the output does not depend on the block size. With no
//! pedal in the chain (or every footswitch off) and the gate off, the unit is a wire times
//! its input and output gains — exactly 1 at 0 dB: the defaults touch no sample.
//!
//! ## Rest
//!
//! A circuit never quite stops (its capacitors hold their charge), so the board counts
//! samples of exact digital silence at its input: after [`SETTLE_S`] of them every pedal
//! goes back to its operating point exactly (`Pedal::snap`) and the filters are cleared —
//! the state a new unit has — and the unit is at rest until sound comes again.

use crate::math;

/// The five pedals' circuits.
#[path = "fragor_circuits.rs"]
pub mod circuits;
/// The circuit solver (the DK method).
#[path = "fragor_dk.rs"]
pub mod dk;
/// The gate before the pedals.
#[path = "fragor_gate.rs"]
pub mod gate;

pub use circuits::{Model, Pedal};

use crate::fervor::os;

/// Settings are read every this many samples.
pub const UPDATE: usize = 16;
/// How long a knob takes to glide to a new value.
pub const GLIDE_MS: f32 = 20.0;
/// A footswitch's crossfade, and each half of a reorder's dip.
pub const FADE_MS: f32 = 10.0;
/// Seconds of digital silence after which the pedals return to rest.
pub const SETTLE_S: f32 = 1.0;
/// Slots in the chain.
pub const SLOTS: usize = 3;
/// The oversampling factors, by the `os` parameter's index.
pub const FACTORS: [usize; 3] = [2, 4, 8];
/// The longest dry delay (8× linear phase: 57 samples).
const DELAY_MAX: usize = 128;
/// Pedals: five per channel.
pub const PEDALS: usize = 5;

/// Fragor's settings, in the units a guitarist reads.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardSettings {
    /// What each slot holds, in signal order.
    pub chain: [Option<Model>; SLOTS],
    /// Each pedal's footswitch, by [`Model::index`].
    pub on: [bool; PEDALS],
    /// Each pedal's knobs, 0 … 1, in [`circuits::default_knobs`]'s order.
    pub knobs: [[f32; 3]; PEDALS],
    /// The gate's threshold, dBFS; at or below [`gate::OFF_DB`] it is off.
    pub gate_db: f32,
    pub in_db: f32,
    pub out_db: f32,
    /// Dry/wet, 0 … 1.
    pub mix: f32,
    /// One circuit for both channels (their sum), as a pedal is.
    pub mono: bool,
    /// 2, 4 or 8.
    pub oversampling: usize,
}

impl Default for BoardSettings {
    fn default() -> Self {
        let mut knobs = [[0.5f32; 3]; PEDALS];
        for m in Model::ALL {
            knobs[m.index()] = circuits::default_knobs(m).map(|v| v as f32);
        }
        BoardSettings {
            chain: [None; SLOTS],
            on: [true; PEDALS],
            knobs,
            gate_db: gate::OFF_DB,
            in_db: 0.0,
            out_db: 0.0,
            mix: 1.0,
            mono: false,
            oversampling: 4,
        }
    }
}

impl BoardSettings {
    /// The pedals that sound, in order (in a slot, footswitch on).
    pub fn live(&self) -> impl Iterator<Item = Model> + '_ {
        self.chain
            .iter()
            .flatten()
            .copied()
            .filter(|m| self.on[m.index()])
    }

    pub fn gate_on(&self) -> bool {
        self.gate_db > gate::OFF_DB
    }

    /// Whether the wet path runs: a pedal on the board (even switched off — a footswitch
    /// never moves the latency) or the gate on.
    pub fn engaged(&self) -> bool {
        self.chain.iter().any(Option::is_some) || self.gate_on()
    }

    /// The delay the unit adds: the oversampling filters', when anything is engaged.
    pub fn latency(&self) -> usize {
        if self.engaged() {
            os::latency(factor(self.oversampling), false)
        } else {
            0
        }
    }
}

/// A valid oversampling factor (4 if `f` is not one).
pub fn factor(f: usize) -> usize {
    FACTORS.iter().copied().find(|x| *x == f).unwrap_or(4)
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

/// Fragor, stereo.
#[derive(Clone, Debug)]
pub struct Fragor {
    sr: f32,
    target: BoardSettings,
    /// The knobs as they sound (gliding toward the target's).
    knobs: [[f32; 3]; PEDALS],
    /// The chain as it sounds, and whether it runs one circuit for both channels.
    chain: [Option<Model>; SLOTS],
    mono: bool,
    factor: usize,
    latency: usize,
    /// Each pedal's footswitch share (0 bypassed … 1 in).
    fade: [f32; PEDALS],
    /// The chain's share against the dry signal while the order changes (1: the chain).
    dip: f32,
    /// −1 dipping toward a change, +1 coming back, 0 still.
    dip_dir: i8,
    fade_step: f32,
    /// The pedals: five for each channel (the second idle in mono).
    /// Built on the heap one by one (ten circuits are too large for a thread's stack).
    /// Channel `ch`'s pedal `i` is `pedals[ch * PEDALS + i]`.
    pedals: Box<[Pedal]>,
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
    /// Samples of exact silence in a row at the board's input and at the chain's (after the
    /// gate), and how many bring the gate and the pedals to rest.
    quiet_in: u32,
    quiet_wet: u32,
    settle_len: u32,
    /// The pedals rest at their operating points and the oversamplers are clear: silence
    /// into the chain is silence out of it, without running it, until sound comes.
    idle: bool,
    tel: Meter,
    blocks: u32,
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
    pedal_peak: [f32; PEDALS],
}

impl Fragor {
    pub fn new(sample_rate: f32) -> Fragor {
        let sr = sample_rate.max(1.0);
        let target = BoardSettings::default();
        let rate = sr as f64 * 4.0;
        let mut v: Vec<Pedal> = Vec::with_capacity(2 * PEDALS);
        for _ in 0..2 {
            for m in Model::ALL {
                v.push(Pedal::new(m, rate));
            }
        }
        let pedals = v.into_boxed_slice();
        let mut f = Fragor {
            sr,
            knobs: target.knobs,
            chain: target.chain,
            mono: target.mono,
            factor: 4,
            latency: 0,
            fade: [1.0; PEDALS],
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

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    /// New settings: they take effect at the next grid point. Allocation-free.
    pub fn set(&mut self, s: &BoardSettings) {
        self.target.clone_from(s);
    }

    pub fn settings(&self) -> &BoardSettings {
        &self.target
    }

    /// The delay the unit adds, in samples; follows the settings as soon as they are set.
    pub fn latency(&self) -> usize {
        self.target.latency()
    }

    /// Rebuild the pedals for an oversampling factor (allocation-free: they live in place).
    fn rebuild_rate(&mut self, factor: usize) {
        self.factor = factor;
        let rate = self.sr as f64 * factor as f64;
        for (i, p) in self.pedals.iter_mut().enumerate() {
            *p = Pedal::new(Model::ALL[i % PEDALS], rate);
        }
        for o in &mut self.os {
            o.configure(factor, false);
            o.reset();
        }
    }

    /// Every sounding pedal's knobs as they are now, and back to its operating point.
    fn rest_pedals(&mut self) {
        for ch in 0..2 {
            for m in self.chain.iter().flatten() {
                let i = m.index();
                let k = self.knobs[i].map(|v| v as f64);
                let p = &mut self.pedals[ch * PEDALS + i];
                p.set_knobs(k);
                p.snap();
            }
        }
    }

    /// fx/final — the settings taken at once (`FxUnit::snap`: a start, a seek, a loop's
    /// wrap): the chain the target's (no dip), every footswitch's fade, the knobs, the input,
    /// output and mix landed, the gate's threshold. The circuits keep what they hold — only
    /// a board resting in silence moves its pedals to the new knobs' operating points, as a
    /// grid point does there — and so do the oversamplers, the gate and the dry delay.
    /// Allocation-free.
    pub fn snap(&mut self) {
        if !self.structure_is_target() {
            self.apply_structure();
        }
        self.dip = 1.0;
        self.dip_dir = 0;
        let t = &self.target;
        self.knobs = t.knobs;
        for m in Model::ALL {
            self.fade[m.index()] = if t.on[m.index()] { 1.0 } else { 0.0 };
        }
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
        for ch in 0..2 {
            for m in self.chain.iter().flatten() {
                let i = m.index();
                let k = self.knobs[i].map(|v| v as f64);
                let p = &mut self.pedals[ch * PEDALS + i];
                p.set_knobs(k);
                if self.idle {
                    p.snap();
                }
            }
        }
    }

    /// Back to silence: every glide landed, the chain the target's, the pedals at rest.
    pub fn reset(&mut self) {
        let t = self.target.clone();
        let f = factor(t.oversampling);
        if f != self.factor {
            self.rebuild_rate(f);
        }
        self.knobs = t.knobs;
        self.chain = t.chain;
        self.mono = t.mono;
        self.latency = t.latency();
        for m in Model::ALL {
            self.fade[m.index()] = if t.on[m.index()] { 1.0 } else { 0.0 };
        }
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

    /// The pedals to their operating points, the oversamplers and the gate cleared.
    fn come_to_rest(&mut self) {
        self.rest_wet();
        self.gate.reset();
    }

    /// The pedals to their operating points and the oversamplers cleared.
    fn rest_wet(&mut self) {
        self.rest_pedals();
        for o in &mut self.os {
            o.reset();
        }
        self.idle = true;
    }

    /// Whether no pedal sounds (none on, none fading) and no dip runs: the chain is then
    /// the dry signal, delayed.
    fn chain_silent(&self) -> bool {
        self.dip_dir == 0
            && self
                .chain
                .iter()
                .flatten()
                .all(|m| self.fade[m.index()] == 0.0)
    }

    /// Whether the structure (the chain, mono, the factor) is the target's.
    fn structure_is_target(&self) -> bool {
        self.chain == self.target.chain
            && self.mono == self.target.mono
            && self.factor == factor(self.target.oversampling)
    }
}

impl Fragor {
    /// Apply the target's structure: the factor (rebuilding the circuits for the new rate),
    /// the chain and mono. The pedals now in the chain start from rest.
    fn apply_structure(&mut self) {
        let f = factor(self.target.oversampling);
        if f != self.factor {
            self.rebuild_rate(f);
        }
        self.chain = self.target.chain;
        self.mono = self.target.mono;
        self.rest_pedals();
    }

    /// A pedal in the chain (its footswitch may be off).
    fn in_chain(&self, i: usize) -> bool {
        self.chain.iter().flatten().any(|m| m.index() == i)
    }

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
        for m in Model::ALL {
            let i = m.index();
            let mut moved = false;
            for k in 0..3 {
                moved |= glide(&mut self.knobs[i][k], self.target.knobs[i][k], a, 1e-5);
            }
            if moved && self.in_chain(i) && self.fade[i] > 0.0 {
                let kn = self.knobs[i].map(|v| v as f64);
                for ch in 0..2 {
                    let p = &mut self.pedals[ch * PEDALS + i];
                    p.set_knobs(kn);
                    if self.idle {
                        p.snap();
                    }
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
        let moving = self.dip_dir != 0
            || !self.structure_is_target()
            || Model::ALL.iter().any(|m| {
                let f = self.fade[m.index()];
                f != if self.target.on[m.index()] { 1.0 } else { 0.0 }
            });
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

    /// Footswitches and the reorder's dip, one sample on.
    #[inline]
    fn move_fades(&mut self) {
        let st = self.fade_step;
        for i in 0..PEDALS {
            let to = if self.target.on[i] { 1.0 } else { 0.0 };
            let f = self.fade[i];
            if f == to {
                continue;
            }
            if f == 0.0 && self.in_chain(i) {
                // Switched on: from rest, with the knobs as they are.
                let kn = self.knobs[i].map(|v| v as f64);
                for ch in 0..2 {
                    self.pedals[ch * PEDALS + i].set_knobs(kn);
                    self.pedals[ch * PEDALS + i].snap();
                }
            }
            self.fade[i] = if to > f {
                (f + st).min(1.0)
            } else {
                (f - st).max(0.0)
            };
        }
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

    /// The chain on one channel, oversampled.
    #[inline]
    fn run_chain(&mut self, ch: usize, x: f32) -> f32 {
        let mut buf = [0.0f32; 8];
        self.os[ch].up(x, &mut buf);
        for b in buf.iter_mut().take(self.factor) {
            let mut v = *b as f64;
            for m in self.chain.iter().flatten() {
                let i = m.index();
                let f = self.fade[i];
                if f <= 0.0 {
                    continue;
                }
                let y = self.pedals[ch * PEDALS + i].tick(v);
                let pk = &mut self.tel.pedal_peak[i];
                *pk = pk.max(y.abs() as f32);
                v = if f >= 1.0 { y } else { v + f as f64 * (y - v) };
            }
            *b = v as f32;
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
                self.move_fades();
                let (dl, dr) = self.write_dry(xl, xr);
                let (mut a, mut b) = (xl * ig, xr * ig);
                if self.target.gate_on() {
                    let g = self.gate.tick(a.abs().max(b.abs()), self.gate_thr);
                    // Closed is +0.0, bit for bit (a negative sample times 0 is −0.0).
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
                    // At rest, silence in: silence out, nothing run.
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
                // No pedal sounding and the gate off: the dry signal, exactly.
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

    /// At rest (ADR 0031 §5.1): every glide landed, the structure and footswitches the
    /// target's, no dip, the gains still; then either nothing is engaged and the dry delay
    /// holds zeros, or the pedals rest at their operating points with the filters clear.
    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    /// The first condition that keeps the unit from resting.
    pub fn rest_blocker(&self) -> Option<&'static str> {
        let t = &self.target;
        if self.knobs != t.knobs {
            return Some("knobs gliding");
        }
        if !self.structure_is_target() || self.dip_dir != 0 || self.dip != 1.0 {
            return Some("chain changing");
        }
        if Model::ALL
            .iter()
            .any(|m| self.fade[m.index()] != if t.on[m.index()] { 1.0 } else { 0.0 })
        {
            return Some("footswitch fading");
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
        if !self.delay.iter().all(|d| crate::rest::all_zero(d)) {
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

    /// What the editor reads, refreshed after every block (layout: [`telemetry`]).
    pub fn write_telemetry(&self, t: &mut [f32; 64]) {
        use telemetry::*;
        let db = |v: f32| {
            if v > 1e-9 {
                20.0 * math::log10(v)
            } else {
                -120.0
            }
        };
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
        for i in 0..PEDALS {
            t[PEDAL_PEAK + i] = db(self.tel.pedal_peak[i]);
            t[FADE + i] = self.fade[i];
        }
        t[IN_LINEAR] = self.tel.in_peak;
        for (s, m) in self.chain.iter().enumerate() {
            t[CHAIN + s] = m.map(|m| m.index() as f32 + 1.0).unwrap_or(0.0);
        }
    }
}

/// The telemetry's layout: what each of the 64 numbers is.
pub mod telemetry {
    /// Blocks processed, modulo 2²⁴.
    pub const BLOCKS: usize = 0;
    /// The last block's input and output peaks, dBFS (−120 for silence).
    pub const IN_PEAK: usize = 1;
    pub const OUT_PEAK: usize = 2;
    /// The gate's gain (0 … 1) and whether it is open.
    pub const GATE: usize = 3;
    pub const GATE_OPEN: usize = 4;
    pub const LATENCY: usize = 5;
    pub const SAMPLE_RATE: usize = 6;
    pub const FACTOR: usize = 7;
    /// 1 when anything but the gains touches the signal.
    pub const ENGAGED: usize = 8;
    /// 1 while the pedals rest (silence).
    pub const RESTING: usize = 9;
    /// Each pedal's output peak in the last block, dBFS, by model index (five).
    pub const PEDAL_PEAK: usize = 10;
    /// Each pedal's footswitch share, 0 … 1 (five).
    pub const FADE: usize = 15;
    /// The last block's input peak, linear (where the display's dot sits).
    pub const IN_LINEAR: usize = 20;
    /// The chain as it sounds: each slot's model index + 1, or 0 (three).
    pub const CHAIN: usize = 21;
}

#[cfg(test)]
#[path = "fragor_spice_tests.rs"]
mod spice_tests;
#[cfg(test)]
#[path = "fragor_tests.rs"]
mod tests;
