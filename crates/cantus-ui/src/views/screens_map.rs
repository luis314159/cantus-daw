//! **Screens and windows** (ADR 0151 §9): every screen to scale where the system places it —
//! its name, its size, the system's scale and ours — and the windows on it. Drag a window
//! onto another screen to send it there; double-click one to bring it to the front. A
//! window waiting for a screen that is not connected is shown on that screen, dashed.
//! View ▸ *Screens and windows…*; Esc, a click outside or its ✕ close it.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::glyph::{self, Glyph};
use crate::kit::text::{self as ktext, Role, Text};
use crate::kit::{with_brush, Fill, Style};
use crate::screens::{PxRect, ScreenKey};
use egui::{pos2, vec2, Align2, Color32, Id, Rect, Sense, Ui};

/// What the map shows of one window.
struct Shown {
    /// `None`: the main window.
    key: Option<u32>,
    rect: PxRect,
    label: String,
    glyph: Glyph,
}

fn words(ui: &Ui, r: Rect, w: &str, role: Role, c: Color32, a: Align2) {
    let s = Style::current();
    with_brush(ui.painter(), |b| {
        let text = Text::new(w, role, c)
            .align(a)
            .policy(ktext::Policy::Elide(ktext::Elide::End));
        let _ = ktext::paint(b, r, &text, s.tk());
    });
}

/// The map, while open (the overlays' pass).
pub fn frame(app: &mut CantusApp, ctx: &egui::Context) {
    if !app.windows.map_open {
        return;
    }
    let s = Style::current();
    let k = s.k;
    let screen = ctx.content_rect();
    let size = vec2(760.0 * k, 430.0 * k).min(screen.size() - vec2(16.0, 16.0));
    let rect = Rect::from_center_size(screen.center(), size);
    // Every screen, the ones a waiting window needs too.
    let mut screens: Vec<(ScreenKey, bool)> = app
        .windows
        .screens
        .iter()
        .map(|s| (s.clone(), true))
        .collect();
    for w in &app.windows.parked {
        if let Some(p) = &w.place {
            if !screens.iter().any(|(s, _)| s.pos == p.screen.pos && s.size == p.screen.size) {
                screens.push((p.screen.clone(), false));
            }
        }
    }
    let mut shown: Vec<Shown> = Vec::new();
    if let Some(r) = app.windows.main.outer {
        shown.push(Shown {
            key: None,
            rect: r,
            label: t("screens.map.main"),
            glyph: Glyph::Arrangement,
        });
    }
    for (i, w) in app.windows.list.iter().enumerate() {
        if let Some(r) = w.seen.outer.or(w.place.as_ref().map(|p| p.rect)) {
            let glyph = app
                .dock(crate::windows::Dock::Window(i))
                .panes()
                .first()
                .map_or(Glyph::Device, |p| p.glyph());
            shown.push(Shown {
                key: Some(w.key),
                rect: r,
                label: app.window_name(i),
                glyph,
            });
        }
    }
    for w in &app.windows.parked {
        if let Some(p) = &w.place {
            shown.push(Shown {
                key: Some(w.key),
                rect: p.rect,
                label: w.name.clone(),
                glyph: w.panes().first().map_or(Glyph::Device, |p| p.glyph()),
            });
        }
    }
    let mut close = false;
    let mut sent: Option<(u32, usize)> = None;
    let mut front: Option<Option<u32>> = None;
    let area = egui::Area::new(Id::new("cantus-screens-map"))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            ui.set_min_size(size);
            crate::views::audio_parts::panel(ui, rect, &s);
            let head = Rect::from_min_size(rect.min, vec2(rect.width(), 32.0 * k));
            if crate::kit::dialog::head(ui, head, &s, Glyph::Maximise, "screens.map.title") {
                close = true;
            }
            let hint = Rect::from_min_max(
                pos2(rect.left() + 20.0 * k, rect.bottom() - 34.0 * k),
                pos2(rect.right() - 20.0 * k, rect.bottom() - 12.0 * k),
            );
            let dim = s.t.text_dim;
            words(ui, hint, &t("screens.map.hint"), Role::Body, dim, Align2::LEFT_CENTER);
            let body = Rect::from_min_max(
                pos2(rect.left() + 20.0 * k, head.bottom() + 16.0 * k),
                pos2(rect.right() - 20.0 * k, hint.top() - 12.0 * k),
            );
            let all = screens.iter().fold(Rect::NOTHING, |a, (sc, _)| {
                a.union(Rect::from_min_size(
                    pos2(sc.pos[0] as f32, sc.pos[1] as f32),
                    vec2(sc.size[0] as f32, sc.size[1] as f32),
                ))
            });
            if !all.is_positive() {
                return;
            }
            let f = (body.width() / all.width()).min(body.height() / all.height());
            let off = body.center() - all.center().to_vec2() * f;
            let to = |x: f32, y: f32| off + vec2(x, y) * f;
            let rect_of = |r: &PxRect| {
                Rect::from_min_max(
                    to(r.x as f32, r.y as f32),
                    to(r.right() as f32, r.bottom() as f32),
                )
            };
            let mut screen_rects = Vec::new();
            for (n, (sc, there)) in screens.iter().enumerate() {
                let r = rect_of(&sc.rect());
                screen_rects.push((r, *there));
                draw_screen(ui, r, n + 1, sc, *there, app_zoom(app, sc));
            }
            for w in &shown {
                let r = rect_of(&w.rect).shrink(2.0 * k);
                let id = Id::new(("cantus-map-window", w.key));
                let resp = ui.interact(r, id, Sense::click_and_drag());
                let lit = resp.hovered() || resp.dragged();
                let shift = if resp.dragged() {
                    resp.ctx.input(|i| i.pointer.delta())
                } else {
                    egui::Vec2::ZERO
                };
                let r = r.translate(ui.ctx().data_mut(|d| {
                    let acc = d.get_temp_mut_or_default::<egui::Vec2>(id);
                    *acc = if resp.dragged() { *acc + shift } else { egui::Vec2::ZERO };
                    *acc
                }));
                draw_window(ui, r, w, lit);
                if resp.double_clicked() {
                    front = Some(w.key);
                }
                if resp.drag_stopped() {
                    let at = resp.interact_pointer_pos().unwrap_or(r.center());
                    let target = screen_rects
                        .iter()
                        .position(|(sr, there)| *there && sr.contains(at));
                    if let (Some(key), Some(t)) = (w.key, target) {
                        sent = Some((key, t));
                    }
                }
            }
        });
    let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    let outside = ctx.input(|i| i.pointer.any_pressed())
        && !ctx
            .pointer_interact_pos()
            .is_some_and(|p| area.response.rect.contains(p));
    if let Some((key, screen)) = sent {
        app.send_window_to_screen(key, screen);
    }
    if let Some(key) = front {
        let id = key.map_or(egui::ViewportId::ROOT, crate::windows::id_of);
        ctx.send_viewport_cmd_to(id, egui::ViewportCommand::Focus);
    }
    if close || escape || outside {
        app.windows.map_open = false;
    }
}

/// Our interface scale on screen `sc` (a window there would draw at it).
fn app_zoom(app: &CantusApp, sc: &ScreenKey) -> f32 {
    let rule = crate::screen_scale::for_screen(sc.size[0] as f32, sc.scale);
    let main = app.windows.main_rule.max(0.25);
    let zoom = app.theme_ctx.as_ref().map_or(1.0, |c| c.zoom_factor());
    zoom * rule / main
}

fn draw_screen(ui: &Ui, r: Rect, n: usize, sc: &ScreenKey, there: bool, zoom: f32) {
    let s = Style::current();
    let k = s.k;
    with_brush(ui.painter(), |b| {
        let r = b.snap_rect(r);
        if there {
            b.rrect(r, 6.0 * k, Fill::Solid(s.t.well));
            b.rrect_stroke(r, 6.0 * k, b.px(), s.t.edge);
        } else {
            let c = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
            b.dashed(&c, 1.5 * b.px(), s.t.text_dim, 6.0 * k, 4.0 * k);
        }
    });
    let line = 18.0 * k;
    let top = Rect::from_min_size(r.min + vec2(10.0 * k, 8.0 * k), vec2(r.width() - 20.0 * k, line));
    let name = crate::windows_docks::screen_label(sc);
    let title = format!("{} · {}", tf("screens.map.screen", &[("n", &n)]), name);
    let ink = if there { s.t.text } else { s.t.text_dim };
    words(ui, top, &title, Role::Section, ink, Align2::LEFT_CENTER);
    let detail = if there {
        let sys = (sc.scale * 100.0).round();
        let z = (zoom * 100.0).round();
        format!(
            "{} · {}",
            tf("screens.map.size", &[("w", &sc.size[0]), ("h", &sc.size[1])]),
            tf("screens.map.scale", &[("sys", &sys), ("ui", &z)])
        )
    } else {
        t("screens.map.missing")
    };
    let second = top.translate(vec2(0.0, line));
    words(ui, second, &detail, Role::Label, s.t.text_dim, Align2::LEFT_CENTER);
}

fn draw_window(ui: &Ui, r: Rect, w: &Shown, lit: bool) {
    let s = Style::current();
    let k = s.k;
    let main = w.key.is_none();
    with_brush(ui.painter(), |b| {
        let r = b.snap_rect(r);
        let fill = if lit || !main { s.t.selection } else { s.t.panel_raised };
        b.rrect(r, 4.0 * k, Fill::Solid(fill));
        let edge = if lit { s.t.highlight } else { s.t.edge };
        b.rrect_stroke(r, 4.0 * k, b.px(), edge);
        let side = (16.0 * k).min(r.height() * 0.4);
        let gl = Rect::from_center_size(r.center() - vec2(0.0, side * 0.6), vec2(side, side));
        glyph::paint(b, gl, w.glyph, if main { s.t.text } else { s.t.highlight });
    });
    let lr = Rect::from_center_size(
        r.center() + vec2(0.0, 10.0 * k),
        vec2((r.width() - 8.0 * k).max(1.0), 18.0 * k),
    );
    words(ui, lr, &w.label, Role::Body, s.t.text, Align2::CENTER_CENTER);
}

impl CantusApp {
    /// The screens map's drag: window `key` onto screen `screen`, maximised there (placed
    /// at its next pass).
    pub fn send_window_to_screen(&mut self, key: u32, screen: usize) {
        let Some(i) = self.windows.index(key) else {
            return;
        };
        if let Some(p) = crate::screens::fresh_place(&self.windows.screens, screen) {
            let w = &mut self.windows.list[i];
            w.place = Some(p);
            w.shown = false;
        }
        if let Some(ctx) = &self.theme_ctx {
            ctx.request_repaint_of(egui::ViewportId::ROOT);
        }
    }
}
