//! Creating a plug-in: component and controller, connected, set up and activated — and
//! taking it apart again in the reverse order.
//!
//! The sequence is the SDK's (`public.sdk/source/vst/hosting/plugprovider.cpp` and the
//! "Audio Processor Call Sequence" of the documentation):
//!
//! 1. `createInstance(cid, IComponent)`, `initialize(host)`;
//! 2. the controller: the component itself when it also implements `IEditController`
//!    (a single-component plug-in), else `getControllerClassId` → `createInstance` →
//!    `initialize(host)`;
//! 3. both `IConnectionPoint`s connected to each other (separate objects only);
//! 4. `getState` from the component → `setComponentState` on the controller;
//! 5. `setComponentHandler`;
//! 6. `setBusArrangements` (stereo main in/out, stereo sidechain when the plug-in has an
//!    aux input), `activateBus` for those, `setupProcessing` (realtime, 32-bit, the
//!    engine's largest block), `setActive(true)`.
//!
//! Teardown reverses it: `setActive(false)`, handler cleared, disconnected, controller
//! then component `terminate`d, every pointer released, and only then the module.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use ::vst3::Steinberg::Vst::BusDirections_::{kInput, kOutput};
use ::vst3::Steinberg::Vst::MediaTypes_::{kAudio, kEvent};
use ::vst3::Steinberg::Vst::ProcessModes_::kRealtime;
use ::vst3::Steinberg::Vst::SymbolicSampleSizes_::kSample32;
use ::vst3::Steinberg::Vst::ViewType::kEditor;
use ::vst3::Steinberg::Vst::{
    IAudioProcessor, IAudioProcessorTrait, IComponent, IComponentHandler, IComponentTrait,
    IConnectionPoint, IConnectionPointTrait, IEditController, IEditControllerTrait, IMidiMapping,
    IMidiMappingTrait, ParamID, ProcessSetup, SpeakerArr, SpeakerArrangement,
};
use ::vst3::Steinberg::{
    FIDString, IBStream, IPlugView, IPluginBaseTrait, IPluginFactoryTrait, TUID,
};
use ::vst3::{ComPtr, ComWrapper, Interface};

use super::host_objects::{ComponentHandler, MemoryStream, Requests, SharedRequests};
use super::module::Module;
use super::params::{Mailbox, Mapping, ParamMap};
use super::scan::{buses, Buses};
use super::util::ok;
use crate::backend::{AudioSetup, HostError};

/// Create an object of class `cid` from the module's factory, as interface `I`.
fn create<I: Interface>(module: &Module, cid: &TUID) -> Option<ComPtr<I>> {
    let mut obj = std::ptr::null_mut();
    // SAFETY: the factory writes an add-ref'd `I*` into `obj` on success.
    unsafe {
        let r = module.factory().createInstance(
            cid.as_ptr() as FIDString,
            I::IID.as_ptr() as FIDString,
            &mut obj,
        );
        if !ok(r) {
            return None;
        }
        ComPtr::from_raw(obj as *mut I)
    }
}

/// Component and controller, initialized and connected; not activated.
pub struct Parts {
    pub component: ComPtr<IComponent>,
    pub controller: Option<ComPtr<IEditController>>,
    /// The controller is its own object (initialized, terminated and connected by us).
    separate: bool,
    connection: Option<(ComPtr<IConnectionPoint>, ComPtr<IConnectionPoint>)>,
    /// Last: the code behind every pointer above, held until they are released.
    _module: Arc<Module>,
}

impl Parts {
    pub fn create(module: &Arc<Module>, cid: &TUID) -> Result<Parts, HostError> {
        let component = create::<IComponent>(module, cid)
            .ok_or_else(|| HostError::NotFound(super::util::tuid_to_string(cid)))?;
        let context = module.host_context();
        // SAFETY: the documented creation sequence (module docs), main thread.
        unsafe {
            if !ok(component.initialize(context)) {
                return Err(HostError::Instantiate(
                    "the component refused to initialize".into(),
                ));
            }
            let mut parts = Parts {
                component,
                controller: None,
                separate: false,
                connection: None,
                _module: module.clone(),
            };
            if let Some(single) = parts.component.cast::<IEditController>() {
                parts.controller = Some(single);
            } else {
                let mut ctrl_cid: TUID = [0; 16];
                if ok(parts.component.getControllerClassId(&mut ctrl_cid)) {
                    if let Some(ctrl) = create::<IEditController>(module, &ctrl_cid) {
                        if ok(ctrl.initialize(context)) {
                            parts.controller = Some(ctrl);
                            parts.separate = true;
                        }
                    }
                }
            }
            if parts.separate {
                let a = parts.component.cast::<IConnectionPoint>();
                let b = parts
                    .controller
                    .as_ref()
                    .and_then(|c| c.cast::<IConnectionPoint>());
                if let (Some(a), Some(b)) = (a, b) {
                    a.connect(b.as_ptr());
                    b.connect(a.as_ptr());
                    parts.connection = Some((a, b));
                }
            }
            parts.sync_controller();
            Ok(parts)
        }
    }

    /// Whether the controller is its own object (not a single-component plug-in).
    pub fn is_separate(&self) -> bool {
        self.separate
    }

    /// Hand the component's state to the controller (`setComponentState`).
    pub fn sync_controller(&self) {
        let Some(ctrl) = &self.controller else { return };
        let stream = ComWrapper::new(MemoryStream::new());
        let Some(s) = stream.as_com_ref::<IBStream>() else {
            return;
        };
        // SAFETY: main-thread state calls with a stream we own.
        unsafe {
            if ok(self.component.getState(s.as_ptr())) {
                stream.rewind();
                ctrl.setComponentState(s.as_ptr());
            }
        }
    }

    /// Whether the controller makes an editor (made and released at once).
    pub fn has_editor(&self) -> bool {
        let Some(ctrl) = &self.controller else {
            return false;
        };
        // SAFETY: `createView` returns an add-ref'd view or null.
        unsafe { ComPtr::<IPlugView>::from_raw(ctrl.createView(kEditor)).is_some() }
    }
}

impl Drop for Parts {
    fn drop(&mut self) {
        // SAFETY: the documented teardown, main thread.
        unsafe {
            if let Some((a, b)) = self.connection.take() {
                a.disconnect(b.as_ptr());
                b.disconnect(a.as_ptr());
            }
            if self.separate {
                if let Some(ctrl) = self.controller.take() {
                    ctrl.terminate();
                }
            }
            self.controller = None;
            self.component.terminate();
        }
        // The pointers are released as the fields drop, the module last.
    }
}

/// Speaker bits → channel count.
pub fn channels(arr: SpeakerArrangement) -> u32 {
    arr.count_ones()
}

/// The buses as configured for processing.
#[derive(Clone, Debug, Default)]
pub struct Layout {
    pub buses: Buses,
    /// Channel count of every audio input / output bus, in bus order.
    pub inputs: Vec<u32>,
    pub outputs: Vec<u32>,
    /// Index of the main input bus (first `kMain` input), if any.
    pub main_input: Option<usize>,
}

/// Everything both halves share. Dropped (on the main thread) when both are gone.
pub struct Instance {
    pub processor: ComPtr<IAudioProcessor>,
    pub handler: ComWrapper<ComponentHandler>,
    pub requests: SharedRequests,
    /// New parameter maps for the audio thread.
    pub maps: Mailbox<ParamMap>,
    /// The latency last read from the plug-in, for whoever needs it without a call
    /// into the plug-in (delay compensation reads it on the audio thread).
    pub latency: AtomicUsize,
    pub setup: AudioSetup,
    pub layout: Layout,
    /// The parameter the controller maps each MIDI channel's controller to
    /// (`IMidiMapping`: CC 0…127, channel pressure 128, pitch bend 129), or
    /// [`NO_MIDI_PARAM`]. Read once at activation; the audio thread only looks up.
    pub midi_map: Box<MidiMap>,
    /// The plug-in's program list — its parameter flagged `kIsProgramChange`, the root
    /// unit's first — and its steps (programs less one): where a MIDI program change goes,
    /// as `program / steps` (round 29, review 1 of ADR 0062).
    pub program_param: Option<(ParamID, i32)>,
    /// A MIDI program change reached the program list since the main thread last looked
    /// (set on the audio thread, no allocation): the plug-in's whole state changed, which
    /// the project records as one state step. A value the host sent the list itself (the
    /// project, automation, a mapped knob) is its own parameter step, never this (round
    /// 29, review 3 of ADR 0062, #3, #21, #52).
    pub program_changed: std::sync::atomic::AtomicBool,
    active: bool,
    /// Last: terminated after deactivation.
    pub parts: Parts,
}

impl Instance {
    /// Steps 1–6 of the module docs.
    pub fn activate(
        parts: Parts,
        setup: AudioSetup,
    ) -> Result<(Instance, rtrb::Consumer<super::host_objects::Edit>), HostError> {
        let processor = parts
            .component
            .cast::<IAudioProcessor>()
            .ok_or_else(|| HostError::Instantiate("not an audio processor".into()))?;
        let (to_audio, from_ui) = rtrb::RingBuffer::new(EDIT_RING);
        let requests = Arc::new(Mutex::new(Requests {
            to_audio: Some(to_audio),
            ..Requests::default()
        }));
        let handler = ComWrapper::new(ComponentHandler {
            requests: requests.clone(),
        });
        let mut inst = Instance {
            processor,
            handler,
            requests,
            maps: Mailbox::new(),
            latency: AtomicUsize::new(0),
            setup,
            layout: Layout::default(),
            midi_map: Box::new([[NO_MIDI_PARAM; MIDI_CONTROLLERS]; 16]),
            program_param: None,
            program_changed: std::sync::atomic::AtomicBool::new(false),
            active: false,
            parts,
        };
        // SAFETY: the documented setup sequence, main thread.
        unsafe {
            if let (Some(ctrl), Some(h)) = (
                &inst.parts.controller,
                inst.handler.as_com_ref::<IComponentHandler>(),
            ) {
                ctrl.setComponentHandler(h.as_ptr());
            }
            inst.layout = arrange(&inst.parts.component, &inst.processor);
            if inst.processor.canProcessSampleSize(kSample32 as i32) != ::vst3::Steinberg::kResultOk
            {
                return Err(HostError::Activate("no 32-bit processing".into()));
            }
            let mut ps = ProcessSetup {
                processMode: kRealtime as i32,
                symbolicSampleSize: kSample32 as i32,
                maxSamplesPerBlock: setup.max_block.clamp(1, i32::MAX as usize) as i32,
                sampleRate: setup.sample_rate,
            };
            if !ok(inst.processor.setupProcessing(&mut ps)) {
                return Err(HostError::Activate("setupProcessing refused".into()));
            }
            if !ok(inst.parts.component.setActive(1)) {
                return Err(HostError::Activate("setActive refused".into()));
            }
            inst.active = true;
            inst.refresh_latency();
            inst.read_midi_map();
            inst.read_program_list();
        }
        Ok((inst, from_ui))
    }

    /// Read the plug-in's latency into the cache and return it (main thread).
    pub fn refresh_latency(&self) -> usize {
        // SAFETY: `getLatencySamples` is a main-thread call on an active processor.
        let n = unsafe { self.processor.getLatencySamples() } as usize;
        self.latency.store(n, Ordering::Release);
        n
    }

    /// Ask the controller which parameter each MIDI controller drives (main thread).
    fn read_midi_map(&mut self) {
        let Some(mapping) = self
            .parts
            .controller
            .as_ref()
            .and_then(|c| c.cast::<IMidiMapping>())
        else {
            return;
        };
        for (channel, row) in self.midi_map.iter_mut().enumerate() {
            for (ctrl, slot) in row.iter_mut().enumerate() {
                let mut id: ParamID = 0;
                // SAFETY: a main-thread query writing one ParamID.
                let r = unsafe {
                    mapping.getMidiControllerAssignment(0, channel as i16, ctrl as i16, &mut id)
                };
                if ok(r) {
                    *slot = id;
                }
            }
        }
    }

    /// Find the program list (main thread): the parameter flagged `kIsProgramChange`, in
    /// the root unit before any other — what Steinberg says a host sends a program change
    /// to (`IMidiMapping` has no number for it).
    fn read_program_list(&mut self) {
        let Some(ctrl) = self.parts.controller.as_ref() else {
            return;
        };
        let mut found = Vec::new();
        // SAFETY: main-thread controller queries into a zeroed struct.
        unsafe {
            for i in 0..ctrl.getParameterCount().max(0) {
                let mut info: ::vst3::Steinberg::Vst::ParameterInfo = std::mem::zeroed();
                if ok(ctrl.getParameterInfo(i, &mut info)) {
                    found.push(ProgramCandidate {
                        id: info.id,
                        flags: info.flags,
                        unit: info.unitId,
                        steps: info.stepCount,
                    });
                }
            }
        }
        self.program_param = program_list(&found);
    }

    /// The parameter a MIDI channel message drives, and its normalized value.
    pub fn midi_param(
        &self,
        channel: u8,
        status: u8,
        data1: u8,
        data2: u8,
    ) -> Option<(ParamID, f64)> {
        // A program change chooses one of the program list's programs (round 29).
        if status & 0xF0 == 0xC0 {
            let (id, steps) = self.program_param?;
            return Some((id, program_value(data1, steps)));
        }
        let row = self.midi_map.get(usize::from(channel & 15))?;
        let (ctrl, value) = match status & 0xF0 {
            0xB0 if data1 < 128 => (usize::from(data1), f64::from(data2.min(127)) / 127.0),
            0xD0 => (128, f64::from(data1.min(127)) / 127.0),
            0xE0 => {
                let raw = (u16::from(data2 & 127) << 7) | u16::from(data1 & 127);
                (129, f64::from(raw) / 16383.0)
            }
            _ => return None,
        };
        let id = row[ctrl];
        (id != NO_MIDI_PARAM).then_some((id, value))
    }

    pub fn controller(&self) -> Option<&ComPtr<IEditController>> {
        self.parts.controller.as_ref()
    }

    /// Every parameter's mapping, asked of the controller (main thread).
    pub fn param_map(&self) -> ParamMap {
        let Some(ctrl) = self.controller() else {
            return ParamMap::default();
        };
        let mut entries = Vec::new();
        // SAFETY: main-thread controller queries into a zeroed struct.
        unsafe {
            for i in 0..ctrl.getParameterCount().max(0) {
                let mut info: ::vst3::Steinberg::Vst::ParameterInfo = std::mem::zeroed();
                if ok(ctrl.getParameterInfo(i, &mut info)) {
                    entries.push((info.id, Mapping::probe(ctrl, info.id, info.stepCount)));
                }
            }
        }
        ParamMap::new(entries)
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        // SAFETY: the documented teardown, main thread; the audio half has already
        // stopped processing (it does so when dropped at the latest).
        unsafe {
            if self.active {
                self.parts.component.setActive(0);
            }
            if let Some(ctrl) = &self.parts.controller {
                ctrl.setComponentHandler(std::ptr::null_mut());
            }
        }
        lock_requests(&self.requests).to_audio = None;
    }
}

fn lock_requests(r: &SharedRequests) -> std::sync::MutexGuard<'_, Requests> {
    super::host_objects::lock(r)
}

/// CC 0…127, then channel pressure (128) and pitch bend (129), as `IMidiMapping` numbers
/// them.
pub const MIDI_CONTROLLERS: usize = 130;

/// A parameter as far as finding the program list needs it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ProgramCandidate {
    pub id: ParamID,
    pub flags: i32,
    pub unit: i32,
    pub steps: i32,
}

/// The program list among `params`: flagged `kIsProgramChange`, the root unit's first, else
/// the first of any unit; with its steps.
pub fn program_list(params: &[ProgramCandidate]) -> Option<(ParamID, i32)> {
    use ::vst3::Steinberg::Vst::ParameterInfo_::ParameterFlags_::kIsProgramChange;
    let lists = || {
        params
            .iter()
            .filter(|p| p.flags & kIsProgramChange as i32 != 0)
    };
    lists()
        .find(|p| p.unit == 0)
        .or_else(|| lists().next())
        .map(|p| (p.id, p.steps.max(0)))
}
/// No parameter for this controller.
pub const NO_MIDI_PARAM: ParamID = u32::MAX;

/// The normalised value of program `program` on a program-change parameter of `steps`
/// steps (programs less one): `program / steps`, the last for one past it; a parameter
/// that says no steps reads the program over MIDI's 127.
pub fn program_value(program: u8, steps: i32) -> f64 {
    let p = f64::from(program & 127);
    if steps > 0 {
        p.min(f64::from(steps)) / f64::from(steps)
    } else {
        p / 127.0
    }
}
pub type MidiMap = [[ParamID; MIDI_CONTROLLERS]; 16];

/// Room for the controller's edits between two blocks.
const EDIT_RING: usize = 1024;

/// Stereo main in/out and a stereo sidechain; the other buses keep what the plug-in
/// proposes. What the plug-in accepts is read back, since it may answer with the nearest
/// arrangement it supports.
///
/// # Safety
/// Main thread, component initialized and not active.
unsafe fn arrange(component: &ComPtr<IComponent>, processor: &ComPtr<IAudioProcessor>) -> Layout {
    let b = buses(component);
    let current = |dir: i32, i: i32| {
        let mut arr: SpeakerArrangement = 0;
        processor.getBusArrangement(dir, i, &mut arr);
        arr
    };
    let mut main_input = None;
    let mut ins: Vec<SpeakerArrangement> = Vec::new();
    for i in 0..b.audio_inputs {
        let mut info: ::vst3::Steinberg::Vst::BusInfo = std::mem::zeroed();
        component.getBusInfo(kAudio as i32, kInput as i32, i, &mut info);
        let is_main =
            info.busType == ::vst3::Steinberg::Vst::BusTypes_::kMain as i32 && main_input.is_none();
        if is_main {
            main_input = Some(i as usize);
        }
        if is_main || Some(i) == b.sidechain {
            ins.push(SpeakerArr::kStereo);
        } else {
            ins.push(current(kInput as i32, i));
        }
    }
    let mut outs: Vec<SpeakerArrangement> = (0..b.audio_outputs)
        .map(|i| current(kOutput as i32, i))
        .collect();
    if let Some(first) = outs.first_mut() {
        *first = SpeakerArr::kStereo;
    }
    processor.setBusArrangements(
        ins.as_mut_ptr(),
        ins.len() as i32,
        outs.as_mut_ptr(),
        outs.len() as i32,
    );
    let inputs: Vec<u32> = (0..b.audio_inputs)
        .map(|i| channels(current(kInput as i32, i)))
        .collect();
    let outputs: Vec<u32> = (0..b.audio_outputs)
        .map(|i| channels(current(kOutput as i32, i)))
        .collect();
    if let Some(i) = main_input {
        component.activateBus(kAudio as i32, kInput as i32, i as i32, 1);
    }
    if let Some(i) = b.sidechain {
        component.activateBus(kAudio as i32, kInput as i32, i, 1);
    }
    if b.audio_outputs > 0 {
        component.activateBus(kAudio as i32, kOutput as i32, 0, 1);
    }
    if b.event_inputs > 0 {
        component.activateBus(kEvent as i32, kInput as i32, 0, 1);
    }
    let mut buses = b;
    buses.main_inputs = main_input.map_or(0, |i| inputs[i]);
    buses.main_outputs = outputs.first().copied().unwrap_or(0);
    Layout {
        buses,
        inputs,
        outputs,
        main_input,
    }
}

#[cfg(test)]
mod tests {
    /// A program change picks one of the program list's programs (round 29, ADR 0062).
    #[test]
    fn a_program_change_picks_its_program() {
        use super::program_value;
        assert_eq!(program_value(0, 7), 0.0);
        assert_eq!(program_value(7, 7), 1.0);
        assert_eq!(program_value(3, 6), 0.5);
        assert_eq!(program_value(20, 7), 1.0, "past the last: the last");
        assert_eq!(program_value(127, 0), 1.0, "no steps said: over 127");
    }

    /// Review 1 of ADR 0062, #53: the program list is the parameter flagged
    /// `kIsProgramChange`, the root unit's first.
    #[test]
    fn the_program_list_is_found_by_its_flag() {
        use super::{program_list, ProgramCandidate};
        use ::vst3::Steinberg::Vst::ParameterInfo_::ParameterFlags_::kIsProgramChange;
        let p = |id, flags, unit, steps| ProgramCandidate {
            id,
            flags,
            unit,
            steps,
        };
        let list = kIsProgramChange as i32;
        assert_eq!(program_list(&[p(1, 0, 0, 3), p(2, 1, 0, 0)]), None);
        assert_eq!(
            program_list(&[p(1, 0, 0, 3), p(5, list, 2, 9), p(7, list | 1, 0, 15)]),
            Some((7, 15)),
            "the root unit's"
        );
        assert_eq!(program_list(&[p(5, list, 2, 9)]), Some((5, 9)));
    }
}
