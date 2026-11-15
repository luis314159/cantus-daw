//! What Cedo shares with Comprimo — the key's band sections (Aequalis's design run in `f64`,
//! a dying band stopped whole) and the stereo delay line its lookahead reads — is shared
//! without moving a bit of the compressor's output: these hashes were taken from Comprimo
//! **before** the sharing, and must never change.

use crate::aequalis::EqShape;
use crate::comprimo::{CompSettings, Comprimo, KeyBand, Style};
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

/// A second of programme: a 110 Hz tone and noise with a gap of silence (so the dying
/// bands are stopped), and a key of kick-like bursts.
fn material() -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let mut s = 11u32;
    let mut noise = move || {
        s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (s >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    };
    let n = 48_000;
    let mut l = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    let mut k = vec![0.0f32; n];
    for i in 0..n {
        let t = i as f32 / 48_000.0;
        if !(20_000..34_000).contains(&i) {
            l[i] = 0.5 * math::sin(std::f32::consts::TAU * 110.0 * t) + 0.05 * noise();
            r[i] = 0.4 * math::sin(std::f32::consts::TAU * 165.0 * t) + 0.05 * noise();
        }
        let ph = i % 12_000;
        if ph < 4_000 {
            let e = math::exp(-(ph as f32) / 900.0);
            k[i] = 0.9 * e * math::sin(std::f32::consts::TAU * 55.0 * ph as f32 / 48_000.0);
        }
    }
    (l, r, k)
}

fn run(s: &CompSettings, keyed: bool) -> u64 {
    let (l0, r0, k) = material();
    let mut c = Comprimo::new(48_000.0);
    c.set(s);
    let (mut l, mut r) = (l0.clone(), r0.clone());
    let mut gr = Vec::new();
    for (a, b) in l.chunks_mut(256).zip(r.chunks_mut(256)).enumerate() {
        let (bl, br) = b;
        let off = a * 256;
        let key = &k[off..off + bl.len()];
        if keyed {
            c.process(bl, br, Some((key, key)));
        } else {
            c.process(bl, br, None);
        }
        gr.push(c.meter().gr_db.to_bits());
    }
    fnv(l.iter().chain(r.iter()).map(|v| v.to_bits()).chain(gr))
}

fn settings() -> Vec<(CompSettings, bool)> {
    let mut eq = CompSettings {
        threshold_db: -30.0,
        ratio: 8.0,
        attack_ms: 1.0,
        release_ms: 80.0,
        hold_ms: 5.0,
        lookahead_ms: 3.0,
        key_hpf_hz: 90.0,
        key_gain_db: 3.0,
        ..CompSettings::default()
    };
    eq.key_bands[0] = KeyBand {
        active: true,
        shape: EqShape::LowCut,
        freq: 40.0,
        slope: 48.0,
        ..KeyBand::default()
    };
    eq.key_bands[2] = KeyBand {
        active: true,
        shape: EqShape::Bell,
        freq: 2_500.0,
        gain_db: 6.0,
        q: 2.0,
        ..KeyBand::default()
    };
    eq.key_bands[5] = KeyBand {
        active: true,
        shape: EqShape::BandPass,
        freq: 120.0,
        q: 1.5,
        ..KeyBand::default()
    };
    let rms = CompSettings {
        style: Style::Opto,
        oversample: 2,
        link: 0.4,
        ..eq
    };
    vec![
        (CompSettings::default(), false),
        (eq, false),
        (eq, true),
        (rms, true),
        (
            CompSettings {
                style: Style::Mastering,
                lookahead_ms: 20.0,
                oversample: 4,
                ..CompSettings::default()
            },
            false,
        ),
    ]
}

/// Taken from Comprimo before Cedo shared its key sections and its delay line.
const BEFORE: [u64; 5] = [
    0x69ff_05ac_52d8_eb0e,
    0xb94d_85e1_404f_5a68,
    0x1416_248f_18cf_c906,
    0x68de_9f5c_c9f9_18c7,
    0xd449_2d38_fd37_1bd5,
];

#[test]
fn comprimos_outputs_are_bit_identical_to_before_the_sharing() {
    let got: Vec<u64> = settings().iter().map(|(s, k)| run(s, *k)).collect();
    assert_eq!(got, BEFORE);
}
