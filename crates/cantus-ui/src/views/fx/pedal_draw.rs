//! A pedal, drawn: the reusable parts of a pedalboard (Fragor, ADR 0131 — and any board
//! that shows effects as pedals). Data in, vectors out: an [`Enclosure`] is a finish
//! (`kit::skin::pedals`), a name, up to six knobs and a footswitch with its LED; a
//! [`cable`] joins two jacks; an [`empty_slot`] invites a pedal; a [`drop_marker`] shows
//! where a dragged pedal will land. Nothing here knows Fragor's five pedals or their
//! parameters — the editor hands each part what to show and reads back where things are
//! ([`Hits`]) to make them interactive.
//!
//! Every shape is at the style's scale (crisp at 200 %), every colour from the finish or
//! the theme's tokens; the words go through the kit's text (measured, elided, in the
//! locale).

use super::canvas;
use crate::kit::color::{ensure_contrast, mix, with_alpha, Hsl};
use crate::kit::skin::pedals::{self as pal, Finish};
use crate::kit::text::{self as ktext, Elide, Policy, Role, Src, Text};
use crate::kit::{Brush, Fill, Style};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};

/// The most knobs an enclosure carries.
pub const MAX_KNOBS: usize = 6;
/// An enclosure's proportions at 100 %: width × height.
pub const SIZE: egui::Vec2 = egui::Vec2::new(176.0, 296.0);

/// One knob on an enclosure.
#[derive(Copy, Clone, Debug)]
pub struct Knob<'a> {
    pub label: Src<'a>,
    /// 0 … 1.
    pub value: f32,
    /// Hovered or being dragged: its scale is lit.
    pub hot: bool,
}

/// What an enclosure shows.
#[derive(Copy, Clone, Debug)]
pub struct Enclosure<'a> {
    pub finish: &'a Finish,
    /// The name on the band.
    pub name: Src<'a>,
    /// A line under the band (what the circuit is), small.
    pub sub: Src<'a>,
    pub knobs: &'a [Knob<'a>],
    /// The footswitch: engaged or bypassed.
    pub on: bool,
    /// The LED's glow, 0 … 1 (follows the signal when engaged; 0 when bypassed).
    pub glow: f32,
    /// Lifted off the board (being dragged): a deeper shadow.
    pub lifted: bool,
    /// The footswitch is hovered.
    pub switch_hot: bool,
}

/// Where an enclosure's parts landed, for the editor's hit tests.
#[derive(Copy, Clone, Debug)]
pub struct Hits {
    pub body: Rect,
    pub knobs: [Rect; MAX_KNOBS],
    pub n: usize,
    pub switch: Rect,
    /// The jacks on its top edge: input on the left, output on the right.
    pub jack_in: Pos2,
    pub jack_out: Pos2,
}

fn c(h: Hsl) -> Color32 {
    h.rgb()
}

fn lighter(h: Hsl, dl: f32) -> Color32 {
    Hsl {
        l: (h.l + dl).clamp(0.0, 1.0),
        ..h
    }
    .rgb()
}

/// The knobs' centres and radius inside an enclosure `r` with `n` knobs: one row of up to
/// three, two rows above that.
fn knob_layout(r: Rect, n: usize, u: f32) -> ([Pos2; MAX_KNOBS], f32) {
    let mut out = [Pos2::ZERO; MAX_KNOBS];
    let rows = if n > 3 { 2 } else { 1 };
    let per = n.div_ceil(rows).max(1);
    let radius = if per >= 3 { 16.5 * u } else { 21.0 * u };
    let top = r.top() + 50.0 * u;
    let row_h = 66.0 * u;
    let mut i = 0;
    for row in 0..rows {
        let in_row = (n - row * per).min(per);
        let span = r.width() - 24.0 * u;
        for j in 0..in_row {
            let x = r.left() + 12.0 * u + span * (j as f32 + 0.5) / in_row as f32;
            out[i] = pos2(x, top + row as f32 * row_h);
            i += 1;
        }
    }
    (out, radius)
}

/// An enclosure in `r` (laid out in its own proportions: [`SIZE`] scaled to `r`'s width).
/// Shapes and words.
pub fn enclosure(b: &mut Brush, s: &Style, r: Rect, e: &Enclosure<'_>) -> Hits {
    let k = s.k;
    // The enclosure's own scale: everything on its face is measured in it.
    let u = r.width() / SIZE.x;
    let f = e.finish;
    let rr = 14.0 * u;
    // The shadow it casts on the board.
    let lift = if e.lifted { 18.0 } else { 7.0 } * u;
    b.rrect_soft(
        r.translate(vec2(0.0, lift)),
        rr,
        Fill::Solid(with_alpha(c(pal::SHADOW), if e.lifted { 0.6 } else { 0.5 })),
        lift * 1.6,
    );
    // The paint: lit from above, a bevel round the edge.
    b.rrect(
        r,
        rr,
        Fill::V(lighter(f.body, 0.08), lighter(f.body, -0.07)),
    );
    if f.metallic {
        // A fine sheen: hairlines a little lighter, every 3 points, barely there.
        let mut y = r.top() + 4.0 * u;
        while y < r.bottom() - 4.0 * u {
            b.hline(
                r.left() + 5.0 * u,
                r.right() - 5.0 * u,
                y,
                b.px(),
                with_alpha(lighter(f.body, 0.12), 0.10),
            );
            y += 3.0 * u;
        }
    }
    b.rrect_stroke(
        r.shrink(0.5 * b.px()),
        rr,
        b.px(),
        with_alpha(lighter(f.body, 0.32), 0.6),
    );
    b.rrect_stroke(
        r.shrink(3.5 * u),
        rr - 3.5 * u,
        b.px(),
        with_alpha(lighter(f.body, -0.22), 0.5),
    );
    // Four screws in the corners.
    for (dx, dy) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
        let cx = if dx > 0.0 { r.left() } else { r.right() } + dx * 11.0 * u;
        let cy = if dy > 0.0 { r.top() } else { r.bottom() } + dy * 11.0 * u;
        let p = pos2(cx, cy);
        b.circle(
            p,
            3.4 * u,
            Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)),
        );
        b.segment(
            p + vec2(-2.2, -1.1) * u,
            p + vec2(2.2, 1.1) * u,
            0.9 * u,
            c(pal::CHROME_DARK),
            false,
        );
    }
    // The jacks on the top edge.
    let jack_in = pos2(r.left() + 34.0 * u, r.top());
    let jack_out = pos2(r.right() - 34.0 * u, r.top());
    for j in [jack_in, jack_out] {
        let jr = Rect::from_center_size(j + vec2(0.0, -2.5 * u), vec2(20.0 * u, 8.0 * u));
        b.rrect(jr, 2.0 * u, Fill::V(c(pal::CHROME_LIGHT), c(pal::NICKEL)));
        b.rrect_stroke(jr, 2.0 * u, b.px(), c(pal::CHROME_DARK));
    }
    // The knobs: a printed scale, a skirted cap and its pointer, the name under it.
    let n = e.knobs.len().min(MAX_KNOBS);
    let (centres, kr) = knob_layout(r, n, u);
    let ink = c(f.ink);
    let mut hits = Hits {
        body: r,
        knobs: [Rect::NOTHING; MAX_KNOBS],
        n,
        switch: Rect::NOTHING,
        jack_in,
        jack_out,
    };
    let per = if n > 3 { n.div_ceil(2) } else { n.max(1) };
    for (i, kn) in e.knobs.iter().take(MAX_KNOBS).enumerate() {
        let p = centres[i];
        knob(b, s, p, kr, u, kn, f);
        hits.knobs[i] =
            Rect::from_center_size(p + vec2(0.0, 7.0 * u), vec2(kr * 2.7, kr * 2.0 + 30.0 * u));
        let t = Text {
            src: kn.label,
            role: Role::Micro,
            caps: true,
            align: Align2::CENTER_CENTER,
            policy: Policy::Elide(Elide::End),
            color: with_alpha(ink, if kn.hot { 1.0 } else { 0.9 }),
        };
        let w = r.width() / per as f32 - 4.0 * u;
        let lr = Rect::from_center_size(p + vec2(0.0, kr + 15.0 * u), vec2(w, 13.0 * k));
        ktext::paint(b, lr, &t, k);
    }
    // The band and the name.
    let rows = if n > 3 { 2.0 } else { 1.0 };
    let band_top = r.top() + (112.0 + 66.0 * (rows - 1.0)) * u;
    let band = Rect::from_min_max(
        pos2(r.left() + 12.0 * u, band_top),
        pos2(r.right() - 12.0 * u, band_top + 44.0 * u),
    );
    b.rrect(
        band,
        5.0 * u,
        Fill::V(lighter(f.band, 0.05), lighter(f.band, -0.05)),
    );
    b.hline(
        band.left() + 4.0 * u,
        band.right() - 4.0 * u,
        band.top() + b.px(),
        b.px(),
        with_alpha(lighter(f.band, 0.3), 0.5),
    );
    let name_ink = if ensure_contrast(c(f.body), c(f.band), 4.5) == c(f.body) {
        c(f.body)
    } else {
        ensure_contrast(ink, c(f.band), 4.5)
    };
    let t = Text {
        src: e.name,
        role: Role::Title,
        caps: true,
        align: Align2::CENTER_CENTER,
        policy: Policy::Elide(Elide::End),
        color: name_ink,
    };
    ktext::paint(b, band.shrink2(vec2(6.0 * u, 2.0 * u)), &t, k);
    let sub = Text {
        src: e.sub,
        role: Role::Micro,
        caps: false,
        align: Align2::CENTER_CENTER,
        policy: Policy::Elide(Elide::End),
        color: with_alpha(ensure_contrast(ink, c(f.body), 4.5), 0.92),
    };
    let sub_r = Rect::from_center_size(
        pos2(r.center().x, band.bottom() + 14.0 * u),
        vec2(r.width() - 22.0 * u, 13.0 * k),
    );
    ktext::paint(b, sub_r, &sub, k);
    // The LED and the footswitch.
    let led = pos2(r.center().x, band.bottom() + 40.0 * u);
    led_lamp(b, u, led, f, e.on, e.glow);
    let sw = pos2(r.center().x, r.bottom() - 48.0 * u);
    footswitch(b, s, u, sw, e.switch_hot);
    hits.switch = Rect::from_center_size(sw, vec2(64.0 * u, 64.0 * u));
    hits
}

/// A pedal knob at `p`, radius `kr` (`u` the enclosure's scale): a printed scale of eleven
/// ticks (lit up to the value), a dark skirt and cap lit from above, a white pointer.
pub fn knob(b: &mut Brush, s: &Style, p: Pos2, kr: f32, u: f32, kn: &Knob<'_>, f: &Finish) {
    let ink = c(f.ink);
    let v = kn.value.clamp(0.0, 1.0);
    for t in 0..=10 {
        let x = t as f32 / 10.0;
        let a = crate::kit::knob::angle(x);
        let d = vec2(a.cos(), a.sin());
        let lit = x <= v + 1e-4;
        let alpha = if lit { 0.95 } else { 0.32 };
        let (r0, r1) = if t % 5 == 0 {
            (kr + 3.0 * u, kr + 7.5 * u)
        } else {
            (kr + 3.5 * u, kr + 6.0 * u)
        };
        b.segment(
            p + d * r0,
            p + d * r1,
            1.3 * u,
            with_alpha(ink, alpha),
            true,
        );
    }
    b.circle(
        p + vec2(0.0, 3.0 * u),
        kr + 1.5 * u,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.5)),
    );
    // The skirt, knurled: dark, with a fine ring of teeth.
    b.circle(p, kr, Fill::V(lighter(f.cap, 0.10), lighter(f.cap, -0.05)));
    for t in 0..36 {
        let a = std::f32::consts::TAU * t as f32 / 36.0;
        let d = vec2(a.cos(), a.sin());
        b.segment(
            p + d * (kr * 0.80),
            p + d * (kr * 0.97),
            0.8 * u,
            with_alpha(lighter(f.cap, 0.22), 0.35),
            false,
        );
    }
    // The cap, lit from the upper left.
    b.circle(
        p,
        kr * 0.74,
        Fill::V(lighter(f.cap, 0.20), lighter(f.cap, 0.03)),
    );
    b.circle_stroke(p, kr * 0.74, b.px(), with_alpha(lighter(f.cap, 0.34), 0.7));
    let a = crate::kit::knob::angle(v);
    let d = vec2(a.cos(), a.sin());
    let pointer = if kn.hot {
        s.t.highlight
    } else {
        c(pal::CHROME_LIGHT)
    };
    b.segment(
        p + d * (kr * 0.16),
        p + d * (kr * 0.95),
        2.4 * u,
        pointer,
        true,
    );
}

/// The LED (`u` the enclosure's scale): dark glass when off; lit, a lens and a halo whose
/// size follows `glow`.
pub fn led_lamp(b: &mut Brush, u: f32, p: Pos2, f: &Finish, on: bool, glow: f32) {
    let lit = c(f.led);
    b.circle(p, 6.0 * u, Fill::Solid(c(pal::CHROME_DARK)));
    if on {
        let g = glow.clamp(0.0, 1.0);
        let halo = (11.0 + 12.0 * g) * u;
        b.radial(
            Rect::from_center_size(p, vec2(halo, halo) * 2.0),
            p,
            vec2(halo, halo),
            &[
                (0.0, with_alpha(lit, 0.55 + 0.35 * g)),
                (0.35, with_alpha(lit, 0.22 + 0.2 * g)),
                (1.0, with_alpha(lit, 0.0)),
            ],
            8,
        );
        b.circle(p, 4.4 * u, Fill::V(lighter(f.led, 0.18), lit));
        b.circle(
            p + vec2(-1.3, -1.3) * u,
            1.4 * u,
            Fill::Solid(with_alpha(c(pal::CHROME_LIGHT), 0.9)),
        );
    } else {
        b.circle(p, 4.4 * u, Fill::Solid(mix(lit, c(pal::SHADOW), 0.72)));
    }
}

/// A stomp footswitch at `p` (`u` the enclosure's scale): its hex nut and a chrome dome.
pub fn footswitch(b: &mut Brush, s: &Style, u: f32, p: Pos2, hot: bool) {
    let mut hex = [Pos2::ZERO; 6];
    for (i, h) in hex.iter_mut().enumerate() {
        let a = std::f32::consts::FRAC_PI_3 * i as f32 + std::f32::consts::FRAC_PI_6;
        *h = p + vec2(a.cos(), a.sin()) * 25.0 * u;
    }
    b.polygon(
        &hex.map(|h| h + vec2(0.0, 3.0 * u)),
        with_alpha(c(pal::SHADOW), 0.35),
    );
    b.polygon(&hex, c(pal::NICKEL));
    b.polygon(
        &hex.map(|h| p + (h - p) * 0.9),
        mix(c(pal::NICKEL), c(pal::CHROME_DARK), 0.3),
    );
    let dome = 16.5 * u;
    b.radial(
        Rect::from_center_size(p, vec2(dome, dome) * 2.0),
        p + vec2(-5.0, -6.0) * u,
        vec2(dome * 1.45, dome * 1.45),
        &[
            (0.0, c(pal::CHROME_LIGHT)),
            (0.5, c(pal::NICKEL)),
            (1.0, c(pal::CHROME_DARK)),
        ],
        8,
    );
    b.circle_stroke(p, dome, b.px(), c(pal::CHROME_DARK));
    if hot {
        b.circle_stroke(p, dome + 4.0 * u, 1.6 * u, s.t.highlight);
    }
}

/// A patch cable from jack `a` to jack `b_` (both on top edges): up out of one, over, and
/// down into the other, sagging `lift` above the higher jack; a nickel plug at each end.
pub fn cable(b: &mut Brush, s: &Style, a: Pos2, b_: Pos2, lift: f32) {
    let k = s.k;
    let top = a.y.min(b_.y) - lift;
    let c1 = pos2(a.x, top);
    let c2 = pos2(b_.x, top);
    let a0 = a - vec2(0.0, 12.0 * k);
    let b0 = b_ - vec2(0.0, 12.0 * k);
    let mut pts = [Pos2::ZERO; 33];
    for (i, q) in pts.iter_mut().enumerate() {
        let t = i as f32 / 32.0;
        let u = 1.0 - t;
        *q = pos2(
            u * u * u * a0.x + 3.0 * u * u * t * c1.x + 3.0 * u * t * t * c2.x + t * t * t * b0.x,
            u * u * u * a0.y + 3.0 * u * u * t * c1.y + 3.0 * u * t * t * c2.y + t * t * t * b0.y,
        );
    }
    let shade: [Pos2; 33] = pts.map(|q| q + vec2(0.0, 5.0 * k));
    b.polyline(&shade, 6.0 * k, with_alpha(c(pal::SHADOW), 0.30));
    b.polyline(&pts, 6.0 * k, c(pal::CABLE));
    let sheen: [Pos2; 33] = pts.map(|q| q + vec2(-1.0, -1.2) * k);
    b.polyline(&sheen, 1.4 * k, with_alpha(c(pal::CABLE_SHEEN), 0.8));
    for (j, end) in [(a, a0), (b_, b0)] {
        let plug = Rect::from_min_max(
            pos2(j.x - 5.0 * k, end.y - 2.0 * k),
            pos2(j.x + 5.0 * k, j.y - 4.0 * k),
        );
        b.rrect(
            plug,
            2.0 * k,
            Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)),
        );
        b.hline(
            plug.left(),
            plug.right(),
            plug.center().y,
            b.px(),
            c(pal::CHROME_DARK),
        );
    }
}

/// The board the pedals stand on: slats of the board's colour, a lip round the edge.
pub fn board(b: &mut Brush, s: &Style, r: Rect, dark: bool) {
    let k = s.k;
    let base = c(pal::BOARD.get(dark));
    let slat = c(pal::BOARD_SLAT.get(dark));
    b.rrect(r, 10.0 * k, Fill::V(slat, base));
    let mut y = r.top() + 26.0 * k;
    while y < r.bottom() - 10.0 * k {
        b.rect(
            Rect::from_min_max(
                pos2(r.left() + 8.0 * k, y),
                pos2(r.right() - 8.0 * k, y + 3.0 * k),
            ),
            Fill::Solid(with_alpha(c(pal::SHADOW), 0.25)),
        );
        b.hline(
            r.left() + 8.0 * k,
            r.right() - 8.0 * k,
            y + 3.0 * k,
            b.px(),
            with_alpha(slat, 0.9),
        );
        y += 34.0 * k;
    }
    b.rrect_stroke(r, 10.0 * k, 1.5 * k, with_alpha(c(pal::SHADOW), 0.5));
}

/// Words and marks on the board: the theme's secondary text, moved until it reads at
/// 4.5:1 on the board (which keeps its own colour in every theme).
pub fn board_ink(s: &Style) -> Color32 {
    let board = c(pal::BOARD_SLAT.get(s.t.dark));
    ensure_contrast(s.t.text_dim, board, 4.5)
}

/// An empty place on the board: a dashed outline, a plus and its words.
pub fn empty_slot(b: &mut Brush, s: &Style, r: Rect, label: Src<'_>, hot: bool) {
    let k = s.k;
    let ink = if hot { s.t.highlight } else { board_ink(s) };
    let x0 = r.left();
    let (x1, y0, y1) = (r.right(), r.top(), r.bottom());
    let corners = [
        pos2(x0, y0),
        pos2(x1, y0),
        pos2(x1, y1),
        pos2(x0, y1),
        pos2(x0, y0),
    ];
    b.dashed(&corners, 1.5 * k, ink, 6.0 * k, 5.0 * k);
    let p = r.center() - vec2(0.0, 14.0 * k);
    b.circle_stroke(p, 16.0 * k, 1.5 * k, ink);
    b.segment(
        p - vec2(7.0 * k, 0.0),
        p + vec2(7.0 * k, 0.0),
        2.0 * k,
        ink,
        true,
    );
    b.segment(
        p - vec2(0.0, 7.0 * k),
        p + vec2(0.0, 7.0 * k),
        2.0 * k,
        ink,
        true,
    );
    let t = Text {
        src: label,
        role: Role::Label,
        caps: false,
        align: Align2::CENTER_CENTER,
        policy: Policy::Elide(Elide::End),
        color: ink,
    };
    ktext::paint(
        b,
        Rect::from_center_size(
            r.center() + vec2(0.0, 22.0 * k),
            vec2(r.width() - 12.0 * k, 16.0 * k),
        ),
        &t,
        k,
    );
}

/// Where a dragged pedal will land: a lit bar at `x` from `y0` to `y1`.
pub fn drop_marker(b: &mut Brush, s: &Style, x: f32, y0: f32, y1: f32) {
    let k = s.k;
    let a = s.t.highlight;
    b.rect(
        Rect::from_min_max(pos2(x - 6.0 * k, y0), pos2(x + 6.0 * k, y1)),
        Fill::Solid(with_alpha(a, 0.18)),
    );
    b.vline(x, y0, y1, 3.0 * k, a);
    for y in [y0, y1] {
        b.circle(pos2(x, y), 5.0 * k, Fill::Solid(a));
    }
}

/// A jack on the board's edge (where the track comes in or goes out), with its word.
pub fn board_jack(b: &mut Brush, s: &Style, p: Pos2, label: Src<'_>, ink: Color32) {
    let k = s.k;
    let jr = Rect::from_center_size(p, vec2(20.0 * k, 9.0 * k));
    b.rrect(jr, 2.5 * k, Fill::V(c(pal::CHROME_LIGHT), c(pal::NICKEL)));
    b.rrect_stroke(jr, 2.5 * k, b.px(), c(pal::CHROME_DARK));
    canvas::put(
        b,
        p + vec2(0.0, 16.0 * k),
        Align2::CENTER_CENTER,
        &Text {
            src: label,
            role: Role::Micro,
            caps: true,
            align: Align2::CENTER_CENTER,
            policy: Policy::Elide(Elide::End),
            color: ink,
        },
        k,
        None,
    );
}
