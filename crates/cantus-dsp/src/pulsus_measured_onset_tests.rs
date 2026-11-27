//! Model 6's rules (r42:tympanum-real, ADR 0124's seventh amendment): a crash plays it as
//! model 5 to the bit, a hat's split pairs ring together; a drum's hit comes in through the stick's contact (its first
//! milliseconds quieter than model 5's, its waveform's peak later); the same bits however the
//! block is cut; every drum comes to rest; each drum grows with the velocity at every step for
//! every seed (one hit's own draw never cancels its level); a drum struck while it rings never
//! comes out quieter than struck at rest.

use super::*;

/// `drums()` playing model `gen`.
fn drums_gen(gen: u8) -> Pulsus {
    let mut p = drums();
    p.measured_gen = gen;
    p
}

/// One hit of table `table` at `vel` (seed `id`) through model `gen`: the first `n` samples.
fn hit(table: u8, vel: u8, id: u64, gen: u8, n: usize) -> Vec<f32> {
    let mut p = Pulsus::new(SR);
    p.enable_measured();
    p.measured_gen = gen;
    p.set_kit(3, 9, 4, false);
    p.set_pad_count(1);
    p.set_pad(0, 60, &pad(table, 1, 0));
    p.note_on(60, vel, id);
    let mut out = vec![0.0f32; n];
    let mut at = 0;
    while at < n {
        let c = (n - at).min(512);
        p.process_buses(c, &mut |_, off, l, r| {
            for (i, (a, b)) in l.iter().zip(r).enumerate() {
                out[at + off + i] += 0.5 * (a + b);
            }
        });
        at += c;
    }
    out
}

fn db(x: &[f32]) -> f32 {
    20.0 * crate::math::log10(rms(x).max(1e-10))
}

const DRUMS: [u8; 10] = [4, 5, 6, 7, 8, 20, 21, 22, 23, 24];

/// A crash (a cymbal, not a hat) plays model 6 as model 5, to the bit.
#[test]
fn a_crash_plays_model_6_as_model_5_to_the_bit() {
    let ev: Vec<(usize, u8, u8)> = GROOVE.iter().copied().filter(|e| e.1 == 49).collect();
    let five = play(&mut kit(true), &ev, 60_000, 333);
    let mut six = kit(true);
    six.measured_gen = 6;
    assert!(rms(&five) > 1e-4);
    assert!(play(&mut six, &ev, 60_000, 333) == five);
}

/// The energy of `x` between `lo` and `hi` Hz (a plain DFT every 10 Hz).
fn band(x: &[f32], lo: f32, hi: f32) -> f32 {
    let mut e = 0.0;
    let mut f = lo;
    while f <= hi {
        let w = std::f32::consts::TAU * f / SR;
        let (mut re, mut im) = (0.0f32, 0.0f32);
        for (n, v) in x.iter().enumerate() {
            re += v * crate::math::cos(w * n as f32);
            im += v * crate::math::sin(w * n as f32);
        }
        e += re * re + im * im;
        f += 10.0;
    }
    e
}

/// A hat's knock (r42:tympanum-real): model 5's, 2 ms of drawn noise, set the soft closed
/// hat's lows (under 1.6 kHz) anew each stroke; model 6's is the stick's push, the same every
/// stroke, and the band's level holds from one stroke to the next.
#[test]
fn a_soft_closed_hats_lows_hold_from_stroke_to_stroke_in_model_6() {
    let spread = |gen: u8| {
        let lv: Vec<f32> = (1..=8u64)
            .map(|id| {
                let x = hit(18, 32, id, gen, 2_400);
                10.0 * crate::math::log10(band(&x, 300.0, 1_500.0).max(1e-30))
            })
            .collect();
        let (lo, hi) = lv
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
        hi - lo
    };
    let (five, six) = (spread(5), spread(6));
    assert!(
        six < 0.5 * five,
        "model 6 {six:.2} dB against model 5 {five:.2}"
    );
}

#[test]
fn a_model_6_drum_renders_the_same_bits_however_cut_and_comes_to_rest() {
    let ev = [
        (0, 36, 110),
        (6_000, 38, 100),
        (6_020, 38, 40),
        (12_000, 40, 120),
        (18_000, 37, 90),
        (24_000, 45, 127),
        (30_000, 36, 60),
        (30_010, 45, 20),
    ];
    let base = play(&mut drums_gen(6), &ev, 72_000, 512);
    assert!(rms(&base) > 1e-4);
    assert!(base != play(&mut drums_gen(5), &ev, 72_000, 512));
    for block in [1, 37, 64, 256, 333] {
        assert!(
            play(&mut drums_gen(6), &ev, 72_000, block) == base,
            "block {block}"
        );
    }
    let mut p = drums_gen(6);
    play(&mut p, &ev, 72_000, 512);
    for _ in 0..(40.0 * SR) as usize / 4_096 {
        p.process_buses(4_096, &mut |_, _, _, _| {});
    }
    assert!(!p.measured.as_deref().unwrap().any(), "at rest");
}

/// The stick's contact: over every drum of both kits, a model 6 hit's first 10 ms are less
/// peaked than model 5's (its crest — peak over RMS — lower: the modes pushed in over the
/// contact, the knock a push and no longer a burst of noise), the same hits, the same seeds.
#[test]
fn a_model_6_drum_comes_in_through_the_contact() {
    let crest = |x: &[f32]| {
        let w = &x[..480];
        let pk = w.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        20.0 * crate::math::log10(pk / rms(w).max(1e-12))
    };
    let (mut c5, mut c6) = (0.0, 0.0);
    for table in DRUMS {
        for id in 1..=4u64 {
            let five = hit(table, 110, id, 5, 2_400);
            let six = hit(table, 110, id, 6, 2_400);
            let first = |x: &[f32]| x.iter().position(|v| v.abs() > 1e-7).unwrap_or(0);
            c5 += crest(&five[first(&five)..]);
            c6 += crest(&six[first(&six)..]);
        }
    }
    let n = (DRUMS.len() * 4) as f32;
    assert!(
        c6 / n < c5 / n - 0.3,
        "first 10 ms crest: model 6 {:.2} dB against model 5 {:.2}",
        c6 / n,
        c5 / n
    );
}

/// Model 6's level is the force's: every drum grows with the velocity at every step a
/// drummer plays, **for each seed alone** (model 5's blend could cancel one hit's ring: the
/// studio kick at 96 3.3 dB under its 80), and spans at least 12 dB from 16 to 127.
#[test]
fn every_model_6_drum_grows_with_the_velocity_for_every_seed() {
    const VELS: [u8; 10] = [16, 32, 48, 56, 64, 80, 96, 104, 112, 127];
    for table in DRUMS {
        for id in 1..=5u64 {
            let lv: Vec<f32> = VELS
                .iter()
                .map(|&v| db(&hit(table, v, id, 6, 3_840)))
                .collect();
            for (j, w) in lv.windows(2).enumerate() {
                assert!(
                    w[1] > w[0] - 0.5,
                    "table {table} seed {id}: {} → {} falls ({lv:?})",
                    VELS[j],
                    VELS[j + 1]
                );
            }
            assert!(lv[9] - lv[0] >= 12.0, "table {table} seed {id}: {lv:?}");
        }
    }
}

/// `hits` (frame, key, velocity, tick) as sequenced notes — each hit's seed its own note's,
/// whatever else was struck — in blocks of 512: the summed lanes.
fn play_seq(p: &mut Pulsus, hits: &[(usize, u8, u8, i64)], n: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; n];
    let (mut at, mut e) = (0, 0);
    while at < n {
        while e < hits.len() && hits[e].0 <= at {
            let (_, key, vel, tick) = hits[e];
            p.trigger(
                key,
                vel,
                super::super::super::Hit {
                    id: tick as u64,
                    tick,
                    ordinal: 0,
                    sequenced: true,
                },
            );
            e += 1;
        }
        let next = hits.get(e).map_or(n, |x| x.0.max(at + 1));
        let c = (n - at).min(512).min(next - at);
        p.process_buses(c, &mut |_, off, l, r| {
            for (i, (a, b)) in l.iter().zip(r).enumerate() {
                out[at + off + i] += 0.5 * (a + b);
            }
        });
        at += c;
    }
    out
}

/// Struck while it rings, a model 6 drum takes the stick's push on top of its moving head
/// (no mode left weaker than the stroke alone would leave it): on average over when it lands
/// (100 ms to 1 s after the last hit) it comes out no quieter than the same hit (the same seed)
/// at rest — the kick at 80 then 96, the snare and a tom likewise. One landing may still come
/// out a few dB under (a tom 100 ms on: the ring's phase against the new push), never 6.
#[test]
fn a_model_6_drum_struck_while_it_rings_never_loses_level_on_average() {
    for (key, table) in [(36u8, 4u8), (38, 5), (45, 6)] {
        let mut diffs = Vec::new();
        for k in 0..10usize {
            let gap = 4_800 + k * 4_800;
            let second = (gap, key, 96, 7_000 + k as i64);
            let two = play_seq(&mut drums_gen(6), &[(0, key, 80, 1), second], gap + 3_840);
            let alone = play_seq(&mut drums_gen(6), &[second], gap + 3_840);
            diffs.push(db(&two[gap..]) - db(&alone[gap..]));
        }
        let mean = diffs.iter().sum::<f32>() / diffs.len() as f32;
        eprintln!("table {table}: {mean:.2} dB on average ({diffs:?})");
        assert!(diffs.iter().all(|d| *d > -6.0), "table {table}: {diffs:?}");
        assert!(
            mean > -0.5,
            "table {table}: {mean:.2} dB on average ({diffs:?})"
        );
    }
}
