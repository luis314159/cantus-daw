//! A hosted CLAP/VST3 instrument behind the engine's own instrument contract,
//! `cantus_engine::InstrumentUnit` (ADR 0028), so a track plays it exactly as it plays
//! Unda or Calor: notes, controllers, the song's position, latency into delay
//! compensation, its window's gestures out as commands.
//!
//! **Timing.** The processor splits each block at note times and calls `process` once per
//! segment, so a note given to the unit belongs at the first sample of the next call: it is
//! queued and sent at offset 0, which is sample-accurate whatever the block size.
//!
//! **Notes.** Every note carries the engine's id, folded to the 31 bits both standards
//! keep, so a note-off or an expression names one sounding note even when two share a key.
//! The unit remembers what sounds, so a stop, a seek or a loop wrap releases exactly those
//! notes (and the sustain pedal) — a plug-in never hangs a note because the song jumped.
//!
//! Everything here runs on the audio thread except [`HostedInstrument::new`]: fixed
//! capacities, no allocation, no locks (ADR 0005).

use crate::backend::{
    Audio, AudioThread, NoteEvent, NoteExpression as HostExpression, NoteKind, ParamInfo,
    TransportInfo,
};
use crate::unit::{transport_of, ParamSync};
use cantus_core::{ExternalPlugin, NativeSpec};
use cantus_engine::fx::{FxParamEvent, SongPosition};
use cantus_engine::inst::{Assets, Controller, InstrumentUnit, NoteExpression, NoteOn};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

/// Events one call can carry; more in one instant are dropped and counted.
pub const NOTE_CAPACITY: usize = 512;
/// Notes the unit remembers as sounding, to release them on a stop.
pub const HELD_CAPACITY: usize = 256;

/// A sounding note: its id as the plug-in knows it, its channel and key.
#[derive(Copy, Clone, Debug, PartialEq)]
struct Held {
    id: i32,
    channel: u8,
    key: u8,
}

pub struct HostedInstrument {
    audio: Box<dyn AudioThread>,
    sync: ParamSync,
    latency: usize,
    transport: Option<TransportInfo>,
    /// Events for the next `process`, all at offset 0.
    notes: Vec<NoteEvent>,
    held: Vec<Held>,
    /// Events that did not fit, since the unit was made.
    pub dropped: usize,
}

impl std::fmt::Debug for HostedInstrument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostedInstrument")
            .field("params", &self.sync.len())
            .field("latency", &self.latency)
            .field("held", &self.held.len())
            .finish()
    }
}

/// The engine's note id as both standards carry it: 31 bits, never −1 (which means "no id").
pub fn plugin_note_id(id: u64) -> i32 {
    (id & 0x7FFF_FFFF) as i32
}

/// A 0…1 value as a 7-bit MIDI data byte.
fn seven_bits(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 127.0).round() as u8
}

impl HostedInstrument {
    /// Built on the main thread, from the plug-in's audio half, its parameters, its current
    /// values (read after its state was loaded) and the latency it reports now.
    pub fn new(
        audio: Box<dyn AudioThread>,
        params: &[ParamInfo],
        values: impl Fn(u32) -> Option<f64>,
        latency: usize,
        sample_rate: f64,
        generation: Arc<AtomicU64>,
    ) -> Self {
        HostedInstrument {
            audio,
            sync: ParamSync::new(params, values, sample_rate, generation),
            latency,
            transport: None,
            notes: Vec::with_capacity(NOTE_CAPACITY),
            held: Vec::with_capacity(HELD_CAPACITY),
            dropped: 0,
        }
    }

    fn push(&mut self, channel: u8, key: u8, note_id: i32, kind: NoteKind) {
        if self.notes.len() < self.notes.capacity() {
            self.notes.push(NoteEvent {
                offset: 0,
                note_id,
                channel: channel & 15,
                key: key & 127,
                kind,
            });
        } else {
            self.dropped += 1;
        }
    }

    fn midi(&mut self, status: u8, data1: u8, data2: u8) {
        self.push(
            0,
            0,
            -1,
            NoteKind::Midi {
                status,
                data1,
                data2,
            },
        );
    }

    fn release(&mut self, key: u8, id: u64, kind: NoteKind) {
        let note_id = plugin_note_id(id);
        let at = self.held.iter().position(|h| h.id == note_id);
        let channel = at.map_or(0, |i| self.held[i].channel);
        if let Some(i) = at {
            self.held.swap_remove(i);
        }
        self.push(channel, key, note_id, kind);
    }
}

impl InstrumentUnit for HostedInstrument {
    /// A native spec never reaches a hosted unit; its values come by `configure_hosted`.
    fn configure(&mut self, _spec: &NativeSpec, _assets: &Assets) {}

    fn configure_hosted(&mut self, plugin: &ExternalPlugin) {
        self.sync.configure(&plugin.params);
    }

    fn note_on(&mut self, note: NoteOn) {
        let id = plugin_note_id(note.id);
        if self.held.len() < self.held.capacity() {
            self.held.push(Held {
                id,
                channel: note.channel & 15,
                key: note.key,
            });
        }
        self.push(
            note.channel,
            note.key,
            id,
            NoteKind::On {
                velocity: note.velocity.clamp(0.0, 1.0),
            },
        );
    }

    fn note_off(&mut self, key: u8, id: u64, velocity: f32) {
        self.release(
            key,
            id,
            NoteKind::Off {
                velocity: velocity.clamp(0.0, 1.0),
            },
        );
    }

    fn choke(&mut self, key: u8, id: u64) {
        self.release(key, id, NoteKind::Choke);
    }

    fn expression(&mut self, id: u64, expression: NoteExpression, value: f32) {
        let note_id = plugin_note_id(id);
        let Some(h) = self.held.iter().find(|h| h.id == note_id).copied() else {
            return;
        };
        let (expression, value) = match expression {
            NoteExpression::Tuning => (HostExpression::Tuning, f64::from(value)),
            NoteExpression::Pressure => (HostExpression::Pressure, f64::from(value)),
            NoteExpression::Brightness => (HostExpression::Brightness, f64::from(value)),
            // The engine's pan is −1…1; both standards' is 0…1.
            NoteExpression::Pan => (HostExpression::Pan, f64::from(value * 0.5 + 0.5)),
            NoteExpression::Volume => (HostExpression::Volume, f64::from(value)),
            NoteExpression::Vibrato => (HostExpression::Vibrato, f64::from(value)),
            NoteExpression::Expression => (HostExpression::Expression, f64::from(value)),
        };
        self.push(
            h.channel,
            h.key,
            note_id,
            NoteKind::Expression { expression, value },
        );
    }

    fn controller(&mut self, controller: Controller, value: f32) {
        match controller {
            Controller::PitchBend => {
                // The processor scales plain MIDI to ±2 semitones; undo that into 14 bits.
                let raw = ((value / 2.0).clamp(-1.0, 1.0) * 8192.0 + 8192.0)
                    .round()
                    .clamp(0.0, 16383.0) as u16;
                self.midi(0xE0, (raw & 127) as u8, (raw >> 7) as u8);
            }
            Controller::Pressure => self.midi(0xD0, seven_bits(value), 0),
            Controller::ModWheel => self.midi(0xB0, 1, seven_bits(value)),
            Controller::Sustain => self.midi(0xB0, 64, seven_bits(value)),
            Controller::Expression => self.midi(0xB0, 11, seven_bits(value)),
            Controller::Cc(n) => self.midi(0xB0, n & 127, seven_bits(value)),
        }
    }

    fn all_notes_off(&mut self) {
        while let Some(h) = self.held.pop() {
            self.push(h.channel, h.key, h.id, NoteKind::Off { velocity: 0.0 });
        }
        // A pedal held when the song stopped would keep every released note sounding.
        self.midi(0xB0, 64, 0);
    }

    fn reset(&mut self) {
        self.notes.clear();
        self.held.clear();
        self.audio.reset();
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        let frames = left.len().min(right.len());
        if frames == 0 {
            return false;
        }
        // The caller zeroed both: the plug-in's output written over them is its output
        // added, as the contract asks.
        self.audio.process(
            Audio {
                left: &mut left[..frames],
                right: &mut right[..frames],
                key: None,
                transport: self.transport.as_ref(),
            },
            &self.sync.pending,
            &self.notes,
            &mut self.sync.out,
        );
        self.notes.clear();
        self.sync.after_process(frames);
        left[..frames].iter().any(|x| *x != 0.0) || right[..frames].iter().any(|x| *x != 0.0)
    }

    fn active_voices(&self) -> usize {
        self.held.len()
    }

    fn latency(&self) -> usize {
        self.latency
    }

    fn set_position(&mut self, position: &SongPosition) {
        self.transport = Some(transport_of(position));
    }

    fn param_events(&self) -> &[FxParamEvent] {
        &self.sync.events
    }

    fn clear_param_events(&mut self) {
        self.sync.events.clear();
    }

    fn stop(&mut self) {
        self.audio.stop();
    }
}

/// What stands in for a hosted instrument while it restarts (its latency changed): it plays
/// nothing, so the plug-in's own audio half can come back to be deactivated.
#[derive(Debug, Default)]
pub struct Silent;

impl InstrumentUnit for Silent {
    fn configure(&mut self, _spec: &NativeSpec, _assets: &Assets) {}
    fn note_on(&mut self, _note: NoteOn) {}
    fn note_off(&mut self, _key: u8, _id: u64, _velocity: f32) {}
    fn all_notes_off(&mut self) {}
    fn reset(&mut self) {}
    fn process(&mut self, _left: &mut [f32], _right: &mut [f32]) -> bool {
        false
    }
    fn active_voices(&self) -> usize {
        0
    }
}
