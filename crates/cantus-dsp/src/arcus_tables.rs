//! Arcus's tables (a child of `arcus`): each instrument's strings, its body, its range and
//! how its dynamics are bowed.
//!
//! **The strings** — open pitch, vibrating length, tension, and the decay of the open string
//! (its fundamental's T60 and the material's exponent above it). The characteristic impedance
//! the bow meets is `Z₀ = T / c` with `c = 2·L·f₀` (the wave speed of a string of that length
//! and pitch), so `Z₀` follows from the tension alone. Sources and their status:
//!
//! - Vibrating lengths: the standard full-size measures (violin 328 mm, viola 370 mm for a
//!   16-inch instrument, cello 690 mm, double bass 1 060 mm, an orchestral 3/4 bass).
//! - Tensions: the manufacturers' published ranges for medium gauges, used **as a range check,
//!   not as a source** (`research/instruments-next/strings-verified.md` §3.2: D'Addario's
//!   tables are an image, Pirastro's figures were read on retailers — Evah Pirazzi violin E
//!   7.8–8.5 kg, A 5.3–5.6, D ~4.8, G 4.6–4.9; NV). Percival's measured table (cited by
//!   Desvages & Bilbao, DAFx-15 §4.1) was not read (NV). Cello strings sit near 13–15 kgf,
//!   a bass's Spirocore set near 26–30 kgf (NV).
//! - Losses: the open string's fundamental T60 on its instrument (the bridge's loss into the
//!   body dominates a string's own, Mansour, Woodhouse & Scavone 2016 Part 1,
//!   doi:10.3813/aaa.919021, whose figures were not read: NV) chosen within what a plucked
//!   open string of each instrument rings — 2.5–6 s low, shorter high — and a material's
//!   exponent `p` (a partial's T60 falls as `(f/f₀)^(−p)`).
//! - Stiffness: Schelleng 1973 (doi:10.1121/1.1913322): "inharmonicity of 0.1 cent per square
//!   of mode number will not perceptibly degrade bowed-string performance". Orchestral strings'
//!   B ≈ 10⁻⁵ (0.009 cent per n²) sits far under it, so the loop holds no stiffness allpass.
//!
//! **The bodies** are Fides's (`fides::body`): the cello's modes as Fides measured them
//! (A0 near 103 Hz, the violin's signature modes scaled), the violin's (Bissinger 2008); the
//! viola's and the double bass's are **those scaled to their size** — no measured admittance
//! of either was found (§3.1's gap): the viola's A0 near 235 Hz (the violin's × 1/1.17), the
//! bass's near 60 Hz (the cello's × 1/1.72).

use crate::fides::body::BodyKind;

/// Which instrument, in `inst`'s order.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Instrument {
    #[default]
    Cello,
    Bass,
    Violin,
    Viola,
}

impl Instrument {
    pub const ALL: [Instrument; 4] = [
        Instrument::Cello,
        Instrument::Bass,
        Instrument::Violin,
        Instrument::Viola,
    ];

    pub fn from_index(i: f32) -> Instrument {
        let k = (i.round().max(0.0) as usize).min(Self::ALL.len() - 1);
        Self::ALL[k]
    }

    pub fn spec(self) -> &'static InstrumentSpec {
        match self {
            Instrument::Cello => &CELLO,
            Instrument::Bass => &BASS,
            Instrument::Violin => &super::upper::tables::VIOLIN, // r42:arcus-upper
            Instrument::Viola => &super::upper::tables::VIOLA,   // r42:arcus-upper
        }
    }
}

/// One string.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct StringSpec {
    /// The open string's MIDI key.
    pub open: u8,
    /// Vibrating length, m.
    pub length_m: f64,
    /// Tension, N.
    pub tension_n: f64,
    /// The open string's fundamental T60, s.
    pub t60_s: f64,
    /// The material's exponent: a partial's T60 falls as `(f/f₀)^(−p)`.
    pub p: f64,
}

impl StringSpec {
    /// The open string's frequency, Hz (A4 = 440).
    pub fn open_hz(&self) -> f64 {
        440.0 * crate::math::exp2((self.open as f64 - 69.0) / 12.0)
    }

    /// The characteristic impedance `Z₀ = T/c = T/(2·L·f₀)`, kg/s.
    pub fn z0(&self) -> f64 {
        self.tension_n / (2.0 * self.length_m * self.open_hz())
    }

    /// The linear mass density `μ = T/c²`, kg/m.
    pub fn mass(&self) -> f64 {
        let c = 2.0 * self.length_m * self.open_hz();
        self.tension_n / (c * c)
    }
}

/// How the dynamic `d` (0 pp … 1 ff) is bowed: speed and the bow–bridge distance move
/// together (Askenfelt 1989, doi:10.1121/1.398230: players raise the dynamic with speed and
/// force and play closer to the bridge — the abstract read; its tables NV), and the force is
/// placed inside the Schelleng region measured on this model (`docs/round42/arcus.md`).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Bowing {
    /// Bow speed at pp and ff, m/s (geometric between).
    pub speed: (f64, f64),
    /// β at pp and ff (linear between).
    pub beta: (f64, f64),
    /// The force as a share of Schelleng's upper limit at the bowing's speed and β, at pp and
    /// ff (the middle of the region measured on this model, in log terms).
    pub place: (f64, f64),
}

/// An instrument.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct InstrumentSpec {
    pub strings: [StringSpec; 4],
    /// The highest key it plays (on its top string).
    pub top: u8,
    /// Its body (Fides's) and its size over that body's measured one.
    pub body: BodyKind,
    pub body_size: f32,
    pub bowing: Bowing,
    /// Vibrato: rate (Hz) and depth (cents) by default (Geringer & Allen 2004, ~5.5 Hz; the
    /// violin's wider than the cello's).
    pub vib_hz: f32,
    pub vib_ct: f32,
    /// The output's trim (dB), so each instrument at mf sits at about the same level.
    pub trim_db: f32,
    /// The bridge's corner rounding, Hz (`string::StringStep::corner_hz`).
    pub corner_hz: f64,
    /// Where the body's radiation efficiency stops rising, Hz: a box radiates a slow
    /// vibration poorly (its pressure follows the volume velocity's rate of change, +6 dB an
    /// octave), so below this the strings' fundamentals are heard weaker than their partials
    /// — matched to the references by `tools/arcus/compare.sh`: the Smolken bass's D2 has
    /// its first harmonics level with its fundamental (120 Hz here), the Karoryfer cello's A3
    /// a strong fundamental (none: Fides's cello body alone).
    pub radiation_hz: f32,
    /// The register's balance: dB per semitone added to the output around `balance_key`, so
    /// a held mf note is about as loud low as high. Measured (`tests_gate2::probe_levels`, a
    /// held note every three semitones at velocity 0.6, integrated LUFS): the cello fell
    /// 0.47 dB a semitone from C2 to F♯5 (−21.5 to −39.9 LUFS; the body's own unevenness
    /// ±5 dB around the line, kept), the bass 0.16 (round 42's preset fit).
    pub balance_db_st: f32,
    pub balance_key: f32,
    /// Model 2's match to the recordings (`docs/round42/arcus-match.md`).
    pub matched: Match,
}

/// The match's correction bands (Hz) and their Q.
pub const EQ_BANDS: usize = 12;
pub const EQ_HZ: [f32; EQ_BANDS] = [
    63.0, 100.0, 160.0, 250.0, 400.0, 630.0, 1000.0, 1600.0, 2500.0, 4000.0, 6300.0, 10_000.0,
];
pub const EQ_Q: f32 = 1.4;

/// Model 2's match to the recordings (round 42): what `tools/arcus/match.sh` fitted, note by
/// note across the range, so the measurements fall within the references' spread.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Match {
    /// The output's correction, dB per band (`EQ_HZ`): the long-term spectrum of held notes.
    pub eq_db: [f32; EQ_BANDS],
    /// The bow-hair noise, dB over model 1's.
    pub noise_db: f32,
    /// A plucked note's ring, × model 1's.
    pub pizz_decay: f32,
    /// The vibrato's depth and rate, × model 1's.
    pub vib_depth: f32,
    pub vib_rate: f32,
}

/// No match: model 1.
pub const NEUTRAL: Match = Match {
    eq_db: [0.0; EQ_BANDS],
    noise_db: 0.0,
    pizz_decay: 1.0,
    vib_depth: 1.0,
    vib_rate: 1.0,
};

impl InstrumentSpec {
    /// The string a key is played on by default: the highest whose open pitch is at or
    /// under it (first position; a key under the lowest plays the lowest).
    pub fn string_for(&self, key: f32) -> usize {
        let mut s = 0;
        for (i, st) in self.strings.iter().enumerate() {
            if key + 1e-3 >= st.open as f32 {
                s = i;
            }
        }
        s
    }

    /// The lowest key it plays.
    pub fn bottom(&self) -> u8 {
        self.strings[0].open
    }
}

/// The cello: C2 G2 D3 A3 over 690 mm.
pub const CELLO: InstrumentSpec = InstrumentSpec {
    strings: [
        StringSpec {
            open: 36,
            length_m: 0.69,
            tension_n: 147.0,
            t60_s: 5.0,
            p: 0.61,
        },
        StringSpec {
            open: 43,
            length_m: 0.69,
            tension_n: 132.0,
            t60_s: 4.5,
            p: 0.61,
        },
        StringSpec {
            open: 50,
            length_m: 0.69,
            tension_n: 128.0,
            t60_s: 4.0,
            p: 0.55,
        },
        StringSpec {
            open: 57,
            length_m: 0.69,
            tension_n: 142.0,
            t60_s: 3.5,
            p: 0.50,
        },
    ],
    top: 79,
    body: BodyKind::Cello,
    body_size: 1.0,
    bowing: Bowing {
        speed: (0.05, 0.35),
        beta: (0.13, 0.075),
        place: (0.42, 0.27),
    },
    vib_hz: 5.5,
    vib_ct: 18.0,
    trim_db: 16.0,
    corner_hz: 3000.0,
    radiation_hz: 0.0,
    balance_db_st: 0.47,
    balance_key: 57.0,
    matched: CELLO_MATCH,
};

/// The double bass, in orchestral tuning: E1 A1 D2 G2 over 1 060 mm.
pub const BASS: InstrumentSpec = InstrumentSpec {
    strings: [
        StringSpec {
            open: 28,
            length_m: 1.06,
            tension_n: 290.0,
            t60_s: 6.0,
            p: 0.66,
        },
        StringSpec {
            open: 33,
            length_m: 1.06,
            tension_n: 290.0,
            t60_s: 5.5,
            p: 0.66,
        },
        StringSpec {
            open: 38,
            length_m: 1.06,
            tension_n: 280.0,
            t60_s: 5.0,
            p: 0.61,
        },
        StringSpec {
            open: 43,
            length_m: 1.06,
            tension_n: 260.0,
            t60_s: 4.5,
            p: 0.55,
        },
    ],
    top: 67,
    body: BodyKind::Cello,
    body_size: 1.72,
    bowing: Bowing {
        speed: (0.06, 0.45),
        beta: (0.14, 0.08),
        place: (0.42, 0.27),
    },
    vib_hz: 5.3,
    vib_ct: 16.0,
    trim_db: 16.0,
    corner_hz: 2000.0,
    radiation_hz: 120.0,
    balance_db_st: 0.16,
    balance_key: 47.5,
    matched: BASS_MATCH,
};

// The violin's and the viola's tables are `upper::tables` (`arcus_upper_tables.rs`). // r42:arcus-upper

/// The friction curve's constants: the hyperbolic law
/// `μ(Δv) = μd + (μs − μd)·v₀/(v₀ + |Δv|)` (Woodhouse's "hyperbolic" curve as Galluzzo,
/// Woodhouse & Mansour 2017 compare it, doi:10.3813/aaa.919136 — abstract read; the values
/// μs 0.8, μd 0.3, v₀ 0.1 m/s are the ones that literature uses for rosin, NV).
pub const MU_S: f32 = 0.8;
pub const MU_D: f32 = 0.3;
pub const V0: f32 = 0.1;

/// Schelleng's upper force limit for Helmholtz motion, N (Schelleng 1973; the bow's static
/// friction holding the string through the Helmholtz corner's step):
/// `F_max = 2·Z₀·v_b / ((μs − μd)·β)`.
pub fn schelleng_max(z0: f64, v_bow: f64, beta: f64) -> f64 {
    2.0 * z0 * v_bow / ((MU_S - MU_D) as f64 * beta.max(1e-3))
}

/// The cello's and the double bass's match (`tools/arcus/match.sh fit`, five passes on two
/// thirds of the notes; `docs/round42/arcus-match.md` has the held-out distances).
pub const CELLO_MATCH: Match = Match {
    eq_db: [
        -9.7, -6.9, -5.5, -3.1, -0.8, -1.4, -2.9, 0.6, 2.8, 1.8, 8.1, 12.0,
    ],
    noise_db: 0.0,
    pizz_decay: 0.81,
    vib_depth: 0.80,
    vib_rate: 1.00,
};
pub const BASS_MATCH: Match = Match {
    // The bass's correction is left flat: fitted (… 6.4 dB at 10 kHz, −8.4 at 630 Hz) it
    // moved the held-out notes *away* from the references (spectrum 4.76 → 5.07 dB, the
    // centroid 0 → 2.2 semitones outside their spread): their upper bands carry the rooms'
    // and the bows' noise more than the instrument (`docs/round42/arcus-match.md`).
    eq_db: [0.0; EQ_BANDS],
    noise_db: 0.0,
    pizz_decay: 0.67,
    vib_depth: 0.44,
    vib_rate: 1.0,
};
