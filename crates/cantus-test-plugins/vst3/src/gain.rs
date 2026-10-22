//! Cantus Test Gain: processor, separate controller, editor.

use std::cell::UnsafeCell;
use std::ffi::{c_char, c_void, CStr};
use std::sync::atomic::{AtomicI32, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

use vst3::Steinberg::Vst::BusDirections_::{kInput, kOutput};
use vst3::Steinberg::Vst::BusInfo_::BusFlags_::kDefaultActive;
use vst3::Steinberg::Vst::BusTypes_::{kAux, kMain};
use vst3::Steinberg::Vst::MediaTypes_::kAudio;
use vst3::Steinberg::Vst::ParameterInfo_::ParameterFlags_::{kCanAutomate, kIsHidden, kIsReadOnly};
use vst3::Steinberg::Vst::RestartFlags_::kLatencyChanged;
use vst3::Steinberg::Vst::SymbolicSampleSizes_::kSample32;
use vst3::Steinberg::Vst::{
    BusDirection, BusInfo, IAttributeListTrait, IAudioProcessor, IAudioProcessorTrait, IComponent,
    IComponentHandler, IComponentHandlerTrait, IComponentTrait, IConnectionPoint,
    IConnectionPointTrait, IEditController, IEditControllerTrait, IHostApplication,
    IHostApplicationTrait, IMessage, IMessageTrait, IParamValueQueueTrait, IParameterChangesTrait,
    IProcessContextRequirements, IProcessContextRequirementsTrait, IoMode, MediaType,
    ParameterInfo, ProcessData, ProcessSetup, RoutingInfo, SpeakerArr, SpeakerArrangement,
    String128, TChar,
};
use vst3::Steinberg::{
    kInvalidArgument, kResultFalse, kResultOk, kResultTrue, tresult, FIDString, FUnknown, IBStream,
    IPlugFrame, IPlugFrameTrait, IPlugView, IPlugViewTrait, IPluginBaseTrait, TBool, ViewRect,
    TUID,
};
use vst3::{uid, Class, ComPtr, ComRef, ComWrapper, Interface};

use crate::{bytes, copy_wstring, not_implemented, Live, Registry};

pub const NAME: &str = "Cantus Test Gain";
pub const PROCESSOR_CID: TUID = uid(0x43414E54, 0x55535447, 0x41494E50, 0x524F4331);
pub const CONTROLLER_CID: TUID = uid(0x43414E54, 0x55535447, 0x41494E43, 0x544C5231);

pub const GAIN: u32 = 0;
pub const LATENCY: u32 = 1;
/// Hidden: a failure on purpose (`crate::fault`, round 29).
pub const FAULT: u32 = 2;
/// Hidden and read-only: the id of the process this instance runs in.
pub const PROCESS: u32 = 3;
pub const MAX_LATENCY: u32 = 4096;
const DELAY_LEN: usize = 8192;

/// Gain: normalized 0…1 ↔ −48…+48 dB.
pub fn gain_db(norm: f64) -> f64 {
    -48.0 + 96.0 * norm
}

/// Latency: the SDK's discrete mapping over 4096 steps.
pub fn latency_samples(norm: f64) -> u32 {
    ((norm * f64::from(MAX_LATENCY + 1)).floor() as u32).min(MAX_LATENCY)
}

fn load(a: &AtomicU64) -> f64 {
    f64::from_bits(a.load(Ordering::SeqCst))
}

fn store(a: &AtomicU64, v: f64) {
    a.store(v.to_bits(), Ordering::SeqCst);
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

// ---------------------------------------------------------------------------------------
// Processor

struct Delay {
    lines: [Box<[f32]>; 2],
    write: usize,
}

pub struct Processor {
    _live: Live,
    gain: AtomicU64,
    latency: AtomicU32,
    delay: UnsafeCell<Delay>,
    host: Mutex<Option<ComPtr<IHostApplication>>>,
    peer: Mutex<Option<ComPtr<IConnectionPoint>>>,
}

// SAFETY: `delay` is touched only inside `process`, which the host never calls
// concurrently for one instance.
unsafe impl Sync for Processor {}
unsafe impl Send for Processor {}

impl Class for Processor {
    type Interfaces = (
        IComponent,
        IAudioProcessor,
        IProcessContextRequirements,
        IConnectionPoint,
    );
}

impl Processor {
    pub fn new() -> Processor {
        Processor {
            _live: Live::new(),
            gain: AtomicU64::new(0.5f64.to_bits()),
            latency: AtomicU32::new(0),
            delay: UnsafeCell::new(Delay {
                lines: [
                    vec![0.0; DELAY_LEN].into_boxed_slice(),
                    vec![0.0; DELAY_LEN].into_boxed_slice(),
                ],
                write: 0,
            }),
            host: Mutex::new(None),
            peer: Mutex::new(None),
        }
    }

    /// Tell the controller we started, through a message the host allocates.
    unsafe fn announce(&self) {
        let host = lock(&self.host).clone();
        let peer = lock(&self.peer).clone();
        let (Some(host), Some(peer)) = (host, peer) else {
            return;
        };
        let mut obj = std::ptr::null_mut();
        let mut iid = IMessage::IID.map(|b| b as c_char);
        if host.createInstance(&mut iid, &mut iid, &mut obj) != kResultOk {
            return;
        }
        let Some(msg) = ComPtr::<IMessage>::from_raw(obj as *mut IMessage) else {
            return;
        };
        msg.setMessageID(c"activated".as_ptr());
        if let Some(attrs) = ComRef::from_raw(msg.getAttributes()) {
            attrs.setInt(
                c"latency".as_ptr(),
                i64::from(self.latency.load(Ordering::SeqCst)),
            );
        }
        peer.notify(msg.as_ptr());
    }
}

impl IPluginBaseTrait for Processor {
    unsafe fn initialize(&self, context: *mut FUnknown) -> tresult {
        if let Some(ctx) = ComRef::from_raw(context) {
            *lock(&self.host) = ctx.cast::<IHostApplication>();
        }
        kResultOk
    }

    unsafe fn terminate(&self) -> tresult {
        *lock(&self.host) = None;
        *lock(&self.peer) = None;
        kResultOk
    }
}

fn bus_info(bus: &mut BusInfo, dir: BusDirection, name: &str, main: bool) {
    bus.mediaType = kAudio as MediaType;
    bus.direction = dir;
    bus.channelCount = 2;
    copy_wstring(name, &mut bus.name);
    bus.busType = if main { kMain } else { kAux } as i32;
    bus.flags = if main { kDefaultActive as u32 } else { 0 };
}

impl IComponentTrait for Processor {
    unsafe fn getControllerClassId(&self, class_id: *mut TUID) -> tresult {
        *class_id = CONTROLLER_CID;
        kResultOk
    }

    unsafe fn setIoMode(&self, _mode: IoMode) -> tresult {
        kResultOk
    }

    unsafe fn getBusCount(&self, media: MediaType, dir: BusDirection) -> i32 {
        match (media == kAudio as i32, dir) {
            (true, d) if d == kInput as i32 => 2,
            (true, d) if d == kOutput as i32 => 1,
            _ => 0,
        }
    }

    unsafe fn getBusInfo(
        &self,
        media: MediaType,
        dir: BusDirection,
        index: i32,
        bus: *mut BusInfo,
    ) -> tresult {
        if media != kAudio as i32 || bus.is_null() {
            return kInvalidArgument;
        }
        let bus = &mut *bus;
        match (dir, index) {
            (d, 0) if d == kInput as i32 => bus_info(bus, d, "Input", true),
            (d, 1) if d == kInput as i32 => bus_info(bus, d, "Sidechain", false),
            (d, 0) if d == kOutput as i32 => bus_info(bus, d, "Output", true),
            _ => return kInvalidArgument,
        }
        kResultOk
    }

    unsafe fn getRoutingInfo(&self, _in: *mut RoutingInfo, _out: *mut RoutingInfo) -> tresult {
        not_implemented()
    }

    unsafe fn activateBus(&self, _m: MediaType, _d: BusDirection, _i: i32, _s: TBool) -> tresult {
        kResultOk
    }

    unsafe fn setActive(&self, state: TBool) -> tresult {
        if state != 0 {
            crate::fault::fire(crate::fault::ABORT_ACTIVATE);
            let d = &mut *self.delay.get();
            for line in &mut d.lines {
                line.fill(0.0);
            }
            self.announce();
        }
        kResultOk
    }

    unsafe fn setState(&self, state: *mut IBStream) -> tresult {
        crate::fault::fire(crate::fault::ABORT_LOAD);
        let (Some(magic), Some(g), Some(l)) = (
            bytes::read::<4>(state),
            bytes::read_f64(state),
            bytes::read_f64(state),
        ) else {
            return kResultFalse;
        };
        if &magic != b"CTGS" {
            return kResultFalse;
        }
        store(&self.gain, g.clamp(0.0, 1.0));
        self.latency.store(latency_samples(l), Ordering::SeqCst);
        kResultOk
    }

    unsafe fn getState(&self, state: *mut IBStream) -> tresult {
        crate::fault::fire(crate::fault::ABORT_SAVE);
        let latency = f64::from(self.latency.load(Ordering::SeqCst)) / f64::from(MAX_LATENCY);
        let ok = bytes::write(state, b"CTGS")
            && bytes::write(state, &load(&self.gain).to_le_bytes())
            && bytes::write(state, &latency.to_le_bytes());
        if ok {
            kResultOk
        } else {
            kResultFalse
        }
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
        let ins = std::slice::from_raw_parts(inputs, num_ins.max(0) as usize);
        let outs = std::slice::from_raw_parts(outputs, num_outs.max(0) as usize);
        let stereo = |a: &[SpeakerArrangement]| a.iter().all(|&x| x == SpeakerArr::kStereo);
        if (1..=2).contains(&ins.len()) && outs.len() == 1 && stereo(ins) && stereo(outs) {
            kResultTrue
        } else {
            kResultFalse
        }
    }

    unsafe fn getBusArrangement(
        &self,
        dir: BusDirection,
        index: i32,
        arr: *mut SpeakerArrangement,
    ) -> tresult {
        let valid = (dir == kInput as i32 && (0..2).contains(&index))
            || (dir == kOutput as i32 && index == 0);
        if !valid || arr.is_null() {
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
        self.latency.load(Ordering::SeqCst)
    }

    unsafe fn setupProcessing(&self, _setup: *mut ProcessSetup) -> tresult {
        kResultOk
    }

    unsafe fn setProcessing(&self, _state: TBool) -> tresult {
        kResultOk
    }

    unsafe fn process(&self, data: *mut ProcessData) -> tresult {
        let data = &*data;
        // Gain points of this block, in offset order (at most 64 kept).
        let mut points = [(0i32, 0.0f64); 64];
        let mut count = 0;
        if let Some(changes) = ComRef::from_raw(data.inputParameterChanges) {
            for i in 0..changes.getParameterCount() {
                let Some(q) = ComRef::from_raw(changes.getParameterData(i)) else {
                    continue;
                };
                let id = q.getParameterId();
                for p in 0..q.getPointCount() {
                    let (mut offset, mut value) = (0, 0.0);
                    if q.getPoint(p, &mut offset, &mut value) != kResultOk {
                        continue;
                    }
                    if id == GAIN && count < points.len() {
                        points[count] = (offset, value);
                        count += 1;
                    } else if id == LATENCY {
                        self.latency.store(latency_samples(value), Ordering::SeqCst);
                    } else if id == FAULT {
                        crate::fault::arm(value);
                    }
                }
            }
        }
        let n = data.numSamples.max(0) as usize;
        if n == 0 || data.numInputs < 1 || data.numOutputs < 1 {
            if let Some(&(_, v)) = points[..count].last() {
                store(&self.gain, v);
            }
            return kResultOk;
        }
        let input = &*data.inputs;
        let output = &*data.outputs;
        if input.numChannels < 2 || output.numChannels < 2 {
            return kResultFalse;
        }
        let ins = std::slice::from_raw_parts(input.__field0.channelBuffers32, 2);
        let outs = std::slice::from_raw_parts(output.__field0.channelBuffers32, 2);
        // The sidechain is only watched: its loudest sample and silence flags.
        if data.numInputs >= 2 {
            let key = &*data.inputs.add(1);
            let mut peak = 0.0f32;
            if key.numChannels >= 1 {
                let chans = std::slice::from_raw_parts(
                    key.__field0.channelBuffers32,
                    key.numChannels as usize,
                );
                for &c in chans {
                    for &x in std::slice::from_raw_parts(c, n) {
                        peak = peak.max(x.abs());
                    }
                }
            }
            KEY_PEAK.store(f64::from(peak).to_bits(), Ordering::SeqCst);
            KEY_SILENCE.store(key.silenceFlags, Ordering::SeqCst);
        }
        let latency = self.latency.load(Ordering::SeqCst) as usize;
        let d = &mut *self.delay.get();
        let mut norm = load(&self.gain);
        let mut factor = 10f64.powf(gain_db(norm) / 20.0) as f32;
        let mut next = 0;
        let start = d.write;
        for c in 0..2 {
            let x = std::slice::from_raw_parts(ins[c], n);
            let y = std::slice::from_raw_parts_mut(outs[c], n);
            let line = &mut d.lines[c];
            let (mut w, mut k) = (start, 0);
            if c == 1 {
                // The second channel replays the gain changes from the start.
                norm = load(&self.gain);
                factor = 10f64.powf(gain_db(norm) / 20.0) as f32;
                next = 0;
            }
            for i in 0..n {
                while next < count && points[next].0 as usize <= i {
                    norm = points[next].1;
                    factor = 10f64.powf(gain_db(norm) / 20.0) as f32;
                    next += 1;
                }
                line[w] = x[i];
                let delayed = line[(w + DELAY_LEN - latency) % DELAY_LEN];
                y[i] = if factor == 1.0 {
                    delayed
                } else {
                    delayed * factor
                };
                w = (w + 1) % DELAY_LEN;
                k += 1;
            }
            d.write = (start + k) % DELAY_LEN;
        }
        store(&self.gain, norm);
        kResultOk
    }

    unsafe fn getTailSamples(&self) -> u32 {
        0
    }
}

impl IProcessContextRequirementsTrait for Processor {
    unsafe fn getProcessContextRequirements(&self) -> u32 {
        0
    }
}

impl IConnectionPointTrait for Processor {
    unsafe fn connect(&self, other: *mut IConnectionPoint) -> tresult {
        *lock(&self.peer) = ComRef::from_raw(other).map(|o| o.to_com_ptr());
        kResultOk
    }

    unsafe fn disconnect(&self, _other: *mut IConnectionPoint) -> tresult {
        *lock(&self.peer) = None;
        kResultOk
    }

    unsafe fn notify(&self, _message: *mut IMessage) -> tresult {
        kResultOk
    }
}

// ---------------------------------------------------------------------------------------
// Controller

pub static CONTROLLERS: Registry = Registry::new();
pub static VIEWS: Registry = Registry::new();
pub static MESSAGES: AtomicI32 = AtomicI32::new(0);
/// The sidechain in the last block: loudest sample, and the host's silence flags.
pub static KEY_PEAK: AtomicU64 = AtomicU64::new(0);
pub static KEY_SILENCE: AtomicU64 = AtomicU64::new(0);

pub struct Controller {
    _live: Live,
    gain: AtomicU64,
    latency: AtomicU64,
    /// The Fault parameter's normalised value, as the controller was told it.
    fault: AtomicU64,
    zoom: AtomicU64,
    handler: Mutex<Option<ComPtr<IComponentHandler>>>,
}

impl Class for Controller {
    type Interfaces = (IEditController, IConnectionPoint);
}

impl Controller {
    pub fn new() -> Controller {
        Controller {
            _live: Live::new(),
            gain: AtomicU64::new(0.5f64.to_bits()),
            latency: AtomicU64::new(0.0f64.to_bits()),
            fault: AtomicU64::new(0.0f64.to_bits()),
            zoom: AtomicU64::new(1.0f64.to_bits()),
            handler: Mutex::new(None),
        }
    }

    fn at(&self) -> usize {
        self as *const Controller as usize
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        CONTROLLERS.remove(self.at());
    }
}

impl IPluginBaseTrait for Controller {
    unsafe fn initialize(&self, _context: *mut FUnknown) -> tresult {
        CONTROLLERS.add(self.at());
        kResultOk
    }

    unsafe fn terminate(&self) -> tresult {
        CONTROLLERS.remove(self.at());
        *lock(&self.handler) = None;
        kResultOk
    }
}

const PARAMS: [(u32, &str, &str, i32, f64); 4] = [
    (GAIN, "Gain", "dB", 0, 0.5),
    (LATENCY, "Latency", "samples", MAX_LATENCY as i32, 0.0),
    (FAULT, "Fault", "", crate::fault::STEPS, 0.0),
    (PROCESS, "Process", "", 0, 0.0),
];

impl IEditControllerTrait for Controller {
    unsafe fn setComponentState(&self, state: *mut IBStream) -> tresult {
        let (Some(magic), Some(g), Some(l)) = (
            bytes::read::<4>(state),
            bytes::read_f64(state),
            bytes::read_f64(state),
        ) else {
            return kResultFalse;
        };
        if &magic != b"CTGS" {
            return kResultFalse;
        }
        store(&self.gain, g);
        store(&self.latency, l);
        kResultOk
    }

    unsafe fn setState(&self, state: *mut IBStream) -> tresult {
        match (bytes::read::<4>(state), bytes::read_f64(state)) {
            (Some(m), Some(z)) if &m == b"CTGC" => {
                store(&self.zoom, z);
                kResultOk
            }
            _ => kResultFalse,
        }
    }

    unsafe fn getState(&self, state: *mut IBStream) -> tresult {
        if bytes::write(state, b"CTGC") && bytes::write(state, &load(&self.zoom).to_le_bytes()) {
            kResultOk
        } else {
            kResultFalse
        }
    }

    unsafe fn getParameterCount(&self) -> i32 {
        PARAMS.len() as i32
    }

    unsafe fn getParameterInfo(&self, index: i32, info: *mut ParameterInfo) -> tresult {
        let Some(&(id, title, units, steps, default)) = PARAMS.get(index as usize) else {
            return kInvalidArgument;
        };
        let info = &mut *info;
        info.id = id;
        copy_wstring(title, &mut info.title);
        copy_wstring(title, &mut info.shortTitle);
        copy_wstring(units, &mut info.units);
        info.stepCount = steps;
        info.defaultNormalizedValue = default;
        info.unitId = 0;
        info.flags = match id {
            FAULT => kIsHidden as i32,
            PROCESS => (kIsHidden | kIsReadOnly) as i32,
            _ => kCanAutomate as i32,
        };
        kResultOk
    }

    unsafe fn getParamStringByValue(&self, id: u32, norm: f64, string: *mut String128) -> tresult {
        let text = match id {
            GAIN => format!("{:.1}", gain_db(norm)),
            LATENCY => format!("{}", latency_samples(norm)),
            FAULT | PROCESS => format!("{}", self.normalizedParamToPlain(id, norm).round()),
            _ => return kInvalidArgument,
        };
        copy_wstring(&text, &mut *string);
        kResultOk
    }

    unsafe fn getParamValueByString(&self, id: u32, string: *mut TChar, norm: *mut f64) -> tresult {
        let mut len = 0;
        while *string.add(len) != 0 {
            len += 1;
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(string, len));
        let Ok(v) = text.trim().parse::<f64>() else {
            return kResultFalse;
        };
        *norm = self.plainParamToNormalized(id, v);
        kResultOk
    }

    unsafe fn normalizedParamToPlain(&self, id: u32, norm: f64) -> f64 {
        match id {
            GAIN => gain_db(norm),
            LATENCY => f64::from(latency_samples(norm)),
            FAULT => (norm * f64::from(crate::fault::STEPS)).round(),
            PROCESS => norm * crate::fault::PROCESS_RANGE,
            _ => norm,
        }
    }

    unsafe fn plainParamToNormalized(&self, id: u32, plain: f64) -> f64 {
        match id {
            GAIN => ((plain + 48.0) / 96.0).clamp(0.0, 1.0),
            LATENCY => (plain / f64::from(MAX_LATENCY)).clamp(0.0, 1.0),
            FAULT => (plain / f64::from(crate::fault::STEPS)).clamp(0.0, 1.0),
            PROCESS => (plain / crate::fault::PROCESS_RANGE).clamp(0.0, 1.0),
            _ => plain,
        }
    }

    unsafe fn getParamNormalized(&self, id: u32) -> f64 {
        match id {
            GAIN => load(&self.gain),
            LATENCY => load(&self.latency),
            FAULT => load(&self.fault),
            PROCESS => crate::fault::process_norm(),
            _ => 0.0,
        }
    }

    unsafe fn setParamNormalized(&self, id: u32, value: f64) -> tresult {
        match id {
            GAIN => store(&self.gain, value),
            LATENCY => {
                let before = latency_samples(load(&self.latency));
                store(&self.latency, value);
                if latency_samples(value) != before {
                    let handler = lock(&self.handler).clone();
                    if let Some(h) = handler {
                        h.restartComponent(kLatencyChanged);
                    }
                }
            }
            // The controller only keeps it: a fault arms in `process`.
            FAULT => store(&self.fault, value),
            PROCESS => {}
            _ => return kInvalidArgument,
        }
        kResultOk
    }

    unsafe fn setComponentHandler(&self, handler: *mut IComponentHandler) -> tresult {
        *lock(&self.handler) = ComRef::from_raw(handler).map(|h| h.to_com_ptr());
        kResultOk
    }

    unsafe fn createView(&self, name: FIDString) -> *mut IPlugView {
        if name.is_null() || CStr::from_ptr(name) != c"editor" {
            return std::ptr::null_mut();
        }
        crate::fault::fire(crate::fault::ABORT_EDITOR);
        let view = ComWrapper::new(View::new());
        VIEWS.add(&*view as *const View as usize);
        match view.to_com_ptr::<IPlugView>() {
            Some(p) => {
                view.me.store(p.as_ptr() as usize, Ordering::SeqCst);
                p.into_raw()
            }
            None => std::ptr::null_mut(),
        }
    }
}

impl IConnectionPointTrait for Controller {
    unsafe fn connect(&self, _other: *mut IConnectionPoint) -> tresult {
        kResultOk
    }

    unsafe fn disconnect(&self, _other: *mut IConnectionPoint) -> tresult {
        kResultOk
    }

    unsafe fn notify(&self, message: *mut IMessage) -> tresult {
        let Some(msg) = ComRef::from_raw(message) else {
            return kInvalidArgument;
        };
        let id = msg.getMessageID();
        if !id.is_null() && CStr::from_ptr(id) == c"activated" {
            MESSAGES.fetch_add(1, Ordering::SeqCst);
        }
        kResultOk
    }
}

/// The newest controller, while the registry is locked.
fn with_newest_controller<R>(f: impl FnOnce(&Controller) -> R) -> Option<R> {
    let at = CONTROLLERS.newest()?;
    // SAFETY: a controller is in the registry only while it is alive (initialize …
    // terminate/drop), and the tests do not destroy one while calling this.
    Some(f(unsafe { &*(at as *const Controller) }))
}

pub fn perform_edit(id: u32, norm: f64) -> i32 {
    with_newest_controller(|c| {
        let handler = lock(&c.handler).clone();
        let Some(h) = handler else { return 0 };
        // SAFETY: host callbacks, called as an editor would, on the caller's thread.
        unsafe {
            h.beginEdit(id);
            c.setParamNormalized(id, norm);
            h.performEdit(id, norm);
            h.endEdit(id);
        }
        1
    })
    .unwrap_or(0)
}

pub fn zoom(set: f64) -> f64 {
    with_newest_controller(|c| {
        if set >= 0.0 {
            store(&c.zoom, set);
        }
        load(&c.zoom)
    })
    .unwrap_or(-1.0)
}

// ---------------------------------------------------------------------------------------
// Editor

pub struct View {
    _live: Live,
    /// This view's own `IPlugView*` (not a reference: set once, for `resizeView`).
    me: AtomicUsize,
    parent: AtomicUsize,
    size: Mutex<(i32, i32)>,
    frame: Mutex<Option<ComPtr<IPlugFrame>>>,
}

impl View {
    fn new() -> View {
        View {
            _live: Live::new(),
            me: AtomicUsize::new(0),
            parent: AtomicUsize::new(0),
            size: Mutex::new((400, 300)),
            frame: Mutex::new(None),
        }
    }
}

impl Drop for View {
    fn drop(&mut self) {
        VIEWS.remove(self as *const View as usize);
    }
}

impl Class for View {
    type Interfaces = (IPlugView,);
}

impl IPlugViewTrait for View {
    unsafe fn isPlatformTypeSupported(&self, kind: FIDString) -> tresult {
        if kind.is_null() {
            return kInvalidArgument;
        }
        let kind = CStr::from_ptr(kind);
        if kind == c"HWND" || kind == c"X11EmbedWindowID" {
            kResultTrue
        } else {
            kResultFalse
        }
    }

    unsafe fn attached(&self, parent: *mut c_void, _kind: FIDString) -> tresult {
        if parent.is_null() {
            return kInvalidArgument;
        }
        self.parent.store(parent as usize, Ordering::SeqCst);
        kResultOk
    }

    unsafe fn removed(&self) -> tresult {
        self.parent.store(0, Ordering::SeqCst);
        kResultOk
    }

    unsafe fn onWheel(&self, _distance: f32) -> tresult {
        kResultFalse
    }

    unsafe fn onKeyDown(&self, _key: u16, _code: i16, _modifiers: i16) -> tresult {
        kResultFalse
    }

    unsafe fn onKeyUp(&self, _key: u16, _code: i16, _modifiers: i16) -> tresult {
        kResultFalse
    }

    unsafe fn getSize(&self, rect: *mut ViewRect) -> tresult {
        if rect.is_null() {
            return kInvalidArgument;
        }
        let (w, h) = *lock(&self.size);
        *rect = ViewRect {
            left: 0,
            top: 0,
            right: w,
            bottom: h,
        };
        kResultOk
    }

    unsafe fn onSize(&self, rect: *mut ViewRect) -> tresult {
        if rect.is_null() {
            return kInvalidArgument;
        }
        let r = &*rect;
        *lock(&self.size) = (r.right - r.left, r.bottom - r.top);
        kResultOk
    }

    unsafe fn onFocus(&self, _state: TBool) -> tresult {
        kResultOk
    }

    unsafe fn setFrame(&self, frame: *mut IPlugFrame) -> tresult {
        *lock(&self.frame) = ComRef::from_raw(frame).map(|f| f.to_com_ptr());
        kResultOk
    }

    unsafe fn canResize(&self) -> tresult {
        kResultTrue
    }

    unsafe fn checkSizeConstraint(&self, _rect: *mut ViewRect) -> tresult {
        kResultTrue
    }
}

fn with_newest_view<R>(f: impl FnOnce(&View) -> R) -> Option<R> {
    let at = VIEWS.newest()?;
    // SAFETY: as for controllers: registered only while alive.
    Some(f(unsafe { &*(at as *const View) }))
}

pub fn resize_editor(width: i32, height: i32) -> i32 {
    with_newest_view(|v| {
        let frame = lock(&v.frame).clone();
        let Some(frame) = frame else { return 0 };
        let mut rect = ViewRect {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };
        let me = v.me.load(Ordering::SeqCst) as *mut IPlugView;
        // SAFETY: the view asks its host frame to resize it, as an editor would.
        let r = unsafe { frame.resizeView(me, &mut rect) };
        i32::from(r == kResultOk)
    })
    .unwrap_or(0)
}

pub fn editor(width: *mut i32, height: *mut i32) -> usize {
    with_newest_view(|v| {
        let (w, h) = *lock(&v.size);
        // SAFETY: the caller passes valid out-pointers or null.
        unsafe {
            if !width.is_null() {
                *width = w;
            }
            if !height.is_null() {
                *height = h;
            }
        }
        v.parent.load(Ordering::SeqCst)
    })
    .unwrap_or(0)
}
