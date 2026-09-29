//! One decimator per instrument, after the voice sum (decimation is linear, so summing
//! first is exact): a polyphase IIR half-band — two chains of first-order all-passes —
//! after Valenzuela & Constantinides, *IEE Proc. G* 130(6), 1983. Its coefficients come
//! from the elliptic half-band design formulas (the transition band and the number of
//! coefficients decide them). Minimum phase: a couple of samples of delay where a
//! linear-phase FIR of the same rejection would add tens.
//!
//! 4× is two 2× stages. Nothing here allocates after [`Decimator::new`].

/// All-pass coefficients per half-band (half in each path).
pub const COEFS: usize = 12;
/// Transition band, as a fraction of the high rate: the pass band ends at
/// `(0.25 − TRANSITION)·fs` — 20 kHz at 96 kHz.
pub const TRANSITION: f64 = 0.0417;

/// The half-band's all-pass coefficients (ascending), from the elliptic design.
pub fn coefficients(n: usize, transition: f64) -> Vec<f64> {
    use std::f64::consts::PI;
    let k = ((1.0 - 2.0 * transition) * PI / 4.0).tan().powi(2);
    let kk = (1.0 - k * k).sqrt().sqrt();
    let e = 0.5 * (1.0 - kk) / (1.0 + kk);
    let e4 = e.powi(4);
    let q = e * (1.0 + e4 * (2.0 + e4 * (15.0 + 150.0 * e4)));
    let order = (2 * n + 1) as f64;
    (1..=n)
        .map(|c| {
            let c = c as f64;
            let mut num = 0.0;
            let mut i = 0i32;
            loop {
                let term = q.powi(i * (i + 1))
                    * ((2 * i + 1) as f64 * c * PI / order).sin()
                    * if i % 2 == 0 { 1.0 } else { -1.0 };
                num += term;
                i += 1;
                if term.abs() < 1e-100 || i > 64 {
                    break;
                }
            }
            num *= q.powf(0.25);
            let mut den = 0.0;
            let mut i = 1i32;
            loop {
                let term = q.powi(i * i)
                    * (2.0 * i as f64 * c * PI / order).cos()
                    * if i % 2 == 1 { -1.0 } else { 1.0 };
                den += term;
                i += 1;
                if term.abs() < 1e-100 || i > 64 {
                    break;
                }
            }
            den = 1.0 + 2.0 * den;
            let ww = 2.0 * num / den;
            let wwsq = ww * ww;
            let x = ((1.0 - wwsq * k) * (1.0 - wwsq / k)).sqrt() / (1.0 + wwsq);
            (1.0 - x) / (1.0 + x)
        })
        .collect()
}

/// One 2:1 stage for one channel.
#[derive(Clone, Debug)]
struct Stage {
    /// Path 0's and path 1's coefficients.
    a0: [f32; COEFS / 2],
    a1: [f32; COEFS / 2],
    /// (previous input, previous output) of every section.
    s0: [(f32, f32); COEFS / 2],
    s1: [(f32, f32); COEFS / 2],
}

impl Stage {
    fn new(c: &[f64]) -> Self {
        let mut a0 = [0.0; COEFS / 2];
        let mut a1 = [0.0; COEFS / 2];
        for (i, v) in c.iter().enumerate() {
            if i % 2 == 0 {
                a0[i / 2] = *v as f32;
            } else {
                a1[i / 2] = *v as f32;
            }
        }
        Stage {
            a0,
            a1,
            s0: [(0.0, 0.0); COEFS / 2],
            s1: [(0.0, 0.0); COEFS / 2],
        }
    }

    #[inline]
    fn chain(a: &[f32; COEFS / 2], s: &mut [(f32, f32); COEFS / 2], x: f32) -> f32 {
        let mut v = x;
        for (c, st) in a.iter().zip(s.iter_mut()) {
            let y = c * (v - st.1) + st.0;
            *st = (v, y);
            v = y;
        }
        v
    }

    /// Two samples in (earlier, later), one out.
    #[inline]
    fn tick(&mut self, early: f32, late: f32) -> f32 {
        0.5 * (Self::chain(&self.a0, &mut self.s0, late)
            + Self::chain(&self.a1, &mut self.s1, early))
    }

    fn peak(&self) -> f32 {
        self.s0
            .iter()
            .chain(self.s1.iter())
            .fold(0.0f32, |m, (x, y)| m.max(x.abs()).max(y.abs()))
    }

    fn clear(&mut self) {
        self.s0 = [(0.0, 0.0); COEFS / 2];
        self.s1 = [(0.0, 0.0); COEFS / 2];
    }
}

/// Stereo decimation by 1, 2 or 4.
#[derive(Clone, Debug)]
pub struct Decimator {
    /// `[channel][stage]`: stage 0 is 2×→1×, stage 1 is 4×→2×.
    st: [[Stage; 2]; 2],
}

impl Default for Decimator {
    fn default() -> Self {
        Self::new()
    }
}

impl Decimator {
    pub fn new() -> Self {
        let c = coefficients(COEFS, TRANSITION);
        let s = Stage::new(&c);
        Decimator {
            st: [[s.clone(), s.clone()], [s.clone(), s]],
        }
    }

    /// One base-rate sample from `factor` (1, 2 or 4) samples per channel.
    #[inline]
    pub fn tick(&mut self, factor: usize, l: &[f32], r: &[f32]) -> (f32, f32) {
        match factor {
            4 => {
                let [cl, cr] = &mut self.st;
                let l0 = cl[1].tick(l[0], l[1]);
                let l1 = cl[1].tick(l[2], l[3]);
                let r0 = cr[1].tick(r[0], r[1]);
                let r1 = cr[1].tick(r[2], r[3]);
                (cl[0].tick(l0, l1), cr[0].tick(r0, r1))
            }
            2 => (
                self.st[0][0].tick(l[0], l[1]),
                self.st[1][0].tick(r[0], r[1]),
            ),
            _ => (l[0], r[0]),
        }
    }

    /// The largest state: what an idle instrument checks before it calls itself silent.
    pub fn peak(&self) -> f32 {
        self.st
            .iter()
            .flat_map(|c| c.iter())
            .fold(0.0f32, |m, s| m.max(s.peak()))
    }

    pub fn clear(&mut self) {
        self.st
            .iter_mut()
            .flat_map(|c| c.iter_mut())
            .for_each(Stage::clear);
    }
}
