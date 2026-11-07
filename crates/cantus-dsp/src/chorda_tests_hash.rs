//! Model 1 never moves (ADR 0098): a phrase through every part of the first model — the
//! hammers across the range, the pedal half and whole, the soft pedal, the sympathetic
//! strings, the board's tail — hashed, against the hash the round-36 code gave (computed on
//! the contract commit, c95ee8a9, before model 2 existed).

use super::tests_engine::{perform, piano, Ev};

/// FNV-1a over the samples' bits, both channels.
pub fn hash(l: &[f32], r: &[f32]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for x in l.iter().chain(r) {
        for b in x.to_bits().to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

pub fn model_1_phrase() -> Vec<(usize, Ev)> {
    let mut ev = vec![
        (0, Ev::On(21, 0.9, 1)),
        (3, Ev::On(45, 0.5, 2)),
        (500, Ev::Pedal(0.45)),
        (1_000, Ev::On(60, 0.7, 3)),
        (1_000, Ev::On(64, 0.65, 4)),
        (1_000, Ev::On(67, 0.6, 5)),
        (6_001, Ev::Pedal(1.0)),
        (8_000, Ev::On(88, 1.0, 6)),
        (9_100, Ev::Off(60, 3)),
        (12_000, Ev::Soft(1.0)),
        (12_333, Ev::On(76, 0.3, 7)),
        (16_000, Ev::Off(21, 1)),
        (16_000, Ev::Off(45, 2)),
        (18_000, Ev::Pedal(0.0)),
        (18_000, Ev::Soft(0.0)),
        (20_000, Ev::On(108, 0.8, 8)),
        (20_500, Ev::Off(108, 8)),
    ];
    ev.sort_by_key(|e| e.0);
    ev
}

/// The round-36 code's hash of [`model_1_phrase`], 30 000 frames in blocks of 256.
const MODEL_1: u64 = 0x2fe3_97ff_c736_7de3;

#[test]
fn model_1_renders_to_the_bits_it_always_did() {
    let (l, r) = perform(&mut piano(), &model_1_phrase(), 30_000, 256, 1);
    let h = hash(&l, &r);
    eprintln!("model 1 phrase hash: {h:#018x}");
    assert_eq!(h, MODEL_1, "model 1 moved: {h:#018x}");
}
