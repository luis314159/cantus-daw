//! One `.clap` file loaded: the shared library and its entry, initialised once however many
//! instances come from it.
//!
//! CLAP asks for `clap_entry.init` once per load of the file and `deinit` once when the host
//! is done with it. The operating system hands the same module back to a second load of
//! the same path, so two slots with the same plug-in must share one `Library`, and the last
//! one to go calls `deinit`. [`Library::open`] keeps a table of the open ones for that.

use crate::backend::HostError;
use clap_sys::entry::clap_plugin_entry;
use clap_sys::factory::plugin_factory::{clap_plugin_factory, CLAP_PLUGIN_FACTORY_ID};
use clap_sys::version::clap_version_is_compatible;
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};

pub(crate) struct Library {
    pub path: PathBuf,
    entry: *const clap_plugin_entry,
    pub factory: *const clap_plugin_factory,
    /// Declared last so it is dropped after `deinit` has run.
    _lib: libloading::Library,
}

// The entry and the factory are plain tables of functions the plug-in documents as
// callable from the main thread; `Library` is only touched there, but lives in `Arc`s that
// the audio halves hold too.
unsafe impl Send for Library {}
unsafe impl Sync for Library {}

static OPEN: Mutex<Vec<(PathBuf, Weak<Library>)>> = Mutex::new(Vec::new());

impl Library {
    /// Load `path`, or share the copy already loaded.
    pub fn open(path: &Path) -> Result<Arc<Library>, HostError> {
        let mut open = OPEN.lock().unwrap_or_else(|e| e.into_inner());
        open.retain(|(_, w)| w.strong_count() > 0);
        if let Some(lib) = open
            .iter()
            .find(|(p, _)| p == path)
            .and_then(|(_, w)| w.upgrade())
        {
            return Ok(lib);
        }
        let lib = Arc::new(Self::load(path)?);
        open.push((path.to_path_buf(), Arc::downgrade(&lib)));
        Ok(lib)
    }

    fn load(path: &Path) -> Result<Library, HostError> {
        let fail = |what: &str| HostError::Load(format!("{}: {what}", path.display()));
        // SAFETY: loading a library runs its initialisers. That is what hosting a plug-in
        // is; the scanner does it first in a separate process, and a file that crashed
        // there is never loaded here.
        let lib = unsafe { libloading::Library::new(path) }.map_err(|e| fail(&e.to_string()))?;
        // SAFETY: `clap_entry` is a data symbol of type `clap_plugin_entry`; asking for a
        // pointer type yields its address.
        let entry: *const clap_plugin_entry = unsafe {
            *lib.get::<*const clap_plugin_entry>(b"clap_entry\0")
                .map_err(|_| fail("no clap_entry"))?
        };
        if entry.is_null() {
            return Err(fail("clap_entry is null"));
        }
        // SAFETY: the entry is valid for as long as the library is loaded.
        let e = unsafe { &*entry };
        if !clap_version_is_compatible(e.clap_version) {
            return Err(fail("incompatible CLAP version"));
        }
        let (Some(init), Some(get_factory)) = (e.init, e.get_factory) else {
            return Err(fail("incomplete entry"));
        };
        let c_path = CString::new(path.to_string_lossy().as_bytes()).map_err(|_| fail("path"))?;
        // SAFETY: the entry's documented contract; called once per load.
        if !unsafe { init(c_path.as_ptr()) } {
            return Err(fail("init refused"));
        }
        // SAFETY: as above.
        let factory =
            unsafe { get_factory(CLAP_PLUGIN_FACTORY_ID.as_ptr()) } as *const clap_plugin_factory;
        if factory.is_null() {
            if let Some(deinit) = e.deinit {
                // SAFETY: balances the `init` above.
                unsafe { deinit() };
            }
            return Err(fail("no plug-in factory"));
        }
        Ok(Library {
            path: path.to_path_buf(),
            entry,
            factory,
            _lib: lib,
        })
    }

    pub fn factory(&self) -> &clap_plugin_factory {
        // SAFETY: checked non-null at load; valid until deinit.
        unsafe { &*self.factory }
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        // Serialised with `open`, so a `deinit` never overlaps an `init` of the same file.
        let _open = OPEN.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: the entry is still mapped (the library is dropped after this), and every
        // instance made from it holds an `Arc` to this, so none is left.
        unsafe {
            if let Some(deinit) = (*self.entry).deinit {
                deinit();
            }
        }
    }
}
