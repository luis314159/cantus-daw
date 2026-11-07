//! Chorda's second model's voicing (ADR 0098): every number the comparison with the
//! reference piano set, in one place. [`Voicing::default`] is what ships; a tool (the
//! `chorda_compare` example's voicing lab) may try others through
//! [`super::Chorda::set_voicing`], which allocates and is never called on the audio thread.

use super::body::BodyVoicing;
use super::room::RoomVoicing;
use crate::math;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Voicing {
    pub body: BodyVoicing,
    pub room: RoomVoicing,
    /// The felt against model 1's (each key's measured hammer): its stiffness `K` scaled,
    /// its exponent `p` moved.
    pub felt_k: f64,
    pub felt_p: f64,
    /// The felt's stiffness at A0 and at C8 against `felt_k` (middle C's), log-interpolated:
    /// a technician voices the bass and the treble hammers apart.
    pub felt_bass: f64,
    pub felt_treble: f64,
    /// The decay against the measured one (> 1: longer), at A0, C4 and C8 (log-interpolated
    /// between), and the frequency-dependent loss's divisor.
    pub decay_bass: f64,
    pub decay_mid: f64,
    pub decay_treble: f64,
    pub hf_decay: f64,
    /// The action: the hammer's blow (a share of its force), the key's thud and the damper's
    /// landing (1: `voice::action`'s levels).
    pub blow: f64,
    pub knock: f64,
    pub landing: f64,
    /// The bridge force to the output's full scale.
    pub output: f64,
    /// The keys' spread across the microphones before the body (0…1; the `width` setting
    /// scales it).
    pub spread: f64,
    /// The slowest hammer against model 1's (< 1: a wider dynamic range — the reference's
    /// fortissimo stands further over its pianissimo).
    pub vmin: f64,
}

/// What shipped in round 37: the best of `chorda_compare`'s coordinate search against the
/// Salamander Grand (its score 38.9 at the first voicing, 33.2 after it, 33.1 with the low
/// modes' weight and a lighter knock that followed; model 1 scores 64.4 —
/// `docs/round37/piano.md`). The output: staged with the family (ADR 0101: the triad at
/// velocity 100 inside −18.7 ± 4.5 LUFS), the output ceiling taking a fortissimo chord's
/// attacks under full scale (ADR 0098 §3).
impl Default for Voicing {
    fn default() -> Self {
        Voicing {
            body: BodyVoicing::default(),
            room: RoomVoicing::default(),
            felt_k: 0.3266,
            felt_p: 0.0,
            felt_bass: 3.905,
            felt_treble: 1.082,
            decay_bass: 0.3,
            decay_mid: 1.71,
            decay_treble: 7.0,
            hf_decay: 4.0,
            blow: 0.05,
            knock: 0.533,
            landing: 1.0,
            output: 0.024_4,
            spread: 1.0,
            vmin: 0.6,
        }
    }
}

impl Voicing {
    /// The decay's scale for key `m` (MIDI).
    pub fn decay_of(&self, m: u8) -> f64 {
        let (x0, y0, x1, y1) = if m < 60 {
            (21.0, self.decay_bass, 60.0, self.decay_mid)
        } else {
            (60.0, self.decay_mid, 108.0, self.decay_treble)
        };
        let t = ((m as f64 - x0) / (x1 - x0)).clamp(0.0, 1.0);
        math::exp(math::ln(y0) + t * (math::ln(y1) - math::ln(y0)))
    }

    /// The felt's stiffness scale for key `m` (MIDI).
    pub fn felt_of(&self, m: u8) -> f64 {
        let (x0, y0, x1, y1) = if m < 60 {
            (21.0, self.felt_bass, 60.0, 1.0)
        } else {
            (60.0, 1.0, 108.0, self.felt_treble)
        };
        let t = ((m as f64 - x0) / (x1 - x0)).clamp(0.0, 1.0);
        self.felt_k * math::exp(math::ln(y0) + t * (math::ln(y1) - math::ln(y0)))
    }

    /// Set `key` = `value` by name (the voicing lab's `key=v,key=v`). False when the name
    /// is not one.
    pub fn set(&mut self, key: &str, v: f64) -> bool {
        let f = match key {
            "felt_k" => &mut self.felt_k,
            "felt_p" => &mut self.felt_p,
            "felt_bass" => &mut self.felt_bass,
            "felt_treble" => &mut self.felt_treble,
            "decay_bass" => &mut self.decay_bass,
            "decay_mid" => &mut self.decay_mid,
            "decay_treble" => &mut self.decay_treble,
            "hf_decay" => &mut self.hf_decay,
            "blow" => &mut self.blow,
            "knock" => &mut self.knock,
            "landing" => &mut self.landing,
            "output" => &mut self.output,
            "spread" => &mut self.spread,
            "vmin" => &mut self.vmin,
            "body.length" => &mut self.body.length,
            "body.t60_100" => &mut self.body.t60_100,
            "body.t60_1k" => &mut self.body.t60_1k,
            "body.lowest" => &mut self.body.lowest,
            "body.spacing" => &mut self.body.spacing,
            "body.spacing_low" => &mut self.body.spacing_low,
            "body.dense_hz" => &mut self.body.dense_hz,
            "body.rise_hz" => &mut self.body.rise_hz,
            "body.fall_hz" => &mut self.body.fall_hz,
            "body.fall_db" => &mut self.body.fall_db,
            "body.top_hz" => &mut self.body.top_hz,
            "body.direct" => &mut self.body.direct,
            "body.low_db" => &mut self.body.low_db,
            "body.low_hz" => &mut self.body.low_hz,
            "body.even" => &mut self.body.even,
            "body.coh_hz" => &mut self.body.coh_hz,
            "body.eq63" => &mut self.body.eq[0],
            "body.eq125" => &mut self.body.eq[1],
            "body.eq250" => &mut self.body.eq[2],
            "body.eq500" => &mut self.body.eq[3],
            "body.eq1k" => &mut self.body.eq[4],
            "body.eq2k" => &mut self.body.eq[5],
            "body.eq4k" => &mut self.body.eq[6],
            "body.eq8k" => &mut self.body.eq[7],
            "room.t60_low" => &mut self.room.t60_low,
            "room.t60_high" => &mut self.room.t60_high,
            "room.early" => &mut self.room.early,
            "room.late" => &mut self.room.late,
            "room.hp_hz" => &mut self.room.hp_hz,
            "room.lp_hz" => &mut self.room.lp_hz,
            _ => return false,
        };
        *f = v;
        true
    }
}
