//! The four writers. Each takes the finished samples — at the file's rate and channels,
//! normalised — and writes a whole file to `out`, telling `progress` how far it has got;
//! `progress` answering `false` stops it with [`ExportError::Cancelled`].

use crate::process::{quantise, Audio};
use crate::settings::{Depth, Dither, ExportSettings, Format, Mp3Mode, Tags};
use crate::tags;
use crate::ExportError;
use std::io::{Seek, SeekFrom, Write};

/// What the progress callback is: the fraction done; `false` stops.
pub type Progress<'a> = dyn FnMut(f32) -> bool + 'a;

/// Frames per step between two progress reports.
const STEP: usize = 16_384;

/// Write `a` in the format `s` asks for.
pub fn write<W: Write + Seek>(
    out: &mut W,
    a: &Audio,
    s: &ExportSettings,
    tags: &Tags,
    progress: &mut Progress<'_>,
) -> Result<(), ExportError> {
    let credit = tags::encoder_credit();
    match s.format {
        Format::Wav => wav(out, a, s.depth, s.dither, tags, &credit, progress),
        Format::Flac => flac(out, a, s.flac_depth(), s.dither, tags, &credit, progress),
        Format::Mp3 => mp3(out, a, s.mp3, tags, &credit, progress),
        Format::Ogg => ogg(out, a, s.ogg_quality, tags, &credit, progress),
    }
}

// ---- WAV ----------------------------------------------------------------------------

/// RIFF/WAVE: `fmt ` (PCM, or IEEE float with its `fact`), the `LIST/INFO` tags, `data`.
pub fn wav<W: Write>(
    out: &mut W,
    a: &Audio,
    depth: Depth,
    dither: Dither,
    tags: &Tags,
    credit: &str,
    progress: &mut Progress<'_>,
) -> Result<(), ExportError> {
    let ch = a.channels.max(1);
    let bytes = (depth.bits() / 8) as u32;
    let data_len = a.samples.len() as u64 * bytes as u64;
    let info = tags::riff_info(tags, credit);
    let float = depth == Depth::Float32;
    let fmt_len: u32 = if float { 18 } else { 16 };
    let fact = if float { 12 } else { 0 };
    let riff_len = 4 + (8 + fmt_len as u64) + fact + info.len() as u64 + 8 + data_len + data_len % 2;
    if riff_len > u32::MAX as u64 {
        return Err(ExportError::TooLong);
    }
    let mut h = Vec::with_capacity(80 + info.len());
    h.extend_from_slice(b"RIFF");
    h.extend_from_slice(&(riff_len as u32).to_le_bytes());
    h.extend_from_slice(b"WAVEfmt ");
    h.extend_from_slice(&fmt_len.to_le_bytes());
    h.extend_from_slice(&(if float { 3u16 } else { 1u16 }).to_le_bytes());
    h.extend_from_slice(&ch.to_le_bytes());
    h.extend_from_slice(&a.rate.to_le_bytes());
    h.extend_from_slice(&(a.rate * bytes * ch as u32).to_le_bytes());
    h.extend_from_slice(&((bytes * ch as u32) as u16).to_le_bytes());
    h.extend_from_slice(&depth.bits().to_le_bytes());
    if float {
        h.extend_from_slice(&0u16.to_le_bytes());
        h.extend_from_slice(b"fact");
        h.extend_from_slice(&4u32.to_le_bytes());
        h.extend_from_slice(&(a.frames() as u32).to_le_bytes());
    }
    h.extend_from_slice(&info);
    h.extend_from_slice(b"data");
    h.extend_from_slice(&(data_len as u32).to_le_bytes());
    out.write_all(&h)?;
    let total = a.samples.len().max(1);
    let step = STEP * ch as usize;
    let mut buf = Vec::with_capacity(step * bytes as usize);
    if float {
        for (i, block) in a.samples.chunks(step).enumerate() {
            buf.clear();
            block.iter().for_each(|x| buf.extend_from_slice(&x.to_le_bytes()));
            out.write_all(&buf)?;
            if !progress(((i + 1) * step).min(total) as f32 / total as f32) {
                return Err(ExportError::Cancelled);
            }
        }
    } else {
        let ints = quantise(&a.samples, ch, depth.bits(), dither);
        for (i, block) in ints.chunks(step).enumerate() {
            buf.clear();
            for v in block {
                buf.extend_from_slice(&v.to_le_bytes()[..bytes as usize]);
            }
            out.write_all(&buf)?;
            if !progress(((i + 1) * step).min(total) as f32 / total as f32) {
                return Err(ExportError::Cancelled);
            }
        }
    }
    if data_len % 2 == 1 {
        out.write_all(&[0])?;
    }
    Ok(())
}

// ---- FLAC ---------------------------------------------------------------------------

/// The integers as flacenc reads them, a block at a time, with the progress asked after
/// every block.
struct FlacSource<'a, 'p> {
    ints: &'a [i32],
    channels: usize,
    bits: usize,
    rate: usize,
    at: usize,
    progress: &'a mut Progress<'p>,
    cancelled: bool,
}

impl flacenc::source::Source for FlacSource<'_, '_> {
    fn channels(&self) -> usize {
        self.channels
    }
    fn bits_per_sample(&self) -> usize {
        self.bits
    }
    fn sample_rate(&self) -> usize {
        self.rate
    }
    fn read_samples<F: flacenc::source::Fill>(
        &mut self,
        block_size: usize,
        dest: &mut F,
    ) -> Result<usize, flacenc::error::SourceError> {
        let frames_left = (self.ints.len() - self.at) / self.channels;
        let n = block_size.min(frames_left);
        let end = self.at + n * self.channels;
        dest.fill_interleaved(&self.ints[self.at..end])?;
        self.at = end;
        if !(self.progress)(self.at as f32 / self.ints.len().max(1) as f32) {
            self.cancelled = true;
            return Err(flacenc::error::SourceError::from_unknown());
        }
        Ok(n)
    }
    fn len_hint(&self) -> Option<usize> {
        Some(self.ints.len() / self.channels)
    }
}

/// FLAC (pure Rust, flacenc), 16 or 24 bits, with a `VORBIS_COMMENT` block. Lossless:
/// decoded, it gives back exactly the integers [`quantise`] made.
pub fn flac<W: Write>(
    out: &mut W,
    a: &Audio,
    depth: Depth,
    dither: Dither,
    tags: &Tags,
    credit: &str,
    progress: &mut Progress<'_>,
) -> Result<(), ExportError> {
    use flacenc::component::BitRepr;
    use flacenc::error::Verify;
    let ch = a.channels.max(1);
    let bits = depth.bits();
    let ints = quantise(&a.samples, ch, bits, dither);
    let config = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|e| ExportError::Encode(format!("{e:?}")))?;
    let mut src = FlacSource {
        ints: &ints,
        channels: ch as usize,
        bits: bits as usize,
        rate: a.rate as usize,
        at: 0,
        progress,
        cancelled: false,
    };
    let block = config.block_size;
    let result = flacenc::encode_with_fixed_block_size(&config, &mut src, block);
    if src.cancelled {
        return Err(ExportError::Cancelled);
    }
    let mut stream = result.map_err(|e| ExportError::Encode(format!("{e:?}")))?;
    let comments = tags::vorbis_comment_block(tags, credit);
    let block = flacenc::component::MetadataBlockData::new_unknown(4, &comments)
        .map_err(|e| ExportError::Encode(format!("{e:?}")))?;
    stream.add_metadata_block(block);
    let mut sink = flacenc::bitsink::ByteSink::new();
    stream
        .write(&mut sink)
        .map_err(|e| ExportError::Encode(format!("{e:?}")))?;
    out.write_all(sink.as_slice())?;
    Ok(())
}

// ---- MP3 ----------------------------------------------------------------------------

/// LAME's quality for a VBR level, V0 … V9.
fn lame_level(v: u8) -> mp3lame_encoder::Quality {
    use mp3lame_encoder::Quality::*;
    match v {
        0 => Best,
        1 => SecondBest,
        2 => NearBest,
        3 => VeryNice,
        4 => Nice,
        5 => Good,
        6 => Decent,
        7 => Ok,
        8 => SecondWorst,
        _ => Worst,
    }
}

fn lame_bitrate(kbps: u16) -> mp3lame_encoder::Bitrate {
    use mp3lame_encoder::Bitrate::*;
    match kbps {
        0..=135 => Kbps128,
        136..=175 => Kbps160,
        176..=207 => Kbps192,
        208..=239 => Kbps224,
        240..=287 => Kbps256,
        _ => Kbps320,
    }
}

fn lame<E: std::fmt::Debug>(e: E) -> ExportError {
    ExportError::Encode(format!("LAME: {e:?}"))
}

/// MP3 through LAME 3.100: an ID3v2.3 tag, then the stream, its first frame LAME's
/// info tag (Xing/LAME: the length, the encoder delay and padding, so players that
/// honour it — most do — play it gaplessly, at its exact length).
pub fn mp3<W: Write + Seek>(
    out: &mut W,
    a: &Audio,
    mode: Mp3Mode,
    tags: &Tags,
    credit: &str,
    progress: &mut Progress<'_>,
) -> Result<(), ExportError> {
    use mp3lame_encoder::{
        Builder, FlushNoGap, InterleavedPcm, Mode, MonoPcm, Quality, VbrMode,
    };
    let ch = a.channels.clamp(1, 2);
    let mut b = Builder::new().ok_or_else(|| ExportError::Encode("LAME: no memory".into()))?;
    b.set_num_channels(ch as u8).map_err(lame)?;
    b.set_sample_rate(a.rate).map_err(lame)?;
    // Never let LAME pick another rate: the file is at the rate the dialog shows.
    b.set_output_sample_rate(std::num::NonZeroU32::new(a.rate))
        .map_err(lame)?;
    b.set_mode(if ch == 1 { Mode::Mono } else { Mode::JointStereo })
        .map_err(lame)?;
    // q2 (LAME's -h): the recommended quality, as good as q0 to within 0.1 dB of NMR at
    // a third of the time (ADR 0033).
    b.set_quality(Quality::NearBest).map_err(lame)?;
    match mode {
        Mp3Mode::Cbr(kbps) => {
            b.set_vbr_mode(VbrMode::Off).map_err(lame)?;
            b.set_brate(lame_bitrate(kbps)).map_err(lame)?;
        }
        Mp3Mode::Vbr(level) => {
            b.set_vbr_mode(VbrMode::Mtrh).map_err(lame)?;
            b.set_vbr_quality(lame_level(level)).map_err(lame)?;
        }
    }
    b.set_to_write_vbr_tag(true).map_err(lame)?;
    let mut enc = b.build().map_err(lame)?;
    let id3 = tags::id3v2(tags, credit);
    out.write_all(&id3)?;
    let mut buf: Vec<u8> = Vec::new();
    let step = STEP * ch as usize;
    let total = a.samples.len().max(1);
    for (i, block) in a.samples.chunks(step).enumerate() {
        buf.clear();
        buf.reserve(mp3lame_encoder::max_required_buffer_size(block.len() / ch as usize));
        if ch == 1 {
            enc.encode_to_vec(MonoPcm(block), &mut buf).map_err(lame)?;
        } else {
            enc.encode_to_vec(InterleavedPcm(block), &mut buf)
                .map_err(lame)?;
        }
        out.write_all(&buf)?;
        if !progress(((i + 1) * step).min(total) as f32 / total as f32) {
            return Err(ExportError::Cancelled);
        }
    }
    buf.clear();
    buf.reserve(7200);
    enc.flush_to_vec::<FlushNoGap>(&mut buf).map_err(lame)?;
    out.write_all(&buf)?;
    // The info frame, filled in now that the stream's length is known, over the empty
    // one LAME wrote first.
    let mut tag = Vec::new();
    if enc.lame_tag_encode_to_vec(&mut tag).is_some() && !tag.is_empty() {
        let end = out.stream_position()?;
        out.seek(SeekFrom::Start(id3.len() as u64))?;
        out.write_all(&tag)?;
        out.seek(SeekFrom::Start(end))?;
    }
    Ok(())
}

// ---- Ogg Vorbis ---------------------------------------------------------------------

fn vorbis<E: std::fmt::Display>(e: E) -> ExportError {
    ExportError::Encode(format!("Vorbis: {e}"))
}

/// Ogg Vorbis through libvorbis (aoTuV), quality-VBR at `-q quality` (0–10), the tags as
/// Vorbis comments. The stream's serial number comes from the tags and the length, so
/// the same export makes the same bytes.
pub fn ogg<W: Write>(
    out: &mut W,
    a: &Audio,
    quality: u8,
    tags: &Tags,
    credit: &str,
    progress: &mut Progress<'_>,
) -> Result<(), ExportError> {
    use std::num::{NonZeroU32, NonZeroU8};
    use vorbis_rs::{VorbisBitrateManagementStrategy, VorbisEncoderBuilder};
    let ch = a.channels.clamp(1, 2) as usize;
    let rate = NonZeroU32::new(a.rate).ok_or_else(|| vorbis("a rate of 0"))?;
    let channels = NonZeroU8::new(ch as u8).expect("one or two");
    let serial = {
        let mut h: u32 = 0x811c_9dc5;
        for b in tags.title.bytes().chain((a.frames() as u64).to_le_bytes()) {
            h = (h ^ b as u32).wrapping_mul(0x0100_0193);
        }
        h as i32
    };
    let mut b = VorbisEncoderBuilder::new_with_serial(rate, channels, out, serial);
    b.bitrate_management_strategy(VorbisBitrateManagementStrategy::QualityVbr {
        target_quality: (quality.min(10) as f32 / 10.0).clamp(-0.2, 1.0),
    });
    for (k, v) in tags::vorbis_pairs(tags, credit) {
        b.comment_tag(k, v).map_err(vorbis)?;
    }
    let mut enc = b.build().map_err(vorbis)?;
    let frames = a.frames();
    let mut planes = vec![vec![0f32; STEP]; ch];
    let mut at = 0;
    while at < frames {
        let n = STEP.min(frames - at);
        for (c, plane) in planes.iter_mut().enumerate() {
            for (i, x) in plane[..n].iter_mut().enumerate() {
                *x = a.samples[(at + i) * ch + c];
            }
        }
        let block: Vec<&[f32]> = planes.iter().map(|p| &p[..n]).collect();
        enc.encode_audio_block(&block).map_err(vorbis)?;
        at += n;
        if !progress(at as f32 / frames.max(1) as f32) {
            return Err(ExportError::Cancelled);
        }
    }
    enc.finish().map_err(vorbis)?;
    Ok(())
}
