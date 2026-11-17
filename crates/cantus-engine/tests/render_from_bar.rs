//! A render that starts mid-song is the whole render from there (offline = real time's
//! rule, ADR 0005's null tests): for a native effect's automated parameter it is not yet.
//!
//! Found by wave D's e2e (`crates/cantus-ui/tests/fxd_e2e.rs`,
//! `a_path_plays_in_time_with_the_song`) and **not fixed there**: a native effect's unit is
//! made at its *stored* values, and at the first control point of a render that starts
//! later its own de-zipper (Aequalis's, about 20 ms) glides from the stored value to the
//! automated one — so the first note after the start differs from the whole render's (up
//! to 5.2e-2 measured for an EQ band held at +9 dB by a lane). The strip's pan and fader
//! take their value at the first control point and are bit-exact from any bar. Fixing it
//! (the unit made, or snapped, at the automated value where the render starts) likely
//! changes golden hashes and so the `DSP_VERSION`: the integrator's, with the owner.

use cantus_core::automation::range::range_of;
use cantus_core::*;
use cantus_engine::*;
use std::sync::Arc;

const SR: u32 = 48_000;
/// 120 BPM: a beat is half a second.
const FRAMES_PER_BEAT: usize = SR as usize / 2;
const BEATS: i64 = 16;
/// Bar 3 (4/4).
const FROM_BEAT: i64 = 8;

/// A Firmus track playing A3 and A6, a sixteenth each, on every beat (Firmus's cutoff and
/// level left at its defaults: its own smoothers start there, the other finding of the
/// e2e), an Aequalis bell at 220 Hz on its strip stored flat (0 dB) and a lane holding that
/// band at +9 dB for the whole song.
fn song() -> Project {
    let mut p = Project::new("render from bar N");
    p.tempo_bpm = 120.0;
    let mut lead = p.make_track("Lead");
    let f = lead.instrument.firmus_mut().expect("Firmus");
    f.osc1_wave = Waveform::Sine;
    f.osc2_wave = Waveform::Sine;
    f.osc_mix = 0.0;
    f.osc2_detune_cents = 0.0;
    f.resonance = 0.0;
    f.filter_env_octaves = 0.0;
    f.key_track = 1.0;
    f.amp_attack_s = 0.0;
    f.amp_decay_s = 0.0;
    f.amp_sustain = 1.0;
    f.amp_release_s = 0.02;
    let mut clip = p.make_clip("beats", Ticks::ZERO, Ticks::beats(BEATS));
    for b in 0..BEATS {
        for k in [57u8, 93] {
            let n = p.make_note(Ticks::beats(b), Ticks(PPQ / 4), k, 100);
            clip.notes.push(n);
        }
    }
    lead.clips.push(clip);
    let strip = lead.strip.expect("a strip");
    p.tracks.push(lead);
    let mut eq = p.make_effect(EffectKind::Eq);
    for (k, v) in [
        ("b0.used", 1.0),
        ("b0.freq", 220.0),
        ("b0.q", 2.0),
        ("b0.gain", 0.0),
    ] {
        eq.spec.set(k, v).expect("an EQ parameter");
    }
    let id = eq.id;
    p.strip_mut(strip).unwrap().effects.push(eq);
    let target = AutoTarget::Effect {
        effect: id,
        key: "b0.gain".into(),
    };
    let held = range_of(&p, &target).expect("the band's range").pos(9.0);
    let lane = p.make_automation_clip(
        "band 0 at +9 dB",
        target,
        Ticks::ZERO,
        Ticks::beats(BEATS),
        vec![
            AutoPoint::new(Ticks::ZERO, held),
            AutoPoint::new(Ticks::beats(BEATS), held),
        ],
    );
    p.tracks[0].clips.push(lane);
    p
}

fn render_beats(p: &Project, from: i64, to: i64) -> Vec<f32> {
    let opts = RenderOptions {
        start: Ticks::beats(from),
        end: Some(Ticks::beats(to)),
        tail_seconds: 0.0,
        ..Default::default()
    };
    render(Arc::new(Snapshot::compile(p, 1)), &opts)
        .expect("the render")
        .samples
}

fn worst(a: &[f32], b: &[f32]) -> (usize, f32) {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .enumerate()
        .fold((0, 0.0), |m, (i, d)| if d > m.1 { (i / 2, d) } else { m })
}

/// From bar 3 the render is the whole render's bars 3… to the bit — every sample, the
/// first note's too — with an EQ band automated.
#[test]
#[ignore = "pre-existing (wave D's e2e finding 1): a native effect starts a render at its \
            stored value and glides ~20 ms to its automated one; the fix is the integrator's \
            (it likely moves the goldens and the DSP_VERSION) — un-ignore it with the fix"]
fn a_render_from_bar_3_is_the_whole_render_from_bar_3_with_an_effect_automated() {
    let p = song();
    let whole = render_beats(&p, 0, BEATS);
    let late = render_beats(&p, FROM_BEAT, BEATS);
    let tail = &whole[FROM_BEAT as usize * FRAMES_PER_BEAT * 2..];
    assert_eq!(late.len(), tail.len(), "the same length from bar 3");
    // The lane is heard: band 0 at +9 dB is not the flat EQ.
    let mut flat = p.clone();
    flat.tracks[0].clips.retain(|c| c.automation.is_none());
    let dry = render_beats(&flat, 0, BEATS);
    assert!(worst(&whole, &dry).1 > 0.1, "the lane moves the band");
    // From the second beat of bar 3 on it is already the whole render's, to the bit.
    let one = FRAMES_PER_BEAT * 2;
    let (at, d) = worst(&late[one..], &tail[one..]);
    assert_eq!(d, 0.0, "from beat 9 on: {d:e} at frame {}", at + FRAMES_PER_BEAT);
    // The first note of bar 3: the unit should start at the automated +9 dB, not glide
    // there from its stored 0 dB (measured before the fix: up to ~5e-2 in its first 20 ms).
    let (at, d) = worst(&late[..one], &tail[..one]);
    assert_eq!(
        d, 0.0,
        "bar 3's first beat differs by {d:e} at frame {at} ({:.1} ms after the start)",
        at as f64 * 1_000.0 / f64::from(SR)
    );
}
