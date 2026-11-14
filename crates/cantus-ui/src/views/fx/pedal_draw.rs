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
use super::pedal_art;
pub use super::pedal_art::{Letter, Motif, Texture};
use crate::kit::color::{ensure_contrast, mix, with_alpha, Hsl};
use crate::kit::skin::pedals::{self as pal, Finish};
use crate::kit::text::{self as ktext, Elide, Policy, Role, Src, Text};
use crate::kit::{Brush, Fill, Style};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};

/// The most knobs an enclosure carries.
pub const MAX_KNOBS: usize = 6;
/// A standard enclosure's proportions at 100 %: width × height.
pub const SIZE: egui::Vec2 = egui::Vec2::new(176.0, 296.0);
pub use crate::kit::skin::pedals::Shape;

/// A shape's proportions at 100 %: width × height.
pub fn size_of(shape: Shape) -> egui::Vec2 {
    let (w, h) = shape.size();
    egui::Vec2::new(w, h)
}

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
    /// The name lettered on the face, under the artwork.
    pub name: Src<'a>,
    /// A line under the name (what the circuit is), small.
    pub sub: Src<'a>,
    /// The artwork printed on the face.
    pub motif: Motif,
    /// How its name is lettered.
    pub letter: Letter,
    /// The paint's surface.
    pub texture: Texture,
    /// The case: mini, standard, wide, double or round.
    pub shape: Shape,
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

/// Where a shape puts its parts, in its own units (`u`, the enclosure's scale).
struct Layout {
    knobs: [Pos2; MAX_KNOBS],
    radii: [f32; MAX_KNOBS],
    art: Rect,
    name_y: f32,
    led: Pos2,
    switch: Pos2,
    jacks: [Pos2; 2],
}

fn layout(r: Rect, shape: Shape, n: usize, u: f32) -> Layout {
    let mut knobs = [Pos2::ZERO; MAX_KNOBS];
    let mut radii = [0.0; MAX_KNOBS];
    let (l, t, w) = (r.left(), r.top(), r.width());
    let cx = r.center().x;
    let row = |knobs: &mut [Pos2; MAX_KNOBS],
               radii: &mut [f32; MAX_KNOBS],
               from: usize,
               count: usize,
               y: f32,
               span: f32,
               kr: f32| {
        for j in 0..count {
            knobs[from + j] = pos2(cx - span * 0.5 + span * (j as f32 + 0.5) / count as f32, y);
            radii[from + j] = kr;
        }
    };
    let jacks = [pos2(l + 34.0 * u, t), pos2(r.right() - 34.0 * u, t)];
    match shape {
        Shape::Mini => {
            // Two knobs side by side, a third smaller one under them.
            let top = n.min(2);
            row(
                &mut knobs,
                &mut radii,
                0,
                top,
                t + 50.0 * u,
                w - 16.0 * u,
                15.0 * u,
            );
            if n > 2 {
                knobs[2] = pos2(cx, t + 102.0 * u);
                radii[2] = 12.0 * u;
            }
            let art_top = if n > 2 { 132.0 } else { 90.0 };
            Layout {
                knobs,
                radii,
                art: Rect::from_min_max(
                    pos2(l + 10.0 * u, t + art_top * u),
                    pos2(r.right() - 10.0 * u, r.bottom() - 74.0 * u),
                ),
                name_y: r.bottom() - 82.0 * u,
                led: pos2(cx, t + 17.0 * u),
                switch: pos2(cx, r.bottom() - 36.0 * u),
                jacks: [pos2(l + 26.0 * u, t), pos2(r.right() - 26.0 * u, t)],
            }
        }
        Shape::Round => {
            // Three knobs across the upper disc, the art in its middle, the switch below.
            let rad = w * 0.5;
            let cy = t + rad;
            row(
                &mut knobs,
                &mut radii,
                0,
                n.min(3),
                t + 70.0 * u,
                174.0 * u,
                17.0 * u,
            );
            let jy = |x: f32| cy - (rad * rad - (x - cx) * (x - cx)).max(0.0).sqrt();
            let (jl, jr) = (cx - rad * 0.42, cx + rad * 0.42);
            Layout {
                knobs,
                radii,
                art: Rect::from_center_size(pos2(cx, t + 142.0 * u), vec2(150.0 * u, 60.0 * u)),
                name_y: t + 172.0 * u,
                led: pos2(cx, t + 26.0 * u),
                switch: pos2(cx, t + 206.0 * u),
                jacks: [pos2(jl, jy(jl)), pos2(jr, jy(jr))],
            }
        }
        Shape::Standard | Shape::Wide | Shape::Double => {
            let per = if n > 3 { n.div_ceil(2) } else { n.max(1) };
            let kr = match shape {
                Shape::Standard => 16.5,
                _ => 19.0,
            } * u;
            let span = w - 24.0 * u;
            row(
                &mut knobs,
                &mut radii,
                0,
                n.min(per),
                t + 50.0 * u,
                span,
                kr,
            );
            if n > per {
                row(
                    &mut knobs,
                    &mut radii,
                    per,
                    n - per,
                    t + 116.0 * u,
                    span,
                    kr,
                );
            }
            let rows = if n > per { 2.0 } else { 1.0 };
            Layout {
                knobs,
                radii,
                art: Rect::from_min_max(
                    pos2(l + 14.0 * u, t + (94.0 + 66.0 * (rows - 1.0)) * u),
                    pos2(r.right() - 14.0 * u, r.bottom() - 74.0 * u),
                ),
                name_y: r.bottom() - 84.0 * u,
                led: pos2(cx, t + 17.0 * u),
                switch: pos2(cx, r.bottom() - 38.0 * u),
                jacks,
            }
        }
    }
}

/// An enclosure in `r`, laid out in its own proportions (`r` is [`size_of`] its shape at
/// some scale). Shapes and words.
pub fn enclosure(b: &mut Brush, s: &Style, r: Rect, e: &Enclosure<'_>) -> Hits {
    let k = s.k;
    // The enclosure's own scale: everything on its face is measured in it.
    let u = r.width() / size_of(e.shape).x;
    let f = e.finish;
    let round = e.shape == Shape::Round;
    let rr = if round { r.width() * 0.5 } else { 14.0 * u };
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
    let print = ensure_contrast(c(f.ink), c(f.body), 4.5);
    pedal_art::texture(
        b,
        u,
        if round { r.shrink(r.width() * 0.15) } else { r },
        e.texture,
        lighter(f.body, 0.30),
        lighter(f.body, -0.25),
        e.motif as u32 * 7919 + 17,
    );
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
    if !round {
        pedal_art::border(b, u, r, e.motif, print, c(f.band));
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
    } else {
        // A round case: a chrome rim and a ring of fine dots inside it.
        b.circle_stroke(
            r.center(),
            r.width() * 0.5 - 1.5 * u,
            2.0 * u,
            with_alpha(c(pal::CHROME_LIGHT), 0.7),
        );
        for i in 0..60 {
            let a = std::f32::consts::TAU * i as f32 / 60.0;
            let p = r.center() + vec2(a.cos(), a.sin()) * (r.width() * 0.5 - 9.0 * u);
            b.circle(p, 0.8 * u, Fill::Solid(with_alpha(c(f.band), 0.6)));
        }
    }
    let n = e.knobs.len().min(MAX_KNOBS);
    let lay = layout(r, e.shape, n, u);
    // The jacks on the top edge.
    for j in lay.jacks {
        let jr = Rect::from_center_size(j + vec2(0.0, -2.5 * u), vec2(20.0 * u, 8.0 * u));
        b.rrect(jr, 2.0 * u, Fill::V(c(pal::CHROME_LIGHT), c(pal::NICKEL)));
        b.rrect_stroke(jr, 2.0 * u, b.px(), c(pal::CHROME_DARK));
    }
    let ink = c(f.ink);
    let mut hits = Hits {
        body: r,
        knobs: [Rect::NOTHING; MAX_KNOBS],
        n,
        switch: Rect::NOTHING,
        jack_in: lay.jacks[0],
        jack_out: lay.jacks[1],
    };
    // The artwork first, under the knobs.
    let accent = c(f.band);
    pedal_art::motif(b, u, lay.art, e.motif, print, accent);
    // The knobs: a printed scale, a skirted cap and its pointer, the name under it.
    for (i, kn) in e.knobs.iter().take(MAX_KNOBS).enumerate() {
        let (p, kr) = (lay.knobs[i], lay.radii[i]);
        knob(b, s, p, kr, u, kn, f);
        hits.knobs[i] =
            Rect::from_center_size(p + vec2(0.0, 7.0 * u), vec2(kr * 2.7, kr * 2.0 + 30.0 * u));
        let t = Text {
            src: kn.label,
            role: Role::Micro,
            caps: true,
            align: Align2::CENTER_CENTER,
            policy: Policy::Elide(Elide::End),
            color: with_alpha(print, if kn.hot { 1.0 } else { 0.92 }),
        };
        let lw = (kr * 2.0 + 30.0 * u).max(46.0 * u);
        let lr = Rect::from_center_size(p + vec2(0.0, kr + 14.0 * u), vec2(lw, 13.0 * k));
        b.rrect(
            Rect::from_center_size(
                lr.center(),
                vec2(ktext::measure(b.ctx(), &t, k).x.min(lw) + 6.0 * u, 12.0 * k),
            ),
            2.0 * u,
            Fill::Solid(with_alpha(c(f.body), 0.75)),
        );
        ktext::paint(b, lr, &t, k);
    }
    let _ = ink;
    let name_w = if round {
        168.0 * u
    } else {
        r.width() - 16.0 * u
    };
    let name_r = Rect::from_center_size(pos2(r.center().x, lay.name_y), vec2(name_w, 22.0 * k));
    pedal_art::lettering(b, k, u, name_r, e.name, e.letter, print, accent, c(f.body));
    led_lamp(b, u, lay.led, f, e.on, e.glow);
    footswitch(b, s, u, lay.switch, e.switch_hot);
    hits.switch = Rect::from_center_size(lay.switch, vec2(64.0 * u, 64.0 * u));
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
            0.9 * u,
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
        1.8 * u,
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
    cable_with(b, s, a, b_, lift, None);
}

/// The signal in a cable: its level (0 … 1, at that point of the chain) and the clock the
/// streaks travel by (seconds).
#[derive(Copy, Clone, Debug)]
pub struct Energy {
    pub level: f32,
    pub time: f64,
}

/// [`cable`] with energy flowing in it: a warm core and streaks of amber light travelling
/// from `a` to `b_` (the signal's direction), as bright as the level there — the mixer's
/// cable gleam (round 39), in the valves' amber. At silence nothing moves and nothing
/// glows.
pub fn cable_with(b: &mut Brush, s: &Style, a: Pos2, b_: Pos2, lift: f32, energy: Option<Energy>) {
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
    if let Some(e) = energy.filter(|e| e.level > 0.01) {
        use crate::views::mixer::wires;
        let l = e.level.clamp(0.0, 1.0);
        let amber = c(pal::JEWEL);
        let light = crate::kit::color::mix(amber, c(pal::CHROME_LIGHT), 0.55);
        b.polyline(&pts, 2.4 * k, with_alpha(amber, 0.30 + 0.30 * l));
        let a_g = wires::gleam_alpha(l) * (0.6 + 0.4 * l);
        let o = wires::gleam_offset(e.time, l) * k;
        wires::gleams_of(&pts, o, wires::GLEAM * 2.4 * k, |p, q| {
            // A halo and a hot core: a pulse of light moving down the cable.
            b.segment(p, q, 5.0 * k, with_alpha(amber, 0.35 * a_g), true);
            b.segment(p, q, 2.4 * k, with_alpha(light, a_g), true);
        });
    }
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
/// A slot's place in the chain, a small numbered tag on the board above it (`n`: "1" …):
/// the slot's own control — its parameter's menu (automate, learn) is offered on it.
pub fn slot_tag(b: &mut Brush, s: &Style, r: Rect, n: &str, hot: bool) {
    let k = s.k;
    let ink = if hot { s.t.highlight } else { board_ink(s) };
    b.rrect(
        r,
        3.0 * k,
        Fill::Solid(with_alpha(c(pal::BOARD_SLAT.get(s.t.dark)), 0.9)),
    );
    b.rrect_stroke(r, 3.0 * k, b.px(), with_alpha(ink, 0.8));
    canvas::put(
        b,
        r.center(),
        Align2::CENTER_CENTER,
        &Text {
            src: Src::Str(n),
            role: Role::Micro,
            caps: false,
            align: Align2::CENTER_CENTER,
            policy: Policy::Elide(Elide::End),
            color: ink,
        },
        k,
        None,
    );
}

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
