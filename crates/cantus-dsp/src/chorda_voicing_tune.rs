//! Chorda's technician's pass (`DSP_VERSION` 10, round 40, ADR 0098's fourth amendment,
//! `docs/round40/chorda-tune.md`): what ships over candidate 16 so every key A0–C8 sounds
//! on its curve. [`SPEC`] (applied after candidate 16's, in the voicing lab's format)
//! switches the model's three root-cause fixes on and redraws the unisons that beat past a
//! recorded grand's; [`TUNE`] is each key's last correction (cents), measured by
//! `chorda::tests_tuning::technician_table` and held by the suite's tuning tests.

use super::KEYS;

/// The pass's spec, over candidate 16's: the unisons centred on their keys, the strings tuned
/// on the bridge they rest on (no resonance pulling one of the first three partials more
/// than 2 cents), every loop with 8 cents of detune room, and eighteen keys redrawn.
pub const SPEC: &str = concat!(
    "uni_center=1,bridge.tune=1,bridge.pull=2,keyboard.frac_room=8,",
    "draw.24=1,draw.46=2,draw.51=4,draw.54=15,draw.56=10,draw.58=12,draw.61=11,draw.63=10,draw.65=7,",
    "draw.76=6,draw.77=9,draw.78=6,draw.88=3,draw.95=32,draw.99=1,draw.100=7,draw.101=4,",
    "draw.106=31",
);

/// Each key's last correction (cents, its whole ladder), A0 first.
#[rustfmt::skip]
pub const TUNE: [f32; KEYS] = [
    -0.54, -0.20, -0.44, -0.27, -0.27, -0.43, -0.40, -0.27, // 21–28
    0.10, -0.10, 0.03, 0.10, 0.00, -0.20, 0.17, 0.20, // 29–36
    -0.18, -0.67, 0.03, -0.10, 0.07, -0.05, -0.21, -0.27, // 37–44
    -0.30, 0.00, 0.20, -1.21, -0.07, 0.24, -0.18, 0.17, // 45–52
    -0.40, -0.56, -0.42, 0.23, 0.78, -0.17, -0.20, -0.13, // 53–60
    0.56, -0.24, 0.80, 1.03, -0.07, 0.27, -0.50, 0.70, // 61–68
    0.79, 0.86, 1.03, 0.77, 0.75, 0.54, 1.08, 0.43, // 69–76
    0.70, 0.78, -0.23, 0.71, 0.33, -0.75, -0.90, -0.83, // 77–84
    0.14, -0.27, 0.34, -0.30, 0.00, 0.00, 0.00, 0.00, // 85–92
    0.00, 0.00, 0.00, 0.00, 0.00, 0.00, 0.00, 0.00, // 93–100
    0.00, 0.00, 0.00, 0.00, 0.00, 0.00, 0.00, 0.00, // 101–108
];
