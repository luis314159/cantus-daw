//! What Comparatio shares with the family's loudness code, and how the sharing is guarded.
//!
//! Comprimo kept its own copy of BS.1770's K-weighting (the same analog prototypes and the
//! same transposed direct form II as Terminus's); Comparatio's branch made it Terminus's
//! sections, two of them, so the family has **one** K-weighting. These hashes were taken
//! from Comprimo's loudness meter, its K-weighting, Comprimo's output with its auto gain
//! and its loudness-matched compare (both driven by that meter), and Terminus's meter,
//! **before** the change (commit noted in `docs/round-fx/comparatio.md`), and must never
//! change.

use crate::comprimo::loudness::{KWeighting as CompK, Meter as CompMeter, HOP_S};
use crate::comprimo::{CompSettings, Comprimo, Listen};
use crate::math;
use crate::terminus::loudness::LoudnessMeter;

pub(crate) fn fnv(bits: impl Iterator<Item = u64>) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in bits {
        for byte in b.to_le_bytes() {
            h ^= byte as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// Four seconds of programme at `sr`: two tones, noise, a gap of silence long enough for
/// the filters to come to rest, and a loud burst.
fn material(sr: f32) -> (Vec<f32>, Vec<f32>) {
    let mut s = 7u32;
    let mut noise = move || {
        s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (s >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    };
    let n = (sr * 4.0) as usize;
    let mut l = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    for i in 0..n {
        let t = i as f32 / sr;
        let gap = (1.2..2.3).contains(&t);
        if !gap {
            let loud = if (3.0..3.4).contains(&t) { 0.9 } else { 0.25 };
            l[i] = loud * math::sin(std::f32::consts::TAU * 97.0 * t) + 0.08 * noise();
            r[i] = loud * 0.7 * math::sin(std::f32::consts::TAU * 3_100.0 * t) + 0.08 * noise();
        }
    }
    (l, r)
}

fn comprimo_meter_hash(sr: f32) -> u64 {
    let (l, r) = material(sr);
    let hop = ((sr as f64 * HOP_S).round() as usize).max(1);
    let mut m = CompMeter::new(sr);
    let mut k = CompK::new(sr);
    let mut out = Vec::new();
    for (i, (&a, &b)) in l.iter().zip(&r).enumerate() {
        m.add(a, b);
        out.push(k.tick(a, b).to_bits());
        if (i + 1) % hop == 0 {
            m.end_hop(hop);
            out.push(m.momentary().to_bits() as u64);
            out.push(m.short_term().to_bits() as u64);
            out.push(m.is_clear() as u64);
        }
    }
    fnv(out.into_iter())
}

fn comprimo_output_hash(s: &CompSettings) -> u64 {
    let (mut l, mut r) = material(48_000.0);
    let mut c = Comprimo::new(48_000.0);
    c.set(s);
    for (bl, br) in l.chunks_mut(300).zip(r.chunks_mut(300)) {
        c.process(bl, br, None);
    }
    fnv(l.iter().chain(&r).map(|v| v.to_bits() as u64))
}

fn terminus_meter_hash() -> u64 {
    let (l, r) = material(48_000.0);
    let mut m = LoudnessMeter::new(48_000.0);
    let mut out = Vec::new();
    for (bl, br) in l.chunks(480).zip(r.chunks(480)) {
        m.process(bl, br);
        out.push(m.momentary().to_bits());
        out.push(m.short_term().to_bits());
        out.push(m.max_true_peak.to_bits() as u64);
    }
    out.push(m.integrated().map(f64::to_bits).unwrap_or(1));
    out.push(m.range().map(f64::to_bits).unwrap_or(1));
    fnv(out.into_iter())
}

fn all_hashes() -> [u64; 7] {
    let auto = CompSettings {
        threshold_db: -30.0,
        auto_gain: true,
        ..CompSettings::default()
    };
    let compare = CompSettings {
        threshold_db: -30.0,
        makeup_db: 6.0,
        listen: Listen::Compare,
        ..CompSettings::default()
    };
    [
        comprimo_meter_hash(44_100.0),
        comprimo_meter_hash(48_000.0),
        comprimo_meter_hash(96_000.0),
        comprimo_output_hash(&auto),
        comprimo_output_hash(&compare),
        comprimo_output_hash(&CompSettings::default()),
        terminus_meter_hash(),
    ]
}

/// Taken before the K-weighting was shared (see the module notes).
const BEFORE: [u64; 7] = [
    14503866490974665177,
    8890811422095702052,
    15441849906119657620,
    4533592652948579966,
    10887660794199736770,
    16658428786669141933,
    12052274695875544797,
];

#[test]
fn the_shared_k_weighting_moves_no_bit_of_comprimo_or_terminus() {
    let now = all_hashes();
    eprintln!("hashes: {now:#018x?}");
    assert_eq!(now, BEFORE);
}
