//! r42:strings-rec — the note's articulation and bow, the controller crossfade and the
//! scripted legato (ADR 0158).

use super::art::{cc_crossfade, DOWN, PIZZICATO, SPICCATO, STACCATO, SUSTAIN, UP};
use super::tests::source_of;
use super::*;
use crate::math;

fn dc(level: f32, frames: usize) -> Source {
    source_of(vec![level; frames], 1, 48_000.0, false)
}

fn sine(period: usize, frames: usize, amp: f32) -> Source {
    let data = (0..frames)
        .map(|i| math::sin(std::f64::consts::TAU * i as f64 / period as f64) as f32 * amp)
        .collect();
    source_of(data, 1, 48_000.0, false)
}

fn with(zones: &[(ZoneParams, Source)], g: Globals) -> Exemplar {
    let mut e = Exemplar::new(48_000.0);
    e.set_globals(&g);
    for (n, (z, s)) in zones.iter().enumerate() {
        e.put_zone(n, z, s);
    }
    e
}

fn flat() -> Globals {
    Globals {
        amp: [0.1, 0.1, 1_000.0, 1.0, 5.0],
        vel_amp: 0.0,
        ..Globals::default()
    }
}

/// Left channel only.
fn render(e: &mut Exemplar, frames: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(frames);
    let (mut l, mut r) = (vec![0.0; 64], vec![0.0; 64]);
    let mut done = 0;
    while done < frames {
        let n = 64.min(frames - done);
        l[..n].fill(0.0);
        r[..n].fill(0.0);
        e.process(&mut l[..n], &mut r[..n]);
        out.extend_from_slice(&l[..n]);
        done += n;
    }
    out
}

/// Sustain (1.0) and pizzicato (0.5) on keyswitches 24 and 25, sustain the default.
fn switched() -> Exemplar {
    let zone = |art: u8, sw: i16| ZoneParams {
        lo: 36,
        art,
        sw,
        ..ZoneParams::default()
    };
    let g = Globals {
        sw_lo: 24,
        sw_hi: 25,
        sw_default: 24,
        ..flat()
    };
    with(
        &[
            (zone(SUSTAIN, 24), dc(1.0, 48_000)),
            (zone(PIZZICATO, 25), dc(0.5, 48_000)),
        ],
        g,
    )
}

#[test]
fn the_keyswitch_and_the_notes_articulation_both_choose() {
    // No articulation: the keyswitch in force chooses.
    let mut e = switched();
    e.note_on_seq(60, 1.0, 1, 0);
    assert_eq!(render(&mut e, 256)[200], 1.0, "the default switch: sustain");
    let mut e = switched();
    e.note_on(25, 1.0, 99);
    e.note_on_seq(60, 1.0, 1, 0);
    assert_eq!(render(&mut e, 256)[200], 0.5, "the pizzicato switch");
    // The note's own articulation wins over the switch, for that note only.
    let mut e = switched();
    e.note_art(1, PIZZICATO, 0);
    e.note_on_seq(60, 1.0, 1, 0);
    assert_eq!(render(&mut e, 256)[200], 0.5, "a pizzicato note");
    e.note_on_seq(64, 1.0, 2, 0);
    let both = render(&mut e, 256)[200];
    assert!(
        (both - 1.5).abs() < 1e-4,
        "the next note follows the switch again: {both}"
    );
    // An articulation handed for another note is forgotten.
    let mut e = switched();
    e.note_art(7, PIZZICATO, 0);
    e.note_on_seq(60, 1.0, 1, 0);
    assert_eq!(render(&mut e, 256)[200], 1.0);
    assert!(e.pending_art.is_none());
}

#[test]
fn an_articulation_the_key_lacks_plays_its_nearest() {
    // Only spiccato recorded: a staccato note plays it; a pizzicato note (no fallback)
    // follows the switch (here: none, so every untagged zone and the spiccato play).
    let spic = ZoneParams {
        art: SPICCATO,
        ..ZoneParams::default()
    };
    let mut e = with(&[(spic, dc(0.25, 48_000))], flat());
    e.note_art(1, STACCATO, 0);
    e.note_on_seq(60, 1.0, 1, 0);
    assert_eq!(render(&mut e, 256)[200], 0.25);
}

#[test]
fn the_bow_turns_while_it_sounds_and_starts_down_after_a_silence() {
    let zone = |bow: u8| ZoneParams {
        bow,
        ..ZoneParams::default()
    };
    let mk = || {
        with(
            &[(zone(DOWN), dc(1.0, 96_000)), (zone(UP), dc(0.5, 96_000))],
            flat(),
        )
    };
    let mut e = mk();
    let mut heard = Vec::new();
    for (i, key) in [60u8, 62, 64, 65].into_iter().enumerate() {
        let id = i as u64 + 1;
        e.note_on_seq(key, 1.0, id, 0);
        let out = render(&mut e, 512);
        heard.push((out[400] * 1e3).round() / 1e3);
        e.note_off(key, id, 0.0);
        // The release tail (5 ms) still rings when the next note starts.
        render(&mut e, 64);
    }
    // Each note's layer alone at 400 (the last one's tail is gone by then).
    assert_eq!(heard, vec![1.0, 0.5, 1.0, 0.5], "down, up, down, up");
    render(&mut e, 4_800);
    e.note_on_seq(67, 1.0, 9, 0);
    let first = render(&mut e, 512)[400];
    assert!(
        (first - 1.0).abs() < 1e-4,
        "after a silence: down ({first})"
    );
    // A bow the note names is played; one the key lacks plays the other, never both.
    let mut e = mk();
    e.note_art(1, 0, UP);
    e.note_on_seq(60, 1.0, 1, 0);
    assert_eq!(render(&mut e, 512)[400], 0.5);
    let mut e = with(&[(zone(DOWN), dc(1.0, 96_000))], flat());
    e.note_art(1, 0, UP);
    e.note_on_seq(60, 1.0, 1, 0);
    assert_eq!(render(&mut e, 512)[400], 1.0);
}

#[test]
fn the_controller_crossfade_is_continuous_equal_power_and_monotone() {
    // The law: squares sum to 1 across one range, at every controller value.
    for c in 0..=127 {
        let c = c as f32;
        let p = cc_crossfade([0, 0], [0, 127], false, c);
        let f = cc_crossfade([0, 127], [127, 127], false, c);
        assert!((p * p + f * f - 1.0).abs() < 1e-5, "{c}: {p} {f}");
    }
    // The defaults are full gain everywhere (every zone before round 42).
    assert_eq!(cc_crossfade([0, 0], [127, 127], false, 0.0), 1.0);
    assert_eq!(cc_crossfade([0, 0], [127, 127], false, 127.0), 1.0);
    // Two layers held while the mod wheel sweeps 0 → 1: the soft one (a quarter), the loud
    // one (full). Uncorrelated noise, so their powers add: the level only rises, and it
    // moves sample by sample — no step larger than one control step's share.
    let noise = |seed: u64| {
        let mut x = seed;
        let data = (0..200_000)
            .map(|_| {
                x = engine::mix(x);
                (x >> 40) as f32 / (1u64 << 24) as f32 * 2.0 - 1.0
            })
            .collect();
        source_of(data, 1, 48_000.0, false)
    };
    let soft = ZoneParams {
        xfcc: 1,
        xfout: [0, 127],
        gain_db: -12.0,
        ..ZoneParams::default()
    };
    let loud = ZoneParams {
        xfcc: 1,
        xfin: [0, 127],
        ..ZoneParams::default()
    };
    let mut e = with(&[(soft, noise(1)), (loud, noise(2))], flat());
    e.note_on_seq(60, 1.0, 1, 0);
    let mut levels = Vec::new();
    for step in 0..=16 {
        e.controller(0, step as f32 / 16.0);
        let out = render(&mut e, 4_800);
        let rms = (out[1_000..].iter().map(|x| x * x).sum::<f32>() / 3_800.0).sqrt();
        levels.push(rms);
    }
    for w in levels.windows(2) {
        assert!(w[1] >= w[0] * 0.98, "the level falls: {levels:?}");
    }
    assert!(levels[16] > levels[0] * 3.5, "soft to loud: {levels:?}");
    // Continuous: a DC layer under a jump of the wheel ramps, it does not step.
    let mut e = with(&[(loud, dc(1.0, 96_000))], flat());
    e.controller(0, 0.0);
    e.note_on_seq(60, 1.0, 1, 0);
    render(&mut e, 512);
    e.controller(0, 1.0);
    let out = render(&mut e, 512);
    let step = out.windows(2).fold(0f32, |m, w| m.max((w[1] - w[0]).abs()));
    assert!(step <= 1.0 / 32.0 + 1e-6, "a step of {step}");
    assert_eq!(out[500], 1.0);
}

/// The frequency of `x` over a window, from its upward zero crossings (cycles per sample).
fn freq(x: &[f32]) -> f64 {
    let ups: Vec<f64> = x
        .windows(2)
        .enumerate()
        .filter(|(_, w)| w[0] < 0.0 && w[1] >= 0.0)
        .map(|(i, w)| i as f64 + (-w[0] / (w[1] - w[0])) as f64)
        .collect();
    (ups.len() - 1) as f64 / (ups[ups.len() - 1] - ups[0])
}

fn legato_globals() -> Globals {
    Globals {
        legato: true,
        legato_xf_ms: 5.0,
        legato_glide_ms: 200.0,
        ..flat()
    }
}

#[test]
fn a_legato_note_crossfades_out_of_the_held_one_and_glides_by_its_interval() {
    let zone = ZoneParams::default();
    let src = sine(100, 480_000, 0.5);
    let mut e = with(&[(zone, src)], legato_globals());
    e.note_on_seq(60, 1.0, 1, 0);
    render(&mut e, 4_800);
    // The next note starts before the first lets go: it slurs.
    e.note_on_seq(64, 1.0, 2, 0);
    let early = render(&mut e, 2_400); // the first 50 ms
    e.note_off(60, 1, 0.0);
    let mid = render(&mut e, 4_800);
    let late = render(&mut e, 19_200);
    let start = 1.0 / 100.0;
    let target = start * math::exp2(4.0 / 12.0);
    let (fe, fm, fl) = (freq(&early[600..]), freq(&mid), freq(&late[9_600..]));
    assert!(
        fe > start * 1.0 && fe < start * math::exp2(1.5 / 12.0),
        "early {fe}"
    );
    assert!(fm > fe && fm < target, "mid {fm}");
    assert!((fl / target - 1.0).abs() < 1e-3, "late {fl} vs {target}");
    // One voice's worth of level throughout: no new attack, no gap.
    assert_eq!(e.active_layers(), 1, "the old note faded out");
    // A chord struck together does not slur.
    let mut e = with(&[(zone, sine(100, 480_000, 0.5))], legato_globals());
    e.note_on_seq(60, 1.0, 1, 0);
    e.note_on_seq(64, 1.0, 2, 0);
    render(&mut e, 4_800);
    assert_eq!(e.active_layers(), 2);
    // Off, an overlap is two notes; a note marked legato still slurs.
    let mut e = with(&[(zone, sine(100, 480_000, 0.5))], flat());
    e.note_on_seq(60, 1.0, 1, 0);
    render(&mut e, 4_800);
    e.note_on_seq(64, 1.0, 2, 0);
    render(&mut e, 4_800);
    assert_eq!(e.active_layers(), 2);
    let mut e = with(&[(zone, sine(100, 480_000, 0.5))], flat());
    e.note_on_seq(60, 1.0, 1, 0);
    render(&mut e, 4_800);
    e.note_art(2, super::art::LEGATO, 0);
    e.note_on_seq(64, 1.0, 2, 0);
    render(&mut e, 9_600);
    assert_eq!(e.active_layers(), 1);
}

#[test]
fn a_slurred_note_plays_no_release_and_pizzicato_never_slurs() {
    let sus = ZoneParams {
        art: SUSTAIN,
        ..ZoneParams::default()
    };
    let pizz = ZoneParams {
        art: PIZZICATO,
        ..ZoneParams::default()
    };
    let rel = ZoneParams {
        trig: Trig::Release,
        ..ZoneParams::default()
    };
    let mut e = with(
        &[
            (sus, sine(100, 480_000, 0.5)),
            (pizz, sine(100, 480_000, 0.5)),
            (rel, dc(0.1, 48_000)),
        ],
        legato_globals(),
    );
    e.note_art(1, SUSTAIN, 0);
    e.note_on_seq(60, 1.0, 1, 0);
    render(&mut e, 4_800);
    e.note_art(2, SUSTAIN, 0);
    e.note_on_seq(62, 1.0, 2, 0);
    render(&mut e, 4_800);
    e.note_off(60, 1, 0.0);
    render(&mut e, 64);
    assert_eq!(
        e.active_layers(),
        1,
        "no release sample for the slurred note"
    );
    e.note_art(3, PIZZICATO, 0);
    e.note_on_seq(65, 1.0, 3, 0);
    render(&mut e, 64);
    assert_eq!(e.active_layers(), 2, "a pizzicato is its own note");
}
