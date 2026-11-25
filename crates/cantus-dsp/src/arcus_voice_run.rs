//! An Arcus voice's note and its run (a child of `arcus::voice`): the note's start (a new
//! stroke, a bow change on a ringing string, a pluck), the slur, and the control step that
//! sets the bow's gesture, the pitch and the string's design for the samples until the next.

use super::*;

/// Pass 4: the pluck's noise share (Fides's exciter): at 0.15 the first 25 ms carried 30–47 dB
/// more 4–12 kHz than the next 100 ms against the recordings' 10–20 (the study's thump).
pub(super) const PLUCK_NOISE: f32 = 0.03;

/// r42:arcus-p5 — how long a re-bow's finger takes to a new note on a ringing string, ms.
pub(super) const REBOW_FINGER_MS: f32 = 6.0;

#[cfg(test)]
thread_local! {
    pub(crate) static REBOW_FINGER: std::cell::Cell<f32> = const { std::cell::Cell::new(REBOW_FINGER_MS) };
}
#[cfg(test)]
fn rebow_finger_ms() -> f32 {
    REBOW_FINGER.with(|c| c.get())
}
#[cfg(not(test))]
fn rebow_finger_ms() -> f32 {
    REBOW_FINGER_MS
}

impl Voice {
    /// The dynamic this voice is asked for now (0 … 1).
    pub(super) fn dynamic_target(&self, sh: &Shared) -> f32 {
        let s = sh.s;
        let mut base = match sh.ctl.dynamic {
            Some(c) => c,
            None => (1.0 - s.vel_dyn) * s.dynamic + s.vel_dyn * self.velocity,
        };
        // r42:arcus-p5: the note's pressure is the vibrato's when `vib.src` says so.
        let press_vib = s.vib_src == super::super::VibSource::MpePressure;
        if self.has_press && s.mpe_press > 0.0 && !press_vib {
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
        let keep = keep && self.active && self.inst == s.instrument && self.same_rate(sh); // r42:arcus-upper
        if !keep {
            self.clear();
            self.vib_age = 0;
            self.trim = 0.0;
            self.ear.reset();
            self.enter_rate(sh); // r42:arcus-upper
        }
        let sh_rate = self.at_rate(sh); // r42:arcus-upper
        let sh = &sh_rate; // r42:arcus-upper
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
            // r42:arcus-p5: model 2's walk starts still, the note's own offsets drawn.
            self.vib_from = (0.0, 0.0);
            self.vib_to = (0.0, 0.0);
            self.vib_note = if s.model >= 2 {
                (self.rng.bipolar(), self.rng.bipolar())
            } else {
                (0.0, 0.0)
            };
            self.jitter = 0.0;
        }
        self.fade = 1.0;
        self.fade_step = 0.0;
        // r42:arcus-p5: where the finger was, on a string still bowed.
        let was = self.note + self.glide + self.tune;
        self.note = key as f32 + s.transpose_st + 0.01 * s.fine_ct;
        // r42 fix2-arcus: the key's tuned offset where the key becomes the note — the attack's
        // bowing, the pluck and the string it is played on all hear it (ADR 0155).
        self.tune = crate::tuning::at(sh.tuning, key);
        self.glide = 0.0;
        self.glide_step = 0.0;
        // r42:arcus-p5: a re-bow to a new note on a ringing string moves the finger over the
        // bow change's first milliseconds (`REBOW_FINGER_MS`), not in one sample: the loop's
        // length jumping under a ringing wave was a step in what the bridge hears — a click
        // 4 ms wide on the cello's and the bass's every détaché (`docs/round42/arcus-match.md`,
        // *Pass 5*).
        let finger = rebow_finger_ms() * 0.001 * sh.sr;
        if keep && self.bowing && tech.is_bowed() && finger >= 1.0 {
            self.glide = was - self.note - self.tune;
            self.glide_step = self.glide / finger;
        }
        self.tuning = 0.0;
        self.has_press = false;
        self.slide = 0.5;
        let spec = self.inst.spec();
        self.string_ix = spec.string_for(self.note + self.tune);
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
        // reach their level in 0.1–0.3 s soft, 0.03–0.08 s loud), from 20 dB under at pp; a
        // re-bow on a ringing string (détaché), a staccato or a spiccato keeps its speech. Pass
        // 4: the bow's own eased onset at the measured cellists' accelerations was tried in
        // place of it and reverted (`docs/round42/arcus-match.md`, *Pass 4*).
        self.lvl_t = 0;
        self.dip = (0, 0.0);
        // Pass 4: the instrument's own swell (`InstrumentSpec::swell_s`, `swell_db`), its length
        // falling with the dynamic's square (players hold back longer at soft and middle
        // dynamics, the study §2.5).
        self.swell = if tech == NoteTech::Arco && !keep {
            let d = d_t.clamp(0.0, 1.0);
            let (len, depth) = (spec.swell_s, spec.swell_db);
            (
                (len.0 + (len.1 - len.0) * d * d) * sh.sr,
                math::db_to_gain(-(depth.0 + (depth.1 - depth.0) * d)),
            )
        } else {
            (0.0, 1.0)
        };
        self.lvl = (self.swell.1, self.swell.1);
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
                noise: PLUCK_NOISE,
                pos: s.pizz_pos,
                ..ExciterSettings::default()
            };
            let noise = NoiseSettings::default();
            let step = ExcStep {
                level: 1.0,
                noise: PLUCK_NOISE,
                noise_band: Biquad::band(2000.0, 3.0, sh.sr),
                ..ExcStep::default()
            };
            let f0 = 440.0 * math::exp2((self.note + self.tune - 69.0) / 12.0);
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
        let open_hz = spec.strings[self.string_ix].open_hz(); // r42:arcus-upper
        let f = 440.0 * math::exp2((self.note as f64 + self.tune as f64 - 69.0) / 12.0);
        let bw = bowing(
            spec,
            self.d as f64,
            self.contact(sh) as f64,
            s.bow_force as f64 + super::super::upper::force_trim(self.inst, open_hz, f), // r42:arcus-upper
            s.bow_speed as f64 * super::super::upper::speed_scale(self.inst, open_hz, f), // r42:arcus-upper
            z0,
            sh.sr as f64 / f,
        );
        let (accel, f_attack) =
            attack_of(s, self.velocity, &bw, tech == NoteTech::Staccato, 1.0 / f);
        self.accel = accel;
        self.f_attack = f_attack;
        self.rebow = false;
        if keep && self.bowing {
            // A bow change on the ringing string: slow to the reversal, then the new
            // stroke's attack in its direction.
            self.phase = Phase::Change;
            self.next_dir = sign;
        } else {
            // r42:arcus-p5: a tremolo's first stroke is its attack (before, an attack to
            // the dynamic's speed, then the strokes from rest: the speed fell to zero within
            // a control step).
            self.phase = if matches!(tech, NoteTech::Spiccato | NoteTech::Tremolo) {
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
            self.force = if matches!(tech, NoteTech::Spiccato | NoteTech::Tremolo) {
                0.0
            } else {
                self.f_attack
            };
        }
        self.bowing = true;
    }

    /// A slur to `key`: the bow goes on, the finger moves (at once, or as a portamento when
    /// asked) between the two keys' **tuned** pitches; a key let go and returned to is held
    /// again. `tech`: the new note's way of bowing when its own articulation slurs it (a
    /// sustain into a tremolo, a tremolo into a sustain — ADR 0158), `None` to keep the
    /// voice's.
    pub fn glide_to(
        &mut self,
        sh: &Shared,
        key: u8,
        id: u64,
        velocity: f32,
        tech: Option<NoteTech>,
    ) {
        let sh_rate = self.at_rate(sh); // r42:arcus-upper
        let sh = &sh_rate; // r42:arcus-upper
        let s = sh.s;
        // r42 fix2-arcus: where the finger is, its tuned offset in it (before, the slur
        // started at the old key's 12-TET pitch plus the new key's offset, a jump of the
        // offsets' difference at once — 61 cents on a one-key slur in 31-EDO).
        let was = self.note + self.glide + self.tune;
        self.key = key;
        self.id = id;
        let _ = velocity;
        self.note = key as f32 + s.transpose_st + 0.01 * s.fine_ct;
        self.tune = crate::tuning::at(sh.tuning, key);
        self.glide = was - self.note - self.tune;
        self.tuning = 0.0;
        if let Some(t) = tech.filter(|t| *t != self.tech) {
            // The bowing changes with the finger: a tremolo starts its strokes afresh.
            self.tech = t;
            if self.phase == Phase::Hold {
                self.t = 0;
            }
        }
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
        let ix = spec.string_for(self.note + self.tune);
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
