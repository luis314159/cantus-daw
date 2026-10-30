//! A voice's control tick (every 16 host samples) and its audio loop.

use super::{Ctx, Voice, AMS, FADE_S};
use crate::math;
use crate::modulus::source::*;
use crate::modulus::tables::wave_level;
use crate::modulus::{db_to_lin, target, OPS, OUT_OFF, PHASE_PER_RAD, VOICE_SCALE};

impl Voice {
    /// Work out the next `span` host samples: advance every envelope and source, evaluate
    /// the modulation, and set the ramps the audio loop follows. `false`: the voice ended.
    pub fn tick(&mut self, span: usize, c: &Ctx) -> bool {
        if self.dying {
            self.active = false;
            return false;
        }
        let (p, g) = (c.p, c.g);
        let dt = span as f32 / c.fs;
        self.elapsed += dt;
        // Per-note expression toward its targets (ADR 0080); untouched when at them.
        let k = crate::expr_glide::coef(span as f32, c.fs);
        for i in 0..3 {
            crate::expr_glide::toward(&mut self.mpe[i], self.mpe_to[i], k);
        }
        if self.pitch != self.pitch_to {
            let step = self.glide_rate * dt;
            let gap = self.pitch_to - self.pitch;
            if !step.is_finite() || gap.abs() <= step {
                self.pitch = self.pitch_to;
            } else {
                self.pitch += step.copysign(gap);
            }
        }
        // Sources.
        let mut src = *c.globals;
        src[ENV1] = self
            .menv
            .advance(p.menv[0], p.menv[1], p.menv[2], p.menv[3], dt);
        let mut lfo = [0f32; 2];
        for (i, v) in lfo.iter_mut().enumerate() {
            let rate = p.lfo_rate[i] * math::exp2(self.lfo_rate_off[i]);
            *v = self.lfo[i].advance(rate, p.lfo_wave[i], p.lfo_delay[i], dt);
        }
        src[LFO1] = lfo[0];
        src[LFO2] = lfo[1];
        src[VEL] = self.vel;
        src[RELVEL] = self.relvel;
        src[KEY] = ((self.key as f32 - 60.0) / 60.0).clamp(-1.0, 1.0);
        src[RND] = self.rnd;
        src[MPE_BEND] = (self.mpe[0] / 48.0).clamp(-1.0, 1.0);
        src[MPE_PRESS] = self.mpe[1];
        src[MPE_SLIDE] = self.mpe[2];
        let mut off = [0f32; target::COUNT];
        if !p.routes.is_empty() {
            p.routes.apply(&src, &mut off);
        }
        self.lfo_rate_off = [off[target::LFO1_RATE], off[target::LFO2_RATE]];
        let vib = lfo[0] * (p.lfo1_pitch + off[target::LFO1_PITCH]) / 100.0;
        let am_db = (p.lfo1_amp + off[target::LFO1_AMP]).max(0.0) * 0.5 * (1.0 - lfo[0]);
        let inv = 1.0 / (span * self.factor) as f32;

        // Operators: frequency, envelope, amplitude.
        let mut carriers_done = true;
        let mut silent = true;
        let mut amps = [0f32; OPS];
        for &o in &g.order[..g.n] {
            let o = o as usize;
            let s = &p.ops[o];
            let hz = self.op_hz(o, c, &off, vib);
            let rate = self.rate;
            let op = &mut self.ops[o];
            op.hz = hz;
            op.inc = (hz as f64 / rate as f64 * 4_294_967_296.0) as i64 as u32;
            op.mip = if s.wave == 0 {
                0
            } else {
                wave_level(hz, rate) as u8
            };
            let (env, done) = if s.dx {
                (op.dx.advance(&s.dxenv, dt), op.dx.done())
            } else {
                (op.time.advance(&s.time, dt), op.time.done(&s.time))
            };
            let mut amp = env * op.gain;
            if off[target::LEVEL + o] != 0.0 {
                amp *= db_to_lin(off[target::LEVEL + o]);
            }
            if s.ams > 0 && am_db > 0.0 {
                amp *= db_to_lin(-am_db * AMS[s.ams.min(3) as usize]);
            }
            if !amp.is_finite() {
                amp = 0.0;
            }
            amps[o] = amp;
            if g.carrier[o] {
                carriers_done &= done;
                silent &= amp == 0.0;
            }
        }
        // A voice whose envelopes are over is let go: at once when it is silent, else
        // through a short fade.
        if self.released && carriers_done {
            self.fade = if silent {
                0.0
            } else {
                (self.fade - dt / FADE_S).max(0.0)
            };
        }
        let gain = db_to_lin(p.gain_db + off[target::GAIN]) * VOICE_SCALE * self.mpe[4]
            / (self.copies as f32).sqrt()
            * self.fade;
        let k = PHASE_PER_RAD
            * math::exp2(2.0 * (p.bright + off[target::BRIGHT]).clamp(-1.0, 1.0))
            * self.tame;
        // A value already at its target stops ramping (and lands on it exactly), so a
        // steady note skips the ramp pass altogether.
        let mut ramping = false;
        let mut step = |now: &mut f32, to: f32| -> f32 {
            if (to - *now).abs() <= 1e-6 * to.abs().max(1.0) {
                *now = to;
                0.0
            } else {
                ramping = true;
                (to - *now) * inv
            }
        };
        for &o in &g.order[..g.n] {
            let o = o as usize;
            let s = &p.ops[o];
            let op = &mut self.ops[o];
            op.damp = (amps[o] - op.amp) * inv;
            if g.carrier[o] {
                let out = s.out_db + off[target::OUT + o];
                let lin = if out <= OUT_OFF { 0.0 } else { db_to_lin(out) } * gain;
                let pan =
                    (s.pan + off[target::PAN + o] + self.uni_pan + self.mpe[3]).clamp(-1.0, 1.0);
                self.dgl[o] = step(&mut self.gl[o], lin * (1.0 - pan).min(1.0));
                self.dgr[o] = step(&mut self.gr[o], lin * (1.0 + pan).min(1.0));
            } else {
                self.dgl[o] = step(&mut self.gl[o], 0.0);
                self.dgr[o] = step(&mut self.gr[o], 0.0);
            }
            for &sv in &g.ins[o][..g.ins_n[o] as usize] {
                let sv = sv as usize;
                let w = (p.m[sv][o] + off[target::M + sv * OPS + o]) * k;
                self.dw[sv][o] = step(&mut self.w[sv][o], w);
            }
            let fb = (s.fb + off[target::FB + o]) * k;
            self.dfbw[o] = step(&mut self.fbw[o], fb);
        }
        self.ramping = ramping;
        if self.fade <= 0.0 {
            self.dying = true;
        }
        for d in &mut self.dec {
            d.flush();
        }
        true
    }

    /// Run the operators for `l.len()` host samples, adding into `l` and `r`.
    pub fn render(&mut self, c: &Ctx, l: &mut [f32], r: &mut [f32]) {
        let (p, g, t) = (c.p, c.g, c.t);
        let n = self.factor;
        let avg = p.fb_avg;
        let order = &g.order[..g.n];
        for (ol, or) in l.iter_mut().zip(r.iter_mut()) {
            let mut bl = [0f32; 8];
            let mut br = [0f32; 8];
            for k in 0..n {
                let p1 = (self.hpos + 16 - n) & 15;
                let p2 = (self.hpos + 16 - 2 * n) & 15;
                let mut sl = 0.0;
                let mut sr = 0.0;
                for &o in order {
                    let o = o as usize;
                    let mut pm = 0.0f32;
                    for &s in &g.ins[o][..g.ins_n[o] as usize] {
                        let s = s as usize;
                        let x = if s > o {
                            self.y[s]
                        } else if avg {
                            0.5 * (self.hist[p1][s] + self.hist[p2][s])
                        } else {
                            self.hist[p1][s]
                        };
                        pm += self.w[s][o] * x;
                    }
                    if g.has_fb[o] {
                        let x = if avg {
                            0.5 * (self.hist[p1][o] + self.hist[p2][o])
                        } else {
                            self.hist[p1][o]
                        };
                        pm += self.fbw[o] * x;
                    }
                    let op = &mut self.ops[o];
                    op.phase = op.phase.wrapping_add(op.inc);
                    let ph = op.phase.wrapping_add(pm as i64 as u32);
                    let wave = p.ops[o].wave;
                    let v = if wave == 0 {
                        t.sin(ph)
                    } else {
                        t.wave(wave as usize - 1, op.mip as usize, ph)
                    };
                    let y = op.amp * v;
                    op.amp += op.damp;
                    self.y[o] = y;
                    sl += y * self.gl[o];
                    sr += y * self.gr[o];
                }
                self.hist[self.hpos] = self.y;
                self.hpos = (self.hpos + 1) & 15;
                for &o in if self.ramping { order } else { &[] } {
                    let o = o as usize;
                    for &s in &g.ins[o][..g.ins_n[o] as usize] {
                        self.w[s as usize][o] += self.dw[s as usize][o];
                    }
                    self.fbw[o] += self.dfbw[o];
                    self.gl[o] += self.dgl[o];
                    self.gr[o] += self.dgr[o];
                }
                bl[k] = sl;
                br[k] = sr;
            }
            if n == 1 {
                *ol += bl[0];
                *or += br[0];
            } else {
                let coefs = &t.halfband;
                *ol += self.dec[0].process(coefs, &mut bl, n);
                *or += self.dec[1].process(coefs, &mut br, n);
            }
        }
    }
}
