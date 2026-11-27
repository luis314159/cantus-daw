//! The upper strings' tests (round 42, `arcus-upper`): the cello and the double bass keep
//! their bits to the last one (the short segment's engine is the violin's and the viola's
//! choice only), and the violin and the viola meet the cello's house invariants.

use super::tables::Instrument;
use super::tests::{phrase, render, variety, SR};
use super::*;
use crate::par::Serial;

/// FNV-1a over both channels' bits.
pub(super) fn hash(l: &[f32], r: &[f32]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for x in l.iter().chain(r) {
        for b in x.to_bits().to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

/// The cello's and the bass's settings of `variety()`, and the defaults.
fn lower() -> Vec<ArcusSettings> {
    variety()
        .into_iter()
        .filter(|s| matches!(s.instrument, Instrument::Cello | Instrument::Bass))
        .collect()
}

/// Measured on `r42/arcus` 2c2b6f202 (pass 4: the radiation's 8 Hz corner, each family's swell,
/// the pluck's T60 table) in a worktree of it with only this test added, before the upper
/// strings' engine: the phrase of `tests::phrase` rendered 60 000 frames in blocks of 256
/// (`ARCUS_UPPER_DUMP=1` prints them). A later change to the cello or the bass on `r42/arcus`
/// moves these on purpose, measured the same way.
///
/// r42:arcus-p5 (`DSP_VERSION` 13): re-made for pass 5 — the radiation's Butterworth under the
/// lowest string, the tremolo's light stroke and its table, the bow change's eased force and
/// quick re-bow, the re-bow's finger, the spiccato's bounce, model 2's vibrato (the players').
/// The measured size against 11e6b8cad (`tests_p5::probe_dump_lower` in both trees): the seven
/// renders' RMS moved −0.3, +0.0, −0.4, −3.2, −0.8, −3.2, +1.3 dB, their difference at the
/// level of the signal (RMS −4.1 … +3.8 dB re each render, max |Δ| 0.17–0.47: the vibrato's
/// phase and the tremolo's strokes decorrelate them) — every note changed on purpose.
///
/// r42:arcus-p6 (`DSP_VERSION` 14): re-made for pass 6 — the cello's measured body radiates to
/// 10 kHz (its cloud 640 Hz – 10 kHz, 55 modes, was 33 to 5.2 kHz), the match's correction
/// fitted again over it, the cello's output trim 2.8 dB lower (what the cloud added). The
/// measured size against c305bc540 (`tests_p5::probe_dump_lower` in both states): the four
/// cello renders' RMS −0.03, −0.03, +0.06, +0.20 dB, their difference −10.9, −11.0, −11.5, −2.9
/// dB re each render (max |Δ| 0.13–0.19: the spectrum above 2 kHz moved, the vibrato's partials
/// sweep more modes); the three bass renders the same to the bit (difference under −267 dB).
///
/// r42 arcus-expr (`DSP_VERSION` 15, ADR 0162): re-made for the player on by default — the bow
/// along the stick and the arm's weight there, a stroke from the string with its force's
/// contour, the open string's and the fast passage's vibrato, the shift's finger. The measured
/// size (`probe_player_size`, the player off — pass 6's bits, [`PASS6_HASHES`] — against on):
/// the RMS of the cello's four renders +0.30, −0.06, +0.01, −0.24 dB and the bass's three −1.93,
/// −1.19, −0.06 dB; their difference +1.2, −4.4, −22.4, −12.7 (cello) and −1.0, −6.4, −21.4 dB
/// (bass) re each render — the bowed ones at the signal's level (the vibrato's and the strokes'
/// shapes move them), every one on purpose.
///
/// r42 arcus-classical (`DSP_VERSION` 20, ADR 0166): re-made for the stroke's shape (the bow's
/// speed and the dynamic it is read at rising, sagging, breathing and tapering inside a held
/// note), the free string after the lift, the pluck's DC corner and contact, the vibrato's way
/// back and the player's intonation in a passage, the violin's and viola's corner with the force,
/// the viola's and cello's narrower vibrato and the upper strings' pitch table measured again with
/// the player on. The measured size: `docs/round42/arcus-classical.md`, *The size of the change*.
const LOWER_HASHES: &[u64] = &[
    0x4f1d1dcda9edf81f,
    0xcb8db4391d301106,
    0x8ab414d6e1b9ffd4,
    0x2499bbc3fbcb96a4,
    0xd7dc01a4b7d153d7,
    0xfc4755af4bd14dce,
    0xfb9c139a75d9d005,
];

/// r42 arcus-expr — the null of the player (ADR 0162): with `player` 0 the seven renders keep
/// the string's own bits — pass 6's until `DSP_VERSION` 20, when the string's model-2 changes of
/// r42 arcus-classical (the free ring after the lift, the pluck's DC corner and contact, the
/// viola's and cello's vibrato) moved them on purpose; the player's shape, taper, way back and
/// intonation stay out of them (`tests_classical::with_the_player_off_the_shape_touches_no_sample`).
const PASS6_HASHES: &[u64] = &[
    0x56171b9d8c3a613f,
    0x5634ca566ba8d2b4,
    0x14bce81194da377a,
    0xff53c5e8c8a78e36,
    0x8fc5c4e1e644e2b7,
    0x46d4102f6774671c,
    0x0022787859074505,
];

#[test]
fn the_player_off_keeps_pass_six_s_bits() {
    let got: Vec<u64> = lower()
        .iter()
        .map(|s| {
            let s = ArcusSettings { player: 0.0, ..*s };
            let (l, r) = render(&s, &phrase(), 60_000, &|_| 256, &Serial);
            hash(&l, &r)
        })
        .collect();
    assert_eq!(got.as_slice(), PASS6_HASHES);
}

#[test]
fn the_cello_and_the_bass_keep_their_bits() {
    let got: Vec<u64> = lower()
        .iter()
        .map(|s| {
            let (l, r) = render(s, &phrase(), 60_000, &|_| 256, &Serial);
            hash(&l, &r)
        })
        .collect();
    if LOWER_HASHES.is_empty() || std::env::var_os("ARCUS_UPPER_DUMP").is_some() {
        println!("LOWER_HASHES = {got:#x?}");
    }
    assert!(!LOWER_HASHES.is_empty(), "no baseline recorded");
    assert_eq!(got.as_slice(), LOWER_HASHES);
    let _ = SR;
}

/// The upper strings at each factor, every technique: block sizes 1, 37 and 512 and a
/// ragged run give the same bits (a voice ends on its control grid, and its way back rings
/// out from there whatever the blocks).
#[test]
fn the_upper_strings_render_the_same_at_every_block_size_at_every_factor() {
    use super::tests::first_difference;
    for base in variety()
        .into_iter()
        .filter(|s| matches!(s.instrument, Instrument::Violin | Instrument::Viola))
    {
        for os in [Some(1u8), Some(2), None] {
            let s = ArcusSettings {
                oversample: os,
                ..base
            };
            let a = render(&s, &phrase(), 50_000, &|_| 512, &Serial);
            for (name, b) in [
                ("1", render(&s, &phrase(), 50_000, &|_| 1, &Serial)),
                ("37", render(&s, &phrase(), 50_000, &|_| 37, &Serial)),
                (
                    "ragged",
                    render(
                        &s,
                        &phrase(),
                        50_000,
                        &|b| [3, 200, 31, 999, 64, 7][b % 6],
                        &Serial,
                    ),
                ),
            ] {
                assert_eq!(
                    first_difference(&a, &b),
                    None,
                    "{:?} {:?} at {os:?}: block {name}",
                    s.instrument,
                    s.technique
                );
            }
        }
    }
}

/// r42 arcus-expr — the size of the player's change (ADR 0162): the seven renders of
/// [`LOWER_HASHES`] with the player off (pass 6's bits) and on, each one's RMS and their
/// difference, dB.
#[test]
#[ignore = "a probe: cargo test -p cantus-dsp --lib arcus::upper_tests::probe_player_size -- --ignored --nocapture"]
fn probe_player_size() {
    let db = |x: &[f32]| {
        let e = x.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / x.len() as f64;
        10.0 * crate::math::log10(e + 1e-30)
    };
    for s in lower() {
        let off = ArcusSettings { player: 0.0, ..s };
        let (a, _) = render(&off, &phrase(), 60_000, &|_| 256, &Serial);
        let (b, _) = render(&s, &phrase(), 60_000, &|_| 256, &Serial);
        let d: Vec<f32> = a.iter().zip(&b).map(|(x, y)| x - y).collect();
        println!(
            "{:?} {:?}: off {:.2} dB, on {:.2} dB, difference {:.1} dB re off",
            s.instrument,
            s.technique,
            db(&a),
            db(&b),
            db(&d) - db(&a)
        );
    }
}
