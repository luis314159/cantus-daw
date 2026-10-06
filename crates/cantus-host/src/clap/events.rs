//! CLAP event lists: what the host sends into a `process` (or `flush`) call, and what the
//! plug-in sends back. Both are fixed-capacity and filled without allocating, so they work
//! on the audio thread (ADR 0005).

use super::instance::NoteDialects;
use crate::backend::{NoteEvent, NoteExpression, NoteKind, ParamChange, ParamEvent, ParamOut};
use clap_sys::events::{
    clap_event_header, clap_event_midi, clap_event_note, clap_event_note_expression,
    clap_event_param_gesture, clap_event_param_value, clap_input_events, clap_output_events,
    CLAP_CORE_EVENT_SPACE_ID, CLAP_EVENT_MIDI, CLAP_EVENT_NOTE_CHOKE, CLAP_EVENT_NOTE_EXPRESSION,
    CLAP_EVENT_NOTE_OFF, CLAP_EVENT_NOTE_ON, CLAP_EVENT_PARAM_GESTURE_BEGIN,
    CLAP_EVENT_PARAM_GESTURE_END, CLAP_EVENT_PARAM_VALUE, CLAP_NOTE_EXPRESSION_BRIGHTNESS,
    CLAP_NOTE_EXPRESSION_EXPRESSION, CLAP_NOTE_EXPRESSION_PAN, CLAP_NOTE_EXPRESSION_PRESSURE,
    CLAP_NOTE_EXPRESSION_TUNING, CLAP_NOTE_EXPRESSION_VIBRATO, CLAP_NOTE_EXPRESSION_VOLUME,
};
use std::ffi::c_void;

/// One input event of any kind the host sends, in the space the largest needs.
#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) union InEvent {
    header: clap_event_header,
    param: clap_event_param_value,
    note: clap_event_note,
    expression: clap_event_note_expression,
    midi: clap_event_midi,
}

/// The events of one call, in time order.
pub(crate) struct InEvents {
    events: Vec<InEvent>,
    len: usize,
    list: clap_input_events,
}

// `list.ctx` points at the struct itself (boxed, never moved), and the events hold no
// pointers the host keeps: the list moves between threads with its owner.
unsafe impl Send for InEvents {}

/// How many events one call can carry; the rest of a block's events are dropped (and the
/// host sends at most one per parameter and a few notes, far below this).
pub(crate) const IN_CAPACITY: usize = 1024;

fn header(size: usize, time: u32, type_: u16) -> clap_event_header {
    clap_event_header {
        size: size as u32,
        time,
        space_id: CLAP_CORE_EVENT_SPACE_ID,
        type_,
        flags: 0,
    }
}

impl InEvents {
    pub fn new() -> Box<InEvents> {
        // SAFETY: an all-zero header is a valid (if meaningless) event; it is never read
        // before being written.
        let blank: InEvent = unsafe { std::mem::zeroed() };
        let mut list = Box::new(InEvents {
            events: vec![blank; IN_CAPACITY],
            len: 0,
            list: clap_input_events {
                ctx: std::ptr::null_mut(),
                size: Some(in_size),
                get: Some(in_get),
            },
        });
        let ptr: *mut InEvents = &mut *list;
        list.list.ctx = ptr.cast();
        list
    }

    pub fn raw(&self) -> *const clap_input_events {
        &self.list
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    fn push(&mut self, e: InEvent) {
        if self.len < self.events.len() {
            self.events[self.len] = e;
            self.len += 1;
        }
    }

    /// Fill with `params` and `notes`, each sorted by offset, merged into one time order.
    /// Offsets at or past `frames` are moved to the last frame.
    pub fn fill(
        &mut self,
        params: &[ParamChange],
        notes: &[NoteEvent],
        frames: u32,
        dialects: NoteDialects,
    ) {
        self.clear();
        let last = frames.saturating_sub(1);
        let (mut i, mut j) = (0, 0);
        while i < params.len() || j < notes.len() {
            let take_param =
                j >= notes.len() || (i < params.len() && params[i].offset <= notes[j].offset);
            if take_param {
                let p = params[i];
                i += 1;
                self.push(InEvent {
                    param: clap_event_param_value {
                        header: header(
                            size_of::<clap_event_param_value>(),
                            p.offset.min(last),
                            CLAP_EVENT_PARAM_VALUE,
                        ),
                        param_id: p.id,
                        cookie: std::ptr::null_mut(),
                        note_id: -1,
                        port_index: -1,
                        channel: -1,
                        key: -1,
                        value: p.value,
                    },
                });
            } else {
                let n = notes[j];
                j += 1;
                if let Some(e) = note_event(&n, n.offset.min(last), dialects) {
                    self.push(e);
                }
            }
        }
    }
}

fn midi(time: u32, status: u8, data1: u8, data2: u8) -> InEvent {
    InEvent {
        midi: clap_event_midi {
            header: header(size_of::<clap_event_midi>(), time, CLAP_EVENT_MIDI),
            port_index: 0,
            data: [status, data1 & 127, data2 & 127],
        },
    }
}

/// A contract note as a CLAP event in the dialect the plug-in's note port speaks; `None`
/// for what that dialect cannot say.
fn note_event(n: &NoteEvent, time: u32, dialects: NoteDialects) -> Option<InEvent> {
    let channel = n.channel & 15;
    if let NoteKind::Midi {
        status,
        data1,
        data2,
    } = n.kind
    {
        let status = (status & 0xF0) | channel;
        if dialects.midi {
            return Some(midi(time, status, data1, data2));
        }
        // No MIDI: a pitch bend can still travel as tuning on every note of the channel.
        if status & 0xF0 == 0xE0 {
            let raw = (i32::from(data2 & 127) << 7 | i32::from(data1 & 127)) - 8192;
            return Some(InEvent {
                expression: clap_event_note_expression {
                    header: header(
                        size_of::<clap_event_note_expression>(),
                        time,
                        CLAP_EVENT_NOTE_EXPRESSION,
                    ),
                    expression_id: CLAP_NOTE_EXPRESSION_TUNING,
                    note_id: -1,
                    port_index: 0,
                    channel: i16::from(channel),
                    key: -1,
                    value: f64::from(raw) / 8192.0 * 2.0,
                },
            });
        }
        return None;
    }
    if dialects.notes_as_midi() {
        let velocity = |v: f32| (v * 127.0).round().clamp(0.0, 127.0) as u8;
        return match n.kind {
            NoteKind::On { velocity: v } => {
                Some(midi(time, 0x90 | channel, n.key, velocity(v).max(1)))
            }
            NoteKind::Off { velocity: v } => Some(midi(time, 0x80 | channel, n.key, velocity(v))),
            NoteKind::Choke => Some(midi(time, 0x80 | channel, n.key, 0)),
            NoteKind::Expression {
                expression: NoteExpression::Pressure,
                value,
            } => Some(midi(
                time,
                0xA0 | channel,
                n.key,
                velocity(value.clamp(0.0, 1.0) as f32),
            )),
            _ => None,
        };
    }
    note_event_clap(n, time)
}

fn note_event_clap(n: &NoteEvent, time: u32) -> Option<InEvent> {
    let note = |type_, velocity: f64| InEvent {
        note: clap_event_note {
            header: header(size_of::<clap_event_note>(), time, type_),
            note_id: n.note_id,
            port_index: 0,
            channel: i16::from(n.channel),
            key: i16::from(n.key),
            velocity,
        },
    };
    Some(match n.kind {
        NoteKind::On { velocity } => note(CLAP_EVENT_NOTE_ON, f64::from(velocity)),
        NoteKind::Off { velocity } => note(CLAP_EVENT_NOTE_OFF, f64::from(velocity)),
        NoteKind::Choke => note(CLAP_EVENT_NOTE_CHOKE, 0.0),
        NoteKind::Expression { expression, value } => InEvent {
            expression: clap_event_note_expression {
                header: header(
                    size_of::<clap_event_note_expression>(),
                    time,
                    CLAP_EVENT_NOTE_EXPRESSION,
                ),
                expression_id: match expression {
                    NoteExpression::Tuning => CLAP_NOTE_EXPRESSION_TUNING,
                    NoteExpression::Pressure => CLAP_NOTE_EXPRESSION_PRESSURE,
                    NoteExpression::Brightness => CLAP_NOTE_EXPRESSION_BRIGHTNESS,
                    NoteExpression::Pan => CLAP_NOTE_EXPRESSION_PAN,
                    NoteExpression::Volume => CLAP_NOTE_EXPRESSION_VOLUME,
                    NoteExpression::Vibrato => CLAP_NOTE_EXPRESSION_VIBRATO,
                    NoteExpression::Expression => CLAP_NOTE_EXPRESSION_EXPRESSION,
                },
                note_id: n.note_id,
                port_index: 0,
                channel: i16::from(n.channel),
                key: i16::from(n.key),
                value,
            },
        },
        NoteKind::Midi { .. } => return None,
    })
}

unsafe extern "C" fn in_size(list: *const clap_input_events) -> u32 {
    let events = &*((*list).ctx as *const InEvents);
    events.len as u32
}

unsafe extern "C" fn in_get(
    list: *const clap_input_events,
    index: u32,
) -> *const clap_event_header {
    let events = &*((*list).ctx as *const InEvents);
    match events.events.get(index as usize) {
        Some(e) if (index as usize) < events.len => &e.header,
        _ => std::ptr::null(),
    }
}

/// The output list of one call: parameter gestures and values go into the [`ParamOut`]
/// given to [`OutEvents::bind`]; everything else the plug-in says is ignored.
pub(crate) struct OutEvents {
    list: clap_output_events,
    target: *mut ParamOut,
}

// `target` is set only for the length of one call, on the calling thread; `list.ctx`
// points at the struct itself.
unsafe impl Send for OutEvents {}

impl OutEvents {
    pub fn new() -> Box<OutEvents> {
        let mut list = Box::new(OutEvents {
            list: clap_output_events {
                ctx: std::ptr::null_mut(),
                try_push: Some(out_try_push),
            },
            target: std::ptr::null_mut(),
        });
        let ptr: *mut OutEvents = &mut *list;
        list.list.ctx = ptr.cast::<c_void>();
        list
    }

    /// Point the list at `out` for one call; [`Self::unbind`] after it.
    pub fn bind(&mut self, out: &mut ParamOut) {
        self.target = out;
    }

    pub fn unbind(&mut self) {
        self.target = std::ptr::null_mut();
    }

    pub fn raw(&self) -> *const clap_output_events {
        &self.list
    }
}

unsafe extern "C" fn out_try_push(
    list: *const clap_output_events,
    event: *const clap_event_header,
) -> bool {
    if list.is_null() || event.is_null() {
        return false;
    }
    let me = &*((*list).ctx as *const OutEvents);
    let Some(out) = me.target.as_mut() else {
        return false;
    };
    let h = &*event;
    if h.space_id != CLAP_CORE_EVENT_SPACE_ID {
        return true; // not ours to understand; accepted and ignored
    }
    match h.type_ {
        CLAP_EVENT_PARAM_VALUE if h.size as usize >= size_of::<clap_event_param_value>() => {
            let e = &*(event as *const clap_event_param_value);
            out.push(ParamEvent::Value(e.param_id, e.value));
        }
        CLAP_EVENT_PARAM_GESTURE_BEGIN
            if h.size as usize >= size_of::<clap_event_param_gesture>() =>
        {
            let e = &*(event as *const clap_event_param_gesture);
            out.push(ParamEvent::Begin(e.param_id));
        }
        CLAP_EVENT_PARAM_GESTURE_END
            if h.size as usize >= size_of::<clap_event_param_gesture>() =>
        {
            let e = &*(event as *const clap_event_param_gesture);
            out.push(ParamEvent::End(e.param_id));
        }
        _ => {}
    }
    true
}
