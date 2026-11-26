//! One Arcus voice (a child of `arcus`): a string, the bow's gesture on it (its speed, force
//! and contact point through the attack, the stroke, a bow change and the lift), Fides's pluck
//! for pizzicato, the vibrato and the pitch — stepped on the control grid, rendered between.

use super::pitch::PitchTable;
use super::string::{Bow, BowedString, StringStep};
use super::tables::{schelleng_max, Instrument, InstrumentSpec};
use super::{bow, ArcusSettings, Art, BowDir, Controls, NoteHow, Technique, VibSource, CTRL};
use crate::fides::exciter::{
    Biquad, ExcStep, Exciter, ExciterKind, ExciterSettings, NoiseSettings, Rng,
};
use crate::lamina::voice::HammerShape;
use crate::math;

#[path = "arcus_voice_run.rs"]
mod run;
#[path = "arcus_voice_step.rs"]
mod step;
#[cfg(test)]
pub(crate) use run::REBOW_FINGER; // r42:arcus-p5
#[cfg(test)]
pub(crate) use step::{CHANGE_KNOBS, SPIC_KNOBS, TREM_KNOBS}; // r42:arcus-p5
#[path = "arcus_voice_pair.rs"] // r42 fiddle
mod pair;
#[path = "arcus_upper_voice.rs"] // r42:arcus-upper
mod upper; // r42:arcus-upper // r42 fiddle
pub use pair::JUST_CT; // r42 fiddle

/// r42 fiddle — a steel set's ring over the table's (exactly 1 at `steel` 0).
pub fn steel_t60_of(s: &ArcusSettings) -> f64 {
    pair::steel_t60(s)
}

/// The longest control step a voice renders: the grid's `CTRL` samples at the rate, which an
/// upper string's voice at `os ×` the rate counts as `CTRL · os` of its own. // r42:arcus-upper
const SPAN: usize = CTRL * super::upper::MAX_OS; // r42:arcus-upper

/// What every voice reads while it renders (the instance's, shared).
pub struct Shared<'a> {
    pub s: &'a ArcusSettings,
    pub sr: f32,
    pub ctl: &'a Controls,
    pub tuning: &'a [f32; 128],
    pub hammer: &'a HammerShape,
    pub hammer_area: f32,
    pub table: Option<&'static PitchTable>,
    /// The stream's sample at the start of the slice being rendered.
    pub clock: u64,
}

/// How one note is played.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum NoteTech {
    #[default]
    Arco,
    Staccato,
    Spiccato,
    Pizzicato,
    Tremolo,
    /// r42 fiddle (ADR 0164) — the chop: from the air at the frog, the force landing past the
    /// Helmholtz region's upper limit at a fraction of the stroke's speed, the hair's noise
    /// raised, stopped on the string ([`CHOP_S`]).
    Chop,
}

/// r42 fiddle — the chop's force over the region's upper limit (Schelleng's raucous side),
/// its speed over the stroke's, how long it draws before the bow stops on the string (s), and
/// its hair noise over the bow's (dB).
pub const CHOP_FORCE: f32 = 2.5;
pub const CHOP_SPEED: f32 = 0.3;
pub const CHOP_S: f32 = 0.06;
pub const CHOP_NOISE_DB: f32 = 12.0;

impl NoteTech {
    /// The note's own articulation over the instrument's technique.
    pub fn of(t: Technique, art: Option<Art>) -> NoteTech {
        match art {
            Some(Art::Staccato) => NoteTech::Staccato,
            Some(Art::Spiccato) => NoteTech::Spiccato,
            Some(Art::Pizzicato) => NoteTech::Pizzicato,
            Some(Art::Tremolo) => NoteTech::Tremolo,
            Some(Art::Chop) => NoteTech::Chop, // r42 fiddle
            Some(Art::Sustain | Art::Legato) => NoteTech::Arco,
            None => match t {
                Technique::Arco => NoteTech::Arco,
                Technique::Pizzicato => NoteTech::Pizzicato,
                Technique::Tremolo => NoteTech::Tremolo,
            },
        }
    }

    pub fn is_bowed(self) -> bool {
        self != NoteTech::Pizzicato
    }
}

/// The bow's gesture.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Phase {
    /// Off the string.
    #[default]
    Off,
    /// Accelerating from rest to the stroke's speed.
    Attack,
    /// Drawn.
    Hold,
    /// A bow change: slowing to the reversal, then the new stroke's attack.
    Change,
    /// Stopped on the string (a staccato's end): the string is held still.
    Stop,
    /// Lifting.
    Release,
}

/// The bowing for a dynamic `d` on a string: speed (m/s), β and force (N), and the region
/// of Helmholtz motion around it as this model measures it (`tests_gate`'s Schelleng test on
/// the cello's open D: the upper limit near Schelleng's `F_max = 2Z₀v_b/((μs − μd)β)`, the
/// lower near `0.3·F_max·(0.1/β)^1.4` — Schelleng's `β⁻²` law, a little gentler).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Bowing {
    pub speed: f64,
    pub beta: f64,
    pub force: f64,
    pub f_min: f64,
    pub f_max: f64,
    /// Schelleng's own upper limit, `2Z₀v_b/((μs − μd)β)`: the unit the force is placed in.
    pub schelleng: f64,
}

impl Bowing {
    /// r42 arcus-classical — the bowing with its speed × `k` (the stroke's shape, `shape`):
    /// Schelleng's limit, the measured region and the force are all linear in the speed, so the
    /// bowing keeps its place in the region (exactly itself at 1).
    pub fn scaled(self, k: f64) -> Bowing {
        if k == 1.0 {
            return self;
        }
        Bowing {
            speed: self.speed * k,
            beta: self.beta,
            force: self.force * k,
            f_min: self.f_min * k,
            f_max: self.f_max * k,
            schelleng: self.schelleng * k,
        }
    }
}

/// The region of Helmholtz motion at `beta` for a bow at `v_bow` on a string of impedance
/// `z0`, as this model measures it (N): `(lower, upper)`.
pub fn limits(z0: f64, v_bow: f64, beta: f64) -> (f64, f64) {
    // Fitted to `tests_gate2::the_helmholtz_region_has_schelleng_s_shape` (the cello's open
    // D at 10 cm/s): the lower limit 0.21 of Schelleng's F_max at β 0.1, falling as β^−1.5;
    // the upper 0.84 of it there, falling only as β^−0.23 in newtons.
    let f_max = schelleng_max(z0, v_bow, beta);
    let b = beta.max(1e-3) / 0.1;
    let hi = 0.84 * f_max * math::powf(b, 0.77);
    let lo = (0.21 * f_max * math::powf(b, -0.5)).min(hi);
    (lo, hi)
}

/// The most β the bowing takes (a contact a third of the way to the nut is no longer a
/// bowed string's).
pub const MAX_BETA: f64 = 0.35;

/// The period (samples) under which the bow–bridge segment can no longer be five samples
/// long at [`MAX_BETA`]: `5 / 0.35`, about 14.3 samples (A7 at 48 kHz on a cello's voice,
/// 13.4 kHz on the violin's at 4×).
pub const SHORTEST_PERIOD: f64 = 5.0 / MAX_BETA;

/// The least β the bowing takes: under it the measured region narrows past the force's
/// place (at β 0.04 it spans 0.5–0.7 of Schelleng's limit), and the bow–bridge segment must
/// stay five samples long at the rate (`period` the note's, in samples) for the junction's
/// lines to hold it — never more than [`MAX_BETA`] (r42 fix2-arcus: past
/// [`SHORTEST_PERIOD`] the five samples cannot be had, the segment shortens with the period
/// at β 0.35, the string's design keeps it two samples or more, and the voice fades out —
/// see [`top_gain`]; before, the bounds crossed and `clamp` panicked on the audio thread).
pub fn min_beta(period: f64) -> f64 {
    (5.0 / period.max(1.0)).clamp(0.055, MAX_BETA)
}

/// r42 fix2-arcus — the level a note keeps at the top of what a string's loop can play
/// (`period` the note's in samples at the voice's rate, `shortest` the loop's least, 12 at
/// [`string::BowedString::max_hz`]): 1 down to [`SHORTEST_PERIOD`] (exactly, so every note an
/// instrument has keeps its bits), falling smoothly to silence at `shortest` — above it the
/// loop could only play a wrong pitch (its length is held at the shortest), so the note fades
/// out rather than sound out of tune, and a bend or vibrato across the edge stays continuous.
pub fn top_gain(period: f64, shortest: f64) -> f32 {
    if period >= SHORTEST_PERIOD {
        return 1.0;
    }
    if period.is_nan() || period <= shortest {
        return 0.0;
    }
    let x = ((period - shortest) / (SHORTEST_PERIOD - shortest)).clamp(0.0, 1.0) as f32;
    x * x * (3.0 - 2.0 * x)
}

/// The bowing of dynamic `d` (0 … 1) at the contact point `pos` (−1 … 1) with the force's
/// trim `place` (−1 … 1, an octave of force either way) and the speed's `speed` (×), on a
/// string of impedance `z0`, for a note whose period is `period` samples.
#[allow(clippy::too_many_arguments)]
pub fn bowing(
    spec: &InstrumentSpec,
    d: f64,
    pos: f64,
    place: f64,
    speed: f64,
    z0: f64,
    period: f64,
) -> Bowing {
    let b = &spec.bowing;
    let d = d.clamp(0.0, 1.0);
    let v = b.speed.0 * math::powf(b.speed.1 / b.speed.0, d) * speed.clamp(0.25, 4.0);
    let beta = ((b.beta.0 + (b.beta.1 - b.beta.0) * d) * math::exp2(-0.5 * pos.clamp(-1.0, 1.0)))
        .clamp(min_beta(period), MAX_BETA);
    let (f_min, f_max) = limits(z0, v, beta);
    let schelleng = schelleng_max(z0, v, beta);
    let k = (b.place.0 + (b.place.1 - b.place.0) * d) * math::exp2(place.clamp(-1.0, 1.0));
    Bowing {
        speed: v,
        beta,
        force: schelleng * k,
        f_min,
        f_max,
        schelleng,
    }
}

/// The attack a velocity asks for (Guettler 2002's two coordinates): the bow's acceleration
/// to the stroke's speed (m/s²) and the force it is placed with (N), for a string of period
/// `period_s`. In this model's own measured terms (`tests_gate::probe_torsion`, the region
/// where the Helmholtz motion starts within 50 ms): the speed gained a period, `α = a·P/v`,
/// from 0.18 (the softest) to 1.45 (the hardest) — a harder note accelerates sooner, a low
/// string speaks slower — with the force at 0.65 of Schelleng's upper limit, lighter where
/// the acceleration is slower than the region's (the wedge's lower edge: 0.2 at `α` 0.045).
pub fn attack_of(
    s: &ArcusSettings,
    velocity: f32,
    bw: &Bowing,
    staccato: bool,
    period_s: f64,
) -> (f32, f32) {
    let x = velocity.clamp(0.0, 1.0) as f64;
    let va = s.vel_attack.clamp(0.0, 1.0) as f64;
    // The velocity's share of the attack: at 0 every note the middle one.
    let x = 0.5 + (x - 0.5) * (2.0 * va).min(1.0);
    let mut alpha = 0.18 * math::powf(8.0, x) / s.bow_attack.clamp(0.25, 4.0) as f64;
    if staccato {
        alpha *= 2.5;
    }
    let accel = alpha * bw.speed / period_s.max(1e-4);
    let u = (math::ln(alpha / 0.045) / math::ln(4.0)).clamp(0.0, 1.0);
    let phi = 0.2 + (0.65 - 0.2) * u;
    let force = (phi * bw.schelleng).max(0.25 * bw.force);
    (accel as f32, force as f32)
}

/// r42 fix2-arcus — how far under its lowest open string an instrument's voice plays in
/// tune, semitones: an octave (a tuning an octave down) and the bend's whole tone under it.
/// Below, the loop's length is held at the longest the lines have.
pub const REACH_ST: f64 = 14.0;

/// r42 fix2-arcus — the period, in samples, a voice's lines hold at `sample_rate`: the
/// longest any instrument plays [`REACH_ST`] under its lowest open string at its own factor
/// (the bass's E1 at 1× — 2 616 samples at 48 kHz; the violin's G3 at 4× needs 2 199, the
/// viola's C3 at 2× and the cello's C2 at 1× 1 648). A voice can be any instrument, so its
/// lines are the longest of them, allocated when the instance is built.
pub fn reach(sample_rate: f32) -> usize {
    Instrument::ALL
        .iter()
        .flat_map(|i| (0..super::upper::tables::CROSS.len() as u8).map(move |c| (*i, c)))
        .map(|(i, c)| {
            let os = super::upper::factor(i) as f64;
            // r42 fiddle: a cross-tuning's lowest string (the violin's DDAD: D3) too.
            let low = i.spec_crossed(c).strings[0].open_hz() * math::exp2(-REACH_ST / 12.0);
            (os * sample_rate as f64 / low).ceil() as usize
        })
        .max()
        .unwrap_or(0)
}

/// The output's scale: the bridge's force (`2·Z₀·v`) to the instrument's level.
const OUT: f32 = 0.06;
/// The pluck's scale into the string (m/s per unit of Fides's pulse).
const PLUCK: f32 = 2.5;
/// The bow-hair noise's relative roughness at 0 dB.
const ROUGH: f32 = 0.02;

/// One voice.
#[derive(Clone, Debug)]
pub struct Voice {
    active: bool,
    ended: bool,
    released: bool,
    first: bool,
    key: u8,
    id: u64,
    velocity: f32,
    tech: NoteTech,
    inst: Instrument,
    pub string: BowedString,
    string_ix: usize,
    pluck: Exciter,
    rng: Rng,
    rough: Biquad,
    // ---- the bow ----
    pub(super) phase: Phase,
    bowing: bool,
    dir: f32,
    next_dir: f32,
    pub(super) v_mag: f32,
    pub(super) force: f32,
    accel: f32,
    f_attack: f32,
    /// r42:arcus-p5 — the attack is a re-bow's, on a ringing string.
    rebow: bool,
    /// r42 arcus-expr — the player's bow (`bow`): its place along the stick (0 the frog … 1
    /// the tip), how this stroke started (0 the classic attack, 1 from the string, 2 from
    /// the air) and an air start's landing force; the note's fingering (an open string) and
    /// the vibrato's factor for it, now and aimed at.
    bx: f32,
    kind: u8,
    land: f32,
    open: bool,
    vib_k: f32,
    vib_kn: f32,
    /// r42 arcus-classical — the stroke's shape (`shape`: the player's contour of the bow inside
    /// a held note) and the taper's length after the key (s).
    shape: super::shape::Shape,
    taper_s: f32,
    /// r42 arcus-classical — the vibrato's way back into a note inside a phrase: seconds since
    /// the bow change or the slur (negative: none).
    vib_re: f32,
    t: u32,
    stroke: f64,
    jitter: f32,
    // ---- the dynamic, the pitch ----
    d: f32,
    press: f32,
    has_press: bool,
    slide: f32,
    tuning: f32,
    /// r42 fix2-arcus — the track's tuning offset of the note's key (ADR 0155), semitones:
    /// set where the key becomes the note (its start, a slur), followed while the table moves
    /// (an MTS-ESP master's retuning) and held when the key's entry turns unmapped (`NAN`)
    /// under a held note — it sounds on at its last pitch.
    tune: f32,
    note: f32,
    glide: f32,
    glide_step: f32,
    vib_phase: f64,
    vib_age: u32,
    vib_wander: (f32, f32),
    /// r42:arcus-p5 — model 2's vibrato walk: the last cycle's and this cycle's values (rate,
    /// extent), glided between through the cycle; the note's own offsets.
    vib_from: (f32, f32),
    vib_to: (f32, f32),
    vib_note: (f32, f32),
    vib_now: f32,
    beta: f32,
    f_min: f32,
    f_max: f32,
    f0: f64,
    // ---- the step's buffers ----
    span: usize,
    pos: usize,
    v: [f32; SPAN],      // r42:arcus-upper
    fbuf: [f32; SPAN],   // r42:arcus-upper
    roughs: [f32; SPAN], // r42:arcus-upper
    strike: [f32; SPAN], // r42:arcus-upper
    sus: [f32; SPAN],    // r42:arcus-upper
    tmp: [f32; SPAN],    // r42:arcus-upper
    bow_on: bool,
    stop_lift: bool,
    /// The closed loop's trim on the table's correction, cents.
    pub(super) trim: f64,
    /// The pitch loop's ear (`ear`): the fundamental heard against the target.
    pub(super) ear: super::ear::Ear,
    /// The level's envelope (round 42 pass 2): samples since the note began, a sustain's
    /// swell (its length in samples and its starting gain), a slur's dip (where it began and
    /// its length), and the gains at the last and this control step.
    lvl_t: u32,
    swell: (f32, f32),
    dip: (u32, f32),
    lvl: (f32, f32),
    /// The register's balance on the output (`InstrumentSpec::balance_db_st`), linear.
    balance: f32,
    fade: f32,
    fade_step: f32,
    /// r42:arcus-p5 — the tremolo's level now (1 for any other stroke).
    trem_g: f32,
    peak: f32,
    /// The rate the voice runs at (the upper strings' `os ×`) and its way back. // r42:arcus-upper
    rate: upper::Rate, // r42:arcus-upper
    /// r42 fiddle (ADR 0164, `pair`) — the second string under the same bow, the first's
    /// share of the bow's force (last step, this step: exactly 1 without a pair), whether the
    /// first string's note is one the allocator never saw (a drone), and whether the first
    /// string starts afresh at the next step (the bow crossed onto a string at rest).
    sec: pair::Second,
    /// r42 fiddle — the strings' cross-tuning this voice plays in (`ArcusSettings::cross`).
    cross: u8,
    w1: (f32, f32),
    orphan: bool,
    restring: bool,
    /// Tests: every sample's slip, recorded while `record` is set.
    #[cfg(test)]
    pub record: bool,
    #[cfg(test)]
    pub slips: Vec<f32>,
}

impl Voice {
    pub fn new(sample_rate: f32) -> Self {
        Voice {
            active: false,
            ended: false,
            released: false,
            first: false,
            key: 60,
            id: 0,
            velocity: 0.0,
            tech: NoteTech::Arco,
            inst: Instrument::Cello,
            string: BowedString::with_reach(sample_rate, reach(sample_rate)),
            string_ix: 0,
            pluck: Exciter::new(sample_rate),
            rng: Rng(1),
            rough: Biquad::default(),
            phase: Phase::Off,
            bowing: false,
            dir: 1.0,
            next_dir: 1.0,
            v_mag: 0.0,
            force: 0.0,
            accel: 0.0,
            f_attack: 0.0,
            rebow: false,
            bx: 0.5,
            kind: 0,
            land: 0.0,
            open: false,
            vib_k: 1.0,
            vib_kn: 1.0,
            shape: super::shape::Shape::default(),
            taper_s: 0.0,
            vib_re: -1.0,
            t: 0,
            stroke: 0.0,
            jitter: 0.0,
            d: 0.5,
            press: 0.0,
            has_press: false,
            slide: 0.5,
            tuning: 0.0,
            tune: 0.0,
            note: 60.0,
            glide: 0.0,
            glide_step: 0.0,
            vib_phase: 0.0,
            vib_age: 0,
            vib_wander: (0.0, 0.0),
            vib_from: (0.0, 0.0),
            vib_to: (0.0, 0.0),
            vib_note: (0.0, 0.0),
            vib_now: 0.0,
            beta: 0.1,
            f_min: 0.0,
            f_max: 0.0,
            f0: 261.6,
            span: CTRL,
            pos: 0,
            v: [0.0; SPAN],      // r42:arcus-upper
            fbuf: [0.0; SPAN],   // r42:arcus-upper
            roughs: [0.0; SPAN], // r42:arcus-upper
            strike: [0.0; SPAN], // r42:arcus-upper
            sus: [0.0; SPAN],    // r42:arcus-upper
            tmp: [0.0; SPAN],    // r42:arcus-upper
            bow_on: false,
            stop_lift: false,
            trim: 0.0,
            ear: super::ear::Ear::default(),
            lvl_t: 0,
            swell: (0.0, 1.0),
            dip: (0, 0.0),
            lvl: (1.0, 1.0),
            balance: 1.0,
            fade: 1.0,
            fade_step: 0.0,
            trem_g: 1.0,
            peak: 0.0,
            rate: upper::Rate::new(sample_rate), // r42:arcus-upper
            sec: pair::Second::new(sample_rate), // r42 fiddle
            cross: 0,
            w1: (1.0, 1.0),
            orphan: false,
            restring: false,
            #[cfg(test)]
            record: false,
            #[cfg(test)]
            slips: Vec::new(),
        }
    }

    /// r42 fiddle — the instrument's table in the voice's cross-tuning (its own table at 0).
    pub fn spec(&self) -> &'static InstrumentSpec {
        self.inst.spec_crossed(self.cross)
    }

    pub fn active(&self) -> bool {
        self.active || self.rate.drain > 0 // r42:arcus-upper
    }
    pub fn key(&self) -> u8 {
        self.key
    }
    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn released(&self) -> bool {
        self.released
    }
    pub fn fade(&self) -> f32 {
        self.fade
    }
    pub fn peak(&self) -> f32 {
        self.peak
    }
    pub fn is_plucked(&self) -> bool {
        self.tech == NoteTech::Pizzicato
    }
    pub fn technique(&self) -> NoteTech {
        self.tech
    }
    pub fn phase(&self) -> Phase {
        self.phase
    }
    /// The bow's velocity now (m/s, signed: + down-bow).
    pub fn bow_velocity(&self) -> f32 {
        self.dir * self.v_mag
    }
    pub fn bow_force(&self) -> f32 {
        self.force
    }
    /// Schelleng's limits for the bowing now, N.
    pub fn limits(&self) -> (f32, f32) {
        (self.f_min, self.f_max)
    }
    pub fn beta(&self) -> f32 {
        self.beta
    }
    pub fn dynamic(&self) -> f32 {
        self.d
    }
    pub fn string_index(&self) -> usize {
        self.string_ix
    }
    pub fn f0(&self) -> f64 {
        self.f0
    }
    /// The vibrato's excursion now, cents.
    pub fn vibrato(&self) -> f32 {
        self.vib_now
    }
    /// Whether the bow holds the string now.
    pub fn sticking(&self) -> bool {
        self.string.is_sticking()
    }

    /// Whether the bow is on the string in this step.
    pub fn bow_on(&self) -> bool {
        self.bow_on
    }

    /// r42 arcus-expr — where the bow is along the stick (0 the frog … 1 the tip).
    pub fn bow_x(&self) -> f32 {
        self.bx
    }

    /// r42 arcus-expr — whether the note is played on an open string.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// r42 arcus-expr — which way the bow moves now, when it is drawn in a sustained stroke (a
    /// tremolo's and a spiccato's strokes turn by themselves: none).
    pub fn bow_moving(&self) -> Option<BowDir> {
        let sustained = matches!(self.tech, NoteTech::Arco | NoteTech::Staccato);
        (self.active && self.bowing && sustained).then_some(if self.dir >= 0.0 {
            BowDir::Down
        } else {
            BowDir::Up
        })
    }

    /// It ended since the last call (the instance hands its slot back).
    pub fn take_ended(&mut self) -> bool {
        std::mem::take(&mut self.ended)
    }

    /// Silence at once and forget everything.
    pub fn clear(&mut self) {
        self.active = false;
        self.rate.drain = 0; // r42:arcus-upper
        self.ended = false;
        self.released = false;
        self.string.clear();
        self.pluck.clear();
        self.rough.clear();
        self.phase = Phase::Off;
        self.bowing = false;
        self.v_mag = 0.0;
        self.force = 0.0;
        self.bow_on = false;
        self.stop_lift = false;
        self.fade = 1.0;
        self.fade_step = 0.0;
        self.peak = 0.0;
        self.glide = 0.0;
        self.glide_step = 0.0;
        self.has_press = false;
        // r42 fiddle: the second string and the pair's state.
        self.sec.clear();
        self.w1 = (1.0, 1.0);
        self.orphan = false;
        self.restring = false;
    }

    /// Fade out over 4 ms (taken by a new note, a choke), then end.
    pub fn fade_out(&mut self, sr: f32) {
        if self.active {
            self.fade_step = 1.0 / (0.004 * sr * self.rate.os as f32); // r42:arcus-upper
        }
    }

    /// A per-note expression: `0` tuning (semitones), `1` pressure, `2` slide (0 … 1).
    pub fn expression(&mut self, which: u8, value: f32) {
        match which {
            0 => self.tuning = value,
            1 => {
                self.press = value.clamp(0.0, 1.0);
                self.has_press = true;
            }
            2 => self.slide = value.clamp(0.0, 1.0),
            _ => {}
        }
    }

    /// The key is let go: the bow lifts (a short stroke ends by itself).
    pub fn release(&mut self) {
        self.sec.held = false; // r42 fiddle: the bow leaves both strings
        if !self.active || self.released {
            return;
        }
        self.released = true;
        // r42 arcus-classical: with the player, a held stroke tapers before the bow lifts
        // (`shape`); the step lifts it when the taper has run.
        let taper = self.tech == NoteTech::Arco
            && self.taper_s > 0.0
            && matches!(self.phase, Phase::Attack | Phase::Hold);
        if taper {
            self.shape.taper(self.taper_s);
        } else if matches!(self.phase, Phase::Attack | Phase::Hold | Phase::Change) {
            self.phase = Phase::Release;
            self.t = 0;
        }
        self.pluck.release();
    }
}
