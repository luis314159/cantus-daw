//! Notes into the exported plug-ins that hear them (round 28): a CLAP note port and a VST3
//! event bus on Tonus and Porta, none on the others, and the notes a host sends — in CLAP's
//! dialect, in MIDI bytes, as VST3 events — arrive at their sample: the plug-in renders
//! exactly what the DSP renders fed the same notes, at any block size. A flood of notes
//! loses no note-off, and CLAP's wildcards release only what they address.

mod common;

use ::vst3::Steinberg::Vst::*;
use ::vst3::Steinberg::*;
use ::vst3::{Class, ComPtr, ComWrapper, Interface};
use cantus_core::EffectKind;
use cantus_dsp::pitch::testkit::{self, Vowel};
use cantus_dsp::porta::{Porta, Trigger};
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
const TONUS: &[(&str, f32)] = &[("src", 1.0), ("speed", 0.0)];

/// Porta played by notes (its MIDI trigger), the whole range so a closed gate is plain.
const PORTA: &[(&str, f32)] = &[("trig", 1.0), ("range", 100.0)];

/// A note a host sends. In CLAP's dialect a note on or off carries the note id
/// `1000 + pitch` and the channel `pitch % 16` (in MIDI bytes, that channel too).
#[derive(Copy, Clone, Debug, PartialEq)]
enum N {
    On(u8),
    Off(u8),
    /// CLAP: an off with every field a wildcard; MIDI: CC 123 on every channel.
    AllOff,
    /// CLAP only: an off addressed by the note id of this pitch's on, its key −1.
    OffId(u8),
    /// An off for every key on this channel: in CLAP key and note id −1, in MIDI CC 123.
    OffChannel(i16),
}

/// The notes the tests play: (sample, note).
const NOTES: &[(u32, N)] = &[
    (9_001, N::On(67)),
    (21_337, N::Off(67)),
    (21_337, N::On(71)),
    (30_500, N::On(64)),
    (38_000, N::Off(71)),
    (40_123, N::Off(64)),
];

fn voice() -> (Vec<f32>, Vec<f32>) {
    let x = testkit::voice(SR, 1.0, Vowel::A, 0.4, |t| 200.0 + 20.0 * t as f32);
    (x.clone(), x)
}

/// What one case plays: the kind, its values, its input, its notes.
struct Case {
    kind: EffectKind,
    values: &'static [(&'static str, f32)],
    input: (Vec<f32>, Vec<f32>),
    notes: Vec<(u32, N)>,
}

impl Case {
    fn tonus() -> Case {
        Case {
            kind: EffectKind::AutoTune,
            values: TONUS,
            input: voice(),
            notes: NOTES.to_vec(),
        }
    }

    fn porta(notes: &[(u32, N)]) -> Case {
        Case {
            kind: EffectKind::Gate,
            values: PORTA,
            input: signal(SR as usize, 7),
            notes: notes.to_vec(),
        }
    }
}

/// The DSP itself, fed the notes the host's ones mean.
enum Dsp {
    Tonus(Box<Tonus>),
    Porta(Box<Porta>),
}

impl Dsp {
    fn note(&mut self, n: N) {
        match self {
            Dsp::Tonus(d) => d.note(
                0,
                match n {
                    N::On(p) => NoteEvent::On(p),
                    N::Off(p) => NoteEvent::Off(p),
                    N::AllOff => NoteEvent::AllOff,
                    _ => unreachable!("the DSP hears plain notes"),
                },
            ),
            Dsp::Porta(d) => d.note(
                0,
                match n {
                    N::On(p) => Trigger::On {
                        pitch: p,
                        velocity: 100,
                    },
                    N::Off(p) => Trigger::Off { pitch: p },
                    N::AllOff => Trigger::AllOff,
                    _ => unreachable!("the DSP hears plain notes"),
                },
            ),
        }
    }

    fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        match self {
            Dsp::Tonus(d) => d.process(l, r),
            Dsp::Porta(d) => d.process(l, r, None),
        }
    }
}

/// The DSP fed `expected` at their samples: processed up to each note's sample, the notes
/// of that sample handed over together, as a host's block hands them.
fn through_dsp(case: &Case, expected: &[(u32, N)]) -> Vec<f32> {
    let _guard = DenormalGuard::new();
    let spec = spec_with(case.kind, case.values);
    let mut d = match case.kind {
        EffectKind::AutoTune => {
            let mut t = Tonus::new(SR);
            t.set(&cantus_engine::fx::tonus::settings(&spec));
            Dsp::Tonus(Box::new(t))
        }
        EffectKind::Gate => {
            let mut p = Porta::new(SR);
            p.set(&cantus_engine::fx::gate::settings(&spec));
            Dsp::Porta(Box::new(p))
        }
        k => panic!("{k:?} hears no notes"),
    };
    let (mut l, mut r) = case.input.clone();
    let mut at = 0usize;
    for &(t, n) in expected {
        let t = t as usize;
        if t > at {
            d.process(&mut l[at..t], &mut r[at..t]);
            at = t;
        }
        d.note(n);
    }
    let end = l.len();
    d.process(&mut l[at..end], &mut r[at..end]);
    l.extend_from_slice(&r);
    l
}

/// The notes of `[at, at + n)`, at their offsets in that block.
fn block_notes(notes: &[(u32, N)], at: usize, n: usize) -> Vec<(u32, N)> {
    notes
        .iter()
        .filter(|(t, _)| (*t as usize) >= at && (*t as usize) < at + n)
        .map(|&(t, note)| (t - at as u32, note))
        .collect()
}

/// The level of the last 0.3 s of a render against the input's there (a render is left
/// then right, concatenated).
fn tail_gain(case: &Case, out: &[f32]) -> f32 {
    let half = out.len() / 2;
    let from = half - (0.3 * SR) as usize;
    let energy = |x: &[f32]| x.iter().map(|v| v * v).sum::<f32>();
    let input = energy(&case.input.0[from..half]);
    (energy(&out[from..half]) / input.max(1e-12)).sqrt()
}

// ---- CLAP ------------------------------------------------------------------------------

/// A host's event list holding notes, in either dialect.
struct Notes {
    clap: clap_input_events,
    // Boxed: `clap.ctx` points at the list, and the list at each event, none of which may
    // move while the plug-in reads them.
    #[allow(clippy::box_collection)]
    _heads: Box<Vec<*const clap_event_header>>,
    #[allow(clippy::vec_box)]
    _notes: Vec<Box<clap_event_note>>,
    #[allow(clippy::vec_box)]
    _midi: Vec<Box<clap_event_midi>>,
}

impl Notes {
    fn new(notes: &[(u32, N)], midi: bool) -> Notes {
        let mut heads: Box<Vec<*const clap_event_header>> = Box::default();
        let mut cn = Vec::new();
        let mut cm = Vec::new();
        for &(t, n) in notes {
            if midi {
                let bytes: Vec<[u8; 3]> = match n {
                    N::On(p) => vec![[0x90 | (p % 16), p, 100]],
                    N::Off(p) => vec![[0x80 | (p % 16), p, 0]],
                    // A channel's all-notes-off is its own: one on each.
                    N::AllOff => (0..16).map(|c| [0xb0 | c, 123, 0]).collect(),
                    N::OffChannel(c) => vec![[0xb0 | c as u8, 123, 0]],
                    other => panic!("{other:?} has no MIDI bytes"),
                };
                for data in bytes {
                    let e = Box::new(clap_event_midi {
                        header: header::<clap_event_midi>(t, CLAP_EVENT_MIDI),
                        port_index: 0,
                        data,
                    });
                    heads.push(&e.header);
                    cm.push(e);
                }
            } else {
                let id = |p: u8| 1000 + p as i32;
                // (type, note id, channel, key)
                let (type_, note_id, channel, key) = match n {
                    N::On(p) => (CLAP_EVENT_NOTE_ON, id(p), (p % 16) as i16, p as i16),
                    N::Off(p) => (CLAP_EVENT_NOTE_OFF, id(p), (p % 16) as i16, p as i16),
                    N::AllOff => (CLAP_EVENT_NOTE_OFF, -1, -1, -1),
                    N::OffId(p) => (CLAP_EVENT_NOTE_OFF, id(p), -1, -1),
                    N::OffChannel(c) => (CLAP_EVENT_NOTE_CHOKE, -1, c, -1),
                };
                let e = Box::new(clap_event_note {
                    header: header::<clap_event_note>(t, type_),
                    note_id,
                    port_index: 0,
                    channel,
                    key,
                    velocity: 100.0 / 127.0,
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
fn through_clap(case: &Case, blocks: &[usize], midi: bool) -> Vec<f32> {
    let plug = Plug::new(case.kind);
    let events: Vec<Ev> = case
        .values
        .iter()
        .map(|(k, v)| {
            let (id, value) = clap_value(plug.kind, k, *v);
            (0, id, value)
        })
        .collect();
    plug.flush(&events);
    plug.activate(4096);
    let (mut l, mut r) = case.input.clone();
    let (mut at, mut b) = (0usize, 0usize);
    while at < l.len() {
        let n = blocks[b % blocks.len()].min(l.len() - at);
        b += 1;
        let list = Notes::new(&block_notes(&case.notes, at, n), midi);
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

const BLOCKS: [&[usize]; 3] = [&[512], &[1, 64, 37, 4096, 333], &[48_000]];

#[test]
fn clap_notes_arrive_at_their_sample_in_either_dialect() {
    for case in [Case::tonus(), Case::porta(NOTES)] {
        let reference = through_dsp(&case, &case.notes);
        let dry = {
            let (mut l, r) = case.input.clone();
            l.extend_from_slice(&r);
            l
        };
        assert!(reference != dry, "{:?}: the notes change it", case.kind);
        for midi in [false, true] {
            for blocks in BLOCKS {
                assert_identical(
                    &format!(
                        "{:?}, CLAP, MIDI bytes {midi}, blocks {blocks:?}",
                        case.kind
                    ),
                    &reference,
                    &through_clap(&case, blocks, midi),
                );
            }
        }
    }
}

/// A note held, then every key released at once (a host's per-key sweep on stop); with
/// `all_off`, then 64 notes and an all-notes-off at once. Either way the gate ends closed.
fn flood(all_off: bool) -> Vec<(u32, N)> {
    let mut notes = vec![(4_000, N::On(72))];
    notes.extend((0..128).map(|p| (20_000, N::Off(p))));
    if all_off {
        notes.extend((0..64).map(|p| (30_000, N::On(p))));
        notes.push((30_000, N::AllOff));
    }
    notes
}

#[test]
fn a_flood_of_notes_through_clap_loses_no_note_off() {
    for all_off in [false, true] {
        let case = Case::porta(&flood(all_off));
        let reference = through_dsp(&case, &case.notes);
        let gain = tail_gain(&case, &reference);
        assert!(gain < 1e-3, "all_off {all_off}: the gate closed: {gain}");
        for midi in [false, true] {
            for blocks in BLOCKS {
                assert_identical(
                    &format!("all_off {all_off}, MIDI bytes {midi}, blocks {blocks:?}"),
                    &reference,
                    &through_clap(&case, blocks, midi),
                );
            }
        }
    }
}

/// More notes in one block than a plug-in's buffer holds (`NOTE_CAPACITY`), then one
/// channel's all-notes-off: the key that channel holds is released all the same — a
/// release the full buffer has no room for becomes an all-notes-off, and the tracker
/// forgets nothing it did not deliver.
#[test]
fn a_channels_all_notes_off_past_a_full_block_still_releases_its_key() {
    use cantus_plugins::instance::NOTE_CAPACITY;
    // 72 is held on channel 8; then, in one block, more than the buffer's worth of notes
    // played and released (none of them on 72), then channel 8's all-notes-off.
    let mut notes = vec![(4_000, N::On(72))];
    for i in 0..NOTE_CAPACITY / 2 + 64 {
        let p = (i % 60) as u8;
        notes.push((20_000, N::On(p)));
        notes.push((20_000, N::Off(p)));
    }
    notes.push((20_000, N::OffChannel(8)));
    assert!(notes.len() > NOTE_CAPACITY + 100);
    let case = Case::porta(&notes);
    for midi in [false, true] {
        for blocks in BLOCKS {
            let gain = tail_gain(&case, &through_clap(&case, blocks, midi));
            assert!(
                gain < 1e-3,
                "MIDI bytes {midi}, blocks {blocks:?}: the gate closed: {gain}"
            );
        }
    }
}

#[test]
fn a_clap_off_addressed_by_note_id_or_channel_releases_only_that() {
    // 60 is on channel 12, 72 on channel 8.
    let held = [(4_000, N::On(60)), (4_000, N::On(72))];
    for (off, means) in [(N::OffId(60), N::Off(60)), (N::OffChannel(8), N::Off(72))] {
        let mut sent = held.to_vec();
        sent.push((20_000, off));
        let case = Case::porta(&sent);
        let mut expected = held.to_vec();
        expected.push((20_000, means));
        let reference = through_dsp(&case, &expected);
        let gain = tail_gain(&case, &reference);
        assert!(
            gain > 0.5,
            "{off:?}: a note is still held, the gate open: {gain}"
        );
        for blocks in BLOCKS {
            assert_identical(
                &format!("{off:?}, blocks {blocks:?}"),
                &reference,
                &through_clap(&case, blocks, false),
            );
        }
    }
    // Every field a wildcard is every note.
    let mut sent = held.to_vec();
    sent.push((20_000, N::AllOff));
    let case = Case::porta(&sent);
    let gain = tail_gain(&case, &through_clap(&case, &[512], false));
    assert!(gain < 1e-3, "all released: {gain}");
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

fn vst3_event(offset: i32, n: N) -> Event {
    let mut e: Event = unsafe { std::mem::zeroed() };
    e.sampleOffset = offset;
    match n {
        N::On(pitch) => {
            e.r#type = Event_::EventTypes_::kNoteOnEvent as u16;
            e.__field0.noteOn = NoteOnEvent {
                channel: 0,
                pitch: pitch as i16,
                tuning: 0.0,
                velocity: 100.0 / 127.0,
                length: 0,
                noteId: -1,
            };
        }
        N::Off(pitch) => {
            e.r#type = Event_::EventTypes_::kNoteOffEvent as u16;
            e.__field0.noteOff = NoteOffEvent {
                channel: 0,
                pitch: pitch as i16,
                velocity: 0.0,
                noteId: -1,
                tuning: 0.0,
            };
        }
        other => panic!("VST3 has no {other:?}"),
    }
    e
}

const EVENT: MediaType = MediaTypes_::kEvent as MediaType;
const INPUT: BusDirection = BusDirections_::kInput as BusDirection;

/// A VST3 component of `kind`, initialised.
fn component(kind: EffectKind) -> ComPtr<IComponent> {
    let d = cantus_plugins::desc::of(kind).unwrap();
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
        component
    }
}

#[test]
fn the_kinds_that_hear_notes_have_a_vst3_event_bus_and_only_they() {
    for kind in kinds() {
        let c = component(kind);
        unsafe {
            let want = i32::from(kind.takes_notes());
            assert_eq!(c.getBusCount(EVENT, INPUT), want, "{kind:?}");
            let output = BusDirections_::kOutput as BusDirection;
            assert_eq!(c.getBusCount(EVENT, output), 0, "{kind:?}: no event output");
            let mut info: BusInfo = std::mem::zeroed();
            let got = c.getBusInfo(EVENT, INPUT, 0, &mut info);
            if kind.takes_notes() {
                assert_eq!(got, kResultOk, "{kind:?}");
                assert_eq!(info.mediaType, EVENT, "{kind:?}");
            } else {
                assert_eq!(got, kInvalidArgument, "{kind:?}: no event bus to describe");
                assert_ne!(c.activateBus(EVENT, INPUT, 0, 1), kResultOk, "{kind:?}");
            }
            c.terminate();
        }
    }
}

fn through_vst3(case: &Case, blocks: &[usize]) -> Vec<f32> {
    let component = component(case.kind);
    unsafe {
        let processor = component.cast::<IAudioProcessor>().unwrap();
        // The values, as a state.
        let state = serde_json::to_vec(&spec_with(case.kind, case.values)).unwrap();
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
        assert_eq!(component.activateBus(EVENT, INPUT, 0, 1), kResultOk);
        assert_eq!(component.setActive(1), kResultOk);
        assert_eq!(processor.setProcessing(1), kResultOk);
        let (mut l, mut r) = case.input.clone();
        let (mut at, mut b) = (0usize, 0usize);
        while at < l.len() {
            let n = blocks[b % blocks.len()].min(l.len() - at);
            b += 1;
            let events = ComWrapper::new(Events {
                list: block_notes(&case.notes, at, n)
                    .into_iter()
                    .map(|(t, note)| vst3_event(t as i32, note))
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
        l
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
    for case in [Case::tonus(), Case::porta(NOTES)] {
        let reference = through_dsp(&case, &case.notes);
        for blocks in &BLOCKS[..2] {
            assert_identical(
                &format!("{:?}, VST3, blocks {blocks:?}", case.kind),
                &reference,
                &through_vst3(&case, blocks),
            );
        }
    }
}

#[test]
fn a_flood_of_note_offs_through_vst3_closes_the_gate() {
    // VST3 has no all-notes-off: a host releases every key, one off each.
    let case = Case::porta(&flood(false));
    let reference = through_dsp(&case, &case.notes);
    for blocks in &BLOCKS[..2] {
        let out = through_vst3(&case, blocks);
        assert_identical(&format!("VST3, blocks {blocks:?}"), &reference, &out);
        let gain = tail_gain(&case, &out);
        assert!(gain < 1e-3, "blocks {blocks:?}: the gate closed: {gain}");
    }
}

/// The units queue a whole block's notes, as many as a plug-in takes: none acts early
/// (checked when this file compiles). Tonus hands its queue over every call; Porta's notes
/// may wait through its lookahead into the next block, so it queues two blocks' worth —
/// enough whenever the host's blocks are at least as long as that wait (at most 10 ms).
const _: () = {
    use cantus_plugins::instance::NOTE_CAPACITY;
    assert!(2 * NOTE_CAPACITY <= cantus_dsp::porta::MAX_TRIGGERS);
    assert!(NOTE_CAPACITY <= cantus_dsp::tonus::MAX_EVENTS);
};
