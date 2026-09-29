//! Pulsus II's tests: every model, the section, layers and round robin, chokes, the
//! pedal, tuning, headroom, aliasing and the seed. Pulsus I's own tests are in
//! `pulsus_legacy_tests.rs` and still hold (a kit built with `Pulsus::new` is model 1).

use super::shape::round_robin;
use super::*;
use crate::nulltest::{assert_null, peak, rms};

const SR: f32 = 48_000.0;

/// Every voice and model at its designed numbers, and every tom material.
fn all_models() -> Vec<(&'static str, PadSettings)> {
    let mut v = vec![
        ("kick pulsus", PadSettings::of(DrumVoice::Kick, 0)),
        (
            "kick 808",
            PadSettings {
                decay: 0.9,
                pitch_drop: 4.0,
                pitch_decay: 0.018,
                click: 0.45,
                tone: 1_600.0,
                ..PadSettings::of(DrumVoice::Kick, 1)
            },
        ),
        (
            "kick 909",
            PadSettings {
                decay: 0.45,
                pitch_drop: 26.0,
                pitch_decay: 0.028,
                click: 0.55,
                tone: 4_000.0,
                ..PadSettings::of(DrumVoice::Kick, 2)
            },
        ),
        ("snare pulsus", PadSettings::of(DrumVoice::Snare, 0)),
        (
            "snare modal",
            PadSettings {
                decay: 0.2,
                noise_decay: 0.24,
                tone: 2_600.0,
                ..PadSettings::of(DrumVoice::Snare, 1)
            },
        ),
        ("hat", PadSettings::of(DrumVoice::HiHat, 0)),
        (
            "cymbal 808",
            PadSettings {
                decay: 1.8,
                tone: 7_100.0,
                hp_cutoff: 3_000.0,
                ..PadSettings::of(DrumVoice::HiHat, 1)
            },
        ),
        (
            "cowbell 808",
            PadSettings {
                decay: 0.35,
                tone: 2_600.0,
                ..PadSettings::of(DrumVoice::HiHat, 2)
            },
        ),
        (
            "cymbal modal",
            PadSettings {
                decay: 2.6,
                tone: 6_500.0,
                hp_cutoff: 2_500.0,
                ..PadSettings::of(DrumVoice::HiHat, 3)
            },
        ),
        (
            "clap",
            PadSettings {
                decay: 0.28,
                tone: 1_150.0,
                ..PadSettings::of(DrumVoice::Clap, 0)
            },
        ),
    ];
    for (name, material) in [
        ("tom membrane", 0u8),
        ("tom bar", 1),
        ("tom wood", 2),
        ("tom plate", 3),
        ("tom bell", 4),
    ] {
        v.push((
            name,
            PadSettings {
                decay: 0.7,
                material,
                ..PadSettings::of(DrumVoice::Tom, 0)
            },
        ));
    }
    v
}

/// A model 2 kit of these pads on pitches 36, 37, …, each with its own id.
fn kit2(pads: &[PadSettings]) -> Pulsus {
    let mut k = Pulsus::new(SR);
    k.set_kit(2, 7, 4, false);
    k.set_pad_count(pads.len());
    for (i, p) in pads.iter().enumerate() {
        let mut p = *p;
        p.id = i as u16 + 1;
        k.set_pad(i, 36 + i as u8, &p);
    }
    k
}

/// A hit: frame, pitch, velocity, and the note's tick and id (its place in the piece).
type Event = (usize, u8, u8, i64, u64);

/// Render `n` frames in `block`-sized chunks, triggering at the given frames and splitting
/// blocks at them exactly as the processor's event loop does.
fn render(k: &mut Pulsus, n: usize, block: usize, events: &[Event]) -> Vec<f32> {
    render_with(k, n, block, events, &[])
}

/// …and with controller changes `(frame, cc, value)`, also split at.
fn render_with(
    k: &mut Pulsus,
    n: usize,
    block: usize,
    events: &[Event],
    ccs: &[(usize, u8, f32)],
) -> Vec<f32> {
    let mut l = vec![0.0; n];
    let mut r = vec![0.0; n];
    let mut done = 0;
    let mut ordinals = [0u32; 128];
    while done < n {
        let mut end = (done + block).min(n);
        for (at, cc, value) in ccs {
            if *at == done {
                k.controller(*cc, *value);
            } else if *at > done && *at < end {
                end = *at;
            }
        }
        for (at, pitch, vel, tick, id) in events {
            if *at == done {
                let ordinal = ordinals[*pitch as usize];
                ordinals[*pitch as usize] += 1;
                k.trigger(
                    *pitch,
                    *vel,
                    Hit {
                        id: *id,
                        tick: *tick,
                        ordinal,
                        sequenced: true,
                    },
                );
            } else if *at > done && *at < end {
                end = *at;
            }
        }
        k.process_add(&mut l[done..end], &mut r[done..end]);
        done = end;
    }
    l.into_iter().zip(r).flat_map(|(a, b)| [a, b]).collect()
}

fn hit(at: usize, pitch: u8, vel: u8) -> Event {
    (at, pitch, vel, at as i64, (at as u64) << 8 | pitch as u64)
}

#[test]
fn every_model_sounds_and_falls_to_exact_silence() {
    for (name, pad) in all_models() {
        let mut k = kit2(&[pad]);
        let out = render(&mut k, SR as usize * 5, 512, &[hit(0, 36, 127)]);
        let p = peak(&out);
        eprintln!("peak {name}: {p:.3} rms100ms {:.4}", rms(&out[..9_600]));
        assert!(out.iter().all(|x| x.is_finite()), "{name} produced NaN/inf");
        assert!(p > 0.05, "{name} is inaudible: peak {p}");
        assert!(p < 0.9, "{name} is too hot on its own: peak {p}");
        assert_eq!(k.active_voices(), 0, "{name} never released its voice");
        assert_eq!(
            out[out.len() - 1],
            0.0,
            "{name} never reached exact silence"
        );
    }
}

/// A kit of every model with the whole section switched on somewhere, played at every
/// block size: where the host splits the buffer must not be audible.
#[test]
fn every_model_and_the_section_are_block_size_independent() {
    let mut pads: Vec<PadSettings> = all_models().into_iter().map(|(_, p)| p).collect();
    pads[0].filter = 1;
    pads[0].cutoff = 900.0;
    pads[0].filter_env = 24.0;
    pads[3].attack = 0.004;
    pads[3].hold = 0.01;
    pads[3].curve = 0.6;
    pads[3].trans_attack = 9.0;
    pads[3].trans_sustain = -6.0;
    pads[4].drive = 0.7;
    pads[4].drive_mode = 1;
    pads[5].pitch_env = -7.0;
    pads[6].curve = -0.8;
    pads[9].filter = 3;
    pads[9].resonance = 0.6;
    pads[10].glide = 3.0;
    pads[10].pitch_env = 5.0;
    let n = pads.len() as u8;
    let events: Vec<Event> = (0..n)
        .flat_map(|i| {
            [
                hit(i as usize * 2_333, 36 + i, 127),
                hit(20_000 + i as usize * 1_111, 36 + i, 64),
            ]
        })
        .collect();
    let reference = render(&mut kit2(&pads), 60_000, 512, &events);
    assert!(peak(&reference) > 0.1);
    for block in [1usize, 7, 64, 480, 1_000, 4_096, 8_192] {
        let out = render(&mut kit2(&pads), 60_000, block, &events);
        assert_null(
            &reference,
            &out,
            0.0,
            &format!("Pulsus II at block {block}"),
        );
    }
}

#[test]
fn a_model_two_kit_is_deterministic_and_its_seed_is_used() {
    let pads: Vec<PadSettings> = all_models().into_iter().map(|(_, p)| p).collect();
    let events: Vec<Event> = (0..pads.len() as u8)
        .map(|i| hit(i as usize * 3_000, 36 + i, 110))
        .collect();
    let a = render(&mut kit2(&pads), 60_000, 256, &events);
    let b = render(&mut kit2(&pads), 60_000, 256, &events);
    assert_null(&a, &b, 0.0, "repeat render");
    let mut other = kit2(&pads);
    other.set_kit(2, 8, 4, false);
    let c = render(&mut other, 60_000, 256, &events);
    let diff = a.iter().zip(&c).filter(|(x, y)| x != y).count();
    assert!(
        diff > 10_000,
        "a different seed changed only {diff} samples"
    );
}

/// The fix for Pulsus I's hit counter: in model 2 a hit sounds the same whether the render
/// began before it or at it, because its noise is seeded from where the note is.
#[test]
fn a_model_two_hit_does_not_depend_on_the_hits_before_it() {
    let snare = PadSettings::snare();
    let events = [hit(0, 36, 100), hit(12_000, 36, 100), hit(24_000, 36, 100)];
    let whole = render(&mut kit2(&[snare]), 36_000, 512, &events);
    // A render that "seeks" to the third hit: only that hit, at the same place.
    let mut late = kit2(&[snare]);
    let tail = render(&mut late, 36_000, 512, &events[2..]);
    assert_null(
        &whole[48_000..],
        &tail[48_000..],
        0.0,
        "model 2 after a seek",
    );

    // Model 1 keeps its defect, which is the point of keeping it.
    let mut old = kit2(&[snare]);
    old.set_kit(1, 7, 4, false);
    let whole1 = render(&mut old, 36_000, 512, &events);
    let mut old_late = kit2(&[snare]);
    old_late.set_kit(1, 7, 4, false);
    let tail1 = render(&mut old_late, 36_000, 512, &events[2..]);
    assert_ne!(&whole1[48_000..], &tail1[48_000..], "model 1 changed");
}

#[test]
fn cycle_visits_every_layer_in_order() {
    for n in 1..=8 {
        let picks: Vec<usize> = (0..n as u32 * 3)
            .map(|k| round_robin(1, n, k, 99))
            .collect();
        for (k, p) in picks.iter().enumerate() {
            assert_eq!(*p, k % n);
        }
    }
}

#[test]
fn no_repeat_never_plays_one_layer_twice_in_a_row() {
    for n in 2..=16 {
        for seed in [1u64, 2, 77, 12_345] {
            let mut last = usize::MAX;
            let mut seen = vec![0usize; n];
            for k in 0..10_000u32 {
                let p = round_robin(3, n, k, seed);
                assert!(p < n);
                assert_ne!(p, last, "n {n} seed {seed}: layer {p} twice at hit {k}");
                seen[p] += 1;
                last = p;
            }
            assert!(seen.iter().all(|c| *c > 0), "n {n}: a layer never played");
        }
    }
}

/// Random is uniform: χ² over 12 000 hits of 6 layers stays under the 0.1 % critical value
/// for five degrees of freedom (20.5).
#[test]
fn random_round_robin_is_uniform() {
    let n = 6;
    let hits = 12_000u32;
    let mut count = [0f64; 6];
    for k in 0..hits {
        count[round_robin(2, n, k, 4242)] += 1.0;
    }
    let expected = hits as f64 / n as f64;
    let chi2: f64 = count
        .iter()
        .map(|c| (c - expected).powi(2) / expected)
        .sum();
    assert!(chi2 < 20.5, "χ² {chi2}: {count:?}");
}

/// A decaying tone as a sample, `hz` at 48 kHz, so layers are told apart by pitch.
fn tone(hz: f32, seconds: f32) -> SampleRef {
    let n = (SR * seconds) as usize;
    let frames: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / SR;
            (std::f32::consts::TAU * hz * t).sin() * (-t * 3.0).exp() * 0.5
        })
        .collect();
    SampleRef {
        frames: frames.into(),
        rate: SR,
        channels: 1,
    }
}

fn sampler_with_layers(layers: &[(LayerSettings, SampleRef)], rr: u8) -> Pulsus {
    let pad = PadSettings {
        voice: DrumVoice::Sample,
        decay: 8.0,
        round_robin: rr,
        layer_count: layers.len() as u8,
        ..PadSettings::kick()
    };
    let mut k = kit2(&[pad]);
    for (i, (l, s)) in layers.iter().enumerate() {
        k.set_layer(0, i, l, Some(s.clone()));
    }
    k
}

/// The round robin reaches the sound: three layers at three pitches, cycled, come back in
/// order hit after hit.
#[test]
fn cycled_layers_play_in_turn() {
    let zone = LayerSettings::default();
    let layers = [
        (zone, tone(200.0, 0.2)),
        (zone, tone(300.0, 0.2)),
        (zone, tone(450.0, 0.2)),
    ];
    let one = |i: usize| {
        let mut k = sampler_with_layers(&layers[i..=i], 0);
        render(&mut k, 4_000, 256, &[hit(0, 36, 127)])
    };
    let solo: Vec<Vec<f32>> = (0..3).map(one).collect();
    let mut k = sampler_with_layers(&layers, 1);
    let events: Vec<Event> = (0..6).map(|i| hit(i * 12_000, 36, 127)).collect();
    let out = render(&mut k, 72_000, 256, &events);
    for i in 0..6 {
        let at = i * 12_000 * 2;
        assert_null(
            &solo[i % 3],
            &out[at..at + 8_000],
            1e-6,
            &format!("hit {i}"),
        );
    }
}

/// Equal-power cross-fades: two uncorrelated layers in adjacent zones keep the level
/// within ±1 dB across the fade.
#[test]
fn velocity_cross_fades_hold_the_level() {
    let noise = |seed: u32| {
        let mut n = shape::Noise(seed);
        let frames: Vec<f32> = (0..24_000).map(|_| n.next() * 0.3).collect();
        SampleRef {
            frames: frames.into(),
            rate: SR,
            channels: 1,
        }
    };
    let lo = LayerSettings {
        vel_hi: 64,
        xfade: 24,
        ..LayerSettings::default()
    };
    let hi = LayerSettings {
        vel_lo: 65,
        xfade: 24,
        ..LayerSettings::default()
    };
    let layers = [(lo, noise(11)), (hi, noise(977))];
    let level = |vel: u8| {
        let mut pad = sampler_with_layers(&layers, 0);
        // Level-flat velocity so only the cross-fade moves the level.
        let mut s = pad.pads[0];
        s.vel_level = 0.0;
        pad.pads[0] = s;
        rms(&render(&mut pad, 20_000, 512, &[hit(0, 36, vel)]))
    };
    let reference = level(20);
    for vel in [50u8, 56, 60, 64, 65, 68, 72, 78, 110] {
        let db = 20.0 * (level(vel) / reference).log10();
        assert!(db.abs() < 1.0, "velocity {vel}: {db:.2} dB");
    }
}

/// With a choke fade of F, the choked voice is 60 dB down by F (plus a sample) and not
/// yet by F/2 — for a short and a long fade, on an amp-envelope voice and a modal one.
#[test]
fn a_choke_fades_over_its_own_time() {
    for fade_ms in [1.0f32, 4.0, 50.0, 250.0] {
        for (name, base) in [
            (
                "hat",
                PadSettings {
                    decay: 3.0,
                    ..PadSettings::of(DrumVoice::HiHat, 0)
                },
            ),
            (
                "tom",
                PadSettings {
                    decay: 3.0,
                    ..PadSettings::of(DrumVoice::Tom, 0)
                },
            ),
        ] {
            let ringing = PadSettings {
                choke: 3,
                choke_fade: fade_ms / 1000.0,
                ..base
            };
            let cutter = PadSettings {
                choke: 3,
                decay: 0.02,
                ..PadSettings::of(DrumVoice::HiHat, 0)
            };
            let at = 4_800;
            let n = at + 20_000;
            let alone = render(&mut kit2(&[ringing, cutter]), n, 256, &[hit(0, 36, 120)]);
            let both = render(
                &mut kit2(&[ringing, cutter]),
                n,
                256,
                &[hit(0, 36, 120), hit(at, 37, 100)],
            );
            let cut = render(&mut kit2(&[ringing, cutter]), n, 256, &[hit(at, 37, 100)]);
            let residue: Vec<f32> = both.iter().zip(&cut).map(|(a, b)| a - b).collect();
            let f = (fade_ms / 1000.0 * SR) as usize;
            let w = (f / 4).clamp(8, 96);
            let ratio = |from: usize| {
                let r = from * 2..(from + w) * 2;
                rms(&residue[r.clone()]) / rms(&alone[r]).max(1e-12)
            };
            let late = ratio(at + f + 1);
            let half = ratio(at + f / 2 - w / 2);
            assert!(late < 1.2e-3, "{name} fade {fade_ms} ms: {late} at F");
            assert!(
                half > 1e-3,
                "{name} fade {fade_ms} ms: {half} at F/2 is already gone"
            );
        }
    }
}

#[test]
fn polyphony_one_is_a_self_choke_and_eight_is_not() {
    let open = |poly: u8| PadSettings {
        decay: 1.0,
        polyphony: poly,
        choke_fade: 0.004,
        ..PadSettings::of(DrumVoice::HiHat, 0)
    };
    for (poly, voices) in [(1u8, 1usize), (8, 3)] {
        let mut k = kit2(&[open(poly)]);
        render(
            &mut k,
            30_000,
            256,
            &[hit(0, 36, 100), hit(10_000, 36, 100), hit(20_000, 36, 100)],
        );
        assert_eq!(k.active_voices(), voices, "polyphony {poly}");
    }
}

#[test]
fn a_note_off_chokes_when_the_pad_asks() {
    let pad = PadSettings {
        decay: 2.0,
        choke_on_off: true,
        choke_fade: 0.002,
        ..PadSettings::of(DrumVoice::HiHat, 1)
    };
    let mut k = kit2(&[pad]);
    let (id, off) = (5u64, 9_600usize);
    let mut l = vec![0.0; 20_000];
    let mut r = vec![0.0; 20_000];
    k.trigger(
        36,
        110,
        Hit {
            id,
            tick: 0,
            ordinal: 0,
            sequenced: true,
        },
    );
    k.process_add(&mut l[..off], &mut r[..off]);
    let before = rms(&l[off - 480..off]);
    k.note_off(36, id);
    k.process_add(&mut l[off..], &mut r[off..]);
    let after = rms(&l[off + 480..off + 960]);
    assert!(before > 1e-3);
    assert!(after < before * 1e-3, "{after} against {before}");
    // Without the flag, the note-off does nothing.
    let mut free = kit2(&[PadSettings {
        choke_on_off: false,
        ..pad
    }]);
    free.trigger(
        36,
        110,
        Hit {
            id,
            tick: 0,
            ordinal: 0,
            sequenced: true,
        },
    );
    free.note_off(36, id);
    assert!(free.voices.iter().all(|v| !v.choking));
}

fn pedal_hat() -> PadSettings {
    PadSettings {
        decay: 0.06,
        open_decay: 1.2,
        pedal: true,
        choke: 1,
        choke_fade: 0.004,
        ..PadSettings::of(DrumVoice::HiHat, 0)
    }
}

/// The pedal opens a synthesized hat continuously: the more open, the longer it rings.
#[test]
fn the_pedal_opens_a_hat_continuously() {
    let ring = |open: f32| {
        let mut k = kit2(&[pedal_hat()]);
        k.controller(4, open);
        let out = render(&mut k, 24_000, 256, &[hit(0, 36, 100)]);
        rms(&out[2 * 4_800..2 * 14_400])
    };
    let levels: Vec<f32> = [0.0f32, 0.25, 0.5, 0.75, 1.0]
        .iter()
        .map(|o| ring(*o))
        .collect();
    assert!(levels.windows(2).all(|w| w[1] > w[0] * 1.05), "{levels:?}");
    // Reversed pedals are a setting, not a rewiring.
    let mut inv = kit2(&[pedal_hat()]);
    inv.set_kit(2, 7, 4, true);
    inv.controller(4, 0.0);
    let out = render(&mut inv, 24_000, 256, &[hit(0, 36, 100)]);
    let open = rms(&out[2 * 4_800..2 * 14_400]);
    assert!(
        (open / levels[4] - 1.0).abs() < 1e-4,
        "inverted pedal at 0 is not fully open"
    );
}

/// Closing the pedal on a ringing open hat chokes it and plays the foot's chick; a CC
/// ramp and the close, split where they fall, render the same at every block size.
#[test]
fn a_closing_pedal_chokes_a_ringing_hat_with_a_chick() {
    let events = [hit(0, 36, 110)];
    let mut ramp: Vec<(usize, u8, f32)> = (0..10)
        .map(|i| (1_000 + i * 700, 4, 1.0 - i as f32 * 0.05))
        .collect();
    ramp.push((12_000, 4, 0.0));
    let open_only = {
        let mut k = kit2(&[pedal_hat()]);
        k.controller(4, 1.0);
        render(&mut k, 30_000, 256, &events)
    };
    let closed = {
        let mut k = kit2(&[pedal_hat()]);
        k.controller(4, 1.0);
        render_with(&mut k, 30_000, 256, &events, &ramp)
    };
    let after = 2 * 12_000 + 2 * 2_400..2 * 30_000;
    assert!(
        rms(&closed[after.clone()]) < rms(&open_only[after]) * 0.02,
        "the pedal did not close the hat"
    );
    // The chick: something new right after the close.
    let chick = 2 * 12_000..2 * 12_000 + 2 * 480;
    assert!(
        peak(&closed[chick]) > 0.01,
        "no chick when the pedal closed"
    );
    for block in [1usize, 7, 64, 480, 4_096] {
        let mut k = kit2(&[pedal_hat()]);
        k.controller(4, 1.0);
        let out = render_with(&mut k, 30_000, block, &events, &ramp);
        assert_null(&closed, &out, 0.0, &format!("pedal at block {block}"));
    }
}

/// The frequency of a steady tone from its zero crossings (rising, interpolated).
fn frequency(x: &[f32]) -> f32 {
    let mut first = None;
    let mut last = 0.0;
    let mut count = 0;
    for i in 1..x.len() {
        if x[i - 1] < 0.0 && x[i] >= 0.0 {
            let at = (i - 1) as f64 + (-x[i - 1] / (x[i] - x[i - 1])) as f64;
            if first.is_none() {
                first = Some(at);
            } else {
                count += 1;
            }
            last = at;
        }
    }
    (count as f64 * SR as f64 / (last - first.unwrap_or(0.0))) as f32
}

fn cents(hz: f32, target: f32) -> f32 {
    1200.0 * (hz / target).log2()
}

fn left(stereo: &[f32]) -> Vec<f32> {
    stereo.iter().step_by(2).copied().collect()
}

/// Model 2's kick at Tune 0 is F1, and every kick model lands its tail within ±2 cents of
/// the note it names, across the range. Model 1's is where it always was: 45 Hz.
#[test]
fn every_kick_lands_on_its_note() {
    for (name, mut pad) in all_models().into_iter().take(3) {
        pad.decay = 3.0;
        pad.click = 0.0;
        pad.drive = 0.0;
        for tune in [-12.0f32, -5.0, 0.0, 7.0, 12.0] {
            pad.tune = tune;
            // The tail: past five pitch-fall times, and past the 808's sigh, which sags
            // with the amplitude and has faded by the second second.
            let out = left(&render(&mut kit2(&[pad]), 120_000, 512, &[hit(0, 36, 100)]));
            let hz = frequency(&out[72_000..115_200]);
            let target = KICK_F1_HZ * 2f32.powf(tune / 12.0);
            let error = cents(hz, target);
            eprintln!("pulsus tuning: {name} at {tune:+} st: {hz:.3} Hz, {error:+.2} cents");
            assert!(
                error.abs() < 2.0,
                "{name} at {tune} st: {hz:.3} Hz, {error:+.2} cents"
            );
        }
    }
    let mut old = kit2(&[PadSettings {
        decay: 3.0,
        click: 0.0,
        drive: 0.0,
        ..PadSettings::kick()
    }]);
    old.set_kit(1, 7, 4, false);
    let hz = frequency(&left(&render(&mut old, 120_000, 512, &[hit(0, 36, 100)]))[72_000..115_200]);
    eprintln!(
        "pulsus tuning: model 1 kick {hz:.3} Hz, {:+.2} cents from F1",
        cents(hz, KICK_F1_HZ)
    );
    assert!(
        (cents(hz, KICK_F1_HZ) - 52.0).abs() < 2.0,
        "model 1 kick at {hz} Hz"
    );
}

/// A Goertzel scan for the strongest frequency within ±60 cents of `near`, Hann-windowed.
fn peak_near(x: &[f32], near: f32) -> f32 {
    let n = x.len();
    let mut best = (0.0f64, near);
    for c in -240..=240 {
        let f = near * 2f32.powf(c as f32 / 4.0 / 1200.0);
        let w = std::f64::consts::TAU * f as f64 / SR as f64;
        let (mut s1, mut s2) = (0.0f64, 0.0f64);
        for (i, v) in x.iter().enumerate() {
            let hann = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / n as f64).cos();
            let s = *v as f64 * hann + 2.0 * w.cos() * s1 - s2;
            s2 = s1;
            s1 = s;
        }
        let p = s1 * s1 + s2 * s2 - 2.0 * w.cos() * s1 * s2;
        if p > best.0 {
            best = (p, f);
        }
    }
    best.1
}

/// A modal tom's fundamental is its tune, within ±5 cents, for every material.
#[test]
fn a_modal_tom_is_in_tune_at_every_material() {
    for material in 0..5u8 {
        for tune in [-12.0f32, 0.0, 9.0] {
            let pad = PadSettings {
                decay: 3.0,
                material,
                tune,
                ..PadSettings::of(DrumVoice::Tom, 0)
            };
            let out = left(&render(&mut kit2(&[pad]), 72_000, 512, &[hit(0, 36, 100)]));
            let target = 220.0 * 2f32.powf(tune / 12.0);
            let hz = peak_near(&out[24_000..72_000], target);
            let error = cents(hz, target);
            eprintln!("pulsus tuning: tom material {material} at {tune:+} st: {error:+.2} cents");
            assert!(
                error.abs() < 5.0,
                "material {material} at {tune} st: {error:+.2} cents"
            );
        }
    }
}

/// Every model hit at once at full velocity leaves headroom, and no pad alone is hot.
#[test]
fn the_whole_pulsus_two_range_at_full_velocity_leaves_headroom() {
    // One of each voice, as a kit would hold them: a kick, a snare, a hat, the two 808
    // metals, the modal cymbal, a clap and a tom.
    let every: Vec<PadSettings> = all_models().into_iter().map(|(_, p)| p).collect();
    let pads: Vec<PadSettings> = [0usize, 4, 5, 6, 7, 8, 9, 10]
        .iter()
        .map(|i| every[*i])
        .collect();
    let events: Vec<Event> = (0..pads.len() as u8).map(|i| hit(0, 36 + i, 127)).collect();
    let p = peak(&render(&mut kit2(&pads), 96_000, 512, &events));
    eprintln!("pulsus: eight voices at once at 127 peak at {p:.3}");
    // Eight different drums on one sample at full velocity is not a groove, and the float
    // path does not clip; it is held so a change that makes the kit hotter shows up.
    assert!(p < 1.2, "every voice at once peaks at {p}");
    // What lands together in a groove — kick, snare, hat, crash — keeps real headroom.
    let groove: Vec<Event> = [0u8, 1, 2, 3].iter().map(|i| hit(0, 36 + i, 127)).collect();
    let p = peak(&render(&mut kit2(&pads[..4]), 96_000, 512, &groove));
    assert!(p < 0.95, "kick, snare, hat and cymbal at once peak at {p}");
    for (name, pad) in all_models() {
        let solo = peak(&render(&mut kit2(&[pad]), 48_000, 512, &[hit(0, 36, 127)]));
        assert!(solo < 0.6, "{name} alone peaks at {solo}");
    }
}

/// Drive at its maximum lifts the tail by no more than +12 dB (the small-signal gain of
/// `k = 4`), in every mode.
#[test]
fn full_drive_lifts_the_tail_twelve_decibels_at_most() {
    let base = PadSettings {
        decay: 1.0,
        click: 0.0,
        ..PadSettings::of(DrumVoice::Kick, 0)
    };
    let tail = |drive: f32, mode: u8| {
        let pad = PadSettings {
            drive,
            drive_mode: mode,
            ..base
        };
        let out = render(&mut kit2(&[pad]), 48_000, 512, &[hit(0, 36, 127)]);
        rms(&out[2 * 36_000..2 * 44_000])
    };
    let clean = tail(0.0, 0);
    for mode in 0..4u8 {
        let db = 20.0 * (tail(1.0, mode) / clean).log10();
        assert!(
            db <= 12.1 && db > 3.0,
            "mode {mode}: the tail rose {db:.2} dB"
        );
    }
}

/// In-place radix-2 FFT, for the aliasing measurement.
fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -std::f64::consts::TAU / len as f64;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (s, c) = (ang * k as f64).sin_cos();
                let (a, b) = (start + k, start + k + len / 2);
                let tr = re[b] * c - im[b] * s;
                let ti = re[b] * s + im[b] * c;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
}

/// Energy away from the harmonics of `f0` (below 20 kHz) against the total, in dB: what
/// folded back from above Nyquist.
fn alias_floor(x: &[f32], f0: f32) -> f32 {
    let n = x.len();
    // Blackman–Harris (−92 dB side lobes), so the window's own leakage is not counted.
    let bh = |i: usize| {
        let a = std::f64::consts::TAU * i as f64 / n as f64;
        0.35875 - 0.48829 * a.cos() + 0.14128 * (2.0 * a).cos() - 0.01168 * (3.0 * a).cos()
    };
    let mut re: Vec<f64> = x
        .iter()
        .enumerate()
        .map(|(i, v)| *v as f64 * bh(i))
        .collect();
    let mut im = vec![0.0; n];
    fft(&mut re, &mut im);
    let bin = SR as f64 / n as f64;
    let (mut total, mut alias) = (0.0f64, 0.0f64);
    for k in 1..n / 2 {
        let f = k as f64 * bin;
        if f > 20_000.0 {
            break;
        }
        let p = re[k] * re[k] + im[k] * im[k];
        total += p;
        let h = (f / f0 as f64).round();
        if (f - h * f0 as f64).abs() > 6.0 * bin {
            alias += p;
        }
    }
    (10.0 * (alias / total).log10()) as f32
}

/// The drive's 2× oversampling, measured where it matters: a 4.99 kHz tone driven fully
/// makes harmonics at 15, 25, 35… kHz; unoversampled, 25 and 35 fold back to 23 and
/// 13 kHz. Soft, tape and clip are oversampled; raw is not, on purpose.
#[test]
fn oversampled_drive_aliases_far_less_than_raw() {
    use super::shape::{half_band_taps, shape, Oversampler};
    let taps = half_band_taps();
    let f0 = 4_987.5f32;
    let driven = |mode: u8, oversample: bool| {
        let mut os = Oversampler::default();
        let y: Vec<f32> = (0..16_384 + 256)
            .map(|n| {
                let x = 0.9 * (std::f32::consts::TAU * f0 * n as f32 / SR).sin();
                if oversample {
                    os.process(x, 1.0, mode, &taps)
                } else {
                    shape(x, 1.0, mode)
                }
            })
            .collect();
        alias_floor(&y[256..], f0)
    };
    for mode in 0..3u8 {
        // Each curve against itself unoversampled (raw is mode 0's curve).
        let raw = driven(mode, false);
        let over = driven(mode, true);
        eprintln!("pulsus drive alias floor: mode {mode} {over:.1} dB, unoversampled {raw:.1} dB");
        // A hard clip's harmonics fall slowly, so 2× buys it less; the smooth curves
        // gain more than 30 dB.
        let gain = if mode == 2 { 10.0 } else { 30.0 };
        assert!(
            over < raw - gain,
            "mode {mode}: oversampled {over:.1} dB against {raw:.1} dB"
        );
    }
}

/// A pad on bus 2 renders only through bus 2, and the buses together are the kit: the
/// hook the engine's per-pad outputs need.
#[test]
fn the_buses_together_are_the_kit() {
    let mut pads: Vec<PadSettings> = all_models().into_iter().map(|(_, p)| p).take(6).collect();
    for (i, p) in pads.iter_mut().enumerate() {
        p.bus = (i % 3) as u8;
    }
    let events: Vec<Event> = (0..6u8)
        .map(|i| hit(i as usize * 1_000, 36 + i, 110))
        .collect();
    let whole = render(&mut kit2(&pads), 20_000, 256, &events);
    let mut k = kit2(&pads);
    let mut sum = vec![0.0f32; 40_000];
    let mut done = 0;
    while done < 20_000 {
        let mut end = (done + 256).min(20_000);
        for (at, pitch, vel, tick, id) in &events {
            if *at == done {
                k.trigger(
                    *pitch,
                    *vel,
                    Hit {
                        id: *id,
                        tick: *tick,
                        ordinal: 0,
                        sequenced: true,
                    },
                );
            } else if *at > done && *at < end {
                end = *at;
            }
        }
        for bus in 0..3u8 {
            let mut l = vec![0.0; end - done];
            let mut r = vec![0.0; end - done];
            k.process_add_bus(Some(bus), &mut l, &mut r);
            for i in 0..end - done {
                sum[2 * (done + i)] += l[i];
                sum[2 * (done + i) + 1] += r[i];
            }
        }
        done = end;
    }
    assert_null(&whole, &sum, 1e-6, "buses summed");
}

/// A generous CPU ceiling for a full pool: 48 voices of the heaviest models (32-mode
/// cymbals and toms, oversampled drive) for two seconds of audio.
#[test]
fn a_full_pool_renders_within_budget() {
    let heavy = PadSettings {
        decay: 3.0,
        modes: 32,
        drive: 0.5,
        ..PadSettings::of(DrumVoice::Tom, 0)
    };
    let cymbal = PadSettings {
        decay: 3.0,
        drive: 0.5,
        ..PadSettings::of(DrumVoice::HiHat, 3)
    };
    let events: Vec<Event> = (0..48)
        .map(|i| hit(i * 10, 36 + (i % 2) as u8, 100))
        .collect();
    // Timed against Pulsus I's own kick and snare rendered the same way, back to back, so a
    // machine shared by a dozen builds slows both: the ratio is the cost of Pulsus II's
    // heaviest voices in units of Pulsus I's.
    let time = |pads: &[PadSettings]| {
        let mut k = kit2(pads);
        let start = std::time::Instant::now();
        render(&mut k, 96_000, 512, &events);
        start.elapsed().as_secs_f32()
    };
    let light = time(&[PadSettings::kick(), PadSettings::snare()]).max(1e-4);
    let secs = time(&[heavy, cymbal]);
    let ratio = secs / light;
    eprintln!(
        "pulsus: 48 heavy voices, 2 s of audio in {secs:.3} s ({:.2} % of real time per          voice), {ratio:.1}x Pulsus I's kick and snare",
        secs / 2.0 / 48.0 * 100.0
    );
    assert!(ratio < 40.0, "the heavy voices cost {ratio:.1}x Pulsus I's");
    // And an absolute ceiling generous enough for a loaded machine.
    assert!(
        secs < 60.0,
        "48 heavy voices took {secs:.2} s for 2 s of audio"
    );
}
