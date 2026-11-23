//! Nitor's low end: what FL's Low Lifter, Aphex's Big Bottom and Waves's MaxxBass do, and
//! the song's key on top.
//!
//! The **low band** is the input's mid (`(l + r) / 2`) under the crossover: a
//! Linkwitz–Riley low-pass (a Butterworth pair twice). From it:
//!
//! - **sub**: in *octave* mode, Saltus's divider ([`crate::saltus::octave::Divider`]) — an
//!   octave under the bass that plays, a sine phase-locked to its edges, no latency, at the
//!   fundamental's peak; in *tonic* mode, a sine on the song's tonic in octave 1 (32.7–61.7
//!   Hz), its phase from zero each time the low band starts from silence, opened by the low
//!   band's envelope (5 ms up, 150 ms down) — an 808 under every hit, in the song's key.
//! - **harmonics**: the low band normalised by its envelope, driven into Nitor's curves at
//!   2× (half even, half odd), brought down and kept between 1.5 and 6 times the crossover —
//!   the bass's second to fifth harmonics, which a small speaker plays and the ear hears as
//!   the fundamental it cannot (the missing fundamental).
//! - **tight**: each hit in the low band (a fast envelope 3 dB over a slow one) opens the
//!   band fully for 30 ms; then its gain falls (120 ms) toward a floor the tightness sets
//!   (−12 dB at 1) until the next hit — the boom shortened, the hit kept, a kick and a bass
//!   that do not smear into each other. Silence brings the gain back to 1. Applied to each
//!   channel by a Butterworth low shelf at the crossover whose gain is the tightening's,
//!   designed each sample (Simper's shelf: `x + k(A − 1)·bp + (A² − 1)·lp` at `g/√A`, so
//!   the lows get `A² = g` exactly and nothing above the band moves).
//! - **lift**: two bells on the tonic's fundamental in octave 1 (up to +6 dB, Q 2.5) and its
//!   octave (half the dB), `y + (A − 1)·k·bp(y)` — the song's root note blooms.
//!
//! The sub and the harmonics are mono, added to both channels: the low end stays centred.

use crate::fervor::os::Oversampler;
use crate::math;
use crate::multiplex::filters::{warp, Svf, SvfCoefs};
use crate::saltus::octave::Divider;

use super::SubMode;

/// The grid the crossover glides on, samples.
pub const UPDATE: usize = 16;
/// The octave sub's source sits this far over the crossover.
const SRC_OVER: f32 = 2.5;
/// The sub at level 1, against the low band's fundamental (twice: +6 dB).
const SUB_GAIN: f32 = 2.0;
/// The lift's most, dB, at the fundamental (its octave: half).
pub const LIFT_DB: f32 = 6.0;
/// The lift's bells' Q.
pub const LIFT_Q: f32 = 2.5;
/// The harmonics kept, as multiples of the crossover.
const HARM_LO: f32 = 1.5;
const HARM_HI: f32 = 6.0;
const HARM_DRIVE: f32 = 1.8;
const HARM_GAIN: f32 = 0.9;
/// The tightening's floor at tightness 1 (−12 dB).
const TIGHT_FLOOR: f32 = 0.25;
/// A hit: the fast envelope this far over the slow one; ready again under `REARM`.
const ONSET: f32 = 1.3;
const REARM: f32 = 1.1;
/// After a hit: open this long, then falling with this time constant, seconds; the way back
/// up to open on a hit, seconds.
const HOLD_S: f32 = 0.03;
/// The fast envelope holds its peak this long (longer than a 40 Hz period, so a note's own
/// cycles are not hits), seconds; and no hit comes sooner than this after another.
const PEAK_HOLD_S: f32 = 0.026;
const REFRACTORY_S: f32 = 0.06;
const DECAY_S: f32 = 0.12;
const RISE_S: f32 = 0.0005;
const FLUSH: f32 = 1e-20;

fn follower(sr: f32, s: f32) -> f32 {
    1.0 - math::exp(-1.0 / (s * sr))
}

#[inline]
fn follow(env: &mut f32, a: f32, att: f32, rel: f32) {
    *env += (a - *env) * if a > *env { att } else { rel };
    if *env < FLUSH {
        *env = 0.0;
    }
}

#[derive(Clone, Debug)]
pub struct Low {
    sr: f32,
    /// r42:tuning — the strip's tuning (ADR 0155): the tonic's pitch in it.
    pub tuning: [f32; 128],
    xover: f32,
    xover_to: f32,
    phase: usize,
    band_c: SvfCoefs,
    band: [Svf; 2],
    // Sub.
    /// The mid of the last sample, and the divider's source: the mid under 2.5 times the
    /// crossover (so a bass note just over the crossover is still followed at its level).
    mid: f32,
    src_c: SvfCoefs,
    src: Svf,
    divider: Divider,
    tonic_hz: f32,
    osc: f32,
    gate: f32,
    gate_att: f32,
    gate_rel: f32,
    // Harmonics.
    harm_env: f32,
    harm_att: f32,
    harm_rel: f32,
    harm_os: Oversampler,
    harm_hp_c: SvfCoefs,
    harm_lp_c: SvfCoefs,
    harm_hp: [Svf; 2],
    harm_lp: Svf,
    // Tight.
    fast: f32,
    slow: f32,
    fast_att: f32,
    fast_rel: f32,
    slow_att: f32,
    slow_rel: f32,
    /// The gain's fall (the part of the distance to the floor kept each sample), its rise.
    t_decay: f32,
    t_rise: f32,
    t_hold_len: u32,
    t_gain: f32,
    t_hold: u32,
    t_armed: bool,
    t_peak_len: u32,
    t_peak_hold: u32,
    t_refractory_len: u32,
    t_refractory: u32,
    /// The hits are heard on the mid through a one-pole at three times the crossover — the
    /// low band's own filter would hear them a few milliseconds late.
    t_det_g: f32,
    t_det: f32,
    /// The shelf's warped crossover, and this sample's design: its coefficients and mixes.
    tight_g: f32,
    tight_c: SvfCoefs,
    tight_m: (f32, f32),
    tight: [Svf; 2],
    // Lift.
    lift_c: [SvfCoefs; 2],
    lift: [[Svf; 2]; 2],
}

impl Low {
    pub fn new(sample_rate: f32) -> Low {
        let sr = sample_rate.max(8_000.0);
        let mut l = Low {
            sr,
            xover: 100.0,
            xover_to: 100.0,
            phase: 0,
            band_c: SvfCoefs::default(),
            band: [Svf::default(); 2],
            mid: 0.0,
            src_c: SvfCoefs::default(),
            src: Svf::default(),
            divider: Divider::new(sr),
            tonic_hz: super::tonic_hz(0),
            tuning: [0.0; 128], // r42:tuning
            osc: 0.0,
            gate: 0.0,
            gate_att: follower(sr, 0.005),
            gate_rel: follower(sr, 0.15),
            harm_env: 0.0,
            harm_att: follower(sr, 0.002),
            harm_rel: follower(sr, 0.08),
            harm_os: Oversampler::new(2, true),
            harm_hp_c: SvfCoefs::default(),
            harm_lp_c: SvfCoefs::default(),
            harm_hp: [Svf::default(); 2],
            harm_lp: Svf::default(),
            fast: 0.0,
            slow: 0.0,
            fast_att: follower(sr, 0.0003),
            fast_rel: follower(sr, 0.04),
            slow_att: follower(sr, 0.04),
            slow_rel: follower(sr, 0.15),
            t_decay: math::exp(-1.0 / (DECAY_S * sr)),
            t_rise: 1.0 / (RISE_S * sr),
            t_hold_len: (HOLD_S * sr) as u32,
            t_gain: 1.0,
            t_hold: 0,
            t_armed: true,
            t_peak_len: (PEAK_HOLD_S * sr) as u32,
            t_peak_hold: 0,
            t_refractory_len: (REFRACTORY_S * sr) as u32,
            t_refractory: 0,
            t_det_g: 0.1,
            t_det: 0.0,
            tight_g: 0.1,
            tight_c: SvfCoefs::default(),
            tight_m: (0.0, 0.0),
            tight: [Svf::default(); 2],
            lift_c: [SvfCoefs::default(); 2],
            lift: [[Svf::default(); 2]; 2],
        };
        l.design_xover();
        l.design_lift();
        l
    }

    fn design_xover(&mut self) {
        let g = warp(self.xover, self.sr);
        self.band_c = SvfCoefs::new(g, core::f32::consts::SQRT_2);
        self.tight_g = g;
        let gd = warp(self.xover * 3.0, self.sr);
        self.t_det_g = gd / (1.0 + gd);
        self.src_c = SvfCoefs::new(
            warp(self.xover * SRC_OVER, self.sr),
            core::f32::consts::SQRT_2,
        );
        self.harm_hp_c = SvfCoefs::new(
            warp(self.xover * HARM_LO, self.sr),
            core::f32::consts::SQRT_2,
        );
        self.harm_lp_c = SvfCoefs::new(
            warp(self.xover * HARM_HI, self.sr),
            core::f32::consts::SQRT_2,
        );
    }

    fn design_lift(&mut self) {
        let k = 1.0 / LIFT_Q;
        self.lift_c = [
            SvfCoefs::new(warp(self.tonic_hz, self.sr), k),
            SvfCoefs::new(warp(2.0 * self.tonic_hz, self.sr), k),
        ];
    }

    /// The crossover: taken at once by a first call (`snap`), glided otherwise.
    pub fn set_xover(&mut self, hz: f32, snap: bool) {
        self.xover_to = hz.clamp(super::XOVER_MIN, super::XOVER_MAX);
        if snap && self.xover != self.xover_to {
            self.xover = self.xover_to;
            self.design_xover();
        }
    }

    pub fn xover(&self) -> f32 {
        self.xover
    }

    /// The tonic (0 = C … 11 = B): the tonic sub and the lift move to it at once.
    pub fn set_tonic(&mut self, tonic: u8) {
        let hz = super::tonic_hz_tuned(tonic, &self.tuning); // r42:tuning
        if hz != self.tonic_hz {
            self.tonic_hz = hz;
            self.design_lift();
        }
    }

    pub fn tonic_hz(&self) -> f32 {
        self.tonic_hz
    }

    /// Whether the crossover is still gliding.
    pub fn gliding(&self) -> bool {
        self.xover != self.xover_to
    }

    /// One step of the grid (the crossover's glide). Call once per sample.
    #[inline]
    pub fn clock(&mut self) {
        self.phase += 1;
        if self.phase == UPDATE {
            self.phase = 0;
            if self.xover != self.xover_to {
                let r = self.xover_to / self.xover;
                // A tenth of the way in the log of the frequency (about 7 ms a step at 48
                // kHz), landing within a hundredth of a per cent.
                let next = self.xover * math::powf(r, 0.1);
                self.xover = if (next / self.xover_to - 1.0).abs() < 1e-4 {
                    self.xover_to
                } else {
                    next
                };
                self.design_xover();
            }
        }
    }

    pub fn skip(&mut self, frames: usize) {
        self.phase = (self.phase + frames % UPDATE) % UPDATE;
    }

    // ---- the low band ---------------------------------------------------------------

    #[inline]
    pub fn band(&mut self, l: f32, r: f32) -> f32 {
        let m = (l + r) * 0.5;
        self.mid = m;
        let a = self.band[0].tick(&self.band_c, m).0;
        self.band[1].tick(&self.band_c, a).0
    }

    pub fn clear_band(&mut self) {
        self.band = [Svf::default(); 2];
        self.mid = 0.0;
    }

    pub fn band_clear(&self) -> bool {
        self.band.iter().all(Svf::is_clear) && crate::rest::z(self.mid)
    }

    // ---- the sub ----------------------------------------------------------------------

    /// The sub for one sample of the low band (before its level).
    #[inline]
    pub fn sub(&mut self, lb: f32, mode: SubMode) -> f32 {
        match mode {
            SubMode::Octave => {
                let src = self.src.tick(&self.src_c, self.mid).0;
                SUB_GAIN * self.divider.tick(src).sub1
            }
            SubMode::Tonic => {
                if self.gate == 0.0 {
                    self.osc = 0.0;
                }
                follow(&mut self.gate, lb.abs(), self.gate_att, self.gate_rel);
                if self.gate == 0.0 {
                    return 0.0;
                }
                let y = SUB_GAIN * math::sin(core::f32::consts::TAU * self.osc) * self.gate;
                self.osc += self.tonic_hz / self.sr;
                if self.osc >= 1.0 {
                    self.osc -= 1.0;
                }
                y
            }
        }
    }

    /// The sub's frequency now, Hz, while it sounds (0: none).
    pub fn sub_hz(&self, mode: SubMode) -> f32 {
        match mode {
            SubMode::Octave => self
                .divider
                .detected(440.0)
                .map(|m| 440.0 * math::exp2((m - 69.0 - 12.0) / 12.0))
                .unwrap_or(0.0),
            SubMode::Tonic => {
                if self.gate > 0.0 {
                    self.tonic_hz
                } else {
                    0.0
                }
            }
        }
    }

    /// In silence, the divider back to a new one's state: with no tracker to guide it, its
    /// conditioning filter would keep the last note's cutoff (Saltus's tracker brings it
    /// back). Only once its source is exactly silent and its levels are zero — what it
    /// would play from here is silence either way.
    pub fn settle_sub(&mut self) {
        if self.src.is_clear() && self.divider.levels() == [0.0; 3] && !self.divider.at_rest() {
            self.divider.reset();
        }
    }

    pub fn clear_sub(&mut self) {
        self.src = Svf::default();
        self.divider.reset();
        self.osc = 0.0;
        self.gate = 0.0;
    }

    pub fn sub_clear(&self) -> bool {
        self.src.is_clear()
            && self.divider.at_rest()
            && crate::rest::z(self.osc)
            && crate::rest::z(self.gate)
    }

    // ---- the harmonics ----------------------------------------------------------------

    #[inline]
    pub fn harm(&mut self, lb: f32) -> f32 {
        follow(&mut self.harm_env, lb.abs(), self.harm_att, self.harm_rel);
        let v = if self.harm_env > 0.0 {
            HARM_DRIVE * lb / self.harm_env.max(1e-9)
        } else {
            0.0
        };
        let mut up = [0.0f32; 8];
        self.harm_os.up(v, &mut up);
        for u in up.iter_mut().take(2) {
            *u = super::excite::harmonics(*u, 0.5);
        }
        let h = self.harm_os.down(&mut up);
        let (_, _, a) = self.harm_hp[0].tick(&self.harm_hp_c, h);
        let (_, _, b) = self.harm_hp[1].tick(&self.harm_hp_c, a);
        let (c, _, _) = self.harm_lp.tick(&self.harm_lp_c, b);
        c * self.harm_env * (HARM_GAIN / HARM_DRIVE)
    }

    pub fn clear_harm(&mut self) {
        self.harm_env = 0.0;
        self.harm_os.reset();
        self.harm_hp = [Svf::default(); 2];
        self.harm_lp = Svf::default();
    }

    pub fn harm_clear(&self) -> bool {
        crate::rest::z(self.harm_env)
            && self.harm_os.is_clear()
            && self.harm_hp.iter().all(Svf::is_clear)
            && self.harm_lp.is_clear()
    }

    // ---- tight ------------------------------------------------------------------------

    /// The low band's gain for one sample at tightness `t` (0 … 1).
    #[inline]
    pub fn tight_gain(&mut self, lb: f32, t: f32) -> f32 {
        let _ = lb;
        // A trapezoidal one-pole low-pass of the mid (`G = g / (1 + g)`).
        let v = (self.mid - self.t_det) * self.t_det_g;
        let d = v + self.t_det;
        self.t_det = d + v;
        if self.t_det.abs() < FLUSH {
            self.t_det = 0.0;
        }
        let a = d.abs();
        // The fast envelope: the peak, held through a period, then released.
        if a >= self.fast {
            self.fast += (a - self.fast) * self.fast_att;
            self.t_peak_hold = self.t_peak_len;
        } else if self.t_peak_hold > 0 {
            self.t_peak_hold -= 1;
        } else {
            self.fast -= self.fast * self.fast_rel;
        }
        if self.fast < FLUSH {
            self.fast = 0.0;
        }
        // The slow one follows the fast one, slowly.
        let f = self.fast;
        follow(&mut self.slow, f, self.slow_att, self.slow_rel);
        if self.slow == 0.0 && self.fast == 0.0 {
            // Silence: open, ready, as new.
            self.t_gain = 1.0;
            self.t_hold = 0;
            self.t_armed = true;
            self.t_peak_hold = 0;
            self.t_refractory = 0;
            self.design_shelf();
            return 1.0;
        }
        if self.t_refractory > 0 {
            self.t_refractory -= 1;
        }
        if self.t_armed && self.t_refractory == 0 && self.fast > ONSET * self.slow {
            self.t_armed = false;
            self.t_hold = self.t_hold_len;
            self.t_refractory = self.t_refractory_len;
        } else if !self.t_armed && self.fast < REARM * self.slow {
            self.t_armed = true;
        }
        if self.t_hold > 0 {
            self.t_hold -= 1;
            self.t_gain = (self.t_gain + self.t_rise).min(1.0);
        } else {
            let floor = 1.0 - t * (1.0 - TIGHT_FLOOR);
            self.t_gain = floor + (self.t_gain - floor) * self.t_decay;
        }
        self.design_shelf();
        self.t_gain
    }

    /// The low shelf for this sample's gain.
    #[inline]
    fn design_shelf(&mut self) {
        let a = self.t_gain.sqrt();
        let k = core::f32::consts::SQRT_2;
        self.tight_c = SvfCoefs::new(self.tight_g / a.sqrt(), k);
        self.tight_m = (k * (a - 1.0), a * a - 1.0);
    }

    /// Channel `ch`'s sample through the shelf [`Self::tight_gain`] designed.
    #[inline]
    pub fn tighten(&mut self, ch: usize, y: f32) -> f32 {
        let (lp, bp, _) = self.tight[ch].tick(&self.tight_c, y);
        y + self.tight_m.0 * bp + self.tight_m.1 * lp
    }

    pub fn clear_tight(&mut self) {
        self.fast = 0.0;
        self.slow = 0.0;
        self.t_gain = 1.0;
        self.t_hold = 0;
        self.t_armed = true;
        self.t_peak_hold = 0;
        self.t_refractory = 0;
        self.t_det = 0.0;
        self.tight = [Svf::default(); 2];
    }

    pub fn tight_clear(&self) -> bool {
        crate::rest::z(self.fast)
            && crate::rest::z(self.slow)
            && self.t_gain == 1.0
            && self.t_hold == 0
            && self.t_armed
            && self.t_peak_hold == 0
            && self.t_refractory == 0
            && crate::rest::z(self.t_det)
            && self.tight.iter().all(Svf::is_clear)
    }

    // ---- lift -------------------------------------------------------------------------

    /// Channel `ch`'s sample through the two bells at lift `v` (0 … 1).
    #[inline]
    pub fn lift(&mut self, ch: usize, y: f32, a1: f32, a2: f32) -> f32 {
        let (_, bp, _) = self.lift[ch][0].tick(&self.lift_c[0], y);
        let y1 = y + (a1 - 1.0) * self.lift_c[0].k * bp;
        let (_, bp, _) = self.lift[ch][1].tick(&self.lift_c[1], y1);
        y1 + (a2 - 1.0) * self.lift_c[1].k * bp
    }

    pub fn clear_lift(&mut self) {
        self.lift = [[Svf::default(); 2]; 2];
    }

    pub fn lift_clear(&self) -> bool {
        self.lift.iter().flatten().all(Svf::is_clear)
    }

    pub fn reset(&mut self) {
        self.clear_band();
        self.clear_sub();
        self.clear_harm();
        self.clear_tight();
        self.clear_lift();
        self.xover = self.xover_to;
        self.design_xover();
        self.phase = 0;
    }
}

/// The lift's two gains (fundamental, octave) at lift `v`, linear.
pub fn lift_gains(v: f32) -> (f32, f32) {
    (
        math::db_to_gain(LIFT_DB * v),
        math::db_to_gain(0.5 * LIFT_DB * v),
    )
}
