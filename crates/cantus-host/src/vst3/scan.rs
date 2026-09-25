//! Reading a module's factory: every "Audio Module Class" with its names, category words
//! and, by instantiating it, its buses and whether it has an editor.

use std::path::Path;
use std::sync::Arc;

use ::vst3::ComPtr;
use ::vst3::Steinberg::Vst::BusDirections_::{kInput, kOutput};
use ::vst3::Steinberg::Vst::BusTypes_::{kAux, kMain};
use ::vst3::Steinberg::Vst::MediaTypes_::{kAudio, kEvent};
use ::vst3::Steinberg::Vst::{BusInfo, IComponent, IComponentTrait};
use ::vst3::Steinberg::{
    IPluginFactory2, IPluginFactory2Trait, IPluginFactory3, IPluginFactory3Trait,
    IPluginFactoryTrait, PClassInfo, PClassInfo2, PClassInfoW, PFactoryInfo, TUID,
};

use super::instance::Parts;
use super::module::Module;
use super::util::{from_char16, from_char8, ok, tuid_to_string};
use crate::backend::{Format, PluginInfo, PluginKind};

/// The factory category of a processor component.
pub const AUDIO_MODULE_CLASS: &str = "Audio Module Class";

/// What the sub-categories say a plug-in is for.
pub fn kind_from_subcategories(features: &[String]) -> PluginKind {
    let has = |word: &str| features.iter().any(|f| f.eq_ignore_ascii_case(word));
    if has("Instrument") {
        PluginKind::Instrument
    } else if has("Fx") {
        PluginKind::Effect
    } else {
        PluginKind::Other
    }
}

/// One class of the factory, as the factory describes it.
#[derive(Clone, Debug)]
pub struct ClassEntry {
    pub cid: TUID,
    pub category: String,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub features: Vec<String>,
}

/// Every class in the factory, with the richest description it offers
/// (`IPluginFactory3` → `IPluginFactory2` → `IPluginFactory`).
pub fn classes(module: &Module) -> Vec<ClassEntry> {
    let factory = module.factory();
    let f2 = factory.cast::<IPluginFactory2>();
    let f3 = factory.cast::<IPluginFactory3>();
    // SAFETY: plain factory queries into zeroed out-structs the factory fills.
    unsafe {
        let mut finfo: PFactoryInfo = std::mem::zeroed();
        let factory_vendor = if ok(factory.getFactoryInfo(&mut finfo)) {
            from_char8(&finfo.vendor)
        } else {
            String::new()
        };
        let mut out = Vec::new();
        for i in 0..factory.countClasses().max(0) {
            let mut entry = None;
            if let Some(f3) = &f3 {
                let mut w: PClassInfoW = std::mem::zeroed();
                if ok(f3.getClassInfoUnicode(i, &mut w)) {
                    entry = Some(ClassEntry {
                        cid: w.cid,
                        category: from_char8(&w.category),
                        name: from_char16(&w.name),
                        vendor: from_char16(&w.vendor),
                        version: from_char16(&w.version),
                        features: split_subcategories(&from_char8(&w.subCategories)),
                    });
                }
            }
            if entry.is_none() {
                if let Some(f2) = &f2 {
                    let mut c2: PClassInfo2 = std::mem::zeroed();
                    if ok(f2.getClassInfo2(i, &mut c2)) {
                        entry = Some(ClassEntry {
                            cid: c2.cid,
                            category: from_char8(&c2.category),
                            name: from_char8(&c2.name),
                            vendor: from_char8(&c2.vendor),
                            version: from_char8(&c2.version),
                            features: split_subcategories(&from_char8(&c2.subCategories)),
                        });
                    }
                }
            }
            if entry.is_none() {
                let mut c: PClassInfo = std::mem::zeroed();
                if ok(factory.getClassInfo(i, &mut c)) {
                    entry = Some(ClassEntry {
                        cid: c.cid,
                        category: from_char8(&c.category),
                        name: from_char8(&c.name),
                        vendor: String::new(),
                        version: String::new(),
                        features: Vec::new(),
                    });
                }
            }
            if let Some(mut e) = entry {
                if e.vendor.is_empty() {
                    e.vendor = factory_vendor.clone();
                }
                out.push(e);
            }
        }
        out
    }
}

fn split_subcategories(s: &str) -> Vec<String> {
    s.split('|')
        .map(str::trim)
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The buses that matter to the contract.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Buses {
    pub main_inputs: u32,
    pub main_outputs: u32,
    /// Index of the first auxiliary audio input (the sidechain), if any.
    pub sidechain: Option<i32>,
    pub audio_inputs: i32,
    pub audio_outputs: i32,
    pub event_inputs: i32,
}

pub fn buses(component: &ComPtr<IComponent>) -> Buses {
    let mut b = Buses::default();
    // SAFETY: bus queries on an initialized component, into zeroed structs.
    unsafe {
        b.audio_inputs = component.getBusCount(kAudio as i32, kInput as i32).max(0);
        b.audio_outputs = component.getBusCount(kAudio as i32, kOutput as i32).max(0);
        b.event_inputs = component.getBusCount(kEvent as i32, kInput as i32).max(0);
        let mut main_in = None;
        for i in 0..b.audio_inputs {
            let mut info: BusInfo = std::mem::zeroed();
            if !ok(component.getBusInfo(kAudio as i32, kInput as i32, i, &mut info)) {
                continue;
            }
            if info.busType == kMain as i32 && main_in.is_none() {
                main_in = Some(info.channelCount.max(0) as u32);
            } else if info.busType == kAux as i32 && b.sidechain.is_none() {
                b.sidechain = Some(i);
            }
        }
        b.main_inputs = main_in.unwrap_or(0);
        for i in 0..b.audio_outputs {
            let mut info: BusInfo = std::mem::zeroed();
            if ok(component.getBusInfo(kAudio as i32, kOutput as i32, i, &mut info))
                && info.busType == kMain as i32
            {
                b.main_outputs = info.channelCount.max(0) as u32;
                break;
            }
        }
    }
    b
}

/// Describe one class completely by instantiating it (buses, editor). A class that
/// refuses to start is still listed, with the stereo/editor defaults.
pub fn describe(module: &Arc<Module>, entry: &ClassEntry, path: &Path) -> PluginInfo {
    let mut kind = kind_from_subcategories(&entry.features);
    let mut info = PluginInfo {
        format: Format::Vst3,
        id: tuid_to_string(&entry.cid),
        name: entry.name.clone(),
        vendor: entry.vendor.clone(),
        version: entry.version.clone(),
        path: path.to_path_buf(),
        kind,
        main_inputs: if kind == PluginKind::Instrument { 0 } else { 2 },
        main_outputs: 2,
        sidechain: false,
        editor: true,
        features: entry.features.clone(),
    };
    if let Ok(parts) = Parts::create(module, &entry.cid) {
        let b = buses(&parts.component);
        info.main_inputs = b.main_inputs;
        info.main_outputs = b.main_outputs;
        info.sidechain = b.sidechain.is_some();
        info.editor = parts.has_editor();
        if kind == PluginKind::Other {
            kind = if b.event_inputs > 0 && b.main_inputs == 0 {
                PluginKind::Instrument
            } else if b.main_inputs > 0 {
                PluginKind::Effect
            } else {
                PluginKind::Other
            };
            info.kind = kind;
        }
    }
    info
}

/// Every audio module class in a loaded module, described completely.
pub fn scan_module(module: &Arc<Module>, path: &Path) -> Vec<PluginInfo> {
    classes(module)
        .iter()
        .filter(|c| c.category == AUDIO_MODULE_CLASS)
        .map(|c| describe(module, c, path))
        .collect()
}
