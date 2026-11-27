//! An Arcus voice's note and its run (a child of `arcus::voice`): the note's start (a new
//! stroke, a bow change on a ringing string, a pluck), the slur, and the control step that
//! sets the bow's gesture, the pitch and the string's design for the samples until the next.

use super::*;

/// Pass 4: the pluck's noise share (Fides's exciter): at 0.15 the first 25 ms carried 30–47 dB
/// more 4–12 kHz than the next 100 ms against the recordings' 10–20 (the study's thump).
pub(super) const PLUCK_NOISE: f32 = 0.03;

/// r42 arcus-classical — how the finger's contact shortens with the note's period (its power).
pub(super) const PIZZ_CONTACT_POW: f32 = 0.8;
/// … never under this share of `pizz.soft` (a key far over the instrument's range plucked
/// with no contact at all rang eight voices to 25 × full scale).
pub(super) const PIZZ_CONTACT_MIN: f32 = 0.5;

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
        how: &NoteHow,
        fast: bool,
    ) {
        let s = sh.s;
        let was_bowing = keep && self.active && self.bowing; // r42 arcus-expr
        let keep = keep
            && self.active
            && self.inst == s.instrument
            && self.cross == s.cross // r42 fiddle
            && self.same_rate(sh); // r42:arcus-upper
                                   // r42 fiddle: one bow on two strings — the note's string found first, the voice's two
                                   // strings placed so the first is on it (`pair::place`); a string the bow leaves rings.
        let mut fresh = false;
        if keep && Self::paired(s) && tech.is_bowed() {
            let note = key as f32 + s.transpose_st + 0.01 * s.fine_ct;
            let ix = self.string_of(s, note + crate::tuning::at(sh.tuning, key), how);
            fresh = self.place(ix);
            if fresh {
                self.trim = 0.0;
                self.ear.reset();
            }
        }
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
        self.cross = s.cross; // r42 fiddle
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
        if keep && self.bowing && tech.is_bowed() && finger >= 1.0 && !fresh {
            self.glide = was - self.note - self.tune;
            self.glide_step = self.glide / finger;
        }
        self.tuning = 0.0;
        self.has_press = false;
        self.slide = 0.5;
        let spec = self.spec();
        // r42 arcus-expr: the fingering (`bow::finger`): the string, an open one's rules.
        let p = s.player.clamp(0.0, 1.0);
        let mut shift = false; // r42 arcus-classical
        if p > 0.0 {
            let from = (keep && was_bowing).then_some((was, self.string_ix));
            let fing = bow::finger(spec, self.note + self.tune, how.string, !fast, from);
            shift = fing.shift;
            self.string_ix = fing.string;
            self.open = fing.open && tech.is_bowed();
            // (A tremolo keeps pass 6's vibrato: its level in music moved 1.5–2.5 LU with it.)
            let k = if tech == NoteTech::Tremolo {
                1.0
            } else {
                bow::vib_scale(self.open, fast, false)
            };
            self.vib_k = 1.0 + (k - 1.0) * p;
        } else {
            self.string_ix = spec.string_for(self.note + self.tune);
            self.open = false;
            self.vib_k = 1.0;
        }
        // An open string cannot vibrate: the finger is off it at once.
        if !keep || (self.open && fast) {
            self.vib_kn = self.vib_k;
        }
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
        // r42 arcus-classical: with the player, a held stroke's shape is the bow's (`shape`):
        // from silence the swell's depth and length move the bow (its output envelope
        // retired), a bow change starts its stroke a few dB under, and the key let go tapers.
        // (A fiddler's bow — one bow on two strings, `bow.pair`, the Fiddle character's — plays
        // its own player: the classical shape gives way to it, `shape::classical`.)
        let p_shape = super::super::shape::classical(p, s.pair);
        let shaped = p_shape > 0.0 && tech == NoteTech::Arco;
        let begin = if keep && was_bowing {
            super::super::shape::Begin::Rebow
        } else {
            let d = d_t.clamp(0.0, 1.0);
            let (len, depth) = (spec.swell_s, spec.swell_db);
            // (A loud stroke from the string is set in less deeply: `Knobs::fresh_ff`.)
            let kn = s.shape_try.unwrap_or(super::super::shape::KNOBS);
            let ff = kn.fresh_ff;
            // (the bass's own cap: `Knobs::fresh_cap_bass`)
            let cap = if self.inst == Instrument::Bass {
                kn.fresh_cap_bass
            } else {
                kn.fresh_cap
            };
            // … and the lowest notes nearer the stroke's speed (under C3 the depth falls to none
            // at E1: a slow string's attack sits at Guettler's limit already — the cello's C and G
            // strings at pp from 15 dB under took 335 and 202 ms against 270 and 180).
            let hz = 440.0 * math::exp2((self.note + self.tune - 69.0) / 12.0);
            let low = ((hz - 41.2) / (130.8 - 41.2)).clamp(0.0, 1.0);
            super::super::shape::Begin::Fresh(
                ((depth.0 + (depth.1 - depth.0) * d) * (1.0 - ff * d) * low).min(cap),
                len.0 + (len.1 - len.0) * d * d,
            )
        };
        let knobs = s.shape_try.unwrap_or(super::super::shape::KNOBS);
        self.shape.begin(
            begin,
            &knobs,
            if shaped { p_shape } else { 0.0 },
            fast,
            s.seed,
            key,
            id,
        );
        self.taper_s = if shaped {
            knobs.taper_k * 0.001 * s.bow_release_ms.max(5.0)
        } else {
            0.0
        };
        if shaped && !keep {
            self.swell = (0.0, 1.0);
        }
        // r42 arcus-classical: a bow change inside a phrase lets the vibrato in again
        // (`shape::Knobs::vib_back`); a fast passage's keeps the passage's.
        self.vib_re = if shaped && keep && was_bowing && !fast {
            0.0
        } else {
            -1.0
        };
        // r42 arcus-classical: the player's intonation in a passage (never a note from
        // silence: `shape::intonation`).
        let (e, tend) = if shaped && keep && was_bowing {
            super::super::shape::intonation(
                &knobs,
                p_shape,
                self.note + self.tune - was,
                shift,
                s.seed,
                key,
                id,
            )
        } else {
            (0.0, 0.0)
        };
        self.into = (e, tend, 0.0);
        // r42 arcus-classical: a fiddler's fast bow change is a quicker turn, and his long
        // note's vibrato comes in late (`shape::fiddle_vib`).
        self.soft = s.pair >= 0.5 && p > 0.0 && keep && was_bowing && fast;
        self.fid_t = 0.0;
        self.lvl = (self.swell.1, self.swell.1);
        self.stop_lift = false;
        self.rough.set(Biquad::band(2500.0, 2.5, sh.sr));
        if tech == NoteTech::Pizzicato {
            // Fides's pluck on the same string; a bow still on it lifts at once.
            self.phase = Phase::Off;
            self.bowing = false;
            self.v_mag = 0.0;
            self.force = 0.0;
            // r42 arcus-classical (model 2): the finger leaves a shorter, tenser string sooner —
            // its contact scaled with the period from the instrument's lowest open string (at a
            // fixed 1.2 ms the upper registers' plucks lost their harmonics: the second octave's
            // third to sixth 25–40 dB under the recordings').
            let contact = if s.model >= 2 {
                let f_note = 440.0 * math::exp2((self.note + self.tune - 69.0) / 12.0);
                let low = spec.strings[0].open_hz() as f32;
                s.pizz_soft_ms
                    * math::powf((low / f_note.max(1.0)).min(1.0), PIZZ_CONTACT_POW)
                        .max(PIZZ_CONTACT_MIN)
            } else {
                s.pizz_soft_ms
            };
            let est = ExciterSettings {
                kind: ExciterKind::Pluck,
                contact_ms: contact,
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
            // r42 arcus-classical (model 2): a string the bow did not leave pushed has no offset
            // for the loop's DC blocker to take out — its own low corner keeps the partials in
            // tune (`string::DC_CORNER_PLUCK`).
            self.string.set_dc_share(
                (s.model >= 2 && !was_bowing).then_some(super::super::string::DC_CORNER_PLUCK),
            );
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
        self.string.set_dc_share(None); // r42 arcus-classical
                                        // The attack from the velocity: a harder note accelerates sooner and presses more
                                        // (the Guettler region measured on this model, `docs/round42/arcus.md`).
        let z0 = spec.strings[self.string_ix].z0();
        let open_hz = spec.strings[self.string_ix].open_hz(); // r42:arcus-upper
        let f = 440.0 * math::exp2((self.note as f64 + self.tune as f64 - 69.0) / 12.0);
        // r42 arcus-classical: the attack is the classic one at the stroke's shape where it
        // begins (a softer bow from silence, a new stroke's under a bow change).
        let (kv, dd) = self
            .shape
            .bowing(self.d, super::super::shape::span_db(spec));
        let bw = bowing(
            spec,
            (self.d + dd) as f64,
            self.contact(sh) as f64,
            s.bow_force as f64
                + super::super::upper::force_trim(self.inst, open_hz, f) // r42:arcus-upper
                + if s.model >= 2 { knobs.press(self.d + dd) } else { 0.0 }, // r42 arcus-classical
            s.bow_speed as f64 * super::super::upper::speed_scale(self.inst, open_hz, f), // r42:arcus-upper
            z0,
            sh.sr as f64 / f,
        )
        .scaled(kv as f64);
        let (accel, mut f_attack) = attack_of(
            s,
            self.velocity,
            &bw,
            matches!(tech, NoteTech::Staccato | NoteTech::Chop), // r42 fiddle
            1.0 / f,
        );
        // r42 arcus-expr: a stroke from silence starts where its direction and part put the
        // bow, with the arm's weight there, from the string or from the air (`bow`).
        self.kind = 0;
        if p > 0.0 && !was_bowing {
            self.bx = bow::start_x(dir, how.part, fast || tech == NoteTech::Spiccato);
            let w = bow::weight(self.bx, s.bow_weight * p);
            if matches!(tech, NoteTech::Arco | NoteTech::Staccato) {
                let start = how
                    .start
                    .unwrap_or_else(|| bow::auto_start(self.velocity, tech == NoteTech::Staccato));
                match start {
                    // The arm's weight on the attack too, on the cello and the bass (measured:
                    // without it the bass's soft E2 took 288 ms to settle, Guettler's limit
                    // 214; on the violin and the viola the soft attacks settle without it and
                    // not reliably with it — the open G at 0.15, 94–224 ms against 90).
                    bow::Start::String => {
                        self.kind = 1;
                        if matches!(self.inst, Instrument::Cello | Instrument::Bass) {
                            f_attack *= w;
                        }
                    }
                    bow::Start::Air => {
                        self.kind = 2;
                        self.land = bow::LAND_FORCE * bw.force as f32 * w;
                    }
                }
            }
        }
        // r42 fiddle (ADR 0164): the chop — from the air at the frog, landing past the region's
        // upper limit (`CHOP_FORCE`) at a fraction of the stroke's speed; never a bow change on
        // the string (the bow is lifted and dropped).
        if tech == NoteTech::Chop {
            self.kind = 2;
            if p > 0.0 && !was_bowing {
                self.bx = bow::start_x(dir, Some(bow::Part::Frog), false);
            }
            self.land = super::CHOP_FORCE * bw.f_max as f32;
        }
        self.accel = accel;
        self.f_attack = f_attack;
        self.rebow = false;
        if keep && self.bowing && tech != NoteTech::Chop {
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
            } else if tech == NoteTech::Chop {
                super::CHOP_SPEED * bw.speed as f32 // r42 fiddle
            } else if self.kind == 2 {
                bow::LAND_SPEED * bw.speed as f32
            } else {
                0.0
            };
            self.force = if matches!(tech, NoteTech::Spiccato | NoteTech::Tremolo) || self.kind == 2
            {
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
    #[allow(clippy::too_many_arguments)] // r42 arcus-expr: the note's how, the passage's speed
    pub fn glide_to(
        &mut self,
        sh: &Shared,
        key: u8,
        id: u64,
        velocity: f32,
        tech: Option<NoteTech>,
        how: &NoteHow,
        fast: bool,
    ) {
        let sh_rate = self.at_rate(sh); // r42:arcus-upper
        let sh = &sh_rate; // r42:arcus-upper
        let s = sh.s;
        // r42 fiddle: a slur across strings under one bow (`pair::place`): the string the bow
        // leaves rings on, the new one is bowed again or starts from rest.
        let mut fresh = false;
        if Self::paired(s) && self.tech.is_bowed() {
            let note = key as f32 + s.transpose_st + 0.01 * s.fine_ct;
            let ix = self.string_of(s, note + crate::tuning::at(sh.tuning, key), how);
            fresh = self.place(ix);
            if fresh {
                self.trim = 0.0;
            }
        }
        // r42 fix2-arcus: where the finger is, its tuned offset in it (before, the slur
        // started at the old key's 12-TET pitch plus the new key's offset, a jump of the
        // offsets' difference at once — 61 cents on a one-key slur in 31-EDO).
        let was = self.note + self.glide + self.tune;
        self.key = key;
        self.id = id;
        let _ = velocity;
        self.note = key as f32 + s.transpose_st + 0.01 * s.fine_ct;
        self.tune = crate::tuning::at(sh.tuning, key);
        self.glide = if fresh {
            0.0 // r42 fiddle: a string at rest has no finger to slide
        } else {
            was - self.note - self.tune
        };
        self.tuning = 0.0;
        if let Some(t) = tech.filter(|t| *t != self.tech) {
            // The bowing changes with the finger: a tremolo starts its strokes afresh.
            self.tech = t;
            if self.phase == Phase::Hold {
                self.t = 0;
            }
        }
        // r42 arcus-expr: the fingering (`bow::finger`): the string the slur stays on, a
        // shift (the hand travels: a longer finger change, the vibrato stopping and growing
        // back), an open string's rules.
        let p = s.player.clamp(0.0, 1.0);
        let spec = self.spec();
        let mut extra_ms = 0.0;
        let mut shift = false; // r42 arcus-classical
        let ix = if p > 0.0 {
            let fing = bow::finger(
                spec,
                self.note + self.tune,
                how.string,
                !fast,
                Some((was, self.string_ix)),
            );
            self.open = fing.open;
            let tremolo = tech.unwrap_or(self.tech) == NoteTech::Tremolo;
            let k = if tremolo {
                1.0
            } else {
                bow::vib_scale(fing.open, fast, true)
            };
            self.vib_k = 1.0 + (k - 1.0) * p;
            if fing.open && fast && !tremolo {
                self.vib_kn = self.vib_k;
            }
            // (A tremolo's slur is its strokes going on: its finger and dip stay pass 6's.)
            if fing.shift && !tremolo {
                shift = true;
                let st = (self.note + self.tune - was).abs();
                extra_ms = p * ((st - 2.0).max(0.0) * bow::SHIFT_MS_PER_ST).min(bow::SHIFT_MS_MAX);
                self.vib_kn *= 1.0 - 0.85 * p;
            }
            fing.string
        } else {
            spec.string_for(self.note + self.tune)
        };
        // r42 arcus-classical: the slurred note's own shape, from where the last one sagged to
        // (`shape`; none for a tremolo's strokes or with the player off).
        let p_shape = super::super::shape::classical(p, s.pair);
        let shaped = p_shape > 0.0 && tech.unwrap_or(self.tech) == NoteTech::Arco;
        let knobs = s.shape_try.unwrap_or(super::super::shape::KNOBS);
        self.shape.begin(
            super::super::shape::Begin::Slur,
            &knobs,
            if shaped { p_shape } else { 0.0 },
            fast,
            s.seed,
            key,
            id,
        );
        if !shaped {
            self.taper_s = 0.0;
        } else if self.taper_s == 0.0 {
            self.taper_s = knobs.taper_k * 0.001 * s.bow_release_ms.max(5.0);
        }
        // … and the vibrato's way back into a slurred note (none in a fast passage).
        self.vib_re = if shaped && !fast { 0.0 } else { -1.0 };
        // … and its intonation.
        let (e, tend) = if shaped {
            super::super::shape::intonation(
                &knobs,
                p_shape,
                self.note + self.tune - was,
                shift,
                s.seed,
                key,
                id,
            )
        } else {
            (0.0, 0.0)
        };
        self.into = (e, tend, 0.0);
        self.fid_t = 0.0;
        // The finger's change dips the level (the study §2.10: ~3 dB over the change).
        self.dip = (
            self.lvl_t,
            (s.finger_ms.max(0.0) + extra_ms) * 0.001 * sh.sr,
        );
        // The ear starts over on the new note (a window across the change hears neither).
        self.ear.reset();
        let ms = if s.porta_ms > 0.0 {
            s.porta_ms
        } else if sh.ctl.porta {
            150.0
        } else {
            s.finger_ms + extra_ms
        };
        let samples = ms * 0.001 * sh.sr;
        if samples < 1.0 {
            self.glide = 0.0;
            self.glide_step = 0.0;
        } else {
            self.glide_step = self.glide / samples;
        }
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
