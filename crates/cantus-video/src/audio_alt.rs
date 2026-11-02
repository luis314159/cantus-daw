//! The MP4's audio where the system has no AAC encoder of its own (Linux; Windows N):
//!
//! - [`PipeAac`] — AAC-LC from an **FFmpeg or GStreamer already on the system** (the
//!   person's own, never shipped or fetched: ADR 0088 §5), driven through pipes: 16-bit PCM
//!   in, ADTS out, the ADTS headers taken off. Both use libavcodec's AAC encoder (FFmpeg's
//!   `aac`, GStreamer's `avenc_aac` from gst-libav), which primes with 1 024 samples.
//! - [`Mp3`] — LAME (already in Cantus for MP3 export) in the MP4, when there is no AAC at
//!   all: every player tested plays it; Instagram may refuse it, which the dialog says.

use crate::audio::{AudioCodec, AudioEncoder};
use crate::VideoError;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};

/// Bits a second of every audio track the export writes.
pub const BITRATE: u32 = 192_000;

/// libavcodec's AAC encoder's priming, in samples (measured by `tests/alt_audio.rs`).
pub const AAC_DELAY: u32 = 1_024;

/// Which program encodes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tool {
    Ffmpeg(PathBuf),
    GStreamer(PathBuf),
}

/// `name` on `PATH`, as an executable.
pub fn on_path(name: &str) -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|d| d.join(&exe))
        .find(|p| p.is_file())
}

/// A command that opens no console window on Windows (`CREATE_NO_WINDOW`): the system's
/// FFmpeg or GStreamer are console programs, and each would flash one (review 1 #5).
pub fn quiet(c: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    c
}

fn runs(p: &Path, args: &[&str], needle: &str) -> bool {
    quiet(&mut Command::new(p))
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .is_ok_and(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).contains(needle))
}

/// The system's AAC encoder, if it has one: FFmpeg with its `aac` encoder, else GStreamer
/// with `avenc_aac`.
pub fn find_aac() -> Option<Tool> {
    find_ffmpeg_aac().or_else(find_gstreamer_aac)
}

/// FFmpeg on `PATH` with its `aac` encoder.
pub fn find_ffmpeg_aac() -> Option<Tool> {
    let f = on_path("ffmpeg")?;
    runs(&f, &["-hide_banner", "-encoders"], " aac ").then_some(Tool::Ffmpeg(f))
}

/// GStreamer on `PATH` with gst-libav's `avenc_aac`.
pub fn find_gstreamer_aac() -> Option<Tool> {
    let launch = on_path("gst-launch-1.0")?;
    let inspect = on_path("gst-inspect-1.0")?;
    runs(&inspect, &["avenc_aac"], "avenc_aac").then_some(Tool::GStreamer(launch))
}

/// AAC-LC through an external program's pipes.
pub struct PipeAac {
    tool: Tool,
    child: Child,
    stdin: Option<ChildStdin>,
    out: Receiver<Vec<u8>>,
    err: Option<std::thread::JoinHandle<String>>,
    pending: Vec<u8>,
    rate: u32,
    channels: u16,
    config: Option<Vec<u8>>,
}

impl PipeAac {
    pub fn new(tool: Tool, rate: u32, channels: u16) -> Result<PipeAac, VideoError> {
        let mut cmd = match &tool {
            Tool::Ffmpeg(p) => {
                let mut c = Command::new(p);
                quiet(&mut c);
                c.args(["-hide_banner", "-loglevel", "error", "-f", "s16le"])
                    .args(["-ar", &rate.to_string(), "-ac", &channels.to_string()])
                    .args(["-i", "pipe:0", "-c:a", "aac", "-b:a", &BITRATE.to_string()])
                    .args(["-f", "adts", "pipe:1"]);
                c
            }
            Tool::GStreamer(p) => {
                let mut c = Command::new(p);
                quiet(&mut c);
                c.args([
                    "-q",
                    "fdsrc",
                    "fd=0",
                    "!",
                    "rawaudioparse",
                    "use-sink-caps=false",
                ])
                .args(["format=pcm", "pcm-format=s16le"])
                .arg(format!("sample-rate={rate}"))
                .arg(format!("num-channels={channels}"))
                .args(["!", "audioconvert", "!", "avenc_aac"])
                .arg(format!("bitrate={BITRATE}"))
                .args([
                    "!",
                    "aacparse",
                    "!",
                    "audio/mpeg,mpegversion=4,stream-format=adts",
                ])
                .args(["!", "fdsink", "fd=1"]);
                c
            }
        };
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| VideoError::encoder("AAC", format!("{tool:?}: {e}")))?;
        let mut stdout = child.stdout.take().expect("piped");
        let mut stderr = child.stderr.take().expect("piped");
        let (tx, out) = channel();
        std::thread::spawn(move || {
            let mut buf = vec![0u8; 1 << 16];
            while let Ok(n) = stdout.read(&mut buf) {
                if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
        });
        let err = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = stderr.read_to_string(&mut s);
            s
        });
        Ok(PipeAac {
            stdin: child.stdin.take(),
            tool,
            child,
            out,
            err: Some(err),
            pending: Vec::new(),
            rate,
            channels,
            config: None,
        })
    }

    /// Take every whole ADTS frame received, as raw AAC.
    fn take(&mut self, out: &mut Vec<Vec<u8>>) -> Result<(), VideoError> {
        while let Ok(b) = self.out.try_recv() {
            self.pending.extend_from_slice(&b);
        }
        let mut at = 0;
        while let Some(f) = adts(&self.pending[at..]) {
            let (head, len) = f?;
            if self.config.is_none() {
                self.config = Some(head.config());
            }
            out.push(self.pending[at + head.header..at + len].to_vec());
            at += len;
        }
        self.pending.drain(..at);
        Ok(())
    }
}

/// An ADTS header's facts.
struct Adts {
    header: usize,
    object: u8,
    rate_index: u8,
    channels: u8,
}

impl Adts {
    fn config(&self) -> Vec<u8> {
        let bits = (self.object as u16) << 11
            | (self.rate_index as u16) << 7
            | (self.channels as u16) << 3;
        bits.to_be_bytes().to_vec()
    }
}

/// The frame at the start of `b`: `None` until it is all there.
fn adts(b: &[u8]) -> Option<Result<(Adts, usize), VideoError>> {
    if b.len() < 7 {
        return None;
    }
    if b[0] != 0xff || b[1] & 0xf0 != 0xf0 {
        return Some(Err(VideoError::encoder("AAC", "the stream is not ADTS")));
    }
    let crc = b[1] & 1 == 0;
    let len = ((b[3] as usize & 3) << 11) | (b[4] as usize) << 3 | (b[5] as usize) >> 5;
    let header = if crc { 9 } else { 7 };
    if len < header {
        return Some(Err(VideoError::encoder("AAC", "a broken ADTS frame")));
    }
    if b.len() < len {
        return None;
    }
    Some(Ok((
        Adts {
            header,
            object: (b[2] >> 6) + 1,
            rate_index: (b[2] >> 2) & 0xf,
            channels: (b[2] & 1) << 2 | b[3] >> 6,
        },
        len,
    )))
}

impl AudioEncoder for PipeAac {
    fn name(&self) -> String {
        match self.tool {
            Tool::Ffmpeg(_) => "AAC-LC (the system's FFmpeg)".into(),
            Tool::GStreamer(_) => "AAC-LC (the system's GStreamer)".into(),
        }
    }
    fn codec(&self) -> AudioCodec {
        AudioCodec::Aac {
            config: self.config.clone().unwrap_or_else(|| {
                crate::audio::aac_lc_config(self.rate, self.channels)
                    .map(|c| c.to_vec())
                    .unwrap_or_default()
            }),
        }
    }
    fn rate(&self) -> u32 {
        self.rate
    }
    fn channels(&self) -> u16 {
        self.channels
    }
    fn frame_len(&self) -> u32 {
        1_024
    }
    fn delay(&self) -> u32 {
        AAC_DELAY
    }
    fn bitrate(&self) -> u32 {
        BITRATE
    }
    fn encode(&mut self, pcm: &[i16], out: &mut Vec<Vec<u8>>) -> Result<(), VideoError> {
        let bytes: Vec<u8> = pcm.iter().flat_map(|s| s.to_le_bytes()).collect();
        let w = self
            .stdin
            .as_mut()
            .expect("open until finish")
            .write_all(&bytes);
        if let Err(e) = w {
            return Err(VideoError::encoder(
                "AAC",
                format!("{e}: {}", self.stderr()),
            ));
        }
        self.take(out)
    }
    fn finish(&mut self, out: &mut Vec<Vec<u8>>) -> Result<(), VideoError> {
        drop(self.stdin.take());
        let status = self.child.wait()?;
        // The reader ends at the pipe's end; take what it still holds.
        while let Ok(b) = self.out.recv() {
            self.pending.extend_from_slice(&b);
        }
        self.take(out)?;
        if !status.success() {
            return Err(VideoError::encoder("AAC", self.stderr()));
        }
        Ok(())
    }
}

impl PipeAac {
    fn stderr(&mut self) -> String {
        self.err
            .take()
            .and_then(|h| h.join().ok())
            .unwrap_or_default()
            .trim()
            .to_string()
    }
}

impl Drop for PipeAac {
    fn drop(&mut self) {
        drop(self.stdin.take());
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// LAME's encoder delay plus the MP3 decoder's (576 + 529), measured by the probe and
/// `tests/alt_audio.rs` at 0 ms.
pub const MP3_DELAY: u32 = 1_105;

/// MP3 (LAME) frames for the MP4.
pub struct Mp3 {
    lame: mp3lame_encoder::Encoder,
    rate: u32,
    pending: Vec<u8>,
    scratch: Vec<u8>,
}

impl Mp3 {
    pub fn new(rate: u32) -> Result<Mp3, VideoError> {
        let e = |e: &dyn std::fmt::Debug| VideoError::encoder("MP3", format!("{e:?}"));
        let mut b = mp3lame_encoder::Builder::new().ok_or_else(|| e(&"no LAME"))?;
        b.set_num_channels(2).map_err(|x| e(&x))?;
        b.set_sample_rate(rate).map_err(|x| e(&x))?;
        b.set_brate(mp3lame_encoder::Bitrate::Kbps192)
            .map_err(|x| e(&x))?;
        b.set_quality(mp3lame_encoder::Quality::NearBest)
            .map_err(|x| e(&x))?;
        // No Xing/LAME tag frame: every packet is audio, so the edit list's count holds.
        b.set_to_write_vbr_tag(false).map_err(|x| e(&x))?;
        Ok(Mp3 {
            lame: b.build().map_err(|x| e(&x))?,
            rate,
            pending: Vec::new(),
            scratch: Vec::new(),
        })
    }

    fn split(&mut self, out: &mut Vec<Vec<u8>>) {
        let mut at = 0;
        while let Some(len) = mp3_frame_len(&self.pending[at..]) {
            if at + len > self.pending.len() {
                break;
            }
            out.push(self.pending[at..at + len].to_vec());
            at += len;
        }
        self.pending.drain(..at);
    }
}

/// The length of the MPEG-1 Layer III frame at the start of `b` (at 32, 44.1 or 48 kHz).
pub fn mp3_frame_len(b: &[u8]) -> Option<usize> {
    const KBPS: [u32; 15] = [
        0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
    ];
    const RATES: [u32; 3] = [44_100, 48_000, 32_000];
    if b.len() < 4 || b[0] != 0xff || b[1] & 0xfe != 0xfa {
        return None;
    }
    let (k, r) = ((b[2] >> 4) as usize, ((b[2] >> 2) & 3) as usize);
    if k == 0 || k == 15 || r == 3 {
        return None;
    }
    let pad = ((b[2] >> 1) & 1) as u32;
    Some((144 * KBPS[k] * 1000 / RATES[r] + pad) as usize)
}

impl AudioEncoder for Mp3 {
    fn name(&self) -> String {
        "MP3 (LAME)".into()
    }
    fn codec(&self) -> AudioCodec {
        AudioCodec::Mp3
    }
    fn rate(&self) -> u32 {
        self.rate
    }
    fn channels(&self) -> u16 {
        2
    }
    fn frame_len(&self) -> u32 {
        1_152
    }
    fn delay(&self) -> u32 {
        MP3_DELAY
    }
    fn bitrate(&self) -> u32 {
        BITRATE
    }
    fn encode(&mut self, pcm: &[i16], out: &mut Vec<Vec<u8>>) -> Result<(), VideoError> {
        self.scratch.clear();
        // LAME takes an empty buffer for an unlimited one (and overruns): give it room.
        self.scratch.reserve(pcm.len() * 5 / 4 + 7_200);
        self.lame
            .encode_to_vec(mp3lame_encoder::InterleavedPcm(pcm), &mut self.scratch)
            .map_err(|e| VideoError::encoder("MP3", format!("{e:?}")))?;
        self.pending.extend_from_slice(&self.scratch);
        self.split(out);
        Ok(())
    }
    fn finish(&mut self, out: &mut Vec<Vec<u8>>) -> Result<(), VideoError> {
        self.scratch.clear();
        self.scratch.reserve(7_200 * 4);
        self.lame
            .flush_to_vec::<mp3lame_encoder::FlushGap>(&mut self.scratch)
            .map_err(|e| VideoError::encoder("MP3", format!("{e:?}")))?;
        self.pending.extend_from_slice(&self.scratch);
        self.split(out);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adts_headers_are_read() {
        // AAC-LC, 48 kHz (index 3), 2 channels, no CRC, frame of 10 bytes.
        let len = 10usize;
        let h = [
            0xff,
            0xf1,
            (1 << 6) | (3 << 2),
            (2 << 6) | ((len >> 11) as u8 & 3),
            (len >> 3) as u8,
            ((len & 7) as u8) << 5 | 0x1f,
            0xfc,
        ];
        let mut b = h.to_vec();
        b.extend_from_slice(&[1, 2, 3]);
        let (a, l) = adts(&b).unwrap().unwrap();
        assert_eq!((a.header, l), (7, 10));
        assert_eq!(a.config(), vec![0x11, 0x90]);
        assert!(adts(&b[..9]).is_none());
        assert!(adts(&[0, 1, 2, 3, 4, 5, 6]).unwrap().is_err());
    }

    #[test]
    fn mp3_frame_lengths() {
        // 192 kbit/s at 48 kHz: 576 bytes; at 44.1 kHz with padding: 627.
        assert_eq!(mp3_frame_len(&[0xff, 0xfb, 0xb4, 0]), Some(576));
        assert_eq!(mp3_frame_len(&[0xff, 0xfb, 0xb2, 0]), Some(627));
        assert_eq!(mp3_frame_len(&[0xff, 0xfb, 0x04, 0]), None);
    }

    #[test]
    fn every_program_started_here_opens_no_window() {
        let src = include_str!("audio_alt.rs");
        let code = &src[..src.find("#[cfg(test)]").unwrap()];
        let made = code.matches("Command::new(").count();
        let quieted = code.matches("quiet(&mut Command::new(").count()
            + code.matches("quiet(&mut c)").count();
        assert_eq!(made, quieted, "a Command without quiet()");
        assert!(made >= 3);
    }
}
