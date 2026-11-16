//! What Purus shares with the audio editor's denoiser (ADR 0096) — the square-root-Hann
//! window and the gain smoothing over frequency and time — is shared without moving a bit
//! of the editor's output: these hashes were taken from `denoise`, `attenuate_region` and
//! the spectrogram **before** the sharing, and must never change.

use crate::denoise::stft::Spectrogram;
use crate::denoise::declick;
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
            let env = if (12_000..30_000).contains(&i) {
                0.4
            } else {
                0.0
            };
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

/// The editor's click repair, the same way: its detector and its rebuilding are shared with
/// Purus's de-click (fx-e:purus); these hashes were taken before the sharing.
#[test]
fn the_declickers_outputs_are_bit_identical_to_before_the_sharing() {
    let clean = take();
    let mut x = clean.clone();
    let mut s = 99u32;
    for c in 0..40 {
        s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let at = 3_000 + (s as usize % 44_000);
        let amp = if c % 2 == 0 { 0.5 } else { -0.35 };
        for j in 0..(1 + c % 5) {
            x[at + j] += amp * (1.0 - 0.2 * j as f32);
        }
    }
    let mut hashes = Vec::new();
    for sens in [0.0f32, 0.5, 1.0] {
        let gaps = declick::detect(&x, 0, x.len(), sens);
        hashes.push(fnv(gaps.iter().flat_map(|(a, b)| [*a as u32, *b as u32])));
        let mut y = x.clone();
        let len = y.len();
        let n = declick::declick(&mut y, 0, len, sens);
        hashes.push(fnv(y.iter().map(|v| v.to_bits()).chain([n as u32])));
    }
    let a = declick::ar_fit(&x[1_000..5_000], declick::ORDER);
    hashes.push(fnv(a.iter().flat_map(|v| {
        let b = v.to_bits();
        [b as u32, (b >> 32) as u32]
    })));
    let want: [u64; 7] = [
        0x72f5_98ff_459c_88db,
        0x9169_64fc_6041_4cd0,
        0xc766_09ac_24f4_7c6c,
        0xea9c_81c0_05a3_e89b,
        0x4883_17d3_4b96_cdea,
        0xfd95_54ab_7c33_81d1,
        0x578b_a6e3_f544_1271,
    ];
    assert_eq!(hashes, want, "the editor's declicker moved: {hashes:#x?}");
}
