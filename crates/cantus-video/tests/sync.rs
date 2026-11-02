//! Decode every export back and check it (ADR 0088 §6): every frame there once, in order, at
//! its time; each beat's flash against its click within half a frame (measured: 0 ms); the
//! click at time 0 kept; the audio exactly the video's length; MOV and PNG bit-exact.
//! At 30 and 60 frames a second.

mod common;

use cantus_video::{export, Container, Control, Quality, Request};
use common::Clip;
use std::path::{Path, PathBuf};
use tiny_skia::Pixmap;

fn clip(fps: u32) -> Clip {
    Clip {
        width: 480,
        height: 854,
        fps,
        rate: 48_000,
        seconds: 12,
    }
}

fn write(clip: Clip, container: Container, alpha: bool, dir: &Path, name: &str) -> Option<PathBuf> {
    let audio = clip.audio();
    let req = Request {
        container,
        width: clip.width,
        height: clip.height,
        fps: clip.fps,
        frames: clip.frames(),
        quality: Quality::High,
        alpha,
        audio: &audio,
        workers: 0,
        replace: false,
        plan: None,
    };
    // CANTUS_VIDEO_KEEP=<folder>: the files are written there and kept, to look at.
    let keep = std::env::var_os("CANTUS_VIDEO_KEEP").map(PathBuf::from);
    let path = keep.as_deref().unwrap_or(dir).join(name);
    let _ = std::fs::remove_file(&path);
    let ctl = Control::new();
    let draw = |n: u32, pm: &mut Pixmap| clip.draw(n, pm, alpha);
    let done = common::mf_or_skip(export(&req, &draw, &path, &ctl))?;
    assert_eq!(ctl.frames(), (clip.frames(), clip.frames()));
    println!(
        "{name}: {} frames, {:.2} MB, {:.2} s, {}",
        done.frames,
        done.bytes as f64 / 1e6,
        done.wall,
        done.encoder
    );
    Some(path)
}

#[cfg(any(windows, target_os = "macos"))]
fn mp4(fps: u32) {
    let dir = tempfile::tempdir().unwrap();
    let c = clip(fps);
    let Some(file) = write(
        c,
        Container::Mp4,
        false,
        dir.path(),
        &format!("clip{fps}.mp4"),
    ) else {
        return;
    };
    if let Some((_, ffprobe)) = common::ffmpeg() {
        // No B-frames; High profile; the index in front.
        let kinds = common::probe(&ffprobe, &file, "v:0", "frame=pict_type");
        assert!(kinds.iter().all(|k| k == "I" || k == "P"), "{kinds:?}");
        let profile = common::probe(
            &ffprobe,
            &file,
            "v:0",
            "stream=profile,color_space,color_range",
        );
        assert_eq!(profile, vec!["High,tv,bt709"]);
    }
    let bytes = std::fs::read(&file).unwrap();
    let order: Vec<[u8; 4]> = cantus_video::bmff::parse(&bytes)
        .iter()
        .map(|b| b.kind)
        .collect();
    let (moov, mdat) = (
        order.iter().position(|k| k == b"moov").unwrap(),
        order.iter().position(|k| k == b"mdat").unwrap(),
    );
    assert!(moov < mdat, "fast start: {order:?}");
    common::check(c, &file, None);
}

#[cfg(any(windows, target_os = "macos"))]
#[test]
fn mp4_at_30_fps_decodes_back_in_sync() {
    mp4(30);
}

#[cfg(any(windows, target_os = "macos"))]
#[test]
fn mp4_at_60_fps_decodes_back_in_sync() {
    mp4(60);
}

fn mov(fps: u32, alpha: bool) {
    let dir = tempfile::tempdir().unwrap();
    let c = clip(fps);
    let file = write(
        c,
        Container::MovPng,
        alpha,
        dir.path(),
        &format!("clip{fps}.mov"),
    )
    .expect("a MOV needs no system encoder");
    common::check(c, &file, Some(alpha));
}

#[test]
fn mov_with_alpha_at_30_fps_decodes_back_bit_exact_and_in_sync() {
    mov(30, true);
}

#[test]
fn mov_opaque_at_60_fps_decodes_back_bit_exact_and_in_sync() {
    mov(60, false);
}

fn webm(fps: u32) {
    let dir = tempfile::tempdir().unwrap();
    // Short: rav1e is slow (about 10 frames a second at this size here).
    let c = Clip {
        seconds: 4,
        ..clip(fps)
    };
    let file = write(
        c,
        Container::WebM,
        false,
        dir.path(),
        &format!("clip{fps}.webm"),
    )
    .expect("WebM needs no system encoder");
    if let Some((_, ffprobe)) = common::ffmpeg() {
        let streams = common::probe(&ffprobe, &file, "v:0", "stream=codec_name,color_space");
        assert_eq!(streams, vec!["av1,bt709"]);
    }
    common::check(c, &file, None);
}

#[test]
fn webm_at_30_fps_decodes_back_in_sync() {
    webm(30);
}

#[test]
fn webm_at_60_fps_decodes_back_in_sync() {
    webm(60);
}
