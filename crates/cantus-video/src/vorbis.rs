//! Vorbis for WebM (ADR 0088 §2): libvorbis (aoTuV, already in Cantus for Ogg export)
//! through `vorbis_rs`, its Ogg pages taken apart into packets, and **every packet timed
//! by the samples before it**.
//!
//! A WebM block's time is that of its first decoded sample. Ogg pages only say how many
//! samples are done at their end, so each packet's length is worked out as a decoder does:
//! a packet of block size `b` after one of `a` gives `a/4 + b/4` samples, the first gives
//! none. The block sizes come from the identification header, and which mode (short or
//! long) each packet uses from the setup header — read from its end, as FFmpeg's parser
//! does, since the modes are the setup header's last field.

use crate::VideoError;
use std::cell::RefCell;
use std::io::Write;
use std::num::{NonZeroU32, NonZeroU8};
use std::rc::Rc;
use vorbis_rs::{VorbisBitrateManagementStrategy, VorbisEncoder, VorbisEncoderBuilder};

/// Bits a second.
pub const BITRATE: u32 = 192_000;

/// The Ogg bytes libvorbis writes, shared with us.
#[derive(Clone, Default)]
struct Sink(Rc<RefCell<Vec<u8>>>);

impl Write for Sink {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// One audio packet and where its decoded samples start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Packet {
    pub start: u64,
    pub data: Vec<u8>,
}

/// What the setup header says about packet lengths.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Modes {
    pub blocksize: [u32; 2],
    /// Each mode's block flag (long or short).
    pub long: Vec<bool>,
}

impl Modes {
    /// From the identification and setup headers.
    pub fn read(ident: &[u8], setup: &[u8]) -> Option<Modes> {
        if ident.len() < 30 || &ident[1..7] != b"vorbis" {
            return None;
        }
        let bs = ident[28];
        let blocksize = [1u32 << (bs & 0xf), 1u32 << (bs >> 4)];
        Some(Modes {
            blocksize,
            long: setup_modes(setup)?,
        })
    }

    /// The block size of an audio packet.
    pub fn size_of(&self, packet: &[u8]) -> Option<u32> {
        let first = *packet.first()?;
        if first & 1 != 0 {
            return None; // a header
        }
        let n = self.long.len();
        let bits = if n <= 1 {
            0
        } else {
            usize::BITS - (n - 1).leading_zeros()
        };
        let mode = ((first >> 1) as usize) & ((1usize << bits) - 1);
        Some(self.blocksize[*self.long.get(mode)? as usize])
    }
}

/// The modes' block flags, read backwards from the setup header's end (the framing bit,
/// then each mode's 41 bits: block flag, window type, transform type, mapping).
fn setup_modes(setup: &[u8]) -> Option<Vec<bool>> {
    // Bits from the end: Vorbis packs least significant bit first, so reading bytes
    // backwards and each byte from its top bit walks the stream backwards.
    let total = setup.len() * 8;
    let bit = |i: usize| -> u32 {
        let byte = setup[setup.len() - 1 - i / 8];
        ((byte >> (7 - i % 8)) & 1) as u32
    };
    let bits = |at: &mut usize, n: usize| -> Option<u32> {
        if *at + n > total {
            return None;
        }
        let mut v = 0;
        for _ in 0..n {
            v = v << 1 | bit(*at);
            *at += 1;
        }
        Some(v)
    };
    let mut at = 0;
    while bits(&mut at, 1)? == 0 {}
    let framing = at;
    // Count back over modes while they look like modes; the count field must match.
    let mut count = 0;
    let mut found = 0;
    loop {
        let mut p = at;
        let mapping = bits(&mut p, 8)?;
        let transform = bits(&mut p, 16)?;
        let window = bits(&mut p, 16)?;
        if mapping > 63 || transform != 0 || window != 0 {
            break;
        }
        p += 1;
        count += 1;
        at = p;
        let mut q = at;
        if bits(&mut q, 6).map(|c| c + 1) == Some(count) {
            found = count;
        }
        if count > 64 {
            break;
        }
    }
    if found == 0 {
        return None;
    }
    let mut at = framing;
    let mut long = vec![false; found as usize];
    for i in (0..found as usize).rev() {
        at += 40;
        long[i] = bits(&mut at, 1)? == 1;
    }
    Some(long)
}

/// The packets of complete Ogg pages at the start of `ogg`, and the bytes used.
fn pages(ogg: &[u8], partial: &mut Vec<u8>, out: &mut Vec<Vec<u8>>) -> usize {
    let mut pos = 0;
    while pos + 27 <= ogg.len() && &ogg[pos..pos + 4] == b"OggS" {
        let n = ogg[pos + 26] as usize;
        if pos + 27 + n > ogg.len() {
            break;
        }
        let lace = &ogg[pos + 27..pos + 27 + n];
        let body: usize = lace.iter().map(|&l| l as usize).sum();
        let mut data = pos + 27 + n;
        if data + body > ogg.len() {
            break;
        }
        for &l in lace {
            partial.extend_from_slice(&ogg[data..data + l as usize]);
            data += l as usize;
            if l < 255 {
                out.push(std::mem::take(partial));
            }
        }
        pos = data;
    }
    pos
}

/// A Vorbis encoder giving timed packets.
pub struct Vorbis {
    enc: Option<VorbisEncoder<Sink>>,
    sink: Sink,
    partial: Vec<u8>,
    headers: Vec<Vec<u8>>,
    modes: Option<Modes>,
    previous: Option<u32>,
    samples: u64,
    channels: usize,
    planes: Vec<Vec<f32>>,
}

impl Vorbis {
    pub fn new(rate: u32, channels: u8) -> Result<Vorbis, VideoError> {
        let sink = Sink::default();
        let e = |e: vorbis_rs::VorbisError| VideoError::encoder("Vorbis", e);
        let mut b = VorbisEncoderBuilder::new_with_serial(
            NonZeroU32::new(rate).ok_or_else(|| VideoError::Unsupported("0 Hz".into()))?,
            NonZeroU8::new(channels)
                .ok_or_else(|| VideoError::Unsupported("no channels".into()))?,
            sink.clone(),
            1,
        );
        b.bitrate_management_strategy(VorbisBitrateManagementStrategy::Vbr {
            target_bitrate: NonZeroU32::new(BITRATE).expect("non-zero"),
        });
        let enc = b.build().map_err(e)?;
        Ok(Vorbis {
            enc: Some(enc),
            sink,
            partial: Vec::new(),
            headers: Vec::new(),
            modes: None,
            previous: None,
            samples: 0,
            channels: channels as usize,
            planes: vec![Vec::new(); channels as usize],
        })
    }

    /// The three headers, Xiph-laced (WebM's CodecPrivate), once they are out.
    pub fn private(&self) -> Option<Vec<u8>> {
        (self.headers.len() == 3).then(|| xiph_lace(&self.headers))
    }

    /// Encode interleaved floats; packets ready go to `out`.
    pub fn encode(&mut self, interleaved: &[f32], out: &mut Vec<Packet>) -> Result<(), VideoError> {
        for p in &mut self.planes {
            p.clear();
        }
        for f in interleaved.chunks_exact(self.channels) {
            for (p, s) in self.planes.iter_mut().zip(f) {
                p.push(*s);
            }
        }
        if !self.planes[0].is_empty() {
            let enc = self.enc.as_mut().expect("open until finish");
            enc.encode_audio_block(&self.planes)
                .map_err(|e| VideoError::encoder("Vorbis", e))?;
        }
        self.take(out)
    }

    /// The end: what is left comes out.
    pub fn finish(&mut self, out: &mut Vec<Packet>) -> Result<(), VideoError> {
        if let Some(enc) = self.enc.take() {
            enc.finish().map_err(|e| VideoError::encoder("Vorbis", e))?;
        }
        self.take(out)
    }

    fn take(&mut self, out: &mut Vec<Packet>) -> Result<(), VideoError> {
        let mut packets = Vec::new();
        {
            let mut ogg = self.sink.0.borrow_mut();
            let used = pages(&ogg, &mut self.partial, &mut packets);
            ogg.drain(..used);
        }
        for p in packets {
            if self.headers.len() < 3 {
                self.headers.push(p);
                if self.headers.len() == 3 {
                    self.modes = Modes::read(&self.headers[0], &self.headers[2]);
                    if self.modes.is_none() {
                        return Err(VideoError::encoder("Vorbis", "an unreadable setup header"));
                    }
                }
                continue;
            }
            let modes = self.modes.as_ref().expect("read with the headers");
            let Some(size) = modes.size_of(&p) else {
                continue;
            };
            let start = self.samples;
            if let Some(prev) = self.previous {
                self.samples += (prev / 4 + size / 4) as u64;
            }
            self.previous = Some(size);
            out.push(Packet { start, data: p });
        }
        Ok(())
    }
}

/// Xiph lacing of packets (all but the last sized), as Matroska's CodecPrivate wants.
pub fn xiph_lace(packets: &[Vec<u8>]) -> Vec<u8> {
    let mut v = vec![(packets.len() - 1) as u8];
    for p in &packets[..packets.len() - 1] {
        let mut n = p.len();
        while n >= 255 {
            v.push(255);
            n -= 255;
        }
        v.push(n as u8);
    }
    for p in packets {
        v.extend_from_slice(p);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packets_are_timed_by_the_samples_before_them() {
        let mut v = Vorbis::new(48_000, 2).unwrap();
        let mut out = Vec::new();
        let second: Vec<f32> = (0..96_000)
            .map(|i| ((i % 100) as f32 / 100.0 - 0.5) * 0.2)
            .collect();
        for _ in 0..3 {
            v.encode(&second, &mut out).unwrap();
        }
        v.finish(&mut out).unwrap();
        let m = v.modes.clone().unwrap();
        assert_eq!(m.blocksize, [256, 2048]);
        assert_eq!(m.long.len(), 2, "libvorbis writes a short and a long mode");
        assert!(v.private().is_some());
        assert_eq!(out[0].start, 0);
        assert!(out.windows(2).all(|w| w[0].start <= w[1].start));
        // Three seconds in, three seconds (and at most a block more) out.
        let last = out.last().unwrap().start;
        assert!(
            (144_000 - 2_048..=144_000 + 2_048).contains(&last),
            "{last}"
        );
    }
}
