//! The audio-thread half: `process` with everything preallocated.
//!
//! Per block: adopt a new parameter map if the main thread posted one; `setProcessing(true)`
//! the first time; the controller's edits (from the ring) and the host's changes into the
//! input queues, the editor's gestures into `ParamOut`; notes into the event list; the
//! transport into the `ProcessContext`; the main input copied to scratch and the main
//! output written straight into the host's buffers; `process`; the processor's own
//! output changes into `ParamOut` and, like the host's changes, on to the controller.
//! Nothing here allocates, locks or panics (ADR 0005).

use std::sync::Arc;

use ::vst3::ComWrapper;
use ::vst3::Steinberg::Vst::Event_::EventFlags_::kIsLive;
use ::vst3::Steinberg::Vst::Event_::EventTypes_::{
    kNoteExpressionValueEvent, kNoteOffEvent, kNoteOnEvent, kPolyPressureEvent,
};
use ::vst3::Steinberg::Vst::NoteExpressionTypeIDs_::{
    kBrightnessTypeID, kExpressionTypeID, kPanTypeID, kTuningTypeID, kVibratoTypeID, kVolumeTypeID,
};
use ::vst3::Steinberg::Vst::ProcessContext_::StatesAndFlags_::{
    kBarPositionValid, kContTimeValid, kCycleActive, kCycleValid, kPlaying, kProjectTimeMusicValid,
    kRecording, kTempoValid, kTimeSigValid,
};
use ::vst3::Steinberg::Vst::ProcessModes_::kRealtime;
use ::vst3::Steinberg::Vst::SymbolicSampleSizes_::kSample32;
use ::vst3::Steinberg::Vst::{
    AudioBusBuffers, AudioBusBuffers__type0, Event, IAudioProcessorTrait, IEventList,
    IParameterChanges, NoteExpressionValueEvent, NoteOffEvent, NoteOnEvent, ParamID,
    PolyPressureEvent, ProcessContext, ProcessData,
};

use super::host_objects::Edit;
use super::instance::Instance;
use super::params::ParamMap;
use super::process_objects::{EventList, ParamChanges};
use crate::backend::{
    Audio, AudioThread, NoteEvent, NoteExpression, NoteKind, ParamChange, ParamEvent, ParamOut,
    TransportInfo,
};

/// Scratch channels and the bus descriptions pointing at them.
struct Buffers {
    /// One per channel of every bus; never reallocated after construction.
    storage: Vec<Vec<f32>>,
    in_ptrs: Vec<Vec<*mut f32>>,
    out_ptrs: Vec<Vec<*mut f32>>,
    inputs: Vec<AudioBusBuffers>,
    outputs: Vec<AudioBusBuffers>,
}

// SAFETY: the raw pointers point into `storage` (owned here) or, during one `process`
// call, into the caller's buffers; the struct moves between threads only as a whole.
unsafe impl Send for Buffers {}

impl Buffers {
    fn new(inputs: &[u32], outputs: &[u32], max_block: usize) -> Buffers {
        let mut storage = Vec::new();
        let mut make = |counts: &[u32]| -> Vec<Vec<*mut f32>> {
            counts
                .iter()
                .map(|&n| {
                    (0..n)
                        .map(|_| {
                            storage.push(vec![0.0f32; max_block.max(1)]);
                            storage.last_mut().expect("just pushed").as_mut_ptr()
                        })
                        .collect()
                })
                .collect()
        };
        let mut in_ptrs = make(inputs);
        let mut out_ptrs = make(outputs);
        let bus = |ptrs: &mut Vec<*mut f32>| AudioBusBuffers {
            numChannels: ptrs.len() as i32,
            silenceFlags: 0,
            __field0: AudioBusBuffers__type0 {
                channelBuffers32: ptrs.as_mut_ptr(),
            },
        };
        let inputs = in_ptrs.iter_mut().map(bus).collect();
        let outputs = out_ptrs.iter_mut().map(bus).collect();
        Buffers {
            storage,
            in_ptrs,
            out_ptrs,
            inputs,
            outputs,
        }
    }
}

pub struct AudioHalf {
    inst: Arc<Instance>,
    map: Box<ParamMap>,
    from_ui: rtrb::Consumer<Edit>,
    to_ui: rtrb::Producer<(ParamID, f64)>,
    in_changes: ComWrapper<ParamChanges>,
    out_changes: ComWrapper<ParamChanges>,
    in_events: ComWrapper<EventList>,
    out_events: ComWrapper<EventList>,
    context: Box<ProcessContext>,
    buffers: Buffers,
    processing: bool,
    frames: i64,
    max_block: usize,
    has_event_input: bool,
    /// Parameters a MIDI controller drove this block (through `IMidiMapping`), so the
    /// controller hears them as it hears the host's changes.
    midi_ids: [ParamID; MIDI_IDS],
    midi_len: usize,
}

/// Distinct MIDI-mapped parameters reported to the controller per block.
const MIDI_IDS: usize = 32;

impl AudioHalf {
    /// Main thread: everything the audio thread will use, allocated now.
    pub fn new(
        inst: Arc<Instance>,
        map: ParamMap,
        from_ui: rtrb::Consumer<Edit>,
        to_ui: rtrb::Producer<(ParamID, f64)>,
    ) -> AudioHalf {
        let max_block = inst.setup.max_block.max(1);
        let buffers = Buffers::new(&inst.layout.inputs, &inst.layout.outputs, max_block);
        // SAFETY: `ProcessContext` is plain data; all-zero is a valid value.
        let mut context: Box<ProcessContext> = Box::new(unsafe { std::mem::zeroed() });
        context.sampleRate = inst.setup.sample_rate;
        let has_event_input = inst.layout.buses.event_inputs > 0;
        AudioHalf {
            map: Box::new(map),
            from_ui,
            to_ui,
            in_changes: ComWrapper::new(ParamChanges::new()),
            out_changes: ComWrapper::new(ParamChanges::new()),
            in_events: ComWrapper::new(EventList::new()),
            out_events: ComWrapper::new(EventList::new()),
            context,
            buffers,
            processing: false,
            frames: 0,
            max_block,
            has_event_input,
            midi_ids: [0; MIDI_IDS],
            midi_len: 0,
            inst,
        }
    }

    fn set_processing(&mut self, on: bool) {
        if self.processing != on {
            // SAFETY: `setProcessing` may be called on the audio thread (SDK docs).
            unsafe {
                self.inst.processor.setProcessing(u8::from(on));
            }
            self.processing = on;
        }
    }

    fn fill_context(&mut self, transport: Option<&TransportInfo>) {
        let c = &mut *self.context;
        c.sampleRate = self.inst.setup.sample_rate;
        c.continousTimeSamples = self.frames;
        let mut state = kContTimeValid;
        match transport {
            None => {
                c.projectTimeSamples = self.frames;
            }
            Some(t) => {
                // Real samples through the tempo map (ADR 0060).
                c.projectTimeSamples = (t.position_seconds * c.sampleRate).round() as i64;
                c.projectTimeMusic = t.position_beats;
                c.barPositionMusic = t.bar_start_beats;
                c.tempo = t.tempo_bpm;
                c.timeSigNumerator = i32::from(t.time_sig_num);
                c.timeSigDenominator = i32::from(t.time_sig_den);
                state |= kTempoValid | kTimeSigValid | kProjectTimeMusicValid | kBarPositionValid;
                if t.playing {
                    state |= kPlaying;
                }
                if t.recording {
                    state |= kRecording;
                }
                if let Some((start, end)) = t.loop_beats {
                    c.cycleStartMusic = start;
                    c.cycleEndMusic = end;
                    state |= kCycleValid | kCycleActive;
                }
            }
        }
        c.state = state as u32;
    }

    /// The controller's edits and the host's changes into the input queues.
    fn gather_params(&mut self, params: &[ParamChange], last: i32, out: &mut ParamOut) {
        self.in_changes.clear();
        self.out_changes.clear();
        while let Ok(edit) = self.from_ui.pop() {
            match edit {
                Edit::Begin(id) => out.push(ParamEvent::Begin(id)),
                Edit::Value(id, norm) => {
                    self.in_changes.add(id, 0, norm);
                    out.push(ParamEvent::Value(id, self.map.to_plain(id, norm)));
                }
                Edit::End(id) => out.push(ParamEvent::End(id)),
            }
        }
        for c in params {
            let norm = self.map.to_normalized(c.id, c.value);
            if !self
                .in_changes
                .add(c.id, c.offset.min(last as u32) as i32, norm)
            {
                // No queue left for it this block: counted, never silent (ADR 0057).
                out.lost_in += 1;
            }
        }
    }

    /// Notes into the input event list.
    fn gather_notes(&mut self, notes: &[NoteEvent], last: i32) {
        self.in_events.clear();
        self.out_events.clear();
        self.midi_len = 0;
        for n in notes {
            if let NoteKind::Midi {
                status,
                data1,
                data2,
            } = n.kind
            {
                // A controller travels as the parameter the controller maps it to.
                if let Some((id, norm)) = self.inst.midi_param(n.channel, status, data1, data2) {
                    self.in_changes
                        .add(id, n.offset.min(last as u32) as i32, norm);
                    let seen = self.midi_ids[..self.midi_len].contains(&id);
                    if !seen && self.midi_len < MIDI_IDS {
                        self.midi_ids[self.midi_len] = id;
                        self.midi_len += 1;
                    }
                }
                continue;
            }
            if !self.has_event_input {
                continue;
            }
            if let Some(e) = note_event(n, n.offset.min(last as u32) as i32) {
                self.in_events.push(e);
            }
        }
    }

    /// Host changes and the processor's output changes go on to the controller, so its
    /// editor follows automation. The controller's own edits are not echoed back.
    fn report(&mut self, params: &[ParamChange], out: &mut ParamOut) {
        for q in self.in_changes.queues() {
            let midi = self.midi_ids[..self.midi_len].contains(&q.id());
            if midi || params.iter().any(|c| c.id == q.id()) {
                if let Some(&(_, v)) = q.points().last() {
                    let _ = self.to_ui.push((q.id(), v));
                }
            }
        }
        for q in self.out_changes.queues() {
            for &(_, v) in q.points() {
                out.push(ParamEvent::Value(q.id(), self.map.to_plain(q.id(), v)));
            }
            if let Some(&(_, v)) = q.points().last() {
                let _ = self.to_ui.push((q.id(), v));
            }
        }
    }
}

impl AudioThread for AudioHalf {
    fn process(
        &mut self,
        audio: Audio<'_>,
        params: &[ParamChange],
        notes: &[NoteEvent],
        out: &mut ParamOut,
    ) {
        let Audio {
            left,
            right,
            key,
            transport,
        } = audio;
        let n = left.len().min(right.len()).min(self.max_block);
        if n == 0 {
            return;
        }
        let last = n as i32 - 1;
        self.inst.maps.take(&mut self.map);
        self.set_processing(true);
        self.gather_params(params, last, out);
        self.gather_notes(notes, last);
        self.fill_context(transport);

        let layout = &self.inst.layout;
        let b = &mut self.buffers;
        // Inputs: the main pair copied (the outputs are written over the host's buffers),
        // the sidechain from the key or silent, any other bus silent.
        for (i, ptrs) in b.in_ptrs.iter().enumerate() {
            let mut silent = false;
            for (c, &p) in ptrs.iter().enumerate() {
                // SAFETY: each pointer is a scratch channel of `max_block` samples.
                let ch = unsafe { std::slice::from_raw_parts_mut(p, n) };
                if Some(i) == layout.main_input {
                    if ptrs.len() == 1 {
                        for ((d, l), r) in ch.iter_mut().zip(&left[..n]).zip(&right[..n]) {
                            *d = 0.5 * (l + r);
                        }
                    } else if c == 0 {
                        ch.copy_from_slice(&left[..n]);
                    } else if c == 1 {
                        ch.copy_from_slice(&right[..n]);
                    } else {
                        ch.fill(0.0);
                    }
                } else if let (true, Some((kl, kr))) =
                    (Some(i as i32) == layout.buses.sidechain, key)
                {
                    let src = if c == 0 { kl } else { kr };
                    let m = src.len().min(n);
                    ch[..m].copy_from_slice(&src[..m]);
                    ch[m..].fill(0.0);
                } else {
                    ch.fill(0.0);
                    silent = true;
                }
            }
            b.inputs[i].silenceFlags = if silent && !ptrs.is_empty() {
                (1u64 << ptrs.len().min(63)) - 1
            } else {
                0
            };
        }
        // Outputs: the main bus writes into the host's pair.
        if let Some(main) = b.out_ptrs.first_mut() {
            if let Some(p) = main.get_mut(0) {
                *p = left.as_mut_ptr();
            }
            if let Some(p) = main.get_mut(1) {
                *p = right.as_mut_ptr();
            }
        }
        for o in b.outputs.iter_mut() {
            o.silenceFlags = 0;
        }

        let changes = |c: &ComWrapper<ParamChanges>| {
            c.as_com_ref::<IParameterChanges>()
                .map_or(std::ptr::null_mut(), |r| r.as_ptr())
        };
        let events = |e: &ComWrapper<EventList>| {
            e.as_com_ref::<IEventList>()
                .map_or(std::ptr::null_mut(), |r| r.as_ptr())
        };
        let mut data = ProcessData {
            processMode: kRealtime as i32,
            symbolicSampleSize: kSample32 as i32,
            numSamples: n as i32,
            numInputs: b.inputs.len() as i32,
            numOutputs: b.outputs.len() as i32,
            inputs: if b.inputs.is_empty() {
                std::ptr::null_mut()
            } else {
                b.inputs.as_mut_ptr()
            },
            outputs: if b.outputs.is_empty() {
                std::ptr::null_mut()
            } else {
                b.outputs.as_mut_ptr()
            },
            inputParameterChanges: changes(&self.in_changes),
            outputParameterChanges: changes(&self.out_changes),
            inputEvents: if self.has_event_input {
                events(&self.in_events)
            } else {
                std::ptr::null_mut()
            },
            outputEvents: events(&self.out_events),
            processContext: &mut *self.context,
        };
        // SAFETY: every pointer in `data` is valid for `n` frames for the whole call, and
        // this is the audio thread of an active, processing plug-in.
        unsafe {
            self.inst.processor.process(&mut data);
        }

        // Point the main bus back at scratch: the host's buffers are not ours after this.
        let first_out: usize = self.inst.layout.inputs.iter().map(|&c| c as usize).sum();
        if let Some(main) = self.buffers.out_ptrs.first_mut() {
            for (c, p) in main.iter_mut().enumerate().take(2) {
                *p = self.buffers.storage[first_out + c].as_mut_ptr();
            }
            if main.len() == 1 {
                right[..n].copy_from_slice(&left[..n]);
            }
        }
        self.report(params, out);
        self.frames += n as i64;
    }

    fn reset(&mut self) {
        // VST3 has no reset call; a processing cycle off and on is how hosts ask a plug-in
        // to clear its tails (whether it does is up to it). `process` turns it back on.
        self.set_processing(false);
    }

    fn stop(&mut self) {
        self.set_processing(false);
    }
}

impl Drop for AudioHalf {
    fn drop(&mut self) {
        // Dropped on the main thread; the plug-in must stop processing before the shared
        // instance deactivates it.
        self.set_processing(false);
    }
}

/// A contract note as a VST3 event. A note without an id gets one made of its channel and
/// key, so its expressions still find it.
fn note_event(n: &NoteEvent, offset: i32) -> Option<Event> {
    // SAFETY: `Event` is plain data; all-zero is a valid value.
    let mut e: Event = unsafe { std::mem::zeroed() };
    e.busIndex = 0;
    e.sampleOffset = offset;
    e.flags = kIsLive as u16;
    let note_id = if n.note_id >= 0 {
        n.note_id
    } else {
        (i32::from(n.channel) << 7) | i32::from(n.key)
    };
    let channel = i16::from(n.channel);
    let pitch = i16::from(n.key);
    match n.kind {
        NoteKind::On { velocity } => {
            e.r#type = kNoteOnEvent as u16;
            e.__field0.noteOn = NoteOnEvent {
                channel,
                pitch,
                tuning: 0.0,
                velocity,
                length: 0,
                noteId: note_id,
            };
        }
        NoteKind::Off { velocity } => {
            e.r#type = kNoteOffEvent as u16;
            e.__field0.noteOff = NoteOffEvent {
                channel,
                pitch,
                velocity,
                noteId: note_id,
                tuning: 0.0,
            };
        }
        // VST3 has no choke: the nearest is a note-off with no release velocity.
        NoteKind::Choke => {
            e.r#type = kNoteOffEvent as u16;
            e.__field0.noteOff = NoteOffEvent {
                channel,
                pitch,
                velocity: 0.0,
                noteId: note_id,
                tuning: 0.0,
            };
        }
        NoteKind::Expression {
            expression: NoteExpression::Pressure,
            value,
        } => {
            e.r#type = kPolyPressureEvent as u16;
            e.__field0.polyPressure = PolyPressureEvent {
                channel,
                pitch,
                pressure: value.clamp(0.0, 1.0) as f32,
                noteId: note_id,
            };
        }
        NoteKind::Expression { expression, value } => {
            // Normalized as `ivstnoteexpression.h` defines each type.
            let (type_id, norm) = match expression {
                NoteExpression::Tuning => (kTuningTypeID, 0.5 + value / 240.0),
                NoteExpression::Volume => (kVolumeTypeID, value / 4.0),
                NoteExpression::Pan => (kPanTypeID, value),
                NoteExpression::Brightness => (kBrightnessTypeID, value),
                NoteExpression::Vibrato => (kVibratoTypeID, value),
                NoteExpression::Expression => (kExpressionTypeID, value),
                NoteExpression::Pressure => return None,
            };
            e.r#type = kNoteExpressionValueEvent as u16;
            e.__field0.noteExpressionValue = NoteExpressionValueEvent {
                typeId: type_id,
                noteId: note_id,
                value: norm.clamp(0.0, 1.0),
            };
        }
        // Parameters, not events (`AudioHalf::gather_notes`).
        NoteKind::Midi { .. } => return None,
    }
    Some(e)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(kind: NoteKind) -> NoteEvent {
        NoteEvent {
            offset: 3,
            note_id: -1,
            channel: 1,
            key: 60,
            kind,
        }
    }

    #[test]
    fn notes_map_to_vst3_events() {
        let on = note_event(&note(NoteKind::On { velocity: 0.5 }), 3).unwrap();
        assert_eq!(on.r#type, kNoteOnEvent as u16);
        assert_eq!(on.sampleOffset, 3);
        // SAFETY: the variant the type says.
        let n = unsafe { on.__field0.noteOn };
        assert_eq!((n.channel, n.pitch, n.velocity), (1, 60, 0.5));
        assert_eq!(n.noteId, (1 << 7) | 60, "made from channel and key");

        let bend = note_event(
            &note(NoteKind::Expression {
                expression: NoteExpression::Tuning,
                value: 12.0,
            }),
            0,
        )
        .unwrap();
        // SAFETY: as above.
        let x = unsafe { bend.__field0.noteExpressionValue };
        assert_eq!(x.typeId, kTuningTypeID);
        assert_eq!(x.value, 0.5 + 12.0 / 240.0);
        assert_eq!(x.noteId, n.noteId, "the expression finds its note");

        let volume = note_event(
            &note(NoteKind::Expression {
                expression: NoteExpression::Volume,
                value: 1.0,
            }),
            0,
        )
        .unwrap();
        // SAFETY: as above.
        let v = unsafe { volume.__field0.noteExpressionValue };
        assert_eq!(v.value, 0.25, "0 dB");
    }
}
