//! Model 2's match to the recordings (a child of `arcus`, round 42,
//! `docs/round42/arcus-match.md`): a correction on the output — twelve peaking sections from
//! 63 Hz to 10 kHz whose gains bring the long-term spectrum of held notes, through Fides's
//! body, to the references' (the Iowa MIS's anechoic cello and double bass, TinySOL's,
//! VSCO 2's and Karoryfer's and Smolken's) — the measured body's response the modal body
//! does not have. All gains 0 (model 1): no section runs, the same bits.

use super::tables::{Match, EQ_BANDS, EQ_HZ, EQ_Q};
use crate::math;

#[derive(Clone, Debug, Default)]
pub struct MatchEq {
    on: bool,
    /// `b0, b1, b2, a1, a2` per section (RBJ's peaking filter, `a0` normalised).
    c: [[f32; 5]; EQ_BANDS],
    /// Transposed direct form II states, per channel and section.
    z: [[[f32; 2]; EQ_BANDS]; 2],
}

impl MatchEq {
    pub fn design(&mut self, m: &Match, sr: f32) {
        self.on = m.eq_db.iter().any(|g| *g != 0.0);
        for (i, g) in m.eq_db.iter().enumerate() {
            let hz = EQ_HZ[i].min(0.45 * sr);
            let a = math::exp2(*g / 40.0 * std::f32::consts::LOG2_10);
            let w = std::f32::consts::TAU * hz / sr;
            let (s, c) = math::sin_cos(w);
            let alpha = s / (2.0 * EQ_Q);
            let a0 = 1.0 + alpha / a;
            self.c[i] = [
                (1.0 + alpha * a) / a0,
                -2.0 * c / a0,
                (1.0 - alpha * a) / a0,
                -2.0 * c / a0,
                (1.0 - alpha / a) / a0,
            ];
        }
        if !self.on {
            self.z = Default::default();
        }
    }

    pub fn is_still(&self) -> bool {
        self.z.iter().flatten().all(|s| *s == [0.0, 0.0])
    }

    pub fn reset(&mut self) {
        self.z = Default::default();
    }

    /// In place on both channels; `quiet` (the input all zeros) skips it once its states
    /// have let go — each at exact zeros, sample by sample, so where it rests is the same at
    /// every block size.
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32], quiet: bool) {
        if !self.on || (quiet && self.is_still()) {
            return;
        }
        for (ch, buf) in [l, r].into_iter().enumerate() {
            for (i, c) in self.c.iter().enumerate() {
                let [mut z1, mut z2] = self.z[ch][i];
                for x in buf.iter_mut() {
                    let input = *x;
                    let y = c[0] * input + z1;
                    z1 = c[1] * input - c[3] * y + z2;
                    z2 = c[2] * input - c[4] * y;
                    if input == 0.0 && z1.abs() < 1e-20 && z2.abs() < 1e-20 {
                        z1 = 0.0;
                        z2 = 0.0;
                    }
                    *x = y;
                }
                self.z[ch][i] = [z1, z2];
            }
        }
    }
}
