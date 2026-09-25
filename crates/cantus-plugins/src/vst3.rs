//! The VST3 entry point: `GetPluginFactory`, a factory listing the plug-ins this binary
//! carries, and one COM class per instance (`plugin.rs`).
//!
//! Written on the `vst3` crate's bindings (generated from the MIT-licensed VST 3.8 SDK
//! headers). Each plug-in is a **single component**: one object is both the processor
//! (`IComponent`, `IAudioProcessor`) and the edit controller (`IEditController`,
//! `IUnitInfo`), as nih-plug and most Rust wrappers do; `getControllerClassId` says there
//! is no separate controller, and every major host then asks the component itself.

use crate::desc::{self, Descriptor};
use ::vst3::Steinberg::Vst::*;
use ::vst3::Steinberg::*;
use ::vst3::{uid, Class, ComWrapper};
use std::ffi::{c_char, c_void};

mod plugin;
mod timer;
#[cfg(all(feature = "editor", windows))]
mod view;

pub use plugin::Plugin;

/// The SDK version the bindings come from, as a host reads it.
pub const SDK_VERSION: &str = "VST 3.8.0";

/// A class id as `TUID` bytes, in the platform's order (`vst3::uid`).
pub fn cid_of(d: &Descriptor) -> TUID {
    let [a, b, c, e] = d.vst3_cid;
    uid(a, b, c, e)
}

pub(crate) fn copy_cstr(dst: &mut [c_char], s: &str) {
    if dst.is_empty() {
        return;
    }
    let mut end = s.len().min(dst.len() - 1);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    for (d, b) in dst.iter_mut().zip(&s.as_bytes()[..end]) {
        *d = *b as c_char;
    }
    dst[end] = 0;
}

pub(crate) fn copy_wstr(dst: &mut [TChar], s: &str) {
    if dst.is_empty() {
        return;
    }
    let mut n = 0;
    let room = dst.len() - 1;
    for (d, u) in dst.iter_mut().take(room).zip(s.encode_utf16()) {
        *d = u as TChar;
        n += 1;
    }
    dst[n] = 0;
}

/// A host's zero-terminated UTF-16 string.
///
/// # Safety
/// `s` must be null or point to a zero-terminated UTF-16 string.
pub(crate) unsafe fn read_wstr(s: *const TChar) -> Option<String> {
    if s.is_null() {
        return None;
    }
    let mut len = 0;
    while *s.add(len) != 0 && len < 4096 {
        len += 1;
    }
    String::from_utf16(std::slice::from_raw_parts(s, len)).ok()
}

struct Factory;

impl Class for Factory {
    type Interfaces = (IPluginFactory3,);
}

fn class(index: i32) -> Option<&'static Descriptor> {
    usize::try_from(index)
        .ok()
        .and_then(|i| desc::compiled().nth(i))
}

impl IPluginFactoryTrait for Factory {
    unsafe fn getFactoryInfo(&self, info: *mut PFactoryInfo) -> tresult {
        if info.is_null() {
            return kInvalidArgument;
        }
        let info = &mut *info;
        copy_cstr(&mut info.vendor, desc::VENDOR);
        copy_cstr(&mut info.url, desc::URL);
        copy_cstr(&mut info.email, desc::EMAIL);
        info.flags = PFactoryInfo_::FactoryFlags_::kUnicode as int32;
        kResultOk
    }

    unsafe fn countClasses(&self) -> int32 {
        desc::compiled().count() as int32
    }

    unsafe fn getClassInfo(&self, index: int32, info: *mut PClassInfo) -> tresult {
        let (Some(d), false) = (class(index), info.is_null()) else {
            return kInvalidArgument;
        };
        let info = &mut *info;
        info.cid = cid_of(d);
        info.cardinality = PClassInfo_::ClassCardinality_::kManyInstances as int32;
        copy_cstr(&mut info.category, "Audio Module Class");
        copy_cstr(&mut info.name, d.full_name);
        kResultOk
    }

    unsafe fn createInstance(
        &self,
        cid: FIDString,
        iid: FIDString,
        obj: *mut *mut c_void,
    ) -> tresult {
        if cid.is_null() || iid.is_null() || obj.is_null() {
            return kInvalidArgument;
        }
        let wanted = *(cid as *const TUID);
        let Some(d) = desc::compiled().find(|d| cid_of(d) == wanted) else {
            return kInvalidArgument;
        };
        let instance = ComWrapper::new(Plugin::new(d));
        let Some(unknown) = instance.to_com_ptr::<FUnknown>() else {
            return kNoInterface;
        };
        let ptr = unknown.as_ptr();
        ((*(*ptr).vtbl).queryInterface)(ptr, iid as *const TUID, obj)
    }
}

impl IPluginFactory2Trait for Factory {
    unsafe fn getClassInfo2(&self, index: int32, info: *mut PClassInfo2) -> tresult {
        let (Some(d), false) = (class(index), info.is_null()) else {
            return kInvalidArgument;
        };
        let info = &mut *info;
        info.cid = cid_of(d);
        info.cardinality = PClassInfo_::ClassCardinality_::kManyInstances as int32;
        copy_cstr(&mut info.category, "Audio Module Class");
        copy_cstr(&mut info.name, d.full_name);
        info.classFlags = 0;
        copy_cstr(&mut info.subCategories, d.vst3_categories);
        copy_cstr(&mut info.vendor, desc::VENDOR);
        copy_cstr(&mut info.version, desc::VERSION);
        copy_cstr(&mut info.sdkVersion, SDK_VERSION);
        kResultOk
    }
}

impl IPluginFactory3Trait for Factory {
    unsafe fn getClassInfoUnicode(&self, index: int32, info: *mut PClassInfoW) -> tresult {
        let (Some(d), false) = (class(index), info.is_null()) else {
            return kInvalidArgument;
        };
        let info = &mut *info;
        info.cid = cid_of(d);
        info.cardinality = PClassInfo_::ClassCardinality_::kManyInstances as int32;
        copy_cstr(&mut info.category, "Audio Module Class");
        copy_wstr(&mut info.name, d.full_name);
        info.classFlags = 0;
        copy_cstr(&mut info.subCategories, d.vst3_categories);
        copy_wstr(&mut info.vendor, desc::VENDOR);
        copy_wstr(&mut info.version, desc::VERSION);
        copy_wstr(&mut info.sdkVersion, SDK_VERSION);
        kResultOk
    }

    unsafe fn setHostContext(&self, _context: *mut FUnknown) -> tresult {
        kResultOk
    }
}

/// The symbol every VST3 host looks for.
///
/// # Safety
/// Called by a host through the C ABI.
#[no_mangle]
pub unsafe extern "system" fn GetPluginFactory() -> *mut IPluginFactory {
    ComWrapper::new(Factory)
        .to_com_ptr::<IPluginFactory>()
        .map_or(std::ptr::null_mut(), |p| p.into_raw())
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

#[cfg(target_os = "macos")]
#[no_mangle]
extern "system" fn BundleEntry(_bundle: *mut c_void) -> bool {
    true
}

#[cfg(target_os = "macos")]
#[no_mangle]
extern "system" fn BundleExit() -> bool {
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

/// `moduleinfo.json` for a bundle holding `d` (VST 3.7.5+ hosts read it instead of
/// loading the binary to scan it).
pub fn moduleinfo_json(d: &Descriptor) -> String {
    let info = serde_json::json!({
        "Name": d.full_name,
        "Version": desc::VERSION,
        "Factory Info": {
            "Vendor": desc::VENDOR,
            "URL": desc::URL,
            "E-Mail": desc::EMAIL,
            "Flags": {
                "Unicode": true,
                "Classes Discardable": false,
                "Component Non Discardable": false,
            },
        },
        "Compatibility": [],
        "Classes": [{
            "CID": d.vst3_cid_hex(),
            "Category": "Audio Module Class",
            "Name": d.full_name,
            "Vendor": desc::VENDOR,
            "Version": desc::VERSION,
            "SDKVersion": SDK_VERSION,
            "Sub Categories": d.vst3_categories.split('|').collect::<Vec<_>>(),
            "Class Flags": 0,
            "Cardinality": 2147483647,
            "Snapshots": [],
        }],
    });
    serde_json::to_string_pretty(&info).unwrap_or_default()
}
