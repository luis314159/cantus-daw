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
const LOWER_HASHES: &[u64] = &[
    0x914d942f0d0453f9,
    0xc02bf7e99fbacc61,
    0xa1ff370904415db3,
    0x3fe5a485e8e429d1,
    0xca2c6a870093b044,
    0xd0b07d0d827ca234,
    0x368b8fa3f06e79bf,
];

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
