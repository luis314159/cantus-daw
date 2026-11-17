//! Vinculum's measurements: the valve colour's aliasing, the Auto release on a drum loop,
//! and (ignored, run in release) CPU per model.

use super::super::*;
use super::{goertzel, programme, run, unit, SR};
use crate::pulsus::{DrumVoice, PadSettings, Pulsus};

/// A drum loop from our own drum instrument (Pulsus, model 2): 100 BPM, kick, snare and
/// eighth-note hats, `bars` bars, stereo, peaking near −6 dBFS.
pub(super) fn drum_loop(bars: usize) -> (Vec<f32>, Vec<f32>) {
    let mut p = Pulsus::new(SR);
    p.set_kit(2, 7, 4, false);
    p.set_pad_count(3);
    let pads = [
        (36u8, PadSettings::of(DrumVoice::Kick, 2)),
        (38, PadSettings::of(DrumVoice::Snare, 2)),
        (42, PadSettings::of(DrumVoice::HiHat, 2)),
    ];
    for (i, (pitch, s)) in pads.iter().enumerate() {
        let s = PadSettings {
            id: i as u16 + 1,
            ..*s
        };
        p.set_pad(i, *pitch, &s);
    }
    let step = (SR * 60.0 / 100.0 / 4.0) as usize;
    let mut events = Vec::new();
    for bar in 0..bars {
        for k in 0..16 {
            let at = (bar * 16 + k) * step;
            if [0, 7, 8, 14].contains(&k) {
                events.push((at, 36u8, if k == 0 { 120u8 } else { 100 }));
            }
            if [4, 12].contains(&k) {
                events.push((at, 38, 115));
            }
            if k % 2 == 0 {
                events.push((at, 42, if k % 4 == 0 { 90 } else { 64 }));
            }
        }
    }
    let n = bars * 16 * step;
    let (mut l, mut r) = (vec![0f32; n], vec![0f32; n]);
    let mut pos = 0;
    let mut tag = 1;
    for (at, pitch, vel) in events.iter().copied().chain([(n, 0, 0)]) {
        while pos < at {
            let end = (pos + 256).min(at);
            p.process_add(&mut l[pos..end], &mut r[pos..end]);
            pos = end;
        }
        if pitch > 0 {
            p.note_on(pitch, vel, tag);
            tag += 1;
        }
    }
    let peak = l.iter().chain(&r).fold(0f32, |m, x| m.max(x.abs()));
    let g = 0.5 / peak.max(1e-9);
    for x in l.iter_mut().chain(r.iter_mut()) {
        *x *= g;
    }
    (l, r)
}

/// The reduction sample by sample.
fn gr_trace(s: &VinculumSettings, l: &[f32], r: &[f32]) -> Vec<f32> {
    let mut v = unit(s);
    let mut out = Vec::with_capacity(l.len());
    let (mut a, mut b) = ([0f32; 1], [0f32; 1]);
    for i in 0..l.len() {
        a[0] = l[i];
        b[0] = r[i];
        v.process(&mut a, &mut b);
        out.push(v.gain_reduction_db());
    }
    out
}

#[test]
fn the_auto_release_on_a_drum_loop() {
    let (l, r) = drum_loop(4);
    let base = VinculumSettings {
        threshold_db: -18.0,
        ratio: 4.0,
        attack_ms: 10.0,
        ..VinculumSettings::default()
    };
    let step = (SR * 60.0 / 100.0 / 4.0) as usize;
    // Skip the first bar (the slow stage charging); read the rest.
    let from = 16 * step;
    let mut rows = Vec::new();
    for (name, release, auto) in [
        ("0.1 s", 100.0, false),
        ("0.3 s", 300.0, false),
        ("1.2 s", 1200.0, false),
        ("Auto", 300.0, true),
    ] {
        let s = VinculumSettings {
            release_ms: release,
            auto_release: auto,
            ..base
        };
        let t = gr_trace(&s, &l, &r);
        let tail = &t[from..];
        let mean = tail.iter().map(|g| *g as f64).sum::<f64>() / tail.len() as f64;
        let var = tail
            .iter()
            .map(|g| math::powi(*g as f64 - mean, 2))
            .sum::<f64>()
            / tail.len() as f64;
        // What is left just before each downbeat's kick: how far it came back.
        let mut before = Vec::new();
        let mut k = from + 16 * step;
        while k < t.len() {
            before.push(t[k - 1] as f64);
            k += 16 * step;
        }
        let left = before.iter().sum::<f64>() / before.len().max(1) as f64;
        let peak = tail.iter().fold(0f32, |m, g| m.max(*g));
        println!(
            "release {name:>5}: mean {mean:.2} dB, swing (σ) {:.2} dB, peak {peak:.1} dB, left before the downbeat {left:.2} dB",
            var.sqrt()
        );
        rows.push((mean, var.sqrt(), left));
    }
    let (fast, slow, auto) = (rows[0], rows[2], rows[3]);
    // Auto pumps less than the fastest fixed release and lets go more than the slowest.
    assert!(
        auto.1 < fast.1,
        "auto swings {} dB, 0.1 s {} dB",
        auto.1,
        fast.1
    );
    assert!(
        auto.2 < slow.2,
        "auto holds {} dB, 1.2 s {} dB",
        auto.2,
        slow.2
    );
}

#[test]
fn the_valve_colour_barely_aliases() {
    println!("valve colour, 0 dBFS sine, full depth: the third's alias at the base rate");
    let n = 1 << 15;
    for f0 in [9_000.0f64, 10_000.0, 11_000.0, 13_000.0, 15_000.0] {
        let three = 3.0 * f0;
        let alias = if three < 48_000.0 {
            48_000.0 - three
        } else {
            three - 48_000.0
        };
        let x: Vec<f32> = (0..n)
            .map(|i| math::sin(2.0 * std::f64::consts::PI * f0 * i as f64 / SR as f64) as f32)
            .collect();
        let mut valve = valve::Valve::new();
        let os: Vec<f32> = x.iter().map(|v| valve.run(0, *v)).collect();
        let naive: Vec<f32> = x.iter().map(|v| valve::naive(*v)).collect();
        let a_os = 20.0 * math::log10(goertzel(&os[n / 4..], alias).max(1e-12));
        let a_naive = 20.0 * math::log10(goertzel(&naive[n / 4..], alias).max(1e-12));
        println!("  {f0:>6.0} Hz → alias at {alias:>6.0} Hz: naive {a_naive:6.1} dB, at 2× {a_os:6.1} dB");
        assert!(a_naive > -45.0, "the naive curve should alias: {a_naive}");
        assert!(a_os < -90.0, "{f0} Hz: {a_os} dB");
    }
}

#[test]
#[ignore]
fn cpu_per_model() {
    let (l, r) = programme(10.0, 1.5);
    let configs = [
        ("VCA", hard_of(Model::Vca, false)),
        ("VCA Auto", hard_of(Model::Vca, true)),
        ("Opto", hard_of(Model::Opto, false)),
        (
            "Opto limit",
            VinculumSettings {
                character: Character::Limit,
                ..hard_of(Model::Opto, false)
            },
        ),
        (
            "Vari-mu, no colour",
            VinculumSettings {
                colour: 0.0,
                ..hard_of(Model::VariMu, false)
            },
        ),
        ("Vari-mu, colour", hard_of(Model::VariMu, false)),
        (
            "Vari-mu limit, colour",
            VinculumSettings {
                character: Character::Limit,
                ..hard_of(Model::VariMu, false)
            },
        ),
    ];
    for (name, s) in configs {
        let mut best = f64::MAX;
        for _ in 0..3 {
            let mut v = unit(&s);
            let t = std::time::Instant::now();
            let out = run(&mut v, &l, &r, 256);
            best = best.min(t.elapsed().as_secs_f64());
            std::hint::black_box(out);
        }
        println!(
            "{name:>22}: {:.3} % of a core (stereo, 48 kHz)",
            best / 10.0 * 100.0
        );
    }
}

fn hard_of(model: Model, auto: bool) -> VinculumSettings {
    VinculumSettings {
        auto_release: auto,
        sc_hpf_hz: 100.0,
        mix: 0.7,
        ..super::hard(model)
    }
}
