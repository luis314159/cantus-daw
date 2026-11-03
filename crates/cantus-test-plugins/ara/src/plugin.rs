//! The audio processor: a stereo effect that, bound to an ARA document controller as a
//! playback renderer, renders the playback regions assigned to it (and passes its input
//! through when it is not bound).

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;

use cantus_ara::sys::*;
use cantus_ara::vst3::{
    IMainFactory, IMainFactoryTrait, IPlugInEntryPoint, IPlugInEntryPoint2,
    IPlugInEntryPoint2Trait, IPlugInEntryPointTrait,
};
use vst3::Steinberg::Vst::BusDirections_::kInput;
use vst3::Steinberg::Vst::BusInfo_::BusFlags_::kDefaultActive;
use vst3::Steinberg::Vst::BusTypes_::kMain;
use vst3::Steinberg::Vst::MediaTypes_::kAudio;
use vst3::Steinberg::Vst::SymbolicSampleSizes_::kSample32;
use vst3::Steinberg::Vst::{
    BusDirection, BusInfo, IAudioProcessor, IAudioProcessorTrait, IComponent, IComponentTrait,
    IoMode, MediaType, ProcessData, ProcessSetup, RoutingInfo, SpeakerArr, SpeakerArrangement,
};
use vst3::Steinberg::{
    kInvalidArgument, kResultFalse, kResultOk, kResultTrue, tresult, FUnknown, IBStream,
    IPluginBaseTrait, TBool, TUID,
};
use vst3::{uid, Class};

use crate::ara::{violation, Controller, FACTORY};
use crate::{copy_wstring, Live};

pub const NAME: &str = "Cantus Test ARA";
pub const CID: TUID = uid(0x41524154, 0x45535450, 0x4C554749, 0x4E000001);
pub const MAIN_FACTORY_CID: TUID = uid(0x41524154, 0x45535450, 0x4C554749, 0x4E000002);

/// Blocks rendered with at least one region, by every instance (the tests read it).
pub static RENDERED_BLOCKS: AtomicU64 = AtomicU64::new(0);
/// Instances bound now.
pub static BOUND: AtomicI32 = AtomicI32::new(0);
/// The latency every instance reports, and delays its output by (the tests set it).
pub static LATENCY: AtomicU32 = AtomicU32::new(0);
/// The next render aborts the process (a plug-in that crashes).
pub static CRASH: AtomicBool = AtomicBool::new(false);
/// The next render sleeps this long first (a plug-in that hangs).
pub static HANG_MS: AtomicU32 = AtomicU32::new(0);
/// Selections the hosts' editor views were told of.
pub static SELECTIONS: AtomicI32 = AtomicI32::new(0);

/// A bound instance's ARA side: its extension instance (handed to the host) and its
/// playback renderer's regions. Its address is the renderer's and extension's ref.
pub struct Extension {
    instance: ARAPlugInExtensionInstance,
    dc: usize,
    pub regions: Mutex<Vec<usize>>,
    active: AtomicBool,
}

/// Live extensions, so a destroyed region still held by a renderer is caught.
static EXTENSIONS: Mutex<Vec<usize>> = Mutex::new(Vec::new());

pub fn region_in_use(region: usize) -> bool {
    EXTENSIONS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .any(|&e| {
            // SAFETY: a live extension (removed from the list before it is freed).
            let e = unsafe { &*(e as *const Extension) };
            e.regions
                .lock()
                .unwrap_or_else(|x| x.into_inner())
                .contains(&region)
        })
}

unsafe fn ext<'a>(r: ARAPlaybackRendererRef) -> &'a Extension {
    &*(r as *const Extension)
}

unsafe extern "C" fn add_region(r: ARAPlaybackRendererRef, g: ARAPlaybackRegionRef) {
    let e = ext(r);
    if e.active.load(Ordering::SeqCst) {
        violation("addPlaybackRegion while the instance is active");
    }
    e.regions
        .lock()
        .unwrap_or_else(|x| x.into_inner())
        .push(g as usize);
}

unsafe extern "C" fn remove_region(r: ARAPlaybackRendererRef, g: ARAPlaybackRegionRef) {
    let e = ext(r);
    if e.active.load(Ordering::SeqCst) {
        violation("removePlaybackRegion while the instance is active");
    }
    e.regions
        .lock()
        .unwrap_or_else(|x| x.into_inner())
        .retain(|&x| x != g as usize);
}

unsafe extern "C" fn editor_add(_r: ARAEditorRendererRef, _g: ARAPlaybackRegionRef) {}
unsafe extern "C" fn editor_seq(_r: ARAEditorRendererRef, _s: ARARegionSequenceRef) {}

static EDITOR_RENDERER: ARAEditorRendererInterface = ARAEditorRendererInterface {
    structSize: std::mem::size_of::<ARAEditorRendererInterface>(),
    addPlaybackRegion: Some(editor_add),
    removePlaybackRegion: Some(editor_add),
    addRegionSequence: Some(editor_seq),
    removeRegionSequence: Some(editor_seq),
};

unsafe extern "C" fn notify_selection(_v: ARAEditorViewRef, s: *const ARAViewSelection) {
    if !s.is_null() && std::ptr::read_unaligned(s).playbackRegionRefsCount > 0 {
        SELECTIONS.fetch_add(1, Ordering::SeqCst);
    }
}
unsafe extern "C" fn hide_sequences(
    _v: ARAEditorViewRef,
    _n: ARASize,
    _s: *const ARARegionSequenceRef,
) {
}

static EDITOR_VIEW: ARAEditorViewInterface = ARAEditorViewInterface {
    structSize: std::mem::size_of::<ARAEditorViewInterface>(),
    notifySelection: Some(notify_selection),
    notifyHideRegionSequences: Some(hide_sequences),
};

static RENDERER: ARAPlaybackRendererInterface = ARAPlaybackRendererInterface {
    structSize: std::mem::size_of::<ARAPlaybackRendererInterface>(),
    addPlaybackRegion: Some(add_region),
    removePlaybackRegion: Some(remove_region),
};

pub struct Processor {
    _live: Live,
    active: AtomicBool,
    rate: Mutex<f64>,
    ext: Mutex<Option<Box<Extension>>>,
}

impl Processor {
    pub fn new() -> Processor {
        Processor {
            _live: Live::new(),
            active: AtomicBool::new(false),
            rate: Mutex::new(48_000.0),
            ext: Mutex::new(None),
        }
    }
}

impl Default for Processor {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Processor {
    fn drop(&mut self) {
        if let Some(e) = self.ext.lock().unwrap_or_else(|x| x.into_inner()).take() {
            let at = &*e as *const Extension as usize;
            EXTENSIONS
                .lock()
                .unwrap_or_else(|x| x.into_inner())
                .retain(|&x| x != at);
            BOUND.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

impl Class for Processor {
    type Interfaces = (
        IComponent,
        IAudioProcessor,
        IPlugInEntryPoint,
        IPlugInEntryPoint2,
    );
}

impl IPluginBaseTrait for Processor {
    unsafe fn initialize(&self, _context: *mut FUnknown) -> tresult {
        kResultOk
    }

    unsafe fn terminate(&self) -> tresult {
        kResultOk
    }
}

impl IPlugInEntryPointTrait for Processor {
    unsafe fn getFactory(&self) -> *const ARAFactory {
        &FACTORY.0
    }
}

impl IPlugInEntryPoint2Trait for Processor {
    unsafe fn bindToDocumentControllerWithRoles(
        &self,
        dc: ARADocumentControllerRef,
        _known: ARAPlugInInstanceRoleFlags,
        assigned: ARAPlugInInstanceRoleFlags,
    ) -> *const ARAPlugInExtensionInstance {
        if self.active.load(Ordering::SeqCst) {
            violation("bound after setActive");
        }
        let mut slot = self.ext.lock().unwrap_or_else(|x| x.into_inner());
        if slot.is_some() {
            violation("bound twice");
            return std::ptr::null();
        }
        let mut e = Box::new(Extension {
            // SAFETY: every field is a pointer, a size or an option; filled below.
            instance: std::mem::zeroed(),
            dc: dc as usize,
            regions: Mutex::new(Vec::new()),
            active: AtomicBool::new(false),
        });
        let me = &*e as *const Extension as *mut std::ffi::c_void;
        e.instance.structSize = std::mem::size_of::<ARAPlugInExtensionInstance>();
        if assigned & kARAPlaybackRendererRole != 0 {
            e.instance.playbackRendererRef = me as ARAPlaybackRendererRef;
            e.instance.playbackRendererInterface = &RENDERER;
        }
        if assigned & kARAEditorRendererRole != 0 {
            e.instance.editorRendererRef = me as ARAEditorRendererRef;
            e.instance.editorRendererInterface = &EDITOR_RENDERER;
        }
        if assigned & kARAEditorViewRole != 0 {
            e.instance.editorViewRef = me as ARAEditorViewRef;
            e.instance.editorViewInterface = &EDITOR_VIEW;
        }
        let out = &e.instance as *const ARAPlugInExtensionInstance;
        EXTENSIONS
            .lock()
            .unwrap_or_else(|x| x.into_inner())
            .push(me as usize);
        BOUND.fetch_add(1, Ordering::SeqCst);
        *slot = Some(e);
        out
    }
}

impl IMainFactoryTrait for MainFactory {
    unsafe fn getFactory(&self) -> *const ARAFactory {
        &FACTORY.0
    }
}

/// The factory class "ARA Main Factory Class": the ARA factory without a processor.
pub struct MainFactory {
    _live: Live,
}

impl MainFactory {
    pub fn new() -> MainFactory {
        MainFactory { _live: Live::new() }
    }
}

impl Default for MainFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl Class for MainFactory {
    type Interfaces = (IMainFactory,);
}

impl IComponentTrait for Processor {
    unsafe fn getControllerClassId(&self, _class_id: *mut TUID) -> tresult {
        kResultFalse
    }

    unsafe fn setIoMode(&self, _mode: IoMode) -> tresult {
        kResultOk
    }

    unsafe fn getBusCount(&self, media: MediaType, _dir: BusDirection) -> i32 {
        i32::from(media == kAudio as i32)
    }

    unsafe fn getBusInfo(
        &self,
        media: MediaType,
        dir: BusDirection,
        index: i32,
        bus: *mut BusInfo,
    ) -> tresult {
        if index != 0 || bus.is_null() || media != kAudio as i32 {
            return kInvalidArgument;
        }
        let bus = &mut *bus;
        bus.channelCount = 2;
        copy_wstring(
            if dir == kInput as i32 {
                "Input"
            } else {
                "Output"
            },
            &mut bus.name,
        );
        bus.mediaType = media;
        bus.direction = dir;
        bus.busType = kMain as i32;
        bus.flags = kDefaultActive as u32;
        kResultOk
    }

    unsafe fn getRoutingInfo(&self, _in: *mut RoutingInfo, _out: *mut RoutingInfo) -> tresult {
        kResultFalse
    }

    unsafe fn activateBus(&self, _m: MediaType, _d: BusDirection, _i: i32, _s: TBool) -> tresult {
        kResultOk
    }

    unsafe fn setActive(&self, state: TBool) -> tresult {
        let on = state != 0;
        self.active.store(on, Ordering::SeqCst);
        if let Some(e) = self.ext.lock().unwrap_or_else(|x| x.into_inner()).as_ref() {
            e.active.store(on, Ordering::SeqCst);
        }
        kResultOk
    }

    unsafe fn setState(&self, _state: *mut IBStream) -> tresult {
        kResultOk
    }

    unsafe fn getState(&self, _state: *mut IBStream) -> tresult {
        kResultOk
    }
}

impl IAudioProcessorTrait for Processor {
    unsafe fn setBusArrangements(
        &self,
        inputs: *mut SpeakerArrangement,
        num_ins: i32,
        outputs: *mut SpeakerArrangement,
        num_outs: i32,
    ) -> tresult {
        let stereo = |p: *mut SpeakerArrangement, n: i32| n == 1 && *p == SpeakerArr::kStereo;
        if stereo(inputs, num_ins) && stereo(outputs, num_outs) {
            kResultTrue
        } else {
            kResultFalse
        }
    }

    unsafe fn getBusArrangement(
        &self,
        _dir: BusDirection,
        index: i32,
        arr: *mut SpeakerArrangement,
    ) -> tresult {
        if index != 0 || arr.is_null() {
            return kInvalidArgument;
        }
        *arr = SpeakerArr::kStereo;
        kResultOk
    }

    unsafe fn canProcessSampleSize(&self, size: i32) -> tresult {
        if size == kSample32 as i32 {
            kResultOk
        } else {
            kResultFalse
        }
    }

    unsafe fn getLatencySamples(&self) -> u32 {
        LATENCY.load(Ordering::SeqCst)
    }

    unsafe fn setupProcessing(&self, setup: *mut ProcessSetup) -> tresult {
        *self.rate.lock().unwrap_or_else(|x| x.into_inner()) = (*setup).sampleRate;
        kResultOk
    }

    unsafe fn setProcessing(&self, _state: TBool) -> tresult {
        kResultOk
    }

    unsafe fn process(&self, data: *mut ProcessData) -> tresult {
        let data = &*data;
        let n = data.numSamples.max(0) as usize;
        if n == 0 || data.numOutputs < 1 {
            return kResultOk;
        }
        let out = &*data.outputs;
        let chans = std::slice::from_raw_parts(out.__field0.channelBuffers32, 2);
        let left = std::slice::from_raw_parts_mut(chans[0], n);
        let right = std::slice::from_raw_parts_mut(chans[1], n);
        let guard = self.ext.lock().unwrap_or_else(|x| x.into_inner());
        let Some(e) = guard.as_ref() else {
            // Not bound: pass the input through.
            if data.numInputs >= 1 {
                let inp = &*data.inputs;
                let ic = std::slice::from_raw_parts(inp.__field0.channelBuffers32, 2);
                left.copy_from_slice(std::slice::from_raw_parts(ic[0], n));
                right.copy_from_slice(std::slice::from_raw_parts(ic[1], n));
            }
            return kResultOk;
        };
        left.fill(0.0);
        right.fill(0.0);
        let Some(ctx) = data.processContext.as_ref() else {
            return kResultOk;
        };
        let rate = *self.rate.lock().unwrap_or_else(|x| x.into_inner());
        if CRASH.load(Ordering::SeqCst) {
            std::process::abort();
        }
        let hang = HANG_MS.swap(0, Ordering::SeqCst);
        if hang > 0 {
            std::thread::sleep(std::time::Duration::from_millis(u64::from(hang)));
        }
        // A plug-in with latency plays what belongs that many samples earlier.
        let at = ctx.projectTimeSamples - i64::from(LATENCY.load(Ordering::SeqCst));
        // SAFETY: the controller outlives every instance bound to it (the host's rule).
        let c = &*(e.dc as *const Controller);
        let regions: Vec<usize> = e.regions.lock().unwrap_or_else(|x| x.into_inner()).clone();
        let mut any = false;
        for g in regions {
            let (reg, source, gain, active) = {
                let m = c.model();
                let Some(reg) = m.regions.get(&g).cloned() else {
                    continue;
                };
                let Some(md) = m.mods.get(&reg.modification) else {
                    continue;
                };
                (reg, md.source, md.gain, md.active)
            };
            if !active {
                violation("rendering an inactive modification");
                continue;
            }
            let f = |s: f64| (s * rate).round() as i64;
            let (p0, s0, len) = (f(reg.start_play), f(reg.start_mod), f(reg.duration));
            let from = at.max(p0);
            let to = (at + n as i64).min(p0 + len);
            if from >= to {
                continue;
            }
            let count = (to - from) as usize;
            let Some(bufs) = c.read(source, from - p0 + s0, count) else {
                continue;
            };
            let off = (from - at) as usize;
            let (l, r) = (&bufs[0], bufs.get(1).unwrap_or(&bufs[0]));
            for i in 0..count {
                left[off + i] += l[i] * gain;
                right[off + i] += r[i] * gain;
            }
            any = true;
        }
        if any {
            RENDERED_BLOCKS.fetch_add(1, Ordering::SeqCst);
        }
        kResultOk
    }

    unsafe fn getTailSamples(&self) -> u32 {
        0
    }
}
