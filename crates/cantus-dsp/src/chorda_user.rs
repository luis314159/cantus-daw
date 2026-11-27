//! Chorda's voicing by hand (round 42, `chorda-presets`, ADR 0098's eighth amendment): the
//! owner's request — *"el usuario debe poder mover cada uno de los parámetros a mano"*.
//!
//! The shipped voicing ([`Voicing`], candidate 18) is the technician's: fitted key by key
//! against two recorded grands. [`UserVoicing`] is what a player moves over it, in a
//! musician's terms — each control a scale, an offset or a value over the technician's
//! numbers, never a table. At its neutral value ([`UserVoicing::NEUTRAL`]) each control
//! leaves its numbers exactly as voiced, so a Chorda whose controls are all at their
//! defaults plays candidate 18 to the bit ([`UserVoicing::over`] copies the voicing then).
//!
//! What a control moves is read by the voices at their next strike, or (the formant, the top
//! roll-off) by the filters at once, their states kept: nothing here allocates, so the audio
//! thread may set them (automation, modulation, MIDI learn).
//!
//! | Group | Control | Over the voicing |
//! |---|---|---|
//! | Hammers | `felt_bass`, `felt_treble` (−1 … 1) | the felt's stiffness at A0 and at C8 ×4^x |
//! | | `mass` (×) | the hammers' mass |
//! | Strings | `decay_bass`, `decay_mid`, `decay_treble` (×) | the decay at A0, C4, C8 |
//! | | `hf` (×) | the overtones' sustain: the frequency-dependent loss's divisor |
//! | | `hf_spread` (×) | that loss's spread by register: its A0 and C8 scales to this power |
//! | | `uneven` (×) | the unison's unevenness: its hammer-weight, skew and loss scatters |
//! | | `polar` (×) | the second polarisation's share |
//! | Soundboard | `formant_hz`, `formant_db`, `formant_q` (values) | the bass formant |
//! | Action | `knock` (×), `knock_tone` (octaves) | the key's knock and its band |
//! | | `key_return` (×) | the key's soft thump coming up |
//! | | `damper` (×), `damper_bass` (×) | the dampers' strength (their hold ÷) and the bass's |
//! | | `damped_db` (value) | the floor a damped note ends at |
//! | Microphones | `itd` (×) | the arrival-time difference across the keyboard |
//! | Tone | `top_hz` (value) | the output's top roll-off |
//! | | `amount` (×) | each key's own partial voicing (the tables stay the base) |

use super::voicing::Voicing;
use crate::math;

/// What a player moves over the shipped voicing. A value-control (`Option`) is `None` when
/// the control sits at its default (the voicing's own number, to the bit).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UserVoicing {
    pub felt_bass: f64,
    pub felt_treble: f64,
    pub mass: f64,
    pub decay_bass: f64,
    pub decay_mid: f64,
    pub decay_treble: f64,
    pub hf: f64,
    pub hf_spread: f64,
    pub uneven: f64,
    pub polar: f64,
    pub formant_hz: Option<f64>,
    pub formant_db: Option<f64>,
    pub formant_q: Option<f64>,
    pub knock: f64,
    pub knock_tone: f64,
    pub key_return: f64,
    pub damper: f64,
    pub damper_bass: f64,
    pub damped_db: Option<f64>,
    pub itd: f64,
    pub top_hz: Option<f64>,
    pub amount: f64,
}

impl Default for UserVoicing {
    fn default() -> Self {
        UserVoicing::NEUTRAL
    }
}

/// The knock's band never passes this (Hz), however far its tone is raised.
const KNOCK_TOP: f64 = 16_000.0;

impl UserVoicing {
    /// Every control at its default: the voicing as the technician left it.
    pub const NEUTRAL: UserVoicing = UserVoicing {
        felt_bass: 0.0,
        felt_treble: 0.0,
        mass: 1.0,
        decay_bass: 1.0,
        decay_mid: 1.0,
        decay_treble: 1.0,
        hf: 1.0,
        hf_spread: 1.0,
        uneven: 1.0,
        polar: 1.0,
        formant_hz: None,
        formant_db: None,
        formant_q: None,
        knock: 1.0,
        knock_tone: 0.0,
        key_return: 1.0,
        damper: 1.0,
        damper_bass: 1.0,
        damped_db: None,
        itd: 1.0,
        top_hz: None,
        amount: 1.0,
    };

    pub fn is_neutral(&self) -> bool {
        *self == UserVoicing::NEUTRAL
    }

    /// `base` with these controls over it: exactly `base` when every control is neutral
    /// (each number is touched only when its control moved). Allocates nothing.
    pub fn over(&self, base: &Voicing) -> Voicing {
        let mut v = *base;
        if self.is_neutral() {
            return v;
        }
        let u = self;
        // Hammers: the felt at the keyboard's ends, as the hardness control scales it.
        if u.felt_bass != 0.0 {
            v.felt_bass *= math::powf(4.0f64, u.felt_bass);
        }
        if u.felt_treble != 0.0 {
            v.felt_treble *= math::powf(4.0f64, u.felt_treble);
        }
        if u.mass != 1.0 {
            v.mass *= u.mass;
        }
        // Strings.
        if u.decay_bass != 1.0 {
            v.decay_bass *= u.decay_bass;
        }
        if u.decay_mid != 1.0 {
            v.decay_mid *= u.decay_mid;
        }
        if u.decay_treble != 1.0 {
            v.decay_treble *= u.decay_treble;
        }
        if u.hf != 1.0 {
            v.hf_decay *= u.hf;
        }
        if u.hf_spread != 1.0 {
            let s = u.hf_spread.max(0.0);
            v.hf_bass = math::powf(v.hf_bass.max(1e-6), s);
            v.hf_treble = math::powf(v.hf_treble.max(1e-6), s);
        }
        if u.uneven != 1.0 {
            let k = u.uneven.max(0.0);
            v.uni_weight = (v.uni_weight * k).min(1.0);
            v.uni_weight_low *= k;
            v.uni_weight_bass *= k;
            v.uni_skew *= k;
            v.uni_loss *= k;
        }
        if u.polar != 1.0 {
            v.polar *= u.polar.max(0.0);
        }
        // Soundboard and case.
        if let Some(hz) = u.formant_hz {
            v.formant[0] = hz;
        }
        if let Some(db) = u.formant_db {
            v.formant[1] = db;
        }
        if let Some(q) = u.formant_q {
            v.formant[2] = q;
        }
        // Action.
        if u.knock != 1.0 {
            v.knock *= u.knock.max(0.0);
        }
        if u.knock_tone != 0.0 {
            let g = math::powf(2.0f64, u.knock_tone);
            let s = &mut v.knock_shape;
            for i in [2, 3, 9] {
                if s[i] > 0.0 {
                    s[i] = (s[i] * g).min(KNOCK_TOP);
                }
            }
            let t = &mut v.knock_treble_shape;
            if t[5] > 0.0 {
                t[5] = (t[5] * g).min(KNOCK_TOP);
            }
        }
        if u.key_return != 1.0 {
            v.key_return *= u.key_return.max(0.0);
        }
        if u.damper != 1.0 {
            v.damper_t60 /= u.damper.max(0.05);
        }
        if u.damper_bass != 1.0 {
            v.damper_bass /= u.damper_bass.max(0.05);
        }
        if let Some(db) = u.damped_db {
            v.damped_db = db;
        }
        // Microphones.
        if u.itd != 1.0 {
            let k = u.itd.max(0.0);
            v.itd_ms *= k;
            v.itd_jitter *= k;
            v.itd_ends *= k;
        }
        // Tone.
        if let Some(hz) = u.top_hz {
            v.top_hz = hz;
        }
        if u.amount != 1.0 {
            v.partials_amount *= u.amount.max(0.0);
        }
        v
    }
}
