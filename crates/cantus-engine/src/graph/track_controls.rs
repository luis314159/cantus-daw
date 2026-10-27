//! A track's **controller lanes** into its instrument (ADR 0068): the snapshot's stream of
//! [`CtlEvent`]s (`TrackSnapshot::controls`), fired at their samples exactly as notes are —
//! the track splits its pass at each — through the instrument contract's `controller`
//! (natives) and, for a hosted instrument, its event list (`HostedInstrument::controller`
//! queues them at the segment's first sample).
//!
//! **At one sample**: note-offs, then controllers, then note-ons — a bend set where a
//! clip begins is in place for its first note, and a note let go where its clip ends is
//! released before the lane rests.
//!
//! **Chase.** Wherever the playhead lands (play, a seek, a loop's wrap) every kind the
//! track's lanes drive is sent the value its stream holds there — the last event before,
//! or its rest — so a render from any start point hears what playing through would have.
//! A stop puts every kind back to rest, so the keyboard is not left bent.
//!
//! Nothing here allocates; a track with no lanes runs none of it.

use super::track::TrackSlot;
use crate::inst::Controller;
use cantus_core::clip_lanes::{CtlEvent, LaneKind};
use cantus_core::TrackSnapshot;

/// The instrument contract's controller and value for a lane's position: the bend in
/// semitones scaled to ±2 (as plain MIDI is, `processor::controller_of`), the rest 0…1.
pub(crate) fn controller_of(kind: LaneKind, pos: f32) -> (Controller, f32) {
    let v = pos.clamp(0.0, 1.0);
    match kind {
        LaneKind::PitchBend => (Controller::PitchBend, (v - 0.5) * 4.0),
        LaneKind::Pressure => (Controller::Pressure, v),
        LaneKind::Cc(1) => (Controller::ModWheel, v),
        LaneKind::Cc(11) => (Controller::Expression, v),
        LaneKind::Cc(64) => (Controller::Sustain, v),
        LaneKind::Cc(n) => (Controller::Cc(n), v),
    }
}

/// Send one event to the slot's instrument.
#[inline]
pub(crate) fn fire_control(slot: &mut TrackSlot, ev: &CtlEvent) {
    let (c, v) = controller_of(ev.kind, ev.value);
    slot.controller(c, v);
}

/// Whether the slot plays its instrument (not frozen, not resting but for its notes).
#[inline]
fn plays(slot: &TrackSlot) -> bool {
    slot.frozen.is_none() && !slot.notes_only && !slot.rest
}

/// Fire every control event due at sample `sub` of the pass (`off` gives an event's
/// distance from the pass's sample `sub`, 0 when due); return how far the next is, if
/// it is due before `end - sub`.
#[inline]
pub(crate) fn due(
    slot: &mut TrackSlot,
    track: &TrackSnapshot,
    off: &dyn Fn(&CtlEvent) -> usize,
) -> Option<usize> {
    while let Some(ev) = track.controls.get(slot.next_control) {
        let d = off(ev);
        if d > 0 {
            return Some(d);
        }
        if plays(slot) {
            fire_control(slot, ev);
        }
        slot.next_control += 1;
    }
    None
}

/// A kind's index in a fixed table (bend, pressure, CCs 0…119).
#[inline]
fn index(kind: LaneKind) -> usize {
    match kind {
        LaneKind::PitchBend => 0,
        LaneKind::Pressure => 1,
        LaneKind::Cc(n) => 2 + (n as usize).min(cantus_core::clip_lanes::MAX_CC as usize),
    }
}

/// The playhead landed where `slot.next_control` points: every kind gets the value its
/// stream holds there (the last event before it, or its rest).
pub(crate) fn chase(slot: &mut TrackSlot, track: &TrackSnapshot) {
    let kinds = &track.control_kinds;
    if kinds.is_empty() || !plays(slot) {
        return;
    }
    let mut seen = [false; cantus_core::clip_lanes::MAX_LANES];
    let mut left = kinds.len();
    let mut i = slot.next_control.min(track.controls.len());
    while i > 0 && left > 0 {
        i -= 1;
        let ev = track.controls[i];
        let k = index(ev.kind);
        if !seen[k] {
            seen[k] = true;
            left -= 1;
            fire_control(slot, &ev);
        }
    }
    for &kind in kinds {
        if !seen[index(kind)] {
            let (c, v) = controller_of(kind, kind.rest());
            slot.controller(c, v);
        }
    }
}

/// Every kind the track's lanes drive back to its rest (a stop).
pub(crate) fn rest(slot: &mut TrackSlot, track: &TrackSnapshot) {
    if !plays(slot) {
        return;
    }
    for &kind in &track.control_kinds {
        let (c, v) = controller_of(kind, kind.rest());
        slot.controller(c, v);
    }
}

/// A live MIDI message as a lane's kind and position, when it is one a lane records:
/// pitch bend, channel pressure, a CC 0…119 (the channel mode messages are not).
pub(crate) fn lane_of(ev: &crate::messages::MidiEvent) -> Option<(LaneKind, f32)> {
    match ev.status & 0xF0 {
        0xE0 => {
            let raw = (u16::from(ev.data2 & 0x7F) << 7) | u16::from(ev.data1 & 0x7F);
            Some((LaneKind::PitchBend, LaneKind::PitchBend.pos_of_midi(raw)))
        }
        0xD0 => Some((
            LaneKind::Pressure,
            LaneKind::Pressure.pos_of_midi(u16::from(ev.data1 & 0x7F)),
        )),
        0xB0 if ev.data1 <= cantus_core::clip_lanes::MAX_CC => {
            let k = LaneKind::Cc(ev.data1);
            Some((k, k.pos_of_midi(u16::from(ev.data2 & 0x7F))))
        }
        _ => None,
    }
}
