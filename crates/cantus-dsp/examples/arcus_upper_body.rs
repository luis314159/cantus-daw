//! Arcus's violin body fitted to a measured bridge admittance (round 42, `arcus-upper`).
//! Reads the CNSM dataset's admittance CSVs (Zenodo 10.5281/zenodo.18696786, CC BY 4.0:
//! `f,H_real,H_imag,…`, 0 – 25.6 kHz every 1.5625 Hz), averages one violin's
//! measurements, takes out the acquisition's delay (the one that makes the admittance's real
//! part positive at every frequency, as a passive driving point's is), fits a sum of
//! band-pass modes to it, prints the fit's distance beside the distance between the three
//! violins, and writes `crates/cantus-dsp/src/arcus_upper_body_data.rs`.
//! Run by `tools/arcus/upper-body.sh`:
//!
//! ```text
//! cargo run -p cantus-dsp --release --example arcus_upper_body -- <cnsm admittances dir> <violin> <out.rs>
//! ```

use cantus_dsp::math;
use std::f64::consts::{PI, TAU};
use std::fmt::Write as _;
use std::path::Path;

/// The band fitted, Hz, and the modes it is fitted with.
const LO: f64 = 120.0;
const HI: f64 = 12_000.0;
const MODES: usize = 64;

type C = (f64, f64);

fn read(path: &Path) -> Option<(Vec<f64>, Vec<C>)> {
    let text = std::fs::read_to_string(path).ok()?;
    let (mut f, mut h) = (Vec::new(), Vec::new());
    for line in text.lines().skip(1) {
        let v: Vec<f64> = line.split(',').filter_map(|x| x.parse().ok()).collect();
        if v.len() >= 3 {
            f.push(v[0]);
            h.push((v[1], v[2]));
        }
    }
    Some((f, h))
}

/// One violin's measurements on the common grid (16 385 points), averaged.
fn violin(dir: &Path, name: &str) -> (Vec<f64>, Vec<C>) {
    let mut sum: Vec<C> = Vec::new();
    let mut freq = Vec::new();
    let mut count = 0;
    let mut files = Vec::new();
    for phase in ["Phase 1", "Phase 2"] {
        if let Ok(rd) = std::fs::read_dir(dir.join(phase).join(name)) {
            for e in rd.flatten() {
                files.push(e.path());
            }
        }
    }
    files.sort();
    for p in files {
        let Some((f, h)) = read(&p) else { continue };
        if f.len() != 16_385 {
            continue; // a finer measurement on another grid
        }
        if sum.is_empty() {
            sum = vec![(0.0, 0.0); h.len()];
            freq = f;
        }
        for (s, x) in sum.iter_mut().zip(&h) {
            s.0 += x.0;
            s.1 += x.1;
        }
        count += 1;
    }
    assert!(
        count > 0,
        "no measurement of {name} under {}",
        dir.display()
    );
    for s in sum.iter_mut() {
        s.0 /= count as f64;
        s.1 /= count as f64;
    }
    eprintln!("{name}: {count} measurements averaged");
    (freq, sum)
}

/// The delay that leaves the fewest points with a negative real part (200 Hz – 8 kHz).
fn undelay(f: &[f64], h: &mut [C]) -> f64 {
    let mut best = (0.0, usize::MAX);
    for i in 0..400 {
        let tau = 0.9e-3 + i as f64 * 2e-6;
        let neg = f
            .iter()
            .zip(h.iter())
            .filter(|(fr, _)| **fr > 200.0 && **fr < 8000.0)
            .filter(|(fr, x)| {
                let (s, c) = math::sin_cos(TAU * **fr * tau);
                x.0 * c - x.1 * s < 0.0
            })
            .count();
        if neg < best.1 {
            best = (tau, neg);
        }
    }
    for (fr, x) in f.iter().zip(h.iter_mut()) {
        let (s, c) = math::sin_cos(TAU * fr * best.0);
        *x = (x.0 * c - x.1 * s, x.0 * s + x.1 * c);
    }
    eprintln!(
        "  delay {:.4} ms taken out ({} points left with Re < 0)",
        best.0 * 1e3,
        best.1
    );
    best.0
}

/// A band-pass mode's admittance at `f`: `(j f/(f_k Q)) / (1 − (f/f_k)² + j f/(f_k Q))`.
fn basis(f: f64, hz: f64, q: f64) -> C {
    let x = f / hz;
    let (nr, ni) = (0.0, x / q);
    let (dr, di) = (1.0 - x * x, x / q);
    let d = dr * dr + di * di;
    ((nr * dr + ni * di) / d, (ni * dr - nr * di) / d)
}

/// The real amplitudes that fit `h` best with these modes (weighted least squares).
fn solve(f: &[f64], h: &[C], w: &[f64], modes: &[(f64, f64)]) -> Vec<f64> {
    let n = modes.len();
    let mut m = vec![0.0f64; n * n];
    let mut b = vec![0.0f64; n];
    let mut col = vec![(0.0, 0.0); n];
    for i in 0..f.len() {
        for (k, (hz, q)) in modes.iter().enumerate() {
            col[k] = basis(f[i], *hz, *q);
        }
        for k in 0..n {
            b[k] += w[i] * (col[k].0 * h[i].0 + col[k].1 * h[i].1);
            for l in k..n {
                m[k * n + l] += w[i] * (col[k].0 * col[l].0 + col[k].1 * col[l].1);
            }
        }
    }
    for k in 0..n {
        for l in 0..k {
            m[k * n + l] = m[l * n + k];
        }
    }
    // Non-negative least squares by projected Gauss–Seidel on the normal equations: a
    // driving point's modes each add a positive mass term (a passive bridge), and a fit
    // that lets two neighbours cancel would come apart the moment `body.ring` moved their
    // Qs differently.
    let mut x = vec![0.0f64; n];
    for _ in 0..4000 {
        let mut moved = 0.0f64;
        for k in 0..n {
            let d = m[k * n + k];
            if d <= 0.0 {
                continue;
            }
            let g: f64 = (0..n).map(|l| m[k * n + l] * x[l]).sum::<f64>() - b[k];
            let nx = (x[k] - g / d).max(0.0);
            moved = moved.max((nx - x[k]).abs() / (x[k].abs() + 1e-12));
            x[k] = nx;
        }
        if moved < 1e-10 {
            break;
        }
    }
    x
}

fn model(f: f64, modes: &[(f64, f64)], amp: &[f64]) -> C {
    let mut y = (0.0, 0.0);
    for ((hz, q), a) in modes.iter().zip(amp) {
        let b = basis(f, *hz, *q);
        y.0 += a * b.0;
        y.1 += a * b.1;
    }
    y
}

/// Band levels, dB, in twelfths of an octave from `LO` to 10 kHz.
fn bands(f: &[f64], mag2: &dyn Fn(usize) -> f64) -> Vec<f64> {
    let mut out = Vec::new();
    let mut lo = LO;
    while lo < 10_000.0 {
        let hi = lo * math::exp2(1.0 / 12.0);
        let (mut s, mut n) = (0.0, 0);
        for (i, fr) in f.iter().enumerate() {
            if *fr >= lo && *fr < hi {
                s += mag2(i);
                n += 1;
            }
        }
        out.push(10.0 * math::log10((s / n.max(1) as f64).max(1e-30)));
        lo = hi;
    }
    out
}

/// The root-mean-square difference of two band sets, dB, after the best gain between them.
fn distance(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len()) as f64;
    let off = a.iter().zip(b).map(|(x, y)| x - y).sum::<f64>() / n;
    (a.iter()
        .zip(b)
        .map(|(x, y)| math::powi(x - y - off, 2))
        .sum::<f64>()
        / n)
        .sqrt()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = Path::new(args.get(1).map_or("cnsm/admittances", |s| s.as_str()));
    let name = args.get(2).map_or("Klimke", |s| s.as_str());
    let out = args.get(3);
    let mut all = Vec::new();
    for v in ["Klimke", "Levaggi", "Stoppani"] {
        let (f, mut h) = violin(dir, v);
        undelay(&f, &mut h);
        all.push((v, f, h));
    }
    let (_, f_all, h_all) = all.iter().find(|(v, _, _)| *v == name).expect("violin");
    // The band, every point weighted by 1/f (each octave alike).
    let keep: Vec<usize> = (0..f_all.len())
        .filter(|&i| f_all[i] >= LO && f_all[i] <= HI)
        .collect();
    let f: Vec<f64> = keep.iter().map(|&i| f_all[i]).collect();
    let h: Vec<C> = keep.iter().map(|&i| h_all[i]).collect();
    let mag: Vec<f64> = h.iter().map(|x| (x.0 * x.0 + x.1 * x.1).sqrt()).collect();
    // Each point weighted so every octave counts alike and a quiet band as much as a loud
    // one (the ear's terms): 1/(f·|H|²) around a smoothed |H|.
    let smooth: Vec<f64> = (0..f.len())
        .map(|i| {
            let (a, b) = (i.saturating_sub(40), (i + 40).min(f.len()));
            mag[a..b].iter().map(|m| m * m).sum::<f64>() / (b - a) as f64
        })
        .collect();
    let w: Vec<f64> = (0..f.len()).map(|i| 1.0 / (f[i] * smooth[i])).collect();
    let target = bands(&f, &|i| mag[i] * mag[i]);
    // Greedy: each new mode where the residual is largest (in the weighted terms), its Q
    // from the measured peak's half-power width.
    let mut modes: Vec<(f64, f64)> = Vec::new();
    let mut amp: Vec<f64> = Vec::new();
    let mut tried: Vec<f64> = Vec::new();
    let mut picks = 0;
    while modes.len() < MODES && picks < 6 * MODES {
        picks += 1;
        // A mode adds a positive real part (a passive mode's), so the next goes where the
        // residual's real part is largest.
        let mut best = (0usize, -1.0f64);
        for i in 0..f.len() {
            let m = model(f[i], &modes, &amp);
            let r = math::powi((h[i].0 - m.0).max(0.0), 2) * w[i];
            let near = modes
                .iter()
                .any(|(hz, q)| (f[i] / hz - 1.0).abs() < (0.25 / q).clamp(0.008, 0.02))
                || tried.iter().any(|hz| (f[i] / hz - 1.0).abs() < 0.01);
            let edge = f[i] < 1.15 * LO || f[i] > HI / 1.1;
            if !near && !edge && r > best.1 {
                best = (i, r);
            }
        }
        if best.1 <= 0.0 {
            break;
        }
        let i = best.0;
        let half = mag[i] / 2f64.sqrt();
        let (mut a, mut b) = (i, i);
        while a > 0 && mag[a] > half && i - a < 400 {
            a -= 1;
        }
        while b + 1 < f.len() && mag[b] > half && b - i < 400 {
            b += 1;
        }
        let q = (f[i] / (f[b] - f[a]).max(1.0)).clamp(6.0, 90.0);
        modes.push((f[i], q));
        amp = solve(&f, &h, &w, &modes);
        // A mode the fit gives nothing is taken out again (and its place not tried twice).
        let mut k = 0;
        while k < modes.len() {
            if amp[k] <= 0.0 {
                tried.push(modes[k].0);
                modes.remove(k);
                amp.remove(k);
            } else {
                k += 1;
            }
        }
    }
    // Refine each mode's frequency and Q by a local search on the band levels' distance.
    let err = |modes: &[(f64, f64)]| -> (f64, Vec<f64>) {
        let amp = solve(&f, &h, &w, modes);
        let got = bands(&f, &|i| {
            let m = model(f[i], modes, &amp);
            m.0 * m.0 + m.1 * m.1
        });
        (distance(&target, &got), amp)
    };
    let (mut e, _) = err(&modes);
    eprintln!(
        "  greedy: {} modes, {e:.2} dB from the measured (1/12 oct)",
        modes.len()
    );
    for sweep in 0..4 {
        for k in 0..modes.len() {
            for (dh, dq) in [(1.006, 1.0), (1.0 / 1.006, 1.0), (1.0, 1.25), (1.0, 0.8)] {
                let mut trial = modes.clone();
                trial[k] = (
                    (trial[k].0 * dh).clamp(LO, HI),
                    (trial[k].1 * dq).clamp(4.0, 150.0),
                );
                let (te, _) = err(&trial);
                if te < e {
                    e = te;
                    modes = trial;
                }
            }
        }
        eprintln!("  sweep {sweep}: {e:.2} dB");
    }
    let (e, amp) = err(&modes);
    // Beside the spread between the three violins.
    let mut report = String::new();
    for (v, fv, hv) in &all {
        let b = bands(fv, &|i| hv[i].0 * hv[i].0 + hv[i].1 * hv[i].1);
        let _ = writeln!(
            report,
            "  {name} measured vs {v} measured: {:.2} dB",
            distance(&target, &b)
        );
    }
    eprintln!("fit vs {name} measured: {e:.2} dB (1/12 oct, 120 Hz – 10 kHz)\n{report}");
    let peak = amp.iter().fold(0.0f64, |m, a| m.max(a.abs()));
    let mut order: Vec<usize> = (0..modes.len()).collect();
    order.sort_by(|&a, &b| modes[a].0.total_cmp(&modes[b].0));
    let mut s = String::new();
    let _ = writeln!(
        s,
        "//! Arcus's violin body — **generated** by `tools/arcus/upper-body.sh`\n\
         //! (`examples/arcus_upper_body.rs`); do not edit by hand. {} modes fitted to the bridge\n\
         //! admittance of the CNSM dataset's violin \"{name}\" (Pauget Ballesteros, Lalitte, Lostanlen,\n\
         //! Fritz; Zenodo 10.5281/zenodo.18696786, CC BY 4.0), its measurements averaged and the\n\
         //! acquisition's delay taken out; {e:.2} dB from it in twelfths of an octave, 120 Hz – 10 kHz.\n",
        modes.len()
    );
    let _ = writeln!(s, "use super::Mode;\n");
    let _ = writeln!(s, "pub const VIOLIN: [Mode; {}] = [", modes.len());
    for k in order {
        let _ = writeln!(
            s,
            "    Mode {{ hz: {:.2}, q: {:.2}, a: {:.5} }},",
            modes[k].0,
            modes[k].1,
            amp[k] / peak
        );
    }
    let _ = writeln!(s, "];");
    let _ = PI;
    match out {
        Some(p) => {
            std::fs::write(p, s).expect("write");
            eprintln!("wrote {p}");
        }
        None => print!("{s}"),
    }
}
