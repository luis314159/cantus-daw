//! Pulsus III's lanes (ADR 0100): a model 3 kit sums each pad on its own lane and the lanes
//! in pad order, so a kit whose pads play into one output and a kit whose pads each play
//! into their own (summed back in pad order, as the kit's strip gathers its pad strips)
//! are the same bits. Null tests first: these hold before any engine wiring.

use super::*;
use crate::nulltest::{assert_null, peak};

const SR: f32 = 48_000.0;

/// A hit: frame, pitch, velocity, tick, id.
type Event = (usize, u8, u8, i64, u64);

fn hit(at: usize, pitch: u8, vel: u8) -> Event {
    (at, pitch, vel, at as i64, (at as u64) << 8 | pitch as u64)
}

/// Eight pads of every family, three of them polyphonic enough to ring over themselves.
fn pads() -> Vec<PadSettings> {
    let mut v = vec![
        PadSettings::of(DrumVoice::Kick, 1),
        PadSettings::of(DrumVoice::Snare, 1),
        PadSettings::of(DrumVoice::HiHat, 0),
        PadSettings::of(DrumVoice::HiHat, 1),
        PadSettings::of(DrumVoice::Clap, 0),
        PadSettings::of(DrumVoice::Tom, 0),
        PadSettings::of(DrumVoice::Kick, 2),
        PadSettings::of(DrumVoice::HiHat, 3),
    ];
    for (i, p) in v.iter_mut().enumerate() {
        p.pan = (i as f32 - 3.5) / 4.0;
        p.decay = p.decay.max(0.4);
    }
    v
}

/// A kit of `model` with `pads`, pad `i` on output `bus(i)`.
fn kit(model: u8, pads: &[PadSettings], bus: impl Fn(usize) -> u8) -> Pulsus {
    let mut k = Pulsus::new(SR);
    k.set_kit(model, 7, 4, false);
    k.set_pad_count(pads.len());
    for (i, p) in pads.iter().enumerate() {
        let mut p = *p;
        p.id = i as u16 + 1;
        p.bus = bus(i);
        k.set_pad(i, 36 + i as u8, &p);
    }
    k
}

/// A busy groove: every pad, overlapping itself and the others.
fn groove() -> Vec<Event> {
    let mut e = Vec::new();
    for step in 0..48usize {
        let at = step * 1_700;
        e.push(hit(at, 36 + (step % 8) as u8, 60 + (step * 13 % 67) as u8));
        if step % 3 == 0 {
            e.push(hit(at, 36 + ((step + 3) % 8) as u8, 110));
        }
        if step % 5 == 0 {
            e.push(hit(at, 38, 127));
        }
    }
    e.sort_by_key(|x| x.0);
    e
}

/// Drive `k` for `n` frames in `block`s split at the events, calling `seg(k, len)` for each
/// segment, as the engine's track node does.
fn drive(
    k: &mut Pulsus,
    n: usize,
    block: usize,
    events: &[Event],
    mut seg: impl FnMut(&mut Pulsus, usize, usize),
) {
    let mut done = 0;
    while done < n {
        let mut end = (done + block).min(n);
        for (at, pitch, vel, tick, id) in events {
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
        seg(k, done, end - done);
        done = end;
    }
}

/// The kit through `process_add`, interleaved.
fn whole(k: &mut Pulsus, n: usize, block: usize, events: &[Event]) -> Vec<f32> {
    let mut out = vec![0.0f32; 2 * n];
    drive(k, n, block, events, |k, at, len| {
        let mut l = vec![0.0; len];
        let mut r = vec![0.0; len];
        k.process_add(&mut l, &mut r);
        for i in 0..len {
            out[2 * (at + i)] = l[i];
            out[2 * (at + i) + 1] = r[i];
        }
    });
    out
}

/// The kit through `process_buses`, each bus into its own zeroed buffer as a pad's strip
/// receives it, and the buses gathered back from +0.0 in bus order, as the kit's strip
/// gathers its pad strips.
fn split(k: &mut Pulsus, buses: usize, n: usize, block: usize, events: &[Event]) -> Vec<f32> {
    let mut out = vec![0.0f32; 2 * n];
    drive(k, n, block, events, |k, at, len| {
        let mut bl = vec![vec![0.0f32; len]; buses];
        let mut br = vec![vec![0.0f32; len]; buses];
        k.process_buses(len, &mut |b, off, a, c| {
            for i in 0..a.len() {
                bl[b as usize][off + i] += a[i];
                br[b as usize][off + i] += c[i];
            }
        });
        for i in 0..len {
            let (mut l, mut r) = (0.0f32, 0.0f32);
            for b in 0..buses {
                l += bl[b][i];
                r += br[b][i];
            }
            out[2 * (at + i)] = l;
            out[2 * (at + i) + 1] = r;
        }
    });
    out
}

fn same_bits(a: &[f32], b: &[f32], what: &str) {
    assert_eq!(a.len(), b.len());
    for (i, (x, y)) in a.iter().zip(b).enumerate() {
        assert!(x == y, "{what}: sample {i} differs ({x:e} against {y:e})");
    }
}

/// The promise of per-pad outputs: every pad on its own output, the outputs summed back in
/// pad order, is the kit — to the bit, not to a tolerance.
#[test]
fn a_model_three_kit_split_to_buses_sums_to_itself_to_the_bit() {
    let p = pads();
    let n = 96_000;
    let ev = groove();
    let unsplit = whole(&mut kit(3, &p, |_| 0), n, 512, &ev);
    assert!(peak(&unsplit) > 0.1, "the groove sounds");
    let parted = split(&mut kit(3, &p, |i| i as u8 + 1), p.len() + 1, n, 512, &ev);
    same_bits(&unsplit, &parted, "split against unsplit");
    // Two pads sharing an output: still the kit.
    let pairs = split(
        &mut kit(3, &p, |i| (i / 2) as u8 + 1),
        p.len() + 1,
        n,
        333,
        &ev,
    );
    assert_null(&unsplit, &pairs, 1e-6, "pads two by two");
}

/// Lanes are cut every `LANE_FRAMES` and wherever the host cuts: neither moves a sample.
#[test]
fn lanes_are_block_size_independent() {
    let p = pads();
    let ev = groove();
    let n = 60_000;
    let reference = whole(&mut kit(3, &p, |_| 0), n, 512, &ev);
    for block in [1usize, 7, 64, 255, 256, 257, 1_000, 4_096] {
        let other = whole(&mut kit(3, &p, |_| 0), n, block, &ev);
        assert_null(&reference, &other, 0.0, &format!("block {block}"));
        let parted = split(&mut kit(3, &p, |i| i as u8 + 1), p.len() + 1, n, block, &ev);
        same_bits(&reference, &parted, &format!("split, block {block}"));
    }
}

/// Model 2 keeps its sum in voice order: an old kit renders as it did. Model 3 differs from
/// it only by the order of the additions — measured, never audible.
#[test]
fn model_two_keeps_its_voice_order_and_model_three_differs_only_in_rounding() {
    let p = pads();
    let ev = groove();
    let n = 96_000;
    let two = whole(&mut kit(2, &p, |_| 0), n, 512, &ev);
    let mut k = kit(2, &p, |_| 0);
    let mut by_bus = vec![0.0f32; 2 * n];
    drive(&mut k, n, 512, &ev, |k, at, len| {
        let mut l = vec![0.0; len];
        let mut r = vec![0.0; len];
        k.process_add_bus(None, &mut l, &mut r);
        for i in 0..len {
            by_bus[2 * (at + i)] = l[i];
            by_bus[2 * (at + i) + 1] = r[i];
        }
    });
    same_bits(&two, &by_bus, "model 2 is its voice-order sum");
    let three = whole(&mut kit(3, &p, |_| 0), n, 512, &ev);
    let worst = two
        .iter()
        .zip(&three)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    eprintln!(
        "pulsus lanes: model 3 against model 2, max |d| {worst:e} ({:.1} dBFS)",
        20.0 * crate::math::log10(worst.max(1e-30))
    );
    assert!(
        worst < 1e-6,
        "only the rounding of the sum may differ: {worst:e}"
    );
}

/// A pad moved to another output takes its ringing voices with it: everything it sounds
/// after the move is on the new output, nothing on the old.
#[test]
fn a_pad_moved_to_another_output_takes_its_ringing_voices() {
    let p = pads();
    let mut k = kit(3, &p, |_| 0);
    k.trigger(36, 120, Hit::default());
    let mut seen = [false; 3];
    k.process_buses(256, &mut |b, _, _, _| seen[b as usize] = true);
    assert_eq!(seen, [true, false, false]);
    let mut moved = p[0];
    moved.id = 1;
    moved.bus = 2;
    k.set_pad(0, 36, &moved);
    let mut seen = [false; 3];
    let mut energy = 0.0f32;
    k.process_buses(256, &mut |b, _, a, _| {
        seen[b as usize] = true;
        energy += a.iter().map(|x| x * x).sum::<f32>();
    });
    assert_eq!(
        seen,
        [false, false, true],
        "the kick rings on its new output"
    );
    assert!(energy > 0.0);
}

/// A stop (all notes off) lets a pad that chokes on its note-off go, as its note-off would
/// have; a one-shot pad rings on (round 37: a grabbed cymbal rang past a stop, held).
#[test]
fn all_notes_off_lets_go_of_a_pad_that_chokes_on_its_note_off() {
    let grabbed = PadSettings {
        decay: 2.0,
        choke_on_off: true,
        choke_fade: 0.002,
        ..PadSettings::of(DrumVoice::HiHat, 1)
    };
    let free = PadSettings {
        decay: 2.0,
        ..PadSettings::of(DrumVoice::HiHat, 1)
    };
    let mut k = kit(3, &[grabbed, free], |_| 0);
    k.trigger(36, 110, Hit::default());
    k.trigger(37, 110, Hit::default());
    let level = |k: &mut Pulsus, pad: u8| {
        let mut e = 0.0f32;
        k.process_buses(480, &mut |_, _, a, _| {
            e += a.iter().map(|x| x * x).sum::<f32>()
        });
        let _ = pad;
        e
    };
    let both = level(&mut k, 0);
    k.all_notes_off();
    let _ = level(&mut k, 0);
    let after = level(&mut k, 0);
    // The grabbed pad is gone within the fade; the free one rings on.
    let mut only_free = kit(3, &[grabbed, free], |_| 0);
    only_free.trigger(37, 110, Hit::default());
    let _ = level(&mut only_free, 0);
    let _ = level(&mut only_free, 0);
    let free_after = level(&mut only_free, 0);
    assert!(both > 0.0 && free_after > 0.0);
    assert!(
        (after - free_after).abs() <= free_after * 1e-3,
        "after the stop only the free pad sounds: {after} against {free_after}"
    );
    assert!(
        k.voices.iter().any(|v| v.active && !v.choking),
        "the free pad rings"
    );
}

/// The kit's own gain (ADR 0100 §8): at unity not one sample changes; any other gain is on
/// each lane before the sum, so the kit split to buses is still the kit to the bit, and the
/// whole is the unity render times the gain within a rounding.
#[test]
fn the_kits_gain_is_a_null_at_unity_and_keeps_the_split_exact() {
    let p = pads();
    let ev = groove();
    let n = 60_000;
    let plain = whole(&mut kit(3, &p, |_| 0), n, 512, &ev);
    let mut unity = kit(3, &p, |_| 0);
    unity.set_gain(1.0);
    same_bits(
        &plain,
        &whole(&mut unity, n, 512, &ev),
        "gain 1.0 at model 3",
    );
    let mut two = kit(2, &p, |_| 0);
    two.set_gain(1.0);
    same_bits(
        &whole(&mut kit(2, &p, |_| 0), n, 512, &ev),
        &whole(&mut two, n, 512, &ev),
        "gain 1.0 at model 2",
    );
    let g = 1.68;
    let mut loud = kit(3, &p, |_| 0);
    loud.set_gain(g);
    let a = whole(&mut loud, n, 512, &ev);
    let mut loud_split = kit(3, &p, |i| i as u8 + 1);
    loud_split.set_gain(g);
    same_bits(
        &a,
        &split(&mut loud_split, p.len() + 1, n, 333, &ev),
        "split under a kit gain",
    );
    let want: Vec<f32> = plain.iter().map(|x| x * g).collect();
    assert_null(&want, &a, 1e-6, "the gain scales the kit");
}
