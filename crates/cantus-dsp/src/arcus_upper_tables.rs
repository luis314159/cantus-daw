//! The violin's and the viola's tables (round 42, `arcus-upper`; a child of `arcus::upper`):
//! their strings, their range and how their dynamics are bowed. `tables::Instrument::spec`
//! reads them here.
//!
//! **The strings**, every number cited:
//!
//! - **Vibrating lengths**: G. Percival's measurements (*Physical modelling meets machine
//!   learning: performing music with a virtual string ensemble*, PhD thesis, University of
//!   Glasgow, 2013, Table 3.2 — five violins 319 – 326 mm, two violas 363 – 371 mm): their
//!   medians, 322 mm and 366 mm (a 15½–16-inch viola).
//! - **Tensions**: the ranges of Rossing (ed.), *The Science of String Instruments*
//!   (Springer 2010, p. 286), as Percival's Table 3.6 gives them — violin E 71 – 91 N, A 48 –
//!   63, D 34 – 61, G 35 – 51; viola A 61 – 100, D, G, C 48 – 61 — checked against the
//!   retailers' Pirastro figures (Evah Pirazzi violin E 7.8 – 8.5 kg, A 5.3 – 5.6, D ~4.8, G
//!   4.6 – 4.9; `strings-verified.md` §3.2, NV): a medium set inside both.
//! - **Losses**: Percival's measured decay rates of open strings on their instruments
//!   (Table 3.5, `r₁` the fundamental's and `r₁₀` the tenth partial's, s⁻¹; T60 = 6.91 / r):
//!   the medians of the five violins — E `r₁` 2.9, `r₁₀` 11.7; A 2.1, 30; D 1.3, 28.3; G 0.6,
//!   20.1 — and the violas' first set — A 2.9, 21.6; D 2.3, 15.7; G 1.3, 17.5; C 0.9, 12.1 —
//!   give each string's fundamental T60 and the exponent `p = log₁₀(r₁₀/r₁)` the partials'
//!   decay falls by (the loop's loss filter holds it to the eighth partial).
//! - **Stiffness**: none in the loop (Schelleng 1973's 0.1 cent·n² tolerance; Percival's
//!   Table 3.3 measured B ≤ 10⁻⁴ on these strings, under it for the partials a bow drives).
//!
//! **The bowing** is re-mapped the players' way after the study's database
//! (`study/bowed-realism.md` §0, item 2: pp → ff the force × 4–5, the bow–bridge distance ×
//! 0.4–0.5, the speed nearly constant, the level 13 – 18 dB): here the speed × 3 (10 → 30 cm/s,
//! inside Askenfelt's violin measurements, *JASA* 86, 1989, doi:10.1121/1.398230, abstract read),
//! β × 0.4 (0.14 → 0.055), so the Helmholtz amplitude × 7.6 (17.6 dB nominal), and the force × 5
//! (its place from 0.36 to 0.24 of Schelleng's limit) inside the region measured on this model
//! (`upper_tests_gate`; Schoonderwaldt, Guettler & Askenfelt 2008, doi:10.3813/aaa.918070, the
//! violin's Schelleng diagram on its D and E strings, read in `downloads/study/papers/s08.txt`).

use super::super::tables::{Bowing, InstrumentSpec, Match, StringSpec, VibSpec, RADIATION_SHARE};
use crate::fides::body::BodyKind;

/// A string from Percival's decay rates: the fundamental's T60 and the partials' exponent.
const fn string(open: u8, length_m: f64, tension_n: f64, r1: f64, p: f64) -> StringSpec {
    StringSpec {
        open,
        length_m,
        tension_n,
        t60_s: 6.91 / r1,
        p,
    }
}

/// r42:arcus-p5 — the violins' vibrato (512 notes: 4.9 / 5.1 / 5.5 Hz soft / medium / loud,
/// 19.5 / 22 / 38 cents peak to peak; `tables::VibSpec`).
pub const VIOLIN_VIB: VibSpec = VibSpec {
    hz: 5.2,
    hz_dyn: 0.22,
    hz_oct: 0.015,
    pp_ct: 17.9,
    pp_oct: 0.15,
    pp_dyn: 1.27,
    jitter: (0.2, 0.2),
    onset_dyn: 1.08,
};

/// r42 fiddle (ADR 0164) — the violin's cross-tunings, in `tuning.strings`' order: standard
/// (G D A E), AEAE, GDGD, ADAE, calico (A E A C#) and DDAD — semitones each string sits from
/// standard. A string tuned up or down keeps its length and its decay; its tension goes as the
/// frequency's square (`T ∝ f²`), so its impedance and its Schelleng limits follow.
pub const CROSS: [[i8; 4]; 6] = [
    [0, 0, 0, 0],
    [2, 2, 0, 0],
    [0, 0, -2, -2],
    [2, 0, 0, 0],
    [2, 2, 0, -3],
    [-5, 0, 0, -2],
];

/// `2^(2d/12)`, the tension's factor for a string `d` semitones from standard (−5 … +2).
const fn tension_k(d: i8) -> f64 {
    match d {
        2 => 1.259_921_049_894_873,
        -2 => 0.793_700_525_984_099_7,
        -3 => 0.707_106_781_186_547_5,
        -5 => 0.561_231_024_154_686_6,
        _ => 1.0,
    }
}

const fn crossed(d: [i8; 4]) -> InstrumentSpec {
    let mut s = VIOLIN;
    let mut i = 0;
    while i < 4 {
        s.strings[i].open = (VIOLIN.strings[i].open as i16 + d[i] as i16) as u8;
        s.strings[i].tension_n = VIOLIN.strings[i].tension_n * tension_k(d[i]);
        i += 1;
    }
    s
}

/// The violin in each cross-tuning ([`CROSS`]'s order; the first is [`VIOLIN`] itself).
pub static VIOLIN_CROSS: [InstrumentSpec; 6] = [
    VIOLIN,
    crossed(CROSS[1]),
    crossed(CROSS[2]),
    crossed(CROSS[3]),
    crossed(CROSS[4]),
    crossed(CROSS[5]),
];

/// r42:arcus-p5 — the violas' (510: 5.5 / 5.5 / 5.7 Hz, 27 / 29 / 44 cents).
pub const VIOLA_VIB: VibSpec = VibSpec {
    hz: 5.6,
    hz_dyn: 0.08,
    hz_oct: 0.01,
    pp_ct: 18.6, // r42 arcus-classical: × 0.8 (was 23.2): in music the violas
    // vibrate narrower than on single notes (the strings-hq phrases: violas 21 cents, cellos 10–25;
    // Arcus's phrases 28–31), the single notes still inside their band.
    pp_oct: 0.11,
    pp_dyn: 0.97,
    jitter: (0.18, 0.17),
    onset_dyn: 0.98,
};

/// The violin: G3 D4 A4 E5 over 322 mm. Its body is Arcus's own (`upper::body`), fitted to
/// a measured admittance; Fides's is off (`BodyKind::None`).
pub const VIOLIN: InstrumentSpec = InstrumentSpec {
    strings: [
        // p = log₁₀(r₁₀/r₁): G 1.52, D 1.34, A 1.16, E 0.61.
        string(55, 0.322, 45.0, 0.6, 1.52),
        string(62, 0.322, 47.0, 1.3, 1.34),
        string(69, 0.322, 54.0, 2.1, 1.16),
        string(76, 0.322, 80.0, 2.9, 0.61),
    ],
    top: 100,
    body: BodyKind::None,
    body_size: 1.0,
    bowing: Bowing {
        speed: (0.10, 0.30),
        beta: (0.14, 0.055),
        place: (0.33, 0.27),
    },
    // Mellody & Wakefield 2000 (5.9 Hz, ±15 cents mean) and MacLeod 2008 (wider high).
    vib_hz: 5.9,
    vib_ct: 17.0,
    vib: VIOLIN_VIB, // r42:arcus-p5
    trim_db: 12.0,
    corner_hz: 6000.0,
    memory: false, // r42:arcus-p6
    // Pass 4's: a sustain's swell over its clean start, fitted to the study's violins' rise
    // (`tools/arcus/upper-measure.sh`, its rise classes against the band).
    swell_s: (0.70, 0.08),
    swell_db: (20.0, 10.0),
    pizz_t60: VIOLIN_PIZZ_T60,
    spic_ring: 0.27, // r42:arcus-p5
    trem_db: 3.0,    // r42:arcus-p5
    // Pass 4's 8 Hz high-pass: the bow's static push reaches the bridge as a DC force no body
    // radiates (measured on the violin too: `docs/round42/arcus-upper.md` §6).
    radiation_hz: RADIATION_SHARE * 196.0, // r42:arcus-p5: G3
    // r42:arcus-p5: measured (`tests_gate2::probe_levels`, every three semitones G3–E7):
    // the violin fell 0.17 dB a semitone, its top 6 dB under its G string.
    balance_db_st: 0.17,
    balance_key: 72.0,
    matched: VIOLIN_MATCH,
};

/// The viola: C3 G3 D4 A4 over 366 mm; its body the violin's scaled (`upper::body`).
pub const VIOLA: InstrumentSpec = InstrumentSpec {
    strings: [
        // p = log₁₀(r₁₀/r₁): C 1.13, G 1.13, D 0.83, A 0.87.
        string(48, 0.366, 54.0, 0.9, 1.13),
        string(55, 0.366, 53.0, 1.3, 1.13),
        string(62, 0.366, 54.0, 2.3, 0.83),
        string(69, 0.366, 72.0, 2.9, 0.87),
    ],
    top: 93,
    body: BodyKind::None,
    body_size: 1.0,
    bowing: Bowing {
        speed: (0.10, 0.30),
        beta: (0.145, 0.06),
        place: (0.33, 0.27),
    },
    // MacLeod 2008: violists slightly slower and narrower than violinists.
    vib_hz: 5.7,
    vib_ct: 16.0,
    vib: VIOLA_VIB, // r42:arcus-p5
    trim_db: 8.0,
    corner_hz: 5000.0,
    memory: false, // r42:arcus-p6
    swell_s: (0.30, 0.10),
    swell_db: (20.0, 12.0),
    pizz_t60: VIOLA_PIZZ_T60,
    spic_ring: 0.35,                        // r42:arcus-p5
    trem_db: 5.0,                           // r42:arcus-p5
    radiation_hz: RADIATION_SHARE * 130.81, // r42:arcus-p5: C3
    // r42:arcus-p5: measured the same way, the viola 0.12 dB a semitone C3–A6.
    balance_db_st: 0.12,
    balance_key: 65.0,
    matched: VIOLA_MATCH,
};

/// The violin's model-2 match (`ArcusSettings::model` 2), fitted by `tools/arcus/upper-measure.sh`
/// over ten passes (logs in `docs/round42/arcus-upper.md`):
///
/// - **the output's correction**: started from the CNSM dataset's scales (14 recordings of
///   three violins by the same players, CC BY 4.0: their mean long-term spectrum over Arcus's
///   two-octave G major scale, read at the correction's bands), then held so the study's
///   estimator puts the spectral centroid inside the real violins' band — the CNSM recordings
///   ask for more air above 6 kHz than the six instruments of the study's band have, and the
///   top two bands follow the band (−4 / −2 dB);
/// - **the hair's noise** raised to the study's measure (−42 dB re the harmonics at first,
///   against the real instruments' −32: `tools/arcus/upper_study.py`);
/// - **the pluck's ring**: now the family's T60 table (pass 4, `VIOLIN_PIZZ_T60`), the ring
///   factor retired;
/// - **the vibrato** narrower and slower on the violin (the study's band: 6.9 – 10.4 cents,
///   4.9 – 5.5 Hz; Arcus measured 17 cents and 6.2 Hz; the viola's was inside its band).
///
/// Judged on the Iowa and VSCO notes and the study's band, which the correction's start did not
/// use.
pub const VIOLIN_MATCH: Match = Match {
    eq_db: [
        0.6, 0.6, -5.0, 10.0, 1.4, -1.2, 4.1, -2.4, -5.0, -0.3, -4.0, -2.0,
    ],
    noise_db: 22.0,
    pizz_decay: 1.0,
    // Pass 5: model 2's vibrato is the study's own (`VIOLIN_VIB`); 0.5 / 0.88 retired.
    vib_depth: 1.0,
    vib_rate: 1.0,
};

/// The viola's: no recordings of a viola's scales to start from, so the violin's correction
/// stands for it (the same family's body at 1.12 × its size — an assumption, said so in
/// `docs/round42/arcus-upper.md`) without its cut under 160 Hz (where the viola's C string
/// sounds) and with +2 dB from 1.6 kHz up, which brings its spectral centroid to the edge of the
/// real violas' band; its noise as the violin's.
pub const VIOLA_MATCH: Match = Match {
    eq_db: [
        0.0, 0.0, 4.0, 10.0, 1.4, -1.2, 4.1, -0.4, -3.0, 0.0, -4.0, -2.0,
    ],
    noise_db: 18.0,
    pizz_decay: 1.0,
    // Pass 5: model 2's vibrato is the study's own (`VIOLA_VIB`); 0.9 retired.
    vib_depth: 1.0,
    vib_rate: 1.0,
};

/// The violins' pluck T60 by frequency (the study's `pizz_t60_by_freq`, 3 violins, the bands
/// with at least two instruments, a point held where the band's notes thin out: 1.9 s at the G
/// string, 1.2 s at 400 Hz, 0.7 s at 1 kHz, 0.4 s at 3–4 kHz). It replaces the ring factor the
/// first match fitted from the Iowa plucks (× 0.27: Percival's open-string decays).
pub const VIOLIN_PIZZ_T60: &[(f64, f64)] = &[
    (198.0, 1.9),
    (250.0, 1.5),
    (315.0, 1.45),
    (400.0, 1.2),
    (500.0, 0.9),
    (630.0, 0.9),
    (800.0, 0.78),
    (1000.0, 0.7),
    (1260.0, 0.68),
    (1600.0, 0.56),
    (2000.0, 0.45),
    (2500.0, 0.44),
    (3150.0, 0.4),
    (4000.0, 0.39),
];

/// The violas' (2 violas; above 2 kHz their few notes read their recordings' floor, so the
/// table holds 0.5 s there).
pub const VIOLA_PIZZ_T60: &[(f64, f64)] = &[
    (157.0, 1.6),
    (198.0, 1.3),
    (250.0, 1.3),
    (315.0, 1.17),
    (400.0, 1.06),
    (500.0, 0.95),
    (630.0, 0.93),
    (800.0, 0.89),
    (1000.0, 0.73),
    (1260.0, 0.62),
    (1600.0, 0.6),
    (2000.0, 0.5),
    (4000.0, 0.5),
];
