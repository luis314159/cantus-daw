//! Nitor's loudness: the input's and the wet path's K-weighted loudness (BS.1770's filters,
//! Comprimo's [`KWeighting`]), each a running mean that becomes an exponential one of
//! [`TAU_S`] (so it is right from the first notes, and follows a song after them), and from
//! them two gains glided on a grid of [`UPDATE`] samples counted from the start of the
//! stream:
//!
//! - **pre**, which brings the input to [`super::REFERENCE_LUFS`] before the character's
//!   dynamics (and is taken back after them), so a character acts alike at any level;
//! - **match**, the input's loudness over the wet path's: the output reads as loud as the
//!   input.
//!
//! Under the gate ([`GATE_LUFS`]) neither moves. After [`FORGET_S`] of silence both glide
//! back to unity and the meters are cleared — a new unit's state, so the unit can rest
//! (ADR 0031 §5.1) and a song that starts again is measured afresh.

use crate::comprimo::loudness::{power, KWeighting};

/// The grid the gains are updated on, samples.
pub const UPDATE: usize = 16;
/// The loudness's time constant once it has heard this much, seconds.
pub const TAU_S: f64 = 1.5;
/// How fast the gains follow their targets, seconds.
pub const GLIDE_S: f64 = 0.15;
/// Under this, neither gain moves, LUFS.
pub const GATE_LUFS: f32 = -70.0;
/// The gate's own loudness's time constant (shorter, so a pause is heard as one), seconds.
pub const GATE_TAU_S: f64 = 0.4;
/// Exact digital silence this long forgets what was measured, seconds.
pub const FORGET_S: f64 = 8.0;
/// How long the gains take to come back to unity once forgotten (or switched off), seconds.
pub const RETURN_S: f64 = 0.1;
/// The match's and the pre-gain's ranges, dB.
pub const MATCH_DB: f32 = 12.0;
pub const PRE_DB: f32 = 24.0;
/// The gate's loudness's floor (−110 LUFS: far under the gate).
const GATE_FLUSH: f64 = 1e-11;
/// The meters' floor: under it a mean is +0.0.
const FLUSH: f64 = 1e-30;

fn db_to_lin(db: f32) -> f32 {
    crate::math::db_to_gain(db)
}

/// A gain on the grid: glided toward its target, or brought back to unity linearly.
#[derive(Copy, Clone, Debug, PartialEq)]
struct Gain {
    g: f32,
    /// Returning to unity: the step per update, and updates left.
    ret_step: f32,
    ret_left: u32,
}

impl Gain {
    const UNITY: Gain = Gain {
        g: 1.0,
        ret_step: 0.0,
        ret_left: 0,
    };

    fn follow(&mut self, target: f32, k: f32) {
        self.ret_left = 0;
        self.g += (target - self.g) * k;
    }

    /// Back to exactly 1 over `updates` updates.
    fn back(&mut self, updates: u32) {
        if self.g == 1.0 {
            *self = Gain::UNITY;
            return;
        }
        if self.ret_left == 0 {
            self.ret_left = updates.max(1);
            self.ret_step = (1.0 - self.g) / self.ret_left as f32;
        }
        self.ret_left -= 1;
        if self.ret_left == 0 {
            *self = Gain::UNITY;
        } else {
            self.g += self.ret_step;
        }
    }
}


/// What the input side knew at its last grid point: all the wet side reads of it.
#[derive(Copy, Clone, Debug, PartialEq)]
struct AtGrid {
    heard_now: bool,
    forgotten: bool,
    e_in: f64,
    just_forgot: bool,
}

impl AtGrid {
    const NEW: AtGrid = AtGrid {
        heard_now: false,
        forgotten: true,
        e_in: 0.0,
        just_forgot: false,
    };
}

/// The two loudness meters and the two gains.
///
/// **Order**: a caller feeds a chunk that ends on the grid (at most [`UPDATE`] samples)
/// to [`Self::input`] sample by sample, then the same chunk's wet path to [`Self::wet`].
/// The wet side reads the input side only at the grid point (the chunk's last sample) and
/// through the coefficient `input` returned per sample, so the result is what feeding the
/// two sample by sample, interleaved, would give — whatever the block size.
#[derive(Clone, Debug)]
pub struct Level {
    k_in: KWeighting,
    k_wet: KWeighting,
    e_in: f64,
    e_wet: f64,
    /// Samples heard (the running mean's count).
    heard: u64,
    /// Samples of exact silence in a row (+0.0 in both channels).
    silent: u64,
    /// The gate's loudness, and its coefficient.
    e_gate: f64,
    c_gate: f64,
    phase_in: usize,
    phase_wet: usize,
    at_grid: AtGrid,
    c_tau: f64,
    k_glide: f32,
    forget: u64,
    ret_updates: u32,
    gate: f64,
    reference: f64,
    pre: Gain,
    gm: Gain,
}

impl Level {
    pub fn new(sample_rate: f32) -> Level {
        let sr = f64::from(sample_rate.max(1.0));
        let updates_per_s = sr / UPDATE as f64;
        Level {
            k_in: KWeighting::new(sample_rate),
            k_wet: KWeighting::new(sample_rate),
            e_in: 0.0,
            e_wet: 0.0,
            heard: 0,
            silent: 0,
            e_gate: 0.0,
            c_gate: 1.0 - crate::math::exp(-1.0 / (GATE_TAU_S * sr)),
            phase_in: 0,
            phase_wet: 0,
            at_grid: AtGrid::NEW,
            c_tau: 1.0 / (TAU_S * sr),
            k_glide: (1.0 / (GLIDE_S * updates_per_s)).min(1.0) as f32,
            forget: (FORGET_S * sr) as u64,
            ret_updates: (RETURN_S * updates_per_s).round().max(1.0) as u32,
            gate: power(GATE_LUFS),
            reference: power(super::REFERENCE_LUFS),
            pre: Gain::UNITY,
            gm: Gain::UNITY,
        }
    }

    /// As new, the grid back at the stream's start.
    pub fn reset(&mut self) {
        self.clear();
        self.phase_in = 0;
        self.phase_wet = 0;
    }

    /// Everything measured forgotten, the filters emptied, the gains at unity; the grid
    /// keeps its place.
    pub fn clear(&mut self) {
        self.k_in.reset();
        self.k_wet.reset();
        self.e_in = 0.0;
        self.e_wet = 0.0;
        self.heard = 0;
        self.silent = 0;
        self.e_gate = 0.0;
        self.at_grid = AtGrid::NEW;
        self.pre = Gain::UNITY;
        self.gm = Gain::UNITY;
    }

    /// The pre-gain and the match now.
    #[inline]
    pub fn gains(&self) -> (f32, f32) {
        (self.pre.g, self.gm.g)
    }

    /// Everything as [`Self::clear`] leaves it.
    pub fn is_clear(&self) -> bool {
        self.k_in.is_clear()
            && self.k_wet.is_clear()
            && crate::rest::z64(self.e_in)
            && crate::rest::z64(self.e_wet)
            && self.heard == 0
            && self.silent == 0
            && crate::rest::z64(self.e_gate)
            && self.at_grid == AtGrid::NEW
            && self.pre == Gain::UNITY
            && self.gm == Gain::UNITY
    }

    /// Both gains at unity with nothing to bring back.
    pub fn at_unity(&self) -> bool {
        self.pre == Gain::UNITY && self.gm == Gain::UNITY
    }

    /// The input's sample. Returns the coefficient its mean moved by (0 before anything
    /// was heard), which [`Self::wet`] takes for the same sample. `pre_on`: the pre-gain
    /// follows (else it returns to unity).
    #[inline]
    pub fn input(&mut self, l: f32, r: f32, pre_on: bool) -> f64 {
        let e = self.k_in.tick(l, r);
        if self.heard > 0 {
            self.silent = if l.to_bits() == 0 && r.to_bits() == 0 {
                self.silent.saturating_add(1)
            } else {
                0
            };
        }
        self.e_gate += (e - self.e_gate) * self.c_gate;
        if self.e_gate < GATE_FLUSH {
            self.e_gate = 0.0;
        }
        let mut c = 0.0;
        if self.heard > 0 || e > FLUSH {
            self.heard = self.heard.saturating_add(1);
            c = (1.0 / self.heard as f64).max(self.c_tau);
            self.e_in += (e - self.e_in) * c;
            if self.e_in < FLUSH {
                self.e_in = 0.0;
            }
        }
        self.phase_in += 1;
        if self.phase_in == UPDATE {
            self.phase_in = 0;
            self.grid_in(pre_on);
        }
        c
    }

    fn grid_in(&mut self, pre_on: bool) {
        let heard_now = self.e_gate > self.gate;
        let mut just_forgot = false;
        if self.heard > 0 && self.silent >= self.forget {
            self.e_in = 0.0;
            self.heard = 0;
            self.silent = 0;
            just_forgot = true;
        }
        let forgotten = self.heard == 0;
        if pre_on && !forgotten {
            if heard_now {
                let t = ((self.reference / self.e_in).sqrt() as f32)
                    .clamp(db_to_lin(-PRE_DB), db_to_lin(PRE_DB));
                self.pre.follow(t, self.k_glide);
            }
        } else {
            self.pre.back(self.ret_updates);
        }
        self.at_grid = AtGrid {
            heard_now,
            forgotten,
            e_in: self.e_in,
            just_forgot,
        };
    }

    /// The wet path's sample, with the coefficient [`Self::input`] returned for it.
    /// `matching`: the match follows (else it returns to unity).
    #[inline]
    pub fn wet(&mut self, l: f32, r: f32, c: f64, matching: bool) {
        let e = self.k_wet.tick(l, r);
        if c > 0.0 {
            self.e_wet += (e - self.e_wet) * c;
            if self.e_wet < FLUSH {
                self.e_wet = 0.0;
            }
        }
        self.phase_wet += 1;
        if self.phase_wet == UPDATE {
            self.phase_wet = 0;
            self.grid_wet(matching);
        }
    }

    fn grid_wet(&mut self, matching: bool) {
        let g = self.at_grid;
        if g.just_forgot {
            self.e_wet = 0.0;
            self.at_grid.just_forgot = false;
        }
        if matching && !g.forgotten {
            if g.heard_now && self.e_wet > 0.0 {
                let t = ((g.e_in / self.e_wet).sqrt() as f32)
                    .clamp(db_to_lin(-MATCH_DB), db_to_lin(MATCH_DB));
                self.gm.follow(t, self.k_glide);
            }
        } else {
            self.gm.back(self.ret_updates);
        }
    }

    /// Samples until the next grid point (where a chunk must end).
    #[inline]
    pub fn to_grid(&self) -> usize {
        UPDATE - self.phase_in
    }

    /// Samples skipped at rest: only the grid moves.
    pub fn skip(&mut self, frames: usize) {
        self.phase_in = (self.phase_in + frames % UPDATE) % UPDATE;
        self.phase_wet = self.phase_in;
    }
}
