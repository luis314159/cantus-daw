//! Nitor — the one-knob enhancer (round 41, ADR 0151). What FL Studio's Soundgoodizer (one
//! knob over curated Maximus settings), Low Lifter (the low end) and the Maximus/Emphasis
//! exciters do, in one unit — and level-matched, so what you hear when you turn it is the
//! colour and not the loudness. Its references for *function* (inspired by, never copied):
//! Soundgoodizer, Low Lifter, Aphex's Aural Exciter and Big Bottom, Waves MaxxBass and
//! Renaissance Bass, iZotope Ozone's Exciter. MPL-2.0: no outside code is in it.
//!
//! ## The signal path, per block
//!
//! ```text
//!        ┌─ × pre ─ Multiplex (the character, dynamic phase) ─ ÷ pre ─ tight ─ lift ─┐
//! in ─┬──┤                                                                            ├─ + ─ × match ─ mix ─ × out
//!     │  └─ the exciter by band (body, presence, air: harmonics only, 4× oversampled) ┤
//!     │  └─ the low end's sub (an octave down, or the song's tonic) and harmonics ───┘
//!     └─ dry ─────────────────────────────────────────────────────────────────── mix ┘
//! ```
//!
//! - **The amount** drives a **character** ([`Character`]): a recipe for Multiplex's dynamic
//!   path (four bands, downward and upward compression, a static tilt), the exciter's bands
//!   and the low end. At amount 0 every ratio is 1:1 and every level 0.
//! - **Level independence.** The character's thresholds are set for programme at −18 LUFS:
//!   a slow K-weighted loudness of the input brings any input there before Multiplex and
//!   takes it back after (`pre`), so the character acts alike on a quiet stem and on a loud
//!   bus. The exciter and the low end's harmonics are normalised by their own envelope, so
//!   they add the same share of harmonics at −40 dBFS as at −6 (an Aphex exciter adds more
//!   the louder it is driven).
//! - **The exciter** ([`excite`]) adds *only harmonics*: each band (body around 400 Hz,
//!   presence around 2.5 kHz, air above 7 kHz) is normalised, oversampled 4× (minimum
//!   phase), shaped by `v/√(1+v²)` (odd) and its square (even, the *colour* between them),
//!   less its own linear part, brought down and high-passed at the band's lower edge.
//! - **The low end** ([`low`]): the mono low band under the crossover feeds a **sub** — an
//!   octave down phase-locked to the bass (Saltus's divider), or a sine on the **song's
//!   tonic** gated by the low band — and **harmonics** (the bass heard on small speakers);
//!   **tight** shortens the low band's decay (a fast envelope over a slow one); **lift** is a
//!   bell on the tonic's fundamental (octave 1) and, half as high, its octave.
//! - **The level match** ([`level`]): the K-weighted loudness (BS.1770's filters) of the
//!   input and of the wet path, each over about 1.5 s, and their ratio glided — the output
//!   reads as loud as the input, which is what an honest A/B needs and what the presets'
//!   render check holds (±2 LU).
//!
//! ## The null, the grid and rest
//!
//! **Every default is a wire, to the bit**: with nothing to do the block is left as it came
//! (only `out` multiplies, and at 0 dB it is not applied). Each part runs only while its
//! level (or its glide) is above zero; when its glide lands on zero its state is cleared on
//! that sample, so what follows does not depend on where blocks fell. Everything runs per
//! sample; glides are linear and land exactly. **At rest**: every part idle, Multiplex at
//! rest, the match landed on unity with its meters cleared — a new unit's state.

use crate::multiplex::{self, BandSettings, Multiplex, Slope};

#[path = "nitor_excite.rs"]
pub mod excite;
#[path = "nitor_level.rs"]
pub mod level;
#[path = "nitor_low.rs"]
pub mod low;
#[path = "nitor_chunk.rs"]
mod chunk;
#[path = "nitor_run.rs"]
mod run;

#[cfg(test)]
#[path = "nitor_testkit.rs"]
pub(crate) mod testkit;
#[cfg(test)]
#[path = "nitor_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "nitor_tests_measure.rs"]
mod tests_measure;

/// The name the unit is shown by.
pub const NAME: &str = "Nitor";
/// The characters' Multiplex bands.
pub const BANDS: usize = 4;
/// How long a level, the amount and the mix take to reach a new value.
pub const GLIDE_MS: f32 = 20.0;
/// The low band's crossover's range, Hz.
pub const XOVER_MIN: f32 = 40.0;
pub const XOVER_MAX: f32 = 250.0;
/// The level the characters' thresholds are set for: the input is brought here, LUFS.
pub const REFERENCE_LUFS: f32 = -18.0;

/// A character: what the amount knob turns up.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Character {
    /// All-round polish: the lows held, the details lifted, a little air.
    Clear,
    /// Drums and beats: the lows tightened, the mids compressed slow enough to let the
    /// attack through, presence excited.
    Punch,
    /// Warmth: even harmonics in the body, the top smoothed, the bass rounder.
    Warm,
    /// Openness: the top lifted by upward compression and excited air.
    Air,
    /// Loudness and density: the four bands squeezed hard, every part a little.
    Loud,
}

impl Character {
    pub const ALL: [Character; 5] = [
        Character::Clear,
        Character::Punch,
        Character::Warm,
        Character::Air,
        Character::Loud,
    ];

    pub fn from_index(i: usize) -> Character {
        Self::ALL[i.min(Self::ALL.len() - 1)]
    }

    pub fn index(self) -> usize {
        self as usize
    }

    /// The character's recipe at amount 1.
    pub fn recipe(self) -> &'static Recipe {
        &RECIPES[self.index()]
    }
}

/// What the sub follows.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum SubMode {
    /// An octave under the bass that plays (Saltus's divider): always in tune with it.
    Octave,
    /// A sine on the song's tonic (octave 1, 32.7–61.7 Hz), opened by the low band.
    Tonic,
}

impl SubMode {
    pub fn from_index(i: usize) -> SubMode {
        if i == 1 {
            SubMode::Tonic
        } else {
            SubMode::Octave
        }
    }
}

/// One Multiplex band of a character at amount 1. The amount takes each ratio from 1:1 to
/// its own and each gain from 0 dB to its own; thresholds, ranges and times stay.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct BandRecipe {
    pub from_hz: f32,
    pub thr_db: f32,
    pub ratio: f32,
    pub range_db: f32,
    pub thr_lo_db: f32,
    /// Upward compression (> 1) under `thr_lo_db`.
    pub ratio_lo: f32,
    pub range_lo_db: f32,
    pub knee_db: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub gain_db: f32,
}

/// A character at amount 1: its Multiplex bands, and what it adds to the exciter's bands
/// (with its colour) and to the low end. The user's own levels add to these.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Recipe {
    pub bands: [BandRecipe; BANDS],
    /// Body, presence, air.
    pub excite: [f32; 3],
    pub color: f32,
    pub sub: f32,
    pub harm: f32,
    pub tight: f32,
    pub lift: f32,
}

const fn band(
    from_hz: f32,
    (thr_db, ratio, range_db): (f32, f32, f32),
    (thr_lo_db, ratio_lo, range_lo_db): (f32, f32, f32),
    (attack_ms, release_ms): (f32, f32),
    gain_db: f32,
) -> BandRecipe {
    BandRecipe {
        from_hz,
        thr_db,
        ratio,
        range_db,
        thr_lo_db,
        ratio_lo,
        range_lo_db,
        knee_db: 8.0,
        attack_ms,
        release_ms,
        gain_db,
    }
}

/// The band edges every character shares, Hz: lows, low mids, presence, air.
pub const EDGES: [f32; BANDS] = [20.0, 150.0, 1_500.0, 7_000.0];

/// The characters at amount 1 (thresholds against band peaks of programme at −18 LUFS,
/// measured: `nitor_tests_measure::band_levels_of_programme_at_the_reference`).
pub const RECIPES: [Recipe; 5] = [
    // Clear: the lows held, the mids and the top lifted from underneath, a little air.
    Recipe {
        bands: [
            band(EDGES[0], (-15.0, 2.0, 6.0), (-60.0, 1.0, 0.0), (20.0, 160.0), 0.0),
            band(EDGES[1], (-16.0, 1.6, 4.0), (-34.0, 1.4, 4.0), (15.0, 120.0), 0.0),
            band(EDGES[2], (-22.0, 1.8, 5.0), (-40.0, 1.6, 6.0), (8.0, 100.0), 1.0),
            band(EDGES[3], (-30.0, 1.5, 4.0), (-50.0, 1.8, 8.0), (5.0, 80.0), 1.5),
        ],
        excite: [0.0, 0.1, 0.25],
        color: 0.4,
        sub: 0.0,
        harm: 0.1,
        tight: 0.0,
        lift: 0.0,
    },
    // Punch: slow attacks so the hits pass, fast releases, the lows tightened.
    Recipe {
        bands: [
            band(EDGES[0], (-15.0, 3.0, 8.0), (-60.0, 1.0, 0.0), (25.0, 100.0), 0.5),
            band(EDGES[1], (-17.0, 2.5, 6.0), (-60.0, 1.0, 0.0), (20.0, 90.0), 0.0),
            band(EDGES[2], (-20.0, 2.0, 5.0), (-36.0, 1.5, 5.0), (15.0, 90.0), 0.5),
            band(EDGES[3], (-30.0, 1.5, 4.0), (-60.0, 1.0, 0.0), (8.0, 80.0), 0.0),
        ],
        excite: [0.1, 0.35, 0.0],
        color: 0.6,
        sub: 0.0,
        harm: 0.2,
        tight: 0.45,
        lift: 0.0,
    },
    // Warm: even harmonics in the body, the top smoothed, the bass rounder.
    Recipe {
        bands: [
            band(EDGES[0], (-15.0, 1.5, 4.0), (-60.0, 1.0, 0.0), (20.0, 180.0), 1.0),
            band(EDGES[1], (-18.0, 1.4, 4.0), (-32.0, 1.6, 5.0), (20.0, 150.0), 0.5),
            band(EDGES[2], (-24.0, 2.0, 5.0), (-60.0, 1.0, 0.0), (5.0, 80.0), 0.0),
            band(EDGES[3], (-32.0, 2.0, 5.0), (-60.0, 1.0, 0.0), (3.0, 60.0), -1.5),
        ],
        excite: [0.45, 0.0, 0.0],
        color: 0.15,
        sub: 0.0,
        harm: 0.25,
        tight: 0.0,
        lift: 0.2,
    },
    // Air: the top lifted from underneath and excited.
    Recipe {
        bands: [
            band(EDGES[0], (-15.0, 1.5, 4.0), (-60.0, 1.0, 0.0), (20.0, 160.0), 0.0),
            band(EDGES[1], (-16.0, 1.3, 3.0), (-60.0, 1.0, 0.0), (15.0, 120.0), 0.0),
            band(EDGES[2], (-22.0, 1.4, 4.0), (-40.0, 1.8, 8.0), (8.0, 100.0), 1.0),
            band(EDGES[3], (-26.0, 1.5, 4.0), (-48.0, 2.2, 10.0), (5.0, 80.0), 2.5),
        ],
        excite: [0.0, 0.2, 0.55],
        color: 0.5,
        sub: 0.0,
        harm: 0.0,
        tight: 0.0,
        lift: 0.0,
    },
    // Loud: every band squeezed, from above and below; a little of every part.
    Recipe {
        bands: [
            band(EDGES[0], (-20.0, 4.0, 10.0), (-40.0, 1.6, 6.0), (15.0, 90.0), 0.5),
            band(EDGES[1], (-22.0, 4.0, 10.0), (-40.0, 2.0, 8.0), (10.0, 80.0), 0.0),
            band(EDGES[2], (-28.0, 4.0, 10.0), (-46.0, 2.0, 8.0), (6.0, 70.0), 1.0),
            band(EDGES[3], (-36.0, 3.0, 8.0), (-54.0, 2.0, 8.0), (4.0, 60.0), 1.5),
        ],
        excite: [0.15, 0.2, 0.2],
        color: 0.5,
        sub: 0.0,
        harm: 0.3,
        tight: 0.25,
        lift: 0.0,
    },
];

/// Everything Nitor is, in real units. **`Default` is the null**: amount 0, every level 0,
/// the output at 0 dB.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct NitorSettings {
    /// 0 … 1: how much of the character.
    pub amount: f32,
    pub character: Character,
    /// The low end's own levels, 0 … 1 (the character's are added).
    pub sub: f32,
    pub sub_mode: SubMode,
    pub harm: f32,
    pub tight: f32,
    pub lift: f32,
    /// The low band's crossover, Hz.
    pub xover_hz: f32,
    /// The tonic the sub and the lift tune to: 0 = C … 11 = B.
    pub tonic: u8,
    /// The exciter's own levels, 0 … 1: body, presence, air (the character's are added).
    pub excite: [f32; 3],
    /// 0: even harmonics (warm) … 1: odd (bright, edgy).
    pub color: f32,
    /// The output matched in loudness to the input.
    pub level_match: bool,
    /// 0 … 1.
    pub mix: f32,
    pub out_db: f32,
}

impl Default for NitorSettings {
    fn default() -> Self {
        NitorSettings {
            amount: 0.0,
            character: Character::Clear,
            sub: 0.0,
            sub_mode: SubMode::Octave,
            harm: 0.0,
            tight: 0.0,
            lift: 0.0,
            xover_hz: 100.0,
            tonic: 0,
            excite: [0.0; 3],
            color: 0.5,
            level_match: true,
            mix: 1.0,
            out_db: 0.0,
        }
    }
}

/// The levels each part runs at once the character and the user's own are added: what the
/// audio thread glides to (also what the editor draws).
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Levels {
    pub excite: [f32; 3],
    /// Each exciter band's colour, the character's and the user's weighted by their levels.
    pub color: [f32; 3],
    pub sub: f32,
    pub harm: f32,
    pub tight: f32,
    pub lift: f32,
}

impl NitorSettings {
    /// The levels in force: the character's at the amount, plus the user's own, each 0 … 1.
    pub fn levels(&self) -> Levels {
        let r = self.character.recipe();
        let a = self.amount.clamp(0.0, 1.0);
        let add = |own: f32, rec: f32| (own.clamp(0.0, 1.0) + a * rec).min(1.0);
        let mut l = Levels {
            sub: add(self.sub, r.sub),
            harm: add(self.harm, r.harm),
            tight: add(self.tight, r.tight),
            lift: add(self.lift, r.lift),
            ..Levels::default()
        };
        for i in 0..3 {
            let own = self.excite[i].clamp(0.0, 1.0);
            let rec = a * r.excite[i];
            l.excite[i] = (own + rec).min(1.0);
            l.color[i] = if own + rec > 0.0 {
                (own * self.color.clamp(0.0, 1.0) + rec * r.color) / (own + rec)
            } else {
                self.color.clamp(0.0, 1.0)
            };
        }
        l
    }

    /// Multiplex's settings for the character at the amount: every ratio 1:1 and every gain
    /// 0 dB at amount 0 (a wire).
    pub fn dynamics(&self) -> multiplex::Settings {
        let r = self.character.recipe();
        let a = self.amount.clamp(0.0, 1.0);
        let mut s = multiplex::Settings {
            mode: multiplex::Mode::Dynamic,
            slope: Slope::S24,
            ..multiplex::Settings::default()
        };
        for (b, rb) in s.bands.iter_mut().zip(&r.bands) {
            *b = BandSettings {
                used: true,
                on: true,
                from_hz: rb.from_hz,
                thr_db: rb.thr_db,
                ratio: 1.0 + a * (rb.ratio - 1.0),
                range_db: rb.range_db,
                thr_lo_db: rb.thr_lo_db,
                ratio_lo: 1.0 + a * (rb.ratio_lo - 1.0),
                range_lo_db: rb.range_lo_db,
                knee_db: rb.knee_db,
                attack_ms: rb.attack_ms,
                release_ms: rb.release_ms,
                gain_db: a * rb.gain_db,
                ..BandSettings::default()
            };
        }
        s
    }

    /// Whether anything but the output gain can touch the signal.
    pub fn colours(&self) -> bool {
        let l = self.levels();
        self.mix > 0.0
            && (self.amount > 0.0
                || l.excite.iter().any(|v| *v > 0.0)
                || l.sub > 0.0
                || l.harm > 0.0
                || l.tight > 0.0
                || l.lift > 0.0)
    }
}

/// The tonic's fundamental in octave 1 (C1 = 32.70 Hz … B1 = 61.74 Hz), A4 = 440 Hz.
pub fn tonic_hz(tonic: u8) -> f32 {
    let midi = 24.0 + f32::from(tonic % 12);
    440.0 * crate::math::exp2((midi - 69.0) / 12.0)
}

/// Nitor: see the module notes.
#[derive(Clone, Debug)]
pub struct Nitor {
    sr: f32,
    target: NitorSettings,
    configured: bool,
    /// The character's dynamics: run only while blended in (`ramps.dynamics`).
    dynamics: Multiplex,
    exciter: excite::Exciter,
    low: low::Low,
    level: level::Level,
    /// The parts' levels, gliding.
    ramps: run::Ramps,
    /// Glides: the mix, the output gain.
    mix: run::Ramp,
    out: run::Ramp,
    /// The level holds something (it was fed since it was last cleared).
    level_live: bool,
    /// The dynamics wait, fresh, for the first cell that carries sound (`nitor_chunk`).
    parked: bool,
    /// Samples of exact silence in a row, on the grid, and how many park the dynamics.
    zero_run: u64,
    park_after: u64,
    /// The lift's level and its two gains as last computed (they change only as it glides).
    lift_cache: (f32, f32, f32),
    /// Where in the current grid cell, and what the cell runs.
    phase: usize,
    cell: chunk::Cell,
    /// Frames since the last reset (what `skip` adds to as well).
    clock: u64,
    peaks: run::PartPeaks,
    meter: Meter,
}

/// What the editor reads, refreshed each block.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Meter {
    pub in_peak: f32,
    pub out_peak: f32,
    /// The level match's gain, and the pre-gain into the character, dB.
    pub match_db: f32,
    pub pre_db: f32,
    /// Multiplex's band gains now, dB.
    pub band_db: [f32; BANDS],
    /// The exciter's harmonics added, per band, peak in the block (linear).
    pub excite: [f32; 3],
    /// The sub's and the low harmonics' peaks in the block, the tight gain now (dB).
    pub sub: f32,
    pub harm: f32,
    pub tight_db: f32,
    /// The sub's frequency now, Hz (0: none).
    pub sub_hz: f32,
    /// Something ran in the block.
    pub active: bool,
}
