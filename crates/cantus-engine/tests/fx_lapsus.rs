//! Lapsus through the whole engine (ADR 0132, provisional): a wire at its defaults and at
//! mix 0 whatever pattern plays, every factory pattern heard, the same render at every block
//! size, live equal to offline, every pattern of the bank from bar 5 equal to bar 5 of the
//! whole render, a tape stop ending on the bar's own sample at 60, 120 and 174 BPM, a
//! track's notes choosing the pattern, and its lookahead compensated.

#[path = "support/modfx.rs"]
mod modfx;
mod support;
use cantus_core::fx::lapsus::SLOTS;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use modfx::*;
use support::*;

#[test]
fn lapsus_at_its_defaults_renders_exactly_as_no_lapsus() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    add_effect(&mut p, EffectKind::TimeShaper);
    assert_null(&reference, &render_of(&p), 0.0, "Lapsus at its defaults");
}

#[test]
fn at_mix_zero_any_pattern_is_a_wire() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::TimeShaper);
    set_all(
        &mut p,
        fx,
        &[
            ("tslot", 16.0),
            ("vslot", 5.0),
            ("mix", 0.0),
            ("xfade", 7.0),
        ],
    );
    assert_null(&reference, &render_of(&p), 0.0, "mix 0");
}

fn peak_diff(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f32::max)
}

/// Every pattern of each bank alone (the other bank on *Through*), then a few together.
fn settings_to_check() -> Vec<(f32, f32)> {
    let mut v: Vec<(f32, f32)> = (0..SLOTS).map(|t| (t as f32, 0.0)).collect();
    v.extend((1..SLOTS).map(|k| (0.0, k as f32)));
    v.extend([(3.0, 21.0), (18.0, 5.0), (24.0, 12.0), (6.0, 30.0)]);
    v
}

#[test]
fn every_factory_pattern_of_both_banks_is_heard_and_through_is_not() {
    let reference = render_of(&demo_project());
    for (t, v) in settings_to_check() {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::TimeShaper);
        set_all(&mut p, fx, &[("tslot", t), ("vslot", v)]);
        let d = peak_diff(&render_of(&p), &reference);
        if (t, v) == (0.0, 0.0) {
            assert_eq!(d, 0.0, "Through and Through");
        } else {
            assert!(
                d > 0.02,
                "time {} with volume {} changes the sound: {d}",
                t + 1.0,
                v + 1.0
            );
        }
    }
}

fn shaped(t: f32, v: f32) -> Project {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::TimeShaper);
    set_all(
        &mut p,
        fx,
        &[("tslot", t), ("vslot", v), ("mix", 0.9), ("xfade", 3.0)],
    );
    p
}

#[test]
fn the_render_does_not_depend_on_the_block_size_while_it_works() {
    for (t, v) in [(1.0, 0.0), (16.0, 0.0), (24.0, 12.0), (0.0, 30.0)] {
        let p = shaped(t, v);
        let reference = render_with(&p, 512);
        for block in [1usize, 7, 64, 1000, 4096] {
            assert_null(
                &reference,
                &render_with(&p, block),
                0.0,
                &format!("{t}/{v}, block {block}"),
            );
        }
    }
}

#[test]
fn live_playback_renders_what_the_offline_render_does() {
    let p = shaped(16.0, 5.0);
    let offline = render_of(&p);
    assert_null(&offline, &realtime(&p, offline.len()), 0.0, "live");
}

/// From bar 5 (beat 16) every pattern of the bank is where the whole render's is: the riff
/// plays from the start and again from bar 5 (the input from bar 5 is the same in both),
/// and whatever Lapsus held from before bar 5 is never read there.
#[test]
fn every_pattern_from_bar_five_is_bar_five_of_the_whole_render() {
    let mut base = lead_only();
    base.tempo_bpm = 120.0;
    let mut c = base.tracks[0].clips[0].clone();
    c.id = ClipId(base.alloc_id());
    c.start = Ticks::beats(16);
    base.tracks[0].clips.push(c);
    let bar5 = 16 * 24_000; // frames at 120 BPM, 48 kHz
    for (t, v) in settings_to_check() {
        let mut p = base.clone();
        let fx = add_effect(&mut p, EffectKind::TimeShaper);
        set_all(&mut p, fx, &[("tslot", t), ("vslot", v), ("mix", 0.85)]);
        let whole = render_from(&p, Ticks::beats(0), 256);
        let part = render_from(&p, Ticks::beats(16), 300);
        let n = (part.len() / 2).min(whole.len() / 2 - bar5);
        assert!(n > 96_000, "the song runs past bar 5");
        assert!(part[..2 * n].iter().any(|v| v.abs() > 0.01), "bar 5 sounds");
        assert_null(
            &whole[2 * bar5..2 * (bar5 + n)],
            &part[..2 * n],
            0.0,
            &format!("time {} with volume {} from bar 5", t + 1.0, v + 1.0),
        );
    }
}

/// *Tape stop ¼* (time 4) with *Fade the last beat* (volume 22) at three tempos: the bar
/// plays as it arrives from the sample the engine starts bar 2 on — the stop ended on that
/// line — and the stop's last moments before it are still and quiet.
#[test]
fn a_tape_stop_ends_on_the_bar_to_the_sample_at_60_120_and_174() {
    for tempo in [60.0, 120.0, 174.0] {
        let mut dry = demo_project();
        dry.tempo_bpm = tempo;
        let reference = render_of(&dry);
        let mut p = dry.clone();
        let fx = add_effect(&mut p, EffectKind::TimeShaper);
        set_all(&mut p, fx, &[("tslot", 3.0), ("vslot", 21.0)]);
        let out = render_of(&p);
        let bar2 = ((4.0 * 60.0 / tempo) * 48_000.0f64).round() as usize;
        let span = 2 * 4_000;
        assert_null(
            &reference[2 * bar2..2 * bar2 + span],
            &out[2 * bar2..2 * bar2 + span],
            0.0,
            &format!("{tempo} BPM: bar 2 from its own sample"),
        );
        let before = &out[2 * (bar2 - 1)..2 * bar2];
        assert!(
            before
                .iter()
                .zip(&reference[2 * (bar2 - 1)..2 * bar2])
                .any(|(a, b)| a != b),
            "{tempo} BPM: the sample before the line is the stop's"
        );
        let quiet = out[2 * (bar2 - 480)..2 * (bar2 - 150)]
            .iter()
            .fold(0f32, |m, v| m.max(v.abs()));
        let loud = reference[..2 * bar2]
            .iter()
            .fold(0f32, |m, v| m.max(v.abs()));
        assert!(
            quiet < 0.05 * loud,
            "{tempo} BPM: still before the line ({quiet} of {loud})"
        );
    }
}

#[test]
fn a_tracks_notes_choose_the_pattern() {
    // The bass plays notes 36 and 43: with time pattern 1's note at 35 they play time 2 (a
    // tape stop) and 9 (half time every beat), and from volume 1's at 24 volume 13 (a pump)
    // and 20 (a fade out), while held.
    let mut p = demo_project();
    let bass = p.tracks[1].id;
    let fx = add_effect(&mut p, EffectKind::TimeShaper);
    set(&mut p, fx, "tkey", 35.0);
    let wire = render_of(&p);
    assert_null(
        &render_of(&demo_project()),
        &wire,
        0.0,
        "no notes routed: a wire",
    );
    strip_of(&mut p, 0)
        .effects
        .iter_mut()
        .find(|e| e.id == fx)
        .unwrap()
        .midi_from = Some(bass);
    let heard = render_of(&p);
    assert!(peak_diff(&heard, &wire) > 0.02, "the notes switch patterns");
    set(&mut p, fx, "notes", 0.0);
    assert_null(&wire, &render_of(&p), 0.0, "notes off: a wire again");
}

#[test]
fn its_lookahead_delays_the_whole_mix_by_its_latency_and_nothing_else() {
    // Through with 20 ms of lookahead: the lead's strip is 960 samples late, the engine
    // delays the bass to meet it, and the master is the dry mix 960 samples on, to the bit.
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::TimeShaper);
    set(&mut p, fx, "ahead", 20.0);
    let out = render_of(&p);
    let lat = 960 * 2;
    let n = reference.len().min(out.len());
    assert!(out[..lat].iter().all(|v| *v == 0.0));
    assert_null(&reference[..n - lat], &out[lat..n], 0.0, "960 samples late");
}
