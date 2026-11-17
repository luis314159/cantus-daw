//! Cribrum through the whole engine (ADR 0127): a wire at its defaults and at
//! mix 0 whatever else is set, a synced sweep that follows the song (from the start and
//! from bar 5), the same render at every block size while it works, live equal to offline,
//! and its latency compensated when it oversamples.

#[path = "support/modfx.rs"]
mod modfx;
mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use modfx::*;
use support::*;

#[test]
fn cribrum_at_its_defaults_renders_exactly_as_no_cribrum() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    add_effect(&mut p, EffectKind::AutoFilter);
    assert_null(&reference, &render_of(&p), 0.0, "Cribrum at its defaults");
}

#[test]
fn at_mix_zero_anything_else_is_a_wire() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::AutoFilter);
    set_all(
        &mut p,
        fx,
        &[
            ("a.model", 1.0),
            ("a.mode", 2.0),
            ("a.cut", 300.0),
            ("a.res", 0.9),
            ("b.on", 1.0),
            ("lfo.cut", 3.0),
            ("env.cut", 2.0),
            ("cur.cut", -2.0),
            ("c4.on", 1.0),
            ("c4.x", 0.3),
        ],
    );
    assert_null(&reference, &render_of(&p), 0.0, "mix 0");
}

/// The settings every moving test uses: a resonant low-pass swept by a synced LFO and the
/// curve, the follower on top.
fn sweep(p: &mut Project) -> EffectId {
    let fx = add_effect(p, EffectKind::AutoFilter);
    set_all(
        p,
        fx,
        &[
            ("mix", 1.0),
            ("a.model", 1.0),
            ("a.slope", 1.0),
            ("a.cut", 600.0),
            ("a.res", 0.6),
            ("lfo.div", 6.0),
            ("lfo.cut", 2.5),
            ("lfo.stereo", 90.0),
            ("env.cut", 1.0),
            ("cur.cut", 1.0),
            ("cur.smooth", 6.0),
        ],
    );
    fx
}

#[test]
fn it_is_heard() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    sweep(&mut p);
    let out = render_of(&p);
    let diff = out
        .iter()
        .zip(&reference)
        .map(|(a, b)| (a - b).abs())
        .fold(0f32, f32::max);
    assert!(diff > 0.05, "the filter changes the sound: {diff}");
}

#[test]
fn the_render_does_not_depend_on_the_block_size_while_it_works() {
    let mut p = demo_project();
    let fx = sweep(&mut p);
    set_all(&mut p, fx, &[("os", 1.0), ("b.on", 1.0), ("b.mode", 5.0)]);
    let reference = render_with(&p, 512);
    for block in [1usize, 7, 64, 1000, 4096] {
        assert_null(
            &reference,
            &render_with(&p, block),
            0.0,
            &format!("block {block}"),
        );
    }
}

#[test]
fn live_playback_renders_what_the_offline_render_does() {
    let mut p = demo_project();
    sweep(&mut p);
    let offline = render_of(&p);
    assert_null(&offline, &realtime(&p, offline.len()), 0.0, "live");
}

/// From bar 5 (beat 16) the sweep is where the whole render's is. The riff plays from the
/// start and again from bar 5, with silence between (so nothing the instrument or the
/// filters remember crosses into bar 5): the two renders are the same bits from bar 5 on.
#[test]
fn a_render_from_bar_five_sweeps_as_bar_five_of_the_whole_render() {
    let mut p = lead_only();
    p.tempo_bpm = 120.0;
    // The riff again from bar 5, so the song runs on past it.
    let mut c = p.tracks[0].clips[0].clone();
    c.id = ClipId(p.alloc_id());
    c.start = Ticks::beats(16);
    p.tracks[0].clips.push(c);
    let fx = sweep(&mut p);
    set_all(&mut p, fx, &[("env.cut", 0.0)]);
    let whole = render_from(&p, Ticks::beats(0), 256);
    let part = render_from(&p, Ticks::beats(16), 300);
    let bar5 = 16 * 24_000; // frames at 120 BPM, 48 kHz
    let n = (part.len() / 2).min(whole.len() / 2 - bar5);
    assert!(n > 96_000, "the song runs past bar 5");
    assert!(part[..2 * n].iter().any(|v| v.abs() > 0.01), "bar 5 sounds");
    assert_null(
        &whole[2 * bar5..2 * (bar5 + n)],
        &part[..2 * n],
        0.0,
        "from bar 5",
    );
}
