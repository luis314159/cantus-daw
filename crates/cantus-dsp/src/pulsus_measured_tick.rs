//! A measured piece's strike and its render, sample by sample (see `pulsus_measured.rs`).

use super::super::shape::Noise;
use super::{Bank, Layer, Object, Strike, Table, BP_MS, CTRL, MAX_SLOTS, NB, NBP};
use crate::math;
use std::f32::consts::{LN_10, PI, TAU};

/// dB (power) to amplitude.
#[inline]
fn amp_of(db: f32) -> f32 {
    if db <= -199.0 {
        0.0
    } else {
        math::exp(db * (LN_10 / 20.0))
    }
}

/// The bands `lo..hi`'s two band-pass sections each (b1 = 0, b2 = −b0) on one input
/// sample `x`, into `out`: the bands side by side, so the loop runs several at once.
#[inline(never)]
pub(super) fn pair(
    z: &mut [[f32; NB]; 4],
    out: &mut [f32; NB],
    bank: &BankCoefs<'_>,
    x: f32,
    lo: usize,
    hi: usize,
) {
    let [z0, z1, z2, z3] = z;
    for b in lo..hi.min(NB) {
        let (b0, a1, a2) = (bank.b0[b], bank.a1[b], bank.a2[b]);
        let v = b0 * x + z0[b];
        z0[b] = -a1 * v + z1[b];
        z1[b] = -b0 * x - a2 * v;
        let y = b0 * v + z2[b];
        z2[b] = -a1 * y + z3[b];
        z3[b] = -b0 * v - a2 * y;
        out[b] = y;
    }
}

/// The listed blocks' phasors one sample on: each turned by its angle and shrunk by its
/// radius (times `d` when `DAMP`: a choke damping the bank; without, the radius alone — the
/// same bits as times 1) — eight lanes side by side — and added to (or, `SUB`, taken from)
/// the eight running sums `va`, `vq` (drums-gen 3: the sums in lanes, kept in registers
/// over the whole list; a block's eight added one after another were a chain of sixteen
/// dependent adds, and the call per block cost more than its work).
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn run<const DAMP: bool, const SUB: bool>(
    re: &mut [f32; super::MAX_MODES],
    im: &mut [f32; super::MAX_MODES],
    r: &[f32; super::MAX_MODES],
    c: &[f32; super::MAX_MODES],
    s: &[f32; super::MAX_MODES],
    list: &[u8],
    d: f32,
    va: &mut [f32; 8],
    vq: &mut [f32; 8],
) {
    // Four lanes at a time, as values (a form the compiler keeps in vectors whatever the
    // crate's codegen split; with slices of eight it did not).
    let (mut a0, mut a1, mut q0, mut q1) = ([0.0f32; 4], [0.0f32; 4], [0.0f32; 4], [0.0f32; 4]);
    for &j in list {
        let o = (j as usize * 8).min(super::MAX_MODES - 8);
        for h in 0..2 {
            let p = o + 4 * h;
            let (Ok(x), Ok(y), Ok(r), Ok(c), Ok(s)) = (
                <[f32; 4]>::try_from(&re[p..p + 4]),
                <[f32; 4]>::try_from(&im[p..p + 4]),
                <[f32; 4]>::try_from(&r[p..p + 4]),
                <[f32; 4]>::try_from(&c[p..p + 4]),
                <[f32; 4]>::try_from(&s[p..p + 4]),
            ) else {
                continue;
            };
            let mut nx = [0.0f32; 4];
            let mut ny = [0.0f32; 4];
            for i in 0..4 {
                let rr = if DAMP { r[i] * d } else { r[i] };
                nx[i] = rr * (x[i] * c[i] - y[i] * s[i]);
                ny[i] = rr * (x[i] * s[i] + y[i] * c[i]);
            }
            re[p..p + 4].copy_from_slice(&nx);
            im[p..p + 4].copy_from_slice(&ny);
            let (a, q) = if h == 0 {
                (&mut a0, &mut q0)
            } else {
                (&mut a1, &mut q1)
            };
            for i in 0..4 {
                if SUB {
                    a[i] -= nx[i];
                    q[i] -= ny[i];
                } else {
                    a[i] += nx[i];
                    q[i] += ny[i];
                }
            }
        }
    }
    for i in 0..4 {
        va[i] += a0[i];
        va[i + 4] += a1[i];
        vq[i] += q0[i];
        vq[i + 4] += q1[i];
    }
}

/// A mode this small is gone (−400 dB): set to exact zero before it decays into subnormal
/// numbers (drums-gen 3: a quiet mode in a block a louder one kept live crawled on through
/// subnormal arithmetic, several times the cost of the whole bank, wherever the hardware does
/// not flush them).
const TINY: f32 = 1e-20;

/// A block's eight phasors at a control step: each under [`TINY`] set to zero; whether any
/// still stands over `floor`.
#[inline]
fn settle(re: &mut [f32], im: &mut [f32], floor: f32) -> bool {
    let mut live = false;
    for (x, y) in re.iter_mut().zip(im.iter_mut()) {
        let m = x.abs() + y.abs();
        if m < TINY {
            *x = 0.0;
            *y = 0.0;
        }
        live |= m > floor;
    }
    live
}

/// The bands' eighth-order high-pass, one sample (four sections, transposed direct form II;
/// `c`: b0, b1, b2, a1, a2 each; `z`: two states each).
#[inline]
pub(super) fn high_pass(c: &[f32; 20], z: &mut [f32; 8], x: f32) -> f32 {
    let mut v = x;
    for k in 0..4 {
        let (b0, b1, b2, a1, a2) = (
            c[5 * k],
            c[5 * k + 1],
            c[5 * k + 2],
            c[5 * k + 3],
            c[5 * k + 4],
        );
        let y = b0 * v + z[2 * k];
        z[2 * k] = b1 * v - a1 * y + z[2 * k + 1];
        z[2 * k + 1] = b2 * v - a2 * y;
        v = y;
    }
    v
}

/// A drum's low bands' fourth-order low-pass, one sample (two sections, as `high_pass`).
#[inline]
pub(super) fn low_pass(c: &[f32; 10], z: &mut [f32; 4], x: f32) -> f32 {
    let mut v = x;
    for k in 0..2 {
        let (b0, b1, b2, a1, a2) = (
            c[5 * k],
            c[5 * k + 1],
            c[5 * k + 2],
            c[5 * k + 3],
            c[5 * k + 4],
        );
        let y = b0 * v + z[2 * k];
        z[2 * k] = b1 * v - a1 * y + z[2 * k + 1];
        z[2 * k + 1] = b2 * v - a2 * y;
        v = y;
    }
    v
}

/// Eight lanes summed.
#[inline]
fn sum8(v: &[f32; 8]) -> f32 {
    ((v[0] + v[1]) + (v[2] + v[3])) + ((v[4] + v[5]) + (v[6] + v[7]))
}

/// `Σ v[b] · norm[b] · g[b]` over a range of bands, in eight lanes (drums-gen 3: one running
/// sum over 38 bands was a chain of dependent adds, each sample).
#[inline(never)]
fn weighted(v: &[f32], norm: &[f32], g: &[f32]) -> f32 {
    let mut a = [0.0f32; 8];
    let n = v.len().min(norm.len()).min(g.len());
    let (v, norm, g) = (&v[..n], &norm[..n], &g[..n]);
    let mut vc = v.chunks_exact(8);
    let mut nc = norm.chunks_exact(8);
    let mut gc = g.chunks_exact(8);
    for ((v, nm), g) in (&mut vc).zip(&mut nc).zip(&mut gc) {
        for i in 0..8 {
            a[i] += v[i] * (nm[i] * g[i]);
        }
    }
    for (i, ((v, nm), g)) in vc
        .remainder()
        .iter()
        .zip(nc.remainder())
        .zip(gc.remainder())
        .enumerate()
    {
        a[i] += v * (nm * g);
    }
    sum8(&a)
}

/// `Σ (v[b] − u[b]) · norm[b] · g[b]` (a drum's side: the second draw less the first).
#[inline(never)]
fn weighted_diff(v: &[f32], u: &[f32], norm: &[f32], g: &[f32]) -> f32 {
    let mut a = [0.0f32; 8];
    let n = v.len().min(u.len()).min(norm.len()).min(g.len());
    let (v, u, norm, g) = (&v[..n], &u[..n], &norm[..n], &g[..n]);
    let mut i = 0;
    while i + 8 <= n {
        for k in 0..8 {
            a[k] += (v[i + k] - u[i + k]) * (norm[i + k] * g[i + k]);
        }
        i += 8;
    }
    for k in 0..n - i {
        a[k] += (v[i + k] - u[i + k]) * (norm[i + k] * g[i + k]);
    }
    sum8(&a)
}

/// A drum's modes `a..b` at the glide `g`: the hit's own angles turned on by `θ · g`.
#[inline]
fn turn_modes(o: &mut Object, a: usize, b: usize, g: f64) {
    for i in a..b {
        let d = o.th[i] as f64 * g;
        let (sd, cd) = if d.abs() < 1.5 {
            small_sin_cos(d)
        } else {
            (math::sin(d), math::cos(d))
        };
        let (cb, sb) = (o.cb[i] as f64, o.sb[i] as f64);
        o.c[i] = (cb * cd - sb * sd) as f32;
        o.s[i] = (sb * cd + cb * sd) as f32;
    }
}

/// `(sin d, cos d)` for `|d| < 1.5` by their series to the 15th power (error under 1e-10).
#[inline]
fn small_sin_cos(d: f64) -> (f64, f64) {
    let x = d * d;
    let s = d
        * (1.0
            - x / 6.0
                * (1.0
                    - x / 20.0
                        * (1.0
                            - x / 42.0
                                * (1.0
                                    - x / 72.0
                                        * (1.0
                                            - x / 110.0
                                                * (1.0 - x / 156.0 * (1.0 - x / 210.0)))))));
    let c = 1.0
        - x / 2.0
            * (1.0
                - x / 12.0
                    * (1.0
                        - x / 30.0
                            * (1.0
                                - x / 56.0
                                    * (1.0
                                        - x / 90.0
                                            * (1.0
                                                - x / 132.0
                                                    * (1.0 - x / 182.0 * (1.0 - x / 240.0)))))));
    (s, c)
}

/// The share of the way between two of a hat's measured openings that still plays the
/// nearer one alone, at either end.
const STATE_HOLD: f32 = 0.2;

/// Below this a mode or a band is silent.
const FLOOR: f32 = 1e-8;
/// Below this a band's filters need not run (−140 dB re a full-scale band).
const BAND_FLOOR: f32 = 1e-7;
/// A piece's modes and bands rest this far under the loudest its hits struck (−100 dB): a
/// crash's long partials ran on, block by block, until −160 dB absolute (drums-gen 2: the
/// cost over budget).
const REL_FLOOR: f32 = 1e-5;
/// The stick's knock lasts 2 ms; its two one-poles sit at 1.2 kHz.
const KNOCK_S: f32 = 0.002;
const KNOCK_HZ: f32 = 1_200.0;
const SQRT3: f32 = 1.732_050_8;
/// A drum's side (drums-gen 2, cue 1): above this its noise and its stick's burst are heard
/// by the far microphones (and every reflection) from a second, uncorrelated draw — a close
/// mic hears the part of the head beside it, an overhead the whole head, and the two never
/// carry the same highs. One source in every mic combed the stroke at the overheads' delay
/// (cepstral peaks 6-15 against the recordings' 3-4.5). Below it the head's push is the
/// same for every mic (the pressure of a wavelength longer than the head). The side is the
/// difference (the second draw less the first), which a path adds to the main at 1.
const DRUM_SIDE_HZ: f32 = 1_000.0;
/// A drum's low bands: centred under this (drums-gen 3), low-passed at `DRUM_LP_HZ`.
const DRUM_LOW_HZ: f32 = 700.0;
pub(super) const DRUM_LP_HZ: f32 = 1_100.0;

/// The weights of the (up to four) layers a hit blends: two states by openness, two
/// layers by velocity in each; and the level offset (dB) below the softest layer.
fn blend(t: &'static Table, h: &Strike) -> ([(&'static Layer, f32); 4], f32) {
    let none = &t.states[0].layers[0];
    let mut w = [(none, 0.0f32); 4];
    let mut states = [(0usize, 1.0f32), (0usize, 0.0f32)];
    if h.foot {
        if let Some(i) = t.states.iter().position(|s| s.foot) {
            states[0] = (i, 1.0);
        }
    } else {
        // The stick states by openness (in the tables' order, closed to open).
        let mut lo: Option<usize> = None;
        let mut hi: Option<usize> = None;
        for (i, s) in t.states.iter().enumerate() {
            if s.foot {
                continue;
            }
            if s.openness <= h.openness && lo.is_none_or(|l| t.states[l].openness <= s.openness) {
                lo = Some(i);
            }
            if s.openness >= h.openness && hi.is_none_or(|k| t.states[k].openness >= s.openness) {
                hi = Some(i);
            }
        }
        match (lo, hi) {
            (Some(a), Some(b)) if a != b => {
                let (oa, ob) = (t.states[a].openness, t.states[b].openness);
                // Near a measured opening the plates play as measured there (the
                // recorded kits' CC4: a nearly closed hat is the closed one); between,
                // the two crossfade.
                let x = ((h.openness - oa) / (ob - oa)).clamp(0.0, 1.0);
                let x = ((x - STATE_HOLD) / (1.0 - 2.0 * STATE_HOLD)).clamp(0.0, 1.0);
                states = [(a, 1.0 - x), (b, x)];
            }
            (Some(a), _) | (None, Some(a)) => states[0] = (a, 1.0),
            _ => {}
        }
    }
    let mut offset = 0.0;
    for (k, &(si, sw)) in states.iter().enumerate() {
        let ls = &t.states[si].layers;
        let v = h.vel.clamp(1.0, 127.0);
        let (a, b, x) = if v <= ls[0].vel {
            if ls.len() > 1 {
                // Softer than the softest layer: its colour, the level law of the two
                // softest continued.
                let slope = (ls[1].level - ls[0].level) / (ls[1].vel - ls[0].vel).max(1.0);
                offset += sw * slope * (v - ls[0].vel);
            }
            (0, 0, 0.0)
        } else if let Some(j) = ls.iter().position(|l| l.vel >= v) {
            let x = (v - ls[j - 1].vel) / (ls[j].vel - ls[j - 1].vel).max(1e-3);
            (j - 1, j, x)
        } else {
            (ls.len() - 1, ls.len() - 1, 0.0)
        };
        w[2 * k] = (&ls[a], sw * (1.0 - x));
        w[2 * k + 1] = (&ls[b], sw * x);
    }
    (w, offset)
}

impl Object {
    /// Add a hit: a slot for its bands, its modes into the ringing bank, its stick.
    pub(super) fn hit(&mut self, t: &'static Table, h: &Strike, sr: f32) {
        let (w, offset) = blend(t, h);
        // The recording's level law across velocity (r42:tympanum-real; 0 for a table
        // without one): the whole hit, modes, bands and stick alike.
        // Model 6: a drum's onset through the stick's contact (`onset`; a cymbal plays 6 as 5).
        let six = t.drum && h.gen >= 6;
        let p6 = super::onset::params();
        let offset = offset
            + if six {
                super::law::law_db6(t.id, h.vel)
            } else {
                super::law::law_db(t.id, h.vel)
            };
        // Each state's share (two when a hat's stroke falls between two openings).
        let sws = [w[0].1 + w[1].1, w[2].1 + w[3].1];
        let two = sws[0] > 0.0 && sws[1] > 0.0;
        self.vary = Noise::from_seed(h.seed ^ 0x5DEE_CE66_D1CE_4E5B);
        // The slot: a free one, or the quietest.
        let si = (0..MAX_SLOTS)
            .find(|&i| !self.slots[i].on)
            .unwrap_or_else(|| {
                (0..MAX_SLOTS)
                    .min_by(|&a, &b| {
                        let e = |i: usize| self.slots[i].g.iter().map(|g| g * g).sum::<f32>();
                        e(a).total_cmp(&e(b))
                    })
                    .unwrap_or(0)
            });
        let vary = &mut self.vary;
        let slot = &mut self.slots[si];
        slot.on = true;
        slot.t = 0;
        slot.seg = 0;
        slot.choking = false;
        slot.ramp = six;
        if six {
            // The bands come in through the head, the lows slower; a snare's wires (its
            // bands from 1 kHz) only once the shell's air has pushed the bottom head.
            let snare = t.id % 16 == 5;
            for b in 0..NB {
                let c = super::CENTRES[b];
                slot.rise[b] = p6.rise(c, sr);
                slot.lag[b] = if snare && c >= 1_000.0 {
                    p6.wire_lag_ms * 0.001 * sr
                } else {
                    0.0
                };
            }
        }
        let tilt = h.tilt.clamp(-1.0, 1.0);
        for b in 0..NB {
            // Each band ± ~0.7 dB this hit, and tilted by where the stick lands.
            let jitter = 0.7 * (vary.next() + vary.next() + vary.next())
                + if tilt != 0.0 {
                    3.0 * tilt * math::log2(super::CENTRES[b] / 1_000.0)
                } else {
                    0.0
                };
            for k in 0..NBP {
                // Two states (a hat between two openings): each state's velocity layers in
                // dB, the states mixed in power, as two recordings crossfaded are — the
                // modes below likewise (their power), so a nearly closed stroke is the
                // closed one with a little of the quarter-open one, partials and noise in
                // the same proportion.
                slot.db[b * NBP + k] = if two {
                    let mut p = 0.0;
                    for (s, sw) in sws.iter().enumerate() {
                        if *sw > 0.0 {
                            let (lo, hi) = (w[2 * s], w[2 * s + 1]);
                            let db =
                                (lo.1 * lo.0.env[b * NBP + k] + hi.1 * hi.0.env[b * NBP + k]) / sw;
                            if db > -199.0 {
                                p += sw * math::exp(db * (LN_10 / 10.0));
                            }
                        }
                    }
                    if p > 0.0 {
                        offset + jitter + 10.0 * math::log10(p)
                    } else {
                        -200.0
                    }
                } else {
                    let mut db = offset + jitter;
                    let mut wsum = 0.0;
                    for (l, x) in w.iter() {
                        if *x > 0.0 {
                            db += x * l.env[b * NBP + k];
                            wsum += x;
                        }
                    }
                    if wsum > 0.0 {
                        db
                    } else {
                        -200.0
                    }
                };
            }
        }
        slot.set_segment(0, 0.0, self.decay, sr);
        let gmax = slot.g.iter().fold(0.0f32, |m, g| m.max(*g));
        self.bfloor = self.bfloor.min((gmax * REL_FLOOR).max(BAND_FLOOR));
        // The hit sounds at once: the bands' gains jump (no wait for the control step). Model
        // 6's come in from the next control step, by their ramps.
        if !six {
            for b in 0..NB {
                let g = slot.g[b];
                self.bg[b] = (self.bg[b] * self.bg[b] + g * g).sqrt();
                self.bd[b] = 0.0;
            }
        }
        // The modes: each layer's amplitude blended, a random phase, ± ~1.5 dB.
        let level = amp_of(offset);
        let main = w
            .iter()
            .enumerate()
            .max_by(|a, b| a.1 .1.total_cmp(&b.1 .1))
            .map(|(i, _)| i)
            .unwrap_or(0);
        let mut amax = 0.0f32;
        let bank = &mut self.banks[self.cur];
        if six {
            amax = super::onset::strike_modes(
                &super::onset::Six {
                    t,
                    w: &w,
                    main,
                    level,
                    tilt,
                    n: self.n,
                    th: &self.th,
                    ratio: self.ratio,
                    decay: self.decay,
                    sr,
                    p: p6,
                },
                bank,
                vary,
            );
        } else {
            // A drum's onset (its main layer's): each mode's phase at the strike, and its
            // early part — the head's first push, a few milliseconds long. The phases scatter
            // a little each hit, the more the higher the mode (a strike's highs differ hit to
            // hit; its low push does not).
            let lay = w[main].0;
            let onset = t.drum && lay.ets > 0.0 && lay.ph.len() >= self.n && lay.ea.len() >= self.n;
            let early_r = if onset {
                math::exp(-1.0 / (lay.ets * self.decay * sr))
            } else {
                0.0
            };
            // Model 6's hats: each split pair's halves drawn together (on the stack).
            let lock = h.gen >= 6 && t.hat && t.lock.len() >= self.n;
            let mut drawn = [f32::NAN; super::MAX_MODES];
            for i in 0..self.n {
                let mut a = 0.0;
                let mut e = 0.0;
                for (l, x) in w.iter() {
                    if *x > 0.0 {
                        a += x * l.amp[i];
                        if onset && l.ea.len() >= self.n {
                            e += x * l.ea[i];
                        }
                    }
                }
                if two {
                    // (the states mixed in power, as the bands are)
                    let mut p = 0.0;
                    for (s, sw) in sws.iter().enumerate() {
                        if *sw > 0.0 {
                            let (lo, hi) = (w[2 * s], w[2 * s + 1]);
                            let q = (lo.1 * lo.0.amp[i] + hi.1 * hi.0.amp[i]) / sw;
                            p += sw * q * q;
                        }
                    }
                    a = p.sqrt();
                }
                // (nothing, or a mode beyond this rate's reach)
                if (a <= 0.0 && e <= 0.0) || self.th[i] == 0.0 {
                    continue;
                }
                let mut a = a * level * amp_of(1.5 * (vary.next() + vary.next()));
                amax = amax.max(a);
                if tilt != 0.0 {
                    a *= amp_of(3.0 * tilt * math::log2(t.hz[i] / 250.0));
                }
                let (ph, jitter) = if onset {
                    let spread = 0.1 + t.hz[i] * self.ratio * 0.001;
                    let j = spread.min(PI) * (vary.next() + vary.next() + vary.next());
                    (lay.ph[i] + j, j)
                } else {
                    let d = TAU * vary.unit();
                    if lock {
                        // Model 6: a hat's close partial takes its pair's draw (`data::locks`).
                        let j = t.lock[i] as usize;
                        let p = if j != i && drawn[j].is_finite() {
                            drawn[j]
                        } else {
                            d
                        };
                        drawn[i] = p;
                        (p, 0.0)
                    } else {
                        (d, 0.0)
                    }
                };
                let (c, s) = (math::cos(ph), math::sin(ph));
                bank.re[i] += a * c;
                bank.im[i] += a * s;
                bank.live[i / 8] = true;
                // The T60 between the layers the hit blends, geometrically (a hat between
                // closed and a quarter open rings between their times, not the main one's).
                let mut lt = 0.0;
                let mut ws = 0.0;
                for (l, x) in w.iter() {
                    if *x > 0.0 {
                        lt += x * math::ln(l.t60[i].max(0.01));
                        ws += x;
                    }
                }
                let t60 = if ws > 0.0 {
                    math::exp(lt / ws)
                } else {
                    w[main].0.t60[i].max(0.01)
                } * self.decay;
                bank.r[i] = math::exp(-6.907_755 / (t60 * sr));
                // The growing part as the layers it comes from have it (by their share of this
                // mode): a partial only a quarter-open state has keeps that state's bloom when
                // a nearly closed stroke blends it in, not the closed state's "at once".
                let (mut rise, mut direct, mut aw) = (0.0f32, 0.0f32, 0.0f32);
                for (l, x) in w.iter() {
                    let k = x * l.amp[i];
                    if k > 0.0 {
                        rise += k * l.rise[i];
                        direct += k * l.direct[i];
                        aw += k;
                    }
                }
                if aw > 0.0 {
                    rise /= aw;
                    direct /= aw;
                } else {
                    rise = w[main].0.rise[i];
                    direct = w[main].0.direct[i];
                }
                if onset {
                    // The early part, as the growing part's phasor (the ring less it: so
                    // it is added inverted), dying in `ets`.
                    if e > 0.0 {
                        let e = e * level * amp_of(1.5 * (vary.next() + vary.next()));
                        let q = lay.ep[i] + jitter;
                        bank.rf[i] = early_r;
                        bank.fre[i] -= e * math::cos(q);
                        bank.fim[i] -= e * math::sin(q);
                        bank.flive[i / 8] = true;
                    }
                } else if rise > 0.0 {
                    let d = direct.clamp(0.0, 1.0);
                    bank.rf[i] = bank.r[i] * math::exp(-1.0 / (rise * self.decay * sr));
                    bank.fre[i] += a * (1.0 - d) * c;
                    bank.fim[i] += a * (1.0 - d) * s;
                    bank.flive[i / 8] = true;
                }
            }
            bank.index(self.n);
        }
        self.mfloor = self.mfloor.min((amax * REL_FLOOR).max(FLOOR));
        // The stick: a burst shorter the harder, and its knock.
        let mut st = [0.0f32; 2];
        for (l, x) in w.iter() {
            st[0] += x * l.stick[0];
            st[1] += x * l.stick[1];
        }
        self.stick = [
            st[0] * level * h.click.max(0.0) * super::law::burst(t.id),
            st[1] * level,
        ];
        if six {
            self.stick[0] *= p6.burst;
            self.stick[1] *= p6.knock;
        }
        // A drum: the hit pushes the head sharp (its glide, the harder the more; a ringing
        // head is pushed again), and is a few cents its own (±3).
        self.drum = t.drum;
        if t.drum {
            // the layers' glides blended as the hit blends them (no step between layers)
            let mut gl = [0.0f32; 2];
            for (l, x) in w.iter() {
                gl[0] += x * l.glide[0];
                gl[1] += x * l.glide[1];
            }
            self.glide = self.glide.max(gl[0].max(0.0));
            self.glide_k = math::exp(-(CTRL as f32) / (gl[1].max(0.005) * sr));
            self.detune = 0.0017 * vary.next();
            self.base();
            self.turn(true);
        }
        self.stick_t = 0;
        self.burst_s = Noise::from_seed(h.seed ^ 0x2F6B_4A1D_93C5_E871);
        let v = h.vel.clamp(1.0, 127.0) / 127.0;
        self.stick_len = ((0.0004 * (1.4 - 0.6 * v) * sr) as u32).max(2);
        self.six = six;
        if six {
            // The contact (a kick's felt beater the longer), and the knock as its push.
            self.contact_len = p6.contact(h.vel, t.id % 16 == 4, sr);
            self.contact_n = 0;
            self.stick_end = (self.contact_len + (0.001 * sr) as u32).max(self.stick_len);
        } else {
            self.contact_len = 0;
            self.contact_n = 0;
            self.stick_end = (KNOCK_S * sr) as u32;
        }
        self.band_range();
        self.active = true;
    }

    /// Render `out.len()` samples, **adding**, and its side into `side` (the same plate
    /// heard from another side: its bands on their own noise, its modes in quadrature —
    /// as loud, uncorrelated). Returns whether the piece still sounds.
    pub(super) fn render(
        &mut self,
        bank: &BankCoefs<'_>,
        out: &mut [f32],
        side: &mut [f32],
    ) -> bool {
        let sr = bank.sr;
        let knock_len = (KNOCK_S * sr) as u32;
        let ka = math::exp(-TAU * KNOCK_HZ / sr);
        for (y, ys) in out.iter_mut().zip(side.iter_mut()) {
            if self.phase.is_multiple_of(CTRL) {
                self.control(sr);
                if !self.active {
                    return false;
                }
            }
            self.phase = self.phase.wrapping_add(1);
            let w = self.noise.next() * SQRT3;
            let ws = self.noise_s.next() * SQRT3;
            let mut acc = 0.0;
            let mut acs = 0.0;
            let nb = bank.nbands;
            // Only the bands that sound (`band_range`): a silent band's filters wait.
            let (lo, hi) = (self.blo, self.bhi.min(nb));
            pair(&mut self.z, &mut self.vz, bank, w, lo, hi);
            if self.split {
                // A kick's or a tom's low bands (under `DRUM_LOW_HZ`) apart: high-passed under its
                // fundamental and low-passed over them (drums-gen 3, cues 4 and 5 — their
                // skirts, 12 dB an octave each way, put the body's noise under the head's
                // fundamental and, 200-500 ms on, kept its 1-4 kHz ringing 1.3 s where the
                // recording's falls in 0.4).
                let s = bank.low_to.clamp(lo, hi);
                let mut xl = if lo < s {
                    weighted(&self.vz[lo..s], &bank.norm[lo..s], &self.bg[lo..s])
                } else {
                    0.0
                };
                if self.hp_hz > 0.0 {
                    xl = high_pass(&self.hp, &mut self.hpm, xl);
                }
                xl = low_pass(bank.lp, &mut self.lpz, xl);
                let xh = if s < hi {
                    weighted(&self.vz[s..hi], &bank.norm[s..hi], &self.bg[s..hi])
                } else {
                    0.0
                };
                acc += xl + xh;
            } else if lo < hi {
                let x = weighted(&self.vz[lo..hi], &bank.norm[lo..hi], &self.bg[lo..hi]);
                acc += if self.hp_hz > 0.0 {
                    high_pass(&self.hp, &mut self.hpm, x)
                } else {
                    x
                };
            }
            if !self.drum {
                if lo < hi {
                    pair(&mut self.y, &mut self.vy, bank, ws, lo, hi);
                    let x = weighted(&self.vy[lo..hi], &bank.norm[lo..hi], &self.bg[lo..hi]);
                    acs += if self.hp_hz > 0.0 {
                        high_pass(&self.hp, &mut self.hps, x)
                    } else {
                        x
                    };
                }
            } else {
                let cut = bank.side_from.max(lo);
                if cut < hi {
                    pair(&mut self.y, &mut self.vy, bank, ws, cut, hi);
                    acs += weighted_diff(
                        &self.vy[cut..hi],
                        &self.vz[cut..hi],
                        &bank.norm[cut..hi],
                        &self.bg[cut..hi],
                    );
                }
            }
            for b in 0..nb {
                self.bg[b] = (self.bg[b] + self.bd[b]).max(0.0);
            }
            let (m, ms) = self.modes();
            acc += m;
            if !self.drum {
                acs += ms;
            }
            if self.stick_t < self.stick_end {
                let t = self.stick_t as f32;
                let z = self.vary.next() * SQRT3;
                if self.stick_t < self.stick_len {
                    let s = self.stick[0] * math::sin(PI * t / self.stick_len as f32);
                    acc += s * z;
                    if self.drum {
                        acs += s * (self.burst_s.next() * SQRT3 - z);
                    } else {
                        // (a cymbal's other side has its own burst: a mono mic hears it)
                        acs += s * self.burst_s.next() * SQRT3;
                    }
                }
                let x = if !self.six {
                    math::sin(PI * t / knock_len as f32) * z
                } else if self.stick_t < self.contact_len {
                    // Model 6: the head's push, the contact's own half-sine.
                    math::sin(PI * t / self.contact_len as f32)
                } else {
                    0.0
                };
                self.knock[0] = (1.0 - ka) * x + ka * self.knock[0];
                self.knock[1] = (1.0 - ka) * self.knock[0] + ka * self.knock[1];
                acc += self.stick[1] * self.knock[1];
                if !self.drum {
                    acs += self.stick[1] * self.knock[1];
                }
                self.stick_t += 1;
            }
            *y += acc * self.gain;
            *ys += acs * self.gain;
        }
        self.active
    }

    /// The bands that sound — now or by the end of this control step — as one range
    /// (`blo..bhi`); a band under [`BAND_FLOOR`] both ways gives nothing a listener could
    /// hear, and its filters wait (their state is noise's either way).
    fn band_range(&mut self) {
        let on = |b: usize| {
            self.bg[b] > self.bfloor || self.bg[b] + self.bd[b] * CTRL as f32 > self.bfloor
        };
        match ((0..NB).find(|&b| on(b)), (0..NB).rev().find(|&b| on(b))) {
            (Some(a), Some(z)) => {
                self.blo = a;
                self.bhi = z + 1;
            }
            _ => {
                self.blo = 0;
                self.bhi = 0;
            }
        }
    }

    /// A drum's modes at its pitch now: each angle times the glide's ratio and the hit's
    /// detune.
    ///
    /// (drums-gen 3, the cost: a cosine and a sine per mode each control step were most of a
    /// drum's render while it glides.) The hit's own angles (`cb`, `sb`: the detune alone,
    /// as before) are turned on by the glide's `θ · glide`, a short series in f64 — the same
    /// angles to an ulp; with no glide left, the hit's own, to the bit.
    ///
    /// `all`: every mode (a hit, whose modes may wake any block); otherwise only the blocks
    /// that sound (`Modes::index`'s lists, both banks) — a silent mode's angle waits for the
    /// hit that wakes it.
    fn turn(&mut self, all: bool) {
        let g = self.glide as f64;
        if g == 0.0 {
            self.c[..self.n].copy_from_slice(&self.cb[..self.n]);
            self.s[..self.n].copy_from_slice(&self.sb[..self.n]);
            return;
        }
        if all {
            turn_modes(self, 0, self.n, g);
            return;
        }
        for k in 0..2 {
            for q in 0..self.banks[k].nl + self.banks[k].nf {
                let b = &self.banks[k];
                let j = if q < b.nl { b.lj[q] } else { b.fj[q - b.nl] } as usize * 8;
                turn_modes(self, j, (j + 8).min(self.n), g);
            }
        }
    }

    /// The hit's own angles: each mode's times its detune.
    fn base(&mut self) {
        let k = 1.0 + self.detune;
        for i in 0..self.n {
            let th = self.th[i] * k;
            self.cb[i] = math::cos(th);
            self.sb[i] = math::sin(th);
        }
    }

    /// Both banks' modes, one sample: the ring less the part still growing in — and the
    /// same in quadrature (the side).
    #[inline]
    pub(super) fn modes(&mut self) -> (f32, f32) {
        let mut va = [0.0f32; 8];
        let mut vq = [0.0f32; 8];
        // Model 6: while the stick is on the head, the part of its modes not yet pushed in
        // shrinks by the contact's step (`onset::contact_step`), to nothing as it leaves.
        let contact = if self.contact_n < self.contact_len {
            let d = super::onset::contact_step(self.contact_n, self.contact_len);
            self.contact_n += 1;
            Some(d)
        } else {
            None
        };
        for k in 0..2 {
            let (choked, d) = (self.bank_choke[k], self.choke);
            let b = &mut self.banks[k];
            let (c, s) = (&self.c, &self.s);
            // Only the blocks that sound (`Modes::index`: the lists kept with the flags).
            let (lj, fj) = (&b.lj[..b.nl], &b.fj[..b.nf]);
            if let (Some(dc), false, true) = (contact, choked, k == self.cur) {
                run::<false, false>(&mut b.re, &mut b.im, &b.r, c, s, lj, d, &mut va, &mut vq);
                run::<true, true>(
                    &mut b.fre, &mut b.fim, &b.rf, c, s, fj, dc, &mut va, &mut vq,
                );
            } else if choked {
                run::<true, false>(&mut b.re, &mut b.im, &b.r, c, s, lj, d, &mut va, &mut vq);
                run::<true, true>(&mut b.fre, &mut b.fim, &b.rf, c, s, fj, d, &mut va, &mut vq);
            } else {
                run::<false, false>(&mut b.re, &mut b.im, &b.r, c, s, lj, d, &mut va, &mut vq);
                run::<false, true>(&mut b.fre, &mut b.fim, &b.rf, c, s, fj, d, &mut va, &mut vq);
            }
        }
        (sum8(&va), sum8(&vq))
    }

    /// One control step: the slots' envelopes, the bands' gains, what has fallen silent.
    fn control(&mut self, sr: f32) {
        let mut any = false;
        if self.drum && self.glide > 1e-5 {
            self.glide *= self.glide_k;
            if self.glide <= 1e-5 {
                self.glide = 0.0;
            }
            self.turn(false);
        }
        for s in &mut self.slots {
            if !s.on {
                continue;
            }
            s.t += CTRL;
            let ms = s.t as f32 * 1000.0 / (sr * self.decay);
            let mut moved = false;
            while s.seg + 1 < NBP && ms >= BP_MS[s.seg + 1] {
                s.seg += 1;
                moved = true;
            }
            if moved {
                s.set_segment(s.seg, ms - BP_MS[s.seg], self.decay, sr);
            } else {
                for b in 0..NB {
                    s.g[b] *= s.m[b];
                }
            }
            if s.choking {
                for g in &mut s.g {
                    *g *= self.choke_ctrl;
                }
            }
            if (ms > 100.0 && s.g.iter().all(|g| *g < FLOOR)) || ms > 30_000.0 {
                s.on = false;
            } else {
                any = true;
            }
        }
        for b in 0..NB {
            let mut p = 0.0;
            for s in &self.slots {
                if s.on {
                    // (model 6: a band still coming in, by its ramp)
                    let g = if s.ramp {
                        s.g[b] * super::onset::ramp(s.t as f32, s.lag[b], s.rise[b])
                    } else {
                        s.g[b]
                    };
                    p += g * g;
                }
            }
            self.bd[b] = (p.sqrt() - self.bg[b]) / CTRL as f32;
        }
        self.band_range();
        for k in 0..2 {
            let b = &mut self.banks[k];
            // (only the listed blocks can be live: the rest stay as they are)
            for q in 0..b.nl {
                let j = b.lj[q] as usize * 8;
                b.live[j / 8] = settle(&mut b.re[j..j + 8], &mut b.im[j..j + 8], self.mfloor);
            }
            for q in 0..b.nf {
                let j = b.fj[q] as usize * 8;
                b.flive[j / 8] = settle(&mut b.fre[j..j + 8], &mut b.fim[j..j + 8], self.mfloor);
            }
            b.index(self.n);
            let alive = b.nl + b.nf > 0;
            if !alive {
                self.bank_choke[k] = false;
            }
            any |= alive;
        }
        for z in self
            .hpm
            .iter_mut()
            .chain(self.hps.iter_mut())
            .chain(self.lpz.iter_mut())
        {
            if z.abs() < TINY {
                *z = 0.0;
            }
        }
        any |= self.stick_t < self.stick_end;
        any |= self.bg.iter().any(|g| *g > self.bfloor);
        self.active = any;
    }
}

impl super::Slot {
    /// Enter segment `k`, `into` ms past its breakpoint: every band's amplitude now and its
    /// factor per control step (past the last breakpoint the last slope goes on).
    pub(super) fn set_segment(&mut self, k: usize, into: f32, decay: f32, sr: f32) {
        let (a, b) = if k + 1 < NBP {
            (k, k + 1)
        } else {
            (NBP - 2, NBP - 1)
        };
        let span = (BP_MS[b] - BP_MS[a]).max(1e-3);
        let step_ms = CTRL as f32 * 1000.0 / (sr * decay);
        for band in 0..NB {
            let d0 = self.db[band * NBP + a];
            let d1 = self.db[band * NBP + b];
            if d0 <= -199.0 || d1 <= -199.0 {
                self.g[band] = 0.0;
                self.m[band] = 0.0;
                continue;
            }
            // Past the last breakpoint the band keeps falling, at least 10 dB a second.
            let slope = if k + 1 < NBP {
                (d1 - d0) / span
            } else {
                ((d1 - d0) / span).min(-0.01)
            };
            let at = self.db[band * NBP + k] + slope * into;
            self.g[band] = amp_of(at.min(20.0));
            self.m[band] = amp_of(slope * step_ms).min(4.0);
        }
    }
}

/// Where a piece's stereo lane goes: `(bus, left, right)`.
pub(crate) type StereoOut<'a> = dyn FnMut(u8, &[f32], &[f32]) + 'a;

/// The bank's shared band filters, borrowed while its objects render.
pub(crate) struct BankCoefs<'a> {
    pub sr: f32,
    pub nbands: usize,
    /// The first band of a drum's side (`DRUM_SIDE_HZ`).
    pub side_from: usize,
    /// The first band over a drum's low bands (`DRUM_LOW_HZ`), and their low-pass.
    pub low_to: usize,
    pub lp: &'a [f32; 10],
    pub b0: &'a [f32; NB],
    pub a1: &'a [f32; NB],
    pub a2: &'a [f32; NB],
    pub norm: &'a [f32; NB],
}

impl Bank {
    /// Render every sounding piece as a stereo lane — `left` its main plus its side,
    /// `right` its main less it (cleared first) — handing each to `out(bus, left, right)`
    /// in object order. Returns whether any piece sounded.
    pub(crate) fn render(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        out: &mut StereoOut<'_>,
        gain: f32,
    ) -> bool {
        let coefs = BankCoefs {
            sr: self.sr,
            nbands: self.nbands,
            side_from: super::CENTRES
                .iter()
                .position(|c| *c >= DRUM_SIDE_HZ)
                .unwrap_or(NB),
            low_to: super::CENTRES
                .iter()
                .position(|c| *c >= DRUM_LOW_HZ)
                .unwrap_or(NB),
            lp: &self.drum_lp,
            b0: &self.b0,
            a1: &self.a1,
            a2: &self.a2,
            norm: &self.norm,
        };
        let mut any = false;
        for o in &mut self.objects {
            if !o.active {
                continue;
            }
            left.iter_mut().for_each(|x| *x = 0.0);
            right.iter_mut().for_each(|x| *x = 0.0);
            o.render(&coefs, left, right);
            for (m, s) in left.iter_mut().zip(right.iter_mut()) {
                let (a, b) = (*m * gain, *s * gain);
                *m = a + b;
                *s = a - b;
            }
            any = true;
            out(o.bus, left, right);
        }
        any
    }
}
