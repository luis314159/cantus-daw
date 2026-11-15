//! What Purus shares with the audio editor's denoiser (ADR 0096) — the square-root-Hann
//! window and the gain smoothing over frequency and time — is shared without moving a bit
//! of the editor's output: these hashes were taken from `denoise`, `attenuate_region` and
//! the spectrogram **before** the sharing, and must never change.

use crate::denoise::stft::Spectrogram;
use crate::denoise::{attenuate_region, denoise_channel, Params, Region};
use crate::math;

fn fnv(bits: impl Iterator<Item = u32>) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in bits {
        for byte in b.to_le_bytes() {
            h ^= byte as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

fn take() -> Vec<f32> {
    let mut s = 7u32;
    (0..48_000)
        .map(|i| {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let n = ((s >> 8) as f32 / (1u32 << 23) as f32 - 1.0) * 0.02;
            let t = i as f32 / 48_000.0;
            let env = if (12_000..30_000).contains(&i) { 0.4 } else { 0.0 };
            n + env * math::sin(std::f32::consts::TAU * 330.0 * t)
        })
        .collect()
}

#[test]
fn the_denoisers_outputs_are_bit_identical_to_before_the_sharing() {
    let x = take();
    let noise: Vec<f32> = {
        let mut st = crate::denoise::stft::Stft::new();
        st.mean_power(&x[..10_000]).0
    };
    let mut hashes = Vec::new();
    for p in [
        Params::default(),
        Params {
            amount: 0.6,
            smoothing: 0.0,
            ..Params::default()
        },
        Params {
            smoothing: 1.0,
            reduction_db: 60.0,
            sensitivity: 1.0,
            ..Params::default()
        },
    ] {
        let y = denoise_channel(&x, &noise, &p);
        hashes.push(fnv(y.iter().map(|v| v.to_bits())));
    }
    let region = Region {
        start: 14_000,
        end: 26_000,
        lo_hz: 250.0,
        hi_hz: 400.0,
    };
    let y = attenuate_region(&x, 48_000, &region, -18.0);
    hashes.push(fnv(y.iter().map(|v| v.to_bits())));
    let sg = Spectrogram::of(&x, 48_000, 256);
    hashes.push(fnv(sg.cells.iter().map(|c| *c as u32)));
    let want: [u64; 5] = [
        0x563e_540b_a3e2_28ff,
        0x1ee0_8758_5ba1_2f23,
        0x87d2_feb4_b6da_501a,
        0x34cf_7370_ac78_e657,
        0xcbe0_d9b6_af10_585e,
    ];
    assert_eq!(hashes, want, "the editor's denoiser moved: {hashes:#x?}");
}
