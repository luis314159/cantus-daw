//! Fluctus through the whole engine (ADR 0055): a wire at its defaults and at depth 0, the
//! pump on the song's beats (from the start and from a later beat), the same render at
//! every block size while it works, and live equal to offline.

mod support;
#[path = "support/modfx.rs"]
mod modfx;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use modfx::*;
use support::*;

#[test]
fn fluctus_at_its_defaults_renders_exactly_as_no_fluctus() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    add_effect(&mut p, EffectKind::VolumeShaper);
    assert_null(&reference, &render_of(&p), 0.0, "Fluctus at its defaults");
}

#[test]
fn at_depth_zero_any_curve_is_a_wire() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::VolumeShaper);
    set_all(
        &mut p,
        fx,
        &[
            ("div", 14.0),
            ("p2.on", 1.0),
            ("p2.x", 0.8),
            ("p2.y", 0.2),
            ("p2.s", 4.0),
            ("smooth", 0.0),
        ],
    );
    assert_null(&reference, &render_of(&p), 0.0, "depth 0");
}

/// The pump at full depth and no smoothing: silent on each beat of the song, untouched
/// (to the bit) through the held second half.
fn check_pump(out: &[f32], reference: &[f32], what: &str) {
    let frames = out.len() / 2;
    let mut beat = 0.0;
    let mut checked = 0;
    while (beat + 1.0) * BEAT < frames as f64 {
        let at = |share: f64| ((beat + share) * BEAT) as usize;
        assert_eq!(out[2 * at(0.0)], 0.0, "{what}: beat {beat} starts silent");
        for f in at(0.55)..at(0.95) {
            assert_eq!(
                out[2 * f].to_bits(),
                reference[2 * f].to_bits(),
                "{what}: frame {f} is held open"
            );
        }
        checked += 1;
        beat += 1.0;
    }
    assert!(checked >= 4, "{what}");
}

#[test]
fn the_pump_follows_the_songs_beats_from_the_start_and_from_a_later_beat() {
    let base = lead_only();
    let mut p = base.clone();
    let fx = add_effect(&mut p, EffectKind::VolumeShaper);
    set_all(&mut p, fx, &[("depth", 1.0), ("smooth", 0.0)]);
    check_pump(&render_with(&p, 256), &render_with(&base, 256), "from the start");
    let start = Ticks::beats(3);
    check_pump(
        &render_from(&p, start, 300),
        &render_from(&base, start, 300),
        "from beat 3",
    );
}

#[test]
fn the_render_does_not_depend_on_the_block_size_while_it_works() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::VolumeShaper);
    set_all(
        &mut p,
        fx,
        &[
            ("depth", 0.85),
            ("div", 11.0),
            ("offset", 0.2),
            ("smooth", 3.0),
            ("p2.on", 1.0),
            ("p2.x", 0.75),
            ("p2.y", 0.4),
            ("p2.s", 1.0),
            ("p2.t", 0.6),
        ],
    );
    let reference = render_with(&p, 512);
    assert!(reference != render_with(&demo_project(), 512), "it works");
    for block in [1usize, 7, 64, 1000, 4096] {
        assert_null(&reference, &render_with(&p, block), 0.0, &format!("block {block}"));
    }
}

#[test]
fn live_playback_renders_what_the_offline_render_does() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::VolumeShaper);
    set_all(&mut p, fx, &[("depth", 0.9), ("div", 11.0), ("smooth", 2.0)]);
    let offline = render_of(&p);
    assert_null(&offline, &realtime(&p, offline.len()), 0.0, "live");
}
