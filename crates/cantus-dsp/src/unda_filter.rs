//! Unda's filters: the shared state-variable filter (`crate::filter::Svf`) in six modes,
//! and a four-pole ladder of Unda's own — zero-delay feedback (Zavalishin, *The Art of VA
//! Filter Design*, ch. 5): four trapezoidal one-poles, the feedback loop solved exactly
//! for its linear part, a `tanh` on the loop's input for the ladder's character, and mode
//! mixing from the stage outputs (low-pass 24 and 12, band-pass, high-pass 24). At
//! resonance 1 the loop gain reaches 4 and the ladder oscillates on its own.
//!
//! Coefficients are set on the control grid (every 16 base samples), never per sample.

use crate::filter::{FilterMode, Svf};

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum FilterType {
    #[default]
    SvfLp12,
    SvfLp24,
    SvfBp,
    SvfHp12,
    SvfHp24,
    SvfNotch,
    LadderLp24,
    LadderLp12,
    LadderBp,
    LadderHp24,
}

impl FilterType {
    pub fn from_index(i: f32) -> FilterType {
        use FilterType::*;
        [
            SvfLp12, SvfLp24, SvfBp, SvfHp12, SvfHp24, SvfNotch, LadderLp24, LadderLp12, LadderBp,
            LadderHp24,
        ][(i.round().max(0.0) as usize).min(9)]
    }

    pub fn is_ladder(self) -> bool {
        matches!(
            self,
            FilterType::LadderLp24
                | FilterType::LadderLp12
                | FilterType::LadderBp
                | FilterType::LadderHp24
        )
    }
}

/// How much of its range the ladder's `tanh` sees at 0 dB of drive: a hint of warmth, not
/// distortion, on a full-scale input.
const LADDER_HEADROOM: f32 = 0.5;

/// `tanh`, as the (3, 2) Padé approximant clipped where it reaches ±1 (at ±3): smooth,
/// odd, monotonic, within 2 % of `tanh`, and a fraction of its cost.
#[inline]
pub fn soft_clip(x: f32) -> f32 {
    let x = x.clamp(-3.0, 3.0);
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}

/// One channel of the ladder.
#[derive(Clone, Debug, Default)]
pub struct Ladder {
    s: [f32; 4],
    g: f32,
    k: f32,
    drive: f32,
    out_gain: f32,
}

impl Ladder {
    /// `cutoff` in Hz at `sr`, `res` 0…1, `drive_db` ≥ 0.
    pub fn set(&mut self, sr: f32, cutoff: f32, res: f32, drive_db: f32) {
        let fc = cutoff.clamp(10.0, sr * 0.45);
        let g = (std::f32::consts::PI * fc / sr).tan();
        self.g = g / (1.0 + g);
        self.k = 4.0 * res.clamp(0.0, 1.0);
        self.drive = 10f32.powf(drive_db.max(0.0) / 20.0) * LADDER_HEADROOM;
        // Keep the pass band from sinking as the resonance rises. The loop's `tanh` has
        // unity gain for small signals whatever the drive (drive moves where it bends,
        // not the level), so the linear response is exactly `magnitude_db`'s.
        self.out_gain = 1.0 + 0.5 * self.k;
    }

    pub fn reset(&mut self) {
        self.s = [0.0; 4];
    }

    #[inline]
    pub fn process(&mut self, x: f32, ty: FilterType) -> f32 {
        let g = self.g;
        let b = 1.0 - g;
        let s = &mut self.s;
        let sum = g * g * g * b * s[0] + g * g * b * s[1] + g * b * s[2] + b * s[3];
        let g4 = g * g * g * g;
        let u = (x - self.k * sum) / (1.0 + self.k * g4);
        let u = soft_clip(u * self.drive) / self.drive;
        let mut y = [0.0f32; 4];
        let mut v = u;
        for i in 0..4 {
            let yi = g * v + b * s[i];
            s[i] = 2.0 * yi - s[i];
            if s[i].abs() < 1e-20 {
                s[i] = 0.0;
            }
            y[i] = yi;
            v = yi;
        }
        let out = match ty {
            FilterType::LadderLp12 => y[1],
            FilterType::LadderBp => 4.0 * (y[1] - 2.0 * y[2] + y[3]),
            FilterType::LadderHp24 => u - 4.0 * y[0] + 6.0 * y[1] - 4.0 * y[2] + y[3],
            _ => y[3],
        };
        out * self.out_gain
    }
}

/// The magnitude response (dB) of a filter type at `hz`, for cutoff `cutoff`, resonance
/// `res`, at the sample rate `sr` — the transfer functions the discretisations realise
/// (the trapezoidal rule maps `s` to `j·tan(πf/fs)/tan(πfc/fs)`), so what the editor
/// draws is what plays (the ladder's `tanh` aside).
pub fn magnitude_db(ty: FilterType, cutoff: f32, res: f32, hz: f32, sr: f32) -> f32 {
    use std::f32::consts::PI;
    let fc = cutoff.clamp(10.0, sr * 0.45);
    let w = (PI * hz.min(sr * 0.499) / sr).tan() / (PI * fc / sr).tan();
    // Complex helpers on (re, im).
    let mul = |a: (f32, f32), b: (f32, f32)| (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0);
    let abs = |a: (f32, f32)| (a.0 * a.0 + a.1 * a.1).sqrt();
    let s = (0.0, w);
    let r = res.clamp(0.0, 1.0);
    let mag = if ty.is_ladder() {
        let k = 4.0 * r;
        let one_s = (1.0, w);
        let p2 = mul(one_s, one_s);
        let p4 = mul(p2, p2);
        let den = (p4.0 + k, p4.1);
        let num = match ty {
            FilterType::LadderLp12 => p2,
            FilterType::LadderBp => {
                let s2 = mul(s, s);
                (4.0 * s2.0, 4.0 * s2.1)
            }
            FilterType::LadderHp24 => {
                let s2 = mul(s, s);
                mul(s2, s2)
            }
            _ => (1.0, 0.0),
        };
        (1.0 + 0.5 * k) * abs(num) / abs(den).max(1e-12)
    } else {
        let k = std::f32::consts::SQRT_2 - (std::f32::consts::SQRT_2 - 0.1) * r;
        // s² + k·s + 1
        let den = (1.0 - w * w, k * w);
        let lp = 1.0 / abs(den).max(1e-12);
        let bw = abs((1.0 - w * w, std::f32::consts::SQRT_2 * w)).max(1e-12);
        match ty {
            FilterType::SvfLp12 => lp,
            FilterType::SvfLp24 => lp / bw,
            FilterType::SvfBp => w * lp,
            FilterType::SvfHp12 => w * w * lp,
            FilterType::SvfHp24 => w * w * lp * w * w / bw,
            FilterType::SvfNotch => (1.0 - w * w).abs() * lp,
            _ => lp,
        }
    };
    20.0 * mag.max(1e-9).log10()
}

/// One of Unda's two filters for one voice, both channels.
#[derive(Clone, Debug)]
pub struct FilterSlot {
    ty: FilterType,
    /// `[channel][stage]`.
    svf: [[Svf; 2]; 2],
    ladder: [Ladder; 2],
    /// The SVF's damping (for the notch), as `Svf::set` computes it.
    k: f32,
    pre: f32,
    drive_on: bool,
    sr: f32,
}

impl FilterSlot {
    pub fn new(sr: f32) -> Self {
        FilterSlot {
            sr,
            ty: FilterType::SvfLp12,
            svf: [[Svf::new(sr), Svf::new(sr)], [Svf::new(sr), Svf::new(sr)]],
            ladder: [Ladder::default(), Ladder::default()],
            k: std::f32::consts::SQRT_2,
            pre: 1.0,
            drive_on: false,
        }
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        for ch in &mut self.svf {
            for f in ch {
                f.set_sample_rate(sr);
            }
        }
    }

    pub fn reset(&mut self) {
        for ch in &mut self.svf {
            for f in ch {
                f.reset();
            }
        }
        for l in &mut self.ladder {
            l.reset();
        }
    }

    /// New coefficients: cutoffs per channel (Hz), resonance 0…1, drive in dB.
    pub fn set(&mut self, sr: f32, ty: FilterType, cut: [f32; 2], res: f32, drive_db: f32) {
        if sr != self.sr {
            self.sr = sr;
            self.set_sample_rate(sr);
        }
        if ty != self.ty {
            self.ty = ty;
            self.reset();
            let (a, b) = match ty {
                FilterType::SvfBp | FilterType::SvfNotch => (FilterMode::Band, FilterMode::Band),
                FilterType::SvfHp12 | FilterType::SvfHp24 => (FilterMode::High, FilterMode::High),
                _ => (FilterMode::Low, FilterMode::Low),
            };
            for ch in &mut self.svf {
                ch[0].set_mode(a);
                ch[1].set_mode(b);
            }
        }
        if ty.is_ladder() {
            for (c, l) in self.ladder.iter_mut().enumerate() {
                l.set(sr, cut[c], res, drive_db);
            }
        } else {
            let r = res.clamp(0.0, 1.0);
            self.k = std::f32::consts::SQRT_2 - (std::f32::consts::SQRT_2 - 0.1) * r;
            for (c, ch) in self.svf.iter_mut().enumerate() {
                ch[0].set(cut[c], r);
                ch[1].set(cut[c], 0.0);
            }
            self.drive_on = drive_db > 0.0;
            self.pre = 10f32.powf(drive_db.max(0.0) / 20.0);
        }
    }

    #[inline]
    pub fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        let mut out = [0.0; 2];
        for c in 0..2 {
            let v = x[c];
            out[c] = if self.ty.is_ladder() {
                self.ladder[c].process(v, self.ty)
            } else {
                let v = if self.drive_on {
                    soft_clip(v * self.pre) / self.pre.sqrt()
                } else {
                    v
                };
                let f = &mut self.svf[c];
                match self.ty {
                    FilterType::SvfLp24 | FilterType::SvfHp24 => {
                        let a = f[0].process(v);
                        f[1].process(a)
                    }
                    FilterType::SvfNotch => v - self.k * f[0].process(v),
                    _ => f[0].process(v),
                }
            };
        }
        out
    }
}
