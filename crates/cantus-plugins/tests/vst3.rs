//! Each exported plug-in through its VST3 entry point (`GetPluginFactory`), in-process:
//! the factory, the component, the processor and the controller, as a host drives them.

// The SDK's untyped enums are `int` on Windows and `unsigned int` elsewhere, so the casts
// between them and the context fields are no-ops here and needed there.
#![allow(clippy::unnecessary_cast)]

mod common;

use ::vst3::Steinberg::Vst::*;
use ::vst3::Steinberg::*;
use ::vst3::{Class, ComPtr, ComWrapper, Interface};
use cantus_core::{EffectKind, EffectSpec};
use cantus_plugins::ftz::DenormalGuard;
use cantus_plugins::params;
use common::*;
use std::cell::{Cell, RefCell};
use std::ffi::c_void;

// ---- A host's objects: a memory stream and parameter changes ---------------------------

struct MemStream {
    data: RefCell<Vec<u8>>,
    pos: Cell<usize>,
}

impl Class for MemStream {
    type Interfaces = (IBStream,);
}

impl IBStreamTrait for MemStream {
    unsafe fn read(&self, buffer: *mut c_void, n: int32, read: *mut int32) -> tresult {
        let data = self.data.borrow();
        let at = self.pos.get();
        // Short reads, to exercise the plug-in's loop.
        let n = (n.max(0) as usize).min(data.len() - at).min(11);
        std::ptr::copy_nonoverlapping(data[at..].as_ptr(), buffer as *mut u8, n);
        self.pos.set(at + n);
        if !read.is_null() {
            *read = n as int32;
        }
        kResultOk
    }
    unsafe fn write(&self, buffer: *mut c_void, n: int32, written: *mut int32) -> tresult {
        let n = n.max(0) as usize;
        self.data
            .borrow_mut()
            .extend_from_slice(std::slice::from_raw_parts(buffer as *const u8, n));
        if !written.is_null() {
            *written = n as int32;
        }
        kResultOk
    }
    unsafe fn seek(&self, pos: int64, _mode: int32, result: *mut int64) -> tresult {
        self.pos.set(pos.max(0) as usize);
        if !result.is_null() {
            *result = pos;
        }
        kResultOk
    }
    unsafe fn tell(&self, pos: *mut int64) -> tresult {
        *pos = self.pos.get() as int64;
        kResultOk
    }
}

fn stream(bytes: &[u8]) -> ComWrapper<MemStream> {
    ComWrapper::new(MemStream {
        data: RefCell::new(bytes.to_vec()),
        pos: Cell::new(0),
    })
}

struct Queue {
    id: ParamID,
    points: Vec<(int32, f64)>,
}

impl Class for Queue {
    type Interfaces = (IParamValueQueue,);
}

impl IParamValueQueueTrait for Queue {
    unsafe fn getParameterId(&self) -> ParamID {
        self.id
    }
    unsafe fn getPointCount(&self) -> int32 {
        self.points.len() as int32
    }
    unsafe fn getPoint(&self, i: int32, offset: *mut int32, value: *mut ParamValue) -> tresult {
        match self.points.get(i as usize) {
            Some((o, v)) => {
                *offset = *o;
                *value = *v;
                kResultOk
            }
            None => kInvalidArgument,
        }
    }
    unsafe fn addPoint(&self, _o: int32, _v: ParamValue, _i: *mut int32) -> tresult {
        kResultFalse
    }
}

struct Changes {
    queues: Vec<ComWrapper<Queue>>,
}

impl Class for Changes {
    type Interfaces = (IParameterChanges,);
}

impl IParameterChangesTrait for Changes {
    unsafe fn getParameterCount(&self) -> int32 {
        self.queues.len() as int32
    }
    unsafe fn getParameterData(&self, i: int32) -> *mut IParamValueQueue {
        self.queues
            .get(i as usize)
            .and_then(|q| q.as_com_ref::<IParamValueQueue>())
            .map_or(std::ptr::null_mut(), |r| r.as_ptr())
    }
    unsafe fn addParameterData(
        &self,
        _id: *const ParamID,
        _i: *mut int32,
    ) -> *mut IParamValueQueue {
        std::ptr::null_mut()
    }
}

// ---- The plug-in, as a VST3 host sees it -----------------------------------------------

struct V3 {
    component: ComPtr<IComponent>,
    processor: ComPtr<IAudioProcessor>,
    controller: ComPtr<IEditController>,
    /// The host's process context sent with each block (round 28, ADR 0055).
    context: std::cell::Cell<Option<ProcessContext>>,
}

fn factory() -> ComPtr<IPluginFactory3> {
    unsafe {
        let f = ComPtr::from_raw(cantus_plugins::vst3::GetPluginFactory()).unwrap();
        f.cast::<IPluginFactory3>().expect("IPluginFactory3")
    }
}

impl V3 {
    fn new(kind: EffectKind) -> V3 {
        let d = cantus_plugins::desc::of(kind).unwrap();
        let cid = cantus_plugins::vst3::cid_of(d);
        unsafe {
            let f = factory();
            let mut obj: *mut c_void = std::ptr::null_mut();
            let r = f.createInstance(
                cid.as_ptr() as FIDString,
                IComponent::IID.as_ptr() as FIDString,
                &mut obj,
            );
            assert_eq!(r, kResultOk, "{kind:?} is created");
            let component = ComPtr::from_raw(obj as *mut IComponent).unwrap();
            assert_eq!(component.initialize(std::ptr::null_mut()), kResultOk);
            let processor = component.cast::<IAudioProcessor>().unwrap();
            let controller = component.cast::<IEditController>().unwrap();
            V3 {
                component,
                processor,
                controller,
                context: std::cell::Cell::new(None),
            }
        }
    }

    fn set_state(&self, bytes: &[u8]) -> bool {
        let s = stream(bytes);
        let p = s.to_com_ptr::<IBStream>().unwrap();
        unsafe { self.component.setState(p.as_ptr()) == kResultOk }
    }

    fn get_state(&self) -> Vec<u8> {
        let s = stream(&[]);
        let p = s.to_com_ptr::<IBStream>().unwrap();
        unsafe { assert_eq!(self.component.getState(p.as_ptr()), kResultOk) };
        let out = s.data.borrow().clone();
        out
    }

    fn activate(&self, max: i32) {
        unsafe {
            let mut setup = ProcessSetup {
                processMode: ProcessModes_::kRealtime as int32,
                symbolicSampleSize: SymbolicSampleSizes_::kSample32 as int32,
                maxSamplesPerBlock: max,
                sampleRate: SR as f64,
            };
            assert_eq!(self.processor.setupProcessing(&mut setup), kResultOk);
            let audio = MediaTypes_::kAudio as MediaType;
            let input = BusDirections_::kInput as BusDirection;
            assert_eq!(self.component.activateBus(audio, input, 1, 1), kResultOk);
            assert_eq!(self.component.setActive(1), kResultOk);
            assert_eq!(self.processor.setProcessing(1), kResultOk);
        }
    }

    /// One block, the given points as (param id, [(offset, normalised)]).
    fn process(
        &self,
        l: &mut [f32],
        r: &mut [f32],
        key: Option<(&[f32], &[f32])>,
        points: &[(u32, Vec<(i32, f64)>)],
    ) {
        let n = l.len();
        let mut in_l = l.to_vec();
        let mut in_r = r.to_vec();
        let mut key_l = key.map(|k| k.0.to_vec()).unwrap_or_else(|| vec![0.0; n]);
        let mut key_r = key.map(|k| k.1.to_vec()).unwrap_or_else(|| vec![0.0; n]);
        let mut in_ptrs = [in_l.as_mut_ptr(), in_r.as_mut_ptr()];
        let mut key_ptrs = [key_l.as_mut_ptr(), key_r.as_mut_ptr()];
        let mut out_ptrs = [l.as_mut_ptr(), r.as_mut_ptr()];
        let mut inputs = [
            AudioBusBuffers {
                numChannels: 2,
                silenceFlags: 0,
                __field0: AudioBusBuffers__type0 {
                    channelBuffers32: in_ptrs.as_mut_ptr(),
                },
            },
            AudioBusBuffers {
                numChannels: 2,
                silenceFlags: 0,
                __field0: AudioBusBuffers__type0 {
                    channelBuffers32: key_ptrs.as_mut_ptr(),
                },
            },
        ];
        let mut outputs = [AudioBusBuffers {
            numChannels: 2,
            silenceFlags: 0,
            __field0: AudioBusBuffers__type0 {
                channelBuffers32: out_ptrs.as_mut_ptr(),
            },
        }];
        let changes = ComWrapper::new(Changes {
            queues: points
                .iter()
                .map(|(id, pts)| {
                    ComWrapper::new(Queue {
                        id: *id,
                        points: pts.clone(),
                    })
                })
                .collect(),
        });
        let changes_ptr = changes.to_com_ptr::<IParameterChanges>().unwrap();
        let mut context = self.context.get();
        let mut data = ProcessData {
            processMode: ProcessModes_::kRealtime as int32,
            symbolicSampleSize: SymbolicSampleSizes_::kSample32 as int32,
            numSamples: n as int32,
            numInputs: 2,
            numOutputs: 1,
            inputs: inputs.as_mut_ptr(),
            outputs: outputs.as_mut_ptr(),
            inputParameterChanges: changes_ptr.as_ptr(),
            outputParameterChanges: std::ptr::null_mut(),
            inputEvents: std::ptr::null_mut(),
            outputEvents: std::ptr::null_mut(),
            processContext: context
                .as_mut()
                .map_or(std::ptr::null_mut(), |c| c as *mut ProcessContext),
        };
        unsafe { assert_eq!(self.processor.process(&mut data), kResultOk) };
    }
}

impl Drop for V3 {
    fn drop(&mut self) {
        unsafe {
            self.processor.setProcessing(0);
            self.component.setActive(0);
            self.component.terminate();
        }
    }
}

fn state_of(kind: EffectKind, values: &[(&str, f32)]) -> Vec<u8> {
    serde_json::to_vec(&spec_with(kind, values)).unwrap()
}

#[test]
fn the_factory_describes_the_seven() {
    let f = factory();
    unsafe {
        assert_eq!(f.countClasses(), cantus_plugins::desc::ALL.len() as i32);
        for (i, d) in cantus_plugins::desc::ALL.iter().enumerate() {
            let mut info: PClassInfo2 = std::mem::zeroed();
            assert_eq!(f.getClassInfo2(i as int32, &mut info), kResultOk);
            assert_eq!(info.cid, cantus_plugins::vst3::cid_of(d));
            let sub = std::ffi::CStr::from_ptr(info.subCategories.as_ptr());
            assert_eq!(sub.to_str().unwrap(), d.vst3_categories);
            let name = std::ffi::CStr::from_ptr(info.name.as_ptr());
            assert_eq!(name.to_str().unwrap(), d.full_name);
        }
    }
}

#[test]
fn through_vst3_is_bit_identical_to_the_dsp() {
    for kind in kinds() {
        let values = settings(kind);
        let v = V3::new(kind);
        assert!(v.set_state(&state_of(kind, &values)));
        v.activate(1024);
        let spec = spec_with(kind, &values);
        let mut d = {
            let _g = DenormalGuard::new();
            Direct::new(&spec)
        };
        unsafe {
            assert_eq!(
                v.processor.getLatencySamples() as usize,
                d.latency(),
                "{kind:?} latency"
            );
        }
        let n = 24_000;
        let (mut l, mut r) = signal(n, 7);
        let (mut el, mut er) = (l.clone(), r.clone());
        for at in (0..n).step_by(1000) {
            let end = (at + 1000).min(n);
            v.process(&mut l[at..end], &mut r[at..end], None, &[]);
        }
        let _g = DenormalGuard::new();
        d.process(&mut el, &mut er, None);
        assert_identical(&format!("{kind:?} left"), &l, &el);
        assert_identical(&format!("{kind:?} right"), &r, &er);
    }
}

#[test]
fn vst3_automation_lands_on_the_grid() {
    let kind = EffectKind::Gate;
    let table = params::table(kind);
    let thr = table.by_key("thr").unwrap();
    let norm = thr.to_normalized(-20.0);
    let plain = thr.from_normalized(norm);
    let n = 24_000;
    // One point at 5003 in the block that starts at 5000.
    let v = V3::new(kind);
    v.activate(1000);
    let (mut l, mut r) = signal(n, 7);
    for at in (0..n).step_by(1000) {
        let pts = if at == 5000 {
            vec![(thr.id, vec![(3, norm)])]
        } else {
            vec![]
        };
        v.process(&mut l[at..at + 1000], &mut r[at..at + 1000], None, &pts);
    }
    let _g = DenormalGuard::new();
    let mut spec = EffectSpec::new(kind);
    let mut d = Direct::new(&spec);
    let (mut el, mut er) = signal(n, 7);
    let at = 5_008; // the first grid point at or after 5003
    d.process(&mut el[..at], &mut er[..at], None);
    spec.set("thr", plain);
    d.set(&spec);
    d.process(&mut el[at..], &mut er[at..], None);
    assert_identical("automated left", &l, &el);
    assert_identical("automated right", &r, &er);
}

#[test]
fn vst3_state_round_trips_and_is_the_clap_state() {
    for kind in kinds() {
        let values = settings(kind);
        let a = V3::new(kind);
        assert!(a.set_state(&state_of(kind, &values)));
        let bytes = a.get_state();
        let clap = Plug::new(kind);
        assert!(clap.load(&bytes));
        assert_eq!(clap.save(), bytes, "{kind:?}: one state for both standards");
        let b = V3::new(kind);
        assert!(b.set_state(&bytes));
        unsafe {
            for i in 0..a.controller.getParameterCount() {
                let mut info: ParameterInfo = std::mem::zeroed();
                assert_eq!(a.controller.getParameterInfo(i, &mut info), kResultOk);
                assert_eq!(
                    a.controller.getParamNormalized(info.id),
                    b.controller.getParamNormalized(info.id),
                    "{kind:?} param {i}"
                );
            }
        }
    }
}

#[test]
fn the_controller_speaks_the_tables() {
    for kind in kinds() {
        let v = V3::new(kind);
        let table = params::table(kind);
        unsafe {
            assert_eq!(
                v.controller.getParameterCount() as usize,
                table.params.len()
            );
            let units = v.component.cast::<IUnitInfo>().expect("IUnitInfo");
            assert_eq!(units.getUnitCount() as usize, 1 + table.groups().len());
            for (i, p) in table.params.iter().enumerate() {
                let mut info: ParameterInfo = std::mem::zeroed();
                assert_eq!(
                    v.controller.getParameterInfo(i as int32, &mut info),
                    kResultOk
                );
                assert_eq!(info.id, p.id);
                assert_eq!(info.stepCount, p.step_count());
                let norm = info.defaultNormalizedValue;
                // A fresh instance holds its starting preset's value (ADR 0071).
                let start = match p.role {
                    params::Role::Spec => p.clamp(f64::from(
                        cantus_core::starting::starting_spec(kind).get(p.key),
                    )),
                    _ => p.default,
                };
                assert_eq!(
                    v.controller.getParamNormalized(p.id),
                    p.to_normalized(start),
                    "{kind:?} {}",
                    p.key
                );
                let mut text: String128 = [0; 128];
                assert_eq!(
                    v.controller.getParamStringByValue(p.id, norm, &mut text),
                    kResultOk
                );
                let len = text.iter().position(|c| *c == 0).unwrap();
                let s = String::from_utf16(&text[..len]).unwrap();
                assert_eq!(s, p.text(p.default), "{kind:?} {}", p.key);
                let mut back = -1.0;
                assert_eq!(
                    v.controller
                        .getParamValueByString(p.id, text.as_mut_ptr(), &mut back),
                    kResultOk
                );
                assert!(
                    (back - norm).abs() < 0.01,
                    "{kind:?} {}: {s} read {back}, not {norm}",
                    p.key
                );
            }
        }
    }
}

#[test]
fn a_reverb_says_how_long_its_tail_is() {
    let v = V3::new(EffectKind::Space);
    assert!(v.set_state(&state_of(
        EffectKind::Space,
        &[("decay", 2.0), ("predelay", 20.0)]
    )));
    v.activate(512);
    let tail = unsafe { v.processor.getTailSamples() };
    let want = ((2.0 * 1.5 + 0.02 + 0.05) * SR) as u32;
    assert!(tail.abs_diff(want) <= 2, "tail {tail}, want about {want}");
    let g = V3::new(EffectKind::Gate);
    g.activate(512);
    assert_eq!(unsafe { g.processor.getTailSamples() }, 0);
}

#[cfg(all(feature = "editor", windows))]
#[test]
fn porta_offers_its_editor_as_a_vst3_view_and_the_others_do_not() {
    for kind in kinds() {
        let v = V3::new(kind);
        let view = unsafe { v.controller.createView(c"editor".as_ptr()) };
        if !cantus_plugins::editor::has_editor(kind) {
            assert!(view.is_null(), "{kind:?} has no editor of its own yet");
            continue;
        }
        let view = unsafe { ComPtr::from_raw(view) }.expect("its editor's view");
        unsafe {
            assert_eq!(view.isPlatformTypeSupported(c"HWND".as_ptr()), kResultTrue);
            assert_eq!(
                view.isPlatformTypeSupported(c"NSView".as_ptr()),
                kResultFalse
            );
            let mut rect: ViewRect = std::mem::zeroed();
            assert_eq!(view.getSize(&mut rect), kResultOk);
            // The device frame's designed size for the kind (ADR 0024 §8), at least.
            let (w, h) = cantus_plugins::editor::size(kind);
            assert!(
                (rect.right - rect.left) as f64 >= w && (rect.bottom - rect.top) as f64 >= h,
                "{kind:?}: {} × {}",
                rect.right - rect.left,
                rect.bottom - rect.top
            );
        }
    }
}

/// Round 28 (ADR 0055): the host's transport through VST3's `ProcessContext` — a synced
/// Tremor gate in another DAW follows that DAW's tempo and position, bit-identical to the
/// engine's DSP given the same song position, and the plug-in asks the host for it.
#[test]
fn a_synced_tremor_follows_the_hosts_tempo_and_position_through_vst3() {
    let gate = [
        ("depth", 1.0),
        ("sync", 1.0),
        ("div", 8.0),
        ("shape", 2.0),
        ("edge", 0.0),
    ];
    let (tempo, start, n) = (90.0f64, 2.5f64, 96_000usize);
    // The engine's DSP, given the song position once per block of 512.
    let spec = spec_with(EffectKind::Tremolo, &gate);
    let mut d = cantus_dsp::tremor::Tremor::new(SR);
    d.set(&cantus_engine::fx::tremor::settings(&spec));
    d.reset();
    let (mut el, mut er) = (vec![1.0f32; n], vec![1.0f32; n]);
    let start_samples = (start * 60.0 / tempo * SR as f64).round() as i64;
    let mut at = 0;
    while at < n {
        let m = 512.min(n - at);
        d.set_transport(&cantus_dsp::modfx_clock::Transport {
            playing: true,
            tempo_bpm: tempo,
            samples: start_samples + at as i64,
        });
        d.process(&mut el[at..at + m], &mut er[at..at + m]);
        at += m;
    }
    for blocks in [&[512][..], &[7], &[4096], &[5, 333, 64]] {
        let v = V3::new(EffectKind::Tremolo);
        assert!(v.set_state(&state_of(EffectKind::Tremolo, &gate)));
        v.activate(4096);
        unsafe {
            let req = v
                .component
                .cast::<IProcessContextRequirements>()
                .unwrap()
                .getProcessContextRequirements();
            use IProcessContextRequirements_::Flags_::*;
            for need in [kNeedTempo, kNeedProjectTimeMusic, kNeedTransportState] {
                assert!(req & need as u32 != 0, "the plug-in asks for {need}");
            }
        }
        let (mut l, mut r) = (vec![1.0f32; n], vec![1.0f32; n]);
        let (mut at, mut b) = (0usize, 0usize);
        while at < n {
            let m = blocks[b % blocks.len()].min(n - at);
            b += 1;
            let mut c: ProcessContext = unsafe { std::mem::zeroed() };
            use ProcessContext_::StatesAndFlags_::*;
            c.state = (kPlaying | kTempoValid | kProjectTimeMusicValid | kTimeSigValid) as u32;
            c.sampleRate = SR as f64;
            c.tempo = tempo;
            c.projectTimeMusic = start + at as f64 * tempo / 60.0 / SR as f64;
            c.timeSigNumerator = 4;
            c.timeSigDenominator = 4;
            v.context.set(Some(c));
            v.process(&mut l[at..at + m], &mut r[at..at + m], None, &[]);
            at += m;
        }
        assert_identical(&format!("VST3 blocks {blocks:?}"), &l, &el);
    }
    // It is the gate on the host's beats: shut at beat 2.5 + a little, open at beat 3.25.
    let at = |beat: f64| ((beat - start) * 60.0 / tempo * SR as f64) as usize;
    assert_eq!(el[at(2.7)], 0.0);
    assert_eq!(el[at(3.25)], 1.0);
}

/// Fix2 (review 2): a VST3 host that gives its tempo and the song in samples
/// (`projectTimeSamples`) but not in beats (no `kProjectTimeMusicValid`) plays as one that
/// gives the beats: the engine's DSP given the same song position.
#[test]
fn a_vst3_host_without_beats_follows_its_project_time_in_samples() {
    let gate = [
        ("depth", 1.0),
        ("sync", 1.0),
        ("div", 8.0),
        ("shape", 2.0),
        ("edge", 0.0),
    ];
    let (tempo, start_samples, n) = (100.0f64, 28_800i64, 48_000usize);
    let spec = spec_with(EffectKind::Tremolo, &gate);
    let mut d = cantus_dsp::tremor::Tremor::new(SR);
    d.set(&cantus_engine::fx::tremor::settings(&spec));
    d.reset();
    let (mut el, mut er) = (vec![1.0f32; n], vec![1.0f32; n]);
    for at in (0..n).step_by(512) {
        let m = 512.min(n - at);
        d.set_transport(&cantus_dsp::modfx_clock::Transport {
            playing: true,
            tempo_bpm: tempo,
            samples: start_samples + at as i64,
        });
        d.process(&mut el[at..at + m], &mut er[at..at + m]);
    }
    let v = V3::new(EffectKind::Tremolo);
    assert!(v.set_state(&state_of(EffectKind::Tremolo, &gate)));
    v.activate(4096);
    let (mut l, mut r) = (vec![1.0f32; n], vec![1.0f32; n]);
    for at in (0..n).step_by(512) {
        let m = 512.min(n - at);
        let mut c: ProcessContext = unsafe { std::mem::zeroed() };
        use ProcessContext_::StatesAndFlags_::*;
        c.state = (kPlaying | kTempoValid | kTimeSigValid) as u32;
        c.sampleRate = SR as f64;
        c.tempo = tempo;
        c.projectTimeSamples = start_samples + at as i64;
        // Beats the host does not vouch for: never read.
        c.projectTimeMusic = 1_000.0;
        c.timeSigNumerator = 4;
        c.timeSigDenominator = 4;
        v.context.set(Some(c));
        v.process(&mut l[at..at + m], &mut r[at..at + m], None, &[]);
    }
    assert_identical("VST3, the song in samples", &l, &el);
}
