//! Unda's DSP tests: the decimator's response, the tables' band limits and accuracy, and
//! a voice's basic behaviour. The instrument as a whole (from its parameters) is tested
//! in `crates/cantus-engine/tests/inst_wavetable.rs`.

use super::decim::{coefficients, Decimator, COEFS, TRANSITION};
use super::osc::Warp;
use super::table::{Table, LEVEL_LEN};
use super::*;
use crate::math;
use std::f32::consts::TAU;

/// Settings as the init patch has them (the engine derives these from the parameters).
pub(crate) fn init() -> Settings {
    let mut s = Settings::default();
    s.osc[0].on = true;
    for o in 0..3 {
        s.osc[o].table = TableChoice::Factory(0);
        s.osc[o].unison = 1;
        s.osc[o].rand = 1.0;
        let b = target::OSC + o * target::OSC_N;
        s.base[b + target::DETUNE] = 20.0;
        s.base[b + target::BLEND] = 0.5;
        s.base[b + target::WIDTH] = 1.0;
    }
    s.base[target::NOISE_LEVEL] = -12.0;
    s.base[target::NOISE_WIDTH] = 1.0;
    for f in 0..2 {
        let b = target::FLT + f * target::FLT_N;
        s.base[b + target::CUTOFF] = 2_000.0;
        s.base[b + target::RES] = 0.1;
        s.base[b + target::MIX] = 1.0;
    }
    for e in 0..4 {
        let b = target::ENV + e * target::ENV_N;
        s.base[b + target::ATTACK] = 0.002;
        s.base[b + target::DECAY] = if e == 0 { 0.6 } else { 0.4 };
        s.base[b + target::SUSTAIN] = if e == 0 { 1.0 } else { 0.0 };
        s.base[b + target::RELEASE] = if e == 0 { 0.12 } else { 0.3 };
        s.env_curves[e] = EnvCurves {
            attack: 0.0,
            decay: -0.6,
            release: -0.6,
        };
    }
    for l in 0..8 {
        s.base[target::LFO_RATE + l] = 2.0;
    }
    s
}

fn render(u: &mut Unda, frames: usize, block: usize) -> Vec<f32> {
    let mut out = vec![0.0; frames * 2];
    let mut l = vec![0.0; block];
    let mut r = vec![0.0; block];
    let mut i = 0;
    while i < frames {
        let n = block.min(frames - i);
        l[..n].iter_mut().for_each(|x| *x = 0.0);
        r[..n].iter_mut().for_each(|x| *x = 0.0);
        u.process(&mut l[..n], &mut r[..n]);
        for j in 0..n {
            out[2 * (i + j)] = l[j];
            out[2 * (i + j) + 1] = r[j];
        }
        i += n;
    }
    out
}

/// Level (dB) of a sine at `hz` through a 2× decimator running at `sr` (the high rate).
fn through_decimator(hz: f32, sr: f32) -> f32 {
    let mut d = Decimator::new();
    let n = 32_768;
    let mut peak = 0.0f32;
    for i in 0..n {
        // In f64: an f32 argument this large jitters the phase into broadband noise.
        let x = |k: usize| {
            let t = (hz as f64 * k as f64 / sr as f64).fract();
            math::sin(std::f64::consts::TAU * t) as f32
        };
        let (y, _) = d.tick(2, &[x(2 * i), x(2 * i + 1)], &[0.0, 0.0]);
        if i > n / 2 {
            peak = peak.max(y.abs());
        }
    }
    20.0 * math::log10(peak.max(1e-12))
}

/// `cargo test -p cantus-dsp --release unda_speed -- --ignored --nocapture`
#[test]
#[ignore]
fn unda_speed() {
    let case = |label: &str, f: &dyn Fn(&mut Settings), voices: usize| {
        let mut s = init();
        f(&mut s);
        let mut u = Unda::new(48_000.0);
        u.set(&s);
        for v in 0..voices {
            u.note_on(40 + 3 * v as u8, v as u64 + 1, 0.8);
        }
        let n = 48_000 * 2;
        let t = std::time::Instant::now();
        let out = render(&mut u, n, 256);
        let secs = t.elapsed().as_secs_f64();
        let ns = secs * 1e9 / n as f64 / voices as f64;
        println!(
            "{label}: {ns:.1} ns per voice-sample = {:.3} % of a core per voice ({})",
            ns * 48_000.0 / 1e7,
            out[out.len() / 2]
        );
    };
    case("init", &|_| {}, 16);
    case("sine table pos", &|s| s.base[target::OSC] = 1.0, 16);
    case("7 lanes", &|s| s.osc[0].unison = 7, 16);
    case(
        "ladder",
        &|s| {
            s.filters[0].on = true;
            s.filters[0].ty = filter::FilterType::LadderLp24;
        },
        16,
    );
    case("1x", &|s| s.oversample = 1, 16);
    // The baseline on this machine, now: Firmus, 16 voices.
    let mut f = crate::Firmus::new(48_000.0);
    for v in 0..16u8 {
        f.note_on(40 + 3 * v, 100, v as u64);
    }
    let mut buf = vec![0.0f32; 256];
    let t = std::time::Instant::now();
    for _ in 0..(48_000 * 2 / 256) {
        buf.iter_mut().for_each(|x| *x = 0.0);
        f.process_add(&mut buf);
    }
    let ns = t.elapsed().as_secs_f64() * 1e9 / (48_000.0 * 2.0) / 16.0;
    println!("firmus baseline: {ns:.1} ns per voice-sample ({})", buf[0]);
}

#[test]
#[ignore]
fn print_half_band_response() {
    for (n, tb) in [(12usize, TRANSITION), (8, 0.1), (16, 0.02)] {
        let c = coefficients(n, tb);
        let resp = |f: f64| {
            let w = std::f64::consts::TAU * f;
            let ap = |a: f64| {
                // (a + z^-2)/(1 + a z^-2) at z = e^{jw}
                let (zr, zi) = (math::cos(2.0 * w), -math::sin(2.0 * w));
                let (nr, ni) = (a + zr, zi);
                let (dr, di) = (1.0 + a * zr, a * zi);
                let d = dr * dr + di * di;
                ((nr * dr + ni * di) / d, (ni * dr - nr * di) / d)
            };
            let mul = |x: (f64, f64), y: (f64, f64)| (x.0 * y.0 - x.1 * y.1, x.0 * y.1 + x.1 * y.0);
            let (mut p0, mut p1) = ((1.0, 0.0), (1.0, 0.0));
            for (i, a) in c.iter().enumerate() {
                if i % 2 == 0 {
                    p0 = mul(p0, ap(*a));
                } else {
                    p1 = mul(p1, ap(*a));
                }
            }
            let p1 = mul(p1, (math::cos(w), -math::sin(w)));
            let h = (0.5 * (p0.0 + p1.0), 0.5 * (p0.1 + p1.1));
            20.0 * math::log10((h.0 * h.0 + h.1 * h.1).sqrt())
        };
        println!("n {n} tb {tb}: {c:?}");
        for f in [0.1, 0.2, 0.25, 0.2917, 0.35, 0.45] {
            println!("  f {f}: {:.1} dB", resp(f));
        }
    }
}

#[test]
fn the_decimator_passes_the_band_and_rejects_what_would_fold() {
    let c = coefficients(COEFS, TRANSITION);
    assert!(c.iter().all(|a| *a > 0.0 && *a < 1.0), "{c:?}");
    assert!(c.windows(2).all(|w| w[0] < w[1]), "ascending: {c:?}");
    let sr = 96_000.0;
    for hz in [100.0, 1_000.0, 10_000.0, 18_000.0] {
        let db = through_decimator(hz, sr);
        assert!(db.abs() < 0.1, "{hz} Hz passes at {db:.3} dB");
    }
    // Everything from 28 kHz up would fold below 20 kHz: it must be gone.
    for hz in [28_000.0, 32_000.0, 40_000.0, 47_000.0] {
        let db = through_decimator(hz, sr);
        assert!(db < -96.0, "{hz} Hz leaks at {db:.1} dB");
    }
}

#[test]
fn a_table_keeps_its_harmonics_and_each_level_stops_where_it_should() {
    let t = &factory::tables()[0];
    assert_eq!(t.frames(), 4);
    // The saw on level 0: a ramp, normalised to a peak of one.
    let c = t.cycle(0);
    assert_eq!(c.len(), LEVEL_LEN[0]);
    let peak = (0..t.frames())
        .flat_map(|f| t.cycle(f).iter())
        .fold(0.0f32, |m, x| m.max(x.abs()));
    assert!((peak - 1.0).abs() < 1e-5, "peak {peak}");
    // Level ℓ holds no harmonic above its top.
    for (l, (&len, &top)) in LEVEL_LEN.iter().zip(&table::LEVEL_TOP).enumerate().skip(1) {
        let cyc = t.level_cycle(l, 0);
        assert_eq!(cyc.len(), len);
        let spec = table::spectrum_of(cyc);
        let above: f32 = spec.iter().skip(top).map(|(a, b)| a * a + b * b).sum();
        let below: f32 = spec.iter().take(top).map(|(a, b)| a * a + b * b).sum();
        assert!(above < 1e-10 * below, "level {l}: {above} above {top}");
    }
    // The cubic read agrees with the stored samples on the grid.
    for i in [0usize, 1, 700, 2047, 8191] {
        let x = t.read(0, 0, i as f32 / LEVEL_LEN[0] as f32);
        assert!((x - c[i]).abs() < 1e-5, "{i}: {x} vs {}", c[i]);
    }
}

#[test]
fn audio_becomes_a_table_frame_by_frame() {
    // Three cycles of 1024 samples: a sine, its octave, silence.
    let mut audio = Vec::new();
    for k in 0..3 {
        for i in 0..1024 {
            let x = i as f32 / 1024.0;
            audio.push(match k {
                0 => math::sin(TAU * x),
                1 => 0.5 * math::sin(2.0 * TAU * x),
                _ => 0.0,
            });
        }
    }
    let t = Table::from_audio(&audio, 1, 1024);
    assert_eq!(t.frames(), 3);
    let c1 = t.cycle(1);
    // Normalised as a whole: the octave keeps its half level against the sine.
    let p1 = c1.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    assert!((p1 - 0.5).abs() < 1e-3, "{p1}");
    assert!((c1[1024] - 0.5 * math::sin(TAU * 2.0 * 1024.0 / 8192.0)).abs() < 1e-3);
    assert!(t.cycle(2).iter().all(|x| x.abs() < 1e-6));
}

#[test]
fn the_init_patch_plays_a_note_and_then_is_exactly_silent() {
    let mut u = Unda::new(48_000.0);
    u.set(&init());
    u.note_on(60, 1, 1.0);
    let a = render(&mut u, 4_800, 64);
    assert!(a.iter().any(|x| x.abs() > 0.01), "it sounds");
    u.note_off(60, 1, 0.0);
    let b = render(&mut u, 48_000, 64);
    assert!(b.iter().all(|x| x.is_finite()));
    assert!(
        b[b.len() - 4_800..].iter().all(|x| *x == 0.0),
        "exact silence"
    );
    assert_eq!(u.active_voices(), 0);
    let mut l = [0.0; 64];
    let mut r = [0.0; 64];
    assert!(!u.process(&mut l, &mut r), "idle writes nothing");
}

#[test]
fn an_envelope_keeps_its_times_and_lands_exactly() {
    use super::modsrc::{Env, EnvSettings, Stage};
    let sr = 48_000.0;
    let s = EnvSettings {
        delay: 0.01,
        attack: 0.02,
        hold: 0.005,
        decay: 0.1,
        sustain: 0.25,
        release: 0.05,
        a_curve: 0.3,
        d_curve: -0.6,
        r_curve: -0.6,
    };
    let mut e = Env::default();
    e.trigger(&s, sr);
    let mut n = 0usize;
    let mut peak_at = None;
    while e.stage != Stage::Sustain {
        let v = e.next();
        n += 1;
        if v == 1.0 && peak_at.is_none() {
            peak_at = Some(n);
        }
        assert!(n < 48_000, "never reached sustain");
    }
    // Delay + attack is 30 ms; hold + decay 105 ms more (within a sample or two).
    let peak = peak_at.expect("the attack lands on exactly 1") as f32;
    assert!((peak - 0.03 * sr).abs() <= 2.0, "peak at {peak}");
    assert!((n as f32 - 0.135 * sr).abs() <= 3.0, "sustain at {n}");
    assert_eq!(e.level(), 0.25);
    e.release();
    let mut m = 0usize;
    while !e.is_idle() {
        e.next();
        m += 1;
    }
    assert!((m as f32 - 0.05 * sr).abs() <= 2.0, "release took {m}");
    assert_eq!(e.level(), 0.0, "a release ends at exactly zero");
}

#[test]
fn the_filter_curve_the_editor_draws_is_what_plays() {
    use filter::{magnitude_db, FilterSlot, FilterType};
    let sr = 96_000.0;
    for ty in [
        FilterType::SvfLp12,
        FilterType::SvfLp24,
        FilterType::SvfBp,
        FilterType::SvfHp24,
        FilterType::SvfNotch,
        FilterType::LadderLp24,
        FilterType::LadderLp12,
        FilterType::LadderHp24,
    ] {
        let mut f = FilterSlot::new(sr);
        f.set(sr, ty, [1_000.0; 2], 0.3, 0.0);
        for hz in [200.0f32, 700.0, 1_000.0, 1_500.0, 4_000.0] {
            f.reset();
            let n = 32_768;
            let mut peak = 0.0f32;
            for i in 0..n {
                // Small, so the ladder's saturation stays out of it.
                let x = 0.01 * math::sin(TAU * hz * i as f32 / sr);
                let y = f.process([x, x])[0];
                if i > n / 2 {
                    peak = peak.max(y.abs());
                }
            }
            let measured = 20.0 * math::log10(peak / 0.01);
            let drawn = magnitude_db(ty, 1_000.0, 0.3, hz, sr);
            if drawn > -40.0 {
                assert!(
                    (measured - drawn).abs() < 0.5,
                    "{ty:?} at {hz} Hz: {measured:.2} vs {drawn:.2} dB"
                );
            }
        }
    }
}

/// RMS of the left channel of one note at `lanes` unison lanes.
fn unison_rms(lanes: usize, width: f32) -> (f32, f32) {
    let mut s = init();
    s.osc[0].unison = lanes;
    s.base[target::OSC + target::WIDTH] = width;
    s.base[target::OSC + target::DETUNE] = 30.0;
    let mut u = Unda::new(48_000.0);
    u.set(&s);
    u.note_on(57, 3, 1.0);
    let out = render(&mut u, 48_000, 256);
    let tail = &out[out.len() / 2..];
    let (mut l, mut m) = (0.0f64, 0.0f64);
    for fr in tail.chunks(2) {
        l += math::powi(fr[0] as f64, 2) + math::powi(fr[1] as f64, 2);
        m += math::powi((fr[0] + fr[1]) as f64 * 0.5, 2);
    }
    (l as f32, m as f32)
}

#[test]
fn unison_keeps_its_level_and_its_mono_sum() {
    let (one, _) = unison_rms(1, 1.0);
    for n in [2usize, 3, 5, 7, 9, 12, 16] {
        let (p, mono) = unison_rms(n, 1.0);
        let db = 10.0 * math::log10(p / one);
        assert!(db.abs() < 1.0, "{n} lanes: {db:.2} dB against one");
        // The mono sum loses about 3 dB at most against the stereo power per channel
        // (two lanes panned hard left and right lose exactly that).
        let loss = 10.0 * math::log10(mono / (p * 0.5));
        assert!(
            loss > -3.2,
            "{n} lanes at full width: the mono sum loses {loss:.2} dB"
        );
    }
}

fn engines_patch() -> Settings {
    let mut s = init();
    // Osc 2: Partes on the saw→square table; osc 3: Grana of the vowel table.
    for (o, engine, table) in [(1, Engine::Partes, 1usize), (2, Engine::Grana, 5)] {
        s.osc[o].on = true;
        s.osc[o].engine = engine;
        s.osc[o].table = TableChoice::Factory(table);
        s.osc[o].partials = 128;
        s.osc[o].pitch_rand = 30.0;
        s.osc[o].reverse = 0.2;
        let b = target::OSC + o * target::OSC_N;
        s.base[b + target::POS] = 0.4;
        s.base[b + target::TILT] = -3.0;
        s.base[b + target::STRETCH] = 0.02;
        s.base[b + target::DENSITY] = 40.0;
        s.base[b + target::GLEN] = 0.06;
        s.base[b + target::SPRAY] = 0.02;
    }
    s
}

#[test]
fn partes_and_grana_give_the_same_samples_at_every_block_size() {
    let s = engines_patch();
    let run = |block: usize| {
        let mut u = Unda::new(48_000.0);
        u.set(&s);
        let mut out = Vec::new();
        u.note_on(52, 11, 0.9);
        out.extend(render(&mut u, 2_000, block));
        u.note_on(59, 12, 0.7);
        out.extend(render(&mut u, 6_001, block));
        u.note_off(52, 11, 0.0);
        out.extend(render(&mut u, 12_000, block));
        out
    };
    let reference = run(512);
    assert!(reference.iter().any(|x| x.abs() > 0.01), "they sound");
    for block in [1usize, 7, 64, 1000] {
        assert_eq!(run(block), reference, "block {block}");
    }
}

/// Energy of the left channel at `hz` (a Goertzel filter over `x`'s second half).
fn energy_at(x: &[f32], hz: f64, sr: f64) -> f64 {
    let n = x.len() / 2;
    let w = std::f64::consts::TAU * hz / sr;
    let (mut s1, mut s2) = (0.0f64, 0.0f64);
    for i in n / 2..n {
        let s0 = x[2 * i] as f64 + 2.0 * math::cos(w) * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    s1 * s1 + s2 * s2 - 2.0 * math::cos(w) * s1 * s2
}

#[test]
fn partes_plays_the_table_and_stretch_moves_its_partials() {
    let one = |engine: Engine, stretch: f32| {
        let mut s = init();
        s.osc[0].engine = engine;
        s.osc[0].rand = 0.0;
        s.osc[0].partials = 256;
        s.base[target::OSC + target::STRETCH] = stretch;
        let mut u = Unda::new(48_000.0);
        u.set(&s);
        u.note_on(57, 1, 1.0);
        render(&mut u, 24_000, 256)
    };
    // At stretch 0 Partes is the table, as partials: the two renders agree.
    let (w, p) = (one(Engine::Wavetable, 0.0), one(Engine::Partes, 0.0));
    let dot: f64 = w.iter().zip(&p).map(|(a, b)| *a as f64 * *b as f64).sum();
    let nw: f64 = w.iter().map(|a| math::powi(*a as f64, 2)).sum();
    let np: f64 = p.iter().map(|a| math::powi(*a as f64, 2)).sum();
    let corr = dot / (nw * np).sqrt();
    assert!(corr > 0.99, "correlation {corr}");
    // Stretched, the second partial leaves the harmonic.
    let st = one(Engine::Partes, 0.05);
    let f0 = 220.0;
    let moved = energy_at(&st, f0 * math::powf(2f64, 1.05), 48_000.0);
    let stayed = energy_at(&st, f0 * 2.0, 48_000.0);
    assert!(moved > 100.0 * stayed, "{moved} vs {stayed}");
}

#[test]
fn every_block_size_gives_the_same_samples() {
    let mut s = init();
    s.osc[0].unison = 7;
    s.osc[0].warp[0] = Warp::BendPlus;
    s.base[target::OSC + target::WARP1] = 0.4;
    s.filters[0].on = true;
    s.filters[0].ty = filter::FilterType::LadderLp24;
    let run = |block: usize| {
        let mut u = Unda::new(48_000.0);
        u.set(&s);
        let mut out = Vec::new();
        u.note_on(48, 7, 0.8);
        out.extend(render(&mut u, 1_000, block));
        u.note_on(55, 8, 0.6);
        out.extend(render(&mut u, 3_001, block));
        u.note_off(48, 7, 0.0);
        out.extend(render(&mut u, 9_000, block));
        out
    };
    let reference = run(512);
    for block in [1usize, 7, 64, 1000] {
        assert_eq!(run(block), reference, "block {block}");
    }
}
