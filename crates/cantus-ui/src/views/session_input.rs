//! The Session pane drawn and driven (ADR 0077): its frame — the header bar, the
//! scrolling, the dividers, a name typed in place, a clip dragged — around the grid
//! (`session_grid.rs`) and the heads, the master column and the stop row
//! (`session_master.rs`).

use super::layout::{Geometry, Hit, BAR};
use super::{Rename, SessionView};
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{with_brush, Fill, Style};
use cantus_core::session::{Quantize, SessionCommand};
use egui::{pos2, vec2, Align2, Id, Rect, Sense, Ui};

#[path = "session_grid.rs"]
mod grid;
#[path = "session_master.rs"]
mod master;

pub(super) use grid::choose as grid_choose;
#[cfg(test)]
pub(super) use grid::progress as grid_progress;
#[allow(unused_imports)]
pub(super) use grid::wave_of;

pub(super) fn pane(app: &mut CantusApp, ui: &mut Ui) {
    let s = Style::current();
    let k = s.k;
    let full = ui.available_rect_before_wrap();
    let bg = ui.allocate_rect(full, Sense::hover());
    // The pane takes the keyboard's focus with a press of the pointer in it, and lets it
    // go with a press elsewhere.
    let pressed = ui.input(|i| i.pointer.any_pressed());
    if pressed {
        app.session_view.focused = bg.contains_pointer();
    }
    let tracks = super::track_ids(app);
    let scenes = super::scene_ids(app);
    let panel = panel_slot(app).map(|_| app.session_view.panel_h * k);
    let sv = &app.session_view;
    let geo = Geometry::new(full, k, tracks, scenes, sv.col_w, sv.scroll * k, panel);
    app.session_view.scroll = geo.scroll / k;
    with_brush(ui.painter(), |b| {
        b.rect(full, Fill::Solid(s.t.panel));
        b.rect(geo.grid, Fill::Solid(s.t.ink));
        b.rect(geo.master, Fill::Solid(shade_panel(&s)));
    });
    header(app, ui, &geo, &s);
    if geo.tracks.is_empty() {
        empty(ui, &geo, &s, "session.empty.tracks");
        app.session_view.geo = Some(geo);
        return;
    }
    scroll(app, ui, &geo);
    let geo = {
        let sv = &app.session_view;
        Geometry::new(
            full,
            k,
            geo.tracks.clone(),
            geo.scenes.clone(),
            sv.col_w,
            sv.scroll * k,
            panel,
        )
    };
    grid::grid(app, ui, &geo, &s);
    master::heads(app, ui, &geo, &s);
    master::master(app, ui, &geo, &s);
    master::stops(app, ui, &geo, &s);
    scrollbars(app, ui, &geo);
    dividers(app, ui, &geo);
    rename(app, ui, &geo);
    if let (Some(rect), Some(slot)) = (geo.panel, panel_slot(app)) {
        if app.session_view.scene_chosen {
            super::clip::scene_panel(app, ui, rect, slot.scene);
        } else {
            super::clip::panel(app, ui, rect, slot);
        }
    }
    if geo.scenes.is_empty() {
        let r = Rect::from_min_max(
            geo.grid.min,
            pos2(geo.grid.right(), geo.grid.top() + 3.0 * geo.row_h),
        );
        empty(
            ui,
            &Geometry {
                grid: r,
                ..geo.clone()
            },
            &s,
            "session.empty.scenes",
        );
    }
    // A sample from the browser lands in the slot under the pointer (ADR 0030's drop).
    crate::samples::drop::publish(ui.ctx(), geo.grid, crate::samples::drop::Target::Session);
    app.session_view.geo = Some(geo);
}

fn shade_panel(s: &Style) -> egui::Color32 {
    crate::kit::color::shade(s.t.panel, if s.t.dark { -0.04 } else { -0.02 })
}

/// The slot whose panel shows (its clip's, or its scene's when the scene was chosen): the
/// cursor's, when the panel is open and there is something to show.
pub(super) fn panel_slot(app: &CantusApp) -> Option<cantus_core::session::SlotRef> {
    let sv = &app.session_view;
    let slot = sv.cursor?;
    let has = if sv.scene_chosen {
        app.project().session.scene(slot.scene).is_some()
    } else {
        app.project().slot_clip(slot).is_some()
    };
    (sv.panel_open && has).then_some(slot)
}

fn empty(ui: &Ui, geo: &Geometry, s: &Style, key: &str) {
    with_brush(ui.painter(), |b| {
        let t = Text::key(key, Role::Body, s.t.placeholder)
            .align(Align2::CENTER_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, geo.grid.shrink(12.0 * s.k), &t, s.tk());
    });
}

/// The header bar: the pane's name; the global launch quantisation; arrangement record;
/// *back to arrangement*; capture scene; a new scene; the clip panel's switch.
fn header(app: &mut CantusApp, ui: &mut Ui, geo: &Geometry, s: &Style) {
    let k = s.k;
    let h = geo.header;
    with_brush(ui.painter(), |b| {
        b.rect(h, Fill::Solid(s.t.toolbar));
        b.hline(h.left(), h.right(), h.bottom() - b.px(), b.px(), s.t.edge);
        let g = Rect::from_center_size(
            pos2(h.left() + 14.0 * k, h.center().y),
            vec2(14.0 * k, 14.0 * k),
        );
        crate::kit::glyph::paint(b, g, crate::kit::glyph::Glyph::Session, s.t.text_dim);
        let tx = Text {
            caps: true,
            ..Text::key("session.pane", Role::Section, s.t.label).align(Align2::LEFT_CENTER)
        };
        let _ = text::paint(
            b,
            Rect::from_min_max(
                pos2(g.right() + 6.0 * k, h.top()),
                pos2(g.right() + 90.0 * k, h.bottom()),
            ),
            &tx,
            s.tk(),
        );
    });
    let y = h.center().y;
    let row = |x: f32, w: f32| Rect::from_min_size(pos2(x, y - 10.0 * k), vec2(w * k, 20.0 * k));
    let mut x = h.left() + 112.0 * k;
    // Launch quantisation: the label, then its value in a menu field.
    x = label(ui, s, x, y, "session.header.quantize");
    let q = app.project().session.settings.launch_quantize;
    let rq = row(x, 70.0);
    let resp = ui.interact(rq, Id::new("cantus-session-quant"), Sense::click());
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp));
    with_brush(ui.painter(), |b| {
        crate::kit::well::menu_field(
            b,
            rq,
            s,
            crate::kit::State::of(&resp, open),
            &t(q.label_key()),
            false,
        )
    });
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            &t("session.header.quantize_tip"),
            "",
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    crate::controllers::learn_spot(
        app,
        ui,
        rq,
        cantus_core::controller::MappingTarget::SessionQuantize,
    );
    egui::Popup::menu(&resp)
        .frame(crate::views::clips_chrome::menu_frame())
        .show(|ui| {
            let all: Vec<Quantize> = Quantize::ALL
                .into_iter()
                .filter(|x| *x != Quantize::Global)
                .collect();
            let labels: Vec<String> = all.iter().map(|x| t(x.label_key())).collect();
            let items: Vec<crate::kit::menu::Item> = all
                .iter()
                .zip(&labels)
                .map(|(x, l)| crate::kit::menu::Item::Action {
                    label: crate::kit::text::Src::Str(l),
                    check: crate::kit::menu::Check::Radio(*x == q),
                    shortcut: None,
                    submenu: false,
                    enabled: true,
                })
                .collect();
            if let Some(i) = crate::kit::widgets::menu_list(ui, &items, 120.0 * k)
                .1
                .clicked
            {
                let mut settings = app.project().session.settings;
                settings.launch_quantize = all[i];
                super::exec(app, SessionCommand::SetSettings { settings });
                ui.close();
            }
        });
    x = rq.right() + 14.0 * k;
    // Keys at the right: arrangement record, back to arrangement, capture, + scene, panel.
    let mut right = h.right() - 6.0 * k;
    let mut key = |ui: &mut Ui, w: f32, id: &str| {
        let r = Rect::from_min_max(pos2(right - w * k, y - 10.0 * k), pos2(right, y + 10.0 * k));
        right = r.left() - 4.0 * k;
        let resp = ui.interact(r, Id::new(("cantus-session-key", id)), Sense::click());
        (r, resp)
    };
    let (r_panel, p_resp) = key(ui, 22.0, "panel");
    let (r_scene, s_resp) = key(ui, 22.0, "scene");
    let (r_cap, c_resp) = key(ui, 64.0, "capture");
    let (r_back, b_resp) = key(ui, 72.0, "back");
    let (r_arr, a_resp) = key(ui, 72.0, "arr");
    let held = app.session_view.live.holds_any();
    let arr = app.session_view.live.arrangement_record;
    let panel_open = app.session_view.panel_open;
    with_brush(ui.painter(), |b| {
        use super::paint::{glyph_key, key as word_key};
        word_key(
            b,
            r_arr,
            s,
            &t("session.header.arr_record"),
            arr.then_some(s.t.danger),
            a_resp.hovered(),
        );
        word_key(
            b,
            r_back,
            s,
            &t("session.header.back"),
            held.then_some(s.t.highlight),
            b_resp.hovered(),
        );
        word_key(
            b,
            r_cap,
            s,
            &t("session.header.capture"),
            None,
            c_resp.hovered(),
        );
        glyph_key(
            b,
            r_scene,
            s,
            crate::kit::glyph::Glyph::Plus,
            None,
            s_resp.hovered(),
        );
        glyph_key(
            b,
            r_panel,
            s,
            crate::kit::glyph::Glyph::Inspector,
            panel_open.then_some(s.t.highlight),
            p_resp.hovered(),
        );
    });
    // The screen reader hears each key by its name.
    for (resp, key) in [
        (&a_resp, "session.header.arr_record_tip"),
        (&b_resp, "session.header.back_tip"),
        (&c_resp, "session.header.capture_tip"),
        (&s_resp, "session.header.scene_tip"),
        (&p_resp, "session.header.panel_tip"),
    ] {
        // Made only when the screen reader asks (nothing allocated a frame otherwise).
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, t(key)));
    }
    for (resp, tip) in [
        (&a_resp, "session.header.arr_record_tip"),
        (&b_resp, "session.header.back_tip"),
        (&c_resp, "session.header.capture_tip"),
        (&s_resp, "session.header.scene_tip"),
        (
            &p_resp,
            if panel_open && geo.panel.is_none() && panel_slot(app).is_some() {
                "session.header.panel_short_tip"
            } else {
                "session.header.panel_tip"
            },
        ),
    ] {
        if resp.hovered() {
            hint::set_text(ui.ctx(), &t(tip), "", Caps::NONE);
        }
    }
    let learn = app.controllers.learn_mode;
    if crate::controllers::learn_action_spot(
        app,
        ui,
        r_arr,
        crate::keymap::Action::SessionArrangementRecord,
    ) || crate::controllers::learn_action_spot(
        app,
        ui,
        r_back,
        crate::keymap::Action::SessionBackToArrangement,
    ) || learn
    {
        return;
    }
    if a_resp.clicked() {
        super::set_arrangement_record(app, !arr);
    }
    if b_resp.clicked() {
        super::back_to_arrangement(app, None);
    }
    if c_resp.clicked() {
        super::keys::capture_scene(app);
    }
    if s_resp.clicked() {
        super::keys::insert_scene(app);
    }
    if p_resp.clicked() {
        app.session_view.panel_open = !panel_open;
    }
    let _ = x;
}

/// A label before a field in the header; returns where the field goes.
fn label(ui: &Ui, s: &Style, x: f32, y: f32, key: &str) -> f32 {
    let k = s.k;
    let mut w = 0.0;
    with_brush(ui.painter(), |b| {
        let tx = Text::key(key, Role::Label, s.t.text_dim).align(Align2::LEFT_CENTER);
        w = text::measure(b.ctx(), &tx, s.tk()).x;
        let _ = text::paint(
            b,
            Rect::from_min_max(pos2(x, y - 10.0 * k), pos2(x + w + 1.0, y + 10.0 * k)),
            &tx,
            s.tk(),
        );
    });
    x + w + 6.0 * k
}

/// The wheel over the grid, the heads, the stop row or the scenes: up and down (Shift:
/// sideways); the middle button pans.
fn scroll(app: &mut CantusApp, ui: &Ui, geo: &Geometry) {
    let Some(p) = ui.input(|i| i.pointer.hover_pos()) else {
        return;
    };
    let body = geo.heads.union(geo.grid).union(geo.stops).union(geo.master);
    if !body.contains(p) || geo.panel.is_some_and(|r| r.contains(p)) {
        return;
    }
    let w = crate::views::clips_chrome::wheel(ui);
    if w.scroll == egui::Vec2::ZERO {
        return;
    }
    let k = geo.k;
    let next = geo.clamp_scroll(geo.scroll - w.scroll);
    app.session_view.scroll = next / k;
}

/// The kit's scrollbars along the grid's right and bottom edges when it overflows.
fn scrollbars(app: &mut CantusApp, ui: &mut Ui, geo: &Geometry) {
    let k = geo.k;
    let content = geo.content();
    let (w, h) = (geo.grid.width() - BAR * k, geo.grid.height() - BAR * k);
    if content.y > h {
        let lane = Rect::from_min_max(
            pos2(geo.grid.right() - 5.0 * k, geo.grid.top()),
            pos2(geo.grid.right(), geo.grid.bottom() - BAR * k),
        );
        let len = (h / content.y).min(1.0);
        let mut start = geo.scroll.y / content.y;
        if crate::kit::widgets::scrollbar(ui, lane, Id::new("cantus-session-vbar"), &mut start, len)
            .dragged()
        {
            app.session_view.scroll.y =
                geo.clamp_scroll(vec2(geo.scroll.x, start * content.y)).y / k;
        }
    }
    if content.x > w {
        let lane = Rect::from_min_max(
            pos2(geo.grid.left(), geo.grid.bottom() - 5.0 * k),
            pos2(geo.grid.right() - BAR * k, geo.grid.bottom()),
        );
        let len = (w / content.x).min(1.0);
        let mut start = geo.scroll.x / content.x;
        if crate::kit::widgets::scrollbar(ui, lane, Id::new("cantus-session-hbar"), &mut start, len)
            .dragged()
        {
            app.session_view.scroll.x =
                geo.clamp_scroll(vec2(start * content.x, geo.scroll.y)).x / k;
        }
    }
}

/// A head's right edge sets every column's width; the clip panel's top edge its height.
/// Right-click resets either, as every splitter does.
fn dividers(app: &mut CantusApp, ui: &mut Ui, geo: &Geometry) {
    let k = geo.k;
    let Some(p) = ui.input(|i| i.pointer.hover_pos()) else {
        return;
    };
    let hit = geo.hit(p);
    let over = matches!(hit, Some(Hit::Divider | Hit::PanelEdge));
    let sv: &mut SessionView = &mut app.session_view;
    let (down, pressed, released, secondary) = ui.input(|i| {
        (
            i.pointer.primary_down(),
            i.pointer.primary_pressed(),
            i.pointer.primary_released(),
            i.pointer.secondary_clicked(),
        )
    });
    if over && sv.resize.is_none() {
        ui.ctx().set_cursor_icon(if hit == Some(Hit::Divider) {
            egui::CursorIcon::ResizeHorizontal
        } else {
            egui::CursorIcon::ResizeVertical
        });
        if pressed {
            let start = if hit == Some(Hit::Divider) {
                sv.col_w
            } else {
                sv.panel_h
            };
            let at = if hit == Some(Hit::Divider) { p.x } else { p.y };
            sv.resize = Some((hit.expect("over"), at, start));
        }
        if secondary {
            match hit {
                Some(Hit::Divider) => sv.col_w = super::layout::COL_W,
                _ => sv.panel_h = super::layout::PANEL_H,
            }
        }
    }
    if let Some((what, from, start)) = sv.resize {
        if down {
            match what {
                Hit::Divider => {
                    sv.col_w = (start + (p.x - from) / k)
                        .clamp(super::layout::MIN_COL_W, super::layout::MAX_COL_W)
                }
                _ => sv.panel_h = (start - (p.y - from) / k).max(super::layout::MIN_PANEL_H),
            }
        }
        if released || !down {
            sv.resize = None;
        }
    }
}

/// A clip's or a scene's name typed in place: Enter or a click elsewhere renames, Esc
/// leaves it.
fn rename(app: &mut CantusApp, ui: &mut Ui, geo: &Geometry) {
    let Some(r) = app.session_view.rename.clone() else {
        return;
    };
    let k = geo.k;
    let (rect, id) = match &r {
        Rename::Clip(slot, _) => {
            let Some((i, j)) = geo.index_of(*slot) else {
                app.session_view.rename = None;
                return;
            };
            let c = geo.cell(i, j);
            (
                Rect::from_min_size(
                    c.min + vec2(18.0 * k, 0.0),
                    vec2((c.width() - 18.0 * k).max(40.0 * k), 18.0 * k),
                ),
                Id::new(("session-rename-clip", slot.track.0, slot.scene.0)),
            )
        }
        Rename::Scene(scene, _) => {
            let Some(j) = geo.scenes.iter().position(|s| s == scene) else {
                app.session_view.rename = None;
                return;
            };
            let c = geo.scene(j);
            (
                Rect::from_min_size(
                    c.min + vec2(18.0 * k, 0.0),
                    vec2(c.width() - 20.0 * k, c.height()),
                ),
                Id::new(("session-rename-scene", scene.0)),
            )
        }
    };
    let mut text = match &r {
        Rename::Clip(_, n) | Rename::Scene(_, n) => n.clone(),
    };
    let out = crate::kit::field::Field::new(id)
        .opens_when_shown()
        .scale(k)
        .show(ui, rect, &mut text);
    match (&r, out.event) {
        (_, None) => {
            app.session_view.rename = Some(match r {
                Rename::Clip(s, _) => Rename::Clip(s, text),
                Rename::Scene(s, _) => Rename::Scene(s, text),
            });
        }
        (_, Some(crate::kit::field::Event::Cancel)) => app.session_view.rename = None,
        (Rename::Clip(slot, _), Some(crate::kit::field::Event::Commit)) => {
            app.session_view.rename = None;
            if let Some(c) = app.project().slot_clip(*slot) {
                let id = c.id();
                app.rename_clip((slot.track, id), text);
            }
        }
        (Rename::Scene(scene, _), Some(crate::kit::field::Event::Commit)) => {
            app.session_view.rename = None;
            let name = text.trim().to_string();
            let same = app
                .project()
                .session
                .scene(*scene)
                .is_some_and(|s| s.name == name);
            if !same {
                super::exec(app, SessionCommand::SetSceneName { id: *scene, name });
            }
        }
    }
}
