//! A dancer's frame and grips (ADR 0091 §1), kit-drawn: they appear while the pointer is
//! over the dancer and fade when it leaves, so the stage stays clean on screen and on
//! stream. Around the body a thin accent line with a soft glow; above it a pill — the
//! stage's mark, the track's name, and the buttons (pin on top, dock or float, hide); at
//! its top-right corner a round grip that resizes it about its feet.
//!
//! Pure layout and painting, like the rest of the kit; [`super::float`] and [`super::dock`]
//! sense the rects and turn them into actions.

use crate::kit::color::{shade, with_alpha};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{Brush, Fill, Style};
use egui::{pos2, vec2, Align2, Color32, Context, Pos2, Rect, Vec2};

/// The pill's height at 100 %.
pub const BAR_H: f32 = 26.0;
/// A button's width in the pill at 100 %.
pub const BUTTON_W: f32 = 24.0;
/// The resize grip's radius at 100 %.
pub const GRIP_R: f32 = 8.0;
/// The widest a name shows, at 100 %.
const NAME_W: f32 = 132.0;
/// The gap between the pill and the body.
const GAP: f32 = 6.0;

/// A button of the pill.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Button {
    /// Keep it above the floating panels (`view.top`).
    Pin,
    /// Put it on the Stage tab.
    Dock,
    /// Take it off the Stage tab, over the workspace.
    Float,
    /// Hide it (`view.mode` = hidden).
    Hide,
}

impl Button {
    /// The i18n key of its tooltip (`lit`: pinned).
    pub fn tip_key(self, lit: bool) -> &'static str {
        match self {
            Button::Pin if lit => "stage.tip.unpin",
            Button::Pin => "stage.tip.pin",
            Button::Dock => "stage.tip.dock",
            Button::Float => "stage.tip.float",
            Button::Hide => "stage.tip.hide",
        }
    }
}

/// Where the frame's parts are.
#[derive(Clone, Debug, PartialEq)]
pub struct Chrome {
    pub body: Rect,
    /// The pill, and the name's place in it.
    pub bar: Rect,
    pub name: Rect,
    pub buttons: Vec<(Button, Rect)>,
    pub grip: Rect,
}

impl Chrome {
    /// Everything the frame covers (what the pointer may be over).
    pub fn extent(&self) -> Rect {
        let g = self.grip;
        self.body.union(self.bar).union(g)
    }
}

/// Lay out the frame around `body`, its pill kept inside `bounds`.
pub fn layout(
    ctx: &Context,
    body: Rect,
    name: &str,
    buttons: &[Button],
    s: &Style,
    bounds: Rect,
) -> Chrome {
    let k = s.k;
    let h = BAR_H * k;
    let t = Text::new(name, Role::Label, s.t.menu_text);
    let name_w = text::measure(ctx, &t, s.tk()).x.min(NAME_W * k).ceil();
    let w = 8.0 * k
        + 16.0 * k
        + 6.0 * k
        + name_w
        + 8.0 * k
        + buttons.len() as f32 * BUTTON_W * k
        + 4.0 * k;
    let mut top = body.top() - GAP * k - h;
    if top < bounds.top() + 2.0 * k {
        // No room above (the dancer near the top): inside the body's top instead.
        top = body.top() + GAP * k;
    }
    let mut left = body.center().x - w * 0.5;
    left = left
        .min(bounds.right() - w - 2.0 * k)
        .max(bounds.left() + 2.0 * k);
    let bar = Rect::from_min_size(pos2(left, top), vec2(w, h));
    let name = Rect::from_min_size(pos2(left + 30.0 * k, top), vec2(name_w, h));
    let mut x = name.right() + 8.0 * k;
    let buttons = buttons
        .iter()
        .map(|b| {
            let r = Rect::from_min_size(pos2(x, top), vec2(BUTTON_W * k, h)).shrink(2.0 * k);
            x += BUTTON_W * k;
            (*b, r)
        })
        .collect();
    let gr = GRIP_R * k;
    let grip = Rect::from_center_size(body.right_top(), Vec2::splat(gr * 2.0));
    Chrome {
        body,
        bar,
        name,
        buttons,
        grip,
    }
}

/// What is lit while it is painted.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Lit {
    /// The part under the pointer.
    pub hot: Option<Button>,
    pub grip_hot: bool,
    pub body_hot: bool,
    /// Pinned on top.
    pub pinned: bool,
    /// 0…1: the frame fading in and out.
    pub fade: f32,
}

/// Paint the frame.
pub fn paint(b: &mut Brush, c: &Chrome, name: &str, s: &Style, lit: Lit) {
    let k = s.k;
    let a = lit.fade.clamp(0.0, 1.0);
    if a <= 0.0 {
        return;
    }
    let accent = s.t.highlight;
    // The body's frame: a soft glow, a thin line.
    let body = c.body;
    b.rrect_soft(
        body.expand(4.0 * k),
        12.0 * k,
        Fill::Solid(with_alpha(accent, 0.10 * a)),
        10.0 * k,
    );
    let line = if lit.body_hot { 0.95 } else { 0.7 };
    b.rrect_stroke(body, 8.0 * k, 1.25 * k, with_alpha(accent, line * a));

    // The pill: a shadow, its face, its edge.
    let bar = c.bar;
    let r = bar.height() * 0.5;
    b.rrect_soft(
        bar.translate(vec2(0.0, 2.0 * k)),
        r,
        Fill::Solid(with_alpha(s.t.shadow, 0.55 * a)),
        8.0 * k,
    );
    let face = s.t.menu;
    b.rrect(
        bar,
        r,
        Fill::V(
            with_alpha(shade(face, 0.05), 0.97 * a),
            with_alpha(face, 0.97 * a),
        ),
    );
    b.rrect_stroke(bar, r, b.px(), with_alpha(s.t.edge, 0.9 * a));
    // The stage's mark, then the name.
    let mark = Rect::from_center_size(
        pos2(bar.left() + 8.0 * k + 8.0 * k, bar.center().y),
        Vec2::splat(15.0 * k),
    );
    let w = (1.25 * mark.width() / 12.0).max(b.px());
    glyph(
        b,
        mark.center(),
        mark.width() / 12.0,
        w,
        with_alpha(accent, a),
    );
    let t = Text::new(name, Role::Label, with_alpha(s.t.menu_text, a))
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let _ = text::paint(b, c.name, &t, s.tk());
    // The buttons.
    for (btn, rect) in &c.buttons {
        let hot = lit.hot == Some(*btn);
        if hot {
            b.rrect(
                *rect,
                rect.height() * 0.5,
                Fill::Solid(with_alpha(s.t.menu_hover, a)),
            );
        }
        let on = *btn == Button::Pin && lit.pinned;
        let color = if on {
            s.t.highlight
        } else if hot {
            s.t.menu_text_hover
        } else {
            s.t.menu_text
        };
        icon(b, rect.shrink(3.0 * k), *btn, with_alpha(color, a));
    }
    // The resize grip.
    let g = c.grip;
    let gr = g.width() * 0.5;
    b.circle(
        g.center() + vec2(0.0, 1.5 * k),
        gr + 1.5 * k,
        Fill::Solid(with_alpha(s.t.shadow, 0.45 * a)),
    );
    let gf = if lit.grip_hot { accent } else { face };
    b.circle(g.center(), gr, Fill::Solid(with_alpha(gf, a)));
    b.circle_stroke(g.center(), gr, b.px(), with_alpha(accent, a));
    let ic = if lit.grip_hot {
        crate::kit::color::on_fill(accent)
    } else {
        s.t.menu_text
    };
    resize_arrows(b, g.shrink(gr * 0.45), with_alpha(ic, a));
}

/// The stage's mark (the Stage tab's glyph): a small figure dancing, arms up, on a floor.
/// `c` is the centre, `u` the glyph unit (12 across), `w` the line width.
pub fn glyph(b: &mut Brush, c: Pos2, u: f32, w: f32, color: Color32) {
    let p = |x: f32, y: f32| c + vec2(x * u, y * u);
    b.circle(p(0.6, -3.9), 1.55 * u, Fill::Solid(color));
    b.segment(p(0.3, -1.9), p(-0.2, 1.4), w * 1.1, color, true);
    b.segment(p(0.2, -1.2), p(-3.2, -3.6), w, color, true);
    b.segment(p(0.2, -1.2), p(3.4, -0.4), w, color, true);
    b.segment(p(-0.2, 1.4), p(-2.6, 3.6), w, color, true);
    b.segment(p(-0.2, 1.4), p(2.0, 2.4), w, color, true);
    b.segment(p(2.0, 2.4), p(2.6, 4.4), w, color, true);
    b.segment(p(-5.0, 5.2), p(5.0, 5.2), w * 0.7, color, true);
}

/// A button's icon in `rect`.
pub fn icon(b: &mut Brush, rect: Rect, btn: Button, color: Color32) {
    let s = rect.width().min(rect.height());
    let c = pos2(b.snap_mid(rect.center().x), b.snap_mid(rect.center().y));
    let u = s / 12.0;
    let w = (1.25 * u).max(b.px());
    let p = |x: f32, y: f32| c + vec2(x * u, y * u);
    match btn {
        // A push pin: its head, its collar, its point.
        Button::Pin => {
            b.polygon(
                &[p(-2.4, -5.0), p(2.4, -5.0), p(1.6, -0.6), p(-1.6, -0.6)],
                color,
            );
            b.segment(p(-4.0, -0.4), p(4.0, -0.4), w * 1.2, color, true);
            b.segment(p(0.0, -0.4), p(0.0, 5.2), w, color, true);
        }
        // A window with its lower part filled: put it in the dock.
        Button::Dock => {
            let r = Rect::from_min_max(p(-5.0, -4.5), p(5.0, 4.5));
            b.rrect_stroke(r, 1.2 * u, w, color);
            b.rect(
                Rect::from_min_max(p(-5.0, 1.0), p(5.0, 4.5)),
                Fill::Solid(color),
            );
        }
        // A window and an arrow leaving it: float it.
        Button::Float => {
            let r = Rect::from_min_max(p(-5.0, -2.5), p(2.5, 5.0));
            b.rrect_stroke(r, 1.2 * u, w, color);
            b.segment(p(-0.5, 0.5), p(5.0, -5.0), w, color, true);
            b.segment(p(5.0, -5.0), p(1.0, -5.0), w, color, true);
            b.segment(p(5.0, -5.0), p(5.0, -1.0), w, color, true);
        }
        // An eye, struck through: hide it.
        Button::Hide => {
            let top: [Pos2; 9] = std::array::from_fn(|i| {
                let t = i as f32 / 8.0;
                let x = -5.5 + 11.0 * t;
                p(x, -3.4 * (1.0 - ((t - 0.5) * 2.0).powi(2)))
            });
            let bottom: [Pos2; 9] = std::array::from_fn(|i| {
                let t = i as f32 / 8.0;
                let x = -5.5 + 11.0 * t;
                p(x, 3.4 * (1.0 - ((t - 0.5) * 2.0).powi(2)))
            });
            b.polyline(&top, w, color);
            b.polyline(&bottom, w, color);
            b.circle(c, 1.7 * u, Fill::Solid(color));
            b.segment(p(-4.6, 4.6), p(4.6, -4.6), w, color, true);
        }
    }
}

/// ↙↗ in `rect`: the resize grip's mark.
fn resize_arrows(b: &mut Brush, rect: Rect, color: Color32) {
    let s = rect.width().min(rect.height());
    let c = rect.center();
    let u = s / 12.0;
    let w = (1.5 * u).max(b.px());
    let p = |x: f32, y: f32| c + vec2(x * u, y * u);
    b.segment(p(-5.0, 5.0), p(5.0, -5.0), w, color, true);
    b.segment(p(5.0, -5.0), p(0.5, -5.0), w, color, true);
    b.segment(p(5.0, -5.0), p(5.0, -0.5), w, color, true);
    b.segment(p(-5.0, 5.0), p(-0.5, 5.0), w, color, true);
    b.segment(p(-5.0, 5.0), p(-5.0, 0.5), w, color, true);
}
