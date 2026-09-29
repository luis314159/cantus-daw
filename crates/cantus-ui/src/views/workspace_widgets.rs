//! Small controls the workspace's own pages need, drawn with the kit's painters and its
//! lighting recipe: a push button, a glyph key, a colour swatch, a text field in a well,
//! a section band. Each writes its hint like a kit widget.

use crate::kit::color::{shade, with_alpha};
use crate::kit::glyph::{self, Glyph};
use crate::kit::hint::{self, Caps};
use crate::kit::light::{self, RaisedOpts};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{band, focus_ring, with_brush, Fill, State, Style, Tok};
use egui::{Align2, Color32, Rect, Response, Sense, Ui, Vec2};

/// The tokens a push button paints with.
pub const BUTTON_TOKENS: &[Tok] = &[Tok::PanelRaised, Tok::Text, Tok::TextDim, Tok::Focus];
/// The tokens a swatch paints with (beside the colour it shows).
pub const SWATCH_TOKENS: &[Tok] = &[Tok::Edge, Tok::Focus];

/// A push button of `width` points (0 = as wide as its label), 22 pt high.
pub fn button(ui: &mut Ui, label: Src<'_>, width: f32, enabled: bool) -> Response {
    let s = Style::current();
    let tk = s.tk();
    let h = s.d(22.0);
    let text = Text {
        src: label,
        role: Role::Control,
        caps: false,
        align: Align2::CENTER_CENTER,
        policy: Policy::Elide(Elide::End),
        color: s.t.text,
    };
    let w = if width > 0.0 {
        width * s.k
    } else {
        text::measure(ui.ctx(), &text, tk).x + 20.0 * s.k
    };
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), sense);
    let st = State {
        disabled: !enabled,
        ..State::of(&resp, false)
    };
    with_brush(ui.painter(), |b| {
        b.register(rect, "button", BUTTON_TOKENS, &[]);
        let base = if st.active {
            shade(s.t.panel_raised, -0.04)
        } else if st.hovered {
            shade(s.t.panel_raised, 0.04)
        } else {
            s.t.panel_raised
        };
        let r = s.s(s.t.radius);
        light::raised(
            b,
            rect,
            r,
            base,
            RaisedOpts {
                drop: !st.active,
                k: s.k,
                alpha: st.alpha(),
                depth: s.t.depth,
                ..Default::default()
            },
        );
        let ink = if enabled { s.t.text } else { s.t.text_dim };
        let _ = text::paint(
            b,
            rect.shrink2(Vec2::new(6.0 * s.k, 0.0)),
            &Text {
                color: ink.gamma_multiply(st.alpha()),
                ..text
            },
            tk,
        );
        if st.focused {
            focus_ring(b, rect, r, &s);
        }
    });
    resp
}

/// A square key showing a glyph; `lit` paints it in the highlight (FL's active key).
pub fn glyph_key(ui: &mut Ui, g: Glyph, lit: bool, size: f32, tip: &str) -> Response {
    let s = Style::current();
    let side = size * s.k;
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(side), Sense::click());
    let st = State::of(&resp, lit);
    if st.hovered {
        hint::set_text(ui.ctx(), tip, "", Caps::NONE);
    }
    with_brush(ui.painter(), |b| {
        crate::kit::cap::toolbar_key(
            b,
            rect,
            &s.scaled(side / 28.0),
            st,
            crate::kit::cap::KeyFace::Glyph(g),
        )
    });
    resp.on_hover_text(tip)
}

/// A colour swatch: the colour in a well, outlined; `changed` marks one the user set.
pub fn swatch(ui: &mut Ui, c: Color32, size: Vec2, changed: bool, tip: &str) -> Response {
    let s = Style::current();
    let (rect, resp) = ui.allocate_exact_size(size * s.k, Sense::click());
    let st = State::of(&resp, false);
    if st.hovered {
        hint::set_text(ui.ctx(), tip, "", Caps::NONE);
    }
    with_brush(ui.painter(), |b| {
        b.register(rect, "swatch", SWATCH_TOKENS, &[]);
        let r = b.snap_rect(rect);
        let rad = s.r(3.0);
        b.rrect(
            r,
            rad,
            Fill::Solid(shade(s.t.edge, if st.hovered { 0.1 } else { 0.0 })),
        );
        b.rrect(
            r.shrink(b.px()),
            (rad - b.px()).max(0.0),
            Fill::Solid(c.to_opaque()),
        );
        // The light falls on it like on everything else: a faint top edge.
        b.hline(
            r.left() + rad,
            r.right() - rad,
            r.top() + b.px() * 1.5,
            b.px(),
            with_alpha(shade(c.to_opaque(), 0.18), 0.8),
        );
        if changed {
            // A dot in the corner: this colour is the user's, not the theme's.
            b.circle(
                egui::pos2(r.right() - 4.0 * s.k, r.top() + 4.0 * s.k),
                2.2 * s.k,
                Fill::Solid(s.t.brand),
            );
        }
        if st.focused {
            focus_ring(b, r, rad, &s);
        }
    });
    resp.on_hover_text(tip)
}

/// A single-line text field in a well: egui's editing inside the kit's look.
pub fn text_field(ui: &mut Ui, text: &mut String, hint_key: &str, width: f32) -> Response {
    let s = Style::current();
    let h = s.d(24.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width * s.k, h), Sense::hover());
    with_brush(ui.painter(), |b| {
        crate::kit::well::paint(b, rect, &s, State::REST, None);
    });
    let inner = rect.shrink2(Vec2::new(7.0 * s.k, 3.0 * s.k));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));
    let hint_text = crate::i18n::t(hint_key);
    child.add_sized(
        inner.size(),
        egui::TextEdit::singleline(text)
            .frame(egui::Frame::NONE)
            .font(Role::Body.font(s.tk()))
            .text_color(s.t.text)
            .hint_text(egui::RichText::new(hint_text).color(s.t.placeholder)),
    )
}

/// A section band across `ui`'s width: the kit's `SectionBand`.
pub fn band(ui: &mut Ui, title: Src<'_>) -> Rect {
    let s = Style::current();
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), s.d(band::BAND)),
        Sense::hover(),
    );
    with_brush(ui.painter(), |b| {
        band::section(b, rect, &s, State::REST, title, None, false)
    });
    rect
}

/// A label | value row's label: right-aligned in a column of `width` (ADR 0024 §7).
pub fn row_label(ui: &mut Ui, key: &str, width: f32, height: f32) {
    let s = Style::current();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width * s.k, height * s.k), Sense::hover());
    with_brush(ui.painter(), |b| {
        let _ = text::paint(
            b,
            rect,
            &Text::key(key, Role::Body, s.t.text_dim)
                .align(Align2::RIGHT_CENTER)
                .policy(Policy::Abbreviate),
            s.tk(),
        );
    });
}

/// A label left-aligned in a box of `width` × `height` points (beside a swatch).
pub fn row_label_left(ui: &mut Ui, key: &str, width: f32, height: f32) {
    let s = Style::current();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width * s.k, height * s.k), Sense::hover());
    with_brush(ui.painter(), |b| {
        let _ = text::paint(
            b,
            rect,
            &Text::key(key, Role::Body, s.t.text)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
    });
}

/// The logos on screen, per context: their texture and quad, kept while the choice, the
/// theme and the pixel size stay the same (the kit's `logo::paint` builds the SVG to key
/// its cache on every call, which the toolbar cannot afford every frame).
type Logos = std::sync::Arc<std::sync::Mutex<Vec<LogoQuad>>>;

struct LogoQuad {
    key: (crate::kit::logo::LogoChoice, u64, u32),
    rect: Rect,
    mesh: std::sync::Arc<egui::epaint::Mesh>,
}

/// Paint the logo `choice` in the square `rect` at its exact pixel size, from a cached
/// texture and quad: a steady frame allocates nothing.
pub fn paint_logo(
    painter: &egui::Painter,
    rect: Rect,
    choice: crate::kit::logo::LogoChoice,
    tokens: &crate::kit::Tokens,
) {
    use crate::kit::logo::{self, Tint};
    let ctx = painter.ctx();
    let ppp = ctx.pixels_per_point();
    let snap = |v: f32| (v * ppp).round() / ppp;
    let rect = Rect::from_min_max(
        egui::pos2(snap(rect.min.x), snap(rect.min.y)),
        egui::pos2(snap(rect.max.x), snap(rect.max.y)),
    );
    let px = (rect.width().min(rect.height()) * ppp).round().max(1.0) as u32;
    let theme = if choice.tint == Tint::Theme {
        tokens.generation
    } else {
        0
    };
    let key = (choice, theme, px);
    let logos: Logos = ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Logos>(egui::Id::new("cantus-logo-quads"))
            .clone()
    });
    let mesh = {
        let mut logos = logos.lock().unwrap_or_else(|e| e.into_inner());
        match logos.iter().find(|q| q.key == key && q.rect == rect) {
            Some(q) => Some(q.mesh.clone()),
            None => logo::texture_now(ctx, choice.logo(), choice.tint, tokens, px).map(|tex| {
                let side = px as f32 / ppp;
                let r = Rect::from_min_size(rect.min, Vec2::splat(side));
                let mut m = egui::epaint::Mesh::with_texture(tex);
                m.add_rect_with_uv(
                    r,
                    Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
                let mesh = std::sync::Arc::new(m);
                logos.retain(|q| q.rect != rect);
                if logos.len() > 16 {
                    logos.remove(0);
                }
                logos.push(LogoQuad {
                    key,
                    rect,
                    mesh: mesh.clone(),
                });
                mesh
            }),
        }
    };
    if let Some(m) = mesh {
        painter.add(egui::Shape::Mesh(m));
    }
}

/// A glyph painted in a square of `size` points, not a control.
pub fn paint_glyph(ui: &Ui, rect: Rect, g: Glyph, c: Color32) {
    with_brush(ui.painter(), |b| glyph::paint(b, rect, g, c));
}
