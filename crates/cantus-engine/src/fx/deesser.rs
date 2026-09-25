//! Sibilus (the de-esser) behind the engine's plug-in contract. Owned by the de-esser.
//!
//! [`settings`] reads the parameters (`cantus_core::fx::deesser`) into the DSP's settings
//! (`cantus_dsp::sibilus`) — the editor draws from the same reading, so the picture and the
//! sound cannot disagree.
//!
//! ## Telemetry
//!
//! The editor's history must not depend on the frame rate or lose an "s" between two
//! frames, so the DSP measures it itself, in columns of [`chunk_for`] samples (about
//! 11 ms), counted from the start of the stream. The telemetry carries the last
//! [`HISTORY`] columns and a running count; the editor appends the ones it has not seen.
//! At 60 frames a second it needs 1–2 per frame and has nine.
//!
//! | Index | What |
//! |---|---|
//! | 0 | columns completed since reset, modulo [`SEQ_WRAP`] |
//! | 1 | sample rate, Hz |
//! | 2 | latency, samples |
//! | 3 | samples per column |
//! | 4 | events counted (stretches cut by more than 1 dB) |
//! | 5 | mean of the events' largest cut, dB |
//! | 6 | the largest cut of any event, dB |
//! | 7 | the last event's centre frequency, Hz |
//! | 8 | the last event's largest cut, dB |
//! | 9 | reserved (0) |
//! | 10 + 6·k … | column slot `k` (`k` = column index mod 9): input peak, output peak (linear), detector level (dB), cut (dB), confidence (0–1), trigger centre (Hz) |

use super::{FxUnit, Telemetry, TELEMETRY_LEN};
use cantus_core::EffectSpec;
use cantus_dsp::sibilus::{
    Column, DeessSettings, Detection, EventStats, Listen, Processing, Sibilus, StereoMode, HISTORY,
};

pub use cantus_dsp::sibilus::chunk_for;

pub const T_SEQ: usize = 0;
pub const T_SAMPLE_RATE: usize = 1;
pub const T_LATENCY: usize = 2;
pub const T_CHUNK: usize = 3;
pub const T_EVENTS: usize = 4;
pub const T_MEAN_GR: usize = 5;
pub const T_MAX_GR: usize = 6;
pub const T_LAST_HZ: usize = 7;
pub const T_LAST_GR: usize = 8;
/// Where the columns start, and how many floats each takes.
pub const T_RING: usize = 10;
pub const T_COLUMN: usize = 6;
/// The count wraps here: below 2²⁴, where an f32 stops holding every integer (49 hours
/// at 48 kHz), and a multiple of [`HISTORY`], so a column's slot survives the wrap.
pub const SEQ_WRAP: u64 = (1 << 24) / HISTORY as u64 * HISTORY as u64;

const _: () = assert!(T_RING + HISTORY * T_COLUMN <= TELEMETRY_LEN);

/// A de-esser's parameters as the DSP wants them.
pub fn settings(spec: &EffectSpec) -> DeessSettings {
    let idx = |k: &str| spec.get(k).round().max(0.0) as usize;
    DeessSettings {
        detection: Detection::from_index(idx("mode")),
        processing: Processing::from_index(idx("proc")),
        threshold_db: spec.get("thr"),
        range_db: spec.get("range"),
        lo_hz: spec.get("lo"),
        hi_hz: spec.get("hi"),
        attack_ms: spec.get("att"),
        release_ms: spec.get("rel"),
        lookahead_ms: spec.get("look"),
        link: spec.get("link"),
        stereo: StereoMode::from_index(idx("stereo")),
        listen: Listen::from_index(idx("listen")),
        sensitivity: spec.get("sens"),
    }
}

/// The telemetry of a running de-esser.
pub fn encode(d: &Sibilus) -> Telemetry {
    let mut t = [0.0; TELEMETRY_LEN];
    let seq = d.seq();
    t[T_SEQ] = (seq % SEQ_WRAP) as f32;
    t[T_SAMPLE_RATE] = d.sample_rate();
    t[T_LATENCY] = d.latency() as f32;
    t[T_CHUNK] = d.chunk() as f32;
    let e = d.events();
    t[T_EVENTS] = e.count as f32;
    t[T_MEAN_GR] = e.mean_gr_db;
    t[T_MAX_GR] = e.max_gr_db;
    t[T_LAST_HZ] = e.last_hz;
    t[T_LAST_GR] = e.last_gr_db;
    for index in seq.saturating_sub(HISTORY as u64)..seq {
        if let Some(c) = d.column(index) {
            let at = T_RING + T_COLUMN * (index % HISTORY as u64) as usize;
            t[at..at + T_COLUMN]
                .copy_from_slice(&[c.input, c.output, c.det_db, c.gr_db, c.conf, c.hz]);
        }
    }
    t
}

/// The header of a telemetry block, read back.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Header {
    pub seq: u64,
    pub sample_rate: f32,
    pub latency: usize,
    pub chunk: usize,
    pub events: EventStats,
}

pub fn header(t: &Telemetry) -> Header {
    Header {
        seq: t[T_SEQ].max(0.0) as u64,
        sample_rate: t[T_SAMPLE_RATE],
        latency: t[T_LATENCY].max(0.0) as usize,
        chunk: t[T_CHUNK].max(0.0) as usize,
        events: EventStats {
            count: t[T_EVENTS].max(0.0) as u32,
            mean_gr_db: t[T_MEAN_GR],
            max_gr_db: t[T_MAX_GR],
            last_hz: t[T_LAST_HZ],
            last_gr_db: t[T_LAST_GR],
        },
    }
}

/// Column `index` from a telemetry block whose count is `seq`, if the block still holds
/// it (the last [`HISTORY`] are held).
pub fn column(t: &Telemetry, seq: u64, index: u64) -> Option<Column> {
    if index >= seq || seq - index > HISTORY as u64 {
        return None;
    }
    let at = T_RING + T_COLUMN * (index % HISTORY as u64) as usize;
    let c = &t[at..at + T_COLUMN];
    Some(Column {
        input: c[0],
        output: c[1],
        det_db: c[2],
        gr_db: c[3],
        conf: c[4],
        hz: c[5],
    })
}

#[derive(Debug)]
pub struct Unit {
    dsp: Sibilus,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Sibilus::new(sample_rate),
        }
    }
}

impl FxUnit for Unit {
    fn configure(&mut self, spec: &EffectSpec) {
        self.dsp.set(&settings(spec));
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], key: Option<(&[f32], &[f32])>) {
        self.dsp.process(left, right, key);
    }

    fn reset(&mut self) {
        self.dsp.reset();
    }

    fn latency(&self) -> usize {
        self.dsp.latency()
    }

    fn telemetry(&self) -> Telemetry {
        encode(&self.dsp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::EffectKind;

    #[test]
    fn telemetry_round_trips_the_columns() {
        let mut spec = EffectSpec::new(EffectKind::DeEsser);
        spec.set("thr", -50.0);
        spec.set("look", 0.0);
        let mut u = Unit::new(48_000.0);
        u.configure(&spec);
        u.reset();
        let mut seed = 5u32;
        let mut l: Vec<f32> = (0..9_000)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                (seed as f32 / u32::MAX as f32 - 0.5) * 0.5
            })
            .collect();
        let mut r = l.clone();
        u.process(&mut l, &mut r, None);
        let t = u.telemetry();
        let h = header(&t);
        assert_eq!(h.seq, u.dsp.seq());
        assert_eq!(h.chunk, 512);
        assert_eq!(h.sample_rate, 48_000.0);
        for index in 0..h.seq {
            assert_eq!(column(&t, h.seq, index), u.dsp.column(index));
        }
        assert!(column(&t, h.seq, h.seq).is_none());
    }

    #[test]
    fn the_settings_read_every_parameter() {
        let mut spec = EffectSpec::new(EffectKind::DeEsser);
        let d = settings(&spec);
        assert_eq!(d, DeessSettings::default(), "defaults agree with the DSP's");
        spec.set("mode", 1.0);
        spec.set("proc", 2.0);
        spec.set("stereo", 3.0);
        spec.set("listen", 2.0);
        let d = settings(&spec);
        assert_eq!(d.detection, Detection::Allround);
        assert_eq!(d.processing, Processing::SplitBand);
        assert_eq!(d.stereo, StereoMode::Side);
        assert_eq!(d.listen, Listen::Removed);
    }
}
