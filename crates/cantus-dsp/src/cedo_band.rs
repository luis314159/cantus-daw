//! Cedo's band split: where the duck acts — the whole signal, only the lows, or only the
//! highs — through one of two crossovers, each with an **exact null path** (a band gain of
//! exactly 1 hands the delayed input on to the bit; the filters run beside it so a duck
//! that starts finds them warm).
//!
//! - **Minimum phase** (no latency): two Simper state-variable sections at the crossover
//!   (Butterworth damping, `k = √2`, coefficients fixed by the frequency alone), read as a
//!   **shelf whose depth moves**: a low shelf `(s² + kAs + A²)/(s² + ks + 1)` per section,
//!   `A² = √G` for a band gain `G`. Its magnitude is `|H|² = (A⁴ + ω⁴)/(1 + ω⁴)` per
//!   section — monotone from `G` at DC to 1 above, **no bump anywhere**, and at a deep duck
//!   −6 dB at the crossover (−3 per section), as a Linkwitz–Riley split is. The highs use
//!   the mirrored high shelf `(A²s² + kAs + 1)/(s² + ks + 1)`. Only the outputs' weights
//!   move with the gain, never the filters' own coefficients, so a duck moving every sample
//!   cannot destabilise them.
//! - **Linear phase**: the low band is a **sharpened B-spline**. The base is two boxcars
//!   of `M` samples (`B = sinc²`, symmetric, `M = 0.44295·fs/fc` so `B(fc) = ½`), and the
//!   kernel is `L = S(B)` with `S(b) = 10b³ − 15b⁴ + 6b⁵` (smootherstep; Kaiser and
//!   Hamming's sharpening, 1977) — the base applied three, four and five times (ten
//!   boxcars), the first two delayed to the last's centre and summed. `S` is non-decreasing
//!   from 0 to 1 on `[0, 1]` and `B` stays in `[0, 1]`, so **`0 ≤ L ≤ 1` at every
//!   frequency**: neither the lows' duck nor the highs' can rise above the input. `L(fc) = ½`
//!   (−6 dB), the passband flat to 0.999 at `fc/4` and 0.973 at `fc/2`, the stopband under
//!   −60 dB above `2fc`. The highs are the delayed input minus the lows, so the two bands
//!   sum to the delayed input exactly. Everything runs in **integers** (the input in steps
//!   of 2⁻³⁶, each boxcar's running sum exact and its output a pure function of the sum,
//!   the polynomial a sum of integers), so nothing drifts and rest is exact. The latency is
//!   the kernel's centre at the **floor** in force (the owner's choice, 2026-11-17,
//!   [`Floor`]): `5·(M − 1)` of the floor's boxcar, whatever the crossover above it — a
//!   higher crossover's kernel reads the input that much later. **Auto** (the default) puts
//!   the floor an octave under the crossover ([`auto_floor`]), so the latency is always the
//!   least the crossover needs; the floor it uses is the one *committed* (the editor commits
//!   it when the crossover is let go, so the latency never moves during a drag). The fixed
//!   floors, 40, 60, 80 and 120 Hz, keep one latency whatever the crossover: at 48 kHz
//!   55.3, 36.8, 27.6 and 18.3 ms. A crossover under the floor in force is heard at the
//!   floor.

use crate::math;

/// The lowest crossover; also what the linear-phase latency is sized for.
pub const MIN_HZ: f32 = 40.0;
/// The highest crossover.
pub const MAX_HZ: f32 = 2_000.0;
/// The linear-phase kernel's lowest floor: what its boxcars and the dry line are sized for.
pub const FLOOR_MIN_HZ: f32 = 40.0;
/// The highest floor Auto puts (an octave under the highest crossover).
pub const FLOOR_MAX_HZ: f32 = 1_000.0;
/// The fixed floors, in [`Floor`]'s order after Auto.
pub const FLOORS: [f32; 4] = [40.0, 60.0, 80.0, 120.0];
/// Auto's committed floor before anything commits one: the default crossover's (120 Hz).
pub const AUTO_DEFAULT_HZ: f32 = 60.0;

/// The linear-phase split's floor: what its latency is sized for.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Floor {
    /// An octave under the crossover, as last committed ([`auto_floor`]).
    #[default]
    Auto,
    Hz40,
    Hz60,
    Hz80,
    Hz120,
}

impl Floor {
    pub fn from_index(i: usize) -> Floor {
        match i {
            1 => Floor::Hz40,
            2 => Floor::Hz60,
            3 => Floor::Hz80,
            4 => Floor::Hz120,
            _ => Floor::Auto,
        }
    }

    /// The floor in hertz: a fixed one's, or Auto's committed `auto_hz`.
    pub fn hz(self, auto_hz: f32) -> f32 {
        match self {
            Floor::Auto => {
                if auto_hz.is_finite() {
                    auto_hz.clamp(FLOOR_MIN_HZ, FLOOR_MAX_HZ)
                } else {
                    AUTO_DEFAULT_HZ
                }
            }
            Floor::Hz40 => FLOORS[0],
            Floor::Hz60 => FLOORS[1],
            Floor::Hz80 => FLOORS[2],
            Floor::Hz120 => FLOORS[3],
        }
    }
}

/// Auto's floor for a crossover: an octave under it, within the floors' range.
pub fn auto_floor(cross_hz: f32) -> f32 {
    let hz = if cross_hz.is_finite() {
        cross_hz
    } else {
        120.0
    };
    (hz * 0.5).clamp(FLOOR_MIN_HZ, FLOOR_MAX_HZ)
}
/// Where `sinc(x)² = ½` (−6 dB): the boxcar length is this many crossover periods.
pub const SINC2_HALF: f64 = 0.442_946_470_689;
/// Boxcars in the linear-phase cascade: the two-boxcar base applied five times.
const STAGES: usize = 10;
/// The input's integer step: 2⁻³⁶ (−217 dBFS).
const SCALE: f64 = 68_719_476_736.0;
/// The input is clamped to this before it is made an integer (+84 dBFS): far above any
/// signal, and the sums stay inside `i64` for the longest boxcar at 192 kHz.
const CLAMP: f64 = 16_384.0;
/// SVF states under this, with silence coming in, are stopped (exact rest).
const TINY: f64 = 1e-25;

/// Where the duck acts.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Band {
    #[default]
    Whole,
    Lows,
    Highs,
}

impl Band {
    pub fn from_index(i: usize) -> Band {
        match i {
            1 => Band::Lows,
            2 => Band::Highs,
            _ => Band::Whole,
        }
    }
}

/// How the band is split.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Crossover {
    #[default]
    MinimumPhase,
    LinearPhase,
}

/// The boxcar length for a crossover at `hz`.
pub fn boxcar_len(hz: f32, sample_rate: f32) -> usize {
    let hz = hz.clamp(FLOOR_MIN_HZ, MAX_HZ) as f64;
    ((SINC2_HALF * sample_rate as f64 / hz).round() as usize).max(2)
}

/// The linear-phase kernel's delay at a floor and a rate: the centre of the floor's kernel.
pub fn linear_latency(floor_hz: f32, sample_rate: f32) -> usize {
    kernel_delay(boxcar_len(floor_hz, sample_rate))
}

/// The sharpened kernel's centre for a boxcar length: the base's `M − 1`, five times.
pub fn kernel_delay(m: usize) -> usize {
    5 * (m - 1)
}

/// One Simper SVF section's coefficients (fixed by the crossover alone).
#[derive(Copy, Clone, Debug, Default)]
struct Svf {
    a1: f64,
    a2: f64,
    a3: f64,
}

const K: f64 = std::f64::consts::SQRT_2;

impl Svf {
    fn new(hz: f32, sr: f32) -> Svf {
        let w = std::f64::consts::PI * (hz as f64 / sr as f64).min(0.49);
        let g = math::tan(w);
        let a1 = 1.0 / (1.0 + g * (g + K));
        let a2 = g * a1;
        Svf { a1, a2, a3: g * a2 }
    }

    /// One sample through a section with states `s`: (band, low).
    #[inline]
    fn tick(&self, s: &mut [f64; 2], v0: f64) -> (f64, f64) {
        let v3 = v0 - s[1];
        let v1 = self.a1 * s[0] + self.a2 * v3;
        let v2 = s[1] + self.a2 * s[0] + self.a3 * v3;
        s[0] = 2.0 * v1 - s[0];
        s[1] = 2.0 * v2 - s[1];
        if v0 == 0.0 && s[0].abs() < TINY && s[1].abs() < TINY {
            *s = [0.0; 2];
        }
        (v1, v2)
    }
}

/// One boxcar of the linear-phase cascade: its last `m` inputs and their exact sum.
#[derive(Clone, Debug)]
struct Boxcar {
    line: Vec<i64>,
    pos: usize,
    sum: i64,
}

impl Boxcar {
    fn new(cap: usize) -> Boxcar {
        Boxcar {
            line: vec![0; cap.max(1)],
            pos: 0,
            sum: 0,
        }
    }

    /// One input; the mean of the last `m`, rounded to the nearest step.
    #[inline]
    fn tick(&mut self, x: i64, m: usize) -> i64 {
        self.sum += x - self.line[self.pos];
        self.line[self.pos] = x;
        self.pos += 1;
        if self.pos == m {
            self.pos = 0;
        }
        mean(self.sum, m)
    }

    fn clear(&mut self) {
        self.line.iter_mut().for_each(|v| *v = 0);
        self.pos = 0;
        self.sum = 0;
    }

    fn is_clear(&self) -> bool {
        self.sum == 0 && self.line.iter().all(|v| *v == 0)
    }
}

/// `sum / m` to the nearest integer (halves up): a pure function of the sum.
#[inline]
fn mean(sum: i64, m: usize) -> i64 {
    let m = m as i64;
    (sum + m / 2).div_euclid(m)
}

#[inline]
fn to_int(x: f32) -> i64 {
    ((x as f64).clamp(-CLAMP, CLAMP) * SCALE).round() as i64
}

/// A fixed-length integer delay.
#[derive(Clone, Debug)]
struct Ring {
    buf: Vec<i64>,
    pos: usize,
}

impl Ring {
    fn new(cap: usize) -> Ring {
        Ring {
            buf: vec![0; cap.max(1)],
            pos: 0,
        }
    }

    /// Push `x`; the value pushed `d` pushes before it (0: `x` itself).
    #[inline]
    fn push_tap(&mut self, x: i64, d: usize) -> i64 {
        let cap = self.buf.len();
        self.buf[self.pos] = x;
        let i = (self.pos + cap - d.min(cap - 1)) % cap;
        self.pos = (self.pos + 1) % cap;
        self.buf[i]
    }

    fn clear(&mut self) {
        self.buf.iter_mut().for_each(|v| *v = 0);
        self.pos = 0;
    }

    fn is_clear(&self) -> bool {
        self.buf.iter().all(|v| *v == 0)
    }
}

/// One channel's sharpened kernel: ten boxcars, the base's third and fourth powers delayed
/// to the fifth's centre.
#[derive(Clone, Debug)]
struct Kernel {
    boxes: [Boxcar; STAGES],
    r3: Ring,
    r4: Ring,
}

impl Kernel {
    fn new(m_max: usize) -> Kernel {
        Kernel {
            boxes: std::array::from_fn(|_| Boxcar::new(m_max)),
            r3: Ring::new(2 * m_max),
            r4: Ring::new(m_max),
        }
    }

    /// One input step; the low band, in steps.
    #[inline]
    fn tick(&mut self, x: i64, m: usize) -> i64 {
        let mut v = x;
        let (mut b3, mut b4) = (0, 0);
        for (k, b) in self.boxes.iter_mut().enumerate() {
            v = b.tick(v, m);
            if k == 5 {
                b3 = v;
            } else if k == 7 {
                b4 = v;
            }
        }
        let b3 = self.r3.push_tap(b3, 2 * (m - 1));
        let b4 = self.r4.push_tap(b4, m - 1);
        10 * b3 - 15 * b4 + 6 * v
    }

    fn clear(&mut self) {
        self.boxes.iter_mut().for_each(Boxcar::clear);
        self.r3.clear();
        self.r4.clear();
    }

    fn is_clear(&self) -> bool {
        self.boxes.iter().all(Boxcar::is_clear) && self.r3.is_clear() && self.r4.is_clear()
    }
}

/// The split for both channels.
#[derive(Clone, Debug)]
pub struct Split {
    sr: f32,
    band: Band,
    cross: Crossover,
    hz: f32,
    /// The floor in force (linear phase), Hz.
    floor: f32,
    svf: Svf,
    /// Per channel, per section: the SVF's two states.
    sv: [[[f64; 2]; 2]; 2],
    m: usize,
    kern: [Kernel; 2],
}

impl Split {
    /// Allocates the boxcars for the longest kernel.
    pub fn new(sample_rate: f32) -> Split {
        let cap = boxcar_len(FLOOR_MIN_HZ, sample_rate);
        let mut s = Split {
            sr: sample_rate,
            band: Band::Whole,
            cross: Crossover::MinimumPhase,
            hz: 120.0,
            floor: AUTO_DEFAULT_HZ,
            svf: Svf::default(),
            sv: [[[0.0; 2]; 2]; 2],
            m: cap,
            kern: [Kernel::new(cap), Kernel::new(cap)],
        };
        s.svf = Svf::new(s.hz, sample_rate);
        s
    }

    /// The latency the split adds: the linear-phase kernel's centre, when it is in.
    pub fn latency(&self) -> usize {
        if self.uses_kernel() {
            linear_latency(self.floor, self.sr)
        } else {
            0
        }
    }

    fn uses_kernel(&self) -> bool {
        self.band != Band::Whole && self.cross == Crossover::LinearPhase
    }

    /// How much later than the dry path's newest sample the kernel's input is read, so its
    /// centre lands on the latency whatever its length.
    pub fn kernel_offset(&self) -> usize {
        linear_latency(self.floor, self.sr) - kernel_delay(self.m)
    }

    /// The boxcar length in force.
    pub fn kernel_len(&self) -> usize {
        self.m
    }

    /// New settings (`floor_hz` the linear phase's floor in force, [`Floor::hz`]). A new
    /// kernel length or floor rebuilds the boxcars from `history` (the dry path's input,
    /// newest first: `history(d)` is the input `d` samples ago), so the kernel is the one it
    /// would be had it always been that long. No allocation.
    pub fn set(
        &mut self,
        band: Band,
        cross: Crossover,
        hz: f32,
        floor_hz: f32,
        history: &dyn Fn(usize) -> [f32; 2],
    ) {
        let hz = if hz.is_finite() {
            hz.clamp(MIN_HZ, MAX_HZ)
        } else {
            120.0
        };
        if hz != self.hz {
            self.hz = hz;
            self.svf = Svf::new(hz, self.sr);
        }
        let floor = if floor_hz.is_finite() {
            floor_hz.clamp(FLOOR_MIN_HZ, FLOOR_MAX_HZ)
        } else {
            AUTO_DEFAULT_HZ
        };
        let was = self.uses_kernel();
        let floor_moved = floor != self.floor;
        self.band = band;
        self.cross = cross;
        self.floor = floor;
        // A crossover under the floor is heard at the floor.
        let m = boxcar_len(hz.max(floor), self.sr);
        if self.uses_kernel() && (m != self.m || !was || floor_moved) {
            self.m = m;
            self.rebuild(history);
        } else {
            self.m = m;
        }
    }

    /// The kernel as it would be after hearing `history` through its input tap.
    fn rebuild(&mut self, history: &dyn Fn(usize) -> [f32; 2]) {
        let m = self.m;
        let off = self.kernel_offset();
        // Boxcar s needs its last m inputs, which need (s + 1)·(m − 1) + 1 inputs back; the
        // delayed powers need no more than the last boxcar does.
        let span = STAGES * (m - 1) + 1;
        for ch in 0..2 {
            self.kern[ch].clear();
            for back in (0..span).rev() {
                self.kern[ch].tick(to_int(history(off + back)[ch]), m);
            }
        }
    }

    /// Dry-line frames the kernel's rebuild may read back, at a rate.
    pub fn history_needed(sample_rate: f32) -> usize {
        let m = boxcar_len(FLOOR_MIN_HZ, sample_rate);
        STAGES * m + 2
    }

    /// One sample: `dry` the delayed input (what passes at a band gain of 1), `kernel_in`
    /// the input at the kernel's tap, `g` each channel's band gain (1 is no duck).
    #[inline]
    pub fn tick(&mut self, dry: [f32; 2], kernel_in: [f32; 2], g: [f32; 2]) -> [f32; 2] {
        let mut out = dry;
        match (self.band, self.cross) {
            (Band::Whole, _) => {
                for ch in 0..2 {
                    if g[ch] != 1.0 {
                        out[ch] = dry[ch] * g[ch];
                    }
                }
            }
            (band, Crossover::MinimumPhase) => {
                for ch in 0..2 {
                    out[ch] = self.shelves(ch, dry[ch], g[ch], band == Band::Lows);
                }
            }
            (band, Crossover::LinearPhase) => {
                for ch in 0..2 {
                    let v = self.kern[ch].tick(to_int(kernel_in[ch]), self.m);
                    if g[ch] == 1.0 {
                        continue;
                    }
                    let low = (v as f64 / SCALE) as f32;
                    let x = dry[ch];
                    out[ch] = if band == Band::Lows {
                        x + (g[ch] - 1.0) * low
                    } else {
                        g[ch] * x + (1.0 - g[ch]) * low
                    };
                }
            }
        }
        out
    }

    /// The two shelves of channel `ch` at band gain `g`; at 1 the input, to the bit.
    #[inline]
    fn shelves(&mut self, ch: usize, x: f32, g: f32, low: bool) -> f32 {
        let svf = self.svf;
        let s = &mut self.sv[ch];
        if g == 1.0 {
            // The null path: each section's output is its input; the states follow it.
            let v0 = x as f64;
            svf.tick(&mut s[0], v0);
            svf.tick(&mut s[1], v0);
            return x;
        }
        // A² = √G per section, so the two give G.
        let a2 = (g.max(0.0) as f64).sqrt();
        let a = a2.sqrt();
        let mut y = x as f64;
        for sec in s.iter_mut() {
            let (v1, v2) = svf.tick(sec, y);
            y = if low {
                y + K * (a - 1.0) * v1 + (a2 - 1.0) * v2
            } else {
                a2 * y + K * a * (1.0 - a) * v1 + (1.0 - a2) * v2
            };
        }
        y as f32
    }

    /// Every filter state empty.
    pub fn is_clear(&self) -> bool {
        self.sv.iter().flatten().flatten().all(|v| v.to_bits() == 0)
            && self.kern.iter().all(Kernel::is_clear)
    }

    pub fn reset(&mut self) {
        self.sv = [[[0.0; 2]; 2]; 2];
        self.kern.iter_mut().for_each(Kernel::clear);
    }

    /// The magnitude, dB, at `hz` for a band gain `g`: what the display draws.
    pub fn response_db(&self, hz: f32, g: f32) -> f32 {
        let g = g.clamp(0.0, 1.0) as f64;
        let m2 = match (self.band, self.cross) {
            (Band::Whole, _) => g * g,
            (band, Crossover::MinimumPhase) => {
                let pi = std::f64::consts::PI;
                let w = math::tan(pi * (hz as f64 / self.sr as f64).min(0.4999))
                    / math::tan(pi * (self.hz as f64 / self.sr as f64).min(0.49));
                let w4 = (w * w) * (w * w);
                // A⁴ per section is √G·√G = G.
                let sec = if band == Band::Lows {
                    (g + w4) / (1.0 + w4)
                } else {
                    (1.0 + g * w4) / (1.0 + w4)
                };
                sec * sec
            }
            (band, Crossover::LinearPhase) => {
                let d = dirichlet(hz as f64 / self.sr as f64, self.m);
                let b = d * d;
                let l = b * b * b * (10.0 - 15.0 * b + 6.0 * b * b);
                let h = if band == Band::Lows {
                    1.0 + (g - 1.0) * l
                } else {
                    g + (1.0 - g) * l
                };
                h * h
            }
        };
        (10.0 * math::log10(m2.max(1e-30))) as f32
    }
}

/// A boxcar of `m` samples at `f` (cycles per sample): `sin(πfm) / (m·sin(πf))`.
fn dirichlet(f: f64, m: usize) -> f64 {
    let pi = std::f64::consts::PI;
    let d = math::sin(pi * f);
    if d.abs() < 1e-12 {
        return 1.0;
    }
    math::sin(pi * f * m as f64) / (m as f64 * d)
}
