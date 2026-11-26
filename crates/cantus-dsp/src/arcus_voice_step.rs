//! An Arcus voice's control step and render (a child of `arcus::voice`).

use super::*;

/// How long a staccato stroke draws before the bow stops on the string, a spiccato's
/// bounce, and how long a stopped bow holds the string before it lifts (s).
const STACCATO_S: f32 = 0.09;
/// r42:arcus-p5: 0.07 → 0.04 s — a bounce's contact is short (the references' strokes peak
/// and fall 5–10 dB within 80 ms).
const SPICCATO_S: f32 = 0.04;
const STOP_S: f32 = 0.04;
/// A spiccato's force at the bounce's middle over the dynamic's.
const SPICCATO_FORCE: f32 = 1.3;
/// A stopped bow's lift, s.
const STOP_LIFT_S: f32 = 0.025;
/// The bow's usable length, m (a cello bow's hair is about 0.65 m).
const BOW_M: f32 = 0.65;
/// A slur's level dip, dB at the middle of the finger's change (the study §2.10).
const DIP_DB: f32 = 3.0;
/// The pitch loop: it moves by `TRIM_GAIN` of what each of its ear's windows heard, never
/// more than `TRIM_STEP` cents a move.
const TRIM_GAIN: f64 = 0.6;
/// r42:arcus-p5 — a tremolo's stroke (`docs/round42/arcus-match.md`, *Pass 5*, by the
/// study's tracker over every instrument's strings): its speed a half-sine clipped at
/// `TREM_KNEE` of its peak (a quick turn, then the stroke drawn), the peak over the
/// dynamic's speed (the mean about the dynamic's); its force a light share of the dynamic's
/// riding the speed, `TREM_FLOOR` of it left at the turn: 0.3 of the Helmholtz region's lower
/// limit, under it (the strokes too short to settle; a share of the dynamic's force scaled
/// badly across the strings). A tremolo is played light: at the dynamic's force (round 42's)
/// the strokes were raucous and 8–255 cents flat, and their turns rang the bodies' lowest
/// modes louder than the note. Its level is the instrument's `trem_db` over the light
/// stroke's.
const TREM_PEAK: f32 = 1.2;
/// … the force a share of the region's lower limit at the dynamic's speed (negative: of
/// `f_min`; a positive value would be a share of the dynamic's force).
const TREM_FORCE: f32 = -0.3;
const TREM_FLOOR: f32 = 0.2;
const TREM_KNEE: f32 = 0.3;
const TRIM_STEP: f64 = 2.0;

#[cfg(test)]
thread_local! {
    pub(crate) static TREM_KNOBS: std::cell::Cell<(f32, f32, f32, f32)> =
        const { std::cell::Cell::new((TREM_PEAK, TREM_FORCE, TREM_FLOOR, TREM_KNEE)) };
}
/// r42:arcus-p5 — a bow change: the force's share of the stroke's left at the turn, and a
/// re-bow's acceleration over the attack's (`tests_p5::probe_bass_line`, the demo's bass).
const CHANGE_FLOOR: f32 = 0.5;
const REBOW_ACCEL: f32 = 2.0;

#[cfg(test)]
thread_local! {
    pub(crate) static CHANGE_KNOBS: std::cell::Cell<(f32, f32)> =
        const { std::cell::Cell::new((CHANGE_FLOOR, REBOW_ACCEL)) };
}
#[cfg(test)]
fn change_knobs() -> (f32, f32) {
    CHANGE_KNOBS.with(|c| c.get())
}
#[cfg(not(test))]
fn change_knobs() -> (f32, f32) {
    (CHANGE_FLOOR, REBOW_ACCEL)
}
#[cfg(test)]
thread_local! {
    pub(crate) static SPIC_KNOBS: std::cell::Cell<(f32, f32, f32)> =
        const { std::cell::Cell::new((SPICCATO_S, 1.0, SPICCATO_FORCE)) };
}
#[cfg(test)]
fn spic_knobs() -> (f32, f32, f32) {
    SPIC_KNOBS.with(|c| c.get())
}
#[cfg(not(test))]
fn spic_knobs() -> (f32, f32, f32) {
    (SPICCATO_S, 1.0, SPICCATO_FORCE)
}
#[cfg(test)]
fn trem_knobs() -> (f32, f32, f32, f32) {
    TREM_KNOBS.with(|c| c.get())
}
#[cfg(not(test))]
fn trem_knobs() -> (f32, f32, f32, f32) {
    (TREM_PEAK, TREM_FORCE, TREM_FLOOR, TREM_KNEE)
}

/// r42:arcus-p5 — model 2's ramp into the vibrato: the delay longer for a soft note and
/// shorter for a loud one (`VibSpec::onset_dyn`), then the rise; 1 under a controller.
fn age_ramp(v: &Voice, s: &ArcusSettings, sr: f32, plucked: bool, ctl: Option<f32>) -> f32 {
    if ctl.is_some() {
        return 1.0;
    }
    if plucked {
        return 0.0;
    }
    let od = v.spec().vib.onset_dyn;
    let delay = s.vib_delay_ms * (1.0 + od * (0.5 - v.d)).clamp(0.3, 2.5);
    let age_ms = 1000.0 * v.vib_age as f32 / sr;
    smooth((age_ms - delay) / s.vib_attack_ms.max(1.0))
}

/// r42:arcus-p5 — the per-cycle walk's memory from one cycle to the next.
const VIB_RHO: f32 = 0.0;

/// r42 fiddle — the grit's rise, dB at `grit` 1: with the force from 0.35 of the region's
/// upper limit to it, and with the contact from β 0.1 to 0.035 (a fiddler's bow at the
/// bridge, pressed). Measured in the lab (`examples/fiddle_lab`, `tools/arcus/fiddle`): at
/// 12 + 8 dB the fast run's noise between the harmonics passed them (+2 dB, the pitch lost
/// to the tracker); at 5 + 3.5 dB it rises −17.4 → −15.4 dB and the bow changes' flux cues
/// fall 0.53 → 0.37 of the notes.
pub(super) const GRIT_F_DB: f32 = 5.0;
pub(super) const GRIT_B_DB: f32 = 3.5;

/// r42 fiddle — the hair noise's gain for the bow's grit at force `f` against the region's
/// upper limit `f_max`, at the contact `beta`.
fn grit_gain(grit: f32, f: f32, f_max: f32, beta: f32) -> f32 {
    let r = if f_max > 0.0 { f / f_max } else { 0.0 };
    let by_f = ((r - 0.35) / 0.65).clamp(0.0, 1.5);
    let by_b = (math::log2(0.1 / beta.max(1e-3)) / 1.5).clamp(0.0, 1.0);
    math::db_to_gain(grit.clamp(0.0, 2.0) * (GRIT_F_DB * by_f + GRIT_B_DB * by_b))
}

/// r42 fiddle — the bite's dynamics: a note's share of the grit rises smoothly from none at
/// the dynamic `BITE_D0` to all of it at `BITE_D1` (at `bow.bite` 1 twice as fast). Chosen
/// on the owner's three takes (2026-11-26): the demos' ordinary notes (velocity 84–92,
/// the dynamic 0.66–0.72) then sit at take C's grit (0.4 of the preset's 0.75 at bite 0.5),
/// the soft ones at B's (none) and the accents at A's (all).
pub(super) const BITE_D0: f32 = 0.4;
pub(super) const BITE_D1: f32 = 0.9;

/// r42 fiddle — the share of the grit a note at the dynamic `d` gets at the bite `bite`:
/// `min(1, 2·bite·w(d))`, `w` a smoothstep from `BITE_D0` to `BITE_D1`.
pub(super) fn bite_share(bite: f32, d: f32) -> f32 {
    let w = smooth((d - BITE_D0) / (BITE_D1 - BITE_D0));
    (2.0 * bite.clamp(0.0, 1.0) * w).min(1.0)
}

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
        let spec = self.spec();
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
        // r42:arcus-p5: the vibrato from the note's own pressure when it is MPE's.
        let ctl_vib = if s.vib_src == VibSource::MpePressure {
            self.has_press.then_some(self.press)
        } else {
            ctl_vib
        };
        // r42 arcus-expr: the player's factor for the note (none on an open string, little
        // in a fast passage, stopped through a shift and growing back), glided.
        self.vib_kn += (self.vib_k - self.vib_kn) * (1.0 - math::exp(-dt / 0.12));
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
        let amount = amount * self.vib_kn;
        // r42 arcus-classical: back into a note inside a phrase, after its bow change or its
        // slur (`shape::Knobs::vib_back`; the players' onset 0.04–0.12 s, Arcus's was 0).
        let amount = if self.vib_re >= 0.0 {
            let k = self.shape.knobs().vib_back(self.vib_re);
            self.vib_re += dt;
            if self.vib_re > self.shape.knobs().vib_back_s.1 {
                self.vib_re = -1.0;
            }
            amount * k
        } else {
            amount
        };
        // r42 arcus-classical: a fiddler's late vibrato on a long note, narrow and quick
        // (`shape::fiddle_vib`; model 2, a fiddler's bow).
        let (amount, vdt) = if s.model >= 2
            && s.grit > 0.0
            && s.player > 0.0
            && self.tech == NoteTech::Arco
        {
            let (kd, kr) = super::super::shape::fiddle_vib(self.fid_t, s.grit * s.player.min(1.0));
            self.fid_t += dt;
            (amount * kd, dt * kr)
        } else {
            (amount, dt)
        };
        if s.model < 2 {
            self.vibrato_model1(spec, s, oct, amount, ramp, dt);
        } else {
            self.vibrato_model2(
                spec,
                s,
                oct,
                amount,
                age_ramp(self, s, sr, plucked, ctl_vib),
                vdt,
            );
        }
        self.vib_age = self.vib_age.saturating_add(span as u32);
        // The pitch: the key, the slur, the bend, MPE's tuning and the track's (ADR 0155).
        // r42 fix2-arcus: the table is followed while it moves (an MTS-ESP master retunes a
        // held note) but a key it turns unmapped (`NAN`) under a held note keeps the last
        // offset — the note sounds on at its pitch, and nothing that is not a number reaches
        // the string.
        let now = sh.tuning[(self.key as usize).min(127)];
        if now.is_finite() {
            self.tune = now;
        }
        let tun = self.tune;
        let mut semis =
            (self.note + self.glide + sh.ctl.bend * s.bend_st + self.tuning) as f64 + tun as f64;
        // r42 arcus-classical: the player's intonation in a passage, corrected as it is heard.
        if self.into.0 != 0.0 || self.into.1 != 0.0 {
            self.into.2 += dt;
            let c = super::super::shape::intonation_at(
                self.shape.knobs(),
                self.into.0,
                self.into.1,
                self.into.2,
            );
            semis += 0.01 * c as f64;
        }
        // r42 fiddle: a double stop's just interval (`pair`; none without one).
        let paired = Self::paired(s);
        let (just1, just2) = if paired {
            self.just_split(s)
        } else {
            (0.0, 0.0)
        };
        if just1 != 0.0 {
            semis += 0.01 * just1;
        }
        let f = 440.0 * math::exp2((semis + 0.01 * self.vib_now as f64 - 69.0) / 12.0);
        self.f0 = f;
        // The register's balance (the vibrato left out: it is the note's level, not its
        // wobble).
        self.balance = math::db_to_gain(spec.balance_db_st * (semis as f32 - spec.balance_key));
        // The string's decay: a stopped string rings shorter than an open one (the finger),
        // a lifted note shorter still.
        let stopped = (st.open_hz() / f).clamp(0.1, 1.0);
        let mut t60 = st.t60_s * s.decay.max(0.05) as f64 * (0.55 + 0.45 * stopped);
        let mut p_loss = st.p;
        // r42 fiddle: steel strings (exactly 1 at `steel` 0).
        if s.steel > 0.0 {
            t60 *= super::pair::steel_t60(s);
            p_loss *= super::pair::steel_p(s);
        }
        if plucked {
            if spec.pizz_t60.is_empty() {
                t60 *= s.pizz_decay.max(0.05) as f64;
            } else {
                // Pass 4: the references' T60 by frequency, at the fundamental and the
                // eighth partial (the loss's two design points), the controls on top.
                let k = (s.decay.max(0.05) * s.pizz_decay.max(0.05)) as f64;
                let (t1, t8) = (
                    super::super::tables::t60_at(spec.pizz_t60, f),
                    super::super::tables::t60_at(spec.pizz_t60, 8.0 * f),
                );
                t60 = t1 * k;
                p_loss = (math::ln(t1 / t8) / math::ln(8.0)).max(0.0);
            }
        }
        if self.released && !self.bowing {
            // r42 arcus-classical (model 2): a string the bow has left rings at its free decay —
            // the bow's damping is gone (the bowed table's T60 holds it: the violin's G 0.6 s,
            // its releases 150–260 dB/s against the players' ≤ 60–150) — the family's measured
            // pluck T60 at the note (`pizz_t60`) and its fall with the partials (the bowed
            // table's loss exponent left the upper partials ringing tens of ms: the level fell
            // 20 dB in 80 ms after the lift), never longer than `ring`.
            if s.model >= 2 && !plucked && !spec.pizz_t60.is_empty() {
                let k = s.decay.max(0.05) as f64;
                let (t1, t8) = (
                    super::super::tables::t60_at(spec.pizz_t60, f),
                    super::super::tables::t60_at(spec.pizz_t60, 8.0 * f),
                );
                t60 = t60.max(t1 * k);
                p_loss = p_loss.min((math::ln(t1 / t8) / math::ln(8.0)).max(0.0));
            }
            t60 = t60.min(s.ring_s.max(0.05) as f64);
        }
        if self.tech == NoteTech::Spiccato && !self.bowing {
            // r42:arcus-p5: between a spiccato's bounces the string rings free, shorter
            // than a held note (`InstrumentSpec::spic_ring`): before, it rang at the bowed
            // string's own decay and each eighth fell 4–9 dB — a light détaché.
            t60 *= (spec.spic_ring * spic_knobs().1) as f64;
        }
        // The bowing for the dynamic, and the gesture's step.
        let mut pos = self.contact(sh);
        // r42 arcus-expr: as the bow runs out the player slows it and plays nearer the
        // bridge (the level goes as v/β, and so does Schelleng's force: it stays put).
        let mut spd = 1.0f32;
        if s.player > 0.0 && self.phase == Phase::Hold && self.tech == NoteTech::Arco {
            let left = bow::left_m(self.bx, self.dir, bow::hair_m(self.inst));
            let k = bow::slow(left, s.player);
            if k < 1.0 {
                spd = k;
                pos = (pos - 2.0 * math::log2(k)).clamp(-1.0, 1.0);
            }
        }
        // r42 arcus-classical: the stroke's shape (`shape`), the bow's speed and the dynamic it
        // is read at; a taper run out lifts the bow.
        if self.bowing && self.tech.is_bowed() {
            self.shape.step(dt);
        }
        if self.shape.tapered() && matches!(self.phase, Phase::Attack | Phase::Hold) {
            // The taper has spent `bow.release`'s time: the hair leaves the string as a stopped
            // bow's does (`STOP_LIFT_S`) — held on at a light force for 0.2 s more it damped the
            // string it had just eased (the violin's release 140 dB/s against ≤ 77).
            self.phase = Phase::Release;
            self.t = 0;
            self.stop_lift = true;
        }
        let (kv, dd) = self
            .shape
            .bowing(self.d, super::super::shape::span_db(spec));
        let d_bow = self.d + dd;
        let bw = bowing(
            spec,
            d_bow as f64,
            pos as f64,
            s.bow_force as f64
                + super::super::upper::force_trim(self.inst, st.open_hz(), f) // r42:arcus-upper
                + if s.model >= 2 {
                    s.shape_try
                        .unwrap_or(super::super::shape::KNOBS)
                        .press(d_bow) // r42 arcus-classical
                } else {
                    0.0
                },
            (s.bow_speed * spd) as f64
                * super::super::upper::speed_scale(self.inst, st.open_hz(), f), // r42:arcus-upper
            st.z0(),
            sr as f64 / f,
        )
        .scaled(kv as f64);
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
        // r42 fiddle: one bow on two strings — the shares of its force, the second string's
        // step (on the first's unscaled force), the first's share (exactly 1 alone).
        if paired {
            self.step_shares(s, dt);
            if self.sec.on {
                self.step_second(sh, span, &bw, pos, spd, just2);
            }
            if self.w1 != (1.0, 1.0) {
                for j in 0..span {
                    let u = (j + 1) as f32 / span as f32;
                    self.fbuf[j] *= self.w1.0 + (self.w1.1 - self.w1.0) * u;
                }
            }
        }
        if self.bow_on && s.bow_noise_db > -60.0 {
            // r42 arcus-classical: the hair's noise rides the shape's speed (the friction's
            // roughness goes with the force, and the force with the speed; exactly 1 unshaped).
            let mut lvl =
                ROUGH * math::db_to_gain(s.bow_noise_db) * self.rate.noise_scale() * kv; // r42:arcus-upper
            // r42 fiddle: the bow's grit, rising near the bridge and with the force.
            // The bite gives each note its share of it by the dynamic (a chop all of it).
            if s.grit > 0.0 {
                let share = if self.tech == NoteTech::Chop {
                    1.0
                } else {
                    bite_share(s.bite, self.d)
                };
                lvl *= grit_gain(s.grit * share, self.force, self.f_max, self.beta);
            }
            // r42 fiddle: the chop's scratch while the bow presses.
            if self.tech == NoteTech::Chop && matches!(self.phase, Phase::Attack | Phase::Hold) {
                lvl *= math::db_to_gain(CHOP_NOISE_DB);
            }
            for j in 0..span {
                let w = self.rng.bipolar();
                self.roughs[j] = lvl * self.rough.tick(w);
            }
        } else {
            self.roughs[..span].fill(0.0);
        }
        // The pitch the loop is set to: the bowed pitch corrected by the measured table.
        // The bowed pitch: the table measured offline, and a closed loop that trims what is
        // left, held within 15 cents: it hears the fundamental's phase against the target
        // (`ear`) and moves by what each of its windows heard.
        if self.bow_on && sh.table.is_some() {
            // r42:arcus-p5: a tremolo's strokes are heard, never trimmed by — each stroke
            // is a new start, its settling not the note's pitch (the loop followed it 2–4
            // cents sharp, and before the ear's cut it random-walked with every reversal).
            let held = matches!(self.tech, NoteTech::Tremolo | NoteTech::Chop); // r42 fiddle
            if let Some(heard) = self.ear.take().filter(|_| !held) {
                self.trim = (self.trim + (TRIM_GAIN * heard).clamp(-TRIM_STEP, TRIM_STEP))
                    .clamp(-15.0, 15.0);
            }
            self.ear.aim(f / sr as f64, sr as f64);
        } else {
            self.ear.reset();
        }
        let corr = if self.bow_on {
            sh.table.map_or(0.0, |t| {
                // r42:arcus-p5: a tremolo's own table (its strokes light and short).
                let c = if self.tech == NoteTech::Tremolo {
                    t.trem_cents(self.inst, semis, self.d as f64)
                } else {
                    t.cents(self.inst, semis, self.d as f64, pos as f64)
                };
                c + self.trim
            })
        } else {
            0.0
        };
        // r42 arcus-classical (model 2): the corner sharpens with the bow's force — Cremer's
        // rounding of the Helmholtz corner goes as 1/F, so a heavier bow is a brighter one (the
        // loud cello's octaves 2–3 sat 5–7 dB under the cellos', the bass's top 6–13 dB): the
        // force now against the mf force of the same bowing, to `bright`'s powers
        // (`shape::Knobs`), within ×0.5 … ×3 and under 0.4 of the rate.
        let knobs = s.shape_try.unwrap_or(super::super::shape::KNOBS);
        let mut corner = super::super::upper::corner_hz(self.inst, spec.corner_hz, f) // r42:arcus-upper
            * super::pair::steel_corner(s); // r42 fiddle
        if s.model >= 2
            && !plucked
            && self.bow_on
            && (knobs.bright != (0.0, 0.0) || knobs.loss_force != 0.0)
        {
            let f_mf = bowing(
                spec,
                0.5,
                pos as f64,
                s.bow_force as f64 + super::super::upper::force_trim(self.inst, st.open_hz(), f),
                (s.bow_speed * spd) as f64
                    * super::super::upper::speed_scale(self.inst, st.open_hz(), f),
                st.z0(),
                sr as f64 / f,
            )
            .force;
            let r = (self.force as f64 / f_mf.max(1e-6)).max(1e-3);
            let g = if r >= 1.0 { knobs.bright.0 } else { knobs.bright.1 };
            corner = (corner * math::powf(r, g as f64).clamp(0.5, 3.0)).min(0.4 * sr as f64);
            // … and the string's upper partials lose less between the bow's re-sharpenings
            // under a heavier bow (the loss exponent, `Knobs::loss_force`).
            if knobs.loss_force != 0.0 {
                p_loss *= math::powf(r, -knobs.loss_force as f64).clamp(0.6, 1.4);
            }
        }
        let step = StringStep {
            f0: f * math::exp2(-corr / 1200.0),
            t60,
            p: p_loss,
            beta: if plucked { s.pizz_pos as f64 } else { bw.beta },
            dc: !self.bow_on,
            corner_hz: corner,
        };
        if self.first {
            self.string.start(st.z0() as f32, &step, true);
            self.restring = false; // r42 fiddle
        } else if self.restring {
            // r42 fiddle: the bow crossed onto a string at rest.
            self.string.start(st.z0() as f32, &step, false);
            self.restring = false;
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
                noise: super::run::PLUCK_NOISE,
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
        // The level's envelope at this step's end: a sustain's swell, a slur's dip.
        let t_end = self.lvl_t + span as u32;
        let mut g = 1.0f32;
        if self.swell.0 > 0.0 && (t_end as f32) < self.swell.0 {
            g = self.swell.1 + (1.0 - self.swell.1) * smooth(t_end as f32 / self.swell.0);
        }
        if self.dip.1 > 0.0 {
            let e = t_end.saturating_sub(self.dip.0) as f32;
            if e < self.dip.1 {
                g *= math::db_to_gain(-DIP_DB * math::sin(std::f32::consts::PI * e / self.dip.1));
            } else {
                self.dip.1 = 0.0;
            }
        }
        // r42 fix2-arcus: past what the loop can play in tune, the note fades out (1 to the
        // bit for every note an instrument has).
        // r42:arcus-p5: a tremolo's level (`InstrumentSpec::trem_db`), glided (20 ms) so a
        // slur from a sustain into a tremolo does not step.
        let want_t = if self.tech == NoteTech::Tremolo {
            math::db_to_gain(spec.trem_db)
        } else {
            1.0
        };
        if self.first {
            self.trem_g = want_t;
        } else {
            self.trem_g += (want_t - self.trem_g) * (1.0 - math::exp(-dt / 0.02));
        }
        g *= self.trem_g;
        let top = top_gain(sr as f64 / f, super::super::string::SHORTEST);
        g *= top;
        if self.first && top < 1.0 {
            // A note that starts past the top is quiet from its first sample.
            self.lvl.1 *= top;
        }
        self.lvl = (self.lvl.1, g);
        self.lvl_t = t_end;
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
        // r42 arcus-expr: with the player, by where the bow is along the stick (`bow`).
        let p = s.player.clamp(0.0, 1.0);
        // (The tremolo's and the spiccato's strokes keep pass 6's profile: their pitch tables
        // and their bounce are measured with it.)
        let prof = if p > 0.0 && matches!(self.tech, NoteTech::Arco | NoteTech::Staccato) {
            bow::stroke_weight(self.bx, s.bow_weight * p)
        } else {
            let used = (self.stroke as f32 / BOW_M).clamp(0.0, 1.0);
            1.0 + 0.1 * self.dir * (0.5 - used)
        };
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
                if self.rebow {
                    // r42:arcus-p5: a re-bow's new stroke on the ringing string: a quick
                    // turn (`REBOW_ACCEL` × the attack's acceleration), the force building
                    // with the speed from `CHANGE_FLOOR` of the stroke's — not a start's
                    // heavier attack force on a string already sounding.
                    let (cf, ra) = change_knobs();
                    self.v_mag = (self.v_mag + ra * self.accel * dt).min(tv);
                    self.force = tf * (cf + (1.0 - cf) * (self.v_mag / tv.max(1e-6)).min(1.0));
                } else if self.kind == 2 {
                    // r42 arcus-expr: from the air — the bow lands moving, the force arrives
                    // at once a little over the stroke's.
                    self.v_mag = (self.v_mag + self.accel * dt).min(tv);
                    self.force = self.land * (t_s / bow::LAND_S).min(1.0);
                } else {
                    self.v_mag = (self.v_mag + self.accel * dt).min(tv);
                    self.force = self.f_attack;
                }
                // r42 arcus-expr: from the string, the acceleration takes the bow to part of
                // the stroke's speed; the stroke's contour does the rest.
                let reach = if self.kind == 1 {
                    bow::STRING_V0 * tv
                } else if self.tech == NoteTech::Chop {
                    CHOP_SPEED * tv // r42 fiddle
                } else {
                    tv
                };
                if self.v_mag >= reach {
                    self.phase = Phase::Hold;
                    self.rebow = false;
                    self.t = 0;
                }
            }
            Phase::Hold => match self.tech {
                NoteTech::Spiccato => {
                    let (len, _, fk) = spic_knobs();
                    let x = t_s / len;
                    if x >= 1.0 {
                        self.phase = Phase::Off;
                        self.v_mag = 0.0;
                        self.force = 0.0;
                    } else {
                        self.v_mag = 1.2 * tv;
                        self.force = fk * tf * math::sin(std::f32::consts::PI * x);
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
                    // r42:arcus-p5: the stroke's speed a clipped half-sine and a light force
                    // riding it (`TREM_*`), instead of the dynamic's steady force over a
                    // speed through zero (the bow dragging the string at every reversal:
                    // raucous, the pitch 8–255 cents flat and wandering).
                    let (peak, fk, floor, knee) = trem_knobs();
                    let shape = (math::sin(std::f32::consts::PI * fr) / knee).min(1.0);
                    self.v_mag = peak * tv * shape;
                    let base = if fk < 0.0 {
                        -fk * bw.f_min as f32
                    } else {
                        fk * tf
                    };
                    self.force = base * (floor + (1.0 - floor) * shape);
                }
                _ => {
                    // r42 arcus-expr: a stroke from the string settles into its force and
                    // speed over its contour (`bow::settle`), a soft one longer.
                    let (c_v, c_f) =
                        if self.kind == 1 && self.tech == NoteTech::Arco && self.force > tf {
                            let (tf_s, _) = bow::settle(self.d);
                            (c_v, 1.0 - math::exp(-dt / tf_s))
                        } else {
                            (c_v, c_f)
                        };
                    // r42 fiddle: the chop holds its landing force at its own slow speed.
                    let (tv, tf, c_f) = if self.tech == NoteTech::Chop {
                        (CHOP_SPEED * tv, self.land, 1.0)
                    } else {
                        (tv, tf, c_f)
                    };
                    self.v_mag += (tv - self.v_mag) * c_v;
                    self.force += (tf - self.force) * c_f;
                    // r42 arcus-expr: the bow has run out — the player changes it.
                    if p > 0.0
                        && self.tech == NoteTech::Arco
                        && bow::left_m(self.bx, self.dir, bow::hair_m(self.inst)) < bow::CHANGE_M
                    {
                        self.phase = Phase::Change;
                        self.next_dir = -self.dir;
                        self.kind = 0;
                        self.t = 0;
                    }
                    if (self.tech == NoteTech::Staccato && t_s >= STACCATO_S)
                        || (self.tech == NoteTech::Chop && t_s >= CHOP_S)
                    {
                        self.phase = Phase::Stop;
                        self.t = 0;
                    }
                }
            },
            Phase::Change => {
                // r42 arcus-classical: a fiddler's fast change is a quicker turn (`soft`).
                let half = 0.0005
                    * s.change_ms.max(2.0)
                    * if self.soft {
                        super::super::shape::SOFT_CHANGE
                    } else {
                        1.0
                    };
                let dec = tv.max(self.v_mag) / half;
                self.v_mag -= dec * dt;
                // r42:arcus-p5: the force eases with the speed into the turn (to
                // `CHANGE_FLOOR` of the stroke's), as a player's fingers soften a change —
                // held at 0.85 of it while the bow stopped, the string was dragged at the
                // turn: an HF tick 15–20 dB over the bass's own high end
                // (`docs/round42/arcus-match.md`, *Pass 5*).
                let (cf, _) = change_knobs();
                let share = (self.v_mag.max(0.0) / tv.max(1e-6)).min(1.0);
                self.force = tf * (cf + (1.0 - cf) * share);
                if self.v_mag <= 0.0 {
                    self.v_mag = 0.0;
                    self.dir = self.next_dir;
                    self.t = 0;
                    if matches!(self.tech, NoteTech::Spiccato | NoteTech::Tremolo) {
                        // r42:arcus-p5: a tremolo's strokes start from the reversal.
                        self.phase = Phase::Hold;
                        self.force = 0.0;
                    } else {
                        self.phase = Phase::Attack;
                        self.rebow = true;
                        self.force = tf * change_knobs().0;
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
                // r42 arcus-classical: a bow lifted after its taper leaves the string still
                // moving (a player lifts a moving bow; slowed on the string it stopped it — the
                // fundamental fell 22 dB in 40 ms at the lift).
                if !(self.stop_lift && self.taper_s > 0.0) {
                    self.v_mag *= math::exp(-2.3 * dt / rel);
                }
                if self.force < 1e-4 {
                    self.force = 0.0;
                    self.v_mag = 0.0;
                    self.phase = Phase::Off;
                }
            }
        }
        if self.dir != dir_was {
            self.stroke = 0.0;
            self.ear.cut(); // r42:arcus-p5
        }
        self.stroke += (self.v_mag * dt) as f64;
        if p > 0.0 && self.tech.is_bowed() {
            // r42 arcus-expr: the bow moves along the stick (down towards the tip).
            let hair = bow::hair_m(self.inst);
            self.bx = (self.bx + self.dir * self.v_mag * dt / hair).clamp(0.0, 1.0);
        }
        self.bowing = self.phase != Phase::Off;
    }

    /// Render the voice **added** into `l`/`r` (mono: the same into both), from the
    /// stream's sample `sh.clock`, at the voice's own rate (`upper::render` calls it).
    pub(super) fn render_core(&mut self, sh: &Shared, l: &mut [f32], r: &mut [f32]) {
        // r42:arcus-upper
        let n = l.len().min(r.len());
        let mut i = 0;
        let mut peak = 0f32;
        while i < n && self.active {
            if self.first || self.pos >= self.span {
                let t = sh.clock + i as u64;
                let grid = CTRL * self.rate.os; // r42:arcus-upper
                let span = grid - (t % grid as u64) as usize; // r42:arcus-upper
                self.step(sh, span);
            }
            let k = (n - i).min(self.span - self.pos);
            let (a, b) = (self.pos, self.pos + k);
            let gain = 2.0 * self.string.z0() * OUT * self.balance;
            self.tmp[..k].fill(0.0);
            let bow = if self.bow_on {
                Some(Bow {
                    v: &self.v[a..b],
                    force: &self.fbuf[a..b],
                    rough: &self.roughs[a..b],
                    friction: self.spec().friction(sh.s.friction), // r42:arcus-p6
                })
            } else {
                None
            };
            let grid_end = b == self.span;
            #[cfg(test)]
            let mut probe = [0.0f32; SPAN]; // r42:arcus-upper
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
            if self.bow_on {
                self.ear.hear(&self.tmp[..k]);
            }
            // r42 fiddle: the second string under the same bow, or ringing.
            if self.sec.on {
                self.render_second(sh, a, b, grid_end);
            }
            for j in 0..k {
                let mut x = self.tmp[j];
                if self.lvl != (1.0, 1.0) {
                    let u = (a + j + 1) as f32 / self.span as f32;
                    x *= self.lvl.0 + (self.lvl.1 - self.lvl.0) * u;
                }
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
                let rang_out =
                    !self.bow_on && self.pluck.done() && !self.string.is_live() && !self.sec.on; // r42 fiddle
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

impl Voice {
    /// Model 1's vibrato (round 42's first), to the bit.
    fn vibrato_model1(
        &mut self,
        spec: &InstrumentSpec,
        s: &ArcusSettings,
        oct: f32,
        amount: f32,
        ramp: f32,
        dt: f32,
    ) {
        let var = s.vib_var.clamp(0.0, 1.0);
        // Wider high, and wider loud (the study §3.5: the cello ±8 → ±13 cents soft to loud).
        let widen = (1.0 + 0.12 * oct) * (0.75 + 0.5 * self.d);
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
    }

    /// r42:arcus-p5 — model 2's vibrato, the players' as the study measured it
    /// (`InstrumentSpec::vib`): its rate rising with the dynamic and the register, its extent
    /// widening with both (`vib.dyn` how much with the dynamic), and both wandering — a slow
    /// random walk from cycle to cycle, glided through each cycle (no step, no metronome),
    /// and a little per note — at `vib.var` 0.25 as much as the players'. Seeded by the note
    /// (`rng`): the same bits at every block size. The string's length moves; the body's
    /// modes make the partials' amplitudes move with it.
    fn vibrato_model2(
        &mut self,
        spec: &InstrumentSpec,
        s: &ArcusSettings,
        oct: f32,
        amount: f32,
        ramp: f32,
        dt: f32,
    ) {
        let v = spec.vib;
        let life = s.vib_var.clamp(0.0, 1.0) / 0.25;
        let reg = oct - 1.5;
        let d = self.d - 0.5;
        let fr = self.vib_phase.fract() as f32;
        let w = (
            self.vib_from.0 + (self.vib_to.0 - self.vib_from.0) * fr,
            self.vib_from.1 + (self.vib_to.1 - self.vib_from.1) * fr,
        );
        let pp = v.pp_ct
            * (1.0 + v.pp_oct * reg).max(0.3)
            * (1.0 + v.pp_dyn * s.vib_dyn.clamp(0.0, 2.0) * d).max(0.2)
            * (1.0 + life * (v.jitter.1 * w.1 + VIB_NOTE.1 * self.vib_note.1)).max(0.2);
        let depth = 0.5 * pp * s.vib_depth * amount * ramp;
        let rate = v.hz
            * s.vib_rate
            * (1.0 + v.hz_dyn * d)
            * (1.0 + v.hz_oct * reg)
            * (1.0 + life * (v.jitter.0 * w.0 + VIB_NOTE.0 * self.vib_note.0)).max(0.5);
        let before = self.vib_phase;
        self.vib_phase += rate as f64 * dt as f64;
        if self.vib_phase.floor() != before.floor() {
            // The walk's next step: unit variance, `VIB_RHO` of the last kept.
            let k = (1.0 - VIB_RHO * VIB_RHO).sqrt() * 3f32.sqrt();
            self.vib_from = self.vib_to;
            self.vib_to = (
                VIB_RHO * self.vib_to.0 + k * self.rng.bipolar(),
                VIB_RHO * self.vib_to.1 + k * self.rng.bipolar(),
            );
        }
        self.vib_now = if depth > 0.0 {
            depth * math::sin(std::f32::consts::TAU * self.vib_phase.fract() as f32)
        } else {
            0.0
        };
    }
}

/// r42:arcus-p5 — a note's own vibrato, ± its share of the rate and the extent (uniform),
/// at `vib.var` 0.25 (players do not vibrate two notes alike).
const VIB_NOTE: (f32, f32) = (0.04, 0.1);
