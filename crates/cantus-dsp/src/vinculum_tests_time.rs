//! Vinculum's time constants, measured on step responses against the stated targets.

use super::super::*;
use super::{unit, SR};

/// The reduction, dB, sample by sample, for a steady `level` lasting `on` seconds and
/// then silence for `off` seconds.
fn trace(s: &VinculumSettings, level: f32, on: f32, off: f32) -> Vec<f32> {
    let mut v = unit(s);
    let (n_on, n_off) = ((on * SR) as usize, (off * SR) as usize);
    let mut out = Vec::with_capacity(n_on + n_off);
    let mut l = [0f32; 1];
    let mut r = [0f32; 1];
    for i in 0..n_on + n_off {
        let x = if i < n_on { level } else { 0.0 };
        l[0] = x;
        r[0] = x;
        v.process(&mut l, &mut r);
        out.push(v.gain_reduction_db());
    }
    out
}

/// Seconds from `from` until the trace first satisfies `done`.
fn time_until(t: &[f32], from: usize, done: impl Fn(f32) -> bool) -> f32 {
    let at = t[from..]
        .iter()
        .position(|g| done(*g))
        .expect("never reached");
    at as f32 / SR
}

#[test]
fn the_vca_attack_and_release_are_their_time_constants() {
    for (attack, release) in [(0.1, 100.0), (1.0, 300.0), (10.0, 600.0), (30.0, 1200.0)] {
        let s = VinculumSettings {
            threshold_db: -20.0,
            ratio: 4.0,
            attack_ms: attack,
            release_ms: release,
            ..VinculumSettings::default()
        };
        let t = trace(&s, 1.0, 0.5, 3.0);
        let settled = t[(0.5 * SR) as usize - 1];
        assert!((settled - 15.0).abs() < 0.01, "{settled}");
        let a = time_until(&t, 0, |g| g >= 0.632 * settled) * 1000.0;
        let on = (0.5 * SR) as usize;
        let r = time_until(&t, on, |g| g <= 0.368 * settled) * 1000.0;
        let tol = 1000.0 / SR;
        assert!(
            (a - attack).abs() <= 0.02 * attack + tol,
            "attack {a} ms, set {attack}"
        );
        assert!(
            (r - release).abs() <= 0.02 * release + tol,
            "release {r} ms, set {release}"
        );
    }
}

#[test]
fn the_vca_auto_release_follows_the_programme() {
    let s = VinculumSettings {
        threshold_db: -20.0,
        ratio: 4.0,
        attack_ms: 0.3,
        auto_release: true,
        ..VinculumSettings::default()
    };
    let mut times = Vec::new();
    for on in [0.02, 1.0, 3.0] {
        let t = trace(&s, 1.0, on, 6.0);
        let at = (on * SR) as usize;
        let peak = t[at - 1];
        times.push(time_until(&t, at, |g| g <= 0.368 * peak));
    }
    println!(
        "auto release to 37 %: after 20 ms {:.3} s, 1 s {:.3} s, 3 s {:.3} s",
        times[0], times[1], times[2]
    );
    assert!(
        times[0] <= 1.3 * AUTO_FAST_MS / 1000.0,
        "a lone hit: {} s",
        times[0]
    );
    assert!(
        times[2] >= 0.6 && times[2] >= 4.0 * times[0],
        "sustained: {} s",
        times[2]
    );
    assert!(times[0] < times[1] && times[1] < times[2]);
}

#[test]
fn the_opto_meets_the_cells_published_times() {
    // Published for the T4 cell's classic leveller: attack about 10 ms; release about
    // 60 ms for the first half, then the rest over 0.5 to 5 s (1 to 15 s in another
    // telling), longer the longer and harder it has worked.
    let s = VinculumSettings {
        model: Model::Opto,
        threshold_db: -20.0,
        ..VinculumSettings::default()
    };
    let level = crate::math::db_to_gain(-4.0);
    let mut fulls = Vec::new();
    for on in [0.1, 1.0, 10.0] {
        let t = trace(&s, level, on, 15.0);
        let at = (on * SR) as usize;
        let gr = t[at - 1];
        if on == 1.0 {
            let attack = time_until(&t, 0, |g| g >= 0.632 * gr) * 1000.0;
            println!("opto: {gr:.1} dB, attack to 63 % {attack:.1} ms");
            assert!((5.0..=20.0).contains(&attack), "attack {attack} ms");
        }
        let half = time_until(&t, at, |g| g <= 0.5 * gr) * 1000.0;
        let full = time_until(&t, at, |g| g <= 0.1 * gr);
        println!("opto after {on} s at {gr:.1} dB: half in {half:.0} ms, 90 % in {full:.2} s");
        assert!(
            (35.0..=90.0).contains(&half),
            "half release {half} ms after {on} s"
        );
        fulls.push(full);
    }
    assert!((0.3..=1.5).contains(&fulls[0]), "short: {} s", fulls[0]);
    assert!((2.0..=8.0).contains(&fulls[2]), "long: {} s", fulls[2]);
    assert!(fulls[0] < fulls[1] && fulls[1] < fulls[2]);
}

#[test]
fn the_vari_mu_positions_release_as_published() {
    for (pos, release_ms) in [(0usize, 300.0f32), (1, 800.0), (2, 2000.0), (3, 5000.0)] {
        let s = VinculumSettings {
            model: Model::VariMu,
            threshold_db: -20.0,
            mu_time: pos,
            ..VinculumSettings::default()
        };
        let t = trace(&s, 1.0, 0.5, release_ms / 1000.0 * 3.0);
        let at = (0.5 * SR) as usize;
        let gr = t[at - 1];
        let attack = time_until(&t, 0, |g| g >= 0.632 * gr) * 1000.0;
        let r = time_until(&t, at, |g| g <= 0.368 * gr) * 1000.0;
        println!(
            "vari-mu position {}: {gr:.1} dB, attack to 63 % {attack:.2} ms (set {}), release {r:.0} ms",
            pos + 1,
            MU_TIMES[pos].0
        );
        // Released, the valve's charge falls freely: its reduction is the time constant.
        assert!(
            (r - release_ms).abs() <= 0.02 * release_ms,
            "release {r} ms"
        );
        // In the loop the attack is quicker than the rectifier's own (the output falls as
        // it charges), never slower.
        assert!(
            attack <= MU_TIMES[pos].0 * 1.05 + 0.05,
            "attack {attack} ms"
        );
    }
}

#[test]
fn the_vari_mu_programme_positions_hold_what_lasted() {
    for pos in [4usize, 5] {
        let s = VinculumSettings {
            model: Model::VariMu,
            threshold_db: -20.0,
            mu_time: pos,
            ..VinculumSettings::default()
        };
        let mut times = Vec::new();
        for on in [0.05, 8.0] {
            let t = trace(&s, 1.0, on, 40.0);
            let at = (on * SR) as usize;
            let gr = t[at - 1];
            times.push(time_until(&t, at, |g| g <= 0.368 * gr));
        }
        println!(
            "vari-mu position {}: a peak lets go in {:.2} s, sustained programme in {:.2} s",
            pos + 1,
            times[0],
            times[1]
        );
        let fast = MU_TIMES[pos].1 / 1000.0;
        assert!(times[0] <= 1.3 * fast, "a peak: {} s", times[0]);
        assert!(times[1] >= 3.0 * times[0], "sustained: {} s", times[1]);
    }
}
