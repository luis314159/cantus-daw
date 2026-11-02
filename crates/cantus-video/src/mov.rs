//! Our QuickTime MOV writer (ADR 0088 §3): PNG frames — RGBA with straight alpha (the
//! transparent layer for a video editor) or RGB — and 16-bit little-endian PCM (`sowt`).
//! Lossless both ways: what is decoded is what was drawn, bit for bit, and the PCM is the
//! song's samples. No patents, nothing downloaded, the same on every system.
//!
//! Frames and their audio are interleaved a frame at a time; the index goes in front
//! ([`crate::mux`]).

use crate::bmff::{bx, full, u32s, MATRIX};
use crate::mp4::mvhd;
use crate::mux::{Spool, TrackLog};
use crate::timing::{Timing, TIMESCALE};
use crate::VideoError;
use std::io::{Read, Seek, Write};

const VIDEO: usize = 0;
const AUDIO: usize = 1;
/// Bytes of one stereo 16-bit PCM sample.
const PCM_FRAME: u32 = 4;

/// A MOV being written.
pub struct MovWriter<W: Read + Write + Seek> {
    spool: Spool<W>,
    width: u32,
    height: u32,
    timing: Timing,
    alpha: bool,
}

impl<W: Read + Write + Seek> MovWriter<W> {
    /// A MOV of `timing.frames` PNG frames (`alpha`: RGBA, else RGB) with stereo audio.
    pub fn new(
        out: W,
        width: u32,
        height: u32,
        timing: Timing,
        alpha: bool,
    ) -> std::io::Result<Self> {
        let spool = Spool::new(out, 2, 2 * timing.frames as u64)?;
        Ok(MovWriter {
            spool,
            width,
            height,
            timing,
            alpha,
        })
    }

    pub fn frames(&self) -> u32 {
        self.spool.tracks[VIDEO].samples()
    }

    pub fn bytes(&self) -> u64 {
        self.spool.written()
    }

    /// The next frame (a PNG file's bytes) and its audio (interleaved stereo 16-bit).
    pub fn frame(&mut self, png: &[u8], audio: &[i16]) -> Result<(), VideoError> {
        self.spool.sample(VIDEO, png, true)?;
        let bytes: Vec<u8> = audio.iter().flat_map(|s| s.to_le_bytes()).collect();
        self.spool.uniform(AUDIO, &bytes, PCM_FRAME)?;
        Ok(())
    }

    /// Write the index in front and give the output back.
    pub fn finish(self) -> Result<W, VideoError> {
        if self.frames() != self.timing.frames {
            return Err(VideoError::encoder(
                "PNG",
                format!("{} frames of {}", self.frames(), self.timing.frames),
            ));
        }
        let (w, h, t, alpha) = (self.width, self.height, self.timing, self.alpha);
        Ok(self
            .spool
            .finish(|tracks, base, wide| header(w, h, t, alpha, tracks, base, wide))?)
    }
}

/// A QuickTime handler: component type, subtype, manufacturer, and a Pascal-string name.
fn hdlr(component: &[u8; 4], subtype: &[u8; 4], name: &str) -> Vec<u8> {
    let mut h = component.to_vec();
    h.extend_from_slice(subtype);
    h.extend_from_slice(b"appl");
    h.extend_from_slice(&[0; 8]);
    h.push(name.len() as u8);
    h.extend_from_slice(name.as_bytes());
    full(b"hdlr", 0, 0, &h)
}

#[allow(clippy::too_many_arguments)]
fn trak(
    id: u32,
    size: Option<(u32, u32)>,
    duration: u32,
    scale: u32,
    media_duration: u32,
    stbl: Vec<u8>,
) -> Vec<u8> {
    let video = size.is_some();
    let mut tk = u32s(&[0, 0, id, 0, duration, 0, 0]);
    tk.extend_from_slice(&[0, 0, 0, 0]);
    tk.extend_from_slice(&[if video { 0 } else { 1 }, 0, 0, 0]);
    tk.extend_from_slice(&u32s(&MATRIX));
    let (w, h) = size.unwrap_or((0, 0));
    tk.extend_from_slice(&u32s(&[w << 16, h << 16]));
    let tkhd = full(b"tkhd", 0, 0xf, &tk);
    let mut md = u32s(&[0, 0, scale, media_duration]);
    md.extend_from_slice(&[0x55, 0xc4, 0, 0]);
    let mdhd = full(b"mdhd", 0, 0, &md);
    let (sub, name) = if video {
        (b"vide", "VideoHandler")
    } else {
        (b"soun", "SoundHandler")
    };
    let media_hdlr = hdlr(b"mhlr", sub, name);
    let xmhd = if video {
        // Graphics mode "dither copy", no opcolour.
        full(b"vmhd", 0, 1, &[0, 0x40, 0x80, 0, 0x80, 0, 0x80, 0])
    } else {
        full(b"smhd", 0, 0, &[0; 4])
    };
    let data_hdlr = hdlr(b"dhlr", b"alis", "DataHandler");
    let dref = full(
        b"dref",
        0,
        0,
        &[u32s(&[1]), full(b"alis", 0, 1, &[])].concat(),
    );
    let minf = bx(
        b"minf",
        &[xmhd, data_hdlr, bx(b"dinf", &dref), stbl].concat(),
    );
    let mdia = bx(b"mdia", &[mdhd, media_hdlr, minf].concat());
    bx(b"trak", &[tkhd, mdia].concat())
}

fn header(
    w: u32,
    h: u32,
    t: Timing,
    alpha: bool,
    tracks: &[TrackLog],
    base: u64,
    wide: bool,
) -> Vec<u8> {
    let ftyp = bx(
        b"ftyp",
        &[b"qt  ".to_vec(), u32s(&[0x200]), b"qt  ".to_vec()].concat(),
    );
    let vdur = t.frames * t.frame_ticks();
    // Video: 'png ', depth 32 with alpha (24 without), compressor "PNG".
    let mut e = vec![0u8; 6];
    e.extend_from_slice(&[0, 1, 0, 0, 0, 0]); // data reference, version, revision
    e.extend_from_slice(b"appl");
    e.extend_from_slice(&u32s(&[0, 512])); // temporal and spatial quality
    e.extend_from_slice(&(w as u16).to_be_bytes());
    e.extend_from_slice(&(h as u16).to_be_bytes());
    e.extend_from_slice(&u32s(&[0x48_0000, 0x48_0000, 0]));
    e.extend_from_slice(&[0, 1]);
    let mut name = [0u8; 32];
    name[0] = 3;
    name[1..4].copy_from_slice(b"PNG");
    e.extend_from_slice(&name);
    e.extend_from_slice(&[0, if alpha { 32 } else { 24 }, 0xff, 0xff]);
    let vstsd = full(b"stsd", 0, 0, &[u32s(&[1]), bx(b"png ", &e)].concat());
    let vlog = &tracks[VIDEO];
    let vstts = full(b"stts", 0, 0, &u32s(&[1, vlog.samples(), t.frame_ticks()]));
    let vstbl = bx(b"stbl", &[vstsd, vstts, vlog.tables(base, wide)].concat());
    let video = trak(1, Some((w, h)), vdur, TIMESCALE, vdur, vstbl);
    // Sound: 'sowt' version 0, stereo, 16-bit, at the song's rate.
    let mut a = vec![0u8; 6];
    a.extend_from_slice(&[0, 1]);
    a.extend_from_slice(&[0; 8]);
    a.extend_from_slice(&[0, 2, 0, 16, 0, 0, 0, 0]);
    a.extend_from_slice(&(t.rate << 16).to_be_bytes());
    let astsd = full(b"stsd", 0, 0, &[u32s(&[1]), bx(b"sowt", &a)].concat());
    let alog = &tracks[AUDIO];
    let astts = full(b"stts", 0, 0, &u32s(&[1, alog.samples(), 1]));
    let astbl = bx(b"stbl", &[astsd, astts, alog.tables(base, wide)].concat());
    let samples = t.audio_samples();
    let adur = (samples * TIMESCALE as u64 / t.rate as u64) as u32;
    let audio = trak(2, None, adur, t.rate, samples as u32, astbl);
    let moov = bx(b"moov", &[mvhd(vdur), video, audio].concat());
    [ftyp, moov].concat()
}
