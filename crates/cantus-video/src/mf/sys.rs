//! Media Foundation's functions, loaded when first needed — never imported.
//!
//! Windows N (without the Media Feature Pack) and Windows Server without its Media
//! Foundation feature have no `mfplat.dll`. A program that imports a function from it does
//! not start at all there ("MFPlat.DLL was not found"), so Cantus must not: the few
//! functions used are looked up in `System32` the first time a video is exported, and their
//! absence is [`VideoError::NoEncoder`] — the export dialog says how to add the free pack.
//! (COM methods are calls through the objects' tables and need nothing of this.)

use crate::VideoError;
use std::ffi::c_void;
use std::sync::OnceLock;
use windows::core::{Interface, HRESULT, PCSTR, PCWSTR};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Media::MediaFoundation::{
    IMFAttributes, IMFMediaBuffer, IMFMediaType, IMFSample, IMFSourceReader,
};
use windows::Win32::System::LibraryLoader::{
    GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32,
};

type Startup = unsafe extern "system" fn(u32, u32) -> HRESULT;
type Shutdown = unsafe extern "system" fn() -> HRESULT;
type CreateOut = unsafe extern "system" fn(*mut *mut c_void) -> HRESULT;
type CreateBuffer = unsafe extern "system" fn(u32, *mut *mut c_void) -> HRESULT;
type CreateReader = unsafe extern "system" fn(PCWSTR, *mut c_void, *mut *mut c_void) -> HRESULT;

/// The functions, from `mfplat.dll` (and `mfreadwrite.dll` for reading files back).
pub struct Api {
    startup: Startup,
    shutdown: Shutdown,
    create_media_type: CreateOut,
    create_memory_buffer: CreateBuffer,
    create_sample: CreateOut,
    create_reader: Option<CreateReader>,
}

fn load(dll: &str) -> Option<HMODULE> {
    let wide: Vec<u16> = dll.encode_utf16().chain([0]).collect();
    // SAFETY: a NUL-terminated name, looked up in System32 only (never the current folder).
    unsafe { LoadLibraryExW(PCWSTR(wide.as_ptr()), None, LOAD_LIBRARY_SEARCH_SYSTEM32) }.ok()
}

/// The address of `name` in `module`, as a function of type `T`.
///
/// # Safety
/// `T` must be the function's true signature.
unsafe fn symbol<T: Copy>(module: HMODULE, name: &str) -> Option<T> {
    let c: Vec<u8> = name.bytes().chain([0]).collect();
    // SAFETY: a NUL-terminated name in a module that stays loaded for the process.
    let f = unsafe { GetProcAddress(module, PCSTR(c.as_ptr())) }?;
    debug_assert_eq!(std::mem::size_of::<T>(), std::mem::size_of_val(&f));
    // SAFETY: the caller states `T` is the function's signature; both are code pointers.
    Some(unsafe { std::mem::transmute_copy(&f) })
}

fn find() -> Option<Api> {
    let plat = load("mfplat.dll")?;
    // SAFETY: each type is the documented signature of the function it is looked up as.
    unsafe {
        Some(Api {
            startup: symbol(plat, "MFStartup")?,
            shutdown: symbol(plat, "MFShutdown")?,
            create_media_type: symbol(plat, "MFCreateMediaType")?,
            create_memory_buffer: symbol(plat, "MFCreateMemoryBuffer")?,
            create_sample: symbol(plat, "MFCreateSample")?,
            create_reader: load("mfreadwrite.dll")
                .and_then(|rw| symbol(rw, "MFCreateSourceReaderFromURL")),
        })
    }
}

/// Media Foundation, if this system has it.
pub fn api() -> Result<&'static Api, VideoError> {
    static API: OnceLock<Option<Api>> = OnceLock::new();
    API.get_or_init(find)
        .as_ref()
        .ok_or(VideoError::NoEncoder("H.264"))
}

/// An interface from a function's out-pointer.
///
/// # Safety
/// `raw` must be null or an owned reference to an object implementing `T`.
unsafe fn owned<T: Interface>(hr: HRESULT, raw: *mut c_void) -> windows::core::Result<T> {
    hr.ok()?;
    if raw.is_null() {
        return Err(windows::core::Error::from(
            windows::Win32::Foundation::E_POINTER,
        ));
    }
    // SAFETY: the function succeeded and handed over one reference.
    Ok(unsafe { T::from_raw(raw) })
}

impl Api {
    pub fn startup(&self, version: u32, flags: u32) -> windows::core::Result<()> {
        // SAFETY: MFStartup's signature.
        unsafe { (self.startup)(version, flags) }.ok()
    }

    pub fn shutdown(&self) -> windows::core::Result<()> {
        // SAFETY: MFShutdown's signature.
        unsafe { (self.shutdown)() }.ok()
    }

    pub fn media_type(&self) -> windows::core::Result<IMFMediaType> {
        let mut p = std::ptr::null_mut();
        // SAFETY: MFCreateMediaType's signature; the out-pointer becomes ours.
        unsafe { owned((self.create_media_type)(&mut p), p) }
    }

    pub fn memory_buffer(&self, bytes: u32) -> windows::core::Result<IMFMediaBuffer> {
        let mut p = std::ptr::null_mut();
        // SAFETY: MFCreateMemoryBuffer's signature; the out-pointer becomes ours.
        unsafe { owned((self.create_memory_buffer)(bytes, &mut p), p) }
    }

    pub fn sample(&self) -> windows::core::Result<IMFSample> {
        let mut p = std::ptr::null_mut();
        // SAFETY: MFCreateSample's signature; the out-pointer becomes ours.
        unsafe { owned((self.create_sample)(&mut p), p) }
    }

    /// A source reader of the file at `path` (Windows' own demuxer and decoders).
    pub fn source_reader(
        &self,
        path: &std::path::Path,
        attributes: Option<&IMFAttributes>,
    ) -> windows::core::Result<IMFSourceReader> {
        let f = self
            .create_reader
            .ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_NOTIMPL))?;
        let wide: Vec<u16> = path
            .as_os_str()
            .to_string_lossy()
            .encode_utf16()
            .chain([0])
            .collect();
        let attrs = attributes.map_or(std::ptr::null_mut(), |a| a.as_raw());
        let mut p = std::ptr::null_mut();
        // SAFETY: MFCreateSourceReaderFromURL's signature; a NUL-terminated path; the
        // attributes borrowed for the call; the out-pointer becomes ours.
        unsafe { owned(f(PCWSTR(wide.as_ptr()), attrs, &mut p), p) }
    }
}
