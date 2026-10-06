//! Pictures of what is plugged in, drawn by hand in vectors (round 20): an interface's
//! front panel with its gain knobs, combo jacks and big monitor knob; a laptop; a pair of
//! headphones; a screen; a virtual cable. Each fits a box about twice as wide as tall,
//! lit from above like everything else in the kit, in the theme's own tokens.

use crate::audio_setup::interfaces::Kind;
use crate::kit::color::{mix, shade, with_alpha};
use crate::kit::light::{self, RaisedOpts};
use crate::kit::{Brush, Fill, Style};
use egui::{Pos2, Rect, Vec2};

/// Paint the picture of `kind` centred in `rect`. `lit` brightens its lamps.
pub fn paint(b: &mut Brush, rect: Rect, s: &Style, kind: Kind, lit: bool) {
    match kind {
        Kind::Interface => interface(b, rect, s, lit),
        Kind::BuiltIn => laptop(b, rect, s, lit),
        Kind::Headphones => headphones(b, rect, s, lit),
        Kind::Display => display(b, rect, s, lit),
        Kind::Virtual => cable(b, rect, s, lit),
    }
}

/// The largest box of aspect `w:h` centred in `rect`.
fn fit(rect: Rect, w: f32, h: f32) -> Rect {
    let k = (rect.width() / w).min(rect.height() / h);
    Rect::from_center_size(rect.center(), Vec2::new(w * k, h * k))
}

fn body(s: &Style) -> egui::Color32 {
    if s.t.dark {
        shade(s.t.ink, 0.06)
    } else {
        shade(s.t.panel_light, -0.30)
    }
}

/// A knob seen from the front: a dark disc, a rim, a pointer.
fn knob(b: &mut Brush, c: Pos2, r: f32, s: &Style, angle: f32) {
    b.circle(
        c + Vec2::new(0.0, r * 0.12),
        r * 1.05,
        Fill::Solid(with_alpha(s.t.shadow, 0.5)),
    );
    b.circle(
        c,
        r,
        Fill::V(shade(s.t.knob_body, 0.10), shade(s.t.knob_body, -0.08)),
    );
    b.circle_stroke(c, r, r * 0.12, s.t.knob_rim);
    let a = angle.to_radians();
    let tip = c + Vec2::new(a.sin(), -a.cos()) * r * 0.78;
    b.segment(
        c + (tip - c) * 0.25,
        tip,
        (r * 0.18).max(1.0),
        s.t.knob_pointer,
        true,
    );
}

/// A combo jack: a ring, a dark hole, the XLR's three pins.
fn jack(b: &mut Brush, c: Pos2, r: f32, s: &Style) {
    b.circle(
        c,
        r,
        Fill::V(shade(s.t.panel_light, 0.10), shade(s.t.panel_light, -0.15)),
    );
    b.circle(c, r * 0.78, Fill::Solid(shade(s.t.ink, -0.05)));
    b.circle(c, r * 0.30, Fill::Solid(shade(s.t.panel_light, -0.25)));
    for i in 0..3 {
        let a = (i as f32 * 120.0 + 90.0).to_radians();
        b.circle(
            c + Vec2::new(a.cos(), a.sin()) * r * 0.55,
            r * 0.09,
            Fill::Solid(s.t.text_dim),
        );
    }
}

fn interface(b: &mut Brush, rect: Rect, s: &Style, lit: bool) {
    let r = fit(rect, 2.2, 1.0);
    let u = r.height();
    let face = Rect::from_min_max(
        r.min + Vec2::new(0.0, u * 0.12),
        r.max - Vec2::new(0.0, u * 0.08),
    );
    // The case: raised, with its feet.
    for x in [face.left() + u * 0.25, face.right() - u * 0.25] {
        b.rrect(
            Rect::from_center_size(
                Pos2::new(x, face.bottom() + u * 0.04),
                Vec2::new(u * 0.22, u * 0.08),
            ),
            u * 0.03,
            Fill::Solid(shade(s.t.ink, -0.04)),
        );
    }
    light::raised(
        b,
        face,
        u * 0.08,
        body(s),
        RaisedOpts {
            k: s.k,
            depth: s.t.depth,
            ..Default::default()
        },
    );
    let cy = face.center().y + u * 0.04;
    let h = face.height();
    // Two inputs: a jack, and its gain knob above-right with a ring of signal lamps.
    for (i, x) in [0.17f32, 0.40].into_iter().enumerate() {
        let cx = face.left() + face.width() * x;
        jack(b, Pos2::new(cx - h * 0.06, cy + h * 0.06), h * 0.24, s);
        knob(
            b,
            Pos2::new(cx + h * 0.30, cy - h * 0.10),
            h * 0.14,
            s,
            if i == 0 { 40.0 } else { -30.0 },
        );
        let lamp = if lit && i == 0 {
            s.t.led
        } else {
            shade(s.t.led, -0.55)
        };
        b.circle(
            Pos2::new(cx + h * 0.30, cy + h * 0.22),
            h * 0.045,
            Fill::Solid(lamp),
        );
    }
    // The monitor knob, big, and the headphone jack under the brand line.
    let mx = face.left() + face.width() * 0.80;
    knob(b, Pos2::new(mx, cy - h * 0.02), h * 0.27, s, 60.0);
    b.circle(
        Pos2::new(face.left() + face.width() * 0.62, cy + h * 0.16),
        h * 0.08,
        Fill::Solid(shade(s.t.ink, -0.06)),
    );
    b.circle_stroke(
        Pos2::new(face.left() + face.width() * 0.62, cy + h * 0.16),
        h * 0.08,
        h * 0.02,
        s.t.text_dim,
    );
    // A USB lamp in the corner.
    let usb = if lit {
        s.t.focus
    } else {
        shade(s.t.focus, -0.5)
    };
    b.circle(
        Pos2::new(face.left() + face.width() * 0.62, cy - h * 0.20),
        h * 0.04,
        Fill::Solid(usb),
    );
}

fn laptop(b: &mut Brush, rect: Rect, s: &Style, lit: bool) {
    let r = fit(rect, 1.9, 1.0);
    let u = r.height();
    let screen = Rect::from_min_max(
        Pos2::new(r.left() + u * 0.28, r.top() + u * 0.06),
        Pos2::new(r.right() - u * 0.28, r.top() + u * 0.74),
    );
    light::raised(
        b,
        screen,
        u * 0.05,
        body(s),
        RaisedOpts {
            drop: false,
            k: s.k,
            depth: s.t.depth,
            ..Default::default()
        },
    );
    let glass = screen.shrink(u * 0.06);
    let glow = if lit {
        mix(s.t.focus, s.t.ink, 0.55)
    } else {
        shade(s.t.ink, -0.02)
    };
    b.rrect(
        glass,
        u * 0.02,
        Fill::V(shade(glow, 0.08), shade(glow, -0.06)),
    );
    // A waveform on the screen.
    let n = 24;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let y =
                (t * 18.0).sin() * (1.0 - (t - 0.5).abs() * 1.6).max(0.1) * glass.height() * 0.28;
            Pos2::new(glass.left() + glass.width() * t, glass.center().y + y)
        })
        .collect();
    b.polyline(&pts, (u * 0.025).max(1.0), with_alpha(s.t.selection, 0.8));
    let base = [
        Pos2::new(r.left() + u * 0.12, r.top() + u * 0.80),
        Pos2::new(r.right() - u * 0.12, r.top() + u * 0.80),
        Pos2::new(r.right(), r.top() + u * 0.92),
        Pos2::new(r.left(), r.top() + u * 0.92),
    ];
    b.polygon(&base, shade(body(s), 0.10));
    b.hline(
        base[0].x,
        base[1].x,
        base[0].y,
        (u * 0.02).max(1.0),
        shade(body(s), 0.22),
    );
}

fn headphones(b: &mut Brush, rect: Rect, s: &Style, lit: bool) {
    let r = fit(rect, 1.6, 1.0);
    let u = r.height();
    let c = Pos2::new(r.center().x, r.top() + u * 0.58);
    b.arc(
        c,
        u * 0.42,
        -170f32.to_radians(),
        -10f32.to_radians(),
        u * 0.09,
        body(s),
        true,
    );
    b.arc(
        c,
        u * 0.42,
        -160f32.to_radians(),
        -20f32.to_radians(),
        u * 0.03,
        shade(body(s), 0.18),
        true,
    );
    for side in [-1.0f32, 1.0] {
        let cup = Rect::from_center_size(
            Pos2::new(c.x + side * u * 0.42, c.y + u * 0.08),
            Vec2::new(u * 0.26, u * 0.42),
        );
        light::raised(
            b,
            cup,
            u * 0.10,
            body(s),
            RaisedOpts {
                k: s.k,
                depth: s.t.depth,
                ..Default::default()
            },
        );
        let pad = cup.shrink2(Vec2::new(u * 0.05, u * 0.07));
        b.rrect(pad, u * 0.06, Fill::Solid(shade(body(s), -0.10)));
    }
    if lit {
        b.circle(
            Pos2::new(c.x + u * 0.42, c.y + u * 0.22),
            u * 0.03,
            Fill::Solid(s.t.focus),
        );
    }
}

fn display(b: &mut Brush, rect: Rect, s: &Style, lit: bool) {
    let r = fit(rect, 1.7, 1.0);
    let u = r.height();
    let screen = Rect::from_min_max(
        r.min + Vec2::new(u * 0.05, u * 0.04),
        Pos2::new(r.right() - u * 0.05, r.top() + u * 0.72),
    );
    light::raised(
        b,
        screen,
        u * 0.04,
        body(s),
        RaisedOpts {
            k: s.k,
            depth: s.t.depth,
            ..Default::default()
        },
    );
    let glass = screen.shrink(u * 0.05);
    let glow = if lit {
        mix(s.t.focus, s.t.ink, 0.6)
    } else {
        shade(s.t.ink, -0.02)
    };
    b.rrect(
        glass,
        u * 0.02,
        Fill::V(shade(glow, 0.10), shade(glow, -0.05)),
    );
    let neck = Rect::from_center_size(
        Pos2::new(r.center().x, r.top() + u * 0.82),
        Vec2::new(u * 0.10, u * 0.20),
    );
    b.rect(neck, Fill::Solid(shade(body(s), 0.06)));
    let foot = Rect::from_center_size(
        Pos2::new(r.center().x, r.top() + u * 0.93),
        Vec2::new(u * 0.55, u * 0.06),
    );
    b.rrect(foot, u * 0.03, Fill::Solid(shade(body(s), 0.10)));
}

fn cable(b: &mut Brush, rect: Rect, s: &Style, lit: bool) {
    let r = fit(rect, 2.0, 1.0);
    let u = r.height();
    let a = Pos2::new(r.left() + u * 0.35, r.center().y + u * 0.15);
    let z = Pos2::new(r.right() - u * 0.35, r.center().y - u * 0.15);
    let n = 28;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let x = a.x + (z.x - a.x) * t;
            let y = a.y + (z.y - a.y) * t + (t * std::f32::consts::PI).sin() * u * 0.35;
            Pos2::new(x, y)
        })
        .collect();
    let wire = if lit { s.t.selection } else { s.t.text_dim };
    b.polyline(&pts, u * 0.07, shade(body(s), 0.12));
    b.polyline(&pts, u * 0.03, with_alpha(wire, 0.7));
    for p in [a, z] {
        let plug = Rect::from_center_size(p, Vec2::new(u * 0.26, u * 0.30));
        light::raised(
            b,
            plug,
            u * 0.06,
            body(s),
            RaisedOpts {
                k: s.k,
                depth: s.t.depth,
                ..Default::default()
            },
        );
        b.circle(p, u * 0.06, Fill::Solid(s.t.text_dim));
    }
}
