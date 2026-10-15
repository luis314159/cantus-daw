//! Measure a DrumGizmo kit's microphones, to choose a stereo mix for its pack (round 28).
//!
//! `cargo run -p cantus-content --example drumkit_probe -- <kit folder> <kit.xml> [instrument…]`
//!
//! For each instrument (all, or those named): its hits, and for its loudest hit each
//! microphone's peak and the RMS of its first 50 ms (dBFS); then, for every pair of
//! microphones the kit has, how alike they are over the hit's first 20 ms — the
//! correlation at the lag (within ±3 ms) where it is largest in size, and that lag — so a
//! microphone that faces another (a snare's bottom, a kick's inside) shows its polarity.

use cantus_content::{drumgizmo, wav};
use std::path::Path;

fn db(x: f64) -> f64 {
    20.0 * (x.max(1e-12)).log10()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = Path::new(&args[0]);
    let kit = drumgizmo::read_kit(root, &args[1]).expect("a kit");
    let only: Vec<&String> = args[2..].iter().collect();
    for inst in &kit.instruments {
        if !only.is_empty() && !only.contains(&&inst.name) {
            continue;
        }
        let s = inst
            .samples
            .iter()
            .max_by(|a, b| a.power.total_cmp(&b.power))
            .unwrap();
        let file = &s.audio[0].file;
        let a = wav::read(&drumgizmo::path_of(root, file)).unwrap();
        let rate = a.rate as usize;
        println!(
            "\n{} — {} hits, loudest {} ({:.2} s, {} Hz)",
            inst.name,
            inst.samples.len(),
            s.name,
            a.frames() as f64 / rate as f64,
            rate
        );
        let ch = |r: &drumgizmo::AudioRef| -> Vec<f64> {
            (0..a.frames())
                .map(|i| a.samples[i * a.channels + r.file_channel] as f64)
                .collect()
        };
        let mut chans = Vec::new();
        for r in &s.audio {
            let x = ch(r);
            let peak = x.iter().fold(0f64, |p, v| p.max(v.abs()));
            let n = (rate / 20).min(x.len());
            let rms = (x[..n].iter().map(|v| v * v).sum::<f64>() / n as f64).sqrt();
            println!(
                "  {:>13}  peak {:7.1}  rms50 {:7.1}",
                r.channel,
                db(peak),
                db(rms)
            );
            chans.push((r.channel.clone(), x));
        }
        // Polarity: each pair at equal level over the first 30 ms, summed and differenced —
        // positive dB means they add (in polarity), negative that they cancel.
        let m = rate * 3 / 100;
        for (p, q) in [
            ("Snare_top", "Snare_bottom"),
            ("Kdrum_front", "Kdrum_back"),
            ("Snare_top", "OHL"),
            ("Snare_top", "OHR"),
            ("Kdrum_front", "OHL"),
            ("Kdrum_back", "OHL"),
            ("Tom1", "OHL"),
            ("Tom2", "OHR"),
            ("Tom3", "OHR"),
            ("OHL", "OHR"),
            ("AmbL", "AmbR"),
        ] {
            let get = |c: &str| chans.iter().find(|(n, _)| n == c).map(|(_, x)| x);
            let (Some(x), Some(y)) = (get(p), get(q)) else {
                continue;
            };
            let m = m.min(x.len()).min(y.len());
            let e = |v: &[f64]| v[..m].iter().map(|a| a * a).sum::<f64>().sqrt();
            let (ex, ey) = (e(x), e(y));
            if ex < 1e-9 || ey < 1e-9 {
                continue;
            }
            let (mut sum, mut diff) = (0.0, 0.0);
            for k in 0..m {
                let (a, b) = (x[k] / ex, y[k] / ey);
                sum += (a + b) * (a + b);
                diff += (a - b) * (a - b);
            }
            println!("    {p} + {q}: {:+.1} dB", 10.0 * (sum / diff).log10());
        }
        let n = rate / 50;
        let maxlag = (rate * 3 / 1000) as i64;
        for i in 0..chans.len() {
            for j in i + 1..chans.len() {
                let (x, y) = (&chans[i].1, &chans[j].1);
                let e = |v: &[f64]| {
                    v[..n.min(v.len())]
                        .iter()
                        .map(|a| a * a)
                        .sum::<f64>()
                        .sqrt()
                };
                let (ex, ey) = (e(x), e(y));
                if ex < 1e-6 || ey < 1e-6 {
                    continue;
                }
                let mut best = (0.0f64, 0i64);
                for lag in -maxlag..=maxlag {
                    let mut c = 0.0;
                    for k in 0..n {
                        let m = k as i64 + lag;
                        if m >= 0 && (m as usize) < y.len() && k < x.len() {
                            c += x[k] * y[m as usize];
                        }
                    }
                    let c = c / (ex * ey);
                    if c.abs() > best.0.abs() {
                        best = (c, lag);
                    }
                }
                if best.0.abs() > 0.5 {
                    println!(
                        "    {} ~ {}: {:+.2} at {:+.2} ms",
                        chans[i].0,
                        chans[j].0,
                        best.0,
                        best.1 as f64 * 1000.0 / rate as f64
                    );
                }
            }
        }
    }
}
