//! Loquax, the vocoder (ADR 0136; the name: [`NAME`]) — Latin *loquax*,
//! talkative.
//!
//! A voice (the **modulator**) speaks through another sound (the **carrier**): the
//! modulator is split into 8 … 64 bands, each band's level is followed, and those levels
//! shape the same bands of the carrier. The carrier is the sidechain (another strip), the
//! built-in polyphonic synthesiser played by notes ([`carrier`]), or both; `swap` takes the
//! strip's own sound as the carrier and the sidechain as the voice.
//!
//! - **The banks** ([`bank`]): Butterworth band-passes of order 4, 6 or 8, linear
//!   zero-delay-feedback sections, log-spaced between `lo` and `hi`, `width` spacings wide.
//!   No look-ahead: **latency 0**.
//! - **Followers** per band (attack and release; no band follows faster than half a period
//!   up or two periods down, so the low bands do not ripple), and **hold**, which freezes
//!   them — the vowel sung on.
//! - **Formant shift** retunes the synthesis bank against the analysis bank (semitones):
//!   the voice's formants move, its words stay. **Band shift** maps each synthesis band to
//!   a modulator band so many bands away (fractional).
//! - **Flatten** normalises the carrier band by band, so a dull pad speaks as clearly as a
//!   bright saw; at 0 the carrier keeps its own spectrum, made up to a steady level.
//! - **Consonants** ([`voicing`]): the sibilance pass-through and the unvoiced noise.
//! - **Tilt** (dB per octave about 1 kHz), per-band levels (the display's drawn bars),
//!   **stereo spread** (bands alternately left and right), the raw carrier's pass-through.
//! - **Robot**: the synthesiser drones one note while the voice speaks, no notes needed.
//!
//! **The null**: at mix 0 (the default) nothing runs but the output gain, which at 0 dB is
//! not applied — every sample passes untouched. Turned up from 0, the banks start empty and
//! the synthesiser starts the keys already held.
//!
//! **At rest** (ADR 0031 §5.1): the mix and the gain still and, while engaged, every bank,
//! follower and detector state +0.0, no voice sounding, no key held, every glide landed.
//! Nothing counts samples at rest, so [`Loquax::skip`] has nothing to move.

#[path = "loquax_bank.rs"]
pub mod bank;
#[path = "loquax_carrier.rs"]
pub mod carrier;
#[path = "loquax_measure.rs"]
pub mod measure;
#[path = "loquax_run.rs"]
mod run;
#[path = "loquax_voicing.rs"]
pub mod voicing;

use crate::dynfx_follow::{Follower, Times};
use crate::math;
use crate::modfx_line::Glide;
use bank::{Bank, MAX_BANDS, MIN_BANDS};
use carrier::{Synth, SynthParams};
use voicing::Voicing;

pub use carrier::{NoteEvent, NoteKind};

/// The unit's name, in one place (approved by the owner, 2026-11-17).
pub const NAME: &str = "Loquax";

/// Where the carrier comes from.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Source {
    /// The sidechain (another strip routed to the unit).
    Sidechain,
    /// The built-in synthesiser, played by the notes routed to the unit.
    #[default]
    Synth,
    /// Both, summed.
    Both,
}

/// The orders a band may have.
pub const ORDERS: [usize; 3] = [4, 6, 8];
/// The analysis range's limits, Hz.
pub const LO_MIN: f32 = 40.0;
pub const LO_MAX: f32 = 1_000.0;
pub const HI_MIN: f32 = 1_500.0;
pub const HI_MAX: f32 = 18_000.0;

/// Everything Loquax is set to.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LoquaxSettings {
    /// Dry (the strip's own sound) to vocoded, 0 … 1.
    pub mix: f32,
    pub out_db: f32,
    pub source: Source,
    /// The strip's sound is the carrier and the sidechain the voice.
    pub swap: bool,
    /// 8 … 64.
    pub bands: usize,
    /// The lowest and highest band centres, Hz.
    pub lo_hz: f32,
    pub hi_hz: f32,
    /// Each band's width in spacings (0.5 … 2; 1: neighbours cross at −3 dB).
    pub width: f32,
    /// 4, 6 or 8.
    pub order: usize,
    pub attack_ms: f32,
    pub release_ms: f32,
    /// Semitones the synthesis bank is moved (−24 … 24).
    pub formant: f32,
    /// Bands the modulator's levels are moved by (−16 … 16, fractional).
    pub band_shift: f32,
    /// dB per octave about 1 kHz (−6 … 6).
    pub tilt: f32,
    /// 0 … 1.
    pub flatten: f32,
    /// The sibilance pass-through's level (0 … 1) and corner (Hz).
    pub sibilance: f32,
    pub sib_hz: f32,
    /// The pass-through only while the detector hears a consonant.
    pub sib_gated: bool,
    /// Noise into the carrier while the detector hears a consonant (0 … 1).
    pub unvoiced: f32,
    /// The band followers frozen.
    pub hold: bool,
    /// Bands alternately left and right (0 … 1).
    pub spread: f32,
    /// The raw carrier added to the output (0 … 1).
    pub carrier_pass: f32,
    /// Each band's level, dB (−24 … 12), by band index.
    pub band_db: [f32; MAX_BANDS],
    pub synth: SynthParams,
    /// The synthesiser drones `robot_note` while the voice is above `robot_gate_db`.
    pub robot: bool,
    pub robot_note: f32,
    pub robot_gate_db: f32,
}

impl Default for LoquaxSettings {
    fn default() -> Self {
        LoquaxSettings {
            mix: 0.0,
            out_db: 0.0,
            source: Source::Synth,
            swap: false,
            bands: 32,
            lo_hz: 100.0,
            hi_hz: 8_000.0,
            width: 1.0,
            order: 6,
            attack_ms: 2.0,
            release_ms: 40.0,
            formant: 0.0,
            band_shift: 0.0,
            tilt: 0.0,
            flatten: 1.0,
            sibilance: 0.5,
            sib_hz: 5_000.0,
            sib_gated: true,
            unvoiced: 0.5,
            hold: false,
            spread: 0.0,
            carrier_pass: 0.0,
            band_db: [0.0; MAX_BANDS],
            synth: SynthParams::default(),
            robot: false,
            robot_note: 48.0,
            robot_gate_db: -45.0,
        }
    }
}

const SNAP: f32 = 1e-6;
/// While a tuning glide moves, the banks are retuned every this many samples (and once
/// more where it lands).
pub const RETUNE: u32 = 64;
/// The carrier followers' times (attack 0: the normalised carrier never exceeds 1).
const CARRIER_RELEASE_MS: f32 = 30.0;
/// The level made up so the vocoded output is about the modulator's (flatten 1).
pub const MAKEUP: f32 = core::f32::consts::SQRT_2;
/// The flattening's largest gain (+60 dB) and its floor.
const FLAT_CAP: f32 = 1_000.0;
const FLAT_FLOOR: f32 = 1e-4;
const NOISE_SEED: u64 = 0x10C0_0015_E000;

/// Loquax's telemetry, after the family's header ([`crate::modfx_clock::tel`]).
pub mod t {
    use crate::modfx_clock::tel::OWN;
    /// Bands in use.
    pub const BANDS: usize = OWN;
    /// 1 while the vocoder runs (the mix above 0).
    pub const ENGAGED: usize = OWN + 1;
    /// The unvoiced gate (0 … 1) and the detector's measure (0 … 1).
    pub const GATE: usize = OWN + 2;
    pub const UNVOICED: usize = OWN + 3;
    /// Voices sounding.
    pub const VOICES: usize = OWN + 4;
    /// 1 while the followers are held.
    pub const HOLD: usize = OWN + 5;
    pub const LATENCY: usize = OWN + 6;
    /// The modulator's and the carrier's levels, dBFS (−200 for silence).
    pub const MOD_DB: usize = OWN + 7;
    pub const CARRIER_DB: usize = OWN + 8;
    /// The formant shift (semitones) and the band shift (bands) now.
    pub const FORMANT: usize = OWN + 9;
    pub const SHIFT: usize = OWN + 10;
    /// 1 while the robot's drone sounds.
    pub const ROBOT: usize = OWN + 11;
}

/// What the display draws: per band, the centres and the levels of the last block.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct BandView {
    pub bands: usize,
    /// The analysis and synthesis centres, Hz.
    pub analysis_hz: [f32; MAX_BANDS],
    pub synthesis_hz: [f32; MAX_BANDS],
    /// The modulator's band levels (after hold), the carrier's, and the output's, dBFS.
    pub modulator_db: [f32; MAX_BANDS],
    pub carrier_db: [f32; MAX_BANDS],
    pub output_db: [f32; MAX_BANDS],
}

impl Default for BandView {
    fn default() -> Self {
        BandView {
            bands: 0,
            analysis_hz: [0.0; MAX_BANDS],
            synthesis_hz: [0.0; MAX_BANDS],
            modulator_db: [-200.0; MAX_BANDS],
            carrier_db: [-200.0; MAX_BANDS],
            output_db: [-200.0; MAX_BANDS],
        }
    }
}

/// The glides, by role.
#[derive(Clone, Debug)]
struct Glides {
    mix: Glide,
    out: Glide,
    /// log₂ Hz.
    lo: Glide,
    hi: Glide,
    width: Glide,
    formant: Glide,
    shift: Glide,
    tilt: Glide,
    spread: Glide,
    flatten: Glide,
    sibilance: Glide,
    unvoiced: Glide,
    carrier: Glide,
}

impl Glides {
    fn new(sr: f32) -> Glides {
        let g = |v: f32, ms: f32| Glide::new(v, ms, sr, SNAP);
        Glides {
            mix: g(0.0, 10.0),
            out: g(1.0, 10.0),
            lo: g(math::log2(100.0f32), 30.0),
            hi: g(math::log2(8_000.0f32), 30.0),
            width: g(1.0, 30.0),
            formant: g(0.0, 30.0),
            shift: g(0.0, 30.0),
            tilt: g(0.0, 30.0),
            spread: g(0.0, 30.0),
            flatten: g(1.0, 10.0),
            sibilance: g(0.5, 10.0),
            unvoiced: g(0.5, 10.0),
            carrier: g(0.0, 10.0),
        }
    }

    /// The glides that retune the banks or their gains.
    fn tuning(&mut self) -> [&mut Glide; 6] {
        [
            &mut self.lo,
            &mut self.hi,
            &mut self.width,
            &mut self.formant,
            &mut self.tilt,
            &mut self.spread,
        ]
    }

    fn tuning_still(&self) -> bool {
        [
            &self.lo,
            &self.hi,
            &self.width,
            &self.formant,
            &self.tilt,
            &self.spread,
        ]
        .iter()
        .all(|g| g.still())
    }

    /// Every glide but the mix and the gain.
    fn inner(&mut self) -> [&mut Glide; 11] {
        [
            &mut self.lo,
            &mut self.hi,
            &mut self.width,
            &mut self.formant,
            &mut self.shift,
            &mut self.tilt,
            &mut self.spread,
            &mut self.flatten,
            &mut self.sibilance,
            &mut self.unvoiced,
            &mut self.carrier,
        ]
    }

    fn inner_still(&self) -> bool {
        self.tuning_still()
            && [
                &self.shift,
                &self.flatten,
                &self.sibilance,
                &self.unvoiced,
                &self.carrier,
            ]
            .iter()
            .all(|g| g.still())
    }
}

#[derive(Clone, Debug)]
pub struct Loquax {
    sr: f32,
    set: LoquaxSettings,
    gl: Glides,
    n: usize,
    m: usize,
    ana: Bank,
    syn: [Bank; 2],
    env_m: [Follower; MAX_BANDS],
    t_m: [Times; MAX_BANDS],
    env_c: [Follower; MAX_BANDS],
    t_c: Times,
    /// The bands' output gains, left and right: where they glide to and where they are.
    gain_to: [[f32; MAX_BANDS]; 2],
    gain: [[f32; MAX_BANDS]; 2],
    gain_k: f32,
    centres_a: [f32; MAX_BANDS],
    centres_s: [f32; MAX_BANDS],
    voicing: Voicing,
    synth: Synth,
    /// The robot's ear: the modulator's peak level, and whether the drone sounds.
    level: Follower,
    t_level: Times,
    robot_open: bool,
    /// The unvoiced noise's sample count (0 while the gate is shut).
    noise_n: i64,
    /// Samples since the last retune while a tuning glide moves.
    retune_n: u32,
    /// What the analysis and synthesis banks were last designed for.
    key_a: [u32; 7],
    key_s: [u32; 6],
    tuning_moving: bool,
    engaged: bool,
    /// The levels the display reads (the last sample of the last block).
    amp: [f32; MAX_BANDS],
    tel: [f32; 32],
    blocks: u32,
}

impl Loquax {
    pub fn new(sample_rate: f32) -> Loquax {
        let set = LoquaxSettings::default();
        let mut q = Loquax {
            sr: sample_rate,
            set,
            gl: Glides::new(sample_rate),
            n: 0,
            m: 0,
            ana: Bank::default(),
            syn: [Bank::default(), Bank::default()],
            env_m: [Follower::default(); MAX_BANDS],
            t_m: [Times::new(1.0, 40.0, sample_rate); MAX_BANDS],
            env_c: [Follower::default(); MAX_BANDS],
            t_c: Times::new(0.0, CARRIER_RELEASE_MS, sample_rate),
            gain_to: [[0.0; MAX_BANDS]; 2],
            gain: [[0.0; MAX_BANDS]; 2],
            gain_k: crate::dynfx_follow::coef(5.0, sample_rate),
            centres_a: [0.0; MAX_BANDS],
            centres_s: [0.0; MAX_BANDS],
            voicing: Voicing::new(sample_rate),
            synth: Synth::new(sample_rate),
            level: Follower::default(),
            t_level: Times::new(1.0, 120.0, sample_rate),
            robot_open: false,
            noise_n: 0,
            retune_n: 0,
            key_a: [u32::MAX; 7],
            key_s: [u32::MAX; 6],
            tuning_moving: false,
            engaged: false,
            amp: [0.0; MAX_BANDS],
            tel: [0.0; 32],
            blocks: 0,
        };
        q.set(&set);
        q.reset();
        q
    }

    pub fn settings(&self) -> &LoquaxSettings {
        &self.set
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    /// The highest centre the analysis bank may take at this sample rate.
    fn hi_cap(&self) -> f32 {
        HI_MAX.min(0.42 * self.sr)
    }

    /// New settings: allocation-free. A new band count or order clears the banks.
    pub fn set(&mut self, s: &LoquaxSettings) {
        let mut s = *s;
        s.bands = s.bands.clamp(MIN_BANDS, MAX_BANDS);
        if !ORDERS.contains(&s.order) {
            s.order = 6;
        }
        s.lo_hz = s.lo_hz.clamp(LO_MIN, LO_MAX);
        let cap = self.hi_cap();
        if cap >= HI_MIN {
            s.hi_hz = s.hi_hz.clamp(HI_MIN, cap);
        } else {
            // A host's rate too low for the bank's range (a validator's 1 234.6 Hz): the bank
            // squeezed under its Nyquist, its lowest band under its highest. Every rate of
            // 3.6 kHz and up takes the branch above, unchanged.
            s.hi_hz = cap;
            s.lo_hz = s.lo_hz.min(cap * 0.5);
        }
        let g = &mut self.gl;
        g.mix.target = s.mix.clamp(0.0, 1.0);
        g.out.target = if s.out_db == 0.0 {
            1.0
        } else {
            math::db_to_gain(s.out_db.clamp(-60.0, 24.0))
        };
        g.lo.target = math::log2(s.lo_hz);
        g.hi.target = math::log2(s.hi_hz);
        g.width.target = s.width.clamp(0.5, 2.0);
        g.formant.target = s.formant.clamp(-24.0, 24.0);
        g.shift.target = s.band_shift.clamp(-16.0, 16.0);
        g.tilt.target = s.tilt.clamp(-6.0, 6.0);
        g.spread.target = s.spread.clamp(0.0, 1.0);
        g.flatten.target = s.flatten.clamp(0.0, 1.0);
        g.sibilance.target = s.sibilance.clamp(0.0, 1.0);
        g.unvoiced.target = s.unvoiced.clamp(0.0, 1.0);
        g.carrier.target = s.carrier_pass.clamp(0.0, 1.0);
        self.voicing.set_sibilance_hz(s.sib_hz);
        self.synth.set(&s.synth);
        let relayout = s.bands != self.n || s.order / 2 != self.m;
        let regain = s.band_db != self.set.band_db
            || s.attack_ms != self.set.attack_ms
            || s.release_ms != self.set.release_ms;
        self.set = s;
        if relayout {
            self.n = s.bands;
            self.m = s.order / 2;
            for f in self.env_m.iter_mut().chain(self.env_c.iter_mut()) {
                f.reset();
            }
            self.retune();
            self.ana.reset();
            for b in &mut self.syn {
                b.reset();
            }
        } else if regain {
            self.retune();
        }
        if !self.set.robot {
            self.robot_open = false;
            self.synth.drone(false, 0.0);
        }
        self.settle_engaged();
    }

    /// Whether the vocoder runs; disengaged, the inner glides jump to their targets and
    /// everything waits empty (the held keys apart).
    fn settle_engaged(&mut self) {
        let off = self.gl.mix.value == 0.0 && self.gl.mix.target == 0.0;
        if off {
            let moved = !self.gl.inner_still() || self.tuning_moving;
            for g in self.gl.inner() {
                g.jump(g.target);
            }
            self.tuning_moving = false;
            self.retune_n = 0;
            if moved {
                self.retune();
            }
            if self.engaged {
                self.clear_wet();
            }
            self.gain = self.gain_to;
        } else if !self.engaged {
            self.clear_wet();
            self.synth.start_held();
        }
        self.engaged = !off;
    }

    fn clear_wet(&mut self) {
        self.ana.reset();
        for b in &mut self.syn {
            b.reset();
        }
        for f in self.env_m.iter_mut().chain(self.env_c.iter_mut()) {
            f.reset();
        }
        self.voicing.reset();
        self.synth.silence();
        self.level.reset();
        self.robot_open = false;
        self.noise_n = 0;
        self.amp = [0.0; MAX_BANDS];
    }

    pub fn reset(&mut self) {
        self.gl.mix.jump(self.gl.mix.target);
        self.gl.out.jump(self.gl.out.target);
        for g in self.gl.inner() {
            g.jump(g.target);
        }
        self.tuning_moving = false;
        self.retune_n = 0;
        self.retune();
        self.gain = self.gain_to;
        self.clear_wet();
        self.synth.reset();
        self.engaged = self.gl.mix.value != 0.0;
        self.tel = [0.0; 32];
        self.blocks = 0;
    }

    /// No look-ahead: the banks are minimum-phase IIR filters.
    pub fn latency(&self) -> usize {
        0
    }

    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    pub fn rest_blocker(&self) -> Option<&'static str> {
        if !self.gl.mix.still() || !self.gl.out.still() {
            return Some("the mix or the gain is gliding");
        }
        if self.synth.any_held() {
            return Some("a key is held");
        }
        if !self.engaged {
            return None;
        }
        if !self.gl.inner_still() || self.tuning_moving {
            return Some("a glide is moving");
        }
        if self.gain != self.gain_to {
            return Some("a band's level is gliding");
        }
        if !self.ana.at_rest() || !self.syn.iter().all(Bank::at_rest) {
            return Some("a band rings");
        }
        let n = self.n;
        if !self.env_m[..n]
            .iter()
            .chain(self.env_c[..n].iter())
            .all(Follower::is_zero)
        {
            return Some("a band's follower is falling");
        }
        if !self.voicing.at_rest() {
            return Some("the consonant detector is settling");
        }
        if !self.synth.at_rest() || self.robot_open || !self.level.is_zero() {
            return Some("a voice sounds");
        }
        if self.noise_n != 0 || !crate::rest::all_zero(&self.amp[..n]) {
            return Some("the unvoiced noise runs");
        }
        None
    }

    /// `frames` of silence skipped at rest: nothing in Loquax counts samples at rest.
    pub fn skip(&mut self, frames: usize) {
        let _ = frames;
    }

    /// The bands as the display draws them.
    pub fn bands(&self, out: &mut BandView) {
        let n = self.n;
        out.bands = n;
        let db = crate::dynfx_follow::to_db;
        for b in 0..n {
            out.analysis_hz[b] = self.centres_a[b];
            out.synthesis_hz[b] = self.centres_s[b];
            out.modulator_db[b] = db(self.amp[b]);
            out.carrier_db[b] = db(self.env_c[b].value().sqrt());
            let g = 0.5 * (self.gain[0][b] + self.gain[1][b]);
            out.output_db[b] = db(self.mapped_amp(b) * g);
        }
    }

    /// The telemetry: the family's header and Loquax's own ([`t`]).
    pub fn telemetry(&self, out: &mut [f32; 64]) {
        out[..32].copy_from_slice(&self.tel);
    }
}

#[cfg(test)]
#[path = "loquax_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "loquax_tests2.rs"]
mod tests2;
