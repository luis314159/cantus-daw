//! The chain's times against the transport: where a pass is, whether a scheduled time is
//! due, at which sample of the pass it falls — the same integer arithmetic clip events use
//! (`Transport::samples_until`), so a step lands on the same sample at every block size.

use super::{Due, Msg, Now};
use crate::transport::Transport;
use cantus_core::NoteEvent;

/// The chain's time at offset `sub` of a pass that starts at engine sample `base`, with
/// `t` the transport at the pass's first sample.
#[inline]
pub(crate) fn now_at(t: &Transport, base: u64, sub: usize) -> Now {
    if t.is_playing() {
        Now {
            rolling: true,
            pos: t.position_at(t.samples() + sub as u64),
            tps: t.ticks_per_sample(),
        }
    } else {
        Now {
            rolling: false,
            pos: (base + sub as u64) as f64,
            tps: t.ticks_per_sample(),
        }
    }
}

/// A clip event's time: its own tick (not the sample's), so a chord on the grid starts its
/// arpeggio exactly on the grid.
#[inline]
pub(crate) fn at_event(now: Now, ev: &NoteEvent) -> Now {
    Now {
        pos: ev.tick.0 as f64,
        ..now
    }
}

/// The live input's time at the start of a block (engine sample `clock`).
#[inline]
pub(crate) fn now_live(t: &Transport, clock: u64) -> Now {
    now_at(t, clock, 0)
}

/// Whether `d` is due by offset `sub` of the pass.
#[inline]
pub(crate) fn is_due(t: &Transport, base: u64, sub: usize, d: Due) -> bool {
    if d.rolling != t.is_playing() {
        // Scheduled in the other domain: the chain moves it over before it fires.
        return true;
    }
    if d.rolling {
        t.samples_until(d.pos) as usize <= sub
    } else {
        d.pos <= (base + sub as u64) as f64
    }
}

/// The offset into the pass at which `d` falls (`None`: in the other domain).
#[inline]
pub(crate) fn offset_of(t: &Transport, base: u64, d: Due) -> Option<usize> {
    if d.rolling != t.is_playing() {
        return None;
    }
    Some(if d.rolling {
        t.samples_until(d.pos) as usize
    } else {
        (d.pos.ceil().max(0.0) as u64).saturating_sub(base) as usize
    })
}

/// A clip event as the chain takes it.
#[inline]
pub(crate) fn msg_of(ev: &NoteEvent) -> Msg {
    Msg {
        on: ev.is_on(),
        pitch: ev.pitch,
        velocity: ev.velocity,
        release: ev.release,
        tag: ev.note.0,
        ordinal: ev.ordinal,
        live: false,
    }
}

/// A note out of the chain as an insert hears it (ADR 0022).
#[inline]
pub(crate) fn fx_note(m: &Msg) -> crate::fx::FxNote {
    if m.on {
        crate::fx::FxNote::On {
            pitch: m.pitch,
            velocity: m.velocity,
        }
    } else {
        crate::fx::FxNote::Off { pitch: m.pitch }
    }
}
