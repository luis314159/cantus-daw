use super::*;
use crate::aula_gen::{factory, FACTORY};
use crate::aula_kernel::Format;
use crate::aula_shape::{overview, resample, GATE_OFF};

const SR: f32 = 48_000.0;

fn noise(n: usize, seed: u64) -> Vec<f32> {
    let mut s = seed | 1;
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            ((s >> 40) as f32 / (1u64 << 23) as f32 - 1.0) * 0.5
        })
        .collect()
}

fn aula_of(ir: &Ir) -> Aula {
    let (_, k) = prepare(ir, SR, &Shape::default(), None);
    Aula::with_kernel(SR, Arc::new(k))
}

fn raw(format: Format, channels: Vec<Vec<f32>>) -> Aula {
    let cap = capacity_for(channels.iter().map(|c| c.len()).max().unwrap_or(0));
    Aula::with_kernel(SR, Arc::new(Kernel::new(format, &channels, cap)))
}

#[test]
fn at_mix_zero_the_dry_passes_to_the_bit_whatever_else_is_set() {
    let mut a = aula_of(&factory(7, SR));
    a.set(&AulaSettings {
        predelay_ms: 120.0,
        width: 0.3,
        mix: 0.0,
        level_db: 0.0,
    });
    let (l0, r0) = (noise(20_000, 3), noise(20_000, 5));
    let (mut l, mut r) = (l0.clone(), r0.clone());
    for (cl, cr) in l.chunks_mut(333).zip(r.chunks_mut(333)) {
        a.process(cl, cr);
    }
    assert!(l.iter().zip(&l0).all(|(x, y)| x.to_bits() == y.to_bits()));
    assert!(r.iter().zip(&r0).all(|(x, y)| x.to_bits() == y.to_bits()));
}

#[test]
fn a_unit_impulse_at_full_mix_is_a_wire_and_the_predelay_delays_it_exactly() {
    let mut a = raw(Format::Mono, vec![vec![1.0]]);
    a.set(&AulaSettings {
        mix: 1.0,
        ..AulaSettings::default()
    });
    a.mix.jump();
    let x = noise(5_000, 9);
    let (mut l, mut r) = (x.clone(), x.clone());
    a.process(&mut l, &mut r);
    assert_eq!(l, x);
    assert_eq!(r, x);
    let mut a = raw(Format::Mono, vec![vec![1.0]]);
    a.set(&AulaSettings {
        mix: 1.0,
        predelay_ms: 10.0,
        ..AulaSettings::default()
    });
    a.mix.jump();
    a.fade = 0;
    a.delay = a.fade_to;
    let (mut l, mut r) = (x.clone(), x.clone());
    a.process(&mut l, &mut r);
    assert_eq!(&l[480..], &x[..x.len() - 480]);
    assert!(l[..480].iter().all(|v| *v == 0.0));
}

#[test]
fn a_room_rings_after_a_click_and_comes_to_rest() {
    for i in [0, 7, 10, 13] {
        let mut a = aula_of(&factory(i, SR));
        a.set(&AulaSettings {
            mix: 1.0,
            ..AulaSettings::default()
        });
        let n = 4 * SR as usize;
        let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
        l[0] = 1.0;
        r[0] = 1.0;
        for (cl, cr) in l.chunks_mut(512).zip(r.chunks_mut(512)) {
            a.process(cl, cr);
        }
        assert!(l.iter().chain(&r).all(|v| v.is_finite()));
        let tail: f64 = l[4_800..].iter().map(|v| (*v as f64) * (*v as f64)).sum();
        assert!(tail > 1e-6, "{} has no tail", FACTORY[i].key);
        let mut silent = 0;
        while !a.at_rest() && silent < 30 * SR as usize {
            let (mut l, mut r) = (vec![0.0; 4096], vec![0.0; 4096]);
            a.process(&mut l, &mut r);
            silent += 4096;
        }
        assert!(
            a.at_rest(),
            "{} never rests: {:?}",
            FACTORY[i].key,
            a.rest_blocker()
        );
    }
}

#[test]
fn the_whole_effect_is_the_same_bits_however_the_blocks_are_cut() {
    let ir = factory(8, SR);
    let x = (noise(100_000, 1), noise(100_000, 2));
    let run = |block: &dyn Fn(usize) -> usize| {
        let mut a = aula_of(&ir);
        let (mut l, mut r) = (x.0.clone(), x.1.clone());
        let mut i = 0;
        while i < l.len() {
            // The settings move at fixed places, whatever the blocks.
            let n = block(i).min(l.len() - i);
            let n = if i < 30_000 { n.min(30_000 - i) } else { n };
            if i == 0 || i == 30_000 {
                a.set(&AulaSettings {
                    predelay_ms: if i == 0 { 20.0 } else { 75.0 },
                    width: if i == 0 { 1.0 } else { 0.4 },
                    mix: if i == 0 { 0.5 } else { 0.8 },
                    level_db: if i == 0 { 0.0 } else { -3.0 },
                });
            }
            a.process(&mut l[i..i + n], &mut r[i..i + n]);
            i += n;
        }
        (l, r)
    };
    let reference = run(&|_| 512);
    for (name, f) in [
        ("1", &(|_: usize| 1usize) as &dyn Fn(usize) -> usize),
        ("7", &|_| 7),
        ("4096", &|_| 4096),
        ("varied", &|i| 1 + (i * 7919) % 1500),
    ] {
        let got = run(f);
        let same = |a: &[f32], b: &[f32]| a.iter().zip(b).all(|(p, q)| p.to_bits() == q.to_bits());
        assert!(
            same(&got.0, &reference.0) && same(&got.1, &reference.1),
            "blocks of {name}"
        );
    }
}

#[test]
fn the_shape_controls_do_what_they_say() {
    let ir = Ir::from_channels(SR, vec![noise(48_000, 4)]);
    let len = |s: Shape| shape(&ir, SR, &s, false).len();
    assert_eq!(
        shape(&ir, SR, &Shape::default(), false),
        ir,
        "the default leaves it be"
    );
    assert_eq!(
        len(Shape {
            stretch: 2.0,
            ..Shape::default()
        }),
        96_000
    );
    assert_eq!(
        len(Shape {
            stretch: 0.5,
            ..Shape::default()
        }),
        24_000
    );
    assert_eq!(
        len(Shape {
            length: 0.25,
            ..Shape::default()
        }),
        12_000
    );
    let rev = shape(
        &ir,
        SR,
        &Shape {
            reverse: true,
            ..Shape::default()
        },
        false,
    );
    assert_eq!(rev.channels[0][0], ir.channels[0][47_999]);
    // A response with a noise floor: the gate takes it away.
    let mut floor = noise(48_000, 6);
    for (i, v) in floor.iter_mut().enumerate() {
        *v *= if i < 12_000 { 1.0 } else { 1e-4 };
    }
    let g = Ir::from_channels(SR, vec![floor]);
    let cut = shape(
        &g,
        SR,
        &Shape {
            gate_db: -60.0,
            ..Shape::default()
        },
        false,
    )
    .len();
    assert!((12_000..13_000).contains(&cut), "gated at {cut}");
    assert_eq!(
        shape(
            &g,
            SR,
            &Shape {
                gate_db: GATE_OFF,
                ..Shape::default()
            },
            false
        )
        .len(),
        48_000
    );
    // Decay: the end is quieter; attack: the start.
    let d = shape(
        &ir,
        SR,
        &Shape {
            decay: 0.5,
            ..Shape::default()
        },
        false,
    );
    let e = |c: &[f32]| c.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>();
    assert!(e(&d.channels[0][40_000..]) < 0.01 * e(&ir.channels[0][40_000..]));
    let a = shape(
        &ir,
        SR,
        &Shape {
            attack_ms: 100.0,
            ..Shape::default()
        },
        false,
    );
    assert!(e(&a.channels[0][..1_000]) < 0.01 * e(&ir.channels[0][..1_000]));
    // Damping darkens the end and leaves the start.
    let dm = shape(
        &ir,
        SR,
        &Shape {
            damp: 1.0,
            ..Shape::default()
        },
        false,
    );
    let highs = |c: &[f32]| e(&c.windows(2).map(|w| w[1] - w[0]).collect::<Vec<_>>());
    assert!(highs(&dm.channels[0][40_000..47_000]) < 0.3 * highs(&ir.channels[0][40_000..47_000]));
    assert!(highs(&dm.channels[0][..2_000]) > 0.6 * highs(&ir.channels[0][..2_000]));
    // The cuts.
    let lo = shape(
        &ir,
        SR,
        &Shape {
            low_hz: 1_000.0,
            ..Shape::default()
        },
        false,
    );
    let sum = |c: &[f32]| c.iter().map(|v| *v as f64).sum::<f64>().abs();
    assert!(sum(&lo.channels[0]) < 0.05 * sum(&ir.channels[0]).max(1.0));
    let hi = shape(
        &ir,
        SR,
        &Shape {
            high_hz: 1_000.0,
            ..Shape::default()
        },
        false,
    );
    assert!(highs(&hi.channels[0]) < 0.05 * highs(&ir.channels[0]));
}

#[test]
#[allow(clippy::needless_range_loop)]
fn resampling_keeps_a_tone_where_it_was() {
    // 1 kHz at 44.1 kHz, to 48 kHz: still 1 kHz (zero crossings 1 ms apart).
    let x: Vec<f32> = (0..44_100)
        .map(|i| math::sin(std::f32::consts::TAU * 1_000.0 * i as f32 / 44_100.0))
        .collect();
    let y = resample(&x, 48_000.0 / 44_100.0, usize::MAX);
    assert_eq!(y.len(), 48_000);
    for i in 1_000..47_000 {
        let want = math::sin(std::f64::consts::TAU * 1_000.0 * i as f64 / 48_000.0) as f32;
        assert!(
            (y[i] - want).abs() < 2e-3,
            "at {i}: {} against {want}",
            y[i]
        );
    }
}

#[test]
fn the_room_a_response_gets_holds_its_stretches() {
    for taps in [10, 30_000, 65_536, 65_537, 300_000, 900_000] {
        let c = capacity_for(taps);
        assert!(c.taps() >= taps);
        assert!(capacity_for(c.taps()) == c, "{taps}");
    }
    assert_eq!(
        capacity_for(100),
        capacity_for(60_000),
        "short ones share a room"
    );
    let ir = factory(0, SR);
    let (_, k) = prepare(&ir, SR, &Shape::default(), None);
    let (_, k2) = prepare(
        &ir,
        SR,
        &Shape {
            stretch: 2.0,
            ..Shape::default()
        },
        Some(k.cap()),
    );
    assert_eq!(
        k.cap(),
        k2.cap(),
        "a stretched studio fits the studio's room"
    );
    assert!(overview(&ir, 100).iter().any(|v| *v > 0.0));
}
