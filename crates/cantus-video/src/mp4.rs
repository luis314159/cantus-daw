//! Our MP4 writer (ADR 0088 §1): one H.264 track and one audio track (AAC-LC, or MP3),
//! `moov` in front of `mdat` (fast start: a platform or a browser starts playing before the
//! file has arrived), video and audio interleaved a frame at a time.
//!
//! - **Video**: one sample a frame, every frame lasting exactly its ticks on a 30 000 Hz
//!   timescale; no B-frames, so no `ctts` and no edit list — frame 0 is time 0. `avcC`
//!   from the stream's parameter sets, `colr` saying BT.709 limited range (what
//!   [`crate::yuv`] wrote), square pixels.
//! - **Audio**: one sample a packet; an **edit list** starts the presentation at the
//!   encoder's delay and lasts exactly the song's length, so the priming is skipped and the
//!   padding at the end too.
//! - The movie's timescale is the video's (30 000): every duration is exact.

use crate::audio::AudioCodec;
use crate::bmff::{bx, full, u32s, MATRIX};
use crate::h264;
use crate::mux::{Spool, TrackLog};
use crate::timing::{Timing, TIMESCALE};
use crate::VideoError;
use std::io::{Read, Seek, Write};

/// The audio stream the MP4 carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioParams {
    pub codec: AudioCodec,
    pub rate: u32,
    pub channels: u16,
    /// Samples a packet decodes to (AAC 1 024, MP3 1 152).
    pub frame_len: u32,
    /// The encoder's priming, in samples: where the edit list starts.
    pub delay: u32,
}

const VIDEO: usize = 0;
const AUDIO: usize = 1;

/// An MP4 being written.
pub struct Mp4Writer<W: Read + Write + Seek> {
    spool: Spool<W>,
    width: u32,
    height: u32,
    timing: Timing,
    audio: AudioParams,
    sps: Option<Vec<u8>>,
    pps: Option<Vec<u8>>,
}

impl<W: Read + Write + Seek> Mp4Writer<W> {
    pub fn new(
        out: W,
        width: u32,
        height: u32,
        timing: Timing,
        audio: AudioParams,
    ) -> std::io::Result<Self> {
        let packets = (timing.audio_samples() + audio.delay as u64) / audio.frame_len as u64 + 4;
        let spool = Spool::new(out, 2, timing.frames as u64 + packets)?;
        Ok(Mp4Writer {
            spool,
            width,
            height,
            timing,
            audio,
            sps: None,
            pps: None,
        })
    }

    /// Video frames written so far.
    pub fn frames(&self) -> u32 {
        self.spool.tracks[VIDEO].samples()
    }

    /// Bytes of media written so far.
    pub fn bytes(&self) -> u64 {
        self.spool.written()
    }

    /// The next frame's access unit (Annex B).
    pub fn video(&mut self, annexb: &[u8]) -> Result<(), VideoError> {
        let s = h264::to_sample(annexb);
        if self.sps.is_none() {
            self.sps = s.sps;
            self.pps = s.pps;
        }
        if s.data.is_empty() {
            return Err(VideoError::encoder(
                "H.264",
                "an access unit with no picture",
            ));
        }
        self.spool.sample(VIDEO, &s.data, s.idr)?;
        Ok(())
    }

    /// The next audio packet.
    pub fn audio(&mut self, packet: &[u8]) -> Result<(), VideoError> {
        self.spool.sample(AUDIO, packet, true)?;
        Ok(())
    }

    /// Write the index in front and give the output back.
    pub fn finish(self) -> Result<W, VideoError> {
        let (Some(sps), Some(pps)) = (self.sps.clone(), self.pps.clone()) else {
            return Err(VideoError::encoder("H.264", "no sequence parameter set"));
        };
        if self.frames() != self.timing.frames {
            return Err(VideoError::encoder(
                "H.264",
                format!(
                    "{} frames came out of {}",
                    self.frames(),
                    self.timing.frames
                ),
            ));
        }
        let head = Header {
            width: self.width,
            height: self.height,
            timing: self.timing,
            audio: self.audio.clone(),
            avcc: h264::avcc(&sps, &pps),
        };
        Ok(self
            .spool
            .finish(|t, base, wide| head.build(t, base, wide))?)
    }
}

struct Header {
    width: u32,
    height: u32,
    timing: Timing,
    audio: AudioParams,
    avcc: Vec<u8>,
}

/// `tkhd`, `mdhd` and `hdlr` with the media's own box.
#[allow(clippy::too_many_arguments)]
fn track(
    id: u32,
    duration: u32,
    size: Option<(u32, u32)>,
    scale: u32,
    media_duration: u32,
    handler: &[u8; 4],
    name: &str,
    stbl: Vec<u8>,
    edit: Option<Vec<u8>>,
) -> Vec<u8> {
    let video = size.is_some();
    let mut tk = u32s(&[0, 0, id, 0, duration, 0, 0]);
    tk.extend_from_slice(&[0, 0, 0, if video { 0 } else { 1 }]); // layer, alternate group
    tk.extend_from_slice(&[if video { 0 } else { 1 }, 0, 0, 0]); // volume, reserved
    tk.extend_from_slice(&u32s(&MATRIX));
    let (w, h) = size.unwrap_or((0, 0));
    tk.extend_from_slice(&u32s(&[w << 16, h << 16]));
    let tkhd = full(b"tkhd", 0, 3, &tk);
    let mut md = u32s(&[0, 0, scale, media_duration]);
    md.extend_from_slice(&[0x55, 0xc4, 0, 0]); // language "und"
    let mdhd = full(b"mdhd", 0, 0, &md);
    let mut hd = u32s(&[0]);
    hd.extend_from_slice(handler);
    hd.extend_from_slice(&[0; 12]);
    hd.extend_from_slice(name.as_bytes());
    hd.push(0);
    let hdlr = full(b"hdlr", 0, 0, &hd);
    let xmhd = if video {
        full(b"vmhd", 0, 1, &[0; 8])
    } else {
        full(b"smhd", 0, 0, &[0; 4])
    };
    let dref = full(
        b"dref",
        0,
        0,
        &[u32s(&[1]), full(b"url ", 0, 1, &[])].concat(),
    );
    let minf = bx(b"minf", &[xmhd, bx(b"dinf", &dref), stbl].concat());
    let mdia = bx(b"mdia", &[mdhd, hdlr, minf].concat());
    let mut parts = vec![tkhd];
    if let Some(e) = edit {
        parts.push(bx(b"edts", &e));
    }
    parts.push(mdia);
    bx(b"trak", &parts.concat())
}

impl Header {
    fn build(&self, tracks: &[TrackLog], base: u64, wide: bool) -> Vec<u8> {
        let ftyp = bx(
            b"ftyp",
            &[
                b"isom".to_vec(),
                u32s(&[0x200]),
                b"isomiso2avc1mp41".to_vec(),
            ]
            .concat(),
        );
        let t = self.timing;
        let vdur = t.frames * t.frame_ticks();
        let moov = bx(
            b"moov",
            &[
                mvhd(vdur),
                self.video_track(&tracks[VIDEO], base, wide),
                self.audio_track(&tracks[AUDIO], base, wide),
            ]
            .concat(),
        );
        [ftyp, moov].concat()
    }

    fn video_track(&self, log: &TrackLog, base: u64, wide: bool) -> Vec<u8> {
        let t = self.timing;
        let mut e = vec![0u8; 6];
        e.extend_from_slice(&[0, 1]); // data reference index
        e.extend_from_slice(&[0; 16]);
        e.extend_from_slice(&(self.width as u16).to_be_bytes());
        e.extend_from_slice(&(self.height as u16).to_be_bytes());
        e.extend_from_slice(&u32s(&[0x48_0000, 0x48_0000, 0])); // 72 dpi, reserved
        e.extend_from_slice(&[0, 1]); // frame count
        let mut name = [0u8; 32];
        let label = b"Cantus H.264";
        name[0] = label.len() as u8;
        name[1..=label.len()].copy_from_slice(label);
        e.extend_from_slice(&name);
        e.extend_from_slice(&[0, 0x18, 0xff, 0xff]); // depth 24, pre-defined -1
        e.extend_from_slice(&bx(b"avcC", &self.avcc));
        // BT.709 primaries, transfer and matrix, limited range.
        e.extend_from_slice(&bx(
            b"colr",
            &[b"nclx".as_slice(), &[0, 1, 0, 1, 0, 1, 0]].concat(),
        ));
        e.extend_from_slice(&bx(b"pasp", &u32s(&[1, 1])));
        let stsd = full(b"stsd", 0, 0, &[u32s(&[1]), bx(b"avc1", &e)].concat());
        let stts = full(b"stts", 0, 0, &u32s(&[1, log.samples(), t.frame_ticks()]));
        let stbl = bx(
            b"stbl",
            &[stsd, stts, log.sync_table(), log.tables(base, wide)].concat(),
        );
        let vdur = t.frames * t.frame_ticks();
        let size = Some((self.width, self.height));
        track(
            1,
            vdur,
            size,
            TIMESCALE,
            vdur,
            b"vide",
            "Cantus video",
            stbl,
            None,
        )
    }

    fn audio_track(&self, log: &TrackLog, base: u64, wide: bool) -> Vec<u8> {
        let a = &self.audio;
        let packets = log.samples();
        let media = packets as u64 * a.frame_len as u64;
        let total: u64 = log.sizes.iter().map(|&s| s as u64).sum();
        let avg = (total * 8 * a.rate as u64 / media.max(1)) as u32;
        let per_second = (a.rate / a.frame_len).max(1) as usize;
        let max = log
            .sizes
            .windows(per_second.min(log.sizes.len()).max(1))
            .map(|w| w.iter().map(|&s| s as u64).sum::<u64>() * 8)
            .max()
            .unwrap_or(0) as u32;
        let biggest = log.sizes.iter().copied().max().unwrap_or(0);
        let mut dcd = vec![a.codec.object_type(), 0x15];
        dcd.extend_from_slice(&biggest.to_be_bytes()[1..]);
        dcd.extend_from_slice(&u32s(&[max.max(avg), avg]));
        if let AudioCodec::Aac { config } = &a.codec {
            dcd.push(0x05);
            dcd.push(config.len() as u8);
            dcd.extend_from_slice(config);
        }
        let mut es = vec![0, 2, 0, 0x04, dcd.len() as u8]; // ES_ID 2, no flags
        es.extend_from_slice(&dcd);
        es.extend_from_slice(&[0x06, 1, 2]); // SLConfig: MP4
        let mut esd = vec![0x03, es.len() as u8];
        esd.extend_from_slice(&es);
        let mut e = vec![0u8; 6];
        e.extend_from_slice(&[0, 1]);
        e.extend_from_slice(&[0; 8]);
        e.extend_from_slice(&a.channels.to_be_bytes());
        e.extend_from_slice(&[0, 16, 0, 0, 0, 0]); // 16-bit, pre-defined, reserved
        e.extend_from_slice(&(a.rate << 16).to_be_bytes());
        e.extend_from_slice(&full(b"esds", 0, 0, &esd));
        let stsd = full(b"stsd", 0, 0, &[u32s(&[1]), bx(b"mp4a", &e)].concat());
        let stts = full(b"stts", 0, 0, &u32s(&[1, packets, a.frame_len]));
        let stbl = bx(b"stbl", &[stsd, stts, log.tables(base, wide)].concat());
        // The edit: from the encoder's delay, exactly the song's length (movie ticks).
        let len = self.timing.audio_samples();
        let ticks = ((len * TIMESCALE as u64 + a.rate as u64 / 2) / a.rate as u64) as u32;
        let elst = full(b"elst", 0, 0, &u32s(&[1, ticks, a.delay, 0x1_0000]));
        track(
            2,
            ticks,
            None,
            a.rate,
            media as u32,
            b"soun",
            "Cantus audio",
            stbl,
            Some(elst),
        )
    }
}

/// The movie header: timescale 30 000, `duration` ticks, next track 3.
pub(crate) fn mvhd(duration: u32) -> Vec<u8> {
    let mut mv = u32s(&[0, 0, TIMESCALE, duration, 0x1_0000]);
    mv.extend_from_slice(&[1, 0]); // volume 1.0
    mv.extend_from_slice(&[0; 10]);
    mv.extend_from_slice(&u32s(&MATRIX));
    mv.extend_from_slice(&[0; 24]);
    mv.extend_from_slice(&u32s(&[3]));
    full(b"mvhd", 0, 0, &mv)
}
