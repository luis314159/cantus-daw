//! The plug-in's side of ARA: a factory, a document controller that keeps the graph the host
//! builds, a playback renderer that renders its regions from the host's audio readers at the
//! gain each modification keeps, and partial archives of those gains.
//!
//! **Every breach of ARA's call rules is counted** ([`VIOLATIONS`]): a graph edit outside
//! `beginEditing`/`endEditing`, audio read without access, a region added to a renderer while
//! its instance is active, a bind after activation, an object destroyed while still in use.
//! The host's tests assert the count stays zero.

use std::collections::HashMap;
use std::ffi::{c_void, CStr, CString};
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use cantus_ara::sys::*;

pub static VIOLATIONS: AtomicI32 = AtomicI32::new(0);
static NEXT_REF: AtomicU64 = AtomicU64::new(1);

pub fn violation(what: &str) {
    VIOLATIONS.fetch_add(1, Ordering::SeqCst);
    eprintln!("cantus-test-ara: ARA rule broken: {what}");
}

fn next_ref<T>() -> *mut T {
    NEXT_REF.fetch_add(1, Ordering::SeqCst) as usize as *mut T
}

pub const PLUGIN_NAME: &CStr = c"Cantus Test ARA";
const VENDOR: &CStr = c"Cantus Tests";
const FACTORY_ID: &CStr = c"org.cantus.test.ara.factory";
pub const ARCHIVE_ID: &CStr = c"org.cantus.test.ara.document.v1";
const OLD_ARCHIVE_ID: &CStr = c"org.cantus.test.ara.document.v0";
const URL: &CStr = c"https://example.invalid";
const VERSION: &CStr = c"1.0.0";

struct Ids([*const std::ffi::c_char; 1]);
unsafe impl Sync for Ids {}
static COMPATIBLE: Ids = Ids([OLD_ARCHIVE_ID.as_ptr()]);

pub struct Source {
    pub host: ARAAudioSourceHostRef,
    pub pid: String,
    pub samples: i64,
    pub rate: f64,
    pub channels: i32,
    pub access: bool,
}

pub struct Modification {
    pub host: ARAAudioModificationHostRef,
    pub source: usize,
    pub pid: String,
    pub gain: f32,
    pub active: bool,
}

#[derive(Clone)]
pub struct Region {
    pub host: ARAPlaybackRegionHostRef,
    pub modification: usize,
    pub start_mod: f64,
    pub start_play: f64,
    pub duration: f64,
}

#[derive(Default)]
pub struct Model {
    pub editing: i32,
    pub sources: HashMap<usize, Source>,
    pub mods: HashMap<usize, Modification>,
    pub regions: HashMap<usize, Region>,
    pub sequences: HashMap<usize, String>,
    pub contexts: usize,
    /// Modifications whose content changed, to tell the host on `notifyModelUpdates`.
    pub pending: Vec<(usize, Option<(f64, f64)>)>,
    /// Tempo entries read from the host at the last content update of a musical context.
    pub tempo_entries: i32,
}

/// One document controller. Its address is the `ARADocumentControllerRef`.
pub struct Controller {
    pub host: ARADocumentControllerHostInstance,
    pub model: Mutex<Model>,
    instance: ARADocumentControllerInstance,
}

unsafe impl Send for Controller {}
unsafe impl Sync for Controller {}

/// Live controllers, for the test exports.
pub static CONTROLLERS: Mutex<Vec<usize>> = Mutex::new(Vec::new());

impl Controller {
    pub fn model(&self) -> MutexGuard<'_, Model> {
        self.model.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn edit_check(&self, what: &str) {
        if self.model().editing <= 0 {
            violation(&format!("{what} outside beginEditing/endEditing"));
        }
    }

    /// Read `count` frames of a source from `pos` through a fresh host reader: channel-major.
    pub unsafe fn read(&self, source: usize, pos: i64, count: usize) -> Option<Vec<Vec<f32>>> {
        let (host_ref, channels) = {
            let m = self.model();
            let s = m.sources.get(&source)?;
            if !s.access {
                drop(m);
                violation("audio read without access");
                return None;
            }
            (s.host, s.channels.max(1) as usize)
        };
        let aa = &*self.host.audioAccessControllerInterface;
        let ctrl = self.host.audioAccessControllerHostRef;
        let reader = (aa.createAudioReaderForSource?)(ctrl, host_ref, 0);
        if reader.is_null() {
            return None;
        }
        let mut bufs = vec![vec![0f32; count]; channels];
        let ptrs: Vec<*mut c_void> = bufs
            .iter_mut()
            .map(|b| b.as_mut_ptr() as *mut c_void)
            .collect();
        let ok = (aa.readAudioSamples?)(ctrl, reader, pos, count as i64, ptrs.as_ptr());
        (aa.destroyAudioReader?)(ctrl, reader);
        (ok != 0).then_some(bufs)
    }
}

unsafe fn dc<'a>(r: ARADocumentControllerRef) -> &'a Controller {
    &*(r as *const Controller)
}

fn cstr(p: *const std::ffi::c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }
}

// --- factory ----------------------------------------------------------------------------------

unsafe extern "C" fn initialize(_config: *const ARAInterfaceConfiguration) {}
unsafe extern "C" fn uninitialize() {}

unsafe extern "C" fn create_dc(
    host: *const ARADocumentControllerHostInstance,
    _props: *const ARADocumentProperties,
) -> *const ARADocumentControllerInstance {
    let c = Box::new(Controller {
        host: std::ptr::read_unaligned(host),
        model: Mutex::new(Model::default()),
        instance: ARADocumentControllerInstance {
            structSize: std::mem::size_of::<ARADocumentControllerInstance>(),
            documentControllerRef: std::ptr::null_mut(),
            documentControllerInterface: &DC_INTERFACE,
        },
    });
    let raw = Box::into_raw(c);
    (*raw).instance.documentControllerRef = raw as ARADocumentControllerRef;
    CONTROLLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(raw as usize);
    &(*raw).instance
}

/// The factory table (raw pointers to static strings: shared read-only).
pub struct SyncFactory(pub ARAFactory);
unsafe impl Sync for SyncFactory {}

pub static FACTORY: SyncFactory = SyncFactory(ARAFactory {
    structSize: std::mem::size_of::<ARAFactory>(),
    lowestSupportedApiGeneration: kARAAPIGeneration_2_0_Final,
    highestSupportedApiGeneration: kARAAPIGeneration_2_0_Final,
    factoryID: FACTORY_ID.as_ptr(),
    initializeARAWithConfiguration: Some(initialize),
    uninitializeARA: Some(uninitialize),
    plugInName: PLUGIN_NAME.as_ptr(),
    manufacturerName: VENDOR.as_ptr(),
    informationURL: URL.as_ptr(),
    version: VERSION.as_ptr(),
    createDocumentControllerWithDocument: Some(create_dc),
    documentArchiveID: ARCHIVE_ID.as_ptr(),
    compatibleDocumentArchiveIDsCount: 1,
    compatibleDocumentArchiveIDs: COMPATIBLE.0.as_ptr(),
    analyzeableContentTypesCount: 0,
    analyzeableContentTypes: std::ptr::null(),
    supportedPlaybackTransformationFlags: kARAPlaybackTransformationNoChanges,
    supportsStoringAudioFileChunks: 0,
});

// --- document controller ------------------------------------------------------------------------

unsafe extern "C" fn destroy_dc(r: ARADocumentControllerRef) {
    let c = Box::from_raw(r as *mut Controller);
    {
        let m = c.model();
        if !m.regions.is_empty() || !m.mods.is_empty() || !m.sources.is_empty() {
            violation("document controller destroyed with objects left");
        }
    }
    CONTROLLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|&a| a != r as usize);
}

unsafe extern "C" fn get_factory(_r: ARADocumentControllerRef) -> *const ARAFactory {
    &FACTORY.0
}

unsafe extern "C" fn begin_editing(r: ARADocumentControllerRef) {
    dc(r).model().editing += 1;
}

unsafe extern "C" fn end_editing(r: ARADocumentControllerRef) {
    let mut m = dc(r).model();
    m.editing -= 1;
    if m.editing < 0 {
        drop(m);
        violation("endEditing without beginEditing");
    }
}

unsafe extern "C" fn notify_model_updates(r: ARADocumentControllerRef) {
    let c = dc(r);
    let pending: Vec<(ARAAudioModificationHostRef, Option<(f64, f64)>)> = {
        let mut m = c.model();
        let p = std::mem::take(&mut m.pending);
        p.iter()
            .filter_map(|(k, r)| m.mods.get(k).map(|x| (x.host, *r)))
            .collect()
    };
    let mu = &*c.host.modelUpdateControllerInterface;
    for (h, range) in pending {
        let r = range.map(|(a, b)| ARAContentTimeRange {
            start: a,
            duration: b - a,
        });
        if let Some(f) = mu.notifyAudioModificationContentChanged {
            f(
                c.host.modelUpdateControllerHostRef,
                h,
                r.as_ref().map_or(std::ptr::null(), |r| r as *const _),
                kARAContentUpdateEverythingChanged,
            );
        }
    }
}

unsafe extern "C" fn create_context(
    r: ARADocumentControllerRef,
    _host: ARAMusicalContextHostRef,
    _p: *const ARAMusicalContextProperties,
) -> ARAMusicalContextRef {
    let c = dc(r);
    c.edit_check("createMusicalContext");
    c.model().contexts += 1;
    next_ref()
}

unsafe extern "C" fn update_context_content(
    r: ARADocumentControllerRef,
    _ctx: ARAMusicalContextRef,
    _range: *const ARAContentTimeRange,
    _flags: ARAContentUpdateFlags,
) {
    let c = dc(r);
    c.edit_check("updateMusicalContextContent");
    let ca = &*c.host.contentAccessControllerInterface;
    let ctrl = c.host.contentAccessControllerHostRef;
    // Read the tempo map back, as a plug-in that follows the song would.
    let reader = (ca.createMusicalContextContentReader.unwrap())(
        ctrl,
        std::ptr::null_mut(),
        kARAContentTypeTempoEntries,
        std::ptr::null(),
    );
    if !reader.is_null() {
        let n = (ca.getContentReaderEventCount.unwrap())(ctrl, reader);
        (ca.destroyContentReader.unwrap())(ctrl, reader);
        c.model().tempo_entries = n;
    }
}

unsafe extern "C" fn destroy_context(r: ARADocumentControllerRef, _ctx: ARAMusicalContextRef) {
    let c = dc(r);
    c.edit_check("destroyMusicalContext");
    let mut m = c.model();
    if !m.sequences.is_empty() {
        drop(m);
        violation("musical context destroyed under its region sequences");
        return;
    }
    m.contexts -= 1;
}

unsafe extern "C" fn create_source(
    r: ARADocumentControllerRef,
    host: ARAAudioSourceHostRef,
    p: *const ARAAudioSourceProperties,
) -> ARAAudioSourceRef {
    let c = dc(r);
    c.edit_check("createAudioSource");
    let p = std::ptr::read_unaligned(p);
    let k: ARAAudioSourceRef = next_ref();
    c.model().sources.insert(
        k as usize,
        Source {
            host,
            pid: cstr(p.persistentID),
            samples: p.sampleCount,
            rate: p.sampleRate,
            channels: p.channelCount,
            access: false,
        },
    );
    k
}

unsafe extern "C" fn enable_access(r: ARADocumentControllerRef, s: ARAAudioSourceRef, on: ARABool) {
    if let Some(src) = dc(r).model().sources.get_mut(&(s as usize)) {
        src.access = on != 0;
    }
}

unsafe extern "C" fn destroy_source(r: ARADocumentControllerRef, s: ARAAudioSourceRef) {
    let c = dc(r);
    c.edit_check("destroyAudioSource");
    let mut m = c.model();
    if m.mods.values().any(|x| x.source == s as usize) {
        drop(m);
        violation("audio source destroyed under its modifications");
        return;
    }
    m.sources.remove(&(s as usize));
}

unsafe extern "C" fn create_mod(
    r: ARADocumentControllerRef,
    s: ARAAudioSourceRef,
    host: ARAAudioModificationHostRef,
    p: *const ARAAudioModificationProperties,
) -> ARAAudioModificationRef {
    let c = dc(r);
    c.edit_check("createAudioModification");
    let p = std::ptr::read_unaligned(p);
    let k: ARAAudioModificationRef = next_ref();
    c.model().mods.insert(
        k as usize,
        Modification {
            host,
            source: s as usize,
            pid: cstr(p.persistentID),
            gain: 1.0,
            active: true,
        },
    );
    k
}

unsafe extern "C" fn clone_mod(
    r: ARADocumentControllerRef,
    from: ARAAudioModificationRef,
    host: ARAAudioModificationHostRef,
    p: *const ARAAudioModificationProperties,
) -> ARAAudioModificationRef {
    let c = dc(r);
    c.edit_check("cloneAudioModification");
    let p = std::ptr::read_unaligned(p);
    let k: ARAAudioModificationRef = next_ref();
    let mut m = c.model();
    let Some((source, gain)) = m.mods.get(&(from as usize)).map(|x| (x.source, x.gain)) else {
        drop(m);
        violation("clone of an unknown modification");
        return std::ptr::null_mut();
    };
    m.mods.insert(
        k as usize,
        Modification {
            host,
            source,
            pid: cstr(p.persistentID),
            gain,
            active: true,
        },
    );
    k
}

unsafe extern "C" fn deactivate_mod(
    r: ARADocumentControllerRef,
    x: ARAAudioModificationRef,
    off: ARABool,
) {
    let c = dc(r);
    c.edit_check("deactivateAudioModificationForUndoHistory");
    let mut m = c.model();
    if off != 0 && m.regions.values().any(|g| g.modification == x as usize) {
        drop(m);
        violation("modification deactivated under its regions");
        return;
    }
    if let Some(md) = m.mods.get_mut(&(x as usize)) {
        md.active = off == 0;
    }
}

unsafe extern "C" fn destroy_mod(r: ARADocumentControllerRef, x: ARAAudioModificationRef) {
    let c = dc(r);
    c.edit_check("destroyAudioModification");
    let mut m = c.model();
    if m.regions.values().any(|g| g.modification == x as usize) {
        drop(m);
        violation("modification destroyed under its regions");
        return;
    }
    m.mods.remove(&(x as usize));
}

fn region_of(
    host: ARAPlaybackRegionHostRef,
    modification: usize,
    p: &ARAPlaybackRegionProperties,
) -> Region {
    Region {
        host,
        modification,
        start_mod: p.startInModificationTime,
        start_play: p.startInPlaybackTime,
        duration: p.durationInPlaybackTime,
    }
}

unsafe extern "C" fn create_region(
    r: ARADocumentControllerRef,
    x: ARAAudioModificationRef,
    host: ARAPlaybackRegionHostRef,
    p: *const ARAPlaybackRegionProperties,
) -> ARAPlaybackRegionRef {
    let c = dc(r);
    c.edit_check("createPlaybackRegion");
    let p = std::ptr::read_unaligned(p);
    let k: ARAPlaybackRegionRef = next_ref();
    let mut m = c.model();
    if m.mods.get(&(x as usize)).is_none_or(|md| !md.active) {
        drop(m);
        violation("region on a missing or inactive modification");
        return std::ptr::null_mut();
    }
    m.regions
        .insert(k as usize, region_of(host, x as usize, &p));
    k
}

unsafe extern "C" fn update_region(
    r: ARADocumentControllerRef,
    g: ARAPlaybackRegionRef,
    p: *const ARAPlaybackRegionProperties,
) {
    let c = dc(r);
    c.edit_check("updatePlaybackRegionProperties");
    let p = std::ptr::read_unaligned(p);
    let mut m = c.model();
    if let Some(reg) = m.regions.get_mut(&(g as usize)) {
        let (host, md) = (reg.host, reg.modification);
        *reg = region_of(host, md, &p);
    }
}

unsafe extern "C" fn destroy_region(r: ARADocumentControllerRef, g: ARAPlaybackRegionRef) {
    let c = dc(r);
    c.edit_check("destroyPlaybackRegion");
    if crate::plugin::region_in_use(g as usize) {
        violation("region destroyed while a renderer holds it");
    }
    c.model().regions.remove(&(g as usize));
}

unsafe extern "C" fn create_sequence(
    r: ARADocumentControllerRef,
    _host: ARARegionSequenceHostRef,
    p: *const ARARegionSequenceProperties,
) -> ARARegionSequenceRef {
    let c = dc(r);
    c.edit_check("createRegionSequence");
    let p = std::ptr::read_unaligned(p);
    let k: ARARegionSequenceRef = next_ref();
    c.model().sequences.insert(k as usize, cstr(p.name));
    k
}

unsafe extern "C" fn update_sequence(
    r: ARADocumentControllerRef,
    s: ARARegionSequenceRef,
    p: *const ARARegionSequenceProperties,
) {
    let c = dc(r);
    c.edit_check("updateRegionSequenceProperties");
    let p = std::ptr::read_unaligned(p);
    if let Some(n) = c.model().sequences.get_mut(&(s as usize)) {
        *n = cstr(p.name);
    }
}

unsafe extern "C" fn destroy_sequence(r: ARADocumentControllerRef, s: ARARegionSequenceRef) {
    let c = dc(r);
    c.edit_check("destroyRegionSequence");
    c.model().sequences.remove(&(s as usize));
}

/// The archive: one line per modification, `persistentID=gain`.
unsafe extern "C" fn store(
    r: ARADocumentControllerRef,
    w: ARAArchiveWriterHostRef,
    f: *const ARAStoreObjectsFilter,
) -> ARABool {
    let c = dc(r);
    let wanted: Option<Vec<usize>> = (!f.is_null()).then(|| {
        let f = std::ptr::read_unaligned(f);
        (0..f.audioModificationRefsCount)
            .map(|i| *f.audioModificationRefs.add(i) as usize)
            .collect()
    });
    let text: String = {
        let m = c.model();
        let mut v: Vec<&Modification> = m
            .mods
            .iter()
            .filter(|(k, _)| wanted.as_ref().is_none_or(|w| w.contains(k)))
            .map(|(_, x)| x)
            .collect();
        v.sort_by(|a, b| a.pid.cmp(&b.pid));
        v.iter()
            .map(|x| format!("{}={}\n", x.pid, x.gain))
            .collect()
    };
    let ar = &*c.host.archivingControllerInterface;
    (ar.writeBytesToArchive.unwrap())(
        c.host.archivingControllerHostRef,
        w,
        0,
        text.len(),
        text.as_ptr(),
    )
}

unsafe extern "C" fn restore(
    r: ARADocumentControllerRef,
    rd: ARAArchiveReaderHostRef,
    f: *const ARARestoreObjectsFilter,
) -> ARABool {
    let c = dc(r);
    c.edit_check("restoreObjectsFromArchive");
    let ar = &*c.host.archivingControllerInterface;
    let ctrl = c.host.archivingControllerHostRef;
    let id = cstr((ar.getDocumentArchiveID.unwrap())(ctrl, rd));
    if id != ARCHIVE_ID.to_string_lossy() && id != OLD_ARCHIVE_ID.to_string_lossy() {
        violation("restore of an archive this plug-in did not write");
        return 0;
    }
    let size = (ar.getArchiveSize.unwrap())(ctrl, rd);
    let mut buf = vec![0u8; size];
    if size > 0 && (ar.readBytesFromArchive.unwrap())(ctrl, rd, 0, size, buf.as_mut_ptr()) == 0 {
        return 0;
    }
    let map: Vec<(String, String)> = if f.is_null() {
        Vec::new()
    } else {
        let f = std::ptr::read_unaligned(f);
        (0..f.audioModificationIDsCount)
            .map(|i| {
                (
                    cstr(*f.audioModificationArchiveIDs.add(i)),
                    cstr(*f.audioModificationCurrentIDs.add(i)),
                )
            })
            .collect()
    };
    let text = String::from_utf8_lossy(&buf).into_owned();
    let mut m = c.model();
    for line in text.lines() {
        let Some((pid, gain)) = line.split_once('=') else {
            return 0;
        };
        let Ok(gain) = gain.parse::<f32>() else {
            return 0;
        };
        let current = map
            .iter()
            .find(|(a, _)| a == pid)
            .map_or(pid.to_string(), |(_, cur)| cur.clone());
        if let Some(md) = m.mods.values_mut().find(|x| x.pid == current) {
            md.gain = gain;
        }
    }
    1
}

/// The head and tail time every region reports (seconds), settable by the tests.
pub static HEAD_TAIL: Mutex<(f64, f64)> = Mutex::new((0.0, 0.0));

unsafe extern "C" fn head_tail(
    _r: ARADocumentControllerRef,
    _g: ARAPlaybackRegionRef,
    head: *mut ARATimeDuration,
    tail: *mut ARATimeDuration,
) {
    let (h, t) = *HEAD_TAIL.lock().unwrap_or_else(|e| e.into_inner());
    *head = h;
    *tail = t;
}

pub static DC_INTERFACE: ARADocumentControllerInterface = {
    // SAFETY: an all-None table (every field is an `Option` of a function or a size).
    let mut i: ARADocumentControllerInterface = unsafe { std::mem::zeroed() };
    i.structSize = std::mem::size_of::<ARADocumentControllerInterface>();
    i.destroyDocumentController = Some(destroy_dc);
    i.getFactory = Some(get_factory);
    i.beginEditing = Some(begin_editing);
    i.endEditing = Some(end_editing);
    i.notifyModelUpdates = Some(notify_model_updates);
    i.createMusicalContext = Some(create_context);
    i.updateMusicalContextContent = Some(update_context_content);
    i.destroyMusicalContext = Some(destroy_context);
    i.createAudioSource = Some(create_source);
    i.enableAudioSourceSamplesAccess = Some(enable_access);
    i.destroyAudioSource = Some(destroy_source);
    i.createAudioModification = Some(create_mod);
    i.cloneAudioModification = Some(clone_mod);
    i.deactivateAudioModificationForUndoHistory = Some(deactivate_mod);
    i.destroyAudioModification = Some(destroy_mod);
    i.createPlaybackRegion = Some(create_region);
    i.updatePlaybackRegionProperties = Some(update_region);
    i.destroyPlaybackRegion = Some(destroy_region);
    i.createRegionSequence = Some(create_sequence);
    i.updateRegionSequenceProperties = Some(update_sequence);
    i.destroyRegionSequence = Some(destroy_sequence);
    i.storeObjectsToArchive = Some(store);
    i.getPlaybackRegionHeadAndTailTime = Some(head_tail);
    i.restoreObjectsFromArchive = Some(restore);
    i
};

/// Find a modification by persistent id in any live controller.
pub fn with_mod<R>(
    pid: &str,
    f: impl FnOnce(&mut Modification, usize, &mut Model) -> R,
) -> Option<R> {
    let list = CONTROLLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    for at in list {
        // SAFETY: a live controller (removed from the list when destroyed).
        let c = unsafe { &*(at as *const Controller) };
        let mut m = c.model();
        let key = m.mods.iter().find(|(_, x)| x.pid == pid).map(|(k, _)| *k);
        if let Some(k) = key {
            let mut md = m.mods.remove(&k).unwrap();
            let out = f(&mut md, k, &mut m);
            m.mods.insert(k, md);
            return Some(out);
        }
    }
    None
}

pub fn pid_of(p: *const std::ffi::c_char) -> String {
    cstr(p)
}

pub fn _keep(_: CString) {}
