//! Tonsor through the whole engine (ADR 0056): a wire at its defaults; with no overs, at
//! every oversampling and with the true-peak guard, the same render later by exactly the
//! latency it reports (delay compensation); the same render at every block size while it
//! clips; live equal to offline; and a latency that changes under automation re-plans the
//! compensation, so the mix stays in line.

#[path = "support/modfx.rs"]
mod modfx;
mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use modfx::*;
use support::*;

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0f32, |m, v| m.max(v.abs()))
}

#[test]
fn tonsor_at_its_defaults_renders_exactly_as_no_tonsor() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    add_effect(&mut p, EffectKind::Clipper);
    assert_null(&reference, &render_of(&p), 0.0, "Tonsor at its defaults");
}

/// With nothing over the ceiling, every configuration is a delay of exactly its latency —
/// the whole mix waits for it (the bass too), so the render is the same render later.
#[test]
fn with_no_overs_every_configuration_is_the_render_later_by_its_latency() {
    let base = demo_project();
    let reference = render_of(&base);
    assert!(peak(&reference) < 0.9, "the demo has no overs");
    for (os, tp) in [(1.0, 0.0), (4.0, 0.0), (0.0, 1.0), (4.0, 1.0)] {
        let mut p = base.clone();
        let fx = add_effect(&mut p, EffectKind::Clipper);
        set_all(&mut p, fx, &[("os", os), ("tp", tp), ("curve", 0.0)]);
        let slot = strip_of(&mut p, 0).effects[0].clone();
        let lat = cantus_engine::EffectUnit::new(&slot, 48_000.0).latency();
        let want = cantus_dsp::tonsor::latency_for(&cantus_engine::fx::tonsor::settings(&slot.spec));
        assert_eq!(lat, want);
        assert!(lat > 0);
        let out = render_of(&p);
        let shift = 2 * lat;
        assert!(out[..shift].iter().all(|x| *x == 0.0), "os {os} tp {tp}");
        assert_null(
            &reference[..reference.len() - shift],
            &out[shift..],
            0.0,
            &format!("os {os} tp {tp}"),
        );
    }
}

fn hot(p: &mut Project, fx: EffectId, os: f32, tp: f32) {
    set_all(
        p,
        fx,
        &[
            ("drive", 14.0),
            ("ceil", -2.0),
            ("knee", 3.0),
            ("curve", 2.0),
            ("os", os),
            ("tp", tp),
            ("match", 1.0),
            ("out", 1.0),
        ],
    );
}

#[test]
fn the_render_does_not_depend_on_the_block_size_while_it_clips() {
    for (os, tp) in [(0.0, 0.0), (2.0, 1.0), (4.0, 0.0)] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::Clipper);
        hot(&mut p, fx, os, tp);
        let reference = render_with(&p, 512);
        assert!(reference != render_with(&demo_project(), 512), "it clips");
        for block in [1usize, 7, 64, 1000, 4096] {
            assert_null(
                &reference,
                &render_with(&p, block),
                0.0,
                &format!("os {os} tp {tp}, block {block}"),
            );
        }
    }
}

#[test]
fn live_playback_renders_what_the_offline_render_does() {
    let mut p = demo_project();
    let fx = add_effect(&mut p, EffectKind::Clipper);
    hot(&mut p, fx, 3.0, 1.0);
    let offline = render_of(&p);
    assert_null(
        &offline,
        &realtime(&p, offline.len()),
        0.0,
        "live = offline",
    );
}

/// A latency that moves under automation — the oversampling switched from 1× to 16× at
/// beat 4 and back at beat 6 — re-plans the compensation as an edit would: once the new
/// lines have filled, the whole mix is the reference later by the new latency (the bass on
/// its own strip waits too), and in line again after the switch back.
#[test]
fn an_automated_latency_keeps_the_mix_in_line() {
    let base = demo_project();
    let reference = render_of(&base);
    let mut p = base.clone();
    let fx = add_effect(&mut p, EffectKind::Clipper);
    let mut up = AutoPoint::new(Ticks::ZERO, 0.0);
    up.shape = Shape::Hold;
    let mut on = AutoPoint::new(Ticks::beats(4), 1.0);
    on.shape = Shape::Hold;
    let mut off = AutoPoint::new(Ticks::beats(6), 0.0);
    off.shape = Shape::Hold;
    let clip = p.make_automation_clip(
        "os",
        AutoTarget::Effect {
            effect: fx,
            key: "os".into(),
        },
        Ticks::ZERO,
        Ticks::beats(8),
        vec![up, on, off],
    );
    p.tracks[0].clips.push(clip);
    let out = render_of(&p);
    let lat = cantus_dsp::dynfx_os::latency(16);
    let beat = BEAT as usize;
    // Inside the 16× stretch, after the lines filled: everything is `lat` later.
    let (a, b) = (4 * beat + 4 * lat, 6 * beat - 16);
    for f in a..b {
        for c in 0..2 {
            assert_eq!(
                out[2 * f + c].to_bits(),
                reference[2 * (f - lat) + c].to_bits(),
                "frame {f} (channel {c}) in the 16x stretch"
            );
        }
    }
    // After the switch back, in line again.
    let (a, b) = (6 * beat + 4 * lat + 16, 8 * beat);
    for f in a..b {
        assert_eq!(out[2 * f].to_bits(), reference[2 * f].to_bits(), "frame {f} after");
    }
}
