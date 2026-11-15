//! One named output's internal mixer on its lane (round 40 phase 3, ADR 0123): level, pan,
//! width, polarity, mute — what Tympanum's mics, a recorded Chorda's perspectives and noises
//! and (later) the model's virtual mics share. Owned by `tymp-engine`; `chorda-src` uses it
//! for Exemplar's buses.
//!
//! **Null at its defaults, to the bit**: an [`OutMix`] at 0 dB, centred, full width, not
//! inverted, not muted is [`OutMix::is_unity`], and [`OutMix::apply`] then touches no
//! sample. The pan here is a **balance** (0 dB at centre, ADR 0100 §3's law for strips made
//! for outputs), so a centred lane passes untouched. The delay (a virtual mic's distance,
//! `o{n}.delay`) and the solo (across outputs) are `tymp-engine`'s to add: the delay needs
//! a preallocated line per lane, the solo the instrument's whole set.

use crate::math;

/// One lane's mix, set off the audio thread or at a snapshot (no allocation).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct OutMix {
    pub level_db: f32,
    /// −1 left … 1 right: a balance.
    pub pan: f32,
    /// 0 mono … 1 the lane's own stereo.
    pub width: f32,
    pub invert: bool,
    pub mute: bool,
}

impl Default for OutMix {
    fn default() -> Self {
        OutMix {
            level_db: 0.0,
            pan: 0.0,
            width: 1.0,
            invert: false,
            mute: false,
        }
    }
}

impl OutMix {
    /// Whether [`apply`](Self::apply) leaves every sample as it is.
    pub fn is_unity(&self) -> bool {
        *self == OutMix::default()
    }

    /// Mix one lane in place. Unity touches nothing.
    pub fn apply(&self, left: &mut [f32], right: &mut [f32]) {
        if self.is_unity() {
            return;
        }
        if self.mute {
            left.fill(0.0);
            right.fill(0.0);
            return;
        }
        let g = math::db_to_gain(self.level_db) * if self.invert { -1.0 } else { 1.0 };
        let pan = self.pan.clamp(-1.0, 1.0);
        let gl = g * (1.0 - pan.max(0.0));
        let gr = g * (1.0 + pan.min(0.0));
        let w = self.width.clamp(0.0, 1.0);
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let (a, b) = if w < 1.0 {
                let m = 0.5 * (*l + *r);
                (m + w * (*l - m), m + w * (*r - m))
            } else {
                (*l, *r)
            };
            *l = a * gl;
            *r = b * gr;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unity_touches_no_sample() {
        let mut l = [0.1f32, -0.3, f32::MIN_POSITIVE, 0.7];
        let mut r = [0.2f32, 0.4, -0.5, 1e-30];
        let (l0, r0) = (l, r);
        OutMix::default().apply(&mut l, &mut r);
        assert_eq!(l.map(f32::to_bits), l0.map(f32::to_bits));
        assert_eq!(r.map(f32::to_bits), r0.map(f32::to_bits));
        let mut m = OutMix {
            invert: true,
            ..OutMix::default()
        };
        m.apply(&mut l, &mut r);
        assert_eq!(l[0], -0.1);
        m.mute = true;
        m.apply(&mut l, &mut r);
        assert!(l.iter().chain(&r).all(|x| *x == 0.0));
    }
}
