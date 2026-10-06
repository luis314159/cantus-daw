//! Small parts the audio-setup views share (round 20), drawn with the kit: a wrapped
//! paragraph (the kit's boxes are one line; a problem's sentence needs two), a primary
//! button lit in the highlight, a choice key with a caption under it (the buffer sizes
//! and their latency), a level bar, and the floating panel those views sit in.

use crate::kit::color::{shade, with_alpha};
use crate::kit::light::{self, RaisedOpts};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{focus_ring, with_brush, Brush, Fill, State, Style, Tok};
use egui::text::{LayoutJob, TextFormat};
use egui::{Align2, Color32, Pos2, Rect, Response, Sense, Ui, Vec2};

/// Lay `s` out in `role`, wrapped at `width`, at most `rows` lines.
pub fn wrapped(ctx: &egui::Context, s: &str, role: Role, k: f32, width: f32, rows: usize) -> std::sync::Arc<egui::Galley> {
    let mut j = LayoutJob::default();
    j.append(
        s,
        0.0,
        TextFormat {
            font_id: role.font(k),
            extra_letter_spacing: role.tracking() * k,
            color: Color32::PLACEHOLDER,
            ..Default::default()
        },
    );
    j.wrap.max_width = width.max(10.0);
    j.wrap.max_rows = rows;
    j.wrap.break_anywhere = false;
    ctx.fonts_mut(|f| f.layout_job(j))
}

/// Paint a wrapped paragraph at `pos`; returns its size.
pub fn paint_wrapped(b: &mut Brush, pos: Pos2, s: &str, role: Role, color: Color32, k: f32, width: f32) -> Vec2 {
    let g = wrapped(b.ctx(), s, role, k, width, 4);
    let size = g.size();
    b.text(Pos2::new(b.snap(pos.x), b.snap(pos.y)), g, color, 0.0);
    size
}

/// A paragraph that takes its height in `ui`.
pub fn paragraph(ui: &mut Ui, s: &str, role: Role, color: Color32) -> Rect {
    let st = Style::current();
    let w = ui.available_width();
    let g = wrapped(ui.ctx(), s, role, st.tk(), w, 4);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, g.size().y), Sense::hover());
    with_brush(ui.painter(), |b| {
        b.text(Pos2::new(b.snap(rect.left()), b.snap(rect.top())), g, color, 0.0);
    });
    rect
}

/// A push button; `primary` lights it in the highlight (the step to take).
pub fn button(ui: &mut Ui, label: &str, primary: bool, enabled: bool, min_w: f32) -> Response {
    let s = Style::current();
    let text = Text::new(label, Role::Control, s.t.text)
        .align(Align2::CENTER_CENTER)
        .policy(Policy::Elide(Elide::End));
    let w = (text::measure(ui.ctx(), &text, s.tk()).x + 24.0 * s.k).max(min_w * s.k);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, s.d(24.0)), sense);
    paint_button(ui, rect, &resp, label, primary, enabled);
    resp
}

/// Paint a button in `rect` for `resp`.
pub fn paint_button(ui: &Ui, rect: Rect, resp: &Response, label: &str, primary: bool, enabled: bool) {
    let s = Style::current();
    let st = State {
        disabled: !enabled,
        ..State::of(resp, false)
    };
    with_brush(ui.painter(), |b| {
        b.register(rect, "audio-button", &[Tok::PanelRaised, Tok::Highlight, Tok::Text], &[]);
        let base = if primary { s.t.highlight } else { s.t.panel_raised };
        let base = if st.active {
            shade(base, -0.05)
        } else if st.hovered {
            shade(base, 0.05)
        } else {
            base
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
        let ink = if primary {
            crate::kit::color::on_fill(base)
        } else if enabled {
            s.t.text
        } else {
            s.t.text_dim
        };
        let _ = text::paint(
            b,
            rect.shrink2(Vec2::new(8.0 * s.k, 0.0)),
            &Text::new(label, Role::Control, ink.gamma_multiply(st.alpha()))
                .align(Align2::CENTER_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
        if st.focused {
            focus_ring(b, rect, r, &s);
        }
    });
}

/// A choice key: a big word on top, a caption under it, a coloured dot beside the
/// caption; lit when chosen. Returns the response for the rect it is given.
#[allow(clippy::too_many_arguments)]
pub fn choice_key(
    ui: &mut Ui,
    rect: Rect,
    id: egui::Id,
    word: &str,
    caption: &str,
    dot: Option<Tok>,
    lit: bool,
) -> Response {
    let s = Style::current();
    let resp = ui.interact(rect, id, Sense::click());
    let st = State::of(&resp, lit);
    with_brush(ui.painter(), |b| {
        b.register(rect, "audio-choice", &[Tok::PanelRaised, Tok::Selection, Tok::Text, Tok::TextDim], &[]);
        let base = if lit {
            shade(s.t.panel_raised, 0.06)
        } else if st.hovered {
            shade(s.t.panel_raised, 0.03)
        } else {
            s.t.panel_raised
        };
        light::raised(
            b,
            rect,
            s.r(4.0),
            base,
            RaisedOpts {
                drop: !lit,
                k: s.k,
                depth: s.t.depth,
                ..Default::default()
            },
        );
        if lit {
            // The selection's underline, as a lit segment has it.
            let u = Rect::from_min_max(
                Pos2::new(rect.left() + s.s(5.0), rect.bottom() - s.s(4.0)),
                Pos2::new(rect.right() - s.s(5.0), rect.bottom() - s.s(2.0)),
            );
            b.rrect(u, s.s(1.0), Fill::Solid(s.t.selection));
            b.rrect_stroke(rect, s.r(4.0), b.px(), with_alpha(s.t.selection, 0.55));
        }
        let top = Rect::from_min_max(rect.min, Pos2::new(rect.right(), rect.center().y + s.s(2.0)));
        let _ = text::paint(
            b,
            top,
            &Text::new(word, Role::Value, s.t.text)
                .align(Align2::CENTER_BOTTOM)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
        let bottom = Rect::from_min_max(
            Pos2::new(rect.left(), rect.center().y + s.s(3.0)),
            Pos2::new(rect.right(), rect.bottom() - s.s(5.0)),
        );
        let cap = Text::new(caption, Role::Micro, s.t.text_dim)
            .align(Align2::CENTER_TOP)
            .policy(Policy::Elide(Elide::End));
        if let Some(p) = text::paint(b, bottom, &cap, s.tk()) {
            if let Some(tok) = dot {
                let c = Pos2::new(p.rect.left() - s.s(5.0), p.rect.center().y);
                b.circle(c, s.s(2.4), Fill::Solid(s.t.get(tok)));
            }
        }
        if st.focused {
            focus_ring(b, rect, s.r(4.0), &s);
        }
    });
    resp
}

/// A horizontal level bar in a well (an input meter on one line), level and hold in dB.
pub fn level_bar(b: &mut Brush, rect: Rect, s: &Style, db: f32, hold: f32) {
    use crate::kit::meter::{pos_of_db, MIN_DB};
    crate::kit::well::paint(b, rect, s, State::REST, Some(s.t.meter_well));
    let inner = rect.shrink(s.s(2.0));
    let x = |d: f32| inner.left() + inner.width() * pos_of_db(d.max(MIN_DB));
    if db > MIN_DB {
        let lit = Rect::from_min_max(inner.min, Pos2::new(x(db), inner.bottom()));
        let stops = [
            (0.0, s.t.meter_low),
            (pos_of_db(-6.0), s.t.meter_low),
            (pos_of_db(-6.0) + 0.001, s.t.meter_high),
            (pos_of_db(0.0), s.t.meter_clip),
            (1.0, s.t.meter_clip),
        ];
        // The ramp runs along the bar: paint it in thin vertical slices.
        let n = 24;
        for i in 0..n {
            let a = lit.left() + lit.width() * i as f32 / n as f32;
            let c = lit.left() + lit.width() * (i + 1) as f32 / n as f32;
            let t = ((a + c) * 0.5 - inner.left()) / inner.width().max(1.0);
            let mut colour = stops[0].1;
            for w in stops.windows(2) {
                if t >= w[0].0 && t <= w[1].0 {
                    let f = (t - w[0].0) / (w[1].0 - w[0].0).max(1e-6);
                    colour = w[0].1.lerp_to_gamma(w[1].1, f);
                }
            }
            b.rect(
                Rect::from_min_max(Pos2::new(a, lit.top()), Pos2::new(c, lit.bottom())),
                Fill::Solid(colour),
            );
        }
    }
    if hold > MIN_DB {
        let hx = x(hold);
        b.vline(hx, inner.top(), inner.bottom(), s.s(2.0), s.t.text);
    }
}

/// A floating panel's face: a soft shadow and the raised panel (as the settings window).
pub fn panel(ui: &Ui, rect: Rect, s: &Style) {
    super::workspace_settings::frame(ui, rect, s);
}

/// dB of a linear peak, floored at the meters' bottom.
pub fn db(x: f32) -> f32 {
    use crate::kit::meter::MIN_DB;
    if x > 1e-6 {
        (20.0 * x.log10()).max(MIN_DB)
    } else {
        MIN_DB
    }
}
