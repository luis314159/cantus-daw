//! Round 25's parts of the appearance page (ADR 0046): each theme's **preview swatch**, and
//! the **taskbar icon's colour** — the mark as designed, the theme's accent, eight shipped
//! colours or one of the user's — applied to the window and the taskbar at once.

use super::appearance::change;
use super::workspace_widgets as w;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::appearance::{self as appearance, IconColour, IconPreset};
use crate::kit::mark::MarkColours;
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{with_brush, Brush, Fill, Style, Tokens};
use egui::{Align2, Context, Id, Order, Pos2, Rect, Sense, TextureHandle, Ui, Vec2};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// The tokens a base theme (`preset:<id>` or `user:<name>`) resolves to, for its preview.
/// Presets are worked out once per page; the user's own themes every time (they change).
pub fn preview_tokens(app: &mut CantusApp, base: &str) -> Tokens {
    if base.starts_with("preset:") {
        if let Some((_, t)) = app.appearance_page.previews.iter().find(|(b, _)| b == base) {
            return *t;
        }
    }
    let t = app
        .appearance
        .base_theme(base, appearance::Mode::Dark)
        .kit_tokens();
    if base.starts_with("preset:") {
        app.appearance_page.previews.push((base.to_string(), t));
    }
    t
}

/// A theme in miniature, painted with its own tokens: the toolbar with a lit key, the
/// browser beside the arrangement's rows, a selected clip, the playhead, and a meter.
pub fn paint_preview(b: &mut Brush, rect: Rect, t: &Tokens, k: f32) {
    let r = b.snap_rect(rect);
    let rad = 4.0 * k;
    b.rrect(r, rad, Fill::Solid(t.ink));
    let inner = r.shrink(1.0);
    // The toolbar, its lit key and a line of text.
    let bar = Rect::from_min_max(
        inner.min,
        Pos2::new(inner.right(), inner.top() + inner.height() * 0.24),
    );
    b.rect(bar, Fill::Solid(t.toolbar));
    let key = Rect::from_center_size(
        Pos2::new(bar.left() + bar.height() * 0.8, bar.center().y),
        Vec2::splat(bar.height() * 0.62),
    );
    b.rrect(key, 1.5 * k, Fill::Solid(t.highlight));
    b.hline(
        key.right() + 3.0 * k,
        key.right() + bar.width() * 0.32,
        bar.center().y,
        (bar.height() * 0.22).max(1.0),
        t.text,
    );
    // The browser (a panel with two lines of text) and the rows beside it.
    let body_top = bar.bottom();
    let side = Rect::from_min_max(
        Pos2::new(inner.left(), body_top),
        Pos2::new(inner.left() + inner.width() * 0.28, inner.bottom()),
    );
    b.rect(side, Fill::Solid(t.panel));
    let lh = (side.height() * 0.09).max(1.0);
    for (i, f) in [0.62, 0.45, 0.55].iter().enumerate() {
        let y = side.top() + side.height() * (0.22 + 0.24 * i as f32);
        let c = if i == 0 { t.text } else { t.text_dim };
        b.hline(
            side.left() + 3.0 * k,
            side.left() + side.width() * f,
            y,
            lh,
            c,
        );
    }
    let rows = Rect::from_min_max(
        Pos2::new(side.right(), body_top),
        Pos2::new(inner.right() - inner.width() * 0.1, inner.bottom()),
    );
    let n = 4;
    for i in 0..n {
        let h = rows.height() / n as f32;
        let row = Rect::from_min_size(
            Pos2::new(rows.left(), rows.top() + h * i as f32),
            Vec2::new(rows.width(), h),
        );
        b.rect(
            row,
            Fill::Solid(if i % 2 == 0 { t.row_white } else { t.row_black }),
        );
    }
    // A clip in the selection's colour and one in a section hue.
    let h = rows.height() / n as f32;
    let clip = Rect::from_min_size(
        Pos2::new(rows.left() + rows.width() * 0.12, rows.top() + h * 1.15),
        Vec2::new(rows.width() * 0.42, h * 0.7),
    );
    b.rrect(clip, 1.0 * k, Fill::Solid(t.selection));
    let clip2 = Rect::from_min_size(
        Pos2::new(rows.left() + rows.width() * 0.4, rows.top() + h * 2.15),
        Vec2::new(rows.width() * 0.45, h * 0.7),
    );
    b.rrect(clip2, 1.0 * k, Fill::Solid(t.hue_source));
    // The playhead.
    b.vline(
        rows.left() + rows.width() * 0.66,
        rows.top(),
        rows.bottom(),
        (1.2 * k).max(1.0),
        t.playhead,
    );
    // A meter: its well, then low and high.
    let m = Rect::from_min_max(Pos2::new(rows.right(), body_top), inner.max);
    b.rect(m, Fill::Solid(t.meter_well));
    let mw = m.width() * 0.5;
    let mx = m.center().x - mw * 0.5;
    let low = Rect::from_min_max(
        Pos2::new(mx, m.top() + m.height() * 0.45),
        Pos2::new(mx + mw, m.bottom() - 2.0 * k),
    );
    b.rect(low, Fill::Solid(t.meter_low));
    let high = Rect::from_min_max(
        Pos2::new(mx, m.top() + m.height() * 0.28),
        Pos2::new(mx + mw, low.top()),
    );
    b.rect(high, Fill::Solid(t.meter_high));
    b.rrect_stroke(r, rad, 1.0, t.edge);
}

/// The square chips of the icon's colours, and what each chooses.
fn choices() -> Vec<(IconColour, String)> {
    let mut v = vec![
        (IconColour::Auto, t("look.icon.auto")),
        (IconColour::Original, t("look.icon.original")),
        (IconColour::Theme, t("look.icon.theme")),
    ];
    for p in IconPreset::ALL {
        v.push((IconColour::Preset(p), t(p.name_key())));
    }
    v
}

/// **Taskbar icon**: the chips, the user's own colour, and what changes where.
pub fn icon_section(app: &mut CantusApp, ui: &mut Ui) {
    let s = Style::current();
    let k = s.k;
    w::band(ui, Src::Key("look.icon"));
    let tokens = crate::kit::tokens::current();
    let current = app.appearance.icon;
    let side = 40.0 * k;
    let custom = match current {
        IconColour::Custom(c) => c,
        _ => {
            let [r, g, b, _] = tokens.brand.to_array();
            [r, g, b]
        }
    };
    let mut all = choices();
    all.push((IconColour::Custom(custom), t("look.icon.custom")));
    let backs = appearance::taskbar_previews();
    let last = all.len() - 1;
    ui.horizontal_wrapped(|ui| {
        for (i, (choice, name)) in all.iter().enumerate() {
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(side + 6.0 * k), Sense::click());
            let on = match (choice, current) {
                (IconColour::Custom(_), IconColour::Custom(_)) => true,
                (a, b) => *a == b,
            };
            // A dark taskbar behind the light colours, a light one behind Graphite, and
            // behind the automatic choice the taskbar this Windows has.
            let light = app.taskbar.light();
            let back = match choice {
                IconColour::Preset(IconPreset::Graphite) => backs[1],
                IconColour::Auto if light => backs[1],
                _ => backs[0],
            };
            let mark = choice.mark_on(&tokens, light);
            let tex = mark_texture(
                ui.ctx(),
                &mark,
                (side * 0.7 * ui.ctx().pixels_per_point()).round() as u32,
            );
            with_brush(ui.painter(), |b| {
                let chip = rect.shrink(3.0 * k);
                b.rrect(chip, s.r(6.0), Fill::Solid(back));
                if on || resp.hovered() {
                    b.rrect_stroke(
                        rect.shrink(1.0),
                        s.r(7.0),
                        if on { 2.0 * k } else { 1.0 },
                        if on { s.t.selection } else { s.t.text_dim },
                    );
                }
                if let Some(tex) = &tex {
                    let px = tex.size()[0] as f32 / b.ppp();
                    let at = b.snap_rect(Rect::from_center_size(chip.center(), Vec2::splat(px)));
                    b.foreign(tex.id(), Rect::from_min_size(at.min, Vec2::splat(px)));
                }
                // The last chip opens a colour picker: it says so with the eyedropper.
                if i == last {
                    let g = Rect::from_min_size(
                        chip.right_bottom() - Vec2::splat(13.0 * k),
                        Vec2::splat(11.0 * k),
                    );
                    b.circle(g.center(), 7.5 * k, Fill::Solid(backs[1]));
                    crate::kit::glyph::paint(
                        b,
                        g.shrink(1.0 * k),
                        crate::kit::glyph::Glyph::Pick,
                        backs[0],
                    );
                }
            });
            let resp = resp.on_hover_text(name.as_str());
            if resp.clicked() {
                let c = *choice;
                change(app, |a| a.icon = c);
                if i == last {
                    app.appearance_page.icon_picker = Some(resp.rect.right_top());
                }
            }
        }
    });
    // What the choice is called, and what it reaches.
    let name = all
        .iter()
        .find(|(c, _)| match (c, current) {
            (IconColour::Custom(_), IconColour::Custom(_)) => true,
            (a, b) => *a == b,
        })
        .map(|(_, n)| n.clone())
        .unwrap_or_default();
    caption(ui, &tf("look.icon.chosen", &[("name", &name)]), s.t.text);
    caption(ui, &t("look.icon.note"), s.t.text_dim);
}

fn caption(ui: &mut Ui, s: &str, color: egui::Color32) {
    let st = Style::current();
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0 * st.k), Sense::hover());
    with_brush(ui.painter(), |b| {
        let _ = text::paint(
            b,
            rect,
            &Text::new(s, Role::Label, color)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            st.tk(),
        );
    });
}

/// The colour picker for the user's own icon colour.
pub fn popups(app: &mut CantusApp, ctx: &Context) {
    let Some(at) = app.appearance_page.icon_picker else {
        return;
    };
    let s = Style::current();
    let mut c = match app.appearance.icon {
        IconColour::Custom(c) => appearance::rgb(c),
        _ => crate::kit::tokens::current().brand,
    };
    let mut close = false;
    let area = egui::Area::new(Id::new("cantus-icon-picker"))
        .order(Order::Foreground)
        .fixed_pos(at + Vec2::new(8.0, 0.0))
        .constrain(true)
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(s.t.menu)
                .corner_radius(s.r(6.0))
                .inner_margin(10.0 * s.k)
                .show(ui, |ui| {
                    ui.set_max_width(260.0 * s.k);
                    caption(ui, &t("look.icon.pick"), s.t.menu_text);
                    if egui::widgets::color_picker::color_picker_color32(
                        ui,
                        &mut c,
                        egui::widgets::color_picker::Alpha::Opaque,
                    ) {
                        let [r, g, b, _] = c.to_array();
                        change(app, |a| a.icon = IconColour::Custom([r, g, b]));
                    }
                    if w::button(ui, Src::Key("workspace.close"), 0.0, true).clicked() {
                        close = true;
                    }
                });
        });
    let outside = ctx.input(|i| i.pointer.any_pressed())
        && !ctx
            .pointer_interact_pos()
            .is_some_and(|p| area.response.rect.contains(p));
    if close || outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.appearance_page.icon_picker = None;
    }
}

type MarkTextures = Arc<Mutex<HashMap<(MarkColours, u32), TextureHandle>>>;

/// The mark in `c` at `px` pixels, as a texture (rendered once per colour and size).
fn mark_texture(ctx: &Context, c: &MarkColours, px: u32) -> Option<TextureHandle> {
    let px = px.clamp(8, 256);
    let store: MarkTextures = ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<MarkTextures>(Id::new("cantus-mark-textures"))
            .clone()
    });
    let mut map = store.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(t) = map.get(&(*c, px)) {
        return Some(t.clone());
    }
    let pix = crate::window_icon::render(c, px)?;
    let img = egui::ColorImage::from_rgba_premultiplied([px as usize, px as usize], pix.data());
    let tex = ctx.load_texture(
        format!("cantus-mark-{px}-{:?}", c),
        img,
        egui::TextureOptions::LINEAR,
    );
    if map.len() > 64 {
        map.clear();
    }
    map.insert((*c, px), tex.clone());
    Some(tex)
}
