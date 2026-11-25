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
    /// r42:arcus-p5 — model 2's vibrato, the players' as the study measured it
    /// (`research/instruments-next/bowed-realism.md`, *The vibrato*); model 1 plays
    /// `vib_hz` and `vib_ct`.
    pub vib: VibSpec,
    /// The output's trim (dB), so each instrument at mf sits at about the same level.
    pub trim_db: f32,
    /// The bridge's corner rounding, Hz (`string::StringStep::corner_hz`).
    pub corner_hz: f64,
    /// A sustain's swell over its clean start: its length (s) at pp and ff (`T₀ + (T₁ − T₀)·d²`)
    /// and how far under its level it starts (dB, linear in the dynamic) — pass 4: the
    /// instrument's own, fitted to its family's rise (the study §2.5).
    pub swell_s: (f32, f32),
    pub swell_db: (f32, f32),
    /// Pass 4: a pluck's T60 against frequency, `(Hz, s)` (the study §2.7: the references'
    /// per-partial T60s, the band's middle; log-log between, held at the ends) — the string's
    /// loss set from it at the note's fundamental and eighth partial. Empty: the bowed decay.
    pub pizz_t60: &'static [(f64, f64)],
    /// r42:arcus-p5 — between a spiccato's bounces the string rings free, its T60 × this:
    /// fitted to the references' strokes (`tools/arcus/pass5/strokes.py`: VSCO's solo violin
    /// and contrabass, single strokes, and Philharmonia's cello and viola phrases — how far a
    /// stroke falls 40–250 ms after its peak).
    pub spic_ring: f32,
    /// r42:arcus-p5 — a tremolo's level over its light stroke's, dB: about a sustain's at
    /// the same dynamic (`tests_p5::probe_tremolo_level`: the stroke alone sat 4–17 dB under).
    pub trem_db: f32,
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
    body: BodyKind::MeasuredCello,
    body_size: 1.0,
    // The players' map (the study §3.4, Askenfelt, Schoonderwaldt): from pp to ff the force
    // ×4.5, the bow nearer the bridge ×0.46, the speed nearly constant (×1.8).
    bowing: Bowing {
        speed: (0.15, 0.27),
        beta: (0.14, 0.07),
        place: (0.30, 0.36),
    },
    vib_hz: 5.5,
    vib_ct: 18.0,
    vib: CELLO_VIB, // r42:arcus-p5
    trim_db: 16.0,
    // Pass 3: 3 kHz rounded the cello's upper partials under the cellos' band at ff (the
    // bow near the bridge could not brighten it): 6 kHz.
    corner_hz: 6000.0,
    // Pass 4: the cellos' rise 0.14–0.28 s soft, 0.10–0.37 middle, 0.01–0.07 loud.
    swell_s: (0.30, 0.05),
    swell_db: (20.0, 10.0),
    pizz_t60: CELLO_PIZZ_T60,
    spic_ring: 0.4, // r42:arcus-p5
    trem_db: 8.0,   // r42:arcus-p5
    // Pass 4: the bow's static push on the string reached the bridge as a DC force the body
    // passed (most of the first second's power, decaying with the string's loss): no box
    // radiates it, so the measured body's radiation ends in a high-pass under the strings.
    radiation_hz: RADIATION_SHARE * 65.406, // r42:arcus-p5: C2
    balance_db_st: 0.47,
    balance_key: 57.0,
    matched: CELLO_MATCH,
};

/// Pass 4: the cellos' pluck T60 by frequency (the study's `pizz_t60_by_freq`, 7 cellos:
/// 4–5 s at 60–125 Hz, 2.5 s at 250, 1.1 s at 1 kHz, 0.7–0.8 s at 2.5–4 kHz).
pub const CELLO_PIZZ_T60: &[(f64, f64)] = &[
    (62.0, 5.0),
    (100.0, 3.9),
    (160.0, 3.2),
    (250.0, 2.8),
    (400.0, 2.5),
    (500.0, 2.2),
    (630.0, 1.65),
    (800.0, 1.41),
    (1000.0, 1.15),
    (1600.0, 0.92),
    (2500.0, 0.78),
    (4000.0, 0.69),
];

/// Pass 4: the basses' (5 basses: ~3 s at 80–200 Hz, 2.1 s at 400, 1.7 s at 1 kHz, 1.2 s at
/// 2 kHz; the 39–62 Hz bands' spread is wide).
pub const BASS_PIZZ_T60: &[(f64, f64)] = &[
    (40.0, 4.0),
    (62.0, 2.7),
    (100.0, 2.8),
    (160.0, 2.9),
    (200.0, 2.8),
    (250.0, 2.4),
    (400.0, 2.1),
    (500.0, 1.8),
    (800.0, 1.72),
    (1000.0, 1.7),
    (1600.0, 1.15),
    (2000.0, 1.1),
    (3200.0, 1.0),
];

/// Pass 4: `table`'s T60 at `hz` (log-log between its points, held at its ends).
pub fn t60_at(table: &[(f64, f64)], hz: f64) -> f64 {
    let (first, last) = (table[0], table[table.len() - 1]);
    if hz <= first.0 {
        return first.1;
    }
    if hz >= last.0 {
        return last.1;
    }
    let i = table
        .iter()
        .position(|p| p.0 >= hz)
        .unwrap_or(table.len() - 1)
        .max(1);
    let (a, b) = (table[i - 1], table[i]);
    let m = |x: f64| crate::math::ln(x);
    let u = m(hz / a.0) / m(b.0 / a.0);
    crate::math::exp(m(a.1) + (m(b.1) - m(a.1)) * u)
}

/// Pass 4: the measured bodies' radiation corner under the strings' fundamentals, Hz (two
/// first-order stages: −0.3 dB at the bass's E1, −0.1 at the cello's C2): what a body never
/// radiates — the bow's static push on the string, a DC force on the bridge — taken out.
/// Pass 5 (r42:arcus-p5) moved each instrument's to [`RADIATION_SHARE`] of its lowest open
/// string (a second-order Butterworth); the constant stays for the record.
pub const RADIATION_DC_HZ: f32 = 8.0;
/// r42:arcus-p5 — the radiation's corner over each instrument's lowest open string (a
/// second-order Butterworth: −0.4 dB on that string, −0.03 an octave up): the push a bow
/// reverses at a tremolo's every stroke (its cycle at half the stroke rate, 5–9 Hz, and the
/// harmonics of its turn) and at a bow change is a force on the bridge under any note the
/// body radiates — at the old 8 Hz it rang the bodies' lowest modes louder than the
/// tremolo's own fundamental (`docs/round42/arcus-match.md`, *Pass 5*).
pub const RADIATION_SHARE: f32 = 0.55;

/// r42:arcus-p5 — a family's vibrato as the reference study measured it, cycle by cycle
/// (`tools/arcus/pass5/vibrato.py` on 2 000 single notes of 4 families; the medians, at mf in
/// the register's middle — octave 1.5 over the lowest string): its rate and how it rises with
/// the dynamic (per unit of dynamic from 0.5) and the register (per octave), its extent peak
/// to peak and how it widens with the register and the dynamic, the cycle-to-cycle variation
/// of both (σ of a slow random walk per cycle, at `vib.var` 0.25), and how much later a soft
/// note's vibrato comes in (the delay's share per unit of dynamic under 0.5).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct VibSpec {
    pub hz: f32,
    pub hz_dyn: f32,
    pub hz_oct: f32,
    pub pp_ct: f32,
    pub pp_oct: f32,
    pub pp_dyn: f32,
    pub jitter: (f32, f32),
    pub onset_dyn: f32,
}

/// r42:arcus-p5 — the cellos' (551 notes with vibrato: 5.0 / 5.1 / 5.5 Hz soft / medium /
/// loud, 19 / 26 / 32 cents peak to peak; 17 → 34 cents from the C string to the A's top).
pub const CELLO_VIB: VibSpec = VibSpec {
    hz: 5.35,
    hz_dyn: 0.17,
    hz_oct: 0.035,
    pp_ct: 23.3,
    pp_oct: 0.21,
    pp_dyn: 0.98,
    jitter: (0.15, 0.16),
    onset_dyn: 0.46,
};

/// r42:arcus-p5 — the basses' (403: 4.6 / 5.0 / 5.35 Hz, 15 / 16.5 / 21 cents; a soft note's
/// vibrato 0.44 s in, a loud one's 0.18).
pub const BASS_VIB: VibSpec = VibSpec {
    hz: 5.5,
    hz_dyn: 0.27,
    hz_oct: 0.02,
    pp_ct: 12.2,
    pp_oct: 0.14,
    pp_dyn: 0.62,
    jitter: (0.28, 0.45),
    onset_dyn: 1.33,
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
    body: BodyKind::MeasuredBass,
    body_size: 1.0,
    bowing: Bowing {
        speed: (0.16, 0.29),
        beta: (0.15, 0.075),
        place: (0.30, 0.36),
    },
    // The study's bass: 5.4 Hz (Mick 2024: 5.17).
    vib_hz: 5.1,
    vib_ct: 16.0,
    vib: BASS_VIB, // r42:arcus-p5
    // Pass 2: its own body and correction made it ~10 dB hotter than the cello: 6 dB.
    trim_db: 6.0,
    corner_hz: 2000.0,
    // Pass 4: the basses' rise is narrower (0.10–0.24 s soft, 0.11–0.15 middle, 0.07–0.12
    // loud): a loud note's swell longer and deeper (its clean start's transient stood over
    // 10 dB).
    swell_s: (0.30, 0.16),
    swell_db: (20.0, 16.0),
    pizz_t60: BASS_PIZZ_T60,
    spic_ring: 0.55, // r42:arcus-p5
    trem_db: 5.0,    // r42:arcus-p5
    // Pass 2: the measured body carries the radiation; pass 4: its DC taken out (the cello's).
    radiation_hz: RADIATION_SHARE * 41.203, // r42:arcus-p5: E1
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

/// r42:arcus-p6 — the friction's memory (`string::Curve::lag_s`, `slip_dc`; the module's
/// notes in `arcus_string.rs`): the time it relaxes over, s, and the slip distance, m (0:
/// that term off).
pub const FRICTION_TAU_S: f32 = 250e-6;
pub const FRICTION_DC_M: f32 = 50e-6;

#[cfg(test)]
thread_local! {
    /// The probes' override of [`FRICTION_TAU_S`] and [`FRICTION_DC_M`].
    pub static FRICTION_MEM: std::cell::Cell<Option<(f32, f32)>> = const { std::cell::Cell::new(None) };
}

/// The friction's memory every new string takes, `(τ, D_c)` (the probes may override it).
pub fn friction_memory() -> (f32, f32) {
    #[cfg(test)]
    if let Some(l) = FRICTION_MEM.with(|c| c.get()) {
        return l;
    }
    (FRICTION_TAU_S, FRICTION_DC_M)
}

/// Schelleng's upper force limit for Helmholtz motion, N (Schelleng 1973; the bow's static
/// friction holding the string through the Helmholtz corner's step):
/// `F_max = 2·Z₀·v_b / ((μs − μd)·β)`.
pub fn schelleng_max(z0: f64, v_bow: f64, beta: f64) -> f64 {
    2.0 * z0 * v_bow / ((MU_S - MU_D) as f64 * beta.max(1e-3))
}

/// The cello's and the double bass's match (`tools/arcus/match.sh fit`, five passes on two
/// thirds of the notes; `docs/round42/arcus-match.md` has the held-out distances).
pub const CELLO_MATCH: Match = Match {
    // Pass 2: over the measured body, what is left of the recordings' spectrum (the match's
    // fit on two thirds of the notes; pass 1 fitted −9.7 dB at 63 Hz … +12 at 10 kHz over
    // the scaled violin body).
    eq_db: [
        -8.1, -5.2, -3.0, -1.5, -0.8, -2.0, -1.9, 0.4, -0.2, -0.7, 6.7, 12.0,
    ],
    // Pass 2, measured by the study's estimator on the held-out notes: the hair's noise 3–6
    // dB under the cellos' band, the vibrato ±16.6 cents against ±6–12 and 5.8 Hz against
    // 5.0–5.4 at soft and medium.
    noise_db: 12.0,
    pizz_decay: 1.0, // pass 4: the pluck's T60 table (`CELLO_PIZZ_T60`) replaces the ring factor 0.81
    // Pass 5: model 2's vibrato is the study's own (`CELLO_VIB`); pass 2's 0.48 / 0.92 retired.
    vib_depth: 1.0,
    vib_rate: 1.0,
};
pub const BASS_MATCH: Match = Match {
    // Pass 2: over its own measured body the fitted correction now moves the held-out notes
    // towards the recordings (spectrum 5.26 → 3.76 dB, centroid 1.8 → 0 semitones); over the
    // scaled cello body (pass 1) it had moved them away and was left flat.
    eq_db: [
        -5.9, -1.2, -1.0, -3.7, -4.1, -2.5, -2.0, -2.1, 0.6, 6.1, 8.6, 7.1,
    ],
    // Pass 2: the noise 8–18 dB under the basses' band, the vibrato ±8 against ±6–7.8.
    noise_db: 18.0,
    pizz_decay: 1.0, // pass 4: `BASS_PIZZ_T60` replaces the ring factor 0.67
    // Pass 5: model 2's vibrato is the study's own (`BASS_VIB`); pass 2's 0.38 retired.
    vib_depth: 1.0,
    vib_rate: 1.0,
};
