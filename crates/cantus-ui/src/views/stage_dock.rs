//! The **Stage** tab (ADR 0091 §1): the dancers whose `view.mode` is docked dance on a stage
//! of their own among the workspace's tabs — by default under the inspector, where FL keeps
//! a dancer in a corner of the window — on the theme's stage: its deepest ground lit from
//! above by a soft spotlight in the theme's accent, a floor line where their feet are.
//!
//! Each dancer stands at its `x`/`y` share of the tab, `size` × [`DOCK_STANDARD`] of its
//! height tall; dragging moves it there, its grip resizes it, its pill floats or hides it.
//! A tab with no dancer on it says how to put one there.

use super::chrome::{self, Button, Lit};
use super::float;
use super::geom::{self, Area, DOCK_STANDARD};
use super::{Surface, ViewMode};
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::color::{mix, shade, with_alpha};
use crate::kit::prose;
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{with_brush, Brush, Fill, Style};
use crate::views::workspace::Pane;
use egui::{pos2, vec2, Align2, Rect, Ui};
use egui_dock::{Node, NodeIndex, Tree};

/// The share of the inspector's column the Stage tab takes when it is first placed.
pub const SHARE: f32 = 0.40;

/// Place the Stage tab in the dock: under the inspector (its lower share of that column),
/// else at the right of everything as the inspector would be.
pub fn place(s: &mut Tree<Pane>, inspector: Option<NodeIndex>, inspector_share: f32) {
    match inspector {
        Some(node) if matches!(s[node], Node::Leaf(_)) => {
            s.split_below(node, 1.0 - SHARE, vec![Pane::Stage]);
        }
        _ => {
            s.split_right(NodeIndex::root(), inspector_share, vec![Pane::Stage]);
        }
    }
}

/// The theme's stage behind the docked dancers.
pub fn paint_ground(b: &mut Brush, r: Rect, s: &Style) {
    paint_ground_at(b, r, s, r.top() + r.height() * 0.86);
}

/// The theme's stage with its floor line at `floor` (points).
pub fn paint_ground_at(b: &mut Brush, r: Rect, s: &Style, floor: f32) {
    let ink = s.t.ink;
    b.rect(r, Fill::V(shade(ink, 0.035), shade(ink, -0.02)));
    // A soft spotlight from above, in the accent.
    let spot = pos2(r.center().x, r.top() - r.height() * 0.15);
    let radii = vec2(r.width() * 0.62, r.height() * 1.05);
    let glow = s.t.highlight;
    let stops = [
        (0.0, with_alpha(glow, 0.16)),
        (0.55, with_alpha(glow, 0.06)),
        (1.0, with_alpha(glow, 0.0)),
    ];
    b.radial(r, spot, radii, &stops, 24);
    // The floor: a line where size-1 dancers stand by default, fading to the sides.
    let y = floor;
    let floor = mix(s.t.grid_strong, glow, 0.25);
    let n = 24;
    for i in 0..n {
        let x0 = r.left() + r.width() * i as f32 / n as f32;
        let x1 = r.left() + r.width() * (i + 1) as f32 / n as f32;
        let mid = ((i as f32 + 0.5) / n as f32 - 0.5).abs() * 2.0;
        let a = (1.0 - mid * mid) * 0.8;
        b.hline(x0, x1, y, s.s(1.0), with_alpha(floor, a));
    }
}

/// The tab.
pub fn show(app: &mut CantusApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    float::begin(app, &ctx);
    let s = Style::current();
    let r = ui.max_rect();
    with_brush(ui.painter(), |b| paint_ground(b, r, &s));
    let playing = app.engine.is_playing();
    let ppp = ctx.pixels_per_point();
    let a = Area::of(r, ppp, DOCK_STANDARD);
    let seen = r.intersect(ctx.content_rect());
    let clip_px = Rect::from_min_max(
        (seen.min.to_vec2() * ppp).to_pos2(),
        (seen.max.to_vec2() * ppp).to_pos2(),
    );
    let mut st = std::mem::take(&mut app.stage);
    let dancers = std::mem::take(&mut st.dancers);
    let mut actions = Vec::new();
    let mut shown = 0;
    let now = ctx.input(|i| i.time);
    let hidden = app.settings.stage.hidden;
    for d in dancers
        .iter()
        .filter(|d| d.mode == ViewMode::Docked && !hidden)
    {
        shown += 1;
        let Some(placed) = geom::place(&a, clip_px, d.character.as_ref(), &d.pose, ppp) else {
            continue;
        };
        st.bodies.push((d.track, Surface::Dock, placed.body));
        let dragging = st.drag.is_some_and(|g| float::drag_track(g) == d.track);
        let Some(tex) = st.texture(
            &ctx,
            (d.track, Surface::Dock),
            d.character.as_ref(),
            &placed.pose,
            &placed.stage,
            super::rate(playing, dragging),
        ) else {
            continue;
        };
        let painter = ui.painter().with_clip_rect(r);
        with_brush(&painter, |b| b.foreign(tex, placed.rect(ppp)));
        let showing = st
            .hover
            .is_some_and(|(tr, sf, until)| tr == d.track && sf == Surface::Dock && until > now)
            || dragging;
        let buttons = [Button::Float, Button::Hide];
        let frame = showing.then(|| chrome::layout(&ctx, placed.body, &d.name, &buttons, &s, r));
        let out = float::sense(
            ui,
            &mut st,
            d,
            &a,
            placed.body,
            frame.as_ref(),
            ppp,
            &mut actions,
        );
        if let Some(c) = &frame {
            let fade = if out.hovered || dragging {
                1.0
            } else {
                let left = st.hover.map_or(0.0, |(_, _, u)| u - now);
                (left / float::LINGER).clamp(0.0, 1.0) as f32
            };
            let lit = Lit {
                hot: out.hot,
                grip_hot: out.grip_hot,
                body_hot: out.hovered,
                pinned: false,
                fade,
            };
            with_brush(&painter, |b| chrome::paint(b, c, &d.name, &s, lit));
        }
    }
    st.dancers = dancers;
    app.stage = st;
    if shown == 0 {
        with_brush(ui.painter(), |b| empty(b, r, &s));
    }
    float::apply(app, &actions);
}

/// What an empty Stage tab says.
fn empty(b: &mut Brush, r: Rect, s: &Style) {
    let k = s.k;
    let c = r.center();
    let mark = Rect::from_center_size(pos2(c.x, c.y - 30.0 * k), vec2(34.0 * k, 34.0 * k));
    let u = mark.width() / 12.0;
    chrome::glyph(b, mark.center(), u, (1.25 * u).max(b.px()), s.t.text_dim);
    let w = (r.width() - 32.0 * k).clamp(40.0 * k, 320.0 * k);
    let title = t("stage.empty.title");
    let line = Rect::from_center_size(pos2(c.x, c.y + 4.0 * k), vec2(w, 20.0 * k));
    let tt = Text::new(&title, Role::Body, s.t.text)
        .align(Align2::CENTER_CENTER)
        .policy(Policy::Elide(Elide::End));
    let _ = text::paint(b, line, &tt, s.tk());
    let hint = t("stage.empty.hint");
    let lines = prose::wrap(b.ctx(), &hint, Role::Label, s.tk(), w);
    let block = Rect::from_min_size(pos2(c.x - w * 0.5, c.y + 22.0 * k), vec2(w, lines.height()));
    let _ = prose::paint(
        b,
        block,
        &lines,
        Role::Label,
        s.tk(),
        s.t.text_dim,
        Align2::CENTER_TOP,
    );
}
