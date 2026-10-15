//! Tremor through the whole engine (ADR 0055): a wire at its defaults and at depth 0 in
//! every mode, the same render at every block size while it works, live equal to offline,
//! and a synced LFO that follows the song position — from the start of the song and from
//! a later bar.

mod support;
#[path = "support/modfx.rs"]
mod modfx;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use modfx::*;
use support::*;

#[test]
fn tremor_at_its_defaults_renders_exactly_as_no_tremor() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    add_effect(&mut p, EffectKind::Tremolo);
    assert_null(&reference, &render_of(&p), 0.0, "Tremor at its defaults");
}

#[test]
fn at_depth_zero_every_mode_is_a_wire() {
    let reference = render_of(&demo_project());
    for mode in [0.0, 1.0, 2.0] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::Tremolo);
        set_all(
            &mut p,
            fx,
            &[("mode", mode), ("shape", 2.0), ("spread", 90.0), ("swing", 0.5)],
        );
        assert_null(&reference, &render_of(&p), 0.0, &format!("mode {mode}"));
    }
}

fn busy(p: &mut Project, fx: EffectId, mode: f32) {
    set_all(
        p,
        fx,
        &[
            ("mode", mode),
            ("depth", 0.9),
            ("div", 13.0),
            ("shape", 2.0),
            ("swing", 0.3),
            ("spread", 60.0),
            ("out", -1.5),
        ],
    );
}

#[test]
fn the_render_does_not_depend_on_the_block_size_while_it_works() {
    for mode in [0.0, 1.0, 2.0] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::Tremolo);
        busy(&mut p, fx, mode);
        let reference = render_with(&p, 512);
        assert!(reference != render_with(&demo_project(), 512), "it works");
        for block in [1usize, 7, 64, 1000, 4096] {
            assert_null(
                &reference,
                &render_with(&p, block),
                0.0,
                &format!("mode {mode}, block {block}"),
            );
        }
    }
}

#[test]
fn live_playback_renders_what_the_offline_render_does() {
    for sync in [1.0, 0.0] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::Tremolo);
        busy(&mut p, fx, 0.0);
        set_all(&mut p, fx, &[("sync", sync), ("rate", 5.3)]);
        let offline = render_of(&p);
        assert_null(
            &offline,
            &realtime(&p, offline.len()),
            0.0,
            &format!("sync {sync}"),
        );
    }
}

/// A square at full depth, a quarter note long: the lead passes untouched through the
/// first half of every beat of the song and is silent through the second.
fn gated(p: &mut Project) {
    let fx = add_effect(p, EffectKind::Tremolo);
    set_all(
        p,
        fx,
        &[("depth", 1.0), ("div", 8.0), ("shape", 2.0), ("edge", 0.0)],
    );
}

fn check_gate(out: &[f32], reference: &[f32], first_beat_at: f64, what: &str) {
    let frames = out.len() / 2;
    let mut checked = 0;
    let mut beat = 0.0;
    while (first_beat_at + (beat + 1.0) * BEAT) < frames as f64 {
        let at = |share: f64| (first_beat_at + (beat + share) * BEAT) as usize;
        for f in at(0.6)..at(0.9) {
            assert_eq!(out[2 * f], 0.0, "{what}: frame {f} is in a shut half");
        }
        for f in at(0.1)..at(0.4) {
            assert_eq!(
                out[2 * f].to_bits(),
                reference[2 * f].to_bits(),
                "{what}: frame {f} is in an open half"
            );
        }
        checked += 1;
        beat += 1.0;
    }
    assert!(checked >= 4, "{what}: {checked} beats");
}

#[test]
fn a_synced_gate_follows_the_song_from_the_start_and_from_a_later_bar() {
    let base = lead_only();
    let mut p = base.clone();
    gated(&mut p);
    let reference = render_with(&base, 256);
    check_gate(&render_with(&p, 256), &reference, 0.0, "from the start");
    // From beat 3: the render's first sample is on a beat, so the halves line up again.
    let start = Ticks::beats(3);
    let reference = render_from(&base, start, 300);
    check_gate(&render_from(&p, start, 300), &reference, 0.0, "from beat 3");
}

#[test]
fn the_unit_hears_the_projects_tempo() {
    let mut base = lead_only();
    base.tempo_bpm = 90.0;
    let mut p = base.clone();
    gated(&mut p);
    let out = render_with(&p, 256);
    // At 90 BPM a beat is 32 000 samples: frame 20 000 of the first beat is shut (at 128
    // BPM it would be open, in the second beat's first half).
    let f = (0.75 * 32_000.0) as usize;
    assert_eq!(out[2 * f], 0.0);
    assert!(render_with(&base, 256)[2 * f] != 0.0, "the lead plays there");
}
