//! Chorda's second model's voicing (ADR 0098): every number the comparison with the
//! reference piano set, in one place. [`Voicing::default`] is what ships — since
//! `DSP_VERSION` 7 round 40's candidate 16 ([`c16`]), before it [`Voicing::beta14`]; a tool
//! (the `chorda_compare` example's voicing lab) may try others through
//! [`super::Chorda::set_voicing`], which allocates and is never called on the audio thread.

use super::body::BodyVoicing;
use super::registers::BANDS;
use super::room::RoomVoicing;
use super::tables::KEYS;
use super::voice::PARTIALS;
use crate::math;
use std::sync::OnceLock;

/// Candidate 16, the shipped voicing's spec and tables (generated from the lab's files).
#[path = "chorda_voicing_c16.rs"]
pub mod c16;

/// The technician's pass over candidate 16 (`DSP_VERSION` 10): its spec and its table.
#[path = "chorda_voicing_tune.rs"]
pub mod tune;

/// The unison pass over the technician's (`DSP_VERSION` 11): its spec and its table.
#[path = "chorda_voicing_unison.rs"]
pub mod unison;

#[path = "chorda_voicing_c17.rs"]
pub mod c17;

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
    /// The frequency-dependent loss by register (round 40): `hf_decay` scaled at A0 and at
    /// C8 (1 at C4, log-interpolated) — the bass's upper partials, its "zing", outlive the
    /// treble's on a real grand. 1 and 1: `hf_decay` everywhere.
    pub hf_bass: f64,
    pub hf_treble: f64,
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
    /// The voicing by register (dB): per register (bass, middle, treble), the gain of each
    /// band at `registers::CENTRES` (`registers.rs`; all 0: the keys summed as they are).
    pub reg: [[f64; BANDS]; 3],
    /// Each register's level (dB; bass, middle, treble — `reg.<register>.level`).
    pub reg_level: [f64; 3],
    /// The registers' centre keys (MIDI; `registers::CENTRE_KEYS` as beta.14): a key between
    /// two feeds both, so a narrower crossfade keeps the bass's voicing out of the middle.
    pub reg_keys: [f64; 3],
    /// A released, damped key ends this many dB under its strike's loudest (0: never; see
    /// `VoiceSettings::damped_floor`).
    pub damped_db: f64,
    /// The dampers' hold: the damped strings' T60 scaled (1: as model 1's).
    pub damper_t60: f64,
    /// The unison's unevenness and the second polarisation (`VoiceSettings`; all 0: none).
    pub uni_weight: f64,
    pub uni_skew: f64,
    /// 1: the unison's shares by rank (`VoiceSettings::uni_rank`); 0: hashed.
    pub uni_rank: f64,
    pub polar: f64,
    pub polar_ct: f64,
    pub polar_yz: f64,
    pub polar_damp: f64,
    pub loose_last: f64,
    /// The hammers' mass scaled (1: as measured; `VoiceSettings::mass`).
    pub mass: f64,
    /// Each key's own partial voicing (dB, partials 2 … 7; `VoiceSettings::partials`), from A0,
    /// scaled by `partials_amount` (0: none).
    pub partials: [[f32; PARTIALS]; KEYS],
    pub partials_amount: f64,
    /// An undamped key's soft thump as it comes up (`VoiceSettings::key_return`; 0: none).
    pub key_return: f64,
    /// The output's top roll-off corner (Hz; 0: none).
    pub top_hz: f64,
    /// The dampers' hold at A0 against `damper_t60` (1 at C4 and above, log-interpolated):
    /// a grand's bass dampers are its heaviest and stop their strings fastest.
    pub damper_bass: f64,
    /// The quiet floor (N at the bridge) under which a voice, the sympathetic strings and
    /// the room are done (beta.14's 1e-4: about −115 dBFS at its output).
    pub quiet: f64,
    /// The microphones' arrival-time difference per key (ms): `itd_ms` across the keyboard
    /// (the bass reaching the left microphone first), plus up to `itd_jitter` key by key.
    /// 0 and 0: none.
    pub itd_ms: f64,
    pub itd_jitter: f64,
    /// The arrival-time difference's shape across the keyboard (round 40): 0 a straight line;
    /// `k` > 0 the path difference of a string at `x` on a line `k` away from two microphones
    /// at the keyboard's ends, `(√((x+1)²+k²) − √((x−1)²+k²)) / (√(4+k²) − k)` — steeper in
    /// the middle, levelling off toward the ends, as a real spaced pair hears the keys.
    pub itd_shape: f64,
    /// A fixed resonance in the bass register (Hz, dB, Q) — the board's formant near
    /// 600 Hz that lifts the bass partials falling there; 0 dB: none.
    pub formant: [f64; 3],
    /// The bridge's share of the strings' loss scaled (1: the `coupling` setting's) — how
    /// much faster a unison's strings moving together fall than against each other.
    pub coupling: f64,
    /// The bridge's share at C8 against C4's (log-interpolated; 1: as in the middle): the
    /// treble's unison falls very fast together (the reference's C7 loses 20 dB in its first
    /// 100 ms) and then sings on against each other.
    pub coupling_treble: f64,
    /// The key's thud against the keyboard: `knock` scaled at C8 (1 at C4 and below,
    /// log-interpolated) — the treble's short keys bottom more lightly.
    pub knock_treble: f64,
    /// The velocity's curve scaled (1: the `vel.curve` setting's; < 1 lifts the middle
    /// velocities against the loudest).
    pub vel_curve: f64,
    /// The output ceiling (ADR 0098 §3): unity up to `knee`, bending toward `ceiling`
    /// (full-scale shares).
    pub knee: f64,
    pub ceiling: f64,
    /// Model 2's keyboard (round 40, finding 9): the bass and treble asymptotes of the
    /// inharmonicity and the treble's octave type its strings are designed with
    /// ([`Voicing::tuning`]; 1, 1, 1: model 1's keyboard, the one design both share).
    pub bass_b: f64,
    pub treble_b: f64,
    pub treble_octave: f64,
    /// Round 40: the keyboard evened as a voicer evens it — each key's strings raised or
    /// lowered by what the board gives its partials against what it gives their third of an
    /// octave (`even` of the difference in dB, at most [`EVEN_LIMIT`]; 0: none). A board's
    /// modes make some keys loud and some weak (Chorda's ±5 dB key to key against the
    /// reference's ±1.7); a technician needles the hammers until the keyboard is even.
    /// `even_hz`: the corner of the hammer's spectrum the partials are weighed with.
    pub even: f64,
    pub even_hz: f64,
    /// The share of a key's evening its action's thud follows (0: none — the thud stays the
    /// keybed's; 1: all of it, as if the whole key were louder).
    pub even_thud: f64,
    /// Round 40: each key's own level (dB), `levels_amount` of it — what the board's lines
    /// cannot predict in the treble, measured key by key (0: none).
    pub levels: [f32; KEYS],
    pub levels_amount: f64,
    /// The A5–A6 octave's own octave type (0: `treble_octave`'s; [`Voicing::tuning`]).
    pub treble_octave_hi: f64,
    /// The bridge's share at A0 against C4's (log-interpolated; 1: as in the middle).
    pub coupling_bass: f64,
    /// Round 40: the bridge's resonances near each key (`[count, Q, gain against YZ, span
    /// in multiples of the key's first partial]`; count 0: a resistive bridge, as before) —
    /// each partial of a unison then meets its own admittance, so each beats on its own
    /// schedule (Weinreich) instead of the whole tone sweeping as one notch.
    pub bridge: [f64; 4],
    /// The lowest hammer speed's scale at C7 against C4's (log-interpolated from C4).
    pub vmin_treble: f64,
    /// How far a unison's strings' stiffness scatters (0: alike) — no two strings of a
    /// unison are alike, so their partials beat each on its own schedule.
    pub uni_b: f64,
    /// `uni_b`'s scale at A0 against C4's (log-interpolated; 1: the same): the bass's
    /// strings are wound and few, their dispersion small, so their scatter must be larger.
    pub uni_b_bass: f64,
    /// 1: each string's first partial held where its detune put it as its stiffness
    /// scatters (0: not; [`super::string::KeyStrings::set_string_stiffness_held`]).
    pub uni_b_hold: f64,
    /// How far a unison's strings' losses scatter (0: alike): each partial of each string
    /// fades at its own rate.
    pub uni_loss: f64,
    /// Extra hammer-weight scatter for the unisons below C5 (0: none), fading out by C6 as
    /// the stiffness and loss scatters and the bridge's resonances do: the treble's
    /// unisons fall together (its two-stage decay), the flanger was A1–A4's.
    pub uni_weight_low: f64,
    /// The unison's detune at A0 against C4's (log-interpolated; 1: the same): the
    /// reference's lowest notes hardly beat — their strings, coupled hard at the bridge,
    /// pull together.
    pub detune_bass: f64,
    /// Model 2's unison detune against the `unison` setting's (1: the setting's) — the
    /// second analysis measured a unison's beats 13–20 dB deep against the reference's
    /// 2–11 at the setting's 1 cent.
    pub detune_scale: f64,
    /// The felt's stiffness scale around F#5 (1: none; a triangle in key from D#4 to F#6) —
    /// the reference's C5–A5 sing on their fundamentals, partials 2–4 15–20 dB under it.
    pub felt_upper: f64,
    /// The key's thud's extra speed exponent (0: none).
    pub knock_vel: f64,
    /// The damper's landing: `[noise fall s (0: 6 ms), band top Hz (0: 2 kHz), modes' share
    /// (1), how far it follows the note's level (0)]`.
    pub landing_shape: [f64; 4],
    /// The bass register's high-pass (Hz; 0: none) — under the lowest notes.
    pub bass_hp: f64,
    /// Its order: 2 (one Butterworth section) or 4 (two) — the reference's A0–F#1
    /// fundamentals stand 10–15 dB under Chorda's against their second partials.
    pub bass_hp_order: f64,
    /// Round 40: the strings' rise (ms; at C3, C4, C5, C6 and C7, all 0: none) — a strike's
    /// tone builds up over that time constant instead of switching on (the reference rises
    /// from −20 to −3 dB of its peak in 7–8 ms from C3 to B5, 4–5 ms above; Chorda in 2–4).
    /// From nothing at B1, linear between the anchors, flat above C7.
    pub rise: [f64; 5],
    /// Round 40: the top keys' release (0: none — a grand's top octave and a half has no
    /// dampers): a released key above E6 is damped at this share of a damper's grip. The
    /// reference (a sampler's release) lets a soft trill's top notes fade within a second.
    pub top_damper: f64,
    /// Round 40: the damper landing's noise rises over this share of its fall (0: at once —
    /// a click); the reference's key-up is a breath, not a click.
    pub landing_rise: f64,
    /// The keys (MIDI) the unison scatters fade out between ([`Voicing::unison_fade`]).
    pub uni_fade: [f64; 2],
    /// Round 40: the soft strikes' level (dB at velocity 30; at A0, C4, C6, C8, linear in key
    /// between; all 0: none) — 0 dB from velocity 75 up, growing below it: the reference's
    /// pianissimo stands 3–5 dB further under its mezzo-forte in the tenor than Chorda's,
    /// its colour already alike (the felt's law alone moved it a dB at most).
    pub pp_trim: [f64; 4],
    /// Round 40 (candidate 7): the unison's detune at C8 against the key's own
    /// (`[from key, scale at C8]`, log-interpolated from 1 at the key; 1: none). A tuner
    /// sets the top unisons tight — a cent there is several hertz — and strings tuned closer
    /// than the bridge pulls them sing a fast first stage and a slow second (Weinreich): the
    /// reference's top fundamentals fall 50 dB/s, then 10–20; Chorda's detuned ones fell
    /// 20–35 all the way, and G7, G#7, B7 and C8 50.
    pub detune_top: [f64; 2],
    /// Round 40 (candidate 7): the treble's velocity curve (`[from key, the hammer's speed
    /// range at C8, the curve's exponent at C8]`, each a share log-interpolated from 1 at the
    /// key; 1 and 1: none) — the range narrows toward the top speed (a soft strike faster,
    /// the hardest the same), the exponent bends the middle velocities up. The reference's
    /// C7–C8 stand 13 dB from mezzo-forte to fortissimo, Chorda's 18.
    pub vel_top: [f64; 3],
    /// Round 40 (candidate 7): extra arrival-time difference at the keyboard's ends (ms,
    /// `itd_ends · x³` on top of `itd_ms`'s curve; 0: none) — wider at the ends, the middle
    /// left where it is.
    pub itd_ends: f64,
    /// Round 40 (candidate 7): extra hammer-weight scatter at A0 (0: none), linear to none at
    /// C4 — the reference's bass unisons beat 2–5 dB deep, Chorda's A1, F#2 and F#3 8–11.
    pub uni_weight_bass: f64,
    /// Round 40 (candidate 7): the loss's rise above the first partial at C8 (`[from key,
    /// factor at C8]`, log-interpolated from 1 at the key; 1: the law) — the top keys'
    /// fundamentals kept living by the tight unisons, their second partials (4–8 kHz) fall
    /// faster than the law has them (the very top's fundamentals a little faster too: the
    /// loop's passive filter cannot take one without the other).
    pub hf_top: [f64; 2],
    /// Round 40 (candidate 10): the damper's time scale when it lands as a key comes up with
    /// the pedal up (0: `damper_t60`, the same as after the pedal) — the reference's single
    /// notes end slower than its pedalled phrases do once the pedal comes up.
    pub damper_key: f64,
    /// `damper_key`'s scale at A0 against C4's (log-interpolated; 1: the same) — the
    /// reference's bass ends faster after its key than its middle does.
    pub damper_key_bass: f64,
    /// Round 40 (candidate 11): the strike point placed against the loop's whole period at
    /// the partial its comb notches (0: against the delay lines alone, beta.14's — middle C's
    /// notch at partials 12–13, not 8; 1: true; between, a blend).
    pub strike_true: f64,
    /// Round 40 (candidate 11): the lowest hammer speed's scale at A0 against C4's
    /// (log-interpolated to 1 at C4; 1: none) — `vmin` raised for the middle's soft strikes
    /// (the reference's are brighter than Chorda's) held where it was in the bass, whose soft
    /// strikes were already bright enough.
    pub vmin_bass: f64,
    /// Round 40 (candidate 12): the unison's detune below the treble scaled (`[scale, to key]`:
    /// the whole scale up to an octave under `to`, log-faded to none at `to`; 1: none) — with
    /// the strike placed true, the middle's partials 10–14 beat: C4 12 dB deep, A3 8, against
    /// the reference's 9 and 5; the top keys' unisons (Weinreich's two stages) left as they are.
    pub detune_low: [f64; 2],
    /// Round 40 (candidate 13): each key's draw (0: its own, beta.14's). A key's unison —
    /// its strings' detune skew, weights, stiffness and loss scatters, its bridge's
    /// resonances — is hashed from the key; a non-zero draw hashes it as another, fixed,
    /// so a technician can re-voice one key whose unison landed badly (D#2's flanger)
    /// without touching any other. Set by name as `draw.<MIDI key>`.
    pub draws: [u8; KEYS],
    /// Round 40 (candidate 13): the felt's stiffness for the soft and mezzo strikes of a
    /// register (`[scale, from key, to key]`: the whole scale between the keys, faded to none
    /// an octave outside; at velocity 75 and under, faded to none at 115; 1: none) — the
    /// reference's middle is brighter than Chorda's at pp and mf while its ff is not.
    pub felt_soft: [f64; 3],
    /// Round 40 (candidate 14): each key's frequency-dependent loss divisor scaled (1: the
    /// register's law; < 1 its upper partials die faster) — the reference's middle C loses
    /// its partial 9 (~2.4 kHz) 20 dB in 1.5 s, Chorda's under the law ~12 for every unison
    /// draw. Set by name as `hf_key.<MIDI key>`.
    pub hf_keys: [f32; KEYS],
    /// Round 40, the technician's pass (`DSP_VERSION` 10): 1 — each unison's scatters
    /// centred on the key, as a tuner tunes the unison to the key's pitch (0: not; beta.14's
    /// and candidate 16's). The strings' detunes (fan and skew) and their stiffness scatter
    /// each lose their mean, weighed by the strings' shares of the blow (their power) — the
    /// strings still beat against each other as drawn, but the key's partials sit on the
    /// design. Uncentred, candidate 16's stiffness scatter moved whole keys' upper partials
    /// (Bb2 −13 cents as the ear weighs them, B0 +18) and the skewed detune their
    /// fundamentals by up to 2 cents. With it the stiffness scatter is held as one set: the
    /// fallbacks a fraction in reach allows (the sign, a half, a quarter, an eighth) scale
    /// every string's together, so the mean stays nought.
    pub uni_center: f64,
    /// Round 40, the technician's last pass: each key's own tuning (cents, its keyboard
    /// design's whole ladder; all 0: none, beta.14's and candidate 16's) — what a key still
    /// needs, measured, to sound on the curve with its unison, bridge, board and room around
    /// it ([`Voicing::tuning`] hands it to the keyboard). Set by name as `tune.<MIDI key>`.
    pub tune: [f32; KEYS],
    /// Round 40, the technician's pass: the cents of detune each way every key's loop keeps
    /// before its fraction meets its clamp ([`super::design::design_with_room`]; 0: none,
    /// beta.14's and candidate 16's keyboard). Set as `keyboard.frac_room`.
    pub frac_room: f64,
    /// Round 40, the technician's pass: 1 — each key's strings tuned with the bridge's load
    /// on them, so its resonances never pull a fundamental off the design
    /// ([`super::string::KeyStrings::set_bridge_tuned`]; 0: not, beta.14's and candidate
    /// 16's — G2 and A#2 sounded 3–5 cents flat).
    pub bridge_tune: f64,
    /// Round 40, the technician's pass: with `bridge_tune`, the most (cents) a bridge
    /// resonance may pull one of a key's first three partials — the key's resonances' gains
    /// scaled down until none does (0: no limit).
    pub bridge_pull: f64,
    /// Round 41, every unison beatless (`DSP_VERSION` 11): with `bridge_tune`, the most (Hz)
    /// a bridge resonance may pull one of a key's first three partials — the beat it leaves
    /// between the unison's in-phase mode and its opposed modes, which the bridge never moves
    /// (0: none, every voicing before; `KeyStrings::set_bridge_tuned`). Set as
    /// `bridge.pull_hz`.
    pub bridge_pull_hz: f64,
    /// Round 41, every unison beatless (`DSP_VERSION` 11): each key's share of the `unison`
    /// control's first cent taken as the technician's (0: none, every voicing before; 1: at
    /// the control's default the key's strings are tuned to each other; each cent above it
    /// detunes them as before) — the owner heard the designed spread as strings out of
    /// tune. Set by name as `uni_tight.<MIDI key>`, or `uni_tight` for every key.
    pub uni_tight: [f32; KEYS],
    /// Round 41 (`DSP_VERSION` 11): each key's share of its strings' loss that goes through
    /// the bridge, scaled (1: the register's, every voicing before) — a unison tuned closer
    /// rings less of the slow second stage a detuned one rang (Weinreich 1977), so its
    /// bridge takes less of the strings' loss and the key sustains as it did. Set by name as
    /// `coupling.<MIDI key>`.
    pub couplings: [f32; KEYS],
    /// Round 41 (`DSP_VERSION` 11): the strings' loss scatter's share where the unison
    /// scatters have faded out (0: faded with them, every voicing before; 1: whole to C8). A
    /// unison tuned to one pitch rings its slow second stage only if its strings differ: a
    /// loss scatter (the loop's loss filter is linear-phase) makes them differ in decay, never
    /// in pitch — an aftersound with no beat. Set as `uni_loss_top`.
    pub uni_loss_top: f64,
    /// Round 41 (`DSP_VERSION` 11): 1 — a key's three-string unison laid as two strings
    /// alike and a third apart (`VoiceSettings::uni_pair`; 0: the designed fan, every
    /// voicing before). Set by name as `uni_pair.<MIDI key>`, or `uni_pair` for every key.
    pub uni_pair: [f32; KEYS],
    /// Round 41 (`DSP_VERSION` 11): each key's stiffness scatter scaled (1: `uni_b`'s, every
    /// voicing before). Set by name as `uni_b.<MIDI key>`.
    pub uni_b_keys: [f32; KEYS],
    /// Round 42 (`chorda-c17`): the action's knock as the case, the keyframe and the board
    /// ring it ([`super::voice::KNOCK_AS_IS`]: the round-40 thud; `VoiceSettings::knock_shape`)
    /// — set as `knock.noise`, `.tau`, `.lo`, `.hi`, `.tail`, `.tail_tau`, `.t60`, `.noise_vel`.
    /// The recorded grands' top keys knock 5–10 dB under their note at 0.5–2.8 kHz for 20 ms
    /// and ring on for a quarter of a second; the round-40 knock was a low thud alone.
    /// Round 42 (`chorda-c18`): `.rise` (the first stage's rise, a share of its fall) and
    /// `.tail_hi` (the second stage's own low-pass, Hz).
    pub knock_shape: [f64; super::voice::KNOCK_PARTS],
    /// Round 42: the knock's noise scaled at A0 and at C8 against C4 (log in key between; 1, 1:
    /// as at C4). Set as `knock.noise_bass`, `knock.noise_treble`.
    pub knock_noise_reg: [f64; 2],
    /// Round 42 (`chorda-c18`): the top keys' knock, from C6 ([`KNOCK_TOP_FROM`]: as below
    /// it) to its values at C7 and above ([`KNOCK_TOP_TO`]; log in key between) — its first
    /// stage's fall, its band's top, its second stage's share and that stage's fall as
    /// factors (1: the same); the first stage's rise (a share of its fall) and the second's
    /// own low-pass (Hz) as values (0: as the shape has them). A grand's top keys click
    /// short, bright and dry (the two recorded grands' 1–3 kHz falls 400 dB/s from 5 ms); its
    /// middle keys' case rings on under their partials. Set as `knock.tau_treble`,
    /// `.hi_treble`, `.tail_treble`, `.tail_tau_treble`, `.rise_treble`, `.tail_hi_treble`.
    pub knock_treble_shape: [f64; 6],
    /// Round 42 (`chorda-c18`): the key's knock and the hammer's blow scaled at A0 against
    /// C4 (log in key between, 1 at C4 and above; 1: as everywhere). A bass string's tone
    /// reaches the bridge 10–15 ms after its hammer; a knock and a blow at the strike as loud
    /// as the middle's stand before it as a thump of their own (the recorded grands' A0–D♯1
    /// rise in 5–13 ms, Chorda's in 21–34). Set as `knock_bass`, `blow_bass`.
    pub knock_bass: f64,
    pub blow_bass: f64,
}

/// Round 42 (`chorda-c18`): the key above which `knock_treble_shape` moves the knock (C6).
pub const KNOCK_TOP_FROM: u8 = 84;
/// … and the key from which it is the top shape whole (C7): the top octave knocks alike.
pub const KNOCK_TOP_TO: u8 = 96;
/// `Voicing::knock_treble_shape`'s neutral value: the top keys knock as the shape says.
pub const KNOCK_TOP_AS_IS: [f64; 6] = [1.0, 1.0, 1.0, 1.0, 0.0, 0.0];

/// The most a key is evened by (dB).
pub const EVEN_LIMIT: f64 = 9.0;
/// The keys the evening fades out between (E5, E6).
pub const EVEN_FADE_FROM: f64 = 76.0;
pub const EVEN_FADE_TO: f64 = 88.0;
/// The partials a key's evening weighs.
pub const EVEN_PARTIALS: u32 = 10;

/// What ships since `DSP_VERSION` 16 (round 42, ADR 0098's sixth amendment): the unison pass
/// of `DSP_VERSION` 11 ([`Voicing::unison`]) with candidate 17 over it ([`Voicing::shipped`]).
/// Made once; nothing allocates.
impl Default for Voicing {
    fn default() -> Self {
        static SHIPPED: OnceLock<Voicing> = OnceLock::new();
        *SHIPPED.get_or_init(Voicing::shipped)
    }
}

impl Voicing {
    /// Round 40's candidate 16 (`docs/round40/chorda.md`): beta.14's voicing, its tables,
    /// then its spec key by key (a later key overriding an earlier one).
    pub fn candidate16() -> Self {
        let mut v = Voicing::beta14();
        v.partials = c16::PARTIALS;
        v.levels = c16::LEVELS;
        let ok = v.apply(c16::SPEC);
        debug_assert!(
            ok,
            "candidate 16's spec names a key the voicing does not have"
        );
        v
    }

    /// What shipped in `DSP_VERSION` 10 (`docs/round40/chorda-tune.md`; beta.15 and
    /// beta.16): candidate 16, then the technician's pass — [`tune::SPEC`] applied over it
    /// (the unisons centred on their keys, the strings tuned on their bridge, the loops'
    /// detune room, sixteen unisons redrawn) and [`tune::TUNE`], each key's last correction,
    /// handed to the keyboard.
    pub fn technician() -> Self {
        let mut v = Voicing::candidate16();
        let ok = v.apply(tune::SPEC);
        debug_assert!(
            ok,
            "the technician's spec names a key the voicing does not have"
        );
        v.tune = tune::TUNE;
        v
    }

    /// What shipped in `DSP_VERSION` 11 to 15 (`docs/round41/chorda-tune.md`; beta.16 and
    /// beta.17, the owner's blind test of 2026-11-25): the technician's pass, then the unison pass — [`unison::SPEC`] applied over it (no bridge resonance pulling
    /// a partial 1–3 more than 0.1 Hz; each unison as wide as it can be without a beat the
    /// ear counts, its stiffness scatter, pattern and bridge share keeping the key's level)
    /// and [`unison::TUNE`], each key's last correction re-measured, handed to the keyboard.
    pub fn unison() -> Self {
        let mut v = Voicing::technician();
        let ok = v.apply(unison::SPEC);
        debug_assert!(ok, "the unison spec names a key the voicing does not have");
        v.tune = unison::TUNE;
        v
    }

    /// What ships (`DSP_VERSION` 16, round 42, `docs/round42/chorda-c17.md`): the unison
    /// pass, then candidate 17 — [`c17::SPEC`] applied over it (the action's knock as the
    /// case rings it, middle C's third partial voiced).
    pub fn shipped() -> Self {
        Voicing::c17()
    }

    /// Candidate 17 (`DSP_VERSION` 16, the owner's blind test of 2026-11-26): the unison pass,
    /// then [`c17::SPEC`] applied over it — named, so its bits stay reproducible.
    pub fn c17() -> Self {
        let mut v = Voicing::unison();
        let ok = v.apply(c17::SPEC);
        debug_assert!(
            ok,
            "candidate 17's spec names a key the voicing does not have"
        );
        v
    }

    /// Apply a spec `key=v,key=v` (the voicing lab's) key by key. False when a key is not
    /// one or a value not a number (the rest still applied).
    pub fn apply(&mut self, spec: &str) -> bool {
        let mut ok = true;
        for kv in spec.split(',').filter(|x| !x.is_empty()) {
            ok &= match kv.split_once('=') {
                Some((k, x)) => match x.parse::<f64>() {
                    Ok(x) => self.set(k, x),
                    Err(_) => false,
                },
                None => false,
            };
        }
        ok
    }

    /// What shipped from round 37 to beta.14 (model 2's second voicing; `DSP_VERSION` 6):
    /// the best of `chorda_compare`'s coordinate search against the Salamander Grand (its
    /// score 38.9 at the first voicing, 33.2 after it, 33.1 with the low modes' weight and a
    /// lighter knock that followed; model 1 scores 64.4 — `docs/round37/piano.md`). The
    /// output: staged with the family (ADR 0101: the triad at velocity 100 inside
    /// −18.7 ± 4.5 LUFS), the output ceiling taking a fortissimo chord's attacks under full
    /// scale (ADR 0098 §3). Every part round 40 added is neutral in it. What the listening
    /// kit's *Chorda 2 (beta.14)* plays and a test pins.
    pub fn beta14() -> Self {
        Voicing {
            body: BodyVoicing::beta14(),
            room: RoomVoicing::beta14(),
            felt_k: 0.3266,
            felt_p: 0.0,
            felt_bass: 3.905,
            felt_treble: 1.082,
            decay_bass: 0.3,
            decay_mid: 1.71,
            decay_treble: 7.0,
            hf_decay: 4.0,
            hf_bass: 1.0,
            hf_treble: 1.0,
            blow: 0.05,
            knock: 0.533,
            landing: 1.0,
            output: 0.024_4,
            spread: 1.0,
            vmin: 0.6,
            reg: [[0.0; BANDS]; 3],
            reg_level: [0.0; 3],
            reg_keys: super::registers::CENTRE_KEYS,
            damped_db: 0.0,
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
            partials: [[0.0; PARTIALS]; KEYS],
            partials_amount: 0.0,
            key_return: 0.0,
            top_hz: 0.0,
            damper_bass: 1.0,
            quiet: super::engine::MODEL_2_QUIET,
            itd_ms: 0.0,
            itd_jitter: 0.0,
            itd_shape: 0.0,
            formant: [600.0, 0.0, 3.0],
            coupling: 1.0,
            coupling_treble: 1.0,
            knock_treble: 1.0,
            vel_curve: 1.0,
            knee: super::engine::KNEE,
            ceiling: super::engine::CEILING,
            bass_b: 1.0,
            treble_b: 1.0,
            treble_octave: 1.0,
            even: 0.0,
            even_hz: 1_500.0,
            even_thud: 0.0,
            levels: [0.0; KEYS],
            levels_amount: 0.0,
            treble_octave_hi: 0.0,
            coupling_bass: 1.0,
            bridge: [0.0, 30.0, 0.0, 10.0],
            vmin_treble: 1.0,
            uni_b: 0.0,
            uni_b_bass: 1.0,
            uni_b_hold: 0.0,
            uni_loss: 0.0,
            uni_weight_low: 0.0,
            detune_bass: 1.0,
            detune_scale: 1.0,
            felt_upper: 1.0,
            knock_vel: 0.0,
            landing_shape: [0.0, 0.0, 1.0, 0.0],
            bass_hp: 0.0,
            bass_hp_order: 2.0,
            rise: [0.0; 5],
            top_damper: 0.0,
            landing_rise: 0.0,
            uni_fade: [72.0, 84.0],
            pp_trim: [0.0; 4],
            detune_top: [84.0, 1.0],
            vel_top: [84.0, 1.0, 1.0],
            itd_ends: 0.0,
            uni_weight_bass: 0.0,
            hf_top: [84.0, 1.0],
            damper_key: 0.0,
            damper_key_bass: 1.0,
            strike_true: 0.0,
            vmin_bass: 1.0,
            detune_low: [1.0, 84.0],
            draws: [0; KEYS],
            felt_soft: [1.0, 55.0, 79.0],
            hf_keys: [1.0; KEYS],
            uni_center: 0.0,
            tune: [0.0; KEYS],
            frac_room: 0.0,
            bridge_tune: 0.0,
            bridge_pull: 0.0,
            bridge_pull_hz: 0.0,
            uni_tight: [0.0; KEYS],
            couplings: [1.0; KEYS],
            uni_loss_top: 0.0,
            uni_pair: [0.0; KEYS],
            uni_b_keys: [1.0; KEYS],
            knock_shape: super::voice::KNOCK_AS_IS,
            knock_noise_reg: [1.0, 1.0],
            knock_treble_shape: KNOCK_TOP_AS_IS,
            knock_bass: 1.0,
            blow_bass: 1.0,
        }
    }

    /// Round 42: the knock's shape for key `m` (MIDI): exactly `knock_shape` when its noise
    /// is not scaled by register and its shape not by the treble.
    pub fn knock_shape_of(&self, m: u8) -> [f64; super::voice::KNOCK_PARTS] {
        let mut s = self.knock_shape;
        let [bass, treble] = self.knock_noise_reg;
        if bass != 1.0 && m < 60 {
            s[0] *= Voicing::bass_scale(m, bass);
        }
        if treble != 1.0 && m > 60 {
            let t = ((m as f64 - 60.0) / (108.0 - 60.0)).clamp(0.0, 1.0);
            s[0] *= math::exp(t * math::ln(treble));
        }
        if m > KNOCK_TOP_FROM {
            // Round 42 (`chorda-c18`): the top keys' click — its fall, band, ring and the
            // ring's fall each scaled toward their values at C7; its rise and the ring's own
            // band moved toward theirs (none at C6).
            let t = ((m - KNOCK_TOP_FROM) as f64 / (KNOCK_TOP_TO - KNOCK_TOP_FROM) as f64)
                .clamp(0.0, 1.0);
            let ts = &self.knock_treble_shape;
            for (i, k) in [1, 3, 4, 5].into_iter().enumerate() {
                let g = ts[i];
                if g != 1.0 && g > 0.0 {
                    s[k] *= math::exp(t * math::ln(g));
                }
            }
            if ts[4] > 0.0 {
                s[8] += t * (ts[4] - s[8]);
            }
            if ts[5] > 0.0 {
                // From the shape's own band (or 20 kHz, nearly none), log in key.
                let from = if s[9] > 0.0 { s[9] } else { 20_000.0 };
                s[9] = math::exp(math::ln(from) + t * (math::ln(ts[5]) - math::ln(from)));
            }
        }
        s
    }

    /// The keyboard model 2 is designed for at the knobs' `base` tuning: its own
    /// asymptotes and treble octaves (none of the knobs' numbers moved). At 1, 1, 1 it is
    /// `base` itself — the same design as model 1's.
    pub fn tuning(&self, base: super::design::Tuning) -> super::design::Tuning {
        super::design::Tuning {
            bass_b: self.bass_b,
            treble_b: self.treble_b,
            treble_octave: self.treble_octave,
            treble_octave_hi: self.treble_octave_hi,
            frac_room: self.frac_room,
            tune: self.tune,
            ..base
        }
    }

    /// Key `key`'s evening (a gain on its strings, 1: none): what `power` — the board's
    /// power at a frequency, raw and smoothed over its third of an octave
    /// ([`super::body::Body::power_at`]) — gives its first [`EVEN_PARTIALS`] partials,
    /// each weighed by a struck string's spectrum (`sin²(nπβ)/n²` at the strike `β`, the
    /// hammer's corner `even_hz`) and the ear's; the key's own partial voicing `partials`
    /// (dB, partials 2 …) is made level-neutral the same way. Allocates nothing.
    pub fn even_of(
        &self,
        key: &super::tables::Key,
        partials: &[f64; PARTIALS],
        power: impl Fn(f64) -> Option<(f64, f64)>,
    ) -> f64 {
        if self.even == 0.0 || key.key as f64 >= EVEN_FADE_TO {
            return 1.0;
        }
        let (mut raw, mut smooth) = (0.0, 0.0);
        for n in 1..=EVEN_PARTIALS {
            let f = key.partial(n);
            let Some((p, ps)) = power(f) else {
                break;
            };
            let s = math::sin(n as f64 * std::f64::consts::PI * key.strike) / n as f64;
            let c = f / self.even_hz.max(1.0);
            // The ear's weighing (ITU-R BS.1770's: +4 dB above ~1.5 kHz, off below 38 Hz).
            let (hi, lo) = (
                f * f / (f * f + 1_500.0 * 1_500.0),
                f * f / (f * f + 38.0 * 38.0),
            );
            let w = s * s / (1.0 + c * c) * (1.0 + 1.512 * hi) * lo * lo;
            // The key's own partial voicing changes its colour, never its loudness: the
            // raw power has it, the smooth reference not.
            let i = n as usize;
            let voiced = if (2..2 + PARTIALS).contains(&i) {
                math::powf(10.0f64, partials[i - 2] / 10.0)
            } else {
                1.0
            };
            raw += w * voiced * p;
            smooth += w * ps;
        }
        if raw <= 0.0 || smooth <= 0.0 {
            return 1.0;
        }
        // Above the middle the treble's partials die within a few hundred milliseconds and
        // its strike carries much of its first second: the board's response at its lines no
        // longer says how loud a key is (measured: the corrections there came out
        // uncorrelated with the keys' levels), so the evening fades out over E5 … E6.
        let fade =
            ((EVEN_FADE_TO - key.key as f64) / (EVEN_FADE_TO - EVEN_FADE_FROM)).clamp(0.0, 1.0);
        let db =
            (10.0 * math::log10(smooth / raw)).clamp(-EVEN_LIMIT, EVEN_LIMIT) * self.even * fade;
        math::powf(10.0f64, db / 20.0)
    }

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
        let k = self.felt_k * math::exp(math::ln(y0) + t * (math::ln(y1) - math::ln(y0)));
        if self.felt_upper == 1.0 {
            return k;
        }
        let u = (1.0 - (m as f64 - 78.0).abs() / 12.0).clamp(0.0, 1.0);
        k * math::exp(u * math::ln(self.felt_upper))
    }

    /// The frequency-dependent loss's divisor for key `m` (MIDI): exactly `hf_decay` when
    /// the registers' scales are 1.
    pub fn hf_of(&self, m: u8) -> f64 {
        if self.hf_bass == 1.0 && self.hf_treble == 1.0 {
            return self.hf_decay;
        }
        let (x0, y0, x1, y1) = if m < 60 {
            (21.0, self.hf_bass, 60.0, 1.0)
        } else {
            (60.0, 1.0, 108.0, self.hf_treble)
        };
        let t = ((m as f64 - x0) / (x1 - x0)).clamp(0.0, 1.0);
        self.hf_decay * math::exp(math::ln(y0) + t * (math::ln(y1) - math::ln(y0)))
    }

    /// Key `m`'s partial voicing (dB): all 0 when `partials_amount` is.
    pub fn partials_of(&self, m: u8) -> [f64; PARTIALS] {
        if self.partials_amount == 0.0 {
            return [0.0; PARTIALS];
        }
        let i = (m.clamp(21, 108) - 21) as usize;
        self.partials[i].map(|g| g as f64 * self.partials_amount)
    }

    /// The coupling's scale for key `m` (MIDI): exactly `coupling` when `coupling_treble`
    /// is 1.
    pub fn coupling_of(&self, m: u8) -> f64 {
        if self.coupling_treble == 1.0 && self.coupling_bass == 1.0 {
            return self.coupling;
        }
        let t = ((m as f64 - 60.0) / (108.0 - 60.0)).clamp(0.0, 1.0);
        let c = self.coupling * math::exp(t * math::ln(self.coupling_treble));
        if self.coupling_bass == 1.0 {
            return c;
        }
        let b = ((60.0 - m as f64) / (60.0 - 21.0)).clamp(0.0, 1.0);
        c * math::exp(b * math::ln(self.coupling_bass))
    }

    /// The unison scatters' share at key `m`: 1 up to `uni_fade[0]`, 0 from `uni_fade[1]`,
    /// linear between (C5 and C6 by default).
    pub fn unison_fade(&self, m: u8) -> f64 {
        let [a, b] = self.uni_fade;
        if b <= a {
            return if (m as f64) < b { 1.0 } else { 0.0 };
        }
        ((b - m as f64) / (b - a)).clamp(0.0, 1.0)
    }

    /// The soft strikes' gain for key `m` at MIDI velocity `vel` (0…1): exactly 1 when
    /// `pp_trim` is all 0, and from velocity 75 up.
    pub fn pp_gain(&self, m: u8, vel: f64) -> f64 {
        if self.pp_trim.iter().all(|t| *t == 0.0) {
            return 1.0;
        }
        let x = m as f64;
        let (k0, k1, i) = if x < 60.0 {
            (21.0, 60.0, 0)
        } else if x < 84.0 {
            (60.0, 84.0, 1)
        } else {
            (84.0, 108.0, 2)
        };
        let t = ((x - k0) / (k1 - k0)).clamp(0.0, 1.0);
        let db = self.pp_trim[i] + (self.pp_trim[i + 1] - self.pp_trim[i]) * t;
        // 0 at velocity 75, 1 at velocity 30, on the velocity's own scale.
        let w = ((75.0 / 127.0 - vel) / ((75.0 - 30.0) / 127.0)).max(0.0);
        if w == 0.0 {
            return 1.0;
        }
        math::powf(10.0f64, -db * w / 20.0)
    }

    /// The strings' rise for key `m` (ms; exactly 0 when every anchor is).
    pub fn rise_of(&self, m: u8) -> f64 {
        if self.rise.iter().all(|r| *r == 0.0) {
            return 0.0;
        }
        let x = m as f64;
        if x <= 35.0 {
            return 0.0;
        }
        if x < 48.0 {
            return self.rise[0] * (x - 35.0) / 13.0;
        }
        let i = (((x - 48.0) / 12.0) as usize).min(4);
        if i == 4 {
            return self.rise[4];
        }
        let t = (x - 48.0 - 12.0 * i as f64) / 12.0;
        self.rise[i] + (self.rise[i + 1] - self.rise[i]) * t
    }

    /// A bass scale `at_a0` for key `m`: 1 from C4 up, log-interpolated to `at_a0` at A0
    /// (exactly 1 when `at_a0` is).
    /// A scale 1 up to key `from`, `at_c8` at C8, log-interpolated between (exactly 1 when
    /// `at_c8` is).
    pub fn top_scale(m: u8, from: f64, at_c8: f64) -> f64 {
        if at_c8 == 1.0 {
            return 1.0;
        }
        let t = ((m as f64 - from) / (108.0 - from).max(1.0)).clamp(0.0, 1.0);
        math::exp(t * math::ln(at_c8))
    }

    /// The extra unevenness `uni_weight_bass` gives key `m`: all of it at A0, none from C4.
    pub fn weight_bass(&self, m: u8) -> f64 {
        self.uni_weight_bass * ((60.0 - m as f64) / (60.0 - 21.0)).clamp(0.0, 1.0)
    }

    pub fn bass_scale(m: u8, at_a0: f64) -> f64 {
        if at_a0 == 1.0 {
            return 1.0;
        }
        let b = ((60.0 - m as f64) / (60.0 - 21.0)).clamp(0.0, 1.0);
        math::exp(b * math::ln(at_a0))
    }

    /// Key `m`'s own level (a gain; exactly 1 when `levels_amount` is 0).
    pub fn level_of(&self, m: u8) -> f64 {
        if self.levels_amount == 0.0 {
            return 1.0;
        }
        let i = (m.clamp(21, 108) - 21) as usize;
        math::powf(10.0f64, self.levels[i] as f64 * self.levels_amount / 20.0)
    }

    /// The knock's scale for key `m` (MIDI): exactly 1 when `knock_treble` is.
    pub fn knock_of(&self, m: u8) -> f64 {
        if self.knock_treble == 1.0 {
            return 1.0;
        }
        let t = ((m as f64 - 60.0) / (108.0 - 60.0)).clamp(0.0, 1.0);
        math::exp(t * math::ln(self.knock_treble))
    }

    /// The dampers' hold for key `m` (MIDI): exactly `damper_t60` when `damper_bass` is 1.
    pub fn damper_of(&self, m: u8) -> f64 {
        if self.damper_bass == 1.0 {
            return self.damper_t60;
        }
        let t = ((m as f64 - 21.0) / (60.0 - 21.0)).clamp(0.0, 1.0);
        self.damper_t60 * math::exp((1.0 - t) * math::ln(self.damper_bass))
    }

    /// Key `m`'s arrival-time difference at `fs` (samples; > 0: the right microphone
    /// later). 0 when `itd_ms` and `itd_jitter` are.
    pub fn lag_of(&self, m: u8, fs: f64) -> i64 {
        if self.itd_ms == 0.0 && self.itd_jitter == 0.0 && self.itd_ends == 0.0 {
            return 0;
        }
        let x = (m as f64 - 64.5) / 43.5;
        // A fixed scatter per key (the strings' places under a spaced pair are irregular).
        let mut z = (m as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x17D;
        z = (z ^ (z >> 29)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        let h = (z >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0;
        let f = if self.itd_shape == 0.0 {
            x
        } else {
            let k = self.itd_shape.abs();
            let k2 = k * k;
            (((x + 1.0) * (x + 1.0) + k2).sqrt() - ((x - 1.0) * (x - 1.0) + k2).sqrt())
                / ((4.0 + k2).sqrt() - k)
        };
        let mut ms = -self.itd_ms * f + self.itd_jitter * h;
        if self.itd_ends != 0.0 {
            ms -= self.itd_ends * x * x * x;
        }
        (ms * fs / 1000.0).round() as i64
    }

    /// The damped floor as a share of the strike's loudest (0: off).
    pub fn damped_floor(&self) -> f64 {
        if self.damped_db <= 0.0 {
            0.0
        } else {
            math::powf(10.0f64, -self.damped_db / 20.0)
        }
    }

    /// `reg.<bass|mid|treble>.<band index>` = `v` dB. False when the name is not one.
    fn set_reg(&mut self, key: &str, v: f64) -> bool {
        let mut it = key.split('.');
        let (Some("reg"), Some(r), Some(b), None) = (it.next(), it.next(), it.next(), it.next())
        else {
            return false;
        };
        let r = match r {
            "bass" => 0,
            "mid" => 1,
            "treble" => 2,
            _ => return false,
        };
        if b == "level" {
            self.reg_level[r] = v;
            return true;
        }
        match b.parse::<usize>() {
            Ok(b) if b < BANDS => {
                self.reg[r][b] = v;
                true
            }
            _ => false,
        }
    }

    /// Set `key` = `value` by name (the voicing lab's `key=v,key=v`). False when the name
    /// is not one.
    pub fn set(&mut self, key: &str, v: f64) -> bool {
        // Round 42 (`chorda-c17`): one cell of the partial table by name, `partial<n>.<key>`
        // (n 2…7, dB before `partials_amount`), so a spec can voice one key's partial alone.
        if let Some(rest) = key.strip_prefix("partial") {
            if let Some((n, k)) = rest.split_once('.') {
                return match (n.parse::<usize>(), k.parse::<u8>()) {
                    (Ok(n), Ok(m)) if (2..=7).contains(&n) && (21..=108).contains(&m) => {
                        self.partials[(m - 21) as usize][n - 2] = v as f32;
                        true
                    }
                    _ => false,
                };
            }
        }
        if let Some(k) = key.strip_prefix("hf_key.") {
            return match k.parse::<u8>() {
                Ok(m) if (21..=108).contains(&m) && v > 0.0 => {
                    self.hf_keys[(m - 21) as usize] = v as f32;
                    true
                }
                _ => false,
            };
        }
        if let Some(k) = key.strip_prefix("coupling.") {
            return match k.parse::<u8>() {
                Ok(m) if (21..=108).contains(&m) => {
                    self.couplings[(m - 21) as usize] = v as f32;
                    true
                }
                _ => false,
            };
        }
        if key == "uni_pair" {
            self.uni_pair = [v as f32; KEYS];
            return true;
        }
        for (prefix, table) in [
            ("uni_pair.", &mut self.uni_pair),
            ("uni_b.", &mut self.uni_b_keys),
        ] {
            if let Some(k) = key.strip_prefix(prefix) {
                return match k.parse::<u8>() {
                    Ok(m) if (21..=108).contains(&m) => {
                        table[(m - 21) as usize] = v as f32;
                        true
                    }
                    _ => false,
                };
            }
        }
        if key == "uni_tight" {
            self.uni_tight = [v as f32; KEYS];
            return true;
        }
        if let Some(k) = key.strip_prefix("uni_tight.") {
            return match k.parse::<u8>() {
                Ok(m) if (21..=108).contains(&m) => {
                    self.uni_tight[(m - 21) as usize] = v as f32;
                    true
                }
                _ => false,
            };
        }
        if let Some(k) = key.strip_prefix("tune.") {
            return match k.parse::<u8>() {
                Ok(m) if (21..=108).contains(&m) => {
                    self.tune[(m - 21) as usize] = v as f32;
                    true
                }
                _ => false,
            };
        }
        if let Some(k) = key.strip_prefix("draw.") {
            return match k.parse::<u8>() {
                Ok(m) if (21..=108).contains(&m) && (0.0..=255.0).contains(&v) => {
                    self.draws[(m - 21) as usize] = v as u8;
                    true
                }
                _ => false,
            };
        }
        let f = match key {
            "felt_k" => &mut self.felt_k,
            "reg_keys.bass" => &mut self.reg_keys[0],
            "reg_keys.mid" => &mut self.reg_keys[1],
            "reg_keys.treble" => &mut self.reg_keys[2],
            "felt_p" => &mut self.felt_p,
            "felt_bass" => &mut self.felt_bass,
            "felt_treble" => &mut self.felt_treble,
            "decay_bass" => &mut self.decay_bass,
            "decay_mid" => &mut self.decay_mid,
            "decay_treble" => &mut self.decay_treble,
            "hf_decay" => &mut self.hf_decay,
            "hf_bass" => &mut self.hf_bass,
            "hf_treble" => &mut self.hf_treble,
            "damped_db" => &mut self.damped_db,
            "damper_t60" => &mut self.damper_t60,
            "uni_weight" => &mut self.uni_weight,
            "uni_skew" => &mut self.uni_skew,
            "uni_rank" => &mut self.uni_rank,
            "polar" => &mut self.polar,
            "polar_ct" => &mut self.polar_ct,
            "polar_yz" => &mut self.polar_yz,
            "polar_damp" => &mut self.polar_damp,
            "loose_last" => &mut self.loose_last,
            "mass" => &mut self.mass,
            "partials_amount" => &mut self.partials_amount,
            "key_return" => &mut self.key_return,
            "top_hz" => &mut self.top_hz,
            "damper_bass" => &mut self.damper_bass,
            "quiet" => &mut self.quiet,
            "itd_ms" => &mut self.itd_ms,
            "itd_jitter" => &mut self.itd_jitter,
            "itd_shape" => &mut self.itd_shape,
            "formant.hz" => &mut self.formant[0],
            "formant.db" => &mut self.formant[1],
            "formant.q" => &mut self.formant[2],
            "coupling" => &mut self.coupling,
            "coupling_treble" => &mut self.coupling_treble,
            "knock_treble" => &mut self.knock_treble,
            "knock.noise" => &mut self.knock_shape[0],
            "knock.tau" => &mut self.knock_shape[1],
            "knock.lo" => &mut self.knock_shape[2],
            "knock.hi" => &mut self.knock_shape[3],
            "knock.tail" => &mut self.knock_shape[4],
            "knock.tail_tau" => &mut self.knock_shape[5],
            "knock.t60" => &mut self.knock_shape[6],
            "knock.noise_vel" => &mut self.knock_shape[7],
            "knock.noise_bass" => &mut self.knock_noise_reg[0],
            "knock.noise_treble" => &mut self.knock_noise_reg[1],
            "knock.rise" => &mut self.knock_shape[8],
            "knock.tail_hi" => &mut self.knock_shape[9],
            "knock.tau_treble" => &mut self.knock_treble_shape[0],
            "knock.hi_treble" => &mut self.knock_treble_shape[1],
            "knock.tail_treble" => &mut self.knock_treble_shape[2],
            "knock.tail_tau_treble" => &mut self.knock_treble_shape[3],
            "knock.rise_treble" => &mut self.knock_treble_shape[4],
            "knock.tail_hi_treble" => &mut self.knock_treble_shape[5],
            "knock_bass" => &mut self.knock_bass,
            "blow_bass" => &mut self.blow_bass,
            "vel_curve" => &mut self.vel_curve,
            "keyboard.bass_b" => &mut self.bass_b,
            "keyboard.treble_b" => &mut self.treble_b,
            "keyboard.treble_octave" => &mut self.treble_octave,
            "even" => &mut self.even,
            "even_hz" => &mut self.even_hz,
            "even_thud" => &mut self.even_thud,
            "levels_amount" => &mut self.levels_amount,
            "keyboard.treble_octave_hi" => &mut self.treble_octave_hi,
            "keyboard.frac_room" => &mut self.frac_room,
            "coupling_bass" => &mut self.coupling_bass,
            "bridge.n" => &mut self.bridge[0],
            "bridge.q" => &mut self.bridge[1],
            "bridge.g" => &mut self.bridge[2],
            "bridge.span" => &mut self.bridge[3],
            "vmin_treble" => &mut self.vmin_treble,
            "uni_b" => &mut self.uni_b,
            "uni_b_bass" => &mut self.uni_b_bass,
            "uni_b_hold" => &mut self.uni_b_hold,
            "uni_loss" => &mut self.uni_loss,
            "uni_center" => &mut self.uni_center,
            "bridge.tune" => &mut self.bridge_tune,
            "bridge.pull" => &mut self.bridge_pull,
            "bridge.pull_hz" => &mut self.bridge_pull_hz,
            "uni_loss_top" => &mut self.uni_loss_top,
            "uni_weight_low" => &mut self.uni_weight_low,
            "detune_bass" => &mut self.detune_bass,
            "detune_scale" => &mut self.detune_scale,
            "felt_upper" => &mut self.felt_upper,
            "knock_vel" => &mut self.knock_vel,
            "landing.tau" => &mut self.landing_shape[0],
            "landing.hi" => &mut self.landing_shape[1],
            "landing.modes" => &mut self.landing_shape[2],
            "landing.follow" => &mut self.landing_shape[3],
            "bass_hp" => &mut self.bass_hp,
            "bass_hp_order" => &mut self.bass_hp_order,
            "rise.c3" => &mut self.rise[0],
            "rise.c4" => &mut self.rise[1],
            "rise.c5" => &mut self.rise[2],
            "rise.c6" => &mut self.rise[3],
            "rise.c7" => &mut self.rise[4],
            "top_damper" => &mut self.top_damper,
            "landing.rise" => &mut self.landing_rise,
            "pp_trim.a0" => &mut self.pp_trim[0],
            "pp_trim.c4" => &mut self.pp_trim[1],
            "pp_trim.c6" => &mut self.pp_trim[2],
            "pp_trim.c8" => &mut self.pp_trim[3],
            "detune_top" => &mut self.detune_top[1],
            "detune_top.from" => &mut self.detune_top[0],
            "vel_top.from" => &mut self.vel_top[0],
            "vel_top.range" => &mut self.vel_top[1],
            "vel_top.curve" => &mut self.vel_top[2],
            "itd_ends" => &mut self.itd_ends,
            "uni_weight_bass" => &mut self.uni_weight_bass,
            "hf_top" => &mut self.hf_top[1],
            "hf_top.from" => &mut self.hf_top[0],
            "damper_key" => &mut self.damper_key,
            "damper_key_bass" => &mut self.damper_key_bass,
            "strike_true" => &mut self.strike_true,
            "vmin_bass" => &mut self.vmin_bass,
            "detune_low" => &mut self.detune_low[0],
            "detune_low.to" => &mut self.detune_low[1],
            "felt_soft" => &mut self.felt_soft[0],
            "felt_soft.from" => &mut self.felt_soft[1],
            "felt_soft.to" => &mut self.felt_soft[2],
            "uni_fade.from" => &mut self.uni_fade[0],
            "uni_fade.to" => &mut self.uni_fade[1],
            "knee" => &mut self.knee,
            "ceiling" => &mut self.ceiling,
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
            "body.onset_ms" => &mut self.body.onset_ms,
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
            _ => return self.set_reg(key, v),
        };
        *f = v;
        true
    }
}
