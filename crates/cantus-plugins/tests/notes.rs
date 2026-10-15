//! Notes into the exported plug-ins that hear them (round 28): a CLAP note port and a VST3
//! event bus on Tonus and Porta, none on the others, and the notes a host sends — in CLAP's
//! dialect, in MIDI bytes, as VST3 events — arrive at their sample: the plug-in renders
//! exactly what the DSP renders fed the same notes, at any block size.

mod common;

use ::vst3::Steinberg::Vst::*;
use ::vst3::Steinberg::*;
use ::vst3::{Class, ComPtr, ComWrapper, Interface};
use cantus_core::EffectKind;
use cantus_dsp::pitch::testkit::{self, Vowel};
use cantus_dsp::tonus::{NoteEvent, Tonus};
use cantus_plugins::ftz::DenormalGuard;
use clap_sys::audio_buffer::clap_audio_buffer;
use clap_sys::events::*;
use clap_sys::ext::note_ports::*;
use clap_sys::process::*;
use common::*;
use std::ffi::c_void;
use std::ptr;

/// Tonus tuning to the notes it is played: the MIDI source, the hard effect.
const VALUES: &[(&str, f32)] = &[("src", 1.0), ("speed", 0.0)];

/// The notes the tests play: (sample, on?, pitch).
const NOTES: &[(u32, bool, u8)] = &[
    (9_001, true, 67),
    (21_337, false, 67),
    (21_337, true, 71),
    (30_500, true, 64),
    (38_000, false, 71),
    (40_123, false, 64),
];

fn voice() -> (Vec<f32>, Vec<f32>) {
    let x = testkit::voice(SR, 1.0, Vowel::A, 0.4, |t| 200.0 + 20.0 * t as f32);
    (x.clone(), x)
}

/// The DSP fed the notes directly, in one call.
fn through_dsp() -> Vec<f32> {
    let _guard = DenormalGuard::new();
    let spec = spec_with(EffectKind::AutoTune, VALUES);
    let mut d = Tonus::new(SR);
    d.set(&cantus_engine::fx::tonus::settings(&spec));
    for &(at, on, pitch) in NOTES {
        d.note(
            at as usize,
            if on {
                NoteEvent::On(pitch)
            } else {
                NoteEvent::Off(pitch)
            },
        );
    }
    let (mut l, mut r) = voice();
    d.process(&mut l, &mut r);
    l.extend_from_slice(&r);
    l
}

// ---- CLAP ------------------------------------------------------------------------------

/// A host's event list holding notes, in either dialect.
struct Notes {
    clap: clap_input_events,
    _heads: Box<Vec<*const clap_event_header>>,
    _notes: Vec<Box<clap_event_note>>,
    _midi: Vec<Box<clap_event_midi>>,
}

impl Notes {
    fn new(notes: &[(u32, bool, u8)], midi: bool) -> Notes {
        let mut heads: Box<Vec<*const clap_event_header>> = Box::default();
        let mut cn = Vec::new();
        let mut cm = Vec::new();
        for &(t, on, pitch) in notes {
            if midi {
                let e = Box::new(clap_event_midi {
                    header: header::<clap_event_midi>(t, CLAP_EVENT_MIDI),
                    port_index: 0,
                    data: [if on { 0x90 } else { 0x80 }, pitch, if on { 100 } else { 0 }],
                });
                heads.push(&e.header);
                cm.push(e);
            } else {
                let e = Box::new(clap_event_note {
                    header: header::<clap_event_note>(
                        t,
                        if on {
                            CLAP_EVENT_NOTE_ON
                        } else {
                            CLAP_EVENT_NOTE_OFF
                        },
                    ),
                    note_id: -1,
                    port_index: 0,
                    channel: 0,
                    key: pitch as i16,
                    velocity: 0.8,
                });
                heads.push(&e.header);
                cn.push(e);
            }
        }
        let ctx = &mut *heads as *mut Vec<*const clap_event_header> as *mut c_void;
        Notes {
            clap: clap_input_events {
                ctx,
                size: Some(size),
                get: Some(get),
            },
            _heads: heads,
            _notes: cn,
            _midi: cm,
        }
    }
}

fn header<T>(time: u32, type_: u16) -> clap_event_header {
    clap_event_header {
        size: std::mem::size_of::<T>() as u32,
        time,
        space_id: CLAP_CORE_EVENT_SPACE_ID,
        type_,
        flags: 0,
    }
}

unsafe extern "C" fn size(l: *const clap_input_events) -> u32 {
    (*((*l).ctx as *const Vec<*const clap_event_header>)).len() as u32
}

unsafe extern "C" fn get(l: *const clap_input_events, i: u32) -> *const clap_event_header {
    (&(*((*l).ctx as *const Vec<*const clap_event_header>)))[i as usize]
}

unsafe extern "C" fn out_push(_l: *const clap_output_events, _e: *const clap_event_header) -> bool {
    true
}

static OUT: clap_output_events = clap_output_events {
    ctx: ptr::null_mut(),
    try_push: Some(out_push),
};

fn note_port_count(plug: &Plug) -> u32 {
    unsafe {
        let ext = (*plug.plugin).get_extension.unwrap()(plug.plugin, CLAP_EXT_NOTE_PORTS.as_ptr());
        if ext.is_null() {
            return 0;
        }
        let ports = &*(ext as *const clap_plugin_note_ports);
        let n = ports.count.unwrap()(plug.plugin, true);
        if n > 0 {
            let mut info: clap_note_port_info = std::mem::zeroed();
            assert!(ports.get.unwrap()(plug.plugin, 0, true, &mut info));
            assert_ne!(info.supported_dialects & CLAP_NOTE_DIALECT_MIDI, 0);
            assert_ne!(info.supported_dialects & CLAP_NOTE_DIALECT_CLAP, 0);
        }
        assert_eq!(ports.count.unwrap()(plug.plugin, false), 0);
        n
    }
}

#[test]
fn the_kinds_that_hear_notes_have_a_clap_note_port_and_only_they() {
    for kind in kinds() {
        let plug = Plug::new(kind);
        let want = u32::from(kind.takes_notes());
        assert_eq!(note_port_count(&plug), want, "{kind:?}");
    }
}

/// Through CLAP, in blocks of `blocks`, the notes in CLAP's dialect or as MIDI bytes.
fn through_clap(blocks: &[usize], midi: bool) -> Vec<f32> {
    let plug = Plug::new(EffectKind::AutoTune);
    let events: Vec<Ev> = VALUES
        .iter()
        .map(|(k, v)| {
            let (id, value) = clap_value(plug.kind, k, *v);
            (0, id, value)
        })
        .collect();
    plug.flush(&events);
    plug.activate(4096);
    let (mut l, mut r) = voice();
    let (mut at, mut b) = (0usize, 0usize);
    while at < l.len() {
        let n = blocks[b % blocks.len()].min(l.len() - at);
        b += 1;
        let notes: Vec<(u32, bool, u8)> = NOTES
            .iter()
            .filter(|(t, _, _)| (*t as usize) >= at && (*t as usize) < at + n)
            .map(|&(t, on, p)| (t - at as u32, on, p))
            .collect();
        let list = Notes::new(&notes, midi);
        let mut ptrs = [l[at..].as_mut_ptr(), r[at..].as_mut_ptr()];
        let mut outs = [l[at..].as_mut_ptr(), r[at..].as_mut_ptr()];
        let inputs = [clap_audio_buffer {
            data32: ptrs.as_mut_ptr(),
            data64: ptr::null_mut(),
            channel_count: 2,
            latency: 0,
            constant_mask: 0,
        }];
        let mut outputs = [clap_audio_buffer {
            data32: outs.as_mut_ptr(),
            data64: ptr::null_mut(),
            channel_count: 2,
            latency: 0,
            constant_mask: 0,
        }];
        let p = clap_process {
            steady_time: -1,
            frames_count: n as u32,
            transport: ptr::null(),
            audio_inputs: inputs.as_ptr(),
            audio_outputs: outputs.as_mut_ptr(),
            audio_inputs_count: 1,
            audio_outputs_count: 1,
            in_events: &list.clap,
            out_events: &OUT,
        };
        let status = unsafe { (*plug.plugin).process.unwrap()(plug.plugin, &p) };
        assert_eq!(status, CLAP_PROCESS_CONTINUE);
        at += n;
    }
    l.extend_from_slice(&r);
    l
}

#[test]
fn clap_notes_arrive_at_their_sample_in_either_dialect() {
    let reference = through_dsp();
    let dry = {
        let (mut l, r) = voice();
        l.extend_from_slice(&r);
        l
    };
    assert!(reference != dry, "the notes move the voice");
    for midi in [false, true] {
        for blocks in [&[512usize][..], &[1, 64, 37, 4096, 333][..], &[48_000][..]] {
            assert_identical(
                &format!("CLAP, MIDI bytes {midi}, blocks {blocks:?}"),
                &reference,
                &through_clap(blocks, midi),
            );
        }
    }
}

// ---- VST3 ------------------------------------------------------------------------------

struct Events {
    list: Vec<Event>,
}

impl Class for Events {
    type Interfaces = (IEventList,);
}

impl IEventListTrait for Events {
    unsafe fn getEventCount(&self) -> int32 {
        self.list.len() as int32
    }
    unsafe fn getEvent(&self, i: int32, e: *mut Event) -> tresult {
        match self.list.get(i as usize) {
            Some(ev) => {
                *e = *ev;
                kResultOk
            }
            None => kInvalidArgument,
        }
    }
    unsafe fn addEvent(&self, _e: *mut Event) -> tresult {
        kResultFalse
    }
}

fn vst3_event(offset: i32, on: bool, pitch: u8) -> Event {
    let mut e: Event = unsafe { std::mem::zeroed() };
    e.sampleOffset = offset;
    if on {
        e.r#type = Event_::EventTypes_::kNoteOnEvent as u16;
        e.__field0.noteOn = NoteOnEvent {
            channel: 0,
            pitch: pitch as i16,
            tuning: 0.0,
            velocity: 0.8,
            length: 0,
            noteId: -1,
        };
    } else {
        e.r#type = Event_::EventTypes_::kNoteOffEvent as u16;
        e.__field0.noteOff = NoteOffEvent {
            channel: 0,
            pitch: pitch as i16,
            velocity: 0.0,
            noteId: -1,
            tuning: 0.0,
        };
    }
    e
}

fn through_vst3(blocks: &[usize]) -> (Vec<f32>, int32) {
    let d = cantus_plugins::desc::of(EffectKind::AutoTune).unwrap();
    let cid = cantus_plugins::vst3::cid_of(d);
    unsafe {
        let f = ComPtr::from_raw(cantus_plugins::vst3::GetPluginFactory()).unwrap();
        let f = f.cast::<IPluginFactory3>().unwrap();
        let mut obj: *mut c_void = ptr::null_mut();
        let r = f.createInstance(
            cid.as_ptr() as FIDString,
            IComponent::IID.as_ptr() as FIDString,
            &mut obj,
        );
        assert_eq!(r, kResultOk);
        let component = ComPtr::from_raw(obj as *mut IComponent).unwrap();
        assert_eq!(component.initialize(ptr::null_mut()), kResultOk);
        let processor = component.cast::<IAudioProcessor>().unwrap();
        let event = MediaTypes_::kEvent as MediaType;
        let input = BusDirections_::kInput as BusDirection;
        let buses = component.getBusCount(event, input);
        let mut info: BusInfo = std::mem::zeroed();
        assert_eq!(component.getBusInfo(event, input, 0, &mut info), kResultOk);
        assert_eq!(info.mediaType, event);
        // The values, as a state.
        let state = serde_json::to_vec(&spec_with(EffectKind::AutoTune, VALUES)).unwrap();
        let stream = ComWrapper::new(Mem {
            data: std::cell::RefCell::new(state),
            pos: std::cell::Cell::new(0),
        });
        let sp = stream.to_com_ptr::<IBStream>().unwrap();
        assert_eq!(component.setState(sp.as_ptr()), kResultOk);
        let mut setup = ProcessSetup {
            processMode: ProcessModes_::kRealtime as int32,
            symbolicSampleSize: SymbolicSampleSizes_::kSample32 as int32,
            maxSamplesPerBlock: 4096,
            sampleRate: SR as f64,
        };
        assert_eq!(processor.setupProcessing(&mut setup), kResultOk);
        assert_eq!(component.activateBus(event, input, 0, 1), kResultOk);
        assert_eq!(component.setActive(1), kResultOk);
        assert_eq!(processor.setProcessing(1), kResultOk);
        let (mut l, mut r) = voice();
        let (mut at, mut b) = (0usize, 0usize);
        while at < l.len() {
            let n = blocks[b % blocks.len()].min(l.len() - at);
            b += 1;
            let events = ComWrapper::new(Events {
                list: NOTES
                    .iter()
                    .filter(|(t, _, _)| (*t as usize) >= at && (*t as usize) < at + n)
                    .map(|&(t, on, p)| vst3_event(t as i32 - at as i32, on, p))
                    .collect(),
            });
            let ep = events.to_com_ptr::<IEventList>().unwrap();
            let mut ptrs = [l[at..].as_mut_ptr(), r[at..].as_mut_ptr()];
            let mut outs = [l[at..].as_mut_ptr(), r[at..].as_mut_ptr()];
            let mut inputs = [AudioBusBuffers {
                numChannels: 2,
                silenceFlags: 0,
                __field0: AudioBusBuffers__type0 {
                    channelBuffers32: ptrs.as_mut_ptr(),
                },
            }];
            let mut outputs = [AudioBusBuffers {
                numChannels: 2,
                silenceFlags: 0,
                __field0: AudioBusBuffers__type0 {
                    channelBuffers32: outs.as_mut_ptr(),
                },
            }];
            let mut data = ProcessData {
                processMode: ProcessModes_::kRealtime as int32,
                symbolicSampleSize: SymbolicSampleSizes_::kSample32 as int32,
                numSamples: n as int32,
                numInputs: 1,
                numOutputs: 1,
                inputs: inputs.as_mut_ptr(),
                outputs: outputs.as_mut_ptr(),
                inputParameterChanges: ptr::null_mut(),
                outputParameterChanges: ptr::null_mut(),
                inputEvents: ep.as_ptr(),
                outputEvents: ptr::null_mut(),
                processContext: ptr::null_mut(),
            };
            assert_eq!(processor.process(&mut data), kResultOk);
            at += n;
        }
        processor.setProcessing(0);
        component.setActive(0);
        component.terminate();
        l.extend_from_slice(&r);
        (l, buses)
    }
}

struct Mem {
    data: std::cell::RefCell<Vec<u8>>,
    pos: std::cell::Cell<usize>,
}

impl Class for Mem {
    type Interfaces = (IBStream,);
}

impl IBStreamTrait for Mem {
    unsafe fn read(&self, buffer: *mut c_void, n: int32, read: *mut int32) -> tresult {
        let data = self.data.borrow();
        let at = self.pos.get();
        let k = (n.max(0) as usize).min(data.len() - at);
        ptr::copy_nonoverlapping(data[at..].as_ptr(), buffer as *mut u8, k);
        self.pos.set(at + k);
        if !read.is_null() {
            *read = k as int32;
        }
        kResultOk
    }
    unsafe fn write(&self, _b: *mut c_void, _n: int32, _w: *mut int32) -> tresult {
        kResultFalse
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

#[test]
fn vst3_notes_arrive_at_their_sample_on_the_event_bus() {
    let reference = through_dsp();
    for blocks in [&[512usize][..], &[1, 64, 37, 4096, 333][..]] {
        let (out, buses) = through_vst3(blocks);
        assert_eq!(buses, 1, "one event input");
        assert_identical(&format!("VST3, blocks {blocks:?}"), &reference, &out);
    }
}
