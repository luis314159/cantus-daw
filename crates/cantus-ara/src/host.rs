//! An ARA document on the host's side: one plug-in's document controller and the objects
//! Cantus creates in it — a musical context, a region sequence per track, an audio source per
//! recording, an audio modification and a playback region per bound clip — kept in step with
//! the project by [`HostDocument::update`].
//!
//! **Undo keeps the plug-in's state.** A clip that leaves the document (deleted, let go)
//! loses its region; its modification is *deactivated for undo* (ARA's own mechanism) and
//! comes back with everything the plug-in did to it when the clip does. A clip that arrives
//! with an archive and no modification here (a project opened, a clip pasted or imported) is
//! restored from it; a copy of a clip whose modification is alive here is cloned from it.
//!
//! **Regions sit on the sample grid** of their source: start, length and source offset are
//! whole frames ([`RegionFrames`]), so the plug-in's render and Cantus's placement agree to
//! the sample.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{CStr, CString};
use std::sync::{Arc, Mutex};

use cantus_core::ara::{modification_persistent_id, source_persistent_id, AraArchive};
use cantus_core::model::{AudioSourceId, ClipId, TrackId};
use cantus_core::{PluginState, SampleData};

use crate::callbacks::{host_instance, ArchiveBuf, HostState, Musical, SourceAudio};
use crate::document::{Document, Region};
use crate::sys::*;

/// A plug-in's ARA factory, checked: ARA 2 (the generation Cantus speaks is 2.0 final).
#[derive(Copy, Clone, Debug)]
pub struct Factory {
    ptr: *const ARAFactory,
}

// SAFETY: the factory is a static table of the plug-in's, read-only, valid while its module is
// loaded (the owner keeps the module).
unsafe impl Send for Factory {}
unsafe impl Sync for Factory {}

/// The API generation Cantus speaks.
pub const GENERATION: ARAAPIGeneration = kARAAPIGeneration_2_0_Final;

fn text(p: *const std::ffi::c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        // SAFETY: ARA strings are NUL-terminated UTF-8, valid while the factory is.
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }
}

impl Factory {
    /// # Safety
    /// `ptr` is null or a plug-in's `ARAFactory`, valid while the caller keeps its module.
    pub unsafe fn new(ptr: *const ARAFactory) -> Result<Factory, String> {
        if ptr.is_null() {
            return Err("no ARA factory".into());
        }
        let f = &*ptr;
        if f.structSize < kARAFactoryMinSize as usize {
            return Err("an ARA factory too small to read".into());
        }
        if f.lowestSupportedApiGeneration > GENERATION
            || f.highestSupportedApiGeneration < GENERATION
        {
            return Err(format!(
                "ARA generations {}..{} do not include ARA 2.0",
                { f.lowestSupportedApiGeneration },
                { f.highestSupportedApiGeneration }
            ));
        }
        if { f.createDocumentControllerWithDocument }.is_none() || {
            f.initializeARAWithConfiguration
        }
        .is_none()
        {
            return Err("an ARA factory without its entry points".into());
        }
        Ok(Factory { ptr })
    }

    fn get(&self) -> &ARAFactory {
        // SAFETY: checked in `new`, valid while the module is loaded.
        unsafe { &*self.ptr }
    }

    pub fn as_ptr(&self) -> *const ARAFactory {
        self.ptr
    }

    pub fn factory_id(&self) -> String {
        text(self.get().factoryID)
    }

    pub fn name(&self) -> String {
        text(self.get().plugInName)
    }

    pub fn vendor(&self) -> String {
        text(self.get().manufacturerName)
    }

    pub fn version(&self) -> String {
        text(self.get().version)
    }

    pub fn document_archive_id(&self) -> String {
        text(self.get().documentArchiveID)
    }

    /// Whether an archive written under `id` can be read by this plug-in.
    pub fn reads_archive(&self, id: &str) -> bool {
        let f = self.get();
        if self.document_archive_id() == id {
            return true;
        }
        if f.compatibleDocumentArchiveIDs.is_null() {
            return false;
        }
        (0..f.compatibleDocumentArchiveIDsCount)
            // SAFETY: an array of `count` strings, valid while the factory is.
            .any(|i| text(unsafe { *f.compatibleDocumentArchiveIDs.add(i) }) == id)
    }
}

/// ARA is initialised once per factory and process (`initializeARAWithConfiguration`) and
/// uninitialised after its last document: counted here.
static INITIALISED: Mutex<Vec<(usize, usize)>> = Mutex::new(Vec::new());

fn acquire(f: &Factory) {
    let mut g = INITIALISED.lock().unwrap_or_else(|e| e.into_inner());
    let key = f.ptr as usize;
    if let Some(e) = g.iter_mut().find(|e| e.0 == key) {
        e.1 += 1;
        return;
    }
    let config = ARAInterfaceConfiguration {
        structSize: std::mem::size_of::<ARAInterfaceConfiguration>(),
        desiredApiGeneration: GENERATION,
        assertFunctionAddress: std::ptr::null_mut(),
    };
    // SAFETY: checked present in `Factory::new`; called once per factory.
    unsafe { (f.get().initializeARAWithConfiguration.unwrap())(&config) };
    g.push((key, 1));
}

fn release(f: &Factory) {
    let mut g = INITIALISED.lock().unwrap_or_else(|e| e.into_inner());
    let key = f.ptr as usize;
    let Some(i) = g.iter().position(|e| e.0 == key) else {
        return;
    };
    g[i].1 -= 1;
    if g[i].1 == 0 {
        g.remove(i);
        if let Some(u) = f.get().uninitializeARA {
            // SAFETY: the last document of this factory is gone.
            unsafe { u() };
        }
    }
}

/// A region's place in whole frames of its source's rate: where it starts on the timeline
/// (`start`, from the song's start), where in the source (`source_start`, may be negative)
/// and how long (`len`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RegionFrames {
    pub start: i64,
    pub source_start: i64,
    pub len: i64,
    pub rate: u32,
    /// What the plug-in plays before the region starts and after it ends (ARA's head and
    /// tail time: a fade it makes, a reverb tail), in frames.
    pub head: i64,
    pub tail: i64,
}

impl RegionFrames {
    pub fn of(r: &Region, rate: u32) -> Self {
        let f = |s: f64| (s * f64::from(rate)).round() as i64;
        RegionFrames {
            start: f(r.start),
            source_start: f(r.source_start),
            len: f(r.duration).max(1),
            rate,
            head: 0,
            tail: 0,
        }
    }

    fn seconds(&self, frames: i64) -> f64 {
        frames as f64 / f64::from(self.rate.max(1))
    }
}

struct SourceEntry {
    aref: ARAAudioSourceRef,
    meta: crate::document::Source,
}

struct ModEntry {
    mref: ARAAudioModificationRef,
    source: AudioSourceId,
    /// The binding it was made for (`AraClip::binding`).
    binding: u64,
    active: bool,
}

struct RegionEntry {
    rref: ARAPlaybackRegionRef,
    region: Region,
    frames: RegionFrames,
}

/// What an update did, for whoever runs the plug-in instances.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Changes {
    /// Clips whose region was made (an instance to bind, a render to make).
    pub created: Vec<ClipId>,
    /// Clips whose region changed (a render to make again).
    pub changed: Vec<ClipId>,
    /// Clips whose region went (their instance was let go first).
    pub removed: Vec<ClipId>,
    /// Archives a plug-in could not read (the clip, why).
    pub refused: Vec<(ClipId, String)>,
}

impl Changes {
    pub fn is_empty(&self) -> bool {
        self.created.is_empty() && self.changed.is_empty() && self.removed.is_empty()
    }
}

/// What the plug-in said changed, in Cantus's ids.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Notifications {
    pub clips: Vec<ClipId>,
    /// Where each clip changed, in frames from its region's start (`None`: all of it).
    pub ranges: BTreeMap<ClipId, Option<(i64, i64)>>,
    pub sources: Vec<AudioSourceId>,
    pub document: bool,
}

/// One plug-in's document. Every call is on the thread that made it (ARA's document thread).
pub struct HostDocument {
    factory: Factory,
    state: Box<HostState>,
    host: Box<ARADocumentControllerHostInstance>,
    dc: *const ARADocumentControllerInstance,
    context: ARAMusicalContextRef,
    musical: Musical,
    sequences: BTreeMap<TrackId, (ARARegionSequenceRef, String, i32)>,
    sources: BTreeMap<AudioSourceId, SourceEntry>,
    mods: BTreeMap<ClipId, ModEntry>,
    regions: BTreeMap<ClipId, RegionEntry>,
}

/// Host refs: the id + 1 (never null).
fn href<T>(id: u64) -> *mut T {
    (id as usize).wrapping_add(1) as *mut T
}

fn unref(r: usize) -> u64 {
    r.wrapping_sub(1) as u64
}

impl HostDocument {
    /// A new, empty document named `name`.
    pub fn new(factory: Factory, name: &str) -> Result<HostDocument, String> {
        acquire(&factory);
        let state = Box::<HostState>::default();
        let host = Box::new(host_instance(&state));
        let cname = CString::new(name).unwrap_or_default();
        let props = ARADocumentProperties {
            structSize: std::mem::size_of::<ARADocumentProperties>(),
            name: cname.as_ptr(),
        };
        // SAFETY: the factory's entry point, with a host instance that outlives the controller
        // (both boxed in the document, dropped after it is destroyed).
        let dc = unsafe {
            (factory.get().createDocumentControllerWithDocument.unwrap())(&*host, &props)
        };
        if dc.is_null() {
            release(&factory);
            return Err("the plug-in made no document controller".into());
        }
        // Every field Cantus calls, checked once against the table the plug-in gave (ARA's
        // ARA_IMPLEMENTS_FIELD): a plug-in that lacks one is refused here, so no call below
        // can reach past its table or through a null.
        // SAFETY: the controller instance and its interface table, as the plug-in made them.
        let missing = unsafe { missing_fields(&*(*dc).documentControllerInterface) };
        if !missing.is_empty() {
            // SAFETY: destroy is checked first among them; if it is missing too, leaked.
            unsafe {
                let i = &*(*dc).documentControllerInterface;
                if !missing.contains(&"destroyDocumentController") {
                    if let Some(f) = { i.destroyDocumentController } {
                        f((*dc).documentControllerRef);
                    }
                }
            }
            release(&factory);
            return Err(format!(
                "an incomplete ARA 2 document controller (no {})",
                missing.join(", ")
            ));
        }
        let mut doc = HostDocument {
            factory,
            state,
            host,
            dc,
            context: std::ptr::null_mut(),
            musical: Musical::default(),
            sequences: BTreeMap::new(),
            sources: BTreeMap::new(),
            mods: BTreeMap::new(),
            regions: BTreeMap::new(),
        };
        let cname = CString::new("Cantus").unwrap_or_default();
        let ctx = ARAMusicalContextProperties {
            structSize: std::mem::size_of::<ARAMusicalContextProperties>(),
            name: cname.as_ptr(),
            orderIndex: 0,
            color: std::ptr::null(),
        };
        doc.edit(|d| {
            // SAFETY: inside an editing session, a host ref of our own.
            d.context =
                unsafe { (d.iface().createMusicalContext.unwrap())(d.dc_ref(), href(0), &ctx) };
        });
        if doc.context.is_null() {
            return Err("the plug-in made no musical context".into());
        }
        Ok(doc)
    }

    pub fn factory(&self) -> &Factory {
        &self.factory
    }

    fn iface(&self) -> &ARADocumentControllerInterface {
        // SAFETY: the controller's interface table, valid until it is destroyed.
        unsafe { &*(*self.dc).documentControllerInterface }
    }

    /// The document controller's ref: what a plug-in instance is bound to.
    pub fn dc_ref(&self) -> ARADocumentControllerRef {
        // SAFETY: valid until destroyed in `drop`.
        unsafe { (*self.dc).documentControllerRef }
    }

    fn edit(&mut self, f: impl FnOnce(&mut Self)) {
        // SAFETY: the editing session's brackets, on the document thread.
        unsafe { (self.iface().beginEditing.unwrap())(self.dc_ref()) };
        f(self);
        unsafe { (self.iface().endEditing.unwrap())(self.dc_ref()) };
    }

    /// The region a clip plays and its frames, while it is in the document.
    pub fn region(&self, clip: ClipId) -> Option<(ARAPlaybackRegionRef, RegionFrames)> {
        let r = self.regions.get(&clip)?;
        let (mut head, mut tail) = (0.0f64, 0.0f64);
        // SAFETY: a region of this document; the field was checked present at creation.
        unsafe {
            (self.iface().getPlaybackRegionHeadAndTailTime.unwrap())(
                self.dc_ref(),
                r.rref,
                &mut head,
                &mut tail,
            )
        };
        let mut f = r.frames;
        let up = |s: f64| {
            (if s.is_finite() {
                s.clamp(0.0, 60.0)
            } else {
                0.0
            } * f64::from(f.rate))
            .ceil() as i64
        };
        f.head = up(head);
        f.tail = up(tail);
        Some((r.rref, f))
    }

    /// The clips with a region.
    pub fn clips(&self) -> impl Iterator<Item = ClipId> + '_ {
        self.regions.keys().copied()
    }

    /// The binding number a clip's modification was made for.
    pub fn binding(&self, clip: ClipId) -> Option<u64> {
        self.mods.get(&clip).map(|m| m.binding)
    }

    /// The clips with a modification here: playing, or kept for undo.
    pub fn modifications(&self) -> impl Iterator<Item = ClipId> + '_ {
        self.mods.keys().copied()
    }

    /// The audio a clip's source holds (what the plug-in reads).
    pub fn source_audio(&self, clip: ClipId) -> Option<Arc<SampleData>> {
        let src = self.regions.get(&clip)?.region.source;
        let m = self.state.sources.lock().ok()?;
        m.get(&(href::<u8>(src.0) as usize)).map(|a| a.data.clone())
    }

    /// Bring the plug-in's copy to `next`. `audio` gives a source's samples at its own rate
    /// (a source with none is left out, and its clips with it); `archive` the archive a clip
    /// arrives with; `letting_go` is called for each clip whose region is about to go, so its
    /// plug-in instance is destroyed first (a region may not die inside a renderer).
    pub fn update(
        &mut self,
        next: &Document,
        musical: &Musical,
        audio: &dyn Fn(AudioSourceId) -> Option<Arc<SampleData>>,
        archive: &dyn Fn(ClipId) -> Option<AraArchive>,
        letting_go: &mut dyn FnMut(ClipId),
    ) -> Changes {
        let mut ch = Changes::default();
        // Sources whose file changed: everything on them is made again.
        let replaced: BTreeSet<AudioSourceId> = next
            .sources
            .iter()
            .filter(|(id, s)| self.sources.get(id).is_some_and(|e| e.meta != **s))
            .map(|(id, _)| *id)
            .collect();
        // Audio for sources not yet here (outside the session: decoding is slow).
        let mut fresh: BTreeMap<AudioSourceId, Arc<SampleData>> = BTreeMap::new();
        for id in next.sources.keys() {
            if !self.sources.contains_key(id) || replaced.contains(id) {
                if let Some(a) = audio(*id) {
                    fresh.insert(*id, a);
                }
            }
        }
        let gone: Vec<ClipId> =
            self.regions
                .iter()
                .filter(|(c, e)| {
                    next.regions.get(c).is_none_or(|r| {
                        r.source != e.region.source || r.binding != e.region.binding
                    }) || replaced.contains(&e.region.source)
                })
                .map(|(c, _)| *c)
                .collect();
        for c in &gone {
            letting_go(*c);
        }
        let musical_changed = *musical != self.musical;
        let wants_edit = musical_changed
            || !gone.is_empty()
            || !replaced.is_empty()
            || next
                .regions
                .iter()
                .any(|(c, r)| self.regions.get(c).is_none_or(|e| e.region != *r))
            || next
                .tracks
                .iter()
                .any(|(t, (n, o))| self.sequences.get(t).is_none_or(|s| s.1 != *n || s.2 != *o))
            || self.sequences.keys().any(|t| !next.tracks.contains_key(t));
        if !wants_edit {
            return ch;
        }
        let mut restores: Vec<(ClipId, AraArchive)> = Vec::new();
        self.edit(|d| {
            if musical_changed {
                *d.state.musical.lock().unwrap_or_else(|e| e.into_inner()) = musical.clone();
                d.musical = musical.clone();
                // SAFETY: inside the session; a null range is the whole context.
                unsafe {
                    (d.iface().updateMusicalContextContent.unwrap())(
                        d.dc_ref(),
                        d.context,
                        std::ptr::null(),
                        kARAContentUpdateEverythingChanged,
                    )
                };
            }
            for c in &gone {
                d.destroy_region(*c);
                ch.removed.push(*c);
                // A clip that left keeps its modification for undo; one that changed source
                // (or whose file changed) has it made again.
                let keep = !next.regions.contains_key(c)
                    && d.mods.get(c).is_some_and(|m| !replaced.contains(&m.source));
                if keep {
                    d.deactivate(*c);
                } else {
                    d.destroy_mod(*c);
                }
            }
            for id in &replaced {
                let dead: Vec<ClipId> = d
                    .mods
                    .iter()
                    .filter(|(_, m)| m.source == *id)
                    .map(|(c, _)| *c)
                    .collect();
                for c in dead {
                    d.destroy_mod(c);
                }
                d.destroy_source(*id);
            }
            for (id, s) in &next.sources {
                if !d.sources.contains_key(id) {
                    if let Some(a) = fresh.get(id) {
                        d.create_source(s, a.clone());
                    }
                }
            }
            for (t, (name, order)) in &next.tracks {
                d.ensure_sequence(*t, name, *order);
            }
            for (c, r) in &next.regions {
                if !d.sources.contains_key(&r.source) {
                    continue;
                }
                match d.mods.get(c) {
                    Some(m) if m.source == r.source && m.binding == r.binding => {
                        if !m.active {
                            d.reactivate(*c);
                        }
                    }
                    Some(_) => {
                        d.destroy_mod(*c);
                        d.make_mod(*c, r.source, r.binding, archive(*c), &mut restores);
                    }
                    None => d.make_mod(*c, r.source, r.binding, archive(*c), &mut restores),
                }
                match d.regions.get(c) {
                    Some(e) if e.region == *r => {}
                    Some(_) => {
                        d.update_region(r);
                        ch.changed.push(*c);
                    }
                    None => {
                        if d.create_region(r) {
                            ch.created.push(*c);
                        }
                    }
                }
            }
            // Restores after the graph is built (ARA's partial persistency).
            for (c, a) in std::mem::take(&mut restores) {
                if let Err(e) = d.restore(c, &a) {
                    ch.refused.push((c, e));
                }
            }
            // Sources nothing uses any more (no region, no modification kept for undo).
            let used: BTreeSet<AudioSourceId> = d.mods.values().map(|m| m.source).collect();
            let unused: Vec<AudioSourceId> = d
                .sources
                .keys()
                .filter(|s| !used.contains(s))
                .copied()
                .collect();
            for s in unused {
                d.destroy_source(s);
            }
            let tracks: Vec<TrackId> = d
                .sequences
                .keys()
                .filter(|t| !next.tracks.contains_key(t))
                .copied()
                .collect();
            for t in tracks {
                if !d.regions.values().any(|r| r.region.track == t) {
                    let (sref, _, _) = d.sequences.remove(&t).unwrap();
                    // SAFETY: no region is on it any more; inside the session.
                    unsafe { (d.iface().destroyRegionSequence.unwrap())(d.dc_ref(), sref) };
                }
            }
        });
        ch
    }

    fn create_source(&mut self, s: &crate::document::Source, audio: Arc<SampleData>) {
        let pid = CString::new(source_persistent_id(s.id)).unwrap_or_default();
        let name = CString::new(s.name.as_str()).unwrap_or_default();
        let ch = audio.channels.max(1) as usize;
        let props = ARAAudioSourceProperties {
            structSize: std::mem::size_of::<ARAAudioSourceProperties>(),
            name: name.as_ptr(),
            persistentID: pid.as_ptr(),
            sampleCount: (audio.frames.len() / ch) as ARASampleCount,
            sampleRate: f64::from(audio.rate),
            channelCount: ch as ARAChannelCount,
            merits64BitSamples: 0,
            channelArrangementDataType: kARAChannelArrangementUndefined,
            channelArrangement: std::ptr::null(),
        };
        let r: usize = href::<u8>(s.id.0) as usize;
        self.state
            .sources
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(r, Arc::new(SourceAudio { data: audio }));
        // SAFETY: inside the session; strings live through the call.
        let aref = unsafe {
            (self.iface().createAudioSource.unwrap())(self.dc_ref(), href(s.id.0), &props)
        };
        if aref.is_null() {
            self.state
                .sources
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&r);
            return;
        }
        // Access is off until the host turns it on (ARA).
        unsafe { (self.iface().enableAudioSourceSamplesAccess.unwrap())(self.dc_ref(), aref, 1) };
        self.sources.insert(
            s.id,
            SourceEntry {
                aref,
                meta: s.clone(),
            },
        );
    }

    fn destroy_source(&mut self, id: AudioSourceId) {
        let Some(e) = self.sources.remove(&id) else {
            return;
        };
        // SAFETY: no modification is left on it (callers destroy them first).
        unsafe {
            (self.iface().enableAudioSourceSamplesAccess.unwrap())(self.dc_ref(), e.aref, 0);
            (self.iface().destroyAudioSource.unwrap())(self.dc_ref(), e.aref);
        }
        self.state
            .sources
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&(href::<u8>(id.0) as usize));
    }

    fn ensure_sequence(&mut self, t: TrackId, name: &str, order: i32) {
        let cname = CString::new(name).unwrap_or_default();
        let props = ARARegionSequenceProperties {
            structSize: std::mem::size_of::<ARARegionSequenceProperties>(),
            name: cname.as_ptr(),
            orderIndex: order,
            musicalContextRef: self.context,
            color: std::ptr::null(),
        };
        match self.sequences.get_mut(&t) {
            Some(s) if s.1 == name && s.2 == order => {}
            Some(s) => {
                s.1 = name.to_string();
                s.2 = order;
                let sref = s.0;
                // SAFETY: inside the session.
                unsafe {
                    (self.iface().updateRegionSequenceProperties.unwrap())(
                        self.dc_ref(),
                        sref,
                        &props,
                    )
                };
            }
            None => {
                // SAFETY: inside the session.
                let sref = unsafe {
                    (self.iface().createRegionSequence.unwrap())(self.dc_ref(), href(t.0), &props)
                };
                self.sequences.insert(t, (sref, name.to_string(), order));
            }
        }
    }

    fn mod_props(clip: ClipId, pid: &CString, name: &CString) -> ARAAudioModificationProperties {
        let _ = clip;
        ARAAudioModificationProperties {
            structSize: std::mem::size_of::<ARAAudioModificationProperties>(),
            name: name.as_ptr(),
            persistentID: pid.as_ptr(),
        }
    }

    fn make_mod(
        &mut self,
        clip: ClipId,
        source: AudioSourceId,
        binding: u64,
        archive: Option<AraArchive>,
        restores: &mut Vec<(ClipId, AraArchive)>,
    ) {
        let Some(src) = self.sources.get(&source).map(|e| e.aref) else {
            return;
        };
        let pid = CString::new(modification_persistent_id(clip)).unwrap_or_default();
        let name = CString::new(format!("Clip {}", clip.0)).unwrap_or_default();
        let props = Self::mod_props(clip, &pid, &name);
        // A copy of a clip whose modification is alive here: cloned from it, the plug-in's
        // edits and all (ARA's clone).
        let original = archive.as_ref().and_then(|a| {
            self.mods
                .iter()
                .find(|(c, m)| {
                    modification_persistent_id(**c) == a.modification_id
                        && m.source == source
                        && **c != clip
                })
                .map(|(_, m)| m.mref)
        });
        // SAFETY: inside the session; strings live through the call.
        let mref = unsafe {
            match original {
                Some(o) => (self.iface().cloneAudioModification.unwrap())(
                    self.dc_ref(),
                    o,
                    href(clip.0),
                    &props,
                ),
                None => (self.iface().createAudioModification.unwrap())(
                    self.dc_ref(),
                    src,
                    href(clip.0),
                    &props,
                ),
            }
        };
        if mref.is_null() {
            return;
        }
        self.mods.insert(
            clip,
            ModEntry {
                mref,
                source,
                binding,
                active: true,
            },
        );
        if original.is_none() {
            if let Some(a) = archive {
                restores.push((clip, a));
            }
        }
    }

    fn deactivate(&mut self, clip: ClipId) {
        let Some(m) = self.mods.get_mut(&clip) else {
            return;
        };
        if m.active {
            m.active = false;
            let mref = m.mref;
            // SAFETY: inside the session, no region left on it.
            unsafe {
                (self
                    .iface()
                    .deactivateAudioModificationForUndoHistory
                    .unwrap())(self.dc_ref(), mref, 1)
            };
        }
    }

    fn reactivate(&mut self, clip: ClipId) {
        let Some(m) = self.mods.get_mut(&clip) else {
            return;
        };
        m.active = true;
        let mref = m.mref;
        // SAFETY: inside the session.
        unsafe {
            (self
                .iface()
                .deactivateAudioModificationForUndoHistory
                .unwrap())(self.dc_ref(), mref, 0)
        };
    }

    fn destroy_mod(&mut self, clip: ClipId) {
        let Some(m) = self.mods.remove(&clip) else {
            return;
        };
        // SAFETY: its region is gone (callers destroy it first).
        unsafe { (self.iface().destroyAudioModification.unwrap())(self.dc_ref(), m.mref) };
    }

    fn region_props(&self, r: &Region, f: &RegionFrames) -> ARAPlaybackRegionProperties {
        ARAPlaybackRegionProperties {
            structSize: std::mem::size_of::<ARAPlaybackRegionProperties>(),
            transformationFlags: kARAPlaybackTransformationNoChanges,
            startInModificationTime: f.seconds(f.source_start),
            durationInModificationTime: f.seconds(f.len),
            startInPlaybackTime: f.seconds(f.start),
            durationInPlaybackTime: f.seconds(f.len),
            musicalContextRef: self.context,
            regionSequenceRef: self
                .sequences
                .get(&r.track)
                .map_or(std::ptr::null_mut(), |s| s.0),
            name: std::ptr::null(),
            color: std::ptr::null(),
        }
    }

    fn rate_of(&self, s: AudioSourceId) -> u32 {
        self.state
            .sources
            .lock()
            .ok()
            .and_then(|m| m.get(&(href::<u8>(s.0) as usize)).map(|a| a.data.rate))
            .unwrap_or(48_000)
    }

    fn create_region(&mut self, r: &Region) -> bool {
        let Some(m) = self.mods.get(&r.clip).map(|m| m.mref) else {
            return false;
        };
        let frames = RegionFrames::of(r, self.rate_of(r.source));
        let props = self.region_props(r, &frames);
        // SAFETY: inside the session; its modification and sequence exist.
        let rref = unsafe {
            (self.iface().createPlaybackRegion.unwrap())(self.dc_ref(), m, href(r.clip.0), &props)
        };
        if rref.is_null() {
            return false;
        }
        self.regions.insert(
            r.clip,
            RegionEntry {
                rref,
                region: r.clone(),
                frames,
            },
        );
        true
    }

    fn update_region(&mut self, r: &Region) {
        let frames = RegionFrames::of(r, self.rate_of(r.source));
        let props = self.region_props(r, &frames);
        let Some(e) = self.regions.get_mut(&r.clip) else {
            return;
        };
        e.region = r.clone();
        e.frames = frames;
        let rref = e.rref;
        // SAFETY: inside the session.
        unsafe {
            (self.iface().updatePlaybackRegionProperties.unwrap())(self.dc_ref(), rref, &props)
        };
    }

    fn destroy_region(&mut self, clip: ClipId) {
        let Some(e) = self.regions.remove(&clip) else {
            return;
        };
        // SAFETY: its plug-in instance was let go first (`letting_go`).
        unsafe { (self.iface().destroyPlaybackRegion.unwrap())(self.dc_ref(), e.rref) };
    }

    fn restore(&mut self, clip: ClipId, a: &AraArchive) -> Result<(), String> {
        if !self.factory.reads_archive(&a.document_archive_id) {
            return Err(format!(
                "an archive of {} the plug-in does not read",
                a.document_archive_id
            ));
        }
        let Some(m) = self.mods.get(&clip) else {
            return Err("no modification".into());
        };
        let cs = |s: &str| CString::new(s).unwrap_or_default();
        let (sa, sc) = (cs(&a.source_id), cs(&source_persistent_id(m.source)));
        let (ma, mc) = (
            cs(&a.modification_id),
            cs(&modification_persistent_id(clip)),
        );
        let (sa_p, sc_p, ma_p, mc_p) = ([sa.as_ptr()], [sc.as_ptr()], [ma.as_ptr()], [mc.as_ptr()]);
        let filter = ARARestoreObjectsFilter {
            structSize: std::mem::size_of::<ARARestoreObjectsFilter>(),
            documentData: 0,
            audioSourceIDsCount: 1,
            audioSourceArchiveIDs: sa_p.as_ptr(),
            audioSourceCurrentIDs: sc_p.as_ptr(),
            audioModificationIDsCount: 1,
            audioModificationArchiveIDs: ma_p.as_ptr(),
            audioModificationCurrentIDs: mc_p.as_ptr(),
        };
        let buf = ArchiveBuf {
            bytes: a.data.bytes().to_vec(),
        };
        *self
            .state
            .restoring_archive_id
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(cs(&a.document_archive_id));
        // SAFETY: inside the session; the reader and filter live through the call.
        let ok = unsafe {
            (self
                .iface()
                .restoreObjectsFromArchive
                .ok_or("no partial persistency")?)(
                self.dc_ref(),
                &buf as *const ArchiveBuf as ARAArchiveReaderHostRef,
                &filter,
            )
        };
        *self
            .state
            .restoring_archive_id
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = None;
        if ok == 0 {
            return Err("the plug-in could not read its archive".into());
        }
        Ok(())
    }

    /// Put a clip's modification back to `archive` (an undo or redo of the plug-in's edit),
    /// in an editing session of its own.
    pub fn reload(&mut self, clip: ClipId, archive: &AraArchive) -> Result<(), String> {
        let mut out = Ok(());
        self.edit(|d| out = d.restore(clip, archive));
        out
    }

    /// Show `clip`'s region in an editor view (`notifySelection`): what the plug-in's editor
    /// opens on.
    ///
    /// # Safety
    /// `view` and `iface` are the editor view of an instance bound to this document.
    pub unsafe fn select(
        &self,
        clip: ClipId,
        view: ARAEditorViewRef,
        iface: *const ARAEditorViewInterface,
    ) -> bool {
        let Some(r) = self.regions.get(&clip) else {
            return false;
        };
        if iface.is_null() || view.is_null() {
            return false;
        }
        let Some(notify) = std::ptr::read_unaligned(iface).notifySelection else {
            return false;
        };
        let regions = [r.rref];
        let sel = ARAViewSelection {
            structSize: std::mem::size_of::<ARAViewSelection>(),
            playbackRegionRefsCount: 1,
            playbackRegionRefs: regions.as_ptr(),
            regionSequenceRefsCount: 0,
            regionSequenceRefs: std::ptr::null(),
            timeRange: std::ptr::null(),
        };
        notify(view, &sel);
        true
    }

    /// The plug-in's archive of one clip's modification (and its source), as it is now.
    pub fn store(&self, clip: ClipId) -> Option<AraArchive> {
        let m = self.mods.get(&clip)?;
        let src = self.sources.get(&m.source)?.aref;
        let (srcs, mods) = ([src], [m.mref]);
        let filter = ARAStoreObjectsFilter {
            structSize: std::mem::size_of::<ARAStoreObjectsFilter>(),
            documentData: 0,
            audioSourceRefsCount: 1,
            audioSourceRefs: srcs.as_ptr(),
            audioModificationRefsCount: 1,
            audioModificationRefs: mods.as_ptr(),
        };
        let mut buf = ArchiveBuf { bytes: Vec::new() };
        // SAFETY: outside any session is allowed; the writer lives through the call.
        let ok = unsafe {
            (self.iface().storeObjectsToArchive?)(
                self.dc_ref(),
                &mut buf as *mut ArchiveBuf as ARAArchiveWriterHostRef,
                &filter,
            )
        };
        (ok != 0).then(|| AraArchive {
            document_archive_id: self.factory.document_archive_id(),
            source_id: source_persistent_id(m.source),
            modification_id: modification_persistent_id(clip),
            data: PluginState::new(buf.bytes),
        })
    }

    /// Let the plug-in send what changed (`notifyModelUpdates`), and take what it said.
    pub fn poll(&mut self) -> Notifications {
        // SAFETY: outside any session, on the document thread.
        unsafe { (self.iface().notifyModelUpdates.unwrap())(self.dc_ref()) };
        let n = std::mem::take(
            &mut *self
                .state
                .notified
                .lock()
                .unwrap_or_else(|e| e.into_inner()),
        );
        let mut ranges: BTreeMap<ClipId, Option<(i64, i64)>> = BTreeMap::new();
        let mut widen = |c: ClipId, r: Option<(i64, i64)>| {
            let e = ranges.entry(c).or_insert(r);
            *e = match (*e, r) {
                (Some(a), Some(b)) => Some((a.0.min(b.0), a.1.max(b.1))),
                _ => None,
            };
        };
        for (m, range) in &n.modifications {
            let c = ClipId(unref(*m));
            let Some(e) = self.regions.get(&c) else {
                continue;
            };
            // Modification time to frames from the region's start.
            let f = e.frames;
            let r = range.map(|(a, b)| {
                let at = |s: f64| (s * f64::from(f.rate)).floor() as i64 - f.source_start;
                (at(a), at(b) + 1)
            });
            widen(c, r);
        }
        for r in &n.regions {
            let c = ClipId(unref(*r));
            if self.regions.contains_key(&c) {
                widen(c, None);
            }
        }
        let mut clips: Vec<ClipId> = ranges.keys().copied().collect();
        let sources: Vec<AudioSourceId> =
            n.sources.iter().map(|r| AudioSourceId(unref(*r))).collect();
        // A source's content changed: every clip on it.
        for s in &sources {
            clips.extend(
                self.regions
                    .iter()
                    .filter(|(_, e)| e.region.source == *s)
                    .map(|(c, _)| *c),
            );
        }
        clips.sort();
        clips.dedup();
        for c in &clips {
            ranges.entry(*c).or_insert(None);
        }
        Notifications {
            clips,
            ranges,
            sources,
            document: n.document,
        }
    }

    /// What the plug-in asked of the transport since the last call.
    pub fn take_playback_requests(&self) -> Vec<crate::callbacks::PlaybackRequest> {
        std::mem::take(
            &mut *self
                .state
                .playback_requests
                .lock()
                .unwrap_or_else(|e| e.into_inner()),
        )
    }
}

impl Drop for HostDocument {
    fn drop(&mut self) {
        // The graph from the leaves up, then the controller, then ARA for this factory.
        let clips: Vec<ClipId> = self.regions.keys().copied().collect();
        let mods: Vec<ClipId> = self.mods.keys().copied().collect();
        let sources: Vec<AudioSourceId> = self.sources.keys().copied().collect();
        self.edit(|d| {
            for c in clips {
                d.destroy_region(c);
            }
            for c in mods {
                d.destroy_mod(c);
            }
            for s in sources {
                d.destroy_source(s);
            }
            for (_, (sref, _, _)) in std::mem::take(&mut d.sequences) {
                // SAFETY: inside the session, no region left.
                unsafe { (d.iface().destroyRegionSequence.unwrap())(d.dc_ref(), sref) };
            }
            if !d.context.is_null() {
                // SAFETY: no sequence left on it.
                unsafe { (d.iface().destroyMusicalContext.unwrap())(d.dc_ref(), d.context) };
            }
        });
        // SAFETY: every object is gone; the host instance outlives this call.
        unsafe { (self.iface().destroyDocumentController.unwrap())(self.dc_ref()) };
        let _ = &self.host;
        release(&self.factory);
    }
}

/// The document controller fields Cantus calls that `iface` lacks: past the size the plug-in
/// gave, or null. Empty when every one is there.
///
/// # Safety
/// `iface` is a plug-in's `ARADocumentControllerInterface`, at least `structSize` long.
pub unsafe fn missing_fields(iface: &ARADocumentControllerInterface) -> Vec<&'static str> {
    let size = { iface.structSize };
    let mut missing = Vec::new();
    macro_rules! need {
        ($($f:ident),*) => {$(
            let end = core::mem::offset_of!(ARADocumentControllerInterface, $f)
                + core::mem::size_of::<usize>();
            if size < end || { iface.$f }.is_none() {
                missing.push(stringify!($f));
            }
        )*};
    }
    need!(
        destroyDocumentController,
        beginEditing,
        endEditing,
        notifyModelUpdates,
        createMusicalContext,
        updateMusicalContextContent,
        destroyMusicalContext,
        createAudioSource,
        enableAudioSourceSamplesAccess,
        destroyAudioSource,
        createAudioModification,
        cloneAudioModification,
        deactivateAudioModificationForUndoHistory,
        destroyAudioModification,
        createPlaybackRegion,
        updatePlaybackRegionProperties,
        destroyPlaybackRegion,
        createRegionSequence,
        updateRegionSequenceProperties,
        destroyRegionSequence,
        getPlaybackRegionHeadAndTailTime,
        restoreObjectsFromArchive,
        storeObjectsToArchive
    );
    missing
}
