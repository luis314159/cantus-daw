//! Caelum's house tests: the null, the block sizes, the exact rest and its twins, the
//! latency, freeze, infinite, and every factory space stable. The measurements of the
//! reverb itself are in `caelum_tests_measure.rs`.

use super::presets;
use super::*;

pub(super) const FS: f32 = 48_000.0;

pub(super) fn noise(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// A test signal: bursts of noise and tones with silences between.
pub(super) fn program(n: usize, seed: u32) -> (Vec<f32>, Vec<f32>) {
    let mut seed = seed.max(1);
    let mut l = vec![0.0; n];
    let mut r = vec![0.0; n];
    for i in 0..n {
        let on = (i / 9_000) % 3 != 2;
        let t = i as f32 / FS;
        if on {
            l[i] = 0.3 * noise(&mut seed) + 0.3 * math::sin(t * 440.0 * std::f32::consts::TAU);
            r[i] = 0.3 * noise(&mut seed) + 0.2 * math::sin(t * 330.0 * std::f32::consts::TAU);
        }
    }
    (l, r)
}

pub(super) fn unit(s: &CaelumSettings) -> Caelum {
    let mut c = Caelum::new(FS);
    c.set(s);
    c.reset();
    c
}

/// Run a whole signal through in blocks of `block`; `change` is applied at its sample.
pub(super) fn render(
    c: &mut Caelum,
    l: &[f32],
    r: &[f32],
    block: usize,
    key: Option<(&[f32], &[f32])>,
    change: Option<(usize, &CaelumSettings)>,
) -> (Vec<f32>, Vec<f32>) {
    let mut ol = l.to_vec();
    let mut or = r.to_vec();
    let mut pos = 0;
    while pos < l.len() {
        let mut m = block.min(l.len() - pos);
        if let Some((at, s)) = change {
            if pos == at {
                c.set(s);
            } else if pos < at && pos + m > at {
                m = at - pos;
            }
        }
        let k = key.map(|(kl, kr)| (&kl[pos..pos + m], &kr[pos..pos + m]));
        c.process(&mut ol[pos..pos + m], &mut or[pos..pos + m], k);
        pos += m;
    }
    (ol, or)
}

pub(super) fn same(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

fn heavy() -> CaelumSettings {
    CaelumSettings {
        shimmer: 0.8,
        pitch_b_level: 1.0,
        bloom_ms: 900.0,
        duck_db: 18.0,
        low_cut_hz: 150.0,
        high_cut_hz: 9_000.0,
        width: 1.4,
        mod_depth: 1.0,
        ..presets::fifth_cathedral()
    }
}

#[test]
fn mix_zero_is_a_null_to_the_bit() {
    let (l, r) = program(96_000, 3);
    for (name, s) in [
        ("defaults", CaelumSettings::default()),
        (
            "heavy at mix 0",
            CaelumSettings {
                mix: 0.0,
                ..heavy()
            },
        ),
        (
            "infinite, frozen, mix 0",
            CaelumSettings {
                mix: 0.0,
                freeze: true,
                ..presets::endless_ascent()
            },
        ),
    ] {
        let mut c = unit(&s);
        let (ol, or) = render(&mut c, &l, &r, 256, None, None);
        assert!(same(&ol, &l) && same(&or, &r), "{name}: not a null");
    }
    assert_eq!(
        CaelumSettings::default().mix,
        0.0,
        "the defaults are a null"
    );
}

#[test]
fn the_output_does_not_depend_on_the_block_size() {
    let (l, r) = program(72_000, 5);
    let (kl, kr) = program(72_000, 11);
    let moved = CaelumSettings {
        size: 0.3,
        decay_s: 2.0,
        shimmer: 0.2,
        pitch_a: -12.0,
        freeze: true,
        lines: Lines::Eight,
        ..heavy()
    };
    for (name, s, keyed) in [
        ("starting", presets::starting(), false),
        ("heavy, keyed", heavy(), true),
        ("bloom pad", presets::bloom_pad(), false),
        ("light (8 lines)", presets::light_shimmer(), false),
    ] {
        let key = keyed.then_some((&kl[..], &kr[..]));
        let mut reference: Option<(Vec<f32>, Vec<f32>)> = None;
        for block in [1usize, 37, 64, 512] {
            let mut c = unit(&s);
            let out = render(&mut c, &l, &r, block, key, Some((30_000, &moved)));
            match &reference {
                None => reference = Some(out),
                Some(want) => assert!(
                    same(&out.0, &want.0) && same(&out.1, &want.1),
                    "{name}: block {block} differs"
                ),
            }
        }
    }
}

#[test]
fn the_latency_is_zero_and_the_dry_is_not_delayed() {
    let mut c = unit(&CaelumSettings {
        mix: 0.25,
        predelay_ms: 0.0,
        ..presets::starting()
    });
    assert_eq!(c.latency(), 0);
    let mut l = vec![0.0f32; 4_800];
    let mut r = vec![0.0f32; 4_800];
    l[0] = 1.0;
    r[0] = 1.0;
    c.process(&mut l, &mut r, None);
    // The dry at unity on the first sample; the wet starts after the diffusers.
    assert_eq!(l[0], 1.0);
    let first_wet = l[1..].iter().position(|v| *v != 0.0).map(|p| p + 1);
    eprintln!("the wet starts {first_wet:?} samples in");
    assert!(first_wet.unwrap_or(0) >= 100);
}

#[test]
fn a_synced_pre_delay_is_its_note_at_the_tempo() {
    assert_eq!(sync_ms(1.0, 120.0), 500.0);
    assert!((sync_ms(0.5, 90.0) - 1000.0 / 3.0).abs() < 1e-3);
    assert_eq!(sync_ms(4.0, 60.0), input::MAX_PREDELAY_MS);
}

/// Feed, silence until it says it is at rest, then four twins agree (the engine's
/// `fx_contract` rule, here on the DSP alone): `a` keeps processing zeros (zeros out, at
/// rest throughout), `d` is skipped, `b` is reset and skipped as far, `c` is new and
/// skipped as far; music again, and all four give the same bits.
fn twins(name: &str, s: &CaelumSettings, cap_s: f32) -> usize {
    let (l, r) = program(30_000, 17);
    let mk = || unit(s);
    let (mut a, mut b, mut d) = (mk(), mk(), mk());
    for u in [&mut a, &mut b, &mut d] {
        render(u, &l, &r, 333, None, None);
    }
    let mut silent = 0usize;
    let mut tail = Vec::new();
    while !a.at_rest() {
        assert!(
            (silent as f32) < cap_s * FS,
            "{name}: not at rest after {cap_s} s of silence ({:?})",
            a.rest_blocker()
        );
        let z = vec![0.0f32; 480];
        let mut first = None;
        for u in [&mut a, &mut b, &mut d] {
            let out = render(u, &z, &z, 480, None, None);
            first.get_or_insert(out);
        }
        if let Some((ol, or)) = first {
            let pk = ol.iter().chain(&or).fold(0.0f32, |m, v| m.max(v.abs()));
            tail.push(pk);
        }
        silent += 480;
        assert_eq!(a.at_rest(), b.at_rest());
    }
    // The last block that carried anything, before the drain left only zeros.
    let last = tail
        .iter()
        .rev()
        .find(|p| **p > 0.0)
        .copied()
        .unwrap_or(0.0);
    eprintln!(
        "{name}: at rest after {:.2} s of silence; the last sound before it {:.1} dBFS",
        silent as f32 / FS,
        20.0 * math::log10(last.max(1e-30))
    );
    assert!(last < 1e-5, "{name}: the drain cut a tail at {last}");
    let k = 12_345;
    let z = vec![0.0f32; k];
    let (zl, zr) = render(&mut a, &z, &z, 100, None, None);
    assert!(
        zl.iter().chain(&zr).all(|v| *v == 0.0),
        "{name}: silence gave sound"
    );
    assert!(a.at_rest(), "{name}: silence left it not at rest");
    let total = l.len() + silent + k;
    d.skip(k);
    b.reset();
    b.skip(total);
    let mut c = mk();
    c.skip(total);
    assert!(d.at_rest() && b.at_rest() && c.at_rest(), "{name}");
    let (l2, r2) = program(40_000, 23);
    let want = render(&mut a, &l2, &r2, 512, None, None);
    for (who, u) in [("skipped", &mut d), ("reset", &mut b), ("new", &mut c)] {
        let got = render(u, &l2, &r2, 77, None, None);
        assert!(
            same(&got.0, &want.0) && same(&got.1, &want.1),
            "{name}: the {who} twin differs"
        );
    }
    silent
}

#[test]
fn it_comes_to_rest_exactly_and_its_twins_agree() {
    twins("defaults (mix 0)", &CaelumSettings::default(), 40.0);
    twins("starting", &presets::starting(), 60.0);
    twins("glass plate", &presets::glass_plate(), 30.0);
    twins("bloom pad", &presets::bloom_pad(), 60.0);
    twins("ducked vocal", &presets::ducked_vocal(), 30.0);
    twins("light (8 lines)", &presets::light_shimmer(), 40.0);
}

#[test]
fn a_new_unit_is_at_rest_even_infinite_or_frozen() {
    for s in [presets::endless_ascent(), presets::freeze_swell()] {
        let mut c = unit(&CaelumSettings { freeze: true, ..s });
        assert!(c.at_rest(), "{:?}", c.rest_blocker());
        c.skip(100_000);
        assert!(c.at_rest());
    }
}

#[test]
fn infinite_never_comes_to_rest_while_it_holds() {
    let (l, r) = program(48_000, 9);
    for s in [presets::endless_ascent(), presets::freeze_swell()] {
        let mut c = unit(&s);
        render(&mut c, &l, &r, 512, None, None);
        let z = vec![0.0f32; 48_000 * 10];
        let (ol, _) = render(&mut c, &z, &z, 512, None, None);
        assert!(!c.at_rest());
        let last = ol[ol.len() - 4_800..]
            .iter()
            .fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(last > 1e-3, "infinite let go: {last}");
    }
}

/// Freeze shuts the input out: notes played into a frozen wash do not reach the wet.
#[test]
fn freeze_holds_and_lets_nothing_in() {
    let s = CaelumSettings {
        mix: 1.0,
        ..presets::freeze_swell()
    };
    let mut held = unit(&s);
    let mut fresh = unit(&s);
    let (l, r) = program(48_000, 31);
    render(&mut held, &l, &r, 256, None, None);
    render(&mut fresh, &l, &r, 256, None, None);
    let frozen = CaelumSettings { freeze: true, ..s };
    held.set(&frozen);
    fresh.set(&frozen);
    let z = vec![0.0f32; 24_000];
    render(&mut held, &z, &z, 256, None, None);
    render(&mut fresh, &z, &z, 256, None, None);
    let (nl, nr) = program(48_000, 77);
    let zz = vec![0.0f32; 48_000];
    let (hl, _) = render(&mut held, &nl, &nr, 256, None, None);
    let (fl, _) = render(&mut fresh, &zz, &zz, 256, None, None);
    // Mix 1: the output is the wet alone, and the new notes are not in it.
    let diff = hl
        .iter()
        .zip(&fl)
        .fold(0.0f32, |m, (a, b)| m.max((a - b).abs()));
    eprintln!("what new notes changed in a frozen wet: {diff:e}");
    assert_eq!(diff, 0.0);
}

#[test]
fn every_factory_space_is_stable_and_sounds() {
    let (l, r) = program(48_000 * 6, 41);
    let list = presets::factory();
    assert!(list.len() >= 10);
    for p in &list {
        let mut c = unit(&p.settings);
        let (ol, or) = render(&mut c, &l, &r, 512, None, None);
        let pk = ol.iter().chain(&or).fold(0.0f32, |m, v| m.max(v.abs()));
        let wet: f32 = ol.iter().zip(&l).map(|(a, b)| (a - b) * (a - b)).sum();
        eprintln!("{:<20} peak {:+.1} dBFS", p.name, 20.0 * math::log10(pk));
        assert!(pk.is_finite() && pk < 4.0, "{}: peak {pk}", p.name);
        assert!(wet > 1.0, "{}: makes no sound", p.name);
        assert!(p.settings.mix > 0.0);
    }
    assert_eq!(presets::starting(), list[0].settings);
}
