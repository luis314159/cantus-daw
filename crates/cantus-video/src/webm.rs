//! Our WebM writer (ADR 0088 §2): one AV1 video track and one Vorbis audio track, written
//! as they come.
//!
//! - Blocks are held until both tracks have passed their time and then written in time
//!   order, so audio and video interleave whatever each encoder's latency.
//! - A cluster starts at each video key frame (and at least every 5 s), and the `Cues` at
//!   the end point to them: a player seeks straight to a key frame.
//! - The `Segment`'s size and the `SeekHead` (in room reserved at the start) are written
//!   at the end. Times are in milliseconds (WebM's usual `TimestampScale`).

use std::collections::BTreeMap;
use std::io::{Seek, SeekFrom, Write};

/// An element: its ID, an 8-byte size, its body.
pub fn el(id: u32, body: &[u8]) -> Vec<u8> {
    let mut v = id_bytes(id);
    v.extend_from_slice(&size8(body.len() as u64));
    v.extend_from_slice(body);
    v
}

fn id_bytes(id: u32) -> Vec<u8> {
    let b = id.to_be_bytes();
    let skip = b.iter().position(|&x| x != 0).unwrap_or(3);
    b[skip..].to_vec()
}

fn size8(n: u64) -> [u8; 8] {
    let mut s = n.to_be_bytes();
    s[0] = 0x01;
    s
}

pub fn uint(id: u32, x: u64) -> Vec<u8> {
    let b = x.to_be_bytes();
    let skip = b.iter().position(|&x| x != 0).unwrap_or(7);
    el(id, &b[skip..])
}

pub fn float(id: u32, x: f64) -> Vec<u8> {
    el(id, &x.to_be_bytes())
}

pub fn string(id: u32, s: &str) -> Vec<u8> {
    el(id, s.as_bytes())
}

const SEGMENT: u32 = 0x1853_8067;
const SEEK_HEAD: u32 = 0x114D_9B74;
const INFO: u32 = 0x1549_A966;
const TRACKS: u32 = 0x1654_AE6B;
const CLUSTER: u32 = 0x1F43_B675;
const CUES: u32 = 0x1C53_BB6B;
const VOID: u32 = 0xEC;
/// Room kept for the `SeekHead` (three entries).
const SEEK_ROOM: usize = 160;
/// The longest cluster, in milliseconds (block times are 16-bit relative to it).
const CLUSTER_MS: u64 = 5_000;

/// The tracks' descriptions.
pub struct Tracks {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    /// `av1C`.
    pub video_private: Vec<u8>,
    pub rate: u32,
    pub channels: u16,
    /// The three Vorbis headers, Xiph-laced.
    pub audio_private: Vec<u8>,
    pub duration_ms: f64,
}

pub const VIDEO: u8 = 1;
pub const AUDIO: u8 = 2;

/// A WebM being written.
pub struct WebmWriter<W: Write + Seek> {
    out: W,
    segment_size_at: u64,
    segment_data: u64,
    seek_at: u64,
    info_at: u64,
    tracks_at: u64,
    pending: BTreeMap<(u64, u8, u64), (bool, Vec<u8>)>,
    order: u64,
    seen: [u64; 2],
    cluster: Option<(u64, u64, Vec<u8>)>,
    cues: Vec<(u64, u64)>,
    bytes: u64,
}

impl<W: Write + Seek> WebmWriter<W> {
    pub fn new(mut out: W, t: &Tracks) -> std::io::Result<WebmWriter<W>> {
        let ebml = el(
            0x1A45_DFA3,
            &[
                uint(0x4286, 1),
                uint(0x42F7, 1),
                uint(0x42F2, 4),
                uint(0x42F3, 8),
                string(0x4282, "webm"),
                uint(0x4287, 4),
                uint(0x4285, 2),
            ]
            .concat(),
        );
        out.write_all(&ebml)?;
        out.write_all(&id_bytes(SEGMENT))?;
        let segment_size_at = out.stream_position()?;
        out.write_all(&[0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF])?;
        let segment_data = out.stream_position()?;
        // Room for the SeekHead: a Void element of SEEK_ROOM bytes.
        out.write_all(&void(SEEK_ROOM))?;
        let info_at = out.stream_position()? - segment_data;
        let info = el(
            INFO,
            &[
                uint(0x2AD7B1, 1_000_000),
                float(0x4489, t.duration_ms),
                string(0x4D80, "Cantus"),
                string(0x5741, concat!("Cantus ", env!("CARGO_PKG_VERSION"))),
            ]
            .concat(),
        );
        out.write_all(&info)?;
        let tracks_at = out.stream_position()? - segment_data;
        let colour = el(
            0x55B0,
            &[
                uint(0x55B1, 1),
                uint(0x55B9, 1),
                uint(0x55BA, 1),
                uint(0x55BB, 1),
            ]
            .concat(),
        );
        let video = el(
            0xAE,
            &[
                uint(0xD7, VIDEO as u64),
                uint(0x73C5, VIDEO as u64),
                uint(0x83, 1),
                string(0x86, "V_AV1"),
                uint(0x9C, 0),
                el(0x63A2, &t.video_private),
                uint(0x23E383, 1_000_000_000 / t.fps as u64),
                el(
                    0xE0,
                    &[
                        uint(0xB0, t.width as u64),
                        uint(0xBA, t.height as u64),
                        colour,
                    ]
                    .concat(),
                ),
            ]
            .concat(),
        );
        let audio = el(
            0xAE,
            &[
                uint(0xD7, AUDIO as u64),
                uint(0x73C5, AUDIO as u64),
                uint(0x83, 2),
                string(0x86, "A_VORBIS"),
                el(0x63A2, &t.audio_private),
                el(
                    0xE1,
                    &[float(0xB5, t.rate as f64), uint(0x9F, t.channels as u64)].concat(),
                ),
            ]
            .concat(),
        );
        out.write_all(&el(TRACKS, &[video, audio].concat()))?;
        Ok(WebmWriter {
            out,
            segment_size_at,
            segment_data,
            seek_at: segment_data,
            info_at,
            tracks_at,
            pending: BTreeMap::new(),
            order: 0,
            seen: [0, 0],
            cluster: None,
            cues: Vec::new(),
            bytes: 0,
        })
    }

    /// Bytes of media written so far.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// A block of `track` at `ms`; written once both tracks have reached that time.
    pub fn block(&mut self, track: u8, ms: u64, key: bool, data: Vec<u8>) -> std::io::Result<()> {
        self.order += 1;
        self.pending.insert((ms, track, self.order), (key, data));
        let i = (track - 1) as usize;
        self.seen[i] = self.seen[i].max(ms);
        let ready = self.seen[0].min(self.seen[1]);
        self.flush_until(Some(ready))
    }

    fn flush_until(&mut self, until: Option<u64>) -> std::io::Result<()> {
        while let Some(entry) = self.pending.first_entry() {
            let (ms, track, _) = *entry.key();
            if until.is_some_and(|u| ms > u) {
                break;
            }
            let (key, data) = entry.remove();
            self.write_block(track, ms, key, &data)?;
        }
        Ok(())
    }

    fn write_block(&mut self, track: u8, ms: u64, key: bool, data: &[u8]) -> std::io::Result<()> {
        let new_cluster = match &self.cluster {
            None => true,
            Some((start, _, _)) => {
                (track == VIDEO && key) || ms >= start + CLUSTER_MS || ms < *start
            }
        };
        if new_cluster {
            self.close_cluster()?;
            let at = self.out.stream_position()? - self.segment_data;
            if track == VIDEO && key {
                self.cues.push((ms, at));
            }
            self.cluster = Some((ms, at, uint(0xE7, ms)));
        }
        let (start, _, body) = self.cluster.as_mut().expect("a cluster is open");
        let mut sb = vec![0x80 | track];
        sb.extend_from_slice(&((ms - *start) as i16).to_be_bytes());
        sb.push(if key { 0x80 } else { 0 });
        sb.extend_from_slice(data);
        body.extend_from_slice(&el(0xA3, &sb));
        self.bytes += data.len() as u64;
        Ok(())
    }

    fn close_cluster(&mut self) -> std::io::Result<()> {
        if let Some((_, _, body)) = self.cluster.take() {
            self.out.write_all(&el(CLUSTER, &body))?;
        }
        Ok(())
    }

    /// Everything written, the cues and the sizes in place; the output back.
    pub fn finish(mut self) -> std::io::Result<W> {
        self.flush_until(None)?;
        self.close_cluster()?;
        let cues_at = self.out.stream_position()? - self.segment_data;
        let points: Vec<u8> = self
            .cues
            .iter()
            .flat_map(|&(ms, at)| {
                el(
                    0xBB,
                    &[
                        uint(0xB3, ms),
                        el(0xB7, &[uint(0xF7, VIDEO as u64), uint(0xF1, at)].concat()),
                    ]
                    .concat(),
                )
            })
            .collect();
        self.out.write_all(&el(CUES, &points))?;
        let end = self.out.stream_position()?;
        self.out.seek(SeekFrom::Start(self.segment_size_at))?;
        self.out.write_all(&size8(end - self.segment_data))?;
        let seek = |id: u32, at: u64| {
            el(
                0x4DBB,
                &[el(0x53AB, &id_bytes(id)), uint(0x53AC, at)].concat(),
            )
        };
        let head = el(
            SEEK_HEAD,
            &[
                seek(INFO, self.info_at),
                seek(TRACKS, self.tracks_at),
                seek(CUES, cues_at),
            ]
            .concat(),
        );
        assert!(head.len() + 9 <= SEEK_ROOM, "the SeekHead fits its room");
        self.out.seek(SeekFrom::Start(self.seek_at))?;
        self.out.write_all(&head)?;
        self.out.write_all(&void(SEEK_ROOM - head.len()))?;
        self.out.seek(SeekFrom::Start(end))?;
        self.out.flush()?;
        Ok(self.out)
    }
}

/// A Void element exactly `n` (≥ 9) bytes long.
fn void(n: usize) -> Vec<u8> {
    let mut v = vec![VOID as u8];
    v.extend_from_slice(&size8((n - 9) as u64));
    v.resize(n, 0);
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn tracks() -> Tracks {
        Tracks {
            width: 64,
            height: 48,
            fps: 30,
            video_private: vec![0x81, 0, 0, 0],
            rate: 48_000,
            channels: 2,
            audio_private: vec![2, 1, 1, 7, 8, 9],
            duration_ms: 1_000.0,
        }
    }

    #[test]
    fn blocks_come_out_in_time_order_with_cues_at_key_frames() {
        let mut w = WebmWriter::new(Cursor::new(Vec::new()), &tracks()).unwrap();
        // Video runs ahead of audio: nothing is written until audio catches up.
        for n in 0..10u64 {
            w.block(VIDEO, n * 33, n % 5 == 0, vec![n as u8; 3])
                .unwrap();
        }
        for k in 0..20u64 {
            w.block(AUDIO, k * 21, true, vec![0xAA]).unwrap();
        }
        let file = w.finish().unwrap().into_inner();
        // The segment's size is the rest of the file.
        let seg = file
            .windows(4)
            .position(|x| x == [0x18, 0x53, 0x80, 0x67])
            .unwrap();
        let size =
            u64::from_be_bytes(file[seg + 4..seg + 12].try_into().unwrap()) & ((1 << 56) - 1);
        assert_eq!(size as usize, file.len() - seg - 12);
        // Two clusters (key frames at 0 and 165 ms), two cue points.
        let clusters = file
            .windows(4)
            .filter(|x| *x == [0x1F, 0x43, 0xB6, 0x75])
            .count();
        assert_eq!(clusters, 2);
        let cues = file
            .windows(4)
            .position(|x| x == [0x1C, 0x53, 0xBB, 0x6B])
            .unwrap();
        assert!(file[cues..].iter().filter(|&&b| b == 0xBB).count() >= 2);
        assert!(
            file.windows(4).any(|x| x == [0x11, 0x4D, 0x9B, 0x74]),
            "a SeekHead"
        );
    }
}
