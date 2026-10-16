//! Cantus Test Pulse: a single-component instrument (the component is its own controller)
//! that writes its note events into the audio, so a test can see where each one landed.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use vst3::Steinberg::Vst::BusDirections_::{kInput, kOutput};
use vst3::Steinberg::Vst::BusInfo_::BusFlags_::kDefaultActive;
use vst3::Steinberg::Vst::BusTypes_::kMain;
use vst3::Steinberg::Vst::Event_::EventTypes_::{
    kNoteExpressionValueEvent, kNoteOffEvent, kNoteOnEvent, kPolyPressureEvent,
};
use vst3::Steinberg::Vst::MediaTypes_::{kAudio, kEvent};
use vst3::Steinberg::Vst::ParameterInfo_::ParameterFlags_::{kIsHidden, kIsReadOnly};
use vst3::Steinberg::Vst::SymbolicSampleSizes_::kSample32;
use vst3::Steinberg::Vst::{
    BusDirection, BusInfo, Event, IAudioProcessor, IAudioProcessorTrait, IComponent,
    IComponentHandler, IComponentTrait, IEditController, IEditControllerTrait, IEventListTrait,
    IParamValueQueueTrait, IParameterChangesTrait, IProcessContextRequirements,
    IProcessContextRequirementsTrait, IoMode, MediaType,
    ParameterInfo, ProcessData, ProcessSetup, RoutingInfo, SpeakerArr, SpeakerArrangement,
    String128, TChar,
};
use vst3::Steinberg::{
    kInvalidArgument, kResultFalse, kResultOk, kResultTrue, tresult, FIDString, FUnknown, IBStream,
    IPlugView, IPluginBaseTrait, TBool, TUID,
};
use vst3::{uid, Class, ComRef};

use crate::{bytes, copy_wstring, not_implemented, Live};

pub const NAME: &str = "Cantus Test Pulse";
pub const CID: TUID = uid(0x43414E54, 0x55535450, 0x554C5345, 0x494E5331);
/// Hidden: a failure on purpose (`crate::fault`, round 29).
pub const FAULT: u32 = 0;
/// Hidden and read-only: the id of the process this instance runs in.
pub const PROCESS: u32 = 1;

static LAST_STATE: AtomicU32 = AtomicU32::new(0);
static LAST_VALUES: [AtomicU64; 8] = [const { AtomicU64::new(0) }; 8];

pub unsafe fn last_context(values: *mut f64) -> u32 {
    if !values.is_null() {
        for (i, v) in LAST_VALUES.iter().enumerate() {
            *values.add(i) = f64::from_bits(v.load(Ordering::SeqCst));
        }
    }
    LAST_STATE.load(Ordering::SeqCst)
}

pub struct Pulse {
    _live: Live,
    /// The Fault parameter's normalised value, as the controller side was told it.
    fault: AtomicU64,
}

impl Pulse {
    pub fn new() -> Pulse {
        Pulse {
            _live: Live::new(),
            fault: AtomicU64::new(0),
        }
    }
}

impl Class for Pulse {
    type Interfaces = (
        IComponent,
        IAudioProcessor,
        IEditController,
        IProcessContextRequirements,
    );
}

impl IPluginBaseTrait for Pulse {
    unsafe fn initialize(&self, _context: *mut FUnknown) -> tresult {
        kResultOk
    }

    unsafe fn terminate(&self) -> tresult {
        kResultOk
    }
}

impl IComponentTrait for Pulse {
    unsafe fn getControllerClassId(&self, _class_id: *mut TUID) -> tresult {
        not_implemented()
    }

    unsafe fn setIoMode(&self, _mode: IoMode) -> tresult {
        kResultOk
    }

    unsafe fn getBusCount(&self, media: MediaType, dir: BusDirection) -> i32 {
        let audio_out = media == kAudio as i32 && dir == kOutput as i32;
        let event_in = media == kEvent as i32 && dir == kInput as i32;
        i32::from(audio_out || event_in)
    }

    unsafe fn getBusInfo(
        &self,
        media: MediaType,
        dir: BusDirection,
        index: i32,
        bus: *mut BusInfo,
    ) -> tresult {
        if index != 0 || bus.is_null() {
            return kInvalidArgument;
        }
        let bus = &mut *bus;
        if media == kAudio as i32 && dir == kOutput as i32 {
            bus.channelCount = 2;
            copy_wstring("Output", &mut bus.name);
        } else if media == kEvent as i32 && dir == kInput as i32 {
            bus.channelCount = 16;
            copy_wstring("Notes", &mut bus.name);
        } else {
            return kInvalidArgument;
        }
        bus.mediaType = media;
        bus.direction = dir;
        bus.busType = kMain as i32;
        bus.flags = kDefaultActive as u32;
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
        }
        kResultOk
    }

    unsafe fn setState(&self, state: *mut IBStream) -> tresult {
        crate::fault::fire(crate::fault::ABORT_LOAD);
        match bytes::read::<4>(state) {
            Some(m) if &m == b"CTPS" => kResultOk,
            _ => kResultFalse,
        }
    }

    unsafe fn getState(&self, state: *mut IBStream) -> tresult {
        crate::fault::fire(crate::fault::ABORT_SAVE);
        if bytes::write(state, b"CTPS") {
            kResultOk
        } else {
            kResultFalse
        }
    }
}

impl IAudioProcessorTrait for Pulse {
    unsafe fn setBusArrangements(
        &self,
        _inputs: *mut SpeakerArrangement,
        num_ins: i32,
        outputs: *mut SpeakerArrangement,
        num_outs: i32,
    ) -> tresult {
        if num_ins == 0 && num_outs == 1 && *outputs == SpeakerArr::kStereo {
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
        if dir != kOutput as i32 || index != 0 || arr.is_null() {
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
        0
    }

    unsafe fn setupProcessing(&self, _setup: *mut ProcessSetup) -> tresult {
        kResultOk
    }

    unsafe fn setProcessing(&self, _state: TBool) -> tresult {
        kResultOk
    }

    unsafe fn process(&self, data: *mut ProcessData) -> tresult {
        let data = &*data;
        // The hidden Fault parameter (round 29): a value that arrives here arms it.
        if let Some(changes) = ComRef::from_raw(data.inputParameterChanges) {
            for i in 0..changes.getParameterCount() {
                let Some(q) = ComRef::from_raw(changes.getParameterData(i)) else {
                    continue;
                };
                if q.getParameterId() != FAULT {
                    continue;
                }
                for p in 0..q.getPointCount() {
                    let (mut offset, mut value) = (0, 0.0);
                    if q.getPoint(p, &mut offset, &mut value) == kResultOk {
                        crate::fault::arm(value);
                    }
                }
            }
        }
        if let Some(c) = data.processContext.as_ref() {
            LAST_STATE.store(c.state, Ordering::SeqCst);
            let values = [
                c.tempo,
                c.projectTimeMusic,
                c.barPositionMusic,
                c.cycleStartMusic,
                c.cycleEndMusic,
                f64::from(c.timeSigNumerator),
                f64::from(c.timeSigDenominator),
                c.projectTimeSamples as f64,
            ];
            for (slot, v) in LAST_VALUES.iter().zip(values) {
                slot.store(v.to_bits(), Ordering::SeqCst);
            }
        }
        let n = data.numSamples.max(0) as usize;
        if n == 0 || data.numOutputs < 1 {
            return kResultOk;
        }
        let out = &*data.outputs;
        if out.numChannels < 2 {
            return kResultFalse;
        }
        let chans = std::slice::from_raw_parts(out.__field0.channelBuffers32, 2);
        let left = std::slice::from_raw_parts_mut(chans[0], n);
        let right = std::slice::from_raw_parts_mut(chans[1], n);
        left.fill(0.0);
        right.fill(0.0);
        let Some(events) = ComRef::from_raw(data.inputEvents) else {
            return kResultOk;
        };
        for i in 0..events.getEventCount() {
            // SAFETY: plain data, filled by `getEvent`.
            let mut e: Event = std::mem::zeroed();
            if events.getEvent(i, &mut e) != kResultOk {
                continue;
            }
            let at = (e.sampleOffset.max(0) as usize).min(n - 1);
            match u32::from(e.r#type) {
                t if t == kNoteOnEvent as u32 => left[at] += e.__field0.noteOn.velocity,
                t if t == kNoteOffEvent as u32 => left[at] -= e.__field0.noteOff.velocity,
                t if t == kPolyPressureEvent as u32 => {
                    right[at] = e.__field0.polyPressure.pressure;
                }
                t if t == kNoteExpressionValueEvent as u32 => {
                    right[at] = e.__field0.noteExpressionValue.value as f32;
                }
                _ => {}
            }
        }
        kResultOk
    }

    unsafe fn getTailSamples(&self) -> u32 {
        0
    }
}

impl IProcessContextRequirementsTrait for Pulse {
    unsafe fn getProcessContextRequirements(&self) -> u32 {
        0
    }
}

impl IEditControllerTrait for Pulse {
    unsafe fn setComponentState(&self, _state: *mut IBStream) -> tresult {
        kResultOk
    }

    unsafe fn setState(&self, _state: *mut IBStream) -> tresult {
        kResultOk
    }

    unsafe fn getState(&self, _state: *mut IBStream) -> tresult {
        kResultOk
    }

    unsafe fn getParameterCount(&self) -> i32 {
        2
    }

    unsafe fn getParameterInfo(&self, index: i32, info: *mut ParameterInfo) -> tresult {
        let (id, title, steps, flags) = match index {
            0 => (FAULT, "Fault", crate::fault::STEPS, kIsHidden as i32),
            1 => (PROCESS, "Process", 0, (kIsHidden | kIsReadOnly) as i32),
            _ => return kInvalidArgument,
        };
        let info = &mut *info;
        info.id = id;
        copy_wstring(title, &mut info.title);
        copy_wstring(title, &mut info.shortTitle);
        copy_wstring("", &mut info.units);
        info.stepCount = steps;
        info.defaultNormalizedValue = 0.0;
        info.unitId = 0;
        info.flags = flags;
        kResultOk
    }

    unsafe fn getParamStringByValue(&self, id: u32, v: f64, s: *mut String128) -> tresult {
        if id != FAULT && id != PROCESS {
            return kInvalidArgument;
        }
        let text = format!("{}", self.normalizedParamToPlain(id, v).round());
        copy_wstring(&text, &mut *s);
        kResultOk
    }

    unsafe fn getParamValueByString(&self, _id: u32, _s: *mut TChar, _v: *mut f64) -> tresult {
        kInvalidArgument
    }

    unsafe fn normalizedParamToPlain(&self, id: u32, v: f64) -> f64 {
        match id {
            FAULT => (v * f64::from(crate::fault::STEPS)).round(),
            PROCESS => v * crate::fault::PROCESS_RANGE,
            _ => v,
        }
    }

    unsafe fn plainParamToNormalized(&self, id: u32, v: f64) -> f64 {
        match id {
            FAULT => (v / f64::from(crate::fault::STEPS)).clamp(0.0, 1.0),
            PROCESS => (v / crate::fault::PROCESS_RANGE).clamp(0.0, 1.0),
            _ => v,
        }
    }

    unsafe fn getParamNormalized(&self, id: u32) -> f64 {
        match id {
            FAULT => f64::from_bits(self.fault.load(Ordering::SeqCst)),
            PROCESS => crate::fault::process_norm(),
            _ => 0.0,
        }
    }

    unsafe fn setParamNormalized(&self, id: u32, v: f64) -> tresult {
        match id {
            // Kept only: a fault arms in `process`.
            FAULT => {
                self.fault.store(v.to_bits(), Ordering::SeqCst);
                kResultOk
            }
            PROCESS => kResultOk,
            _ => kInvalidArgument,
        }
    }

    unsafe fn setComponentHandler(&self, _handler: *mut IComponentHandler) -> tresult {
        kResultOk
    }

    unsafe fn createView(&self, _name: FIDString) -> *mut IPlugView {
        std::ptr::null_mut()
    }
}
