//! What Purus's screen draws, held between frames (design sheet v1): the engine's telemetry
//! (`cantus_engine::fx::purus::t`) read every frame — the hum's fundamental and each
//! harmonic's level, depth and hold, the de-reverb's 48 reduction bands three pages at a
//! time, the clicks rebuilt, the decay — and the spectrogram's columns: what is heard in a
//! warm heat, and what was taken out of each row in the residue's colour.
//!
//! The columns come from the slot's signal before and after the unit, cut into
//! [`ROWS`] logarithmic rows from 20 Hz to 20 kHz: in the design sheet from the real unit
//! run over a demo take ([`demo`]); live, the slot's scope (step 3 of the round, as
//! Speculum's). The clicks' lane counts each column's repairs from the telemetry.

use cantus_engine::fx::purus::t;
use cantus_engine::fx::TELEMETRY_LEN;

/// Rows of the spectrogram, logarithmic from [`LO_HZ`] to [`HI_HZ`].
pub const ROWS: usize = 84;
/// Columns kept: the last six seconds at [`HOP_S`].
pub const COLS: usize = 150;
pub const HOP_S: f32 = 0.04;
pub const LO_HZ: f32 = 20.0;
pub const HI_HZ: f32 = 20_000.0;
/// The heat's floor and top, dBFS (a cell's 0 and 255).
pub const FLOOR_DB: f32 = -90.0;
/// Harmonics the de-hum can take.
pub const HARMONICS: usize = 16;
/// The de-reverb's reduction bands.
pub const BANDS: usize = 48;

/// A row's centre frequency.
pub fn row_hz(r: f32) -> f32 {
    LO_HZ * (HI_HZ / LO_HZ).powf((r + 0.5) / ROWS as f32)
}

/// Where a frequency falls, 0 (20 Hz) … 1 (20 kHz).
pub fn frac(hz: f32) -> f32 {
    (cantus_dsp::math::log10(hz.max(1.0) / LO_HZ) / cantus_dsp::math::log10(HI_HZ / LO_HZ))
        .clamp(0.0, 1.0)
}

/// One column: each row's level heard (0 … 255 over [`FLOOR_DB`] … 0 dBFS) and the dB taken
/// out of it (0 … 255 over 0 … 48 dB).
#[derive(Clone, Copy, Debug)]
pub struct Column {
    pub heard: [u8; ROWS],
    pub removed: [u8; ROWS],
    /// Clicks rebuilt in this column's time.
    pub clicks: u16,
}

impl Default for Column {
    fn default() -> Self {
        Column {
            heard: [0; ROWS],
            removed: [0; ROWS],
            clicks: 0,
        }
    }
}

/// The screen's memory.
#[derive(Clone, Debug)]
pub struct Picture {
    /// The newest last; at most [`COLS`].
    pub cols: Vec<Column>,
    pub hum_hz: f32,
    pub hum_nominal: f32,
    pub level: [f32; HARMONICS],
    pub depth: [f32; HARMONICS],
    pub held: u32,
    pub decay: f32,
    pub decay_est: f32,
    pub clicks_total: f32,
    pub in_peak: f32,
    pub out_peak: f32,
    pub removed_db: f32,
    pub latency: f32,
    pub sample_rate: f32,
    /// The de-reverb's reduction per band, dB (pages merged).
    pub verb: [f32; BANDS],
    /// Seconds since a column last came: the screen says it waits after a while.
    pub idle: f32,
    /// Clicks counted since the last column.
    pending_clicks: u32,
    last_total: Option<f32>,
}

impl Default for Picture {
    fn default() -> Self {
        Picture {
            cols: Vec::new(),
            hum_hz: 50.0,
            hum_nominal: 50.0,
            level: [-120.0; HARMONICS],
            depth: [0.0; HARMONICS],
            held: 0,
            decay: 0.8,
            decay_est: 0.0,
            clicks_total: 0.0,
            in_peak: -120.0,
            out_peak: -120.0,
            removed_db: -120.0,
            latency: 0.0,
            sample_rate: 48_000.0,
            verb: [0.0; BANDS],
            idle: f32::INFINITY,
            pending_clicks: 0,
            last_total: None,
        }
    }
}

impl Picture {
    /// Read a block's telemetry (every frame).
    pub fn update(&mut self, tel: &[f32; TELEMETRY_LEN], dt: f32) {
        self.idle += dt;
        if tel[t::SAMPLE_RATE] <= 0.0 {
            return;
        }
        self.hum_hz = tel[t::HUM_HZ];
        self.hum_nominal = tel[t::HUM_NOMINAL];
        self.held = tel[t::HELD] as u32;
        self.decay = tel[t::DECAY];
        self.decay_est = tel[t::DECAY_ESTIMATED];
        self.in_peak = tel[t::IN_PEAK];
        self.out_peak = tel[t::OUT_PEAK];
        self.removed_db = tel[t::REMOVED];
        self.latency = tel[t::LATENCY];
        self.sample_rate = tel[t::SAMPLE_RATE];
        self.level
            .copy_from_slice(&tel[t::HUM_LEVEL..t::HUM_LEVEL + HARMONICS]);
        self.depth
            .copy_from_slice(&tel[t::HUM_DEPTH..t::HUM_DEPTH + HARMONICS]);
        let page = tel[t::PAGE] as usize;
        for j in 0..t::PER_PAGE {
            if let Some(b) = cantus_engine::fx::purus::page_band(page, j) {
                self.verb[b] = tel[t::VERB + j];
            }
        }
        let total = tel[t::CLICKS_TOTAL];
        if let Some(last) = self.last_total {
            let d = total - last;
            if d > 0.0 {
                self.pending_clicks += d as u32;
            }
        }
        self.last_total = Some(total);
        self.clicks_total = total;
    }

    /// A new column, the clicks counted since the last one with it.
    pub fn push(&mut self, mut c: Column) {
        c.clicks = c
            .clicks
            .max(self.pending_clicks.min(u16::MAX as u32) as u16);
        self.pending_clicks = 0;
        if self.cols.len() >= COLS {
            self.cols.remove(0);
        }
        self.cols.push(c);
        self.idle = 0.0;
    }

    /// Whether the screen is waiting for a signal (no column for a second).
    pub fn waiting(&self) -> bool {
        self.idle > 1.0
            && self
                .cols
                .iter()
                .rev()
                .take(25)
                .all(|c| c.heard.iter().all(|v| *v < 8))
    }

    /// The mean de-reverb reduction over the bands, dB.
    pub fn verb_mean(&self) -> f32 {
        self.verb.iter().map(|v| v.abs()).sum::<f32>() / BANDS as f32
    }

    /// Keep the demo's picture still (a design sheet's frames do not age it).
    pub fn hold_demo(&mut self) {
        self.idle = 0.0;
    }
}

/// Fold a spectrogram's linear bins (`cells`, `bins` a column, 8 bits from
/// `floor_db` … 0 dB) into [`ROWS`] logarithmic rows, in dB.
pub fn fold(cells: &[u8], bins: usize, rate: f32, floor_db: f32, out: &mut [f32; ROWS]) {
    let bin_hz = rate * 0.5 / (bins - 1) as f32;
    for (r, o) in out.iter_mut().enumerate() {
        let lo = LO_HZ * (HI_HZ / LO_HZ).powf(r as f32 / ROWS as f32);
        let hi = LO_HZ * (HI_HZ / LO_HZ).powf((r + 1) as f32 / ROWS as f32);
        let v = if hi - lo < 2.0 * bin_hz {
            // Fewer bins than a row is wide: read between the two nearest (the low rows
            // are smoother than the bins).
            let x = (row_hz(r as f32) / bin_hz).min((bins - 1) as f32);
            let i = (x.floor() as usize).min(bins - 2);
            let t = x - i as f32;
            f32::from(cells[i]) * (1.0 - t) + f32::from(cells[i + 1]) * t
        } else {
            let b0 = (lo / bin_hz).floor() as usize;
            let b1 = ((hi / bin_hz).ceil() as usize).max(b0 + 1).min(bins - 1);
            cells[b0.min(bins - 1)..=b1]
                .iter()
                .fold(0u8, |m, c| m.max(*c)) as f32
        };
        *o = floor_db * (1.0 - v / 255.0);
    }
}

/// A dB level as a cell: [`FLOOR_DB`] … 0 dBFS onto 0 … 255.
pub fn heat_cell(db: f32) -> u8 {
    ((1.0 - db / FLOOR_DB).clamp(0.0, 1.0) * 255.0).round() as u8
}

/// The dB taken out as a cell: 0 … 48 dB onto 0 … 255.
pub fn removed_cell(db: f32) -> u8 {
    ((db / 48.0).clamp(0.0, 1.0) * 255.0).round() as u8
}
