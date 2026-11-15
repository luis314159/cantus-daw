//! Arca, amp and cabinet (ADR 0137, provisional): a valve amp solved as its circuit, and
//! its cabinet by impulse response.
//!
//! ```text
//!  in (L+R)/2 ─ up ×2/4/8 ─ preamp circuits ─ master ─ power amp ─ down ─ cabinet ─ mix ─ out
//!       └──────────────────────── dry, delayed by the latency ─────────────────┘
//! ```
//!
//! - **The preamp** ([`net`], [`models`]): each model's published schematic as triode
//!   stages ([`crate::fragor::dk`]'s [`crate::fragor::dk::Nl::Triode`], Koren's plate
//!   current), coupling and cathode networks, the tone stack from its component values.
//! - **The power amp** ([`power`]): a phase inverter into push-pull pentodes and an output
//!   transformer, negative feedback and presence (or the cut), the supply's sag.
//! - **Oversampling** ([`crate::fervor::os`], linear phase): 2×, 4× (the default) or 8×;
//!   the latency it adds is reported, and the dry is delayed to match.
//! - **The cabinet** ([`cab`] makes the factory responses; the engine supplies a kernel —
//!   factory or the user's — to [`Arca::supply_cab`]): [`crate::aula_conv`], zero latency.
//!
//! **The null** (an amp colours by design: no setting of it is transparent): **no amp
//! and no cabinet** — the defaults — leaves the signal untouched, to the bit, with no
//! latency. A cabinet alone (an impulse-response loader) has no latency either.
//!
//! **Rest** (ADR 0031 §5.1): after a stretch of digital silence, once the amp's own tail
//! has fallen below −100 dB, the circuits are put back at their operating points, the
//! oversampler cleared; from then on silence in is silence out and nothing moves until
//! sound returns (the cabinet rests on its own count).

#[path = "arca_cab.rs"]
pub mod cab;
#[path = "arca_models.rs"]
pub mod models;
#[path = "arca_net.rs"]
pub mod net;
#[path = "arca_power.rs"]
pub mod power;
#[path = "arca_tube.rs"]
pub mod tube;

pub use models::AmpModel;

use crate::aula_conv::{Convolver, State as ConvState, Supplied};
use crate::aula_kernel::Kernel;
use crate::fervor::os;
use net::{Knobs, Preamp};
use power::Power;
use std::sync::Arc;

/// The guitar's lead into the input jack (Ω).
pub const SOURCE_OHMS: f64 = 1_000.0;
/// The dry line's length: more than the longest latency (8×: 57 samples).
const DELAY_MAX: usize = 64;
/// Samples per chunk (the cabinet runs on whole chunks).
const CHUNK: usize = 128;
/// The knobs move on a grid of this many samples, counted from the stream's start.
const GRID: u64 = 32;
/// A model change fades the amp out and in over this many samples.
const FADE: u32 = 96;
/// Rest: at least this much silence in (s), the amp's output under −100 dB for
/// [`QUIET_TAIL`] samples; or, whatever the tail, this much silence (s).
const SETTLE_S: f32 = 0.25;
const SETTLE_MAX_S: f32 = 4.0;
const QUIET_TAIL: u32 = 1_024;
const TAIL_FLOOR: f32 = 1e-5;

/// Everything Arca is set to, in real units.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ArcaSettings {
    /// The amp, or none (the cabinet alone, or nothing).
    pub model: Option<AmpModel>,
    /// The front panel, 0 … 1: the preamp's volume or gain pot, the tone stack, the
    /// power amp's presence (the top-boost amp's cut, turned the other way), and a master
    /// volume before the phase inverter (1: the circuit as built, without one).
    pub gain: f32,
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    pub presence: f32,
    pub master: f32,
    /// The supply's stiffness: 0.5 the amp's own rectifier, 0 three times stiffer, 1 three
    /// times softer.
    pub sag: f32,
    /// The cabinet on (it sounds once a kernel is supplied).
    pub cab: bool,
    /// 2, 4 or 8.
    pub oversampling: usize,
    /// Wet share, 0 … 1.
    pub mix: f32,
    pub out_db: f32,
}

impl Default for ArcaSettings {
    fn default() -> Self {
        ArcaSettings {
            model: None,
            gain: 0.5,
            bass: 0.5,
            mid: 0.5,
            treble: 0.5,
            presence: 0.5,
            master: 1.0,
            sag: 0.5,
            cab: false,
            oversampling: 4,
            mix: 1.0,
            out_db: 0.0,
        }
    }
}

impl ArcaSettings {
    /// The latency these settings give (base-rate samples).
    pub fn latency(&self) -> usize {
        match self.model {
            Some(_) => os::latency(factor(self.oversampling), false),
            None => 0,
        }
    }

    fn knobs(&self) -> Knobs {
        Knobs {
            gain: self.gain.clamp(0.0, 1.0) as f64,
            bass: self.bass.clamp(0.0, 1.0) as f64,
            mid: self.mid.clamp(0.0, 1.0) as f64,
            treble: self.treble.clamp(0.0, 1.0) as f64,
        }
    }
}

/// The oversampling factor for a setting (2, 4 or 8; anything else is 4).
pub fn factor(f: usize) -> usize {
    match f {
        2 | 4 | 8 => f,
        _ => 4,
    }
}

/// A gain that glides, landing exactly.
#[derive(Copy, Clone, Debug)]
struct Ramp {
    v: f32,
    to: f32,
}

impl Ramp {
    #[inline]
    fn next(&mut self, a: f32) -> f32 {
        if self.v != self.to {
            self.v += (self.to - self.v) * a;
            if (self.to - self.v).abs() < 1e-6 {
                self.v = self.to;
            }
        }
        self.v
    }
    fn still(&self) -> bool {
        self.v == self.to
    }
}

/// The master pot's attenuation for its rotation (an audio taper; 1 at full).
fn master_gain(m: f64) -> f64 {
    if m >= 1.0 {
        1.0
    } else {
        crate::fragor::circuits::taper_a(m)
    }
}

/// The supply resistance's factor for the sag setting (0.5 → 1).
fn sag_factor(s: f64) -> f64 {
    crate::math::powf(3.0f64, (s.clamp(0.0, 1.0) - 0.5) * 2.0)
}

pub struct Arca {
    sr: f32,
    set: ArcaSettings,
    factor: usize,
    /// The model the circuits hold.
    built: Option<AmpModel>,
    pre: Box<Preamp>,
    pow: Box<Power>,
    os: os::Oversampler,
    /// The knobs as the circuits have them (they move on the grid toward the settings).
    knobs: Knobs,
    master: f64,
    presence: f64,
    sag: f64,
    conv: Option<Convolver>,
    dry: [[f32; DELAY_MAX]; 2],
    dpos: usize,
    latency: usize,
    /// The amp's level through a model change (0 … FADE).
    fade: u32,
    mix: Ramp,
    out: Ramp,
    /// Samples since the stream's start (the knobs' grid).
    t: u64,
    /// Silent input samples in a row, and samples since the amp's output was last above
    /// the tail's floor.
    quiet: u32,
    since_loud: u32,
    /// The circuits sit at their operating points (silence in is silence out).
    snapped: bool,
    wet: [f32; CHUNK],
    /// Telemetry: the supply's sag (V below idle) and the output's peak in the last block.
    sag_volts: f32,
    peak: f32,
}

impl Arca {
    /// Arca at `sample_rate`, no amp, no cabinet (the null). Allocates (off the audio
    /// thread): the circuits' room.
    pub fn new(sample_rate: f32) -> Arca {
        let s = ArcaSettings::default();
        let f = factor(s.oversampling);
        let rate = sample_rate as f64 * f as f64;
        let model = AmpModel::Clean;
        Arca {
            sr: sample_rate,
            set: s,
            factor: f,
            built: None,
            pre: Box::new(Preamp::new(model.preamp(), SOURCE_OHMS, rate, &s.knobs())),
            pow: Box::new(Power::new(model.power(), rate)),
            os: os::Oversampler::new(f, false),
            knobs: s.knobs(),
            master: 1.0,
            presence: s.presence as f64,
            sag: 1.0,
            conv: None,
            dry: [[0.0; DELAY_MAX]; 2],
            dpos: 0,
            latency: 0,
            fade: FADE,
            mix: Ramp { v: 1.0, to: 1.0 },
            out: Ramp { v: 1.0, to: 1.0 },
            t: 0,
            quiet: u32::MAX / 2,
            since_loud: u32::MAX / 2,
            snapped: true,
            wet: [0.0; CHUNK],
            sag_volts: 0.0,
            peak: 0.0,
        }
    }

    /// With a cabinet of this kernel (allocates its state: off the audio thread).
    pub fn with_cab(sample_rate: f32, kernel: Arc<Kernel>) -> Arca {
        let mut a = Arca::new(sample_rate);
        let state = Box::new(ConvState::new(kernel.cap()));
        a.conv = Some(Convolver::new(state, kernel));
        a
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    pub fn settings(&self) -> &ArcaSettings {
        &self.set
    }

    /// The cabinet's response, swapped in without a click (`state` must hold it: see
    /// [`Convolver::supply`]); the first supplies the convolver.
    pub fn supply_cab(&mut self, kernel: Arc<Kernel>, state: Option<Box<ConvState>>) -> Supplied {
        match &mut self.conv {
            Some(c) => c.supply(kernel, state),
            None => match state {
                Some(st) if st.cap().holds(&kernel.cap()) => {
                    self.conv = Some(Convolver::new(st, kernel));
                    Ok(None)
                }
                other => Err((kernel, other)),
            },
        }
    }

    /// Whether a cabinet response is held.
    pub fn has_cab(&self) -> bool {
        self.conv.is_some()
    }

    pub fn set(&mut self, s: &ArcaSettings) {
        let f = factor(s.oversampling);
        if f != self.factor {
            self.factor = f;
            self.os.configure(f, false);
            self.os.reset();
            // The circuits are rebuilt for the new rate when next built.
            self.built = None;
            self.fade = 0;
        }
        self.set = *s;
        self.mix.to = s.mix.clamp(0.0, 1.0);
        self.out.to = if s.out_db == 0.0 {
            1.0
        } else {
            crate::math::db_to_gain(s.out_db)
        };
        self.latency = s.latency();
    }

    pub fn latency(&self) -> usize {
        self.set.latency()
    }

    fn rate(&self) -> f64 {
        self.sr as f64 * self.factor as f64
    }

    /// Build the circuits for `model` at the current rate and knobs, at rest
    /// (allocation-free: in place).
    fn build(&mut self, model: AmpModel) {
        let rate = self.rate();
        self.knobs = self.set.knobs();
        self.pre
            .build(model.preamp(), SOURCE_OHMS, rate, &self.knobs);
        *self.pow = Power::new(model.power(), rate);
        self.presence = self.set.presence as f64;
        self.pow.set_presence(self.presence);
        self.sag = sag_factor(self.set.sag as f64);
        self.pow.set_sag(self.sag);
        self.master = master_gain(self.set.master as f64);
        self.os.reset();
        self.built = Some(model);
        self.snapped = true;
    }

    /// Back to silence at the stream's start: every glide landed, the circuits built and
    /// at rest.
    pub fn reset(&mut self) {
        if let Some(m) = self.set.model {
            self.build(m);
        } else {
            self.built = None;
        }
        self.fade = FADE;
        self.mix.v = self.mix.to;
        self.out.v = self.out.to;
        self.dry = [[0.0; DELAY_MAX]; 2];
        self.dpos = 0;
        self.os.reset();
        if let Some(c) = &mut self.conv {
            c.reset();
        }
        self.t = 0;
        self.quiet = u32::MAX / 2;
        self.since_loud = u32::MAX / 2;
        self.snapped = true;
    }

    /// The knobs a step toward the settings, on the grid (rebuilds the circuits'
    /// matrices while they move).
    fn update(&mut self) {
        let target = self.set.knobs();
        let mut moved = false;
        let glide = |v: &mut f64, to: f64, moved: &mut bool| {
            if *v != to {
                *moved = true;
                let d = to - *v;
                *v = if d.abs() < 1e-3 { to } else { *v + 0.35 * d };
            }
        };
        let mut k = self.knobs;
        glide(&mut k.gain, target.gain, &mut moved);
        glide(&mut k.bass, target.bass, &mut moved);
        glide(&mut k.mid, target.mid, &mut moved);
        glide(&mut k.treble, target.treble, &mut moved);
        if moved {
            self.knobs = k;
            self.pre.set_knobs(&k);
        }
        let mut p = self.presence;
        let mut unused = false;
        glide(&mut p, self.set.presence.clamp(0.0, 1.0) as f64, &mut unused);
        if p != self.presence {
            self.presence = p;
            self.pow.set_presence(p);
        }
        let mut m = self.master;
        glide(&mut m, master_gain(self.set.master as f64), &mut unused);
        self.master = m;
        let sf = sag_factor(self.set.sag as f64);
        if sf != self.sag {
            self.sag = sf;
            self.pow.set_sag(sf);
        }
    }

    fn knobs_still(&self) -> bool {
        self.knobs == self.set.knobs()
            && self.presence == self.set.presence.clamp(0.0, 1.0) as f64
            && self.master == master_gain(self.set.master as f64)
    }

    /// Whether the amp's structure is the settings' (no model change under way).
    fn structure_is_target(&self) -> bool {
        self.built == self.set.model && self.fade == FADE
    }

    #[inline]
    fn amp_sample(&mut self, x: f32) -> f32 {
        let mut v = [0.0f32; 8];
        self.os.up(x, &mut v);
        let master = self.master;
        for s in v.iter_mut().take(self.factor) {
            let p = self.pre.step(*s as f64);
            *s = self.pow.step(p * master) as f32;
        }
        self.os.down(&mut v)
    }

    fn snap(&mut self) {
        self.pre.rest();
        self.pow.rest();
        self.os.reset();
        self.snapped = true;
    }
}

impl Arca {
    fn cab_on(&self) -> bool {
        self.set.cab && self.conv.is_some()
    }

    /// One sample's bookkeeping before the amp: the model's change (a fade out, the new
    /// circuits, a fade in), the knobs' grid, the silence count; the dry line.
    #[inline]
    fn begin(&mut self, a: f32, b: f32) -> (f32, f32) {
        if self.built != self.set.model {
            if self.fade > 0 {
                self.fade -= 1;
            } else {
                match self.set.model {
                    Some(m) => self.build(m),
                    None => self.built = None,
                }
            }
        } else if self.fade < FADE {
            self.fade += 1;
        }
        if self.t.is_multiple_of(GRID) && self.built.is_some() {
            self.update();
        }
        self.t += 1;
        if a == 0.0 && b == 0.0 {
            self.quiet = self.quiet.saturating_add(1);
        } else {
            self.quiet = 0;
        }
        let p = self.dpos;
        self.dry[0][p] = a;
        self.dry[1][p] = b;
        let r = (p + DELAY_MAX - self.latency.min(DELAY_MAX - 1)) % DELAY_MAX;
        self.dpos = (p + 1) % DELAY_MAX;
        (self.dry[0][r], self.dry[1][r])
    }

    /// The amp on one sample (or the cabinet's input alone, without an amp), with the
    /// rest bookkeeping.
    #[inline]
    fn wet_sample(&mut self, x: f32) -> f32 {
        let w = match self.built {
            Some(_) => {
                if self.snapped && self.quiet > 0 {
                    0.0
                } else {
                    self.snapped = false;
                    let y = self.amp_sample(x);
                    if y.abs() >= TAIL_FLOOR {
                        self.since_loud = 0;
                    } else {
                        self.since_loud = self.since_loud.saturating_add(1);
                    }
                    let settle = (SETTLE_S * self.sr) as u32;
                    let most = (SETTLE_MAX_S * self.sr) as u32;
                    if self.quiet >= settle && (self.since_loud >= QUIET_TAIL || self.quiet >= most)
                    {
                        self.snap();
                    }
                    y
                }
            }
            None => x,
        };
        if self.fade < FADE {
            w * (self.fade as f32 / FADE as f32)
        } else {
            w
        }
    }

    /// Process a stereo block in place.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let n = left.len().min(right.len());
        // The null: no amp, no cabinet — the signal untouched, to the bit.
        if self.set.model.is_none()
            && self.built.is_none()
            && !self.cab_on()
            && self.fade == FADE
            && self.out.v == 1.0
            && self.out.still()
        {
            for i in 0..n {
                self.begin(left[i], right[i]);
            }
            self.peak = 0.0;
            return;
        }
        let mut dl = [0.0f32; CHUNK];
        let mut dr = [0.0f32; CHUNK];
        let mut peak = 0.0f32;
        let mut i = 0;
        while i < n {
            let m = (n - i).min(CHUNK);
            for j in 0..m {
                let (a, b) = (left[i + j], right[i + j]);
                let (da, db) = self.begin(a, b);
                dl[j] = da;
                dr[j] = db;
                self.wet[j] = self.wet_sample(0.5 * (a + b));
            }
            let (l, r) = (&mut left[i..i + m], &mut right[i..i + m]);
            l.copy_from_slice(&self.wet[..m]);
            r.copy_from_slice(&self.wet[..m]);
            if self.cab_on() {
                if let Some(c) = &mut self.conv {
                    c.process(l, r);
                }
            }
            for j in 0..m {
                let mix = self.mix.next(0.002);
                let out = self.out.next(0.002);
                let (mut ya, mut yb) = (l[j], r[j]);
                if mix != 1.0 {
                    ya = dl[j] * (1.0 - mix) + ya * mix;
                    yb = dr[j] * (1.0 - mix) + yb * mix;
                }
                if out != 1.0 {
                    ya *= out;
                    yb *= out;
                }
                peak = peak.max(ya.abs()).max(yb.abs());
                l[j] = ya;
                r[j] = yb;
            }
            i += m;
        }
        self.peak = peak;
        self.sag_volts = match self.built {
            Some(_) => (self.pow.bias_and_supply().1 - self.pow.supply()) as f32,
            None => 0.0,
        };
    }

    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    pub fn rest_blocker(&self) -> Option<&'static str> {
        if !self.structure_is_target() {
            return Some("the amp is changing model");
        }
        if self.built.is_some() && !self.knobs_still() {
            return Some("a knob is moving");
        }
        if !self.mix.still() || !self.out.still() {
            return Some("a level is gliding");
        }
        if (self.quiet as usize) < DELAY_MAX {
            return Some("the dry line holds sound");
        }
        if self.built.is_some() && !self.snapped {
            return Some("the amp's tail is still sounding");
        }
        if self.cab_on() {
            if let Some(c) = &self.conv {
                return c.rest_blocker();
            }
        }
        None
    }

    /// `frames` of silence skipped while at rest: the grid and the counts move on.
    pub fn skip(&mut self, frames: usize) {
        self.t += frames as u64;
        let f = frames.min(u32::MAX as usize) as u32;
        self.quiet = self.quiet.saturating_add(f);
        self.since_loud = self.since_loud.saturating_add(f);
        if self.cab_on() {
            if let Some(c) = &mut self.conv {
                c.skip(frames);
            }
        }
        self.dpos = (self.dpos + frames) % DELAY_MAX;
    }

    /// The supply's sag below idle (V) and the output's peak, at the last block.
    pub fn meters(&self) -> (f32, f32) {
        (self.sag_volts, self.peak)
    }
}

#[cfg(test)]
#[path = "arca_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "arca_spice_tests.rs"]
mod spice_tests;
