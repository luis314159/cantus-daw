//! Measure a video export: the default character dancing to a 120 BPM song, drawn through
//! the one renderer (`cantus_dance::render`) at the export's size, encoded and written.
//!
//! `cargo run -p cantus-video --release --example measure -- <out-dir> <mp4|mov|seq|render>
//!  [width] [height] [fps] [seconds] [good|high|best] [workers]`
//!
//! Prints the wall time, frames a second, the file's size and bitrate, and the process's
//! peak memory (Windows). One format a run, so the peak is that format's.

use cantus_dance::render::{render_into, Background, Stage};
use cantus_dance::{cast, pose_at, DanceFrame};
use cantus_video::audio::Pcm;
use cantus_video::{export, Container, Control, Quality, Request};
use std::path::PathBuf;
use std::time::Instant;
use tiny_skia::Pixmap;

const BPM: f64 = 120.0;

/// The song: a kick on every beat, a hat between, a soft chord — enough for an ear to
/// judge the sync and for the encoder to have music to code.
fn song(rate: u32, seconds: f64) -> Pcm {
    let len = (rate as f64 * seconds) as usize;
    let mut s = vec![0.0f32; len * 2];
    let beat = rate as f64 * 60.0 / BPM;
    for i in 0..len {
        let t = i as f64 / rate as f64;
        let in_beat = (i as f64 % beat) / rate as f64;
        let in_half = (i as f64 % (beat / 2.0)) / rate as f64;
        let kick = (-in_beat * 30.0).exp()
            * (std::f64::consts::TAU * (50.0 + 90.0 * (-in_beat * 40.0).exp()) * in_beat).sin();
        let hat = if (i as f64 / (beat / 2.0)).floor() as i64 % 2 == 1 {
            (-in_half * 80.0).exp() * (((i as u64).wrapping_mul(2_654_435_761) >> 7) % 2_000) as f64
                / 1_000.0
                - (-in_half * 80.0).exp()
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

fn frame_at(t: f64) -> DanceFrame {
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
    if let Some(i) = cantus_core::inst::dancer::KEYS
        .iter()
        .position(|k| *k == "size")
    {
        f.values[i] = 1.8;
    }
    f
}

#[cfg(windows)]
fn peak_mb() -> f64 {
    use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows::Win32::System::Threading::GetCurrentProcess;
    let mut c = PROCESS_MEMORY_COUNTERS::default();
    // SAFETY: the counters struct is the size it says.
    unsafe {
        let _ = GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut c,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        );
    }
    c.PeakWorkingSetSize as f64 / 1_048_576.0
}

#[cfg(not(windows))]
fn peak_mb() -> f64 {
    0.0
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let arg = |i: usize, d: &str| a.get(i).cloned().unwrap_or_else(|| d.to_string());
    let num = |i: usize, d: u32| a.get(i).and_then(|s| s.parse().ok()).unwrap_or(d);
    let out = PathBuf::from(arg(1, "measure-out"));
    let kind = arg(2, "mp4");
    let (w, h, fps, seconds) = (num(3, 1080), num(4, 1920), num(5, 30), num(6, 30));
    let quality = match arg(7, "high").as_str() {
        "good" => Quality::Good,
        "best" => Quality::Best,
        _ => Quality::High,
    };
    std::fs::create_dir_all(&out).expect("the output folder");
    let ch = cast::default_character();
    let (container, alpha) = match kind.as_str() {
        "mov" => (Container::MovPng, true),
        "movopaque" => (Container::MovPng, false),
        "webm" => (Container::WebM, false),
        "seq" => (Container::PngSequence, true),
        _ => (Container::Mp4, false),
    };
    let background = if alpha {
        Background::Transparent
    } else {
        Background::Solid([29, 31, 36, 255])
    };
    let stage = Stage::new(w, h, background);
    let draw = |n: u32, pm: &mut Pixmap| {
        let t = n as f64 / fps as f64;
        let pose = pose_at(&frame_at(t), ch.as_ref(), t);
        render_into(pm, &stage, &[(ch.as_ref(), pose)]);
    };
    let frames = fps * seconds;
    if kind == "render" {
        let mut pm = Pixmap::new(w, h).unwrap();
        let t0 = Instant::now();
        for n in 0..frames {
            draw(n, &mut pm);
        }
        let s = t0.elapsed().as_secs_f64();
        println!(
            "render only, one thread: {frames} frames in {s:.2} s = {:.2} ms a frame",
            s * 1000.0 / frames as f64
        );
        return;
    }
    let audio = song(48_000, seconds as f64);
    let req = Request {
        container,
        width: w,
        height: h,
        fps,
        frames,
        quality,
        alpha,
        audio: &audio,
        workers: num(8, 0) as usize,
        replace: true,
        plan: None,
    };
    let path = out.join(match container {
        Container::Mp4 => format!("dance-{w}x{h}-{fps}.mp4"),
        Container::WebM => format!("dance-{w}x{h}-{fps}.webm"),
        Container::MovPng => format!("dance-{w}x{h}-{fps}.mov"),
        Container::PngSequence => format!("dance-{w}x{h}-{fps}"),
    });
    if container == Container::PngSequence && path.exists() {
        std::fs::remove_dir_all(&path).expect("the old sequence removed");
    }
    let done = export(&req, &draw, &path, &Control::new()).expect("the export");
    println!(
        "RESULT {kind} {w}x{h} {fps} fps {seconds} s: wall {:.2} s ({:.0} fps, {:.1}x real time), \
         {:.2} MB ({:.2} Mbit/s), peak memory {:.0} MB, {}",
        done.wall,
        frames as f64 / done.wall,
        seconds as f64 / done.wall,
        done.bytes as f64 / 1e6,
        done.bytes as f64 * 8.0 / 1e6 / seconds as f64,
        peak_mb(),
        done.encoder
    );
}
