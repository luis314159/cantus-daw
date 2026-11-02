//! Media Foundation (Windows): the encoders driven directly, and our MP4 played back by
//! Windows' own decoders (the stack its player and the browsers use) — every frame at its
//! time and every click on its beat, with no FFmpeg anywhere.

#![cfg(windows)]

mod common;

use cantus_video::audio::{aac_lc_config, AudioCodec, AudioEncoder};
use cantus_video::h264::{nal_units, H264Encoder};
use cantus_video::mf::{MfAac, MfH264, Session};
use cantus_video::yuv::{rgba_to_nv12, Nv12};
use cantus_video::{export, Container, Control, Quality, Request};
use common::Clip;
use tiny_skia::Pixmap;
use windows::Win32::Media::MediaFoundation::*;

#[test]
fn h264_gives_one_access_unit_a_frame_with_a_key_frame_every_two_seconds() {
    let Some(_s) = common::mf_or_skip(Session::start()) else {
        return;
    };
    let clip = Clip {
        width: 320,
        height: 240,
        fps: 30,
        rate: 48_000,
        seconds: 3,
    };
    let Some(mut enc) = common::mf_or_skip(MfH264::new(320, 240, 30, 2_000_000)) else {
        return;
    };
    let (mut pm, mut f, mut aus) = (Pixmap::new(320, 240).unwrap(), Nv12::default(), Vec::new());
    for n in 0..clip.frames() {
        clip.draw(n, &mut pm, false);
        rgba_to_nv12(pm.data(), 320, 240, &mut f);
        enc.encode(&f, &mut aus).unwrap();
    }
    enc.finish(&mut aus).unwrap();
    assert_eq!(aus.len() as u32, clip.frames());
    let types = |au: &Vec<u8>| -> Vec<u8> { nal_units(au).iter().map(|n| n[0] & 0x1f).collect() };
    let first = types(&aus[0]);
    assert!(
        first.contains(&7) && first.contains(&8) && first.contains(&5),
        "{first:?}"
    );
    let idr: Vec<usize> = (0..aus.len())
        .filter(|&i| types(&aus[i]).contains(&5))
        .collect();
    assert_eq!(idr, vec![0, 60]);
}

#[test]
fn aac_states_the_lc_config_and_pads_its_ends() {
    let Some(_s) = common::mf_or_skip(Session::start()) else {
        return;
    };
    for rate in [44_100, 48_000] {
        let Some(mut enc) = common::mf_or_skip(MfAac::new(rate, 2)) else {
            return;
        };
        assert_eq!(
            enc.codec(),
            AudioCodec::Aac {
                config: aac_lc_config(rate, 2).unwrap().to_vec()
            }
        );
        let mut packets = Vec::new();
        let second = vec![0i16; rate as usize * 2];
        enc.encode(&second, &mut packets).unwrap();
        enc.finish(&mut packets).unwrap();
        let fed = cantus_video::mf::aac::LEAD + rate + cantus_video::mf::aac::TAIL;
        let covered = packets.len() as u32 * 1_024;
        assert!(
            covered >= fed && covered < fed + 2_048,
            "{rate}: {covered} for {fed}"
        );
    }
}

/// Decode `path` with Media Foundation's source reader: the video frames' times (s), and
/// the audio as interleaved stereo floats.
fn play(path: &std::path::Path) -> (Vec<f64>, Vec<f32>) {
    // SAFETY: the reader and its samples are used on this thread while the session lives.
    unsafe {
        let api = cantus_video::mf::sys::api().unwrap();
        let r = api.source_reader(path, None).unwrap();
        let video = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
        let audio = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;
        let t = api.media_type().unwrap();
        t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).unwrap();
        t.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_Float).unwrap();
        r.SetCurrentMediaType(audio, None, &t).unwrap();
        let mut times = Vec::new();
        let mut floats = Vec::new();
        for (stream, is_video) in [(video, true), (audio, false)] {
            loop {
                let (mut flags, mut ts, mut s) = (0u32, 0i64, None);
                r.ReadSample(
                    stream,
                    0,
                    None,
                    Some(&mut flags),
                    Some(&mut ts),
                    Some(&mut s),
                )
                .unwrap();
                if let Some(s) = s {
                    if is_video {
                        times.push(ts as f64 / 1e7);
                    } else {
                        let b = s.ConvertToContiguousBuffer().unwrap();
                        let (mut p, mut len) = (std::ptr::null_mut(), 0u32);
                        b.Lock(&mut p, None, Some(&mut len)).unwrap();
                        let bytes = std::slice::from_raw_parts(p, len as usize);
                        floats.extend(
                            bytes
                                .chunks_exact(4)
                                .map(|c| f32::from_le_bytes(c.try_into().unwrap())),
                        );
                        b.Unlock().unwrap();
                    }
                }
                if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                    break;
                }
            }
        }
        (times, floats)
    }
}

/// Media Foundation's offset of every click from its beat in `path` (ms; positive: late).
fn clicks_late(path: &std::path::Path, clip: Clip) -> Vec<f64> {
    let (times, floats) = play(path);
    assert_eq!(times.len() as u32, clip.frames());
    for (n, t) in times.iter().enumerate() {
        assert!(
            (t - n as f64 / clip.fps as f64).abs() < 0.001,
            "frame {n} at {t}"
        );
    }
    let clicks = common::clicks(&floats, clip.rate);
    clicks
        .iter()
        .enumerate()
        .map(|(k, &c)| (c as f64 / clip.rate as f64 - k as f64 * common::BEAT_SECONDS) * 1000.0)
        .collect()
}

/// What Windows' own player does with the MP4: every frame at its time; the audio **one
/// AAC packet late** — Media Foundation's MPEG-4 source ignores the audio edit list that
/// FFmpeg, the platforms and browsers honour — exactly as late as with an MP4 FFmpeg itself
/// makes (its AAC primes 1 024 samples too; compared when FFmpeg is here). Never earlier,
/// and never more than one packet.
#[test]
fn windows_plays_our_mp4_with_every_frame_in_place_and_the_audio_one_packet_late() {
    let clip = Clip {
        width: 480,
        height: 854,
        fps: 30,
        rate: 48_000,
        seconds: 6,
    };
    let audio = clip.audio();
    let dir = tempfile::tempdir().unwrap();
    let draw = |n: u32, pm: &mut Pixmap| clip.draw(n, pm, false);
    let mut req = Request {
        container: Container::Mp4,
        width: clip.width,
        height: clip.height,
        fps: clip.fps,
        frames: clip.frames(),
        quality: Quality::High,
        alpha: false,
        audio: &audio,
        workers: 0,
        replace: false,
        plan: None,
    };
    let ours = dir.path().join("ours.mp4");
    if common::mf_or_skip(export(&req, &draw, &ours, &Control::new())).is_none() {
        return;
    }
    let _s = Session::start().unwrap();
    let late = clicks_late(&ours, clip);
    assert_eq!(
        late.len() as u32,
        clip.beats(),
        "every click, the first one too"
    );
    let packet = 1_024.0 / clip.rate as f64 * 1000.0;
    for l in &late {
        assert!((l - packet).abs() < 0.05, "{late:?}");
    }
    println!(
        "Media Foundation playback: every click {:.3} ms late",
        late[0]
    );
    // FFmpeg's own MP4 of the same clip, through the same player.
    let Some((ffmpeg, _)) = common::ffmpeg() else {
        return;
    };
    req.container = Container::MovPng;
    let mov = dir.path().join("source.mov");
    export(&req, &draw, &mov, &Control::new()).unwrap();
    let theirs = dir.path().join("ffmpeg.mp4");
    let ok = std::process::Command::new(ffmpeg)
        .args(["-v", "error", "-i"])
        .arg(&mov)
        .args([
            "-c:v", "libx264", "-bf", "0", "-pix_fmt", "yuv420p", "-c:a", "aac",
        ])
        .args(["-b:a", "192k", "-movflags", "+faststart"])
        .arg(&theirs)
        .status()
        .unwrap();
    assert!(ok.success());
    let reference = clicks_late(&theirs, clip);
    println!("FFmpeg's own MP4 in Media Foundation: clicks {reference:.3?} ms late");
    assert_eq!(reference.len(), late.len());
    for l in &reference {
        assert!((l - packet).abs() < 0.05, "{reference:?}");
    }
}

/// The DLLs a PE file imports by name at load time (its import table).
fn imported_dlls(exe: &[u8]) -> Vec<String> {
    let u16_at = |o: usize| u16::from_le_bytes([exe[o], exe[o + 1]]) as usize;
    let u32_at = |o: usize| u32::from_le_bytes(exe[o..o + 4].try_into().unwrap()) as usize;
    let pe = u32_at(0x3c);
    let opt = pe + 24;
    let dirs = opt + if u16_at(opt) == 0x20b { 112 } else { 96 };
    let sections = opt + u16_at(pe + 20);
    let off = |rva: usize| {
        (0..u16_at(pe + 6))
            .map(|i| sections + 40 * i)
            .find_map(|s| {
                let (size, va, raw) = (
                    u32_at(s + 8).max(u32_at(s + 16)),
                    u32_at(s + 12),
                    u32_at(s + 20),
                );
                (va <= rva && rva < va + size).then_some(rva - va + raw)
            })
            .expect("an address inside a section")
    };
    let mut out = Vec::new();
    let mut d = off(u32_at(dirs + 8));
    while u32_at(d + 12) != 0 {
        let name = off(u32_at(d + 12));
        let end = name + exe[name..].iter().position(|&b| b == 0).unwrap();
        out.push(String::from_utf8_lossy(&exe[name..end]).to_lowercase());
        d += 20;
    }
    out
}

/// Cantus starts on Windows N and on servers without Media Foundation: nothing of it is
/// imported (a missing imported DLL stops a program before `main`); it is looked up when a
/// video is exported. This test binary uses every Media Foundation path there is.
#[test]
fn media_foundation_is_never_imported() {
    let exe = std::fs::read(std::env::current_exe().unwrap()).unwrap();
    let dlls = imported_dlls(&exe);
    assert!(dlls.iter().any(|d| d == "kernel32.dll"), "{dlls:?}");
    for mf in ["mfplat.dll", "mfreadwrite.dll", "mf.dll", "mfuuid.dll"] {
        assert!(!dlls.iter().any(|d| d == mf), "{mf} imported: {dlls:?}");
    }
}

/// Asking whether the encoders exist, on a thread with no session of its own, leaves the
/// thread as it found it (each transform released while COM is up).
#[test]
fn asking_for_the_encoders_alone_is_safe() {
    std::thread::spawn(|| {
        let (v, a) = cantus_video::mf::available();
        assert_eq!(v, a, "both or neither on one system");
    })
    .join()
    .unwrap();
}
