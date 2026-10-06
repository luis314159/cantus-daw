//! Other targets, and Miri (which cannot call the OS): every request is refused with a reason.

use super::{PowerThrottling, Promotion, Refusal, WorkerRole};
use std::time::Duration;

#[derive(Debug, Default)]
pub struct Revert;

const NONE: Refusal = Refusal {
    step: "no real-time scheduling on this target",
    code: 0,
};

pub fn realtime_share() -> f32 {
    1.0
}

pub fn promote(
    _role: WorkerRole,
    _period: Duration,
    _share: Option<f32>,
    _revert: &mut Revert,
) -> Promotion {
    Promotion::Refused(NONE)
}

pub fn revert(_r: &mut Revert) {}

pub fn set_cpus(_ids: &[u32]) -> Result<(), Refusal> {
    Err(NONE)
}

pub fn opt_out_of_power_throttling() -> PowerThrottling {
    PowerThrottling::NotApplicable
}

pub fn priority() -> Option<i32> {
    None
}
