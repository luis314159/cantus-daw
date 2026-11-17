//! Vetus through the whole engine (ADR 0130): a wire at its defaults and with
//! every module off whatever else is set, every module heard, the same render at every block
//! size, live equal to offline, and the noise and the age from bar 5 what the whole render
//! has there — to the bit.

#[path = "support/modfx.rs"]
mod modfx;
mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use modfx::*;
use support::*;

#[test]
fn vetus_at_its_defaults_renders_exactly_as_no_vetus() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    add_effect(&mut p, EffectKind::LoFi);
    assert_null(&reference, &render_of(&p), 0.0, "Vetus at its defaults");
}

#[test]
fn with_every_module_off_anything_else_is_a_wire() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::LoFi);
    set_all(
        &mut p,
        fx,
        &[
            ("char", 1.0),
            ("mix", 0.5),
            ("bits.depth", 2.0),
            ("rate.hz", 2_000.0),
            ("wow", 80.0),
            ("crackle", 1.0),
            ("hiss", 1.0),
            ("tone.medium", 2.0),
        ],
    );
    assert_null(&reference, &render_of(&p), 0.0, "every module off");
}

#[test]
fn twenty_four_bits_without_dither_is_a_wire() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::LoFi);
    set_all(&mut p, fx, &[("bits.on", 1.0), ("bits.depth", 24.0)]);
    assert_null(&reference, &render_of(&p), 0.0, "24 bits");
}

/// The settings the moving tests use: every module on, the wow on the song.
fn worn(p: &mut Project) -> EffectId {
    let fx = add_effect(p, EffectKind::LoFi);
    set_all(
        p,
        fx,
        &[
            ("char", 0.3),
            ("bits.on", 1.0),
            ("bits.depth", 7.0),
            ("bits.dither", 1.0),
            ("rate.on", 1.0),
            ("rate.hz", 10_000.0),
            ("rate.filter", 1.0),
            ("wob.on", 1.0),
            ("wow.sync", 1.0),
            ("wow", 20.0),
            ("age.on", 1.0),
            ("age.drop", 0.8),
            ("noise.on", 1.0),
            ("crackle", 0.6),
            ("hum", 0.3),
            ("tone.on", 1.0),
            ("tone.medium", 1.0),
        ],
    );
    fx
}

#[test]
fn every_module_is_heard() {
    let reference = render_of(&demo_project());
    for key in [
        "bits.on", "rate.on", "wob.on", "age.on", "noise.on", "tone.on",
    ] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::LoFi);
        set_all(&mut p, fx, &[(key, 1.0), ("rate.hz", 5_000.0)]);
        let out = render_of(&p);
        let diff = out
            .iter()
            .zip(&reference)
            .map(|(a, b)| (a - b).abs())
            .fold(0f32, f32::max);
        assert!(diff > 1e-3, "{key} changes the sound: {diff}");
    }
}

#[test]
fn the_render_does_not_depend_on_the_block_size() {
    let mut p = demo_project();
    worn(&mut p);
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
    worn(&mut p);
    let offline = render_of(&p);
    assert_null(&offline, &realtime(&p, offline.len()), 0.0, "live");
}

/// The riff from the start and again from bar 5 (beat 16), with silence between, through
/// `settings` on top of [`worn`]: a render from bar 5 against the whole render's bar 5.
fn from_bar_five(settings: &[(&str, f32)]) {
    let mut p = lead_only();
    p.tempo_bpm = 120.0;
    let mut c = p.tracks[0].clips[0].clone();
    c.id = ClipId(p.alloc_id());
    c.start = Ticks::beats(16);
    p.tracks[0].clips.push(c);
    let fx = worn(&mut p);
    set_all(&mut p, fx, settings);
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

/// From bar 5 the dither, the dropouts, the synced wow and the noise that follows the music
/// are where the whole render's are, to the bit: nothing the filters remember crosses the
/// silence before bar 5.
#[test]
fn a_render_from_bar_five_is_bar_five_of_the_whole_render() {
    from_bar_five(&[("noise.follow", 1.0)]);
}

/// The noise alone, playing through the silence too, is a function of the song: from bar 5
/// it is the whole render's, to the bit.
#[test]
fn the_noise_from_bar_five_is_the_whole_renders() {
    from_bar_five(&[
        ("bits.on", 0.0),
        ("rate.on", 0.0),
        ("wob.on", 0.0),
        ("age.on", 0.0),
        ("tone.on", 0.0),
        ("noise.follow", 0.0),
        ("hiss", 0.4),
    ]);
}
