//! From a curve's 0…1 to a parameter's own units, and back — and what each target is in
//! a given project: its range, its value when nothing automates it, whether it exists.

use super::AutoTarget;
use crate::model::{Project, MAX_GAIN_DB, MAX_WIDTH, MIN_GAIN_DB};
use cantus_dsp::math;

/// How a control maps its travel onto its range.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Taper {
    Linear,
    /// Equal ratios per equal travel (frequencies, times). Needs a positive range.
    Log,
    /// The console fader's scale, in dB: unity at 80 % of the travel, −∞ at the bottom.
    /// One definition for the mixer's faders and for automation, so a curve drawn at
    /// "0 dB" plays 0 dB.
    Fader,
}

/// `(position from the bottom, decibels)`, ascending; interpolated linearly in dB. The
/// fader widget reads its scale from here.
pub const FADER_ANCHORS: [(f32, f32); 6] = [
    (0.00, MIN_GAIN_DB),
    (0.12, -60.0),
    (0.25, -40.0),
    (0.40, -24.0),
    (0.80, 0.0),
    (1.00, MAX_GAIN_DB),
];

/// The decibel value at a position on the fader's travel (0 = bottom, 1 = top).
pub fn fader_db_at(pos: f32) -> f32 {
    let pos = if pos.is_finite() {
        pos.clamp(0.0, 1.0)
    } else {
        0.0
    };
    for w in FADER_ANCHORS.windows(2) {
        let ((p0, d0), (p1, d1)) = (w[0], w[1]);
        if pos <= p1 {
            let t = if p1 > p0 { (pos - p0) / (p1 - p0) } else { 0.0 };
            return d0 + t * (d1 - d0);
        }
    }
    MAX_GAIN_DB
}

/// Where a decibel value sits on the fader. The exact inverse of [`fader_db_at`].
pub fn fader_pos_of(db: f32) -> f32 {
    let db = if db.is_finite() {
        db.clamp(MIN_GAIN_DB, MAX_GAIN_DB)
    } else {
        MIN_GAIN_DB
    };
    for w in FADER_ANCHORS.windows(2) {
        let ((p0, d0), (p1, d1)) = (w[0], w[1]);
        if db <= d1 {
            let t = if d1 > d0 { (db - d0) / (d1 - d0) } else { 0.0 };
            return p0 + t * (p1 - p0);
        }
    }
    1.0
}

/// A target's range: what 0 and 1 mean, and how the travel between them is spent.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct AutoRange {
    pub min: f32,
    pub max: f32,
    pub taper: Taper,
    /// Integer-valued (a mode switch): the value is rounded.
    pub stepped: bool,
    /// The parameter's default, in its own units.
    pub default: f32,
}

impl AutoRange {
    pub const fn linear(min: f32, max: f32, default: f32) -> Self {
        AutoRange {
            min,
            max,
            taper: Taper::Linear,
            stepped: false,
            default,
        }
    }

    fn of_info(min: f32, max: f32, default: f32, log: bool, stepped: bool) -> Self {
        AutoRange {
            min,
            max,
            taper: if log && min > 0.0 && max > min {
                Taper::Log
            } else {
                Taper::Linear
            },
            stepped,
            default,
        }
    }

    /// The parameter's value at a position (0…1) of the curve.
    #[inline]
    pub fn value(&self, pos: f32) -> f32 {
        let p = if pos.is_finite() {
            pos.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let v = match self.taper {
            Taper::Linear => self.min + (self.max - self.min) * p,
            Taper::Log => self.min * math::powf(self.max / self.min, p),
            Taper::Fader => fader_db_at(p),
        };
        if self.stepped {
            v.round()
        } else {
            v
        }
    }

    /// Where a value sits on the curve's 0…1. The inverse of [`AutoRange::value`] (up to
    /// rounding for a stepped one).
    pub fn pos(&self, value: f32) -> f32 {
        let v = if value.is_finite() {
            value.clamp(self.min.min(self.max), self.max.max(self.min))
        } else {
            self.default
        };
        match self.taper {
            Taper::Linear => (v - self.min) / (self.max - self.min).abs().max(1e-12),
            Taper::Log => math::ln(v / self.min) / math::ln(self.max / self.min),
            Taper::Fader => fader_pos_of(v),
        }
        .clamp(0.0, 1.0)
    }
}

/// The fader's range, in dB.
pub const GAIN: AutoRange = AutoRange {
    min: MIN_GAIN_DB,
    max: MAX_GAIN_DB,
    taper: Taper::Fader,
    stepped: false,
    default: 0.0,
};
/// Pan and balance: hard left to hard right.
pub const PAN: AutoRange = AutoRange::linear(-1.0, 1.0, 0.0);
/// Stereo width: mono to twice as wide.
pub const WIDTH: AutoRange = AutoRange::linear(0.0, MAX_WIDTH, 1.0);
/// A send's level, linear.
pub const SEND: AutoRange = AutoRange::linear(0.0, 1.0, 1.0);

/// A target's range in `project`, or `None` when it names nothing that exists (a removed
/// effect, a key the instrument does not have, a send that is gone).
pub fn range_of(project: &Project, target: &AutoTarget) -> Option<AutoRange> {
    Some(match target {
        AutoTarget::TrackGain(t) => {
            project.track(*t)?;
            GAIN
        }
        AutoTarget::TrackPan(t) => {
            project.track(*t)?;
            PAN
        }
        AutoTarget::StripGain(s) => {
            project.strip(*s)?;
            GAIN
        }
        AutoTarget::StripPan(s) => {
            project.strip(*s)?;
            PAN
        }
        AutoTarget::StripWidth(s) => {
            project.strip(*s)?;
            WIDTH
        }
        AutoTarget::Send { strip, to } => {
            project.strip(*strip)?.send_to(*to)?;
            SEND
        }
        AutoTarget::MasterGain => GAIN,
        AutoTarget::MasterPan => PAN,
        AutoTarget::MasterWidth => WIDTH,
        AutoTarget::Effect { effect, key } => {
            let host = project.effect_host(*effect)?;
            let slot = project.effect(host, *effect)?;
            let i = slot.spec.kind.param(key)?;
            AutoRange::of_info(i.min, i.max, i.default, i.logarithmic, i.stepped)
        }
        AutoTarget::Instrument { track, key } => {
            let n = project.track(*track)?.instrument.native()?;
            let i = n.kind.param(key)?;
            AutoRange::of_info(i.min, i.max, i.default, i.logarithmic, i.stepped)
        }
        AutoTarget::Firmus { track, param } => {
            project.track(*track)?.instrument.firmus()?;
            let i = param.info();
            AutoRange::of_info(i.min, i.max, i.default, i.logarithmic, i.stepped)
        }
    })
}

/// The target's value as the project holds it — what plays where no automation clip
/// has been yet, and what its knob shows when nothing automates it.
pub fn base_value(project: &Project, target: &AutoTarget) -> Option<f32> {
    Some(match target {
        AutoTarget::TrackGain(t) => project.track(*t)?.gain_db,
        AutoTarget::TrackPan(t) => project.track(*t)?.pan,
        AutoTarget::StripGain(s) => project.strip(*s)?.gain_db,
        AutoTarget::StripPan(s) => project.strip(*s)?.pan,
        AutoTarget::StripWidth(s) => project.strip(*s)?.width,
        AutoTarget::Send { strip, to } => project.strip(*strip)?.send_to(*to)?.level,
        AutoTarget::MasterGain => project.master.gain_db,
        AutoTarget::MasterPan => project.master.pan,
        AutoTarget::MasterWidth => project.master.width,
        AutoTarget::Effect { effect, key } => {
            let host = project.effect_host(*effect)?;
            let slot = project.effect(host, *effect)?;
            slot.spec.kind.param(key)?;
            slot.spec.get(key)
        }
        AutoTarget::Instrument { track, key } => {
            let n = project.track(*track)?.instrument.native()?;
            n.kind.param(key)?;
            n.get(key)
        }
        AutoTarget::Firmus { track, param } => {
            param.get(project.track(*track)?.instrument.firmus()?)
        }
    })
}

/// The i18n key of the parameter's own name, where the target has one (an effect's or an
/// instrument's parameter); the mixer's controls are named by the view.
pub fn label_key(project: &Project, target: &AutoTarget) -> Option<&'static str> {
    match target {
        AutoTarget::Effect { effect, key } => {
            let host = project.effect_host(*effect)?;
            let slot = project.effect(host, *effect)?;
            Some(slot.spec.kind.param(key)?.label_key)
        }
        AutoTarget::Instrument { track, key } => {
            let n = project.track(*track)?.instrument.native()?;
            Some(n.kind.param(key)?.label_key)
        }
        AutoTarget::Firmus { param, .. } => Some(param.info().label_key),
        _ => None,
    }
}
