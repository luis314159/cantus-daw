//! The workspace's skin (ADR 0024 §4, §5): `egui_dock` drawn in the kit's look.
//!
//! `egui_dock` lays out and senses; what shows is ours. Its style is set from the kit's
//! tokens (the tab bar on `panel`, 1-px splitters that light in the focus colour, drop
//! targets lit in the selection colour), and every tab is painted over by the kit's `Tab`
//! part — its glyph, its name in the `control` role, the active tab's 2-pt underline —
//! from `TabViewer::on_tab_button`. Splitters get the kit's grip while hovered.

use super::workspace::Pane;
use crate::kit::color::{shade, with_alpha};
use crate::kit::glyph::{self, Glyph};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{chrome, with_brush, State, Style};
use egui::text::{LayoutJob, TextFormat};
use egui::{Align2, Color32, Context, CornerRadius, Margin, Pos2, Rect, Stroke, Vec2, WidgetText};
use std::sync::Arc;

/// Room for the pane's glyph before its name, in points at 100 %.
pub const GLYPH_ROOM: f32 = 20.0;
/// egui_dock's close button: a square of the tab bar's height at the tab's right end.
pub const CLOSE: f32 = 24.0;

/// The tabs' titles, laid out once per language and text size: a tab's title costs no
/// allocation per frame.
#[derive(Default)]
pub struct Titles {
    locale: Option<crate::i18n::Locale>,
    tk: f32,
    items: Vec<(Pane, WidgetText)>,
    device: String,
}

impl Titles {
    /// The title of `pane` for egui_dock to measure (its colour is the style's).
    pub fn get(&mut self, ctx: &Context, pane: Pane, tk: f32) -> WidgetText {
        let locale = crate::i18n::locale();
        if self.locale != Some(locale) || (self.tk - tk).abs() > 1e-4 {
            self.items.clear();
            self.locale = Some(locale);
            self.tk = tk;
        }
        if let Pane::Device(key) = pane {
            // Devices name themselves ("Aequalis — Keys"); they are few and short-lived.
            if !super::workspace_device::title(ctx, key, &mut self.device) {
                self.device.clear();
                self.device.push_str(&crate::i18n::t(pane.title_key()));
            }
            return job(&self.device, tk);
        }
        if let Some((_, w)) = self.items.iter().find(|(p, _)| *p == pane) {
            return w.clone();
        }
        let w = job(&crate::i18n::t(pane.title_key()), tk);
        self.items.push((pane, w.clone()));
        w
    }

    /// The text of a title, for painting.
    pub fn text(&self, ctx: &Context, pane: Pane, out: &mut String) {
        out.clear();
        match pane {
            Pane::Device(key) => {
                if !super::workspace_device::title(ctx, key, out) {
                    out.push_str(&crate::i18n::t(pane.title_key()));
                }
            }
            _ => out.push_str(&crate::i18n::t(pane.title_key())),
        }
    }
}

fn job(s: &str, tk: f32) -> WidgetText {
    let mut j = LayoutJob::default();
    j.append(
        s,
        GLYPH_ROOM * tk,
        TextFormat {
            font_id: Role::Control.font(tk),
            // The style's text colour: visible only where the kit does not paint over
            // (the tab being dragged).
            color: Color32::PLACEHOLDER,
            ..Default::default()
        },
    );
    WidgetText::LayoutJob(Arc::new(j))
}

/// `egui_dock`'s style, from the kit's tokens.
pub fn dock_style(s: &Style, egui_style: &egui::Style, ppp: f32) -> egui_dock::Style {
    let t = &s.t;
    let mut d = egui_dock::Style::from_egui(egui_style);
    let line = shade(t.panel, if t.dark { -0.14 } else { -0.12 });
    let px = 1.0 / ppp.max(0.5);
    d.dock_area_padding = None;
    d.main_surface_border_stroke = Stroke::NONE;
    d.main_surface_border_rounding = CornerRadius::ZERO;

    d.separator.width = px;
    d.separator.extra_interact_width = chrome::SPLITTER * s.k + 2.0;
    d.separator.extra = 96.0 * s.k;
    d.separator.color_idle = line;
    d.separator.color_hovered = t.focus;
    d.separator.color_dragged = t.focus;

    d.tab_bar.bg_fill = t.panel;
    d.tab_bar.height = (chrome::TAB * s.k * t.density).round();
    d.tab_bar.inner_margin = Margin::symmetric(4, 0);
    d.tab_bar.corner_radius = CornerRadius::ZERO;
    d.tab_bar.hline_color = line;
    d.tab_bar.fill_tab_bar = false;
    d.tab_bar.show_scroll_bar_on_overflow = false;

    let r = CornerRadius {
        nw: s.r(3.0).round() as u8,
        ne: s.r(3.0).round() as u8,
        sw: 0,
        se: 0,
    };
    let hidden = egui_dock::TabInteractionStyle {
        outline_color: Color32::TRANSPARENT,
        corner_radius: r,
        bg_fill: Color32::TRANSPARENT,
        text_color: Color32::TRANSPARENT,
    };
    // The active tab is painted by the kit on top; these show only while a tab is dragged.
    let shown = egui_dock::TabInteractionStyle {
        outline_color: shade(t.panel_raised, -0.18),
        corner_radius: r,
        bg_fill: t.panel_raised,
        text_color: t.text,
    };
    d.tab.active = shown.clone();
    d.tab.focused = shown.clone();
    d.tab.active_with_kb_focus = shown.clone();
    d.tab.focused_with_kb_focus = shown;
    d.tab.inactive = hidden.clone();
    d.tab.hovered = hidden.clone();
    d.tab.inactive_with_kb_focus = hidden;
    d.tab.hline_below_active_tab_name = false;
    d.tab.spacing = 2.0;
    d.tab.minimum_width = Some(64.0 * s.k);
    d.tab.tab_body.bg_fill = t.panel;
    d.tab.tab_body.stroke = Stroke::NONE;
    d.tab.tab_body.inner_margin = Margin::ZERO;
    d.tab.tab_body.corner_radius = CornerRadius::ZERO;

    d.buttons.close_tab_color = Color32::TRANSPARENT;
    d.buttons.close_tab_active_color = Color32::TRANSPARENT;
    d.buttons.close_tab_bg_fill = with_alpha(t.text, 0.10);
    d.buttons.add_tab_bg_fill = t.panel;

    // A drag shows where it lands: the drop target lit in the selection colour (§10).
    d.overlay.overlay_type = egui_dock::OverlayType::HighlightedAreas;
    d.overlay.selection_color = with_alpha(t.selection, 0.28);
    d.overlay.selection_stroke_width = 2.0 * s.k;
    d.overlay.button_color = t.panel_raised;
    d.overlay.button_border_stroke = Stroke::new(px, t.selection);
    d.overlay.hovered_leaf_highlight.color = with_alpha(t.selection, 0.06);
    d.overlay.hovered_leaf_highlight.stroke = Stroke::new(px, with_alpha(t.selection, 0.6));
    d.overlay.hovered_leaf_highlight.corner_radius = CornerRadius::ZERO;
    d.overlay.surface_fade_opacity = 0.35;
    d
}

/// Paint one tab in the kit's look over what egui_dock drew.
#[allow(clippy::too_many_arguments)]
pub fn paint_tab(
    ctx: &Context,
    resp: &egui::Response,
    clip: Rect,
    pane: Pane,
    title: &str,
    lit: bool,
    focused: bool,
    closeable: bool,
) {
    let s = Style::current();
    let t = s.t;
    let rect = resp.rect;
    let st = State {
        hovered: resp.hovered(),
        active: false,
        disabled: false,
        focused: resp.has_focus(),
        lit,
    };
    let painter = egui::Painter::new(ctx.clone(), resp.layer_id, clip.intersect(rect.expand(2.0)));
    with_brush(&painter, |b| {
        chrome::tab(b, rect, &s, st, Src::Str(""));
        let k = s.k;
        let tk = s.tk();
        let ink = if lit || st.hovered {
            t.text
        } else {
            t.text_dim
        };
        // The focused leaf's tab says so with the brand colour on its glyph.
        let gink = if lit && focused { t.brand } else { ink };
        let g = Rect::from_center_size(
            Pos2::new(rect.left() + 8.0 * k + 7.0 * tk, rect.center().y),
            Vec2::splat(12.0 * tk),
        );
        glyph::paint(b, g, pane.glyph(), gink);
        let close = if closeable {
            CLOSE.min(rect.height())
        } else {
            0.0
        };
        let tr = Rect::from_min_max(
            Pos2::new(rect.left() + 8.0 * k + GLYPH_ROOM * tk, rect.top()),
            Pos2::new(rect.right() - close - 4.0 * k, rect.bottom()),
        );
        let tt = Text::new(title, Role::Control, ink)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, tr, &tt, tk);
        let c = Rect::from_center_size(
            Pos2::new(rect.right() - close * 0.5, rect.center().y),
            Vec2::splat(9.0 * k),
        );
        let over = ctx
            .pointer_hover_pos()
            .is_some_and(|p| c.expand(7.0 * k).contains(p));
        if closeable && (lit || st.hovered || over) {
            glyph::paint(b, c, Glyph::Close, if over { t.text } else { t.text_dim });
        }
    });
}

/// The kit's grip over a splitter the pointer is on or dragging.
pub fn paint_splitter(painter: &egui::Painter, rect: Rect, active: bool) {
    let s = Style::current();
    let st = State {
        hovered: true,
        active,
        ..State::REST
    };
    with_brush(painter, |b| chrome::splitter(b, rect, &s, st));
}

/// Whether a colour is fully transparent (egui_dock's hidden styles).
pub fn invisible(c: Color32) -> bool {
    c.a() == 0
}
