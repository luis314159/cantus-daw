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

use super::super::tables::{Bowing, InstrumentSpec, Match, StringSpec};
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
        place: (0.36, 0.24),
    },
    // Mellody & Wakefield 2000 (5.9 Hz, ±15 cents mean) and MacLeod 2008 (wider high).
    vib_hz: 5.9,
    vib_ct: 17.0,
    trim_db: 12.0,
    corner_hz: 6000.0,
    radiation_hz: 0.0,
    balance_db_st: 0.0,
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
        place: (0.36, 0.24),
    },
    // MacLeod 2008: violists slightly slower and narrower than violinists.
    vib_hz: 5.7,
    vib_ct: 16.0,
    trim_db: 8.0,
    corner_hz: 5000.0,
    radiation_hz: 0.0,
    balance_db_st: 0.0,
    balance_key: 65.0,
    matched: VIOLA_MATCH,
};

/// The violin's model-2 match (`ArcusSettings::model` 2): the output's correction from the CNSM
/// dataset's scales — 14 recordings of three violins by the same players (CC BY 4.0) — their
/// mean long-term third-octave spectrum over Arcus's playing a two-octave G major scale,
/// read at the correction's bands (`tools/arcus/upper-compare.sh` prints it); the hair's noise
/// raised to the study's measure (its own estimator on Arcus's notes: −42 dB re the harmonics
/// against the real instruments' −32, `tools/arcus/upper_study.py`); the pluck's ring shortened by what the Iowa MIS's plucked notes measured (Arcus's
/// partials 1–4 rang 1.9 octaves of T60 longer on the violin, 1.65 on the viola: Percival's
/// decays are open strings', a stopped and plucked string loses more to the finger); the
/// violin's vibrato narrower and slower (the study's band: 6.9 – 10.4 cents, 4.9 – 5.5 Hz;
/// Arcus measured 17 cents and 6.2 Hz; the viola's was inside its band). Judged on the Iowa and VSCO notes, which the fit did not use.
pub const VIOLIN_MATCH: Match = Match {
    eq_db: [
        -7.6, -7.6, -9.0, 7.4, 4.9, 7.6, 2.8, 3.6, -0.7, 1.3, -1.2, 0.4,
    ],
    noise_db: 22.0,
    pizz_decay: 0.27,
    vib_depth: 0.5,
    vib_rate: 0.88,
};

/// The viola's: no recordings of a viola's scales were found to fit its own, so the violin's
/// correction stands for it (the same family's body at 1.14 × its size — an assumption, said
/// so in `docs/round42/arcus-upper.md`), its noise as the violin's.
pub const VIOLA_MATCH: Match = Match {
    eq_db: VIOLIN_MATCH.eq_db,
    noise_db: 18.0,
    pizz_decay: 0.32,
    vib_depth: 0.9,
    vib_rate: 1.0,
};
