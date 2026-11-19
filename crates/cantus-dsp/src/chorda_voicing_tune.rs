//! Chorda's technician's pass (`DSP_VERSION` 10, round 40, ADR 0098's fourth amendment,
//! `docs/round40/chorda-tune.md`): what ships over candidate 16 so every key A0–C8 sounds
//! on its curve. [`SPEC`] (applied after candidate 16's, in the voicing lab's format)
//! switches the model's three root-cause fixes on and redraws the unisons that beat past a
//! recorded grand's; [`TUNE`] is each key's last correction (cents), measured with
//! `chorda::tests_tuning::tuning_map` and iterated (secant steps) until every key A0–E6 sat
//! within a tenth of a cent, held by `chorda::tests_tune_gate`.

use super::KEYS;

/// The pass's spec, over candidate 16's: the unisons centred on their keys, the strings tuned
/// on the bridge they rest on (no resonance pulling one of the first three partials more
/// than 2 cents), every loop with 8 cents of detune room, and sixteen keys redrawn.
pub const SPEC: &str = concat!(
    "uni_center=1,bridge.tune=1,bridge.pull=2,keyboard.frac_room=8,",
    "draw.24=25,draw.46=24,draw.51=9,draw.54=30,draw.56=21,draw.58=12,draw.61=31,draw.63=30,",
    "draw.65=8,draw.76=26,draw.78=3,draw.88=24,draw.95=17,draw.100=25,draw.101=5,draw.106=31",
);

/// Each key's last correction (cents, its whole ladder), A0 first.
#[rustfmt::skip]
pub const TUNE: [f32; KEYS] = [
    -0.54, -0.20, -0.44, -0.47, -0.27, -0.43, -0.40, -0.27, // 21–28
    0.10, -0.10, 0.03, 0.10, 0.00, -0.20, 0.17, 0.20, // 29–36
    -0.18, -0.67, 0.03, -0.10, 0.07, -0.05, -0.21, -0.27, // 37–44
    -0.30, 0.00, 0.20, -1.21, -0.07, 0.24, -0.75, 0.17, // 45–52
    -0.40, -0.23, -0.42, -0.20, 0.78, -0.17, -0.20, -0.13, // 53–60
    0.19, -0.24, 0.10, 1.03, 0.65, 0.27, -0.50, 0.70, // 61–68
    0.79, 0.86, 1.03, 0.77, 0.75, 0.54, 1.08, -0.25, // 69–76
    0.23, -0.19, -0.23, 0.71, 0.33, -0.75, -0.90, -0.83, // 77–84
    0.14, -0.27, 0.34, -0.30, 0.00, 0.00, 0.00, 0.00, // 85–92
    0.00, 0.00, 0.00, 0.00, 0.00, 0.00, 0.00, 0.00, // 93–100
    0.00, 0.00, 0.00, 0.00, 0.00, 0.00, 0.00, 0.00, // 101–108
];
