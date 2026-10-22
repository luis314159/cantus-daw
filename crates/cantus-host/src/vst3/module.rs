//! Finding a plug-in's binary inside its bundle, loading it once per process, and the
//! module's entry points (`InitDll`/`ExitDll` on Windows, `ModuleEntry`/`ModuleExit` on
//! Linux, `bundleEntry`/`bundleExit` on macOS, `GetPluginFactory` everywhere).
//!
//! A module stays loaded while any instance or scan uses it and is unloaded when the last
//! one lets go: the factory is released first, then `ExitDll`, then the library itself.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};

use ::vst3::Steinberg::{FUnknown, IPluginFactory, IPluginFactory3, IPluginFactory3Trait};
use ::vst3::{ComPtr, ComWrapper};

use super::host_objects::HostApplication;
use crate::backend::HostError;

/// The folder inside `Contents` that holds this platform's binary.
fn arch_dirs() -> &'static [&'static str] {
    if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        &["x86_64-win"]
    } else if cfg!(all(target_os = "windows", target_arch = "aarch64")) {
        &["arm64-win", "arm64ec-win", "arm64x-win"]
    } else if cfg!(all(target_os = "windows", target_arch = "x86")) {
        &["x86-win"]
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        &["x86_64-linux"]
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        &["aarch64-linux"]
    } else if cfg!(all(target_os = "linux", target_arch = "x86")) {
        &["i386-linux"]
    } else if cfg!(target_os = "macos") {
        // One universal binary for every architecture (ADR 0066).
        &["MacOS"]
    } else {
        &[]
    }
}

/// The binary's extension inside a bundle.
fn binary_extension() -> &'static str {
    if cfg!(target_os = "windows") {
        "vst3"
    } else {
        "so"
    }
}

/// The loadable file for `path`: the path itself for a legacy single-file plug-in, or
/// `X.vst3/Contents/<arch>/X.vst3` (`.so` on Linux) for a bundle. A bundle whose binary
/// has another name is accepted when its architecture folder holds exactly one binary.
pub fn binary_path(path: &Path) -> Result<PathBuf, HostError> {
    if path.is_file() {
        return Ok(path.to_path_buf());
    }
    if !path.is_dir() {
        return Err(HostError::Load(format!(
            "{} does not exist",
            path.display()
        )));
    }
    #[cfg(target_os = "macos")]
    if let Some(binary) = crate::bundle::binary(path) {
        return Ok(binary);
    }
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = binary_extension();
    for arch in arch_dirs() {
        let dir = path.join("Contents").join(arch);
        let named = dir.join(format!("{stem}.{ext}"));
        if named.is_file() {
            return Ok(named);
        }
        if let Ok(entries) = std::fs::read_dir(&dir) {
            let found: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.is_file()
                        && p.extension()
                            .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case(ext))
                })
                .collect();
            if let [only] = found.as_slice() {
                return Ok(only.clone());
            }
        }
    }
    Err(HostError::Load(format!(
        "{}: no binary for this platform in the bundle",
        path.display()
    )))
}

/// The bundle folder a path belongs to, when it is (inside) a `.vst3` bundle.
pub fn bundle_root(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|p| {
            p.is_dir()
                && p.extension()
                    .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("vst3"))
        })
        .map(Path::to_path_buf)
}

type EntryFn = unsafe extern "system" fn() -> bool;
type FactoryFn = unsafe extern "system" fn() -> *mut IPluginFactory;

/// One loaded plug-in binary and its factory.
pub struct Module {
    /// Released in `drop` before `ExitDll` and the unload.
    factory: Option<ComPtr<IPluginFactory>>,
    exit: Option<EntryFn>,
    library: Option<libloading::Library>,
    /// The host context handed to the factory and to every instance made from it.
    pub host: ComWrapper<HostApplication>,
    pub binary: PathBuf,
}

impl Module {
    pub fn factory(&self) -> &ComPtr<IPluginFactory> {
        self.factory
            .as_ref()
            .expect("the factory lives as long as the module")
    }

    /// The host context as the `FUnknown*` that `initialize` takes.
    pub fn host_context(&self) -> *mut FUnknown {
        self.host
            .as_com_ref::<FUnknown>()
            .map_or(std::ptr::null_mut(), |r| r.as_ptr())
    }

    fn load(binary: &Path) -> Result<Module, HostError> {
        let err = |what: &str| HostError::Load(format!("{}: {what}", binary.display()));
        let library = open_library(binary).map_err(|e| err(&e.to_string()))?;
        // SAFETY: the symbols are the VST3 module entry points, whose signatures the SDK
        // fixes; a missing optional one is simply not called.
        unsafe {
            if !enter(&library, binary) {
                return Err(err("its module entry point refused to start"));
            }
            let exit = exit_point(&library);
            let get_factory: libloading::Symbol<FactoryFn> =
                match library.get(b"GetPluginFactory\0") {
                    Ok(f) => f,
                    Err(_) => {
                        if let Some(exit) = exit {
                            exit();
                        }
                        return Err(err("not a VST3 module (no GetPluginFactory)"));
                    }
                };
            let Some(factory) = ComPtr::from_raw(get_factory()) else {
                if let Some(exit) = exit {
                    exit();
                }
                return Err(err("GetPluginFactory returned nothing"));
            };
            let host = ComWrapper::new(HostApplication::new());
            if let Some(f3) = factory.cast::<IPluginFactory3>() {
                if let Some(ctx) = host.as_com_ref::<FUnknown>() {
                    f3.setHostContext(ctx.as_ptr());
                }
            }
            Ok(Module {
                factory: Some(factory),
                exit,
                library: Some(library),
                host,
                binary: binary.to_path_buf(),
            })
        }
    }
}

impl Drop for Module {
    fn drop(&mut self) {
        // Order matters: nothing of the plug-in's may outlive its code.
        self.factory = None;
        if let Some(exit) = self.exit {
            // SAFETY: `ExitDll`/`ModuleExit` pairs the entry call made in `load`.
            unsafe {
                exit();
            }
        }
        self.library = None;
    }
}

#[cfg(target_os = "windows")]
fn open_library(path: &Path) -> Result<libloading::Library, libloading::Error> {
    use libloading::os::windows::{Library, LOAD_WITH_ALTERED_SEARCH_PATH};
    // SAFETY: loading a plug-in runs its initialisers; that is what hosting it means. The
    // altered search path lets it find the DLLs it ships next to itself.
    unsafe { Library::load_with_flags(path, LOAD_WITH_ALTERED_SEARCH_PATH).map(Into::into) }
}

#[cfg(not(target_os = "windows"))]
fn open_library(path: &Path) -> Result<libloading::Library, libloading::Error> {
    // SAFETY: as above.
    unsafe { libloading::Library::new(path) }
}

/// Call the module's entry point (`InitDll` on Windows, `ModuleEntry` on Linux) when it
/// exports one; `false` when it refuses to start.
///
/// # Safety
/// `library` must be a VST3 module.
#[cfg(target_os = "windows")]
unsafe fn enter(library: &libloading::Library, _binary: &Path) -> bool {
    match library.get::<EntryFn>(b"InitDll\0") {
        Ok(init) => init(),
        Err(_) => true,
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
unsafe fn enter(library: &libloading::Library, _binary: &Path) -> bool {
    // `ModuleEntry` takes the library handle, which the SDK's own modules only keep.
    type ModuleEntryFn = unsafe extern "system" fn(*mut std::ffi::c_void) -> bool;
    match library.get::<ModuleEntryFn>(b"ModuleEntry\0") {
        Ok(entry) => entry(std::ptr::null_mut()),
        Err(_) => true,
    }
}

/// macOS: `bundleEntry(CFBundleRef)` with the plug-in's bundle (the SDK's macOS entry; a
/// JUCE or iPlug plug-in finds its resources through that bundle). The bundle object is
/// kept for the life of the process: CoreFoundation makes one per bundle path and hands the
/// same one back, so a module loaded again costs nothing more.
#[cfg(target_os = "macos")]
unsafe fn enter(library: &libloading::Library, binary: &Path) -> bool {
    type BundleEntryFn = unsafe extern "C" fn(*mut std::ffi::c_void) -> bool;
    let Ok(entry) = library.get::<BundleEntryFn>(b"bundleEntry\0") else {
        return true;
    };
    let root = bundle_root(binary).unwrap_or_else(|| binary.to_path_buf());
    entry(cf::bundle(&root))
}

#[cfg(target_os = "macos")]
mod cf {
    use std::ffi::c_void;
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFURLCreateFromFileSystemRepresentation(
            alloc: *const c_void,
            buffer: *const u8,
            len: isize,
            is_directory: u8,
        ) -> *const c_void;
        fn CFBundleCreate(alloc: *const c_void, url: *const c_void) -> *mut c_void;
        fn CFRelease(cf: *const c_void);
    }

    /// A `CFBundleRef` for the folder (null if CoreFoundation cannot make one), retained
    /// for good.
    pub fn bundle(path: &Path) -> *mut c_void {
        let bytes = path.as_os_str().as_bytes();
        // SAFETY: CoreFoundation's create rule: the URL is ours and released once; the
        // bundle is kept (see `enter`).
        unsafe {
            let url = CFURLCreateFromFileSystemRepresentation(
                std::ptr::null(),
                bytes.as_ptr(),
                bytes.len() as isize,
                1,
            );
            if url.is_null() {
                return std::ptr::null_mut();
            }
            let bundle = CFBundleCreate(std::ptr::null(), url);
            CFRelease(url);
            bundle
        }
    }
}

/// The module's exit point (`ExitDll` / `ModuleExit`), when exported.
///
/// # Safety
/// `library` must be a VST3 module.
unsafe fn exit_point(library: &libloading::Library) -> Option<EntryFn> {
    let name: &[u8] = if cfg!(target_os = "windows") {
        b"ExitDll\0"
    } else if cfg!(target_os = "macos") {
        b"bundleExit\0"
    } else {
        b"ModuleExit\0"
    };
    library.get::<EntryFn>(name).ok().map(|s| *s)
}

/// Modules loaded now, by binary path. Main thread (and the scanner) only.
static LOADED: Mutex<Vec<(PathBuf, Weak<Module>)>> = Mutex::new(Vec::new());

/// Load the module for a plug-in path (bundle or file), or share the one already loaded.
pub fn open(path: &Path) -> Result<Arc<Module>, HostError> {
    let binary = binary_path(path)?;
    let key = std::fs::canonicalize(&binary).unwrap_or_else(|_| binary.clone());
    let mut loaded = LOADED.lock().unwrap_or_else(|p| p.into_inner());
    loaded.retain(|(_, m)| m.strong_count() > 0);
    if let Some(m) = loaded
        .iter()
        .find(|(p, _)| *p == key)
        .and_then(|(_, m)| m.upgrade())
    {
        return Ok(m);
    }
    let module = Arc::new(Module::load(&binary)?);
    loaded.push((key, Arc::downgrade(&module)));
    Ok(module)
}
