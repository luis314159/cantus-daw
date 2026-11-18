//! fxc:comparatio — Comparatio's references in the exported plug-in (ADR 0143 phase B).
//!
//! A reference is a file on the user's disk, never audio in the state: the state names each
//! one by its path with the size and hash it had when it was loaded (`EffectSpec::refs`,
//! the one compared in `EffectSpec::ir`, the project's own format), so a session that moves
//! to a machine without the file — or whose file changed — says so plainly, and nothing is
//! skipped silently.
//!
//! The main thread keeps the list ([`RefPost::set`]: a state loaded, the editor's *Load
//! reference…* or a file dropped on it) and asks the reference store to read the one
//! compared on a thread of its own at the processor's rate
//! (`cantus_engine::fx::comparatio_store::request`); the file compared goes to the running
//! processor through the same lock-free post as Fractus's lanes. The unit then asks for its
//! reference (`MemoryNeed::Comparatio`), and the plug-in's memory builder waits on its own
//! thread for the store to have it (`crate::memory`). The audio thread never reads a file,
//! allocates or locks for it.
//!
//! **A host's offline render hears the mix**, as every render in Cantus does: CLAP's
//! `render` extension and VST3's `processMode` set [`RefPost::set_offline`], and the
//! processor then sets the unit to your mix whatever the panel says.

use crate::lapsus_bank::{Content, ContentPost};
use cantus_core::fx::comparatio::References;
use cantus_core::IrFile;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;

impl Content for IrFile {
    fn is_empty(&self) -> bool {
        self.path.trim().is_empty()
    }
}

/// Comparatio's references in a plug-in instance.
#[derive(Debug, Default)]
pub struct RefPost {
    /// The list, as the main thread holds it.
    list: Mutex<Option<References>>,
    /// The file compared, posted to the processor (`EffectSpec::ir`).
    pub file: ContentPost<IrFile>,
    /// The rate the processor runs at, `f32` bits (0 before the first activation).
    rate: AtomicU32,
    /// The host renders offline (a bounce): the unit hears the mix.
    offline: AtomicBool,
}

impl RefPost {
    /// The list and the file compared (main thread).
    pub fn get(&self) -> (Option<References>, Option<IrFile>) {
        let list = self.list.lock().ok().and_then(|l| l.clone());
        (list, self.file.get().map(|b| *b))
    }

    /// A new list and file compared (a state loaded, the editor): kept, the file posted to
    /// the processor, and read at the processor's rate when there is one. Main thread.
    pub fn set(&self, refs: Option<References>, selected: Option<IrFile>) {
        if let Ok(mut l) = self.list.lock() {
            *l = refs.filter(|r| !r.is_empty());
        }
        let rate = self.rate();
        if let (Some(f), true) = (selected.as_ref(), rate > 0.0) {
            read(&f.path, rate);
        }
        if rate > 0.0 {
            cantus_engine::fx::comparatio_store::keep_only(
                selected.as_ref().map(|f| f.path.as_str()),
                rate.round() as u32,
            );
        }
        self.file.set(selected.map(Box::new));
    }

    /// The processor's rate, when it is made (main thread): the file compared read at it.
    pub fn activate(&self, rate: f32) {
        self.rate.store(rate.to_bits(), Ordering::Release);
        if let Some(f) = self.file.get() {
            read(&f.path, rate);
        }
    }

    pub fn rate(&self) -> f32 {
        f32::from_bits(self.rate.load(Ordering::Acquire))
    }

    /// The host renders offline (`true`) or in real time.
    pub fn set_offline(&self, offline: bool) {
        self.offline.store(offline, Ordering::Release);
    }

    pub fn offline(&self) -> bool {
        self.offline.load(Ordering::Acquire)
    }
}

/// Ask the store to read `path` at `rate` on a thread of its own (a failure is the store's
/// to report: the editor shows it).
fn read(path: &str, rate: f32) {
    let at = std::path::Path::new(path);
    let _ = cantus_engine::fx::comparatio_store::request(path, at, rate.round() as u32);
}
