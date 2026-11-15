//! Espectro hears the same music on screen and in a video (round 40, ADR 0122): the four
//! bands a video export analyses from its own render (`dance_live::video_spectrum`, offline,
//! on the video's instants) are the live analyser's (the companion's `Visualiser`, fed as
//! the tap feeds it) on the same audio, within a stated tolerance — the two analyse at 30
//! and 20 instants a second, so a band can differ by what it falls between two analyses
//! (1.4 of the scale a second × 1/20 s) plus the windows' offset. And the export's are a
//! pure function of the render: two exports, the same bands.

use cantus_dsp::math;
use cantus_ui::dance_live::{spectrum_bands, video_spectrum};
use cantus_ui::stage::vis::Visualiser;
use cantus_ui::stage::vis_video::analyse;

const SR: u32 = 48_000;
/// What the live and the offline bands may differ by, 0…1 of the scale.
const TOLERANCE: f32 = 0.15;

/// Three seconds of a song-like sound: a low thump on every beat (120 BPM), a bright tick
/// on every eighth, a mid tone throughout — interleaved stereo.
fn song() -> Vec<f32> {
    let n = 3 * SR as usize;
    let mut out = Vec::with_capacity(n * 2);
    for i in 0..n {
        let t = i as f32 / SR as f32;
        let beat = (t * 2.0).fract();
        let eighth = (t * 4.0).fract();
        let tau = 2.0 * std::f32::consts::PI;
        let thump = math::exp(-beat * 6.0) * math::sin(tau * 70.0 * t);
        let tick = math::exp(-eighth * 12.0) * math::sin(tau * 9_000.0 * t);
        let tone = 0.15 * math::sin(tau * 600.0 * t);
        let s = 0.6 * thump + 0.3 * tick + tone;
        out.push(s);
        out.push(s);
    }
    out
}

#[test]
fn an_exports_bands_are_the_live_analysers_on_the_same_audio() {
    let samples = song();
    let video = video_spectrum(&analyse(&samples, 2, SR));
    // The live analyser, fed a frame's worth at a time at 60 frames a second.
    let mut live = Visualiser::default();
    let per_frame = SR as usize / 60;
    let mut worst = 0.0f32;
    let mut compared = 0;
    for frame in 0..(3 * 60) {
        let from = frame * per_frame * 2;
        let to = ((frame + 1) * per_frame * 2).min(samples.len());
        live.feed(&samples[from..to]);
        let now = (frame + 1) as f64 / 60.0;
        live.update(now, SR as f32, 0.5);
        if now < 0.5 {
            continue; // the first analysis windows fill
        }
        let a = spectrum_bands(&live.bands);
        for (band, x) in a.iter().enumerate() {
            let nearest = (-30..=30)
                .map(|k| (x - video.at(now + k as f64 / 600.0)[band]).abs())
                .fold(f32::MAX, f32::min);
            worst = worst.max(nearest);
        }
        compared += 1;
    }
    assert!(compared > 100);
    assert!(
        worst <= TOLERANCE,
        "the live and the export's bands differ by {worst:.3} at most (tolerance {TOLERANCE})"
    );
    // The sound reaches the bands: the lows thump, the highs tick.
    let peak = |band: usize| {
        (0..90)
            .map(|k| video.at(0.5 + k as f64 / 36.0)[band])
            .fold(0.0f32, f32::max)
    };
    assert!(peak(0) > 0.5 && peak(3) > 0.3, "{} {}", peak(0), peak(3));
}

#[test]
fn two_exports_hear_the_same_bands() {
    let samples = song();
    let a = video_spectrum(&analyse(&samples, 2, SR));
    let b = video_spectrum(&analyse(&samples, 2, SR));
    assert_eq!(a, b);
    for k in 0..90 {
        let t = k as f64 / 30.0;
        assert_eq!(a.at(t), b.at(t));
    }
}

#[test]
fn a_video_draws_espectro_with_the_renders_bands() {
    use cantus_dance::render::{Background, Stage};
    use cantus_video::dance::{Dancer, SongFrames};
    let samples = song();
    let ch = cantus_dance::cast::by_id("builtin:espectro").expect("Espectro");
    assert!(ch.hears_spectrum());
    let tl: Vec<cantus_dance::DanceFrame> = (0..300)
        .map(|i| cantus_dance::DanceFrame {
            seconds: i as f64 * 0.01,
            beats: i as f64 * 0.02,
            tempo_bpm: 120.0,
            playing: true,
            bar_beats: 4.0,
            ..cantus_dance::DanceFrame::default()
        })
        .collect();
    let frames = |spectrum| SongFrames {
        dancers: vec![Dancer {
            character: ch.clone(),
            timeline: tl.clone(),
        }],
        stage: Stage::new(200, 200, Background::Transparent),
        start_seconds: 0.0,
        fps: 30,
        overlay: None,
        spectrum,
    };
    let heard = frames(Some(video_spectrum(&analyse(&samples, 2, SR))));
    let p = heard.poses(45)[0];
    assert!(p.bands.iter().any(|b| *b > 0.0), "{:?}", p.bands);
    // Without the render's spectrum the bars dance to the beat-made one.
    assert_eq!(frames(None).poses(45)[0].bands, [0.0; 4]);
}
