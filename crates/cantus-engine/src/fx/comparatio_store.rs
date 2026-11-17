//! Comparatio's reference tracks off the audio thread (ADR 0146, provisional). Owned by the
//! wave-C builder.
//!
//! A reference is a file the project names by path (`EffectSpec::ir`, as Aula's response
//! and Arca's cabinet are). [`prepare`] runs where every snapshot is compiled
//! (`EngineHandle::compile`, live and offline):
//!
//! - **Live**, a reference not yet read is decoded and resampled to the engine's rate on a
//!   thread of its own (the decoders and the windowed sinc the audio clips use,
//!   `samples::decode_audio`), split into its two channels, and its whole file analysed
//!   (`cantus_dsp::comparatio::analyse_whole`: the integrated loudness for the whole-file
//!   match, and the readings, histogram and tonal curve the panel shows before a note has
//!   played). Until it is in, the snapshot names no reference — the unit plays the mix —
//!   and its arrival asks for a new compile (`SampleCache::poll_loaded`), whose snapshot
//!   names it; the unit then asks for it (`MemoryNeed::Comparatio`) and [`build`] hands it
//!   over. Nothing waits on the interface's thread.
//! - **Offline** (tests, a manual engine) the file is read where `prepare` is called.
//! - **A render never hears a reference** (`render`): every Comparatio in the snapshot is
//!   set to the mix and its reference taken away, so an export, a stem, a freeze or a
//!   bounce holds the mix only — whatever the panel was set to.
//!
//! **The list** (phase B): a Comparatio holds several references (`EffectSpec::refs`); the
//! one compared is read as above, and every other one in the list is read once on the same
//! thread for its **summary** ([`Summary`]: length, loudness, readings, tonal curves, an
//! overview, the file's size and hash), which is kept after its audio is let go — each
//! reference keeps its own analysis, and the panel shows it before it is chosen.
//!
//! **A file that changed or went missing is said so**: a reference is read with its
//! fingerprint ([`fingerprint`]: the size and a hash of the file's head, tail and places
//! between), which the panel compares with the one the project kept; a missing file is a
//! failure with its path.
//!
//! **Why a pool**: a unit lets a reference go on the audio thread, where nothing may be
//! freed. Every reference a unit holds has a sibling here, so letting one go is an atomic
//! decrement; a reference no unit holds (its count is the pool's alone) is freed here, on
//! the next prepare, once more than [`SPARE`] are kept.

use cantus_core::{EffectKind, Snapshot};
use cantus_dsp::comparatio::tonal::{Curve, Tonal};
use cantus_dsp::comparatio::{analyse_whole, tonal::Resolution, WholeAnalysis};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// The longest reference read, seconds (longer files are cut there).
pub const MAX_SECONDS: usize = 20 * 60;
/// References kept once no unit holds them (undo, a second instance, the list swapped back).
const SPARE: usize = 3;

/// A reference track at one engine rate, both channels whole.
#[derive(Debug)]
pub struct RefTrack {
    /// The project's path's key (`aula_store::file_key`).
    pub key: u64,
    /// A new number for every file read: the DSP starts a new reference from silence.
    pub id: u64,
    pub rate: u32,
    pub left: Arc<[f32]>,
    pub right: Arc<[f32]>,
    /// The whole file's integrated loudness, LUFS, as `f64` bits (NaN until analysed, or
    /// for a file too quiet to have one). The audio thread reads it; the analysis writes it.
    whole_lufs: AtomicU64,
    /// The whole file's analysis, for the panel (the interface's thread only).
    analysis: Mutex<Option<Arc<WholeAnalysis>>>,
    /// The file's fingerprint when it was read ([`fingerprint`]; 0 and empty: made in
    /// memory).
    pub bytes: u64,
    pub hash: String,
}

/// What the panel shows of a reference, kept after its audio is let go (one per path).
#[derive(Debug)]
pub struct Summary {
    pub seconds: f64,
    /// The whole file's integrated loudness, LUFS.
    pub lufs: Option<f64>,
    pub analysis: Arc<WholeAnalysis>,
    /// Its long-term tonal curves of the middle, `(L + R) / 2`, in thirds and sixths of an
    /// octave — what the slot's analyser tap carries of your mix, so like meets like.
    pub mid: [Curve; 2],
    /// Its level over its length: 240 peaks, 0 … 1 of the loudest.
    pub overview: Vec<f32>,
    /// The file's fingerprint when it was read.
    pub bytes: u64,
    pub hash: String,
}

/// The tonal curve of the middle of `left`/`right` at `resolution` (allocates).
pub fn mid_curve(left: &[f32], right: &[f32], rate: f32, resolution: Resolution) -> Curve {
    let mut t = Tonal::new(rate, resolution);
    let mut mid = vec![0f32; 4_096];
    for (l, r) in left.chunks(4_096).zip(right.chunks(4_096)) {
        let m = &mut mid[..l.len()];
        for ((m, a), b) in m.iter_mut().zip(l).zip(r) {
            *m = 0.5 * (a + b);
        }
        t.push(m, m);
    }
    t.curve()
}

/// 240 peaks over the length, scaled to the loudest (allocates).
pub fn overview(left: &[f32], right: &[f32]) -> Vec<f32> {
    let n = left.len().min(right.len());
    let per = (n / 240).max(1);
    let mut out: Vec<f32> = left[..n]
        .chunks(per)
        .zip(right[..n].chunks(per))
        .take(240)
        .map(|(l, r)| l.iter().chain(r).fold(0f32, |m, v| m.max(v.abs())))
        .collect();
    let top = out.iter().fold(1e-9f32, |m, v| m.max(*v));
    for v in &mut out {
        *v /= top;
    }
    out
}

/// A file's fingerprint: its size in bytes and a 64-bit FNV-1a hash (hexadecimal) of its
/// size, its first and last 64 KiB and 62 blocks of 4 KiB spread evenly between — enough to
/// tell a re-exported master from the one loaded (every sample moves), read in a few
/// milliseconds whatever the file's length. Blocks on the disk: never the audio thread.
pub fn fingerprint(at: &Path) -> std::io::Result<(u64, String)> {
    use std::io::{Read, Seek, SeekFrom};
    const EDGE: u64 = 64 * 1024;
    let mut f = std::fs::File::open(at)?;
    let len = f.metadata()?.len();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    eat(&len.to_le_bytes());
    let mut buf = vec![0u8; EDGE as usize];
    let mut take = |from: u64, n: u64| -> std::io::Result<()> {
        f.seek(SeekFrom::Start(from))?;
        let n = n.min(EDGE).min(len.saturating_sub(from)) as usize;
        f.read_exact(&mut buf[..n])?;
        eat(&buf[..n]);
        Ok(())
    };
    take(0, len.min(EDGE))?;
    if len > 2 * EDGE {
        for i in 1..=62u64 {
            take(len / 63 * i, 4_096)?;
        }
        take(len - EDGE, EDGE)?;
    } else if len > EDGE {
        take(EDGE, len - EDGE)?;
    }
    Ok((len, format!("{h:016x}")))
}

impl RefTrack {
    /// Two channels at `rate` (a mono file is given twice).
    pub fn new(key: u64, rate: u32, left: Vec<f32>, right: Vec<f32>) -> RefTrack {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        RefTrack {
            key,
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            rate,
            left: left.into(),
            right: right.into(),
            whole_lufs: AtomicU64::new(f64::NAN.to_bits()),
            analysis: Mutex::new(None),
            bytes: 0,
            hash: String::new(),
        }
    }

    pub fn frames(&self) -> usize {
        self.left.len().min(self.right.len())
    }

    pub fn seconds(&self) -> f64 {
        self.frames() as f64 / f64::from(self.rate.max(1))
    }

    /// The whole file's integrated loudness, once analysed. Lock-free: the audio thread's.
    pub fn whole_lufs(&self) -> Option<f64> {
        let v = f64::from_bits(self.whole_lufs.load(Ordering::Relaxed));
        v.is_finite().then_some(v)
    }

    /// The whole file's analysis, once made. For the interface's thread.
    pub fn analysis(&self) -> Option<Arc<WholeAnalysis>> {
        self.analysis.lock().ok()?.clone()
    }

    /// Analyse the whole file (allocates, takes a while: never the audio thread), and keep
    /// its [`Summary`] in the store under its key.
    pub fn analyse(&self) {
        let rate = self.rate as f32;
        let a = Arc::new(analyse_whole(
            &self.left,
            &self.right,
            rate,
            Resolution::Sixth,
        ));
        let lufs = a.readings.integrated.unwrap_or(f64::NAN);
        if let Ok(mut s) = self.analysis.lock() {
            *s = Some(a.clone());
        }
        self.whole_lufs.store(lufs.to_bits(), Ordering::Relaxed);
        let summary = Summary {
            seconds: self.seconds(),
            lufs: a.readings.integrated,
            analysis: a,
            mid: [
                mid_curve(&self.left, &self.right, rate, Resolution::Third),
                mid_curve(&self.left, &self.right, rate, Resolution::Sixth),
            ],
            overview: overview(&self.left, &self.right),
            bytes: self.bytes,
            hash: self.hash.clone(),
        };
        let mut s = store().lock().unwrap_or_else(|e| e.into_inner());
        s.summaries.retain(|(k, _)| *k != self.key);
        s.summaries.push((self.key, Arc::new(summary)));
    }
}

/// What became of a reference the project names.
#[derive(Clone, Debug, PartialEq)]
pub enum RefStatus {
    Loading,
    /// Read: its length, seconds, and whether its whole file is analysed.
    Loaded {
        seconds: f64,
        analysed: bool,
    },
    Failed(String),
    /// Never asked for.
    Unknown,
}

enum Entry {
    Loading,
    Ready(Arc<RefTrack>),
    Failed(String),
}

#[derive(Default)]
struct Store {
    /// By (path key, rate): where it was read from and what came of it, newest last.
    tracks: Vec<((u64, u32), PathBuf, Entry)>,
    /// Each reference's summary by its path's key, kept after its audio is let go.
    summaries: Vec<(u64, Arc<Summary>)>,
}

fn store() -> &'static Mutex<Store> {
    static S: OnceLock<Mutex<Store>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Store::default()))
}

/// A reference arrived (or failed) on its thread since the last [`take_arrived`].
static ARRIVED: AtomicBool = AtomicBool::new(false);

/// Whether a reference arrived since the last call: the loader's poll asks for a compile.
pub fn take_arrived() -> bool {
    ARRIVED.swap(false, Ordering::Relaxed)
}

/// Read a file as a reference at `rate`: decoded, resampled, its channels split (a mono file
/// on both, more than two: the first two). Allocates and blocks on the disk.
pub fn read(key: u64, at: &Path, rate: u32) -> Result<RefTrack, String> {
    let (bytes, hash) = fingerprint(at).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => format!("the file is not there: {}", at.display()),
        _ => format!("{}: {e}", at.display()),
    })?;
    let d = crate::samples::decode_audio(at, rate)?;
    let ch = d.channels.max(1) as usize;
    let frames = (d.frames.len() / ch).min(MAX_SECONDS * rate as usize);
    if frames == 0 {
        return Err("the file holds no audio".into());
    }
    let mut left = Vec::with_capacity(frames);
    let mut right = Vec::with_capacity(frames);
    for f in d.frames.chunks_exact(ch).take(frames) {
        let l = f[0];
        let r = if ch > 1 { f[1] } else { f[0] };
        if !l.is_finite() || !r.is_finite() {
            return Err("the file holds values that are not numbers".into());
        }
        left.push(l);
        right.push(r);
    }
    let mut t = RefTrack::new(key, rate, left, right);
    t.bytes = bytes;
    t.hash = hash;
    Ok(t)
}

/// Read every reference `snapshot` names that is not read yet, resolving a stored path with
/// `resolve`; `background`: on a thread of its own (a live engine). A reference not in yet
/// (or that failed) is taken out of the snapshot, so the unit plays the mix until a compile
/// names it. `render`: every Comparatio is set to the mix and its reference taken away.
/// Returns the files that could not be read. For the thread that compiles snapshots.
pub fn prepare(
    snapshot: &mut Snapshot,
    resolve: &dyn Fn(&str) -> PathBuf,
    rate: u32,
    render: bool,
    background: bool,
) -> Vec<(PathBuf, String)> {
    let mut errors = Vec::new();
    let mut named: Vec<(u64, u32)> = Vec::new();
    let slots = snapshot
        .strips
        .iter_mut()
        .flat_map(|s| s.effects.iter_mut())
        .chain(snapshot.master_effects.iter_mut());
    for slot in slots {
        if slot.spec.kind != EffectKind::Reference {
            continue;
        }
        if render {
            strip_for_render(&mut slot.spec);
            continue;
        }
        // Phase B: every other reference in the list read once for its summary.
        if let Some(refs) = slot.spec.refs.as_deref() {
            let compared = slot.spec.ir.as_deref().map(|f| f.path.as_str());
            for r in refs
                .list
                .iter()
                .filter(|r| Some(r.path.as_str()) != compared)
            {
                let key = (super::aula_store::file_key(&r.path), rate);
                if has_summary(key.0) {
                    continue;
                }
                let at = resolve(&r.path);
                if let Err(e) = ask(key, &at, background) {
                    errors.push((at, e));
                }
            }
        }
        let Some(file) = slot.spec.ir.as_deref() else {
            continue;
        };
        let key = (super::aula_store::file_key(&file.path), rate);
        let at = resolve(&file.path);
        named.push(key);
        let ready = match ask(key, &at, background) {
            Ok(r) => r,
            Err(e) => {
                errors.push((at, e));
                false
            }
        };
        if !ready {
            slot.spec.ir = None;
        }
    }
    sweep(&named);
    errors
}

fn has_summary(key: u64) -> bool {
    let s = store().lock().unwrap_or_else(|e| e.into_inner());
    s.summaries.iter().any(|(k, _)| *k == key)
}

/// The summary of the reference the project calls `path`, once it was read (the panel's).
pub fn summary(path: &str) -> Option<Arc<Summary>> {
    let key = super::aula_store::file_key(path);
    let s = store().lock().ok()?;
    s.summaries
        .iter()
        .rev()
        .find(|(k, _)| *k == key)
        .map(|(_, x)| x.clone())
}

/// Start reading the reference the project calls `path`, found at `at`, at `rate` — on a
/// thread of its own — unless it is read or being read (a plug-in's editor, a state loaded:
/// they have no compile to do it). Whether it is in.
pub fn request(path: &str, at: &Path, rate: u32) -> Result<bool, String> {
    ask((super::aula_store::file_key(path), rate), at, true)
}

/// The reference `key` at `rate` once it is in, waiting for one being read up to `timeout`
/// (a plug-in's memory builder, on its own thread; never the audio thread). `None` for one
/// that failed, was never asked for, or did not come in time.
pub fn wait_for(key: u64, rate: u32, timeout: std::time::Duration) -> Option<Arc<RefTrack>> {
    let until = std::time::Instant::now() + timeout;
    loop {
        {
            let s = store().lock().unwrap_or_else(|e| e.into_inner());
            match s.tracks.iter().rev().find(|(k, _, _)| *k == (key, rate)) {
                Some((_, _, Entry::Ready(t))) => return Some(t.clone()),
                Some((_, _, Entry::Loading)) => {}
                _ => return None,
            }
        }
        if std::time::Instant::now() >= until {
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// Whether the reference `key` at `rate` is being read now.
pub fn loading(key: u64, rate: u32) -> bool {
    let s = store().lock().unwrap_or_else(|e| e.into_inner());
    matches!(
        s.tracks.iter().rev().find(|(k, _, _)| *k == (key, rate)),
        Some((_, _, Entry::Loading))
    )
}

/// Let go of every reference read whose audio no unit holds but `keep` (a plug-in, which
/// compiles no snapshot: its references are swept here when the one compared changes). The
/// summaries stay.
pub fn keep_only(keep: Option<&str>, rate: u32) {
    let named: Vec<(u64, u32)> = keep
        .map(|p| (super::aula_store::file_key(p), rate))
        .into_iter()
        .collect();
    sweep(&named);
}

/// A Comparatio as a render hears it: the mix, no reference (ADR 0146).
pub fn strip_for_render(spec: &mut cantus_core::EffectSpec) {
    spec.ir = None;
    let _ = spec.set("listen", cantus_core::fx::comparatio::LISTEN_MIX);
}

/// Whether the reference `key` at `at` is in; starts reading it if it is not.
fn ask(key: (u64, u32), at: &Path, background: bool) -> Result<bool, String> {
    {
        let mut s = store().lock().unwrap_or_else(|e| e.into_inner());
        match s.tracks.iter().position(|(k, p, _)| *k == key && p == at) {
            Some(i) => {
                return match &s.tracks[i].2 {
                    Entry::Ready(_) => Ok(true),
                    Entry::Loading => Ok(false),
                    Entry::Failed(e) => Err(e.clone()),
                }
            }
            None => {
                // Read from elsewhere before (the project moved): read again.
                s.tracks.retain(|(k, _, _)| *k != key);
                s.tracks.push((key, at.to_path_buf(), Entry::Loading));
            }
        }
    }
    let job = {
        let at = at.to_path_buf();
        move || {
            let got = read(key.0, &at, key.1).map(Arc::new);
            if let Ok(t) = &got {
                t.analyse();
            }
            let mut s = store().lock().unwrap_or_else(|e| e.into_inner());
            if let Some(e) = s.tracks.iter_mut().find(|(k, p, _)| *k == key && *p == at) {
                e.2 = match got {
                    Ok(t) => Entry::Ready(t),
                    Err(e) => Entry::Failed(e),
                };
            }
        }
    };
    if background {
        let spawned = std::thread::Builder::new()
            .name("cantus-reference".into())
            .spawn(move || {
                job();
                ARRIVED.store(true, Ordering::Relaxed);
            });
        if spawned.is_ok() {
            return Ok(false);
        }
        // No thread: read here rather than never.
        return ask_now(key, at);
    }
    job();
    ask_now(key, at)
}

fn ask_now(key: (u64, u32), at: &Path) -> Result<bool, String> {
    let s = store().lock().unwrap_or_else(|e| e.into_inner());
    match s.tracks.iter().find(|(k, p, _)| *k == key && p == at) {
        Some((_, _, Entry::Ready(_))) => Ok(true),
        Some((_, _, Entry::Failed(e))) => Err(e.clone()),
        _ => Ok(false),
    }
}

/// Free the references no unit holds and no snapshot names, beyond [`SPARE`], oldest first.
fn sweep(named: &[(u64, u32)]) {
    let mut s = store().lock().unwrap_or_else(|e| e.into_inner());
    let idle = |e: &((u64, u32), PathBuf, Entry)| {
        !named.contains(&e.0) && matches!(&e.2, Entry::Ready(t) if Arc::strong_count(t) == 1)
    };
    let mut spare = s.tracks.iter().filter(|e| idle(e)).count();
    let mut i = 0;
    while spare > SPARE && i < s.tracks.len() {
        if idle(&s.tracks[i]) {
            s.tracks.remove(i);
            spare -= 1;
        } else {
            i += 1;
        }
    }
}

/// The reference for a unit's need, if it is in (`EffectMemory::build`).
pub fn get(key: u64, rate: u32) -> Option<Arc<RefTrack>> {
    let s = store().lock().ok()?;
    s.tracks.iter().rev().find_map(|(k, _, e)| match e {
        Entry::Ready(t) if *k == (key, rate) => Some(t.clone()),
        _ => None,
    })
}

/// What became of the reference the project calls `path`, at any rate (the panel's).
pub fn status(path: &str) -> RefStatus {
    let key = super::aula_store::file_key(path);
    let Ok(s) = store().lock() else {
        return RefStatus::Unknown;
    };
    match s.tracks.iter().rev().find(|(k, _, _)| k.0 == key) {
        Some((_, _, Entry::Loading)) => RefStatus::Loading,
        Some((_, _, Entry::Ready(t))) => RefStatus::Loaded {
            seconds: t.seconds(),
            analysed: t.whole_lufs().is_some() || t.analysis().is_some(),
        },
        Some((_, _, Entry::Failed(e))) => RefStatus::Failed(e.clone()),
        None => RefStatus::Unknown,
    }
}

/// The reference the project calls `path`, at any rate it was read at (the panel's: its
/// whole-file analysis and its overview).
pub fn track(path: &str) -> Option<Arc<RefTrack>> {
    let key = super::aula_store::file_key(path);
    let s = store().lock().ok()?;
    s.tracks.iter().rev().find_map(|(k, _, e)| match e {
        Entry::Ready(t) if k.0 == key => Some(t.clone()),
        _ => None,
    })
}

/// Forget a reference that failed, so the next compile reads it again (the panel's Retry).
pub fn retry(path: &str) {
    let key = super::aula_store::file_key(path);
    if let Ok(mut s) = store().lock() {
        s.tracks
            .retain(|(k, _, e)| !(k.0 == key && matches!(e, Entry::Failed(_))));
    }
}

/// Forget everything read of `path` (its audio no unit holds, and its summary), so the next
/// compile reads the file again — a reference whose file changed, read anew (the panel's).
pub fn reload(path: &str) {
    let key = super::aula_store::file_key(path);
    if let Ok(mut s) = store().lock() {
        s.tracks.retain(|(k, _, e)| {
            k.0 != key || matches!(e, Entry::Ready(t) if Arc::strong_count(t) > 1)
        });
        s.summaries.retain(|(k, _)| *k != key);
    }
}

/// References read and kept, for the memory report: their bytes.
pub fn bytes() -> usize {
    let Ok(s) = store().lock() else { return 0 };
    s.tracks
        .iter()
        .map(|(_, _, e)| match e {
            Entry::Ready(t) => (t.left.len() + t.right.len()) * std::mem::size_of::<f32>(),
            _ => 0,
        })
        .sum()
}

/// For the tests: a reference made in memory, as if read from `path`, at `rate`.
pub fn insert_for_test(path: &str, rate: u32, left: Vec<f32>, right: Vec<f32>) -> Arc<RefTrack> {
    let key = super::aula_store::file_key(path);
    let t = Arc::new(RefTrack::new(key, rate, left, right));
    t.analyse();
    let mut s = store().lock().unwrap_or_else(|e| e.into_inner());
    s.tracks.retain(|(k, _, _)| *k != (key, rate));
    s.tracks
        .push(((key, rate), PathBuf::from(path), Entry::Ready(t.clone())));
    t
}
