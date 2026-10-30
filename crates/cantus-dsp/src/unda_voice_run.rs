//! A voice at work: the control tick (sources → matrix → values → oscillators, filters,
//! envelopes) and the per-sample render at the oversampled rate.

use super::super::grana::{GranaControl, Source as GrainSource};
use super::super::modsrc::{self, unit, LfoMode};
use super::super::osc::{OscControl, WarpSlot};
use super::super::partes::PartesControl;
use super::super::{factory, source, target, Engine, Route, Settings};
use super::{db, env_settings, table_of, Globals, Voice, VOICE_GAIN};
use crate::math;

impl Voice {
    /// Evaluate the modulation and set everything for the next [`CONTROL`] samples.
    /// `first`: the note's first tick, so values land at once rather than ramp.
    ///
    /// [`CONTROL`]: super::super::CONTROL
    #[allow(clippy::needless_range_loop)] // an oscillator's index reaches six arrays
    pub fn control(&mut self, s: &Settings, g: &Globals, first: bool) {
        let base_sr = g.sr;
        let q = g.oversample.max(1);
        let sr = base_sr * q as f32;
        let elapsed = (self.since - self.last_tick) as f32;
        self.last_tick = self.since;
        // Per-note expression toward its targets (ADR 0080); untouched when at them.
        let k = if first {
            1.0
        } else {
            crate::expr_glide::coef(elapsed, base_sr)
        };
        crate::expr_glide::toward(&mut self.tuning, self.expr_to[0], k);
        crate::expr_glide::toward(&mut self.pressure, self.expr_to[1], k);
        crate::expr_glide::toward(&mut self.slide, self.expr_to[2], k);
        // ---- sources ----
        self.src = *g.src;
        for e in 0..4 {
            self.src[source::ENV + e] = self.env[e].level();
        }
        self.src[source::VEL] = self.vel;
        self.src[source::RELVEL] = self.relvel;
        self.src[source::KEY] = ((self.pitch - 60.0) / 64.0).clamp(-1.0, 1.0);
        self.src[source::RND] = self.rnd_value;
        self.src[source::MPE_BEND] = (self.tuning / 48.0).clamp(-1.0, 1.0);
        self.src[source::MPE_PRESS] = self.pressure;
        self.src[source::MPE_SLIDE] = self.slide;
        let t_note = self.since as f32 / base_sr;
        for l in 0..8 {
            if !s.used_sources[source::LFO + l] {
                continue;
            }
            let ls = &s.lfo[l];
            let raw = if ls.mode == LfoMode::Free {
                g.free_lfo[l]
            } else {
                let st = &mut self.lfo[l];
                let rate = self.val[target::LFO_RATE + l].max(0.0);
                if !st.done {
                    st.phase += rate * elapsed / base_sr;
                    while st.phase >= 1.0 {
                        if ls.mode == LfoMode::Envelope {
                            st.phase = 0.999_999;
                            st.done = true;
                            break;
                        }
                        st.phase -= 1.0;
                        st.cycle += 1;
                        st.r0 = st.r1;
                        st.r1 = 2.0 * unit(&[self.seed, 6, l as u64, st.cycle + 1]) - 1.0;
                    }
                }
                modsrc::wave(ls.wave, st.phase, st.r0, st.r1, &ls.shape)
            };
            let st = &mut self.lfo[l];
            st.y = if ls.smooth > 0.0 && !first {
                let tau = 0.25 * ls.smooth;
                let a = 1.0 - math::exp(-elapsed / (base_sr * tau));
                st.y + (raw - st.y) * a
            } else {
                raw
            };
            let fade = if t_note < ls.delay {
                0.0
            } else if ls.rise > 0.0 {
                ((t_note - ls.delay) / ls.rise).min(1.0)
            } else {
                1.0
            };
            self.src[source::LFO + l] = st.y * fade;
        }
        // ---- values ----
        self.val = s.base;
        if !s.routes.is_empty() {
            let mut off = [0.0f32; target::COUNT];
            s.routes.apply(&self.src, &mut off);
            for (t, o) in off.iter().enumerate() {
                if *o != 0.0 {
                    self.val[t] = s.range[t].apply(s.base[t], *o);
                }
            }
        }
        let v = &self.val;
        // ---- oscillators ----
        let pitch = self.pitch + self.tuning + g.src[source::PB] * s.bend_st + self.drift_ct * 0.01;
        let nyq = if q > 1 { 0.5 * sr } else { 0.5 * base_sr };
        let steps = (super::super::CONTROL * q) as u32;
        let mut uses = [false; 5];
        self.sr_int = sr;
        // An oscillator can sound when it has something to play: a table, or (Grana) a
        // sample.
        let playable = |os: &super::super::OscSettings| {
            table_of(&os.table).is_some() || (os.engine == Engine::Grana && os.sample.is_some())
        };
        for o in 0..3 {
            let os = &s.osc[o];
            self.hear[o] = os.on && playable(os);
            if self.hear[o] && os.engine == Engine::Wavetable {
                for k in 0..2 {
                    let amt = v[target::OSC + o * target::OSC_N + target::WARP1 + k];
                    if os.warp[k].needs_source() && amt > 0.0 {
                        uses[os.warp_src[k].min(4)] = true;
                    }
                }
            }
        }
        for o in 0..3 {
            let os = &s.osc[o];
            self.run[o] = (self.hear[o] || uses[o]) && playable(os);
            if !self.run[o] {
                continue;
            }
            let b = target::OSC + o * target::OSC_N;
            let semis =
                pitch + os.oct * 12.0 + os.semi + v[b + target::PITCH] + 0.01 * v[b + target::FINE];
            let hz = 440.0 * math::exp2((semis - 69.0) / 12.0);
            let gain = if self.hear[o] {
                db(v[b + target::LEVEL])
            } else {
                0.0
            };
            match os.engine {
                Engine::Partes => {
                    if let Some(table) = table_of(&os.table) {
                        let pc = PartesControl {
                            hz,
                            gain,
                            pan: v[b + target::PAN],
                            pos: v[b + target::POS],
                            partials: os.partials,
                            tilt_db: v[b + target::TILT],
                            oddeven: v[b + target::ODDEVEN],
                            stretch: v[b + target::STRETCH],
                        };
                        self.partes[o].control(&pc, table, sr, steps, first);
                    }
                    continue;
                }
                Engine::Grana => {
                    self.grana[o].control(&GranaControl {
                        hz,
                        gain,
                        pan: v[b + target::PAN],
                        pos: v[b + target::POS],
                        density: v[b + target::DENSITY],
                        length: v[b + target::GLEN],
                        spray: v[b + target::SPRAY],
                        pitch_rand: os.pitch_rand,
                        reverse: os.reverse,
                        width: v[b + target::WIDTH],
                        window: os.window,
                    });
                    continue;
                }
                Engine::Wavetable => {}
            }
            let c = OscControl {
                hz,
                gain,
                pan: v[b + target::PAN],
                pos: v[b + target::POS],
                lanes: os.unison,
                detune_ct: v[b + target::DETUNE],
                curve: os.curve,
                blend: v[b + target::BLEND],
                width: v[b + target::WIDTH],
                stack: os.stack,
                spread: v[b + target::SPREAD],
                warps: [
                    WarpSlot {
                        mode: os.warp[0],
                        amt: v[b + target::WARP1],
                        src: os.warp_src[0],
                    },
                    WarpSlot {
                        mode: os.warp[1],
                        amt: v[b + target::WARP2],
                        src: os.warp_src[1],
                    },
                ],
            };
            if let Some(table) = table_of(&os.table) {
                self.osc[o].control(&c, table, sr, nyq, steps, &self.rnd[o], first);
            }
        }
        self.sub_run = s.sub_on || uses[3];
        if self.sub_run {
            let semis = pitch + 12.0 * s.sub_oct;
            let c = OscControl {
                hz: 440.0 * math::exp2((semis - 69.0) / 12.0),
                gain: if s.sub_on {
                    db(v[target::SUB_LEVEL])
                } else {
                    0.0
                },
                pan: v[target::SUB_PAN],
                pos: s.sub_wave as f32 / 4.0,
                lanes: 1,
                ..OscControl::default()
            };
            self.sub.control(
                &c,
                factory::sub_table(),
                sr,
                nyq,
                steps,
                &self.rnd[0],
                first,
            );
        }
        self.noise_run = s.noise_on || uses[4];
        let ng = if s.noise_on {
            db(v[target::NOISE_LEVEL])
        } else {
            0.0
        };
        let th = (v[target::NOISE_PAN] + 1.0) * std::f32::consts::FRAC_PI_4;
        self.noise_gain = [ng * math::cos(th) * 1.414, ng * math::sin(th) * 1.414];
        self.noise_color = v[target::NOISE_COLOR];
        self.noise_width = v[target::NOISE_WIDTH];
        // ---- filters ----
        for f in 0..2 {
            self.flt_on[f] = s.filters[f].on;
            if !self.flt_on[f] {
                continue;
            }
            let b = target::FLT + f * target::FLT_N;
            let cut = v[b + target::CUTOFF]
                * math::exp2(v[b + target::KEY] * (pitch - 60.0) / 12.0 + self.drift_oct);
            let st = v[b + target::STEREO] * 0.5;
            let cuts = [cut * math::exp2(-st), cut * math::exp2(st)];
            self.filt[f].set(
                sr,
                s.filters[f].ty,
                cuts,
                v[b + target::RES],
                v[b + target::DRIVE],
            );
            self.mix[f] = v[b + target::MIX];
        }
        self.routing = v[target::ROUTING];
        // ---- envelopes and the amplifier ----
        for e in 0..4 {
            self.env[e].update(&env_settings(s, v, e));
        }
        let vel = 1.0 - s.vel_depth + s.vel_depth * self.vel * self.vel.sqrt();
        self.amp_to = VOICE_GAIN * vel * self.volume * db(v[target::GAIN]);
        if first {
            self.amp = self.amp_to * self.env[0].level();
        }
    }

    /// Render `len` base samples at `q`× into `l`/`r` (added). Returns whether the voice
    /// is still sounding afterwards.
    #[allow(clippy::needless_range_loop)] // an oscillator's index reaches five arrays
    pub fn render(
        &mut self,
        s: &Settings,
        q: usize,
        len: usize,
        l: &mut [f32],
        r: &mut [f32],
    ) -> bool {
        let tables = [
            table_of(&s.osc[0].table),
            table_of(&s.osc[1].table),
            table_of(&s.osc[2].table),
        ];
        let sub_t = factory::sub_table();
        let inv_q = 1.0 / q as f32;
        let pan = self.pan;
        let (pl, pr) = if pan == 0.0 {
            (1.0, 1.0)
        } else {
            let th = (pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
            (math::cos(th) * 1.414, math::sin(th) * 1.414)
        };
        for j in 0..len {
            let e = self.env[0].next();
            for env in &mut self.env[1..] {
                env.next();
            }
            if self.glide != 0.0 {
                self.pitch += self.glide;
                if (self.glide > 0.0 && self.pitch >= self.target)
                    || (self.glide < 0.0 && self.pitch <= self.target)
                {
                    self.pitch = self.target;
                    self.glide = 0.0;
                }
            }
            let a0 = self.amp;
            let a1 = self.amp_to * e;
            self.amp = a1;
            self.since += 1;
            for k in 0..q {
                let a = a0 + (a1 - a0) * (k + 1) as f32 * inv_q;
                let (nl, nr) = if self.noise_run {
                    self.noise.next(self.noise_color, self.noise_width)
                } else {
                    (0.0, 0.0)
                };
                let (sl, sr) = if self.sub_run {
                    self.sub.tick(sub_t, [0.0; 2])
                } else {
                    (0.0, 0.0)
                };
                let mut bus = [[0.0f32; 2]; 3];
                let mut add = |route: Route, x: (f32, f32)| {
                    let i = match route {
                        Route::F1 => 0,
                        Route::F2 => 1,
                        Route::Direct => 2,
                        Route::Both => {
                            bus[0][0] += x.0;
                            bus[0][1] += x.1;
                            1
                        }
                    };
                    bus[i][0] += x.0;
                    bus[i][1] += x.1;
                };
                if s.sub_on {
                    add(s.sub_route, (sl, sr));
                }
                if s.noise_on {
                    add(
                        s.noise_route,
                        (nl * self.noise_gain[0], nr * self.noise_gain[1]),
                    );
                }
                for o in 0..3 {
                    if !self.run[o] {
                        continue;
                    }
                    let os = &s.osc[o];
                    let y = match os.engine {
                        Engine::Partes => {
                            let y = self.partes[o].tick();
                            self.last_out[o] = self.partes[o].out0;
                            y
                        }
                        Engine::Grana => {
                            let src = match (&os.sample, tables[o]) {
                                (Some(smp), _) => GrainSource::Sample {
                                    frames: &smp.frames,
                                    channels: smp.channels,
                                    rate: smp.rate,
                                },
                                (None, Some(t)) => GrainSource::Table(t),
                                _ => continue,
                            };
                            let y = self.grana[o].tick(&src, self.sr_int);
                            self.last_out[o] = self.grana[o].out0;
                            y
                        }
                        Engine::Wavetable => {
                            let Some(t) = tables[o] else { continue };
                            let (outs, sub0) = (self.last_out, self.sub.out0);
                            let pick = |src: Option<usize>| match src {
                                Some(i @ 0..=2) => outs[i],
                                Some(3) => sub0,
                                Some(4) => nl,
                                _ => 0.0,
                            };
                            let srcs = self.osc[o].sources();
                            let y = self.osc[o].tick(t, [pick(srcs[0]), pick(srcs[1])]);
                            self.last_out[o] = self.osc[o].out0;
                            y
                        }
                    };
                    if self.hear[o] {
                        add(os.route, y);
                    }
                }
                // Filters: F1 feeds F2 by (1 − routing); F1's output also goes out by
                // routing. An off filter is a wire.
                let f1in = bus[0];
                let f1 = if self.flt_on[0] {
                    let y = self.filt[0].process(f1in);
                    let m = self.mix[0];
                    [
                        f1in[0] + (y[0] - f1in[0]) * m,
                        f1in[1] + (y[1] - f1in[1]) * m,
                    ]
                } else {
                    f1in
                };
                let rt = self.routing;
                let f2in = [
                    bus[1][0] + (1.0 - rt) * f1[0],
                    bus[1][1] + (1.0 - rt) * f1[1],
                ];
                let f2 = if self.flt_on[1] {
                    let y = self.filt[1].process(f2in);
                    let m = self.mix[1];
                    [
                        f2in[0] + (y[0] - f2in[0]) * m,
                        f2in[1] + (y[1] - f2in[1]) * m,
                    ]
                } else {
                    f2in
                };
                let idx = j * q + k;
                l[idx] += (f2[0] + rt * f1[0] + bus[2][0]) * a * pl;
                r[idx] += (f2[1] + rt * f1[1] + bus[2][1]) * a * pr;
            }
            if self.env[0].is_idle() {
                self.active = false;
                return false;
            }
        }
        true
    }
}
