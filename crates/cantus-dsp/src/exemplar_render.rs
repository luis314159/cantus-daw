//! Rendering: every sounding layer, sample by sample, with a control step every
//! [`CONTROL`] samples of the layer's own life (pitch, filter, modulation) — on the caller,
//! or (ADR 0038) each layer on whichever thread of the pool takes it, into its own lane,
//! the lanes summed in layer order.

use super::*;
use crate::exemplar::resample::Reader;
use crate::exemplar::voice::{lfo_value, Stage, CONTROL};
use crate::math;
use crate::par::{add_into, clear_lane, run_each, Parallel, Serial};

use super::{LANES, SPAN};

/// What a layer reads of the instrument while it renders: nothing it writes.
struct Ctx<'a> {
    fs: f32,
    globals: &'a Globals,
    matrix: &'a ModMatrix,
    readers: &'a [Reader; 4],
    mw: f32,
    at: f32,
    bend: f32,
    expr: f32,
    sus: f32,
}

impl Exemplar {
    /// Render into `left`/`right` (added). `false` only when nothing sounds, so the
    /// buffers are untouched zeros.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        self.process_par(left, right, &Serial)
    }

    /// [`Self::process`] with the layers handed to `par` when it has helpers and two or
    /// more layers sound (at most [`LANES`]). The same bits as the serial render: each
    /// layer's samples are added in layer order either way, and the allocator hears of the
    /// layers that ended in layer order.
    pub fn process_par(&mut self, left: &mut [f32], right: &mut [f32], par: &dyn Parallel) -> bool {
        let n = left.len().min(right.len());
        let (left, right) = (&mut left[..n], &mut right[..n]);
        let active = self.layers.iter().filter(|l| l.active).count();
        let any = active > 0;
        if par.width() > 1 && (2..=LANES).contains(&active) {
            let mut i = 0;
            while i < n {
                let m = (n - i).min(SPAN);
                self.span_par(&mut left[i..i + m], &mut right[i..i + m], par);
                i += m;
            }
            for li in 0..MAX_LAYERS {
                if std::mem::take(&mut self.ended[li]) {
                    let slot = self.layers[li].slot;
                    self.layer_ended(slot);
                }
            }
        } else {
            for li in 0..MAX_LAYERS {
                if !self.layers[li].active {
                    continue;
                }
                if self.render_layer(li, left, right) {
                    self.layers[li].active = false;
                    let slot = self.layers[li].slot;
                    self.layer_ended(slot);
                }
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

    /// One span of the parallel render: every sounding layer into its lane (the k-th
    /// sounding layer into lane k), then the lanes into the output in layer order.
    fn span_par(&mut self, left: &mut [f32], right: &mut [f32], par: &dyn Parallel) {
        let m = left.len();
        let Exemplar {
            layers,
            readers,
            globals,
            matrix,
            lanes,
            lane_end,
            ..
        } = self;
        let cx = Ctx {
            fs: self.fs,
            globals,
            matrix,
            readers,
            mw: self.mw,
            at: self.at,
            bend: self.bend,
            expr: self.expr,
            sus: self.sus,
        };
        let sounding = layers.iter_mut().enumerate().filter(|(_, l)| l.active);
        let jobs = sounding.zip(lanes.split(m).zip(lane_end.iter_mut()));
        run_each(par, jobs, &|((li, l), ((ll, lr), end))| {
            clear_lane(ll, lr);
            *end = if render_into(&cx, l, ll, lr) {
                li as u16
            } else {
                u16::MAX
            };
        });
        let mut k = 0;
        for li in 0..MAX_LAYERS {
            if !self.layers[li].active {
                continue;
            }
            let (ll, lr) = self.lanes.lane(k, m);
            add_into(left, ll);
            add_into(right, lr);
            if self.lane_end[k] != u16::MAX {
                self.layers[li].active = false;
                self.ended[li] = true;
            }
            k += 1;
        }
    }

    /// Returns whether the layer has ended.
    fn render_layer(&mut self, li: usize, left: &mut [f32], right: &mut [f32]) -> bool {
        let Exemplar {
            layers,
            readers,
            globals,
            matrix,
            ..
        } = self;
        let cx = Ctx {
            fs: self.fs,
            globals,
            matrix,
            readers,
            mw: self.mw,
            at: self.at,
            bend: self.bend,
            expr: self.expr,
            sus: self.sus,
        };
        render_into(&cx, &mut layers[li], left, right)
    }
}

/// Render one layer added into `left`/`right`; whether it has ended.
fn render_into(cx: &Ctx, l: &mut Layer, left: &mut [f32], right: &mut [f32]) -> bool {
    // The resampler's scratch (written before it is read), one per thread.
    let mut coefs = [0.0f32; resample::MAX_TAPS];
    let n = left.len();
    let mut i = 0;
    while i < n {
        if l.ctl == 0 {
            control(cx, l);
        }
        let (readers, globals) = (cx.readers, cx.globals);
        let reader = &readers[globals.quality as usize];
        let run = (l.ctl as usize).min(n - i);
        for t in i..i + run {
            if l.env.stage == Stage::Delay {
                l.env.next();
                l.env2.next();
                continue;
            }
            let e = l.env.next();
            l.env2.next();
            let x = l.play.next(l.ratio, reader, &mut coefs);
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
fn control(cx: &Ctx, l: &mut Layer) {
    let fs = cx.fs;
    let g = cx.globals;
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
    src[16] = cx.mw;
    src[17] = cx.at;
    src[18] = (cx.bend / 2.0).clamp(-1.0, 1.0);
    src[19] = cx.expr;
    src[20] = cx.sus;
    let mut off = [0.0f32; TARGETS.len()];
    if !cx.matrix.is_empty() {
        cx.matrix.apply(&src, &mut off);
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
    let bend = cx.bend * g.bend_range / 2.0;
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
    let readers = cx.readers;
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
