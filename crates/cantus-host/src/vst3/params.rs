//! Plain ⇄ normalized values without asking the controller.
//!
//! VST3 moves parameters as normalized values (0…1) and the contract moves plain ones (the
//! plug-in's own units, [`crate::ParamInfo::min`] … `max`). The controller converts
//! (`normalizedParamToPlain`, `plainParamToNormalized`), but the controller lives on the UI
//! thread and may lock or allocate, so the audio thread cannot call it. [`ParamMap`] is
//! built on the main thread by asking the controller, and answers on the audio thread:
//!
//! - **Linear** when 17 probes lie on the line from `min` to `max` (the common case, and
//!   exact both ways);
//! - **Steps** for a parameter with ≤ 4096 steps: the plug-in's own plain value at every
//!   step, found back by nearest match, and `floor(norm × (steps + 1))` as the SDK does;
//! - **Table** otherwise: 257 probes of the plug-in's curve, interpolated, inverted by
//!   bisection (a monotonic curve; a curve that is not falls back to linear).
//!
//! A new map reaches the audio thread through a [`Mailbox`] when the plug-in says its
//! parameters changed.

use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};

use ::vst3::ComPtr;
use ::vst3::Steinberg::Vst::{IEditController, IEditControllerTrait, ParamID};

const TABLE_POINTS: usize = 257;
const MAX_STEPS: i32 = 4096;
const PROBES: usize = 17;

#[derive(Clone, Debug, PartialEq)]
pub enum Mapping {
    Linear { min: f64, max: f64 },
    Steps { plain: Box<[f64]> },
    Table { plain: Box<[f64]> },
}

impl Mapping {
    /// Ask the controller how `id` maps (main thread).
    pub fn probe(controller: &ComPtr<IEditController>, id: ParamID, steps: i32) -> Mapping {
        // SAFETY: plain calls into the controller on the main thread.
        let at = |n: f64| unsafe { controller.normalizedParamToPlain(id, n) };
        Self::from_fn(at, steps)
    }

    /// The mapping of a curve `norm → plain` with `steps` steps (0 = continuous).
    pub fn from_fn(at: impl Fn(f64) -> f64, steps: i32) -> Mapping {
        let min = at(0.0);
        let max = at(1.0);
        if !min.is_finite() || !max.is_finite() {
            return Mapping::Linear { min: 0.0, max: 1.0 };
        }
        if (1..=MAX_STEPS).contains(&steps) {
            let plain: Box<[f64]> = (0..=steps)
                .map(|k| at(f64::from(k) / f64::from(steps)))
                .collect();
            return Mapping::Steps { plain };
        }
        let tolerance = 1e-9 * (max - min).abs().max(1.0);
        let linear = (0..PROBES).all(|i| {
            let n = i as f64 / (PROBES - 1) as f64;
            (at(n) - (min + n * (max - min))).abs() <= tolerance
        });
        if linear {
            return Mapping::Linear { min, max };
        }
        let plain: Box<[f64]> = (0..TABLE_POINTS)
            .map(|i| at(i as f64 / (TABLE_POINTS - 1) as f64))
            .collect();
        let rising = plain.windows(2).all(|w| w[1] >= w[0]);
        let falling = plain.windows(2).all(|w| w[1] <= w[0]);
        if (rising || falling) && plain.iter().all(|v| v.is_finite()) {
            Mapping::Table { plain }
        } else {
            Mapping::Linear { min, max }
        }
    }

    /// Normalized → plain. Allocation-free.
    pub fn to_plain(&self, norm: f64) -> f64 {
        let norm = norm.clamp(0.0, 1.0);
        match self {
            Mapping::Linear { min, max } => min + norm * (max - min),
            Mapping::Steps { plain } => {
                let steps = plain.len() - 1;
                let k = ((norm * (steps + 1) as f64) as usize).min(steps);
                plain[k]
            }
            Mapping::Table { plain } => {
                let x = norm * (plain.len() - 1) as f64;
                let i = (x as usize).min(plain.len() - 2);
                let t = x - i as f64;
                plain[i] + t * (plain[i + 1] - plain[i])
            }
        }
    }

    /// Plain → normalized. Allocation-free.
    pub fn to_normalized(&self, value: f64) -> f64 {
        let n = match self {
            Mapping::Linear { min, max } => {
                if max == min {
                    0.0
                } else {
                    (value - min) / (max - min)
                }
            }
            Mapping::Steps { plain } => {
                let steps = plain.len() - 1;
                let mut best = 0;
                for (k, p) in plain.iter().enumerate() {
                    if (p - value).abs() < (plain[best] - value).abs() {
                        best = k;
                    }
                }
                best as f64 / steps as f64
            }
            Mapping::Table { plain } => {
                let rising = plain[plain.len() - 1] >= plain[0];
                let before = |p: f64| if rising { p <= value } else { p >= value };
                // Last index whose plain value is at or before `value`.
                let (mut lo, mut hi) = (0usize, plain.len() - 1);
                if !before(plain[0]) {
                    return 0.0;
                }
                if before(plain[hi]) {
                    return 1.0;
                }
                while hi - lo > 1 {
                    let mid = (lo + hi) / 2;
                    if before(plain[mid]) {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                let span = plain[hi] - plain[lo];
                let t = if span == 0.0 {
                    0.0
                } else {
                    (value - plain[lo]) / span
                };
                (lo as f64 + t) / (plain.len() - 1) as f64
            }
        };
        n.clamp(0.0, 1.0)
    }
}

/// Every parameter's mapping, sorted by id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParamMap {
    ids: Box<[ParamID]>,
    maps: Box<[Mapping]>,
}

impl ParamMap {
    pub fn new(mut entries: Vec<(ParamID, Mapping)>) -> Self {
        entries.sort_by_key(|(id, _)| *id);
        entries.dedup_by_key(|(id, _)| *id);
        let (ids, maps): (Vec<_>, Vec<_>) = entries.into_iter().unzip();
        ParamMap {
            ids: ids.into_boxed_slice(),
            maps: maps.into_boxed_slice(),
        }
    }

    pub fn get(&self, id: ParamID) -> Option<&Mapping> {
        self.ids.binary_search(&id).ok().map(|i| &self.maps[i])
    }

    /// Plain → normalized for `id`; a parameter the map does not know passes as is.
    pub fn to_normalized(&self, id: ParamID, value: f64) -> f64 {
        self.get(id).map_or(value, |m| m.to_normalized(value))
    }

    pub fn to_plain(&self, id: ParamID, norm: f64) -> f64 {
        self.get(id).map_or(norm, |m| m.to_plain(norm))
    }
}

/// One slot to hand a boxed value to the audio thread and get the old one back, without
/// locks and without the audio thread allocating or freeing.
pub struct Mailbox<T> {
    pending: AtomicPtr<T>,
    retired: AtomicPtr<T>,
}

// SAFETY: ownership of every pointer moves by atomic swaps, so each box has one owner.
unsafe impl<T: Send> Send for Mailbox<T> {}
unsafe impl<T: Send> Sync for Mailbox<T> {}

impl<T> Mailbox<T> {
    pub fn new() -> Self {
        Mailbox {
            pending: AtomicPtr::new(ptr::null_mut()),
            retired: AtomicPtr::new(ptr::null_mut()),
        }
    }

    /// Main thread: offer a new value (replacing one not yet taken) and free what the
    /// audio thread handed back.
    pub fn post(&self, value: Box<T>) {
        let old = self.pending.swap(Box::into_raw(value), Ordering::AcqRel);
        if !old.is_null() {
            // SAFETY: the swap made this side the only owner of `old`.
            drop(unsafe { Box::from_raw(old) });
        }
        self.collect();
    }

    /// Main thread: free what the audio thread handed back.
    pub fn collect(&self) {
        let old = self.retired.swap(ptr::null_mut(), Ordering::AcqRel);
        if !old.is_null() {
            // SAFETY: as above.
            drop(unsafe { Box::from_raw(old) });
        }
    }

    /// Audio thread: swap in the offered value, if any, handing the old one back. Waits
    /// (returns `false`) while the last handed-back value has not been collected.
    pub fn take(&self, current: &mut Box<T>) -> bool {
        if !self.retired.load(Ordering::Acquire).is_null() {
            return false;
        }
        let new = self.pending.swap(ptr::null_mut(), Ordering::AcqRel);
        if new.is_null() {
            return false;
        }
        // SAFETY: the swap made this side the only owner of `new`.
        let old = std::mem::replace(current, unsafe { Box::from_raw(new) });
        self.retired.store(Box::into_raw(old), Ordering::Release);
        true
    }
}

impl<T> Default for Mailbox<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Drop for Mailbox<T> {
    fn drop(&mut self) {
        for p in [*self.pending.get_mut(), *self.retired.get_mut()] {
            if !p.is_null() {
                // SAFETY: `&mut self`: nobody else holds the mailbox.
                drop(unsafe { Box::from_raw(p) });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_parameters_convert_exactly() {
        let m = Mapping::from_fn(|n| -48.0 + 96.0 * n, 0);
        assert_eq!(
            m,
            Mapping::Linear {
                min: -48.0,
                max: 48.0
            }
        );
        assert_eq!(m.to_normalized(0.0), 0.5);
        assert_eq!(m.to_plain(0.5), 0.0);
        assert_eq!(m.to_normalized(99.0), 1.0, "clamped");
    }

    #[test]
    fn stepped_parameters_use_the_plug_ins_own_values() {
        // The SDK's discrete mapping: plain = min(steps, floor(norm * (steps + 1))).
        let m = Mapping::from_fn(|n| (n * 5.0).floor().min(4.0) * 10.0, 4);
        for k in 0..=4 {
            let plain = f64::from(k) * 10.0;
            let norm = m.to_normalized(plain);
            assert_eq!(norm, f64::from(k) / 4.0);
            assert_eq!(m.to_plain(norm), plain);
        }
    }

    #[test]
    fn curved_parameters_round_trip_through_the_table() {
        let curve = |n: f64| 20.0 * (1000.0f64).powf(n); // 20 Hz … 20 kHz
        let m = Mapping::from_fn(curve, 0);
        assert!(matches!(m, Mapping::Table { .. }));
        for hz in [20.0, 100.0, 1000.0, 15_000.0, 20_000.0] {
            let back = m.to_plain(m.to_normalized(hz));
            assert!((back - hz).abs() / hz < 1e-9, "{hz} → {back}");
        }
        let n = m.to_normalized(632.455_532);
        assert!((n - 0.5).abs() < 1e-3, "{n}");
    }

    #[test]
    fn a_mailbox_hands_values_over_and_back() {
        let mb = Mailbox::new();
        let mut current = Box::new(1);
        assert!(!mb.take(&mut current));
        mb.post(Box::new(2));
        mb.post(Box::new(3)); // replaces 2, never seen by the audio side
        assert!(mb.take(&mut current));
        assert_eq!(*current, 3);
        assert!(!mb.take(&mut current), "nothing new");
        mb.post(Box::new(4)); // also frees 1, handed back by the take
        assert!(mb.take(&mut current));
        assert_eq!(*current, 4);
    }
}
