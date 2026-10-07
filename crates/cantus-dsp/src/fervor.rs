//! Fervor: saturation and colour, for a track or a whole mix. Seven models — tape, tube,
//! drive, fuzz, clip, fold and a curve you draw — oversampled up to 8× with linear- or
//! minimum-phase filters, with a bias for even harmonics, tone around the curve, bit and
//! rate reduction, mid/side, dry/wet, and an output matched in level so you hear the colour
//! and not the loudness.
//!
//! ## The signal path, per channel
//!
//! ```text
//! in ─┬─ low cut ─ emphasis ─ ↑os ─ curve ─ ↓os ─ de-emphasis ─ DC block ─ crush ─ tone ─ high cut ─ × match ─┐
//!     └─ delay (the latency) ──────────────────────────────────────────────────────────────── dry ─ mix ───────┴─ × out
//! ```
//!
//! - **The curve** is [`shape::Shaper`]: drive is a gain into a curve whose knee comes down
//!   from infinity as the drive rises, so 0 dB is a wire to the bit and there is no step at
//!   the first fraction of a dB (the formula is in `fervor_shape.rs`).
//! - **Emphasis** is where a model bites: tape lifts the highs before the curve and takes
//!   them back after it, so the highs saturate first — the level-dependent high-end loss of
//!   tape; drive lowers the lows before and restores them after, so the low end stays clean
//!   (an overdrive pedal's mid hump). The two filters are exact inverses.
//! - **Oversampling** ([`os`]) runs the curve 2×, 4× or 8× faster, so the harmonics above
//!   Nyquist are filtered out instead of folding back as aliasing. The editor shows what
//!   folds back, in red, so the choice is made by seeing.
//! - **Crush** reduces the bits (continuous: 12.5 bits is a step between 12 and 13) and holds
//!   samples at a lower rate (the aliasing is the point, so it runs after the down-sampler).
//! - **The level match** takes back the curve's change in loudness at a reference level
//!   (a sine at −18 dBFS RMS, [`shape::REF_PEAK`]): static, so it never pumps.
//!
//! ## On a grid, and the null
//!
//! Every setting is read on a grid of [`UPDATE`] samples counted from the start of the
//! stream, and continuous ones glide (20 ms) — so the output does not depend on the block
//! size, and nothing zips. A change of model or of the drawn curve crossfades over 10 ms.
//!
//! With every parameter at its default the unit takes a path that copies its input to the
//! output — multiplied by an output gain of exactly 1 — so it is a wire to the bit; the
//! same holds whenever nothing would colour the signal (no drive on an analytic model, no
//! filter, no crush, 1×). States below 10⁻²⁰ are flushed: silence stays silent.

use crate::aequalis::{design, Coefs, EqShape, MAX_SECTIONS};

/// The oversamplers.
#[path = "fervor_os.rs"]
pub mod os;
/// The curves.
#[path = "fervor_shape.rs"]
pub mod shape;

pub use shape::{CurveSpec, Model, Point, Shaper, LUT_N};

/// Settings are read every this many samples, counted from the start of the stream.
pub const UPDATE: usize = 16;
/// How long a continuous setting takes to glide to a new value.
pub const GLIDE_MS: f32 = 20.0;
/// How long a change of model or of the drawn curve crossfades.
pub const FADE_MS: f32 = 10.0;
/// The longest dry delay (the latency of 8× linear phase is 57 samples).
const DELAY_MAX: usize = 128;
/// The DC blocker's corner.
const DC_HZ: f64 = 5.0;
/// Tone's pivot.
pub const TONE_HZ: f32 = 1_000.0;
/// Where tape's and drive's emphasis sit, and how far each goes at 100 % character.
pub const TAPE_EMPH_HZ: f32 = 3_000.0;
pub const TAPE_EMPH_DB: f32 = 12.0;
pub const DRIVE_EMPH_HZ: f32 = 700.0;
pub const DRIVE_EMPH_DB: f32 = 18.0;
/// The ×100 switch: 40 dB.
pub const BOOST_DB: f32 = 40.0;

/// What is coloured.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Channels {
    Stereo,
    Mid,
    Side,
}

impl Channels {
    pub fn from_index(i: usize) -> Channels {
        match i {
            1 => Channels::Mid,
            2 => Channels::Side,
            _ => Channels::Stereo,
        }
    }
}

/// Fervor's settings, in the units a musician reads.
#[derive(Clone, Debug, PartialEq)]
pub struct SatSettings {
    pub model: Model,
    pub drive_db: f32,
    pub boost: bool,
    /// The model's character, 0 … 1.
    pub character: f32,
    /// −1 … 1.
    pub bias: f32,
    pub lo_cut_hz: f32,
    pub hi_cut_hz: f32,
    pub tone_db: f32,
    /// 1, 2, 4 or 8.
    pub oversampling: usize,
    pub minimum_phase: bool,
    pub bits: f32,
    pub rate_hz: f32,
    pub channels: Channels,
    pub mix: f32,
    pub auto_gain: bool,
    pub out_db: f32,
    pub curve: CurveSpec,
}

impl Default for SatSettings {
    fn default() -> Self {
        SatSettings {
            model: Model::Tape,
            drive_db: 0.0,
            boost: false,
            character: 0.5,
            bias: 0.0,
            lo_cut_hz: 20.0,
            hi_cut_hz: 20_000.0,
            tone_db: 0.0,
            oversampling: 1,
            minimum_phase: false,
            bits: 24.0,
            rate_hz: 50_000.0,
            channels: Channels::Stereo,
            mix: 1.0,
            auto_gain: true,
            out_db: 0.0,
            curve: CurveSpec::default(),
        }
    }
}

impl SatSettings {
    /// The drive the curve sees: the knob, and 40 dB more with ×100.
    pub fn total_drive_db(&self) -> f32 {
        self.drive_db.max(0.0) + if self.boost { BOOST_DB } else { 0.0 }
    }

    pub fn lo_cut_on(&self) -> bool {
        lo_on(self.lo_cut_hz)
    }

    pub fn hi_cut_on(&self) -> bool {
        hi_on(self.hi_cut_hz)
    }

    pub fn bits_on(&self) -> bool {
        bits_on(self.bits)
    }

    pub fn rate_on(&self, sample_rate: f32) -> bool {
        rate_on(self.rate_hz, sample_rate)
    }

    /// The model's emphasis: a shelf before the curve (its exact inverse after), as
    /// (shape, frequency, gain); `None` when it is flat.
    pub fn emphasis(&self) -> Option<(EqShape, f32, f32)> {
        emphasis(self.model, self.character)
    }

    /// The latency at these settings, in samples.
    pub fn latency(&self) -> usize {
        os::latency(self.oversampling, self.minimum_phase)
    }
}

fn lo_on(hz: f32) -> bool {
    hz > 20.5
}

fn hi_on(hz: f32) -> bool {
    hz < 19_999.0
}

fn bits_on(bits: f32) -> bool {
    bits < 23.99
}

fn rate_on(hz: f32, sr: f32) -> bool {
    hz < 49_999.0 && hz < sr
}

fn emphasis(model: Model, ch: f32) -> Option<(EqShape, f32, f32)> {
    let e = match model {
        Model::Tape => (EqShape::HighShelf, TAPE_EMPH_HZ, ch * TAPE_EMPH_DB),
        Model::Drive => (EqShape::LowShelf, DRIVE_EMPH_HZ, -ch * DRIVE_EMPH_DB),
        _ => return None,
    };
    (e.2.abs() > 1e-4).then_some(e)
}

/// One biquad section, transposed direct form II (Aequalis's convention), its state in
/// f64: a 30 Hz high-pass in f32 lifts its own rounding noise to about −100 dB at low
/// frequencies, which the harmonics display would show as red.
#[derive(Copy, Clone, Debug)]
struct Biquad {
    /// b0, b1, b2, a1, a2.
    c: [f64; 5],
    z1: f64,
    z2: f64,
}

impl Default for Biquad {
    fn default() -> Self {
        Biquad {
            c: [1.0, 0.0, 0.0, 0.0, 0.0],
            z1: 0.0,
            z2: 0.0,
        }
    }
}

impl Biquad {
    fn set(&mut self, c: Coefs) {
        self.c = [
            c.b0 as f64,
            c.b1 as f64,
            c.b2 as f64,
            c.a1 as f64,
            c.a2 as f64,
        ];
    }

    #[inline]
    fn tick(&mut self, x: f32) -> f32 {
        let [b0, b1, b2, a1, a2] = self.c;
        let x = x as f64;
        let y = b0 * x + self.z1;
        self.z1 = flush64(b1 * x - a1 * y + self.z2);
        self.z2 = flush64(b2 * x - a2 * y);
        y as f32
    }

    /// The exact inverse of a minimum-phase section (numerator and denominator swapped),
    /// computed from the same f64 values the section runs with.
    fn set_inverse(&mut self, c: Coefs) {
        let k = 1.0 / c.b0 as f64;
        self.c = [
            k,
            c.a1 as f64 * k,
            c.a2 as f64 * k,
            c.b1 as f64 * k,
            c.b2 as f64 * k,
        ];
    }

    fn clear(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

#[inline]
fn flush64(x: f64) -> f64 {
    if x.abs() < 1e-20 {
        0.0
    } else {
        x
    }
}

/// The one section of a shape (shelves, tilt, 12 dB/oct cuts are all one); identity when
/// the design is a wire.
fn one_section(shape: EqShape, sr: f32, hz: f32, db: f32, q: f32) -> Coefs {
    let mut out = [Coefs::IDENTITY; MAX_SECTIONS];
    let n = design(shape, sr, hz, db, q, 12.0, &mut out);
    if n == 0 {
        Coefs::IDENTITY
    } else {
        out[0]
    }
}

/// One channel's state on the wet path.
#[derive(Clone, Debug)]
struct Chan {
    lo: Biquad,
    pre: Biquad,
    post: Biquad,
    tone: Biquad,
    hi: Biquad,
    dc_x1: f64,
    dc_y1: f64,
    hold: f32,
    os: os::Oversampler,
    /// The dry path's delay (the latency).
    delay: [f32; DELAY_MAX],
}

impl Chan {
    fn new() -> Chan {
        Chan {
            lo: Biquad::default(),
            pre: Biquad::default(),
            post: Biquad::default(),
            tone: Biquad::default(),
            hi: Biquad::default(),
            dc_x1: 0.0,
            dc_y1: 0.0,
            hold: 0.0,
            os: os::Oversampler::new(1, false),
            delay: [0.0; DELAY_MAX],
        }
    }

    /// The wet path is empty, as [`Self::clear_wet`] leaves it.
    fn wet_clear(&self) -> bool {
        [&self.lo, &self.pre, &self.post, &self.tone, &self.hi]
            .iter()
            .all(|b| crate::rest::z64(b.z1) && crate::rest::z64(b.z2))
            && crate::rest::z64(self.dc_x1)
            && crate::rest::z64(self.dc_y1)
            && crate::rest::z(self.hold)
            && self.os.is_clear()
    }

    /// Clear the wet path (the dry delay keeps running).
    fn clear_wet(&mut self) {
        for b in [
            &mut self.lo,
            &mut self.pre,
            &mut self.post,
            &mut self.tone,
            &mut self.hi,
        ] {
            b.clear();
        }
        self.dc_x1 = 0.0;
        self.dc_y1 = 0.0;
        self.hold = 0.0;
        self.os.reset();
    }
}

/// The settings as they are now, gliding toward the target.
#[derive(Copy, Clone, Debug, PartialEq)]
struct Live {
    drive_db: f32,
    ch: f32,
    bias: f32,
    tone_db: f32,
    lo_hz: f32,
    hi_hz: f32,
    mix: f32,
    out_db: f32,
}

impl Live {
    fn of(s: &SatSettings) -> Live {
        Live {
            drive_db: s.total_drive_db(),
            ch: s.character,
            bias: s.bias,
            tone_db: s.tone_db,
            lo_hz: s.lo_cut_hz,
            hi_hz: s.hi_cut_hz,
            mix: s.mix,
            out_db: s.out_db,
        }
    }
}

/// One step of a glide: a one-pole toward the target that lands on it exactly once it is
/// within `tol`.
#[inline]
fn glide(v: &mut f32, target: f32, alpha: f32, tol: f32) -> bool {
    if *v == target {
        return false;
    }
    let d = target - *v;
    if d.abs() <= tol {
        *v = target;
    } else {
        *v += d * alpha;
    }
    true
}

/// A glide in octaves, for frequencies.
#[inline]
fn glide_hz(v: &mut f32, target: f32, alpha: f32) -> bool {
    if *v == target {
        return false;
    }
    let r = (target / v.max(1.0)).log2();
    if r.abs() <= 1e-3 {
        *v = target;
    } else {
        *v *= (r * alpha).exp2();
    }
    true
}

/// Fervor, stereo.
#[derive(Clone, Debug)]
pub struct Fervor {
    sr: f32,
    target: SatSettings,
    live: Live,
    model: Model,
    /// The oversampling now (it changes on the grid).
    factor: usize,
    minimum: bool,
    latency: usize,
    alpha: f32,
    shaper: Shaper,
    old: Shaper,
    /// Samples of crossfade left from `old` to `shaper`, and its length.
    fade: usize,
    fade_len: usize,
    lut: Box<[f32; LUT_N + 1]>,
    lut_old: Box<[f32; LUT_N + 1]>,
    /// The drawn curve the table was made from.
    curve: CurveSpec,
    chans: [Chan; 2],
    /// Samples into the current grid step.
    grid: usize,
    engaged: bool,
    /// The level match now, and the gains, interpolated across each grid step.
    comp: f32,
    wet_g: f32,
    wet_end: f32,
    wet_step: f32,
    out_g: f32,
    out_end: f32,
    out_step: f32,
    mix_v: f32,
    mix_end: f32,
    mix_step: f32,
    dc_r: f64,
    /// Sample-and-hold phase, shared by both channels.
    hold_phase: f32,
    pos: usize,
    /// Telemetry of the last block.
    tel: Meter,
    blocks: u32,
}

#[derive(Copy, Clone, Debug, Default)]
struct Meter {
    in_peak: f32,
    out_peak: f32,
    in_sq: f64,
    out_sq: f64,
    n: usize,
}

impl Fervor {
    pub fn new(sample_rate: f32) -> Fervor {
        let sr = sample_rate.max(1.0);
        let target = SatSettings::default();
        let mut lut = Box::new([0.0f32; LUT_N + 1]);
        target.curve.table(&mut lut);
        let mut f = Fervor {
            sr,
            live: Live::of(&target),
            model: target.model,
            factor: 1,
            minimum: false,
            latency: 0,
            alpha: 1.0 - (-(UPDATE as f32) / (GLIDE_MS * 0.001 * sr)).exp(),
            shaper: Shaper::WIRE,
            old: Shaper::WIRE,
            fade: 0,
            fade_len: ((FADE_MS * 0.001 * sr) as usize).max(1),
            lut_old: lut.clone(),
            lut,
            curve: target.curve.clone(),
            chans: [Chan::new(), Chan::new()],
            grid: 0,
            engaged: false,
            comp: 1.0,
            wet_g: 1.0,
            wet_end: 1.0,
            wet_step: 0.0,
            out_g: 1.0,
            out_end: 1.0,
            out_step: 0.0,
            mix_v: 1.0,
            mix_end: 1.0,
            mix_step: 0.0,
            dc_r: 1.0 - std::f64::consts::TAU * DC_HZ / sr as f64,
            hold_phase: 0.0,
            pos: 0,
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

    /// New settings: they take effect at the next grid point, continuous ones gliding.
    /// Allocation-free.
    pub fn set(&mut self, s: &SatSettings) {
        self.target.clone_from(s);
    }

    pub fn settings(&self) -> &SatSettings {
        &self.target
    }

    /// The delay the unit adds, in samples (the oversampling filters'). It follows the
    /// settings as soon as they are set, as delay compensation needs.
    pub fn latency(&self) -> usize {
        self.target.latency()
    }

    /// Back to silence, every glide landed on its target.
    pub fn reset(&mut self) {
        self.live = Live::of(&self.target);
        self.model = self.target.model;
        self.factor = self.target.oversampling;
        self.minimum = self.target.minimum_phase;
        self.latency = os::latency(self.factor, self.minimum);
        for c in &mut self.chans {
            c.os.configure(self.factor, self.minimum);
            c.clear_wet();
            c.delay = [0.0; DELAY_MAX];
        }
        if self.curve != self.target.curve {
            self.curve.clone_from(&self.target.curve);
            self.curve.table(&mut self.lut);
        }
        self.fade = 0;
        self.grid = 0;
        self.pos = 0;
        self.hold_phase = 0.0;
        self.shaper = self.make_shaper();
        self.comp = self.shaper.level_match(&self.lut[..]);
        self.design_filters();
        self.engaged = !self.neutral();
        let (w, o, m) = self.gains();
        (self.wet_g, self.wet_end, self.wet_step) = (w, w, 0.0);
        (self.out_g, self.out_end, self.out_step) = (o, o, 0.0);
        (self.mix_v, self.mix_end, self.mix_step) = (m, m, 0.0);
    }

    /// At rest (ADR 0031 §5.1): what [`Self::reset`] leaves for these settings — every
    /// glide landed, the structure (model, curve, oversampling) the target's, no crossfade,
    /// the gains still — with the dry delay all zeros and, when the wet path runs, every
    /// filter, the DC blocker, the hold and the oversampler empty. A disengaged wet path
    /// does not count: it is cleared when it engages. Never at rest while the sample-and-hold
    /// runs: its phase follows the history of the settings, not the sample count.
    pub fn at_rest(&self) -> bool {
        let t = &self.target;
        if self.fade != 0
            || rate_on(t.rate_hz, self.sr)
            || self.live != Live::of(t)
            || self.model != t.model
            || self.factor != t.oversampling
            || self.minimum != t.minimum_phase
            || self.engaged == self.neutral()
            || self.curve != t.curve
        {
            return false;
        }
        let (w, o, m) = self.gains();
        let still = |v: f32, end: f32, step: f32, to: f32| v == to && end == to && step == 0.0;
        still(self.wet_g, self.wet_end, self.wet_step, w)
            && still(self.out_g, self.out_end, self.out_step, o)
            && still(self.mix_v, self.mix_end, self.mix_step, m)
            && self
                .chans
                .iter()
                .all(|c| crate::rest::all_zero(&c.delay) && (!self.engaged || c.wet_clear()))
    }

    /// Samples of silence skipped while at rest (ADR 0031 §5.1): the grid's position moves
    /// on, so a setting that moves later glides on the same samples as it would have.
    pub fn skip(&mut self, frames: usize) {
        self.grid = (self.grid + frames % UPDATE) % UPDATE;
    }

    fn make_shaper(&self) -> Shaper {
        Shaper::new(
            self.model,
            self.live.drive_db as f64,
            self.live.ch as f64,
            self.live.bias as f64,
            &self.lut[..],
        )
    }

    /// The wet gain (the level match), the output gain and the mix the settings ask for.
    fn gains(&self) -> (f32, f32, f32) {
        let wet = if self.target.auto_gain {
            self.comp
        } else {
            1.0
        };
        (
            wet,
            10f32.powf(self.live.out_db / 20.0),
            self.live.mix.clamp(0.0, 1.0),
        )
    }

    /// Whether nothing would colour the signal: then the unit is a wire (times the output).
    fn neutral(&self) -> bool {
        self.factor == 1
            && self.fade == 0
            && self.shaper.is_wire()
            && !lo_on(self.live.lo_hz)
            && !hi_on(self.live.hi_hz)
            && self.live.tone_db == 0.0
            && !bits_on(self.target.bits)
            && !rate_on(self.target.rate_hz, self.sr)
    }

    fn design_filters(&mut self) {
        let sr = self.sr;
        let lo = one_section(EqShape::LowCut, sr, self.live.lo_hz, 0.0, 0.707);
        let hi = one_section(EqShape::HighCut, sr, self.live.hi_hz, 0.0, 0.707);
        let tone = one_section(EqShape::TiltShelf, sr, TONE_HZ, self.live.tone_db, 0.5);
        let pre = match emphasis(self.model, self.live.ch) {
            Some((shape, hz, db)) => one_section(shape, sr, hz, db, 0.707),
            None => Coefs::IDENTITY,
        };
        for c in &mut self.chans {
            c.lo.set(lo);
            c.hi.set(hi);
            c.tone.set(tone);
            c.pre.set(pre);
            c.post.set_inverse(pre);
        }
    }

    /// A grid point: read the target, glide, redesign what moved.
    fn update(&mut self) {
        let t = &self.target;
        if t.oversampling != self.factor || t.minimum_phase != self.minimum {
            self.factor = os::FACTORS
                .iter()
                .copied()
                .find(|f| *f == t.oversampling)
                .unwrap_or(1);
            self.minimum = t.minimum_phase;
            self.latency = os::latency(self.factor, self.minimum);
            for c in &mut self.chans {
                c.os.configure(self.factor, self.minimum);
            }
        }
        let curve_changed = self.curve != t.curve;
        let model_changed = t.model != self.model;
        let mut shaper_dirty = false;
        let mut filters_dirty = false;
        if (model_changed || (curve_changed && self.model == Model::Curve)) && self.engaged {
            // Crossfade from what is playing now.
            self.lut_old.copy_from_slice(&self.lut[..]);
            self.old = self.shaper;
            self.fade = self.fade_len;
        }
        if curve_changed {
            self.curve.clone_from(&t.curve);
            self.curve.table(&mut self.lut);
            shaper_dirty = true;
        }
        if model_changed {
            self.model = t.model;
            shaper_dirty = true;
            filters_dirty = true;
        }
        let a = self.alpha;
        let tl = Live::of(t);
        let l = &mut self.live;
        shaper_dirty |= glide(&mut l.drive_db, tl.drive_db, a, 0.005);
        let ch_moved = glide(&mut l.ch, tl.ch, a, 1e-4);
        shaper_dirty |= ch_moved | glide(&mut l.bias, tl.bias, a, 1e-4);
        filters_dirty |= ch_moved && emphasis(self.model, 1.0).is_some();
        filters_dirty |= glide(&mut l.tone_db, tl.tone_db, a, 0.005);
        filters_dirty |= glide_hz(&mut l.lo_hz, tl.lo_hz, a);
        filters_dirty |= glide_hz(&mut l.hi_hz, tl.hi_hz, a);
        glide(&mut l.mix, tl.mix, a, 1e-4);
        glide(&mut l.out_db, tl.out_db, a, 0.005);
        if shaper_dirty {
            self.shaper = self.make_shaper();
            self.comp = self.shaper.level_match(&self.lut[..]);
        }
        if filters_dirty {
            self.design_filters();
        }
        let engaged = !self.neutral();
        if engaged && !self.engaged {
            for c in &mut self.chans {
                c.clear_wet();
            }
        }
        self.engaged = engaged;
        let (w, o, m) = self.gains();
        let n = UPDATE as f32;
        self.wet_g = self.wet_end;
        self.wet_end = w;
        self.wet_step = (w - self.wet_g) / n;
        self.out_g = self.out_end;
        self.out_end = o;
        self.out_step = (o - self.out_g) / n;
        self.mix_v = self.mix_end;
        self.mix_end = m;
        self.mix_step = (m - self.mix_v) / n;
    }

    /// Process a stereo block in place.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let n = left.len().min(right.len());
        let mut m = Meter::default();
        for i in 0..n {
            if self.grid == 0 {
                self.update();
            }
            self.grid = (self.grid + 1) % UPDATE;
            let (xl, xr) = (left[i], right[i]);
            m.in_peak = m.in_peak.max(xl.abs()).max(xr.abs());
            m.in_sq += (xl * xl + xr * xr) as f64 * 0.5;
            let (yl, yr) = if self.engaged {
                self.tick(xl, xr)
            } else {
                self.write_dry(xl, xr);
                (xl, xr)
            };
            let g = self.out_g;
            let (ol, or) = (yl * g, yr * g);
            left[i] = ol;
            right[i] = or;
            m.out_peak = m.out_peak.max(ol.abs()).max(or.abs());
            m.out_sq += (ol * ol + or * or) as f64 * 0.5;
            self.out_g += self.out_step;
            self.wet_g += self.wet_step;
            self.mix_v += self.mix_step;
            if self.fade > 0 {
                self.fade -= 1;
            }
        }
        m.n = n;
        self.tel = m;
        self.blocks = (self.blocks + 1) % (1 << 24);
    }

    /// Write the dry path's delay; returns the delayed pair.
    #[inline]
    fn write_dry(&mut self, a: f32, b: f32) -> (f32, f32) {
        let p = self.pos;
        self.pos = (p + 1) % DELAY_MAX;
        let r = (p + DELAY_MAX - self.latency.min(DELAY_MAX - 1)) % DELAY_MAX;
        let [c0, c1] = &mut self.chans;
        c0.delay[p] = a;
        c1.delay[p] = b;
        (c0.delay[r], c1.delay[r])
    }

    /// One engaged sample.
    #[inline]
    fn tick(&mut self, xl: f32, xr: f32) -> (f32, f32) {
        let mode = self.target.channels;
        let (a, b) = match mode {
            Channels::Stereo => (xl, xr),
            _ => ((xl + xr) * 0.5, (xl - xr) * 0.5),
        };
        let (da, db) = self.write_dry(a, b);
        let rate = rate_on(self.target.rate_hz, self.sr);
        let take = if rate {
            self.hold_phase += self.target.rate_hz / self.sr;
            if self.hold_phase >= 1.0 {
                self.hold_phase -= 1.0;
                true
            } else {
                false
            }
        } else {
            true
        };
        let ctx = Ctx {
            shaper: &self.shaper,
            old: &self.old,
            fade: if self.fade > 0 {
                1.0 - self.fade as f32 / self.fade_len as f32
            } else {
                1.0
            },
            lut: &self.lut[..],
            lut_old: &self.lut_old[..],
            factor: self.factor,
            lo: lo_on(self.live.lo_hz),
            hi: hi_on(self.live.hi_hz),
            tone: self.live.tone_db != 0.0,
            emph: emphasis(self.model, self.live.ch).is_some(),
            bits: bits_on(self.target.bits).then(|| (1.0 - self.target.bits.max(1.0)).exp2()),
            hold: rate,
            take,
            dc_r: self.dc_r,
        };
        let (wg, mix) = (self.wet_g, self.mix_v);
        let blend = |wet: f32, dry: f32| {
            if mix >= 1.0 {
                wet
            } else {
                dry * (1.0 - mix) + wet * mix
            }
        };
        let [c0, c1] = &mut self.chans;
        let ya = if mode == Channels::Side {
            da
        } else {
            blend(wet(c0, a, &ctx) * wg, da)
        };
        let yb = if mode == Channels::Mid {
            db
        } else {
            blend(wet(c1, b, &ctx) * wg, db)
        };
        match mode {
            Channels::Stereo => (ya, yb),
            _ => (ya + yb, ya - yb),
        }
    }

    /// What the editor reads, refreshed after every block (layout: [`telemetry`]).
    pub fn write_telemetry(&self, t: &mut [f32; 64]) {
        use telemetry::*;
        let db = |v: f32| if v > 1e-9 { 20.0 * v.log10() } else { -120.0 };
        let rms = |sq: f64, n: usize| {
            if n == 0 {
                -120.0
            } else {
                let v = (sq / n as f64).sqrt() as f32;
                db(v)
            }
        };
        t[BLOCKS] = self.blocks as f32;
        t[IN_PEAK] = db(self.tel.in_peak);
        t[OUT_PEAK] = db(self.tel.out_peak);
        t[IN_RMS] = rms(self.tel.in_sq, self.tel.n);
        t[OUT_RMS] = rms(self.tel.out_sq, self.tel.n);
        t[MATCH] = if self.target.auto_gain {
            db(self.comp)
        } else {
            0.0
        };
        t[LATENCY] = self.latency as f32;
        t[SAMPLE_RATE] = self.sr;
        t[FACTOR] = self.factor as f32;
        t[ENGAGED] = self.engaged as u8 as f32;
        t[DRIVE] = self.live.drive_db;
        t[IN_LINEAR] = self.tel.in_peak;
    }
}

/// What one wet sample needs besides its channel.
struct Ctx<'a> {
    shaper: &'a Shaper,
    old: &'a Shaper,
    /// 1 = only `shaper`; below, the share of `shaper` in a crossfade from `old`.
    fade: f32,
    lut: &'a [f32],
    lut_old: &'a [f32],
    factor: usize,
    lo: bool,
    hi: bool,
    tone: bool,
    emph: bool,
    /// The quantisation step, when the bits are reduced.
    bits: Option<f32>,
    hold: bool,
    take: bool,
    dc_r: f64,
}

#[inline]
fn shape_at(ctx: &Ctx<'_>, v: f32) -> f32 {
    let now = ctx.shaper.apply(v, ctx.lut);
    if ctx.fade >= 1.0 {
        now
    } else {
        let was = ctx.old.apply(v, ctx.lut_old);
        was + (now - was) * ctx.fade
    }
}

/// One channel's wet sample.
#[inline]
fn wet(c: &mut Chan, x: f32, ctx: &Ctx<'_>) -> f32 {
    let mut x = x;
    if ctx.lo {
        x = c.lo.tick(x);
    }
    if ctx.emph {
        x = c.pre.tick(x);
    }
    let mut y = if ctx.factor == 1 {
        shape_at(ctx, x)
    } else {
        let mut buf = [0.0f32; 8];
        c.os.up(x, &mut buf);
        for v in buf.iter_mut().take(ctx.factor) {
            *v = shape_at(ctx, *v);
        }
        c.os.down(&mut buf)
    };
    if ctx.emph {
        y = c.post.tick(y);
    }
    let y64 = y as f64;
    let d = y64 - c.dc_x1 + ctx.dc_r * c.dc_y1;
    c.dc_x1 = y64;
    c.dc_y1 = flush64(d);
    y = c.dc_y1 as f32;
    if ctx.hold {
        if ctx.take {
            c.hold = y;
        }
        y = c.hold;
    }
    if let Some(step) = ctx.bits {
        y = (y / step).round() * step;
    }
    if ctx.tone {
        y = c.tone.tick(y);
    }
    if ctx.hi {
        y = c.hi.tick(y);
    }
    y
}

/// The telemetry's layout: what each of the 64 numbers is.
pub mod telemetry {
    /// Blocks processed since the unit was made, modulo 2²⁴.
    pub const BLOCKS: usize = 0;
    /// The last block's input and output peaks and RMS, dBFS (−120 for silence).
    pub const IN_PEAK: usize = 1;
    pub const OUT_PEAK: usize = 2;
    pub const IN_RMS: usize = 3;
    pub const OUT_RMS: usize = 4;
    /// The level match's gain, dB (0 when off).
    pub const MATCH: usize = 5;
    /// Latency, samples.
    pub const LATENCY: usize = 6;
    pub const SAMPLE_RATE: usize = 7;
    /// The oversampling factor running.
    pub const FACTOR: usize = 8;
    /// 1 when the wet path runs; 0 when nothing would colour the signal.
    pub const ENGAGED: usize = 9;
    /// The drive the curve sees now (gliding), dB.
    pub const DRIVE: usize = 10;
    /// The last block's input peak, linear (where the editor's ball sits on the curve).
    pub const IN_LINEAR: usize = 11;
}

/// The curve as the editor draws it: the shaper these settings make (no glide), and the
/// drawn curve's table.
pub fn static_curve(s: &SatSettings) -> (Shaper, Box<[f32; LUT_N + 1]>) {
    let mut lut = Box::new([0.0f32; LUT_N + 1]);
    s.curve.table(&mut lut);
    let shaper = Shaper::new(
        s.model,
        s.total_drive_db() as f64,
        s.character as f64,
        s.bias as f64,
        &lut[..],
    );
    (shaper, lut)
}

#[cfg(test)]
#[path = "fervor_tests.rs"]
mod tests;
