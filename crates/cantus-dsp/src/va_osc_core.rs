//! The oscillator and the sub-oscillator (children of `va::osc`).

use super::{Blep, Crossings, Shape};

/// One analog oscillator: a phase and its last naive value.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct VaOsc {
    /// 0…1.
    pub phase: f64,
    /// The naive value at the end of the last sample — what modulates the others (FM,
    /// poly-mod, filter FM), where aliasing does not matter.
    pub last: f32,
}

impl VaOsc {
    pub fn new(phase: f64) -> Self {
        VaOsc { phase, last: 0.0 }
    }

    /// One sample at `dt` cycles per sample. Adds `gain ×` its waves into `blep`, the
    /// discontinuities corrected. `sync`: the master wrapped at `τ` this sample, so the
    /// phase restarts there. Returns where this oscillator crossed its wrap and its middle.
    #[inline]
    pub fn tick(
        &mut self,
        dt: f64,
        shape: &Shape,
        gain: f32,
        blep: &mut Blep,
        sync: Option<f32>,
    ) -> Crossings {
        let dt = dt.clamp(1e-9, 0.45);
        let dtf = dt as f32;
        let pw = shape
            .pw
            .clamp((2.0 * dtf).min(0.5), (1.0 - 2.0 * dtf).max(0.5));
        let loud = !(shape.is_silent() || gain == 0.0);
        let cross = match sync {
            None => {
                let (p, c) = segment(self.phase, dt, 0.0, 1.0, shape, pw, gain, blep, loud);
                self.phase = p;
                c
            }
            Some(ts) => {
                let (pre, _) = segment(self.phase, dt, 0.0, ts, shape, pw, gain, blep, loud);
                if loud {
                    let p = pre as f32;
                    blep.step(gain * (shape.value(0.0, pw) - shape.value(p, pw)), ts);
                    blep.ramp(gain * (shape.slope(0.0) - shape.slope(p)) * dtf, ts);
                }
                let (p, c) = segment(0.0, dt, ts, 1.0, shape, pw, gain, blep, loud);
                self.phase = p;
                c
            }
        };
        let v = shape.value(self.phase as f32, pw);
        self.last = v;
        if loud {
            blep.add(gain * v);
        }
        cross
    }
}

/// The phase moving from `p0` over the part `[t0, t1]` of a sample: every discontinuity it
/// crosses, corrected. Returns the phase at `t1` and the crossings.
#[allow(clippy::too_many_arguments)]
#[inline]
fn segment(
    p0: f64,
    dt: f64,
    t0: f32,
    t1: f32,
    shape: &Shape,
    pw: f32,
    gain: f32,
    blep: &mut Blep,
    loud: bool,
) -> (f64, Crossings) {
    let pe = p0 + (t1 - t0) as f64 * dt;
    let at = |q: f64| -> Option<f32> {
        (q > p0 && q <= pe).then(|| (t0 + ((q - p0) / dt) as f32).clamp(t0, t1))
    };
    let mut c = Crossings::default();
    let dtf = dt as f32;
    let corner = 8.0 * (1.0 - 2.0 * shape.round);
    if let Some(tau) = at(1.0) {
        c.wrap = Some(tau);
        if loud {
            blep.step(gain * (2.0 * shape.pulse - 2.0 * shape.saw), tau);
            let d = shape.tri * corner - shape.saw * 8.0 * shape.curve;
            blep.ramp(gain * d * dtf, tau);
        }
    }
    if loud && shape.pulse != 0.0 {
        let pw = pw as f64;
        if let Some(tau) = at(pw).or_else(|| at(pw + 1.0)) {
            blep.step(-2.0 * gain * shape.pulse, tau);
        }
    }
    if let Some(tau) = at(0.5).or_else(|| at(1.5)) {
        c.half = Some(tau);
        if loud && shape.tri != 0.0 {
            blep.ramp(-gain * shape.tri * corner * dtf, tau);
        }
    }
    let p = if pe >= 1.0 { pe - 1.0 } else { pe };
    (p, c)
}

/// The sub-oscillator's shapes: it follows oscillator 1 an octave or two down.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum SubShape {
    /// A square an octave down.
    #[default]
    Square1,
    /// A square two octaves down.
    Square2,
    /// A 25 % pulse an octave down.
    Pulse1,
}

impl SubShape {
    pub fn from_index(i: f32) -> SubShape {
        match i.round() as i32 {
            1 => SubShape::Square2,
            2 => SubShape::Pulse1,
            _ => SubShape::Square1,
        }
    }
}

/// A divider on oscillator 1's wraps.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct SubOsc {
    count: u8,
}

impl SubOsc {
    pub fn reset(&mut self) {
        self.count = 0;
    }

    #[inline]
    fn value(shape: SubShape, count: u8, first_half: bool) -> f32 {
        match shape {
            SubShape::Square1 => {
                if count.is_multiple_of(2) {
                    1.0
                } else {
                    -1.0
                }
            }
            SubShape::Square2 => {
                if (count / 2).is_multiple_of(2) {
                    1.0
                } else {
                    -1.0
                }
            }
            // A quarter of the time high; the mean (−½) taken out.
            SubShape::Pulse1 => {
                if count.is_multiple_of(2) && first_half {
                    1.5
                } else {
                    -0.5
                }
            }
        }
    }

    /// One sample, after oscillator 1's: its crossings and its phase at the end of it.
    #[inline]
    pub fn tick(
        &mut self,
        shape: SubShape,
        gain: f32,
        master: Crossings,
        master_phase: f64,
        blep: &mut Blep,
    ) {
        if let Some(tau) = master.wrap {
            let before = Self::value(shape, self.count, false);
            self.count = (self.count + 1) & 3;
            let after = Self::value(shape, self.count, true);
            blep.step(gain * (after - before), tau);
        }
        if let Some(tau) = master.half {
            let before = Self::value(shape, self.count, true);
            let after = Self::value(shape, self.count, false);
            blep.step(gain * (after - before), tau);
        }
        blep.add(gain * Self::value(shape, self.count, master_phase < 0.5));
    }
}
