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
    (1, [-1157, -883, -574, -265, 54, 113, 142, 91, 100, 52, 55, 159, 223, 175, 74, 74, 0]),
    (2, [-1373, -1066, -704, -351, 1, 113, 245, 245, 235, 184, 104, 202, 229, 185, 179, 143, 0]),
    (3, [-1253, -943, -598, -252, 103, 192, 251, 162, 133, 111, 48, 221, 274, 267, 270, 53, 0]),
    (4, [-1748, -1395, -995, -596, -196, -89, -691, -906, -51, -54, 223, -108, -279, -174, -116, -247, 0]),
    (5, [-1506, -1137, -720, -304, 113, 260, 338, 207, 285, 198, 130, 164, 37, 28, -24, 34, 0]),
    (6, [-1590, -1236, -828, -429, -20, 127, 215, 226, 197, 61, -105, 328, 500, 352, 112, 53, 0]),
    (7, [-1787, -1433, -1034, -625, -226, -60, 115, 186, 227, 217, 177, 347, 337, 109, -136, -61, 0]),
    (8, [-2011, -1659, -1242, -836, -429, -304, -98, -31, 37, 36, 65, 117, 79, 7, -41, 72, 0]),
    (17, [-7, -4, -4, -9, -4, -39, 8, 1, -2, -9, -22, 21, 11, -48, -79, 40, 0]),
    (18, [0, 2, 0, -2, 5, -10, -8, 0, -2, -20, 48, -17, 10, 5, -27, 10, 0]),
    (19, [-322, -315, -322, -320, -325, -327, -302, -332, -332, -310, -272, -342, -419, -573, -540, -180, 0]),
    (20, [-5, 5, -7, -5, -3, -5, -5, -10, 0, -3, -35, -12, 5, -12, -34, -11, 0]),
    (21, [0, 15, 0, -3, 3, 3, -5, -2, 0, 12, -43, 20, -5, -25, -15, 75, 0]),
    (22, [0, 15, 3, -3, 0, 73, -60, -43, -7, 35, -27, -25, 5, -8, -8, 12, 0]),
    (23, [0, 15, 3, -3, 0, 178, -175, -57, -8, -5, 33, -12, -23, -22, 15, 5, 0]),
    (24, [0, 12, 0, 0, 3, 100, -90, -55, 2, -2, 92, -62, -27, -25, 10, 5, 0]),
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

/// A table's stick burst (the sub-millisecond broadband click, `Layer::stick[0]`) scaled: the
/// jazz kit's 10″ and 12″ toms (table 22, fitted off Virtuosity's rack tom) carried a burst
/// no other drum has (0.27 at 127, the others 0), and it made their first 3 ms erratic — a
/// centroid from 400 Hz to 10 kHz from one take to the next at one velocity, where
/// Virtuosity's own tom holds 1.3–2.1 kHz and every recorded kit's tom 0.4–2.1 kHz. The knock
/// (`stick[1]`) stays. A table not listed keeps its burst.
pub(crate) const BURST: &[(u8, f32)] = &[(22, 0.0)];

/// Table `id`'s burst scale (1 for a table not in [`BURST`]).
pub(crate) fn burst(id: u8) -> f32 {
    BURST
        .iter()
        .find(|(t, _)| *t == id)
        .map_or(1.0, |(_, g)| *g)
}
