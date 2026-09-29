//! A voice's control update and its per-sample render (a child of `calor::voice::run`).

use super::super::*;

/// The mixer's bandwidth when oversampled: what is above it would only fold back from the
/// filter's saturation.
const BANDWIDTH_HZ: f32 = 22_000.0;

impl Voice {
    /// Work out the controls for the next [`CONTROL`] base samples and set their ramps.
    /// `clock`: the stream position (base samples) now.
    fn tick(&mut self, sh: &Shared, clock: f64) {
        let s = sh.s;
        let n = (CONTROL * sh.factor) as f32;
        let tick_s = CONTROL as f32 / sh.fs;
        let t_note = self.age as f64 / sh.fs as f64;
        let trims = sh.trims[self.card];
        // Glide, a tick at a time.
        if self.glide_coef > 0.0 {
            self.pitch += (self.target - self.pitch) * self.glide_coef;
            if (self.target - self.pitch).abs() < 1e-4 {
                self.pitch = self.target;
                self.glide_coef = 0.0;
            }
        } else if self.glide_step > 0.0 {
            let d = self.target - self.pitch;
            if d.abs() <= self.glide_step {
                self.pitch = self.target;
                self.glide_step = 0.0;
            } else {
                self.pitch += self.glide_step * d.signum();
            }
        }
        // The LFOs (their rate routes are the last tick's).
        let clock_s = clock / sh.fs as f64;
        let r1 = s.lfo[0].rate * exp2(self.mods[Target::Lfo1Rate as usize]);
        let r2 = s.lfo[1].rate * exp2(self.mods[Target::Lfo2Rate as usize]);
        let l1 = self.lfo[0].tick(&s.lfo[0], r1, clock_s, t_note, tick_s, self.seed);
        let l2 = self.lfo[1].tick(&s.lfo[1], r2, clock_s, t_note, tick_s, self.seed ^ 2);
        // What the wheel moves: LFO 1, noise, or a blend.
        let wmod = if s.wheel_mix > 0.0 {
            let k = self.age / CONTROL as u64;
            l1 + (bipolar(hash3(self.seed, 0x3ee1, k)) - l1) * s.wheel_mix
        } else {
            l1
        };
        if !sh.matrix.is_empty() {
            let mut sources = *sh.globals;
            sources[src::ENV1] = self.env[0].level * self.env_level[0];
            sources[src::ENV2] = self.env[1].level * self.env_level[1];
            sources[src::LFO1] = l1;
            sources[src::LFO2] = l2;
            sources[src::VEL] = self.vel;
            sources[src::RELVEL] = self.rel_vel;
            sources[src::KEY] = ((self.key as f32 - 60.0) / 60.0).clamp(-1.0, 1.0);
            sources[src::RND] = self.rnd;
            sources[src::MPE_BEND] = (self.x[x::TUNING] / 48.0).clamp(-1.0, 1.0);
            sources[src::MPE_PRESS] = self.x[x::PRESSURE];
            sources[src::MPE_SLIDE] = self.x[x::BRIGHTNESS];
            self.mods = [0.0; TARGETS];
            sh.matrix.apply(&sources, &mut self.mods);
            if sh.routes_env {
                self.env_coefs(sh);
            }
        }
        let m = self.mods;
        let t = |target: Target| m[target as usize];
        let cs = s.condition_scale();
        // Pitch.
        let vib = l1 * (s.lfo1_pitch + t(Target::Vibrato))
            + wmod * s.wheel_vibrato * (sh.wheel + self.x[x::VIBRATO]);
        let stack_cents = self.stack_pos * s.stack_detune * 0.5;
        let played = self.pitch + s.transpose + self.x[x::TUNING];
        let cents = s.fine + t(Target::Pitch) + vib + stack_cents;
        let common = played + sh.bend + cents / 100.0;
        let track_err = trims.tracking_cents * (self.key as f32 - 60.0) / 12.0;
        let mut drift_c = [0.0f32; 3];
        if s.drift > 0.0 && cs > 0.0 {
            for (i, d) in drift_c.iter_mut().enumerate() {
                let seed = trims.seed ^ self.seed ^ (i as u64 + 1).wrapping_mul(0x9e37_79b9);
                *d = drift(seed, t_note, s.drift_rate) * s.drift * cs;
            }
        }
        const PITCH: [Target; 3] = [Target::Osc1Pitch, Target::Osc2Pitch, Target::Osc3Pitch];
        const PW: [Target; 3] = [Target::Pw1, Target::Pw2, Target::Pw3];
        let mut dt_new = [0f64; 3];
        let mut pw_new = [0f32; 3];
        let mut vintage = false;
        for i in 0..3 {
            let o = &s.osc[i];
            let note = if i == 2 && !s.osc3_kbd {
                60.0 + s.transpose + sh.bend + cents / 100.0
            } else {
                common
            };
            let low = if i == 2 && s.osc3_low { 84.0 } else { 0.0 };
            let c = o.fine + trims.osc_cents[i] + drift_c[i] + track_err + t(PITCH[i]);
            let p = note + o.octave * 12.0 + o.semi - low + c / 100.0;
            dt_new[i] = (midi_hz(p) as f64 / sh.fs_os as f64).min(0.45);
            pw_new[i] = (o.pw + trims.pw + l2 * o.pwm * 0.45 + wmod * s.wheel_pw * 0.45 + t(PW[i]))
                .clamp(0.02, 0.98);
            let (curve, round) = o.rev.shape();
            vintage |= o.rev != OscRev::Ideal;
            self.shape[i] = Shape {
                saw: o.saw,
                pulse: o.pulse,
                tri: o.tri,
                pw: pw_new[i],
                curve,
                round,
            };
        }
        // Mixer.
        const MIX: [Target; 5] = [
            Target::Mix1,
            Target::Mix2,
            Target::Mix3,
            Target::MixSub,
            Target::MixNoise,
        ];
        let mut mix_new = [0f32; 5];
        for j in 0..5 {
            mix_new[j] = (s.mix[j] + t(MIX[j])).clamp(0.0, 1.5);
        }
        self.feedback = (s.feedback + t(Target::Feedback)).clamp(0.0, 1.0);
        self.drive = s.mix_drive * db_gain(t(Target::MixDrive));
        // The filter.
        let track = self.pitch + s.transpose + self.x[x::TUNING] - 60.0;
        let cut_new = sh.cutoff_log2
            + s.key_track * track / 12.0
            + trims.cutoff_st / 12.0
            + drift_c[0] / 2400.0
            + l2 * s.lfo_amount
            + wmod * s.wheel_vcf
            + t(Target::Cutoff);
        self.env_amt = (s.env_amount + t(Target::EnvAmount))
            * (1.0 - s.env_velocity + s.env_velocity * self.vel);
        self.fm = (s.filter_fm + t(Target::FilterFm)).clamp(0.0, 1.0);
        self.coefs = if sh.routes_shape_filter {
            FilterCoefs::new(&FilterSettings {
                reso: (s.filter.reso + t(Target::Reso)).clamp(0.0, 1.25),
                drive: s.filter.drive * db_gain(t(Target::VcfDrive)),
                morph: (s.filter.morph + t(Target::Morph)).clamp(0.0, 1.0),
                bass_comp: (s.filter.bass_comp + t(Target::BassComp)).clamp(0.0, 1.0),
                ..s.filter
            })
        } else {
            sh.coefs
        };
        let hpf_on = s.hpf > 10.5 || t(Target::Hpf) != 0.0;
        self.g_hpf = if hpf_on {
            let hz = (s.hpf * exp2(t(Target::Hpf))).clamp(5.0, 0.45 * sh.fs_os);
            OnePole::coef(tan_pi(hz / sh.fs_os))
        } else {
            0.0
        };
        self.g_shelf = OnePole::coef(tan_pi(150.0 / sh.fs_os));
        self.bw_on = sh.factor > 1;
        if self.bw_on {
            self.bw.set(tan_pi(BANDWIDTH_HZ / sh.fs_os));
        }
        self.g_ac = if vintage {
            OnePole::coef(tan_pi(8.0 / sh.fs_os))
        } else {
            0.0
        };
        // Amplifier, pan.
        self.vca_drive = (s.vca_drive + t(Target::VcaDrive)).clamp(0.0, 1.0);
        self.vel_gain = 1.0 - s.vca_velocity + s.vca_velocity * self.vel;
        self.xmod = (s.xmod + t(Target::Xmod)).clamp(0.0, 1.0);
        self.pm_env = (s.polymod_env + t(Target::PolyEnv)).clamp(-1.0, 1.0);
        self.pm_osc3 = (s.polymod_osc3 + t(Target::PolyOsc3)).clamp(0.0, 1.0);
        let pan =
            (s.pan + trims.pan + self.stack_pos * s.stack_spread + self.x[x::PAN] + t(Target::Pan))
                .clamp(-1.0, 1.0);
        let gain = VOICE_SCALE / (s.stack.max(1) as f32).sqrt()
            * db_gain(t(Target::Gain))
            * self.x[x::VOLUME]
            * self.x[x::EXPRESSION]
            * self.vel_gain;
        let theta = (pan + 1.0) * 0.125;
        let (gl_new, gr_new) = (sin_turns(theta + 0.25) * gain, sin_turns(theta) * gain);
        // Ramps to where the controls are going.
        if self.first_tick {
            self.first_tick = false;
            self.dt = dt_new;
            self.cut = cut_new;
            self.pw = pw_new;
            self.mix = mix_new;
            self.gl = gl_new;
            self.gr = gr_new;
            self.ddt = [0.0; 3];
            self.dpw = [0.0; 3];
            self.dmix = [0.0; 5];
            self.dcut = 0.0;
            self.dgl = 0.0;
            self.dgr = 0.0;
        } else {
            let inv = 1.0 / n;
            for i in 0..3 {
                self.ddt[i] = (dt_new[i] - self.dt[i]) / n as f64;
                self.dpw[i] = (pw_new[i] - self.pw[i]) * inv;
            }
            for ((d, new), now) in self.dmix.iter_mut().zip(mix_new).zip(self.mix) {
                *d = (new - now) * inv;
            }
            self.dcut = (cut_new - self.cut) * inv;
            self.dgl = (gl_new - self.gl) * inv;
            self.dgr = (gr_new - self.gr) * inv;
        }
        self.cut_hz = exp2(cut_new + self.env[0].level * self.env_level[0] * self.env_amt);
        self.age += CONTROL as u64;
    }

    /// Render oversampled samples added into `l` and `r`. Returns whether the voice still
    /// sounds.
    pub fn render(&mut self, sh: &Shared, l: &mut [f32], r: &mut [f32]) -> bool {
        let s = sh.s;
        let fs_os = sh.fs_os;
        let max_hz = 0.45 * fs_os;
        let inv_fs = 1.0 / fs_os;
        let (sync2, sync3) = (s.osc[1].sync, s.osc[2].sync);
        let amp_gate = s.vca_gate;
        for i in 0..l.len() {
            if self.ctl_left == 0 {
                self.tick(sh, sh.clock as f64 + (i / sh.factor) as f64);
                self.ctl_left = CONTROL * sh.factor;
            }
            self.ctl_left -= 1;
            for k in 0..3 {
                self.dt[k] += self.ddt[k];
                self.pw[k] += self.dpw[k];
            }
            for k in 0..5 {
                self.mix[k] += self.dmix[k];
            }
            self.cut += self.dcut;
            self.gl += self.dgl;
            self.gr += self.dgr;
            let e1 = self.env[0].tick(&self.env_c[0]) * self.env_level[0];
            let e2 = self.env[1].tick(&self.env_c[1]) * self.env_level[1];
            let gate = self.gate.tick(&self.gate_c);
            let amp = if amp_gate { gate } else { e2 };
            self.amp_level = amp;
            // Poly-mod: envelope 1 and oscillator 3 into oscillator 1 and the filter.
            let pm = self.pm_env * e1 + self.pm_osc3 * self.osc[2].last;
            // The oscillators, into one band-limited line.
            self.blep.begin();
            let mut dt1 = self.dt[0];
            let mut fm_oct = 0.0;
            if self.xmod > 0.0 {
                fm_oct += 4.0 * self.xmod * self.osc[1].last;
            }
            if s.polymod_freq && pm != 0.0 {
                fm_oct += 4.0 * pm;
            }
            if fm_oct != 0.0 {
                dt1 *= exp2(fm_oct) as f64;
            }
            self.shape[0].pw = self.pw[0] + if s.polymod_pw { 0.45 * pm } else { 0.0 };
            self.shape[1].pw = self.pw[1];
            self.shape[2].pw = self.pw[2];
            let c1 = self.osc[0].tick(dt1, &self.shape[0], self.mix[0], &mut self.blep, None);
            if self.mix[3] > 0.0 {
                let phase = self.osc[0].phase;
                self.sub
                    .tick(s.sub_shape, self.mix[3], c1, phase, &mut self.blep);
            }
            let sync = |on: bool| if on { c1.wrap } else { None };
            // An oscillator nobody hears and nothing listens to is not run.
            if self.mix[1] > 0.0 || self.dmix[1] != 0.0 || self.xmod > 0.0 {
                self.osc[1].tick(
                    self.dt[1],
                    &self.shape[1],
                    self.mix[1],
                    &mut self.blep,
                    sync(sync2),
                );
            }
            if self.mix[2] > 0.0 || self.dmix[2] != 0.0 || self.fm > 0.0 || self.pm_osc3 > 0.0 {
                self.osc[2].tick(
                    self.dt[2],
                    &self.shape[2],
                    self.mix[2],
                    &mut self.blep,
                    sync(sync3),
                );
            }
            if self.mix[4] > 0.0 {
                let w = self.noise.next();
                let v = if s.noise_pink {
                    let p = &mut self.pink;
                    p[0] = crate::va::math::flush(0.99765 * p[0] + w * 0.099_046);
                    p[1] = crate::va::math::flush(0.963 * p[1] + w * 0.296_516_4);
                    p[2] = crate::va::math::flush(0.57 * p[2] + w * 1.052_691_3);
                    0.25 * (p[0] + p[1] + p[2] + w * 0.1848)
                } else {
                    w
                };
                self.blep.add(self.mix[4] * v);
            }
            // The mixer: feedback from the amplifier, AC coupling, the drive.
            let mut xin = self.blep.end() + self.fb * self.feedback;
            if self.bw_on {
                xin = self.bw.tick(xin);
            }
            if self.g_ac > 0.0 {
                xin = self.ac.hp_c(xin, self.g_ac);
            }
            xin *= self.drive;
            self.in_peak = self.in_peak.max(xin.abs());
            if self.coefs.track > 0.9 {
                // A seeded floor at -80 dBFS, so a filter at its threshold starts on its own.
                xin += 1e-4 * self.noise.next();
            }
            if self.g_hpf > 0.0 && s.hpf_pos == HpfPos::Pre {
                xin = self.hpf.hp_c(xin, self.g_hpf);
            }
            // The filter, its cutoff moved every sample by envelope 1 and the FM.
            let mut c = self.cut + e1 * self.env_amt;
            if self.fm > 0.0 {
                c += 2.0 * self.fm * self.osc[2].last;
            }
            if s.polymod_vcf {
                c += 4.0 * pm;
            }
            if c != self.last_cut {
                self.last_cut = c;
                self.g = tan_pi(exp2(c).clamp(10.0, max_hz) * inv_fs);
            }
            let mut y = self.filter.tick(xin, self.g, &self.coefs, sh.solve);
            if self.g_hpf > 0.0 && s.hpf_pos != HpfPos::Pre {
                y = self.hpf.hp_c(y, self.g_hpf);
            }
            if s.hpf_pos == HpfPos::Boost {
                y += self.shelf.lp_c(y, self.g_shelf);
            }
            // The amplifier.
            y *= amp;
            if self.vca_drive > 0.0 {
                let d = 1.0 + 4.0 * self.vca_drive;
                y = tanh(y * d) / d * (1.0 + self.vca_drive);
            }
            self.fb = y;
            if self.fade_step > 0.0 {
                y *= self.fade;
                self.fade = (self.fade - self.fade_step).max(0.0);
            }
            l[i] += y * self.gl;
            r[i] += y * self.gr;
            let idle = if amp_gate {
                self.gate.is_idle()
            } else {
                self.env[1].is_idle()
            };
            if idle || (self.fade_step > 0.0 && self.fade <= 0.0) {
                self.active = false;
                break;
            }
        }
        self.active
    }
}
