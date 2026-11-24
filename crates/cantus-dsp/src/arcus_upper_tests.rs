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

/// Measured on `r42/arcus` 58cafea8e (the cello's and the bass's model 2 matched) in a worktree
/// of it with only this test added, before the upper strings' engine: the phrase of
/// `tests::phrase` rendered 60 000 frames in blocks of 256 (`ARCUS_UPPER_DUMP=1` prints them).
/// A later change to the cello or the bass on `r42/arcus` moves these on purpose, measured the
/// same way.
const LOWER_HASHES: &[u64] = &[
    0x1a21b4078f0ca02c,
    0x2e6e7fdd44def356,
    0x7480af8c4a7e1f00,
    0x99d9b8c04e754489,
    0x116db09a872adb65,
    0xa3767968c3ccdb4b,
    0xbeeb6e47f7b34e89,
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
