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
    /// fx/final — the `processMode` each block carries (real time unless a test renders
    /// offline).
    mode: std::cell::Cell<int32>,
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
                mode: std::cell::Cell::new(ProcessModes_::kRealtime as int32),
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
            processMode: self.mode.get(),
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

/// fxa:loquax — Loquax's carrier through VST3's sidechain bus (the aux input, activated as a
/// host does), the plug-in's switch on the external key in its state: exactly the DSP keyed
/// by the same sound, and loud where the unkeyed vocoder is silent.
#[test]
fn loquax_s_carrier_is_the_vst3_sidechain_bus() {
    let kind = EffectKind::Vocoder;
    // Off the starting preset's robot, the consonants' noise and sibilance off: only the
    // key carries.
    let values = [
        ("mix", 1.0),
        ("robot", 0.0),
        ("src", 0.0),
        ("bands", 16.0),
        ("sib", 0.0),
        ("unv", 0.0),
    ];
    let spec = spec_with(kind, &values);
    let mut state: serde_json::Value = serde_json::to_value(&spec).unwrap();
    state
        .as_object_mut()
        .unwrap()
        .insert("sidechain".into(), true.into());
    let v = V3::new(kind);
    assert!(v.set_state(&serde_json::to_vec(&state).unwrap()));
    v.activate(1024);
    let n = 24_000;
    let (key_l, key_r) = signal(n, 99);
    let (mut l, mut r) = signal(n, 3);
    let (mut el, mut er) = (l.clone(), r.clone());
    for at in (0..n).step_by(1000) {
        let end = (at + 1000).min(n);
        v.process(
            &mut l[at..end],
            &mut r[at..end],
            Some((&key_l[at..end], &key_r[at..end])),
            &[],
        );
    }
    let _g = DenormalGuard::new();
    let mut d = Direct::new(&spec);
    d.process(&mut el, &mut er, Some((&key_l, &key_r)));
    assert_identical("Loquax keyed, left", &l, &el);
    assert_identical("Loquax keyed, right", &r, &er);
    assert!(
        l.iter().map(|x| x * x).sum::<f32>() > 1.0,
        "the key carries"
    );
}

/// fxa:caelum — Caelum's ducking keyed by VST3's sidechain bus (the aux input, activated as a
/// host does), the plug-in's switch on the external key in its state: exactly the DSP keyed
/// by the same sound, and the wet stepped aside where the unkeyed one is not.
#[test]
fn caelum_s_ducking_is_keyed_by_the_vst3_sidechain_bus() {
    let kind = EffectKind::Shimmer;
    let values = [
        ("mix", 0.5),
        ("decay", 2.0),
        ("shim", 0.4),
        ("duck", 24.0),
        ("dthr", -30.0),
        ("datt", 5.0),
        ("drel", 200.0),
    ];
    let spec = spec_with(kind, &values);
    let n = 48_000;
    let (key_l, key_r) = signal(n, 99);
    let (quiet_l, quiet_r): (Vec<f32>, Vec<f32>) = {
        let (l, r) = signal(n, 3);
        (
            l.iter().map(|x| x * 0.01).collect(),
            r.iter().map(|x| x * 0.01).collect(),
        )
    };
    let mut wets = Vec::new();
    for keyed in [false, true] {
        let mut state: serde_json::Value = serde_json::to_value(&spec).unwrap();
        state
            .as_object_mut()
            .unwrap()
            .insert("sidechain".into(), keyed.into());
        let v = V3::new(kind);
        assert!(v.set_state(&serde_json::to_vec(&state).unwrap()));
        v.activate(1024);
        let (mut l, mut r) = (quiet_l.clone(), quiet_r.clone());
        for at in (0..n).step_by(1000) {
            let end = (at + 1000).min(n);
            v.process(
                &mut l[at..end],
                &mut r[at..end],
                Some((&key_l[at..end], &key_r[at..end])),
                &[],
            );
        }
        let _g = DenormalGuard::new();
        let mut d = Direct::new(&spec);
        let (mut el, mut er) = (quiet_l.clone(), quiet_r.clone());
        d.process(&mut el, &mut er, keyed.then_some((&key_l[..], &key_r[..])));
        assert_identical(&format!("Caelum keyed {keyed}, left"), &l, &el);
        assert_identical(&format!("Caelum keyed {keyed}, right"), &r, &er);
        wets.push(
            l.iter()
                .zip(&quiet_l)
                .map(|(o, i)| o - i)
                .collect::<Vec<f32>>(),
        );
    }
    let energy = |x: &[f32]| x[24_000..].iter().map(|v| v * v).sum::<f32>();
    assert!(
        energy(&wets[1]) < 0.1 * energy(&wets[0]),
        "the key ducks the wet: {} against {}",
        energy(&wets[1]),
        energy(&wets[0])
    );
}

/// fxa:caelum — Caelum's pre-delay synced to a VST3 host's tempo (`ProcessContext`): a
/// quarter at 90 BPM is 666.7 ms — bit-identical to the DSP with that pre-delay, and the wet's
/// first sound that far after the click (the earliest tap 13 ms on).
#[test]
fn caelum_s_synced_pre_delay_follows_the_vst3_hosts_tempo() {
    let kind = EffectKind::Shimmer;
    let tempo = 90.0f64;
    let values = [("mix", 1.0), ("psync", 6.0), ("decay", 1.0), ("out", 0.0)];
    let spec = spec_with(kind, &values);
    let n = 48_000;
    let mut input = (vec![0.0f32; n], vec![0.0f32; n]);
    input.0[0] = 1.0;
    input.1[0] = 1.0;
    let mut d = cantus_dsp::caelum::Caelum::new(SR);
    let mut s = cantus_engine::fx::caelum::settings(&spec);
    s.predelay_ms = cantus_dsp::caelum::sync_ms(1.0, tempo as f32);
    d.set(&s);
    d.reset();
    let (mut el, mut er) = input.clone();
    d.process(&mut el, &mut er, None);
    let v = V3::new(kind);
    assert!(v.set_state(&state_of(kind, &values)));
    v.activate(4096);
    let (mut l, mut r) = input.clone();
    for at in (0..n).step_by(512) {
        let m = 512.min(n - at);
        let mut c: ProcessContext = unsafe { std::mem::zeroed() };
        use ProcessContext_::StatesAndFlags_::*;
        c.state = (kPlaying | kTempoValid | kProjectTimeMusicValid | kTimeSigValid) as u32;
        c.sampleRate = SR as f64;
        c.tempo = tempo;
        c.projectTimeMusic = at as f64 * tempo / 60.0 / SR as f64;
        c.timeSigNumerator = 4;
        c.timeSigDenominator = 4;
        v.context.set(Some(c));
        v.process(&mut l[at..at + m], &mut r[at..at + m], None, &[]);
    }
    assert_identical("Caelum synced, left", &l, &el);
    assert_identical("Caelum synced, right", &r, &er);
    let first = l[1..].iter().position(|x| x.abs() > 1e-6).map(|i| i + 1);
    let want = (2.0 / 3.0 * SR) as usize;
    let first = first.expect("the wet sounds");
    assert!(
        first >= want && first < want + SR as usize / 20,
        "the wet after the quarter's pre-delay: {first} (want {want} and on)"
    );
}

/// fxb:vinculum — Vinculum's detector keyed by VST3's sidechain bus (the aux input,
/// activated as a host does), the plug-in's switch on the external key in its state:
/// exactly the DSP keyed by the same sound, and the quiet input pressed down where the
/// unkeyed one passes untouched.
#[test]
fn vinculum_s_key_is_the_vst3_sidechain_bus() {
    let kind = EffectKind::BusCompressor;
    let values = [
        ("thr", -30.0),
        ("ratio", 2.0),
        ("att", 0.0),
        ("rel", 1.0),
        ("makeup", 0.0),
        ("out", 0.0),
    ];
    let spec = spec_with(kind, &values);
    let n = 48_000;
    let (key_l, key_r) = signal(n, 99);
    let (quiet_l, quiet_r): (Vec<f32>, Vec<f32>) = {
        let (l, r) = signal(n, 3);
        (
            l.iter().map(|x| x * 0.01).collect(),
            r.iter().map(|x| x * 0.01).collect(),
        )
    };
    let mut outs = Vec::new();
    for keyed in [false, true] {
        let mut state: serde_json::Value = serde_json::to_value(&spec).unwrap();
        state
            .as_object_mut()
            .unwrap()
            .insert("sidechain".into(), keyed.into());
        let v = V3::new(kind);
        assert!(v.set_state(&serde_json::to_vec(&state).unwrap()));
        v.activate(1024);
        let (mut l, mut r) = (quiet_l.clone(), quiet_r.clone());
        for at in (0..n).step_by(1000) {
            let end = (at + 1000).min(n);
            v.process(
                &mut l[at..end],
                &mut r[at..end],
                Some((&key_l[at..end], &key_r[at..end])),
                &[],
            );
        }
        let _g = DenormalGuard::new();
        let mut d = Direct::new(&spec);
        let (mut el, mut er) = (quiet_l.clone(), quiet_r.clone());
        d.process(&mut el, &mut er, keyed.then_some((&key_l[..], &key_r[..])));
        assert_identical(&format!("Vinculum keyed {keyed}, left"), &l, &el);
        assert_identical(&format!("Vinculum keyed {keyed}, right"), &r, &er);
        outs.push(l);
    }
    assert_identical("unkeyed: the quiet input untouched", &outs[0], &quiet_l);
    let energy = |x: &[f32]| x[4_800..].iter().map(|v| v * v).sum::<f32>();
    assert!(
        energy(&outs[1]) < 0.25 * energy(&outs[0]),
        "the key presses the input: {} against {}",
        energy(&outs[1]),
        energy(&outs[0])
    );
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
        ("out", 0.0), // unity: the starting preset (ADR 0101) lifts `out`
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

/// fxa:fractus — Fractus's lanes on a VST3 host's beats (`ProcessContext`): its state's lanes
/// (the starting stutter) at 90 BPM from beat 2.5, bit-identical to the DSP given the same
/// song position, whatever the blocks.
#[test]
fn fractus_s_lanes_follow_the_hosts_tempo_through_vst3() {
    let kind = EffectKind::Glitch;
    let (tempo, start, n) = (90.0f64, 2.5f64, 96_000usize);
    let spec = spec_with(kind, &[]);
    let input = signal(n, 7);
    let mut d = cantus_dsp::fractus::Fractus::new(SR);
    let mut pat = Box::new(cantus_dsp::fractus::Pattern::EMPTY);
    d.set(&cantus_engine::fx::fractus::settings(&spec));
    cantus_engine::fx::fractus::pattern_into(spec.fractus.as_deref(), &mut pat);
    d.set_pattern(&pat);
    d.reset();
    let (mut el, mut er) = input.clone();
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
    assert!(diff(&el, &input.0).is_some(), "the stutter is heard");
    for blocks in [&[512][..], &[7], &[5, 333, 64]] {
        let v = V3::new(kind);
        assert!(v.set_state(&state_of(kind, &[])));
        v.activate(4096);
        let (mut l, mut r) = input.clone();
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
        assert_identical(&format!("Fractus VST3 blocks {blocks:?}"), &l, &el);
        assert_identical(&format!("Fractus VST3 blocks {blocks:?}, right"), &r, &er);
    }
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
        ("out", 0.0), // unity: the starting preset (ADR 0101) lifts `out`
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

/// fxb:lenis — a host's component handler that counts the restarts it is asked for.
struct Handler {
    latency_restarts: Cell<u32>,
}

impl Class for Handler {
    type Interfaces = (IComponentHandler,);
}

impl IComponentHandlerTrait for Handler {
    unsafe fn beginEdit(&self, _id: ParamID) -> tresult {
        kResultOk
    }
    unsafe fn performEdit(&self, _id: ParamID, _value: ParamValue) -> tresult {
        kResultOk
    }
    unsafe fn endEdit(&self, _id: ParamID) -> tresult {
        kResultOk
    }
    unsafe fn restartComponent(&self, flags: int32) -> tresult {
        if flags & RestartFlags_::kLatencyChanged as int32 != 0 {
            self.latency_restarts.set(self.latency_restarts.get() + 1);
        }
        kResultOk
    }
}

/// The host's UI thread for a while: its messages (the plug-in's timer among them)
/// dispatched until `done` or a second has passed.
#[cfg(windows)]
fn pump_until(done: impl Fn() -> bool) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
    };
    let start = std::time::Instant::now();
    while !done() && start.elapsed() < std::time::Duration::from_secs(1) {
        // SAFETY: this thread's own message queue.
        unsafe {
            let mut msg: MSG = std::mem::zeroed();
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// fxb:lenis — Lenis's latency through VST3 is its quality's frame (255, 511, 1 023
/// samples at 48 kHz) from `getLatencySamples`; a quality changed while it plays makes the
/// plug-in call the host's `restartComponent(kLatencyChanged)` from the UI thread's tick
/// (once), and the host reading the latency again gets the new one.
#[test]
fn lenis_s_latency_follows_its_quality_through_vst3() {
    let kind = EffectKind::ResonanceSuppressor;
    for (qual, want) in [(0.0, 255u32), (1.0, 511), (2.0, 1_023)] {
        let v = V3::new(kind);
        assert!(v.set_state(&state_of(kind, &[("qual", qual)])));
        v.activate(1024);
        unsafe {
            assert_eq!(v.processor.getLatencySamples(), want, "quality {qual}");
        }
    }
    let v = V3::new(kind);
    let handler = ComWrapper::new(Handler {
        latency_restarts: Cell::new(0),
    });
    let h = handler.to_com_ptr::<IComponentHandler>().unwrap();
    unsafe {
        assert_eq!(v.controller.setComponentHandler(h.as_ptr()), kResultOk);
    }
    v.activate(1024);
    unsafe {
        assert_eq!(v.processor.getLatencySamples(), 511, "Standard");
    }
    let qual = params::table(kind).by_key("qual").unwrap();
    let (mut l, mut r) = signal(1024, 3);
    v.process(
        &mut l,
        &mut r,
        None,
        &[(qual.id, vec![(0, qual.to_normalized(2.0))])],
    );
    v.process(&mut l, &mut r, None, &[]);
    #[cfg(windows)]
    {
        pump_until(|| handler.latency_restarts.get() > 0);
        assert_eq!(
            handler.latency_restarts.get(),
            1,
            "restartComponent(kLatencyChanged) asked for"
        );
        pump_until(|| false);
        assert_eq!(handler.latency_restarts.get(), 1, "once");
    }
    unsafe {
        assert_eq!(
            v.processor.getLatencySamples(),
            1_023,
            "the new latency read"
        );
        v.processor.setProcessing(0);
        v.component.setActive(0);
        assert_eq!(v.component.setActive(1), kResultOk);
        v.processor.setProcessing(1);
        assert_eq!(
            v.processor.getLatencySamples(),
            1_023,
            "after a restart too"
        );
        v.controller.setComponentHandler(std::ptr::null_mut());
    }
}

/// fxb:cedo — Cedo's sidechain trigger keyed by VST3's sidechain bus, the plug-in's switch
/// on the external key in its state: exactly the DSP keyed by the same sound; unkeyed, the
/// input passes untouched (its lookahead late).
#[test]
fn cedo_s_key_is_the_vst3_sidechain_bus() {
    let kind = EffectKind::Ducker;
    let values = [
        ("trig", 0.0),
        ("depth", 24.0),
        ("thr", -30.0),
        ("att", 0.5),
        ("hold", 20.0),
        ("rel", 80.0),
        ("look", 1.0),
    ];
    let spec = spec_with(kind, &values);
    let n = 48_000;
    let (key_l, key_r) = signal(n, 99);
    let (quiet_l, quiet_r): (Vec<f32>, Vec<f32>) = {
        let (l, r) = signal(n, 3);
        (
            l.iter().map(|x| x * 0.01).collect(),
            r.iter().map(|x| x * 0.01).collect(),
        )
    };
    let mut outs = Vec::new();
    for keyed in [false, true] {
        let mut state: serde_json::Value = serde_json::to_value(&spec).unwrap();
        state
            .as_object_mut()
            .unwrap()
            .insert("sidechain".into(), keyed.into());
        let v = V3::new(kind);
        assert!(v.set_state(&serde_json::to_vec(&state).unwrap()));
        v.activate(1024);
        let (mut l, mut r) = (quiet_l.clone(), quiet_r.clone());
        for at in (0..n).step_by(1000) {
            let end = (at + 1000).min(n);
            v.process(
                &mut l[at..end],
                &mut r[at..end],
                Some((&key_l[at..end], &key_r[at..end])),
                &[],
            );
        }
        let _g = DenormalGuard::new();
        let mut d = Direct::new(&spec);
        let (mut el, mut er) = (quiet_l.clone(), quiet_r.clone());
        d.process(&mut el, &mut er, keyed.then_some((&key_l[..], &key_r[..])));
        assert_identical(&format!("Cedo keyed {keyed}, left"), &l, &el);
        assert_identical(&format!("Cedo keyed {keyed}, right"), &r, &er);
        outs.push(l);
    }
    let energy = |x: &[f32]| x[4_800..].iter().map(|v| v * v).sum::<f32>();
    assert!(
        energy(&outs[1]) < 0.1 * energy(&outs[0]),
        "the key ducks it"
    );
}

/// fxb:cedo — Cedo's latency through VST3: its floor's kernel (and the lookahead) from
/// `getLatencySamples`; a crossover moved by the host leaves it, the floor committed moves it
/// (the plug-in asks the host to restart from its UI-thread tick, on Windows), and the host
/// reading it again gets the new one.
#[test]
fn cedo_s_latency_follows_its_committed_floor_through_vst3() {
    let kind = EffectKind::Ducker;
    let lows = [("band", 1.0), ("xover", 1.0), ("depth", 12.0)];
    for (extra, want) in [
        (&[][..], 1_765u32),
        (&[("lfloor", 3.0)][..], 1_325),
        (&[("look", 5.0)][..], 1_765 + 240),
    ] {
        let v = V3::new(kind);
        assert!(v.set_state(&state_of(kind, &[&lows[..], extra].concat())));
        v.activate(1024);
        unsafe {
            assert_eq!(v.processor.getLatencySamples(), want, "{extra:?}");
        }
    }
    let v = V3::new(kind);
    let handler = ComWrapper::new(Handler {
        latency_restarts: Cell::new(0),
    });
    let h = handler.to_com_ptr::<IComponentHandler>().unwrap();
    unsafe {
        assert_eq!(v.controller.setComponentHandler(h.as_ptr()), kResultOk);
    }
    assert!(v.set_state(&state_of(kind, &lows)));
    v.activate(1024);
    let xhz = params::table(kind).by_key("xhz").unwrap();
    let lfhz = params::table(kind).by_key("lfhz").unwrap();
    let (mut l, mut r) = signal(1024, 3);
    v.process(
        &mut l,
        &mut r,
        None,
        &[(xhz.id, vec![(0, xhz.to_normalized(400.0))])],
    );
    unsafe {
        assert_eq!(
            v.processor.getLatencySamples(),
            1_765,
            "the crossover alone: no change"
        );
    }
    v.process(
        &mut l,
        &mut r,
        None,
        &[(lfhz.id, vec![(0, lfhz.to_normalized(200.0))])],
    );
    v.process(&mut l, &mut r, None, &[]);
    #[cfg(windows)]
    {
        pump_until(|| handler.latency_restarts.get() > 0);
        assert_eq!(handler.latency_restarts.get(), 1, "one restart asked for");
    }
    let want = cantus_dsp::cedo::band::linear_latency(200.0, SR) as u32;
    unsafe {
        assert_eq!(v.processor.getLatencySamples(), want, "the 200 Hz floor's");
        v.processor.setProcessing(0);
        v.component.setActive(0);
        v.controller.setComponentHandler(std::ptr::null_mut());
    }
}

/// fxb:cedo — Cedo's tempo pump on VST3's `ProcessContext`, its drawn shape in the state:
/// the engine's unit given the same song position, to the bit, in blocks that divide
/// nothing.
#[test]
fn cedo_s_pump_follows_the_hosts_tempo_through_vst3() {
    use cantus_core::fx::cedo::Shape;
    use cantus_core::fx::lapsus::Point;
    let kind = EffectKind::Ducker;
    let mut spec = spec_with(kind, &[("trig", 1.0), ("depth", 30.0), ("div", 11.0)]);
    spec.cedo = Some(Box::new(Shape {
        points: vec![
            Point::new(0.0, 1.0).bend(-0.3),
            Point::new(0.6, 0.0).shape(2),
        ],
    }));
    let (tempo, start, n) = (128.0f64, 4.0f64, 96_000usize);
    let mut u = cantus_engine::fx::make(kind, SR).unwrap();
    u.configure(&spec);
    u.reset();
    let (mut el, mut er) = (vec![1.0f32; n], vec![1.0f32; n]);
    let spb = 60.0 / tempo * SR as f64;
    let mut at = 0;
    while at < n {
        let m = 512.min(n - at);
        let beats = start + at as f64 / spb;
        u.set_position(&cantus_engine::SongPosition {
            playing: true,
            tempo_bpm: tempo,
            tempo_now: tempo,
            samples: (beats * spb).round() as i64,
            beats,
            time_sig_num: 4,
            time_sig_den: 4,
            ..Default::default()
        });
        u.process(&mut el[at..at + m], &mut er[at..at + m], None);
        at += m;
    }
    for blocks in [&[512][..], &[5, 333, 64]] {
        let v = V3::new(kind);
        assert!(v.set_state(&serde_json::to_vec(&spec).unwrap()));
        v.activate(4096);
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
        assert_identical(&format!("Cedo, VST3 blocks {blocks:?}"), &l, &el);
    }
    assert!(el.iter().any(|v| *v < 0.1), "it pumps");
}

// ---- fx/final: a jump snaps (ADR 0027, amended) ---------------------------------------------

/// The host's context, playing (or stopped) at `beats`, 120 BPM.
fn context_at(beats: f64, playing: bool) -> ProcessContext {
    let mut c: ProcessContext = unsafe { std::mem::zeroed() };
    use ProcessContext_::StatesAndFlags_::*;
    let state = kTempoValid | kProjectTimeMusicValid | kTimeSigValid;
    c.state = (if playing { kPlaying | state } else { state }) as u32;
    c.sampleRate = SR as f64;
    c.tempo = 120.0;
    c.projectTimeMusic = beats;
    c.timeSigNumerator = 4;
    c.timeSigDenominator = 4;
    c
}

/// Utility's output gain (it glides over 20 ms), as VST3's normalised value.
fn out_norm(db: f32) -> (u32, f64) {
    let p = params::table(EffectKind::Utility).by_key("out").unwrap();
    (p.id, p.to_normalized(db))
}

/// The test signal through `v` in blocks of 512, its song playing from `from`; the gain to
/// +9 dB in the first block's first sample when `change`.
fn play_v3(v: &V3, from: f64, change: bool, frames: usize) -> Vec<f32> {
    let (mut l, mut r) = signal(frames, 11);
    let (id, norm) = out_norm(9.0);
    let mut at = 0;
    while at < frames {
        let m = 512.min(frames - at);
        let beats = from + at as f64 * 120.0 / 60.0 / SR as f64;
        v.context.set(Some(context_at(beats, true)));
        let pts = if at == 0 && change {
            vec![(id, vec![(0, norm)])]
        } else {
            vec![]
        };
        v.process(&mut l[at..at + m], &mut r[at..at + m], None, &pts);
        at += m;
    }
    l.extend_from_slice(&r);
    l
}

/// A Utility at 0 dB through 4 096 frames of silence, the host's song `playing` from 0.
fn warmed_v3(playing: bool) -> V3 {
    let v = V3::new(EffectKind::Utility);
    assert!(v.set_state(&state_of(EffectKind::Utility, &[("out", 0.0)])));
    v.activate(512);
    for k in 0..8 {
        let (mut l, mut r) = (vec![0.0f32; 512], vec![0.0f32; 512]);
        let beats = if playing {
            k as f64 * 512.0 * 2.0 / SR as f64
        } else {
            0.0
        };
        v.context.set(Some(context_at(beats, playing)));
        v.process(&mut l, &mut r, None, &[]);
    }
    v
}

/// A fresh Utility made at +9 dB, playing from `from`.
fn fresh_v3(from: f64) -> Vec<f32> {
    let v = V3::new(EffectKind::Utility);
    assert!(v.set_state(&state_of(EffectKind::Utility, &[("out", 9.0)])));
    v.activate(512);
    play_v3(&v, from, false, 8_192)
}

#[test]
fn a_vst3_seek_lands_a_value_changed_since_the_last_block_at_once() {
    let heard = play_v3(&warmed_v3(true), 8.0, true, 8_192);
    assert_identical("VST3 seek", &heard, &fresh_v3(8.0));
    let heard = play_v3(&warmed_v3(false), 0.0, true, 8_192);
    assert_identical("VST3 play from stop", &heard, &fresh_v3(0.0));
}

#[test]
fn a_vst3_offline_render_begun_lands_at_once_and_a_change_while_playing_glides() {
    let on = 4_096.0 * 2.0 / SR as f64;
    let v = warmed_v3(true);
    v.mode.set(ProcessModes_::kOffline as int32);
    let heard = play_v3(&v, on, true, 8_192);
    let landed = fresh_v3(on);
    assert_identical("VST3 offline render", &heard, &landed);
    // The same change while the song plays on in real time glides.
    let heard = play_v3(&warmed_v3(true), on, true, 8_192);
    let (at, _, _, _) = diff(&heard, &landed).expect("it glides");
    assert!(
        at < 960,
        "the glide begins at once (first difference at {at})"
    );
    assert_identical("landed", &heard[7_168..8_192], &landed[7_168..8_192]);
}

// ---- fxc:purus — the host's component handler is `Handler` (above, Lenis's), counting the
// restarts the plug-in asks for.

/// Run this thread's message loop for `ms` (the plug-in's latency tick is a thread timer on
/// the thread that initialised it, the host's UI thread).
#[cfg(windows)]
fn pump(ms: u64) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
    };
    let end = std::time::Instant::now() + std::time::Duration::from_millis(ms);
    while std::time::Instant::now() < end {
        // SAFETY: a plain message loop on this thread.
        unsafe {
            let mut msg: MSG = std::mem::zeroed();
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// fxc:purus — Purus's latency through VST3: a module switched by the host's automation
/// changes what `getLatencySamples` reports to the engine's own latency for the new
/// settings, and the plug-in tells the host on its UI thread (`restartComponent` with
/// `kLatencyChanged`, once per change) — the room module, the de-click, the gentle de-hum's
/// look-ahead, and the room off again.
#[test]
fn purus_s_latency_follows_its_modules_through_vst3() {
    let kind = EffectKind::Repair;
    let table = params::table(kind);
    let v = V3::new(kind);
    let handler = ComWrapper::new(Handler {
        latency_restarts: Cell::new(0),
    });
    let h = handler.to_com_ptr::<IComponentHandler>().unwrap();
    unsafe {
        assert_eq!(v.controller.setComponentHandler(h.as_ptr()), kResultOk);
    }
    v.activate(1024);
    assert_eq!(
        unsafe { v.processor.getLatencySamples() },
        0,
        "the starting preset notches: no latency"
    );
    let mut values: Vec<(&str, f32)> = Vec::new();
    let mut last = 0usize;
    for (key, value) in [("von", 1.0), ("con", 1.0), ("hgentle", 1.0), ("von", 0.0)] {
        let p = table.by_key(key).unwrap();
        let before = handler.latency_restarts.get();
        let (mut l, mut r) = signal(1024, 5);
        v.process(
            &mut l,
            &mut r,
            None,
            &[(p.id, vec![(0, p.to_normalized(value))])],
        );
        values.push((key, value));
        let want = engine_unit(&spec_with(kind, &values)).latency();
        assert_ne!(want, last, "{key} {value} moves the latency");
        #[cfg(windows)]
        {
            pump(200);
            assert_eq!(
                handler.latency_restarts.get(),
                before + 1,
                "{key} {value}: the host is told, once"
            );
        }
        let _ = before;
        assert_eq!(
            unsafe { v.processor.getLatencySamples() } as usize,
            want,
            "{key} {value}: the engine's latency"
        );
        last = want;
    }
    unsafe {
        v.controller.setComponentHandler(std::ptr::null_mut());
    }
}
