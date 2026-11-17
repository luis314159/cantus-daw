//! The CPU at 48 kHz: a minute of stereo noise in blocks of 512, best of three runs per
//! case, as a share of one core.

use cantus_dsp::comparatio::reader::{RefAudio, Sync};
use cantus_dsp::comparatio::tonal::{Resolution, Tonal};
use cantus_dsp::comparatio::{Comparatio, ComparatioSettings, Listen};
use cantus_dsp::modfx_clock::Transport;
use std::time::Instant;

const SR: f32 = 48_000.0;
const SECS: usize = 60;
const BLOCK: usize = 512;

pub struct Cpu {
    /// (case, % of one core).
    pub cases: Vec<(&'static str, f64)>,
    /// The tonal worker: % of one core for two sources, and its longest single call, ms.
    pub tonal: f64,
    pub tonal_worst_ms: f64,
}

fn noise(n: usize, seed: u32) -> Vec<f32> {
    let mut s = seed.max(1);
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            (s as f32 / u32::MAX as f32) * 0.5 - 0.25
        })
        .collect()
}

fn run(listen: Listen, with_ref: bool, silent: bool) -> f64 {
    let n = SECS * SR as usize;
    let (ml, mr) = if silent {
        (vec![0.0; n], vec![0.0; n])
    } else {
        (noise(n, 1), noise(n, 2))
    };
    let (rl, rr) = (noise(n + SR as usize, 3), noise(n + SR as usize, 4));
    let mut best = f64::INFINITY;
    for _ in 0..3 {
        let mut u = Comparatio::new(SR);
        u.set(&ComparatioSettings {
            listen,
            sync: Sync::Locked { offset: 0 },
            ..ComparatioSettings::default()
        });
        let (mut l, mut r) = (vec![0.0f32; BLOCK], vec![0.0f32; BLOCK]);
        let t0 = Instant::now();
        for (k, (a, b)) in ml.chunks(BLOCK).zip(mr.chunks(BLOCK)).enumerate() {
            let m = a.len();
            l[..m].copy_from_slice(a);
            r[..m].copy_from_slice(b);
            u.set_transport(&Transport {
                playing: !silent,
                tempo_bpm: 120.0,
                samples: (k * BLOCK) as i64,
            });
            let audio = with_ref.then_some(RefAudio {
                left: &rl,
                right: &rr,
                id: 1,
            });
            if silent && u.at_rest() {
                u.skip(m);
            } else {
                u.process(&mut l[..m], &mut r[..m], audio);
            }
            if k % 9 == 0 {
                std::hint::black_box(u.meter());
            }
        }
        std::hint::black_box(&l);
        best = best.min(t0.elapsed().as_secs_f64());
    }
    best / SECS as f64 * 100.0
}

fn tonal() -> (f64, f64) {
    let n = SECS * SR as usize;
    let (l, r) = (noise(n, 5), noise(n, 6));
    let mut best = f64::INFINITY;
    let mut worst_call = 0f64;
    for _ in 0..3 {
        let mut a = Tonal::new(SR, Resolution::Sixth);
        let mut b = Tonal::new(SR, Resolution::Sixth);
        let t0 = Instant::now();
        for (x, y) in l.chunks(BLOCK).zip(r.chunks(BLOCK)) {
            let c = Instant::now();
            a.push(x, y);
            b.push(y, x);
            worst_call = worst_call.max(c.elapsed().as_secs_f64());
        }
        std::hint::black_box(a.curve());
        std::hint::black_box(b.curve());
        best = best.min(t0.elapsed().as_secs_f64());
    }
    (best / SECS as f64 * 100.0, worst_call * 1_000.0)
}

pub fn measure() -> Cpu {
    let cases = vec![
        (
            "the reference heard (read, analysed, matched)",
            run(Listen::Reference, true, false),
        ),
        (
            "the mix heard, the reference analysed beside it",
            run(Listen::Mix, true, false),
        ),
        (
            "the mix alone (no reference loaded)",
            run(Listen::Mix, false, false),
        ),
        (
            "at rest (silence skipped, the song stopped)",
            run(Listen::Mix, true, true),
        ),
    ];
    let (tonal, tonal_worst_ms) = tonal();
    Cpu {
        cases,
        tonal,
        tonal_worst_ms,
    }
}
