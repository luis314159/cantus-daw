//! The MP4 without the system's own encoders (Linux; Windows N), decoded back in sync:
//! Cisco's OpenH264 (fetched through the content manager from a local copy of Cisco's
//! `.bz2`: `CANTUS_OPENH264_DIR`), AAC from the system's FFmpeg or GStreamer, and MP3 when
//! there is no AAC. Each check says so and stops where its tool is missing.

mod common;

use cantus_content::fetch::{file_url, Net};
use cantus_video::audio_alt::{self, Tool};
use cantus_video::openh264::{self, Install};
use cantus_video::select::{AudioPath, Plan, VideoPath, Warning};
use cantus_video::{export, Container, Control, Quality, Request, VideoError};
use common::Clip;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use tiny_skia::Pixmap;

fn clip() -> Clip {
    Clip {
        width: 480,
        height: 854,
        fps: 30,
        rate: 48_000,
        seconds: 8,
    }
}

/// Cisco's library for this system, installed from `CANTUS_OPENH264_DIR` into `into`.
fn openh264(into: &Path) -> Option<PathBuf> {
    let Some(dir) = std::env::var_os("CANTUS_OPENH264_DIR").map(PathBuf::from) else {
        eprintln!("no CANTUS_OPENH264_DIR (Cisco's .bz2 files): the OpenH264 check is skipped");
        return None;
    };
    let inst = Install::new(into.join("OpenH264"))?;
    let url = file_url(&dir.join(inst.blob.file));
    let mut seen = 0;
    let lib = inst
        .install(&Net::new(), &url, &AtomicBool::new(false), &mut |n, of| {
            assert!(n <= of);
            seen = n;
        })
        .expect("Cisco's binary installs");
    assert_eq!(seen, inst.blob.size);
    assert_eq!(inst.library(), Some(lib.clone()));
    assert_eq!(
        inst.credit(),
        Some(openh264::CREDIT),
        "the About box names Cisco"
    );
    assert!(
        !into.join("OpenH264").join(inst.blob.file).exists(),
        "the .bz2 removed"
    );
    Some(lib)
}

fn write(plan: Plan, dir: &Path, name: &str) -> (PathBuf, Vec<Warning>, String) {
    let c = clip();
    let audio = c.audio();
    let req = Request {
        container: Container::Mp4,
        width: c.width,
        height: c.height,
        fps: c.fps,
        frames: c.frames(),
        quality: Quality::High,
        alpha: false,
        audio: &audio,
        workers: 0,
        replace: false,
        plan: Some(plan),
    };
    let path = dir.join(name);
    let draw = |n: u32, pm: &mut Pixmap| c.draw(n, pm, false);
    let done = export(&req, &draw, &path, &Control::new()).expect("the export");
    println!(
        "{name}: {:.2} MB, {:.2} s, {}",
        done.bytes as f64 / 1e6,
        done.wall,
        done.encoder
    );
    (path, done.warnings, done.encoder)
}

/// The system's FFmpeg as an AAC tool (the test's own, from `CANTUS_FFMPEG`).
fn ffmpeg_tool() -> Option<Tool> {
    common::ffmpeg().map(|(f, _)| Tool::Ffmpeg(f))
}

#[test]
fn openh264_with_mp3_decodes_back_in_sync_and_warns() {
    let dir = tempfile::tempdir().unwrap();
    let Some(lib) = openh264(dir.path()) else {
        return;
    };
    let plan = Plan {
        video: Some(VideoPath::OpenH264(lib)),
        audio: AudioPath::Mp3,
    };
    let (file, warnings, encoder) = write(plan, dir.path(), "oh-mp3.mp4");
    assert_eq!(warnings, vec![Warning::Mp3Audio]);
    assert!(
        encoder.contains("OpenH264") && encoder.contains("MP3"),
        "{encoder}"
    );
    common::check(clip(), &file, None);
}

#[test]
fn openh264_with_the_systems_aac_decodes_back_in_sync() {
    let dir = tempfile::tempdir().unwrap();
    let (Some(lib), Some(tool)) = (openh264(dir.path()), ffmpeg_tool()) else {
        return;
    };
    let plan = Plan {
        video: Some(VideoPath::OpenH264(lib)),
        audio: AudioPath::System(tool),
    };
    let (file, warnings, encoder) = write(plan, dir.path(), "oh-aac.mp4");
    assert!(warnings.is_empty());
    assert!(encoder.contains("FFmpeg"), "{encoder}");
    common::check(clip(), &file, None);
}

#[test]
fn gstreamers_aac_decodes_back_in_sync() {
    let Some(tool) = audio_alt::find_gstreamer_aac() else {
        eprintln!("no GStreamer with avenc_aac here: its check is skipped");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let Some(lib) = openh264(dir.path()) else {
        return;
    };
    let plan = Plan {
        video: Some(VideoPath::OpenH264(lib)),
        audio: AudioPath::System(tool),
    };
    let (file, _, encoder) = write(plan, dir.path(), "oh-gst.mp4");
    assert!(encoder.contains("GStreamer"), "{encoder}");
    common::check(clip(), &file, None);
}

#[test]
fn a_plan_without_h264_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let c = clip();
    let audio = c.audio();
    let req = Request {
        container: Container::Mp4,
        width: c.width,
        height: c.height,
        fps: c.fps,
        frames: 3,
        quality: Quality::High,
        alpha: false,
        audio: &audio,
        workers: 0,
        replace: false,
        plan: Some(Plan {
            video: None,
            audio: AudioPath::Mp3,
        }),
    };
    let draw = |n: u32, pm: &mut Pixmap| c.draw(n, pm, false);
    let e = export(&req, &draw, &dir.path().join("x.mp4"), &Control::new()).unwrap_err();
    assert!(matches!(e, VideoError::NoEncoder("H.264")), "{e}");
    assert!(
        std::fs::read_dir(dir.path()).unwrap().next().is_none(),
        "nothing left"
    );
}

#[test]
fn a_download_that_is_not_ciscos_is_refused_and_removed() {
    let dir = tempfile::tempdir().unwrap();
    let Some(inst) = Install::new(dir.path().join("OpenH264")) else {
        return; // macOS: VideoToolbox, no OpenH264
    };
    // A proxy's page in place of the binary.
    let html = dir.path().join("page.bz2");
    std::fs::write(&html, b"<html><body>quarantined</body></html>").unwrap();
    let r = inst.install(
        &Net::new(),
        &file_url(&html),
        &AtomicBool::new(false),
        &mut |_, _| {},
    );
    assert!(r.is_err());
    // A file of the right size and the wrong bytes.
    let wrong = dir.path().join("wrong.bz2");
    std::fs::write(&wrong, vec![7u8; inst.blob.size as usize]).unwrap();
    let e = inst
        .install(
            &Net::new(),
            &file_url(&wrong),
            &AtomicBool::new(false),
            &mut |_, _| {},
        )
        .unwrap_err()
        .to_string();
    assert!(e.contains("download"), "{e}");
    assert_eq!(inst.library(), None);
    let left: Vec<_> = std::fs::read_dir(dir.path().join("OpenH264"))
        .unwrap()
        .collect();
    assert!(left.is_empty(), "{left:?}");
    assert!(openh264::CREDIT.contains("Cisco Systems"));
}
