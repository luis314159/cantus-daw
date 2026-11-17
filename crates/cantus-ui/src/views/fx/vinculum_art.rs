//! Vinculum's hardware (design sheet v1): the rack the unit is screwed into (its steel rails
//! with their square holes and cage nuts), the faceplate in its colourway's finish with its
//! ears and their slots, the button-head hex screws on nylon washers, the engraving filled
//! with paint, the recessed windows cut into the plate, the faceted jewel lamps, the
//! skirted stepped knobs and the bat toggles. Vectors at the style's scale, colours from
//! `kit::skin::racks`; nothing animates by itself — the display hands each part its state.
//! Original to Cantus.

use super::super::canvas;
use crate::kit::color::{mix, with_alpha, Hsl};
use crate::kit::skin::racks as pal;
use crate::kit::text::{Role, Text};
use crate::kit::{Brush, Fill, Style};
use cantus_dsp::math::{cos, sin};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Vec2};
use std::f32::consts::{PI, TAU};

pub fn c(h: Hsl) -> Color32 {
    h.rgb()
}

/// A number in 0 … 1 for `i` (a grain line, a screw's turn): the same every frame.
pub fn hash(i: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ 0x68E3_1DA4;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65_535.0
}

/// The faceplate's finish for the face's colourway: charcoal anodised (Console), cream
/// enamel (Broadcast), brushed stainless (Steel).
pub fn plate(s: &Style) -> &'static pal::Plate {
    match s.skin.key {
        "fx.vinculum.broadcast" => &pal::CREAM,
        "fx.vinculum.steel" => &pal::STAINLESS,
        _ => &pal::CHARCOAL,
    }
}

/// The knobs the colourway's faceplate wears.
pub fn knob_of(s: &Style) -> &'static pal::Knob {
    match s.skin.key {
        "fx.vinculum.broadcast" => &pal::KNOB_BROADCAST,
        "fx.vinculum.steel" => &pal::KNOB_STEEL,
        _ => &pal::KNOB_CONSOLE,
    }
}

/// The paint in the engraving.
pub fn legend(s: &Style) -> Color32 {
    c(plate(s).legend)
}

/// Whether the plate is a pale one (its engraving dark).
pub fn pale(s: &Style) -> bool {
    plate(s).face.l > 0.5
}

/// Words engraved in the plate at `at`: the paint, and the cut's lip catching the light
/// under it. `scale` enlarges the role (the maker's name).
#[allow(clippy::too_many_arguments)]
pub fn engrave(
    b: &mut Brush,
    s: &Style,
    at: Pos2,
    align: Align2,
    text: &Text<'_>,
    scale: f32,
) -> Rect {
    let k = s.k * scale;
    let p = plate(s);
    let lip = Text {
        color: with_alpha(c(if pale(s) { p.edge } else { p.cut }), 0.85),
        ..*text
    };
    let d = if pale(s) {
        vec2(0.0, 0.8 * s.k)
    } else {
        vec2(0.0, -0.7 * s.k)
    };
    canvas::put(b, at + d, align, &lip, k, None);
    canvas::put(
        b,
        at,
        align,
        &Text {
            color: c(p.legend),
            ..*text
        },
        k,
        None,
    )
}

/// An engraved key's words (a caption).
pub fn engrave_key(b: &mut Brush, s: &Style, at: Pos2, align: Align2, key: &str) -> Rect {
    engrave(
        b,
        s,
        at,
        align,
        &Text::key(key, Role::Micro, Color32::TRANSPARENT),
        1.0,
    )
}

/// An engraved line from `a` to `z`.
pub fn engrave_line(b: &mut Brush, s: &Style, a: Pos2, z: Pos2, w: f32) {
    let p = plate(s);
    let d = if pale(s) {
        vec2(0.0, 0.7 * s.k)
    } else {
        vec2(0.0, -0.6 * s.k)
    };
    b.segment(
        a + d,
        z + d,
        w,
        with_alpha(c(if pale(s) { p.edge } else { p.cut }), 0.7),
        false,
    );
    b.segment(a, z, w, c(p.legend), false);
}

/// The rack behind the unit: its dark, and a steel rail down each side with square holes
/// (each holding a cage nut where a screw goes) every third of a rack unit.
pub fn rack(b: &mut Brush, s: &Style, r: Rect, rail_w: f32, unit: f32) {
    let k = s.k;
    b.rect(
        r,
        Fill::V(c(pal::BACK), mix(c(pal::BACK), c(pal::SHADOW), 0.5)),
    );
    for side in [0.0f32, 1.0] {
        let x0 = if side == 0.0 {
            r.left()
        } else {
            r.right() - rail_w
        };
        let rail = Rect::from_min_max(pos2(x0, r.top()), pos2(x0 + rail_w, r.bottom()));
        b.rect(
            rail,
            Fill::Stops(&[
                (0.0, c(pal::RAIL_FOOT)),
                (0.18, c(pal::RAIL_TOP)),
                (0.5, c(pal::RAIL)),
                (1.0, c(pal::RAIL_FOOT)),
            ]),
        );
        // The rail's fold: a lit edge on the outside, a shadow on the inside.
        let (lit, dark) = if side == 0.0 {
            (rail.left() + 0.8 * k, rail.right() - 0.8 * k)
        } else {
            (rail.right() - 0.8 * k, rail.left() + 0.8 * k)
        };
        b.vline(lit, r.top(), r.bottom(), 1.0 * k, c(pal::RAIL_TOP));
        b.vline(
            dark,
            r.top(),
            r.bottom(),
            1.4 * k,
            with_alpha(c(pal::SHADOW), 0.6),
        );
        // The square holes, three to a rack unit, centred across the rail.
        let hole = (rail_w * 0.34).min(9.0 * k);
        let mut y = r.top() + unit / 6.0;
        let mut i = 0u32;
        while y < r.bottom() {
            let h = Rect::from_center_size(pos2(rail.center().x, y), vec2(hole, hole));
            b.rect(h, Fill::Solid(c(pal::HOLE)));
            b.hline(h.left(), h.right(), h.bottom(), 0.6 * k, c(pal::RAIL_TOP));
            // A cage nut's brass tongue in every second hole.
            if i % 3 == 1 {
                b.rect(
                    h.shrink(hole * 0.18),
                    Fill::V(c(pal::CAGE), mix(c(pal::CAGE), c(pal::SHADOW), 0.5)),
                );
            }
            y += unit / 3.0;
            i += 1;
        }
    }
}

/// A button-head hex screw at `p` on its nylon washer, its socket turned `turn` (0 … 1).
pub fn screw(b: &mut Brush, k: f32, p: Pos2, rad: f32, turn: f32) {
    b.circle(
        p + vec2(0.4, 1.2) * k,
        rad * 1.45,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.45)),
    );
    b.circle(p, rad * 1.38, Fill::V(c(pal::WASHER), c(pal::WASHER_SHADE)));
    b.circle_stroke(
        p,
        rad * 1.38,
        0.5 * k,
        with_alpha(c(pal::WASHER_SHADE), 0.8),
    );
    b.circle(p, rad, Fill::V(c(pal::SCREW_LIGHT), c(pal::SCREW)));
    b.circle(p, rad * 0.82, Fill::V(c(pal::SCREW), c(pal::SCREW_LIGHT)));
    // The hex socket.
    let a0 = turn * PI / 3.0;
    let hex: [Pos2; 6] = std::array::from_fn(|i| {
        let a = a0 + i as f32 * TAU / 6.0;
        p + vec2(cos(a), sin(a)) * rad * 0.44
    });
    b.polygon(&hex, with_alpha(c(pal::SHADOW), 0.9));
    b.circle(
        p - vec2(rad * 0.38, rad * 0.42),
        rad * 0.2,
        Fill::Solid(with_alpha(c(pal::CHROME_LIGHT), 0.5)),
    );
}

/// The faceplate over `r`: its sheet in the colourway's finish (brushed or sheened), its two
/// ears `ear` wide with a slot each and its screw in it, the bevel along its edges.
/// Returns the face between the ears.
pub fn faceplate(b: &mut Brush, s: &Style, r: Rect, ear: f32) -> Rect {
    let k = s.k;
    let p = plate(s);
    b.rrect_soft(
        r.translate(vec2(0.0, 4.0 * k)),
        4.0 * k,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.65)),
        9.0 * k,
    );
    b.rrect(
        r,
        3.0 * k,
        Fill::Stops(&[
            (0.0, c(p.top)),
            (0.05, mix(c(p.top), c(p.face), 0.5)),
            (0.45, c(p.face)),
            (1.0, mix(c(p.face), c(p.foot), 0.75)),
        ]),
    );
    match s.skin.key {
        "fx.vinculum.steel" => {
            // Stainless: the brush's long lines, dense and uneven.
            let mut y = r.top() + 1.0 * k;
            let mut i = 0u32;
            while y < r.bottom() - 1.0 * k {
                let a = 0.05 + 0.22 * hash(i);
                b.hline(
                    r.left() + 1.0 * k,
                    r.right() - 1.0 * k,
                    y,
                    0.5 * k,
                    with_alpha(c(if i.is_multiple_of(3) { p.foot } else { p.grain }), a),
                );
                y += 1.1 * k;
                i += 1;
            }
        }
        "fx.vinculum.broadcast" => {
            // Enamel: a broad soft sheen across its upper third, a few specks of its grain.
            b.radial(
                r,
                pos2(r.left() + r.width() * 0.32, r.top() + r.height() * 0.06),
                vec2(r.width() * 0.55, r.height() * 0.34),
                &[
                    (0.0, with_alpha(c(p.grain), 0.42)),
                    (1.0, with_alpha(c(p.grain), 0.0)),
                ],
                12,
            );
            for i in 0..90u32 {
                let q = pos2(
                    r.left() + hash(i) * r.width(),
                    r.top() + hash(i + 400) * r.height(),
                );
                b.dot(q, 0.5 * k, with_alpha(c(p.foot), 0.25));
            }
        }
        _ => {
            // Anodised: a fine satin grain and a metallic band of light.
            let mut y = r.top() + 1.0 * k;
            let mut i = 0u32;
            while y < r.bottom() - 1.0 * k {
                let a = 0.02 + 0.07 * hash(i);
                b.hline(
                    r.left() + 1.0 * k,
                    r.right() - 1.0 * k,
                    y,
                    0.5 * k,
                    with_alpha(c(p.grain), a),
                );
                y += 1.6 * k;
                i += 1;
            }
            b.rect(
                Rect::from_min_max(
                    pos2(r.left(), r.top() + r.height() * 0.10),
                    pos2(r.right(), r.top() + r.height() * 0.26),
                ),
                Fill::Stops(&[
                    (0.0, with_alpha(c(p.grain), 0.0)),
                    (0.5, with_alpha(c(p.grain), 0.06)),
                    (1.0, with_alpha(c(p.grain), 0.0)),
                ]),
            );
        }
    }
    // The bevel: lit along the top and the left, dark along the foot and the right.
    b.hline(
        r.left() + 2.0 * k,
        r.right() - 2.0 * k,
        r.top() + 0.6 * k,
        1.1 * k,
        c(p.edge),
    );
    b.hline(
        r.left() + 2.0 * k,
        r.right() - 2.0 * k,
        r.bottom() - 0.6 * k,
        1.1 * k,
        with_alpha(c(pal::SHADOW), 0.55),
    );
    b.rrect_stroke(r, 3.0 * k, 0.6 * k, with_alpha(c(p.foot), 0.8));
    // The ears: a fold line where each bends back, the slots, the screws.
    let face = Rect::from_min_max(
        pos2(r.left() + ear, r.top()),
        pos2(r.right() - ear, r.bottom()),
    );
    for (j, x) in [face.left(), face.right()].into_iter().enumerate() {
        b.vline(
            x,
            r.top() + 2.0 * k,
            r.bottom() - 2.0 * k,
            0.8 * k,
            c(p.edge),
        );
        b.vline(
            x + 1.0 * k,
            r.top() + 2.0 * k,
            r.bottom() - 2.0 * k,
            0.8 * k,
            with_alpha(c(pal::SHADOW), 0.35),
        );
        let cx = if j == 0 {
            r.left() + ear * 0.5
        } else {
            r.right() - ear * 0.5
        };
        for (n, fy) in [0.12f32, 0.88].into_iter().enumerate() {
            let cy = r.top() + r.height() * fy;
            let slot = Rect::from_center_size(pos2(cx, cy), vec2(ear * 0.52, ear * 0.30));
            b.rrect(slot, slot.height() * 0.5, Fill::Solid(c(pal::HOLE)));
            b.hline(
                slot.left() + slot.height() * 0.5,
                slot.right() - slot.height() * 0.5,
                slot.bottom() - 0.3 * k,
                0.6 * k,
                with_alpha(c(p.edge), 0.7),
            );
            let off = (hash(j as u32 * 2 + n as u32) - 0.5) * slot.width() * 0.4;
            screw(
                b,
                k,
                pos2(cx + off, cy),
                ear * 0.15,
                hash(j as u32 * 7 + n as u32 + 3),
            );
        }
    }
    face
}

/// A window cut into the plate round `r`: the cut's chamfer (dark at the top, lit at the
/// foot), a chrome ring, the gasket. The window's inside.
pub fn window(b: &mut Brush, s: &Style, r: Rect, rad: f32) -> Rect {
    let k = s.k;
    let p = plate(s);
    b.rrect(
        r.expand(3.0 * k),
        rad + 3.0 * k,
        Fill::V(mix(c(p.foot), c(pal::SHADOW), 0.45), c(p.edge)),
    );
    b.rrect(
        r.expand(1.2 * k),
        rad + 1.2 * k,
        Fill::V(c(pal::CHROME_DARK), c(pal::CHROME_LIGHT)),
    );
    b.rrect_stroke(
        r.expand(1.6 * k),
        rad + 1.6 * k,
        0.5 * k,
        with_alpha(c(pal::CHROME_LIGHT), 0.6),
    );
    b.rrect(r, rad, Fill::Solid(c(pal::GASKET)));
    r.shrink(1.4 * k)
}

/// Glass over `r`: a curved reflection along its top, a faint diagonal sheen, the lit lip.
pub fn glass(b: &mut Brush, r: Rect, k: f32) {
    let (w, h) = (r.width(), r.height());
    b.rect(
        Rect::from_min_max(r.min, pos2(r.right(), r.top() + h * 0.22)),
        Fill::V(
            with_alpha(c(pal::GLASS), 0.10),
            with_alpha(c(pal::GLASS), 0.0),
        ),
    );
    let clip = |q: Pos2| {
        pos2(
            q.x.clamp(r.left(), r.right()),
            q.y.clamp(r.top(), r.bottom()),
        )
    };
    let sheen = [
        pos2(r.left() + w * 0.62, r.top()),
        pos2(r.left() + w * 0.74, r.top()),
        pos2(r.left() + w * 0.58, r.bottom()),
        pos2(r.left() + w * 0.46, r.bottom()),
    ];
    b.polygon(&sheen.map(clip), with_alpha(c(pal::GLASS), 0.035));
    b.hline(
        r.left() + 3.0 * k,
        r.right() - 3.0 * k,
        r.top() + 0.6 * k,
        0.8 * k,
        with_alpha(c(pal::GLASS), 0.22),
    );
}

/// A faceted jewel lamp at `p` in its chrome bezel: the lens's six facets, lit `lit`
/// (0 … 1) in `lens`, its glow on the plate round it.
pub fn jewel(b: &mut Brush, k: f32, p: Pos2, rad: f32, lens: Color32, lit: f32) {
    let l = lit.clamp(0.0, 1.0);
    if l > 0.0 {
        b.radial(
            Rect::from_center_size(p, Vec2::splat(rad * 7.0)),
            p,
            Vec2::splat(rad * 3.2),
            &[
                (0.0, with_alpha(lens, 0.55 * l)),
                (0.45, with_alpha(lens, 0.16 * l)),
                (1.0, with_alpha(lens, 0.0)),
            ],
            10,
        );
    }
    b.circle(
        p + vec2(0.0, 1.0 * k),
        rad * 1.5,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.5)),
    );
    // The bezel: a knurled chrome ring.
    b.circle(
        p,
        rad * 1.42,
        Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)),
    );
    for i in 0..24 {
        let a = i as f32 * TAU / 24.0;
        let d = vec2(cos(a), sin(a));
        b.segment(
            p + d * rad * 1.22,
            p + d * rad * 1.40,
            0.45 * k,
            with_alpha(c(pal::CHROME_DARK), 0.55),
            false,
        );
    }
    b.circle(p, rad * 1.12, Fill::Solid(c(pal::GASKET)));
    let dark = mix(lens, c(pal::SHADOW), 0.74);
    let hot = mix(lens, c(pal::CHROME_LIGHT), 0.5);
    // Six facets round a table, each lit a little differently (the light comes from above).
    for i in 0..6 {
        let a0 = -PI / 2.0 + i as f32 * TAU / 6.0;
        let a1 = a0 + TAU / 6.0;
        let face = [
            p + vec2(cos(a0), sin(a0)) * rad * 0.48,
            p + vec2(cos(a0), sin(a0)) * rad,
            p + vec2(cos(a1), sin(a1)) * rad,
            p + vec2(cos(a1), sin(a1)) * rad * 0.48,
        ];
        let up = 0.5 - 0.5 * sin(0.5 * (a0 + a1));
        let col = mix(
            mix(dark, mix(dark, lens, 0.35), up),
            mix(lens, hot, up * 0.6),
            l,
        );
        b.polygon(&face, col);
    }
    let table: [Pos2; 6] = std::array::from_fn(|i| {
        let a = -PI / 2.0 + i as f32 * TAU / 6.0;
        p + vec2(cos(a), sin(a)) * rad * 0.5
    });
    b.polygon(&table, mix(mix(dark, lens, 0.3), hot, l));
    b.circle(
        p + vec2(-0.32, -0.36) * rad,
        rad * 0.18,
        Fill::Solid(with_alpha(c(pal::CHROME_LIGHT), 0.55 + 0.35 * l)),
    );
}

/// A stepped knob at `p`, `rad` across its skirt, pointing at `angle` (radians, 0 is up,
/// clockwise): its shadow, the knurled skirt, the turned cap, the pointer's line.
pub fn knob(b: &mut Brush, s: &Style, p: Pos2, rad: f32, angle: f32, hover: bool) {
    let k = s.k;
    let m = knob_of(s);
    b.circle(
        p + vec2(1.0, 3.0) * k,
        rad * 1.08,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.5)),
    );
    // The skirt: the body's colour, lit along its upper rim, its fluting.
    b.circle(p, rad, Fill::V(c(m.body_light), c(m.body)));
    b.circle(
        p,
        rad * 0.93,
        Fill::V(c(m.body), mix(c(m.body), c(m.body_light), 0.4)),
    );
    for i in 0..36 {
        let a = i as f32 * TAU / 36.0 + angle;
        let d = vec2(sin(a), -cos(a));
        let lit = 0.5 - 0.5 * cos(a - angle + PI);
        b.segment(
            p + d * rad * 0.80,
            p + d * rad * 0.93,
            0.8 * k,
            with_alpha(
                c(if i % 2 == 0 {
                    m.body_light
                } else {
                    pal::SHADOW
                }),
                0.25 + 0.3 * lit,
            ),
            false,
        );
    }
    // The skirt's pointer: a line engraved across its edge.
    let d = vec2(sin(angle), -cos(angle));
    b.segment(
        p + d * rad * 0.72,
        p + d * rad * 0.97,
        1.6 * k,
        c(m.pointer),
        true,
    );
    // The cap: turned in fine rings, its edge chamfered.
    let cap = rad * 0.66;
    b.circle(p, cap * 1.04, Fill::Solid(with_alpha(c(pal::SHADOW), 0.55)));
    b.circle(p, cap, Fill::V(c(m.cap_top), c(m.cap_foot)));
    let mut rr = cap * 0.92;
    let mut i = 0u32;
    while rr > cap * 0.12 {
        b.circle_stroke(
            p,
            rr,
            0.4 * k,
            with_alpha(
                c(if i.is_multiple_of(2) {
                    m.cap_foot
                } else {
                    m.cap_top
                }),
                0.35,
            ),
        );
        rr -= 1.3 * k;
        i += 1;
    }
    b.arc(
        p,
        cap * 0.96,
        PI * 1.1,
        PI * 1.9,
        0.8 * k,
        with_alpha(c(pal::CHROME_LIGHT), 0.55),
        false,
    );
    // The cap's own line, from its centre toward the pointer.
    b.segment(
        p + d * cap * 0.15,
        p + d * cap * 0.86,
        1.4 * k,
        with_alpha(c(m.pointer), 0.9),
        true,
    );
    if hover {
        b.circle_stroke(p, rad * 1.06, 1.0 * k, with_alpha(c(m.pointer), 0.35));
    }
}

/// A bat-handle toggle at `p` thrown up (`up`) or down: its knurled chrome nut and
/// washer on the plate, the lever's shadow, the tapered bat.
pub fn toggle(b: &mut Brush, s: &Style, p: Pos2, size: f32, up: bool, hover: bool) {
    let k = s.k;
    let nut = size * 0.42;
    b.circle(
        p + vec2(1.0, 2.0) * k,
        nut * 1.25,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.5)),
    );
    // The hex nut.
    let hex: [Pos2; 6] = std::array::from_fn(|i| {
        let a = PI / 6.0 + i as f32 * TAU / 6.0;
        p + vec2(cos(a), sin(a)) * nut * 1.18
    });
    b.polygon(&hex, c(pal::CHROME_DARK));
    let hex_in: [Pos2; 6] = std::array::from_fn(|i| {
        let a = PI / 6.0 + i as f32 * TAU / 6.0;
        p + vec2(cos(a), sin(a)) * nut * 1.05
    });
    b.polygon(&hex_in, c(pal::CHROME));
    b.circle(
        p,
        nut * 0.78,
        Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)),
    );
    b.circle(p, nut * 0.52, Fill::Solid(c(pal::GASKET)));
    // The bat: from the bushing out, widening to its rounded tip.
    let dir = if up { -1.0 } else { 1.0 };
    let len = size * 1.05;
    let tip = p + vec2(0.0, dir * len);
    let sh = vec2(1.6, 2.6) * k;
    b.segment(
        p + sh,
        tip + sh,
        size * 0.30,
        with_alpha(c(pal::SHADOW), 0.45),
        true,
    );
    let n = 8;
    for i in 0..n {
        let t0 = i as f32 / n as f32;
        let t1 = (i + 1) as f32 / n as f32;
        let w = size * (0.16 + 0.16 * t1);
        let shade = if up { 1.0 - t1 } else { t1 };
        b.segment(
            p + (tip - p) * t0,
            p + (tip - p) * t1,
            w,
            mix(c(pal::CHROME), c(pal::CHROME_DARK), 0.25 + 0.35 * shade),
            true,
        );
    }
    b.circle(
        tip,
        size * 0.17,
        Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME)),
    );
    b.segment(
        p + (tip - p) * 0.2 + vec2(-size * 0.05, 0.0),
        tip + vec2(-size * 0.06, -dir * size * 0.05),
        0.9 * k,
        with_alpha(c(pal::CHROME_LIGHT), 0.8),
        true,
    );
    if hover {
        b.circle_stroke(
            tip,
            size * 0.24,
            1.0 * k,
            with_alpha(c(pal::CHROME_LIGHT), 0.5),
        );
    }
}

/// The maker's emblem engraved at `p`: two rings linked (a *vinculum*, a bond), each
/// passing over the other once.
pub fn emblem(b: &mut Brush, s: &Style, p: Pos2, rad: f32) {
    let k = s.k;
    let pl = plate(s);
    let d = rad * 0.55;
    let (a, z) = (p - vec2(d, 0.0), p + vec2(d, 0.0));
    let lip = with_alpha(c(if pale(s) { pl.edge } else { pl.cut }), 0.8);
    let paint = c(pl.legend);
    let w = 1.8 * k;
    let off = if pale(s) {
        vec2(0.0, 0.8 * k)
    } else {
        vec2(0.0, -0.7 * k)
    };
    for (q, col) in [(off, lip), (Vec2::ZERO, paint)] {
        b.circle_stroke(a + q, rad * 0.78, w, col);
        b.circle_stroke(z + q, rad * 0.78, w, col);
    }
    // Where the right ring passes over the left one (the upper crossing): a gap cut in the
    // left ring's stroke, the right ring's stroke laid over it.
    let cross = pos2(p.x, p.y - (rad * 0.78 * rad * 0.78 - d * d).max(0.0).sqrt());
    b.circle(cross, w * 1.3, Fill::Solid(c(pl.face)));
    b.arc(z, rad * 0.78, PI * 1.05, PI * 1.45, w, paint, false);
}
