use super::*;
use crate::aula_kernel::{Op, Schedule};

pub(crate) struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1)
    }
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    /// −1 … 1.
    pub fn f(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 23) as f32 - 1.0
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// A decaying random response of `taps` taps.
fn response(rng: &mut Rng, taps: usize) -> Vec<f32> {
    (0..taps)
        .map(|i| rng.f() * crate::math::exp(-3.0 * i as f32 / taps.max(1) as f32))
        .collect()
}

fn signal(rng: &mut Rng, n: usize) -> (Vec<f32>, Vec<f32>) {
    // Bursts of noise, a click and silence between (sparse, so the reference is quick).
    let mut l = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    let bursts = [(0, 1_000.min(n)), (2 * n / 3, (2 * n / 3 + 500).min(n))];
    for (a, b) in bursts {
        for i in a..b {
            l[i] = 0.5 * rng.f();
            r[i] = 0.5 * rng.f();
        }
    }
    l[n / 2] = 1.0;
    (l, r)
}

/// The reference: a direct convolution in f64.
fn direct(x: &[f32], h: &[f32]) -> Vec<f64> {
    let mut y = vec![0.0f64; x.len()];
    for (n, xn) in x.iter().enumerate() {
        if *xn == 0.0 {
            continue;
        }
        for (j, hj) in h.iter().enumerate().take(x.len() - n) {
            y[n + j] += *hj as f64 * *xn as f64;
        }
    }
    y
}

fn reference(format: Format, hs: &[Vec<f32>], l: &[f32], r: &[f32]) -> (Vec<f64>, Vec<f64>) {
    let add = |a: Vec<f64>, b: Vec<f64>| a.iter().zip(&b).map(|(x, y)| x + y).collect();
    match format {
        Format::Mono => (direct(l, &hs[0]), direct(r, &hs[0])),
        Format::Stereo => (direct(l, &hs[0]), direct(r, &hs[1])),
        Format::TrueStereo => (
            add(direct(l, &hs[0]), direct(r, &hs[2])),
            add(direct(l, &hs[1]), direct(r, &hs[3])),
        ),
    }
}

fn convolver(format: Format, hs: &[Vec<f32>], cap_taps: usize) -> Convolver {
    let cap = Layout::for_taps(cap_taps);
    Convolver::new(
        Box::new(State::new(cap)),
        Arc::new(Kernel::new(format, hs, cap)),
    )
}

fn drive(
    c: &mut Convolver,
    l: &[f32],
    r: &[f32],
    mut block: impl FnMut() -> usize,
) -> (Vec<f32>, Vec<f32>) {
    let (mut ol, mut or) = (l.to_vec(), r.to_vec());
    let mut i = 0;
    while i < ol.len() {
        let n = block().clamp(1, ol.len() - i);
        c.process(&mut ol[i..i + n], &mut or[i..i + n]);
        i += n;
    }
    (ol, or)
}

/// The largest error against the reference, relative to the reference's peak.
fn error(got: &(Vec<f32>, Vec<f32>), want: &(Vec<f64>, Vec<f64>)) -> f64 {
    let peak = want
        .0
        .iter()
        .chain(&want.1)
        .fold(0.0f64, |m, v| m.max(v.abs()))
        .max(1e-12);
    let e = |g: &[f32], w: &[f64]| {
        g.iter()
            .zip(w)
            .fold(0.0f64, |m, (a, b)| m.max((*a as f64 - b).abs()))
    };
    e(&got.0, &want.0).max(e(&got.1, &want.1)) / peak
}

#[test]
fn it_is_a_direct_convolution_for_every_length_and_format() {
    let mut rng = Rng::new(7);
    let mut worst = 0.0f64;
    for taps in [1, 63, 64, 65, 200, 1_023, 1_024, 1_025, 9_000, 70_000] {
        for format in [Format::Mono, Format::Stereo, Format::TrueStereo] {
            let hs: Vec<Vec<f32>> = (0..format.paths())
                .map(|_| response(&mut rng, taps))
                .collect();
            let n = taps + 6_000;
            let (l, r) = signal(&mut rng, n);
            let want = reference(format, &hs, &l, &r);
            let mut c = convolver(format, &hs, taps);
            let got = drive(&mut c, &l, &r, || 256);
            let e = error(&got, &want);
            worst = worst.max(e);
            assert!(e < 2e-5, "{taps} taps, {format:?}: error {e:e} of the peak");
        }
    }
    eprintln!("worst error against direct convolution: {worst:e} of the peak");
}

#[test]
#[allow(clippy::type_complexity)]
fn any_split_of_the_blocks_gives_the_same_bits() {
    let mut rng = Rng::new(11);
    for (taps, format) in [
        (900, Format::Stereo),
        (10_000, Format::TrueStereo),
        (80_000, Format::Mono),
    ] {
        let hs: Vec<Vec<f32>> = (0..format.paths())
            .map(|_| response(&mut rng, taps))
            .collect();
        let (l, r) = signal(&mut rng, taps + 70_000);
        let reference = drive(&mut convolver(format, &hs, taps), &l, &r, || 512);
        let mut split = Rng::new(taps as u64);
        let runs: [(&str, Box<dyn FnMut() -> usize>); 4] = [
            ("1", Box::new(|| 1)),
            ("7", Box::new(|| 7)),
            ("4096", Box::new(|| 4096)),
            ("random", Box::new(move || 1 + split.below(3000))),
        ];
        for (name, block) in runs {
            let got = drive(&mut convolver(format, &hs, taps), &l, &r, block);
            let same =
                |a: &[f32], b: &[f32]| a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits());
            assert!(
                same(&got.0, &reference.0) && same(&got.1, &reference.1),
                "{taps} taps {format:?}: blocks of {name} differ"
            );
        }
    }
}

#[test]
fn a_swapped_kernel_keeps_the_tail_and_becomes_the_new_convolution() {
    let mut rng = Rng::new(13);
    let taps = 40_000;
    let cap = Layout::for_taps(taps);
    let a: Vec<Vec<f32>> = (0..2).map(|_| response(&mut rng, taps)).collect();
    let b: Vec<Vec<f32>> = (0..2).map(|_| response(&mut rng, taps / 2)).collect();
    let (l, r) = signal(&mut rng, 3 * taps);
    let mut c = Convolver::new(
        Box::new(State::new(cap)),
        Arc::new(Kernel::new(Format::Stereo, &a, cap)),
    );
    let at = taps + 1_234;
    let (mut ol, mut or) = (l.clone(), r.clone());
    c.process(&mut ol[..at], &mut or[..at]);
    assert!(c
        .supply(Arc::new(Kernel::new(Format::Stereo, &b, cap)), None)
        .is_ok());
    assert!(c.swapping());
    c.process(&mut ol[at..], &mut or[at..]);
    assert!(
        !c.swapping(),
        "the swap is through within a period of the largest level"
    );
    // Once every level runs the new response, the output is the new response's
    // convolution of the whole input, history included.
    let want = reference(Format::Stereo, &b, &l, &r);
    let from = at + 2 * cap.top() + B;
    let got = (ol[from..].to_vec(), or[from..].to_vec());
    let want = (want.0[from..].to_vec(), want.1[from..].to_vec());
    let e = error(&got, &want);
    assert!(e < 2e-5, "after the swap: error {e:e}");
}

#[test]
fn it_rests_after_its_settling_time_and_rests_exactly() {
    let mut rng = Rng::new(17);
    for taps in [10, 500, 5_000, 70_000] {
        let cap = Layout::for_taps(taps);
        assert!(
            cap.settle() >= cap.ring(),
            "the rings are clear by the time it rests"
        );
        let hs: Vec<Vec<f32>> = (0..4).map(|_| response(&mut rng, taps)).collect();
        let mut c = convolver(Format::TrueStereo, &hs, taps);
        assert!(c.at_rest(), "a new convolver is at rest");
        let (mut l, mut r) = signal(&mut rng, 3_000);
        l[2_999] = 0.25;
        c.process(&mut l, &mut r);
        assert!(!c.at_rest());
        let mut silent = 0;
        while !c.at_rest() {
            let (mut l, mut r) = (vec![0.0; 100], vec![0.0; 100]);
            c.process(&mut l, &mut r);
            silent += 100;
        }
        assert!(silent <= cap.settle() + 100);
        let (mut l, mut r) = (vec![0.0; 50_000], vec![0.0; 50_000]);
        c.process(&mut l, &mut r);
        assert!(
            l.iter().chain(&r).all(|v| v.to_bits() == 0),
            "silence at rest is +0.0"
        );
        // As good as new: a new one skipped as far gives the same bits from here on.
        let mut fresh = convolver(Format::TrueStereo, &hs, taps);
        fresh.skip(3_000 + silent + 50_000);
        let (x, y) = signal(&mut rng, taps + 9_000);
        let a = drive(&mut c, &x, &y, || 333);
        let b = drive(&mut fresh, &x, &y, || 64);
        assert!(
            a.0.iter()
                .zip(&b.0)
                .all(|(p, q)| p.to_bits() == q.to_bits()),
            "{taps}"
        );
        assert!(
            a.1.iter()
                .zip(&b.1)
                .all(|(p, q)| p.to_bits() == q.to_bits()),
            "{taps}"
        );
    }
}

#[test]
#[allow(clippy::needless_range_loop)]
fn each_levels_work_is_spread_evenly_and_done_once() {
    for k in 0..LEVELS {
        let most_parts = if k == LEVELS - 1 { 115 } else { 15 };
        for parts in [0, 1, most_parts] {
            let s = Schedule::new(k, parts, Format::TrueStereo);
            let ticks = s.starts.len() - 1;
            assert_eq!(ticks, if k == 0 { 1 } else { SIZES[k] / B });
            assert_eq!(s.starts[ticks] as usize, s.units.len(), "every unit once");
            assert!(s.starts.windows(2).all(|w| w[0] <= w[1]));
            // No tick holds more than about twice its share, plus one piece (in the
            // schedule's own weights: a product costs four times a copy in true stereo).
            let cost = |u: &Unit| {
                let w = match u.op {
                    Op::Gather | Op::Pack | Op::Extract => 1,
                    Op::Fwd(_) | Op::Inv(_) | Op::Unpack => 2,
                    Op::Mac(_) => 8,
                };
                (u.to - u.from) as usize * w
            };
            let total: usize = s.units.iter().map(cost).sum();
            let most = (0..ticks)
                .map(|j| s.tick(j).iter().map(cost).sum::<usize>())
                .max()
                .unwrap();
            assert!(
                most <= 2 * total / ticks + 2 * 1_024 * 8,
                "level {k}, {parts} parts: {most} of {total}"
            );
        }
    }
}

#[test]
fn layouts_cover_the_response() {
    for taps in [0, 1, 64, 65, 1_024, 1_025, 8_192, 65_536, 65_537, 960_000] {
        let l = Layout::for_taps(taps);
        assert!(l.taps() >= taps, "{taps}");
        assert!(Layout::for_taps(l.taps()) == l);
        assert!(l.holds(&Layout::for_taps(taps / 2)));
    }
    assert_eq!(Layout::for_taps(1_024).parts, [15, 0, 0, 0]);
    assert_eq!(Layout::for_taps(65_536).parts, [15, 14, 14, 0]);
}
