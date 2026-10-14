//! What every Exemplar importer produces — zones, groups, instrument values and a log of
//! what could not be mapped — plus the pieces they share: note names, the loop and root a
//! WAV or AIFF file carries (`smpl`/`inst`, `INST`/`MARK`), and mapping a folder of files
//! by metadata, then by the note and dynamic in their names
//! (`research/instruments/sampler.md` §5.6).

use cantus_core::inst::sampler::{zf, Group, Zone, MAX_GROUPS, MAX_ZONES};
use std::path::{Path, PathBuf};

/// An instrument read from another format, ready to become one batch of commands.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Import {
    pub name: Option<String>,
    pub zones: Vec<Zone>,
    /// Groups by index.
    pub groups: Vec<(usize, Group)>,
    /// The instrument's own numbers (`amp.release`, `filter.cutoff`, …).
    pub values: Vec<(String, f32)>,
    /// What was not carried over, one line each ("unsupported opcode width (12 regions)").
    pub log: Vec<String>,
}

impl Import {
    /// Keep within the model's limits, logging what had to go.
    pub fn clamp(&mut self) {
        if self.zones.len() > MAX_ZONES {
            self.log.push(format!(
                "{} zones: only the first {MAX_ZONES} were kept",
                self.zones.len()
            ));
            self.zones.truncate(MAX_ZONES);
        }
        self.groups.retain(|(m, _)| *m < MAX_GROUPS);
    }
}

/// What a sample file says about itself.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meta {
    pub rate: u32,
    pub channels: u16,
    pub frames: u64,
    /// MIDI unity note and fine tune in cents (`smpl` / `inst` / AIFF `INST`).
    pub root: Option<u8>,
    pub fine_ct: f32,
    /// First sustain loop, inclusive frames, and whether it ping-pongs.
    pub sustain_loop: Option<(u32, u32, bool)>,
    /// Key and velocity range the file claims (`inst` / AIFF `INST`).
    pub keys: Option<(u8, u8)>,
    pub vels: Option<(u8, u8)>,
    pub gain_db: f32,
}

fn u16le(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn u32le(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}
fn u16be(b: &[u8], i: usize) -> u16 {
    u16::from_be_bytes([b[i], b[i + 1]])
}
fn u32be(b: &[u8], i: usize) -> u32 {
    u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

/// Read a WAV's or an AIFF's metadata from its bytes (no decoding).
pub fn meta_of_bytes(b: &[u8]) -> Option<Meta> {
    if b.len() >= 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WAVE" {
        return Some(wav_meta(b));
    }
    if b.len() >= 12 && &b[0..4] == b"FORM" && (&b[8..12] == b"AIFF" || &b[8..12] == b"AIFC") {
        return Some(aiff_meta(b));
    }
    None
}

/// A file's metadata; for formats without chunks (FLAC, Ogg) only what the name says.
///
/// Reads the chunks' headers and every chunk but the audio, which is skipped (round 26: a
/// piano of 641 FLAC files took 7.9 s to import because every file was read whole). The
/// audio chunk's header and first bytes go last, so the parsers see every other chunk and
/// the audio's true size.
pub fn meta_of(path: &Path) -> Option<Meta> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let mut head = [0u8; 12];
    f.read_exact(&mut head).ok()?;
    let riff = &head[0..4] == b"RIFF" && &head[8..12] == b"WAVE";
    let form = &head[0..4] == b"FORM" && (&head[8..12] == b"AIFF" || &head[8..12] == b"AIFC");
    if !riff && !form {
        return None;
    }
    let mut out = head.to_vec();
    let mut audio: Vec<u8> = Vec::new();
    let len = f.metadata().ok()?.len();
    let mut at = 12u64;
    while at + 8 <= len {
        let mut h = [0u8; 8];
        f.seek(SeekFrom::Start(at)).ok()?;
        f.read_exact(&mut h).ok()?;
        let size = if riff {
            u32::from_le_bytes([h[4], h[5], h[6], h[7]])
        } else {
            u32::from_be_bytes([h[4], h[5], h[6], h[7]])
        } as u64;
        let is_audio = &h[0..4] == b"data" || &h[0..4] == b"SSND";
        let keep = if is_audio { size.min(16) } else { size };
        let mut body = vec![0u8; keep.min(len - at - 8) as usize];
        f.read_exact(&mut body).ok()?;
        let dst = if is_audio { &mut audio } else { &mut out };
        dst.extend_from_slice(&h);
        dst.extend_from_slice(&body);
        if !is_audio && size % 2 == 1 {
            dst.push(0);
        }
        at += 8 + size + (size & 1);
    }
    out.extend_from_slice(&audio);
    meta_of_bytes(&out)
}

fn wav_meta(b: &[u8]) -> Meta {
    let mut m = Meta::default();
    let mut block = 1usize;
    let mut at = 12;
    while at + 8 <= b.len() {
        let id = &b[at..at + 4];
        let size = u32le(b, at + 4) as usize;
        let body = &b[at + 8..(at + 8 + size).min(b.len())];
        match id {
            b"fmt " if body.len() >= 16 => {
                m.channels = u16le(body, 2);
                m.rate = u32le(body, 4);
                block = u16le(body, 12).max(1) as usize;
            }
            b"data" => m.frames = (size / block) as u64,
            b"smpl" if body.len() >= 36 => {
                let unity = u32le(body, 12);
                if unity <= 127 {
                    m.root = Some(unity as u8);
                }
                // Pitch fraction: a fraction of a semitone above the unity note.
                m.fine_ct = (u32le(body, 16) as f64 / 4_294_967_296.0 * 100.0) as f32;
                let loops = u32le(body, 28) as usize;
                if loops > 0 && body.len() >= 36 + 24 {
                    let l = &body[36..60];
                    let kind = u32le(l, 4);
                    let (s, e) = (u32le(l, 8), u32le(l, 12));
                    if e > s {
                        m.sustain_loop = Some((s, e, kind == 1));
                    }
                }
            }
            b"inst" if body.len() >= 7 => {
                if m.root.is_none() && body[0] <= 127 {
                    m.root = Some(body[0]);
                }
                m.fine_ct = m.fine_ct.max(body[1] as i8 as f32);
                m.gain_db = body[2] as i8 as f32;
                m.keys = Some((body[3].min(127), body[4].min(127)));
                m.vels = Some((body[5].clamp(1, 127), body[6].clamp(1, 127)));
            }
            _ => {}
        }
        at += 8 + size + (size & 1);
    }
    m
}

fn aiff_meta(b: &[u8]) -> Meta {
    let mut m = Meta::default();
    let mut markers: Vec<(u16, u32)> = Vec::new();
    let mut inst: Option<&[u8]> = None;
    let mut at = 12;
    while at + 8 <= b.len() {
        let id = &b[at..at + 4];
        let size = u32be(b, at + 4) as usize;
        let body = &b[at + 8..(at + 8 + size).min(b.len())];
        match id {
            b"COMM" if body.len() >= 18 => {
                m.channels = u16be(body, 0);
                m.frames = u32be(body, 2) as u64;
                let e = &body[8..18];
                let exp = (((e[0] as i32) & 0x7F) << 8 | e[1] as i32) - 16_383;
                let mant = u64::from_be_bytes([e[2], e[3], e[4], e[5], e[6], e[7], e[8], e[9]]);
                m.rate = (mant as f64 * 2f64.powi(exp - 63)).round() as u32;
            }
            b"MARK" if body.len() >= 2 => {
                let n = u16be(body, 0) as usize;
                let mut i = 2;
                for _ in 0..n {
                    if i + 7 > body.len() {
                        break;
                    }
                    markers.push((u16be(body, i), u32be(body, i + 2)));
                    let len = body[i + 6] as usize;
                    i += 6 + 1 + len + ((len + 1) & 1);
                }
            }
            b"INST" if body.len() >= 20 => inst = Some(body),
            _ => {}
        }
        at += 8 + size + (size & 1);
    }
    if let Some(body) = inst {
        m.root = Some(body[0].min(127));
        m.fine_ct = body[1] as i8 as f32;
        m.keys = Some((body[2].min(127), body[3].min(127)));
        m.vels = Some((body[4].clamp(1, 127), body[5].clamp(1, 127)));
        m.gain_db = u16be(body, 6) as i16 as f32;
        let mode = u16be(body, 8);
        let (b0, b1) = (u16be(body, 10), u16be(body, 12));
        let pos = |id: u16| markers.iter().find(|(i, _)| *i == id).map(|(_, p)| *p);
        if mode != 0 {
            if let (Some(s), Some(e)) = (pos(b0), pos(b1)) {
                // AIFF's end marker is the first frame after the loop.
                if e > s + 1 {
                    m.sustain_loop = Some((s, e - 1, mode == 2));
                }
            }
        }
    }
    m
}

/// A note name (`c4`, `C#3`, `db-1`, `A#2`) or number (`60`) as a MIDI key; C4 = 60.
pub fn parse_note(s: &str) -> Option<u8> {
    let s = s.trim();
    if let Ok(n) = s.parse::<i32>() {
        return (0..=127).contains(&n).then_some(n as u8);
    }
    let b = s.as_bytes();
    let base = match b.first()?.to_ascii_lowercase() {
        b'c' => 0,
        b'd' => 2,
        b'e' => 4,
        b'f' => 5,
        b'g' => 7,
        b'a' => 9,
        b'b' => 11,
        _ => return None,
    };
    let mut i = 1;
    let mut acc = 0i32;
    while i < b.len() && matches!(b[i], b'#' | b'b' | b'B') {
        acc += if b[i] == b'#' { 1 } else { -1 };
        i += 1;
    }
    let octave: i32 = s.get(i..)?.parse().ok()?;
    let n = (octave + 1) * 12 + base + acc;
    (0..=127).contains(&n).then_some(n as u8)
}

/// The name as tokens: split at `_`, `-`, spaces, dots and brackets.
fn tokens(stem: &str) -> Vec<String> {
    stem.split(|c: char| "_- .()[]".contains(c))
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

/// A dynamic marking or velocity token's rank (higher is louder).
fn dynamic_rank(t: &str) -> Option<i32> {
    let l = t.to_ascii_lowercase();
    let named = ["ppp", "pp", "p", "mp", "mf", "f", "ff", "fff"];
    if let Some(i) = named.iter().position(|n| *n == l) {
        return Some(i as i32 * 16);
    }
    for pre in ["vel", "v", "l", "layer"] {
        if let Some(n) = l.strip_prefix(pre).and_then(|r| r.parse::<i32>().ok()) {
            return Some(n);
        }
    }
    None
}

fn rr_index(t: &str) -> Option<u8> {
    let l = t.to_ascii_lowercase();
    ["rr", "seq", "rnd"]
        .iter()
        .find_map(|p| l.strip_prefix(p).and_then(|r| r.parse::<u8>().ok()))
}

/// What a file name says: root (the last token that is a note name, else a bare number
/// 0…127), dynamic rank, round-robin index.
pub fn name_hints(stem: &str) -> (Option<u8>, Option<i32>, Option<u8>) {
    let toks = tokens(stem);
    let mut root = toks.iter().rev().find_map(|t| {
        let first = t.chars().next()?;
        (first.is_ascii_alphabetic() && t.len() <= 4)
            .then(|| parse_note(t))
            .flatten()
    });
    if root.is_none() {
        root = toks
            .iter()
            .rev()
            .find_map(|t| t.parse::<u8>().ok().filter(|n| *n <= 127 && t.len() <= 3));
    }
    let dynamic = toks.iter().find_map(|t| dynamic_rank(t));
    let rr = toks.iter().find_map(|t| rr_index(t));
    (root, dynamic, rr)
}

/// Map files to zones: roots from metadata, else from names; key ranges split halfway
/// between neighbouring roots (the lowest reaching down to 0, the highest up to 127);
/// velocity layers from dynamics in the names; round robins from `rr` tokens. Files with
/// no root at all go one per key upwards from C1 (a kit).
pub fn map_files(files: &[(PathBuf, Option<Meta>)]) -> Import {
    struct F {
        path: String,
        root: Option<u8>,
        dynamic: i32,
        rr: u8,
        meta: Meta,
    }
    let mut fs: Vec<F> = files
        .iter()
        .map(|(p, m)| {
            let stem = p
                .file_stem()
                .map(|s| s.to_string_lossy())
                .unwrap_or_default();
            let (root, dynamic, rr) = name_hints(&stem);
            let meta = m.clone().unwrap_or_default();
            F {
                path: p.to_string_lossy().into_owned(),
                root: meta.root.or(root),
                dynamic: dynamic.unwrap_or(0),
                rr: rr.unwrap_or(1),
                meta,
            }
        })
        .collect();
    fs.sort_by(|a, b| a.path.cmp(&b.path));
    let mut out = Import::default();
    let mut roots: Vec<u8> = fs.iter().filter_map(|f| f.root).collect();
    roots.sort_unstable();
    roots.dedup();
    let mut next_key = 36u8;
    for f in &fs {
        let mut z = Zone::new(Some(f.path.clone()), 0, 127, 60);
        match f.root {
            Some(r) => {
                let i = roots.iter().position(|x| *x == r).unwrap_or(0);
                let lo = if i == 0 {
                    0
                } else {
                    (roots[i - 1] as u16 + r as u16) / 2 + 1
                };
                let hi = if i + 1 == roots.len() {
                    127
                } else {
                    (r as u16 + roots[i + 1] as u16) / 2
                };
                z.set(zf::LO, lo as f32)
                    .set(zf::HI, hi as f32)
                    .set(zf::ROOT, r as f32);
            }
            None => {
                let k = next_key.min(127);
                next_key = next_key.saturating_add(1);
                z.set(zf::LO, k as f32)
                    .set(zf::HI, k as f32)
                    .set(zf::ROOT, k as f32);
                z.set(zf::KEYTRACK, 0.0).set(zf::MODE, 1.0);
            }
        }
        if let Some((lo, hi)) = f
            .meta
            .keys
            .filter(|(lo, hi)| lo <= hi && !(*lo == 0 && *hi == 127))
        {
            z.set(zf::LO, lo as f32).set(zf::HI, hi as f32);
        }
        z.set(zf::TUNE, -f.meta.fine_ct)
            .set(zf::GAIN, f.meta.gain_db);
        if let Some((s, e, alt)) = f.meta.sustain_loop {
            z.set(zf::MODE, 2.0)
                .set(zf::LS, s as f32)
                .set(zf::LE, e as f32);
            if alt {
                z.set(zf::DIR, 1.0);
            }
        }
        out.zones.push(z);
    }
    // Velocity layers and round robins among files sharing a root.
    for r in &roots {
        let idx: Vec<usize> = (0..fs.len()).filter(|i| fs[*i].root == Some(*r)).collect();
        let mut ranks: Vec<i32> = idx.iter().map(|i| fs[*i].dynamic).collect();
        ranks.sort_unstable();
        ranks.dedup();
        let layers = ranks.len();
        for &i in &idx {
            let li = ranks.iter().position(|x| *x == fs[i].dynamic).unwrap_or(0);
            if layers > 1 {
                let lo = 1 + (li * 127) / layers;
                let hi = ((li + 1) * 127) / layers;
                out.zones[i].set(zf::VLO, lo as f32).set(zf::VHI, hi as f32);
            }
            let same = idx
                .iter()
                .filter(|j| fs[**j].dynamic == fs[i].dynamic)
                .count();
            if same > 1 {
                let pos = idx
                    .iter()
                    .filter(|j| fs[**j].dynamic == fs[i].dynamic && fs[**j].rr < fs[i].rr)
                    .count()
                    + 1;
                out.zones[i]
                    .set(zf::SEQ, pos.min(16) as f32)
                    .set(zf::SEQLEN, same.min(16) as f32);
            }
        }
    }
    out
}
