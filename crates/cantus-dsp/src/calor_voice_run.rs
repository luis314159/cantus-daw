//! What a voice does: start, move, release, update its controls, render (a child of
//! `calor::voice`).

use super::*;

/// The amplifier's envelope when it follows the gate: fast, but never a click.
const GATE: EnvSettings = EnvSettings {
    attack: 0.001,
    decay: 0.01,
    sustain: 1.0,
    release: 0.004,
    model: EnvModel::Rc,
};

/// Stolen voices fade over this long.
pub const DECLICK_S: f32 = 0.003;

impl Voice {
    /// A fresh note on this voice: every generator restarted, the envelopes from zero.
    /// `from`: the pitch to glide from, if it glides.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        &mut self,
        sh: &Shared,
        key: u8,
        vel: f32,
        id: u64,
        seed: u64,
        card: usize,
        stack_pos: f32,
        from: Option<f32>,
    ) {
        let s = sh.s;
        *self = Voice {
            active: true,
            key,
            id,
            card,
            stack_pos,
            vel,
            seed,
            rnd: bipolar(hash3(seed, 0x7a9d, 0)),
            noise: Noise::new(seed ^ 0x4e01),
            blep: Blep::new(Kernel::Wide),
            ..Default::default()
        };
        for (i, o) in self.osc.iter_mut().enumerate() {
            o.phase = if s.phase_reset {
                0.0
            } else {
                start_phase(seed, i as u64)
            };
        }
        self.target = key as f32;
        self.pitch = key as f32;
        self.glide_to(sh, from);
        for (l, set) in self.lfo.iter_mut().zip(s.lfo.iter()) {
            l.start(set, seed);
        }
        self.env_coefs(sh);
        self.gate_c = EnvCoefs::new(&GATE, sh.fs_os, 1.0);
        self.env[0].gate_on();
        self.env[1].gate_on();
        self.gate.gate_on();
    }

    /// Move to another note without a new attack (legato, or back to a note still held),
    /// or with one from where the envelopes are (`retrigger`).
    pub fn move_to(&mut self, sh: &Shared, key: u8, id: u64, vel: Option<f32>, retrigger: bool) {
        let from = self.pitch;
        self.key = key;
        self.id = id;
        if let Some(v) = vel {
            self.vel = v;
        }
        self.target = key as f32;
        self.released = false;
        self.glide_to(sh, Some(from));
        if retrigger {
            self.env[0].gate_on();
            self.env[1].gate_on();
            self.gate.gate_on();
        }
    }

    /// Glide from `from` to the target, if gliding is on.
    fn glide_to(&mut self, sh: &Shared, from: Option<f32>) {
        let s = sh.s;
        let t = s.glide * sh.trims[self.card].glide_scale;
        match from {
            Some(f) if s.glide > 0.0011 && (f - self.target).abs() > 1e-4 => {
                self.pitch = f;
                let interval = (self.target - f).abs();
                let tick = CONTROL as f32 / sh.fs;
                let time = if s.glide_rate { t * interval / 12.0 } else { t };
                if s.glide_linear {
                    self.glide_step = interval * tick / time.max(tick);
                    self.glide_coef = 0.0;
                } else {
                    // An RC lag: ~95 % of the way in the time set.
                    self.glide_coef = 1.0 - crate::va::math::exp(-3.0 * tick / time.max(tick));
                    self.glide_step = 0.0;
                }
            }
            _ => {
                self.pitch = self.target;
                self.glide_step = 0.0;
                self.glide_coef = 0.0;
            }
        }
    }

    /// Land on the target pitch now (no glide).
    pub fn snap(&mut self) {
        self.pitch = self.target;
        self.glide_step = 0.0;
        self.glide_coef = 0.0;
    }

    pub fn release(&mut self, rel_vel: f32) {
        self.released = true;
        self.rel_vel = rel_vel;
        self.env[0].gate_off(&self.env_c[0]);
        self.env[1].gate_off(&self.env_c[1]);
        self.gate.gate_off(&self.gate_c);
    }

    /// Start fading out (a stolen voice, a choke).
    pub fn fade_out(&mut self, sh: &Shared) {
        self.fade_step = 1.0 / (DECLICK_S * sh.fs_os);
    }

    /// The amplifier's envelope level (what the card lights show).
    pub fn level(&self) -> f32 {
        self.amp_level
    }

    /// Rebuild what depends on the sample rate (the quality changed).
    pub fn rebuild(&mut self, sh: &Shared) {
        self.env_coefs(sh);
        self.gate_c = EnvCoefs::new(&GATE, sh.fs_os, 1.0);
        self.last_cut = f32::NAN;
    }

    /// Envelope 1's and 2's levels.
    pub fn env_levels(&self) -> [f32; 2] {
        [self.env[0].level, self.env[1].level]
    }

    fn env_coefs(&mut self, sh: &Shared) {
        let s = sh.s;
        let trims = &sh.trims[self.card];
        let m = &self.mods;
        let key = self.key as f32 - 60.0;
        for e in 0..2 {
            let base = &s.env[e];
            let scale = trims.env_scale * exp2(-key / 12.0 * s.env_kbd[e]);
            let (ta, td, ts, tr) = if e == 0 {
                (Target::Env1A, Target::Env1D, Target::Env1S, Target::Env1R)
            } else {
                (Target::Env2A, Target::Env2D, Target::Env2S, Target::Env2R)
            };
            let set = EnvSettings {
                attack: base.attack * scale * exp2(m[ta as usize]),
                decay: base.decay * scale * exp2(m[td as usize]),
                sustain: (base.sustain + m[ts as usize]).clamp(0.0, 1.0),
                release: base.release * scale * exp2(m[tr as usize]),
                model: base.model,
            };
            self.env_c[e] = EnvCoefs::new(&set, sh.fs_os, 1.0);
            self.env_level[e] = 1.0 - s.env_vel[e] + s.env_vel[e] * self.vel;
        }
    }
}

#[path = "calor_voice_tick.rs"]
mod tick;
