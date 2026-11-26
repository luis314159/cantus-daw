//! Each measured table's level law across velocity (round 42, `tympanum-real`).
//!
//! The tables hold four velocity layers (40, 80, 110, 127) and a hit between two blends them;
//! measured through the engine, the blend's level does not follow the recording's: the studio
//! kick at velocity 48 sounded louder than at 64 and as loud as at 80, and from 100 to 127 it
//! hardly grew, where DRSKit's own kick grows 12, 6, 5.5, 3, 3.5 and 3 dB over the same steps
//! (`docs/round42/tympanum-real.md`, §3). [`LAW`] is the correction, in dB at [`LAW_VELS`],
//! that brings each table's first 80 ms to the level law of the recorded kit it was measured
//! from — rendered through the same engine — with its hardest hit where the calibration put
//! it: `tools/tympanum-real/law.py` writes it from the `level_law_dump` test's output and the
//! recorded kits' measured hits. Only the level moves; each velocity keeps its own colour.
//! A table not listed renders as it did (0 dB everywhere).

/// The velocities [`LAW`] is given at.
pub(crate) const LAW_VELS: [f32; 17] = [
    1.0, 8.0, 16.0, 24.0, 32.0, 40.0, 48.0, 56.0, 64.0, 72.0, 80.0, 88.0, 96.0, 104.0, 112.0,
    120.0, 127.0,
];

/// Per table id: centi-dB to add at each of [`LAW_VELS`].
#[rustfmt::skip]
pub(crate) const LAW: &[(u8, [i16; 17])] = &[
];

/// The level (dB) table `id`'s law adds at velocity `vel` (1 … 127), linear between the
/// given velocities; 0 for a table without one.
pub(crate) fn law_db(id: u8, vel: f32) -> f32 {
    let Some((_, c)) = LAW.iter().find(|(t, _)| *t == id) else {
        return 0.0;
    };
    let v = vel.clamp(LAW_VELS[0], LAW_VELS[LAW_VELS.len() - 1]);
    let j = LAW_VELS
        .iter()
        .position(|x| *x >= v)
        .unwrap_or(LAW_VELS.len() - 1)
        .max(1);
    let (a, b) = (LAW_VELS[j - 1], LAW_VELS[j]);
    let x = (v - a) / (b - a);
    0.01 * (c[j - 1] as f32 + x * (c[j] as f32 - c[j - 1] as f32))
}
