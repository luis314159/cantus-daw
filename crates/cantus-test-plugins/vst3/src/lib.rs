//! Minimal VST3 plug-ins, built from source in this repository, for `cantus-host`'s tests.
//! Nothing here is shipped; nothing third-party is loaded by the tests.
//!
//! - **Cantus Test Gain** (`gain.rs`): a processor and a *separate* controller class,
//!   connected through `IConnectionPoint`. Two parameters — Gain (−48…+48 dB, 0 dB at
//!   normalized 0.5, applied sample-accurately) and Latency (0…4096 samples, 4096 steps; a
//!   pure delay the plug-in reports, announced with `restartComponent(kLatencyChanged)`
//!   when the controller learns of a change). A stereo main bus and a stereo sidechain
//!   (aux input). Component and controller state. An editor (`IPlugView`) that draws
//!   nothing but keeps its parent, frame and size.
//! - **Cantus Test Pulse** (`pulse.rs`): a *single-component* instrument (the component
//!   is also its controller) with an event input. A note-on writes its velocity into the
//!   left channel at its sample offset, a note-off the negative velocity; a note expression
//!   or poly pressure writes its value into the right channel. It records the last
//!   `ProcessContext` it saw.
//!
//! Both carry a hidden **Fault** parameter and a hidden read-only **Process** one (round 29,
//! `fault.rs`): failures on purpose and the id of the process they run in, for the host's
//! isolation tests.
//!
//! Extra C exports (`cantus_test_*`) let the tests look inside: live objects, driving the
//! controller as a user would, the last transport.

#![allow(non_snake_case)]
#![allow(clippy::missing_safety_doc)]
// The SDK's untyped enums are `int` on Windows and `unsigned int` elsewhere, so the casts
// between them and the `int32` fields are no-ops here and needed there.
#![allow(clippy::unnecessary_cast)]

mod fault;
mod gain;
mod pulse;

use std::ffi::{c_char, c_void};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Mutex;

use vst3::Steinberg::Vst::TChar;
use vst3::Steinberg::{
    kInvalidArgument, kResultFalse, kResultOk, tresult, FIDString, FUnknown, IPluginFactory,
    IPluginFactory2Trait, IPluginFactory3, IPluginFactory3Trait, IPluginFactoryTrait, PClassInfo,
    PClassInfo2, PClassInfoW, PClassInfo_::ClassCardinality_, PFactoryInfo,
    PFactoryInfo_::FactoryFlags_, TUID,
};
use vst3::{Class, ComWrapper};

pub const VENDOR: &str = "Cantus Tests";
pub const VERSION: &str = "1.0.0";
pub const SDK: &str = "VST 3.8.0";

/// COM objects of this module alive now (factory, processors, controllers, views). The
/// host's load/unload test asserts it returns to where it started.
pub static LIVE: AtomicI32 = AtomicI32::new(0);

/// Counts one live object for as long as it exists.
pub struct Live;

impl Live {
    pub fn new() -> Live {
        LIVE.fetch_add(1, Ordering::SeqCst);
        Live
    }
}

impl Default for Live {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        LIVE.fetch_sub(1, Ordering::SeqCst);
    }
}

pub fn copy_cstring(src: &str, dst: &mut [c_char]) {
    let n = src.len().min(dst.len().saturating_sub(1));
    for (d, s) in dst.iter_mut().zip(src.bytes().take(n)) {
        *d = s as c_char;
    }
    if let Some(end) = dst.get_mut(n) {
        *end = 0;
    }
}

pub fn copy_wstring(src: &str, dst: &mut [TChar]) {
    let mut n = 0;
    for unit in src.encode_utf16() {
        if n + 1 >= dst.len() {
            break;
        }
        dst[n] = unit;
        n += 1;
    }
    if let Some(end) = dst.get_mut(n) {
        *end = 0;
    }
}

/// Read and write the little-endian numbers the states are made of.
pub mod bytes {
    use vst3::ComRef;
    use vst3::Steinberg::{kResultOk, IBStream, IBStreamTrait};

    pub unsafe fn write(stream: *mut IBStream, data: &[u8]) -> bool {
        let Some(s) = ComRef::from_raw(stream) else {
            return false;
        };
        let mut written = 0;
        s.write(data.as_ptr() as *mut _, data.len() as i32, &mut written) == kResultOk
            && written == data.len() as i32
    }

    pub unsafe fn read<const N: usize>(stream: *mut IBStream) -> Option<[u8; N]> {
        let s = ComRef::from_raw(stream)?;
        let mut buf = [0u8; N];
        let mut read = 0;
        (s.read(buf.as_mut_ptr() as *mut _, N as i32, &mut read) == kResultOk && read == N as i32)
            .then_some(buf)
    }

    pub unsafe fn read_f64(stream: *mut IBStream) -> Option<f64> {
        read::<8>(stream).map(f64::from_le_bytes)
    }
}

struct Factory {
    _live: Live,
}

impl Class for Factory {
    type Interfaces = (IPluginFactory3,);
}

/// Every class this module makes: (cid, category, name, sub-categories).
fn classes() -> [(TUID, &'static str, &'static str, &'static str); 3] {
    [
        (
            gain::PROCESSOR_CID,
            "Audio Module Class",
            gain::NAME,
            "Fx|Dynamics",
        ),
        (
            gain::CONTROLLER_CID,
            "Component Controller Class",
            gain::NAME,
            "",
        ),
        (
            pulse::CID,
            "Audio Module Class",
            pulse::NAME,
            "Instrument|Synth",
        ),
    ]
}

impl IPluginFactoryTrait for Factory {
    unsafe fn getFactoryInfo(&self, info: *mut PFactoryInfo) -> tresult {
        let info = &mut *info;
        copy_cstring(VENDOR, &mut info.vendor);
        copy_cstring("https://example.invalid", &mut info.url);
        copy_cstring("tests@example.invalid", &mut info.email);
        info.flags = FactoryFlags_::kUnicode as i32;
        kResultOk
    }

    unsafe fn countClasses(&self) -> i32 {
        classes().len() as i32
    }

    unsafe fn getClassInfo(&self, index: i32, info: *mut PClassInfo) -> tresult {
        let Some(&(cid, category, name, _)) = classes().get(index as usize) else {
            return kInvalidArgument;
        };
        let info = &mut *info;
        info.cid = cid;
        info.cardinality = ClassCardinality_::kManyInstances as i32;
        copy_cstring(category, &mut info.category);
        copy_cstring(name, &mut info.name);
        kResultOk
    }

    unsafe fn createInstance(
        &self,
        cid: FIDString,
        iid: FIDString,
        obj: *mut *mut c_void,
    ) -> tresult {
        let cid = *(cid as *const TUID);
        let instance = if cid == gain::PROCESSOR_CID {
            ComWrapper::new(gain::Processor::new()).to_com_ptr::<FUnknown>()
        } else if cid == gain::CONTROLLER_CID {
            ComWrapper::new(gain::Controller::new()).to_com_ptr::<FUnknown>()
        } else if cid == pulse::CID {
            ComWrapper::new(pulse::Pulse::new()).to_com_ptr::<FUnknown>()
        } else {
            None
        };
        match instance {
            Some(i) => {
                let ptr = i.as_ptr();
                ((*(*ptr).vtbl).queryInterface)(ptr, iid as *const TUID, obj)
            }
            None => kInvalidArgument,
        }
    }
}

impl IPluginFactory2Trait for Factory {
    unsafe fn getClassInfo2(&self, index: i32, info: *mut PClassInfo2) -> tresult {
        let Some(&(cid, category, name, sub)) = classes().get(index as usize) else {
            return kInvalidArgument;
        };
        let info = &mut *info;
        info.cid = cid;
        info.cardinality = ClassCardinality_::kManyInstances as i32;
        copy_cstring(category, &mut info.category);
        copy_cstring(name, &mut info.name);
        info.classFlags = 0;
        copy_cstring(sub, &mut info.subCategories);
        copy_cstring(VENDOR, &mut info.vendor);
        copy_cstring(VERSION, &mut info.version);
        copy_cstring(SDK, &mut info.sdkVersion);
        kResultOk
    }
}

impl IPluginFactory3Trait for Factory {
    unsafe fn getClassInfoUnicode(&self, index: i32, info: *mut PClassInfoW) -> tresult {
        let Some(&(cid, category, name, sub)) = classes().get(index as usize) else {
            return kInvalidArgument;
        };
        let info = &mut *info;
        info.cid = cid;
        info.cardinality = ClassCardinality_::kManyInstances as i32;
        copy_cstring(category, &mut info.category);
        copy_wstring(name, &mut info.name);
        info.classFlags = 0;
        copy_cstring(sub, &mut info.subCategories);
        copy_wstring(VENDOR, &mut info.vendor);
        copy_wstring(VERSION, &mut info.version);
        copy_wstring(SDK, &mut info.sdkVersion);
        kResultOk
    }

    unsafe fn setHostContext(&self, _context: *mut FUnknown) -> tresult {
        kResultOk
    }
}

/// `InitDll`/`ExitDll` calls so far: the host must pair them.
pub static MODULE_ENTRIES: AtomicI32 = AtomicI32::new(0);
pub static MODULE_EXITS: AtomicI32 = AtomicI32::new(0);

#[cfg(target_os = "windows")]
#[no_mangle]
extern "system" fn InitDll() -> bool {
    MODULE_ENTRIES.fetch_add(1, Ordering::SeqCst);
    true
}

#[cfg(target_os = "windows")]
#[no_mangle]
extern "system" fn ExitDll() -> bool {
    MODULE_EXITS.fetch_add(1, Ordering::SeqCst);
    true
}

#[cfg(target_os = "linux")]
#[no_mangle]
extern "system" fn ModuleEntry(_handle: *mut c_void) -> bool {
    MODULE_ENTRIES.fetch_add(1, Ordering::SeqCst);
    true
}

#[cfg(target_os = "linux")]
#[no_mangle]
extern "system" fn ModuleExit() -> bool {
    MODULE_EXITS.fetch_add(1, Ordering::SeqCst);
    true
}

#[no_mangle]
extern "system" fn GetPluginFactory() -> *mut IPluginFactory {
    ComWrapper::new(Factory { _live: Live::new() })
        .to_com_ptr::<IPluginFactory>()
        .map_or(std::ptr::null_mut(), |p| p.into_raw())
}

// ---------------------------------------------------------------------------------------
// What the tests read and do from outside.

/// A registry of live objects by address, newest last.
pub struct Registry(Mutex<Vec<usize>>);

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    pub const fn new() -> Registry {
        Registry(Mutex::new(Vec::new()))
    }

    /// Remember a live object.
    pub fn add(&self, at: usize) {
        let mut v = self.0.lock().unwrap_or_else(|p| p.into_inner());
        if !v.contains(&at) {
            v.push(at);
        }
    }

    pub fn remove(&self, at: usize) {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .retain(|&a| a != at);
    }

    pub fn newest(&self) -> Option<usize> {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .last()
            .copied()
    }
}

#[no_mangle]
pub extern "C" fn cantus_test_live_objects() -> i32 {
    LIVE.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn cantus_test_module_entries() -> i32 {
    MODULE_ENTRIES.load(Ordering::SeqCst) - MODULE_EXITS.load(Ordering::SeqCst)
}

/// The newest gain controller does what its editor would when the user drags a knob:
/// `beginEdit`, `performEdit(value)`, `endEdit`. 1 when it had a handler to tell.
#[no_mangle]
pub extern "C" fn cantus_test_perform_edit(id: u32, normalized: f64) -> i32 {
    gain::perform_edit(id, normalized)
}

/// Set (≥ 0) and read the newest gain controller's own state (an editor zoom).
#[no_mangle]
pub extern "C" fn cantus_test_zoom(set: f64) -> f64 {
    gain::zoom(set)
}

/// The newest editor asks its frame to resize it. 1 when it had a frame.
#[no_mangle]
pub extern "C" fn cantus_test_resize_editor(width: i32, height: i32) -> i32 {
    gain::resize_editor(width, height)
}

/// The newest editor's size and parent window (0 when detached).
#[no_mangle]
pub extern "C" fn cantus_test_editor(width: *mut i32, height: *mut i32) -> usize {
    gain::editor(width, height)
}

/// Messages the gain controllers received from their processors through the host's
/// `IMessage`.
#[no_mangle]
pub extern "C" fn cantus_test_messages() -> i32 {
    gain::MESSAGES.load(Ordering::SeqCst)
}

/// The sidechain the gain saw in its last block: its peak, and the silence flags through
/// `silence`.
#[no_mangle]
pub unsafe extern "C" fn cantus_test_key(silence: *mut u64) -> f64 {
    if !silence.is_null() {
        *silence = gain::KEY_SILENCE.load(Ordering::SeqCst);
    }
    f64::from_bits(gain::KEY_PEAK.load(Ordering::SeqCst))
}

/// The last `ProcessContext` the pulse saw: state flags, tempo, musical position, bar.
#[no_mangle]
pub unsafe extern "C" fn cantus_test_last_context(values: *mut f64) -> u32 {
    pulse::last_context(values)
}

pub(crate) fn not_implemented() -> tresult {
    kResultFalse
}
