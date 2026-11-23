//! r42:tuning — what every pitched unit shares of the project's tuning (ADR 0155): a table
//! of 128 offsets in semitones from 12-TET at A440, one per MIDI key (`cantus-core`'s
//! `TuningTable` holds the same array). A unit adds `table[key]` where its key first
//! becomes a pitch; at the default every entry is `0.0` and `x + 0.0 == x` keeps every bit.

/// A key's offset in semitones (`f32::NAN`: a key a keyboard map leaves unmapped).
pub type Table = [f32; 128];

/// 12-TET at A440.
pub const EQUAL: Table = [0.0; 128];

/// Whether two tables are the same to the bit (`NAN` equal to itself, `-0.0` not `0.0`).
#[inline]
pub fn same(a: &Table, b: &Table) -> bool {
    a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

/// Whether the table is 12-TET at A440 to the bit.
#[inline]
pub fn is_equal(t: &Table) -> bool {
    t.iter().all(|v| v.to_bits() == 0)
}

/// `key`'s offset, `0.0` for an unmapped key (the unit skips its note before it asks).
#[inline]
pub fn at(t: &Table, key: u8) -> f32 {
    let v = t[usize::from(key & 0x7F)];
    if v.is_nan() {
        0.0
    } else {
        v
    }
}
