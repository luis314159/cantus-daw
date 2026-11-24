//! An Arcus voice's note and its run (a child of `arcus::voice`): the note's start (a new
//! stroke, a bow change on a ringing string, a pluck), the slur, and the control step that
//! sets the bow's gesture, the pitch and the string's design for the samples until the next.

use super::*;

/// A sustain's swell: its length (s) and how far under its level it starts (dB), at pp and
/// at ff (linear between, by the dynamic).
const SWELL_S: (f32, f32) = (0.25, 0.05);
const SWELL_DB: (f32, f32) = (20.0, 10.0);
/// A pluck's snap: how long (s), its decay's time constant (s) and its level against the
/// string's peak so far.
const SNAP_S: f32 = 0.025;
pub(super) const SNAP_TAU_S: f32 = 0.005;
pub(super) const SNAP_GAIN: f32 = 0.0;

impl Voice {
    /// The dynamic this voice is asked for now (0 … 1).
    pub(super) fn dynamic_target(&self, sh: &Shared) -> f32 {
        let s = sh.s;
        let mut base = match sh.ctl.dynamic {
            Some(c) => c,
            None => (1.0 - s.vel_dyn) * s.dynamic + s.vel_dyn * self.velocity,
        };
        if self.has_press && s.mpe_press > 0.0 {
            base += (self.press - base) * s.mpe_press.clamp(0.0, 1.0);
        }
        if s.dyn_src == super::super::DynSource::Auto {
            base *= sh.ctl.expression;
        }
        base.clamp(0.0, 1.0)
    }

    /// The contact point now: the knob's, and MPE's slide around it.
    pub(super) fn contact(&self, sh: &Shared) -> f32 {
        (sh.s.bow_pos + (self.slide - 0.5) * 2.0 * sh.s.mpe_slide).clamp(-1.0, 1.0)
    }

    /// A note starts: `keep`, its string is still ringing (a re-bow, a pluck again).
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        &mut self,
        sh: &Shared,
        key: u8,
        id: u64,
        velocity: f32,
        tech: NoteTech,
        dir: BowDir,
        keep: bool,
    ) {
        let s = sh.s;
        let keep = keep && self.active && self.inst == s.instrument;
        if !keep {
            self.clear();
            self.vib_age = 0;
            self.trim = 0.0;
            self.ear.reset();
        }
        self.active = true;
        self.ended = false;
        self.released = false;
        self.first = true;
        self.key = key;
        self.id = id;
        self.velocity = velocity.clamp(0.0, 1.0);
        self.tech = tech;
        self.inst = s.instrument;
        self.rng = Rng::of(s.seed, key, id);
        if !keep {
            self.vib_phase = 0.25 * self.rng.unit() as f64;
            self.vib_wander = (0.0, 0.0);
            self.jitter = 0.0;
        }
        self.fade = 1.0;
        self.fade_step = 0.0;
        self.note = key as f32 + s.transpose_st + 0.01 * s.fine_ct;
        self.glide = 0.0;
        self.glide_step = 0.0;
        self.tuning = 0.0;
        self.has_press = false;
        self.slide = 0.5;
        let spec = self.inst.spec();
        self.string_ix = spec.string_for(self.note);
        let d_t = self.dynamic_target(sh);
        if !keep {
            self.d = d_t;
        }
        let sign = match dir {
            BowDir::Down => 1.0,
            BowDir::Up => -1.0,
        };
        self.t = 0;
        self.stroke = 0.0;
        // The level's swell over a sustain's clean start (the study §2.5, §3.4: players
        // reach their level in 0.1–0.3 s soft, 0.03–0.08 s loud), from 9 dB under at pp to 4
        // under at ff; a re-bow on a ringing string (détaché), a staccato or a spiccato keeps
        // its speech.
        self.lvl_t = 0;
        self.dip = (0, 0.0);
        self.swell = if tech == NoteTech::Arco && !keep {
            let d = d_t.clamp(0.0, 1.0);
            (
                (SWELL_S.0 + (SWELL_S.1 - SWELL_S.0) * d) * sh.sr,
                math::db_to_gain(-(SWELL_DB.0 + (SWELL_DB.1 - SWELL_DB.0) * d)),
            )
        } else {
            (0.0, 1.0)
        };
        self.lvl = (self.swell.1, self.swell.1);
        self.snap_n = 0;
        if tech == NoteTech::Pizzicato {
            self.snap_n = (SNAP_S * sh.sr) as u32;
            self.snap_env = 1.0;
            self.snap_pk = 0.0;
            self.snap_bq = Biquad::default();
            self.snap_bq.set(Biquad::band(2500.0, 2.6, sh.sr));
        }
        self.stop_lift = false;
        self.rough.set(Biquad::band(2500.0, 2.5, sh.sr));
        if tech == NoteTech::Pizzicato {
            // Fides's pluck on the same string; a bow still on it lifts at once.
            self.phase = Phase::Off;
            self.bowing = false;
            self.v_mag = 0.0;
            self.force = 0.0;
            let est = ExciterSettings {
                kind: ExciterKind::Pluck,
                contact_ms: s.pizz_soft_ms,
                noise: 0.15,
                pos: s.pizz_pos,
                ..ExciterSettings::default()
            };
            let noise = NoiseSettings::default();
            let step = ExcStep {
                level: 1.0,
                noise: 0.15,
                noise_band: Biquad::band(2000.0, 3.0, sh.sr),
                ..ExcStep::default()
            };
            let f0 = 440.0 * math::exp2((self.note - 69.0) / 12.0);
            let rng = self.rng;
            self.pluck.start(
                &est,
                &noise,
                &step,
                self.velocity,
                1.0,
                sh.hammer_area,
                f0,
                rng,
            );
            return;
        }
        self.pluck.clear();
        // The attack from the velocity: a harder note accelerates sooner and presses more
        // (the Guettler region measured on this model, `docs/round42/arcus.md`).
        let z0 = spec.strings[self.string_ix].z0();
        let f = 440.0 * math::exp2((self.note as f64 - 69.0) / 12.0);
        let bw = bowing(
            spec,
            self.d as f64,
            self.contact(sh) as f64,
            s.bow_force as f64,
            s.bow_speed as f64,
            z0,
            sh.sr as f64 / f,
        );
        let (accel, f_attack) =
            attack_of(s, self.velocity, &bw, tech == NoteTech::Staccato, 1.0 / f);
        self.accel = accel;
        self.f_attack = f_attack;
        if keep && self.bowing {
            // A bow change on the ringing string: slow to the reversal, then the new
            // stroke's attack in its direction.
            self.phase = Phase::Change;
            self.next_dir = sign;
        } else {
            self.phase = if tech == NoteTech::Spiccato {
                Phase::Hold
            } else {
                Phase::Attack
            };
            self.dir = sign;
            self.next_dir = sign;
            self.v_mag = if tech == NoteTech::Spiccato {
                1.2 * bw.speed as f32
            } else {
                0.0
            };
            self.force = if tech == NoteTech::Spiccato {
                0.0
            } else {
                self.f_attack
            };
        }
        self.bowing = true;
    }

    /// A slur to `key`: the bow goes on, the finger moves (at once, or as a portamento when
    /// asked); a key let go and returned to is held again.
    pub fn glide_to(&mut self, sh: &Shared, key: u8, id: u64, velocity: f32) {
        let s = sh.s;
        let was = self.note + self.glide;
        self.key = key;
        self.id = id;
        let _ = velocity;
        self.note = key as f32 + s.transpose_st + 0.01 * s.fine_ct;
        self.glide = was - self.note;
        self.tuning = 0.0;
        // The finger's change dips the level (the study §2.10: ~3 dB over the change).
        self.dip = (self.lvl_t, s.finger_ms.max(0.0) * 0.001 * sh.sr);
        // The ear starts over on the new note (a window across the change hears neither).
        self.ear.reset();
        let ms = if s.porta_ms > 0.0 {
            s.porta_ms
        } else if sh.ctl.porta {
            150.0
        } else {
            s.finger_ms
        };
        let samples = ms * 0.001 * sh.sr;
        if samples < 1.0 {
            self.glide = 0.0;
            self.glide_step = 0.0;
        } else {
            self.glide_step = self.glide / samples;
        }
        let spec = self.inst.spec();
        let ix = spec.string_for(self.note);
        if ix != self.string_ix {
            self.string_ix = ix;
            self.string.set_z0(spec.strings[ix].z0() as f32);
        }
        if self.released {
            self.released = false;
            if self.phase == Phase::Release && self.tech.is_bowed() {
                self.phase = Phase::Hold;
            }
        }
    }
}
