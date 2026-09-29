//! One sample of one voice: each model's core, then the section (attack, filter, drive,
//! transient). The three Pulsus I cores and the sample path are Pulsus I's arithmetic in
//! Pulsus I's order; the section is skipped entirely unless something in it is on.

use super::shape::{saturate, shape, HALF_TAPS};
use super::voice::Voice;
use super::DrumVoice;
use std::f32::consts::TAU;

/// `tanh(1.6)`: the 909 shaper's normaliser.
const TANH_1_6: f32 = 0.921_668_5;

impl Voice {
    /// This sample's contribution before the pad's pan and level, or `None` when the voice
    /// ended (a sample ran out) and freed itself.
    #[inline]
    pub(crate) fn render(&mut self, taps: &[f32; HALF_TAPS]) -> Option<(f32, f32)> {
        self.env = self.env_value();
        let (mut l, mut r, stereo) = match (self.kind, self.model) {
            (DrumVoice::Sample, _) => {
                let (a, b) = self.render_sample()?;
                (a, b, true)
            }
            (DrumVoice::Kick, 0) => {
                let x = self.render_kick();
                (x, x, false)
            }
            (DrumVoice::Snare, 0) => {
                let x = self.render_snare();
                (x, x, false)
            }
            (DrumVoice::HiHat, 0) => {
                let x = self.render_hat();
                (x, x, false)
            }
            _ => {
                let x = self.render_model();
                (x, x, false)
            }
        };
        if self.uses_fade {
            l *= self.fade;
            r *= self.fade;
        }
        if !self.post {
            return Some((l, r));
        }
        Some(self.section(l, r, stereo, taps))
    }

    /// The generic pitch envelope's frequency ratio this sample, advancing it.
    #[inline]
    fn pitch_ratio(&mut self) -> f32 {
        if self.penv_st == 0.0 {
            return 1.0;
        }
        let r = 2f32.powf(self.penv_st * self.penv / 12.0);
        self.penv *= self.penv_coef;
        r
    }

    fn render_sample(&mut self) -> Option<(f32, f32)> {
        let pm = self.pitch_ratio();
        let Some(sample) = self.sample.as_ref() else {
            self.silence();
            return None;
        };
        let ended = if self.reverse {
            self.pos < 0.0
        } else {
            self.pos >= (sample.len_frames() - 1) as f64
        };
        if ended {
            self.silence();
            return None;
        }
        let (sl, sr_) = if self.hermite {
            sample.at_hermite(self.pos)
        } else {
            sample.at(self.pos)
        };
        if pm == 1.0 {
            self.pos += self.inc;
        } else {
            self.pos += self.inc * pm as f64;
        }
        Some((sl * self.env, sr_ * self.env))
    }

    fn render_kick(&mut self) -> f32 {
        let sr = self.sample_rate;
        let hz = self.base_hz * (1.0 + self.pitch_span * self.pitch_env + self.knock);
        self.knock *= self.knock_coef;
        self.inc_a = hz / sr;
        self.phase_a += self.inc_a;
        if self.phase_a >= 1.0 {
            self.phase_a -= 1.0;
        }
        self.pitch_env *= self.pitch_coef;
        let body = (TAU * self.phase_a).sin() * self.env;
        let click = self.high.process(self.noise.next()) * self.aux;
        self.aux *= self.aux_coef;
        saturate(body + click, self.drive)
    }

    fn render_snare(&mut self) -> f32 {
        let mut bend = 1.0 + self.pitch_span * self.pitch_env;
        self.pitch_env *= self.pitch_coef;
        if self.penv_st != 0.0 {
            bend *= self.pitch_ratio();
        }
        self.phase_a += self.inc_a * bend;
        if self.phase_a >= 1.0 {
            self.phase_a -= 1.0;
        }
        self.phase_b += self.inc_b * bend;
        if self.phase_b >= 1.0 {
            self.phase_b -= 1.0;
        }
        let body = ((TAU * self.phase_a).sin() * 0.6 + (TAU * self.phase_b).sin() * 0.4) * self.env;
        // The noise has its own envelope: the bodies stop and the wires keep going.
        let white = self.noise.next();
        let wires = self.band.process(white) * 1.7;
        let crack = self.high.process(white) * 0.35;
        let noise = (wires + crack) * self.aux;
        self.aux *= self.aux_coef;
        body * (1.0 - self.mix) + noise * self.mix
    }

    fn render_hat(&mut self) -> f32 {
        if self.penv_st != 0.0 {
            let pm = self.pitch_ratio();
            let base = self.base_hz * pm;
            for (h, ratio) in self.hats.iter_mut().zip(super::start::hat_ratios()) {
                h.set_frequency(base * ratio);
            }
        }
        let mut sum = 0.0;
        for h in &mut self.hats {
            sum += h.next();
        }
        // Band-pass then high-pass: the 808's order, and why a hat is metallic rather than
        // merely bright. The stick joins before the high-pass.
        let tick = self.noise.next() * self.aux;
        self.aux *= self.aux_coef;
        let bp = self.band.process(sum * (1.0 / 6.0));
        self.high.process(bp + tick * 0.3) * self.env * 3.0
    }

    /// Pulsus II's models.
    fn render_model(&mut self) -> f32 {
        let sr = self.sample_rate;
        let e = self.env;
        match (self.kind, self.model) {
            (DrumVoice::Kick, 1) => {
                let hz = self.base_hz
                    * (1.0 + self.pitch_span * self.pitch_env)
                    * (1.0 + 0.12 * self.sigh * e * e);
                self.pitch_env *= self.pitch_coef;
                self.phase_a += hz / sr;
                if self.phase_a >= 1.0 {
                    self.phase_a -= 1.0;
                }
                let body = (TAU * self.phase_a).sin() * e;
                let pulse = self.lp.tick(self.aux * (1.0 + 0.3 * self.noise.next())).0;
                self.aux *= self.aux_coef;
                body + pulse * 0.8
            }
            (DrumVoice::Kick, _) => {
                let hz = self.base_hz * 2f32.powf(self.pitch_span * self.pitch_env / 12.0);
                self.pitch_env *= self.pitch_coef;
                self.phase_a += hz / sr;
                if self.phase_a >= 1.0 {
                    self.phase_a -= 1.0;
                }
                let body = (1.6 * (TAU * self.phase_a).sin()).tanh() / TANH_1_6 * e;
                let click = self.lp.tick(self.noise.next()).0 * self.aux * 0.7 + self.aux2 * 0.6;
                self.aux *= self.aux_coef;
                self.aux2 *= self.aux2_coef;
                body + click
            }
            (DrumVoice::Snare, _) => {
                let body = self.bank.tick() * 1.6;
                self.follow = body.abs().max(self.follow * self.follow_coef);
                let white = self.noise.next();
                let w = self.band.process(white) * 1.7;
                let crack = self.high.process(white) * 0.35;
                let wires = if self.t >= self.delay {
                    w * self.follow * self.wires * 3.0
                } else {
                    0.0
                };
                let snappy = (w + crack) * self.aux * self.mix;
                self.aux *= self.aux_coef;
                body * (1.0 - 0.5 * self.mix) + wires + snappy
            }
            (DrumVoice::HiHat, 1) => {
                let mut sum = 0.0;
                for h in &mut self.hats {
                    sum += h.next();
                }
                sum *= 1.0 / 6.0;
                let low = self.b2.tick(sum).1;
                let high = self.b3.tick(sum).1;
                let x = low * (1.0 - self.mix) * 2.0 * e
                    + high * self.mix * 2.0 * (0.6 * self.aux + 0.4 * self.aux2);
                self.aux *= self.aux_coef;
                self.aux2 *= self.aux2_coef;
                self.high.process(x) * 2.5
            }
            (DrumVoice::HiHat, 2) => {
                let sum = self.hats[0].next() + self.hats[1].next();
                let bp = self.b2.tick(sum * 0.5).1 * 0.4;
                let x = bp * (0.35 * e + 0.65 * self.aux);
                self.aux *= self.aux_coef;
                x * 2.0
            }
            (DrumVoice::HiHat, _) => {
                let modes = self.bank.tick();
                let white = self.noise.next();
                let hiss = self.b2.tick(self.high.process(white)).1;
                let rise = (self.t as f32 / self.swell_len as f32).min(1.0);
                let swell = rise * rise * (3.0 - 2.0 * rise) * e;
                let stick = white * self.aux;
                self.aux *= self.aux_coef;
                modes * 0.6 + hiss * swell * 0.5 + stick * 0.15
            }
            (DrumVoice::Clap, _) => {
                if self.bursts_left > 0 && self.t >= self.next_burst {
                    self.bursts_left -= 1;
                    self.aux = 0.8 + 0.2 * self.noise.unit();
                    let gap = self.jittered(self.spacing);
                    self.next_burst = self.t + gap;
                }
                if self.bursts_left == 0 {
                    self.aux2 = 1.0;
                }
                let white = self.noise.next();
                let hands = self.b2.tick(white).1;
                let room = self.lp.tick(hands).1;
                let x = hands * self.aux + room * e * self.aux2 * 0.6;
                self.aux *= self.aux_coef;
                x * 1.4
            }
            (DrumVoice::Tom, _) => {
                if (self.glide > 0.0 || self.penv_st != 0.0)
                    && self.t.is_multiple_of(self.retune_every)
                {
                    let ratio = self.modal_ratio();
                    self.bank.retune(ratio);
                }
                if self.penv_st != 0.0 {
                    self.penv *= self.penv_coef;
                }
                self.bank.tick() * 1.8
            }
            _ => 0.0,
        }
    }

    /// The section: attack, filter (following the envelope), drive, transient.
    fn section(&mut self, l: f32, r: f32, stereo: bool, taps: &[f32; HALF_TAPS]) -> (f32, f32) {
        let (mut l, mut r) = (l, r);
        if self.attack_len > 0 && self.t < self.attack_len {
            let g = self.t as f32 / self.attack_len as f32;
            l *= g;
            r *= g;
        }
        if self.filter != 0 {
            // The envelope moves the cutoff on a grid counted from the hit.
            if self.filter_env != 0.0 && self.t.is_multiple_of(8) {
                let hz = self.filter_hz(self.env);
                self.fl.set(hz, self.filter_q, self.sample_rate);
                self.fr.set(hz, self.filter_q, self.sample_rate);
            }
            l = self.fl.mode(l, self.filter);
            r = if stereo {
                self.fr.mode(r, self.filter)
            } else {
                l
            };
        }
        if self.gdrive > 0.0 {
            l = if self.gmode == 3 {
                shape(l, self.gdrive, 0)
            } else {
                self.os_l.process(l, self.gdrive, self.gmode, taps)
            };
            r = if !stereo {
                l
            } else if self.gmode == 3 {
                shape(r, self.gdrive, 0)
            } else {
                self.os_r.process(r, self.gdrive, self.gmode, taps)
            };
        }
        if self.trans {
            let sr = self.sample_rate;
            let (ta, tb) = (0.008 * sr, 0.020 * sr);
            let t = self.t as f32;
            let g = if t < ta {
                self.trans_a
            } else if t < tb {
                self.trans_a + (self.trans_s - self.trans_a) * (t - ta) / (tb - ta)
            } else {
                self.trans_s
            };
            l *= g;
            r *= g;
            // A boost meets a soft ceiling rather than the top of the scale.
            if g > 1.0 {
                l = l.tanh();
                r = r.tanh();
            }
        }
        (l, r)
    }
}
