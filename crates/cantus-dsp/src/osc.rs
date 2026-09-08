//! Band-limited oscillators using PolyBLEP (polynomial band-limited step) correction.
//!
//! PolyBLEP is cheap, allocation-free and good enough for a subtractive synth; wavetable
//! and additive engines come later as separate modules.

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Wave {
    Sine,
    Triangle,
    Saw,
    Square,
}

#[derive(Clone, Debug)]
pub struct PolyBlepOsc {
    phase: f32,
    inc: f32,
    wave: Wave,
    /// Leaky integrator state for the triangle (integrated square).
    tri_state: f32,
    sample_rate: f32,
}

impl PolyBlepOsc {
    pub fn new(sample_rate: f32) -> Self {
        PolyBlepOsc {
            phase: 0.0,
            inc: 0.0,
            wave: Wave::Saw,
            tri_state: -1.0,
            sample_rate,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    pub fn set_wave(&mut self, wave: Wave) {
        self.wave = wave;
    }

    pub fn set_frequency(&mut self, hz: f32) {
        // Never let the increment reach 0.5 (Nyquist) or PolyBLEP breaks down.
        self.inc = (hz / self.sample_rate).clamp(0.0, 0.49);
    }

    pub fn reset(&mut self, phase: f32) {
        self.phase = phase.rem_euclid(1.0);
        // Start the integrator on the ideal triangle so there is no DC transient.
        let t = self.phase;
        self.tri_state = if t < 0.5 {
            -1.0 + 4.0 * t
        } else {
            3.0 - 4.0 * t
        };
    }

    #[inline]
    pub fn next(&mut self) -> f32 {
        let t = self.phase;
        let dt = self.inc;
        let out = match self.wave {
            Wave::Sine => (t * std::f32::consts::TAU).sin(),
            Wave::Saw => {
                let naive = 2.0 * t - 1.0;
                naive - poly_blep(t, dt)
            }
            Wave::Square => {
                let naive = if t < 0.5 { 1.0 } else { -1.0 };
                naive + poly_blep(t, dt) - poly_blep((t + 0.5).rem_euclid(1.0), dt)
            }
            Wave::Triangle => {
                let sq = {
                    let naive = if t < 0.5 { 1.0 } else { -1.0 };
                    naive + poly_blep(t, dt) - poly_blep((t + 0.5).rem_euclid(1.0), dt)
                };
                // Leaky integration of the band-limited square gives a band-limited
                // triangle; scale keeps amplitude ≈ ±1 independent of frequency.
                let leak = 0.999;
                self.tri_state = leak * self.tri_state + (4.0 * dt) * sq;
                self.tri_state
            }
        };
        self.phase += dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        out
    }
}

/// Two-sided polynomial correction of a step discontinuity at phase 0.
#[inline]
fn poly_blep(t: f32, dt: f32) -> f32 {
    if dt <= 0.0 {
        return 0.0;
    }
    if t < dt {
        let x = t / dt;
        x + x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + x + x + 1.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(wave: Wave, hz: f32, n: usize) -> Vec<f32> {
        let mut o = PolyBlepOsc::new(48_000.0);
        o.set_wave(wave);
        o.set_frequency(hz);
        (0..n).map(|_| o.next()).collect()
    }

    #[test]
    fn amplitudes_are_sane() {
        for w in [Wave::Sine, Wave::Saw, Wave::Square, Wave::Triangle] {
            let buf = render(w, 220.0, 48_000);
            let peak = buf.iter().fold(0f32, |m, x| m.max(x.abs()));
            assert!(peak <= 1.2 && peak > 0.7, "{w:?} peak {peak}");
            assert!(buf.iter().all(|x| x.is_finite()));
        }
    }

    #[test]
    fn saw_has_less_aliasing_than_naive() {
        // Choose a frequency that is exactly periodic in the analysis window so every
        // harmonic (and every aliased harmonic) lands on an exact DFT bin.
        let sr = 48_000.0f32;
        let n = 4096usize;
        let cycles = 171usize;
        let f0 = sr * cycles as f32 / n as f32; // about 2004 Hz
        let blep = render(Wave::Saw, f0, n);
        let mut phase = 0f32;
        let naive: Vec<f32> = (0..n)
            .map(|_| {
                let v = 2.0 * phase - 1.0;
                phase = (phase + f0 / sr).rem_euclid(1.0);
                v
            })
            .collect();
        // Energy in bins that are not multiples of the fundamental bin is aliasing.
        let alias_energy = |buf: &[f32]| {
            let mut e = 0f64;
            for k in 1..n / 2 {
                if k % cycles == 0 {
                    continue;
                }
                let (mut re, mut im) = (0f64, 0f64);
                for (i, x) in buf.iter().enumerate() {
                    let a = -std::f64::consts::TAU * k as f64 * i as f64 / n as f64;
                    re += *x as f64 * a.cos();
                    im += *x as f64 * a.sin();
                }
                e += re * re + im * im;
            }
            e
        };
        let a_blep = alias_energy(&blep);
        let a_naive = alias_energy(&naive);
        assert!(a_blep < a_naive * 0.1, "blep {a_blep} vs naive {a_naive}");
    }
}
