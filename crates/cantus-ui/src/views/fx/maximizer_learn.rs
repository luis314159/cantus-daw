//! *Learn drive*: listen, set the drive, listen again, until the output reads the target.
//!
//! Ozone's *Learn Input Gain* listens once and guesses; a limiter's loudness does not grow
//! one for one with its drive (the more it limits, the less each dB of drive adds), so a
//! guess lands short. Here the loop is closed: each step measures three seconds of the
//! output (after half a second for the brickwall and the bands to settle), and the next
//! drive comes from the slope measured between the last two steps — a secant search, which
//! lands within 0.3 LU in two or three steps on real material. Every step is the same
//! gesture, so the whole session is one step of undo.

use super::State;
use crate::i18n::{t, tf};
use crate::views::fx::host::FxHost;

/// Columns (10 ms) skipped after the drive moves, for the stages to settle.
pub const SETTLE: usize = 50;
/// Columns measured per step: three seconds, a short-term window.
pub const WINDOW: usize = 300;
/// Columns of silence after which learning gives up.
pub const SILENT_STOP: usize = 1_000;
/// Steps at most.
pub const MAX_STEPS: usize = 8;
/// Close enough, LU.
pub const TOLERANCE: f64 = 0.3;
/// The drive's range (the parameter's).
const DRIVE: (f32, f32) = (-12.0, 30.0);

/// How a learning session ended.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Outcome {
    /// The output reads the target: the drive it took, the loudness measured.
    Done { drive: f32, lufs: f64 },
    /// No sound for ten seconds.
    Silent,
    /// The drive reached the end of its range.
    Limit { drive: f32 },
    /// Stopped by hand, or out of steps.
    Stopped,
}

impl Outcome {
    pub fn words(&self) -> String {
        match *self {
            Outcome::Done { drive, lufs } => tf(
                "max.learn.done",
                &[
                    ("db", &format!("{drive:+.1}")),
                    ("lufs", &format!("{lufs:.1}")),
                ],
            ),
            Outcome::Silent => t("max.learn.silent"),
            Outcome::Limit { drive } => tf("max.learn.limit", &[("db", &format!("{drive:+.1}"))]),
            Outcome::Stopped => t("max.learn.stopped"),
        }
    }
}

/// A learning session.
#[derive(Clone, Debug, PartialEq)]
pub struct Learn {
    target: f64,
    drive: f32,
    settle: usize,
    sum: f64,
    count: usize,
    silent: usize,
    /// (drive, measured loudness) of each step so far.
    points: Vec<(f32, f64)>,
    pending: Option<f32>,
    outcome: Option<Outcome>,
}

impl Learn {
    /// Start from the drive set now, aiming at `target` LUFS.
    pub fn new(target: f32, drive: f32) -> Self {
        Learn {
            target: target as f64,
            drive,
            settle: SETTLE,
            sum: 0.0,
            count: 0,
            silent: 0,
            points: Vec::new(),
            pending: None,
            outcome: None,
        }
    }

    /// Steps measured so far.
    pub fn steps(&self) -> usize {
        self.points.len()
    }

    /// The last loudness measured.
    pub fn measured(&self) -> Option<f64> {
        self.points.last().map(|p| p.1)
    }

    pub fn outcome(&self) -> Option<Outcome> {
        self.outcome
    }

    /// Stop by hand.
    pub fn stop(&mut self) {
        self.outcome.get_or_insert(Outcome::Stopped);
    }

    /// The drive to set now, once.
    pub fn take_pending(&mut self) -> Option<f32> {
        self.pending.take()
    }

    /// One 10 ms column of the output's K-weighted energy.
    pub fn column(&mut self, energy: f64) {
        if self.outcome.is_some() {
            return;
        }
        if self.settle > 0 {
            self.settle -= 1;
            return;
        }
        if super::lufs(energy) < cantus_dsp::terminus::loudness::ABSOLUTE_GATE {
            self.silent += 1;
            if self.silent >= SILENT_STOP {
                self.outcome = Some(Outcome::Silent);
            }
            return;
        }
        self.silent = 0;
        self.sum += energy;
        self.count += 1;
        if self.count >= WINDOW {
            let measured = super::lufs(self.sum / self.count as f64);
            self.sum = 0.0;
            self.count = 0;
            self.decide(measured);
        }
    }

    fn decide(&mut self, measured: f64) {
        self.points.push((self.drive, measured));
        let error = self.target - measured;
        if error.abs() <= TOLERANCE {
            self.outcome = Some(Outcome::Done {
                drive: self.drive,
                lufs: measured,
            });
            return;
        }
        if self.points.len() >= MAX_STEPS {
            self.outcome = Some(Outcome::Stopped);
            return;
        }
        // The slope of loudness against drive, measured between the last two steps; one
        // for one before there are two. Limiting flattens it; never trust it below 0.15.
        let slope = match self.points.len() {
            n if n >= 2 => {
                let (d0, l0) = self.points[n - 2];
                let (d1, l1) = self.points[n - 1];
                if (d1 - d0).abs() > 0.01 {
                    ((l1 - l0) / (d1 - d0) as f64).clamp(0.15, 1.0)
                } else {
                    1.0
                }
            }
            _ => 1.0,
        };
        let step = (error / slope).clamp(-12.0, 12.0) as f32;
        let next = (self.drive + step).clamp(DRIVE.0, DRIVE.1);
        if (next - self.drive).abs() < 0.01 {
            self.outcome = Some(Outcome::Limit { drive: self.drive });
            return;
        }
        // Drive is set to a tenth of a dB, as the knob prints it.
        let next = (next * 10.0).round() / 10.0;
        self.drive = next;
        self.pending = Some(next);
        self.settle = SETTLE;
    }
}

/// Apply what the learning decided: the next drive (one gesture for the whole session),
/// and the end of the session.
pub fn step(host: &mut dyn FxHost, st: &mut State) {
    let Some(l) = st.learn.as_mut() else {
        return;
    };
    if let Some(d) = l.take_pending() {
        host.set("gain", d, Some("learn"));
    }
    if let Some(o) = l.outcome() {
        host.end_gesture();
        st.learned = Some(o);
        st.learn = None;
    }
}

/// Start learning (or stop, when under way).
pub fn toggle(host: &mut dyn FxHost, st: &mut State) {
    match st.learn.as_mut() {
        Some(l) => l.stop(),
        None => {
            let spec = host.spec();
            st.learn = Some(Learn::new(spec.get("target"), spec.get("gain")));
            st.learned = None;
        }
    }
    step(host, st);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A model of a limiter's loudness against drive: one for one up to −12 LUFS, then
    /// flattening — what the secant search has to cope with.
    fn model(drive: f32) -> f64 {
        let lin = -20.0 + drive as f64;
        if lin < -12.0 {
            lin
        } else {
            -12.0 + (lin + 12.0) * 0.3
        }
    }

    fn energy(lufs: f64) -> f64 {
        cantus_dsp::terminus::loudness::energy_of(lufs)
    }

    /// Run a session against the model; returns the outcome and the steps it took.
    fn session(target: f32, start: f32) -> (Outcome, usize) {
        let mut l = Learn::new(target, start);
        let mut drive = start;
        for _ in 0..20_000 {
            if let Some(d) = l.take_pending() {
                drive = d;
            }
            if let Some(o) = l.outcome() {
                return (o, l.steps());
            }
            l.column(energy(model(drive)));
        }
        panic!("no outcome");
    }

    #[test]
    fn it_lands_on_the_target_where_the_loudness_is_linear_and_where_it_is_not() {
        for (target, start) in [(-14.0, 0.0), (-9.0, 0.0), (-8.0, 20.0), (-23.0, 6.0)] {
            let (o, steps) = session(target, start);
            match o {
                Outcome::Done { drive, lufs } => {
                    assert!(
                        (lufs - target as f64).abs() <= TOLERANCE,
                        "{target}: {lufs}"
                    );
                    assert!((model(drive) - target as f64).abs() <= TOLERANCE + 0.05);
                }
                other => panic!("{target}: {other:?}"),
            }
            assert!(steps <= 5, "{target} from {start}: {steps} steps");
        }
    }

    #[test]
    fn silence_and_the_end_of_the_range_stop_it() {
        let mut l = Learn::new(-14.0, 0.0);
        for _ in 0..SETTLE + SILENT_STOP {
            l.column(0.0);
        }
        assert_eq!(l.outcome(), Some(Outcome::Silent));
        // −5 LUFS is out of the model's reach: the drive runs to its end.
        let (o, _) = session(-2.0, 0.0);
        assert!(
            matches!(o, Outcome::Limit { .. } | Outcome::Stopped),
            "{o:?}"
        );
        let mut l = Learn::new(-14.0, 0.0);
        l.stop();
        assert_eq!(l.outcome(), Some(Outcome::Stopped));
    }
}
