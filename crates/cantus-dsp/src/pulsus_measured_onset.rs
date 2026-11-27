//! Model 6, the drums' **onset** (round 42, `tympanum-real`; ADR 0124's seventh amendment).
//!
//! Model 5 starts every mode of a drum at its full amplitude at the strike, adds an early
//! part fitted to the recording's first 80 ms on top, a noise knock beside it, and its bands
//! jump to their first breakpoint at once — so every band of its snare peaked 0.5–5 ms after
//! the strike where the recorded kits' peak 5–11 ms (the lows last), and its grooves stood
//! 2–10 dB over the recordings' crest. Model 6 keeps the tables and changes how a hit enters
//! them:
//!
//! - **the contact**: every mode is driven through the stick's (a beater's) contact force, a
//!   half-sine `contact_ff_ms` … `contact_pp_ms` long — about 0.8 ms at full force, 1.8 ms
//!   soft, longer for a felt beater. A mode driven so grows as the force's integral, `F(t) =
//!   (1 − cos(π t / τ)) / 2`, and rings on as struck once the stick has left: the engine
//!   gives each mode a third phasor, the part not yet pushed in, shrunk by `(1 − F(n + 1)) /
//!   (1 − F(n))` a sample until it is exactly nothing;
//! - **the hit's level is the force's**: each mode's own ± level draw moves its colour, not
//!   its loudness — the hit's modal power is brought back to the blend's;
//! - **one draw a mode**: the ring and its measured early part (the head's first push, blob
//!   version 2) take the same level draw and the main layer's phases, so they stand as the
//!   recording had them (model 5 drew them apart: for some seeds the early part cancelled the
//!   ring — the studio kick at velocity 96 3.3 dB under its 80);
//! - **the bands come in through the head**: each band's noise rises over its own time
//!   (lows slower) after a lag — a snare's wires answer only once the air in the shell has
//!   pushed the bottom head ([`Onset6::wire_lag_ms`]);
//! - **the knock is the head's push**: the stick's knock is the contact's own half-sine (no
//!   noise), at [`Onset6::knock`] of model 5's.
//!
//! The cymbals are not drums: model 6 plays them as model 5 does, to the bit — but for a
//! hat's **split pairs** (two partials within 1 %, `data::locks`), whose halves one stroke
//! sets ringing together: model 6 draws their phases together, so they no longer beat
//! differently from one stroke to the next (the jazz closed hat's 6.1-dB take-to-take
//! change).

/// Model 6's onset constants (fitted against the recorded kits' per-band peak timing:
/// `tools/tympanum-real/onset.py`).
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct Onset6 {
    /// The stick's contact at full force and at the softest stroke, ms.
    pub contact_ff_ms: f32,
    pub contact_pp_ms: f32,
    /// A kick's felt beater: its contact times this.
    pub mallet: f32,
    /// 0: the main layer's measured phases at the strike; 1: drawn.
    pub drawn_phase: f32,
    /// The knock (the head's push) against model 5's.
    pub knock: f32,
    /// The stick's burst against model 5's.
    pub burst: f32,
    /// Each mode's own level draw, dB (model 5: 1.5).
    pub vary_db: f32,
    /// A snare's wires: their lag through the shell's air, ms.
    pub wire_lag_ms: f32,
    /// A band's rise at 100 Hz and at 8 kHz (log-frequency between), ms.
    pub rise_lo_ms: f32,
    pub rise_hi_ms: f32,
    /// The measured early part against model 5's.
    pub early: f32,
}

/// The constants model 6 plays.
pub(crate) const ONSET6: Onset6 = Onset6 {
    contact_ff_ms: 0.8,
    contact_pp_ms: 1.8,
    mallet: 2.0,
    drawn_phase: 0.0,
    knock: 0.0,
    burst: 1.0,
    vary_db: 1.5,
    wire_lag_ms: 1.5,
    rise_lo_ms: 6.0,
    rise_hi_ms: 2.0,
    early: 1.0,
};

/// The constants, read once (with the kit, off the audio thread: `Bank::new` asks first).
/// A build with the `tune` feature reads `CANTUS_M6` (`key=value,…`) over them — the fitting
/// cycles try values without rebuilding; never in a distributed build.
pub(crate) fn params() -> &'static Onset6 {
    static P: std::sync::OnceLock<Onset6> = std::sync::OnceLock::new();
    P.get_or_init(|| {
        #[allow(unused_mut)]
        let mut p = ONSET6;
        #[cfg(feature = "tune")]
        if let Ok(s) = std::env::var("CANTUS_M6") {
            for kv in s.split(',') {
                let mut it = kv.splitn(2, '=');
                let (Some(k), Some(v)) = (it.next(), it.next().and_then(|v| v.parse().ok())) else {
                    continue;
                };
                match k.trim() {
                    "contact_ff" => p.contact_ff_ms = v,
                    "contact_pp" => p.contact_pp_ms = v,
                    "mallet" => p.mallet = v,
                    "drawn_phase" => p.drawn_phase = v,
                    "knock" => p.knock = v,
                    "burst" => p.burst = v,
                    "vary" => p.vary_db = v,
                    "wire_lag" => p.wire_lag_ms = v,
                    "rise_lo" => p.rise_lo_ms = v,
                    "rise_hi" => p.rise_hi_ms = v,
                    "early" => p.early = v,
                    _ => {}
                }
            }
        }
        p
    })
}

impl Onset6 {
    /// The contact's length, samples, for a stroke at `vel` (1 … 127); `mallet` a felt beater.
    pub(crate) fn contact(&self, vel: f32, mallet: bool, sr: f32) -> u32 {
        let v = (vel.clamp(1.0, 127.0) - 1.0) / 126.0;
        let ms = self.contact_pp_ms + (self.contact_ff_ms - self.contact_pp_ms) * v;
        let ms = if mallet { ms * self.mallet } else { ms };
        ((ms * 0.001 * sr).round() as u32).max(1)
    }

    /// A band's rise (samples) at centre `hz`.
    pub(crate) fn rise(&self, hz: f32, sr: f32) -> f32 {
        let x = (crate::math::log2(hz.max(1.0) / 100.0) / crate::math::log2(80.0)).clamp(0.0, 1.0);
        let ms = self.rise_lo_ms + (self.rise_hi_ms - self.rise_lo_ms) * x;
        (ms * 0.001 * sr).max(1.0)
    }
}

/// The part of a mode not yet pushed in, from sample `n` to `n + 1` of a contact `len` long:
/// `(1 − F(n + 1)) / (1 − F(n))`, `F` the half-sine's integral (0 at the last sample).
#[inline]
pub(crate) fn contact_step(n: u32, len: u32) -> f32 {
    use std::f32::consts::PI;
    if n + 1 >= len {
        return 0.0;
    }
    let a = 1.0 + crate::math::cos(PI * n as f32 / len as f32);
    let b = 1.0 + crate::math::cos(PI * (n + 1) as f32 / len as f32);
    if a > 0.0 {
        (b / a).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// A band's share of its hit while it comes in: nothing before `lag` samples, then a raised
/// cosine over `rise` (1 once in).
#[inline]
pub(crate) fn ramp(t: f32, lag: f32, rise: f32) -> f32 {
    let x = (t - lag) / rise.max(1.0);
    if x >= 1.0 {
        1.0
    } else if x <= 0.0 {
        0.0
    } else {
        0.5 - 0.5 * crate::math::cos(std::f32::consts::PI * x)
    }
}

use super::super::shape::Noise;
use super::{Layer, Modes, Table, MAX_MODES};
use crate::math;
use std::f32::consts::{LN_10, PI, TAU};

/// What a model 6 hit's modes are struck from.
pub(crate) struct Six<'a> {
    pub t: &'static Table,
    /// The layers the hit blends, and their weights (`tick::blend`).
    pub w: &'a [(&'static Layer, f32); 4],
    pub main: usize,
    /// The law's level (amplitude) and where the stick lands (−1 … 1).
    pub level: f32,
    pub tilt: f32,
    pub n: usize,
    /// The object's angles (0: a mode beyond this rate's reach).
    pub th: &'a [f32; MAX_MODES],
    pub ratio: f32,
    pub decay: f32,
    pub sr: f32,
    pub p: &'a Onset6,
}

/// A model 6 hit's modes into `bank`: each mode's ring and early part the layers' blended,
/// one level draw for both (the hit's modal power then brought back to the blend's), their
/// phases the main layer's at the strike (a little scattered, the more the higher) or drawn;
/// the ring added to what rings, the early part to the growing phasors (as model 5's), and
/// their sum as the part the contact has still to push in (`contact_step` shrinks it to
/// nothing). What a previous contact had not pushed in yet comes in at once. Returns the
/// loudest mode's amplitude.
pub(crate) fn strike_modes(s: &Six<'_>, bank: &mut Modes, vary: &mut Noise) -> f32 {
    let amp = |db: f32| math::exp(db * (LN_10 / 20.0));
    bank.cre = [0.0; MAX_MODES];
    bank.cim = [0.0; MAX_MODES];
    bank.clive = [false; MAX_MODES / 8];
    let lay = s.w[s.main].0;
    let measured = s.p.drawn_phase < 0.5 && lay.ph.len() >= s.n;
    let onset = measured && lay.ets > 0.0 && lay.ea.len() >= s.n && lay.ep.len() >= s.n;
    let early_r = if onset {
        math::exp(-1.0 / (lay.ets * s.decay * s.sr))
    } else {
        0.0
    };
    // (on the stack: the audio thread allocates nothing)
    let mut a = [0.0f32; MAX_MODES];
    let mut e = [0.0f32; MAX_MODES];
    let mut q = [0.0f32; MAX_MODES];
    let mut qe = [0.0f32; MAX_MODES];
    let (mut want, mut got) = (0.0f32, 0.0f32);
    for i in 0..s.n {
        if s.th[i] == 0.0 {
            continue;
        }
        let (mut x, mut y) = (0.0, 0.0);
        for (l, k) in s.w.iter() {
            if *k > 0.0 {
                x += k * l.amp[i];
                if onset && l.ea.len() >= s.n {
                    y += k * l.ea[i];
                }
            }
        }
        if x <= 0.0 && y <= 0.0 {
            continue;
        }
        let mut g = s.level;
        if s.tilt != 0.0 {
            g *= amp(3.0 * s.tilt * math::log2(s.t.hz[i] / 250.0));
        }
        let (x, y) = (x * g, y * g * s.p.early);
        want += x * x + y * y;
        let d = amp(s.p.vary_db * (vary.next() + vary.next()));
        got += d * d * (x * x + y * y);
        a[i] = x * d;
        e[i] = y * d;
        if measured {
            let spread = 0.1 + s.t.hz[i] * s.ratio * 0.001;
            let j = spread.min(PI) * (vary.next() + vary.next() + vary.next());
            q[i] = lay.ph[i] + j;
            qe[i] = if onset { lay.ep[i] + j } else { 0.0 };
        } else {
            q[i] = TAU * vary.unit();
        }
    }
    let k = if got > 0.0 { (want / got).sqrt() } else { 1.0 };
    let mut amax = 0.0f32;
    for i in 0..s.n {
        let (x, y) = (a[i] * k, e[i] * k);
        if x <= 0.0 && y <= 0.0 {
            continue;
        }
        amax = amax.max(x);
        let (c, sn) = (math::cos(q[i]), math::sin(q[i]));
        bank.re[i] += x * c;
        bank.im[i] += x * sn;
        bank.live[i / 8] = true;
        // the layers' T60s blended geometrically, as model 5's
        let (mut lt, mut ws) = (0.0f32, 0.0f32);
        for (l, w) in s.w.iter() {
            if *w > 0.0 {
                lt += w * math::ln(l.t60[i].max(0.01));
                ws += w;
            }
        }
        let t60 = math::exp(lt / ws.max(1e-6)) * s.decay;
        bank.r[i] = math::exp(-6.907_755 / (t60 * s.sr));
        let (mut cr, mut ci) = (x * c, x * sn);
        if y > 0.0 {
            // the early part, as the growing part's phasor (the ring less it: so it is
            // added inverted), dying in its time
            let (ce, se) = (math::cos(qe[i]), math::sin(qe[i]));
            bank.rf[i] = early_r;
            bank.fre[i] -= y * ce;
            bank.fim[i] -= y * se;
            bank.flive[i / 8] = true;
            cr += y * ce;
            ci += y * se;
        }
        bank.cre[i] = cr;
        bank.cim[i] = ci;
        bank.clive[i / 8] = true;
    }
    bank.index(s.n);
    amax
}
