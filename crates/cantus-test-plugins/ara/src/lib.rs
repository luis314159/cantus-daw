//! **Cantus Test ARA**: a minimal ARA 2 plug-in (VST3), built from source in this repository
//! for the ARA host's tests (ADR 0097). Nothing here is shipped.
//!
//! - The audio processor (`plugin.rs`) is a stereo effect implementing `IPlugInEntryPoint2`:
//!   bound as a playback renderer it renders its playback regions — reading the host's audio
//!   source through the host's reader — at the gain its modification keeps (1 by default:
//!   the source to the bit); unbound it passes its input through.
//! - A factory class of category "ARA Main Factory Class" (`IMainFactory`).
//! - The document controller (`ara.rs`) keeps the graph and **counts every breach of ARA's
//!   call rules**, which the tests assert is zero.
//!
//! C exports (`cantus_test_ara_*`) stand in for what a user would do in the plug-in's editor
//! (set a modification's gain) and let the tests look inside.

#![allow(non_snake_case)]
#![allow(clippy::missing_safety_doc)]
#![allow(clippy::unnecessary_cast)]

pub mod ara;
pub mod plugin;

use std::ffi::{c_char, c_void};
use std::sync::atomic::{AtomicI32, Ordering};

use vst3::Steinberg::Vst::TChar;
use vst3::Steinberg::{
    kInvalidArgument, kResultOk, tresult, FIDString, FUnknown, IPluginFactory,
    IPluginFactory2Trait, IPluginFactory3, IPluginFactory3Trait, IPluginFactoryTrait, PClassInfo,
    PClassInfo2, PClassInfoW, PClassInfo_::ClassCardinality_, PFactoryInfo,
    PFactoryInfo_::FactoryFlags_, TUID,
};
use vst3::{Class, ComWrapper};

pub const VENDOR: &str = "Cantus Tests";
pub const VERSION: &str = "1.0.0";
pub const SDK: &str = "VST 3.8.0";

pub static LIVE: AtomicI32 = AtomicI32::new(0);

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

struct Factory {
    _live: Live,
}

impl Class for Factory {
    type Interfaces = (IPluginFactory3,);
}

fn classes() -> [(TUID, &'static str, &'static str, &'static str); 2] {
    [
        (plugin::CID, "Audio Module Class", plugin::NAME, "Fx"),
        (
            plugin::MAIN_FACTORY_CID,
            cantus_ara::vst3::MAIN_FACTORY_CLASS,
            plugin::NAME,
            "",
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
        let instance = if cid == plugin::CID {
            ComWrapper::new(plugin::Processor::new()).to_com_ptr::<FUnknown>()
        } else if cid == plugin::MAIN_FACTORY_CID {
            ComWrapper::new(plugin::MainFactory::new()).to_com_ptr::<FUnknown>()
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

#[cfg(target_os = "windows")]
#[no_mangle]
extern "system" fn InitDll() -> bool {
    true
}

#[cfg(target_os = "windows")]
#[no_mangle]
extern "system" fn ExitDll() -> bool {
    true
}

#[cfg(target_os = "linux")]
#[no_mangle]
extern "system" fn ModuleEntry(_handle: *mut c_void) -> bool {
    true
}

#[cfg(target_os = "linux")]
#[no_mangle]
extern "system" fn ModuleExit() -> bool {
    true
}

#[cfg(target_os = "macos")]
#[no_mangle]
extern "C" fn bundleEntry(_bundle: *mut c_void) -> bool {
    true
}

#[cfg(target_os = "macos")]
#[no_mangle]
extern "C" fn bundleExit() -> bool {
    true
}

#[no_mangle]
extern "system" fn GetPluginFactory() -> *mut IPluginFactory {
    ComWrapper::new(Factory { _live: Live::new() })
        .to_com_ptr::<IPluginFactory>()
        .map_or(std::ptr::null_mut(), |p| p.into_raw())
}

// --- what the tests do and read ---------------------------------------------------------------

/// Breaches of ARA's call rules so far.
#[no_mangle]
pub extern "C" fn cantus_test_ara_violations() -> i32 {
    ara::VIOLATIONS.load(Ordering::SeqCst)
}

/// What a user does in the editor: set the gain of the modification with this persistent
/// id; the host is told at its next `notifyModelUpdates`. 1 when found.
#[no_mangle]
pub unsafe extern "C" fn cantus_test_ara_set_gain(pid: *const c_char, gain: f32) -> i32 {
    let pid = ara::pid_of(pid);
    ara::with_mod(&pid, |m, k, model| {
        m.gain = gain;
        model.pending.push((k, None));
    })
    .map_or(0, |_| 1)
}

/// The gain of the modification with this persistent id (NaN: none).
#[no_mangle]
pub unsafe extern "C" fn cantus_test_ara_gain(pid: *const c_char) -> f32 {
    ara::with_mod(&ara::pid_of(pid), |m, _, _| m.gain).unwrap_or(f32::NAN)
}

/// Counts over every live controller: [controllers, sources, modifications, active
/// modifications, regions, region sequences, bound instances, tempo entries last read].
#[no_mangle]
pub unsafe extern "C" fn cantus_test_ara_counts(out: *mut i32) {
    let list = ara::CONTROLLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let mut v = [0i32; 8];
    v[0] = list.len() as i32;
    for at in list {
        let c = &*(at as *const ara::Controller);
        let m = c.model();
        v[1] += m.sources.len() as i32;
        v[2] += m.mods.len() as i32;
        v[3] += m.mods.values().filter(|x| x.active).count() as i32;
        v[4] += m.regions.len() as i32;
        v[5] += m.sequences.len() as i32;
        v[7] = v[7].max(m.tempo_entries);
    }
    v[6] = plugin::BOUND.load(Ordering::SeqCst);
    for (i, x) in v.iter().enumerate() {
        *out.add(i) = *x;
    }
}

/// Blocks rendered with a region in them, by every instance.
#[no_mangle]
pub extern "C" fn cantus_test_ara_rendered_blocks() -> u64 {
    plugin::RENDERED_BLOCKS.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn cantus_test_live_objects() -> i32 {
    LIVE.load(Ordering::SeqCst)
}

/// The edit of [`cantus_test_ara_set_gain`], said to the host as changing only modification
/// time [start, end) seconds (what the host renders again).
#[no_mangle]
pub unsafe extern "C" fn cantus_test_ara_set_gain_range(
    pid: *const c_char,
    gain: f32,
    start: f64,
    end: f64,
) -> i32 {
    let pid = ara::pid_of(pid);
    ara::with_mod(&pid, |m, k, model| {
        m.gain = gain;
        model.pending.push((k, Some((start, end))));
    })
    .map_or(0, |_| 1)
}

/// The latency every instance reports from its next activation (samples).
#[no_mangle]
pub extern "C" fn cantus_test_ara_set_latency(samples: u32) {
    plugin::LATENCY.store(samples, Ordering::SeqCst);
}

/// The head and tail time every region reports (seconds).
#[no_mangle]
pub extern "C" fn cantus_test_ara_set_head_tail(head: f64, tail: f64) {
    *ara::HEAD_TAIL.lock().unwrap_or_else(|e| e.into_inner()) = (head, tail);
}

/// The next render aborts the process.
#[no_mangle]
pub extern "C" fn cantus_test_ara_crash_next_render() {
    plugin::CRASH.store(true, Ordering::SeqCst);
}

/// The next render sleeps `ms` first.
#[no_mangle]
pub extern "C" fn cantus_test_ara_hang_next_render(ms: u32) {
    plugin::HANG_MS.store(ms, Ordering::SeqCst);
}

/// Selections the hosts' editor views were told of.
#[no_mangle]
pub extern "C" fn cantus_test_ara_selections() -> i32 {
    plugin::SELECTIONS.load(Ordering::SeqCst)
}

/// Every live controller asks its host to start playback, then to move to `seconds`.
#[no_mangle]
pub unsafe extern "C" fn cantus_test_ara_request_play(seconds: f64) -> i32 {
    let list = ara::CONTROLLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    for at in &list {
        let c = &*(*at as *const ara::Controller);
        let pc = &*c.host.playbackControllerInterface;
        if let Some(f) = pc.requestStartPlayback {
            f(c.host.playbackControllerHostRef);
        }
        if let Some(f) = pc.requestSetPlaybackPosition {
            f(c.host.playbackControllerHostRef, seconds);
        }
    }
    list.len() as i32
}
