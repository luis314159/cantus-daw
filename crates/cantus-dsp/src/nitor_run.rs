//! Nitor's audio thread: the glides, `set`, `process` in chunks that end on the grid, rest.

use super::{excite, level, low, Levels, Meter, Nitor, NitorSettings, GLIDE_MS};
use crate::multiplex::Multiplex;

/// The grid every part is clocked on (the level's), samples.
pub(super) const GRID: usize = level::UPDATE;

/// A linear glide that lands exactly on its target after a fixed number of samples.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Ramp {
    pub v: f32,
    t: f32,
    step: f32,
    left: u32,
    len: u32,
}

impl Ramp {
    pub fn new(v: f32, len: u32) -> Ramp {
        Ramp {
            v,
            t: v,
            step: 0.0,
            left: 0,
            len: len.max(1),
        }
    }

    /// Glide to `t` from where it is.
    pub fn to(&mut self, t: f32) {
        if t == self.t {
            return;
        }
        self.t = t;
        self.left = self.len;
        self.step = (t - self.v) / self.len as f32;
    }

    /// At zero at once, staying there.
    pub fn park(&mut self) {
        self.v = 0.0;
        self.t = 0.0;
        self.left = 0;
    }

    /// At the target at once.
    pub fn snap(&mut self) {
        self.v = self.t;
        self.left = 0;
    }

    #[inline]
    pub fn next(&mut self) -> f32 {
        if self.left > 0 {
            self.left -= 1;
            self.v = if self.left == 0 {
                self.t
            } else {
                self.v + self.step
            };
        }
        self.v
    }

    #[inline]
    pub fn landed(&self) -> bool {
        self.left == 0
    }

    /// At zero and staying there.
    #[inline]
    pub fn idle(&self) -> bool {
        self.left == 0 && self.v == 0.0
    }
}

/// The parts' glides, gathered so `set` and `process` read alike.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(super) struct Ramps {
    pub excite: [Ramp; excite::BANDS],
    pub even: [Ramp; excite::BANDS],
    pub sub: Ramp,
    pub harm: Ramp,
    pub tight: Ramp,
    pub lift: Ramp,
    /// The character's dynamics blended in (1) or out (0).
    pub dynamics: Ramp,
}

impl Ramps {
    pub fn new(len: u32) -> Ramps {
        let r = Ramp::new(0.0, len);
        Ramps {
            excite: [r; excite::BANDS],
            even: [Ramp::new(0.5, len); excite::BANDS],
            sub: r,
            harm: r,
            tight: r,
            lift: r,
            dynamics: r,
        }
    }

    fn to(&mut self, l: &Levels, dynamics: bool) {
        for i in 0..excite::BANDS {
            let was_idle = self.excite[i].idle();
            self.excite[i].to(l.excite[i]);
            self.even[i].to(1.0 - l.color[i]);
            if was_idle {
                // A band that starts takes its colour at once.
                self.even[i].snap();
            }
        }
        self.sub.to(l.sub);
        self.harm.to(l.harm);
        self.tight.to(l.tight);
        self.lift.to(l.lift);
        self.dynamics.to(if dynamics { 1.0 } else { 0.0 });
    }

    fn snap(&mut self) {
        for r in self.excite.iter_mut().chain(self.even.iter_mut()) {
            r.snap();
        }
        for r in [
            &mut self.sub,
            &mut self.harm,
            &mut self.tight,
            &mut self.lift,
            &mut self.dynamics,
        ] {
            r.snap();
        }
    }

    /// Every part at zero and staying there.
    pub fn idle(&self) -> bool {
        self.excite.iter().all(Ramp::idle)
            && self.sub.idle()
            && self.harm.idle()
            && self.tight.idle()
            && self.lift.idle()
            && self.dynamics.idle()
    }

    /// Every glide landed.
    pub fn landed(&self) -> bool {
        self.excite.iter().chain(&self.even).all(Ramp::landed)
            && [self.sub, self.harm, self.tight, self.lift, self.dynamics]
                .iter()
                .all(Ramp::landed)
    }

    /// The low band is needed (the sub, the harmonics or the tightening runs).
    pub fn low_band(&self) -> bool {
        !(self.sub.idle() && self.harm.idle() && self.tight.idle())
    }
}

impl Nitor {
    pub fn new(sample_rate: f32) -> Nitor {
        let sr = sample_rate.max(1.0);
        let len = (GLIDE_MS * 0.001 * sr).round().max(1.0) as u32;
        Nitor {
            sr,
            target: NitorSettings::default(),
            configured: false,
            dynamics: Multiplex::new(sr),
            exciter: excite::Exciter::new(sr),
            low: low::Low::new(sr),
            level: level::Level::new(sr),
            ramps: Ramps::new(len),
            mix: Ramp::new(1.0, len),
            out: Ramp::new(1.0, len),
            level_live: false,
            parked: false,
            zero_run: 0,
            park_after: (super::chunk::PARK_S * sr) as u64,
            lift_cache: (0.0, 1.0, 1.0),
            phase: 0,
            cell: super::chunk::Cell::default(),
            clock: 0,
            peaks: PartPeaks::NONE,
            meter: Meter::default(),
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    pub fn settings(&self) -> &NitorSettings {
        &self.target
    }

    /// New settings, between blocks. The first call takes them at once; later ones glide
    /// over [`GLIDE_MS`]. No allocation.
    /// r42:tuning — the strip's tuning (ADR 0155): the tonic sub and the lift move to the
    /// tonic's tuned pitch; the same table again changes nothing.
    pub fn set_tuning(&mut self, table: &[f32; 128]) {
        if crate::tuning::same(&self.low.tuning, table) {
            return;
        }
        self.low.tuning = *table;
        self.low.set_tonic(self.target.tonic);
    }

    pub fn set(&mut self, s: &NitorSettings) {
        let snap = !self.configured;
        self.configured = true;
        let old = self.target;
        self.target = *s;
        let dyn_on = s.amount > 0.0 && s.mix > 0.0;
        if dyn_on {
            if snap || (self.ramps.dynamics.idle() && !self.parked) {
                // From off: fresh, and parked until the first cell that carries sound
                // (see `nitor_chunk`), so what follows does not depend on how long it was
                // off.
                self.dynamics.reset();
                self.dynamics.snap_next();
                self.dynamics.set(&s.dynamics());
                self.parked = true;
                self.zero_run = 0;
            } else if s.amount != old.amount || s.character != old.character {
                if self.parked {
                    self.dynamics.snap_next();
                }
                self.dynamics.set(&s.dynamics());
            }
        } else {
            self.parked = false;
        }
        let dyn_in = dyn_on && !self.parked;
        self.ramps.to(&s.levels(), dyn_in);
        self.mix.to(s.mix.clamp(0.0, 1.0));
        self.out.to(crate::math::db_to_gain(s.out_db));
        self.low.set_tonic(s.tonic);
        self.low.set_xover(
            s.xover_hz,
            snap || !self.ramps.low_band() || self.low.band_clear(),
        );
        if snap {
            self.ramps.snap();
            self.mix.snap();
            self.out.snap();
        }
        for b in 0..excite::BANDS {
            if self.ramps.excite[b].idle() {
                self.ramps.even[b].snap();
            }
        }
    }

    /// fx/final — every glide at its target at once (a start, a seek, a loop's wrap); what
    /// the parts hold of the sound is kept.
    pub fn snap(&mut self) {
        self.ramps.snap();
        self.mix.snap();
        self.out.snap();
        // Parked (or off), the dynamics are a new Multiplex already: its own `snap` would
        // start its gain rings at unity rather than at the bands' static gains, which is not
        // what a new one holds.
        if !self.parked && !self.ramps.dynamics.idle() {
            self.dynamics.snap();
        }
        self.low.set_xover(self.target.xover_hz, true);
        self.clear_idle();
    }

    pub fn reset(&mut self) {
        self.reset_dynamics();
        self.exciter.reset();
        self.low.reset();
        self.level.reset();
        self.level_live = false;
        // As a new unit with these settings: glides at their targets, the dynamics parked.
        self.ramps.snap();
        self.mix.snap();
        self.out.snap();
        self.ramps.dynamics.park();
        self.parked = self.target.amount > 0.0 && self.target.mix > 0.0;
        self.zero_run = 0;
        self.low.set_xover(self.target.xover_hz, true);
        self.phase = 0;
        self.cell = super::chunk::Cell::default();
        self.clock = 0;
        self.meter = Meter::default();
    }

    /// No latency: the oversampling is minimum phase, the character's dynamics have no
    /// look-ahead.
    pub fn latency(&self) -> usize {
        0
    }

    pub fn meter(&self) -> &Meter {
        &self.meter
    }

    /// The tonic's fundamental the low end tunes to now, Hz.
    pub fn tonic_hz(&self) -> f32 {
        self.low.tonic_hz()
    }

    /// The character's dynamics (for the editor's band gains).
    pub fn dynamics(&self) -> &Multiplex {
        &self.dynamics
    }

    /// Whether anything runs: a part, the dynamics, or the level match coming back.
    pub(super) fn busy(&self) -> bool {
        !self.ramps.idle() || self.level_live
    }

    /// Clear what an idle part holds, on the sample it landed.
    pub(super) fn clear_idle(&mut self) {
        for b in 0..excite::BANDS {
            if self.ramps.excite[b].idle() && !self.exciter.is_clear(b) {
                self.exciter.clear(b);
            }
        }
        if self.ramps.sub.idle() && !self.low.sub_clear() {
            self.low.clear_sub();
        }
        if self.ramps.harm.idle() && !self.low.harm_clear() {
            self.low.clear_harm();
        }
        if self.ramps.tight.idle() && !self.low.tight_clear() {
            self.low.clear_tight();
        }
        if self.ramps.lift.idle() && !self.low.lift_clear() {
            self.low.clear_lift();
        }
        if !self.ramps.low_band() && !self.low.band_clear() {
            self.low.clear_band();
        }
        if !self.ramps.low_band() {
            self.low.set_xover(self.target.xover_hz, true);
        }
        // An idle band's colour takes its target at once (a new unit's would).
        for b in 0..excite::BANDS {
            if self.ramps.excite[b].idle() {
                self.ramps.even[b].snap();
            }
        }
    }

    /// At rest (ADR 0031 §5.1): every part idle and its state a new one's, the dynamics
    /// at rest, the level cleared, every glide landed.
    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    /// The first condition of [`Self::at_rest`] that fails, by name.
    pub fn rest_blocker(&self) -> Option<&'static str> {
        let checks: [(&'static str, &dyn Fn() -> bool); 7] = [
            ("level", &|| {
                self.level.is_clear() && (!self.ramps.idle() || !self.level_live)
            }),
            ("glides", &|| {
                self.ramps.landed() && self.mix.landed() && self.out.landed()
            }),
            ("exciter", &|| {
                (0..excite::BANDS).all(|b| self.exciter.is_clear(b))
            }),
            ("low band", &|| self.low.band_clear() && !self.low.gliding()),
            ("sub and harmonics", &|| {
                self.low.sub_clear() && self.low.harm_clear()
            }),
            ("tight and lift", &|| {
                self.low.tight_clear() && self.low.lift_clear()
            }),
            ("dynamics", &|| {
                self.ramps.dynamics.idle() && (self.parked || self.dynamics.at_rest())
            }),
        ];
        checks.iter().find(|(_, ok)| !ok()).map(|(name, _)| *name)
    }

    /// Samples of silence skipped while at rest: the grids move as processing would.
    pub fn skip(&mut self, frames: usize) {
        self.dynamics.skip(frames);
        self.low.skip(frames);
        self.level.skip(frames);
        self.clock += frames as u64;
        self.phase = (self.phase + frames % GRID) % GRID;
        // At rest the cells' choices are what the settings make them.
        self.cell = self.latch();
    }

    /// Frames since the last reset.
    pub fn clock(&self) -> u64 {
        self.clock
    }
}

impl Nitor {
    /// Process a stereo block in place.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let n = left.len().min(right.len());
        self.meter.in_peak = peak(&left[..n], &right[..n]);
        self.meter.active = self.busy();
        self.peaks = PartPeaks::NONE;
        let mut i = 0;
        while i < n {
            if self.phase == 0 {
                self.cell = self.latch();
            }
            let len = (GRID - self.phase).min(n - i);
            let (l, r) = (&mut left[i..i + len], &mut right[i..i + len]);
            if self.cell.busy {
                self.chunk(l, r);
            } else {
                self.idle_chunk(l, r);
            }
            self.phase = (self.phase + len) % GRID;
            self.clock += len as u64;
            if self.phase == 0 {
                self.end_cell();
            }
            i += len;
        }
        self.meter.out_peak = peak(&left[..n], &right[..n]);
        let (pre, gm) = self.level.gains();
        self.meter.pre_db = lin_db(pre);
        self.meter.match_db = lin_db(gm);
        self.meter.band_db = band_gains(&self.dynamics);
        self.meter.excite = self.peaks.excite;
        self.meter.sub = self.peaks.sub;
        self.meter.harm = self.peaks.harm;
        self.meter.tight_db = lin_db(self.peaks.tight);
        self.meter.sub_hz = if self.ramps.sub.idle() {
            0.0
        } else {
            self.low.sub_hz(self.target.sub_mode)
        };
    }
}

#[inline]
fn peak(l: &[f32], r: &[f32]) -> f32 {
    l.iter().chain(r).fold(0.0f32, |m, x| m.max(x.abs()))
}

fn lin_db(g: f32) -> f32 {
    20.0 * crate::math::log10(g.max(1e-9))
}

fn band_gains(m: &Multiplex) -> [f32; super::BANDS] {
    let g = m.gains_now_db();
    let mut out = [0.0; super::BANDS];
    out.copy_from_slice(&g[..super::BANDS]);
    out
}

/// The parts' peaks over a block (the tightening's lowest gain).
#[derive(Copy, Clone, Debug, PartialEq)]
pub(super) struct PartPeaks {
    pub tight: f32,
    pub sub: f32,
    pub harm: f32,
    pub excite: [f32; excite::BANDS],
}

impl PartPeaks {
    pub const NONE: PartPeaks = PartPeaks {
        tight: 1.0,
        sub: 0.0,
        harm: 0.0,
        excite: [0.0; excite::BANDS],
    };
}

impl Nitor {
    pub(super) fn meter_parts(&self) -> PartPeaks {
        self.peaks
    }

    pub(super) fn meter_store(&mut self, p: PartPeaks) {
        self.peaks = p;
    }

    /// The dynamics as new: the character's bands when the amount is up, none at all when
    /// it is not (a Multiplex with bands is not at rest until its gain rings have run).
    pub(super) fn reset_dynamics(&mut self) {
        self.dynamics.reset();
        self.dynamics.snap_next();
        let d = if self.target.amount > 0.0 && self.target.mix > 0.0 {
            self.target.dynamics()
        } else {
            crate::multiplex::Settings::default()
        };
        self.dynamics.set(&d);
    }
}
