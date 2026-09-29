//! One wavetable oscillator in one voice: up to 16 unison lanes, each with its own u32
//! phase (exact, and it wraps for free), its own mip level and frame pair, its own pan,
//! two warp slots, all read with the table's cubic interpolator.
//!
//! The lanes are set on the control grid ([`Osc::control`]): pitch, level, position, pan;
//! the per-sample loop ([`Osc::tick`]) only advances phases, ramps increments and gains,
//! warps and reads.

use super::table::{Read, Table, LEVELS, LEVEL_TOP};
use std::f32::consts::{FRAC_PI_2, PI};

pub const MAX_UNISON: usize = 16;
/// Semitones of frequency deviation per unit of FM at amount 1, as a multiple of the
/// carrier's frequency (linear, through zero).
const FM_DEPTH: f32 = 4.0;
/// Cycles of phase shift per unit of PM at amount 1.
const PM_DEPTH: f32 = 1.0;
/// The fraction of an octave below a level's limit over which it crossfades to the next.
const MIP_FADE: f32 = 0.25;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Warp {
    #[default]
    Off,
    Sync,
    BendPlus,
    BendMinus,
    AsymPlus,
    AsymMinus,
    Pwm,
    Mirror,
    Flip,
    Quantize,
    Fold,
    SoftClip,
    HardClip,
    Fm,
    Pm,
    Am,
    Rm,
}

impl Warp {
    pub fn from_index(i: f32) -> Warp {
        use Warp::*;
        [
            Off, Sync, BendPlus, BendMinus, AsymPlus, AsymMinus, Pwm, Mirror, Flip, Quantize, Fold,
            SoftClip, HardClip, Fm, Pm, Am, Rm,
        ][(i.round().max(0.0) as usize).min(16)]
    }

    /// It reads another signal (FM, PM, AM, RM).
    pub fn needs_source(self) -> bool {
        matches!(self, Warp::Fm | Warp::Pm | Warp::Am | Warp::Rm)
    }

    /// It moves the read phase (rather than shaping the output).
    fn on_phase(self) -> bool {
        matches!(
            self,
            Warp::Sync
                | Warp::BendPlus
                | Warp::BendMinus
                | Warp::AsymPlus
                | Warp::AsymMinus
                | Warp::Pwm
                | Warp::Mirror
                | Warp::Quantize
                | Warp::Pm
        )
    }
}

/// A warp slot as the per-sample loop sees it.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct WarpSlot {
    pub mode: Warp,
    /// 0…1; 0 is skipped entirely, so it is the identity by construction.
    pub amt: f32,
    /// `0..=5`: osc 1–3, sub, noise, self.
    pub src: usize,
}

impl WarpSlot {
    pub fn active(&self) -> bool {
        self.mode != Warp::Off && self.amt > 0.0
    }
}

/// What the control grid computes for one lane.
#[derive(Copy, Clone, Debug, Default)]
struct Lane {
    phase: u32,
    inc: u32,
    dinc: i32,
    gl: f32,
    gr: f32,
    dgl: f32,
    dgr: f32,
    level: usize,
    /// Weight of `level + 1` (the mip crossfade).
    lw: f32,
    frame: usize,
    /// Weight of `frame + 1`.
    fw: f32,
    /// Where the four cycles a read may blend start: (level, frame), (level, frame + 1),
    /// (level + 1, frame), (level + 1, frame + 1).
    base: [usize; 4],
    /// The lane's last output (for self-modulation).
    last: f32,
    /// Above the Nyquist limit altogether: silent.
    mute: bool,
}

/// One control tick's settings for an oscillator (already modulated).
#[derive(Copy, Clone, Debug, Default)]
pub struct OscControl {
    /// Frequency of the centre lane, Hz.
    pub hz: f32,
    /// Linear gain.
    pub gain: f32,
    pub pan: f32,
    pub pos: f32,
    pub lanes: usize,
    pub detune_ct: f32,
    pub curve: usize,
    pub blend: f32,
    pub width: f32,
    pub stack: usize,
    pub spread: f32,
    pub warps: [WarpSlot; 2],
}

/// The seeded parts of a note: per-lane random numbers drawn at note-on.
#[derive(Copy, Clone, Debug)]
pub struct LaneRandom {
    pub phase: [f32; MAX_UNISON],
    pub detune: [f32; MAX_UNISON],
}

#[derive(Clone, Debug)]
pub struct Osc {
    lanes: [Lane; MAX_UNISON],
    n: usize,
    warps: [WarpSlot; 2],
    /// The previous output of lane 0, what another oscillator's FM reads.
    pub out0: f32,
}

impl Default for Osc {
    fn default() -> Self {
        Osc {
            lanes: [Lane::default(); MAX_UNISON],
            n: 1,
            warps: [WarpSlot::default(); 2],
            out0: 0.0,
        }
    }
}

/// The Szabó (2010) supersaw spacing — the JP-8000's seven voices sit at about ±0.11,
/// ±0.063 and ±0.020 of the outer detune — as a curve through the lane positions.
fn super_curve(u: f32) -> f32 {
    const X: [f32; 4] = [0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0];
    const Y: [f32; 4] = [0.0, 0.1774, 0.5715, 1.0];
    let a = u.abs().min(1.0);
    let i = ((a * 3.0) as usize).min(2);
    let t = (a - X[i]) * 3.0;
    u.signum() * (Y[i] + (Y[i + 1] - Y[i]) * t)
}

/// Interval stacks (semitones), cycled across the lanes.
const STACK: [&[f32]; 8] = [
    &[0.0],
    &[0.0, 12.0],
    &[0.0, 7.0, 12.0],
    &[0.0, -12.0],
    &[0.0, -12.0, -24.0],
    &[0.0, 7.0],
    &[0.0, 4.0, 7.0],
    &[0.0, 3.0, 7.0],
];

impl Osc {
    /// A note starts: phases from the seeded randoms (`rand` of the way), the start
    /// phase `phase0` in cycles.
    pub fn start(&mut self, lanes: usize, phase0: f32, rand: f32, r: &LaneRandom) {
        self.n = lanes.clamp(1, MAX_UNISON);
        for (i, l) in self.lanes.iter_mut().enumerate() {
            let p = (phase0 + rand * r.phase[i]).rem_euclid(1.0);
            *l = Lane {
                phase: (p as f64 * 4_294_967_296.0) as u64 as u32,
                ..Lane::default()
            };
        }
        self.out0 = 0.0;
    }

    /// Set every lane for the next `steps` samples at the rate `sr` (the oversampled
    /// rate). `first`: the note's first tick, so values land at once instead of ramping.
    #[allow(clippy::too_many_arguments)]
    pub fn control(
        &mut self,
        c: &OscControl,
        table: &Table,
        sr: f32,
        nyquist_hz: f32,
        steps: u32,
        r: &LaneRandom,
        first: bool,
    ) {
        self.warps = c.warps;
        let frames = table.frames();
        let n = c.lanes.clamp(1, MAX_UNISON);
        self.n = n;
        // Blend: 0 the centre alone, ½ all equal, 1 the sides alone — equal power, and
        // the total held at the level of one lane (uncorrelated lanes add in power).
        let centre = |i: usize| {
            if n % 2 == 1 {
                i == n / 2
            } else {
                i == n / 2 || i + 1 == n / 2
            }
        };
        let (gc, gs, norm) = if n == 1 {
            (1.0, 0.0, 1.0)
        } else {
            let nc = if n % 2 == 1 { 1.0 } else { 2.0 };
            let ns = n as f32 - nc;
            let gc = (c.blend * FRAC_PI_2).cos();
            let gs = (c.blend * FRAC_PI_2).sin();
            (gc, gs, 1.0 / (nc * gc * gc + ns * gs * gs).sqrt().max(1e-6))
        };
        let stack = STACK[c.stack.min(STACK.len() - 1)];
        let inv_steps = 1.0 / steps.max(1) as f32;
        for i in 0..n {
            let u = if n == 1 {
                0.0
            } else {
                -1.0 + 2.0 * i as f32 / (n - 1) as f32
            };
            let shape = match c.curve {
                1 => u,
                2 => u * u * u,
                3 => 2.0 * r.detune[i] - 1.0,
                _ => super_curve(u),
            };
            let semis = if n == 1 {
                0.0
            } else {
                c.detune_ct * 0.01 * shape + stack[i % stack.len()]
            };
            let hz = c.hz * (semis / 12.0).exp2();
            let l = &mut self.lanes[i];
            // Increment, ramped over the tick.
            let target = ((hz / sr).clamp(0.0, 0.5) as f64 * 4_294_967_296.0) as i64;
            let target = target.min(i32::MAX as i64);
            if first {
                l.inc = target as u32;
                l.dinc = 0;
            } else {
                l.dinc = ((target - l.inc as i64) / steps.max(1) as i64) as i32;
            }
            // Mip level: the lowest whose top harmonic stays under the limit, faded
            // into the next over the top quarter octave.
            let x = (hz * LEVEL_TOP[0] as f32 / nyquist_hz).max(1e-9).log2();
            let lv = x.ceil().max(0.0);
            l.mute = lv > (LEVELS - 1) as f32 || hz <= 0.0;
            l.level = (lv as usize).min(LEVELS - 1);
            let below = lv - x;
            l.lw = if l.level + 1 < LEVELS {
                ((MIP_FADE - below) / MIP_FADE).clamp(0.0, 1.0)
            } else {
                0.0
            };
            // Frame pair.
            let pos = (c.pos + c.spread * u * 0.5).clamp(0.0, 1.0);
            let fp = pos * (frames.max(1) - 1) as f32;
            l.frame = (fp as usize).min(frames.max(1) - 1);
            l.fw = if l.frame + 1 < frames {
                fp - l.frame as f32
            } else {
                0.0
            };
            let next = (l.level + 1).min(LEVELS - 1);
            l.base = [
                table.base(l.level, l.frame),
                table.base(l.level, l.frame + 1),
                table.base(next, l.frame),
                table.base(next, l.frame + 1),
            ];
            // Level and pan.
            let g = c.gain * norm * if centre(i) { gc } else { gs };
            let pair = i.min(n - 1 - i);
            let side = if (i < n - 1 - i) ^ (pair % 2 == 1) {
                -1.0
            } else {
                1.0
            };
            let pan = (c.pan + c.width * u.abs() * side).clamp(-1.0, 1.0);
            let th = (pan + 1.0) * PI * 0.25;
            let (tl, tr) = (
                th.cos() * std::f32::consts::SQRT_2,
                th.sin() * std::f32::consts::SQRT_2,
            );
            let (tl, tr) = (g * tl, g * tr);
            if first {
                (l.gl, l.gr, l.dgl, l.dgr) = (tl, tr, 0.0, 0.0);
            } else {
                l.dgl = (tl - l.gl) * inv_steps;
                l.dgr = (tr - l.gr) * inv_steps;
            }
        }
    }

    /// Whether a warp reads another signal, and which (`0..=5`).
    pub fn sources(&self) -> [Option<usize>; 2] {
        let f = |w: &WarpSlot| (w.active() && w.mode.needs_source()).then_some(w.src);
        [f(&self.warps[0]), f(&self.warps[1])]
    }

    /// One sample: `m` are the two warp slots' source values. Returns (left, right).
    #[inline]
    pub fn tick(&mut self, t: &Table, m: [f32; 2]) -> (f32, f32) {
        let (mut sl, mut sr) = (0.0, 0.0);
        let warps = self.warps;
        let any_warp = warps[0].active() || warps[1].active();
        for (i, l) in self.lanes[..self.n].iter_mut().enumerate() {
            // Advance: linear FM through zero moves the increment itself.
            let mut inc = l.inc as i64;
            if any_warp {
                for (k, w) in warps.iter().enumerate() {
                    if w.active() && w.mode == Warp::Fm {
                        let src = if w.src == 5 { l.last } else { m[k] };
                        inc += (w.amt * FM_DEPTH * src * l.inc as f32) as i64;
                    }
                }
            }
            l.phase = l.phase.wrapping_add(inc as u32);
            l.inc = (l.inc as i64 + l.dinc as i64).clamp(0, i32::MAX as i64) as u32;
            let (gl, gr) = (l.gl, l.gr);
            l.gl += l.dgl;
            l.gr += l.dgr;
            if l.mute {
                continue;
            }
            let mut phi = (l.phase >> 8) as f32 * (1.0 / 16_777_216.0);
            let mut flip = false;
            if any_warp {
                for (k, w) in warps.iter().enumerate() {
                    if !w.active() || !w.mode.on_phase() {
                        continue;
                    }
                    let src = if w.src == 5 { l.last } else { m[k] };
                    phi = warp_phase(w.mode, w.amt, phi, src);
                }
                flip = warps
                    .iter()
                    .any(|w| w.active() && w.mode == Warp::Flip && phi >= 0.5);
            }
            let mut y = read(t, l, phi);
            if any_warp {
                for (k, w) in warps.iter().enumerate() {
                    if !w.active() || w.mode.on_phase() {
                        continue;
                    }
                    let src = if w.src == 5 { l.last } else { m[k] };
                    y = warp_out(w.mode, w.amt, y, src, flip);
                }
            }
            l.last = y;
            if i == 0 {
                self.out0 = y;
            }
            sl += y * gl;
            sr += y * gr;
        }
        (sl, sr)
    }
}

/// The warped cycle at table position `pos` (0…1), `out.len()` points, for displays:
/// the phase warps, the read (frames crossfaded at full bandwidth), then the output warps
/// — as [`Osc::tick`] applies them. A modulator (FM, PM, AM, RM) is drawn as a sine at
/// the oscillator's own pitch, and FM as the phase modulation it integrates to.
pub fn display_cycle(t: &Table, pos: f32, warps: &[WarpSlot; 2], out: &mut [f32]) {
    let frames = t.frames().max(1);
    let fp = pos.clamp(0.0, 1.0) * (frames - 1) as f32;
    let k = (fp as usize).min(frames - 1);
    let fw = if k + 1 < frames { fp - k as f32 } else { 0.0 };
    let n = out.len().max(1);
    for (i, o) in out.iter_mut().enumerate() {
        let phi0 = i as f32 / n as f32;
        let m = (std::f32::consts::TAU * phi0).sin();
        let mut phi = phi0;
        for w in warps.iter().filter(|w| w.active()) {
            phi = match w.mode {
                Warp::Fm => {
                    let dev = -(std::f32::consts::TAU * phi0).cos() / std::f32::consts::TAU;
                    (phi + w.amt * FM_DEPTH * dev).rem_euclid(1.0)
                }
                mode if mode.on_phase() => warp_phase(mode, w.amt, phi, m),
                _ => phi,
            };
        }
        let a = t.read(0, k, phi.min(1.0));
        let mut y = if fw > 0.0 {
            a + (t.read(0, k + 1, phi.min(1.0)) - a) * fw
        } else {
            a
        };
        let flip = warps
            .iter()
            .any(|w| w.active() && w.mode == Warp::Flip && phi >= 0.5);
        for w in warps
            .iter()
            .filter(|w| w.active() && !w.mode.on_phase() && w.mode != Warp::Fm)
        {
            y = warp_out(w.mode, w.amt, y, m, flip);
        }
        *o = y;
    }
}

#[inline]
fn read(t: &Table, l: &Lane, phi: f32) -> f32 {
    let r = Read::new(l.level, phi);
    let mut a = t.apply(l.base[0], &r);
    if l.fw > 0.0 {
        a += (t.apply(l.base[1], &r) - a) * l.fw;
    }
    if l.lw > 0.0 {
        let r = Read::new(l.level + 1, phi);
        let mut b = t.apply(l.base[2], &r);
        if l.fw > 0.0 {
            b += (t.apply(l.base[3], &r) - b) * l.fw;
        }
        a += (b - a) * l.lw;
    }
    a
}

#[inline]
fn warp_phase(mode: Warp, amt: f32, phi: f32, m: f32) -> f32 {
    match mode {
        Warp::Sync => (phi * (1.0 + 15.0 * amt)).fract(),
        Warp::BendPlus | Warp::BendMinus => {
            let p = if mode == Warp::BendPlus { 1.0 } else { -1.0 } * 3.0 * amt;
            let u = 2.0 * phi - 1.0;
            0.5 + 0.5 * u.signum() * u.abs().powf(p.exp2())
        }
        Warp::AsymPlus | Warp::AsymMinus => {
            let p = if mode == Warp::AsymPlus { 1.0 } else { -1.0 } * 3.0 * amt;
            phi.powf(p.exp2())
        }
        Warp::Pwm => (phi / (1.0 - 0.98 * amt)).min(1.0),
        Warp::Mirror => phi + ((1.0 - (2.0 * phi - 1.0).abs()) - phi) * amt,
        Warp::Quantize => {
            let n = 2048.0 * (-10.0 * amt).exp2();
            (phi * n).floor() / n
        }
        Warp::Pm => (phi + amt * PM_DEPTH * m).rem_euclid(1.0),
        _ => phi,
    }
    .clamp(0.0, 1.0)
}

#[inline]
fn warp_out(mode: Warp, amt: f32, y: f32, m: f32, flip: bool) -> f32 {
    match mode {
        Warp::Flip => {
            if flip {
                y * (1.0 - 2.0 * amt)
            } else {
                y
            }
        }
        Warp::Fold => y + ((y * (1.0 + 7.0 * amt) * FRAC_PI_2).sin() - y) * amt,
        Warp::SoftClip => {
            let g = 1.0 + 15.0 * amt;
            y + ((g * y).tanh() / g.tanh() - y) * amt
        }
        Warp::HardClip => (y * (1.0 + 15.0 * amt)).clamp(-1.0, 1.0),
        Warp::Am => y * (1.0 - amt * 0.5 * (1.0 - m)),
        Warp::Rm => y * (1.0 - amt + amt * m),
        _ => y,
    }
}
