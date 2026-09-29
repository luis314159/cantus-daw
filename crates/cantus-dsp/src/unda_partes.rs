//! Partes — Unda's additive oscillator engine (`research/instruments/fm-additive-
//! granular.md` §4.7, §8.2): a bank of up to 256 sine partials, each a recursive rotator
//! (`z ← z·w`, four multiplies), culled above 20 kHz with a taper so a glide never clicks.
//!
//! Its spectrum is the current frame of the oscillator's table — so Partes plays the
//! table as partials, exactly (at stretch 0, with the table's phases) — shaped by a tilt
//! in dB per octave, an odd/even balance and a **stretch** that moves partial *k* to
//! `k^(1 + s)` times the fundamental: inharmonic bells and pianos no table can hold.
//!
//! Cost follows the note, not the knob: a partial above 20 kHz is never computed. The
//! rotators are renormalised on the control grid, deterministically.

use super::table::{Table, HARMONICS};

pub const MAX_PARTIALS: usize = 256;
/// Partials fade out between these frequencies (Hz) and are culled above.
const TAPER: (f32, f32) = (18_000.0, 20_000.0);

/// One control tick's settings for a Partes oscillator (already modulated).
#[derive(Copy, Clone, Debug, Default)]
pub struct PartesControl {
    pub hz: f32,
    pub gain: f32,
    pub pan: f32,
    /// Table position, 0…1: which frame's spectrum.
    pub pos: f32,
    pub partials: usize,
    pub tilt_db: f32,
    /// −1 odd partials only … +1 even partials (and the fundamental) only.
    pub oddeven: f32,
    /// Inharmonicity: partial k at `k^(1 + stretch)` × the fundamental.
    pub stretch: f32,
}

#[derive(Clone, Debug)]
pub struct Partes {
    c: [f32; MAX_PARTIALS],
    s: [f32; MAX_PARTIALS],
    cw: [f32; MAX_PARTIALS],
    sw: [f32; MAX_PARTIALS],
    amp: [f32; MAX_PARTIALS],
    damp: [f32; MAX_PARTIALS],
    /// Partials running (the rest are above the cull or beyond the count).
    n: usize,
    gl: f32,
    gr: f32,
    /// What the rotation factors were computed for, so a still note does not recompute.
    last_hz: f32,
    last_stretch: f32,
    last_sr: f32,
    /// The previous output (what another oscillator's FM reads).
    pub out0: f32,
}

impl Default for Partes {
    fn default() -> Self {
        Partes {
            c: [1.0; MAX_PARTIALS],
            s: [0.0; MAX_PARTIALS],
            cw: [1.0; MAX_PARTIALS],
            sw: [0.0; MAX_PARTIALS],
            amp: [0.0; MAX_PARTIALS],
            damp: [0.0; MAX_PARTIALS],
            n: 0,
            gl: 0.0,
            gr: 0.0,
            last_hz: -1.0,
            last_stretch: 0.0,
            last_sr: 0.0,
            out0: 0.0,
        }
    }
}

impl Partes {
    /// A note starts: each partial at the table's own phase for it (so stretch 0 is the
    /// table's waveform), offset by `phase0` cycles of the fundamental.
    pub fn start(&mut self, table: &Table, pos: f32, phase0: f32) {
        let frame = frame_of(table, pos).0;
        for k in 0..MAX_PARTIALS {
            let (a, b) = table.partial(frame, k);
            let phi = a.atan2(b) + std::f32::consts::TAU * phase0 * (k + 1) as f32;
            self.c[k] = phi.cos();
            self.s[k] = phi.sin();
            self.amp[k] = 0.0;
            self.damp[k] = 0.0;
        }
        self.last_hz = -1.0;
        self.out0 = 0.0;
    }

    /// Set the bank for the next `steps` samples at `sr`. `first`: land at once.
    pub fn control(&mut self, c: &PartesControl, table: &Table, sr: f32, steps: u32, first: bool) {
        let count = c.partials.clamp(1, MAX_PARTIALS).min(HARMONICS);
        // Partials that have faded out stop being computed.
        while self.n > 0 && self.amp[self.n - 1] <= 0.0 && self.damp[self.n - 1] <= 0.0 {
            self.amp[self.n - 1] = 0.0;
            self.damp[self.n - 1] = 0.0;
            self.n -= 1;
        }
        let (f0, fw) = frame_of(table, c.pos);
        let f1 = (f0 + 1).min(table.frames() - 1);
        let recompute =
            c.hz != self.last_hz || c.stretch != self.last_stretch || sr != self.last_sr;
        let w0 = std::f32::consts::TAU * c.hz / sr;
        let inv = 1.0 / steps.max(1) as f32;
        let mut n = 0;
        for k in 0..count {
            let kk = (k + 1) as f32;
            let ratio = if c.stretch == 0.0 {
                kk
            } else {
                kk.powf(1.0 + c.stretch)
            };
            let hz = c.hz * ratio;
            if hz >= TAPER.1 || hz >= 0.49 * sr {
                break;
            }
            n = k + 1;
            if recompute {
                let w = w0 * ratio;
                self.cw[k] = w.cos();
                self.sw[k] = w.sin();
            }
            // Renormalise the rotator (one Newton step towards the unit circle).
            let g = 1.5 - 0.5 * (self.c[k] * self.c[k] + self.s[k] * self.s[k]);
            self.c[k] *= g;
            self.s[k] *= g;
            let m = {
                let (a0, b0) = table.partial(f0, k);
                let m0 = (a0 * a0 + b0 * b0).sqrt();
                if fw > 0.0 {
                    let (a1, b1) = table.partial(f1, k);
                    m0 + ((a1 * a1 + b1 * b1).sqrt() - m0) * fw
                } else {
                    m0
                }
            };
            let tilt = if c.tilt_db == 0.0 {
                1.0
            } else {
                10f32.powf(c.tilt_db * kk.log2() / 20.0)
            };
            let odd = (k + 1) % 2 == 1;
            let oe = if k == 0 {
                1.0
            } else if odd {
                1.0 - c.oddeven.max(0.0)
            } else {
                1.0 + c.oddeven.min(0.0)
            };
            let taper = if hz > TAPER.0 {
                (TAPER.1 - hz) / (TAPER.1 - TAPER.0)
            } else {
                1.0
            };
            let target = m * tilt * oe * taper;
            if first {
                self.amp[k] = target;
                self.damp[k] = 0.0;
            } else {
                self.damp[k] = (target - self.amp[k]) * inv;
            }
        }
        // Partials that just left the band fade over the tick rather than stop.
        for k in n..self.n.max(n) {
            self.damp[k] = -self.amp[k] * inv;
        }
        self.n = n.max(self.n);
        if first {
            self.n = n;
        }
        self.last_hz = c.hz;
        self.last_stretch = c.stretch;
        self.last_sr = sr;
        let th = (c.pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
        self.gl = c.gain * th.cos() * std::f32::consts::SQRT_2;
        self.gr = c.gain * th.sin() * std::f32::consts::SQRT_2;
    }

    /// One sample: (left, right).
    #[inline]
    pub fn tick(&mut self) -> (f32, f32) {
        let mut y = 0.0;
        for k in 0..self.n {
            let (c, s) = (self.c[k], self.s[k]);
            let (cw, sw) = (self.cw[k], self.sw[k]);
            self.c[k] = c * cw - s * sw;
            self.s[k] = c * sw + s * cw;
            y += self.amp[k] * self.s[k];
            self.amp[k] = (self.amp[k] + self.damp[k]).max(0.0);
        }
        self.out0 = y;
        (y * self.gl, y * self.gr)
    }

    /// Partials running now (a CPU cue).
    pub fn running(&self) -> usize {
        self.n
    }
}

/// The frame a position reads and the weight of the next.
fn frame_of(table: &Table, pos: f32) -> (usize, f32) {
    let frames = table.frames().max(1);
    let fp = pos.clamp(0.0, 1.0) * (frames - 1) as f32;
    let k = (fp as usize).min(frames - 1);
    (k, if k + 1 < frames { fp - k as f32 } else { 0.0 })
}
