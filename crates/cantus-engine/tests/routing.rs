//! The output's routing (round 20, audio setup): the master on another pair, a headphone
//! cue, the test tone — and, first, the null test: with the defaults the routing touches
//! nothing, so what the processor wrote is what the device plays, bit for bit.

use cantus_engine::routing::{route, OutputRouting, Route, Tone, TONE_LEVEL};

/// A block of `frames` frames on `channels` channels with a master in 1–2 the way the
/// processor leaves it: noise-like values in channels 0 and 1, silence elsewhere.
fn block(frames: usize, channels: usize) -> Vec<f32> {
    let mut v = vec![0.0f32; frames * channels];
    let mut x = 0x1234_5678u32;
    for f in 0..frames {
        for c in 0..2.min(channels) {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            v[f * channels + c] = (x as f32 / u32::MAX as f32) * 2.0 - 1.0;
        }
    }
    v
}

#[test]
fn the_default_routing_leaves_the_output_bit_identical() {
    for channels in [1, 2, 4, 8] {
        let before = block(512, channels);
        let mut after = before.clone();
        let mut tone = Tone::default();
        route(&mut after, channels, 48_000.0, Route::default(), &mut tone);
        assert_eq!(
            before.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
            after.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
            "{channels} channels: the null routing must not touch a sample"
        );
    }
    // And the atomics start at that default.
    assert_eq!(OutputRouting::default().read(), Route::default());
}

#[test]
fn the_master_moves_to_the_main_pair() {
    let before = block(64, 4);
    let mut out = before.clone();
    let r = Route {
        main: 2,
        ..Default::default()
    };
    route(&mut out, 4, 48_000.0, r, &mut Tone::default());
    for f in 0..64 {
        assert_eq!(out[f * 4], 0.0);
        assert_eq!(out[f * 4 + 1], 0.0);
        assert_eq!(out[f * 4 + 2], before[f * 4]);
        assert_eq!(out[f * 4 + 3], before[f * 4 + 1]);
    }
    // A pair the device does not have plays on 1–2 (a device with fewer channels).
    let mut out = before.clone();
    let r = Route {
        main: 6,
        ..Default::default()
    };
    route(&mut out, 4, 48_000.0, r, &mut Tone::default());
    assert_eq!(out, before);
}

#[test]
fn the_cue_is_a_copy_of_the_master_at_its_level() {
    let before = block(64, 4);
    let mut out = before.clone();
    let r = Route {
        main: 0,
        cue: Some((2, 0.5)),
        tone: None,
    };
    route(&mut out, 4, 48_000.0, r, &mut Tone::default());
    for f in 0..64 {
        assert_eq!(out[f * 4], before[f * 4], "the main pair is untouched");
        assert_eq!(out[f * 4 + 1], before[f * 4 + 1]);
        assert_eq!(out[f * 4 + 2], before[f * 4] * 0.5);
        assert_eq!(out[f * 4 + 3], before[f * 4 + 1] * 0.5);
    }
    // Through the atomics: the gain is clamped, a cue off is off.
    let routing = OutputRouting::default();
    routing.set_cue(Some((2, 9.0)));
    assert_eq!(routing.read().cue, Some((2, 4.0)));
    routing.set_cue(Some((2, f32::NAN)));
    assert_eq!(routing.read().cue, Some((2, 0.0)));
    routing.set_cue(None);
    assert_eq!(routing.read().cue, None);
}

#[test]
fn the_test_tone_plays_on_its_pair_fades_in_and_out_and_only_there() {
    let rate = 48_000.0;
    let frames = 4_800;
    let mut out = vec![0.0f32; frames * 4];
    let mut tone = Tone::default();
    let on = Route {
        tone: Some(2),
        ..Default::default()
    };
    route(&mut out, 4, rate, on, &mut tone);
    let peak = |o: &[f32], c: usize| {
        o.chunks_exact(4)
            .map(|f| f[c].abs())
            .fold(0.0f32, f32::max)
    };
    assert_eq!(peak(&out, 0), 0.0, "nothing on 1–2");
    assert_eq!(peak(&out, 1), 0.0);
    let p = peak(&out, 2);
    assert!(
        (p - TONE_LEVEL).abs() < 1e-3,
        "−18 dBFS once faded in: {p}"
    );
    assert_eq!(out[2], 0.0, "the first sample starts from silence (no click)");
    assert!(out[4 * 10 + 2].abs() < TONE_LEVEL * 0.05, "a fade, not a step");
    assert!(tone.sounding());
    // Off: it fades out where it was, then stops.
    let mut out = vec![0.0f32; frames * 4];
    route(&mut out, 4, rate, Route::default(), &mut tone);
    assert!(!tone.sounding());
    let tail = &out[(frames - 100) * 4..];
    assert_eq!(peak(tail, 2), 0.0, "silent after the fade");
    assert!(peak(&out[..400], 2) > 0.0, "the fade-out is heard");
    // And then the routing is the null routing again.
    let before = block(256, 4);
    let mut after = before.clone();
    route(&mut after, 4, rate, Route::default(), &mut tone);
    assert_eq!(before, after);
}
