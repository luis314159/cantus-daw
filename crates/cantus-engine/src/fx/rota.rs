//! Rota, the rotary speaker (ADR 0133), behind the engine's plug-in contract.
//! Owned by the rota builder.
//!
//! [`settings`] reads the table (`cantus_core::fx::rota`) into the DSP's settings
//! (`cantus_dsp::rota`). **Notes** (ADR 0022) throw the speed switch, to the sample, when
//! `notes` is on: each note played toggles slow and fast from where the parameter has it.
//! No latency.
//!
//! ## Telemetry
//!
//! | Index | What |
//! |---|---|
//! | 0 | sample rate, Hz |
//! | 1, 2 | the horn's angle (turns) and speed (rpm) |
//! | 3, 4 | the drum's angle (turns) and speed (rpm) |
//! | 5 | the speed switch as it is now: 0 stop, 1 slow, 2 fast |

use super::{FxNote, FxUnit, Telemetry, TELEMETRY_LEN};
use cantus_core::EffectSpec;
use cantus_dsp::organum::rotary::RotorSpeed;
use cantus_dsp::rota::{Rota, RotaSettings};

pub const T_SAMPLE_RATE: usize = 0;
pub const T_HORN: usize = 1;
pub const T_DRUM: usize = 3;
pub const T_SPEED: usize = 5;

/// The published spin-up and spin-down times the `ramp` scales (s): horn up, horn down,
/// drum up, drum down.
const RAMPS: [f32; 4] = [0.161, 0.321, 4.127, 1.371];
/// Notes held for one block at most (more in one block are folded: only the switch's last
/// state matters).
const PENDING: usize = 32;

/// Rota's parameters as the DSP wants them, and whether notes throw the switch.
pub fn settings(spec: &EffectSpec) -> (RotaSettings, bool) {
    let ramp = spec.get("ramp").clamp(0.25, 4.0);
    let speed = match spec.get("speed").round() as i32 {
        0 => RotorSpeed::Stop,
        2 => RotorSpeed::Fast,
        _ => RotorSpeed::Slow,
    };
    (
        RotaSettings {
            speed,
            horn_slow_rpm: spec.get("hslow"),
            horn_fast_rpm: spec.get("hfast"),
            drum_slow_rpm: spec.get("dslow"),
            drum_fast_rpm: spec.get("dfast"),
            horn_up_s: RAMPS[0] * ramp,
            horn_down_s: RAMPS[1] * ramp,
            drum_up_s: RAMPS[2] * ramp,
            drum_down_s: RAMPS[3] * ramp,
            balance_db: spec.get("balance"),
            spread_deg: spec.get("spread"),
            distance_cm: spec.get("dist"),
            open: spec.get("open") >= 0.5,
            drive: spec.get("drive"),
            mix: spec.get("mix"),
            out_db: spec.get("out"),
        },
        spec.get("notes") >= 0.5,
    )
}

#[derive(Clone)]
pub struct Unit {
    dsp: Box<Rota>,
    set: RotaSettings,
    notes: bool,
    /// The switch thrown by notes an odd number of times (slow and fast exchanged).
    flipped: bool,
    pending: [usize; PENDING],
    np: usize,
}

impl std::fmt::Debug for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "rota::Unit({:?}, flipped {})", self.set, self.flipped)
    }
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        let dsp = Box::new(Rota::new(sample_rate));
        Unit {
            set: *dsp.settings(),
            dsp,
            notes: true,
            flipped: false,
            pending: [0; PENDING],
            np: 0,
        }
    }

    fn apply(&mut self) {
        let mut s = self.set;
        if self.flipped {
            s.speed = match s.speed {
                RotorSpeed::Fast => RotorSpeed::Slow,
                RotorSpeed::Slow => RotorSpeed::Fast,
                RotorSpeed::Stop => RotorSpeed::Stop,
            };
        }
        self.dsp.set(&s);
    }
}

impl FxUnit for Unit {
    fn fork(&self) -> Option<Box<dyn FxUnit>> {
        Some(Box::new(self.clone()))
    }

    fn configure(&mut self, spec: &EffectSpec) {
        let (s, notes) = settings(spec);
        self.set = s;
        self.notes = notes;
        if !notes {
            self.flipped = false;
        }
        self.apply();
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], _key: Option<(&[f32], &[f32])>) {
        let n = left.len().min(right.len());
        let mut at = 0;
        for k in 0..self.np {
            let off = self.pending[k].min(n);
            if off > at {
                self.dsp.process(&mut left[at..off], &mut right[at..off]);
                at = off;
            }
            self.flipped = !self.flipped;
            self.apply();
        }
        self.np = 0;
        if at < n {
            self.dsp.process(&mut left[at..n], &mut right[at..n]);
        }
    }

    fn note(&mut self, offset: usize, note: FxNote) {
        if self.notes && matches!(note, FxNote::On { .. }) {
            if self.np < PENDING {
                self.pending[self.np] = offset;
                self.np += 1;
            } else {
                // Folded: two more throws are none.
                self.np -= 1;
            }
        }
    }

    fn reset(&mut self) {
        self.flipped = false;
        self.np = 0;
        self.apply();
        self.dsp.reset();
    }

    fn at_rest(&self) -> bool {
        self.np == 0 && self.dsp.at_rest()
    }

    fn rest_blocker(&self) -> Option<&'static str> {
        if self.np > 0 {
            return Some("a note waits to throw the switch");
        }
        self.dsp.rest_blocker()
    }

    fn skip(&mut self, frames: usize) {
        self.dsp.skip(frames);
    }

    fn latency(&self) -> usize {
        0
    }

    fn telemetry(&self) -> Telemetry {
        let mut t = [0.0; TELEMETRY_LEN];
        let [(ha, hr), (da, dr)] = self.dsp.rotors();
        t[T_SAMPLE_RATE] = self.dsp.sample_rate();
        t[T_HORN] = ha as f32;
        t[T_HORN + 1] = hr as f32;
        t[T_DRUM] = da as f32;
        t[T_DRUM + 1] = dr as f32;
        t[T_SPEED] = match self.dsp.settings().speed {
            RotorSpeed::Stop => 0.0,
            RotorSpeed::Slow => 1.0,
            RotorSpeed::Fast => 2.0,
        };
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::EffectKind;

    #[test]
    fn the_defaults_touch_no_sample() {
        let mut u = Unit::new(48_000.0);
        u.configure(&EffectSpec::new(EffectKind::Rotary));
        let x: Vec<f32> = (0..256).map(|i| (i as f32 * 0.37).fract() - 0.5).collect();
        let (mut l, mut r) = (x.clone(), x.clone());
        u.process(&mut l, &mut r, None);
        assert!(l
            .iter()
            .chain(&r)
            .zip(x.iter().chain(&x))
            .all(|(a, b)| a.to_bits() == b.to_bits()));
        assert_eq!(u.latency(), 0);
    }

    /// A note throws the switch at its sample, wherever the block is cut.
    #[test]
    fn a_note_throws_the_switch_at_its_sample() {
        let mut spec = EffectSpec::new(EffectKind::Rotary);
        spec.set("mix", 1.0);
        let x: Vec<f32> = (0..4_096)
            .map(|i| cantus_dsp::math::sin(i as f32 * 0.05) * 0.3)
            .collect();
        let mut outs = vec![];
        for block in [64usize, 1_000] {
            let mut u = Unit::new(48_000.0);
            u.configure(&spec);
            let (mut l, mut r) = (x.clone(), x.clone());
            let mut start = 0;
            while start < x.len() {
                let end = (start + block).min(x.len());
                if (start..end).contains(&1_500) {
                    u.note(
                        1_500 - start,
                        FxNote::On {
                            pitch: 60,
                            velocity: 90,
                        },
                    );
                }
                u.process(&mut l[start..end], &mut r[start..end], None);
                start = end;
            }
            assert_eq!(u.telemetry()[T_SPEED], 2.0);
            outs.push(l);
        }
        assert!(outs[0]
            .iter()
            .zip(&outs[1])
            .all(|(a, b)| a.to_bits() == b.to_bits()));
    }
}
