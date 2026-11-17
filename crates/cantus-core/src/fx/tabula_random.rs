//! Tabula's *Randomise* (ADR 0131 §2): new values for a board's pedals within musical ranges
//! — never a wall of feedback, a mix of nothing, or a delay of a minute — seeded, so the
//! same seed gives the same board (and the command that sets them is one undo step).

use crate::effect::{EffectParamInfo, EffectSpec};
use crate::fx::tabula::{pedal_key, PedalId};

/// The musical part of a parameter's range, by its key (the rest: the middle 70 % of its
/// range, along its taper). Stepped parameters are left as they are.
fn range_of(key: &str, i: &EffectParamInfo) -> Option<(f32, f32)> {
    if i.stepped {
        return None;
    }
    let tail = key.rsplit('.').next().unwrap_or(key);
    let r = match tail {
        "mix" => (0.15, 0.6),
        "feedback" | "fb" => (0.1, 0.65),
        "time_ms" => (90.0, 650.0),
        "rate" => (0.15, 5.0),
        "depth" => (0.2, 0.85),
        "drive" | "dist" | "fuzz" | "sustain" => (0.1, 0.85),
        "tone" | "filter" => (0.25, 0.75),
        "level" | "vol" => (0.4, 0.7),
        "decay" => (0.6, 4.0),
        "space" => (0.25, 0.8),
        "out" if i.unit == "dB" => return None,
        "in" | "gain" if i.unit == "dB" => return None,
        _ => {
            let (lo, hi) = (i.min, i.max);
            if i.logarithmic && lo > 0.0 {
                let r = hi / lo;
                (lo * pow(r, 0.15), lo * pow(r, 0.85))
            } else {
                (lo + (hi - lo) * 0.15, lo + (hi - lo) * 0.85)
            }
        }
    };
    Some((r.0.clamp(i.min, i.max), r.1.clamp(i.min, i.max)))
}

fn pow(base: f32, e: f32) -> f32 {
    cantus_dsp::math::powf(base, e)
}

/// 0 … 1 from a seed and a place (a pedal, a key): the same each time.
fn hash01(seed: u64, pedal: PedalId, key: &str) -> f32 {
    let mut h = seed ^ 0x9E37_79B9_7F4A_7C15 ^ (u64::from(pedal.0) << 32);
    for b in key.bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01B3);
    }
    h ^= h >> 33;
    h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    h ^= h >> 33;
    (h >> 40) as f32 / (1u64 << 24) as f32
}

/// New values for `keys` (each a pedal and one of its own keys) on the board `spec`, from
/// `seed`: the board's `p{id}.{key}` and the value, within the key's musical range along its
/// taper. Keys the pedal has not, and stepped ones, are left out.
pub fn randomise(spec: &EffectSpec, keys: &[(PedalId, &str)], seed: u64) -> Vec<(String, f32)> {
    let mut out = Vec::new();
    for (id, key) in keys {
        let k = pedal_key(*id, key);
        let Some(info) = spec.param(&k) else {
            continue;
        };
        let Some((lo, hi)) = range_of(key, info) else {
            continue;
        };
        let x = hash01(seed, *id, key);
        let v = if info.logarithmic && lo > 0.0 && hi > lo {
            lo * pow(hi / lo, x)
        } else {
            lo + (hi - lo) * x
        };
        out.push((k, v));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::tabula::guitar_rig;

    #[test]
    fn randomise_is_seeded_and_stays_musical() {
        let rig = guitar_rig();
        let keys = [
            (PedalId(2), "time_ms"),
            (PedalId(2), "feedback"),
            (PedalId(2), "mix"),
            (PedalId(1), "mode"),
            (PedalId(0), "od.drive"),
            (PedalId(9), "mix"),
        ];
        let a = randomise(&rig, &keys, 7);
        assert_eq!(
            a,
            randomise(&rig, &keys, 7),
            "the same seed, the same board"
        );
        assert_ne!(a, randomise(&rig, &keys, 8));
        // A stepped key and a pedal the board has not: left out.
        assert_eq!(a.len(), 4);
        for (k, v) in &a {
            match k.as_str() {
                "p2.time_ms" => assert!((90.0..=650.0).contains(v)),
                "p2.feedback" => assert!((0.1..=0.65).contains(v)),
                "p2.mix" => assert!((0.15..=0.6).contains(v)),
                "p0.od.drive" => assert!((0.1..=0.85).contains(v)),
                _ => panic!("{k}"),
            }
        }
    }
}
