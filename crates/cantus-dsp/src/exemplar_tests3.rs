//! Exemplar's DSP tests, part three: zones a controller starts (round 26: a piano's pedal
//! noises, SFZ `on_loccN`/`on_hiccN`).

use super::tests::source_of;
use super::*;

fn engine_with(zones: &[(ZoneParams, Source)]) -> Exemplar {
    let mut e = Exemplar::new(48_000.0);
    let g = Globals {
        amp: [0.1, 0.1, 1_000.0, 1.0, 5.0],
        ..Globals::default()
    };
    e.set_globals(&g);
    for (n, (z, s)) in zones.iter().enumerate() {
        e.put_zone(n, z, s);
    }
    e
}

fn dc(level: f32, frames: usize) -> Source {
    source_of(vec![level; frames], 1, 48_000.0, false)
}

fn ramp(frames: usize) -> Source {
    source_of(
        (0..frames).map(|i| (i % 977) as f32 / 1_000.0).collect(),
        1,
        48_000.0,
        false,
    )
}

fn render(e: &mut Exemplar, frames: usize, block: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(frames);
    let (mut l, mut r) = (vec![0.0; block], vec![0.0; block]);
    let mut done = 0;
    while done < frames {
        let n = block.min(frames - done);
        l[..n].fill(0.0);
        r[..n].fill(0.0);
        e.process(&mut l[..n], &mut r[..n]);
        out.extend_from_slice(&l[..n]);
        done += n;
    }
    out
}

/// A pedal-down noise: CC 64 crossing into 126…127, exclusive class 1, silenced by 2.
fn pedal_down() -> ZoneParams {
    ZoneParams {
        trig: Trig::Controller,
        cc: 64,
        cclo: 126,
        cchi: 127,
        excl: 1,
        offby: 2,
        ..ZoneParams::default()
    }
}

/// A pedal-up noise: CC 64 crossing into 0…1, class 2.
fn pedal_up() -> ZoneParams {
    ZoneParams {
        trig: Trig::Controller,
        cc: 64,
        cclo: 0,
        cchi: 1,
        excl: 2,
        ..ZoneParams::default()
    }
}

#[test]
fn a_controller_crossing_into_its_range_starts_its_zone_on_that_sample() {
    let mut e = engine_with(&[(pedal_down(), dc(0.5, 48_000))]);
    assert_eq!(render(&mut e, 100, 64), vec![0.0; 100]);
    // Keys never start it.
    e.note_on(60, 1.0, 1);
    assert!(render(&mut e, 100, 64).iter().all(|x| *x == 0.0));
    e.note_off(60, 1, 0.0);
    e.cc(64, 127);
    let out = render(&mut e, 200, 64);
    assert_eq!(out[0], 0.5, "full level from the very next sample");
    assert!(out.iter().all(|x| *x == 0.5));
    assert_eq!(e.active_layers(), 1);
    // Staying in the range starts nothing more.
    e.cc(64, 126);
    render(&mut e, 10, 10);
    assert_eq!(e.active_layers(), 1, "no crossing, no new layer");
    // Leaving the range lets a sample that does not loop play on (SFZ players do the same
    // with pedal noises); crossing in again starts another.
    e.cc(64, 100);
    assert!(render(&mut e, 4_800, 256).iter().all(|x| *x == 0.5));
    e.cc(64, 127);
    assert_eq!(render(&mut e, 1, 1)[0], 1.0, "two layers now");
    assert_eq!(e.active_layers(), 2);
}

#[test]
fn a_looping_controller_zone_is_held_while_the_value_stays_in_its_range() {
    let held = ZoneParams {
        region: Region {
            looping: true,
            ls: 0,
            le: 999,
            ..Region::default()
        },
        ..pedal_down()
    };
    let mut e = engine_with(&[(held, dc(0.5, 1_000))]);
    e.cc(64, 127);
    assert!(
        render(&mut e, 20_000, 512).iter().all(|x| *x == 0.5),
        "loops"
    );
    e.cc(64, 120);
    let gone = render(&mut e, 4_800, 256);
    assert!(gone[4_799].abs() < 1e-6, "released (5 ms): {}", gone[4_799]);
    assert_eq!(e.active_layers(), 0);
}

#[test]
fn a_value_already_inside_the_range_triggers_nothing() {
    // A new unit's controllers are at 0: pedal-up's range already holds it.
    let mut e = engine_with(&[(pedal_up(), dc(0.25, 48_000))]);
    e.cc(64, 0);
    e.cc(64, 1);
    assert!(render(&mut e, 64, 64).iter().all(|x| *x == 0.0));
    e.cc(64, 127);
    e.cc(64, 0);
    assert_eq!(render(&mut e, 1, 1)[0], 0.25, "out and back in: it plays");
}

#[test]
fn pedal_up_silences_pedal_down_without_a_click() {
    let mut e = engine_with(&[
        (pedal_down(), dc(0.5, 96_000)),
        (pedal_up(), dc(0.0, 96_000)),
    ]);
    e.cc(64, 127);
    assert_eq!(render(&mut e, 1_000, 256)[999], 0.5);
    e.cc(64, 0);
    let out = render(&mut e, 2_000, 256);
    assert_eq!(out[1_999], 0.0, "the down noise is choked");
    let steps = out.windows(2).fold(0f32, |m, w| m.max((w[1] - w[0]).abs()));
    assert!(steps <= 0.5 / 240.0 + 1e-6, "the choke clicks: {steps}");
}

#[test]
fn a_random_pair_plays_one_of_the_two_and_repeats_exactly() {
    let a = ZoneParams {
        rhi: 0.5,
        ..pedal_down()
    };
    let b = ZoneParams {
        rlo: 0.5,
        ..pedal_down()
    };
    let picks = || {
        let mut e = engine_with(&[(a, dc(0.25, 2_000)), (b, dc(0.5, 2_000))]);
        let mut seen = Vec::new();
        for _ in 0..40 {
            e.cc(64, 127);
            seen.push(render(&mut e, 1, 1)[0]);
            assert_eq!(e.active_layers(), 1, "exactly one of the pair");
            e.cc(64, 0);
            render(&mut e, 3_000, 512);
        }
        seen
    };
    let first = picks();
    assert_eq!(first, picks(), "the same presses choose the same samples");
    assert!(first.contains(&0.25) && first.contains(&0.5), "{first:?}");
}

#[test]
fn a_controller_zone_plays_its_file_at_its_root_bit_for_bit() {
    let src = ramp(10_000);
    let z = ZoneParams {
        root: 48,
        lo: 0,
        hi: 0,
        ..pedal_down()
    };
    let mut e = engine_with(&[(z, src.clone())]);
    e.cc(64, 127);
    let out = render(&mut e, 5_000, 333);
    let file = src.levels[0].as_deref().unwrap();
    assert!(out == file[..5_000], "the samples themselves, at unity");
}

#[test]
fn renders_with_controller_zones_are_the_same_at_every_block_size() {
    let zones = [(pedal_down(), ramp(30_000)), (pedal_up(), ramp(20_000))];
    let play = |block: usize| {
        let mut e = engine_with(&zones);
        let mut out = Vec::new();
        for (v, n) in [(127u8, 7_001), (0, 9_003), (127, 2_000), (64, 30_000)] {
            e.cc(64, v);
            out.extend(render(&mut e, n, block));
        }
        out
    };
    let reference = play(512);
    assert!(reference.iter().any(|x| *x != 0.0));
    for block in [1usize, 31, 128, 4_096] {
        assert!(play(block) == reference, "block size {block} differs");
    }
}

#[test]
fn a_unit_rests_again_once_the_pedal_is_up_and_the_noises_have_ended() {
    let mut e = engine_with(&[(pedal_down(), dc(0.5, 4_800)), (pedal_up(), dc(0.5, 4_800))]);
    assert!(e.at_rest());
    e.cc(64, 127);
    assert!(!e.at_rest(), "the pedal is down");
    render(&mut e, 10_000, 512);
    e.cc(64, 0);
    render(&mut e, 10_000, 512);
    assert_eq!(e.active_layers(), 0);
    // The presses counted choose the next random sample: rest only once that count is
    // forgotten (a stop forgets it), as a new unit would choose.
    assert!(!e.at_rest());
    e.all_notes_off();
    assert!(e.at_rest(), "pedal up, noises over, the count forgotten");
    e.cc(64, 127);
    e.reset();
    assert!(e.at_rest(), "a reset forgets the pedal");
}

/// Review 22: a continuous loop a controller started keeps looping through its release, as
/// a key's does — the release tail is the same as a key-released note's.
#[test]
fn a_continuous_loop_keeps_looping_through_its_release_like_a_key_note() {
    let cycle: Vec<f32> = (0..4_800)
        .map(|i| crate::math::sin(i as f32 * std::f32::consts::TAU / 480.0) * 0.5)
        .collect();
    let looped = |trig: Trig| ZoneParams {
        trig,
        region: Region {
            looping: true,
            ls: 0,
            le: 4_799,
            ..Region::default()
        },
        ..pedal_down()
    };
    let tail = |trig: Trig| {
        let mut e = engine_with(&[(looped(trig), source_of(cycle.clone(), 1, 48_000.0, false))]);
        let mut g = e.globals().clone();
        g.amp[4] = 500.0;
        e.set_globals(&g);
        if trig == Trig::Controller {
            e.cc(64, 127);
        } else {
            e.note_on(60, 1.0, 1);
        }
        render(&mut e, 10_000, 512);
        if trig == Trig::Controller {
            e.cc(64, 0);
        } else {
            e.note_off(60, 1, 0.0);
        }
        render(&mut e, 30_000, 512)
    };
    let by_key = tail(Trig::Attack);
    let by_cc = tail(Trig::Controller);
    let late = by_key[19_000..21_000]
        .iter()
        .fold(0f32, |m, x| m.max(x.abs()));
    assert!(late > 1e-4, "the key's release still sounds: {late}");
    let d = by_cc.iter().zip(&by_key).position(|(a, b)| a != b);
    assert!(
        by_cc == by_key,
        "the controller's release is the key's: first difference at {d:?} ({:?} vs {:?})",
        d.map(|i| by_cc[i]),
        d.map(|i| by_key[i])
    );
}

/// Review 23: a key or velocity crossfade does not silence a zone a controller starts (it
/// starts on its root at full velocity: crossfades are the keys' business).
#[test]
fn a_crossfaded_controller_zone_still_sounds() {
    for z in [
        ZoneParams {
            vfout: 10,
            ..pedal_down()
        },
        ZoneParams {
            lo: 60,
            hi: 60,
            root: 60,
            kfin: 3,
            kfout: 3,
            ..pedal_down()
        },
    ] {
        let mut e = engine_with(&[(z, dc(0.5, 48_000))]);
        e.cc(64, 127);
        assert_eq!(render(&mut e, 10, 10)[9], 0.5);
    }
}
