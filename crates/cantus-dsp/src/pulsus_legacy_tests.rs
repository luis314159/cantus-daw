//! Pulsus I's own tests, kept as they were: a kit built with `Pulsus::new` is model 1,
//! so these hold the old kit to the letter while Pulsus II grows beside it.

use super::shape::saturate;
use super::*;
use crate::nulltest::{assert_null, peak, rms};

const KICK: u8 = 36;
const SNARE: u8 = 38;
const CLOSED: u8 = 42;
const OPEN: u8 = 46;

/// A kit laid out on the General MIDI pitches, with the hats sharing a choke group.
fn kit(sample_rate: f32) -> Pulsus {
    let mut k = Pulsus::new(sample_rate);
    let mut closed = PadSettings::hat(0.06);
    closed.choke = 1;
    let mut open = PadSettings::hat(0.45);
    open.choke = 1;
    k.set_pad_count(4);
    k.set_pad(0, KICK, &PadSettings::kick());
    k.set_pad(1, SNARE, &PadSettings::snare());
    k.set_pad(2, CLOSED, &closed);
    k.set_pad(3, OPEN, &open);
    k
}

/// Render `n` frames in `block`-sized chunks, hitting `pattern` at the given frame.
fn render(k: &mut Pulsus, n: usize, block: usize, pattern: &[(usize, u8, u8)]) -> Vec<f32> {
    let mut l = vec![0.0; n];
    let mut r = vec![0.0; n];
    let mut done = 0;
    while done < n {
        // A hit that falls inside this block splits it, exactly as the processor's
        // event loop does — which is what makes the block-size test mean anything.
        let mut end = (done + block).min(n);
        for (at, pitch, vel) in pattern {
            if *at == done {
                k.note_on(*pitch, *vel, 0);
            } else if *at > done && *at < end {
                end = *at;
            }
        }
        k.process_add(&mut l[done..end], &mut r[done..end]);
        done = end;
    }
    // Interleave, so a difference in either channel shows up.
    l.into_iter().zip(r).flat_map(|(a, b)| [a, b]).collect()
}

fn beat() -> Vec<(usize, u8, u8)> {
    vec![
        (0, KICK, 127),
        (0, CLOSED, 90),
        (6_000, CLOSED, 70),
        (12_000, SNARE, 110),
        (12_000, OPEN, 100),
        (18_000, CLOSED, 80),
        (24_000, KICK, 120),
    ]
}

#[test]
fn every_voice_sounds_and_falls_to_exact_silence() {
    for (pitch, name) in [(KICK, "kick"), (SNARE, "snare"), (OPEN, "open hat")] {
        let mut k = kit(48_000.0);
        let out = render(&mut k, 96_000, 512, &[(0, pitch, 127)]);
        let p = peak(&out);
        assert!(p > 0.1, "{name} is inaudible: peak {p}");
        assert!(p < 1.0, "{name} clips: peak {p}");
        assert!(out.iter().all(|x| x.is_finite()), "{name} produced NaN/inf");
        assert_eq!(k.active_voices(), 0, "{name} never released its voice");
        // Not "small": zero. A one-shot that only asymptotes leaves denormals behind.
        assert_eq!(out[out.len() - 1], 0.0, "{name} never reached silence");
    }
}

/// The guardrail every DSP module gets before it lands: where the host chose to split
/// the buffer must not be audible.
#[test]
fn pulsus_is_block_size_independent() {
    let reference = render(&mut kit(48_000.0), 48_000, 512, &beat());
    assert!(peak(&reference) > 0.1, "silent reference");
    for block in [1usize, 7, 64, 480, 1_000, 4_096, 8_192] {
        let out = render(&mut kit(48_000.0), 48_000, block, &beat());
        assert_null(&reference, &out, 0.0, &format!("pulsus block {block}"));
    }
}

#[test]
fn pulsus_is_deterministic() {
    let a = render(&mut kit(44_100.0), 40_000, 256, &beat());
    let b = render(&mut kit(44_100.0), 40_000, 256, &beat());
    assert_null(&a, &b, 0.0, "pulsus repeat render");
}

/// The one that says this is a kit and not two unrelated noises.
///
/// The voices sum linearly, so the open hat's *own* contribution after the choke is
/// exactly `(open + closed) − closed`. Comparing that against the same window of an
/// unchoked open hat is a measurement of the choke itself, not of how loud the closed
/// hat happens to be — and it fails outright if `choke` is ignored.
#[test]
fn a_closed_hat_chokes_an_open_one() {
    const CHOKE_AT: usize = 4_800; // 100 ms in.
    let window = CHOKE_AT * 2 + 960..CHOKE_AT * 2 + 19_200; // interleaved: ×2.

    let alone = render(&mut kit(48_000.0), 48_000, 256, &[(0, OPEN, 110)]);
    let both = render(
        &mut kit(48_000.0),
        48_000,
        256,
        &[(0, OPEN, 110), (CHOKE_AT, CLOSED, 110)],
    );
    let closed_only = render(&mut kit(48_000.0), 48_000, 256, &[(CHOKE_AT, CLOSED, 110)]);
    let residue: Vec<f32> = both.iter().zip(&closed_only).map(|(a, b)| a - b).collect();

    let free = rms(&alone[window.clone()]);
    let choked = rms(&residue[window]);
    assert!(free > 1e-4, "the unchoked open hat is silent to begin with");
    assert!(
        choked < free * 0.01,
        "the closed hat did not choke the open one: {choked} against {free}"
    );
}

/// A pad with no choke group must not be cut by anything, or every kick would be
/// killed by the next hat.
#[test]
fn a_pad_without_a_choke_group_is_never_choked() {
    let kick_alone = render(&mut kit(48_000.0), 48_000, 256, &[(0, KICK, 120)]);
    let with_hats = render(
        &mut kit(48_000.0),
        48_000,
        256,
        &[(0, KICK, 120), (2_400, CLOSED, 100), (4_800, OPEN, 100)],
    );
    let hats_only = render(
        &mut kit(48_000.0),
        48_000,
        256,
        &[(2_400, CLOSED, 100), (4_800, OPEN, 100)],
    );
    let residue: Vec<f32> = with_hats
        .iter()
        .zip(&hats_only)
        .map(|(a, b)| a - b)
        .collect();
    assert_null(&kick_alone, &residue, 1e-5, "kick under hats");
}

#[test]
fn a_pitch_with_no_pad_makes_no_sound() {
    let mut k = kit(48_000.0);
    let out = render(&mut k, 4_800, 256, &[(0, 60, 127)]);
    assert_eq!(k.active_voices(), 0);
    assert_eq!(peak(&out), 0.0, "a melodic note woke the kit up");
}

#[test]
fn a_pad_at_the_centre_is_identical_in_both_channels() {
    let mut k = kit(48_000.0);
    let out = render(&mut k, 24_000, 256, &[(0, SNARE, 100)]);
    for pair in out.chunks_exact(2) {
        assert_eq!(pair[0], pair[1], "a centred pad is not centred");
    }
}

#[test]
fn stealing_voices_never_panics_or_leaks() {
    let mut k = kit(48_000.0);
    for i in 0..(MAX_DRUM_VOICES * 4) {
        k.note_on([KICK, SNARE, OPEN][i % 3], 100, i as u64);
        let mut l = [0.0; 32];
        let mut r = [0.0; 32];
        k.process_add(&mut l, &mut r);
        assert!(l.iter().all(|x| x.is_finite()));
    }
    assert!(k.active_voices() <= MAX_DRUM_VOICES);
    let out = render(&mut k, 192_000, 1_024, &[]);
    assert_eq!(k.active_voices(), 0, "voices never freed");
    assert_eq!(out[out.len() - 1], 0.0);
}

/// A kit is a dozen sources landing on one channel, so the whole thing hit at once at
/// full velocity has to leave headroom. Without this the kick alone reached 0 dBFS and
/// every busy bar arrived at the fader already clipped — which is a mixing problem
/// nobody can fix downstream.
#[test]
fn the_whole_kit_at_full_velocity_leaves_headroom() {
    let mut k = kit(48_000.0);
    let hits: Vec<(usize, u8, u8)> = [KICK, SNARE, CLOSED, OPEN]
        .iter()
        .map(|p| (0usize, *p, 127u8))
        .collect();
    let out = render(&mut k, 96_000, 512, &hits);
    let p = peak(&out);
    assert!(
        p < 0.95,
        "the kit clips when everything lands together: {p}"
    );
    assert!(p > 0.25, "the kit is too quiet to be usable: {p}");

    // And one pad on its own leaves considerably more, so a mix has somewhere to go.
    let loudest = peak(&render(&mut kit(48_000.0), 96_000, 512, &[(0, KICK, 127)]));
    assert!(
        loudest < 0.6,
        "the loudest pad alone is at {loudest}: no headroom for the rest of the kit"
    );
}

/// A decaying sine at 44.1 kHz — not the engine's rate, so resampling is exercised —
/// long enough to outlast every block size in the test.
fn test_sample(channels: usize) -> SampleRef {
    let rate = 44_100.0f32;
    let n = 22_050;
    let mut frames = Vec::with_capacity(n * channels);
    for i in 0..n {
        let t = i as f32 / rate;
        let env = (-t * 6.0).exp();
        let l = (std::f32::consts::TAU * 220.0 * t).sin() * env * 0.8;
        frames.push(l);
        if channels == 2 {
            // A different note on the right, so a stereo file's channels are told apart.
            frames.push((std::f32::consts::TAU * 330.0 * t).sin() * env * 0.8);
        }
    }
    SampleRef {
        frames: frames.into(),
        rate,
        channels,
    }
}

fn sampler(channels: usize, tune: f32) -> Pulsus {
    let mut k = Pulsus::new(48_000.0);
    let mut pad = PadSettings::kick();
    pad.voice = DrumVoice::Sample;
    pad.tune = tune;
    pad.decay = 4.0;
    k.set_pad_count(1);
    k.set_pad(0, 60, &pad);
    k.set_sample(0, Some(test_sample(channels)));
    k
}

#[test]
fn a_sample_pad_plays_its_file_and_is_block_size_independent() {
    let reference = render(&mut sampler(1, 0.0), 40_000, 512, &[(0, 60, 127)]);
    let p = peak(&reference);
    assert!(p > 0.5 && p <= 0.8, "sample level is off: {p}");
    for block in [1usize, 7, 64, 480, 4_096] {
        let out = render(&mut sampler(1, 0.0), 40_000, block, &[(0, 60, 127)]);
        assert_null(&reference, &out, 0.0, &format!("sample at block {block}"));
    }
}

/// Tuning a sample up an octave plays it twice as fast, so it ends in half the time.
#[test]
fn tuning_a_sample_changes_its_speed() {
    let last = |tune: f32| {
        let out = render(&mut sampler(1, tune), 60_000, 256, &[(0, 60, 127)]);
        out.iter().rposition(|x| *x != 0.0).unwrap_or(0) / 2
    };
    let normal = last(0.0);
    let octave_up = last(12.0);
    // 22 050 frames at 44.1 kHz is 24 000 output frames at 48 kHz.
    assert!(
        (normal as i64 - 24_000).abs() < 50,
        "untuned length {normal}"
    );
    assert!(
        (octave_up as i64 - 12_000).abs() < 50,
        "an octave up should halve the length: {octave_up}"
    );
}

#[test]
fn a_stereo_sample_keeps_its_channels_apart_and_a_missing_one_is_silent() {
    let out = render(&mut sampler(2, 0.0), 24_000, 256, &[(0, 60, 127)]);
    let differ = out.chunks_exact(2).filter(|f| f[0] != f[1]).count();
    assert!(differ > 1_000, "a stereo sample came out mono");

    let mut k = sampler(1, 0.0);
    k.set_sample(0, None);
    let out = render(&mut k, 4_800, 256, &[(0, 60, 127)]);
    assert_eq!(peak(&out), 0.0, "a pad with no file made a sound");
    assert_eq!(k.active_voices(), 0);
}

/// A sample obeys the pad's choke group like any other voice: a long sample cut by a
/// pad in the same group stops.
#[test]
fn a_sample_is_choked_like_any_other_pad() {
    let build = || {
        let mut k = sampler(1, 0.0);
        k.set_pad_count(2);
        let mut long = PadSettings::kick();
        long.voice = DrumVoice::Sample;
        long.decay = 4.0;
        long.choke = 2;
        k.set_pad(0, 60, &long);
        let mut cutter = PadSettings::hat(0.03);
        cutter.choke = 2;
        k.set_pad(1, 61, &cutter);
        k
    };
    let alone = render(&mut build(), 48_000, 256, &[(0, 60, 127)]);
    let both = render(&mut build(), 48_000, 256, &[(0, 60, 127), (4_800, 61, 100)]);
    let cutter_only = render(&mut build(), 48_000, 256, &[(4_800, 61, 100)]);
    let residue: Vec<f32> = both.iter().zip(&cutter_only).map(|(a, b)| a - b).collect();
    let window = 2 * 4_800 + 960..2 * 4_800 + 19_200;
    assert!(rms(&residue[window.clone()]) < rms(&alone[window]) * 0.01);
}

#[test]
fn drive_at_zero_is_exactly_the_identity() {
    for x in [-1.0f32, -0.3, 0.0, 1e-9, 0.7, 1.0] {
        assert_eq!(saturate(x, 0.0), x, "drive 0 changed {x}");
    }
    assert!(saturate(1.0, 1.0) <= 1.0);
    assert!(saturate(0.2, 1.0) > 0.2, "drive did not raise the body");
}
