//! The devices panel (round 20, ADR 0029 §6): the picker's catalogue as a browsing panel
//! docked in the workspace (View ▸ Devices) — FL Studio's browser and Bitwig's browser
//! panel. Instruments or effects, a search, rows with a small picture grouped by shelf.
//!
//! A row can be **dragged** onto a track or a mixer slot: it carries a
//! [`DevicePayload`](crate::picker::DevicePayload), which drop targets read (the device-ux
//! builder wires them). Double-click adds it: an instrument on a new track, an effect at the
//! end of the selected strip. The ⤢ key opens the full picker.

use super::picker::PickerView;
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::color::{mix, shade, with_alpha};
use crate::kit::glyph::{self, Glyph};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::widgets::Segmented;
use crate::kit::{with_brush, Fill, Style, Tok};
use crate::picker::{DevicePayload, PickPurpose};
use crate::picker_catalog::Role as DeviceRole;
use crate::picker_query::{groups, Query};
use egui::{pos2, vec2, Align2, Id, Rect, Sense, Ui};

const ROW: f32 = 44.0;
const HEAD: f32 = 24.0;

/// What the panel remembers between frames (inside [`PickerView`]).
#[derive(Clone, Debug, Default)]
pub struct Panel {
    pub effects: bool,
    pub text: String,
    pub scroll: f32,
    pub selected: Option<String>,
    /// Rows laid out this frame, on screen (published for tests).
    pub rows: Vec<(String, Rect)>,
}

/// Draw the panel in `ui` (a tab of the workspace).
pub fn show(app: &mut CantusApp, ui: &mut Ui) {
    app.picker_view.refresh(&app.plugins.db);
    let s = Style::current();
    let full = ui.max_rect();
    let _ = ui.allocate_rect(full, Sense::hover());
    let pad = s.s(8.0);
    let top = Rect::from_min_size(full.min, vec2(full.width(), s.s(70.0)));
    let list = Rect::from_min_max(pos2(full.left(), top.bottom()), full.max);
    let v = &mut app.picker_view;
    with_brush(ui.painter(), |b| {
        b.register(full, "devices-panel", &[Tok::Ink, Tok::Strip], &[]);
        b.rect(top, Fill::Solid(s.t.strip));
        b.rect(list, Fill::Solid(s.t.ink));
        b.hline(full.left(), full.right(), b.snap_mid(top.bottom()), b.px(), shade(s.t.strip, -0.12));
    });
    // Instruments | Effects, the full picker's key, the search.
    let seg = Rect::from_min_size(top.min + vec2(pad, pad), vec2(full.width() - 3.0 * pad - s.s(26.0), s.s(24.0)));
    let labels = [Src::Key("picker.panel.instruments"), Src::Key("picker.panel.effects")];
    let (_, pick) = Segmented::new(v.panel.effects as usize, &labels).id(Id::new("devices-role")).show_at(ui, seg);
    if let Some(i) = pick {
        v.panel.effects = i == 1;
        v.panel.scroll = 0.0;
    }
    let open = Rect::from_min_size(pos2(seg.right() + pad, seg.top() - s.s(1.0)), vec2(s.s(26.0), s.s(26.0)));
    let open_resp = ui.interact(open, Id::new("devices-open-picker"), Sense::click()).on_hover_text(t("picker.panel.open"));
    with_brush(ui.painter(), |b| {
        let st = crate::kit::State::of(&open_resp, false);
        crate::kit::cap::toolbar_key(b, open, &s, st, crate::kit::cap::KeyFace::Glyph(Glyph::Resize));
    });
    let search = Rect::from_min_size(pos2(full.left() + pad, seg.bottom() + s.s(6.0)), vec2(full.width() - 2.0 * pad, s.s(24.0)));
    with_brush(ui.painter(), |b| {
        crate::kit::well::paint(b, search, &s, crate::kit::State::REST, None);
        let g = Rect::from_center_size(pos2(search.left() + s.s(13.0), search.center().y), egui::Vec2::splat(s.s(12.0)));
        super::picker_layout::magnifier(b, g, s.t.text_dim, s.s(1.5));
    });
    let inner = Rect::from_min_max(pos2(search.left() + s.s(24.0), search.top() + s.s(2.0)), pos2(search.right() - s.s(6.0), search.bottom() - s.s(2.0)));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));
    let hint = t("picker.search.name");
    child.add_sized(
        inner.size(),
        egui::TextEdit::singleline(&mut v.panel.text)
            .id(Id::new("devices-search"))
            .frame(egui::Frame::NONE)
            .font(Role::Body.font(s.tk()))
            .text_color(s.t.text)
            .hint_text(egui::RichText::new(hint).color(s.t.placeholder)),
    );
    let role = if v.panel.effects { DeviceRole::Effect } else { DeviceRole::Instrument };
    let chosen = rows(v, ui, list, &s, role);
    if open_resp.clicked() {
        let purpose = match role {
            DeviceRole::Instrument => PickPurpose::AddInstrument { index: None },
            DeviceRole::Effect => match target_strip(app) {
                Some(strip) => PickPurpose::AddEffect { strip },
                None => return,
            },
        };
        app.open_picker(purpose);
        return;
    }
    if let Some(key) = chosen {
        add(app, &key);
    }
}

/// The strip a double-clicked effect goes on: the selected strip, or the selected track's.
fn target_strip(app: &CantusApp) -> Option<cantus_core::StripId> {
    app.selected_strip.or_else(|| {
        app.selected_track
            .and_then(|t| app.project().track(t))
            .and_then(|t| t.strip)
    })
}

/// Add the device `key` where a double-click puts it.
pub fn add(app: &mut CantusApp, key: &str) {
    let Some(d) = app.picker_view.device(key).cloned() else {
        return;
    };
    let purpose = match d.role {
        DeviceRole::Instrument => PickPurpose::AddInstrument { index: None },
        DeviceRole::Effect => match target_strip(app) {
            Some(strip) => PickPurpose::AddEffect { strip },
            None => return,
        },
    };
    app.picker_view.prefs.used(key);
    app.picker_view.store_prefs();
    app.apply_pick(purpose, d.choice);
}

/// The rows: headings and devices, culled to the viewport. Returns a double-clicked key.
fn rows(v: &mut PickerView, ui: &mut Ui, view: Rect, s: &Style, role: DeviceRole) -> Option<String> {
    let q = Query { role: Some(role), text: v.panel.text.clone(), ..Default::default() };
    let gs = groups(&v.catalog, &q, &v.prefs);
    // Lay out: (heading or device index, y in content).
    let mut items: Vec<(Option<usize>, Option<crate::picker_catalog::Category>, f32)> = Vec::new();
    let mut y = s.s(4.0);
    for g in &gs {
        if let Some(c) = g.title {
            items.push((None, Some(c), y));
            y += s.s(HEAD);
        }
        for &i in &g.items {
            items.push((Some(i), None, y));
            y += s.s(ROW);
        }
    }
    let content = y + s.s(8.0);
    if ui.rect_contains_pointer(view) {
        v.panel.scroll -= ui.input(|i| i.smooth_scroll_delta.y);
    }
    v.panel.scroll = v.panel.scroll.clamp(0.0, (content - view.height()).max(0.0));
    let off = view.top() - v.panel.scroll;
    let painter = ui.painter().with_clip_rect(view);
    let mut chosen = None;
    v.panel.rows.clear();
    let mut hovered = None;
    for (idx, _, y) in &items {
        let Some(i) = idx else { continue };
        let r = Rect::from_min_size(pos2(view.left() + s.s(4.0), off + y), vec2(view.width() - s.s(8.0), s.s(ROW) - s.s(2.0)));
        if !r.intersects(view) {
            continue;
        }
        let key = v.catalog[*i].key.clone();
        let resp = ui.interact(r.intersect(view), Id::new(("devices-row", &key)), Sense::click_and_drag());
        if resp.hovered() {
            hovered = Some(key.clone());
        }
        if resp.clicked() {
            v.panel.selected = Some(key.clone());
        }
        if resp.double_clicked() {
            chosen = Some(key.clone());
        }
        if resp.drag_started() {
            egui::DragAndDrop::set_payload(ui.ctx(), DevicePayload(v.catalog[*i].choice.clone()));
        }
        if resp.dragged() {
            ghost(ui, &v.catalog[*i].name, s);
        }
        v.panel.rows.push((key, r));
    }
    with_brush(&painter, |b| {
        for (idx, cat, y) in &items {
            if let Some(c) = cat {
                let r = Rect::from_min_size(pos2(view.left() + s.s(10.0), off + y), vec2(view.width() - s.s(20.0), s.s(HEAD)));
                if r.intersects(view) {
                    let name = t(c.key());
                    let _ = text::paint(b, r, &Text::new(&name, Role::Section, s.t.label).align(Align2::LEFT_CENTER), s.tk());
                }
            }
            let Some(i) = idx else { continue };
            let r = Rect::from_min_size(pos2(view.left() + s.s(4.0), off + y), vec2(view.width() - s.s(8.0), s.s(ROW) - s.s(2.0)));
            if !r.intersects(view) {
                continue;
            }
            let d = &v.catalog[*i];
            let sel = v.panel.selected.as_deref() == Some(d.key.as_str());
            let hov = hovered.as_deref() == Some(d.key.as_str());
            if sel || hov {
                let base = if sel { mix(s.t.ink, s.t.selection, 0.18) } else { shade(s.t.ink, if s.t.dark { 0.05 } else { -0.04 }) };
                b.rrect(r, s.r(5.0), Fill::Solid(base));
            }
            let pic = Rect::from_min_size(r.min + vec2(s.s(4.0), s.s(4.0)), vec2((r.height() - s.s(8.0)) * 1.6, r.height() - s.s(8.0)));
            let has = super::picker_grid::subject(d).and_then(|x| v.thumbs.get(x)).is_some();
            if !has {
                b.rrect(pic, s.r(4.0), Fill::V(mix(s.t.ink, super::picker_cover::hue(d, s), 0.5), mix(s.t.ink, super::picker_cover::hue(d, s), 0.2)));
                let g = Rect::from_center_size(pic.center(), egui::Vec2::splat(pic.height() * 0.8));
                super::picker_cover::glyph(b, g, d.primary(), with_alpha(s.t.text, 0.8), s.s(1.4));
            }
            let tx = pic.right() + s.s(8.0);
            let nr = Rect::from_min_max(pos2(tx, r.top() + s.s(5.0)), pos2(r.right() - s.s(22.0), r.center().y + s.s(1.0)));
            let _ = text::paint(b, nr, &Text::new(&d.name, Role::Title, s.t.text).align(Align2::LEFT_CENTER).policy(Policy::Elide(Elide::End)), s.tk());
            let wr = Rect::from_min_max(pos2(tx, r.center().y), pos2(r.right() - s.s(6.0), r.bottom() - s.s(4.0)));
            let what = match d.source.badge() {
                Some(bdg) => format!("{} · {bdg}", d.vendor),
                None => d.what.clone(),
            };
            let _ = text::paint(b, wr, &Text::new(&what, Role::Label, s.t.text_dim).align(Align2::LEFT_CENTER).policy(Policy::Elide(Elide::End)), s.tk());
            if v.prefs.is_favourite(&d.key) {
                let z = Rect::from_center_size(pos2(r.right() - s.s(12.0), r.top() + s.s(13.0)), egui::Vec2::splat(s.s(12.0)));
                glyph::paint(b, z, Glyph::StarFilled, s.t.brand_bright);
            }
        }
    });
    // The pictures, one mesh.
    let mut mesh = egui::Mesh::default();
    for (idx, _, y) in &items {
        let Some(i) = idx else { continue };
        let r = Rect::from_min_size(pos2(view.left() + s.s(4.0), off + y), vec2(view.width() - s.s(8.0), s.s(ROW) - s.s(2.0)));
        if !r.intersects(view) {
            continue;
        }
        if let Some((tex, uv)) = super::picker_grid::subject(&v.catalog[*i]).and_then(|x| v.thumbs.get(x)) {
            let pic = Rect::from_min_size(r.min + vec2(s.s(4.0), s.s(4.0)), vec2((r.height() - s.s(8.0)) * 1.6, r.height() - s.s(8.0)));
            mesh.texture_id = tex;
            mesh.add_rect_with_uv(pic, uv, egui::Color32::WHITE);
        }
    }
    if !mesh.is_empty() {
        painter.add(egui::Shape::mesh(mesh));
    }
    v.thumbs.pump(ui.ctx());
    chosen
}

/// The device's name following the pointer while it is dragged.
fn ghost(ui: &Ui, name: &str, s: &Style) {
    let Some(p) = ui.ctx().pointer_interact_pos() else {
        return;
    };
    egui::Area::new(Id::new("devices-drag-ghost"))
        .order(egui::Order::Tooltip)
        .fixed_pos(p + vec2(14.0, 10.0))
        .interactable(false)
        .show(ui.ctx(), |ui| {
            let t = Text::new(name, Role::Control, s.t.highlight_glyph);
            let w = text::measure(ui.ctx(), &t, s.tk()).x + s.s(20.0);
            let (r, _) = ui.allocate_exact_size(vec2(w, s.s(24.0)), Sense::hover());
            with_brush(ui.painter(), |b| {
                b.rrect(r, s.r(5.0), Fill::Solid(s.t.highlight));
                let _ = text::paint(b, r, &t, s.tk());
            });
        });
}
