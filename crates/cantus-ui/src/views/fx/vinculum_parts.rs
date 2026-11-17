//! Vinculum's gain elements seen through the faceplate's window (design sheet v1), one per
//! model, so each model's face has its own heart:
//!
//! - **VCA**: the console's gain cell on its board — the eight-legged chip with its notch
//!   and silk legend, the resistors with their colour bands, the electrolytic capacitors'
//!   vented tops, the trimmer, the copper traces and the gilded pads, and the control
//!   voltage's LED lit by the reduction, its light on the board.
//! - **Opto**: the photocell leveller's cell cut away — the black tube with its end caps,
//!   the electroluminescent panel glowing as the output lights it, the light crossing to
//!   the photoresistor's cadmium track between its silver electrodes.
//! - **Vari-mu**: two valves on the chassis — the glass, the getter's silvering in the
//!   dome, the mica spacers, the grey anodes, the heaters glowing (hotter with the
//!   colour), a blue fluorescence on the glass under heavy reduction.
//!
//! Vectors at the style's scale, colours from `kit::skin::racks`. Original to Cantus.

use super::super::canvas;
use super::art::{c, hash};
use crate::kit::color::{mix, with_alpha};
use crate::kit::skin::racks as pal;
use crate::kit::text::{Role, Text};
use crate::kit::{Brush, Fill, Style};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Vec2};

/// The cavity behind a window: the unit's dark inside.
fn cavity(b: &mut Brush, r: Rect) {
    b.rect(
        r,
        Fill::V(c(pal::CAVITY), mix(c(pal::CAVITY), c(pal::SHADOW), 0.6)),
    );
}

/// A resistor lying from `a` to `z`: its leads, its tan body, `bands` of colour.
fn resistor(b: &mut Brush, k: f32, a: Pos2, z: Pos2, bands: [Color32; 4]) {
    b.segment(a, z, 0.9 * k, c(pal::LEG), false);
    let m = a + (z - a) * 0.5;
    let d = (z - a).normalized();
    let len = (z - a).length() * 0.46;
    let w = 5.5 * k;
    let n = vec2(-d.y, d.x);
    let p0 = m - d * len * 0.5;
    let p1 = m + d * len * 0.5;
    b.segment(
        p0 + n,
        p1 + n,
        w + 1.0 * k,
        with_alpha(c(pal::SHADOW), 0.5),
        true,
    );
    b.segment(p0, p1, w, c(pal::MICA), true);
    b.segment(
        p0 - n * w * 0.22,
        p1 - n * w * 0.22,
        w * 0.3,
        with_alpha(c(pal::CARD), 0.5),
        true,
    );
    for (i, col) in bands.into_iter().enumerate() {
        let t = 0.18 + 0.2 * i as f32 + if i == 3 { 0.06 } else { 0.0 };
        let q = p0 + (p1 - p0) * t;
        b.segment(q - n * w * 0.5, q + n * w * 0.5, 1.3 * k, col, false);
    }
}

/// An electrolytic capacitor seen from above at `p`: its printed silk circle, the sleeve
/// (a polarity stripe down one side), the rolled edge, the aluminium top scored with its
/// three-way vent.
fn capacitor(b: &mut Brush, k: f32, p: Pos2, rad: f32, sleeve: Color32) {
    b.circle_stroke(p, rad * 1.18, 0.6 * k, with_alpha(c(pal::SILK), 0.55));
    b.circle(
        p + vec2(1.2, 2.0) * k,
        rad * 1.04,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.55)),
    );
    b.circle(
        p,
        rad,
        Fill::V(
            mix(sleeve, c(pal::CHROME_LIGHT), 0.30),
            mix(sleeve, c(pal::SHADOW), 0.35),
        ),
    );
    // The stripe: a pale band down the sleeve's minus side.
    b.arc(
        p,
        rad * 0.84,
        2.4,
        3.6,
        rad * 0.26,
        with_alpha(c(pal::CARD), 0.75),
        false,
    );
    // The rolled edge, then the top.
    b.circle(
        p,
        rad * 0.66,
        Fill::Solid(mix(sleeve, c(pal::SHADOW), 0.55)),
    );
    b.circle(p, rad * 0.58, Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME)));
    // The vent scored in a cross.
    let v = rad * 0.42;
    for d in [vec2(1.0, 0.0), vec2(0.0, 1.0)] {
        b.segment(p - d * v, p + d * v, 0.9 * k, c(pal::CHROME_DARK), false);
        b.segment(
            p - d * v + vec2(0.0, 0.6) * k,
            p + d * v + vec2(0.0, 0.6) * k,
            0.5 * k,
            with_alpha(c(pal::CHROME_LIGHT), 0.6),
            false,
        );
    }
    b.circle(
        p - vec2(rad * 0.2, rad * 0.25),
        rad * 0.12,
        Fill::Solid(with_alpha(c(pal::CHROME_LIGHT), 0.8)),
    );
}

/// A surface-mounted part from `a` (across, `w` wide): its body and its tinned ends.
fn smd(b: &mut Brush, k: f32, p: Pos2, len: f32, w: f32, body: Color32, upright: bool) {
    let (dx, dy) = if upright { (w, len) } else { (len, w) };
    let r = Rect::from_center_size(p, vec2(dx, dy));
    b.rect(
        r.translate(vec2(0.6, 0.9) * k),
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.5)),
    );
    b.rect(r, Fill::V(mix(body, c(pal::CHROME_LIGHT), 0.15), body));
    let cap = len * 0.22;
    let ends = if upright {
        [
            Rect::from_min_max(r.min, pos2(r.right(), r.top() + cap)),
            Rect::from_min_max(pos2(r.left(), r.bottom() - cap), r.max),
        ]
    } else {
        [
            Rect::from_min_max(r.min, pos2(r.left() + cap, r.bottom())),
            Rect::from_min_max(pos2(r.right() - cap, r.top()), r.max),
        ]
    };
    for e in ends {
        b.rect(e, Fill::V(c(pal::CHROME_LIGHT), c(pal::LEG)));
    }
}

/// A via: a drilled hole in its gilded ring.
fn via(b: &mut Brush, k: f32, p: Pos2) {
    b.circle(p, 2.2 * k, Fill::V(c(pal::GOLD), c(pal::COPPER)));
    b.circle(p, 0.9 * k, Fill::Solid(c(pal::CAVITY)));
}

/// The VCA's board in `r`, its control-voltage LED lit `cv` (0 … 1) in `led`.
pub fn vca(b: &mut Brush, s: &Style, r: Rect, cv: f32, led: Color32) {
    let k = s.k;
    cavity(b, r);
    let board = r.shrink(6.0 * k);
    b.rrect(board, 3.0 * k, Fill::V(c(pal::BOARD_LIGHT), c(pal::BOARD)));
    // The board's weave, faint.
    let mut y = board.top() + 2.0 * k;
    while y < board.bottom() {
        b.hline(
            board.left(),
            board.right(),
            y,
            0.4 * k,
            with_alpha(c(pal::BOARD_LIGHT), 0.25),
        );
        y += 3.0 * k;
    }
    let (w, h) = (board.width(), board.height());
    let at = |x: f32, y: f32| pos2(board.left() + w * x, board.top() + h * y);
    // The traces: copper under the solder mask, in right angles and forty-fives.
    let traces: [[(f32, f32); 4]; 7] = [
        [(0.04, 0.30), (0.20, 0.30), (0.26, 0.38), (0.38, 0.38)],
        [(0.04, 0.62), (0.22, 0.62), (0.30, 0.54), (0.38, 0.54)],
        [(0.62, 0.38), (0.74, 0.38), (0.80, 0.30), (0.96, 0.30)],
        [(0.62, 0.54), (0.72, 0.54), (0.80, 0.70), (0.96, 0.70)],
        [(0.43, 0.24), (0.43, 0.12), (0.70, 0.12), (0.76, 0.18)],
        [(0.57, 0.68), (0.57, 0.84), (0.30, 0.84), (0.24, 0.78)],
        [(0.10, 0.84), (0.10, 0.92), (0.90, 0.92), (0.90, 0.84)],
    ];
    for t in traces {
        let pts = t.map(|(x, y)| at(x, y));
        b.polyline(&pts, 2.4 * k, mix(c(pal::COPPER), c(pal::BOARD), 0.55));
        b.polyline(&pts, 0.8 * k, with_alpha(c(pal::COPPER), 0.55));
        for p in [pts[0], pts[3]] {
            b.circle(p, 3.0 * k, Fill::V(c(pal::GOLD), c(pal::COPPER)));
            b.circle(p, 1.1 * k, Fill::Solid(c(pal::CAVITY)));
        }
    }
    // The chip: eight legs, its body, the notch, the silk legend.
    let chip = Rect::from_center_size(at(0.5, 0.46), vec2(w * 0.22, h * 0.36));
    for i in 0..4 {
        let y = chip.top() + chip.height() * (0.16 + 0.226 * i as f32);
        for (x0, x1) in [
            (chip.left() - 7.0 * k, chip.left()),
            (chip.right(), chip.right() + 7.0 * k),
        ] {
            b.segment(pos2(x0, y), pos2(x1, y), 3.0 * k, c(pal::LEG), false);
            b.segment(
                pos2(x0, y - 0.8 * k),
                pos2(x1, y - 0.8 * k),
                0.7 * k,
                c(pal::CHROME_LIGHT),
                false,
            );
        }
    }
    b.rrect(
        chip.translate(vec2(1.5, 2.5) * k),
        2.0 * k,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.6)),
    );
    b.rrect(chip, 2.0 * k, Fill::V(c(pal::CHIP_LIGHT), c(pal::CHIP)));
    b.rrect(
        chip.shrink(1.5 * k),
        1.5 * k,
        Fill::V(c(pal::CHIP), mix(c(pal::CHIP), c(pal::CHIP_LIGHT), 0.3)),
    );
    b.arc(
        pos2(chip.center().x, chip.top()),
        4.0 * k,
        0.0,
        std::f32::consts::PI,
        1.4 * k,
        c(pal::SHADOW),
        false,
    );
    b.circle(
        pos2(chip.left() + 6.0 * k, chip.top() + 7.0 * k),
        1.6 * k,
        Fill::Solid(with_alpha(c(pal::CHIP_LIGHT), 0.9)),
    );
    canvas::put(
        b,
        chip.center(),
        Align2::CENTER_CENTER,
        &Text::key("fx.vinculum.model.0", Role::Label, c(pal::SILK)),
        k,
        None,
    );
    // The resistors and the capacitors round it.
    let bands = [
        [
            c(pal::COPPER),
            c(pal::SHADOW),
            c(pal::CARD_RED),
            c(pal::GOLD),
        ],
        [
            c(pal::LENS_ORANGE),
            c(pal::LENS_ORANGE),
            c(pal::COPPER),
            c(pal::GOLD),
        ],
        [
            c(pal::LENS_AMBER),
            c(pal::FLUORESCE),
            c(pal::SHADOW),
            c(pal::GOLD),
        ],
        [
            c(pal::CARD_RED),
            c(pal::CARD_RED),
            c(pal::LENS_ORANGE),
            c(pal::GOLD),
        ],
    ];
    resistor(b, k, at(0.08, 0.16), at(0.34, 0.16), bands[0]);
    resistor(b, k, at(0.08, 0.46), at(0.34, 0.46), bands[1]);
    resistor(b, k, at(0.66, 0.46), at(0.92, 0.46), bands[2]);
    resistor(b, k, at(0.66, 0.84), at(0.84, 0.84), bands[3]);
    capacitor(b, k, at(0.15, 0.74), h * 0.12, c(pal::FLUORESCE));
    capacitor(b, k, at(0.86, 0.17), h * 0.10, c(pal::CHIP_LIGHT));
    capacitor(b, k, at(0.29, 0.74), h * 0.075, c(pal::BOARD_LIGHT));
    // The small parts round the chip, their silk outlines, the vias.
    let silk = with_alpha(c(pal::SILK), 0.55);
    b.rrect_stroke(chip.expand(5.0 * k), 2.0 * k, 0.6 * k, silk);
    for i in 0..3 {
        let y = chip.top() + chip.height() * (0.2 + 0.3 * i as f32);
        smd(
            b,
            k,
            pos2(chip.left() - 26.0 * k, y),
            9.0 * k,
            4.5 * k,
            c(pal::CHIP),
            false,
        );
        smd(
            b,
            k,
            pos2(chip.right() + 26.0 * k, y),
            9.0 * k,
            4.5 * k,
            c(pal::MICA),
            false,
        );
    }
    for (x, y) in [(0.44f32, 0.70f32), (0.56, 0.24), (0.62, 0.62), (0.38, 0.30)] {
        smd(b, k, at(x, y), 8.0 * k, 4.0 * k, c(pal::CDS), true);
    }
    for (x, y) in [
        (0.20f32, 0.30f32),
        (0.22, 0.62),
        (0.74, 0.38),
        (0.72, 0.54),
        (0.57, 0.12),
        (0.30, 0.84),
        (0.50, 0.92),
        (0.70, 0.92),
    ] {
        via(b, k, at(x, y));
    }
    // A header of six pins along the board's top edge, its silk box.
    let hdr = Rect::from_center_size(at(0.5, 0.06), vec2(w * 0.20, h * 0.07));
    b.rect(hdr, Fill::V(c(pal::CHIP_LIGHT), c(pal::CHIP)));
    b.rrect_stroke(hdr.expand(2.5 * k), 1.0 * k, 0.6 * k, silk);
    for i in 0..6 {
        let p = pos2(
            hdr.left() + hdr.width() * (i as f32 + 0.5) / 6.0,
            hdr.center().y,
        );
        b.rect(
            Rect::from_center_size(p, Vec2::splat(2.6 * k)),
            Fill::V(c(pal::GOLD), c(pal::COPPER)),
        );
    }
    // The trimmer: a blue square, its white rotor and slot.
    let tp = at(0.72, 0.70);
    let tr = Rect::from_center_size(tp, Vec2::splat(h * 0.14));
    b.rrect(
        tr,
        1.5 * k,
        Fill::V(
            mix(c(pal::FLUORESCE), c(pal::CHROME_LIGHT), 0.2),
            c(pal::FLUORESCE),
        ),
    );
    b.circle(tp, h * 0.045, Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME)));
    b.segment(
        tp - vec2(h * 0.03, h * 0.02),
        tp + vec2(h * 0.03, h * 0.02),
        1.0 * k,
        c(pal::CHROME_DARK),
        false,
    );
    // The control voltage's LED: its glow on the board, the lens.
    let lp = at(0.5, 0.80);
    let l = cv.clamp(0.0, 1.0);
    if l > 0.01 {
        b.radial(
            board,
            lp,
            Vec2::splat(h * 0.42),
            &[
                (0.0, with_alpha(led, 0.55 * l)),
                (1.0, with_alpha(led, 0.0)),
            ],
            12,
        );
    }
    b.circle(
        lp + vec2(0.0, 1.0) * k,
        5.5 * k,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.5)),
    );
    b.circle(
        lp,
        5.0 * k,
        Fill::V(
            mix(led, c(pal::CHROME_LIGHT), 0.3 * l),
            mix(led, c(pal::SHADOW), 0.7 - 0.5 * l),
        ),
    );
    b.circle(
        lp - vec2(1.5, 1.5) * k,
        1.4 * k,
        Fill::Solid(with_alpha(c(pal::CHROME_LIGHT), 0.4 + 0.5 * l)),
    );
    // A screw at each corner of the board.
    for (x, y) in [
        (0.025f32, 0.05f32),
        (0.975, 0.05),
        (0.025, 0.95),
        (0.975, 0.95),
    ] {
        let p = at(x, y);
        b.circle(
            p,
            3.2 * k,
            Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)),
        );
        b.segment(
            p - vec2(2.0, 0.0) * k,
            p + vec2(2.0, 0.0) * k,
            0.8 * k,
            c(pal::GASKET),
            false,
        );
        b.segment(
            p - vec2(0.0, 2.0) * k,
            p + vec2(0.0, 2.0) * k,
            0.8 * k,
            c(pal::GASKET),
            false,
        );
    }
    let _ = hash;
}

/// The opto's cell in `r`, cut away: the panel lit `light` (0 … 1), the photoresistor
/// across from it; `limit` draws the Limit's second, brighter panel strip.
pub fn opto(b: &mut Brush, s: &Style, r: Rect, light: f32, limit: bool) {
    let k = s.k;
    cavity(b, r);
    let l = light.clamp(0.0, 1.0);
    let lit = mix(c(pal::EL_DARK), c(pal::EL_LIT), l);
    // The tube lies across the window over a phenolic board, its four leads bent down to
    // a strip of solder lugs along the foot.
    let tube = Rect::from_center_size(
        r.center() - vec2(0.0, r.height() * 0.06),
        vec2(r.width() * 0.78, r.height() * 0.50),
    );
    let board = r.shrink2(vec2(r.width() * 0.05, r.height() * 0.07));
    b.rrect(
        board,
        3.0 * k,
        Fill::V(
            mix(c(pal::CDS), c(pal::SHADOW), 0.45),
            mix(c(pal::CDS), c(pal::SHADOW), 0.65),
        ),
    );
    b.rrect_stroke(board, 3.0 * k, 0.6 * k, with_alpha(c(pal::MICA), 0.35));
    for (x, y) in [
        (board.left() + 9.0 * k, board.top() + 9.0 * k),
        (board.right() - 9.0 * k, board.top() + 9.0 * k),
    ] {
        let q = pos2(x, y);
        b.circle(
            q,
            3.2 * k,
            Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)),
        );
        b.segment(
            q - vec2(2.0, 0.0) * k,
            q + vec2(2.0, 0.0) * k,
            0.8 * k,
            c(pal::GASKET),
            false,
        );
    }
    let lugs = Rect::from_min_max(
        pos2(
            board.left() + board.width() * 0.12,
            board.bottom() - 18.0 * k,
        ),
        pos2(
            board.right() - board.width() * 0.12,
            board.bottom() - 8.0 * k,
        ),
    );
    b.rect(
        lugs,
        Fill::V(c(pal::MICA), mix(c(pal::MICA), c(pal::SHADOW), 0.35)),
    );
    let lug_x: [f32; 4] = [0.08, 0.30, 0.70, 0.92].map(|f| lugs.left() + lugs.width() * f);
    for x in lug_x {
        let lug = Rect::from_center_size(
            pos2(x, lugs.center().y),
            vec2(6.0 * k, lugs.height() + 6.0 * k),
        );
        b.rect(lug, Fill::V(c(pal::CHROME_LIGHT), c(pal::LEG)));
        b.circle(
            pos2(x, lugs.top() - 1.0 * k),
            1.6 * k,
            Fill::Solid(c(pal::CAVITY)),
        );
        b.circle(
            pos2(x, lugs.top() - 1.0 * k),
            2.6 * k,
            Fill::Solid(with_alpha(c(pal::LEG), 0.6)),
        );
    }
    let mut j = 0;
    for (x, dir) in [(tube.left(), -1.0f32), (tube.right(), 1.0)] {
        for dy in [-0.18f32, 0.18] {
            let y = tube.center().y + tube.height() * dy;
            let p0 = pos2(x, y);
            let p1 = pos2(x + dir * 10.0 * k, y);
            let lx = if dir < 0.0 {
                lug_x[if dy < 0.0 { 0 } else { 1 }]
            } else {
                lug_x[if dy < 0.0 { 3 } else { 2 }]
            };
            let p2 = pos2(lx, tube.bottom() + 6.0 * k + 3.0 * k * j as f32);
            let p3 = pos2(lx, lugs.top() - 1.0 * k);
            b.polyline(&[p0, p1, p2, p3], 1.3 * k, c(pal::LEG));
            j += 1;
        }
    }
    b.rrect(
        tube.translate(vec2(2.0, 3.0) * k),
        tube.height() * 0.2,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.6)),
    );
    b.rrect(
        tube,
        tube.height() * 0.2,
        Fill::Stops(&[
            (0.0, c(pal::HOUSING_LIGHT)),
            (0.22, c(pal::HOUSING)),
            (0.8, c(pal::HOUSING)),
            (1.0, mix(c(pal::HOUSING), c(pal::HOUSING_LIGHT), 0.4)),
        ]),
    );
    for x in [tube.left() + 10.0 * k, tube.right() - 10.0 * k] {
        let cap = Rect::from_center_size(
            pos2(x, tube.center().y),
            vec2(14.0 * k, tube.height() + 4.0 * k),
        );
        b.rrect(
            cap,
            3.0 * k,
            Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)),
        );
        b.vline(
            cap.center().x,
            cap.top() + 2.0 * k,
            cap.bottom() - 2.0 * k,
            0.6 * k,
            with_alpha(c(pal::CHROME_LIGHT), 0.7),
        );
    }
    // The cut: the inside of the tube, dark, lit by the panel.
    let inside = tube.shrink2(vec2(26.0 * k, tube.height() * 0.16));
    b.rrect(
        inside,
        4.0 * k,
        Fill::Solid(mix(c(pal::CAVITY), c(pal::SHADOW), 0.5)),
    );
    b.rrect_stroke(
        inside,
        4.0 * k,
        0.8 * k,
        with_alpha(c(pal::HOUSING_LIGHT), 0.7),
    );
    // The panel on the left: a ribbed phosphor strip, glowing.
    let panel = Rect::from_min_size(
        inside.min + vec2(8.0 * k, 8.0 * k),
        vec2(inside.width() * 0.13, inside.height() - 16.0 * k),
    );
    if l > 0.01 {
        b.radial(
            inside,
            panel.center(),
            vec2(inside.width() * 0.75, inside.height() * 0.9),
            &[
                (0.0, with_alpha(c(pal::EL_LIT), 0.45 * l)),
                (0.5, with_alpha(c(pal::EL_LIT), 0.12 * l)),
                (1.0, with_alpha(c(pal::EL_LIT), 0.0)),
            ],
            12,
        );
    }
    b.rrect(
        panel,
        2.0 * k,
        Fill::V(mix(lit, c(pal::CHROME_LIGHT), 0.35 * l), lit),
    );
    let strips = if limit { 2 } else { 1 };
    for i in 0..strips {
        let x = panel.left() + panel.width() * (0.35 + 0.3 * i as f32);
        b.vline(
            x,
            panel.top() + 3.0 * k,
            panel.bottom() - 3.0 * k,
            1.6 * k,
            with_alpha(c(pal::CHROME_LIGHT), 0.15 + 0.6 * l),
        );
    }
    let mut y = panel.top() + 3.0 * k;
    while y < panel.bottom() - 2.0 * k {
        b.hline(
            panel.left() + 1.0 * k,
            panel.right() - 1.0 * k,
            y,
            0.5 * k,
            with_alpha(c(pal::SHADOW), 0.25),
        );
        y += 3.0 * k;
    }
    // The light crossing to the cell: soft rays.
    let cell_r = inside.height() * 0.40;
    let cell_c = pos2(inside.right() - cell_r - 10.0 * k, inside.center().y);
    if l > 0.01 {
        for i in 0..7 {
            let t = (i as f32 + 0.5) / 7.0;
            let a = pos2(panel.right(), panel.top() + panel.height() * t);
            let z = pos2(cell_c.x - cell_r * 0.7, cell_c.y + (t - 0.5) * cell_r * 1.4);
            b.segment(
                a,
                z,
                3.0 * k,
                with_alpha(c(pal::EL_LIT), 0.05 + 0.10 * l),
                false,
            );
            b.segment(
                a,
                z,
                0.8 * k,
                with_alpha(c(pal::EL_LIT), 0.10 + 0.22 * l),
                false,
            );
        }
    }
    // The photoresistor: a ceramic disc, its cadmium track between two comb electrodes.
    b.circle(
        cell_c + vec2(1.0, 2.0) * k,
        cell_r * 1.05,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.5)),
    );
    b.circle(
        cell_c,
        cell_r,
        Fill::V(c(pal::MICA), mix(c(pal::MICA), c(pal::SHADOW), 0.3)),
    );
    b.circle(
        cell_c,
        cell_r * 0.86,
        Fill::Solid(mix(c(pal::CDS), c(pal::EL_LIT), 0.18 * l)),
    );
    let n = 6;
    let pitch = cell_r * 1.5 / n as f32;
    for i in 0..=n {
        let y = cell_c.y - cell_r * 0.75 + i as f32 * pitch;
        let dy = y - cell_c.y;
        let half = (cell_r * cell_r * 0.70 - dy * dy).max(0.0).sqrt();
        let (x0, x1) = if i % 2 == 0 {
            (cell_c.x - half, cell_c.x + half * 0.72)
        } else {
            (cell_c.x - half * 0.72, cell_c.x + half)
        };
        b.hline(x0, x1, y, pitch * 0.42, c(pal::ELECTRODE));
    }
    b.circle_stroke(
        cell_c,
        cell_r * 0.86,
        0.8 * k,
        with_alpha(c(pal::SHADOW), 0.5),
    );
    // The cell's face lit by the panel.
    if l > 0.01 {
        b.radial(
            Rect::from_center_size(cell_c, Vec2::splat(cell_r * 2.0)),
            cell_c - vec2(cell_r * 0.5, 0.0),
            Vec2::splat(cell_r),
            &[
                (0.0, with_alpha(c(pal::EL_LIT), 0.30 * l)),
                (1.0, with_alpha(c(pal::EL_LIT), 0.0)),
            ],
            10,
        );
    }
    // The tube's sheen along its top.
    b.hline(
        tube.left() + 18.0 * k,
        tube.right() - 18.0 * k,
        tube.top() + 3.0 * k,
        1.2 * k,
        with_alpha(c(pal::CHROME_LIGHT), 0.25),
    );
}

/// One valve standing on the chassis at `foot`, `w` wide, `h` tall, its heater at `heat`
/// (0 … 1) and the glass's fluorescence `blue` (0 … 1).
#[allow(clippy::too_many_arguments)]
fn valve(b: &mut Brush, k: f32, foot: Pos2, w: f32, h: f32, heat: f32, blue: f32, seed: u32) {
    let glass = Rect::from_min_max(
        pos2(foot.x - w * 0.5, foot.y - h),
        pos2(foot.x + w * 0.5, foot.y - h * 0.08),
    );
    let hot = mix(c(pal::HEATER), c(pal::HEATER_HOT), heat);
    // The glow on the cavity behind it.
    b.radial(
        Rect::from_center_size(glass.center(), vec2(w * 3.2, h * 1.6)),
        pos2(glass.center().x, glass.top() + glass.height() * 0.6),
        vec2(w * 1.5, h * 0.75),
        &[
            (0.0, with_alpha(hot, 0.22 * heat)),
            (1.0, with_alpha(hot, 0.0)),
        ],
        12,
    );
    // The socket.
    let sock = Rect::from_min_max(
        pos2(foot.x - w * 0.62, foot.y - h * 0.09),
        pos2(foot.x + w * 0.62, foot.y),
    );
    b.rrect(
        sock,
        3.0 * k,
        Fill::V(c(pal::HOUSING_LIGHT), c(pal::HOUSING)),
    );
    // The pins between the glass and the socket.
    for i in 0..5 {
        let x = glass.left() + w * (0.2 + 0.15 * i as f32);
        b.vline(
            x,
            glass.bottom() - 2.0 * k,
            sock.top(),
            0.8 * k,
            c(pal::LEG),
        );
    }
    // The heater's light on the chassis round the socket.
    b.radial(
        Rect::from_center_size(foot, vec2(w * 3.0, h * 0.3)),
        foot + vec2(0.0, 4.0 * k),
        vec2(w * 1.3, h * 0.10),
        &[
            (0.0, with_alpha(hot, 0.30 * heat)),
            (1.0, with_alpha(hot, 0.0)),
        ],
        10,
    );
    // The envelope, then the getter's silvering inside the dome.
    let dome = w * 0.5;
    b.rrect(
        glass,
        dome,
        Fill::Solid(with_alpha(c(pal::VALVE_GLASS), 0.10)),
    );
    let gt = Rect::from_min_max(glass.min, pos2(glass.right(), glass.top() + h * 0.17));
    b.rrect(
        gt,
        dome,
        Fill::V(
            with_alpha(c(pal::GETTER), 0.85),
            with_alpha(c(pal::GETTER), 0.15),
        ),
    );
    // The mica spacers and the anode between them.
    let top_mica = glass.top() + h * 0.22;
    let low_mica = glass.bottom() - h * 0.12;
    for y in [top_mica, low_mica] {
        b.rect(
            Rect::from_min_max(
                pos2(glass.left() + 3.0 * k, y - 1.5 * k),
                pos2(glass.right() - 3.0 * k, y + 1.5 * k),
            ),
            Fill::Solid(with_alpha(c(pal::MICA), 0.85)),
        );
    }
    let anode = Rect::from_min_max(
        pos2(glass.left() + w * 0.18, top_mica + 2.0 * k),
        pos2(glass.right() - w * 0.18, low_mica - 2.0 * k),
    );
    b.rect(anode, Fill::V(c(pal::ANODE_LIGHT), c(pal::ANODE)));
    // Its slit, the heater glowing through it.
    let slit = Rect::from_center_size(
        anode.center(),
        vec2(anode.width() * 0.22, anode.height() * 0.84),
    );
    b.rect(
        slit,
        Fill::Solid(mix(c(pal::SHADOW), hot, 0.35 + 0.65 * heat)),
    );
    b.radial(
        anode.expand(4.0 * k),
        slit.center(),
        vec2(slit.width() * 2.0, slit.height() * 0.6),
        &[
            (0.0, with_alpha(hot, 0.65 * heat)),
            (1.0, with_alpha(hot, 0.0)),
        ],
        10,
    );
    // The grid's fine winding across the slit, dark against the heater's light.
    let mut gy = slit.top() + 2.0 * k;
    while gy < slit.bottom() - 1.0 * k {
        b.hline(
            slit.left() - 2.0 * k,
            slit.right() + 2.0 * k,
            gy,
            0.5 * k,
            with_alpha(c(pal::SHADOW), 0.55),
        );
        gy += 2.4 * k;
    }
    for i in 0..5 {
        let x = anode.left() + anode.width() * (0.1 + 0.2 * i as f32);
        b.vline(
            x,
            anode.top(),
            anode.bottom(),
            0.5 * k,
            with_alpha(c(pal::SHADOW), 0.4),
        );
    }
    // The support rods.
    for x in [anode.left() - 2.0 * k, anode.right() + 2.0 * k] {
        b.vline(x, top_mica - 4.0 * k, glass.bottom(), 0.9 * k, c(pal::LEG));
    }
    // The glass's blue fluorescence under heavy work, on its inner wall.
    if blue > 0.01 {
        b.rrect(
            glass.shrink(1.5 * k),
            dome,
            Fill::V(
                with_alpha(c(pal::FLUORESCE), 0.0),
                with_alpha(c(pal::FLUORESCE), 0.28 * blue),
            ),
        );
    }
    // The glass's highlights: a long one on the left, a short one on the right, its outline.
    b.rrect_stroke(glass, dome, 0.8 * k, with_alpha(c(pal::VALVE_GLASS), 0.55));
    b.vline(
        glass.left() + w * 0.16,
        glass.top() + h * 0.12,
        glass.bottom() - h * 0.06,
        2.0 * k,
        with_alpha(c(pal::VALVE_GLASS), 0.40),
    );
    b.vline(
        glass.right() - w * 0.14,
        glass.top() + h * 0.30,
        glass.top() + h * 0.55,
        1.2 * k,
        with_alpha(c(pal::VALVE_GLASS), 0.30),
    );
    // The pip at the dome's top.
    b.circle(
        pos2(
            glass.center().x + (hash(seed) - 0.5) * 2.0 * k,
            glass.top() - 1.0 * k,
        ),
        2.0 * k,
        Fill::Solid(with_alpha(c(pal::VALVE_GLASS), 0.6)),
    );
}

/// The vari-mu's two valves in `r`: their heaters at `heat` (0 … 1), the glass's
/// fluorescence at `blue` (0 … 1).
pub fn valves(b: &mut Brush, s: &Style, r: Rect, heat: f32, blue: f32) {
    let k = s.k;
    cavity(b, r);
    // The chassis they stand on: a strip of plated steel along the foot.
    let chassis = Rect::from_min_max(pos2(r.left(), r.bottom() - r.height() * 0.14), r.max);
    b.rect(chassis, Fill::V(c(pal::CHROME), c(pal::CHROME_DARK)));
    b.hline(
        chassis.left(),
        chassis.right(),
        chassis.top() + 0.5 * k,
        1.0 * k,
        c(pal::CHROME_LIGHT),
    );
    // The output transformer between the valves: its laminations at the sides, the black
    // bell over them, the four bolts.
    let tw = r.width() * 0.20;
    let th = r.height() * 0.42;
    let tr = Rect::from_min_max(
        pos2(r.center().x - tw * 0.5, chassis.top() - th),
        pos2(r.center().x + tw * 0.5, chassis.top() + 1.0 * k),
    );
    b.rect(
        tr.translate(vec2(2.0, 3.0) * k),
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.5)),
    );
    let core = tr.shrink2(vec2(-4.0 * k, th * 0.12));
    let mut ly = core.top();
    let mut i = 0u32;
    while ly < core.bottom() {
        b.hline(
            core.left(),
            core.right(),
            ly,
            1.0 * k,
            if i % 2 == 0 {
                c(pal::ANODE_LIGHT)
            } else {
                c(pal::ANODE)
            },
        );
        ly += 1.5 * k;
        i += 1;
    }
    b.rrect(
        tr,
        4.0 * k,
        Fill::Stops(&[
            (0.0, c(pal::HOUSING_LIGHT)),
            (0.15, c(pal::HOUSING)),
            (0.85, c(pal::HOUSING)),
            (1.0, mix(c(pal::HOUSING), c(pal::SHADOW), 0.5)),
        ]),
    );
    b.vline(
        tr.left() + tw * 0.18,
        tr.top() + 4.0 * k,
        tr.bottom() - 4.0 * k,
        1.4 * k,
        with_alpha(c(pal::HOUSING_LIGHT), 0.6),
    );
    for (x, y) in [(0.15f32, 0.12f32), (0.85, 0.12), (0.15, 0.88), (0.85, 0.88)] {
        let q = pos2(tr.left() + tw * x, tr.top() + th * y);
        b.circle(
            q,
            2.6 * k,
            Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)),
        );
    }
    let h = r.height() * 0.80;
    let w = (r.width() * 0.15).min(h * 0.34);
    for (i, fx) in [0.25f32, 0.75].into_iter().enumerate() {
        let foot = pos2(r.left() + r.width() * fx, chassis.top() + 2.0 * k);
        valve(b, k, foot, w, h, heat, blue, i as u32 + 11);
    }
    // A screw at each end of the chassis.
    for x in [r.left() + 14.0 * k, r.right() - 14.0 * k] {
        let p = pos2(x, chassis.center().y);
        b.circle(
            p,
            3.4 * k,
            Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)),
        );
        b.segment(
            p - vec2(2.2, 0.0) * k,
            p + vec2(2.2, 0.0) * k,
            0.8 * k,
            c(pal::GASKET),
            false,
        );
    }
}
