//! One sounding key of Chorda: its strings struck, its damper rising and falling with the
//! key and the pedal, and the phantom partials of its strings' tension.
//!
//! **The damper** rests on the strings with a contact between 0 (lifted) and 1 (down),
//! which the key (held: lifted) and the sustain pedal set: the pedal lifts the dampers
//! progressively over its travel (`1 − smoothstep(0.15, 0.65, pedal)`), so half-pedalling
//! is continuous. The contact follows its target over ~8 ms (felt does not land in a
//! sample) and adds `contact · σ_d` to every partial's decay rate, `σ_d` the felt's grip:
//! a damped string dies in about 1.5 s at A0, 0.3 s at C4 and 0.15 s at E6. Keys above E6
//! have no damper, as on a grand.
//!
//! **Phantom partials** (Bank & Sujbert 2005): a string's tension rises with the square of
//! its slope, `ΔT ≈ (EA / 2c²) v²` for the transverse velocity `v`; the square holds the
//! sums and differences of the partials, which the tension carries to the bridge, through
//! the string's first longitudinal mode (`c_L / 2L`, steel's `c_L` = 5 100 m/s) and
//! directly. `EA` from the wire's diameter, itself from `B = π³ E d⁴ / (64 T L²)`.

use super::design::LoopDesign;
use super::noise::{self, Thud};
use super::string::{loss_steep, Felt, KeyStrings, MAX_STRINGS};
use super::tables::Key;
use crate::math;

/// The control step (samples): the damper and the loss filter move at this rate, counted
/// from the voice's own start.
pub const CONTROL: u32 = 32;
/// Control steps of near-silence (and no hammer) after which a voice ends.
const QUIET_STEPS: u32 = 64;
/// Below this force (N) a step is quiet.
const QUIET_N: f64 = 1.0e-10;
/// Steel: Young's modulus (Pa) and longitudinal wave speed (m/s).
const STEEL_E: f64 = 2.0e11;
const STEEL_CL: f64 = 5_100.0;
/// How much of the tension's swing reaches the bridge as a vertical force (the bridge's
/// rocking under the strings' pull): a tenth.
const LONGITUDINAL_COUPLING: f64 = 0.1;

/// What a strike and a voice's life take from the instrument's settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoiceSettings {
    pub felt: Felt,
    /// Strike velocity range (m/s) and the curve (1: exponential in the MIDI velocity).
    pub v_min: f64,
    pub v_max: f64,
    pub vel_curve: f64,
    /// The decay against the measured one (> 1: longer), the bridge's share of `b₁`.
    pub decay: f64,
    pub coupling: f64,
    /// The unison strings' spread (cents) and the damper's grip (0…1, 1: a grand's).
    pub detune_ct: f64,
    pub damper: f64,
    /// The phantom partials' level (1: the physics').
    pub phantom: f64,
    /// The soft pedal's reach (0…1) and how far it is pressed now (0…1).
    pub una_depth: f64,
    /// Model 2 (ADR 0098), all 0 in model 1: the hammer's blow carried into the frame (a
    /// share of its force), the key's thud and the damper's landing (1: as voiced), and how
    /// much each strike varies from the last (1: as voiced).
    pub blow: f64,
    pub knock: f64,
    pub landing: f64,
    pub vary: f64,
    /// Model 2: the strings' decay laws scaled — `b₁` (by register, set per strike) and
    /// `b₂` (the frequency-dependent loss) divided by these (1: as measured, model 1).
    pub b1_scale: f64,
    pub b2_scale: f64,
    /// Below this force (N) a control step is quiet (model 1: 1e-10; model 2 higher, so
    /// its longer decays still end).
    pub quiet: f64,
    /// Model 2 (round 40): a released key whose damper has landed is done once its strings
    /// fall this far under the strike's loudest (a share, 0: never — model 1). On the lowest
    /// strings one loop mode near a dispersion section's resonance barely loses a second
    /// (A0's 1.4 kHz partial: −79 dB under its peak, then 0.25 dB/s for minutes); a damped
    /// string has nothing audible left there.
    pub damped_floor: f64,
    /// Model 2 (round 40): the damped strings' T60 scaled (1: `damped_t60`'s, model 1's) —
    /// the reference's middle C still sounds 17 dB more after its release than beta.14's.
    pub damper_t60: f64,
    /// Model 2 (round 40), all 0 in model 1 and beta.14: a key's unison is never three
    /// identical strings — the hammer meets them unevenly (`uni_weight`: each string's share
    /// of the blow scattered by up to this, fixed per key) and their tunings are not a
    /// symmetric fan (`uni_skew`: each string's detune moved by up to this share of the
    /// spread, per key). Both keep the beats from cancelling to silence and feed the strings'
    /// slow (antisymmetric) mode: the aftersound.
    pub uni_weight: f64,
    pub uni_skew: f64,
    /// Model 2 (round 40): 1 — the unison's shares set by rank (the leader whole, then
    /// `1 − 0.75·uni_weight`, `1 − uni_weight`), the hash picking only the order; 0: each
    /// share hashed.
    pub uni_rank: f64,
    /// Model 2 (round 40): keys of one or two strings lay one horizontal-polarisation loop
    /// (`string.rs`), struck at `polar` of the blow, detuned `polar_ct` cents, on a bridge
    /// side `polar_yz` times as yielding. 0: none (model 1, beta.14).
    pub polar: f64,
    pub polar_ct: f64,
    pub polar_yz: f64,
    /// How firmly the damper grips the horizontal loop against the vertical strings (1:
    /// as firmly; < 1: the release's slow second stage).
    pub polar_damp: f64,
    /// 1: the last string of a three-string unison is gripped by its damper as the
    /// horizontal loop is (`polar_damp`) — the release's slow second stage in the middle
    /// and treble. 0: as firmly as the others.
    pub loose_last: f64,
    /// Model 2 (round 40): the hammer's mass scaled (1: the key's measured hammer) — a
    /// heavier hammer stays longer on the strings, and a longer contact takes the upper
    /// partials of a mezzo-forte strike down (the reference's middle C sings on its first
    /// two partials; Chorda's 3rd to 9th stood 8-14 dB over them).
    pub mass: f64,
    /// Model 2 (round 40): the key's own voicing of its partials — partials 2 … 7 raised or
    /// lowered (dB) against the fundamental, as a technician needles each hammer apart (the
    /// reference's middle C sings on its first two partials, its third 20 dB under them; a
    /// model's keys are otherwise all alike). All 0: none.
    pub partials: [f64; PARTIALS],
    /// Model 2 (round 40): the key's evening — its strings' gain (1: none).
    pub level: f64,
    /// Model 2 (round 40): the action's thud's gain (1: none).
    pub thud_level: f64,
    /// Model 2 (round 40): the bridge's resonances near the key — how many (0: none), their
    /// Q, their gain against the bridge's `YZ`, and the span they fall in (multiples of the
    /// key's first partial).
    pub bridge_n: f64,
    pub bridge_q: f64,
    pub bridge_g: f64,
    pub bridge_span: f64,
    /// Model 2 (round 40): how far a unison's strings' stiffness scatters (their
    /// dispersion's pole angles, ± this share; 0: alike).
    pub uni_b: f64,
    /// Model 2 (round 40): 1 — each string's first partial held where its detune put it
    /// as its stiffness scatters (0: not held).
    pub uni_b_hold: f64,
    /// Model 2 (round 40, `DSP_VERSION` 10): 1 — the unison's detunes and stiffness scatter
    /// centred on the key, weighed by the strings' power (`Voicing::uni_center`; 0: not).
    pub uni_center: f64,
    /// Model 2 (round 40, `DSP_VERSION` 10): 1 — the strings tuned with the bridge's load
    /// on them (`Voicing::bridge_tune`; 0: not).
    pub bridge_tune: f64,
    /// Model 2 (round 40, `DSP_VERSION` 10): with `bridge_tune`, the most (cents) a bridge
    /// resonance may pull one of a key's first three partials (`Voicing::bridge_pull`; 0:
    /// no limit).
    pub bridge_pull: f64,
    /// Model 2 (round 41, `DSP_VERSION` 11): with `bridge_tune`, the most (Hz) a bridge
    /// resonance may pull one of a key's first three partials — the beat its in-phase mode
    /// keeps against the opposed ones (`Voicing::bridge_pull_hz`; 0: no limit in hertz).
    pub bridge_pull_hz: f64,
    /// Model 2 (round 41, `DSP_VERSION` 11): 1 — a three-string unison laid as two strings
    /// alike and the third apart by the fan's whole width, no skew (`Voicing::uni_pair`; 0:
    /// the designed fan). Two strings alike share one mode the bridge never hears; the third
    /// makes one slow mode it does, which a slight detune locks to the strings' pitch on
    /// their bridge (Weinreich 1977): a second stage with no beat in it, where a fan of three
    /// rings two slow modes that beat against each other.
    pub uni_pair: f64,
    /// Model 2 (round 40): how far a unison's strings' losses scatter (± this share; 0:
    /// alike).
    pub uni_loss: f64,
    /// Model 2 (round 40): the key's thud's speed exponent beyond the model's (0: as it is;
    /// > 0 quieter when soft — at the hardest strike unchanged).
    pub knock_vel: f64,
    /// Model 2 (round 40): the damper's landing — its noise's fall (s; 0: the model's 6 ms),
    /// its band's top (Hz; 0: 2 kHz), its modes' share (1: as is), and how far it follows
    /// the note's own level as it lands (0: not at all).
    pub landing_tau: f64,
    pub landing_hi: f64,
    pub landing_modes: f64,
    pub landing_follow: f64,
    /// Model 2 (round 40): a key with no damper still thumps softly as it comes up — the key
    /// on its rail, the action settling (the damper landing's thud, at this share). 0: silent.
    pub key_return: f64,
    /// Model 2 (round 40): the strings' rise (ms; 0: at once) — a silent key's tone builds
    /// up over this time constant (`1 − e^{−t/τ}`), the action's noises not.
    pub rise_ms: f64,
    /// Model 2 (round 40): a key with no damper is damped at this share of a damper's grip
    /// once released (0: never, as a grand's top keys).
    pub top_damper: f64,
    /// Model 2 (round 40, candidate 10): the damper's time scale when it lands as the key
    /// comes up with the pedal already up (0: `damper_t60`, as after the pedal) — a single
    /// note's end apart from a pedalled phrase's.
    pub damper_key: f64,
    /// Model 2 (round 40, candidate 7): the loss's rise above the first partial, a factor on
    /// the law's (1: the law; [`super::string::loss_steep`]).
    pub hf_steep: f64,
    /// Model 2 (round 40, candidate 11): the strike point against the loop's whole period
    /// (0: the delay lines' alone, beta.14's; 1: true; [`super::string::KeyStrings::nut_samples`]).
    pub strike_true: f64,
    /// Model 2 (round 40, candidate 13): the key's draw (0: its own; `Voicing::draws`).
    pub draw: u8,
    /// Model 2 (round 40, candidate 13): the felt's stiffness scale at velocity 75 and under,
    /// faded to none at 115 (1: none; `Voicing::felt_soft`).
    pub felt_soft: f64,
    /// Model 2 (round 40): the damper landing's noise rises over this share of its fall (0:
    /// at once).
    pub landing_rise: f64,
    /// Model 2: what this strike's variation and noises are seeded from — the note's id
    /// (a clip note's own, the same on every play, live or offline, from any start; ADR
    /// 0023 rule 1).
    pub seed: u64,
}

/// Model 2's voicing of the action (ADR 0098), calibrated by `chorda_compare`.
pub mod action {
    /// The key bottoming: its two modes (Hz, T60 s) and their kicks (N at 1 m/s, growing as
    /// the speed to the 1.5), its noise (N at 1 m/s, as the speed squared).
    pub const KEY_HZ: [f64; 2] = [72.0, 185.0];
    pub const KEY_T60: [f64; 2] = [0.06, 0.04];
    pub const KEY_KICK: [f64; 2] = [0.020, 0.012];
    pub const KEY_NOISE: f64 = 0.010;
    /// The damper landing: modes, kicks and noise (N), at a key's release.
    pub const DAMPER_HZ: [f64; 2] = [95.0, 240.0];
    pub const DAMPER_T60: [f64; 2] = [0.05, 0.035];
    pub const DAMPER_KICK: [f64; 2] = [0.006, 0.004];
    pub const DAMPER_NOISE: f64 = 0.003;
    /// A strike's variation: its speed (±) and its felt's stiffness (±), as fractions.
    pub const VARY_SPEED: f64 = 0.025;
    pub const VARY_FELT: f64 = 0.08;
}

/// A damped string's T60 at its first partial (s), by key: interpolated on a log scale.
pub fn damped_t60(m: f64) -> f64 {
    let pts = [(21.0, 1.5), (60.0, 0.3), (88.0, 0.15)];
    let i = if m < 60.0 { 0 } else { 1 };
    let (x0, y0) = pts[i];
    let (x1, y1) = pts[i + 1];
    let t = ((m - x0) / (x1 - x0)).clamp(-1.0, 2.0);
    math::exp(math::ln(y0) + t * (math::ln(y1) - math::ln(y0)))
}

/// The damper's contact a pedal position leaves on a released key (1: fully down).
pub fn pedal_contact(pedal: f64) -> f64 {
    let t = ((pedal - 0.15) / 0.5).clamp(0.0, 1.0);
    1.0 - t * t * (3.0 - 2.0 * t)
}

/// The ear's weight of key `key`'s partial `n` for its pitch (the technician's pass): the
/// struck string's amplitude there (`|sin(nπβ)|/n` at the strike `β`) and the dominance
/// region (Plomp 1967, Ritsma 1967: the partials from ~300 Hz to ~2 kHz decide a complex
/// tone's pitch) as a log-normal bump at 700 Hz, 1.5 octaves wide, floored at a tenth.
pub fn ear_weight(key: &Key, n: u32) -> f64 {
    let a = math::sin(n as f64 * std::f64::consts::PI * key.strike).abs() / n as f64;
    let o = math::log2(key.partial(n) / 700.0) / 1.5;
    a * (0.1 + 0.9 * math::exp(-0.5 * o * o))
}

/// How a strike's blow is shared among a unison's `strings` (each string's share of the
/// hammer's speed; the soft pedal at `soft` moves it off the last), with the voicing's fixed
/// per-key scatter hashed from `dk`. Allocates nothing.
fn unison_weights(dk: u8, strings: u8, settings: &VoiceSettings, soft: f64) -> [f64; MAX_STRINGS] {
    let mut weight = match strings {
        3 => [1.0, 1.0, 1.0 - soft],
        2 => [1.0, 1.0 - 0.5 * soft, 0.0],
        _ => [1.0, 0.0, 0.0],
    };
    if settings.uni_weight != 0.0 && strings > 1 {
        // The hammer's crown meets the strings unevenly: a fixed scatter per key — the
        // blow shared differently, its whole the same (round 40: unheld, a key whose
        // strings all drew a light share stood 12 dB under its neighbours).
        let n = strings as usize;
        let before: f64 = weight[..n].iter().sum();
        if settings.uni_rank != 0.0 {
            // By rank: the hash picks which string leads, the shares are fixed — the
            // leader whole, the next `1 − 0.75·w`, the last `1 − w` — so no key draws
            // two strings alike (round 40: those were the keys that beat 13–18 dB deep).
            let mut order = [0usize, 1, 2];
            let h = |i: usize| string_hash(dk, i as u8, 0xA7);
            order[..n].sort_by(|a, b| h(*b).total_cmp(&h(*a)));
            let shares = [0.0, 0.75, 1.0];
            for (r, i) in order[..n].iter().enumerate() {
                let s = if n == 2 { [0.0, 1.0][r] } else { shares[r] };
                weight[*i] *= (1.0 - settings.uni_weight * s).max(0.0);
            }
        } else {
            for (i, w) in weight.iter_mut().enumerate().take(n) {
                *w *= 1.0 - settings.uni_weight * 0.5 * (1.0 + string_hash(dk, i as u8, 0xA7));
            }
        }
        let after: f64 = weight[..n].iter().sum();
        if after > 0.0 {
            // Never a string struck much past the hammer's speed: the felt's law would
            // make it far brighter than its neighbours.
            let top = weight[..n].iter().fold(0.0f64, |m, w| m.max(*w));
            let g = (before / after).min(UNI_WEIGHT_TOP / top.max(1e-9));
            for w in weight[..n].iter_mut() {
                *w *= g;
            }
        }
    }
    weight
}

/// `x`'s first `n` entries less their mean weighed by `p` (nothing when the weights are all
/// nought).
fn centre(x: &mut [f64; MAX_STRINGS], p: &[f64; MAX_STRINGS], n: usize) {
    let sp: f64 = p[..n].iter().sum();
    if sp <= 0.0 {
        return;
    }
    let m = x[..n].iter().zip(&p[..n]).map(|(a, b)| a * b).sum::<f64>() / sp;
    for v in x[..n].iter_mut() {
        *v -= m;
    }
}

/// A small, fixed variation per key and string (−1…1), hashed, never random.
fn string_hash(key: u8, string: u8, salt: u8) -> f64 {
    key_hash(key.wrapping_mul(3).wrapping_add(string.wrapping_mul(97)) ^ salt)
}

/// A small, fixed variation per key (−1…1): hashed from the key, never random.
fn key_hash(key: u8) -> f64 {
    let mut z = (key as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xC40D_A5EED;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
}

#[derive(Clone, Debug)]
pub struct Voice {
    pub key: Key,
    strings: KeyStrings,
    pub active: bool,
    pub key_down: bool,
    /// The damper landed as the key came up (the pedal up): `damper_key`'s time, if set.
    by_key: bool,
    pub id: u64,
    /// The damper's contact now and where it goes.
    contact: f64,
    target: f64,
    /// The loss filter's grip as last set (to skip setting it again).
    set_contact: f64,
    fs: f64,
    /// The damper's glide per control step (~8 ms).
    glide: f64,
    settings: VoiceSettings,
    /// The phantom path: the tension's coefficient, its DC blocker and the longitudinal
    /// mode's resonator.
    tension: f64,
    dc_x: f64,
    dc_y: f64,
    res: [f64; 2],
    res_b: [f64; 3],
    phase: u32,
    quiet: u32,
    step_peak: f64,
    /// The strings' loudest control step since the strike (the damped floor's reference).
    loudest: f64,
    /// The design the strings were laid out for (its generation in the instrument).
    pub design_gen: u64,
    /// A strike for a newer keyboard while the strings still ring for an older one: they
    /// fade out over [`RETUNE_FADE`] samples, then the new strings are laid and struck
    /// (sound review 2: a key struck again while it rang kept the old tuning for good).
    retune: Option<Retune>,
    /// Model 2: the key's thud on the keybed and the damper's as it lands.
    knock: Thud,
    landing: Thud,
    /// Model 2 (round 40): the key's arrival-time difference between the microphones — one
    /// side hears its strings this many samples later (a spaced pair's: up to a millisecond
    /// or so, key by key). Empty: none (model 1, beta.14).
    pub lag: Lag,
    /// Where the last [`Voice::render`] ended the voice (samples into its piece; `None`: it
    /// sounded to the piece's end, or was not sounding). What a later side heard of it past
    /// that point is dropped, so a voice's end never depends on where a piece ends
    /// (`DSP_VERSION` 7: with the arrival-time difference on, a lag that drained in a long
    /// piece and was dropped in a short one made the block size audible at −115 dBFS).
    pub ended: Option<usize>,
    /// Model 2 (round 40): the key's own partial voicing (empty: none).
    voiced: PartialVoicing,
    /// Model 2 (round 40): the strings' rise — its envelope now (1: none) and its step.
    rise_env: f64,
    rise_k: f64,
}

/// The most a unison's string is struck at against the hammer's speed once its weight
/// scatter is rescaled to the key's whole blow (round 40).
pub const UNI_WEIGHT_TOP: f64 = 1.15;

/// How many of a key's partials its own voicing reaches (2 … 7).
pub const PARTIALS: usize = 6;

/// A key's partial voicing: one narrow peaking filter per voiced partial, mono, after the
/// strings (RBJ's cookbook; Q = 2n, a band of half the fundamental around partial n).
#[derive(Clone, Copy, Debug, Default)]
pub struct PartialVoicing {
    c: [[f64; 5]; PARTIALS],
    z: [[f64; 2]; PARTIALS],
    n: usize,
}

impl PartialVoicing {
    /// The filters for `gains` (dB, partials 2 …) of `key` at `fs`; states cleared. No
    /// allocation (a strike may set them on the audio thread).
    pub fn set(&mut self, key: &Key, gains: &[f64; PARTIALS], fs: f64) {
        self.n = 0;
        for (i, g) in gains.iter().enumerate() {
            let n = (i + 2) as u32;
            let f = key.partial(n);
            if *g == 0.0 || f >= 0.45 * fs {
                continue;
            }
            let a = math::powf(10.0f64, g / 40.0);
            let w = std::f64::consts::TAU * f / fs;
            let (s, c) = (math::sin(w), math::cos(w));
            let alpha = s / (2.0 * 2.0 * n as f64);
            let a0 = 1.0 + alpha / a;
            self.c[self.n] = [
                (1.0 + alpha * a) / a0,
                -2.0 * c / a0,
                (1.0 - alpha * a) / a0,
                -2.0 * c / a0,
                (1.0 - alpha / a) / a0,
            ];
            self.z[self.n] = [0.0; 2];
            self.n += 1;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    pub fn clear(&mut self) {
        self.z = [[0.0; 2]; PARTIALS];
    }

    #[inline]
    pub fn tick(&mut self, x: f64) -> f64 {
        let mut y = x;
        for (c, z) in self.c[..self.n].iter().zip(self.z[..self.n].iter_mut()) {
            let o = c[0] * y + z[0];
            z[0] = c[1] * y - c[3] * o + z[1];
            z[1] = c[2] * y - c[4] * o;
            y = o;
        }
        y
    }
}

/// A short delay for one side of a key's panned strings: the far microphone's.
#[derive(Clone, Debug)]
pub struct Lag {
    buf: Vec<f64>,
    len: usize,
    pos: usize,
    /// Whether the right side is the later one.
    pub right: bool,
}

impl Lag {
    /// Room for up to `capacity` samples. Allocates.
    pub fn new(capacity: usize) -> Lag {
        Lag {
            buf: vec![0.0; capacity.max(1)],
            len: 0,
            pos: 0,
            right: true,
        }
    }

    /// The delay now (samples; 0: none).
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether there is no delay.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Set the delay (`samples` > 0: the right side later; < 0: the left) and silence it.
    pub fn set(&mut self, samples: i64) {
        self.right = samples >= 0;
        self.len = (samples.unsigned_abs() as usize).min(self.buf.len());
        self.pos = 0;
        self.buf[..self.len].fill(0.0);
    }

    /// `x` in, what came in `len` samples ago out.
    #[inline]
    pub fn tick(&mut self, x: f64) -> f64 {
        if self.len == 0 {
            return x;
        }
        let y = self.buf[self.pos];
        self.buf[self.pos] = x;
        self.pos += 1;
        if self.pos == self.len {
            self.pos = 0;
        }
        y
    }
}

/// Samples a ringing voice takes to fade out before a strike for a newer keyboard.
pub const RETUNE_FADE: u32 = 128;

/// A strike waiting for the old strings to fade (everything [`Voice::strike`] takes).
#[derive(Clone, Copy, Debug)]
struct Retune {
    key: Key,
    design: LoopDesign,
    gen: u64,
    settings: VoiceSettings,
    vel: f64,
    soft: f64,
    id: u64,
    /// Samples of the fade left.
    left: u32,
    /// Let go before the strike landed: the pedal it was let go at.
    released: Option<f64>,
}

impl Voice {
    /// Its strings (a test's look inside).
    #[cfg(test)]
    pub(crate) fn strings(&self) -> &KeyStrings {
        &self.strings
    }

    /// Each loop's share kept (round 41, the technician's felt: [`KeyStrings::set_kept`]);
    /// from the next strike. For tests and the voicing lab.
    #[doc(hidden)]
    pub fn set_kept(&mut self, keep: [f64; MAX_STRINGS]) {
        self.strings.set_kept(keep);
    }

    /// What the last strike's lay designed for each loop (round 41).
    #[doc(hidden)]
    pub fn laid(&self) -> super::string::LaidLoops {
        self.strings.laid()
    }

    /// A voice for `key`, with strings for loops up to `capacity` samples. Allocates.
    pub fn new(key: Key, capacity: usize, fs: f64) -> Voice {
        let d = ((64.0 * key.impedance * key.speed * key.length * key.length * key.b)
            / (std::f64::consts::PI * std::f64::consts::PI * std::f64::consts::PI * STEEL_E))
            .max(0.0);
        let diameter = math::powf(d, 0.25).max(2.0e-4);
        let area = std::f64::consts::PI * diameter * diameter / 4.0;
        let tension = STEEL_E * area / (2.0 * key.speed * key.speed);
        // The first longitudinal mode, a resonator with Q 30.
        let fl = (STEEL_CL / (2.0 * key.length)).min(0.45 * fs);
        let w = std::f64::consts::TAU * fl / fs;
        let r = math::exp(-w / (2.0 * 30.0));
        let res_b = [-2.0 * r * math::cos(w), r * r, 1.0 - r];
        Voice {
            key,
            strings: KeyStrings::new(capacity, fs),
            active: false,
            key_down: false,
            by_key: false,
            id: 0,
            contact: 1.0,
            target: 1.0,
            set_contact: -1.0,
            fs,
            glide: 1.0 - math::exp(-(CONTROL as f64) / (0.008 * fs)),
            settings: VoiceSettings {
                felt: Felt {
                    k: key.hammer_k,
                    p: key.hammer_p,
                    eps: 0.9,
                    tau: 1.0e-5,
                },
                v_min: 0.35,
                v_max: 5.5,
                vel_curve: 1.0,
                decay: 1.0,
                coupling: 0.5,
                detune_ct: 1.0,
                damper: 1.0,
                phantom: 1.0,
                una_depth: 1.0,
                blow: 0.0,
                knock: 0.0,
                landing: 0.0,
                vary: 0.0,
                b1_scale: 1.0,
                b2_scale: 1.0,
                quiet: QUIET_N,
                damped_floor: 0.0,
                damper_t60: 1.0,
                uni_weight: 0.0,
                uni_skew: 0.0,
                uni_rank: 0.0,
                polar: 0.0,
                polar_ct: 0.0,
                polar_yz: 0.0,
                polar_damp: 1.0,
                loose_last: 0.0,
                mass: 1.0,
                partials: [0.0; PARTIALS],
                level: 1.0,
                thud_level: 1.0,
                bridge_n: 0.0,
                bridge_q: 30.0,
                bridge_g: 0.0,
                bridge_span: 10.0,
                uni_b: 0.0,
                uni_b_hold: 0.0,
                uni_center: 0.0,
                bridge_tune: 0.0,
                bridge_pull: 0.0,
                bridge_pull_hz: 0.0,
                uni_pair: 0.0,
                uni_loss: 0.0,
                knock_vel: 0.0,
                landing_tau: 0.0,
                landing_hi: 0.0,
                landing_modes: 1.0,
                landing_follow: 0.0,
                key_return: 0.0,
                rise_ms: 0.0,
                top_damper: 0.0,
                damper_key: 0.0,
                hf_steep: 1.0,
                strike_true: 0.0,
                draw: 0,
                felt_soft: 1.0,
                landing_rise: 0.0,
                seed: 0,
            },
            tension,
            dc_x: 0.0,
            dc_y: 0.0,
            res: [0.0; 2],
            res_b,
            phase: 0,
            quiet: 0,
            step_peak: 0.0,
            loudest: 0.0,
            design_gen: 0,
            retune: None,
            knock: Thud::new(fs, action::KEY_HZ, action::KEY_T60, 300.0, 3_000.0, 0.004),
            landing: Thud::new(
                fs,
                action::DAMPER_HZ,
                action::DAMPER_T60,
                200.0,
                2_000.0,
                0.006,
            ),
            // Up to 4 ms at 48 kHz's frames (the rate's own at higher rates).
            lag: Lag::new((0.004 * fs) as usize + 1),
            ended: None,
            voiced: PartialVoicing::default(),
            rise_env: 1.0,
            rise_k: 0.0,
        }
    }

    /// Whether this voice's strings can hold `design`'s loop.
    pub fn fits(&self, design: &LoopDesign) -> bool {
        self.strings.fits(design)
    }

    /// Strike the key at MIDI velocity `vel` (0…1), the soft pedal at `soft`.
    #[allow(clippy::too_many_arguments)]
    pub fn strike(
        &mut self,
        key: Key,
        design: &LoopDesign,
        design_gen: u64,
        settings: VoiceSettings,
        vel: f64,
        soft: f64,
        id: u64,
    ) {
        // The key the unison's draws are hashed from: its own unless re-voiced.
        let dk = if settings.draw == 0 {
            key.key
        } else {
            key.key.wrapping_add(settings.draw.wrapping_mul(89))
        };
        if self.active && self.design_gen != design_gen {
            // Strings ringing for an older keyboard: they fade, then this strike lands on
            // the new ones (the newest strike wins if several come meanwhile).
            let left = self.retune.map_or(RETUNE_FADE, |r| r.left);
            self.retune = Some(Retune {
                key,
                design: *design,
                gen: design_gen,
                settings,
                vel,
                soft,
                id,
                left,
                released: None,
            });
            return;
        }
        self.settings = settings;
        // The settings may have moved (decay, coupling): the loss follows at the next step.
        self.set_contact = -1.0;
        // A ringing voice keeps its strings (a new keyboard waits until it has ended: never
        // a cut); a silent one is laid out afresh, its damper already lifted by the key's
        // travel before the hammer lands.
        if !self.active {
            self.key = key;
            let spread = settings.detune_ct * (0.7 + 0.3 * key_hash(dk));
            let pair = settings.uni_pair != 0.0 && key.strings == 3;
            let mut detune = match key.strings {
                3 if pair => [0.0, 0.0, 2.0 * spread],
                3 => [-spread, 0.0, spread],
                2 => [-0.5 * spread, 0.5 * spread, 0.0],
                _ => [0.0; MAX_STRINGS],
            };
            if settings.uni_skew != 0.0 && key.strings > 1 && !pair {
                for (i, d) in detune.iter_mut().enumerate().take(key.strings as usize) {
                    *d += settings.uni_skew * spread * string_hash(dk, i as u8, 0x5E);
                }
            }
            // The technician's pass: the unison tuned to the key — every string's detune and
            // stiffness scatter less their mean, weighed by the strings' power.
            let n = key.strings as usize;
            let power = (settings.uni_center != 0.0 && n >= 1).then(|| {
                let w = unison_weights(dk, key.strings, &settings, 0.0);
                let mut p = [0.0; MAX_STRINGS];
                for (a, b) in p.iter_mut().zip(&w).take(n) {
                    *a = b * b;
                }
                p
            });
            if let Some(p) = &power {
                centre(&mut detune, p, n);
            }
            let polar = usize::from(settings.polar != 0.0 && key.strings < 3);
            if polar == 1 {
                detune[key.strings as usize] = settings.polar_ct;
            }
            self.strings.lay(
                &key,
                design,
                detune,
                key.strings as usize,
                polar,
                settings.polar_yz,
                settings.strike_true,
            );
            if settings.uni_b != 0.0 {
                let mut eps = [0.0; MAX_STRINGS];
                for (i, e) in eps.iter_mut().enumerate() {
                    *e = settings.uni_b * string_hash(dk, i as u8, 0x5B);
                }
                let hold = (settings.uni_b_hold != 0.0)
                    .then(|| std::f64::consts::TAU * key.partial(1) / self.fs);
                if let Some(p) = &power {
                    centre(&mut eps, p, n);
                    self.strings.set_string_stiffness_set(design, eps, hold, n);
                } else {
                    self.strings.set_string_stiffness_held(design, eps, hold);
                }
            }
            if settings.uni_loss != 0.0 {
                let mut ls = [(0.0, 0.0); MAX_STRINGS];
                for (i, l) in ls.iter_mut().enumerate() {
                    *l = (
                        settings.uni_loss * string_hash(dk, i as u8, 0x1C),
                        settings.uni_loss * string_hash(dk, i as u8, 0x1D),
                    );
                }
                self.strings.set_string_losses(ls);
            }
            if settings.loose_last != 0.0 && key.strings == 3 {
                self.strings.set_loose_from(2);
            }
            self.design_gen = design_gen;
            self.dc_x = 0.0;
            self.dc_y = 0.0;
            self.res = [0.0; 2];
            self.phase = 0;
            self.contact = 0.0;
            self.target = 0.0;
            self.voiced.set(&key, &settings.partials, self.fs);
            if settings.rise_ms > 0.0 {
                self.rise_env = 0.0;
                self.rise_k = 1.0 - math::exp(-1000.0 / (settings.rise_ms * self.fs));
            } else {
                self.rise_env = 1.0;
            }
            if settings.bridge_n >= 1.0 && settings.bridge_g > 0.0 {
                let k = (settings.bridge_n as usize).min(super::string::BRIDGE_MODES);
                let mut modes = [(0.0, 0.0, 0.0); super::string::BRIDGE_MODES];
                let (lo, span) = (key.f1 * 0.85, settings.bridge_span.max(1.5));
                for (j, m) in modes.iter_mut().enumerate().take(k) {
                    // Spread over the span, each moved within its slot by the key's hash.
                    let u = (j as f64 + 0.5 + 0.45 * string_hash(dk, j as u8, 0xB9)) / k as f64;
                    let g = 0.5 + 0.5 * (1.0 + string_hash(dk, j as u8, 0xBA));
                    *m = (
                        lo * math::powf(span, u),
                        settings.bridge_q,
                        settings.bridge_g * g,
                    );
                }
                self.strings.set_bridge_modes(&modes[..k]);
            }
            if settings.bridge_tune != 0.0 {
                let mut parts = [(0.0, 0.0, 0.0); super::string::TUNED_PARTIALS];
                let mut np = 0;
                for (k, p) in parts.iter_mut().enumerate() {
                    let f = key.partial(k as u32 + 1);
                    if k > 0 && (f > 5_000.0 || f > 0.45 * self.fs) {
                        break;
                    }
                    let w = std::f64::consts::TAU * f / self.fs;
                    *p = (w, design.group_delay(w), ear_weight(&key, k as u32 + 1));
                    np += 1;
                }
                self.strings.set_bridge_tuned(
                    &parts[..np],
                    settings.bridge_pull,
                    settings.bridge_pull_hz,
                );
            }
        }
        let soft = (soft * settings.una_depth).clamp(0.0, 1.0);
        let mut felt = settings.felt;
        felt.k *= 1.0 - 0.4 * soft;
        if settings.felt_soft != 1.0 {
            // The soft and mezzo strikes' felt, none from velocity 115.
            let w = ((115.0 / 127.0 - vel) / (40.0 / 127.0)).clamp(0.0, 1.0);
            if w > 0.0 {
                felt.k *= math::exp(w * math::ln(settings.felt_soft));
            }
        }
        let x = ((vel - 1.0 / 127.0) / (1.0 - 1.0 / 127.0)).clamp(0.0, 1.0);
        let shaped = math::powf(x, settings.vel_curve.max(0.1));
        let mut v = settings.v_min * math::powf(settings.v_max / settings.v_min, shaped);
        let seed = noise::seed(key.key, (settings.seed ^ (settings.seed >> 32)) as u32);
        if settings.vary != 0.0 {
            // No two strikes alike: the speed and the felt vary a little, seeded.
            v *= 1.0 + settings.vary * action::VARY_SPEED * noise::draw(seed ^ 0x51);
            felt.k *= 1.0 + settings.vary * action::VARY_FELT * noise::draw(seed ^ 0xF3);
        }
        if settings.knock != 0.0 {
            let soft_knock = if settings.knock_vel != 0.0 {
                math::powf((v / settings.v_max).clamp(1e-3, 1.0), settings.knock_vel)
            } else {
                1.0
            };
            let k = settings.knock * soft_knock * math::powf(v, 1.5);
            let kick = [k * action::KEY_KICK[0], k * action::KEY_KICK[1]];
            self.knock.strike(
                kick,
                settings.knock * soft_knock * v * v * action::KEY_NOISE,
                seed,
            );
        }
        let mut weight = unison_weights(dk, key.strings, &settings, soft);
        if self.strings.strings() > key.strings as usize {
            weight[key.strings as usize] = settings.polar;
        }
        let mass = if settings.mass == 1.0 {
            key.hammer_mass
        } else {
            key.hammer_mass * settings.mass
        };
        self.strings.strike(felt, mass, v, weight);
        self.loudest = 0.0;
        self.active = true;
        self.key_down = true;
        self.id = id;
        self.quiet = 0;
    }

    /// How firmly the damper rests on the strings now (0 off … 1 down).
    pub fn contact(&self) -> f64 {
        self.contact
    }

    /// Where the damper goes, for the key's state and the pedal (0…1).
    pub fn set_pedal(&mut self, pedal: f64) {
        if let Some(r) = self.retune.as_mut().filter(|r| r.released.is_some()) {
            r.released = Some(pedal);
        }
        if self.by_key && !self.key_down {
            // The pedal moved after the key came up: the pedal's damper from here.
            self.by_key = false;
            self.set_contact = -1.0;
        }
        self.target = if self.key_down {
            0.0
        } else if self.key.damper {
            pedal_contact(pedal) * self.settings.damper
        } else if self.settings.top_damper != 0.0 {
            pedal_contact(pedal) * self.settings.damper * self.settings.top_damper
        } else {
            0.0
        };
    }

    pub fn release(&mut self, pedal: f64) {
        self.key_down = false;
        self.set_pedal(pedal);
        // Damped by the key itself (the pedal up), or later by the pedal (`set_pedal`).
        // (Only a key with a damper: the top keys' release, `top_damper`, stays the pedal's.)
        let by_key = self.settings.damper_key != 0.0 && self.key.damper && self.target > 0.0;
        if by_key != self.by_key {
            self.by_key = by_key;
            self.set_contact = -1.0;
        }
        // Model 2: the damper lands on the strings (unless the pedal holds it up).
        if self.settings.landing != 0.0 && self.key.damper && self.active {
            let held = 1.0 - pedal_contact(pedal);
            let mut g = self.settings.landing * (1.0 - held);
            let st = &self.settings;
            if st.landing_follow != 0.0 && self.loudest > 0.0 {
                // The felt brushes strings that still move: as loud as they are.
                g *= math::powf(
                    (self.step_peak / self.loudest).clamp(1e-6, 1.0),
                    st.landing_follow,
                );
            }
            if g > 0.0 {
                if st.landing_tau != 0.0 || st.landing_hi != 0.0 {
                    let tau = if st.landing_tau != 0.0 {
                        st.landing_tau
                    } else {
                        0.006
                    };
                    let hi = if st.landing_hi != 0.0 {
                        st.landing_hi
                    } else {
                        2_000.0
                    };
                    self.landing = Thud::new(
                        self.fs,
                        action::DAMPER_HZ,
                        action::DAMPER_T60,
                        200.0,
                        hi,
                        tau,
                    );
                }
                if st.landing_rise != 0.0 || self.landing.rises() {
                    let tau = if st.landing_tau != 0.0 {
                        st.landing_tau
                    } else {
                        0.006
                    };
                    self.landing.set_rise(self.fs, tau, st.landing_rise);
                }
                let gm = g * st.landing_modes;
                let c = self.settings.seed;
                let seed = noise::seed(self.key.key, (c ^ (c >> 32)) as u32) ^ 0xDA;
                self.landing.strike(
                    [gm * action::DAMPER_KICK[0], gm * action::DAMPER_KICK[1]],
                    g * action::DAMPER_NOISE,
                    seed,
                );
            }
        } else if self.settings.key_return != 0.0 && !self.key.damper && self.active {
            let g = self.settings.key_return;
            let c = self.settings.seed;
            let seed = noise::seed(self.key.key, (c ^ (c >> 32)) as u32) ^ 0xDB;
            self.landing.strike(
                [g * action::DAMPER_KICK[0], g * action::DAMPER_KICK[1]],
                g * action::DAMPER_NOISE,
                seed,
            );
        }
    }

    /// Let go of any strike waiting for the old strings to fade (all notes off).
    pub fn release_all_waiting(&mut self, pedal: f64) {
        if let Some(r) = self.retune.as_mut() {
            r.released = Some(pedal);
        }
    }

    /// Let go of the strike waiting for the old strings to fade, if it is `id`'s: it
    /// lands released. Whether it was.
    pub fn release_waiting(&mut self, id: u64, pedal: f64) -> bool {
        match self.retune.as_mut() {
            Some(r) if r.id == id => {
                r.released = Some(pedal);
                true
            }
            _ => false,
        }
    }

    /// Stop at once (a reset).
    pub fn kill(&mut self) {
        self.retune = None;
        if self.active {
            self.strings.silence();
        }
        self.active = false;
        self.key_down = false;
        self.contact = 1.0;
        self.target = 1.0;
        self.quiet = 0;
        self.phase = 0;
        self.dc_x = 0.0;
        self.dc_y = 0.0;
        self.res = [0.0; 2];
        self.design_gen = 0;
        self.knock.silence();
        self.landing.silence();
        self.voiced.clear();
        self.rise_env = 1.0;
    }

    fn control(&mut self) {
        // The damper glides to its target over ~8 ms.
        let k = self.glide;
        self.contact += (self.target - self.contact) * k;
        if (self.contact - self.target).abs() < 1e-4 {
            self.contact = self.target;
        }
        if self.contact != self.set_contact {
            let t60 = if self.by_key {
                self.settings.damper_key
            } else {
                self.settings.damper_t60
            };
            let grip = 6.907_755 / (damped_t60(self.key.key as f64) * t60);
            // Model 2's decay laws (model 1: the key as measured, untouched).
            let mut key = self.key;
            if self.settings.b1_scale != 1.0 || self.settings.b2_scale != 1.0 {
                key.b1 /= self.settings.b1_scale;
                key.b2 /= self.settings.b2_scale;
            }
            let (fs, cp, dc) = (self.fs, self.settings.coupling, self.settings.decay);
            let steep = self.settings.hf_steep;
            let lossy = move |k: &Key, damp: f64| loss_steep(k, fs, cp, dc, damp, 0.0, steep);
            let (g, a, yz) = lossy(&key, self.contact * grip);
            self.strings.set_loss(g, a, yz);
            let loose = self.strings.strings() > key.strings as usize
                || (self.settings.loose_last != 0.0 && key.strings == 3);
            if loose && self.settings.polar_damp != 1.0 {
                let (gh, ah, _) = lossy(&key, self.contact * grip * self.settings.polar_damp);
                self.strings.set_loss_polar(gh, ah);
            }
            self.set_contact = self.contact;
        }
    }

    /// Render `out.len()` samples of the bridge force (N) into `out`, and model 2's
    /// action noises (N, the frame's: they reach the body, never the sympathetic strings)
    /// into `noise` (both added).
    pub fn render(&mut self, out: &mut [f64], noise: &mut [f64]) {
        self.ended = None;
        if let Some(r) = self.retune {
            // The old strings fade out; then the waiting strike lands on the new ones.
            let n = (r.left as usize).min(out.len());
            self.render_gain(&mut out[..n], &mut noise[..n], true);
            if self.retune.is_some_and(|r| r.left == 0) {
                let r = self.retune.take().unwrap_or(r);
                self.kill();
                self.strike(r.key, &r.design, r.gen, r.settings, r.vel, r.soft, r.id);
                if let Some(pedal) = r.released {
                    self.release(pedal);
                }
            }
            self.render_gain(&mut out[n..], &mut noise[n..], false);
            self.ended = self.ended.map(|e| e + n);
            return;
        }
        self.render_gain(out, noise, false);
    }

    /// [`Self::render`]'s samples; `fading`, each scaled by what is left of the fade.
    fn render_gain(&mut self, out: &mut [f64], noise: &mut [f64], fading: bool) {
        if !self.active {
            return;
        }
        let ph = self.settings.phantom * self.tension * LONGITUDINAL_COUPLING;
        let blow = self.settings.blow;
        for (i, (o, nz)) in out.iter_mut().zip(noise.iter_mut()).enumerate() {
            if self.phase == 0 {
                self.control();
            }
            let f = self.strings.tick();
            let mut y = f;
            if ph != 0.0 {
                let mut sq = 0.0;
                for i in 0..self.strings.strings() {
                    let a = self.strings.arrived(i);
                    sq += a * a;
                }
                // DC blocker, then half straight to the bridge, half through the
                // longitudinal mode.
                let x = sq * ph;
                let dc = x - self.dc_x + 0.9995 * self.dc_y;
                self.dc_x = x;
                self.dc_y = dc;
                let r =
                    self.res_b[2] * dc - self.res_b[0] * self.res[0] - self.res_b[1] * self.res[1];
                self.res[1] = self.res[0];
                self.res[0] = r;
                y += 0.5 * dc + 0.5 * r;
            }
            if blow != 0.0 {
                *nz += blow * self.strings.hammer_force();
            }
            if self.knock.busy() || self.landing.busy() {
                let t = self.knock.tick() + self.landing.tick();
                *nz += if self.settings.thud_level != 1.0 {
                    t * self.settings.thud_level
                } else {
                    t
                };
            }
            if fading {
                if let Some(r) = self.retune.as_mut() {
                    y *= f64::from(r.left) / f64::from(RETUNE_FADE);
                    r.left = r.left.saturating_sub(1);
                }
            }
            if !self.voiced.is_empty() {
                y = self.voiced.tick(y);
            }
            if self.settings.level != 1.0 {
                y *= self.settings.level;
            }
            if self.rise_env < 1.0 {
                y *= self.rise_env;
                self.rise_env += (1.0 - self.rise_env) * self.rise_k;
                if self.rise_env > 1.0 - 1.0e-9 {
                    self.rise_env = 1.0;
                }
            }
            *o += y;
            self.step_peak = self.step_peak.max(f.abs());
            self.phase += 1;
            if self.phase == CONTROL {
                self.phase = 0;
                self.loudest = self.loudest.max(self.step_peak);
                let damped = self.settings.damped_floor != 0.0
                    && self.target > 0.0
                    && self.contact == self.target
                    && self.step_peak < self.settings.damped_floor * self.loudest;
                if (self.step_peak < self.settings.quiet || damped)
                    && !self.key_down
                    && !self.strings.hammering()
                    && !self.knock.busy()
                    && !self.landing.busy()
                    && self.retune.is_none()
                {
                    self.quiet += 1;
                    if self.quiet >= QUIET_STEPS {
                        self.kill();
                        self.ended = Some(i + 1);
                        return;
                    }
                } else {
                    self.quiet = 0;
                }
                self.step_peak = 0.0;
            }
        }
    }
}
