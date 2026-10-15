//! Impetus: the transient processor (ADR 0056), SPL Transient Designer's idea done on the
//! family's detector (`dynfx_transient`): **level differences, not a threshold**.
//!
//! The detector reads, per sample, how much of the sound is an onset (*attack*, the fast
//! envelope over the slow) and how much is a tail (*sustain*, the long release over the
//! short), in dB. The gain is `attack × that + sustain × this` (each amount −100 … +100 %),
//! clamped to ±24 dB: at +100 % an onset's rise is doubled, at −100 % flattened — at any
//! level, so a quiet hit and a loud one are shaped alike, to the bit when they differ by a
//! power of two.
//!
//! **Split band**: with *highs* (or *lows*) only the band above (below) a crossover — the
//! Butterworth high- (low-) pass of a state-variable filter, 90° from the rest at the crossover
//! so nothing cancels — is detected and shaped: `y = x + (g − 1)·band`. **Sidechain**: with
//! a key routed, the key (its band) is what is detected.
//!
//! **Nulls**: at 0 % attack and 0 % sustain (the defaults) the gain is exactly 1 and every
//! sample passes untouched, whatever the band, the times or the key; no latency. The band
//! filters run only in a split mode and are emptied when it ends; after a run of silence
//! everything is flushed to +0.0 (per sample), so the unit comes to rest exactly.

use crate::dynfx_follow::Quiet;
use crate::dynfx_meter::{Columns, Fold, Hold};
use crate::dynfx_transient::{Diff, DiffTimes};
use crate::math;
use crate::dynfx_transient::{Svf, SvfCoef};
use crate::modfx_line::Glide;

/// Which part of the spectrum is shaped.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Band {
    #[default]
    Full,
    Highs,
    Lows,
}

impl Band {
    pub fn from_index(i: f32) -> Band {
        match i.round() as i32 {
            1 => Band::Highs,
            2 => Band::Lows,
            _ => Band::Full,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ImpetusSettings {
    /// −1 … 1: the onsets' gain, as a share of their level difference.
    pub attack: f32,
    /// −1 … 1: the tails' gain.
    pub sustain: f32,
    /// The slow attack: how long an onset lasts, ms.
    pub speed_ms: f32,
    /// The long release: how long a tail is heard as one, ms.
    pub length_ms: f32,
    pub band: Band,
    pub xover_hz: f32,
    pub out_db: f32,
}

impl Default for ImpetusSettings {
    fn default() -> Self {
        ImpetusSettings {
            attack: 0.0,
            sustain: 0.0,
            speed_ms: 15.0,
            length_ms: 250.0,
            band: Band::Full,
            xover_hz: 200.0,
            out_db: 0.0,
        }
    }
}

/// The most the gain moves either way, dB.
pub const MAX_GAIN_DB: f32 = 24.0;
/// Glides land within this.
const SNAP: f32 = 1e-6;
/// Filter states under this are flushed after a run of silence.
const SETTLE: f32 = 1e-20;

/// The gain law, dB: what the editor draws and the DSP plays.
#[inline]
pub fn gain_db(attack: f32, sustain: f32, attack_db: f32, sustain_db: f32) -> f32 {
    (attack * attack_db + sustain * sustain_db).clamp(-MAX_GAIN_DB, MAX_GAIN_DB)
}

/// The telemetry's layout; columns (`dynfx_meter`) of the input's peak (linear), the gain's
/// highest and lowest (dB) from index 16.
pub mod tel {
    pub const SAMPLE_RATE: usize = 0;
    pub const BLOCKS: usize = 1;
    /// Held about 50 ms, dB.
    pub const IN_PEAK: usize = 3;
    pub const OUT_PEAK: usize = 4;
    /// The deepest boost and the deepest cut, dB (both ≥ 0), held.
    pub const GAIN_MAX: usize = 5;
    pub const GAIN_MIN: usize = 6;
    /// The detector's readings, dB, held.
    pub const ATTACK: usize = 7;
    pub const SUSTAIN: usize = 8;
    /// 1 when the last block was detected from a key.
    pub const KEYED: usize = 9;
}

#[derive(Clone, Debug)]
pub struct Impetus {
    sr: f32,
    set: ImpetusSettings,
    times: DiffTimes,
    diff: Diff,
    attack: Glide,
    sustain: Glide,
    out: Glide,
    xover: SvfCoef,
    lp: [Svf; 2],
    key_lp: [Svf; 2],
    quiet: Quiet,
    // ---- the meter ----
    holds: [Hold; 6],
    cols: Columns,
    keyed: bool,
    blocks: u32,
}

impl Impetus {
    pub fn new(sample_rate: f32) -> Impetus {
        let set = ImpetusSettings::default();
        Impetus {
            sr: sample_rate,
            set,
            times: DiffTimes::new(set.speed_ms, set.length_ms, sample_rate),
            diff: Diff::default(),
            attack: Glide::new(0.0, 10.0, sample_rate, SNAP),
            sustain: Glide::new(0.0, 10.0, sample_rate, SNAP),
            out: Glide::new(1.0, 10.0, sample_rate, SNAP),
            xover: SvfCoef::new(set.xover_hz, sample_rate),
            lp: [Svf::default(); 2],
            key_lp: [Svf::default(); 2],
            quiet: Quiet::default(),
            holds: [Hold::default(); 6],
            cols: Columns::new([Fold::Max, Fold::Max, Fold::Min]),
            keyed: false,
            blocks: 0,
        }
    }

    pub fn settings(&self) -> &ImpetusSettings {
        &self.set
    }

    /// New settings: amounts and output glide; a band that ends empties its filters.
    pub fn set(&mut self, s: &ImpetusSettings) {
        if s.speed_ms != self.set.speed_ms || s.length_ms != self.set.length_ms {
            self.times = DiffTimes::new(s.speed_ms, s.length_ms, self.sr);
        }
        if s.xover_hz != self.set.xover_hz {
            self.xover = SvfCoef::new(s.xover_hz.clamp(20.0, 0.45 * self.sr), self.sr);
        }
        if s.band == Band::Full && self.set.band != Band::Full {
            self.lp = [Svf::default(); 2];
            self.key_lp = [Svf::default(); 2];
        }
        self.attack.target = s.attack.clamp(-1.0, 1.0);
        self.sustain.target = s.sustain.clamp(-1.0, 1.0);
        self.out.target = if s.out_db == 0.0 {
            1.0
        } else {
            math::db_to_gain(s.out_db)
        };
        self.set = *s;
    }

    pub fn reset(&mut self) {
        self.diff.reset();
        self.attack.jump(self.attack.target);
        self.sustain.jump(self.sustain.target);
        self.out.jump(self.out.target);
        self.lp = [Svf::default(); 2];
        self.key_lp = [Svf::default(); 2];
        self.quiet.reset();
        self.holds = [Hold::default(); 6];
        self.cols.reset();
        self.blocks = 0;
    }

    pub fn latency(&self) -> usize {
        0
    }

    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    pub fn rest_blocker(&self) -> Option<&'static str> {
        if !(self.attack.still() && self.sustain.still() && self.out.still()) {
            return Some("an amount is gliding");
        }
        if !self.diff.is_zero() {
            return Some("the detector remembers");
        }
        if !self.lp.iter().chain(&self.key_lp).all(Svf::is_zero) {
            return Some("the crossover rings");
        }
        None
    }

    pub fn skip(&mut self, frames: usize) {
        self.diff.skip(frames);
        self.quiet.skip(frames);
        self.cols.skip(frames, [0.0, 0.0, 0.0]);
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32], key: Option<(&[f32], &[f32])>) {
        let n = l.len().min(r.len());
        let band = self.set.band;
        let key = key.filter(|(kl, kr)| kl.len() >= n && kr.len() >= n);
        self.keyed = key.is_some();
        let (mut in_pk, mut out_pk, mut boost, mut cut, mut ad_pk, mut sd_pk) =
            (0f32, 0f32, 0f32, 0f32, 0f32, 0f32);
        for i in 0..n {
            let a = self.attack.next();
            let su = self.sustain.next();
            let out = self.out.next();
            let (x, y) = (l[i], r[i]);
            // The band of the input (and of the key): only in a split mode.
            let (hx, hy) = match band {
                Band::Full => (x, y),
                Band::Highs => (
                    self.lp[0].run(x, &self.xover).1,
                    self.lp[1].run(y, &self.xover).1,
                ),
                Band::Lows => (
                    self.lp[0].run(x, &self.xover).0,
                    self.lp[1].run(y, &self.xover).0,
                ),
            };
            let (kx, ky) = match key {
                Some((kl, kr)) => (kl[i], kr[i]),
                None => (0.0, 0.0),
            };
            let level = if key.is_some() {
                let (bx, by) = match band {
                    Band::Full => (kx, ky),
                    Band::Highs => (
                        self.key_lp[0].run(kx, &self.xover).1,
                        self.key_lp[1].run(ky, &self.xover).1,
                    ),
                    Band::Lows => (
                        self.key_lp[0].run(kx, &self.xover).0,
                        self.key_lp[1].run(ky, &self.xover).0,
                    ),
                };
                bx.abs().max(by.abs())
            } else {
                hx.abs().max(hy.abs())
            };
            let (ad, sd) = self.diff.run(level, &self.times);
            // After a run of silence (input and key), what is left of the filters' ring is
            // flushed — per sample, so where it happens does not depend on the blocks.
            let all = x.abs().max(y.abs()).max(kx.abs()).max(ky.abs());
            if self.quiet.note(all) && band != Band::Full {
                for f in self.lp.iter_mut().chain(self.key_lp.iter_mut()) {
                    f.settle(SETTLE);
                }
            }
            let gdb = gain_db(a, su, ad, sd);
            let (mut ox, mut oy) = (x, y);
            if gdb != 0.0 {
                let g = math::db_to_gain(gdb);
                if band == Band::Full {
                    ox *= g;
                    oy *= g;
                } else {
                    ox += (g - 1.0) * hx;
                    oy += (g - 1.0) * hy;
                }
            }
            if out != 1.0 {
                ox *= out;
                oy *= out;
            }
            l[i] = ox;
            r[i] = oy;
            // The meter.
            let pk = x.abs().max(y.abs());
            in_pk = in_pk.max(pk);
            out_pk = out_pk.max(ox.abs().max(oy.abs()));
            boost = boost.max(gdb);
            cut = cut.max(-gdb);
            ad_pk = ad_pk.max(ad);
            sd_pk = sd_pk.max(sd);
            self.cols.push([pk, gdb, gdb]);
        }
        self.blocks = (self.blocks + 1) % crate::dynfx_meter::WRAP;
        let window = (0.05 * self.sr) as u32;
        for (h, v) in self
            .holds
            .iter_mut()
            .zip([in_pk, out_pk, boost, cut, ad_pk, sd_pk])
        {
            h.push(v, n, window);
        }
    }

    /// The editor's numbers ([`tel`]).
    pub fn telemetry(&self, out: &mut [f32; 64]) {
        use crate::dynfx_follow::to_db;
        out[tel::SAMPLE_RATE] = self.sr;
        out[tel::BLOCKS] = self.blocks as f32;
        out[tel::IN_PEAK] = to_db(self.holds[0].get());
        out[tel::OUT_PEAK] = to_db(self.holds[1].get());
        out[tel::GAIN_MAX] = self.holds[2].get();
        out[tel::GAIN_MIN] = self.holds[3].get();
        out[tel::ATTACK] = self.holds[4].get();
        out[tel::SUSTAIN] = self.holds[5].get();
        out[tel::KEYED] = if self.keyed { 1.0 } else { 0.0 };
        self.cols.write(out);
    }
}

#[cfg(test)]
#[path = "impetus_tests.rs"]
mod tests;
