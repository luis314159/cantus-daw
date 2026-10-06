//! Multiplex: the multiband dynamics. Up to [`MAX_BANDS`] bands that tile the spectrum,
//! each with **two-sided** dynamics — a section above an upper threshold and one below a
//! lower threshold, each with its own ratio and range — so one band can compress downward
//! and upward at once, or gate and compress, which the benchmark cannot
//! (`research/fabfilter/pro-mb.md` §7). Times are in milliseconds, levels in dB.
//!
//! ## Two ways to apply the gains, honestly named
//!
//! Every band has a detector — the key (its own input or the slot's sidechain) through a
//! band-pass at the band's edges, or at a free range — and a gain computer. What differs is
//! how the gains reach the audio:
//!
//! - **Dynamic** (the default): the signal is *not* split. The output is the input times
//!   the first band's gain, through one shelf per crossover that steps from one band's
//!   gain to the next (`multiplex_filters::Shelf`). A shelf whose two sides are equal is
//!   the identity and is skipped, so **while no band moves the output is the input, to the
//!   bit**, and when every band moves together it is a pure gain with no phase shift at
//!   all. Phase shift exists only at a crossover whose two bands differ, and only as much
//!   as they differ. Zero latency. This is what the benchmark calls "Dynamic Phase".
//! - **Split**: a Linkwitz–Riley tree (6, 12, 24 or 48 dB/oct) with the bands below each
//!   crossover passed through its all-pass, so the bands at unity sum to an all-pass —
//!   flat magnitude, measured in the tests. Each band is a real band of the signal. When
//!   no band *can* act (every ratio 1:1 or range 0, no gain, no solo or mute) the split is
//!   not run and the device is a wire; switching between the two cross-fades over
//!   [`FADE_MS`], so the switch is never a click.
//!
//! Solo and mute need real bands, so in the dynamic mode they are heard through the split
//! (with the same cross-fade); that is the one place the two modes sound alike on purpose.
//!
//! ## What the null tests rely on
//!
//! - Per-sample work (detectors, split gains) is per sample; the dynamic path's shelves and
//!   every glide are updated every [`UPDATE`] samples counted from the start of the stream,
//!   not of the block, so the output is independent of the block size. The dynamic path
//!   interpolates its shelves linearly between updates, from the gains at the previous
//!   update to the gains now — so it follows the per-sample gains 16 samples (0.33 ms at
//!   48 kHz) behind; the split applies them to the sample.
//! - Lookahead delays the audio by the largest lookahead of any band ([`Multiplex::latency`])
//!   and each band's gain by the difference, so each band looks ahead by its own amount.

#[path = "multiplex_filters.rs"]
pub mod filters;
#[path = "multiplex_linear.rs"]
pub mod linear;

pub use filters::Slope;
use filters::{shelf_params, warp, Shelf, Svf, SvfCoefs, Xover, XoverCoefs, C};
use linear::Linear;

pub const MAX_BANDS: usize = 8;
pub const MAX_XOVERS: usize = MAX_BANDS - 1;
/// Samples between updates of the glides and of the dynamic path's shelves.
pub const UPDATE: usize = 16;
pub const MAX_LOOKAHEAD_MS: f32 = 20.0;
/// How long a switch between the dynamic path and the split takes.
pub const FADE_MS: f32 = 10.0;
/// How long a solo or mute takes to come in or go out.
const AUDIBLE_MS: f32 = 5.0;
/// Samples per window of the meters' hold: a peak stays visible for one to two windows
/// (21–43 ms at 48 kHz), longer than a display frame.
pub const HOLD: usize = 1024;
/// How much of the remaining distance a glide covers per update: about 20 ms to settle.
const GLIDE: f32 = 0.35;
/// The lowest level the gain computer sees, dB — so silence is a number.
const FLOOR_DB: f32 = -150.0;

/// How the gains reach the audio. See the module notes.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    Dynamic,
    Split,
    /// The split with linear-phase crossovers (`linear`): no phase shift at all, at the
    /// cost of latency.
    Linear,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Dynamic, Mode::Split, Mode::Linear];

    pub fn from_index(i: usize) -> Mode {
        match i {
            1 => Mode::Split,
            2 => Mode::Linear,
            _ => Mode::Dynamic,
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

/// Which part of the stereo image a band listens to and acts on.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Placement {
    /// Both channels, one gain, from the louder of the two.
    Stereo,
    Mid,
    Side,
}

impl Placement {
    pub const ALL: [Placement; 3] = [Placement::Stereo, Placement::Mid, Placement::Side];

    pub fn from_index(i: usize) -> Placement {
        Self::ALL[i.min(2)]
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|p| *p == self).unwrap_or(0)
    }

    /// Whether the band's gain applies to the mid (0) or side (1) signal.
    fn applies(self, h: usize) -> bool {
        match self {
            Placement::Stereo => true,
            Placement::Mid => h == 0,
            Placement::Side => h == 1,
        }
    }
}

/// What to hear.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Listen {
    Output,
    /// What the bands change: the path at unity minus the output.
    Delta,
    /// The detectors' input: each audible band's key through its detector filter.
    Key,
}

impl Listen {
    pub fn from_index(i: usize) -> Listen {
        match i {
            1 => Listen::Delta,
            2 => Listen::Key,
            _ => Listen::Output,
        }
    }
}

/// One band, as the engine is told it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct BandSettings {
    /// The band exists: it has a place in the spectrum.
    pub used: bool,
    /// Not bypassed. A band that is off keeps its place and does nothing.
    pub on: bool,
    pub solo: bool,
    pub mute: bool,
    /// Where the band starts; ignored for the lowest band.
    pub from_hz: f32,
    /// Above this level the upper ratio applies.
    pub thr_db: f32,
    /// > 1 compresses downward, < 1 expands upward, 1 is off.
    pub ratio: f32,
    /// The most the upper section moves the gain, dB.
    pub range_db: f32,
    /// Below this level the lower ratio applies.
    pub thr_lo_db: f32,
    /// > 1 compresses upward, < 1 expands downward, 1 is off.
    pub ratio_lo: f32,
    pub range_lo_db: f32,
    pub knee_db: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub lookahead_ms: f32,
    /// Static gain, dB.
    pub gain_db: f32,
    pub placement: Placement,
    /// Listen to the slot's sidechain rather than the band's own input.
    pub key_ext: bool,
    /// The detector hears `key_lo_hz`…`key_hi_hz` rather than the band's own range.
    pub key_free: bool,
    pub key_lo_hz: f32,
    pub key_hi_hz: f32,
}

impl Default for BandSettings {
    fn default() -> Self {
        BandSettings {
            used: false,
            on: true,
            solo: false,
            mute: false,
            from_hz: 1_000.0,
            thr_db: -24.0,
            ratio: 2.0,
            range_db: 12.0,
            thr_lo_db: -60.0,
            ratio_lo: 1.0,
            range_lo_db: 12.0,
            knee_db: 6.0,
            attack_ms: 10.0,
            release_ms: 150.0,
            lookahead_ms: 0.0,
            gain_db: 0.0,
            placement: Placement::Stereo,
            key_ext: false,
            key_free: false,
            key_lo_hz: 200.0,
            key_hi_hz: 4_000.0,
        }
    }
}

impl BandSettings {
    /// Whether this band can change the signal at all, whatever the signal.
    pub fn can_act(&self) -> bool {
        self.used
            && self.on
            && ((self.ratio != 1.0 && self.range_db > 0.0)
                || (self.ratio_lo != 1.0 && self.range_lo_db > 0.0)
                || self.gain_db != 0.0)
    }
}

/// Everything the device is.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Settings {
    pub bands: [BandSettings; MAX_BANDS],
    pub mode: Mode,
    pub slope: Slope,
    pub output_db: f32,
    pub listen: Listen,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            bands: [BandSettings::default(); MAX_BANDS],
            mode: Mode::Dynamic,
            slope: Slope::S24,
            output_db: 0.0,
            listen: Listen::Output,
        }
    }
}

/// The used bands in frequency order, and the crossovers between them.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Layout {
    /// Band numbers, lowest first; `order[..n]` is meaningful.
    pub order: [usize; MAX_BANDS],
    pub n: usize,
    /// `crossovers[i]` sits between `order[i]` and `order[i + 1]`.
    pub crossovers: [f32; MAX_XOVERS],
}

impl Layout {
    pub fn of(bands: &[BandSettings; MAX_BANDS]) -> Layout {
        let mut order = [0usize; MAX_BANDS];
        let mut n = 0;
        for (i, b) in bands.iter().enumerate() {
            if !b.used {
                continue;
            }
            // Insertion by frequency; ties keep the band number's order.
            let mut at = n;
            while at > 0 && bands[order[at - 1]].from_hz > b.from_hz {
                order[at] = order[at - 1];
                at -= 1;
            }
            order[at] = i;
            n += 1;
        }
        let mut crossovers = [1_000.0; MAX_XOVERS];
        for (c, b) in crossovers.iter_mut().zip(&order[1..n.max(1)]) {
            *c = bands[*b].from_hz;
        }
        Layout {
            order,
            n,
            crossovers,
        }
    }

    /// Where band `band` sits in the order, if it is used.
    pub fn position(&self, band: usize) -> Option<usize> {
        self.order[..self.n].iter().position(|b| *b == band)
    }

    /// A band's edges in Hz: `None` below for the lowest, `None` above for the highest.
    pub fn edges(&self, band: usize) -> Option<(Option<f32>, Option<f32>)> {
        let pos = self.position(band)?;
        let lo = (pos > 0).then(|| self.crossovers[pos - 1]);
        let hi = (pos + 1 < self.n).then(|| self.crossovers[pos]);
        Some((lo, hi))
    }
}

/// `0` below `−knee/2`, `over` above `knee/2`, and the quadratic that joins them.
#[inline]
fn soft(over: f32, knee: f32) -> f32 {
    if knee <= 0.0 {
        return over.max(0.0);
    }
    if over <= -knee * 0.5 {
        0.0
    } else if over >= knee * 0.5 {
        over
    } else {
        (over + knee * 0.5).powi(2) / (2.0 * knee)
    }
}

/// The dynamic gain a level earns, dB (positive boosts): the upper section plus the lower
/// one, each within its range. The band's static gain is not included.
pub fn gain_db(level_db: f32, b: &BandSettings) -> f32 {
    let level = level_db.max(FLOOR_DB);
    let mut g = 0.0;
    if b.ratio != 1.0 && b.range_db > 0.0 {
        let hi = (1.0 / b.ratio.max(0.01) - 1.0) * soft(level - b.thr_db, b.knee_db);
        g += hi.clamp(-b.range_db, b.range_db);
    }
    if b.ratio_lo != 1.0 && b.range_lo_db > 0.0 {
        let lo = (1.0 - 1.0 / b.ratio_lo.max(0.01)) * soft(b.thr_lo_db - level, b.knee_db);
        g += lo.clamp(-b.range_lo_db, b.range_lo_db);
    }
    g
}

/// The output level for an input level on the band's static curve — what its transfer
/// graph draws.
pub fn transfer_db(level_db: f32, b: &BandSettings) -> f32 {
    level_db + gain_db(level_db, b)
}

/// The device's response at `hz`, dB, with each band at `band_db` (static plus dynamic,
/// by band number; the caller decides what a bypassed or silent band is at) — the curve
/// the editor draws, from the same filters the audio runs. In the dynamic mode this is the
/// mid signal's path (a side-only band is left out).
pub fn response_db(s: &Settings, band_db: &[f32; MAX_BANDS], sample_rate: f32, hz: f32) -> f32 {
    response_part_db(s, band_db, sample_rate, hz, false)
}

/// The same, for the mid (`side == false`) or the side part of the signal: a mid-only band
/// moves only the first, a side-only band only the second.
pub fn response_part_db(
    s: &Settings,
    band_db: &[f32; MAX_BANDS],
    sample_rate: f32,
    hz: f32,
    side: bool,
) -> f32 {
    let part = usize::from(side);
    let lay = Layout::of(&s.bands);
    if lay.n == 0 {
        return 0.0;
    }
    let lin = |pos: usize, h: usize| -> f64 {
        let b = &s.bands[lay.order[pos]];
        if b.placement.applies(h) {
            10f64.powf(band_db[lay.order[pos]] as f64 / 20.0)
        } else {
            1.0
        }
    };
    let h = match s.mode {
        Mode::Dynamic => {
            let mut h = C::ONE.scale(lin(0, part));
            for i in 0..lay.n - 1 {
                let r = (lin(i + 1, part) / lin(i, part)) as f32;
                let (g, rho) = shelf_params(s.slope, lay.crossovers[i], r, sample_rate);
                h = h.mul(filters::shelf_response(s.slope, g, rho, hz, sample_rate));
            }
            h
        }
        Mode::Linear => {
            // Zero phase: the top gain plus each crossover's low-pass magnitude times the
            // difference of the gains either side of it.
            let n = lay.n;
            let mut h = lin(n - 1, part);
            for i in 0..n - 1 {
                let c = XoverCoefs::new(s.slope, lay.crossovers[i], sample_rate);
                let lp = filters::xover_response(&c, hz, sample_rate).0.abs();
                h += (lin(i, part) - lin(i + 1, part)) * lp;
            }
            C::new(h, 0.0)
        }
        Mode::Split => {
            let n = lay.n;
            let mut parts = [(C::ONE, C::ONE, C::ONE); MAX_XOVERS];
            for (i, part) in parts.iter_mut().enumerate().take(n - 1) {
                let c = XoverCoefs::new(s.slope, lay.crossovers[i], sample_rate);
                *part = filters::xover_response(&c, hz, sample_rate);
            }
            let mut acc = C::new(0.0, 0.0);
            // The bands with their compensation, exactly as the audio path sums them.
            let mut above = C::ONE; // Π of the high halves below the band
            for k in 0..n {
                let mut band = above;
                if k + 1 < n {
                    band = band.mul(parts[k].0);
                    above = above.mul(parts[k].1);
                }
                for part in parts.iter().take(n - 1).skip(k + 1) {
                    band = band.mul(part.2);
                }
                acc = acc.add(band.scale(lin(k, part)));
            }
            acc
        }
    };
    (20.0 * h.abs().max(1e-9).log10()) as f32
}

/// What the display reads: the band gains and levels, and the meters — each the extreme of
/// the last one or two [`HOLD`] windows, so nothing between two frames is lost.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct MbMeter {
    /// The dynamic gain each band is applying, dB, by band number (the larger in size of
    /// the held extremes).
    pub gain_db: [f32; MAX_BANDS],
    /// Each band's detector level, dB.
    pub level_db: [f32; MAX_BANDS],
    /// Input and output peaks, left and right, linear.
    pub input: [f32; 2],
    pub output: [f32; 2],
    /// How much of the output comes from the split, 0…1.
    pub split: f32,
}

#[derive(Copy, Clone, Debug)]
struct HoldWindow {
    gain: [f32; MAX_BANDS],
    level: [f32; MAX_BANDS],
    input: [f32; 2],
    output: [f32; 2],
}

impl Default for HoldWindow {
    fn default() -> Self {
        HoldWindow {
            gain: [0.0; MAX_BANDS],
            level: [FLOOR_DB; MAX_BANDS],
            input: [0.0; 2],
            output: [0.0; 2],
        }
    }
}

/// A band's parameters as they glide toward the settings.
#[derive(Copy, Clone, Debug)]
struct Live {
    b: BandSettings,
    att: f32,
    rel: f32,
    /// Either section can move the gain (so the gain computer has work to do).
    moves: bool,
}

/// A band's detector: the band-pass (two sections each side) per channel, and the envelope.
#[derive(Copy, Clone, Debug, Default)]
struct Detector {
    hp: [[Svf; 2]; 2],
    lp: [[Svf; 2]; 2],
    hp_c: Option<SvfCoefs>,
    lp_c: Option<SvfCoefs>,
    env: f32,
}

impl Detector {
    #[inline]
    fn filter(&mut self, ch: usize, x: f32) -> f32 {
        let mut v = x;
        if let Some(c) = self.hp_c {
            for s in &mut self.hp[ch] {
                v = s.tick(&c, v).2;
            }
        }
        if let Some(c) = self.lp_c {
            for s in &mut self.lp[ch] {
                v = s.tick(&c, v).0;
            }
        }
        v
    }
}

/// The multiband dynamics, stereo in place.
#[derive(Clone, Debug)]
pub struct Multiplex {
    sr: f32,
    s: Settings,
    configured: bool,
    lay: Layout,
    live: [Live; MAX_BANDS],
    can_act: bool,
    /// Crossover frequencies in force (gliding) and their coefficients.
    xo_hz: [f32; MAX_XOVERS],
    xo: [XoverCoefs; MAX_XOVERS],
    /// The split: per channel, per crossover; and the all-passes of the delta's sum.
    split: [[Xover; MAX_XOVERS]; 2],
    split_delta: [[Xover; MAX_XOVERS]; 2],
    /// The dynamic path: per chain (mid, side), per crossover.
    shelves: [[Shelf; MAX_XOVERS]; 2],
    /// Each stage's (pole coefficient, section gain) at the last update and the one before.
    stage_from: [[(f32, f32); MAX_XOVERS]; 2],
    stage_to: [[(f32, f32); MAX_XOVERS]; 2],
    g0_from: [f32; 2],
    g0_to: [f32; 2],
    det: [Detector; MAX_BANDS],
    /// Per band, the gain computed now (before its lookahead delay), in a ring, and its
    /// dynamic part in dB for the display.
    gain_ring: Vec<f32>,
    dyn_ring: Vec<f32>,
    /// Per band, its lookahead in samples.
    la: [usize; MAX_BANDS],
    audio_ring: [Vec<f32>; 2],
    ring_len: usize,
    ring_pos: usize,
    /// The gain rings are longer than the audio's: in the linear mode the audio is also
    /// late by the filters.
    gain_len: usize,
    gain_pos: usize,
    /// The linear-phase split, and whether it has been running.
    lin: Linear,
    lin_live: bool,
    /// The whole latency: the lookahead, plus the linear filters in that mode.
    total: usize,
    /// The largest lookahead of any band that is on, samples.
    la_max: usize,
    /// Per band: the linear gain being applied, and its dynamic part in dB.
    g_now: [f32; MAX_BANDS],
    dyn_now: [f32; MAX_BANDS],
    /// Per band, 0…1: how audible solo and mute leave it, and where it is going.
    audible: [f32; MAX_BANDS],
    audible_t: [f32; MAX_BANDS],
    /// Per band, 0…1: how far switched on it is (bypass fades).
    on_amt: [f32; MAX_BANDS],
    /// The per-sample step of the 5 ms fades.
    astep: f32,
    /// Some band is soloed or muted.
    monitor: bool,
    /// How much of the output is the split (0…1), and where it is going.
    w: f32,
    split_live: bool,
    dyn_live: bool,
    out_from: f32,
    out_to: f32,
    /// Position inside the current update, counted from the start of the stream.
    phase: usize,
    hold_pos: usize,
    hold: [HoldWindow; 2],
    // ADR 0031 §5.1, at rest.
    /// Frames since the last reset (what `skip` adds to as well).
    clock: u64,
    /// Where the linear split's blocks began, on that clock (mod its block).
    lin_origin: usize,
    /// Frames in a row in which the audio ring received silence.
    quiet: usize,
    /// Frames in a row in which every band in the layout wrote unity gain and no dynamic
    /// gain into its rings.
    unity_run: usize,
}

impl Multiplex {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1.0);
        let ring_len = (MAX_LOOKAHEAD_MS * 0.001 * sr).ceil() as usize + 1;
        let lin = Linear::new(sr);
        let gain_len = ring_len + lin.latency();
        let xo = XoverCoefs::new(Slope::S24, 1_000.0, sr);
        let base = (xo.g, 1.0);
        let live = Live {
            b: BandSettings::default(),
            att: 0.0,
            rel: 0.0,
            moves: false,
        };
        Multiplex {
            sr,
            s: Settings::default(),
            configured: false,
            lay: Layout::of(&[BandSettings::default(); MAX_BANDS]),
            live: [live; MAX_BANDS],
            can_act: false,
            xo_hz: [1_000.0; MAX_XOVERS],
            xo: [xo; MAX_XOVERS],
            split: [[Xover::default(); MAX_XOVERS]; 2],
            split_delta: [[Xover::default(); MAX_XOVERS]; 2],
            shelves: [[Shelf::default(); MAX_XOVERS]; 2],
            stage_from: [[base; MAX_XOVERS]; 2],
            stage_to: [[base; MAX_XOVERS]; 2],
            g0_from: [1.0; 2],
            g0_to: [1.0; 2],
            det: [Detector::default(); MAX_BANDS],
            gain_ring: vec![1.0; gain_len * MAX_BANDS],
            dyn_ring: vec![0.0; gain_len * MAX_BANDS],
            la: [0; MAX_BANDS],
            audio_ring: [vec![0.0; ring_len], vec![0.0; ring_len]],
            ring_len,
            ring_pos: 0,
            gain_len,
            gain_pos: 0,
            lin,
            lin_live: false,
            total: 0,
            la_max: 0,
            g_now: [1.0; MAX_BANDS],
            dyn_now: [0.0; MAX_BANDS],
            audible: [1.0; MAX_BANDS],
            audible_t: [1.0; MAX_BANDS],
            on_amt: [1.0; MAX_BANDS],
            astep: 1.0 / (AUDIBLE_MS * 0.001 * sr),
            monitor: false,
            w: 0.0,
            split_live: false,
            dyn_live: false,
            out_from: 1.0,
            out_to: 1.0,
            phase: 0,
            hold_pos: 0,
            hold: [HoldWindow::default(); 2],
            clock: 0,
            lin_origin: 0,
            quiet: usize::MAX,
            unity_run: usize::MAX,
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    pub fn reset(&mut self) {
        self.split = [[Xover::default(); MAX_XOVERS]; 2];
        self.split_delta = [[Xover::default(); MAX_XOVERS]; 2];
        self.shelves = [[Shelf::default(); MAX_XOVERS]; 2];
        for d in &mut self.det {
            d.hp = Default::default();
            d.lp = Default::default();
            d.env = 0.0;
        }
        self.gain_ring.fill(1.0);
        self.dyn_ring.fill(0.0);
        for r in &mut self.audio_ring {
            r.fill(0.0);
        }
        self.ring_pos = 0;
        self.gain_pos = 0;
        self.lin.reset();
        self.g_now = [1.0; MAX_BANDS];
        self.dyn_now = [0.0; MAX_BANDS];
        self.phase = 0;
        self.hold_pos = 0;
        self.hold = [HoldWindow::default(); 2];
        self.reset_stages();
        self.clock = 0;
        self.lin_origin = 0;
        self.quiet = usize::MAX;
        self.unity_run = usize::MAX;
    }

    fn reset_stages(&mut self) {
        for h in 0..2 {
            for i in 0..MAX_XOVERS {
                let base = (self.xo[i].g, 1.0);
                self.stage_from[h][i] = base;
                self.stage_to[h][i] = base;
            }
        }
        self.g0_from = [1.0; 2];
        self.g0_to = [1.0; 2];
    }

    /// Samples of delay this adds: the largest lookahead of any band that is on, plus the
    /// linear-phase filters' delay in that mode.
    pub fn latency(&self) -> usize {
        self.total
    }

    pub fn settings(&self) -> &Settings {
        &self.s
    }

    /// The layout in force: bands in frequency order and the crossovers between them.
    pub fn layout(&self) -> Layout {
        self.lay
    }

    /// Take the next settings at once, as if they were the first: for a host that sets a
    /// unit up more than once before it has processed anything (Culmen, a plug-in loading
    /// its state after activation). No allocation.
    pub fn snap_next(&mut self) {
        self.configured = false;
    }

    /// New settings. The first call snaps; later ones glide over ~20 ms.
    pub fn set(&mut self, s: &Settings) {
        let snap = !self.configured;
        self.configured = true;
        let slope_changed = s.slope != self.s.slope;
        self.s = *s;
        let lay = Layout::of(&s.bands);
        let topology = snap || lay.n != self.lay.n || lay.order != self.lay.order;
        if topology || s.bands.iter().zip(&self.live).any(|(a, l)| a.used != l.b.used) {
            self.unity_run = 0;
        }
        self.lay = lay;
        for (b, live) in self.live.iter_mut().enumerate() {
            let t = s.bands[b];
            let fresh = snap || !live.b.used || !t.used;
            if t.used && !live.b.used {
                // A band made (or made again): its detector starts from silence.
                self.det[b] = Detector::default();
                self.on_amt[b] = if t.on { 1.0 } else { 0.0 };
            }
            let keep = live.b;
            live.b = t;
            if !fresh {
                // These glide; everything else is taken at once.
                live.b.thr_db = keep.thr_db;
                live.b.ratio = keep.ratio;
                live.b.range_db = keep.range_db;
                live.b.thr_lo_db = keep.thr_lo_db;
                live.b.ratio_lo = keep.ratio_lo;
                live.b.range_lo_db = keep.range_lo_db;
                live.b.knee_db = keep.knee_db;
                live.b.gain_db = keep.gain_db;
            }
            let coef = |ms: f32| (-1.0 / (ms.max(0.01) * 0.001 * self.sr)).exp();
            live.att = coef(t.attack_ms);
            live.rel = coef(t.release_ms);
            // While a section glides toward 1:1 it still moves; it stops once it lands.
            live.moves = (t.ratio != 1.0 && t.range_db > 0.0)
                || (t.ratio_lo != 1.0 && t.range_lo_db > 0.0)
                || (live.b.ratio != 1.0 && live.b.range_db > 0.0)
                || (live.b.ratio_lo != 1.0 && live.b.range_lo_db > 0.0);
        }
        if topology {
            self.xo_hz = lay.crossovers;
        }
        if topology || slope_changed {
            for i in 0..MAX_XOVERS {
                self.xo[i] = XoverCoefs::new(s.slope, self.xo_hz[i], self.sr);
            }
            self.reset_stages();
            self.detector_coefs();
        }
        let max_la = self.ring_len - 1;
        let mut latency = 0;
        for (b, band) in s.bands.iter().enumerate() {
            let la = ((band.lookahead_ms.clamp(0.0, MAX_LOOKAHEAD_MS) * 0.001 * self.sr).round()
                as usize)
                .min(max_la);
            self.la[b] = la;
            if band.used && band.on {
                latency = latency.max(la);
            }
        }
        self.la_max = latency;
        self.total = latency
            + if s.mode == Mode::Linear && lay.n > 0 {
                self.lin.latency()
            } else {
                0
            };
        self.can_act = s.bands.iter().any(|b| b.can_act());
        let any_solo = s.bands.iter().any(|b| b.used && b.solo);
        self.monitor = s.bands.iter().any(|b| b.used && (b.solo || b.mute));
        for (t, band) in self.audible_t.iter_mut().zip(&s.bands) {
            *t = if band.used && (band.mute || (any_solo && !band.solo)) {
                0.0
            } else {
                1.0
            };
        }
        let out = db_to_lin(s.output_db);
        if snap {
            self.out_from = out;
            self.out_to = out;
            self.audible = self.audible_t;
            for (a, b) in self.on_amt.iter_mut().zip(&s.bands) {
                *a = if b.on { 1.0 } else { 0.0 };
            }
            self.w = self.split_target();
            // Start from the static gains, so a band's gain is in force from the first
            // sample rather than one update later.
            for b in 0..MAX_BANDS {
                let band = &s.bands[b];
                let g = if band.used && band.on {
                    db_to_lin(band.gain_db)
                } else {
                    1.0
                };
                self.g_now[b] = g;
                self.gain_ring[b * self.gain_len..(b + 1) * self.gain_len].fill(g);
            }
            self.update_stages();
            self.stage_from = self.stage_to;
            self.g0_from = self.g0_to;
        }
    }

    /// 1 when the output should come from the split.
    fn split_target(&self) -> f32 {
        let monitoring = self.monitor || self.audible.iter().any(|a| *a < 1.0);
        if monitoring || (self.s.mode == Mode::Split && self.can_act) {
            1.0
        } else {
            0.0
        }
    }

    /// At rest (ADR 0031 §5.1): a wire; or every glide landed (band settings, crossovers,
    /// output, solo and mute, bypass fades, the split's share), every band at unity with
    /// no dynamic gain — in force and all through its lookahead ring, as a reset leaves
    /// it — every detector, split, shelf and the audio ring empty, and in linear phase the
    /// FFT blocks empty, on the grid a new unit's would run on, with filters designed exactly
    /// where the crossovers are. A band with a static gain, or an expander that acts on
    /// silence, is never at rest: a reset starts every band at unity and lets it move.
    pub fn at_rest(&self) -> bool {
        if !self.is_active() {
            return true;
        }
        let s = &self.s;
        let n = self.lay.n;
        if self.quiet < self.ring_len
            || self.unity_run < self.gain_len
            || self.out_from != self.out_to
            || self.out_to != db_to_lin(s.output_db)
            || self.g0_from != [1.0; 2]
            || self.g0_to != [1.0; 2]
        {
            return false;
        }
        for h in 0..2 {
            for i in 0..MAX_XOVERS {
                let base = (self.xo[i].g, 1.0);
                if self.stage_from[h][i] != base || self.stage_to[h][i] != base {
                    return false;
                }
            }
        }
        for &b in &self.lay.order[..n] {
            let band = &s.bands[b];
            let d = &self.det[b];
            if self.live[b].b != *band
                || self.g_now[b] != 1.0
                || self.dyn_now[b] != 0.0
                || self.audible[b] != self.audible_t[b]
                || self.on_amt[b] != if band.on { 1.0 } else { 0.0 }
                || d.env != 0.0
                || !d.hp.iter().chain(&d.lp).flatten().all(Svf::is_clear)
            {
                return false;
            }
        }
        if (0..n.saturating_sub(1)).any(|i| self.xo_hz[i] != self.lay.crossovers[i]) {
            return false;
        }
        if s.mode == Mode::Linear && n > 0 {
            // Its blocks must fall where a new unit's would: started with the stream.
            self.lin_live
                && self.lin_origin == 0
                && self.lin.is_clear()
                && (0..n - 1).all(|c| self.lin.designed_at(c, self.xo_hz[c], s.slope))
        } else {
            let clear = |x: &[[Xover; MAX_XOVERS]; 2]| x.iter().flatten().all(Xover::is_clear);
            self.w == self.split_target()
                && (!self.split_live || (clear(&self.split) && clear(&self.split_delta)))
                && (!self.dyn_live || self.shelves.iter().flatten().all(Shelf::is_clear))
        }
    }

    /// Samples of silence skipped while at rest (ADR 0031 §5.1): the update grid, the
    /// clock and the linear split's block position move on as processing silence would
    /// have moved them.
    pub fn skip(&mut self, frames: usize) {
        if self.is_active() {
            if self.s.mode == Mode::Linear && self.lay.n > 0 {
                if !self.lin_live {
                    self.lin.reset();
                    self.lin_live = true;
                    self.lin_origin = (self.clock % self.lin.block_pos().1 as u64) as usize;
                }
                self.lin.skip(frames);
            } else {
                self.lin_live = false;
            }
            self.quiet = self.quiet.saturating_add(frames);
            self.unity_run = self.unity_run.saturating_add(frames);
            // A band made later reads its old ring where processing would have left it.
            self.ring_pos = (self.ring_pos + frames % self.ring_len) % self.ring_len;
            self.gain_pos = (self.gain_pos + frames % self.gain_len) % self.gain_len;
        }
        self.phase = (self.phase + frames % UPDATE) % UPDATE;
        self.clock += frames as u64;
    }

    /// Whether processing would change anything, or has anything to meter.
    pub fn is_active(&self) -> bool {
        self.lay.n > 0 || self.out_to != 1.0 || self.out_from != 1.0
    }

    /// What the display reads.
    pub fn meter(&self) -> MbMeter {
        let (a, b) = (&self.hold[0], &self.hold[1]);
        let mut m = MbMeter {
            split: self.w,
            ..MbMeter::default()
        };
        for i in 0..MAX_BANDS {
            m.gain_db[i] = if a.gain[i].abs() >= b.gain[i].abs() {
                a.gain[i]
            } else {
                b.gain[i]
            };
            m.level_db[i] = a.level[i].max(b.level[i]);
        }
        for ch in 0..2 {
            m.input[ch] = a.input[ch].max(b.input[ch]);
            m.output[ch] = a.output[ch].max(b.output[ch]);
        }
        m
    }

    /// The dynamic gain each band applies right now, dB (no hold).
    pub fn gains_now_db(&self) -> [f32; MAX_BANDS] {
        self.dyn_now
    }

    /// Each band's detector level right now, dB.
    pub fn levels_now_db(&self) -> [f32; MAX_BANDS] {
        let mut out = [FLOOR_DB; MAX_BANDS];
        for (o, d) in out.iter_mut().zip(&self.det) {
            *o = lin_to_db(d.env);
        }
        out
    }

    /// Detector filters from the crossovers in force (or each band's free range).
    fn detector_coefs(&mut self) {
        let k = std::f32::consts::SQRT_2;
        for pos in 0..self.lay.n {
            let b = self.lay.order[pos];
            let band = &self.s.bands[b];
            let d = &mut self.det[b];
            if band.key_free {
                let lo = band.key_lo_hz.min(band.key_hi_hz);
                let hi = band.key_hi_hz.max(band.key_lo_hz);
                d.hp_c = (lo > 20.5).then(|| SvfCoefs::new(warp(lo, self.sr), k));
                d.lp_c = (hi < 19_500.0).then(|| SvfCoefs::new(warp(hi, self.sr), k));
            } else {
                d.hp_c = (pos > 0).then(|| SvfCoefs::new(self.xo[pos - 1].g, k));
                d.lp_c = (pos + 1 < self.lay.n).then(|| SvfCoefs::new(self.xo[pos].g, k));
            }
        }
    }

    /// Advance every glide by one update, and set the dynamic path's next stage targets.
    fn update(&mut self) {
        let sr = self.sr;
        // Band parameters.
        for b in 0..MAX_BANDS {
            let t = self.s.bands[b];
            if !t.used {
                continue;
            }
            let l = &mut self.live[b].b;
            glide(&mut l.thr_db, t.thr_db, false);
            glide(&mut l.ratio, t.ratio, true);
            glide(&mut l.range_db, t.range_db, false);
            glide(&mut l.thr_lo_db, t.thr_lo_db, false);
            glide(&mut l.ratio_lo, t.ratio_lo, true);
            glide(&mut l.range_lo_db, t.range_lo_db, false);
            glide(&mut l.knee_db, t.knee_db, false);
            glide(&mut l.gain_db, t.gain_db, false);
        }
        // Crossovers.
        let mut moved = false;
        for i in 0..self.lay.n.saturating_sub(1) {
            if glide(&mut self.xo_hz[i], self.lay.crossovers[i], true) {
                self.xo[i] = XoverCoefs::new(self.s.slope, self.xo_hz[i], sr);
                moved = true;
            }
        }
        // The band detectors follow the crossovers; free key ranges move at once.
        if moved || self.s.bands.iter().any(|b| b.used && b.key_free) {
            self.detector_coefs();
        }
        // Output gain.
        self.out_from = self.out_to;
        self.out_to = db_to_lin(self.s.output_db);
        // The dynamic path: from where it was aiming to the gains now.
        self.stage_from = self.stage_to;
        self.g0_from = self.g0_to;
        self.update_stages();
    }

    /// The dynamic path's stage targets from the gains in force.
    fn update_stages(&mut self) {
        let sr = self.sr;
        let n = self.lay.n;
        for h in 0..2 {
            let gain = |pos: usize| -> f32 {
                let b = self.lay.order[pos];
                if self.s.bands[b].placement.applies(h) {
                    self.g_now[b]
                } else {
                    1.0
                }
            };
            if n == 0 {
                self.g0_to[h] = 1.0;
                continue;
            }
            self.g0_to[h] = gain(0);
            for i in 0..n - 1 {
                let r = gain(i + 1) / gain(i);
                self.stage_to[h][i] = shelf_params(self.s.slope, self.xo_hz[i], r, sr);
            }
        }
    }

    /// Process a stereo block in place. `key` is the external sidechain, when routed.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32], key: Option<(&[f32], &[f32])>) {
        let n = left.len().min(right.len());
        if !self.is_active() {
            // A wire. Keep the clocks running, so a band made later lands on the same grid
            // whatever the block size was before, and keep the meters honest.
            for i in 0..n {
                self.meter_frame(left[i], right[i], left[i], right[i]);
            }
            self.phase = (self.phase + n) % UPDATE;
            self.clock += n as u64;
            return;
        }
        let mut i = 0;
        while i < n {
            if self.phase == 0 {
                self.update();
            }
            let len = (UPDATE - self.phase).min(n - i);
            for j in i..i + len {
                let k = key.map(|(a, b)| {
                    (
                        a.get(j).copied().unwrap_or(0.0),
                        b.get(j).copied().unwrap_or(0.0),
                    )
                });
                let (yl, yr) = self.frame(left[j], right[j], k);
                left[j] = yl;
                right[j] = yr;
                self.phase += 1;
            }
            self.phase %= UPDATE;
            i += len;
        }
    }

    fn meter_frame(&mut self, xl: f32, xr: f32, yl: f32, yr: f32) {
        let w = &mut self.hold[1];
        w.input[0] = w.input[0].max(xl.abs());
        w.input[1] = w.input[1].max(xr.abs());
        w.output[0] = w.output[0].max(yl.abs());
        w.output[1] = w.output[1].max(yr.abs());
        self.hold_pos += 1;
        if self.hold_pos == HOLD {
            self.hold_pos = 0;
            self.hold[0] = self.hold[1];
            self.hold[1] = HoldWindow::default();
        }
    }

    /// One stereo frame through everything.
    #[inline]
    fn frame(&mut self, xl: f32, xr: f32, key: Option<(f32, f32)>) -> (f32, f32) {
        let sr = self.sr;
        let n = self.lay.n;
        let alpha = (self.phase + 1) as f32 / UPDATE as f32;
        let listen = self.s.listen;

        // ---- detectors, gains, lookahead ----
        self.quiet = if xl == 0.0 && xr == 0.0 {
            self.quiet.saturating_add(1)
        } else {
            0
        };
        let mut unity = true;
        let mut key_out = (0.0f32, 0.0f32);
        let pos_w = self.ring_pos;
        let rl = self.ring_len;
        let gp = self.gain_pos;
        let gl = self.gain_len;
        for pos in 0..n {
            let b = self.lay.order[pos];
            let live = &self.live[b];
            let band = &live.b;
            let (kl, kr) = match key {
                Some(k) if band.key_ext => k,
                _ => (xl, xr),
            };
            let d = &mut self.det[b];
            let (level, heard) = match band.placement {
                Placement::Stereo => {
                    let fl = d.filter(0, kl);
                    let fr = d.filter(1, kr);
                    (fl.abs().max(fr.abs()), (fl, fr))
                }
                Placement::Mid => {
                    let m = d.filter(0, (kl + kr) * 0.5);
                    (m.abs(), (m, m))
                }
                Placement::Side => {
                    let s = d.filter(0, (kl - kr) * 0.5);
                    (s.abs(), (s, -s))
                }
            };
            let c = if level > d.env { live.att } else { live.rel };
            d.env = level + (d.env - level) * c;
            if d.env < 1e-12 {
                d.env = 0.0;
            }
            let level_db = if d.env > 0.0 {
                lin_to_db(d.env)
            } else {
                FLOOR_DB
            };
            let audible = self.audible[b];
            if listen == Listen::Key && audible > 0.0 {
                key_out.0 += heard.0 * audible;
                key_out.1 += heard.1 * audible;
            }
            // A band switched on or off fades its whole gain in or out over 5 ms; fully
            // off is exactly 0 dB, so a bypassed band touches nothing.
            let on_t = if band.on { 1.0 } else { 0.0 };
            let on = &mut self.on_amt[b];
            if *on < on_t {
                *on = (*on + self.astep).min(on_t);
            } else if *on > on_t {
                *on = (*on - self.astep).max(on_t);
            }
            let on = *on;
            let (dyn_db, g) = if on == 0.0 {
                (0.0, 1.0)
            } else {
                let dyn_db = if live.moves {
                    gain_db(level_db, band) * on
                } else {
                    0.0
                };
                (dyn_db, db_to_lin(dyn_db + band.gain_db * on))
            };
            // Each band looks ahead by its own amount: the audio is late by the largest,
            // this band's gain by the difference.
            let delay = self.total - self.la[b].min(self.la_max);
            let at = (gp + gl - delay) % gl;
            unity &= g == 1.0 && dyn_db == 0.0;
            let ring = &mut self.gain_ring[b * gl..(b + 1) * gl];
            ring[gp] = g;
            self.g_now[b] = ring[at];
            let ring = &mut self.dyn_ring[b * gl..(b + 1) * gl];
            ring[gp] = dyn_db;
            self.dyn_now[b] = ring[at];
            let hw = &mut self.hold[1];
            if self.dyn_now[b].abs() > hw.gain[b].abs() {
                hw.gain[b] = self.dyn_now[b];
            }
            hw.level[b] = hw.level[b].max(level_db);
        }
        self.unity_run = if unity {
            self.unity_run.saturating_add(1)
        } else {
            0
        };
        // ---- the audio, late by the lookahead ----
        let (dl, dr) = {
            self.audio_ring[0][pos_w] = xl;
            self.audio_ring[1][pos_w] = xr;
            let at = (pos_w + rl - self.la_max) % rl;
            (self.audio_ring[0][at], self.audio_ring[1][at])
        };
        self.ring_pos = (pos_w + 1) % rl;
        self.gain_pos = (gp + 1) % gl;

        // ---- solo / mute, and which path ----
        let astep = 1.0 / (AUDIBLE_MS * 0.001 * sr);
        for b in 0..MAX_BANDS {
            let t = self.audible_t[b];
            let a = &mut self.audible[b];
            if *a < t {
                *a = (*a + astep).min(t);
            } else if *a > t {
                *a = (*a - astep).max(t);
            }
        }
        let delta = listen == Listen::Delta;
        let ((mut yl, mut yr), (dl_out, dr_out)) = if self.s.mode == Mode::Linear && n > 0 {
            // The linear split is a true split with no phase to hide: solo and mute are
            // its own gains, and it has no other path to fade to.
            if !self.lin_live {
                self.lin.reset();
                self.lin_live = true;
                self.lin_origin = (self.clock % self.lin.block_pos().1 as u64) as usize;
            }
            self.split_live = false;
            self.dyn_live = false;
            self.linear_path(dl, dr)
        } else {
            self.lin_live = false;
            let wt = self.split_target();
            let wstep = 1.0 / (FADE_MS * 0.001 * sr);
            if self.w < wt {
                self.w = (self.w + wstep).min(wt);
            } else if self.w > wt {
                self.w = (self.w - wstep).max(wt);
            }
            let need_split = self.w > 0.0 || wt > 0.0;
            let need_dyn = self.w < 1.0 || wt < 1.0;
            if need_split && !self.split_live {
                self.split = [[Xover::default(); MAX_XOVERS]; 2];
                self.split_delta = [[Xover::default(); MAX_XOVERS]; 2];
            }
            self.split_live = need_split;
            if need_dyn && !self.dyn_live {
                self.shelves = [[Shelf::default(); MAX_XOVERS]; 2];
            }
            self.dyn_live = need_dyn;
            let mut y_dyn = (dl, dr);
            let mut d_dyn = (0.0, 0.0);
            if need_dyn {
                y_dyn = self.dynamic_path(dl, dr, alpha);
                if delta {
                    d_dyn = (dl - y_dyn.0, dr - y_dyn.1);
                }
            }
            let mut y_split = (0.0, 0.0);
            let mut d_split = (0.0, 0.0);
            if need_split {
                let (y, d) = self.split_path(dl, dr, delta);
                y_split = y;
                d_split = d;
            }
            let w = self.w;
            let mix = |a: (f32, f32), b: (f32, f32)| -> (f32, f32) {
                if w <= 0.0 {
                    a
                } else if w >= 1.0 {
                    b
                } else {
                    (a.0 + (b.0 - a.0) * w, a.1 + (b.1 - a.1) * w)
                }
            };
            (mix(y_dyn, y_split), mix(d_dyn, d_split))
        };
        match listen {
            Listen::Output => {
                let out = self.out_from + (self.out_to - self.out_from) * alpha;
                if out != 1.0 {
                    yl *= out;
                    yr *= out;
                }
            }
            Listen::Delta => {
                yl = dl_out;
                yr = dr_out;
            }
            Listen::Key => {
                yl = key_out.0;
                yr = key_out.1;
            }
        }
        self.meter_frame(xl, xr, yl, yr);
        self.clock += 1;
        (yl, yr)
    }

    /// The dynamic path: the first band's gain, through a shelf per crossover.
    #[inline]
    fn dynamic_path(&mut self, xl: f32, xr: f32, alpha: f32) -> (f32, f32) {
        let steps = self.lay.n.saturating_sub(1);
        let slope = self.s.slope;
        let still = |h: usize, s: &Self| -> bool {
            s.g0_from[h] == s.g0_to[h]
                && (0..steps).all(|i| s.stage_from[h][i].1 == 1.0 && s.stage_to[h][i].1 == 1.0)
        };
        let (still_m, still_s) = (still(0, self), still(1, self));
        let m = (xl + xr) * 0.5;
        let sd = (xl - xr) * 0.5;
        if still_m && still_s {
            // No shelf acts: keep their state warm on the input and apply a plain gain —
            // exactly the input when every band is at unity.
            for (h, v) in [(0usize, m), (1usize, sd)] {
                for i in 0..steps {
                    let (g0, _) = self.stage_from[h][i];
                    let (g1, _) = self.stage_to[h][i];
                    let g = g0 + (g1 - g0) * alpha;
                    self.shelves[h][i].tick(slope, g, 1.0, v, false);
                }
            }
            let (gm, gs) = (self.g0_to[0], self.g0_to[1]);
            if gm == gs {
                return if gm == 1.0 {
                    (xl, xr)
                } else {
                    (xl * gm, xr * gm)
                };
            }
            let (m, sd) = (m * gm, sd * gs);
            return (m + sd, m - sd);
        }
        let mut out = [m, sd];
        for (h, v) in out.iter_mut().enumerate() {
            for i in 0..steps {
                let (g0, r0) = self.stage_from[h][i];
                let (g1, r1) = self.stage_to[h][i];
                let g = g0 + (g1 - g0) * alpha;
                let rho = r0 + (r1 - r0) * alpha;
                let mix = !(r0 == 1.0 && r1 == 1.0);
                *v = self.shelves[h][i].tick(slope, g, rho, *v, mix);
            }
            let g0 = self.g0_from[h] + (self.g0_to[h] - self.g0_from[h]) * alpha;
            *v *= g0;
        }
        (out[0] + out[1], out[0] - out[1])
    }

    /// The split: bands through a Linkwitz–Riley tree, each at its gain (and placement),
    /// summed with the all-pass compensation that keeps the sum flat. Also the delta's sum
    /// when asked for.
    #[inline]
    fn split_path(&mut self, xl: f32, xr: f32, delta: bool) -> ((f32, f32), (f32, f32)) {
        let n = self.lay.n;
        if n == 0 {
            return ((xl, xr), (0.0, 0.0));
        }
        let mut bands = [[0.0f32; MAX_BANDS]; 2];
        for (ch, x) in [xl, xr].into_iter().enumerate() {
            let mut rest = x;
            let pairs = self.split[ch].iter_mut().zip(&self.xo).take(n - 1);
            for (band, (state, coefs)) in bands[ch].iter_mut().zip(pairs) {
                let (lo, hi) = state.split(coefs, rest);
                *band = lo;
                rest = hi;
            }
            bands[ch][n - 1] = rest;
        }
        // Gains, placement, solo and mute.
        let mut dbands = [[0.0f32; MAX_BANDS]; 2];
        for pos in 0..n {
            let b = self.lay.order[pos];
            let g = self.g_now[b];
            let (l, r) = (bands[0][pos], bands[1][pos]);
            let (nl, nr) = match self.s.bands[b].placement {
                Placement::Stereo => (l * g, r * g),
                Placement::Mid => {
                    let m = (l + r) * 0.5 * (g - 1.0);
                    (l + m, r + m)
                }
                Placement::Side => {
                    let s = (l - r) * 0.5 * (g - 1.0);
                    (l + s, r - s)
                }
            };
            // Solo and mute take the whole band, whatever part of the image it acts on.
            let a = self.audible[b];
            let (nl, nr) = if a == 1.0 { (nl, nr) } else { (nl * a, nr * a) };
            if delta {
                dbands[0][pos] = l - nl;
                dbands[1][pos] = r - nr;
            }
            bands[0][pos] = nl;
            bands[1][pos] = nr;
        }
        let mut y = [0.0f32; 2];
        let mut d = [0.0f32; 2];
        for ch in 0..2 {
            let mut acc = bands[ch][0];
            let mut dacc = dbands[ch][0];
            // Before band k joins, everything below it goes through crossover k's
            // all-pass: the bands under a crossover never went through it.
            for k in 1..n {
                if k + 1 < n {
                    acc = self.split[ch][k].allpass(&self.xo[k], acc);
                    if delta {
                        dacc = self.split_delta[ch][k].allpass(&self.xo[k], dacc);
                    }
                }
                acc += bands[ch][k];
                dacc += dbands[ch][k];
            }
            y[ch] = acc;
            d[ch] = dacc;
        }
        ((y[0], y[1]), (d[0], d[1]))
    }
}

impl Multiplex {
    /// The linear-phase split: the input late by the filters, times the top band's gain,
    /// plus each crossover's low-pass times the difference of the gains either side of it
    /// (see `linear`). At unity every difference is exactly zero. Returns the output and
    /// the delta (the late input minus the output).
    #[inline]
    fn linear_path(&mut self, xl: f32, xr: f32) -> ((f32, f32), (f32, f32)) {
        let n = self.lay.n;
        let steps = n.saturating_sub(1);
        if self.lin.at_block_start() {
            for i in 0..steps {
                self.lin.want(i, self.xo_hz[i], self.s.slope);
            }
        }
        let (tl, tr) = self.lin.frame(xl, xr, steps);
        if n == 0 {
            return ((tl, tr), (0.0, 0.0));
        }
        // Per part of the image, the gains in frequency order.
        let mut g = [[1.0f32; MAX_BANDS]; 2];
        for pos in 0..n {
            let b = self.lay.order[pos];
            let a = self.audible[b];
            for (h, gh) in g.iter_mut().enumerate() {
                let own = if self.s.bands[b].placement.applies(h) {
                    self.g_now[b]
                } else {
                    1.0
                };
                gh[pos] = own * a;
            }
        }
        let top = [g[0][n - 1], g[1][n - 1]];
        let flat = (0..steps).all(|c| g[0][c] == g[0][c + 1] && g[1][c] == g[1][c + 1]);
        let y = if flat && top[0] == top[1] {
            // Every band alike: the late input times one gain — to the bit at unity.
            if top[0] == 1.0 {
                (tl, tr)
            } else {
                (tl * top[0], tr * top[0])
            }
        } else {
            let mut ms = [(tl + tr) * 0.5 * top[0], (tl - tr) * 0.5 * top[1]];
            for (h, v) in ms.iter_mut().enumerate() {
                for c in 0..steps {
                    let d = g[h][c] - g[h][c + 1];
                    if d != 0.0 {
                        *v += d * self.lin.low(c, h);
                    }
                }
            }
            (ms[0] + ms[1], ms[0] - ms[1])
        };
        (y, (tl - y.0, tr - y.1))
    }
}

/// Glide `cur` toward `target` by one update; true if it moved.
fn glide(cur: &mut f32, target: f32, log: bool) -> bool {
    if *cur == target {
        return false;
    }
    let next = if log && *cur > 0.0 && target > 0.0 {
        (cur.ln() + (target.ln() - cur.ln()) * GLIDE).exp()
    } else {
        *cur + (target - *cur) * GLIDE
    };
    *cur = if (next - target).abs() <= target.abs().max(1.0) * 1e-4 {
        target
    } else {
        next
    };
    true
}

#[inline]
fn db_to_lin(db: f32) -> f32 {
    if db == 0.0 {
        1.0
    } else {
        // 10^(dB/20) as one exp: cheaper than powf, and the same to the last bits that
        // matter here.
        (db * (std::f32::consts::LN_10 / 20.0)).exp()
    }
}

#[inline]
fn lin_to_db(x: f32) -> f32 {
    if x > 1e-9 {
        (x.ln() * (20.0 / std::f32::consts::LN_10)).max(FLOOR_DB)
    } else {
        FLOOR_DB
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn noise(n: usize, amp: f32, seed: u32) -> Vec<f32> {
        let mut s = seed.max(1);
        (0..n)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                amp * ((s as f32 / u32::MAX as f32) * 2.0 - 1.0)
            })
            .collect()
    }

    fn sine(n: usize, hz: f32, amp: f32) -> Vec<f32> {
        (0..n)
            .map(|i| amp * (std::f32::consts::TAU * hz * i as f32 / SR).sin())
            .collect()
    }

    /// Bands starting at `froms` (the first one's is ignored), each shaped by `f`.
    fn settings(
        mode: Mode,
        slope: Slope,
        froms: &[f32],
        f: impl Fn(usize, &mut BandSettings),
    ) -> Settings {
        let mut s = Settings {
            mode,
            slope,
            ..Settings::default()
        };
        for (i, from) in froms.iter().enumerate() {
            s.bands[i].used = true;
            s.bands[i].from_hz = *from;
            f(i, &mut s.bands[i]);
        }
        s
    }

    fn unity(_: usize, b: &mut BandSettings) {
        b.ratio = 1.0;
        b.ratio_lo = 1.0;
    }

    fn run(s: &Settings, l: &[f32], r: &[f32], block: usize) -> (Vec<f32>, Vec<f32>) {
        run_keyed(s, l, r, None, block)
    }

    fn run_keyed(
        s: &Settings,
        l: &[f32],
        r: &[f32],
        key: Option<(&[f32], &[f32])>,
        block: usize,
    ) -> (Vec<f32>, Vec<f32>) {
        let mut m = Multiplex::new(SR);
        m.set(s);
        let (mut l, mut r) = (l.to_vec(), r.to_vec());
        let mut i = 0;
        while i < l.len() {
            let e = (i + block).min(l.len());
            let k = key.map(|(a, b)| (&a[i..e], &b[i..e]));
            m.process(&mut l[i..e], &mut r[i..e], k);
            i = e;
        }
        (l, r)
    }

    fn rms_db(x: &[f32]) -> f32 {
        let ms = x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32;
        10.0 * ms.max(1e-30).log10()
    }

    fn peak(x: &[f32]) -> f32 {
        x.iter().fold(0.0, |m, v| m.max(v.abs()))
    }

    /// Nothing that can act, or nothing that does: the output is the input, to the bit.
    #[test]
    fn a_multiplex_that_does_nothing_is_a_wire_to_the_bit() {
        let src = noise(20_000, 0.8, 3);
        let src_r = noise(20_000, 0.6, 9);
        let froms = [20.0, 150.0, 1_200.0, 6_000.0];
        // No bands at all, in any mode.
        for mode in Mode::ALL {
            let s = Settings {
                mode,
                ..Settings::default()
            };
            let (l, r) = run(&s, &src, &src_r, 256);
            assert!(
                l == src && r == src_r,
                "an empty Multiplex changed the signal"
            );
            let mut m = Multiplex::new(SR);
            m.set(&s);
            assert_eq!(m.latency(), 0, "{mode:?}: no bands, no latency");
        }
        for slope in Slope::ALL {
            for mode in [Mode::Dynamic, Mode::Split] {
                // Every band at unity: in either mode nothing can act, so nothing runs.
                let s = settings(mode, slope, &froms, unity);
                let (l, r) = run(&s, &src, &src_r, 256);
                assert!(l == src && r == src_r, "{mode:?} {slope:?} at unity");
            }
            // Dynamic: bands that could act but do not (a quiet signal under every
            // threshold) leave it alone too — the split would be an all-pass here.
            let quiet = noise(20_000, 0.01, 5);
            let s = settings(Mode::Dynamic, slope, &froms, |_, b| {
                b.thr_db = -12.0;
                b.ratio = 4.0;
                b.knee_db = 0.0;
            });
            let (l, r) = run(&s, &quiet, &quiet, 256);
            assert!(l == quiet && r == quiet, "dynamic {slope:?}: a band moved");
        }
    }

    /// The split's bands at unity sum to an all-pass: flat magnitude, to within 0.001 dB
    /// from 20 Hz to 20 kHz, for every slope and eight bands.
    #[test]
    fn the_split_at_unity_sums_to_an_all_pass() {
        let froms = [
            20.0, 60.0, 180.0, 500.0, 1_400.0, 3_500.0, 8_000.0, 14_000.0,
        ];
        for slope in Slope::ALL {
            // Bands that can act (so the split runs) on a signal far under the threshold.
            let s = settings(Mode::Split, slope, &froms, |_, b| {
                b.thr_db = 0.0;
                b.knee_db = 0.0;
            });
            let n = 1 << 15;
            let mut imp = vec![0.0f32; n];
            imp[0] = 1e-3;
            let (l, _) = run(&s, &imp, &imp, 512);
            // The response at 160 frequencies from 20 Hz to 20 kHz, by a DFT in f64 (a
            // 32k-point FFT in f32 is itself a few thousandths of a dB out).
            let mut worst = 0.0f32;
            let mut at = 0.0f32;
            for i in 0..160 {
                let hz = 20.0 * 1_000f64.powf(i as f64 / 159.0);
                let w = std::f64::consts::TAU * hz / SR as f64;
                let (mut re, mut im) = (0.0f64, 0.0f64);
                for (t, v) in l.iter().enumerate() {
                    let (sn, cs) = (w * t as f64).sin_cos();
                    re += *v as f64 * 1e3 * cs;
                    im -= *v as f64 * 1e3 * sn;
                }
                let db = (10.0 * (re * re + im * im).log10()) as f32;
                if db.abs() > worst {
                    worst = db.abs();
                    at = hz as f32;
                }
            }
            if std::env::var("MB_REPORT").is_ok() {
                eprintln!("split sum {slope:?}: worst {worst:.6} dB at {at:.0} Hz");
            }
            assert!(
                worst < 1e-3,
                "{slope:?}: the sum is {worst} dB off flat at {at} Hz"
            );
            if slope != Slope::S6 {
                // An all-pass, not a wire: the energy is spread in time.
                assert!(l[0].abs() < 0.99e-3, "{slope:?}");
            }
        }
    }

    /// The linear-phase split is a **delayed** wire at unity — to the bit — its latency is
    /// its filters' centre plus a block, and all bands at one gain are that gain, delayed.
    #[test]
    fn the_linear_split_is_a_delayed_wire_at_unity() {
        let src = noise(20_000, 0.8, 71);
        let src_r = noise(20_000, 0.6, 73);
        let froms = [20.0, 150.0, 1_200.0, 6_000.0];
        for slope in Slope::ALL {
            // Bands that can act, on a signal under every threshold: the filters run, and
            // every difference of gains is zero.
            let s = settings(Mode::Linear, slope, &froms, |_, b| {
                b.thr_db = 0.0;
                b.knee_db = 0.0;
            });
            let mut m = Multiplex::new(SR);
            m.set(&s);
            let d = m.latency();
            assert_eq!(d, 2048 + 256, "4096 taps and a 256-sample block at 48 kHz");
            let quiet: Vec<f32> = src.iter().map(|v| v * 0.001).collect();
            let quiet_r: Vec<f32> = src_r.iter().map(|v| v * 0.001).collect();
            let (l, r) = run(&s, &quiet, &quiet_r, 100);
            assert!(l[..d].iter().chain(&r[..d]).all(|v| *v == 0.0));
            assert_eq!(&l[d..], &quiet[..quiet.len() - d], "{slope:?}: left");
            assert_eq!(&r[d..], &quiet_r[..quiet_r.len() - d], "{slope:?}: right");
        }
        let s = settings(Mode::Linear, Slope::S24, &froms, |_, b| {
            unity(0, b);
            b.gain_db = -6.0;
        });
        let (l, _) = run(&s, &src, &src, 333);
        let g = db_to_lin(-6.0);
        let d = 2048 + 256;
        for (y, x) in l[d..].iter().zip(&src) {
            assert_eq!(*y, *x * g);
        }
    }

    /// Linear phase, measured: with bands at different gains the impulse response is
    /// symmetric about the latency.
    #[test]
    fn the_linear_split_has_a_symmetric_impulse_response() {
        let s = settings(Mode::Linear, Slope::S24, &[20.0, 400.0, 3_000.0], |i, b| {
            unity(i, b);
            b.gain_db = [-6.0, 4.0, -9.0][i];
        });
        let mut m = Multiplex::new(SR);
        m.set(&s);
        let d = m.latency();
        let n = d * 3;
        let mut imp = vec![0.0f32; n];
        imp[0] = 1.0;
        let (h, _) = run(&s, &imp, &imp, 512);
        let peak = h.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        let worst = (1..2_000)
            .map(|k| (h[d + k] - h[d - k]).abs())
            .fold(0.0f32, f32::max);
        assert!(
            worst <= peak * 1e-4,
            "asymmetry {worst} against a peak of {peak}"
        );
    }

    /// The same bands, all at the same gain, are a plain gain in the dynamic path — to the
    /// bit, with no phase shift anywhere.
    #[test]
    fn equal_gains_in_every_band_are_a_pure_gain() {
        let src = noise(8_000, 0.5, 11);
        let s = settings(
            Mode::Dynamic,
            Slope::S48,
            &[20.0, 200.0, 2_000.0],
            |_, b| {
                unity(0, b);
                b.gain_db = -6.0;
            },
        );
        let (l, _) = run(&s, &src, &src, 100);
        let g = db_to_lin(-6.0);
        assert!((g - 0.501_187).abs() < 1e-6);
        for (y, x) in l.iter().zip(&src) {
            assert_eq!(*y, *x * g);
        }
    }

    /// What is drawn is what is heard: a band's static gain, measured with sines against
    /// [`response_db`], within 0.05 dB, in both modes and every slope.
    #[test]
    fn static_band_gains_match_the_drawn_curve() {
        for mode in Mode::ALL {
            for slope in Slope::ALL {
                let s = settings(mode, slope, &[20.0, 300.0, 3_000.0], |i, b| {
                    unity(i, b);
                    b.gain_db = [2.0, -9.0, 4.0][i];
                });
                let db = [2.0, -9.0, 4.0, 0.0, 0.0, 0.0, 0.0, 0.0];
                for hz in [60.0f32, 250.0, 300.0, 700.0, 3_000.0, 9_000.0] {
                    let x = sine(24_000, hz, 0.25);
                    let (l, _) = run(&s, &x, &x, 333);
                    let measured = rms_db(&l[12_000..]) - rms_db(&x[12_000..]);
                    let drawn = response_db(&s, &db, SR, hz);
                    assert!(
                        (measured - drawn).abs() < 0.05,
                        "{mode:?} {slope:?} at {hz} Hz: measured {measured:.3}, drawn {drawn:.3}"
                    );
                }
            }
        }
    }

    /// Cutting one band of two leaves no bump at the crossover in either mode: the response
    /// runs monotonically from the cut to 0 dB. (The benchmark's dynamic mode is said to
    /// bump here; this is the number we publish instead.)
    #[test]
    fn a_cut_next_to_a_flat_band_has_no_bump() {
        for mode in [Mode::Dynamic, Mode::Split] {
            for slope in Slope::ALL {
                let s = settings(mode, slope, &[20.0, 1_000.0], unity);
                let db = [-6.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
                let mut last = -100.0f32;
                let mut hz = 20.0f32;
                while hz <= 20_000.0 {
                    let v = response_db(&s, &db, SR, hz);
                    assert!(v >= last - 1e-3, "{mode:?} {slope:?}: falls at {hz} Hz");
                    assert!(
                        (-6.01..=0.01).contains(&v),
                        "{mode:?} {slope:?}: {v} at {hz}"
                    );
                    last = v;
                    hz *= 1.05;
                }
                if std::env::var("MB_REPORT").is_ok() {
                    eprintln!(
                        "{mode:?} {slope:?}: at the crossover {:.2} dB, an octave below {:.2}, above {:.2}",
                        response_db(&s, &db, SR, 1_000.0),
                        response_db(&s, &db, SR, 500.0),
                        response_db(&s, &db, SR, 2_000.0)
                    );
                }
            }
        }
    }

    /// The four behaviours of a band, each measured on a steady tone against the static
    /// curve: downward and upward compression, downward and upward expansion.
    #[test]
    fn each_side_of_a_band_does_what_its_ratio_says() {
        type Shape = fn(&mut BandSettings);
        let down_comp: Shape = |b| {
            b.thr_db = -20.0;
            b.ratio = 4.0;
        };
        let up_exp: Shape = |b| {
            b.thr_db = -20.0;
            b.ratio = 0.5;
            b.range_db = 30.0;
        };
        let up_comp: Shape = |b| {
            b.ratio = 1.0;
            b.thr_lo_db = -30.0;
            b.ratio_lo = 2.0;
        };
        let down_exp: Shape = |b| {
            b.ratio = 1.0;
            b.thr_lo_db = -30.0;
            b.ratio_lo = 0.5;
        };
        let cases = [
            ("down-comp", 0.5, down_comp, -10.5),
            ("up-exp", 0.5, up_exp, 14.0),
            ("up-comp", 0.01, up_comp, 5.0),
            ("down-exp", 0.01, down_exp, -10.0),
        ];
        for (name, amp, shape, want) in cases {
            for mode in Mode::ALL {
                let s = settings(mode, Slope::S24, &[20.0], |_, b| {
                    b.knee_db = 0.0;
                    b.attack_ms = 1.0;
                    b.release_ms = 2_000.0;
                    shape(b);
                });
                let x = sine(48_000, 1_000.0, amp);
                let (l, _) = run(&s, &x, &x, 256);
                let got = 20.0 * (peak(&l[36_000..]) / peak(&x[36_000..])).log10();
                assert!(
                    (got - want).abs() < 0.3,
                    "{name} {mode:?}: {got:.2} dB, the curve says {want}"
                );
            }
        }
    }

    /// Attack and release are time constants in milliseconds: after one release time of
    /// silence the detector has fallen 8.69 dB (1/e), after one attack time it has risen to
    /// 1 − 1/e.
    #[test]
    fn attack_and_release_are_milliseconds() {
        let s = settings(Mode::Dynamic, Slope::S24, &[20.0], |_, b| {
            b.attack_ms = 5.0;
            b.release_ms = 100.0;
        });
        let mut m = Multiplex::new(SR);
        m.set(&s);
        let attack = (5.0 * 0.001 * SR) as usize;
        let (mut l, mut r) = (vec![1.0f32; attack], vec![1.0f32; attack]);
        m.process(&mut l, &mut r, None);
        let risen = 10f32.powf(m.levels_now_db()[0] / 20.0);
        assert!(
            (risen - (1.0 - (-1.0f32).exp())).abs() < 0.01,
            "attack: {risen}"
        );
        let (mut l, mut r) = (vec![1.0f32; 48_000], vec![1.0f32; 48_000]);
        m.process(&mut l, &mut r, None);
        let release = (100.0 * 0.001 * SR) as usize;
        let (mut l, mut r) = (vec![0.0f32; release], vec![0.0f32; release]);
        m.process(&mut l, &mut r, None);
        let fallen = m.levels_now_db()[0];
        assert!((fallen + 8.686).abs() < 0.05, "release: {fallen} dB");
    }

    /// Lookahead: the audio is late by exactly the latency, and the gain arrives before the
    /// loud part does.
    #[test]
    fn lookahead_delays_the_audio_and_leads_the_gain() {
        let s = settings(Mode::Dynamic, Slope::S24, &[20.0, 1_000.0], |_, b| {
            unity(0, b);
            b.lookahead_ms = 5.0;
        });
        let mut m = Multiplex::new(SR);
        m.set(&s);
        assert_eq!(m.latency(), 240);
        let src = noise(4_000, 0.5, 21);
        let (l, _) = run(&s, &src, &src, 64);
        assert!(l[..240].iter().all(|v| *v == 0.0));
        assert_eq!(&l[240..], &src[..4_000 - 240], "a delayed wire, to the bit");

        // A quiet tone that jumps to loud: with lookahead the reduction is already there
        // when the loud part comes out.
        let s = settings(Mode::Split, Slope::S24, &[20.0], |_, b| {
            b.thr_db = -30.0;
            b.ratio = 10.0;
            b.range_db = 40.0;
            b.knee_db = 0.0;
            b.attack_ms = 0.5;
            b.lookahead_ms = 5.0;
        });
        let mut x = sine(9_600, 200.0, 0.01);
        for (i, v) in x.iter_mut().enumerate().skip(4_800) {
            *v = 0.9 * (std::f32::consts::TAU * 200.0 * i as f32 / SR).sin();
        }
        let (l, _) = run(&s, &x, &x, 128);
        let onset = 4_800 + 240;
        let first = peak(&l[onset..onset + 120]);
        assert!(first < 0.9 * 0.5, "the gain came late: {first}");
    }

    #[test]
    fn the_result_does_not_depend_on_the_block_size() {
        let l = noise(24_000, 0.7, 31);
        let r: Vec<f32> = noise(24_000, 0.5, 37)
            .iter()
            .zip(&l)
            .map(|(a, b)| a * 0.5 + b * 0.5)
            .collect();
        let key = sine(24_000, 90.0, 0.8);
        for mode in Mode::ALL {
            for listen in [Listen::Output, Listen::Delta, Listen::Key] {
                let mut s = settings(mode, Slope::S48, &[20.0, 120.0, 900.0, 5_000.0], |i, b| {
                    b.thr_db = -30.0 + 4.0 * i as f32;
                    b.ratio = [3.0, 0.7, 2.0, 5.0][i];
                    b.thr_lo_db = -50.0;
                    b.ratio_lo = [1.0, 2.0, 0.5, 1.5][i];
                    b.attack_ms = [0.1, 3.0, 10.0, 1.0][i];
                    b.lookahead_ms = [0.0, 3.0, 1.5, 0.0][i];
                    b.placement = Placement::from_index(i % 3);
                    b.key_ext = i == 0;
                    b.key_free = i == 3;
                    b.gain_db = i as f32 - 1.5;
                });
                s.listen = listen;
                s.output_db = -1.0;
                s.bands[2].solo = listen == Listen::Key;
                let keyed = Some((&key[..], &key[..]));
                let reference = run_keyed(&s, &l, &r, keyed, 512);
                assert!(reference
                    .0
                    .iter()
                    .chain(&reference.1)
                    .all(|v| v.is_finite()));
                for block in [1usize, 7, 64, 1000, 4096] {
                    assert!(
                        run_keyed(&s, &l, &r, keyed, block) == reference,
                        "{mode:?} {listen:?}: block {block} changed the output"
                    );
                }
            }
        }
    }

    #[test]
    fn solo_and_mute_are_heard_through_the_split() {
        let low = sine(24_000, 100.0, 0.5);
        for mode in Mode::ALL {
            let mut s = settings(mode, Slope::S24, &[20.0, 1_000.0], unity);
            s.bands[1].solo = true;
            let (l, _) = run(&s, &low, &low, 256);
            let left = rms_db(&l[12_000..]) - rms_db(&low[12_000..]);
            assert!(
                left < -40.0,
                "{mode:?}: soloing the high band left {left} dB of the low"
            );
            let mut s = settings(mode, Slope::S24, &[20.0, 1_000.0], unity);
            s.bands[0].mute = true;
            let (l, _) = run(&s, &low, &low, 256);
            let left = rms_db(&l[12_000..]) - rms_db(&low[12_000..]);
            assert!(left < -40.0, "{mode:?}: muting the low band left {left} dB");
            // Solo the band the tone is in: it is all there.
            let mut s = settings(mode, Slope::S24, &[20.0, 1_000.0], unity);
            s.bands[0].solo = true;
            let (l, _) = run(&s, &low, &low, 256);
            let left = rms_db(&l[12_000..]) - rms_db(&low[12_000..]);
            assert!(left.abs() < 0.1, "{mode:?}: the soloed band lost {left} dB");
        }
    }

    /// A band keyed from the sidechain ducks when the key is loud; its neighbour, which
    /// listens to itself, does not move.
    #[test]
    fn a_key_moves_only_the_bands_that_listen_to_it() {
        // The signal is well under the threshold; the key is well over it.
        let x = noise(24_000, 0.005, 41);
        let key = sine(24_000, 80.0, 0.9);
        let s = settings(Mode::Dynamic, Slope::S24, &[20.0, 500.0], |i, b| {
            b.thr_db = -30.0;
            b.ratio = 8.0;
            b.range_db = 24.0;
            b.key_ext = i == 0;
        });
        let mut m = Multiplex::new(SR);
        m.set(&s);
        let (mut l, mut r) = (x.clone(), x.clone());
        m.process(&mut l, &mut r, Some((&key, &key)));
        let g = m.gains_now_db();
        assert!(g[0] < -10.0, "the keyed band did not duck: {}", g[0]);
        assert_eq!(g[1], 0.0, "the other band moved: {}", g[1]);
    }

    /// A side-only band leaves a mono signal alone, however hard it works.
    #[test]
    fn a_side_band_leaves_the_mid_alone() {
        let x = noise(12_000, 0.8, 43);
        for mode in [Mode::Dynamic, Mode::Split] {
            let s = settings(mode, Slope::S24, &[20.0], |_, b| {
                b.placement = Placement::Side;
                b.gain_db = -12.0;
                b.ratio = 10.0;
                b.thr_db = -60.0;
            });
            let (l, r) = run(&s, &x, &x, 256);
            let err = l
                .iter()
                .zip(&x)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f32::max);
            assert!(err < 1e-6, "{mode:?}: the mid moved by {err}");
            assert_eq!(l, r);
        }
    }

    /// The delta is what the bands change: output plus delta is the path at unity.
    #[test]
    fn delta_is_what_the_bands_change() {
        let x = noise(12_000, 0.7, 47);
        for mode in Mode::ALL {
            let shape = |_: usize, b: &mut BandSettings| {
                b.thr_db = -30.0;
                b.ratio = 4.0;
            };
            let mut s = settings(mode, Slope::S24, &[20.0, 400.0, 4_000.0], shape);
            let (out, _) = run(&s, &x, &x, 256);
            s.listen = Listen::Delta;
            let (delta, _) = run(&s, &x, &x, 256);
            // The same path with nothing over the threshold.
            let u = settings(mode, Slope::S24, &[20.0, 400.0, 4_000.0], |_, b| {
                b.thr_db = 0.0;
                b.ratio = 4.0;
                b.knee_db = 0.0;
            });
            let (unity_path, _) = run(&u, &x, &x, 256);
            let err = (0..x.len())
                .map(|i| (out[i] + delta[i] - unity_path[i]).abs())
                .fold(0.0, f32::max);
            assert!(
                err < 1e-5,
                "{mode:?}: output + delta is {err} from the unity path"
            );
        }
    }

    /// Loud, then silence: nothing becomes a NaN, the states flush, and the output lands on
    /// exact zero instead of crawling through denormals.
    #[test]
    fn silence_after_noise_is_exact_silence() {
        for mode in Mode::ALL {
            let s = settings(
                mode,
                Slope::S48,
                &[20.0, 100.0, 1_000.0, 10_000.0],
                |_, b| {
                    b.thr_db = -40.0;
                    b.ratio = 3.0;
                    b.ratio_lo = 2.0;
                    b.thr_lo_db = -50.0;
                },
            );
            let mut x = noise(48_000, 1.0, 53);
            x.extend(std::iter::repeat_n(0.0, 48_000 * 20));
            let (l, r) = run(&s, &x, &x, 4096);
            assert!(l.iter().chain(&r).all(|v| v.is_finite()));
            assert!(
                l[l.len() - 1_000..].iter().all(|v| *v == 0.0),
                "{mode:?}: the tail never reached zero"
            );
        }
    }

    #[test]
    fn two_runs_are_identical() {
        let x = noise(10_000, 0.9, 59);
        let s = settings(Mode::Split, Slope::S12, &[20.0, 250.0, 2_500.0], |_, b| {
            b.thr_db = -20.0;
            b.ratio = 3.0;
        });
        assert!(run(&s, &x, &x, 300) == run(&s, &x, &x, 300));
    }

    /// Switching between the dynamic path and the split is a cross-fade: no sample jumps
    /// further than the signal itself moves.
    #[test]
    fn a_solo_fades_in_and_out() {
        let x = sine(48_000, 440.0, 0.5);
        let mut s = settings(Mode::Dynamic, Slope::S24, &[20.0, 1_000.0], unity);
        let mut m = Multiplex::new(SR);
        m.set(&s);
        let (mut l, mut r) = (x.clone(), x.clone());
        let mut out = Vec::new();
        for (i, (cl, cr)) in l.chunks_mut(480).zip(r.chunks_mut(480)).enumerate() {
            if i == 20 {
                s.bands[0].solo = true;
                m.set(&s);
            }
            if i == 60 {
                s.bands[0].solo = false;
                m.set(&s);
            }
            m.process(cl, cr, None);
            out.extend_from_slice(cl);
        }
        // A 440 Hz sine at 0.5 moves at most 0.029 per sample; the fade adds a little.
        let jump = out
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0, f32::max);
        assert!(jump < 0.05, "a jump of {jump}");
    }

    /// Bypassing a band fades it out over 5 ms, and a bypassed band touches nothing: once
    /// the fade is over the output is the input, to the bit.
    #[test]
    fn a_bypassed_band_fades_out_and_then_touches_nothing() {
        let x = sine(24_000, 300.0, 0.5);
        for mode in [Mode::Dynamic, Mode::Split] {
            let mut s = settings(mode, Slope::S24, &[20.0, 1_000.0], |i, b| {
                unity(i, b);
                b.gain_db = if i == 0 { -12.0 } else { 0.0 };
            });
            let mut m = Multiplex::new(SR);
            m.set(&s);
            let (mut l, mut r) = (x.clone(), x.clone());
            let mut out = Vec::new();
            for (i, (cl, cr)) in l.chunks_mut(480).zip(r.chunks_mut(480)).enumerate() {
                if i == 10 {
                    s.bands[0].on = false;
                    m.set(&s);
                }
                m.process(cl, cr, None);
                out.extend_from_slice(cl);
            }
            let jump = out
                .windows(2)
                .map(|w| (w[1] - w[0]).abs())
                .fold(0.0, f32::max);
            // A 300 Hz sine at 0.5 moves at most 0.02 per sample.
            assert!(jump < 0.03, "{mode:?}: a jump of {jump}");
            assert_eq!(
                &out[12_000..],
                &x[12_000..],
                "{mode:?}: not a wire once off"
            );
        }
    }

    /// What eight working bands cost, per second of stereo audio. Not a test of
    /// correctness: `cargo test --release -p cantus-dsp cost_of -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn cost_of_eight_bands() {
        let froms = [
            20.0, 60.0, 150.0, 400.0, 1_000.0, 2_500.0, 6_000.0, 12_000.0,
        ];
        let x = noise(48_000, 0.7, 61);
        for mode in Mode::ALL {
            for slope in Slope::ALL {
                let s = settings(mode, slope, &froms, |i, b| {
                    b.thr_db = -40.0;
                    b.ratio = 3.0;
                    b.ratio_lo = 2.0;
                    b.thr_lo_db = -60.0;
                    b.lookahead_ms = if i % 2 == 0 { 2.0 } else { 0.0 };
                });
                let start = std::time::Instant::now();
                let _ = run(&s, &x, &x, 256);
                let ms = start.elapsed().as_secs_f64() * 1_000.0;
                eprintln!(
                    "{mode:?} {slope:?}: {ms:.1} ms per second of audio ({:.2} % of one core)",
                    ms / 10.0
                );
            }
        }
    }
}
