//! Vetus's DSP tests (ADR 0133, provisional).

use super::digital::{BitsMode, RateFilter};
use super::*;
use crate::modfx_clock::Transport;
use crate::modfx_lfo::Rate as LfoRate;

const SR: f32 = 48_000.0;

/// A test signal: a chord with a decaying pluck every half second, stereo.
fn music(n: usize) -> (Vec<f32>, Vec<f32>) {
    let mut l = Vec::with_capacity(n);
    let mut r = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / SR;
        let env = math::exp(-((i % 24_000) as f32) / 6_000.0);
        let a = math::sin(std::f32::consts::TAU * 220.0 * t);
        let b = math::sin(std::f32::consts::TAU * 330.0 * t + 0.4);
        let c = math::sin(std::f32::consts::TAU * 2_750.0 * t);
        l.push(0.3 * env * (a + 0.6 * b) + 0.05 * c);
        r.push(0.3 * env * (0.7 * a + b) - 0.05 * c);
    }
    (l, r)
}

fn transport(samples: i64) -> Transport {
    Transport {
        playing: true,
        tempo_bpm: 120.0,
        samples,
    }
}

/// Every module on, Character up, a synced wow, dither, the noise following the music.
fn everything() -> VetusSettings {
    VetusSettings {
        character: 0.4,
        bits_on: true,
        bits: 7.5,
        dither: true,
        bits_mode: BitsMode::MuLaw,
        rate_on: true,
        rate_hz: 9_000.0,
        rate_filter: RateFilter::AntiAlias,
        wob_on: true,
        wow_ct: 25.0,
        wow_rate: LfoRate::Beats(4.0),
        flut_ct: 6.0,
        age_on: true,
        noise_on: true,
        noise: noise::NoiseParams {
            crackle: 0.6,
            hiss: 0.4,
            vinyl: true,
            hum: 0.3,
            hum_60: true,
        },
        follow: 0.5,
        tone_on: true,
        medium: Medium::Gramophone,
        lo: 150.0,
        hi: 6_000.0,
        res: 0.4,
        mix: 0.85,
        out_db: -2.0,
        ..VetusSettings::default()
    }
}

/// `x` through a new unit in blocks of `block`, the transport told at each block from
/// song sample `start`.
fn render(s: &VetusSettings, x: &(Vec<f32>, Vec<f32>), block: usize, start: i64) -> Vec<f32> {
    let mut v = Vetus::new(SR);
    v.set(s);
    v.reset();
    let (mut l, mut r) = x.clone();
    let mut at = 0;
    while at < l.len() {
        let end = (at + block).min(l.len());
        v.set_transport(&transport(start + at as i64));
        v.process(&mut l[at..end], &mut r[at..end]);
        at = end;
    }
    l.into_iter().zip(r).flat_map(|(a, b)| [a, b]).collect()
}

fn same_bits(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

fn interleave(x: &(Vec<f32>, Vec<f32>)) -> Vec<f32> {
    x.0.iter().zip(&x.1).flat_map(|(a, b)| [*a, *b]).collect()
}

/// Every module off is a wire to the bit, whatever else is set.
#[test]
fn every_module_off_is_a_wire() {
    let x = music(30_000);
    let dry = interleave(&x);
    assert!(same_bits(
        &render(&VetusSettings::default(), &x, 64, 0),
        &dry
    ));
    let mut s = everything();
    for m in [
        &mut s.bits_on,
        &mut s.rate_on,
        &mut s.wob_on,
        &mut s.age_on,
        &mut s.noise_on,
        &mut s.tone_on,
    ] {
        *m = false;
    }
    s.out_db = 0.0;
    for mix in [1.0, 0.4] {
        s.mix = mix;
        assert!(same_bits(&render(&s, &x, 37, 0), &dry), "mix {mix}");
    }
}

/// Twenty-four bits without dither is a wire, in every character.
#[test]
fn twenty_four_bits_without_dither_is_a_wire() {
    let x = music(20_000);
    let dry = interleave(&x);
    for mode in [BitsMode::Round, BitsMode::Truncate, BitsMode::MuLaw] {
        let s = VetusSettings {
            bits_on: true,
            bits: 24.0,
            bits_mode: mode,
            ..VetusSettings::default()
        };
        assert!(same_bits(&render(&s, &x, 64, 0), &dry), "{mode:?}");
    }
}

/// Each module alone changes the sound.
#[test]
fn every_module_is_heard() {
    let x = music(48_000);
    let dry = interleave(&x);
    let base = VetusSettings::default();
    let cases: [(&str, VetusSettings); 6] = [
        (
            "bits",
            VetusSettings {
                bits_on: true,
                ..base
            },
        ),
        (
            "rate",
            VetusSettings {
                rate_on: true,
                rate_hz: 6_000.0,
                ..base
            },
        ),
        (
            "wobble",
            VetusSettings {
                wob_on: true,
                ..base
            },
        ),
        (
            "age",
            VetusSettings {
                age_on: true,
                ..base
            },
        ),
        (
            "noise",
            VetusSettings {
                noise_on: true,
                ..base
            },
        ),
        (
            "tone",
            VetusSettings {
                tone_on: true,
                medium: Medium::Telephone,
                lo: 300.0,
                hi: 3_400.0,
                ..base
            },
        ),
    ];
    for (name, s) in cases {
        let y = render(&s, &x, 512, 0);
        let diff = y
            .iter()
            .zip(&dry)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(diff > 1e-3, "{name}: {diff}");
    }
}

/// The same bits at every block size, with everything moving.
#[test]
fn the_render_does_not_depend_on_the_block_size() {
    let x = music(40_000);
    let s = everything();
    let reference = render(&s, &x, 512, 0);
    for block in [1usize, 37, 64, 1000] {
        assert!(
            same_bits(&render(&s, &x, block, 0), &reference),
            "block {block}"
        );
    }
}

/// The noise, the dropouts and the wear are functions of the song sample: rendered from
/// sample 96 000 the noise of silence is the whole render's from there, to the bit.
#[test]
fn the_noise_is_the_same_from_any_point_of_the_song() {
    let s = VetusSettings {
        noise_on: true,
        age_on: true,
        bits_on: true,
        dither: true,
        noise: noise::NoiseParams {
            crackle: 0.7,
            hiss: 0.5,
            vinyl: false,
            hum: 0.4,
            hum_60: false,
        },
        ..VetusSettings::default()
    };
    let silent = (vec![0.0f32; 144_000], vec![0.0f32; 144_000]);
    let whole = render(&s, &silent, 512, 0);
    let part = (vec![0.0f32; 48_000], vec![0.0f32; 48_000]);
    let late = render(&s, &part, 333, 96_000);
    assert!(late.iter().any(|v| v.abs() > 1e-3), "the noise sounds");
    assert!(same_bits(&whole[2 * 96_000..], &late));
}

/// Live, the host's blocks change size; offline they are even: the same bits.
#[test]
fn live_equals_offline() {
    let x = music(30_000);
    let s = everything();
    let offline = render(&s, &x, 512, 0);
    let mut v = Vetus::new(SR);
    v.set(&s);
    v.reset();
    let (mut l, mut r) = x.clone();
    let mut at = 0;
    let sizes = [128usize, 17, 480, 1, 256, 999, 64];
    let mut k = 0;
    while at < l.len() {
        let end = (at + sizes[k % sizes.len()]).min(l.len());
        k += 1;
        v.set_transport(&transport(at as i64));
        v.process(&mut l[at..end], &mut r[at..end]);
        at = end;
    }
    let live: Vec<f32> = l.into_iter().zip(r).flat_map(|(a, b)| [a, b]).collect();
    assert!(same_bits(&live, &offline));
}
