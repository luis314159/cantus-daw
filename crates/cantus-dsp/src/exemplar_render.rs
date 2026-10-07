//! Rendering: every sounding layer, sample by sample, with a control step every
//! [`CONTROL`] samples of the layer's own life (pitch, filter, modulation).

use super::*;
use crate::exemplar::voice::{lfo_value, Stage, CONTROL};
use crate::math;

impl Exemplar {
    /// Render into `left`/`right` (added). `false` only when nothing sounds, so the
    /// buffers are untouched zeros.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        let n = left.len().min(right.len());
        let (left, right) = (&mut left[..n], &mut right[..n]);
        let mut any = false;
        for li in 0..MAX_LAYERS {
            if !self.layers[li].active {
                continue;
            }
            any = true;
            if self.render_layer(li, left, right) {
                self.layers[li].active = false;
                let slot = self.layers[li].slot;
                self.layer_ended(slot);
            }
        }
        self.clock += n as u64;
        if any {
            let p = left
                .iter()
                .chain(right.iter())
                .fold(0f32, |m, x| m.max(x.abs()));
            self.peak = self.peak.max(p);
        }
        any
    }

    /// Returns whether the layer has ended.
    fn render_layer(&mut self, li: usize, left: &mut [f32], right: &mut [f32]) -> bool {
        let n = left.len();
        let mut i = 0;
        while i < n {
            if self.layers[li].ctl == 0 {
                self.control(li);
            }
            let Exemplar {
                layers,
                readers,
                coefs,
                globals,
                ..
            } = self;
            let reader = &readers[globals.quality as usize];
            let l = &mut layers[li];
            let run = (l.ctl as usize).min(n - i);
            for t in i..i + run {
                if l.env.stage == Stage::Delay {
                    l.env.next();
                    l.env2.next();
                    continue;
                }
                let e = l.env.next();
                l.env2.next();
                let x = l.play.next(l.ratio, reader, coefs);
                let (mut a, mut b) = (x[0], x[1]);
                if l.filter.mode != 0 {
                    a = l.filter.process(a, 0);
                    b = l.filter.process(b, 1);
                }
                l.gmod += l.gstep;
                let mut g = e * l.gmod;
                if l.fade_step != 0.0 {
                    l.fade += l.fade_step;
                    if l.fade <= 0.0 {
                        return true;
                    }
                    g *= l.fade;
                }
                left[t] += a * g * l.gain[0];
                right[t] += b * g * l.gain[1];
                if l.env.stage == Stage::Off || l.play.is_done() {
                    return true;
                }
            }
            l.ctl -= run as u32;
            l.age += run as u64;
            i += run;
        }
        false
    }

    /// One control step for a layer: modulation, pitch, filter, levels.
    fn control(&mut self, li: usize) {
        let fs = self.fs;
        let g = &self.globals;
        let l = &self.layers[li];
        let mut src = [0.0f32; SOURCES.len()];
        src[0] = l.env.level;
        src[1] = l.env2.level;
        src[2] = lfo_value(g.lfo_shape[0], l.lfo[0]);
        src[3] = lfo_value(g.lfo_shape[1], l.lfo[1]);
        src[4] = l.vel;
        src[5] = l.relvel;
        src[6] = (l.key as f32 - 60.0) / 60.0;
        src[7] = l.rnd;
        src[SRC_GLOBAL..SRC_GLOBAL + 8].copy_from_slice(&g.macros);
        src[16] = self.mw;
        src[17] = self.at;
        src[18] = (self.bend / 2.0).clamp(-1.0, 1.0);
        src[19] = self.expr;
        src[20] = self.sus;
        let mut off = [0.0f32; TARGETS.len()];
        if !self.matrix.is_empty() {
            self.matrix.apply(&src, &mut off);
            for (o, t) in off.iter_mut().zip(TARGETS) {
                *o *= t.1;
            }
        }
        let rate1 = g.lfo_rate[0]
            * if off[7] != 0.0 {
                math::exp2(off[7])
            } else {
                1.0
            };
        let rate2 = g.lfo_rate[1]
            * if off[8] != 0.0 {
                math::exp2(off[8])
            } else {
                1.0
            };
        let delay = g.lfo1_delay_ms * fs / 1000.0;
        let fade_in = if delay > 0.0 {
            (l.age as f32 / delay).min(1.0)
        } else {
            1.0
        };
        let depth = g.vibrato_ct + off[6];
        let vib = if depth != 0.0 {
            depth * src[2] * fade_in
        } else {
            0.0
        };
        let bend = self.bend * g.bend_range / 2.0;
        let semis = l.semis + l.glide + (bend + off[3] + (off[2] + vib) / 100.0) as f64;
        let ratio = if semis == 0.0 {
            l.rate_ratio
        } else {
            l.rate_ratio * math::exp2(semis / 12.0)
        };
        let cutoff = if l.filter.mode != 0 {
            let oct = (l.key as f32 - 60.0) / 12.0 * g.filter_key
                + l.vel * g.filter_vel / 12.0
                + l.env2.level * g.filter_env / 12.0
                + off[4];
            Some((g.cutoff * math::exp2(oct), (g.reso + off[5]) / 100.0))
        } else {
            None
        };
        let gmod = if off[0] != 0.0 { db(off[0]) } else { 1.0 };
        let bal = balance(l.pan + off[1] / 100.0);
        let quality = g.quality as usize;
        let Exemplar {
            layers, readers, ..
        } = self;
        let l = &mut layers[li];
        if let Some((fc, res)) = cutoff {
            l.filter.set(fc, res, fs);
        }
        if ratio != l.ratio {
            l.ratio = ratio;
            l.play.retune(ratio, &readers[quality]);
        }
        l.gain = [l.amp * bal[0], l.amp * bal[1]];
        if l.first {
            l.gmod = gmod;
            l.gstep = 0.0;
            l.first = false;
        } else {
            l.gstep = (gmod - l.gmod) / CONTROL as f32;
        }
        let step = CONTROL as f64 / fs as f64;
        l.lfo[0] = (l.lfo[0] + rate1 as f64 * step).fract();
        l.lfo[1] = (l.lfo[1] + rate2 as f64 * step).fract();
        if l.glide_step != 0.0 {
            l.glide += l.glide_step;
            if (l.glide > 0.0) == (l.glide_step > 0.0) || l.glide == 0.0 {
                l.glide = 0.0;
                l.glide_step = 0.0;
            }
        }
        l.ctl = CONTROL;
    }
}
