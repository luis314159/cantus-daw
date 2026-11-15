//! Loquax's sample loop and the banks' tuning (a child of [`super`]).

use super::bank::{design, layout_min, BandDesign, CENTRE_MAX, MAX_BANDS, MIN_BAND_HZ};
use super::carrier::NoteEvent;
use super::{t, Loquax, Source, FLAT_CAP, FLAT_FLOOR, MAKEUP, NOISE_SEED, RETUNE};
use crate::dynfx_follow::{sane, to_db, Times};
use crate::math;
use crate::modfx_clock::{peak_db, peak_of, random_bipolar, tel};

/// The unvoiced noise's level in the carrier at full gate and amount.
const NOISE_LEVEL: f32 = 0.3;
/// A state under this, its input exactly zero, settles to +0.0 (−140 dB on the square).
const ENV_SETTLE: f32 = 1e-14;

impl Loquax {
    /// The banks, the followers' times and the bands' gains for the glides as they are. A
    /// bank whose layout has not moved is not redesigned (a formant glide retunes only the
    /// synthesis bank).
    pub(super) fn retune(&mut self) {
        let n = self.n;
        let m = self.m.max(1);
        let sr = self.sr;
        let g = &self.gl;
        let (lo, hi) = (math::exp2(g.lo.value), math::exp2(g.hi.value));
        let width = g.width.value;
        let shift = math::exp2(g.formant.value / 12.0);
        let (tilt, spread) = (g.tilt.value, g.spread.value);
        let s = &self.set;
        let key_a = [
            n as u32,
            m as u32,
            g.lo.value.to_bits(),
            g.hi.value.to_bits(),
            width.to_bits(),
            s.attack_ms.to_bits(),
            s.release_ms.to_bits(),
        ];
        let key_s = [key_a[0], key_a[1], key_a[2], key_a[3], key_a[4], g.formant.value.to_bits()];
        let (redo_a, redo_s) = (key_a != self.key_a, key_s != self.key_s);
        let mut da = [BandDesign::default(); MAX_BANDS];
        let mut ds = [BandDesign::default(); MAX_BANDS];
        for b in 0..n {
            let (c, el, eh) = layout_min(b, n, lo, hi, width, MIN_BAND_HZ);
            // A band the floor widened, turned down by the root of the widening.
            let (_, nl, nh) = layout_min(b, n, lo, hi, width, 0.0);
            let wide = if eh / el > nh / nl {
                (math::log2(nh / nl) / math::log2(eh / el)).sqrt()
            } else {
                1.0
            };
            let cs = c * shift;
            if redo_a {
                da[b] = design(el, eh, m, sr);
                self.centres_a[b] = da[b].centre;
                // No band follows faster than half its period up or two periods down.
                let att = s.attack_ms.clamp(0.1, 500.0).max(500.0 / c);
                let rel = s.release_ms.clamp(1.0, 5_000.0).max(2_000.0 / c);
                // The follower runs on the square: half the time constant there is the
                // time constant of the level heard.
                self.t_m[b] = Times::new(0.5 * att, 0.5 * rel, sr);
            }
            if redo_s {
                ds[b] = design(el * shift, eh * shift, m, sr);
                ds[b].live = cs <= CENTRE_MAX * sr && cs >= 20.0;
                self.centres_s[b] = if ds[b].live { ds[b].centre } else { cs };
            }
            let db = s.band_db[b].clamp(-24.0, 12.0) + tilt * math::log2(cs / 1_000.0);
            let lvl = if db == 0.0 { 1.0 } else { math::db_to_gain(db) };
            let pan = if spread == 0.0 {
                0.0
            } else if b % 2 == 0 {
                -spread
            } else {
                spread
            };
            let (pl, pr) = if pan == 0.0 {
                (1.0, 1.0)
            } else {
                ((1.0 - pan).sqrt(), (1.0 + pan).sqrt())
            };
            self.gain_to[0][b] = lvl * pl * wide;
            self.gain_to[1][b] = lvl * pr * wide;
        }
        if redo_a {
            self.ana.tune(&da[..n], m);
            self.key_a = key_a;
        }
        if redo_s {
            for bank in &mut self.syn {
                bank.tune(&ds[..n], m);
            }
            self.key_s = key_s;
        }
    }

    /// The modulator's level for synthesis band `b`, the band shift applied.
    #[inline]
    pub(super) fn mapped_amp(&self, b: usize) -> f32 {
        let shift = self.gl.shift.value;
        if shift == 0.0 {
            return self.amp[b];
        }
        let src = b as f32 - shift;
        let last = (self.n - 1) as f32;
        if !(0.0..=last).contains(&src) {
            return 0.0;
        }
        let i0 = src.floor();
        let f = src - i0;
        let i0 = i0 as usize;
        if f == 0.0 || i0 + 1 >= self.n {
            self.amp[i0]
        } else {
            self.amp[i0] + (self.amp[i0 + 1] - self.amp[i0]) * f
        }
    }

    /// One block. `side`: the sidechain, when one is routed; `notes`: the carrier's notes,
    /// in frame order, frames within the block (a later one lands after the last sample).
    pub fn process(
        &mut self,
        l: &mut [f32],
        r: &mut [f32],
        side: Option<(&[f32], &[f32])>,
        notes: &[NoteEvent],
    ) {
        let n = l.len().min(r.len());
        let in_peak = peak_of(&l[..n], &r[..n]);
        let mut car_peak = 0.0f32;
        if self.engaged {
            car_peak = self.run(&mut l[..n], &mut r[..n], side, notes);
        } else {
            for e in notes {
                self.synth.note(e.kind, false);
            }
            for i in 0..n {
                let g = self.gl.out.next();
                if g != 1.0 {
                    l[i] *= g;
                    r[i] *= g;
                }
            }
        }
        self.telemetry_block(in_peak, car_peak, &l[..n], &r[..n]);
        self.settle_engaged();
    }

    fn run(
        &mut self,
        l: &mut [f32],
        r: &mut [f32],
        side: Option<(&[f32], &[f32])>,
        notes: &[NoteEvent],
    ) -> f32 {
        let frames = l.len();
        let nb = self.n;
        let s = self.set;
        let use_synth = s.source != Source::Sidechain || s.robot;
        let use_ext = s.source != Source::Synth;
        let (gate_on, gate_off) = (
            math::db_to_gain(s.robot_gate_db.clamp(-90.0, 0.0)),
            math::db_to_gain(s.robot_gate_db.clamp(-90.0, 0.0) - 6.0),
        );
        let mut ev = 0usize;
        let mut ya = [0.0f32; MAX_BANDS];
        let mut yl = [0.0f32; MAX_BANDS];
        let mut yr = [0.0f32; MAX_BANDS];
        let mut car_peak = 0.0f32;
        for i in 0..frames {
            while ev < notes.len() && notes[ev].frame as usize <= i {
                self.synth.note(notes[ev].kind, true);
                ev += 1;
            }
            let g = &mut self.gl;
            let mix = g.mix.next();
            let out = g.out.next();
            if !g.tuning_still() {
                for gl in g.tuning() {
                    gl.next();
                }
                self.retune_n += 1;
                let landed = self.gl.tuning_still();
                if landed || self.retune_n >= RETUNE {
                    self.retune();
                    self.retune_n = 0;
                }
                self.tuning_moving = !landed;
            }
            let g = &mut self.gl;
            g.shift.next();
            let flat = g.flatten.next();
            let sib = g.sibilance.next();
            let unv = g.unvoiced.next();
            let cpass = g.carrier.next();
            if self.gain != self.gain_to {
                let k = self.gain_k;
                for ch in 0..2 {
                    for b in 0..nb {
                        let (v, to) = (self.gain[ch][b], self.gain_to[ch][b]);
                        if v != to {
                            let nv = v + (to - v) * k;
                            self.gain[ch][b] = if (nv - to).abs() < super::SNAP || nv == v {
                                to
                            } else {
                                nv
                            };
                        }
                    }
                }
            }
            let (xl, xr) = (sane(l[i]), sane(r[i]));
            let (sl, sr) = match side {
                Some((a, b)) if i < a.len() && i < b.len() => (sane(a[i]), sane(b[i])),
                _ => (0.0, 0.0),
            };
            let (voice, ext) = if s.swap {
                (0.5 * (sl + sr), (xl, xr))
            } else {
                (0.5 * (xl + xr), (sl, sr))
            };
            // The robot's ear.
            let a = voice.abs();
            let lvl = self.level.run(a, &self.t_level);
            if a.to_bits() == 0 {
                self.level.settle(1e-6);
            }
            if s.robot {
                if !self.robot_open && lvl > gate_on {
                    self.robot_open = true;
                } else if self.robot_open && lvl < gate_off {
                    self.robot_open = false;
                }
                self.synth.drone(self.robot_open, s.robot_note.clamp(12.0, 108.0));
            }
            // The carrier.
            let (mut cl, mut cr) = if use_ext { ext } else { (0.0, 0.0) };
            if use_synth {
                let (a, b) = self.synth.tick();
                cl += a;
                cr += b;
            }
            car_peak = car_peak.max(cl.abs()).max(cr.abs());
            let (gate, hp) = self.voicing.run(voice);
            if gate > 0.0 {
                self.noise_n += 1;
                let k = unv * gate * NOISE_LEVEL;
                if k != 0.0 {
                    cl += k * random_bipolar(NOISE_SEED, self.noise_n);
                    cr += k * random_bipolar(NOISE_SEED ^ 1, self.noise_n);
                }
            } else {
                self.noise_n = 0;
            }
            // Analysis: the voice's band levels.
            self.ana.run(voice, &mut ya);
            if !s.hold {
                for b in 0..nb {
                    let e = ya[b] * ya[b];
                    self.env_m[b].run(e, &self.t_m[b]);
                    if e.to_bits() == 0 {
                        self.env_m[b].settle(ENV_SETTLE);
                    }
                }
            }
            for b in 0..nb {
                self.amp[b] = self.env_m[b].value().sqrt();
            }
            // Synthesis: the carrier's bands, normalised, shaped by the voice.
            self.syn[0].run(cl, &mut yl);
            self.syn[1].run(cr, &mut yr);
            let mut sum = 0.0f32;
            for b in 0..nb {
                let e = (yl[b] * yl[b]).max(yr[b] * yr[b]);
                let v = self.env_c[b].run(e, &self.t_c);
                if e.to_bits() == 0 {
                    self.env_c[b].settle(ENV_SETTLE);
                }
                sum += v;
            }
            let broad = (1.0 / ((sum / nb as f32).sqrt() + FLAT_FLOOR)).min(FLAT_CAP);
            let (mut wl, mut wr) = (0.0f32, 0.0f32);
            for b in 0..nb {
                let band = (1.0 / (self.env_c[b].value().sqrt() + FLAT_FLOOR)).min(FLAT_CAP);
                let k = self.mapped_amp(b) * (broad + (band - broad) * flat) * MAKEUP;
                wl += yl[b] * k * self.gain[0][b];
                wr += yr[b] * k * self.gain[1][b];
            }
            let sib = sib * if s.sib_gated { gate } else { 1.0 } * hp;
            wl += sib + cpass * cl;
            wr += sib + cpass * cr;
            // At mix 0 (gliding out, until the block ends and the unit stops) exactly
            // what the unit gives once it has: the dry, its zeros' signs and all.
            let blend = |dry: f32, wet: f32| {
                if mix == 0.0 {
                    if out == 1.0 {
                        dry
                    } else {
                        dry * out
                    }
                } else {
                    (dry + (wet - dry) * mix) * out
                }
            };
            l[i] = blend(l[i], wl);
            r[i] = blend(r[i], wr);
        }
        // Events after the block's last frame land now.
        for e in &notes[ev..] {
            self.synth.note(e.kind, true);
        }
        car_peak
    }

    fn telemetry_block(&mut self, in_peak: f32, car_peak: f32, l: &[f32], r: &[f32]) {
        self.blocks = (self.blocks + 1) % (1 << 24);
        let tl = &mut self.tel;
        tl[tel::SAMPLE_RATE] = self.sr;
        tl[tel::IN_PEAK] = peak_db(in_peak);
        tl[tel::OUT_PEAK] = peak_db(peak_of(l, r));
        tl[tel::BLOCKS] = self.blocks as f32;
        tl[t::BANDS] = self.n as f32;
        tl[t::ENGAGED] = if self.engaged { 1.0 } else { 0.0 };
        tl[t::GATE] = self.voicing.gate();
        tl[t::UNVOICED] = self.voicing.measure();
        tl[t::VOICES] = self.synth.sounding() as f32;
        tl[t::HOLD] = if self.set.hold { 1.0 } else { 0.0 };
        tl[t::LATENCY] = 0.0;
        tl[t::MOD_DB] = to_db(self.level.value());
        tl[t::CARRIER_DB] = peak_db(car_peak);
        tl[t::FORMANT] = self.gl.formant.value;
        tl[t::SHIFT] = self.gl.shift.value;
        tl[t::ROBOT] = if self.robot_open { 1.0 } else { 0.0 };
    }
}
