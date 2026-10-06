//! What a buffer size will cost, said before it is chosen.
//!
//! The round trip a musician hears (what they play → the speakers) is the input's buffer,
//! the output's buffer, and whatever the drivers and the converters add. Cantus measures
//! the whole of it on the running device (round 19's `DeviceLatency`), so the estimate
//! for another buffer size keeps the part that is not the buffers — the driver's and the
//! converters' overhead — and swaps the buffers: `2 × buffer + overhead`. Without a
//! measurement (no device yet) the overhead is taken as nothing, and the figure is the
//! buffers alone: the least it can be.

use cantus_engine::DeviceLatency;

/// The overhead in frames that is not the two buffers, from a measurement made while the
/// device ran blocks of `block` frames.
pub fn overhead(measured: DeviceLatency, block: u32) -> u32 {
    measured.round_trip().saturating_sub(2 * block)
}

/// The round trip in milliseconds a buffer of `buffer` frames at `rate` should give.
pub fn estimate_ms(buffer: u32, rate: f64, overhead_frames: u32) -> f64 {
    if rate <= 0.0 {
        return 0.0;
    }
    (2.0 * buffer as f64 + overhead_frames as f64) / rate * 1000.0
}

/// A latency in ms the way the chip and the lists print it: one decimal below 100.
pub fn fmt_ms(ms: f64, out: &mut String) {
    use std::fmt::Write as _;
    if ms < 100.0 {
        let _ = write!(out, "{ms:.1}");
    } else {
        let _ = write!(out, "{ms:.0}");
    }
}

/// How a round trip feels for playing through the computer: for the colour of its number.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Feel {
    /// Under 10 ms: play through it like an amp.
    Tight,
    /// Under 20 ms: fine for most playing.
    Fine,
    /// Over: mixing, not monitoring.
    Loose,
}

pub fn feel(ms: f64) -> Feel {
    if ms < 10.0 {
        Feel::Tight
    } else if ms < 20.0 {
        Feel::Fine
    } else {
        Feel::Loose
    }
}

/// The rate printed short: 44 100 → "44.1 k", 48 000 → "48 k".
pub fn short_rate(rate: u32) -> String {
    if rate.is_multiple_of(1000) {
        format!("{} k", rate / 1000)
    } else {
        format!("{:.1} k", rate as f64 / 1000.0)
    }
}
