//! VCA faders on the audio thread (ADR 0093): a VCA at unity is a null to the bit; a
//! VCA's fader is the members' gain stage (the same samples as moving the members' own
//! faders); a ridden VCA is block-size independent at tolerance 0 and the same samples as
//! the ride drawn on each member; mute follows; the VCA itself costs nothing.

mod support;

use cantus_core::automation::range::GAIN;
use cantus_core::vca::StripKind;
use cantus_core::*;
use support::*;

/// The demo project with a VCA after the sixteen strips, both tracks' strips following it.
fn with_vca(db: f32) -> (Project, StripId) {
    let mut p = demo_project();
    let mut v = p.make_strip("VCA");
    v.kind = StripKind::Vca;
    v.gain_db = db;
    let id = v.id;
    p.strips.push(v);
    for t in 0..2 {
        strip_of(&mut p, t).vcas.push(id);
    }
    (p, id)
}

/// The curve's position and value are a round trip (a taper), so a ride drawn on a member and
/// one summed from a VCA agree to rounding, not to the bit: a few millionths of full scale.
fn near(a: &[f32], b: &[f32]) {
    assert_eq!(a.len(), b.len());
    let worst = a
        .iter()
        .zip(b)
        .fold(0.0f32, |m, (x, y)| m.max((x - y).abs()));
    assert!(worst < 2e-5, "worst difference {worst}");
}

fn bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

#[test]
fn a_vca_at_unity_is_a_null_to_the_bit() {
    let plain = render_of(&demo_project());
    let (p, _) = with_vca(0.0);
    assert_eq!(bits(&render_of(&p)), bits(&plain));
    assert!(
        plain.iter().any(|x| x.abs() > 0.01),
        "the project makes sound"
    );
}

#[test]
fn a_vca_fader_is_the_members_gain_stage() {
    let (p, _) = with_vca(-6.0);
    let mut moved = demo_project();
    for t in 0..2 {
        strip_of(&mut moved, t).gain_db += -6.0;
    }
    assert_eq!(bits(&render_of(&p)), bits(&render_of(&moved)));
    // And the VCA is not the members' own fader value: it still reads what it did.
    assert_eq!(strip_of(&mut p.clone(), 0).gain_db, -3.0);
}

#[test]
fn a_muted_vca_silences_its_members() {
    let (mut p, v) = with_vca(0.0);
    p.strip_mut(v).unwrap().muted = true;
    let mut muted = demo_project();
    for t in 0..2 {
        strip_of(&mut muted, t).muted = true;
    }
    assert_eq!(bits(&render_of(&p)), bits(&render_of(&muted)));
}

fn step(p: &mut Project, track: usize, target: AutoTarget, db_a: f32, db_b: f32) {
    let c = p.make_automation_clip(
        "ride",
        target,
        Ticks::ZERO,
        Ticks::beats(8),
        vec![
            AutoPoint {
                tick: Ticks::ZERO,
                value: GAIN.pos(db_a),
                tension: 0.0,
                shape: Shape::Hold,
            },
            AutoPoint {
                tick: Ticks::beats(4),
                value: GAIN.pos(db_b),
                tension: 0.0,
                shape: Shape::Hold,
            },
        ],
    );
    p.tracks[track].clips.push(c);
}

#[test]
fn a_ridden_vca_is_the_ride_drawn_on_each_member_and_block_size_independent() {
    let (mut p, v) = with_vca(0.0);
    // The VCA steps 0 dB then -12 dB at beat 4; its project value stays 0.
    step(&mut p, 0, AutoTarget::StripGain(v), 0.0, -12.0);
    // The same ride drawn on each member (their faders at -3 dB and 0 dB): the sum.
    let mut drawn = demo_project();
    for t in 0..2 {
        let base = strip_of(&mut drawn, t).gain_db;
        let id = drawn.tracks[t].strip.unwrap();
        step(&mut drawn, 0, AutoTarget::StripGain(id), base, base - 12.0);
    }
    let a = render_with(&p, 256);
    near(&a, &render_with(&drawn, 256));
    for block in [16, 64, 333, 1024] {
        assert_eq!(bits(&render_with(&p, block)), bits(&a), "block {block}");
    }
    // It really moved: the second half is quieter than the first.
    let half = a.len() / 2;
    let peak = |s: &[f32]| s.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    assert!(peak(&a[half..]) < peak(&a[..half]) * 0.6);
}

#[test]
fn a_member_with_its_own_ride_adds_the_vca_to_it() {
    let (mut p, _) = with_vca(-6.0);
    let id = p.tracks[0].strip.unwrap();
    step(&mut p, 0, AutoTarget::StripGain(id), 0.0, -9.0);
    let mut drawn = demo_project();
    for t in 0..2 {
        strip_of(&mut drawn, t).gain_db += -6.0;
    }
    step(&mut drawn, 0, AutoTarget::StripGain(id), -6.0, -15.0);
    near(&render_with(&p, 128), &render_with(&drawn, 128));
}

#[test]
fn a_cue_recalls_the_snapshot_at_its_bar_in_the_render() {
    use cantus_core::mix_snap::{capture, MixCue, MixCueId, MixSnapId, MixSnapshot};
    // The snapshot: both strips 12 dB down. The cue plays it from beat 4.
    let mut p = demo_project();
    let mut quiet = p.clone();
    for t in 0..2 {
        strip_of(&mut quiet, t).gain_db -= 12.0;
    }
    let state = capture(&quiet, &[], false);
    let sid = MixSnapId(p.alloc_id());
    p.mix.snapshots.push(MixSnapshot {
        id: sid,
        name: "quiet".into(),
        state,
    });
    let cue = MixCueId(p.alloc_id());
    p.mix.cues.push(MixCue {
        id: cue,
        at: Ticks::beats(4),
        snapshot: sid,
        scope: vec![],
    });
    // The same thing drawn as a ride on each strip.
    let mut drawn = demo_project();
    for t in 0..2 {
        let base = strip_of(&mut drawn, t).gain_db;
        let id = drawn.tracks[t].strip.unwrap();
        step(&mut drawn, 0, AutoTarget::StripGain(id), base, base - 12.0);
    }
    let a = render_with(&p, 256);
    near(&a, &render_with(&drawn, 256));
    for block in [16, 777] {
        assert_eq!(bits(&render_with(&p, block)), bits(&a), "block {block}");
    }
    // Without the cue the project renders as it always did.
    p.mix.cues.clear();
    assert_eq!(bits(&render_of(&p)), bits(&render_of(&demo_project())));
}

// ---- review 1 ---------------------------------------------------------------------------

/// Item 3, on the audio thread: a member ridden to the bottom under a raised VCA is silent,
/// and so is a member under a VCA ridden to the bottom. Failed before: −96 + 12 = −84 dB.
#[test]
fn the_floor_holds_on_a_ride() {
    use cantus_core::{MAX_GAIN_DB, MIN_GAIN_DB};
    let mut muted = demo_project();
    strip_of(&mut muted, 0).muted = true;
    let silent = render_with(&muted, 256);
    // The member's own ride at the bottom, the VCA at the top.
    let (mut p, _) = with_vca(MAX_GAIN_DB);
    let id = p.tracks[0].strip.unwrap();
    step(
        &mut p,
        0,
        AutoTarget::StripGain(id),
        MIN_GAIN_DB,
        MIN_GAIN_DB,
    );
    strip_of(&mut p, 1).vcas.clear();
    assert_eq!(bits(&render_with(&p, 256)), bits(&silent));
    // The VCA ridden to the bottom, the member at the top.
    let (mut p, v) = with_vca(0.0);
    strip_of(&mut p, 0).gain_db = MAX_GAIN_DB;
    strip_of(&mut p, 1).vcas.clear();
    step(
        &mut p,
        0,
        AutoTarget::StripGain(v),
        MIN_GAIN_DB,
        MIN_GAIN_DB,
    );
    assert_eq!(bits(&render_with(&p, 256)), bits(&silent));
}

/// A cue at beat 4 of a snapshot where `edit` changed some switches.
fn cued(edit: impl Fn(&mut cantus_core::mix_snap::MixState, StripId)) -> Project {
    use cantus_core::mix_snap::{capture, MixCue, MixCueId, MixSnapId, MixSnapshot};
    let mut p = demo_project();
    let lead = p.tracks[0].strip.unwrap();
    let mut state = capture(&p, &[], true);
    edit(&mut state, lead);
    let sid = MixSnapId(p.alloc_id());
    p.mix.snapshots.push(MixSnapshot {
        id: sid,
        name: "s".into(),
        state,
    });
    let cue = MixCueId(p.alloc_id());
    p.mix.cues.push(MixCue {
        id: cue,
        at: Ticks::beats(4),
        snapshot: sid,
        scope: vec![],
    });
    p
}

/// Beat 4 at 128 BPM and 48 kHz, in frames: on the 16-sample grid.
const CUE: usize = 90_000;

/// Item 5: a cue's mute switches at its grid point, faded out over the 16 samples before —
/// the same samples as the project before it, as the project with the strip muted after it,
/// and in between each sample nearer the muted one than the unmuted one is, down to a
/// sixteenth of the strip at the last; block-size independent. And a cue that switches
/// nothing is a null to the bit.
#[test]
fn a_cue_mutes_at_its_grid_point_with_a_fade() {
    let plain = render_with(&demo_project(), 256);
    let none = cued(|_, _| {});
    assert_eq!(bits(&render_with(&none, 256)), bits(&plain));
    let p = cued(|s, lead| s.strips.iter_mut().find(|x| x.id == lead).unwrap().muted = true);
    let out = render_with(&p, 256);
    let mut m = demo_project();
    strip_of(&mut m, 0).muted = true;
    let muted = render_with(&m, 256);
    let f = |frame: usize| frame * 2;
    assert_eq!(bits(&out[..f(CUE - 16)]), bits(&plain[..f(CUE - 16)]));
    assert_eq!(bits(&out[f(CUE)..]), bits(&muted[f(CUE)..]));
    for i in f(CUE - 16)..f(CUE) {
        let (o, p, q) = (out[i], plain[i], muted[i]);
        assert!(
            (o - q).abs() <= (p - q).abs() + 1e-7,
            "sample {i}: fades toward muted"
        );
    }
    let last = f(CUE) - 2;
    assert!((out[last] - muted[last]).abs() <= (plain[last] - muted[last]).abs() / 15.0 + 1e-7);
    for block in [16, 64, 333, 1024] {
        assert_eq!(bits(&render_with(&p, block)), bits(&out), "block {block}");
    }
}

/// Item 5: polarity flips at the cue, faded in after it: from the fade's end, the project
/// with the lead's polarity flipped.
#[test]
fn a_cue_flips_polarity_with_a_fade_in() {
    let p = cued(|s, lead| {
        s.strips
            .iter_mut()
            .find(|x| x.id == lead)
            .unwrap()
            .phase_invert = true
    });
    let out = render_with(&p, 256);
    let plain = render_with(&demo_project(), 256);
    let mut flipped = demo_project();
    strip_of(&mut flipped, 0).phase_invert = true;
    let flipped = render_with(&flipped, 256);
    assert_eq!(bits(&out[..(CUE - 16) * 2]), bits(&plain[..(CUE - 16) * 2]));
    assert_eq!(
        bits(&out[(CUE + 16) * 2..]),
        bits(&flipped[(CUE + 16) * 2..])
    );
    assert_eq!(bits(&render_with(&p, 777)), bits(&out));
}

/// Review 1, item 4: what a full-desk cue costs — 64 strips, each with an EQ and a compressor,
/// a cue that moves every fader. `cargo test --release -p cantus-engine --test vca --
/// --ignored --nocapture full_desk`; the numbers are in `docs/round36/mix.md`.
#[test]
#[ignore]
fn full_desk_cue_cost() {
    use cantus_core::mix_snap::{capture, MixCue, MixCueId, MixSnapId, MixSnapshot};
    let mut p = demo_project();
    while p.strips.len() < 64 {
        let s = p.make_strip("");
        p.strips.push(s);
    }
    let mut params = 0;
    for i in 0..64 {
        for kind in [EffectKind::Eq, EffectKind::Compressor] {
            let fx = p.make_effect(kind);
            params += kind.params().len();
            p.strips[i].effects.push(fx);
        }
    }
    let mut state = capture(&p, &[], true);
    for s in &mut state.strips {
        s.gain_db -= 3.0;
        s.muted = !s.muted;
    }
    let sid = MixSnapId(p.alloc_id());
    p.mix.snapshots.push(MixSnapshot {
        id: sid,
        name: "all".into(),
        state,
    });
    let cue = MixCueId(p.alloc_id());
    p.mix.cues.push(MixCue {
        id: cue,
        at: Ticks::beats(4),
        snapshot: sid,
        scope: vec![],
    });
    let time = |f: &mut dyn FnMut()| {
        let mut best = f64::MAX;
        for _ in 0..5 {
            let t = std::time::Instant::now();
            f();
            best = best.min(t.elapsed().as_secs_f64() * 1e3);
        }
        best
    };
    let mut snap = None;
    let compile = time(&mut || snap = Some(Snapshot::compile(&p, 1)));
    let s = snap.unwrap();
    let mut q = p.clone();
    q.mix.cues.clear();
    let plain = time(&mut || {
        let _ = Snapshot::compile(&q, 1);
    });
    let render_cue = time(&mut || {
        let _ = render_with(&p, 256);
    });
    let render_plain = time(&mut || {
        let _ = render_with(&q, 256);
    });
    println!(
        "full-desk cue, 64 strips: {} lanes (of {} values a cue of the whole desk could hold), {} regions; compile {compile:.2} ms (without the cue {plain:.2} ms); render of the song {render_cue:.1} ms (without {render_plain:.1} ms)",
        s.automation.len(),
        64 * 3 + params,
        s.mix_scenes.len(),
    );
    assert_eq!(
        s.automation.len(),
        64,
        "one lane per moved fader, nothing else"
    );
}
