use super::*;
use crate::tympanum::{Outputs, LANE};

const FS: f32 = 48_000.0;

/// A file of `frames` frames at `value`, mono.
fn file(value: f32, frames: usize) -> Source {
    let mut s = Source {
        channels: 1,
        rate: FS,
        frames,
        ..Default::default()
    };
    s.levels[0] = Some(vec![value; frames].into());
    s
}

/// One articulation on `key`, one layer, `takes` takes, `mics` mics: take t's file for mic
/// m is file `t * mics + m`.
fn art(key: u8, takes: usize, mics: usize, openness: Option<f32>, base: u32) -> RecArt {
    RecArt {
        piece: 0,
        key,
        openness,
        layers: vec![RecLayer {
            lo: 1,
            hi: 127,
            gain: [1.0; 128],
            takes: (0..takes)
                .map(|t| RecTake {
                    mics: (0..mics)
                        .map(|m| RecMicTake {
                            mic: m as u8,
                            file: base + (t * mics + m) as u32,
                            gain: 1.0,
                            pan: 0.0,
                            offset: 0,
                            cents: 0.0,
                            highpass: 0.0,
                        })
                        .collect(),
                })
                .collect(),
        }],
        chokes: Vec::new(),
        choke_s: 0.01,
        polyphony: 0,
    }
}

fn render(e: &mut RecEngine, outs: &mut Outputs, frames: usize, chunk: usize) -> Vec<Vec<f32>> {
    let mut lanes = vec![Vec::new(); outs.count];
    let mut done = 0;
    while done < frames {
        let c = chunk.min(frames - done).min(LANE);
        outs.begin(c);
        e.render(c, outs);
        for (m, lane) in lanes.iter_mut().enumerate() {
            let (l, _) = outs.lane(m, c);
            lane.extend_from_slice(l);
        }
        done += c;
    }
    lanes
}

#[test]
fn every_mic_of_a_hit_plays_the_same_take_and_takes_never_repeat() {
    let (takes, mics) = (4usize, 3usize);
    let kit = Arc::new(RecKit {
        mics,
        arts: vec![art(38, takes, mics, None, 0)],
        pedal: None,
    });
    // Take t's files all read t + 1: every mic of one hit must read the same.
    let src = |i: u32| Some(file((i as usize / mics + 1) as f32, 4_000));
    let mut last = 0.0;
    for ordinal in 0..40u32 {
        let mut e = RecEngine::new(FS);
        e.set_kit(Some(&kit));
        e.seed = 9;
        let mut outs = Outputs::new(FS, 0);
        outs.count = mics;
        e.note_on(38, 100, 1, ordinal, &src);
        let lanes = render(&mut e, &mut outs, 64, 64);
        let first = lanes[0][10];
        assert!(first > 0.0);
        for lane in &lanes {
            assert_eq!(lane[10].to_bits(), first.to_bits(), "ordinal {ordinal}");
        }
        assert_ne!(first, last, "a take twice running at ordinal {ordinal}");
        last = first;
    }
}

#[test]
fn a_strike_renders_the_same_bits_whatever_the_chunks() {
    let kit = Arc::new(RecKit {
        mics: 2,
        arts: vec![art(36, 2, 2, None, 0), art(38, 2, 2, None, 4)],
        pedal: None,
    });
    let decay = |i: u32| {
        let mut s = file(0.0, 3_000);
        let v: Vec<f32> = (0..3_000)
            .map(|t| {
                crate::math::sin(t as f32 * 0.01 * (i + 1) as f32)
                    * crate::math::exp(-(t as f32) / 900.0)
            })
            .collect();
        s.levels[0] = Some(v.into());
        Some(s)
    };
    let run = |chunk: usize| {
        let mut e = RecEngine::new(FS);
        e.set_kit(Some(&kit));
        let mut outs = Outputs::new(FS, 0);
        outs.count = 2;
        e.note_on(36, 90, 1, 0, &decay);
        let mut a = render(&mut e, &mut outs, 700, chunk);
        e.note_on(38, 60, 2, 0, &decay);
        let b = render(&mut e, &mut outs, 3_000, chunk);
        a[0].extend_from_slice(&b[0]);
        a[1].extend_from_slice(&b[1]);
        a
    };
    let whole = run(256);
    for chunk in [1usize, 37, 64, 255] {
        assert_eq!(run(chunk), whole, "chunks of {chunk}");
    }
}

#[test]
fn a_hat_between_two_levels_plays_both_and_follows_the_pedal() {
    let kit = Arc::new(RecKit {
        mics: 1,
        arts: vec![
            art(46, 1, 1, Some(0.2), 0),
            art(46, 1, 1, Some(0.8), 1),
            art(42, 1, 1, Some(0.0), 0),
        ],
        pedal: None,
    });
    // The closed level reads 1, the open one 3.
    let src = |i: u32| Some(file(if i == 0 { 1.0 } else { 3.0 }, 48_000));
    let mut e = RecEngine::new(FS);
    e.set_kit(Some(&kit));
    let mut outs = Outputs::new(FS, 0);
    outs.count = 1;
    e.pedal(0.5, &src);
    e.note_on(46, 127, 1, 0, &src);
    let g = std::f32::consts::FRAC_1_SQRT_2;
    let lanes = render(&mut e, &mut outs, 256, 256);
    // Halfway: half of each, 2 × the centre's law.
    assert!((lanes[0][100] - 2.0 * g).abs() < 1e-5, "{}", lanes[0][100]);
    // The pedal opens fully: the crossfade follows to the open level.
    e.pedal(0.8, &src);
    let lanes = render(&mut e, &mut outs, 4_096, 256);
    assert!(
        (lanes[0][4_000] - 3.0 * g).abs() < 1e-5,
        "{}",
        lanes[0][4_000]
    );
    // A closed strike silences it (it rings more open than the strike).
    e.note_on(42, 100, 2, 0, &src);
    assert_eq!(e.active_voices(), 2);
    let _ = render(&mut e, &mut outs, 2_000, 256);
    assert_eq!(e.active_voices(), 1, "the open hat was choked");
}

/// fix-e: a kit of sixteen mics struck faster than its plays last (a crash ringing on
/// every mic, again and again): the oldest strikes are stolen — each over `STEAL_MS`, all
/// its mics together — and no mic ever steps by more than a fade's slope.
#[test]
fn stealing_fades_every_mic_of_the_victim_never_a_step() {
    let mics = 16usize;
    let kit = Arc::new(RecKit {
        mics,
        arts: vec![art(49, 1, mics, None, 0)],
        pedal: None,
    });
    // A strike rises over 10 ms and then rings at full for seconds: a cut is a full step.
    let frames = 200_000;
    let rise = 480;
    let mut x: Vec<f32> = (0..frames)
        .map(|t| (t as f32 / rise as f32).min(1.0))
        .collect();
    x[frames - 1] = 0.0;
    let mut src = Source {
        channels: 1,
        rate: FS,
        frames,
        ..Default::default()
    };
    src.levels[0] = Some(x.into());
    let source = |_: u32| Some(src.clone());
    let mut e = RecEngine::new(FS);
    e.set_kit(Some(&kit));
    let mut outs = Outputs::new(FS, 0);
    outs.count = mics;
    let mut lanes = vec![Vec::new(); mics];
    // 60 strikes, 400 frames apart: three times what the pool holds at once.
    for k in 0..60u32 {
        e.note_on(49, 127, u64::from(k) + 1, k, &source);
        for (m, l) in render(&mut e, &mut outs, 400, 400).into_iter().enumerate() {
            lanes[m].extend(l);
        }
    }
    assert!(e.active_voices() < 60, "voices were stolen");
    // A fade's slope (0.71 over 240 frames) and a rise's, with room for two at once.
    for (m, lane) in lanes.iter().enumerate() {
        let step = lane
            .windows(2)
            .fold(0.0f32, |s, w| s.max((w[1] - w[0]).abs()));
        assert!(step < 0.02, "mic {m} steps by {step}");
    }
}
