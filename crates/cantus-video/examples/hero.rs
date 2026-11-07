//! The family's hero clip (round 35, cast; Fuga and Volta from round 37): the eight
//! characters dancing together on one stage, each its own routine, eight bars at 120 BPM —
//! for the README and the landing page.
//!
//! `cargo run -p cantus-video --release --example hero -- png <dir> [width] [height] [fps] [size] [y]`
//! writes transparent PNG frames (a GIF is assembled from them);
//! `cargo run -p cantus-video --release --example hero -- mp4 <file.mp4> [width] [height] [fps] [size] [y]`
//! exports an MP4 on the Cantus Night stage with a simple beat, through `cantus_video::export`.
//! `size` and `y` are the dancers' own numbers (0.74 and 0.93 by default; the landing page's
//! banner is `1280 400 30 0.98 0.95`).

use cantus_dance::render::{render_into, Background, Stage};
use cantus_dance::{cast, pose_at, Character, DanceFrame};
use cantus_video::audio::Pcm;
use cantus_video::{export, Container, Control, Quality, Request};
use std::path::PathBuf;
use std::sync::Arc;
use tiny_skia::Pixmap;

const BPM: f64 = 120.0;
const BARS: f64 = 8.0;
/// Cantus Night's panel.
const NIGHT: [u8; 4] = [0x1f, 0x17, 0x12, 255];

/// A beat to dance to: a kick on every beat, a soft hat between, a quiet chord.
fn song(rate: u32, seconds: f64) -> Pcm {
    let len = (f64::from(rate) * seconds) as usize;
    let mut s = vec![0.0f32; len * 2];
    let beat = f64::from(rate) * 60.0 / BPM;
    for i in 0..len {
        let t = i as f64 / f64::from(rate);
        let in_beat = (i as f64 % beat) / f64::from(rate);
        let in_half = (i as f64 % (beat / 2.0)) / f64::from(rate);
        let kick = (-in_beat * 30.0).exp()
            * (std::f64::consts::TAU * (50.0 + 90.0 * (-in_beat * 40.0).exp()) * in_beat).sin();
        let hat = if (i as f64 / (beat / 2.0)).floor() as i64 % 2 == 1 {
            (-in_half * 80.0).exp()
                * ((((i as u64).wrapping_mul(2_654_435_761) >> 7) % 2_000) as f64 / 1_000.0 - 1.0)
        } else {
            0.0
        };
        let chord: f64 = [220.0, 277.18, 329.63]
            .iter()
            .map(|f| (std::f64::consts::TAU * f * t).sin())
            .sum::<f64>()
            / 3.0;
        let x = 0.6 * kick + 0.08 * hat + 0.06 * chord;
        s[2 * i] = x as f32;
        s[2 * i + 1] = x as f32;
    }
    Pcm {
        rate,
        channels: 2,
        samples: s,
    }
}

/// The song at `t` seconds, with a dancer standing at `x` across the stage.
fn frame_at(t: f64, x: f32, size: f32, y: f32) -> DanceFrame {
    let beats = t * BPM / 60.0;
    let bar = (beats / 4.0).floor();
    let mut f = DanceFrame {
        beats,
        seconds: t,
        tempo_bpm: BPM as f32,
        playing: true,
        bar: bar as i32,
        bar_start_beats: bar * 4.0,
        bar_beats: 4.0,
        ..DanceFrame::default()
    };
    let key = |k: &str| {
        cantus_core::inst::dancer::KEYS
            .iter()
            .position(|x| *x == k)
            .expect("a key")
    };
    f.values[key("x")] = x;
    f.values[key("y")] = y;
    f.values[key("size")] = size;
    f.values[key("energy")] = 0.85;
    f
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let num = |i: usize, d: u32| a.get(i).and_then(|s| s.parse().ok()).unwrap_or(d);
    let kind = a.get(1).cloned().unwrap_or_else(|| "png".into());
    let out = PathBuf::from(a.get(2).cloned().unwrap_or_else(|| "hero".into()));
    let (w, h, fps) = (num(3, 960), num(4, 360), num(5, 24));
    let real = |i: usize, d: f32| a.get(i).and_then(|s| s.parse().ok()).unwrap_or(d);
    let (size, y) = (real(6, 0.74), real(7, 0.93));
    // The default character in the middle, the others either side.
    let order = [
        "cat", "lamb", "songbird", "ninja", "mecha", "note", "robot", "child",
    ];
    let cast: Vec<Arc<dyn Character>> = order
        .iter()
        .map(|id| cast::by_id(&format!("builtin:{id}")).expect("a character"))
        .collect();
    let opaque = kind == "mp4";
    let stage = Stage::new(
        w,
        h,
        if opaque {
            Background::Solid(NIGHT)
        } else {
            Background::Transparent
        },
    );
    let draw = |n: u32, pm: &mut Pixmap| {
        let t = f64::from(n) / f64::from(fps);
        let poses: Vec<_> = cast
            .iter()
            .enumerate()
            .map(|(k, ch)| {
                let x = (k as f32 + 0.5) / cast.len() as f32;
                (
                    ch.as_ref(),
                    pose_at(&frame_at(t, x, size, y), ch.as_ref(), t),
                )
            })
            .collect();
        render_into(pm, &stage, &poses);
    };
    let seconds = BARS * 4.0 * 60.0 / BPM;
    let frames = (seconds * f64::from(fps)).round() as u32;
    if kind == "png" {
        std::fs::create_dir_all(&out).expect("the output folder");
        let mut pm = Pixmap::new(w, h).expect("a stage");
        for n in 0..frames {
            draw(n, &mut pm);
            pm.save_png(out.join(format!("frame_{n:04}.png")))
                .expect("a PNG");
        }
        println!("{frames} frames in {}", out.display());
        return;
    }
    let audio = song(48_000, seconds);
    let req = Request {
        container: Container::Mp4,
        width: w,
        height: h,
        fps,
        frames,
        quality: Quality::High,
        alpha: false,
        audio: &audio,
        workers: 0,
        replace: true,
        plan: None,
    };
    let done = export(&req, &draw, &out, &Control::new()).expect("the export");
    println!(
        "{}: {} frames, {:.2} MB, {}",
        done.path.display(),
        done.frames,
        done.bytes as f64 / 1e6,
        done.encoder
    );
}
