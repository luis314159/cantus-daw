//! What Culmen tells its editor: 64 floats, refreshed after every block.
//!
//! | Slot | What |
//! |---|---|
//! | 0 | columns published since the stream began (wraps at [`WRAP`]) |
//! | 1 | latency, samples |
//! | 2 | sample rate |
//! | 3 | the Match monitor's gain, dB (0 when off) |
//! | 4 | the clipper's largest reduction in the last column, dB |
//! | 5–8 | each band's detector level now — low, mid, high, full — dB |
//! | 9 | the brickwall's reduction now, dB (the deeper channel) |
//! | 10–63 | a ring of the last [`COLUMNS`] columns of 10 ms, [`COLUMN`] floats each ([`Column`]) |
//!
//! A column is counted from the start of the stream. The ring holds 60 ms: the editor
//! loses nothing unless a frame comes later than that, and it tells a gap from the counter.

/// Floats in the block.
pub const LEN: usize = 64;
/// Header fields before the ring.
pub const HEADER: usize = 10;
/// Floats per column.
pub const COLUMN: usize = 9;
/// Columns in the ring.
pub const COLUMNS: usize = (LEN - HEADER) / COLUMN;
/// The column counter wraps here: a multiple of the ring, exact in f32.
pub const WRAP: u32 = (16_777_216 / COLUMNS as u32) * COLUMNS as u32;

pub const SEQ: usize = 0;
pub const LATENCY: usize = 1;
pub const RATE: usize = 2;
pub const MATCH: usize = 3;
pub const CLIP: usize = 4;
/// Four levels from here: low, mid, high, full.
pub const LEVELS: usize = 5;
pub const GR_NOW: usize = 9;

/// One 10 ms column of what Culmen did.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Column {
    /// K-weighted energy of the output (channels summed), for loudness.
    pub energy_out: f32,
    /// K-weighted energy of the input, before anything (for the input's loudness).
    pub energy_in: f32,
    /// The input's peak, dBFS, lined up with the output (delayed by the latency).
    pub input_db: f32,
    /// The output's true peak, dBTP.
    pub output_db: f32,
    /// The brickwall's largest reduction, dB (positive).
    pub limit_db: f32,
    /// Each band's dynamic gain — low, mid, high, full — dB: the largest in size over
    /// the column (negative cuts, positive lifts).
    pub bands_db: [f32; 4],
}

impl Column {
    /// Read column `index` (a count of columns since the stream began). Valid for the
    /// last [`COLUMNS`] columns.
    pub fn read(t: &[f32], index: u32) -> Column {
        let at = HEADER + (index as usize % COLUMNS) * COLUMN;
        Column {
            energy_out: t[at],
            energy_in: t[at + 1],
            input_db: t[at + 2],
            output_db: t[at + 3],
            limit_db: t[at + 4],
            bands_db: [t[at + 5], t[at + 6], t[at + 7], t[at + 8]],
        }
    }

    /// Write it as column `index`.
    pub fn write(&self, t: &mut [f32], index: u32) {
        let at = HEADER + (index as usize % COLUMNS) * COLUMN;
        t[at] = self.energy_out;
        t[at + 1] = self.energy_in;
        t[at + 2] = self.input_db;
        t[at + 3] = self.output_db;
        t[at + 4] = self.limit_db;
        t[at + 5..at + 9].copy_from_slice(&self.bands_db);
    }
}

/// How many columns came since `last` (a counter the editor kept), whatever the wrap.
pub fn since(now: f32, last: f32) -> u32 {
    let (now, last) = (now.max(0.0) as u32, last.max(0.0) as u32);
    if now >= last {
        now - last
    } else {
        now + WRAP - last
    }
}

const _: () = assert!(HEADER + COLUMNS * COLUMN <= LEN);
