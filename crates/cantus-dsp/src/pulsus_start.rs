//! How a hit starts a voice: the velocity laws, the section, and each model's set-up.
//!
//! The first block of `start` is Pulsus I's note-on, statement for statement, so a model 1
//! kit starts every voice with exactly the numbers it did.

use super::modal::{Object, Strike};
use super::shape::{decay_coef, pan_gains, semitones, Noise};
use super::voice::{decay_at, open_tone, Voice};
use super::{
    voice_level, DrumVoice, LayerSettings, PadSettings, SampleRef, KICK_F1_HZ, KICK_LEGACY_HZ,
};
use crate::math;

/// The six squares of the 808's metal voice, as ratios of the pad's base frequency (the
/// usually quoted 205.3, 304.4, 369.6, 522.7, 540 and 800 Hz).
const HAT_RATIOS: [f32; 6] = [1.0, 1.4827, 1.8003, 2.5461, 2.6303, 3.8967];
const HAT_BASE_HZ: f32 = 205.3;
/// The 808 cowbell's two squares.
const COWBELL_HZ: [f32; 2] = [540.0, 800.0];
const SNARE_BASE_HZ: f32 = 180.0;
/// The upper snare body as a ratio of the lower.
const SNARE_BODY_RATIO: f32 = 1.8333;
/// A modal tom's Tune 0: A3.
const TOM_BASE_HZ: f32 = 220.0;
const CYMBAL_BASE_HZ: f32 = 330.0;
/// The kick's click, the knock and the hat's stick, as Pulsus I timed them.
const CLICK_S: f32 = 0.004;
const KNOCK_S: f32 = 0.008;
const STICK_S: f32 = 0.002;

/// The metal voice's six ratios, for the render's pitch envelope.
pub(crate) fn hat_ratios() -> [f32; 6] {
    HAT_RATIOS
}

/// What a hit knows besides the pad.
#[derive(Copy, Clone, Debug)]
pub(crate) struct HitCtx {
    pub legacy: bool,
    pub pad_index: usize,
    pub legacy_hit: u32,
    pub seed: u64,
    pub velocity: u8,
    pub age: u64,
    pub note_id: u64,
    /// The metal voice's openness: the pedal's, or the pad's own.
    pub openness: f32,
}

/// The velocity laws, from the raw velocity: `(level, curve)` where `curve` is Pulsus I's
/// v^1.5 (what the tone follows) and `level` is it at the pad's sensitivity.
fn velocity(s: &PadSettings, velocity: u8, legacy: bool) -> (f32, f32, f32) {
    let lin = velocity.clamp(1, 127) as f32 / 127.0;
    let curve = math::powf(lin, 1.5);
    let level = if legacy || s.vel_level == 100.0 {
        curve
    } else {
        math::powf(lin, 1.5 * (s.vel_level / 100.0).max(0.0))
    };
    (level, curve, lin)
}

impl Voice {
    /// Start the pad's own voice. `sample` is the pad's file, for a sample pad.
    pub(crate) fn start(
        &mut self,
        s: &PadSettings,
        c: &HitCtx,
        sample: Option<SampleRef>,
        sr: f32,
    ) {
        let (vel, curve, lin) = velocity(s, c.velocity, c.legacy);
        let (gl, gr) = pan_gains(s.pan);
        let tune = if s.vel_pitch == 0.0 {
            s.tune
        } else {
            s.tune + s.vel_pitch * lin
        };
        let mut decay = if s.vel_decay == 0.0 {
            s.decay
        } else {
            s.decay * math::powf(2f32, 2.0 * (s.vel_decay / 100.0) * (lin - 1.0))
        };
        let metal_opens = s.voice == DrumVoice::HiHat && s.model != 2 && c.openness > 0.0;
        if metal_opens {
            decay = decay_at(decay, s.open_decay, c.openness);
        }
        // ---- Pulsus I's note-on ------------------------------------------------------
        self.active = true;
        self.kind = s.voice;
        self.choke = s.choke;
        self.age = c.age;
        let level = s.gain * voice_level(s.voice, s.model) * vel;
        self.gain_l = level * gl;
        self.gain_r = level * gr;
        self.amp = 1.0;
        self.amp_coef = decay_coef(decay, sr);
        self.pitch_env = 1.0;
        self.pitch_coef = decay_coef(s.pitch_decay, sr);
        self.knock = 0.0;
        self.knock_coef = decay_coef(KNOCK_S, sr);
        // Harder hits are brighter as well as louder, the way a drum is.
        let bright = if c.legacy || s.vel_tone == 50.0 {
            0.75 + 0.5 * curve
        } else {
            1.0 + (s.vel_tone / 50.0) * (0.5 * curve - 0.25)
        };
        // A model 1 kick saturates inside the voice; model 2 drives every voice after it.
        self.drive = if c.legacy {
            s.drive.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.mix = s.mix.clamp(0.0, 1.0);
        self.phase_a = 0.0;
        self.phase_b = 0.0;
        self.choking = false;
        self.noise = if c.legacy {
            Noise::seeded(c.pad_index, c.legacy_hit as u64)
        } else {
            Noise::from_seed(c.seed)
        };
        self.band.reset();
        self.high.reset();
        self.sample = None;
        // ---- Pulsus II's bookkeeping -------------------------------------------------
        self.begin(s, c, decay, sr);
        self.model = s.model;
        match (s.voice, s.model) {
            (DrumVoice::Sample, _) => {
                // No file, no sound: a missing sample is silence, not a click.
                let Some(sample) = sample else {
                    self.active = false;
                    return;
                };
                if sample.len_frames() == 0 {
                    self.active = false;
                    return;
                }
                self.start_sample(sample, tune, 0.0, false, c.legacy, sr);
            }
            (DrumVoice::Kick, 0) => {
                let base_hz = if c.legacy { KICK_LEGACY_HZ } else { KICK_F1_HZ };
                let base = semitones(base_hz, tune);
                self.base_hz = base;
                self.pitch_span = math::powf(2f32, s.pitch_drop.clamp(0.0, 48.0) / 12.0) - 1.0;
                self.inc_a = base / sr;
                // The knock: extra octaves for the first few milliseconds, scaled by the
                // click, which is what a beater hitting a head does before the shell rings.
                self.knock = 2.5 * s.click.clamp(0.0, 1.0);
                // The click is a few milliseconds of high-passed noise: a snap, not a thump.
                self.aux = s.click.clamp(0.0, 1.0) * bright;
                self.aux_coef = decay_coef(CLICK_S, sr);
                self.high.set(s.tone.clamp(200.0, 16_000.0), 0.0);
                if !c.legacy {
                    self.phase_a = s.phase.clamp(0.0, 90.0) / 360.0;
                }
            }
            (DrumVoice::Snare, 0) => {
                let base = semitones(SNARE_BASE_HZ, tune);
                self.base_hz = base;
                self.inc_a = base / sr;
                self.inc_b = base * SNARE_BODY_RATIO / sr;
                // The head's pitch drops a quarter of an octave over 15 ms at the hit.
                self.pitch_span = 0.35;
                self.pitch_coef = decay_coef(0.015, sr);
                self.aux = 1.0;
                self.aux_coef = decay_coef(s.noise_decay, sr);
                self.mix = (s.mix * bright).clamp(0.0, 1.0);
                // The band-pass at `tone` is the wires; a high-pass an octave below it is
                // the crack. One band alone is a paper bag.
                self.band.set(s.tone.clamp(200.0, 16_000.0), 0.2);
                self.high.set((s.tone * 0.5).clamp(200.0, 16_000.0), 0.0);
            }
            (DrumVoice::HiHat, 0) => {
                let base = semitones(HAT_BASE_HZ, tune);
                self.base_hz = base;
                for (h, ratio) in self.hats.iter_mut().zip(HAT_RATIOS) {
                    h.set_frequency(base * ratio);
                    h.reset(0.0);
                }
                // The stick: two milliseconds of noise on top of the metal.
                self.aux = 0.5 * bright;
                self.aux_coef = decay_coef(STICK_S, sr);
                let tone = if metal_opens {
                    open_tone(s.tone, c.openness)
                } else {
                    s.tone
                };
                self.band.set(tone.clamp(200.0, 18_000.0), 0.5);
                self.high.set(s.hp_cutoff.clamp(200.0, 18_000.0), 0.0);
            }
            _ => self.start_model(s, c, tune, decay, bright, sr),
        }
    }
}

impl Voice {
    /// Pulsus II's per-hit state: identity, the section, and a clean slate for the models.
    /// At every neutral value it leaves the voice rendering exactly as Pulsus I's.
    fn begin(&mut self, s: &PadSettings, c: &HitCtx, decay: f32, sr: f32) {
        self.legacy = c.legacy;
        self.pad_id = s.id;
        self.note_id = c.note_id;
        self.bus = s.bus;
        self.primary = true;
        self.pedal = s.pedal && s.voice == DrumVoice::HiHat;
        self.own_choke_coef = decay_coef(s.choke_fade, sr);
        self.uses_fade = false;
        self.fade = 1.0;
        self.hermite = false;
        self.reverse = false;
        self.t = 0;
        self.env = 1.0;
        // Attack and hold keep the envelope at the top before it decays.
        self.attack_len = (s.attack.max(0.0) * sr) as u32;
        self.freeze = self.attack_len + (s.hold.max(0.0) * sr) as u32;
        self.curve = s.curve.clamp(-1.0, 1.0);
        if self.curve != 0.0 {
            self.lin = 1.0;
            self.lin_step = 1.0 / (decay.max(0.0005) * sr);
            self.amp2 = 1.0;
            self.amp2_coef = self.amp_coef * self.amp_coef * self.amp_coef;
        }
        // A pitch envelope for the voices that have no sweep of their own.
        self.penv_st = if s.voice == DrumVoice::Kick {
            0.0
        } else {
            s.pitch_env
        };
        self.penv = 1.0;
        self.penv_coef = decay_coef(s.pitch_env_time, sr);
        self.filter = s.filter.min(3);
        if self.filter != 0 {
            self.cutoff = s.cutoff.clamp(20.0, 20_000.0);
            self.filter_q = 0.5 + 11.5 * math::powi(s.resonance.clamp(0.0, 1.0), 2);
            self.filter_env = s.filter_env;
            self.fl.reset();
            self.fr.reset();
            let hz = self.filter_hz(1.0);
            self.fl.set(hz, self.filter_q, sr);
            self.fr.set(hz, self.filter_q, sr);
        }
        // Model 1 kits keep drive on the kick, inside the voice.
        self.gdrive = if c.legacy {
            0.0
        } else {
            s.drive.clamp(0.0, 1.0)
        };
        self.gmode = s.drive_mode.min(3);
        if self.gdrive > 0.0 {
            self.os_l.reset();
            self.os_r.reset();
        }
        self.trans = s.trans_attack != 0.0 || s.trans_sustain != 0.0;
        self.trans_a = math::powf(10f32, s.trans_attack.clamp(-24.0, 24.0) / 20.0);
        self.trans_s = math::powf(10f32, s.trans_sustain.clamp(-24.0, 24.0) / 20.0);
        self.post = self.attack_len > 0 || self.filter != 0 || self.gdrive > 0.0 || self.trans;
        self.bank.clear();
        self.aux2 = 0.0;
        self.aux2_coef = 0.0;
        self.follow = 0.0;
        self.bursts_left = 0;
        self.glide = 0.0;
        self.openness = c.openness;
        self.closed_decay = s.decay;
        self.open_decay = s.open_decay;
        self.tone_hz = s.tone;
        self.sample_rate = sr;
    }

    /// The filter's cutoff with its envelope amount applied at envelope value `env`.
    #[inline]
    pub(crate) fn filter_hz(&self, env: f32) -> f32 {
        if self.filter_env == 0.0 {
            self.cutoff
        } else {
            semitones(self.cutoff, self.filter_env * env)
        }
    }

    fn start_sample(
        &mut self,
        sample: SampleRef,
        tune: f32,
        start_s: f32,
        reverse: bool,
        legacy: bool,
        sr: f32,
    ) {
        // Play at the file's own rate, retuned by the pad: a 44.1 kHz sample on a 48 kHz
        // engine advances 0.919 frames per output sample.
        self.inc =
            (sample.rate / sr) as f64 * math::powf(2f64, (tune.clamp(-48.0, 48.0) / 12.0) as f64);
        self.pos = 0.0;
        if !legacy {
            self.hermite = true;
            let len = sample.len_frames() as f64;
            let offset = (start_s.max(0.0) as f64 * sample.rate as f64).min(len - 1.0);
            self.reverse = reverse;
            self.pos = if reverse { len - 1.0 - offset } else { offset };
            if reverse {
                self.inc = -self.inc;
            }
        }
        self.sample = Some(sample);
    }

    /// Start one of the pad's layers: the pad's section, the layer's file, tune, level and
    /// pan, and its share of the velocity cross-fade.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn start_layer(
        &mut self,
        s: &PadSettings,
        c: &HitCtx,
        layer: &LayerSettings,
        xfade: f32,
        sample: SampleRef,
        primary: bool,
        sr: f32,
    ) {
        if sample.len_frames() == 0 {
            return;
        }
        let mut pad = *s;
        pad.voice = DrumVoice::Sample;
        pad.pan = (s.pan + layer.pan).clamp(-1.0, 1.0);
        pad.gain = s.gain * layer.gain * xfade;
        // The synth pad's own decay would cut a sampled click riding along short; a layer
        // on a synth pad plays to its end unless the pad is a sample pad.
        if s.voice != DrumVoice::Sample {
            pad.decay = 8.0;
            pad.attack = 0.0;
            pad.hold = 0.0;
            pad.curve = 0.0;
        }
        let mut ctx = *c;
        ctx.legacy = false;
        self.start(&pad, &ctx, None, sr);
        self.active = true;
        self.primary = primary;
        let tune = s.tune + layer.tune;
        self.start_sample(sample, tune, layer.start, layer.reverse, false, sr);
    }
}

impl Voice {
    /// Pulsus II's models.
    fn start_model(
        &mut self,
        s: &PadSettings,
        c: &HitCtx,
        tune: f32,
        decay: f32,
        bright: f32,
        sr: f32,
    ) {
        match (s.voice, s.model) {
            (DrumVoice::Kick, 1) => {
                // 808: a resonator struck by a pulse; it rings down by itself. A small
                // sweep at the hit, the sigh (pitch sagging with the amplitude) after.
                let base = semitones(KICK_F1_HZ, tune);
                self.base_hz = base;
                self.pitch_span = math::powf(2f32, s.pitch_drop.clamp(0.0, 48.0) / 12.0) - 1.0;
                self.sigh = s.sigh.clamp(0.0, 1.0);
                self.aux = s.click.clamp(0.0, 1.0) * bright;
                self.aux_coef = decay_coef(0.0015, sr);
                self.lp.reset();
                self.lp.set(s.tone.clamp(200.0, 16_000.0), 0.707, sr);
            }
            (DrumVoice::Kick, _) => {
                // 909: a shaped saw with a deep exponential fall, and a click of low-passed
                // noise and a pulse on envelopes of their own.
                let base = semitones(KICK_F1_HZ, tune);
                self.base_hz = base;
                self.pitch_span = s.pitch_drop.clamp(0.0, 48.0);
                self.aux = s.click.clamp(0.0, 1.0) * bright;
                self.aux_coef = decay_coef(0.006, sr);
                self.aux2 = s.click.clamp(0.0, 1.0);
                self.aux2_coef = decay_coef(0.0012, sr);
                self.lp.reset();
                self.lp.set(s.tone.clamp(200.0, 16_000.0), 0.707, sr);
                self.phase_a = s.phase.clamp(0.0, 90.0) / 360.0;
            }
            (DrumVoice::Snare, _) => {
                // Modal: the head's membrane modes, wires that follow it, a snappy tail.
                self.uses_fade = true;
                let f0 = semitones(SNARE_BASE_HZ, tune);
                self.base_hz = f0;
                self.bank.strike(
                    Object::Membrane { shell: 0.3 },
                    12,
                    Strike {
                        f0,
                        decay,
                        damping: s.damping,
                        position: s.position,
                        hardness: s.hardness,
                    },
                );
                self.amp_coef = decay_coef(decay.max(s.noise_decay), sr);
                self.aux = 1.0;
                self.aux_coef = decay_coef(s.noise_decay, sr);
                self.mix = (s.mix * bright).clamp(0.0, 1.0);
                self.wires = s.wires.clamp(0.0, 1.0);
                self.follow_coef = decay_coef(0.03, sr);
                self.delay = (0.0015 * sr) as u32;
                self.band.set(s.tone.clamp(200.0, 16_000.0), 0.2);
                self.high.set((s.tone * 0.5).clamp(200.0, 16_000.0), 0.0);
            }
            (DrumVoice::HiHat, 1) => {
                // 808 cymbal: the six squares through two bands, each with its own decay.
                self.uses_fade = true;
                let base = semitones(HAT_BASE_HZ, tune);
                self.base_hz = base;
                for (h, ratio) in self.hats.iter_mut().zip(HAT_RATIOS) {
                    h.set_frequency(base * ratio);
                    h.reset(0.0);
                }
                let tone = open_tone(s.tone, c.openness);
                self.b2.reset();
                self.b3.reset();
                self.b2.set(tone * 0.4845, 1.2, sr);
                self.b3.set(tone, 1.4, sr);
                self.aux = 1.0;
                self.aux_coef = decay_coef(decay * 0.12, sr);
                self.aux2 = 1.0;
                self.aux2_coef = decay_coef(decay * 0.55, sr);
                self.mix = s.band_mix.clamp(0.0, 1.0);
                self.high.set(s.hp_cutoff.clamp(200.0, 18_000.0), 0.0);
            }
            (DrumVoice::HiHat, 2) => {
                // 808 cowbell: two squares, a band-pass, a sharp first decay then a tail.
                self.uses_fade = true;
                for (h, hz) in self.hats.iter_mut().zip(COWBELL_HZ) {
                    h.set_frequency(semitones(hz, tune));
                    h.reset(0.0);
                }
                self.b2.reset();
                self.b2.set(s.tone.clamp(500.0, 12_000.0), 2.5, sr);
                self.aux = 1.0;
                self.aux_coef = decay_coef(0.012, sr);
            }
            (DrumVoice::HiHat, _) => {
                // Modal cymbal: a dense inharmonic bank and a shimmer that swells.
                self.uses_fade = true;
                let f0 = semitones(CYMBAL_BASE_HZ, tune);
                self.base_hz = f0;
                self.bank.strike(
                    Object::Cymbal,
                    32,
                    Strike {
                        f0,
                        decay,
                        damping: 0.25,
                        position: 0.5,
                        hardness: 0.55,
                    },
                );
                self.swell_len = ((s.swell.max(0.0) * sr) as u32).max(1);
                self.b2.reset();
                self.b2.set(open_tone(s.tone, c.openness), 0.9, sr);
                self.high.set(s.hp_cutoff.clamp(200.0, 18_000.0), 0.0);
                self.aux = 0.6 * bright;
                self.aux_coef = decay_coef(STICK_S, sr);
            }
            (DrumVoice::Clap, _) => {
                // Hands a few milliseconds apart, then the room.
                self.uses_fade = true;
                let centre = semitones(s.tone, tune);
                self.b2.reset();
                self.b2.set(centre, s.q, sr);
                self.lp.reset();
                self.lp.set(centre * 0.8, (s.q * 0.5).max(0.5), sr);
                let bursts = s.bursts.clamp(1, 6);
                self.spacing = ((s.spacing.clamp(0.001, 0.05) * sr) as u32).max(1);
                self.bursts_left = bursts - 1;
                self.next_burst = self.jittered(self.spacing);
                self.aux = bright;
                self.aux_coef = decay_coef(s.spacing.clamp(0.001, 0.05) * 1.1, sr);
                // The tail waits for the last hand.
                self.freeze += self.spacing * (bursts as u32 - 1);
                self.aux2 = 0.0;
            }
            (DrumVoice::Tom, _) => {
                self.uses_fade = true;
                let f0 = semitones(TOM_BASE_HZ, tune);
                self.base_hz = f0;
                self.bank.strike(
                    Object::from_material(s.material, s.shell),
                    s.modes.clamp(4, 32) as usize,
                    Strike {
                        f0,
                        decay,
                        damping: s.damping,
                        position: s.position,
                        hardness: s.hardness,
                    },
                );
                self.glide = math::powf(2f32, s.glide.clamp(0.0, 4.0) / 12.0) - 1.0;
                if self.glide > 0.0 || self.penv_st != 0.0 {
                    self.bank.retune(self.modal_ratio());
                }
            }
            _ => {}
        }
    }

    /// A burst interval with ±15 % of seeded jitter, so a clap is hands, not a buzz.
    pub(crate) fn jittered(&mut self, spacing: u32) -> u32 {
        let j = 0.85 + 0.3 * self.noise.unit();
        ((spacing as f32 * j) as u32).max(1)
    }

    /// The modal bank's frequency ratio now: the tension glide (falling with the energy)
    /// and the pitch envelope.
    #[inline]
    pub(crate) fn modal_ratio(&self) -> f32 {
        let mut r = 1.0 + self.glide * self.amp * self.amp;
        if self.penv_st != 0.0 {
            r *= math::powf(2f32, self.penv_st * self.penv / 12.0);
        }
        r
    }
}
