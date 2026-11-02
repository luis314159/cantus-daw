//! The test clip and the decoders that check what was written.
//!
//! The clip: 120 BPM; frame `n` shows its own number in twelve binary blocks along the
//! top (so a decoder's output says which frame it is — a dropped or doubled frame shows),
//! a white flash below the strip on the frame where each beat starts, and a ball moving;
//! the audio is silence with a sharp click starting exactly on each beat's sample.
//!
//! FFmpeg is a **test tool only** (ADR 0088 §5): never in the repository or a package.
//! It is found through `CANTUS_FFMPEG` (the path to `ffmpeg`; `ffprobe` beside it) or on
//! `PATH`; without it the decode-back tests say so and stop, unless
//! `CANTUS_REQUIRE_FFMPEG=1`, when they fail.

#![allow(dead_code)]

use cantus_video::audio::Pcm;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Rect, Transform};

pub const BITS: u32 = 12;
pub const BLOCK: u32 = 32;
pub const STRIP_H: u32 = BLOCK + 16;
pub const BEAT_SECONDS: f64 = 0.5;

#[derive(Copy, Clone, Debug)]
pub struct Clip {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub rate: u32,
    pub seconds: u32,
}

impl Clip {
    pub fn frames(&self) -> u32 {
        self.fps * self.seconds
    }

    pub fn frames_per_beat(&self) -> u32 {
        (self.fps as f64 * BEAT_SECONDS) as u32
    }

    pub fn beats(&self) -> u32 {
        self.frames() / self.frames_per_beat()
    }

    pub fn is_flash(&self, n: u32) -> bool {
        n.is_multiple_of(self.frames_per_beat())
    }

    /// Frame `n`, opaque (a dark stage) or `clear` (transparent around the drawing).
    pub fn draw(&self, n: u32, pm: &mut Pixmap, clear: bool) {
        let (w, h) = (pm.width() as f32, pm.height() as f32);
        if !clear {
            pm.fill(Color::from_rgba8(24, 26, 32, 255));
        }
        let mut paint = Paint::default();
        if self.is_flash(n) {
            paint.set_color_rgba8(255, 255, 255, 255);
            let r = Rect::from_xywh(0.0, STRIP_H as f32, w, h - STRIP_H as f32).unwrap();
            pm.fill_rect(r, &paint, Transform::identity(), None);
        }
        let t = n as f32 / self.fps as f32;
        let (cx, cy) = (
            w * (0.5 + 0.35 * (t * 1.3).sin()),
            h * (0.6 + 0.25 * (t * 0.9).cos()),
        );
        paint.anti_alias = true;
        paint.set_color_rgba8(240, 120, 40, 200);
        let ball = PathBuilder::from_circle(cx, cy, w.min(h) * 0.12).unwrap();
        pm.fill_path(
            &ball,
            &paint,
            tiny_skia::FillRule::Winding,
            Transform::identity(),
            None,
        );
        paint.anti_alias = false;
        paint.set_color_rgba8(0, 0, 0, 255);
        let strip = Rect::from_xywh(0.0, 0.0, (BITS * BLOCK + 16) as f32, STRIP_H as f32).unwrap();
        pm.fill_rect(strip, &paint, Transform::identity(), None);
        paint.set_color_rgba8(255, 255, 255, 255);
        for b in 0..BITS {
            if n >> (BITS - 1 - b) & 1 == 1 {
                let x = (8 + b * BLOCK + 4) as f32;
                let r = Rect::from_xywh(x, 12.0, (BLOCK - 8) as f32, (BLOCK - 8) as f32).unwrap();
                pm.fill_rect(r, &paint, Transform::identity(), None);
            }
        }
    }

    /// Silence with a click on every beat: 10 ms of a decaying 2 kHz square wave.
    pub fn audio(&self) -> Pcm {
        let len = (self.rate * self.seconds) as usize;
        let mut samples = vec![0.0f32; len * 2];
        let beat = (self.rate as f64 * BEAT_SECONDS) as usize;
        for start in (0..len).step_by(beat) {
            for i in 0..(self.rate as usize / 100).min(len - start) {
                let tt = i as f32 / self.rate as f32;
                let sq = if (tt * 2_000.0).fract() < 0.5 {
                    1.0
                } else {
                    -1.0
                };
                let x = 0.8 * (-tt * 300.0).exp() * sq;
                samples[2 * (start + i)] = x;
                samples[2 * (start + i) + 1] = x;
            }
        }
        Pcm {
            rate: self.rate,
            channels: 2,
            samples,
        }
    }
}

/// The frame number shown in a picture's strip (from its luma, any range).
pub fn read_counter(luma: &[u8], stride: usize) -> u32 {
    let mut n = 0;
    for b in 0..BITS {
        let x = (8 + b * BLOCK + BLOCK / 2) as usize;
        let y = (8 + BLOCK / 2) as usize;
        n = n << 1 | (luma[y * stride + x] > 128) as u32;
    }
    n
}

/// Whether a picture's luma is a flash: bright below the strip.
pub fn is_flash_luma(luma: &[u8], w: usize) -> bool {
    let below = &luma[(STRIP_H as usize + 8) * w..];
    let mean = below.iter().map(|&x| x as u64).sum::<u64>() as f64 / below.len() as f64;
    mean > 180.0
}

/// Click onsets (sample indices) in interleaved stereo: |left| over 0.3 after 50 ms quiet.
pub fn clicks(stereo: &[f32], rate: u32) -> Vec<usize> {
    let mut out = Vec::new();
    let mut quiet = usize::MAX;
    for (i, f) in stereo.chunks_exact(2).enumerate() {
        if f[0].abs() > 0.3 {
            if quiet > rate as usize / 20 {
                out.push(i);
            }
            quiet = 0;
        } else {
            quiet = quiet.saturating_add(1);
        }
    }
    out
}

/// `ffmpeg` and `ffprobe`, if this machine has them.
pub fn ffmpeg() -> Option<(PathBuf, PathBuf)> {
    let ff = std::env::var_os("CANTUS_FFMPEG")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("ffmpeg"));
    let probe = match ff.parent().filter(|p| !p.as_os_str().is_empty()) {
        Some(dir) => dir.join(if cfg!(windows) {
            "ffprobe.exe"
        } else {
            "ffprobe"
        }),
        None => PathBuf::from("ffprobe"),
    };
    let ok = |p: &Path| {
        Command::new(p)
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    };
    if ok(&ff) && ok(&probe) {
        return Some((ff, probe));
    }
    let need = std::env::var("CANTUS_REQUIRE_FFMPEG").is_ok_and(|v| v == "1");
    assert!(
        !need,
        "CANTUS_REQUIRE_FFMPEG=1 but no ffmpeg/ffprobe at {ff:?}"
    );
    eprintln!("no ffmpeg (set CANTUS_FFMPEG): the decode-back check is skipped here");
    None
}

/// `ffprobe`'s CSV answer for `entries` of `stream`.
pub fn probe(ffprobe: &Path, file: &Path, stream: &str, entries: &str) -> Vec<String> {
    let o = Command::new(ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            stream,
            "-show_entries",
            entries,
        ])
        .args(["-of", "csv=p=0"])
        .arg(file)
        .output()
        .expect("ffprobe runs");
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .map(|l| l.trim().trim_end_matches(',').to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Every decoded frame's presentation time, in seconds.
pub fn frame_times(ffprobe: &Path, file: &Path, stream: &str) -> Vec<f64> {
    probe(ffprobe, file, stream, "frame=best_effort_timestamp_time")
        .iter()
        .filter_map(|l| l.parse().ok())
        .collect()
}

/// Decode the video to raw frames of `pix_fmt` (`yuv420p`: the Y plane first; `rgba`),
/// calling `each` with every frame's bytes.
pub fn decode_video(
    ffmpeg: &Path,
    file: &Path,
    pix_fmt: &str,
    size: usize,
    mut each: impl FnMut(&[u8]),
) {
    let mut child = Command::new(ffmpeg)
        .args(["-v", "error", "-i"])
        .arg(file)
        .args([
            "-map",
            "0:v:0",
            "-fps_mode",
            "passthrough",
            "-f",
            "rawvideo",
            "-pix_fmt",
            pix_fmt,
            "-",
        ])
        .stdout(Stdio::piped())
        .spawn()
        .expect("ffmpeg runs");
    let mut out = child.stdout.take().unwrap();
    let mut buf = vec![0u8; size];
    while out.read_exact(&mut buf).is_ok() {
        each(&buf);
    }
    assert!(child.wait().unwrap().success(), "ffmpeg decoded {file:?}");
}

/// Decode the audio to interleaved stereo floats at `rate` (edit lists applied).
pub fn decode_audio(ffmpeg: &Path, file: &Path, rate: u32) -> Vec<f32> {
    let o = Command::new(ffmpeg)
        .args(["-v", "error", "-i"])
        .arg(file)
        .args([
            "-map",
            "0:a:0",
            "-f",
            "f32le",
            "-ac",
            "2",
            "-ar",
            &rate.to_string(),
            "-",
        ])
        .output()
        .expect("ffmpeg runs");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    o.stdout
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

/// A Media Foundation result, or `None` (said) on a system without it — Windows N, or a
/// Windows Server without its Media Foundation feature (CI's runners may be one) — unless
/// `CANTUS_REQUIRE_MF=1`, when its absence fails.
pub fn mf_or_skip<T>(r: Result<T, cantus_video::VideoError>) -> Option<T> {
    match r {
        Ok(v) => Some(v),
        Err(cantus_video::VideoError::NoEncoder(what)) => {
            let need = std::env::var("CANTUS_REQUIRE_MF").is_ok_and(|v| v == "1");
            assert!(
                !need,
                "CANTUS_REQUIRE_MF=1 but this system has no {what} encoder"
            );
            eprintln!("this system has no Media Foundation {what} encoder: skipped");
            None
        }
        Err(e) => panic!("{e}"),
    }
}

/// The source frame `n` as the file should hold it: straight RGBA, or RGB.
pub fn source(clip: Clip, n: u32, alpha: bool) -> Vec<u8> {
    let mut pm = Pixmap::new(clip.width, clip.height).unwrap();
    clip.draw(n, &mut pm, alpha);
    let mut v = pm.data().to_vec();
    if alpha {
        cantus_video::yuv::demultiply(&mut v);
        v
    } else {
        v.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect()
    }
}

/// Check one decoded file; `lossless`: Some(alpha) for PNG frames (compared bit for bit).
pub fn check(clip: Clip, file: &Path, lossless: Option<bool>) {
    let Some((ffmpeg, ffprobe)) = ffmpeg() else {
        return;
    };
    let (w, h) = (clip.width as usize, clip.height as usize);
    let half_frame_ms = 500.0 / clip.fps as f64;
    // Every frame at its exact time.
    let times = frame_times(&ffprobe, file, "v:0");
    assert_eq!(times.len() as u32, clip.frames(), "frames decoded");
    // WebM times are whole milliseconds (its usual timestamp scale): half a millisecond off
    // at most. MP4 and MOV are exact.
    let webm = file.extension().is_some_and(|e| e == "webm");
    let tolerance = if webm { 5e-4 + 1e-6 } else { 1e-4 };
    for (n, t) in times.iter().enumerate() {
        assert!(
            (t - n as f64 / clip.fps as f64).abs() < tolerance,
            "frame {n} at {t}"
        );
    }
    // Every frame's number, the flashes, and (PNG) every pixel.
    let (pix, size) = match lossless {
        Some(true) => ("rgba", w * h * 4),
        Some(false) => ("rgb24", w * h * 3),
        None => ("yuv420p", w * h * 3 / 2),
    };
    let (mut n, mut wrong, mut flashes, mut exact) = (0u32, 0, Vec::new(), true);
    let mut luma = Vec::new();
    decode_video(&ffmpeg, file, pix, size, |buf| {
        let y: &[u8] = match lossless {
            None => &buf[..w * h],
            Some(a) => {
                let k = if a { 4 } else { 3 };
                luma.clear();
                luma.extend(buf.chunks_exact(k).map(|p| {
                    if a && p[3] == 0 {
                        0
                    } else {
                        ((p[0] as u32 + p[1] as u32 + p[2] as u32) / 3) as u8
                    }
                }));
                if n % 7 == 0 && buf != source(clip, n, a).as_slice() {
                    exact = false;
                }
                &luma
            }
        };
        if read_counter(y, w) != n {
            wrong += 1;
        }
        if is_flash_luma(y, w) {
            flashes.push(n);
        }
        n += 1;
    });
    assert_eq!(n, clip.frames());
    assert_eq!(wrong, 0, "frames out of place");
    let expected: Vec<u32> = (0..clip.frames()).filter(|&n| clip.is_flash(n)).collect();
    assert_eq!(flashes, expected, "the flashes");
    if lossless.is_some() {
        assert!(exact, "PNG frames decode bit-exact");
    }
    // The clicks against the flashes.
    let audio = decode_audio(&ffmpeg, file, clip.rate);
    let len = audio.len() / 2;
    let want = (clip.rate * clip.seconds) as usize;
    // A WebM's Vorbis ends on a whole block (Matroska has no end trim for it): up to one
    // long block more, silent.
    let extra = if webm { 2_048 } else { 1 };
    assert!(
        len + 1 >= want && len <= want + extra,
        "audio {len} samples, the video {want}"
    );
    let clicks = clicks(&audio, clip.rate);
    assert_eq!(clicks.len() as u32, clip.beats(), "the clicks");
    // Every click whole, the first one too (no encoder start-up eating it).
    let peaks: Vec<f32> = clicks
        .iter()
        .map(|&c| (c..c + 480).map(|i| audio[2 * i].abs()).fold(0.0, f32::max))
        .collect();
    let least = peaks.iter().copied().fold(f32::MAX, f32::min);
    let most = peaks.iter().copied().fold(0.0, f32::max);
    assert!(least > most * 0.9, "click peaks {peaks:?}");
    let mut worst: f64 = 0.0;
    for (k, &f) in flashes.iter().enumerate() {
        let v = times[f as usize];
        let a = clicks[k] as f64 / clip.rate as f64;
        worst = worst.max(((a - v) * 1000.0).abs());
    }
    println!(
        "SYNC {} at {} fps: worst |click - flash| {worst:.3} ms over {} beats (half a frame \
         {half_frame_ms:.2} ms); first click at sample {}",
        file.file_name().unwrap().to_string_lossy(),
        clip.fps,
        flashes.len(),
        clicks[0]
    );
    assert!(
        clicks[0] <= clip.rate as usize / 1_000,
        "the click at time 0 is kept"
    );
    assert!(worst < half_frame_ms, "in sync within half a frame");
}
