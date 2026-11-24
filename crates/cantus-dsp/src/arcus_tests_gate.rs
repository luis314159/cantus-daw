//! Arcus's gate (ADR 0157), measured before anyone listens: the Schelleng region, the
//! attack's pre-Helmholtz time, the bowed pitch, the vibrato's partials moving through the
//! body, the body's modes. The string alone is bowed with a fixed force, speed and contact
//! point for the Schelleng and Guettler tests (as a bowing machine bows it); the instrument
//! is played for the rest.

use super::string::{Bow, BowedString, Friction, StringStep};
use super::tables::{schelleng_max, Instrument};
use super::tests::SR;
use crate::math;
use std::cell::Cell;

thread_local! {
    /// The corner the probes bow with (the instrument's by default).
    pub(super) static CORNER: Cell<f64> = const { Cell::new(3000.0) };
    /// The friction curve the probes bow with (`None`: the default).
    pub(super) static CURVE: Cell<Option<super::string::Curve>> = const { Cell::new(None) };
    /// The torsion the probes bow with (`Some(None)`: none; `None`: the default).
    pub(super) static TORSION: Cell<Option<Option<super::string::Torsion>>> = const { Cell::new(None) };
}

/// What the bow did, sample by sample: the slip (0 while it sticks).
pub(super) struct Bowed {
    pub slips: Vec<f32>,
    #[allow(dead_code)]
    pub out: Vec<f32>,
    pub period: f64,
    pub beta: f64,
}

/// Bow string `ix` of `inst` stopped at `key`, at `beta`, pressing `force` (N), the bow
/// accelerating at `accel` (m/s²) to `speed` (m/s), for `seconds`.
#[allow(clippy::too_many_arguments)]
pub(super) fn bow_string(
    inst: Instrument,
    ix: usize,
    key: f64,
    beta: f64,
    force: f32,
    speed: f32,
    accel: f32,
    seconds: f64,
    friction: Friction,
) -> Bowed {
    bow_gesture(
        inst,
        ix,
        key,
        beta,
        (force, force),
        speed,
        accel,
        seconds,
        friction,
    )
}

/// [`bow_string`] with the force placed at `force.0` while the bow accelerates, then gliding
/// to `force.1` (a 60 ms one-pole) once it has its speed — the voice's attack.
#[allow(clippy::too_many_arguments)]
pub(super) fn bow_gesture(
    inst: Instrument,
    ix: usize,
    key: f64,
    beta: f64,
    force: (f32, f32),
    speed: f32,
    accel: f32,
    seconds: f64,
    friction: Friction,
) -> Bowed {
    let spec = inst.spec();
    let st = spec.strings[ix];
    let f = 440.0 * math::exp2((key - 69.0) / 12.0);
    let stopped = (st.open_hz() / f).clamp(0.1, 1.0);
    let t60 = st.t60_s * (0.55 + 0.45 * stopped);
    let mut s = BowedString::new(SR);
    let step = StringStep {
        f0: f,
        t60,
        p: st.p,
        beta,
        dc: false,
        corner_hz: CORNER.with(|c| c.get()),
    };
    if let Some(k) = CURVE.with(|c| c.get()) {
        s.set_curve(k);
    }
    if let Some(t) = TORSION.with(|c| c.get()) {
        s.set_torsion(t);
    }
    s.start(st.z0() as f32, &step, false);
    let n = (seconds * SR as f64) as usize;
    let mut out = vec![0.0f32; n];
    let mut slips = vec![0.0f32; n];
    let zeros = [0.0f32; 32];
    let mut v = [0.0f32; 32];
    let mut fs = [0.0f32; 32];
    let mut f_now = force.0;
    let keep = math::exp(-1.0 / (0.06 * SR));
    for at in (0..n).step_by(32) {
        let k = 32.min(n - at);
        for j in 0..k {
            let t = (at + j + 1) as f32 / SR;
            v[j] = (accel * t).min(speed);
            if v[j] >= speed {
                f_now = force.1 + (f_now - force.1) * keep;
            }
            fs[j] = f_now;
        }
        let bow = Bow {
            v: &v[..k],
            force: &fs[..k],
            rough: &zeros[..k],
            friction,
        };
        s.render(
            &zeros[..k],
            Some(bow),
            1.0,
            &mut out[at..at + k],
            Some(&mut slips[at..at + k]),
            false,
        );
    }
    Bowed {
        slips,
        out,
        period: SR as f64 / f,
        beta,
    }
}

/// The slips' onsets (the samples where a stick ends) from `from` on, a stick shorter than
/// 3 % of the period not counted (the ripple's chatter inside a slip: Schoonderwaldt 2008).
pub(super) fn onsets(slips: &[f32], from: usize, period: f64) -> Vec<usize> {
    onsets_after(slips, from, (0.03 * period).max(1.0) as usize)
}

/// The slips with every slip shorter than `min_slip` samples taken for a stick (a
/// secondary slip inside the stick — the ripple's — does not end it).
pub(super) fn without_short_slips(slips: &[f32], min_slip: usize) -> Vec<f32> {
    let mut out = slips.to_vec();
    let mut i = 0;
    while i < out.len() {
        if out[i] != 0.0 {
            let start = i;
            while i < out.len() && out[i] != 0.0 {
                i += 1;
            }
            if i - start < min_slip && i < out.len() {
                out[start..i].fill(0.0);
            }
        } else {
            i += 1;
        }
    }
    out
}

/// The onsets of the slips that follow a stick at least `min_stick` samples long.
pub(super) fn onsets_after(slips: &[f32], from: usize, min_stick: usize) -> Vec<usize> {
    let mut o = Vec::new();
    let mut stuck = 0usize;
    for i in from.max(1)..slips.len() {
        if slips[i] == 0.0 {
            stuck += 1;
        } else {
            if slips[i - 1] == 0.0 && stuck >= min_stick {
                o.push(i);
            }
            stuck = 0;
        }
    }
    o
}

/// The motion over `slips[from..]`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum Motion {
    /// One slip a period, the onsets a period apart.
    Helmholtz,
    /// More than one slip a period, periodic (too little force: "surface" sound).
    Multiple,
    /// Irregular, or periods longer than the string's (too much force: raucous).
    Raucous,
    /// The bow never holds the string.
    Sliding,
}

pub(super) fn classify(b: &Bowed, from: usize) -> Motion {
    let periods = (b.slips.len() - from) as f64 / b.period;
    // Helmholtz motion: one long stick a period — (1 − β) of it; a secondary slip inside
    // (the ripple's) does not break it.
    let long = (0.6 * (1.0 - b.beta) * b.period) as usize;
    let clean = without_short_slips(&b.slips, (0.25 * b.beta * b.period).max(2.0) as usize);
    let main = onsets_after(&clean, from, long);
    let even = |o: &[usize], k: usize| {
        (0..o.len().saturating_sub(k))
            .map(|i| ((o[i + k] - o[i]) as f64 / b.period - 1.0).abs())
            .fold(0.0f64, f64::max)
    };
    if (main.len() as f64 / periods - 1.0).abs() < 0.06 && even(&main, 1) < 0.04 {
        return Motion::Helmholtz;
    }
    let o = onsets(&b.slips, from, b.period);
    if o.len() < 3 {
        return Motion::Sliding;
    }
    let per_period = o.len() as f64 / periods;
    let k = per_period.round().max(1.0) as usize;
    if k >= 2 && even(&o, k) < 0.04 && (per_period - k as f64).abs() < 0.1 {
        Motion::Multiple
    } else {
        Motion::Raucous
    }
}

/// The pre-Helmholtz time, s: from the bow's start to the first of six onsets in a row a
/// period apart (within 4 %), one to each period. `None`: it never settles in `b`.
pub(super) fn attack_time(b: &Bowed) -> Option<f64> {
    let long = (0.6 * (1.0 - b.beta) * b.period) as usize;
    let clean = without_short_slips(&b.slips, (0.25 * b.beta * b.period).max(2.0) as usize);
    let o = onsets_after(&clean, 0, long);
    let ok = |g: usize| ((g as f64) / b.period - 1.0).abs() < 0.04;
    for i in 0..o.len().saturating_sub(6) {
        if (0..5).all(|j| ok(o[i + j + 1] - o[i + j])) {
            return Some(o[i] as f64 / SR as f64);
        }
    }
    None
}

#[test]
#[ignore = "a probe: cargo test -p cantus-dsp --lib arcus::tests_gate::probe -- --ignored --nocapture"]
fn probe_schelleng_cello_d() {
    let inst = Instrument::Cello;
    let z0 = inst.spec().strings[2].z0();
    for speed in [0.1f32, 0.2] {
        println!("cello open D, v = {speed} m/s (rows β, columns F/F_max)");
        for beta in [0.04f64, 0.06, 0.08, 0.1, 0.13, 0.17, 0.22] {
            let fmax = schelleng_max(z0, speed as f64, beta) as f32;
            let mut line = format!("β {beta:.2} Fmax {fmax:5.2} N:");
            for frac in [
                0.01f32, 0.03, 0.06, 0.1, 0.15, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.85, 1.0, 1.2, 1.4,
            ] {
                let b = bow_string(
                    inst,
                    2,
                    50.0,
                    beta,
                    fmax * frac,
                    speed,
                    20.0,
                    0.8,
                    Friction::Hyperbolic,
                );
                let m = classify(&b, (0.5 * SR) as usize);
                line += match m {
                    Motion::Helmholtz => " H",
                    Motion::Multiple => " m",
                    Motion::Raucous => " R",
                    Motion::Sliding => " .",
                };
            }
            println!("{line}");
        }
    }
}

#[test]
#[ignore = "a probe: cargo test -p cantus-dsp --lib arcus::tests_gate::probe -- --ignored --nocapture"]
fn probe_curves() {
    use super::string::Curve;
    let inst = Instrument::Cello;
    let z0 = inst.spec().strings[2].z0();
    let (speed, beta) = (0.15f32, 0.1f64);
    let fmax = schelleng_max(z0, speed as f64, beta) as f32;
    let mut cases: Vec<(String, Friction, Curve)> = Vec::new();
    for v0 in [0.02f32, 0.05, 0.1, 0.2, 0.4] {
        cases.push((
            format!("hyp v0 {v0}"),
            Friction::Hyperbolic,
            Curve {
                v0,
                ..Curve::default()
            },
        ));
    }
    for (tau, w) in [
        (0.0002f32, 2.0f32),
        (0.0002, 8.0),
        (0.001, 2.0),
        (0.001, 6.0),
        (0.003, 1.0),
        (0.003, 4.0),
    ] {
        cases.push((
            format!("thermal tau {tau} per_w {w}"),
            Friction::Thermal,
            Curve {
                tau_s: tau,
                per_w: w,
                ..Curve::default()
            },
        ));
    }
    for corner in [0.0f64, 3500.0] {
        CORNER.with(|c| c.set(corner));
        for (name, fr, k) in &cases {
            CURVE.with(|c| c.set(Some(*k)));
            let fm = schelleng_max(z0, 0.1, 0.1) as f32;
            let mut steady = String::new();
            for frac in [0.05f32, 0.1, 0.15, 0.2, 0.3, 0.5, 0.7, 0.85, 1.0, 1.2] {
                let b = bow_string(inst, 2, 50.0, 0.1, fm * frac, 0.1, 20.0, 0.8, *fr);
                steady += match classify(&b, (0.5 * SR) as usize) {
                    Motion::Helmholtz => "H",
                    Motion::Multiple => "m",
                    Motion::Raucous => "R",
                    Motion::Sliding => ".",
                };
            }
            let mut good = 0;
            let mut sum = 0.0;
            let mut cells = 0;
            for accel in [1.0f32, 2.0, 4.0, 8.0, 16.0] {
                for frac in [0.2f32, 0.3, 0.4, 0.5, 0.6, 0.8] {
                    let b = bow_gesture(
                        inst,
                        2,
                        50.0,
                        beta,
                        (fmax * frac, 0.5 * fmax),
                        speed,
                        accel,
                        0.4,
                        *fr,
                    );
                    let t = attack_time(&b).unwrap_or(0.4);
                    cells += 1;
                    sum += t;
                    if t <= 0.05 {
                        good += 1;
                    }
                }
            }
            println!("corner {corner:5.0} {name:32}: steady {steady}  attacks <=50 ms {good}/{cells}, mean {:.0} ms", 1000.0 * sum / cells as f64);
        }
    }
    CURVE.with(|c| c.set(None));
    CORNER.with(|c| c.set(3000.0));
}

#[test]
#[ignore = "a probe: cargo test -p cantus-dsp --lib arcus::tests_gate::probe -- --ignored --nocapture"]
fn probe_torsion() {
    use super::string::Torsion;
    let inst = Instrument::Cello;
    let z0 = inst.spec().strings[2].z0();
    let (speed, beta) = (0.15f32, 0.1f64);
    let fmax = schelleng_max(z0, speed as f64, beta) as f32;
    let mut cases: Vec<(String, Option<Torsion>)> = vec![("none".into(), None)];
    for ratio in [5.0f64, 7.0] {
        for imp in [1.0f32, 1.5, 2.0, 3.0] {
            for q in [3.0f32, 5.0, 8.0] {
                cases.push((
                    format!("ratio {ratio} Z {imp} Q {q}"),
                    Some(Torsion {
                        ratio,
                        impedance: imp,
                        q,
                    }),
                ));
            }
        }
    }
    for (name, t) in &cases {
        TORSION.with(|c| c.set(Some(*t)));
        let fm = schelleng_max(z0, 0.1, 0.1) as f32;
        let mut steady = String::new();
        for frac in [
            0.05f32, 0.1, 0.15, 0.2, 0.3, 0.5, 0.7, 0.85, 1.0, 1.2, 1.5, 2.0,
        ] {
            let b = bow_string(
                inst,
                2,
                50.0,
                0.1,
                fm * frac,
                0.1,
                20.0,
                0.8,
                Friction::Hyperbolic,
            );
            steady += match classify(&b, (0.5 * SR) as usize) {
                Motion::Helmholtz => "H",
                Motion::Multiple => "m",
                Motion::Raucous => "R",
                Motion::Sliding => ".",
            };
        }
        let mut good = 0;
        let mut sum = 0.0;
        let mut cells = 0;
        let mut grid = String::new();
        for accel in [1.0f32, 2.0, 4.0, 8.0, 16.0, 32.0] {
            for frac in [0.2f32, 0.3, 0.4, 0.5, 0.6, 0.8, 1.0, 1.3] {
                let b = bow_gesture(
                    inst,
                    2,
                    50.0,
                    beta,
                    (fmax * frac, 0.5 * fmax),
                    speed,
                    accel,
                    0.4,
                    Friction::Hyperbolic,
                );
                let t = attack_time(&b).unwrap_or(0.4);
                cells += 1;
                sum += t;
                if t <= 0.05 {
                    good += 1;
                    grid += "+";
                } else {
                    grid += ".";
                }
            }
            grid += "|";
        }
        println!(
            "{name:24}: steady {steady}  attacks <=50 ms {good}/{cells} {grid} mean {:.0} ms",
            1000.0 * sum / cells as f64
        );
    }
    TORSION.with(|c| c.set(None));
}
