//! A track's **per-note expression** into its instrument (ADR 0080): the snapshot's stream
//! of [`ExprEvent`]s (`TrackSnapshot::expr`), each addressed to one sounding note by its
//! voice tag, fired at its sample.
//!
//! **Two ways in, the same samples.** A native unit takes each value between two
//! `process` calls, the track splitting its pass at the value's sample as it does for a
//! note. A unit that says it takes offsets ([`InstrumentUnit::expression_offsets`], a
//! hosted CLAP or VST3 instrument) is handed every value of the segment ahead with its
//! sample offset, so a plug-in's block is not cut every few milliseconds by a moving bend
//! — the format's own event lists carry the offset to the plug-in.
//!
//! **At one sample**: notes, then controllers, then expression — a note's first value
//! reaches the voice its note-on just made, before a sample of it is rendered.
//!
//! A seek sets the cursor ([`first_at_or_after`]); nothing is chased: a note started before
//! the playhead is not sounding, and a note sounding has had its values. A track with no
//! expression runs none of this.
//!
//! [`InstrumentUnit::expression_offsets`]: super::InstrumentUnit::expression_offsets

use super::NoteExpression;
use crate::processor::track::TrackSlot;
use crate::processor::SlotInstrument;
use crate::transport::Transport;
use cantus_core::model::note_expr::compile::ExprEvent;
use cantus_core::model::note_expr::ExprKind;
use cantus_core::{Ticks, TrackSnapshot};

/// The instrument contract's expression for a curve's kind.
#[inline]
pub fn expression_of(kind: ExprKind) -> NoteExpression {
    match kind {
        ExprKind::Bend => NoteExpression::Tuning,
        ExprKind::Pressure => NoteExpression::Pressure,
        ExprKind::Timbre => NoteExpression::Brightness,
    }
}

/// Index of the first event at or after `tick` (binary search).
#[inline]
pub fn first_at_or_after(events: &[ExprEvent], tick: Ticks) -> usize {
    events.partition_point(|e| e.tick < tick)
}

/// Fire the expression due in the track's segment `[sub, end)` of the pass and return the
/// segment's (possibly earlier) end: for a unit that splits, the next value's sample; for
/// one that takes offsets, `end` itself, every value before it already handed over.
#[inline]
pub(crate) fn due(
    slot: &mut TrackSlot,
    track: &TrackSnapshot,
    transport: &Transport,
    sub: usize,
    end: usize,
) -> usize {
    let offsets = takes_offsets(slot);
    let plays = slot.frozen.is_none() && !slot.notes_only && !slot.rest;
    while let Some(ev) = track.expr.get(slot.next_expr) {
        // Counted from the transport's integer sample, as the notes are.
        let off = (transport.samples_until(ev.tick.0 as f64) as usize).saturating_sub(sub);
        let now = off == 0 || (offsets && sub + off < end);
        if !now {
            return if offsets { end } else { end.min(sub + off) };
        }
        if plays {
            fire(slot, ev, off as u32);
        }
        slot.next_expr += 1;
    }
    end
}

/// Whether the slot's unit takes values ahead with their offsets.
#[inline]
fn takes_offsets(slot: &TrackSlot) -> bool {
    matches!(slot.instrument, SlotInstrument::Native)
        && slot
            .native
            .as_ref()
            .is_some_and(|n| n.unit.expression_offsets())
}

/// One value to the note it names. Firmus and Pulsus read none.
#[inline]
fn fire(slot: &mut TrackSlot, ev: &ExprEvent, offset: u32) {
    if let (SlotInstrument::Native, Some(n)) = (slot.instrument, slot.native.as_mut()) {
        n.unit
            .expression_at(ev.note.0, expression_of(ev.kind), ev.value, offset);
    }
}
