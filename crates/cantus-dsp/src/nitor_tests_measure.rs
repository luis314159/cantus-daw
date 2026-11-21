//! Nitor measured: what each character does to the programme, at two levels (the numbers
//! the recipes were set from, printed with `--nocapture`).

use super::testkit::*;
use super::*;
use crate::math;

/// The programme brought to the reference: each band's detector level (Multiplex's) and the
/// character's band gains, sampled every 10 ms after the first 2 s.
fn band_stats(c: Character, gain: f32) -> ([f32; BANDS], [f32; BANDS], [f32; BANDS]) {
    let input = programme(8.0, gain, 29);
    let mut u = Nitor::new(SR);
    u.set(&NitorSettings {
        amount: 1.0,
        character: c,
        level_match: false,
        ..NitorSettings::default()
    });
    let mut level_max = [-200.0f32; BANDS];
    let mut g_min = [0.0f32; BANDS];
    let mut g_max = [-100.0f32; BANDS];
    let (mut l, mut r) = input;
    let step = 480;
    for (k, (a, b)) in l.chunks_mut(step).zip(r.chunks_mut(step)).enumerate() {
        u.process(a, b);
        if k * step < 2 * SR as usize {
            continue;
        }
        let lv = u.dynamics().levels_now_db();
        let g = u.meter().band_db;
        for i in 0..BANDS {
            level_max[i] = level_max[i].max(lv[i]);
            g_min[i] = g_min[i].min(g[i]);
            g_max[i] = g_max[i].max(g[i]);
        }
    }
    (level_max, g_min, g_max)
}

#[test]
fn band_levels_of_programme_at_the_reference() {
    for c in Character::ALL {
        for gain in [1.0f32, 0.1] {
            let (lv, lo, hi) = band_stats(c, gain);
            eprintln!(
                "{c:?} at {:+.0} dB: band peaks {:?}, gains {:?} … {:?}",
                20.0 * math::log10(gain),
                lv.map(|v| (v * 10.0).round() / 10.0),
                lo.map(|v| (v * 10.0).round() / 10.0),
                hi.map(|v| (v * 10.0).round() / 10.0)
            );
            // Level independence: the pre-gain brings both levels to the same place.
            if gain < 1.0 {
                let (lv1, lo1, _) = band_stats(c, 1.0);
                for i in 0..BANDS {
                    assert!((lv[i] - lv1[i]).abs() < 1.5, "{c:?} band {i}: {} vs {}", lv[i], lv1[i]);
                    assert!((lo[i] - lo1[i]).abs() < 1.5, "{c:?} band {i}");
                }
            }
        }
    }
}

#[test]
fn what_each_character_does_to_the_programme() {
    let input = programme(8.0, 1.0, 37);
    let dry = loudness(&input, 3 * SR as usize);
    for c in Character::ALL {
        for amount in [0.35f32, 1.0] {
            let mut u = Nitor::new(SR);
            u.set(&NitorSettings {
                amount,
                character: c,
                ..NitorSettings::default()
            });
            let mut l = input.0.clone();
            let mut r = input.1.clone();
            for (a, b) in l.chunks_mut(512).zip(r.chunks_mut(512)) {
                u.process(a, b);
            }
            let m = *u.meter();
            let wet = loudness(&(l, r), 3 * SR as usize);
            eprintln!(
                "{c:?} at {:.0} %: match {:+.2} dB, pre {:+.1} dB, bands {:?}, exciter {:?}, out {:+.2} LU",
                100.0 * amount,
                m.match_db,
                m.pre_db,
                m.band_db.map(|v| (v * 10.0).round() / 10.0),
                m.excite,
                wet - dry
            );
            assert!(m.match_db < 0.5, "{c:?}: the match takes back what the character adds");
        }
    }
}
