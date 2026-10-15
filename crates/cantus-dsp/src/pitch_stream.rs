//! The streaming tracker: a sample in, a frame out every hop.

use super::{fine_period, level_and_sibilance, Design, PitchFrame, Yin, GATE_DB, THRESHOLD};
use crate::rest::ZeroRun;

/// The voicing rules: a frame is a voiced *candidate* under these, and a voiced stretch
/// *keeps* going under the looser second set (hysteresis); two frames in a row change the
/// decision either way, so a lone frame never flips it.
const ON_APERIODICITY: f32 = 0.25;
const ON_SIBILANCE: f32 = 0.5;
const KEEP_APERIODICITY: f32 = 0.4;
const KEEP_SIBILANCE: f32 = 0.65;
const KEEP_GATE_DB: f32 = GATE_DB - 6.0;
const FRAMES_TO_FLIP: u8 = 2;
/// An octave jump from the last voiced frame is checked against the dip at the old lag:
/// if that dip is this deep, the voice did not jump.
const OCTAVE_KEEP: f32 = 0.3;
/// … and no shallower than the new dip by more than this (a wrong octave never locks in).
const OCTAVE_MARGIN: f32 = 0.1;

/// The streaming monophonic tracker (see the module docs). Allocates in `new` only.
#[derive(Clone, Debug)]
pub struct PitchTracker {
    design: Design,
    /// The mono input at the full rate, a power-of-two ring indexed by the sample count.
    raw: Vec<f32>,
    raw_zero: ZeroRun,
    /// The decimated low band, a power-of-two ring indexed by the decimated count.
    dec: Vec<f32>,
    dec_zero: ZeroRun,
    /// Samples pushed since reset (skips included).
    n: u64,
    yin: Yin,
    /// The coarse window, oldest first.
    lin: Vec<f32>,
    /// The fine window, oldest first.
    lin_fine: Vec<f32>,
    scratch: Vec<f32>,
    voiced: bool,
    on_run: u8,
    off_run: u8,
    /// The coarse lag of the last voiced frame (0: none).
    last_tau: f32,
    /// The newest frame, for telemetry.
    last: PitchFrame,
}

impl PitchTracker {
    pub fn new(sample_rate: f32) -> Self {
        let design = Design::new(sample_rate);
        let d = design.decimation;
        let fine_span = d + 1;
        let fine_len = design.fine_window() + design.tau_max * d + fine_span + 2;
        let raw_len = fine_len.max(design.fir.len()).next_power_of_two();
        let coarse_len = design.window + design.tau_max + 1;
        PitchTracker {
            raw: vec![0.0; raw_len],
            raw_zero: ZeroRun::FULL,
            dec: vec![0.0; coarse_len.next_power_of_two()],
            dec_zero: ZeroRun::FULL,
            n: 0,
            yin: Yin::new(design.tau_max),
            lin: vec![0.0; coarse_len],
            lin_fine: vec![0.0; fine_len],
            scratch: vec![0.0; 2 * fine_span + 3],
            voiced: false,
            on_run: 0,
            off_run: 0,
            last_tau: 0.0,
            last: PitchFrame::default(),
            design,
        }
    }

    pub fn design(&self) -> &Design {
        &self.design
    }

    /// The hop, full-rate samples.
    pub fn hop(&self) -> usize {
        self.design.hop
    }

    /// Samples pushed (and skipped) since reset.
    pub fn count(&self) -> u64 {
        self.n
    }

    /// The newest frame (the default before the first).
    pub fn last(&self) -> PitchFrame {
        self.last
    }

    /// Whether the current stretch is voiced.
    pub fn voiced(&self) -> bool {
        self.voiced
    }

    pub fn reset(&mut self) {
        self.raw.fill(0.0);
        self.dec.fill(0.0);
        self.raw_zero.clear();
        self.dec_zero.clear();
        self.n = 0;
        self.voiced = false;
        self.on_run = 0;
        self.off_run = 0;
        self.last_tau = 0.0;
        self.last = PitchFrame::default();
    }

    /// Everything that reaches a frame is as a new tracker holds it: the rings hold only
    /// zeros and the decision is back at "unvoiced, no history". The count is not state:
    /// [`Self::skip`] advances it.
    pub fn at_rest(&self) -> bool {
        self.raw_zero.covers(self.raw.len())
            && self.dec_zero.covers(self.dec.len())
            && !self.voiced
            && self.on_run == 0
            && self.off_run == 0
            && self.last_tau == 0.0
    }

    /// `frames` samples of silence went by unprocessed, at rest.
    pub fn skip(&mut self, frames: usize) {
        self.n += frames as u64;
    }

    /// One mono sample in; a frame out when it completes a hop.
    #[inline]
    pub fn push(&mut self, x: f32) -> Option<PitchFrame> {
        let mask = self.raw.len() - 1;
        self.raw[self.n as usize & mask] = x;
        self.raw_zero.push(x);
        self.n += 1;
        let d = self.design.decimation as u64;
        if !self.n.is_multiple_of(d) {
            return None;
        }
        self.decimate();
        if !self.n.is_multiple_of(self.design.hop as u64) {
            return None;
        }
        let f = self.frame();
        self.last = f;
        Some(f)
    }

    /// The decimator's output for the newest sample.
    fn decimate(&mut self) {
        let mask = self.raw.len() - 1;
        let newest = self.n as usize - 1;
        let mut acc = 0.0f32;
        for (k, h) in self.design.fir.iter().enumerate() {
            acc += h * self.raw[newest.wrapping_sub(k) & mask];
        }
        let i = (self.n / self.design.decimation as u64) as usize - 1;
        let dmask = self.dec.len() - 1;
        self.dec[i & dmask] = acc;
        self.dec_zero.push(acc);
    }

    fn frame(&mut self) -> PitchFrame {
        let dz = &self.design;
        let (w, tau_min, tau_max, d) = (dz.window, dz.tau_min, dz.tau_max, dz.decimation);
        // The windows, oldest first.
        let di = (self.n / d as u64) as usize;
        let dmask = self.dec.len() - 1;
        let len = self.lin.len();
        for (k, v) in self.lin.iter_mut().enumerate() {
            *v = self.dec[(di + k).wrapping_sub(len) & dmask];
        }
        let rmask = self.raw.len() - 1;
        let n = self.n as usize;
        let flen = self.lin_fine.len();
        for (k, v) in self.lin_fine.iter_mut().enumerate() {
            *v = self.raw[(n + k).wrapping_sub(flen) & rmask];
        }
        let wf = dz.fine_window();
        let raw_e: f32 = self.lin_fine[flen - wf..].iter().map(|v| v * v).sum();
        let low_e: f32 = self.lin[len - w..].iter().map(|v| v * v).sum();
        let (level_db, sibilance) = level_and_sibilance(raw_e, wf, low_e, d);

        self.yin.run(&self.lin, w, tau_max);
        let mut tau = self
            .yin
            .first_dip(tau_min, tau_max, THRESHOLD)
            .unwrap_or_else(|| self.yin.global_min(tau_min, tau_max));
        if self.last_tau > 0.0 {
            let r = tau as f32 / self.last_tau;
            if (1.8..2.2).contains(&r) || (0.45..0.55).contains(&r) {
                let around = self.last_tau.round() as usize;
                let span = (self.last_tau * 0.1).ceil() as usize + 1;
                let alt = self.yin.local_min_near(around, span, tau_min, tau_max);
                let (old, new) = (self.yin.cmnd[alt], self.yin.cmnd[tau]);
                if old < OCTAVE_KEEP && old < new + OCTAVE_MARGIN {
                    tau = alt;
                }
            }
        }
        let aperiodicity = self.yin.cmnd[tau];
        let tau_c = self.yin.refine(tau, tau_max);
        let silent = level_db <= -120.0;
        let period = if silent {
            0.0
        } else {
            fine_period(
                &self.lin_fine,
                wf,
                tau_c * d as f32,
                d + 1,
                &mut self.scratch,
            )
        };

        let candidate = !silent
            && aperiodicity < ON_APERIODICITY
            && level_db > GATE_DB
            && sibilance < ON_SIBILANCE;
        let keep = !silent
            && aperiodicity < KEEP_APERIODICITY
            && level_db > KEEP_GATE_DB
            && sibilance < KEEP_SIBILANCE;
        if self.voiced {
            self.on_run = 0;
            if keep {
                self.off_run = 0;
            } else {
                self.off_run += 1;
                if self.off_run >= FRAMES_TO_FLIP {
                    self.voiced = false;
                    self.off_run = 0;
                }
            }
        } else if candidate {
            self.on_run += 1;
            if self.on_run >= FRAMES_TO_FLIP {
                self.voiced = true;
                self.on_run = 0;
            }
        } else {
            self.on_run = 0;
        }
        self.last_tau = if self.voiced { tau_c } else { 0.0 };

        let newest = self.n as f64 - 1.0;
        let span = (w * d) as f64 + period as f64;
        PitchFrame {
            centre: newest - span * 0.5,
            f0: if period > 0.0 {
                dz.sample_rate / period
            } else {
                0.0
            },
            period,
            voiced: self.voiced,
            aperiodicity,
            level_db,
            sibilance,
            voiced_prob: if self.voiced { 1.0 } else { 0.0 },
        }
    }
}
