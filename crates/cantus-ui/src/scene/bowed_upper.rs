//! r42:arcus-upper — Arcus's violin and viola for the scene engine (the cello's subject,
//! `scene::bowed`): their measures, their outline and the stand they rest on.
//!
//! **Sources** (measures are facts; no drawing is copied): the violin's body and bouts are the
//! Stradivari "Messiah" (1716) as its published measures give them — 356 mm long, 166 / 109 /
//! 208 mm across the upper, middle and lower bouts, ribs about 30–32 mm, arching about 15 mm;
//! the viola a 16-inch (406 mm) instrument as makers publish its measures — about 192 / 132 /
//! 238 mm across, ribs about 38–39 mm, arching about 17 mm. The set-up (bridge, fingerboard,
//! stop, neck, afterlength, tailpiece) is the trade's standard: a violin's stop 195 mm and neck
//! 130 mm (a 325 mm string; Percival measured 319 – 326 mm on five violins), a bridge 33 mm high and 41 mm across its feet, a fingerboard
//! 270 mm; the viola's in proportion over its string. The strings' gauges are Percival's
//! measured diameters (Glasgow thesis 2013, Table 3.2).
//!
//! **The outline**: the 'Davidov' cello's three curves (`bowed::CELLO_HALF`) drawn to each
//! instrument's length and to its own bouts' widths — the violin family shares the form, and
//! the cello's proportions of bouts to length (0.45 / 0.31 / 0.58) are the 'Messiah''s within
//! 0.02 — so the corners, the C-bouts' depth and the shoulders keep a luthier's line.
//!
//! **The stand**: a violin or a viola is not stood on the floor and cannot be held without a
//! player, so it rests as a maker's or a photographer's picture shows it — upright on an
//! instrument stand, its lower bout in a padded cradle, its neck in a padded fork, leant back a
//! little — at a seated player's height, in the cello's room (chair, microphones).

use super::bowed::Dims;

pub const VIOLIN: Dims = Dims {
    body: 35.6,
    rib_lo: 3.2,
    rib_hi: 3.0,
    arch: 1.55,
    arch_back: 1.45,
    flat_back: false,
    stop: 19.5,
    neck: 13.0,
    bridge_h: 3.3,
    bridge_w: 4.1,
    board: 27.0,
    board_w: (2.35, 4.25),
    spacing: (0.55, 1.13),
    clear: 0.35,
    overstand: 0.6,
    after: 5.5,
    tail_len: 11.0,
    tail_w: (1.6, 4.0),
    pegbox: 5.6,
    scroll_r: 1.9,
    head_w: 2.45,
    endpin: 70.0,
    lean: 11.0,
    roll: 10.0,
    gauge: [0.040, 0.036, 0.031, 0.014],
    f_len: 7.4,
    f_upper: 2.1,
    f_lower: 4.8,
    machines: false,
    stand: true,
};

pub const VIOLA: Dims = Dims {
    body: 40.6,
    rib_lo: 3.9,
    rib_hi: 3.7,
    arch: 1.7,
    arch_back: 1.6,
    flat_back: false,
    stop: 22.0,
    neck: 14.8,
    bridge_h: 3.6,
    bridge_w: 4.6,
    board: 30.0,
    board_w: (2.5, 4.7),
    spacing: (0.6, 1.25),
    clear: 0.4,
    overstand: 0.7,
    after: 6.2,
    tail_len: 12.5,
    tail_w: (1.7, 4.4),
    pegbox: 6.4,
    scroll_r: 2.15,
    head_w: 2.75,
    endpin: 66.0,
    lean: 11.0,
    roll: 10.0,
    gauge: [0.045, 0.033, 0.031, 0.0165],
    f_len: 8.4,
    f_upper: 2.4,
    f_lower: 5.5,
    machines: false,
    stand: true,
};

/// How the cello's outline is drawn to an instrument (cm → the cello's mm grid): its
/// length's scale, and its half-widths' at the lower, the middle and the upper bout (the
/// 'Davidov''s 218 / 116 / 170 mm), between them linearly along the length.
#[derive(Copy, Clone, Debug)]
pub struct Draw {
    pub y: f32,
    pub x: [f32; 3],
}

pub const VIOLIN_DRAW: Draw = Draw {
    y: 356.0 / 757.0,
    x: [104.0 / 218.0, 54.5 / 116.0, 83.0 / 170.0],
};

pub const VIOLA_DRAW: Draw = Draw {
    y: 406.0 / 757.0,
    x: [119.0 / 218.0, 66.0 / 116.0, 96.0 / 170.0],
};

impl Draw {
    /// A point of the cello's half-outline (mm) drawn to this instrument (mm). The widths'
    /// scale moves between the bouts' widest places on the cello (162, 396 and 600 mm up).
    pub fn at(&self, (x, y): (f32, f32)) -> (f32, f32) {
        let k = if y <= 162.0 {
            self.x[0]
        } else if y <= 396.0 {
            let t = (y - 162.0) / (396.0 - 162.0);
            self.x[0] + (self.x[1] - self.x[0]) * t
        } else if y <= 600.0 {
            let t = (y - 396.0) / (600.0 - 396.0);
            self.x[1] + (self.x[2] - self.x[1]) * t
        } else {
            self.x[2]
        };
        (x * k, y * self.y)
    }
}
