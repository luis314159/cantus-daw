//! One VST3 instance: the component, the processor and the controller in one object.
//!
//! Threads: `IAudioProcessor::process` on the audio thread; everything else on the UI
//! thread, and — as the standard promises — never `setActive`/`setupProcessing` while
//! `process` runs. The processor half lives in an `UnsafeCell` touched only there.

use super::{copy_wstr, read_wstr, timer};
use crate::desc::Descriptor;
use crate::ftz::DenormalGuard;
use crate::instance::{ParamEvent, Processor, Shared};
use crate::params::{Param, Role};
use ::vst3::Steinberg::Vst::*;
use ::vst3::Steinberg::*;
use ::vst3::{Class, ComRef};
use std::cell::{Cell, UnsafeCell};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Parameter points one block can carry (see `clap::EVENT_CAPACITY`).
const EVENT_CAPACITY: usize = 8192;

pub struct Plugin {
    pub(crate) shared: Arc<Shared>,
    audio: UnsafeCell<Option<Processor>>,
    /// The block's parameter points, and their arrival order (ties at one offset keep it).
    events: UnsafeCell<Vec<(u32, u32, ParamEvent)>>,
    flat: UnsafeCell<Vec<ParamEvent>>,
    scratch: UnsafeCell<Vec<f32>>,
    sample_rate: Cell<f64>,
    max_block: Cell<i32>,
    /// The host turned the sidechain bus on.
    sidechain_bus: AtomicBool,
    /// The unit (group) of each parameter: 0 is the root, n is `Table::groups()[n − 1]`.
    units: Vec<UnitID>,
    pub(crate) notifier: Arc<timer::Notifier>,
}

// SAFETY: the host calls the UI-thread methods from one thread and `process` from the
// audio thread, never both on the processor half at once (the standard's rules, above).
unsafe impl Send for Plugin {}
unsafe impl Sync for Plugin {}

impl Class for Plugin {
    type Interfaces = (
        IComponent,
        IAudioProcessor,
        IProcessContextRequirements,
        IEditController,
        IUnitInfo,
    );
}

impl Plugin {
    pub fn new(d: &'static Descriptor) -> Plugin {
        let shared = Shared::new(d);
        let groups = shared.table.groups();
        let units = shared
            .table
            .params
            .iter()
            .map(|p| {
                groups
                    .iter()
                    .position(|g| *g == p.group)
                    .map_or(kRootUnitId, |i| i as UnitID + 1)
            })
            .collect();
        Plugin {
            notifier: timer::Notifier::new(shared.clone()),
            shared,
            audio: UnsafeCell::new(None),
            events: UnsafeCell::new(Vec::new()),
            flat: UnsafeCell::new(Vec::new()),
            scratch: UnsafeCell::new(Vec::new()),
            sample_rate: Cell::new(48_000.0),
            max_block: Cell::new(4096),
            sidechain_bus: AtomicBool::new(false),
            units,
        }
    }

    fn param(&self, id: ParamID) -> Option<&Param> {
        self.shared.table.by_id(id)
    }

    #[allow(clippy::mut_from_ref)]
    unsafe fn audio(&self) -> &mut Option<Processor> {
        &mut *self.audio.get()
    }

    /// The host's parameter points, in the table's terms, in time order.
    unsafe fn read_changes(&self, changes: *mut IParameterChanges) -> &[ParamEvent] {
        let events = &mut *self.events.get();
        let flat = &mut *self.flat.get();
        events.clear();
        flat.clear();
        let Some(changes) = ComRef::from_raw(changes) else {
            return flat;
        };
        let table = self.shared.table;
        let mut seq = 0u32;
        for q in 0..changes.getParameterCount() {
            let Some(queue) = ComRef::from_raw(changes.getParameterData(q)) else {
                continue;
            };
            let Some(index) = table.index_of(queue.getParameterId()) else {
                continue;
            };
            let p = &table.params[index];
            for point in 0..queue.getPointCount() {
                let mut offset = 0;
                let mut value = 0.0;
                if queue.getPoint(point, &mut offset, &mut value) != kResultOk {
                    continue;
                }
                let ev = ParamEvent {
                    offset: offset.max(0) as u32,
                    index: index as u32,
                    value: p.from_normalized(value),
                    modulation: false,
                };
                if events.len() < events.capacity() {
                    events.push((ev.offset, seq, ev));
                    seq += 1;
                }
            }
        }
        // In time, ties in arrival order: a unique key, so an in-place unstable sort is
        // deterministic and allocates nothing.
        events.sort_unstable_by_key(|(offset, seq, _)| (*offset, *seq));
        for (_, _, ev) in events.iter() {
            if flat.len() < flat.capacity() {
                flat.push(*ev);
            }
        }
        flat
    }
}

unsafe fn read_stream(stream: *mut IBStream) -> Option<Vec<u8>> {
    let stream = ComRef::from_raw(stream)?;
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let mut n: int32 = 0;
        let r = stream.read(
            chunk.as_mut_ptr() as *mut c_void,
            chunk.len() as int32,
            &mut n,
        );
        if r != kResultOk || n <= 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..n as usize]);
    }
    Some(bytes)
}

unsafe fn write_stream(stream: *mut IBStream, bytes: &[u8]) -> bool {
    let Some(stream) = ComRef::from_raw(stream) else {
        return false;
    };
    let mut at = 0;
    while at < bytes.len() {
        let mut n: int32 = 0;
        let r = stream.write(
            bytes[at..].as_ptr() as *mut c_void,
            (bytes.len() - at) as int32,
            &mut n,
        );
        if r != kResultOk || n <= 0 {
            return false;
        }
        at += n as usize;
    }
    true
}

impl IPluginBaseTrait for Plugin {
    unsafe fn initialize(&self, _context: *mut FUnknown) -> tresult {
        timer::register(&self.notifier);
        kResultOk
    }

    unsafe fn terminate(&self) -> tresult {
        timer::unregister(&self.notifier);
        self.notifier.set_handler(None);
        kResultOk
    }
}

const AUDIO: MediaType = MediaTypes_::kAudio as MediaType;
const INPUT: BusDirection = BusDirections_::kInput as BusDirection;
const OUTPUT: BusDirection = BusDirections_::kOutput as BusDirection;

impl IComponentTrait for Plugin {
    unsafe fn getControllerClassId(&self, _class_id: *mut TUID) -> tresult {
        // One object is component and controller: hosts query it for IEditController.
        kNotImplemented
    }

    unsafe fn setIoMode(&self, _mode: IoMode) -> tresult {
        kResultOk
    }

    unsafe fn getBusCount(&self, media: MediaType, dir: BusDirection) -> int32 {
        match (media, dir) {
            (AUDIO, INPUT) => 2,
            (AUDIO, OUTPUT) => 1,
            _ => 0,
        }
    }

    unsafe fn getBusInfo(
        &self,
        media: MediaType,
        dir: BusDirection,
        index: int32,
        bus: *mut BusInfo,
    ) -> tresult {
        if bus.is_null() || media != AUDIO {
            return kInvalidArgument;
        }
        let (name, main) = match (dir, index) {
            (INPUT, 0) => ("export.bus.input", true),
            (INPUT, 1) => ("export.bus.sidechain", false),
            (OUTPUT, 0) => ("export.bus.output", true),
            _ => return kInvalidArgument,
        };
        let bus = &mut *bus;
        bus.mediaType = AUDIO;
        bus.direction = dir;
        bus.channelCount = 2;
        copy_wstr(&mut bus.name, &crate::text::t(name));
        if main {
            bus.busType = BusTypes_::kMain as BusType;
            bus.flags = BusInfo_::BusFlags_::kDefaultActive as uint32;
        } else {
            bus.busType = BusTypes_::kAux as BusType;
            bus.flags = 0;
        }
        kResultOk
    }

    unsafe fn getRoutingInfo(&self, _in: *mut RoutingInfo, _out: *mut RoutingInfo) -> tresult {
        kNotImplemented
    }

    unsafe fn activateBus(
        &self,
        media: MediaType,
        dir: BusDirection,
        index: int32,
        state: TBool,
    ) -> tresult {
        if (media, dir, index) == (AUDIO, INPUT, 1) {
            self.sidechain_bus.store(state != 0, Ordering::Release);
        }
        kResultOk
    }

    unsafe fn setActive(&self, state: TBool) -> tresult {
        if state != 0 {
            // The old one first: it hands back the analyser tap the new one takes.
            *self.audio() = None;
            *self.audio() = Some(Processor::new(
                self.shared.clone(),
                self.sample_rate.get() as f32,
            ));
            let events = &mut *self.events.get();
            events.clear();
            events.reserve(EVENT_CAPACITY);
            let flat = &mut *self.flat.get();
            flat.clear();
            flat.reserve(EVENT_CAPACITY);
            *self.scratch.get() = vec![0.0; self.max_block.get().max(1) as usize];
            self.shared.take_latency_change();
            self.notifier.mark_reported();
        } else {
            *self.audio() = None;
        }
        kResultOk
    }

    unsafe fn setState(&self, state: *mut IBStream) -> tresult {
        match read_stream(state) {
            Some(bytes) if self.shared.load_state(&bytes).is_ok() => kResultOk,
            _ => kResultFalse,
        }
    }

    unsafe fn getState(&self, state: *mut IBStream) -> tresult {
        if write_stream(state, &self.shared.save_state()) {
            kResultOk
        } else {
            kResultFalse
        }
    }
}

impl IAudioProcessorTrait for Plugin {
    unsafe fn setBusArrangements(
        &self,
        inputs: *mut SpeakerArrangement,
        num_ins: int32,
        outputs: *mut SpeakerArrangement,
        num_outs: int32,
    ) -> tresult {
        // Stereo in, stereo out, a stereo sidechain: anything else, the host adapts to
        // what `getBusArrangement` says.
        let stereo = |p: *mut SpeakerArrangement, n: int32| {
            !p.is_null() && (0..n.max(0) as usize).all(|i| *p.add(i) == SpeakerArr::kStereo)
        };
        if num_ins >= 1 && num_outs == 1 && stereo(inputs, num_ins) && stereo(outputs, num_outs) {
            kResultTrue
        } else {
            kResultFalse
        }
    }

    unsafe fn getBusArrangement(
        &self,
        dir: BusDirection,
        index: int32,
        arr: *mut SpeakerArrangement,
    ) -> tresult {
        if arr.is_null() {
            return kInvalidArgument;
        }
        match (dir, index) {
            (INPUT, 0 | 1) | (OUTPUT, 0) => {
                *arr = SpeakerArr::kStereo;
                kResultOk
            }
            _ => kInvalidArgument,
        }
    }

    unsafe fn canProcessSampleSize(&self, size: int32) -> tresult {
        if size == SymbolicSampleSizes_::kSample32 as int32 {
            kResultTrue
        } else {
            kResultFalse
        }
    }

    unsafe fn getLatencySamples(&self) -> uint32 {
        self.notifier.mark_reported();
        self.shared.latency()
    }

    unsafe fn setupProcessing(&self, setup: *mut ProcessSetup) -> tresult {
        if setup.is_null() {
            return kInvalidArgument;
        }
        let setup = &*setup;
        if setup.symbolicSampleSize != SymbolicSampleSizes_::kSample32 as int32 {
            return kResultFalse;
        }
        if setup.sampleRate.is_finite() && setup.sampleRate > 0.0 {
            self.sample_rate.set(setup.sampleRate);
        }
        self.max_block.set(setup.maxSamplesPerBlock.max(1));
        kResultOk
    }

    unsafe fn setProcessing(&self, state: TBool) -> tresult {
        if state != 0 {
            if let Some(p) = self.audio().as_mut() {
                p.reset();
            }
        }
        kResultOk
    }

    unsafe fn process(&self, data: *mut ProcessData) -> tresult {
        if data.is_null() {
            return kInvalidArgument;
        }
        let Some(proc_) = self.audio().as_mut() else {
            return kNotInitialized;
        };
        let _denormals = DenormalGuard::new();
        let data = &*data;
        let events = self.read_changes(data.inputParameterChanges);
        let n = data.numSamples.max(0) as usize;
        if n == 0 || data.numOutputs < 1 || data.outputs.is_null() {
            // A parameter-only call: the values still land.
            proc_.process(&mut [], &mut [], None, events);
            return kResultOk;
        }
        if data.symbolicSampleSize != SymbolicSampleSizes_::kSample32 as int32 {
            return kInvalidArgument;
        }
        let out = &*data.outputs;
        let out_ch = out.__field0.channelBuffers32;
        if out.numChannels < 1 || out_ch.is_null() || (*out_ch).is_null() {
            return kResultOk;
        }
        let out_l = *out_ch;
        let scratch = &mut *self.scratch.get();
        let out_r = if out.numChannels > 1 {
            *out_ch.add(1)
        } else if scratch.len() >= n {
            scratch.as_mut_ptr()
        } else {
            return kResultOk;
        };
        let bus = |i: usize| -> Option<(*const f32, *const f32)> {
            if (data.numInputs.max(0) as usize) <= i || data.inputs.is_null() {
                return None;
            }
            let b = &*data.inputs.add(i);
            let ch = b.__field0.channelBuffers32;
            if b.numChannels < 1 || ch.is_null() || (*ch).is_null() {
                return None;
            }
            let l = *ch as *const f32;
            let r = if b.numChannels > 1 {
                *ch.add(1) as *const f32
            } else {
                l
            };
            Some((l, r))
        };
        match bus(0) {
            Some((in_l, in_r)) => {
                if !std::ptr::eq(in_l, out_l) {
                    std::ptr::copy(in_l, out_l, n);
                }
                if !std::ptr::eq(in_r, out_r) {
                    std::ptr::copy(in_r, out_r, n);
                }
            }
            None => {
                std::ptr::write_bytes(out_l, 0, n);
                std::ptr::write_bytes(out_r, 0, n);
            }
        }
        // The sidechain bus is not active by default: only once the host turned it on
        // (`activateBus`) does it carry a key.
        let side = if self.sidechain_bus.load(Ordering::Acquire) {
            bus(1)
        } else {
            None
        };
        let key = side.map(|(l, r)| {
            (
                std::slice::from_raw_parts(l, n),
                std::slice::from_raw_parts(r, n),
            )
        });
        proc_.process(
            std::slice::from_raw_parts_mut(out_l, n),
            std::slice::from_raw_parts_mut(out_r, n),
            key,
            events,
        );
        kResultOk
    }

    unsafe fn getTailSamples(&self) -> uint32 {
        self.shared.tail_samples()
    }
}

impl IProcessContextRequirementsTrait for Plugin {
    unsafe fn getProcessContextRequirements(&self) -> uint32 {
        0
    }
}

impl IEditControllerTrait for Plugin {
    unsafe fn setComponentState(&self, _state: *mut IBStream) -> tresult {
        // The component is this object: its `setState` already took the values.
        kResultOk
    }

    unsafe fn setState(&self, _state: *mut IBStream) -> tresult {
        kResultOk
    }

    unsafe fn getState(&self, _state: *mut IBStream) -> tresult {
        kResultOk
    }

    unsafe fn getParameterCount(&self) -> int32 {
        self.shared.table.params.len() as int32
    }

    unsafe fn getParameterInfo(&self, index: int32, info: *mut ParameterInfo) -> tresult {
        let Some(p) = usize::try_from(index)
            .ok()
            .and_then(|i| self.shared.table.params.get(i))
        else {
            return kInvalidArgument;
        };
        if info.is_null() {
            return kInvalidArgument;
        }
        let info = &mut *info;
        info.id = p.id;
        copy_wstr(&mut info.title, &p.name);
        copy_wstr(&mut info.shortTitle, &p.short_name);
        copy_wstr(
            &mut info.units,
            if p.choices.is_empty() { p.unit } else { "" },
        );
        info.stepCount = p.step_count();
        info.defaultNormalizedValue = p.to_normalized(p.default);
        info.unitId = self.units[p.index];
        let mut flags = ParameterInfo_::ParameterFlags_::kCanAutomate as int32;
        if p.stepped && !p.choices.is_empty() {
            flags |= ParameterInfo_::ParameterFlags_::kIsList as int32;
        }
        if p.role == Role::Bypass {
            flags |= ParameterInfo_::ParameterFlags_::kIsBypass as int32;
        }
        info.flags = flags;
        kResultOk
    }

    unsafe fn getParamStringByValue(
        &self,
        id: ParamID,
        value: ParamValue,
        string: *mut String128,
    ) -> tresult {
        let Some(p) = self.param(id) else {
            return kInvalidArgument;
        };
        if string.is_null() {
            return kInvalidArgument;
        }
        copy_wstr(&mut *string, &p.text(p.from_normalized(value)));
        kResultOk
    }

    unsafe fn getParamValueByString(
        &self,
        id: ParamID,
        string: *mut TChar,
        value: *mut ParamValue,
    ) -> tresult {
        let Some(p) = self.param(id) else {
            return kInvalidArgument;
        };
        match (read_wstr(string).and_then(|s| p.parse(&s)), value.is_null()) {
            (Some(v), false) => {
                *value = p.to_normalized(v);
                kResultOk
            }
            _ => kResultFalse,
        }
    }

    unsafe fn normalizedParamToPlain(&self, id: ParamID, value: ParamValue) -> ParamValue {
        self.param(id)
            .map_or(value, |p| p.from_normalized(value) as ParamValue)
    }

    unsafe fn plainParamToNormalized(&self, id: ParamID, plain: ParamValue) -> ParamValue {
        self.param(id)
            .map_or(plain, |p| p.to_normalized(p.clamp(plain)))
    }

    unsafe fn getParamNormalized(&self, id: ParamID) -> ParamValue {
        self.param(id)
            .map_or(0.0, |p| p.to_normalized(self.shared.value(p.index)))
    }

    unsafe fn setParamNormalized(&self, id: ParamID, value: ParamValue) -> tresult {
        let Some(p) = self.param(id) else {
            return kInvalidArgument;
        };
        // The host keeps the controller in step with what it sends the processor; the
        // processor takes the value from that stream, so this only updates what is shown.
        self.shared
            .show_value(p.index, p.from_normalized(value) as f64);
        kResultOk
    }

    unsafe fn setComponentHandler(&self, handler: *mut IComponentHandler) -> tresult {
        self.notifier
            .set_handler(ComRef::from_raw(handler).map(|h| h.to_com_ptr()));
        kResultOk
    }

    unsafe fn createView(&self, name: FIDString) -> *mut IPlugView {
        #[cfg(all(feature = "editor", windows))]
        {
            let editor = !name.is_null() && std::ffi::CStr::from_ptr(name) == c"editor";
            if editor && crate::editor::has_editor(self.shared.desc.kind) {
                let view = super::view::View::new(self.shared.clone(), self.notifier.clone());
                return ::vst3::ComWrapper::new(view)
                    .to_com_ptr::<IPlugView>()
                    .map_or(std::ptr::null_mut(), |p| p.into_raw());
            }
        }
        let _ = name;
        std::ptr::null_mut()
    }
}

impl IUnitInfoTrait for Plugin {
    unsafe fn getUnitCount(&self) -> int32 {
        1 + self.shared.table.groups().len() as int32
    }

    unsafe fn getUnitInfo(&self, index: int32, info: *mut UnitInfo) -> tresult {
        if info.is_null() {
            return kInvalidArgument;
        }
        let groups = self.shared.table.groups();
        let info = &mut *info;
        info.programListId = kNoProgramListId;
        match index {
            0 => {
                info.id = kRootUnitId;
                info.parentUnitId = kNoParentUnitId;
                copy_wstr(&mut info.name, self.shared.desc.full_name);
            }
            i if i > 0 && (i as usize) <= groups.len() => {
                info.id = i;
                info.parentUnitId = kRootUnitId;
                copy_wstr(&mut info.name, groups[i as usize - 1]);
            }
            _ => return kInvalidArgument,
        }
        kResultOk
    }

    unsafe fn getProgramListCount(&self) -> int32 {
        0
    }

    unsafe fn getProgramListInfo(&self, _index: int32, _info: *mut ProgramListInfo) -> tresult {
        kInvalidArgument
    }

    unsafe fn getProgramName(
        &self,
        _list: ProgramListID,
        _index: int32,
        _name: *mut String128,
    ) -> tresult {
        kInvalidArgument
    }

    unsafe fn getProgramInfo(
        &self,
        _list: ProgramListID,
        _index: int32,
        _attribute: FIDString,
        _value: *mut String128,
    ) -> tresult {
        kInvalidArgument
    }

    unsafe fn hasProgramPitchNames(&self, _list: ProgramListID, _index: int32) -> tresult {
        kResultFalse
    }

    unsafe fn getProgramPitchName(
        &self,
        _list: ProgramListID,
        _index: int32,
        _pitch: int16,
        _name: *mut String128,
    ) -> tresult {
        kResultFalse
    }

    unsafe fn getSelectedUnit(&self) -> UnitID {
        kRootUnitId
    }

    unsafe fn selectUnit(&self, _unit: UnitID) -> tresult {
        kResultOk
    }

    unsafe fn getUnitByBus(
        &self,
        _media: MediaType,
        _dir: BusDirection,
        _bus: int32,
        _channel: int32,
        _unit: *mut UnitID,
    ) -> tresult {
        kResultFalse
    }

    unsafe fn setUnitProgramData(
        &self,
        _list_or_unit: int32,
        _index: int32,
        _data: *mut IBStream,
    ) -> tresult {
        kNotImplemented
    }
}
