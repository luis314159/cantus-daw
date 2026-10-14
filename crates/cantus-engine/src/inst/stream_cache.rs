//! The cache files streamed samples are read from (ADR 0050 §2): level 0 of a decoded
//! sample at the narrowest depth that holds its values exactly — `i16`, `i24` or `f32` —
//! behind a header that names the source it came from (its path, size, modification time
//! and a fingerprint of its first and last bytes) and a checksum of every block.
//!
//! Written beside its name, flushed to the disk and only then renamed into place, so a
//! crash never leaves a whole-looking file with missing data; a block whose checksum does
//! not match is never played (the reader marks the stream failed and deletes the file, and
//! the next load decodes the source again). The folder is kept under a size limit, least
//! recently used first, and entries whose source has gone or changed are removed
//! ([`sweep`]).

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 8] = b"CNTSTRM3";
/// Bumped whenever the layout or the decoders' output change (3: the whole source's hash —
/// second review #9).
const VERSION: u32 = 3;
/// The fixed part of the header; the source's path and the block checksums follow.
const FIXED: usize = 96;
/// Frames a checksum covers (the stream's block).
pub const BLOCK: usize = cantus_dsp::exemplar::stream::BLOCK;
/// How much of each end of a source its fingerprint reads.
const PRINT: u64 = 4_096;

/// How level 0 is stored.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Depth {
    I16,
    I24,
    F32,
}

impl Depth {
    pub fn bytes(self) -> usize {
        match self {
            Depth::I16 => 2,
            Depth::I24 => 3,
            Depth::F32 => 4,
        }
    }

    fn code(self) -> u32 {
        self.bytes() as u32
    }

    fn of(code: u32) -> Option<Depth> {
        Some(match code {
            2 => Depth::I16,
            3 => Depth::I24,
            4 => Depth::F32,
            _ => return None,
        })
    }
}

/// The narrowest depth that gives back exactly these bits: every value `k · 2⁻¹⁵` with
/// `k` an `i16` → `I16`; `k · 2⁻²³` with `k` in 24 bits → `I24`; otherwise `F32`. A
/// negative zero has no integer form, so it keeps `F32` (review 12).
pub fn exact_depth(samples: &[f32]) -> Depth {
    if samples.iter().any(|v| *v == 0.0 && v.is_sign_negative()) {
        return Depth::F32;
    }
    let fits = |scale: f32, lim: f32| {
        samples.iter().all(|v| {
            let k = v * scale;
            k == k.trunc() && (-lim..lim).contains(&k)
        })
    };
    if fits(32_768.0, 32_768.0) {
        Depth::I16
    } else if fits(8_388_608.0, 8_388_608.0) {
        Depth::I24
    } else {
        Depth::F32
    }
}

pub(crate) fn encode(samples: &[f32], depth: Depth, out: &mut Vec<u8>) {
    out.reserve(samples.len() * depth.bytes());
    for v in samples {
        match depth {
            Depth::I16 => out.extend_from_slice(&((v * 32_768.0) as i16).to_le_bytes()),
            Depth::I24 => out.extend_from_slice(&((v * 8_388_608.0) as i32).to_le_bytes()[..3]),
            Depth::F32 => out.extend_from_slice(&v.to_le_bytes()),
        }
    }
}

/// Bytes of `depth` back into `f32`, exactly the values written.
pub fn decode(bytes: &[u8], depth: Depth, out: &mut [f32]) {
    match depth {
        Depth::I16 => {
            for (o, b) in out.iter_mut().zip(bytes.chunks_exact(2)) {
                *o = i16::from_le_bytes([b[0], b[1]]) as f32 * (1.0 / 32_768.0);
            }
        }
        Depth::I24 => {
            for (o, b) in out.iter_mut().zip(bytes.chunks_exact(3)) {
                let v = i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8;
                *o = v as f32 * (1.0 / 8_388_608.0);
            }
        }
        Depth::F32 => {
            for (o, b) in out.iter_mut().zip(bytes.chunks_exact(4)) {
                *o = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
            }
        }
    }
}

/// A 64-bit checksum of bytes (8 at a time, then the rest, then the length).
pub fn checksum(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0x243F_6A88_85A3_08D3;
    let mut words = bytes.chunks_exact(8);
    for w in &mut words {
        let v = u64::from_le_bytes([w[0], w[1], w[2], w[3], w[4], w[5], w[6], w[7]]);
        h = (h.rotate_left(5) ^ v).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
    for b in words.remainder() {
        h = (h.rotate_left(5) ^ *b as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
    (h ^ bytes.len() as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
}

/// What a source file is, for the cache: its size, its modification time (ns from 1970,
/// negative before it — review 9) and a fingerprint of its first and last 4 KiB (review 7).
/// A file replaced with another of the same size and time is told apart when its ends
/// differ; one changed only in its middle is not — reading it all at every open would undo
/// streaming — so the whole file's hash, stored when the cache file is written, is checked
/// beside the reader once it opens ([`source_hash`]; second review #9).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Identity {
    pub size: u64,
    pub mtime: i64,
    pub print: u64,
}

/// The identity of `source` now; `None` only when it cannot be read.
pub fn identity(source: &Path) -> Option<Identity> {
    let mut f = File::open(source).ok()?;
    let m = f.metadata().ok()?;
    let mtime = match m.modified() {
        Ok(t) => match t.duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => d.as_nanos().min(i64::MAX as u128) as i64,
            Err(e) => -(e.duration().as_nanos().min(i64::MAX as u128) as i64),
        },
        Err(_) => 0,
    };
    let size = m.len();
    let half = PRINT.min(size) as usize;
    let mut buf = vec![0u8; half * 2];
    f.read_exact(&mut buf[..half]).ok()?;
    f.seek(SeekFrom::Start(size - half as u64)).ok()?;
    f.read_exact(&mut buf[half..]).ok()?;
    Some(Identity {
        size,
        mtime,
        print: checksum(&buf),
    })
}

/// A hash of the whole of `source`'s bytes (1 MiB at a time, `pause` after each: a check
/// beside the reader takes its time). Never on the audio thread.
pub fn source_hash(source: &Path, pause: std::time::Duration) -> std::io::Result<u64> {
    let mut f = File::open(source)?;
    let mut buf = vec![0u8; 1 << 20];
    let mut h: u64 = 0x1357_9BDF_2468_ACE0;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            return Ok(h);
        }
        h = (h.rotate_left(7) ^ checksum(&buf[..n])).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        if !pause.is_zero() {
            std::thread::sleep(pause);
        }
    }
}

/// The cache file's name for a source as it is.
pub fn path_for(dir: &Path, source: &Path, id: &Identity) -> PathBuf {
    let mut key = source.to_string_lossy().into_owned().into_bytes();
    key.extend_from_slice(&id.size.to_le_bytes());
    key.extend_from_slice(&id.mtime.to_le_bytes());
    key.extend_from_slice(&id.print.to_le_bytes());
    key.extend_from_slice(&VERSION.to_le_bytes());
    dir.join(format!("{:016x}.cstream", checksum(&key)))
}

/// An open cache file's facts.
#[derive(Clone, Debug, PartialEq)]
pub struct CacheFile {
    pub path: PathBuf,
    pub channels: usize,
    pub rate: u32,
    pub frames: usize,
    pub depth: Depth,
    /// The whole source's hash when it was written ([`source_hash`]; 0: not known).
    pub whole: u64,
    /// Where the samples start.
    data: u64,
    /// One checksum per block of level 0.
    sums: Vec<u64>,
}

/// Why a read from a cache file failed.
#[derive(Debug)]
pub enum ReadError {
    Io(std::io::Error),
    /// The data is not what was written (a crash, a bad disk): never played.
    Damaged,
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadError::Io(e) => write!(f, "{e}"),
            ReadError::Damaged => f.write_str("the cached samples are damaged"),
        }
    }
}

impl CacheFile {
    /// Blocks `first..first + n` of level 0 into `out` (interleaved `f32`, as many frames
    /// as they hold), every block's checksum checked. Returns the frames read.
    pub fn read_blocks(
        &self,
        f: &mut File,
        first: usize,
        n: usize,
        out: &mut [f32],
    ) -> Result<usize, ReadError> {
        let ch = self.channels;
        let bpf = self.depth.bytes() * ch;
        let from = first * BLOCK;
        let to = ((first + n) * BLOCK).min(self.frames);
        if from >= to {
            return Ok(0);
        }
        if failing(&self.path, first, n) {
            return Err(ReadError::Io(std::io::Error::other(
                "an injected read error",
            )));
        }
        let mut buf = vec![0u8; (to - from) * bpf];
        f.seek(SeekFrom::Start(self.data + (from * bpf) as u64))
            .map_err(ReadError::Io)?;
        f.read_exact(&mut buf).map_err(ReadError::Io)?;
        for (i, chunk) in buf.chunks(BLOCK * bpf).enumerate() {
            if self.sums.get(first + i) != Some(&checksum(chunk)) {
                return Err(ReadError::Damaged);
            }
        }
        decode(&buf, self.depth, &mut out[..(to - from) * ch]);
        Ok(to - from)
    }
}

/// A cache file's block that fails to read for good (tests of a bad sector: fourth review
/// #5), or none.
static READ_FAULT: std::sync::Mutex<Option<(PathBuf, usize)>> = std::sync::Mutex::new(None);

/// Make reads of `block` of the cache file at `path` fail with an I/O error from now on
/// (`None`: no more).
pub fn set_read_fault(fault: Option<(PathBuf, usize)>) {
    if let Ok(mut f) = READ_FAULT.lock() {
        *f = fault;
    }
}

fn failing(path: &Path, first: usize, n: usize) -> bool {
    READ_FAULT.lock().is_ok_and(|f| {
        f.as_ref()
            .is_some_and(|(p, b)| p == path && (first..first + n).contains(b))
    })
}

fn u32_at(h: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([h[i], h[i + 1], h[i + 2], h[i + 3]])
}

fn u64_at(h: &[u8], i: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&h[i..i + 8]);
    u64::from_le_bytes(b)
}

/// Files written whose data a thread of its own makes sure reaches the disk (review 10):
/// flushing each before its rename doubled a first load here (1 GB: 9.1 s → 20.7 s), so the
/// flush follows at once beside the loads instead. A crash before it leaves at worst blocks
/// whose checksums fail — never played, decoded again.
fn flush_later(path: PathBuf) {
    use std::sync::mpsc::{channel, Sender};
    use std::sync::{Mutex, OnceLock};
    static FLUSHER: OnceLock<Mutex<Sender<PathBuf>>> = OnceLock::new();
    let tx = FLUSHER.get_or_init(|| {
        let (tx, rx) = channel::<PathBuf>();
        let _ = std::thread::Builder::new()
            .name("cantus-cache-flush".into())
            .spawn(move || {
                while let Ok(p) = rx.recv() {
                    if let Ok(f) = File::options().write(true).open(&p) {
                        let _ = f.sync_all();
                    }
                    #[cfg(unix)]
                    if let Some(d) = p.parent().and_then(|d| File::open(d).ok()) {
                        let _ = d.sync_all();
                    }
                }
            });
        Mutex::new(tx)
    });
    if let Ok(tx) = tx.lock() {
        let _ = tx.send(path);
    }
}

/// Write level 0 of a decoded sample (`samples` interleaved) for `source`, whose identity
/// was `id` when its decoding began (review 7: never the identity read after). Written
/// beside its name and renamed into place, then flushed to the disk beside the loads
/// ([`flush_later`]); nothing is left behind on failure (review 8).
pub fn write(
    dir: &Path,
    source: &Path,
    id: &Identity,
    channels: usize,
    rate: u32,
    samples: &[f32],
) -> std::io::Result<CacheFile> {
    std::fs::create_dir_all(dir)?;
    let path = path_for(dir, source, id);
    let depth = exact_depth(samples);
    let frames = samples.len() / channels.max(1);
    let src = source.to_string_lossy().into_owned().into_bytes();
    let mut data = Vec::with_capacity(samples.len() * depth.bytes());
    encode(samples, depth, &mut data);
    let sums: Vec<u64> = data
        .chunks(BLOCK * depth.bytes() * channels)
        .map(checksum)
        .collect();
    let data_at = (FIXED + src.len() + sums.len() * 8) as u64;
    let mut head = Vec::with_capacity(data_at as usize);
    head.extend_from_slice(MAGIC);
    head.extend_from_slice(&VERSION.to_le_bytes());
    head.extend_from_slice(&id.size.to_le_bytes());
    head.extend_from_slice(&id.mtime.to_le_bytes());
    head.extend_from_slice(&id.print.to_le_bytes());
    head.extend_from_slice(&(channels as u32).to_le_bytes());
    head.extend_from_slice(&rate.to_le_bytes());
    head.extend_from_slice(&(frames as u64).to_le_bytes());
    head.extend_from_slice(&depth.code().to_le_bytes());
    head.extend_from_slice(&(src.len() as u32).to_le_bytes());
    head.extend_from_slice(&(sums.len() as u32).to_le_bytes());
    head.extend_from_slice(&data_at.to_le_bytes());
    // 72..80: the header's checksum, below. 80..88: the whole source's hash.
    let whole = source_hash(source, std::time::Duration::ZERO).unwrap_or(0);
    head.resize(80, 0);
    head.extend_from_slice(&whole.to_le_bytes());
    head.resize(FIXED, 0);
    head.extend_from_slice(&src);
    for s in &sums {
        head.extend_from_slice(&s.to_le_bytes());
    }
    let hsum = checksum(&head);
    head[72..80].copy_from_slice(&hsum.to_le_bytes());
    let part = path.with_extension(format!("part{}", std::process::id()));
    let written = (|| -> std::io::Result<()> {
        let mut f = File::create(&part)?;
        f.write_all(&head)?;
        f.write_all(&data)
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    if let Err(e) = std::fs::rename(&part, &path) {
        // Another loader wrote the same file meanwhile: keep theirs.
        let _ = std::fs::remove_file(&part);
        if !path.is_file() {
            return Err(e);
        }
    }
    flush_later(path.clone());
    Ok(CacheFile {
        path,
        channels,
        rate,
        frames,
        depth,
        whole,
        data: data_at,
        sums,
    })
}

/// The header of a cache file (`None` when it is not one of ours, or damaged), and the
/// source it names.
fn header(path: &Path) -> Option<(CacheFile, PathBuf, Identity)> {
    let mut f = File::open(path).ok()?;
    let mut h = vec![0u8; FIXED];
    f.read_exact(&mut h).ok()?;
    if &h[..8] != MAGIC || u32_at(&h, 8) != VERSION {
        return None;
    }
    let id = Identity {
        size: u64_at(&h, 12),
        mtime: u64_at(&h, 20) as i64,
        print: u64_at(&h, 28),
    };
    let channels = u32_at(&h, 36) as usize;
    let rate = u32_at(&h, 40);
    let frames = u64_at(&h, 44) as usize;
    let depth = Depth::of(u32_at(&h, 52))?;
    let plen = u32_at(&h, 56) as usize;
    let blocks = u32_at(&h, 60) as usize;
    let data = u64_at(&h, 64);
    if !(1..=2).contains(&channels)
        || blocks != frames.div_ceil(BLOCK)
        || data != (FIXED + plen + blocks * 8) as u64
        || plen > 32_768
    {
        return None;
    }
    let mut rest = vec![0u8; plen + blocks * 8];
    f.read_exact(&mut rest).ok()?;
    let stored = u64_at(&h, 72);
    h[72..80].fill(0);
    h.extend_from_slice(&rest);
    if checksum(&h) != stored {
        return None;
    }
    let want = data + (frames * channels * depth.bytes()) as u64;
    if f.metadata().ok()?.len() != want {
        return None;
    }
    let source = PathBuf::from(String::from_utf8_lossy(&rest[..plen]).into_owned());
    let sums = rest[plen..].chunks_exact(8).map(|c| u64_at(c, 0)).collect();
    Some((
        CacheFile {
            path: path.to_path_buf(),
            channels,
            rate,
            frames,
            depth,
            whole: u64_at(&h, 80),
            data,
            sums,
        },
        source,
        id,
    ))
}

/// The cache file for `source` as it is now (`id`), when one was written for it — touched,
/// so the size limit removes the least recently used first.
pub fn open(dir: &Path, source: &Path, id: &Identity) -> Option<CacheFile> {
    let path = path_for(dir, source, id);
    let (c, src, cid) = header(&path)?;
    if cid != *id || src != source {
        return None;
    }
    if let Ok(f) = File::options().write(true).open(&path) {
        let _ = f.set_modified(std::time::SystemTime::now());
    }
    Some(c)
}

/// What a sweep did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Swept {
    pub removed: usize,
    /// Bytes the folder's cache files hold afterwards.
    pub bytes: u64,
}

/// Bytes the folder's cache files hold.
pub fn size_of(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().extension().is_some_and(|x| x == "cstream"))
                .filter_map(|e| e.metadata().ok())
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0)
}

/// Open a cache file to stream from, holding a shared lock on it while it is open: no
/// sweep — this process's or another Cantus's — deletes a file a stream reads (second
/// review #6). `None` when it is not there, or a sweep is deleting it now.
pub fn open_locked(path: &Path) -> Option<File> {
    let f = File::open(path).ok()?;
    f.try_lock_shared().ok()?;
    Some(f)
}

/// Whether a source an entry names is gone for good (second review #7): it changed, or it
/// is not there while the folder it was in is — a drive unplugged or a share offline takes
/// the folder away too, and an empty folder may be a mount point with nothing mounted.
/// `removed` are folders known to be deleted (a pack removed): what was under them is gone.
fn gone(source: &Path, id: &Identity, removed: &[PathBuf]) -> bool {
    if removed.iter().any(|r| source.starts_with(r)) {
        return true;
    }
    if let Some(now) = identity(source) {
        return now != *id;
    }
    let absent = matches!(
        std::fs::metadata(source),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound
    );
    absent
        && source
            .parent()
            .and_then(|d| std::fs::read_dir(d).ok())
            .is_some_and(|mut rd| rd.next().is_some())
}

/// Delete a cache file nothing streams (second review #6): not `in_use` here, no shared
/// lock on it anywhere (an open stream's, in any process), not touched within the last
/// minute nor since the sweep began (a load that just opened it). A file already gone
/// counts as deleted (another sweep took it — review #8), and so does one that cannot be
/// opened any more (being deleted: Windows lists a file until its last handle closes).
fn delete_unused(p: &Path, in_use: &dyn Fn(&Path) -> bool, began: std::time::SystemTime) -> bool {
    use std::io::ErrorKind::NotFound;
    if in_use(p) {
        return false;
    }
    let f = match File::open(p) {
        Ok(f) => f,
        Err(e) => return e.kind() == NotFound || leaving(p),
    };
    if f.try_lock().is_err() {
        return false;
    }
    let young = f
        .metadata()
        .and_then(|m| m.modified())
        .is_ok_and(|t| t + std::time::Duration::from_secs(60) > began);
    if young {
        return false;
    }
    match std::fs::remove_file(p) {
        Ok(()) => true,
        Err(e) => e.kind() == NotFound,
    }
}

/// A file listed but on its way out (Windows: deleted, a handle still open somewhere — an
/// indexer, an antivirus): it holds no bytes the limit should count.
fn leaving(p: &Path) -> bool {
    std::fs::metadata(p).is_err()
}

/// Keep the folder tidy (off the audio thread): stray `.part` files older than ten minutes
/// go (review 20), and so do files that are not ours or are damaged, entries whose source
/// is gone for good or changed (a pack removed or reinstalled — review 11; never one whose
/// drive is only away — second review #7), and then the least recently used until the
/// folder holds at most `limit` bytes. A file a stream reads — `in_use` here, or locked by
/// another Cantus — or one touched within the last minute is never removed (#6). One sweep
/// at a time over a folder, across processes (#8).
pub fn sweep(
    dir: &Path,
    limit: u64,
    in_use: &dyn Fn(&Path) -> bool,
    removed_roots: &[PathBuf],
) -> Swept {
    let Ok(lock) = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join(".sweep.lock"))
    else {
        return Swept::default();
    };
    // Another sweep (here or in another Cantus) finishes first; this one then lists anew.
    if lock.lock().is_err() {
        return Swept::default();
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Swept::default();
    };
    // Removed packs' folders, kept beside the cache until no entry under them is left (an
    // entry a stream held while its pack was removed goes at a later sweep — third review
    // #2); a folder there again (the pack reinstalled) is not removed any more.
    let mut roots = removed_roots_of(dir);
    for r in removed_roots {
        if !roots.contains(r) {
            roots.push(r.clone());
        }
    }
    roots.retain(|r| !r.exists());
    let removed_roots = &roots[..];
    let began = std::time::SystemTime::now();
    let mut kept: Vec<(std::time::SystemTime, u64, PathBuf)> = Vec::new();
    // In use, or just written: counted, never removed.
    let mut protected: Vec<PathBuf> = Vec::new();
    let mut removed = 0;
    for e in rd.flatten() {
        let p = e.path();
        let Ok(m) = e.metadata() else { continue };
        let ext = p.extension().map(|x| x.to_string_lossy().into_owned());
        let age = m
            .modified()
            .ok()
            .and_then(|t| began.duration_since(t).ok())
            .unwrap_or_default();
        match ext.as_deref() {
            Some(x) if x.starts_with("part") => {
                if age > std::time::Duration::from_secs(600) && std::fs::remove_file(&p).is_ok() {
                    removed += 1;
                }
            }
            // A file this young may be one a stream is about to switch to.
            Some("cstream") if !in_use(&p) && age > std::time::Duration::from_secs(60) => {
                let stale = match header(&p) {
                    Some((_, src, id)) => gone(&src, &id, removed_roots),
                    None => true,
                };
                if !stale {
                    kept.push((m.modified().unwrap_or(began), m.len(), p));
                } else if delete_unused(&p, in_use, began) {
                    removed += 1;
                } else {
                    protected.push(p);
                }
            }
            Some("cstream") => protected.push(p),
            _ => {}
        }
    }
    kept.sort_by_key(|k| k.0);
    let mut bytes: u64 = kept.iter().map(|k| k.1).sum::<u64>()
        + protected
            .iter()
            .filter_map(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .sum::<u64>();
    for (_, len, p) in &kept {
        if bytes <= limit {
            break;
        }
        if delete_unused(p, in_use, began) {
            bytes -= len;
            removed += 1;
        }
    }
    let left: Vec<&PathBuf> = removed_roots
        .iter()
        .filter(|r| {
            protected
                .iter()
                .filter_map(|p| header(p))
                .any(|(_, src, _)| src.starts_with(r))
        })
        .collect();
    let list = dir.join(REMOVED_ROOTS);
    if left.is_empty() {
        let _ = std::fs::remove_file(&list);
    } else {
        let text: String = left.iter().map(|r| format!("{}\n", r.display())).collect();
        let _ = std::fs::write(&list, text);
    }
    drop(lock);
    Swept { removed, bytes }
}

/// The file beside the cache listing removed packs' folders still to sweep.
const REMOVED_ROOTS: &str = "removed-roots.txt";

fn removed_roots_of(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_to_string(dir.join(REMOVED_ROOTS))
        .map(|t| {
            t.lines()
                .filter(|l| !l.trim().is_empty())
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "stream_cache_tests.rs"]
mod tests;
