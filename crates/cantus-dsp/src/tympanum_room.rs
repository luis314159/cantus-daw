//! A modelled kit's room (ADR 0124 §2): its early reflections are the virtual mics' image
//! paths (`model`); this is the late field every mic in the room hears — eight delay lines
//! whose lengths follow the room's size, a Hadamard matrix, and a damping filter in each
//! loop fitted so the tail falls 60 dB in the room's RT60 (Jot's rule: each line's loss
//! proportional to its length), quicker in the highs, as wood and air make it.
//!
//! Deterministic and allocation-free once built: integer delays, one-pole filters,
//! nothing that depends on how a call is cut.

use crate::math;

const LINES: usize = 8;
/// Line lengths for a 1 m room, ms: spread so no two loops share a period.
const BASE_MS: [f32; LINES] = [3.17, 3.71, 4.13, 4.61, 5.23, 5.89, 6.37, 7.03];
/// The longest a line may be: a hall 25 m across.
const MAX_SCALE: f32 = 25.0;

#[derive(Clone, Debug)]
pub struct Room {
    fs: f32,
    buf: Vec<f32>,
    cap: usize,
    len: [usize; LINES],
    pos: [usize; LINES],
    gain: [f32; LINES],
    /// The loop's one-pole low-pass per line (`y += a (x − y)`).
    lp_a: [f32; LINES],
    lp: [f32; LINES],
    /// The pre-delay before the tail (the room's first reflections come by the paths).
    pre: Vec<f32>,
    pre_len: usize,
    pre_pos: usize,
    /// Frames since anything went in, for the rest rule.
    quiet: usize,
    energy: f32,
}

impl Room {
    /// Allocates: with the unit, never on the audio thread.
    pub fn new(fs: f32) -> Room {
        let longest = (BASE_MS[LINES - 1] * MAX_SCALE * fs / 1000.0).ceil() as usize + 1;
        let pre_cap = (0.06 * fs) as usize + 1;
        Room {
            fs,
            buf: vec![0.0; longest * LINES],
            cap: longest,
            len: [1; LINES],
            pos: [0; LINES],
            gain: [0.0; LINES],
            lp_a: [1.0; LINES],
            lp: [0.0; LINES],
            pre: vec![0.0; pre_cap],
            pre_len: 1,
            pre_pos: 0,
            quiet: usize::MAX / 2,
            energy: 0.0,
        }
    }

    /// The room: its mean dimension (m), its RT60 at low and at high frequencies (s).
    /// Changing it does not clear the tail (a size moved while it rings glides by itself).
    pub fn set(&mut self, size_m: f32, t60: f32, t60_high: f32) {
        let scale = size_m.clamp(1.0, MAX_SCALE);
        let t60 = t60.max(0.05);
        let t60_high = t60_high.clamp(0.03, t60);
        for (i, base) in BASE_MS.iter().enumerate() {
            let len = ((base * scale * self.fs / 1000.0) as usize).clamp(8, self.cap);
            self.len[i] = len;
            self.pos[i] %= len;
            let secs = len as f32 / self.fs;
            // 60 dB in t60: the gain per pass of a line `secs` long.
            self.gain[i] = math::db_to_gain(-60.0 * secs / t60);
            // At high frequencies the pass loses more: the one-pole's loss at ~6 kHz makes
            // up the difference between the two decays.
            let hi = math::db_to_gain(-60.0 * secs / t60_high) / self.gain[i];
            let w = 2.0 * std::f32::consts::PI * 6_000.0 / self.fs;
            // |H(w)| of y += a(x − y) ≈ a / sqrt(a² + 2(1 − a)(1 − cos w)); solve for a.
            let c = 2.0 * (1.0 - math::cos(w));
            let h2 = (hi * hi).clamp(1e-4, 0.9999);
            // h² (a² + c(1 − a)) = a²  →  a²(1 − h²) + h² c a − h² c = 0.
            let (qa, qb, qc) = (1.0 - h2, h2 * c, -h2 * c);
            self.lp_a[i] = ((-qb + (qb * qb - 4.0 * qa * qc).sqrt()) / (2.0 * qa)).clamp(0.01, 1.0);
        }
        self.pre_len =
            ((size_m * 2.2 / 343.0) * self.fs).clamp(1.0, (self.pre.len() - 1) as f32) as usize;
    }

    pub fn clear(&mut self) {
        self.buf.iter_mut().for_each(|x| *x = 0.0);
        self.pre.iter_mut().for_each(|x| *x = 0.0);
        self.lp = [0.0; LINES];
        self.quiet = usize::MAX / 2;
        self.energy = 0.0;
    }

    /// Whether it rings no more (cleared to exact zeros once it fell below hearing).
    pub fn at_rest(&self) -> bool {
        self.quiet >= usize::MAX / 2
    }

    /// One sample in (mono), the tail out (left, right).
    #[inline]
    pub fn tick(&mut self, x: f32) -> (f32, f32) {
        if x != 0.0 {
            self.quiet = 0;
        } else if self.quiet >= usize::MAX / 2 {
            return (0.0, 0.0);
        }
        // Pre-delay.
        let d = self.pre[self.pre_pos];
        self.pre[self.pre_pos] = x;
        self.pre_pos = (self.pre_pos + 1) % self.pre_len.max(1);
        let mut o = [0.0f32; LINES];
        for (i, v) in o.iter_mut().enumerate() {
            *v = self.buf[i * self.cap + self.pos[i]];
        }
        // Hadamard 8 (normalised): three butterfly stages.
        let mut h = o;
        let mut span = 1;
        while span < LINES {
            for start in (0..LINES).step_by(span * 2) {
                for k in start..start + span {
                    let (a, b) = (h[k], h[k + span]);
                    h[k] = a + b;
                    h[k + span] = a - b;
                }
            }
            span *= 2;
        }
        let norm = 1.0 / (LINES as f32).sqrt();
        let mut e = 0.0f32;
        for (i, hi) in h.iter().enumerate() {
            let fb = hi * norm * self.gain[i];
            self.lp[i] += self.lp_a[i] * (fb - self.lp[i]);
            let v = self.lp[i] + d * if i % 2 == 0 { 0.5 } else { -0.5 };
            self.buf[i * self.cap + self.pos[i]] = v;
            self.pos[i] = (self.pos[i] + 1) % self.len[i];
            e += v * v;
        }
        let l = o[0] - o[2] + o[4] - o[6] + 0.5 * (o[1] + o[5]);
        let r = o[1] - o[3] + o[5] - o[7] + 0.5 * (o[2] - o[6]);
        // The rest rule: a few seconds of silence in and nothing left to hear.
        self.energy = e;
        if x == 0.0 {
            self.quiet += 1;
            if self.quiet > (self.fs as usize) / 4 && self.energy < 1e-14 {
                self.clear();
            }
        }
        (l * 0.35, r * 0.35)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tail_decays_by_its_rt60_and_comes_to_rest() {
        let fs = 48_000.0;
        let mut room = Room::new(fs);
        room.set(6.0, 0.5, 0.3);
        assert!(room.at_rest());
        let mut energy = [0.0f64; 4];
        for t in 0..(fs as usize * 2) {
            let x = if t == 0 { 1.0 } else { 0.0 };
            let (l, r) = room.tick(x);
            let w = t / (fs as usize / 4);
            if w < 4 {
                energy[w] += (l * l + r * r) as f64;
            }
        }
        // Each quarter second about 30 dB under the one before (60 dB per 0.5 s).
        let drop = 10.0 * math::log10(energy[1] / energy[2]);
        assert!(drop > 20.0 && drop < 40.0, "{drop} dB");
        assert!(room.at_rest(), "it rests after it falls silent");
        assert_eq!(room.tick(0.0), (0.0, 0.0));
    }
}
