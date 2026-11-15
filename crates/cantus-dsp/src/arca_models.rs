//! Arca's five amps, each a family of published circuits (generic names in the interface;
//! no product's name, logo or look). Values are the schematics'; where a text source could
//! not confirm one, the family's usual value is used and marked *typical* here and in
//! ADR 0137.
//!
//! | Model | Family (published schematic) | Preamp | Power |
//! |---|---|---|---|
//! | Clean | Fender AB763 (Deluxe Reverb / Twin Reverb vibrato channel) | 12AX7 100k/1.5k/25 µF → TMB (100k, 250 pF, 0.1 µF, 0.047 µF; 250k, 250k, 10k) → 1M volume → 12AX7 100k/1.64k/12.5 µF | 2 × 6V6GT, 6.6k, NFB 820 Ω/47 Ω from 8 Ω, tube rectifier |
//! | Crunch | Marshall JTM45 / 1959 "Plexi" bright channel | 12AX7 100k/2.7k/0.68 µF → 2.2 nF → 1M volume (5 nF bright) → 470k ‖ 470 pF → 12AX7 100k/820/0.68 µF → follower → TMB (33k, 470 pF, 22 nF, 22 nF; 220k, 1M, 25k) | 2 × EL34, 3.4k, NFB with presence, solid-state rectifier |
//! | Chime | Vox AC30 Top Boost | 12AX7 220k/1.5k shared/25 µF → 47 nF → 500k volume (100 pF) → 12AX7 gain + follower → top-boost tone (47 pF, 22 nF, 22 nF, 100k, 10k; 1M, 1M) | 4 × EL84 cathode-biased (47 Ω), 4k, no NFB, cut 250k + 4.7 nF, tube rectifier |
//! | High gain | Soldano SLO-100 overdrive channel | V1 220k/1.8k/1 µF → 470k ‖ 2 nF → 500k gain (1 nF bright) → V2 100k/1.8k → 470k, 1M, 220k → cold clipper 100k ‖ 1 nF/39k → V4 220k/1.8k/1 µF → follower → TMB (47k slope) | 4 × 6L6GC, 2.2k, NFB 39k, solid-state rectifier |
//! | Bass | Fender 5F6-A Bassman normal channel | 12AY7 100k/820 shared/250 µF → 1M volume → 12AX7 100k/820 → follower → TMB (56k, 250 pF, 20 nF, 20 nF; 250k, 1M, 25k — Yeh & Smith's) | 2 × 5881 (6L6), 2k, NFB with presence, tube rectifier |

use super::net::{Block, Knob, Taper, Tone, Triode};
use super::power::{Bias, Feedback, PowerSpec};
use super::tube::{Pentode, AX7, AY7, EL34, EL84, L6GC, V6GT};

/// The amps.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum AmpModel {
    Clean,
    Crunch,
    Chime,
    HighGain,
    Bass,
}

impl AmpModel {
    pub const ALL: [AmpModel; 5] = [
        AmpModel::Clean,
        AmpModel::Crunch,
        AmpModel::Chime,
        AmpModel::HighGain,
        AmpModel::Bass,
    ];

    pub fn index(self) -> usize {
        AmpModel::ALL.iter().position(|m| *m == self).unwrap_or(0)
    }

    pub fn from_index(i: usize) -> AmpModel {
        AmpModel::ALL[i.min(4)]
    }

    /// The preamp, left to right (input jack to the phase inverter's grid).
    pub fn preamp(self) -> &'static [Block] {
        match self {
            AmpModel::Clean => CLEAN,
            AmpModel::Crunch => CRUNCH,
            AmpModel::Chime => CHIME,
            AmpModel::HighGain => HIGH_GAIN,
            AmpModel::Bass => BASS,
        }
    }

    pub fn power(self) -> PowerSpec {
        match self {
            AmpModel::Clean => P_CLEAN,
            AmpModel::Crunch => P_CRUNCH,
            AmpModel::Chime => P_CHIME,
            AmpModel::HighGain => P_HIGH_GAIN,
            AmpModel::Bass => P_BASS,
        }
    }

    /// Whether its presence knob is the cut control (turned the other way).
    pub fn has_cut(self) -> bool {
        self.power().cut.is_some()
    }

    /// Whether it has a presence (or cut) control at all.
    pub fn has_presence(self) -> bool {
        let p = self.power();
        p.cut.is_some() || p.feedback.is_some_and(|f| f.presence.is_some())
    }
}

const fn tri(tube: crate::fragor::dk::Koren, rp: f64, rk: f64, ck: Option<f64>, bplus: f64) -> Triode {
    Triode {
        tube,
        rp,
        rk,
        ck,
        cp: None,
        bplus,
    }
}

const fn gain_pot(r: f64, bright: Option<f64>) -> Block {
    Block::Pot {
        r,
        taper: Taper::Log,
        knob: Knob::Gain,
        bright,
    }
}

/// The input jack: 1M to ground, then the grid stopper.
const fn jack(stopper: f64) -> [Block; 2] {
    [Block::Leak(1e6), Block::Series(stopper, None)]
}

const J68: [Block; 2] = jack(68e3);

static CLEAN: &[Block] = &[
    J68[0],
    J68[1],
    Block::Triode(tri(AX7, 100e3, 1.5e3, Some(25e-6), 280.0)),
    Block::Tone(Tone::Tmb {
        slope: 100e3,
        c1: 250e-12,
        c2: 0.1e-6,
        c3: 0.047e-6,
        treble: (250e3, Taper::Lin),
        bass: (250e3, Taper::Log),
        mid: (10e3, Taper::Lin),
    }),
    gain_pot(1e6, None),
    // V1B/V2B share 820 Ω and 25 µF: each half sees 1.64k and 12.5 µF.
    Block::Triode(tri(AX7, 100e3, 1.64e3, Some(12.5e-6), 280.0)),
    // Into the reverb mixer's 220k and the inverter's grid (typical 1M).
    Block::Couple(0.02e-6),
    Block::Series(220e3, None),
    Block::Leak(1e6),
];

static CRUNCH: &[Block] = &[
    J68[0],
    J68[1],
    Block::Triode(tri(AX7, 100e3, 2.7e3, Some(0.68e-6), 275.0)),
    Block::Couple(2.2e-9),
    gain_pot(1e6, Some(5e-9)),
    Block::Series(470e3, Some(470e-12)),
    // The other channel's mixing resistor, its volume down.
    Block::Leak(470e3),
    Block::Triode(tri(AX7, 100e3, 820.0, Some(0.68e-6), 275.0)),
    // Typical: the follower's 100k (its 615 Ω output is published).
    Block::Follower(100e3),
    Block::Tone(Tone::Tmb {
        slope: 33e3,
        c1: 470e-12,
        c2: 22e-9,
        c3: 22e-9,
        treble: (220e3, Taper::Lin),
        bass: (1e6, Taper::Log),
        mid: (25e3, Taper::Lin),
    }),
    Block::Leak(517e3),
];

static CHIME: &[Block] = &[
    J68[0],
    J68[1],
    // A shared 1.5k and 25 µF: each half sees 3k and 12.5 µF.
    Block::Triode(tri(AX7, 220e3, 3e3, Some(12.5e-6), 275.0)),
    Block::Couple(0.047e-6),
    gain_pot(500e3, Some(100e-12)),
    // The top boost's gain stage (typical: 220k, 1.5k unbypassed) and follower (typical
    // 56k; its 612–717 Ω output is published).
    Block::Triode(tri(AX7, 220e3, 1.5e3, None, 275.0)),
    Block::Follower(56e3),
    Block::Tone(Tone::Vox {
        r1: 100e3,
        r2: 10e3,
        c1: 47e-12,
        c2: 22e-9,
        c3: 22e-9,
        treble: 1e6,
        bass: 1e6,
    }),
    Block::Leak(600e3),
];

static HIGH_GAIN: &[Block] = &[
    J68[0],
    J68[1],
    Block::Triode(tri(AX7, 220e3, 1.8e3, Some(1e-6), 360.0)),
    Block::Couple(0.02e-6),
    Block::Series(470e3, Some(2e-9)),
    gain_pot(500e3, Some(1e-9)),
    // Typical: V2's cathode bypassed as V1's.
    Block::Triode(tri(AX7, 100e3, 1.8e3, Some(1e-6), 350.0)),
    Block::Couple(0.02e-6),
    Block::Series(470e3, None),
    Block::Leak(1e6),
    Block::Series(220e3, None),
    // The cold clipper: 39k unbypassed, 1 nF across its plate resistor.
    Block::Triode(Triode {
        tube: AX7,
        rp: 100e3,
        rk: 39e3,
        ck: None,
        cp: Some(1e-9),
        bplus: 350.0,
    }),
    // Typical coupling into V4.
    Block::Couple(0.022e-6),
    Block::Leak(1e6),
    Block::Triode(tri(AX7, 220e3, 1.8e3, Some(1e-6), 350.0)),
    Block::Follower(100e3),
    // The 47k slope is published; the rest the family's (typical).
    Block::Tone(Tone::Tmb {
        slope: 47e3,
        c1: 470e-12,
        c2: 22e-9,
        c3: 22e-9,
        treble: (220e3, Taper::Lin),
        bass: (1e6, Taper::Log),
        mid: (25e3, Taper::Lin),
    }),
    Block::Leak(1e6),
];

static BASS: &[Block] = &[
    Block::Leak(1e6),
    Block::Series(34e3, None),
    // V1 (12AY7) shares 820 Ω and 250 µF with its other half.
    Block::Triode(tri(AY7, 100e3, 1.64e3, Some(125e-6), 325.0)),
    Block::Couple(0.02e-6),
    gain_pot(1e6, None),
    Block::Triode(tri(AX7, 100e3, 820.0, None, 325.0)),
    // Typical: the follower's 100k.
    Block::Follower(100e3),
    Block::Tone(Tone::Tmb {
        slope: 56e3,
        c1: 250e-12,
        c2: 20e-9,
        c3: 20e-9,
        treble: (250e3, Taper::Lin),
        bass: (1e6, Taper::Log),
        mid: (25e3, Taper::Lin),
    }),
    Block::Leak(1e6),
];

const fn power(tube: Pentode, per_side: f64, supply: f64, raa: f64, watts: f64) -> PowerSpec {
    PowerSpec {
        tube,
        per_side,
        supply,
        r_supply: 30.0,
        c_filter: 50e-6,
        raa,
        r_speaker: 8.0,
        bias: Bias::Fixed { idle: 0.035 },
        feedback: None,
        cut: None,
        pi_gain: 25.0,
        pi_swing: 70.0,
        pi_r: 40e3,
        c_couple: 0.1e-6,
        r_grid: 220e3,
        watts,
        low_hz: 30.0,
        high_hz: 18e3,
    }
}

const P_CLEAN: PowerSpec = PowerSpec {
    r_supply: 180.0,
    c_filter: 16e-6,
    bias: Bias::Fixed { idle: 0.023 },
    feedback: Some(Feedback {
        tap: 8.0,
        r_fb: 820.0,
        r_tail: 47.0,
        presence: None,
    }),
    ..power(V6GT, 1.0, 435.0, 6.6e3, 22.0)
};

const P_CRUNCH: PowerSpec = PowerSpec {
    r_speaker: 16.0,
    bias: Bias::Fixed { idle: 0.036 },
    // Typical: 100k from the 16 Ω tap into 4.7k, the presence 5k and 0.1 µF.
    feedback: Some(Feedback {
        tap: 16.0,
        r_fb: 100e3,
        r_tail: 4.7e3,
        presence: Some((5e3, 0.1e-6)),
    }),
    ..power(EL34, 1.0, 470.0, 3.4e3, 50.0)
};

const P_CHIME: PowerSpec = PowerSpec {
    r_supply: 200.0,
    c_filter: 32e-6,
    // Typical: the "fat" cathode capacitor (500 µF).
    bias: Bias::Cathode {
        rk: 47.0,
        ck: 500e-6,
    },
    cut: Some((250e3, 4.7e-9)),
    low_hz: 40.0,
    ..power(EL84, 2.0, 335.0, 4e3, 30.0)
};

const P_HIGH_GAIN: PowerSpec = PowerSpec {
    bias: Bias::Fixed { idle: 0.040 },
    // 39k published; the tail and presence typical.
    feedback: Some(Feedback {
        tap: 8.0,
        r_fb: 39e3,
        r_tail: 4.7e3,
        presence: Some((25e3, 0.1e-6)),
    }),
    ..power(L6GC, 2.0, 475.0, 2.2e3, 100.0)
};

const P_BASS: PowerSpec = PowerSpec {
    r_supply: 150.0,
    c_filter: 32e-6,
    r_speaker: 2.0,
    bias: Bias::Fixed { idle: 0.035 },
    // Typical: 27k from the 2 Ω tap into 100 Ω, the presence 5k and 0.1 µF.
    feedback: Some(Feedback {
        tap: 2.0,
        r_fb: 27e3,
        r_tail: 100.0,
        presence: Some((5e3, 0.1e-6)),
    }),
    low_hz: 20.0,
    ..power(L6GC, 1.0, 440.0, 2e3, 45.0)
};
