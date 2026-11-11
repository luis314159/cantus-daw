//! Aula's responses off the audio thread (ADR 0112): the files the project names, read once
//! and kept; Cantus's own responses, made at the engine's rate; and every kernel handed to
//! an engine, kept here too.
//!
//! **Why a pool**: a unit swaps kernels on the audio thread, where nothing may be freed.
//! Every kernel a unit holds has a sibling here, so the unit letting one go is an atomic
//! decrement and never a free (the sample cache's rule, `crate::samples`). A kernel no unit
//! holds any more (its count is the pool's alone) is freed here, on the next build, off the
//! audio thread.
//!
//! **How a file gets here**: [`prepare`] runs where every snapshot is compiled
//! (`EngineHandle::compile`, live and offline), so every response a snapshot names is read
//! before an engine asks for its kernel ([`build`]).

use super::aula::{AulaNeed, Memory};
use cantus_core::{EffectKind, Snapshot};
use cantus_dsp::aula::prepare as prepare_kernel;
use cantus_dsp::aula_conv::State;
use cantus_dsp::aula_kernel::{Kernel, Layout};
use cantus_dsp::aula_shape::{overview, Ir, Shape};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// What became of a file a project names.
#[derive(Clone, Debug, PartialEq)]
pub enum IrStatus {
    /// Read: its rate, channels and length (seconds).
    Loaded {
        rate: u32,
        channels: usize,
        seconds: f32,
    },
    Failed(String),
    /// Never asked for.
    Unknown,
}

/// A kernel and what the editor draws from it.
#[derive(Clone, Debug)]
pub struct Built {
    pub kernel: Arc<Kernel>,
    /// The shaped response's outline (`OUTLINE` peaks, linear).
    pub outline: Arc<[f32]>,
    pub seconds: f32,
    /// Whether its source was there (a file that failed builds silence).
    pub found: bool,
}

/// Points in a response's outline.
pub const OUTLINE: usize = 512;
/// Kernels kept once no unit holds them (undo, a second instance of the same room).
const SPARE: usize = 6;
/// Cantus's own responses kept, made, at a rate.
const MADE: usize = 4;

#[derive(Default)]
struct Store {
    /// Files by key: where it was read from and what it held.
    files: HashMap<u64, (PathBuf, Result<Arc<Ir>, String>)>,
    /// Cantus's own, by (index, rate), newest last.
    made: Vec<((u64, u32), Arc<Ir>)>,
    /// Kernels by what made them, oldest first.
    pool: Vec<(PoolKey, Built)>,
    silent: Vec<Arc<Kernel>>,
    /// Sources at the editor's rate, and the outlines drawn from them, newest last.
    previews: Vec<(u64, Arc<Ir>)>,
    outlines: Vec<((u64, [u32; 9]), Outline)>,
}

/// A response's picture for the editor: drawn from a copy at a low rate, so a knob moving
/// redraws it at once without building a kernel.
#[derive(Clone, Debug)]
pub struct Outline {
    /// [`OUTLINE`] peaks over the response's length, linear.
    pub peaks: Arc<[f32]>,
    pub seconds: f32,
    /// 1 mono, 2 stereo, 4 true stereo.
    pub paths: usize,
    /// Whether its source was there.
    pub found: bool,
}

/// The rate the editor's outlines are drawn at.
const PREVIEW_RATE: f32 = 12_000.0;
const PREVIEWS: usize = 8;
const OUTLINES: usize = 24;

/// The outline of the response `source` shaped by `shape` (the bits of
/// [`shape_bits`]): from the store when drawn before. For the interface's thread.
pub fn outline(source: u64, shape: [u32; 9]) -> Outline {
    let mut s = store().lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, o)) = s.outlines.iter().find(|(k, _)| *k == (source, shape)) {
        return o.clone();
    }
    let src = if let Some((_, ir)) = s.previews.iter().find(|(k, _)| *k == source) {
        Some(ir.clone())
    } else {
        let made = if is_file(source) {
            match s.files.get(&source) {
                Some((_, Ok(ir))) => Some(Arc::new(cantus_dsp::aula_shape::shape(
                    ir,
                    PREVIEW_RATE,
                    &Shape::default(),
                    false,
                ))),
                _ => None,
            }
        } else {
            Some(Arc::new(cantus_dsp::aula_gen::factory(
                source as usize,
                PREVIEW_RATE,
            )))
        };
        if let Some(ir) = &made {
            s.previews.push((source, ir.clone()));
            if s.previews.len() > PREVIEWS {
                s.previews.remove(0);
            }
        }
        made
    };
    let Some(src) = src else {
        return Outline {
            peaks: vec![0.0; OUTLINE].into(),
            seconds: 0.0,
            paths: 0,
            found: false,
        };
    };
    let shape_of = AulaNeed {
        source,
        shape,
        cap: Layout::default(),
    }
    .shape();
    let ir = cantus_dsp::aula_shape::shape(&src, PREVIEW_RATE, &shape_of, true);
    let o = Outline {
        peaks: overview(&ir, OUTLINE).into(),
        seconds: ir.seconds(),
        paths: ir.format.paths(),
        found: true,
    };
    s.outlines.push(((source, shape), o.clone()));
    if s.outlines.len() > OUTLINES {
        s.outlines.remove(0);
    }
    o
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct PoolKey {
    source: u64,
    shape: [u32; 9],
    rate: u32,
    cap: Layout,
}

fn store() -> &'static Mutex<Store> {
    static S: OnceLock<Mutex<Store>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Store::default()))
}

/// The key of a file's response: FNV-1a of its path as the project stores it, with the top
/// bit set (Cantus's own are their small indices). Allocation-free: the audio thread uses it.
pub fn file_key(path: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in path.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h | (1 << 63)
}

/// Whether a key names a file (the others are Cantus's own).
pub fn is_file(key: u64) -> bool {
    key >> 63 == 1
}

/// Read every response `snapshot` names that is not read yet, resolving a stored path with
/// `resolve` (the project's folder). Returns the files that could not be read. Blocks on
/// the disk: for the thread that compiles snapshots, never the audio thread.
pub fn prepare(snapshot: &Snapshot, resolve: &dyn Fn(&str) -> PathBuf) -> Vec<(PathBuf, String)> {
    let mut errors = Vec::new();
    let slots = snapshot
        .strips
        .iter()
        .flat_map(|s| s.effects.iter())
        .chain(snapshot.master_effects.iter());
    for slot in slots {
        if slot.spec.kind != EffectKind::Aula {
            continue;
        }
        let Some(file) = &slot.spec.ir else { continue };
        if let Err(e) = load(&file.path, &resolve(&file.path)) {
            errors.push((resolve(&file.path), e));
        }
    }
    errors
}

/// Read the file at `at` as the response the project calls `path` (once: a second call is
/// answered from the store, unless it now resolves elsewhere or failed before).
pub fn load(path: &str, at: &Path) -> Result<Arc<Ir>, String> {
    let key = file_key(path);
    if let Ok(s) = store().lock() {
        if let Some((p, Ok(ir))) = s.files.get(&key) {
            if p == at {
                return Ok(ir.clone());
            }
        }
    }
    let read = super::aula_load::read(at).map(Arc::new);
    if let Ok(mut s) = store().lock() {
        s.files.insert(key, (at.to_path_buf(), read.clone()));
    }
    read
}

/// What became of the file the project calls `path`.
pub fn status(path: &str) -> IrStatus {
    let Ok(s) = store().lock() else {
        return IrStatus::Unknown;
    };
    match s.files.get(&file_key(path)) {
        Some((_, Ok(ir))) => IrStatus::Loaded {
            rate: ir.rate as u32,
            channels: ir.channels.len(),
            seconds: ir.seconds(),
        },
        Some((_, Err(e))) => IrStatus::Failed(e.clone()),
        None => IrStatus::Unknown,
    }
}

/// Forget a file that failed, so the next snapshot reads it again (it was put back).
pub fn retry(path: &str) {
    if let Ok(mut s) = store().lock() {
        if matches!(s.files.get(&file_key(path)), Some((_, Err(_)))) {
            s.files.remove(&file_key(path));
        }
    }
}

/// The source a key names, at `rate` for Cantus's own.
fn source(s: &mut Store, key: u64, rate: f32) -> Option<Arc<Ir>> {
    if is_file(key) {
        return match s.files.get(&key) {
            Some((_, Ok(ir))) => Some(ir.clone()),
            _ => None,
        };
    }
    let id = (key, rate.to_bits());
    if let Some(i) = s.made.iter().position(|(k, _)| *k == id) {
        let e = s.made.remove(i);
        let ir = e.1.clone();
        s.made.push(e);
        return Some(ir);
    }
    let ir = Arc::new(cantus_dsp::aula_gen::factory(key as usize, rate));
    s.made.push((id, ir.clone()));
    if s.made.len() > MADE {
        s.made.remove(0);
    }
    Some(ir)
}

/// The silence a convolver of capacity `cap` runs before its first response, kept here.
pub fn silent(cap: Layout) -> Arc<Kernel> {
    let mut s = store().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(k) = s.silent.iter().find(|k| k.cap() == cap) {
        return k.clone();
    }
    let k = Arc::new(Kernel::silent(cap));
    s.silent.push(k.clone());
    k
}

/// The kernel `need` asks for, at `rate`: from the pool, or made (and kept). Allocates.
pub fn kernel(need: &AulaNeed, rate: f32) -> Built {
    let key = PoolKey {
        source: need.source,
        shape: need.shape,
        rate: rate.to_bits(),
        cap: need.cap,
    };
    let mut s = store().lock().unwrap_or_else(|e| e.into_inner());
    prune(&mut s);
    if let Some((_, b)) = s.pool.iter().find(|(k, _)| *k == key) {
        return b.clone();
    }
    let built = match source(&mut s, need.source, rate) {
        Some(src) => {
            // The store stays locked while the kernel is made: two builds of one response
            // never run twice over, and a build is a few tens of milliseconds.
            let (ir, k) = prepare_kernel(&src, rate, &need.shape(), Some(need.cap));
            Built {
                outline: overview(&ir, OUTLINE).into(),
                seconds: ir.seconds(),
                kernel: Arc::new(k),
                found: true,
            }
        }
        None => Built {
            kernel: Arc::new(Kernel::silent(need.cap)),
            outline: vec![0.0; OUTLINE].into(),
            seconds: 0.0,
            found: false,
        },
    };
    // A file not read yet builds silence, but that silence is not kept: once the file is
    // in, the next ask makes the real kernel.
    if built.found {
        s.pool.push((key, built.clone()));
    }
    built
}

/// The memory a unit asked for: the kernel, and a state when the kernel needs more room.
pub fn build(need: AulaNeed, rate: f32) -> Memory {
    let b = kernel(&need, rate);
    let state = (b.kernel.cap() != need.cap).then(|| Box::new(State::new(b.kernel.cap())));
    Memory {
        need,
        kernel: Some(b.kernel),
        state,
    }
}

/// Free the kernels no unit holds, keeping the newest [`SPARE`].
fn prune(s: &mut Store) {
    let idle: Vec<usize> = (0..s.pool.len())
        .filter(|&i| Arc::strong_count(&s.pool[i].1.kernel) == 1)
        .collect();
    if idle.len() > SPARE {
        let drop: Vec<usize> = idle[..idle.len() - SPARE].to_vec();
        for i in drop.into_iter().rev() {
            s.pool.remove(i);
        }
    }
}

/// Bytes the pool's kernels hold (the memory report).
pub fn pool_bytes() -> usize {
    store()
        .lock()
        .map(|s| s.pool.iter().map(|(_, b)| b.kernel.bytes()).sum())
        .unwrap_or(0)
}

/// Shape bits for a key (tests).
pub fn shape_bits(s: &Shape) -> [u32; 9] {
    [
        s.stretch.to_bits(),
        s.length.to_bits(),
        u32::from(s.reverse),
        s.attack_ms.to_bits(),
        s.decay.to_bits(),
        s.gate_db.to_bits(),
        s.low_hz.to_bits(),
        s.high_hz.to_bits(),
        s.damp.to_bits(),
    ]
}
