//! An Arcus voice's control step and render (a child of `arcus::voice`).

use super::*;

/// How long a staccato stroke draws before the bow stops on the string, a spiccato's
/// bounce, and how long a stopped bow holds the string before it lifts (s).
const STACCATO_S: f32 = 0.09;
const SPICCATO_S: f32 = 0.07;
const STOP_S: f32 = 0.04;
/// A stopped bow's lift, s.
const STOP_LIFT_S: f32 = 0.025;
/// The bow's usable length, m (a cello bow's hair is about 0.65 m).
const BOW_M: f32 = 0.65;

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

impl Voice {
    /// The control step: the dynamic, the vibrato, the pitch, the bow's gesture and the
    /// string's design for the next `span` samples, and the buffers they fill.
    pub(super) fn step(&mut self, sh: &Shared, span: usize) {
        let s = sh.s;
        let sr = sh.sr;
        let spec = self.inst.spec();
        let st = spec.strings[self.string_ix];
        let dt = span as f32 / sr;
        // The dynamic glides to its target (a controller's steps smoothed).
        let d_t = self.dynamic_target(sh);
        self.d += (d_t - self.d) * (1.0 - math::exp(-dt / 0.04));
        // A slur's or a portamento's glide.
        if self.glide_step != 0.0 {
            let g = self.glide - self.glide_step * span as f32;
            if g * self.glide <= 0.0 {
                self.glide = 0.0;
                self.glide_step = 0.0;
            } else {
                self.glide = g;
            }
        }
        // The vibrato: delayed, wider high and loud (MacLeod 2008), its rate and depth
        // wandering a little from cycle to cycle; the string's length moves, the body makes
        // the partials' amplitudes move.
        let plucked = self.tech == NoteTech::Pizzicato;
        let oct = ((self.note - spec.bottom() as f32) / 12.0).max(0.0);
        let ctl_vib = if s.vib_src == VibSource::Auto {
            None
        } else {
            sh.ctl.vibrato
        };
        let (amount, ramp) = match ctl_vib {
            Some(c) => (2.0 * c, 1.0),
            None if plucked => (0.0, 0.0),
            None => {
                let age_ms = 1000.0 * self.vib_age as f32 / sr;
                (
                    1.0,
                    smooth((age_ms - s.vib_delay_ms) / s.vib_attack_ms.max(1.0)),
                )
            }
        };
        let var = s.vib_var.clamp(0.0, 1.0);
        let widen = 1.0 + 0.12 * oct + 0.2 * (self.d - 0.5);
        let depth = spec.vib_ct
            * s.vib_depth
            * amount
            * widen
            * ramp
            * (1.0 + 0.25 * var * self.vib_wander.1);
        let rate =
            spec.vib_hz * s.vib_rate * (1.0 + 0.03 * oct) * (1.0 + 0.08 * var * self.vib_wander.0);
        let before = self.vib_phase;
        self.vib_phase += rate as f64 * dt as f64;
        if self.vib_phase.floor() != before.floor() {
            self.vib_wander = (self.rng.bipolar(), self.rng.bipolar());
        }
        self.vib_now = if depth > 0.0 {
            depth * math::sin(std::f32::consts::TAU * self.vib_phase.fract() as f32)
        } else {
            0.0
        };
        self.vib_age = self.vib_age.saturating_add(span as u32);
        // The pitch: the key, the slur, the bend, MPE's tuning and the track's (ADR 0155).
        let tun = sh.tuning[(self.key as usize).min(127)];
        let semis =
            (self.note + self.glide + sh.ctl.bend * s.bend_st + self.tuning) as f64 + tun as f64;
        let f = 440.0 * math::exp2((semis + 0.01 * self.vib_now as f64 - 69.0) / 12.0);
        self.f0 = f;
        // The string's decay: a stopped string rings shorter than an open one (the finger),
        // a lifted note shorter still.
        let stopped = (st.open_hz() / f).clamp(0.1, 1.0);
        let mut t60 = st.t60_s * s.decay.max(0.05) as f64 * (0.55 + 0.45 * stopped);
        if plucked {
            t60 *= s.pizz_decay.max(0.05) as f64;
        }
        if self.released && !self.bowing {
            t60 = t60.min(s.ring_s.max(0.05) as f64);
        }
        // The bowing for the dynamic, and the gesture's step.
        let pos = self.contact(sh);
        let bw = bowing(
            spec,
            self.d as f64,
            pos as f64,
            s.bow_force as f64,
            s.bow_speed as f64,
            st.z0(),
        );
        self.beta = bw.beta as f32;
        self.f_min = bw.f_min as f32;
        self.f_max = bw.f_max as f32;
        let (v_a, f_a) = (self.dir * self.v_mag, self.force);
        let ticks = span as u32;
        self.gesture(sh, &bw, dt, ticks);
        let (v_b, f_b) = (self.dir * self.v_mag, self.force);
        self.bow_on = v_a != 0.0 || f_a != 0.0 || v_b != 0.0 || f_b != 0.0;
        for j in 0..span {
            let u = (j + 1) as f32 / span as f32;
            self.v[j] = v_a + (v_b - v_a) * u;
            self.fbuf[j] = f_a + (f_b - f_a) * u;
        }
        // The bow-hair noise: the friction's roughness, a band of noise.
        if self.bow_on && s.bow_noise_db > -60.0 {
            let lvl = ROUGH * math::db_to_gain(s.bow_noise_db);
            for j in 0..span {
                let w = self.rng.bipolar();
                self.roughs[j] = lvl * self.rough.tick(w);
            }
        } else {
            self.roughs[..span].fill(0.0);
        }
        // The pitch the loop is set to: the bowed pitch corrected by the measured table.
        let corr = if self.bow_on {
            sh.table.map_or(0.0, |t| {
                t.cents(self.inst, semis, self.d as f64, pos as f64)
            })
        } else {
            0.0
        };
        let step = StringStep {
            f0: f * math::exp2(-corr / 1200.0),
            t60,
            p: st.p,
            beta: if plucked { s.pizz_pos as f64 } else { bw.beta },
            dc: !self.bow_on,
            corner_hz: spec.corner_hz,
        };
        if self.first {
            self.string.start(st.z0() as f32, &step, true);
        } else {
            self.string.design(&step, span as u32);
        }
        // Fides's pluck, scaled as Fides's strike is (√P around C4's: a pulse of one length
        // about as loud low as high).
        self.strike[..span].fill(0.0);
        if !self.pluck.done() {
            self.sus[..span].fill(0.0);
            let est = ExcStep {
                level: 1.0,
                noise: 0.15,
                ..ExcStep::default()
            };
            self.pluck.render(
                &est,
                sh.hammer,
                None,
                &mut self.strike[..span],
                &mut self.sus[..span],
                &mut self.tmp[..span],
            );
            let p_ref = sr as f64 / 261.63;
            let g = PLUCK * (0.5 * (self.string.period() * p_ref).sqrt()) as f32;
            for x in self.strike[..span].iter_mut() {
                *x *= g;
            }
        }
        self.span = span;
        self.pos = 0;
        self.first = false;
    }

    /// The bow's gesture over one step of `dt` seconds (`ticks` samples).
    fn gesture(&mut self, sh: &Shared, bw: &Bowing, dt: f32, ticks: u32) {
        let s = sh.s;
        let tv = bw.speed as f32;
        if self.phase != Phase::Off {
            self.jitter = 0.97 * self.jitter + 0.03 * self.rng.bipolar();
        }
        // A down-bow from the frog lightens towards the tip; an up-bow the other way.
        let used = (self.stroke as f32 / BOW_M).clamp(0.0, 1.0);
        let prof = 1.0 + 0.1 * self.dir * (0.5 - used);
        let tf = bw.force as f32 * prof * (1.0 + 0.03 * self.jitter);
        let c_v = 1.0 - math::exp(-dt / 0.03);
        let c_f = 1.0 - math::exp(-dt / 0.06);
        self.t = self.t.saturating_add(ticks);
        let t_s = self.t as f32 / sh.sr;
        let dir_was = self.dir;
        match self.phase {
            Phase::Off => {
                self.v_mag = 0.0;
                self.force = 0.0;
            }
            Phase::Attack => {
                self.v_mag = (self.v_mag + self.accel * dt).min(tv);
                self.force = self.f_attack;
                if self.v_mag >= tv {
                    self.phase = Phase::Hold;
                    self.t = 0;
                }
            }
            Phase::Hold => match self.tech {
                NoteTech::Spiccato => {
                    let x = t_s / SPICCATO_S;
                    if x >= 1.0 {
                        self.phase = Phase::Off;
                        self.v_mag = 0.0;
                        self.force = 0.0;
                    } else {
                        self.v_mag = 1.2 * tv;
                        self.force = 1.3 * tf * math::sin(std::f32::consts::PI * x);
                    }
                }
                NoteTech::Tremolo => {
                    let ph = t_s * s.trem_rate.max(1.0);
                    let n = ph.floor();
                    let fr = ph - n;
                    self.dir = if (n as i64) % 2 == 0 {
                        self.next_dir
                    } else {
                        -self.next_dir
                    };
                    self.v_mag = 1.5 * tv * math::sin(std::f32::consts::PI * fr);
                    self.force += (0.9 * tf - self.force) * c_f;
                }
                _ => {
                    self.v_mag += (tv - self.v_mag) * c_v;
                    self.force += (tf - self.force) * c_f;
                    if self.tech == NoteTech::Staccato && t_s >= STACCATO_S {
                        self.phase = Phase::Stop;
                        self.t = 0;
                    }
                }
            },
            Phase::Change => {
                let half = 0.0005 * s.change_ms.max(2.0);
                let dec = tv.max(self.v_mag) / half;
                self.v_mag -= dec * dt;
                self.force += (0.85 * tf - self.force) * c_f;
                if self.v_mag <= 0.0 {
                    self.v_mag = 0.0;
                    self.dir = self.next_dir;
                    self.t = 0;
                    if self.tech == NoteTech::Spiccato {
                        self.phase = Phase::Hold;
                        self.force = 0.0;
                    } else {
                        self.phase = Phase::Attack;
                        self.force = self.f_attack;
                    }
                }
            }
            Phase::Stop => {
                self.v_mag = (self.v_mag - tv / 0.015 * dt).max(0.0);
                if self.v_mag == 0.0 && t_s >= STOP_S {
                    self.phase = Phase::Release;
                    self.t = 0;
                    self.stop_lift = true;
                }
            }
            Phase::Release => {
                let rel = if self.stop_lift {
                    STOP_LIFT_S
                } else {
                    0.001 * s.bow_release_ms.max(5.0)
                };
                self.force *= math::exp(-4.6 * dt / rel);
                self.v_mag *= math::exp(-2.3 * dt / rel);
                if self.force < 1e-4 {
                    self.force = 0.0;
                    self.v_mag = 0.0;
                    self.phase = Phase::Off;
                }
            }
        }
        if self.dir != dir_was {
            self.stroke = 0.0;
        }
        self.stroke += (self.v_mag * dt) as f64;
        self.bowing = self.phase != Phase::Off;
    }

    /// Render the voice **added** into `l`/`r` (mono: the same into both), from the
    /// stream's sample `sh.clock`.
    pub fn render(&mut self, sh: &Shared, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let mut i = 0;
        let mut peak = 0f32;
        while i < n && self.active {
            if self.first || self.pos >= self.span {
                let t = sh.clock + i as u64;
                let span = CTRL - (t % CTRL as u64) as usize;
                self.step(sh, span);
            }
            let k = (n - i).min(self.span - self.pos);
            let (a, b) = (self.pos, self.pos + k);
            let gain = 2.0 * self.string.z0() * OUT;
            self.tmp[..k].fill(0.0);
            let bow = if self.bow_on {
                Some(Bow {
                    v: &self.v[a..b],
                    force: &self.fbuf[a..b],
                    rough: &self.roughs[a..b],
                    friction: sh.s.friction,
                })
            } else {
                None
            };
            let grid_end = b == self.span;
            #[cfg(test)]
            let mut probe = [0.0f32; CTRL];
            #[cfg(test)]
            let probe_arg = Some(&mut probe[..k]);
            #[cfg(not(test))]
            let probe_arg = None;
            self.string.render(
                &self.strike[a..b],
                bow,
                gain,
                &mut self.tmp[..k],
                probe_arg,
                grid_end,
            );
            #[cfg(test)]
            if self.record {
                self.slips.extend_from_slice(&probe[..k]);
            }
            for j in 0..k {
                let mut x = self.tmp[j];
                if self.fade_step > 0.0 {
                    self.fade = (self.fade - self.fade_step).max(0.0);
                    x *= self.fade;
                }
                l[i + j] += x;
                r[i + j] += x;
                peak = peak.max(x.abs());
            }
            self.pos = b;
            i += k;
            if grid_end {
                let rang_out = !self.bow_on && self.pluck.done() && !self.string.is_live();
                let faded = self.fade_step > 0.0 && self.fade <= 0.0;
                if rang_out || faded {
                    self.end();
                }
            }
        }
        self.peak = peak;
    }

    fn end(&mut self) {
        self.clear();
        self.ended = true;
    }
}
