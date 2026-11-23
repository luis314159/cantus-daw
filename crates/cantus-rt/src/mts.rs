//! MTS-ESP (ADR 0156): the tuning link between programs, through ODDSound's `LIBMTS` — loaded
//! at run time from its standard place, never shipped (the library is published as binaries
//! only; `research/instruments/microtuning-verified.md` §3). Cantus follows a master (the
//! client side) and can be one (the master side). r42:tuning-face
//!
//! Where the library lives, as ODDSound's own wrappers look (`libMTSClient.cpp`):
//!
//! - Windows: `Program Files\Common Files\MTS-ESP\LIBMTS.dll` (the 64-bit Common Files);
//! - macOS: `/Library/Application Support/MTS-ESP/libMTS.dylib`;
//! - Linux: `/usr/local/lib/libMTS.so`.
//!
//! **Threads.** [`Mts::open`], the registrations and the names run on the main thread (a
//! library is loaded, the master's state may be shared between processes). [`Mts::read`],
//! [`Mts::has_master`] and [`Mts::publish`] are what the audio thread may call: a call into
//! the library that reads or writes its table — no lock, no allocation, no I/O (ODDSound's
//! client reads the same table through the same pointer on its audio thread).
//!
//! The library is never unloaded: a client in another module may still hold its table's
//! pointer, and there is one per process. Dropping an [`Mts`] gives up the master's role and
//! the client's registration.

use std::ffi::{c_char, c_void, CStr, CString};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// The keys of a table: MIDI 0…127.
pub const KEYS: usize = 128;

/// Why the library is not there to use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// Nothing at the standard place (MTS-ESP is not installed): the usual case.
    NotInstalled(PathBuf),
    /// The system has no standard place for it (a platform ODDSound does not ship).
    NoPlace,
    /// A file is there, but the system would not load it (the reason it gave).
    Open(String),
    /// It loaded, but lacks the functions every client needs (named): not `LIBMTS`.
    Symbols(Vec<&'static str>),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::NotInstalled(p) => write!(f, "not installed ({})", p.display()),
            LoadError::NoPlace => f.write_str("no standard place on this system"),
            LoadError::Open(why) => write!(f, "could not be loaded: {why}"),
            LoadError::Symbols(s) => write!(f, "is not LIBMTS (missing {})", s.join(", ")),
        }
    }
}

/// Whether Cantus may be the master now.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MasterState {
    /// Cantus is the master.
    Ours,
    /// No master is registered: Cantus may become it.
    Free,
    /// Another program is the master. `can_reinitialize`: the library shares its state
    /// between processes, so a master that crashed may have left the flag set — ODDSound's
    /// header lets the user *reinitialise* MTS-ESP then (it also disconnects a live master).
    Another { can_reinitialize: bool },
    /// This library has no master functions (an old or partial `LIBMTS`).
    Unsupported,
}

/// Why [`Mts::become_master`] was refused.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Another master is registered (ODDSound: "only one master … at any one time").
    Another { can_reinitialize: bool },
    Unsupported,
}

type FnVoid = unsafe extern "C" fn();
type FnBool = unsafe extern "C" fn() -> bool;
type FnTable = unsafe extern "C" fn() -> *const f64;
type FnName = unsafe extern "C" fn() -> *const c_char;
type FnKeyFilter = unsafe extern "C" fn(c_char, c_char) -> bool;
type FnRegisterMaster = unsafe extern "C" fn(*mut c_void);
type FnInt = unsafe extern "C" fn() -> i32;
type FnSetTable = unsafe extern "C" fn(*const f64);
type FnSetName = unsafe extern "C" fn(*const c_char);
type FnFilterNote = unsafe extern "C" fn(bool, c_char, c_char);

/// The functions a client needs: without one of them the file is not `LIBMTS`.
const CLIENT_SYMBOLS: [&str; 4] = [
    "MTS_RegisterClient",
    "MTS_DeregisterClient",
    "MTS_HasMaster",
    "MTS_GetTuningTable",
];

#[derive(Copy, Clone)]
struct Symbols {
    register_client: FnVoid,
    deregister_client: FnVoid,
    has_master: FnBool,
    get_table: FnTable,
    should_filter: Option<FnKeyFilter>,
    scale_name: Option<FnName>,
    register_master: Option<FnRegisterMaster>,
    deregister_master: Option<FnVoid>,
    reinitialize: Option<FnVoid>,
    has_ipc: Option<FnBool>,
    clients: Option<FnInt>,
    set_table: Option<FnSetTable>,
    set_scale_name: Option<FnSetName>,
    filter_note: Option<FnFilterNote>,
    clear_filter: Option<FnVoid>,
}

/// `LIBMTS`, loaded, and Cantus's place in it (client, master).
pub struct Mts {
    path: PathBuf,
    f: Symbols,
    client: AtomicBool,
    master: AtomicBool,
}

impl std::fmt::Debug for Mts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mts")
            .field("path", &self.path)
            .field("client", &self.client.load(Ordering::Relaxed))
            .field("master", &self.master.load(Ordering::Relaxed))
            .finish()
    }
}

// SAFETY: `Mts` holds function pointers into a library that is never unloaded, and two
// flags; libMTS is made to be called from any thread (a plug-in's audio thread reads its
// table, its interface thread registers), and no pointer to Rust data is shared.
unsafe impl Send for Mts {}
// SAFETY: as for `Send`: every method takes `&self` and touches only atomics and the
// library's own functions.
unsafe impl Sync for Mts {}

/// Where `LIBMTS` is installed on this system, if the system has a place for it.
pub fn standard_path() -> Option<PathBuf> {
    if cfg!(windows) {
        // The 64-bit Common Files folder (`FOLDERID_ProgramFilesCommon` for a 64-bit
        // process, which is what `CommonProgramW6432` and `CommonProgramFiles` hold).
        let common = std::env::var_os("CommonProgramW6432")
            .or_else(|| std::env::var_os("CommonProgramFiles"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Program Files\Common Files"));
        Some(common.join("MTS-ESP").join("LIBMTS.dll"))
    } else if cfg!(target_os = "macos") {
        Some(PathBuf::from(
            "/Library/Application Support/MTS-ESP/libMTS.dylib",
        ))
    } else if cfg!(target_os = "linux") {
        Some(PathBuf::from("/usr/local/lib/libMTS.so"))
    } else {
        None
    }
}

impl Mts {
    /// `LIBMTS` from its standard place (main thread).
    pub fn open_standard() -> Result<Mts, LoadError> {
        let path = standard_path().ok_or(LoadError::NoPlace)?;
        if !path.is_file() {
            return Err(LoadError::NotInstalled(path));
        }
        Mts::open(&path)
    }

    /// A library exporting `LIBMTS`'s functions at `path` (main thread): the standard one,
    /// or a test's fake.
    pub fn open(path: &Path) -> Result<Mts, LoadError> {
        if !path.is_file() {
            return Err(LoadError::NotInstalled(path.to_path_buf()));
        }
        let lib = sys::Lib::open(path).map_err(LoadError::Open)?;
        let missing: Vec<&'static str> = CLIENT_SYMBOLS
            .iter()
            .copied()
            .filter(|s| lib.symbol(s).is_null())
            .collect();
        if !missing.is_empty() {
            return Err(LoadError::Symbols(missing));
        }
        // SAFETY: each pointer is the library's export of that name, and its type is the
        // one ODDSound's headers give it (`microtuning-verified.md` §3.2–3.3); a null one
        // becomes `None` (the four a client needs were checked above).
        let f = unsafe {
            Symbols {
                register_client: lib.get("MTS_RegisterClient").expect("checked"),
                deregister_client: lib.get("MTS_DeregisterClient").expect("checked"),
                has_master: lib.get("MTS_HasMaster").expect("checked"),
                get_table: lib.get("MTS_GetTuningTable").expect("checked"),
                should_filter: lib.get("MTS_ShouldFilterNote"),
                scale_name: lib.get("MTS_GetScaleName"),
                register_master: lib.get("MTS_RegisterMaster"),
                deregister_master: lib.get("MTS_DeregisterMaster"),
                reinitialize: lib.get("MTS_Reinitialize"),
                has_ipc: lib.get("MTS_HasIPC"),
                clients: lib.get("MTS_GetNumClients"),
                set_table: lib.get("MTS_SetNoteTunings"),
                set_scale_name: lib.get("MTS_SetScaleName"),
                filter_note: lib.get("MTS_FilterNote"),
                clear_filter: lib.get("MTS_ClearNoteFilter"),
            }
        };
        // Never unloaded (see the module's notes).
        std::mem::forget(lib);
        Ok(Mts {
            path: path.to_path_buf(),
            f,
            client: AtomicBool::new(false),
            master: AtomicBool::new(false),
        })
    }

    /// The file it was loaded from.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Register as a client (main thread; once — again is nothing).
    pub fn register_client(&self) {
        if !self.client.swap(true, Ordering::AcqRel) {
            // SAFETY: a library function taking nothing, on the main thread.
            unsafe { (self.f.register_client)() }
        }
    }

    /// Give the client's registration back (main thread).
    pub fn deregister_client(&self) {
        if self.client.swap(false, Ordering::AcqRel) {
            // SAFETY: as `register_client`.
            unsafe { (self.f.deregister_client)() }
        }
    }

    /// Whether a master is registered — Cantus itself included (audio thread too).
    pub fn has_master(&self) -> bool {
        // SAFETY: a flag read in the library; lock-free by ODDSound's design (§3.5).
        unsafe { (self.f.has_master)() }
    }

    /// The master's table, in Hz per key, into `out`; `false` (leaving `out` alone) when
    /// there is no master, when Cantus is the master (it does not follow itself), or when
    /// the library hands no table. Audio thread: no lock, no allocation.
    pub fn read(&self, out: &mut [f64; KEYS]) -> bool {
        if self.master.load(Ordering::Acquire) || !self.has_master() {
            return false;
        }
        // SAFETY: the library's table pointer, read now (it may move between masters).
        let p = unsafe { (self.f.get_table)() };
        if p.is_null() {
            return false;
        }
        // SAFETY: `MTS_GetTuningTable` points at 128 doubles that live as long as the
        // library (the client wrapper reads `table[note]` the same way). A read racing a
        // master's write sees each double whole on every platform libMTS ships for.
        let table = unsafe { std::slice::from_raw_parts(p, KEYS) };
        out.copy_from_slice(table);
        true
    }

    /// Whether the master leaves `key` unmapped (it should sound nothing).
    pub fn should_filter(&self, key: u8) -> bool {
        match self.f.should_filter {
            // SAFETY: a library query by value; channel −1 = any (the header's "no channel").
            Some(f) => unsafe { f(key.min(127) as c_char, -1) },
            None => false,
        }
    }

    /// The master's scale name, if it gives one (main thread).
    pub fn scale_name(&self) -> Option<String> {
        let f = self.f.scale_name?;
        if !self.has_master() {
            return None;
        }
        // SAFETY: a library function returning a NUL-terminated string it owns (or null).
        let p = unsafe { f() };
        if p.is_null() {
            return None;
        }
        // SAFETY: non-null, NUL-terminated, valid until the master renames — copied now.
        let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().trim().to_string();
        (!s.is_empty()).then_some(s)
    }
}

impl Mts {
    /// Whether Cantus is the master.
    pub fn is_master(&self) -> bool {
        self.master.load(Ordering::Acquire)
    }

    /// Whether Cantus may become the master now (main thread).
    pub fn master_state(&self) -> MasterState {
        if self.is_master() {
            return MasterState::Ours;
        }
        if self.f.register_master.is_none() || self.f.set_table.is_none() {
            return MasterState::Unsupported;
        }
        if !self.has_master() {
            return MasterState::Free;
        }
        // SAFETY: a library query taking nothing.
        let ipc = self.f.has_ipc.is_some_and(|f| unsafe { f() });
        MasterState::Another {
            can_reinitialize: ipc && self.f.reinitialize.is_some(),
        }
    }

    /// Become the master (main thread), as ODDSound's header prescribes: refused while
    /// another master is registered — unless `reinitialize`, which (where the library shares
    /// its state between processes) clears a master a crashed program left, and registers.
    pub fn become_master(&self, reinitialize: bool) -> Result<(), Refusal> {
        match self.master_state() {
            MasterState::Ours => return Ok(()),
            MasterState::Unsupported => return Err(Refusal::Unsupported),
            MasterState::Free => {}
            MasterState::Another { can_reinitialize } => {
                if !(reinitialize && can_reinitialize) {
                    return Err(Refusal::Another { can_reinitialize });
                }
                if let Some(f) = self.f.reinitialize {
                    // SAFETY: a library call taking nothing, on the main thread.
                    unsafe { f() }
                }
            }
        }
        let register = self.f.register_master.ok_or(Refusal::Unsupported)?;
        // SAFETY: the wrapper passes a null pointer (its "master data", unused).
        unsafe { register(std::ptr::null_mut()) };
        self.master.store(true, Ordering::Release);
        Ok(())
    }

    /// Give the master's role back (main thread); clients fall back to their own tuning.
    pub fn resign_master(&self) {
        if self.master.swap(false, Ordering::AcqRel) {
            if let Some(f) = self.f.deregister_master {
                // SAFETY: a library call taking nothing, on the main thread.
                unsafe { f() }
            }
        }
    }

    /// Publish `hz` (one frequency per key) as the master's table — nothing unless Cantus is
    /// the master. Audio thread too: one copy into the library's table.
    pub fn publish(&self, hz: &[f64; KEYS]) {
        if !self.is_master() {
            return;
        }
        if let Some(f) = self.f.set_table {
            // SAFETY: the library copies 128 doubles from a pointer valid for the call.
            unsafe { f(hz.as_ptr()) }
        }
    }

    /// Which keys clients should not play (a keyboard map's unmapped keys), when master
    /// (main thread).
    pub fn filter(&self, unmapped: &[bool; KEYS]) {
        if !self.is_master() {
            return;
        }
        let (Some(clear), Some(set)) = (self.f.clear_filter, self.f.filter_note) else {
            return;
        };
        // SAFETY: library calls by value; channel −1 = every channel.
        unsafe {
            clear();
            for (k, off) in unmapped.iter().enumerate() {
                if *off {
                    set(true, k as c_char, -1);
                }
            }
        }
    }

    /// The scale's name the master shows its clients (main thread).
    pub fn set_scale_name(&self, name: &str) {
        if !self.is_master() {
            return;
        }
        let (Some(f), Ok(c)) = (self.f.set_scale_name, CString::new(name)) else {
            return;
        };
        // SAFETY: a NUL-terminated string valid for the call; the library copies it.
        unsafe { f(c.as_ptr()) }
    }

    /// How many clients are connected, when the library says.
    pub fn clients(&self) -> Option<i32> {
        // SAFETY: a library query taking nothing.
        self.f.clients.map(|f| unsafe { f() })
    }
}

impl Mts {
    /// A test library's own lever: its export `name`, of type `fn(bool)`, called with `on`
    /// (`false` when it has none). The fake `LIBMTS` of the tests uses it to play another
    /// program; the real library has no such export.
    #[doc(hidden)]
    pub fn lever(&self, name: &str, on: bool) -> bool {
        let Ok(lib) = sys::Lib::open(&self.path) else {
            return false;
        };
        // SAFETY: the test library's levers are `extern "C" fn(bool)`.
        let f: Option<unsafe extern "C" fn(bool)> = unsafe { lib.get(name) };
        std::mem::forget(lib);
        match f {
            // SAFETY: as above.
            Some(f) => {
                unsafe { f(on) };
                true
            }
            None => false,
        }
    }
}

impl Drop for Mts {
    fn drop(&mut self) {
        self.resign_master();
        self.deregister_client();
    }
}

#[cfg(windows)]
mod sys {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

    pub struct Lib(*mut c_void);

    impl Lib {
        pub fn open(path: &std::path::Path) -> Result<Lib, String> {
            let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            // SAFETY: a NUL-terminated wide path valid for the call.
            let h = unsafe { LoadLibraryW(wide.as_ptr()) };
            if h.is_null() {
                return Err(std::io::Error::last_os_error().to_string());
            }
            Ok(Lib(h))
        }

        pub fn symbol(&self, name: &str) -> *const c_void {
            let Ok(c) = std::ffi::CString::new(name) else {
                return std::ptr::null();
            };
            // SAFETY: a loaded module and a NUL-terminated name.
            match unsafe { GetProcAddress(self.0, c.as_ptr().cast()) } {
                Some(f) => f as *const c_void,
                None => std::ptr::null(),
            }
        }

        /// The export `name` as a function of type `F` (`None` when absent).
        ///
        /// # Safety
        /// `F` must be the export's true function-pointer type.
        pub unsafe fn get<F: Copy>(&self, name: &str) -> Option<F> {
            let p = self.symbol(name);
            // SAFETY: the caller names the true type; a function pointer is pointer-sized.
            (!p.is_null()).then(|| unsafe { std::mem::transmute_copy::<*const c_void, F>(&p) })
        }
    }
}

#[cfg(unix)]
mod sys {
    use std::ffi::c_void;
    use std::os::unix::ffi::OsStrExt;

    pub struct Lib(*mut c_void);

    impl Lib {
        pub fn open(path: &std::path::Path) -> Result<Lib, String> {
            let c = std::ffi::CString::new(path.as_os_str().as_bytes())
                .map_err(|e| e.to_string())?;
            // SAFETY: a NUL-terminated path valid for the call.
            let h = unsafe { libc::dlopen(c.as_ptr(), libc::RTLD_NOW) };
            if h.is_null() {
                // SAFETY: `dlerror` returns a string the system owns, or null.
                let e = unsafe { libc::dlerror() };
                return Err(if e.is_null() {
                    "dlopen failed".into()
                } else {
                    // SAFETY: non-null and NUL-terminated, copied now.
                    unsafe { std::ffi::CStr::from_ptr(e) }
                        .to_string_lossy()
                        .into_owned()
                });
            }
            Ok(Lib(h))
        }

        pub fn symbol(&self, name: &str) -> *const c_void {
            let Ok(c) = std::ffi::CString::new(name) else {
                return std::ptr::null();
            };
            // SAFETY: a loaded library and a NUL-terminated name.
            unsafe { libc::dlsym(self.0, c.as_ptr()) as *const c_void }
        }

        /// The export `name` as a function of type `F` (`None` when absent).
        ///
        /// # Safety
        /// `F` must be the export's true function-pointer type.
        pub unsafe fn get<F: Copy>(&self, name: &str) -> Option<F> {
            let p = self.symbol(name);
            // SAFETY: the caller names the true type; a function pointer is pointer-sized.
            (!p.is_null()).then(|| unsafe { std::mem::transmute_copy::<*const c_void, F>(&p) })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_system_odd_sound_ships_for_names_its_place() {
        if let Some(p) = standard_path() {
            let name = p.file_name().unwrap().to_string_lossy().to_lowercase();
            assert!(name.starts_with("libmts"), "{name}");
        }
        assert!(cfg!(not(any(windows, target_os = "macos", target_os = "linux"))) || standard_path().is_some());
    }

    #[test]
    fn a_missing_file_is_not_installed() {
        let p = std::env::temp_dir().join("cantus-no-such-libmts.bin");
        assert_eq!(Mts::open(&p).unwrap_err(), LoadError::NotInstalled(p.clone()));
    }
}
