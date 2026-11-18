//! Lenis's hardware (design sheet v1): the bench analyser on its black epoxy bench — the
//! case's wrap and its enamelled front panel (screen-printed, not engraved: a measuring
//! instrument's legend), the chrome carrying handles on their stand-offs, the pan-head
//! screws, the moulded bezel round the screen and the phosphor glass behind it, the push
//! keys with their lamp windows, the Depth dial with its fluted grip and turned skirt, the
//! readouts' seven-segment digits behind smoked glass, the BNC sockets and their signal
//! lamps, the maker's emblem (calm water over rough stones). Vectors at the style's scale,
//! colours from `kit::skin::analysers`; nothing animates by itself — the display hands each
//! part its state. Original to Cantus.

use super::super::canvas;
use crate::kit::color::{mix, with_alpha, Hsl};
use crate::kit::skin::analysers::{self as pal, Finish};
use crate::kit::text::Text;
use crate::kit::{Brush, Fill, Style};
use cantus_dsp::math::{cos, sin};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};
use std::f32::consts::{PI, TAU};

pub fn c(h: Hsl) -> Color32 {
    h.rgb()
}

/// Light on glass and metal: the highlights' colour.
pub fn glint() -> Color32 {
    c(pal::GLINT)
}

/// The shadow under a part.
pub fn shadow() -> Color32 {
    c(pal::SHADOW)
}

/// A number in 0 … 1 for `i` (a speck, a screw's turn): the same every frame.
pub fn hash(i: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ 0x51F1_5EED;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65_535.0
}

/// The finish of the face's colourway: Laboratory, Ivory or Graphite.
pub fn finish(s: &Style) -> &'static Finish {
    match s.skin.key {
        "fx.lenis.ivory" => &pal::IVORY,
        "fx.lenis.graphite" => &pal::GRAPHITE,
        _ => &pal::LABORATORY,
    }
}

/// Whether the panel is a pale one (its print dark).
pub fn pale(s: &Style) -> bool {
    finish(s).panel.l > 0.5
}

/// The print's ink.
pub fn ink(s: &Style) -> Color32 {
    c(finish(s).print)
}

/// Words screen-printed on the panel at `at` (a hair of the ink's own thickness under
/// them, as a print stands on enamel). `scale` enlarges the role (the maker's name).
pub fn print(
    b: &mut Brush,
    s: &Style,
    at: Pos2,
    align: Align2,
    text: &Text<'_>,
    scale: f32,
) -> Rect {
    let f = finish(s);
    let k = s.k * scale;
    let under = Text {
        color: with_alpha(c(if pale(s) { f.panel_foot } else { pal::SHADOW }), 0.35),
        ..*text
    };
    canvas::put(b, at + vec2(0.0, 0.5 * s.k), align, &under, k, None);
    canvas::put(
        b,
        at,
        align,
        &Text {
            color: c(f.print),
            ..*text
        },
        k,
        None,
    )
}

/// A printed rule from `a` to `z`.
pub fn rule(b: &mut Brush, s: &Style, a: Pos2, z: Pos2, w: f32) {
    b.segment(a, z, w, c(finish(s).rule), false);
}

/// The black epoxy bench under the instrument: darker toward the wall, a fine speckle, the
/// light along its front edge.
pub fn bench(b: &mut Brush, s: &Style, r: Rect) {
    let k = s.k;
    b.rect(r, Fill::V(c(pal::BENCH_FAR), c(pal::BENCH)));
    let n = ((r.width() * r.height()) / (900.0 * k * k)).min(900.0) as u32;
    let speck = c(pal::BENCH_SPECK);
    for i in 0..n {
        let p = pos2(
            r.left() + hash(i * 2 + 7) * r.width(),
            r.top() + hash(i * 2 + 8) * r.height(),
        );
        let a = 0.10 + 0.30 * hash(i + 9001) * ((p.y - r.top()) / r.height());
        b.rect(
            Rect::from_min_size(p, vec2(0.9 * k, 0.9 * k)),
            Fill::Solid(with_alpha(speck, a)),
        );
    }
    b.hline(
        r.left(),
        r.right(),
        r.bottom() - 1.0 * k,
        1.0 * k,
        with_alpha(c(pal::BENCH_SPECK), 0.6),
    );
}

/// The case standing on the bench in `r` (its feet below it): the contact shadow, two
/// rubber feet, the wrap with its seams, the front panel inset with its enamel and
/// chamfer, a screw in each corner. Returns the front panel.
pub fn case(b: &mut Brush, s: &Style, r: Rect, wrap: f32) -> Rect {
    let k = s.k;
    let f = finish(s);
    // The shadow it casts on the bench, soft and wide at its foot.
    b.rrect_soft(
        Rect::from_min_max(
            pos2(r.left() + 6.0 * k, r.bottom() - 6.0 * k),
            pos2(r.right() - 6.0 * k, r.bottom() + 12.0 * k),
        ),
        8.0 * k,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.55)),
        14.0 * k,
    );
    // The feet: rubber pads at the front corners, lit along their top.
    for x in [r.left() + 40.0 * k, r.right() - 40.0 * k - 54.0 * k] {
        let foot = Rect::from_min_size(pos2(x, r.bottom() - 2.0 * k), vec2(54.0 * k, 9.0 * k));
        b.rrect(foot, 3.0 * k, Fill::V(c(pal::FOOT_LIGHT), c(pal::FOOT)));
        b.hline(
            foot.left() + 3.0 * k,
            foot.right() - 3.0 * k,
            foot.bottom() - 0.8 * k,
            0.8 * k,
            with_alpha(c(pal::SHADOW), 0.8),
        );
    }
    // The wrap: a rounded shell, lit along its top.
    let rad = 10.0 * k;
    b.rrect(
        r,
        rad,
        Fill::Stops(&[
            (0.0, c(f.wrap_top)),
            (0.06, c(f.wrap)),
            (0.85, c(f.wrap)),
            (1.0, c(f.wrap_foot)),
        ]),
    );
    b.rrect_stroke(
        r.shrink(0.5 * k),
        rad,
        1.0 * k,
        with_alpha(c(f.wrap_top), 0.7),
    );
    // The front panel, set into the wrap: a dark seam round it, then the enamel.
    let panel = r.shrink(wrap);
    b.rrect(
        panel.expand(1.4 * k),
        5.0 * k,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.75)),
    );
    b.rrect(
        panel,
        4.0 * k,
        Fill::Stops(&[
            (0.0, c(f.panel_top)),
            (0.10, c(f.panel)),
            (0.80, c(f.panel)),
            (1.0, c(f.panel_foot)),
        ]),
    );
    // The enamel's fine speckle.
    let n = ((panel.width() * panel.height()) / (260.0 * k * k)).min(2400.0) as u32;
    let speck = c(f.speckle);
    for i in 0..n {
        let p = pos2(
            panel.left() + 2.0 * k + hash(i * 3 + 1) * (panel.width() - 4.0 * k),
            panel.top() + 2.0 * k + hash(i * 3 + 2) * (panel.height() - 4.0 * k),
        );
        let a = 0.10 + 0.22 * hash(i * 3 + 3);
        b.rect(
            Rect::from_min_size(p, vec2(0.8 * k, 0.8 * k)),
            Fill::Solid(with_alpha(speck, a)),
        );
    }
    // The chamfer: light along the top and the left, shade along the foot and the right.
    b.hline(
        panel.left() + 4.0 * k,
        panel.right() - 4.0 * k,
        panel.top() + 0.6 * k,
        1.0 * k,
        with_alpha(c(f.chamfer), 0.9),
    );
    b.vline(
        panel.left() + 0.6 * k,
        panel.top() + 4.0 * k,
        panel.bottom() - 4.0 * k,
        1.0 * k,
        with_alpha(c(f.chamfer), 0.6),
    );
    b.hline(
        panel.left() + 4.0 * k,
        panel.right() - 4.0 * k,
        panel.bottom() - 0.6 * k,
        1.0 * k,
        with_alpha(c(pal::SHADOW), 0.45),
    );
    b.vline(
        panel.right() - 0.6 * k,
        panel.top() + 4.0 * k,
        panel.bottom() - 4.0 * k,
        1.0 * k,
        with_alpha(c(pal::SHADOW), 0.35),
    );
    for (j, p) in [
        panel.left_top() + vec2(9.0, 9.0) * k,
        panel.right_top() + vec2(-9.0, 9.0) * k,
        panel.left_bottom() + vec2(9.0, -9.0) * k,
        panel.right_bottom() + vec2(-9.0, -9.0) * k,
    ]
    .into_iter()
    .enumerate()
    {
        screw(b, k, p, 3.6 * k, hash(j as u32 + 70));
    }
    panel
}

/// A nickel pan-head screw at `p`, its cross slot turned `turn` (0 … 1).
pub fn screw(b: &mut Brush, k: f32, p: Pos2, rad: f32, turn: f32) {
    b.circle(
        p + vec2(0.3, 0.9) * k,
        rad * 1.15,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.40)),
    );
    b.circle(p, rad, Fill::V(c(pal::SCREW_LIGHT), c(pal::SCREW_DARK)));
    b.circle(p, rad * 0.78, Fill::V(c(pal::SCREW), c(pal::SCREW_LIGHT)));
    let a = turn * PI;
    for d in [a, a + PI * 0.5] {
        let v = vec2(cos(d), sin(d)) * rad * 0.62;
        b.segment(p - v, p + v, 0.9 * k, c(pal::SCREW_DARK), false);
    }
}

/// A chrome carrying handle down the panel at `x`, from `top` to `bottom`: two stand-off
/// blocks screwed to the panel, the bent tube between them standing off it, its shadow on
/// the enamel.
pub fn handle(b: &mut Brush, s: &Style, x: f32, top: f32, bottom: f32) {
    let k = s.k;
    let f = finish(s);
    let w = 15.0 * k;
    let tube = Rect::from_min_max(
        pos2(x - w * 0.5, top + 10.0 * k),
        pos2(x + w * 0.5, bottom - 10.0 * k),
    );
    // The tube's shadow on the panel, down and to the right.
    b.rrect_soft(
        tube.translate(vec2(5.0, 7.0) * k),
        w * 0.5,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.30)),
        8.0 * k,
    );
    // The stand-offs: a block at each end, a screw in each.
    for y in [top, bottom - 26.0 * k] {
        let blk = Rect::from_min_size(pos2(x - 12.0 * k, y), vec2(24.0 * k, 26.0 * k));
        b.rrect_soft(
            blk.translate(vec2(1.5, 2.5) * k),
            5.0 * k,
            Fill::Solid(with_alpha(c(pal::SHADOW), 0.45)),
            4.0 * k,
        );
        b.rrect(blk, 5.0 * k, Fill::V(c(f.handle), c(f.handle_dark)));
        b.rrect_stroke(blk, 5.0 * k, 0.8 * k, with_alpha(c(f.handle_light), 0.55));
        screw(b, k, blk.center(), 3.4 * k, hash(y as u32));
    }
    // The tube: lit down its left, its own shade on the right.
    b.hrect(
        tube,
        &[
            (0.0, c(f.handle_dark)),
            (0.22, c(f.handle_light)),
            (0.40, c(f.handle)),
            (0.80, c(f.handle_dark)),
            (1.0, mix(c(f.handle_dark), c(pal::SHADOW), 0.4)),
        ],
    );
    b.vline(
        tube.left() + w * 0.24,
        tube.top() + 2.0 * k,
        tube.bottom() - 2.0 * k,
        1.2 * k,
        with_alpha(c(f.handle_light), 0.9),
    );
    // Where the tube meets each block: a turned ring.
    for y in [tube.top(), tube.bottom() - 3.0 * k] {
        b.rect(
            Rect::from_min_size(pos2(tube.left() - 1.0 * k, y), vec2(w + 2.0 * k, 3.0 * k)),
            Fill::V(c(f.handle_light), c(f.handle_dark)),
        );
    }
}

/// The moulded bezel round the screen in `r`: its lit lip, its face stepping down to the
/// glass, four small screws holding it. Returns the glass.
pub fn bezel(b: &mut Brush, s: &Style, r: Rect) -> Rect {
    let k = s.k;
    let f = finish(s);
    b.rrect_soft(
        r.translate(vec2(0.0, 3.0 * k)).expand(1.0 * k),
        12.0 * k,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.45)),
        8.0 * k,
    );
    b.rrect(
        r,
        12.0 * k,
        Fill::Stops(&[
            (0.0, c(f.bezel_top)),
            (0.05, c(f.bezel)),
            (0.95, c(f.bezel)),
            (1.0, c(f.bezel_foot)),
        ]),
    );
    b.rrect_stroke(
        r.shrink(0.6 * k),
        12.0 * k,
        1.0 * k,
        with_alpha(c(f.bezel_top), 0.9),
    );
    // The step down to the glass: a chamfer, dark below its lit edge.
    let step = r.shrink(13.0 * k);
    b.rrect(
        step.expand(4.0 * k),
        9.0 * k,
        Fill::V(c(f.bezel_foot), c(f.bezel)),
    );
    b.rrect_stroke(
        step.expand(4.0 * k),
        9.0 * k,
        0.8 * k,
        with_alpha(c(f.bezel_top), 0.45),
    );
    for (j, p) in [
        r.left_top() + vec2(7.0, 7.0) * k,
        r.right_top() + vec2(-7.0, 7.0) * k,
        r.left_bottom() + vec2(7.0, -7.0) * k,
        r.right_bottom() + vec2(-7.0, -7.0) * k,
    ]
    .into_iter()
    .enumerate()
    {
        b.circle(p, 2.6 * k, Fill::V(c(f.bezel_top), c(f.bezel_foot)));
        let a = hash(j as u32 + 300) * PI;
        let v = vec2(cos(a), sin(a)) * 1.8 * k;
        b.segment(p - v, p + v, 0.7 * k, c(f.bezel_foot), false);
    }
    step
}

/// The phosphor glass's ground in `r`: lighter at its middle, darker to its rim.
pub fn glass(b: &mut Brush, s: &Style, r: Rect) {
    let f = finish(s);
    b.rrect(r, 6.0 * s.k, Fill::Solid(c(f.glass_rim)));
    b.radial(
        r,
        r.center(),
        vec2(r.width() * 0.62, r.height() * 0.70),
        &[
            (0.0, c(f.glass)),
            (0.7, mix(c(f.glass), c(f.glass_rim), 0.45)),
            (1.0, c(f.glass_rim)),
        ],
        16,
    );
}

/// The glass over the screen, painted last: a soft reflection across its upper left, the
/// curve of its face catching the light along the top, its edge in shade.
pub fn glass_cover(b: &mut Brush, s: &Style, r: Rect) {
    let k = s.k;
    let glint = c(pal::GLINT);
    b.radial(
        r,
        pos2(r.left() + r.width() * 0.22, r.top() + r.height() * 0.10),
        vec2(r.width() * 0.34, r.height() * 0.30),
        &[
            (0.0, with_alpha(glint, 0.07)),
            (1.0, with_alpha(glint, 0.0)),
        ],
        12,
    );
    b.hline(
        r.left() + 14.0 * k,
        r.right() - 14.0 * k,
        r.top() + 1.5 * k,
        1.0 * k,
        with_alpha(glint, 0.10),
    );
    for (i, a) in [(0.0f32, 0.55f32), (1.0, 0.30), (2.0, 0.14)] {
        b.rrect_stroke(
            r.shrink((0.5 + i) * k),
            6.0 * k,
            1.0 * k,
            with_alpha(c(pal::SHADOW), a),
        );
    }
}

/// A push key in `r`: its well in the panel, the cap (sunk a little while it is in), the
/// lamp window across the cap's top lit in `lit` when the key is in. `hot` lightens it
/// under the pointer.
pub fn key(b: &mut Brush, s: &Style, r: Rect, lit: Option<Color32>, hot: bool) {
    let k = s.k;
    let f = finish(s);
    let well = r.expand(2.2 * k);
    b.rrect(well, 4.0 * k, Fill::V(c(pal::SHADOW), c(f.bezel)));
    let down = if lit.is_some() { 1.2 * k } else { 0.0 };
    let cap = r.translate(vec2(0.0, down));
    if lit.is_none() {
        b.rrect_soft(
            cap.translate(vec2(0.0, 1.8 * k)),
            3.0 * k,
            Fill::Solid(with_alpha(c(pal::SHADOW), 0.55)),
            2.0 * k,
        );
    }
    let (top, face, foot) = (c(f.key_top), c(f.key), c(f.key_foot));
    let face = if hot { mix(face, top, 0.35) } else { face };
    b.rrect(
        cap,
        3.0 * k,
        Fill::Stops(&[(0.0, top), (0.18, face), (0.85, face), (1.0, foot)]),
    );
    b.rrect_stroke(cap, 3.0 * k, 0.7 * k, with_alpha(foot, 0.9));
    // The lamp window: a strip of frosted glass across the cap's top.
    let win = Rect::from_min_max(
        cap.left_top() + vec2(4.0, 3.5) * k,
        pos2(
            cap.right() - 4.0 * k,
            cap.top() + 3.5 * k + (cap.height() * 0.30).max(4.0 * k),
        ),
    );
    match lit {
        Some(l) => {
            b.rrect_soft(
                win.expand(3.0 * k),
                3.0 * k,
                Fill::Solid(with_alpha(l, 0.35)),
                5.0 * k,
            );
            b.rrect(win, 1.5 * k, Fill::V(mix(l, c(pal::GLINT), 0.45), l));
        }
        None => {
            let off = c(pal::LAMP_OFF);
            b.rrect(win, 1.5 * k, Fill::V(off, mix(off, c(pal::SHADOW), 0.4)));
        }
    }
    b.hline(
        win.left(),
        win.right(),
        win.top() + 0.5 * k,
        0.6 * k,
        with_alpha(c(pal::GLINT), 0.35),
    );
}

/// The angle (radians, 0 up, clockwise) of `t` (0 … 1) over a dial's `sweep`.
pub fn dial_angle(t: f32, sweep: f32) -> f32 {
    -sweep * 0.5 + sweep * t.clamp(0.0, 1.0)
}

fn dir(a: f32) -> egui::Vec2 {
    vec2(sin(a), -cos(a))
}

/// The Depth dial at `centre`: the printed scale round it (a tick every two, a longer one
/// every ten), the lit arc of the value, the turned skirt with its fine knurl and the
/// index turning with the value, the fluted grip and its spun centre. `v` is 0 … 1 over
/// `sweep` radians centred on the top.
#[allow(clippy::too_many_arguments)]
pub fn dial(
    b: &mut Brush,
    s: &Style,
    centre: Pos2,
    rad: f32,
    v: f32,
    sweep: f32,
    hot: bool,
    lamp: Color32,
) {
    let k = s.k;
    let f = finish(s);
    for i in 0..=50 {
        let t = i as f32 / 50.0;
        let a = dial_angle(t, sweep);
        let big = i % 5 == 0;
        let r1 = if big { rad + 13.0 * k } else { rad + 9.0 * k };
        rule(
            b,
            s,
            centre + dir(a) * (rad + 5.0 * k),
            centre + dir(a) * r1,
            if big { 1.1 * k } else { 0.6 * k },
        );
    }
    let a1 = dial_angle(v, sweep);
    if v > 0.001 {
        b.arc(
            centre,
            rad + 2.5 * k,
            dial_angle(0.0, sweep) - PI * 0.5,
            a1 - PI * 0.5,
            2.2 * k,
            lamp,
            false,
        );
    }
    // The skirt.
    b.circle(
        centre + vec2(1.0, 3.0) * k,
        rad * 1.02,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.45)),
    );
    b.circle(centre, rad, Fill::V(c(f.handle_light), c(f.handle_dark)));
    b.circle(
        centre,
        rad * 0.95,
        Fill::V(c(f.handle), mix(c(f.handle_dark), c(f.handle), 0.5)),
    );
    for i in 0..72 {
        let a = a1 + i as f32 * TAU / 72.0;
        let d = dir(a);
        let lit = -d.y > 0.0;
        b.segment(
            centre + d * rad * 0.80,
            centre + d * rad * 0.94,
            0.6 * k,
            with_alpha(
                if lit {
                    c(f.handle_light)
                } else {
                    c(f.handle_dark)
                },
                0.55,
            ),
            false,
        );
    }
    let d = dir(a1);
    b.segment(
        centre + d * rad * 0.66,
        centre + d * rad * 0.97,
        2.4 * k,
        c(pal::SHADOW),
        true,
    );
    b.segment(
        centre + d * rad * 0.66,
        centre + d * rad * 0.97,
        1.4 * k,
        lamp,
        true,
    );
    // The grip: fluted, its flutes catching the light at the top.
    let g = rad * 0.66;
    b.circle(
        centre + vec2(0.0, 2.0) * k,
        g * 1.02,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.6)),
    );
    b.circle(centre, g, Fill::V(c(f.bezel_top), c(f.bezel_foot)));
    for i in 0..24 {
        let a = a1 + i as f32 * TAU / 24.0;
        let d = dir(a);
        let light = 0.5 - 0.5 * d.y;
        b.segment(
            centre + d * g * 0.72,
            centre + d * g * 0.98,
            1.4 * k,
            with_alpha(mix(c(f.bezel_foot), c(f.bezel_top), light), 0.95),
            false,
        );
    }
    let top = if hot {
        mix(c(f.handle_light), c(pal::GLINT), 0.3)
    } else {
        c(f.handle_light)
    };
    b.circle(centre, g * 0.62, Fill::V(top, c(f.handle_dark)));
    for i in 1..5 {
        b.circle_stroke(
            centre,
            g * 0.62 * i as f32 / 5.0,
            0.5 * k,
            with_alpha(c(f.handle_dark), 0.35),
        );
    }
    b.circle(
        centre,
        g * 0.10,
        Fill::V(c(f.handle_dark), c(f.handle_light)),
    );
}

/// The segments each digit lights (a b c d e f g, the usual order: top, top right, bottom
/// right, bottom, bottom left, top left, middle).
fn segments(ch: char) -> u8 {
    match ch {
        '0' => 0b011_1111,
        '1' => 0b000_0110,
        '2' => 0b101_1011,
        '3' => 0b100_1111,
        '4' => 0b110_0110,
        '5' => 0b110_1101,
        '6' => 0b111_1101,
        '7' => 0b000_0111,
        '8' => 0b111_1111,
        '9' => 0b110_1111,
        '-' => 0b100_0000,
        _ => 0,
    }
}

/// One segment as a long hexagon from `a` to `z`, `w` thick.
fn segment(b: &mut Brush, a: Pos2, z: Pos2, w: f32, col: Color32) {
    let d = z - a;
    let len = d.length().max(1e-3);
    let u = d / len;
    let n = vec2(-u.y, u.x) * (w * 0.5);
    let tip = u * (w * 0.5);
    let gap = u * (w * 0.30);
    let (a, z) = (a + gap, z - gap);
    b.polygon(
        &[a, a + tip + n, z - tip + n, z, z - tip - n, a + tip - n],
        col,
    );
}

/// The width a readout of `cells` digits takes at height `h`.
pub fn digits_width(cells: usize, h: f32) -> f32 {
    cells as f32 * h * 0.62
}

/// Seven-segment digits in `r` (right-aligned in `cells` cells): every segment drawn, the
/// unlit ones faint behind the smoked glass, the lit ones in `lit` with their glow. A '.'
/// lights the point of the digit before it.
pub fn digits(b: &mut Brush, s: &Style, r: Rect, text: &str, cells: usize, lit: Color32) {
    let k = s.k;
    let h = r.height();
    let cw = h * 0.62;
    let w = (h * 0.13).max(1.6 * k);
    let off = c(pal::LED_OFF);
    // The characters right-aligned, a point riding on the cell before it.
    let mut cell_chars = [(' ', false); 8];
    let n = cells.min(8);
    let mut i = n;
    for ch in text.chars().rev() {
        if ch == '.' {
            if i > 0 {
                cell_chars[i - 1].1 = true;
            }
            continue;
        }
        if i == 0 {
            break;
        }
        i -= 1;
        cell_chars[i].0 = ch;
    }
    // A point read before its digit: move it onto that digit.
    for j in (1..n).rev() {
        if cell_chars[j].1 && cell_chars[j].0 == ' ' {
            cell_chars[j].1 = false;
            cell_chars[j - 1].1 = true;
        }
    }
    let x0 = r.right() - cw * n as f32;
    let slant = h * 0.07;
    for (j, &(ch, dot)) in cell_chars.iter().take(n).enumerate() {
        let left = x0 + cw * j as f32 + cw * 0.10;
        let right = left + cw * 0.64;
        let (top, mid, bot) = (r.top() + w * 0.5, r.center().y, r.bottom() - w * 0.5);
        // The slant: the top leans right, as a bench meter's digits do.
        let at = |x: f32, y: f32| pos2(x + slant * (mid - y) / (h * 0.5), y);
        let segs = [
            (at(left, top), at(right, top)),
            (at(right, top), at(right, mid)),
            (at(right, mid), at(right, bot)),
            (at(left, bot), at(right, bot)),
            (at(left, mid), at(left, bot)),
            (at(left, top), at(left, mid)),
            (at(left, mid), at(right, mid)),
        ];
        let on = segments(ch);
        if on != 0 {
            b.radial(
                r.expand(4.0 * k),
                pos2((left + right) * 0.5, mid),
                vec2(cw * 0.75, h * 0.62),
                &[(0.0, with_alpha(lit, 0.16)), (1.0, with_alpha(lit, 0.0))],
                8,
            );
        }
        for (bit, (a, z)) in segs.into_iter().enumerate() {
            let col = if on & (1 << bit) != 0 { lit } else { off };
            segment(b, a, z, w, col);
        }
        let p = at(right + cw * 0.15, bot);
        b.circle(p, w * 0.6, Fill::Solid(if dot { lit } else { off }));
    }
}

/// The smoked window of a readout in `r`: a dark glass set in the panel, a glint on it.
pub fn smoke(b: &mut Brush, s: &Style, r: Rect) {
    let k = s.k;
    let f = finish(s);
    b.rrect(
        r.expand(2.0 * k),
        4.0 * k,
        Fill::V(c(f.bezel_foot), c(f.bezel_top)),
    );
    b.rrect(
        r,
        3.0 * k,
        Fill::V(c(pal::SMOKE), mix(c(pal::SMOKE), c(pal::SHADOW), 0.5)),
    );
}

/// The glint across a readout's glass, painted after its digits.
pub fn smoke_cover(b: &mut Brush, s: &Style, r: Rect) {
    let k = s.k;
    b.rect(
        Rect::from_min_max(r.left_top(), pos2(r.right(), r.top() + r.height() * 0.42)),
        Fill::V(
            with_alpha(c(pal::GLINT), 0.07),
            with_alpha(c(pal::GLINT), 0.0),
        ),
    );
    b.rrect_stroke(r, 3.0 * k, 0.8 * k, with_alpha(c(pal::SHADOW), 0.8));
}

/// A BNC socket at `p`: its square flange with four screws, the nickel shell with its two
/// bayonet pins, the white insulator, the gold centre pin.
pub fn socket(b: &mut Brush, s: &Style, p: Pos2, rad: f32) {
    let k = s.k;
    let flange = Rect::from_center_size(p, egui::Vec2::splat(rad * 2.9));
    b.rrect_soft(
        flange.translate(vec2(0.8, 1.8) * k),
        2.5 * k,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.40)),
        3.0 * k,
    );
    b.rrect(
        flange,
        2.5 * k,
        Fill::V(c(pal::NICKEL_LIGHT), c(pal::NICKEL_DARK)),
    );
    b.rrect(
        flange.shrink(1.0 * k),
        2.0 * k,
        Fill::V(
            c(pal::NICKEL),
            mix(c(pal::NICKEL), c(pal::NICKEL_DARK), 0.6),
        ),
    );
    for q in [
        flange.left_top() + vec2(1.0, 1.0) * rad * 0.42,
        flange.right_top() + vec2(-1.0, 1.0) * rad * 0.42,
        flange.left_bottom() + vec2(1.0, -1.0) * rad * 0.42,
        flange.right_bottom() + vec2(-1.0, -1.0) * rad * 0.42,
    ] {
        b.circle(
            q,
            rad * 0.16,
            Fill::V(c(pal::NICKEL_DARK), c(pal::NICKEL_LIGHT)),
        );
    }
    b.circle(
        p + vec2(0.6, 1.4) * k,
        rad,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.5)),
    );
    b.circle(p, rad, Fill::V(c(pal::NICKEL_LIGHT), c(pal::NICKEL_DARK)));
    b.circle_stroke(p, rad * 0.86, 0.6 * k, with_alpha(c(pal::NICKEL_DARK), 0.7));
    for side in [-1.0f32, 1.0] {
        let q = p + vec2(side * rad * 1.02, 0.0);
        b.circle(
            q,
            rad * 0.16,
            Fill::V(c(pal::NICKEL_LIGHT), c(pal::NICKEL_DARK)),
        );
    }
    b.circle(p, rad * 0.62, Fill::Solid(c(pal::NICKEL_DARK)));
    b.circle(
        p,
        rad * 0.52,
        Fill::V(
            c(pal::INSULATOR),
            mix(c(pal::INSULATOR), c(pal::SHADOW), 0.25),
        ),
    );
    b.circle(
        p,
        rad * 0.14,
        Fill::V(mix(c(pal::PIN), c(pal::GLINT), 0.4), c(pal::PIN)),
    );
}

/// A small signal lamp at `p`: lit by `level` (0 … 1) in green, its lens ringed.
pub fn signal(b: &mut Brush, s: &Style, p: Pos2, level: f32) {
    let k = s.k;
    let f = finish(s);
    let g = c(pal::SIGNAL);
    b.circle(p, 4.2 * k, Fill::V(c(f.bezel_foot), c(f.bezel_top)));
    let l = level.clamp(0.0, 1.0);
    b.circle(p, 3.0 * k, Fill::Solid(mix(c(pal::LAMP_OFF), g, l)));
    if l > 0.05 {
        b.radial(
            Rect::from_center_size(p, egui::Vec2::splat(20.0 * k)),
            p,
            egui::Vec2::splat(9.0 * k),
            &[(0.0, with_alpha(g, 0.45 * l)), (1.0, with_alpha(g, 0.0))],
            8,
        );
    }
    b.circle(
        p + vec2(-0.9, -0.9) * k,
        0.9 * k,
        Fill::Solid(with_alpha(c(pal::GLINT), 0.5)),
    );
}

/// The maker's emblem in `r`: calm water over rough stones — three stones on the foot in
/// the print's ink, each with its lit crown left bare, and two smooth lines of water over
/// them.
pub fn emblem(b: &mut Brush, s: &Style, r: Rect, ink: Color32, bare: Color32) {
    let k = s.k;
    let w = r.width();
    let h = r.height();
    for (cx, rw, rh) in [
        (0.20f32, 0.17f32, 0.30f32),
        (0.52, 0.22, 0.40),
        (0.83, 0.14, 0.24),
    ] {
        let c0 = pos2(r.left() + w * cx, r.bottom());
        let mut pts = [Pos2::ZERO; 24];
        for (i, p) in pts.iter_mut().enumerate() {
            let a = PI + PI * i as f32 / 23.0;
            let bump = 1.0 + 0.07 * sin(a * 3.0 + cx * 9.0);
            *p = c0 + vec2(cos(a) * w * rw * bump, sin(a) * h * rh * bump);
        }
        b.polygon(&pts, ink);
        // The stone's wet crown catching the light.
        let top = c0 - vec2(w * rw * 0.25, h * rh * 0.62);
        b.arc(
            top,
            w * rw * 0.42,
            -2.6,
            -1.6,
            1.0 * k,
            with_alpha(bare, 0.85),
            true,
        );
    }
    for (j, y) in [0.10f32, 0.36].into_iter().enumerate() {
        let mut pts = [Pos2::ZERO; 28];
        for (i, p) in pts.iter_mut().enumerate() {
            let t = i as f32 / 27.0;
            let yy = r.top() + h * y + h * 0.07 * sin(TAU * (t * 1.25 + j as f32 * 0.3));
            *p = pos2(r.left() + w * t, yy);
        }
        b.polyline(&pts, 1.6 * k, ink);
    }
}

/// The maker's badge in `r`: a nickel plate riveted to the panel, brushed along its length,
/// its emblem and `name` stamped into it and filled with black enamel. Returns the plate.
pub fn badge(b: &mut Brush, s: &Style, r: Rect, name: &Text<'_>, scale: f32) -> Rect {
    let k = s.k;
    b.rrect_soft(
        r.translate(vec2(0.8, 2.0) * k),
        4.0 * k,
        Fill::Solid(with_alpha(shadow(), 0.45)),
        4.0 * k,
    );
    b.rrect(
        r,
        4.0 * k,
        Fill::Stops(&[
            (0.0, c(pal::NICKEL_LIGHT)),
            (0.45, c(pal::NICKEL)),
            (1.0, c(pal::NICKEL_DARK)),
        ]),
    );
    // The brushing: fine lines along the plate.
    let n = (r.height() / (1.6 * k)) as u32;
    for i in 0..n {
        let y = r.top() + 2.0 * k + i as f32 * 1.6 * k;
        if y > r.bottom() - 2.0 * k {
            break;
        }
        let a = 0.08 + 0.10 * hash(i + 500);
        b.hline(
            r.left() + 3.0 * k,
            r.right() - 3.0 * k,
            y,
            0.5 * k,
            with_alpha(c(pal::NICKEL_LIGHT), a),
        );
    }
    b.rrect_stroke(r, 4.0 * k, 0.8 * k, with_alpha(c(pal::NICKEL_DARK), 0.9));
    b.rrect_stroke(
        r.shrink(1.2 * k),
        3.0 * k,
        0.6 * k,
        with_alpha(c(pal::NICKEL_LIGHT), 0.8),
    );
    // The rivets at its ends.
    for x in [r.left() + 7.0 * k, r.right() - 7.0 * k] {
        let p = pos2(x, r.center().y);
        b.circle(
            p + vec2(0.3, 0.8) * k,
            2.8 * k,
            Fill::Solid(with_alpha(shadow(), 0.45)),
        );
        b.circle(
            p,
            2.6 * k,
            Fill::V(c(pal::NICKEL_LIGHT), c(pal::NICKEL_DARK)),
        );
        b.circle(
            p - vec2(0.7, 0.7) * k,
            0.8 * k,
            Fill::Solid(with_alpha(glint(), 0.8)),
        );
    }
    // The stamped emblem and name, enamel-filled, a lit lip under each.
    let ink = c(pal::BADGE_INK);
    let em = Rect::from_min_size(
        pos2(r.left() + 16.0 * k, r.top() + 5.0 * k),
        vec2((r.height() - 10.0 * k) * 1.55, r.height() - 10.0 * k),
    );
    emblem(b, s, em, ink, c(pal::NICKEL_LIGHT));
    let at = pos2(em.right() + 9.0 * k, r.center().y);
    let lip = Text {
        color: with_alpha(c(pal::NICKEL_LIGHT), 0.9),
        ..*name
    };
    canvas::put(
        b,
        at + vec2(0.0, 0.8 * k),
        Align2::LEFT_CENTER,
        &lip,
        k * scale,
        None,
    );
    canvas::put(
        b,
        at,
        Align2::LEFT_CENTER,
        &Text {
            color: ink,
            ..*name
        },
        k * scale,
        None,
    );
    r
}

/// The badge's width for a name `name_w` wide, at height `h`.
pub fn badge_width(name_w: f32, h: f32, k: f32) -> f32 {
    16.0 * k + (h - 10.0 * k) * 1.55 + 9.0 * k + name_w + 17.0 * k
}
