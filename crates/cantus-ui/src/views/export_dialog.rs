//! The export dialog, drawn with the kit (ADR 0024, ADR 0033 §7): a floating panel over a
//! dimmed room, its title row, the presets as cards down the left, the choices in banded
//! rows on the right (what, format and quality, rate, channels, dither, loudness, tags),
//! and a footer with the progress and the buttons. No stock egui widget: segments, menu
//! fields, lamps, wells and caps.

use super::audio_parts as parts;
use super::export::{self as ex};
use super::workspace_widgets as w;
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::glyph::Glyph;
use crate::kit::menu::{Check, Item};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{band, well, with_brush, Fill, State, Style, Tok};
use cantus_export::{ExportSettings, Preset, Scope};
use egui::{Align2, Id, Pos2, Rect, Response, Sense, Ui, Vec2};

/// The window at 100 %.
pub const SIZE: Vec2 = Vec2::new(820.0, 740.0);
/// The presets' column.
const LEFT: f32 = 210.0;
/// A row's caption column.
pub(super) const CAPTION: f32 = 118.0;

/// The dialog, when open.
pub fn show(app: &mut CantusApp, ctx: &egui::Context) {
    if app.export.dialog.is_none() {
        return;
    }
    ctx.data_mut(|d| d.remove::<Geo>(geo_id()));
    // A menu left open behind the dialog when it opened (a clip's context menu) goes: it
    // would sit under the veil and take Esc and Enter.
    if std::mem::take(&mut app.export.fresh) {
        egui::Popup::close_all(ctx);
    }
    // A list open as the frame starts takes Esc for itself, and a tag being typed takes
    // Enter (egui closes the one and lets go of the other during the frame — a single-line
    // field gives up its focus on the Enter that confirms it — so both are asked before
    // anything is drawn).
    let list_open = egui::Popup::is_any_open(ctx);
    let typing = ctx.egui_wants_keyboard_input();
    let s = Style::current();
    let k = s.k;
    let screen = ctx.content_rect();
    let busy = app.export.busy();
    if busy {
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }
    egui::Area::new(Id::new("cantus-export-veil"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (r, _) = ui.allocate_exact_size(screen.size(), Sense::click());
            with_brush(ui.painter(), |b| {
                b.rect(
                    r,
                    Fill::Solid(crate::kit::color::with_alpha(s.t.shadow, 0.5)),
                )
            });
        });
    let size = (SIZE * k).min(screen.size() - Vec2::splat(24.0));
    let rect = Rect::from_center_size(screen.center(), size);
    let mut close = false;
    egui::Area::new(layer().id)
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            ui.set_min_size(size);
            parts::panel(ui, rect, &s);
            let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 34.0 * k));
            if super::workspace_settings::title(ui, head, &s, Glyph::Send, "export_audio.title") {
                close = true;
            }
            let foot = Rect::from_min_max(
                Pos2::new(rect.left() + 18.0 * k, rect.bottom() - 52.0 * k),
                Pos2::new(rect.right() - 18.0 * k, rect.bottom() - 14.0 * k),
            );
            let left = Rect::from_min_max(
                Pos2::new(rect.left() + 18.0 * k, head.bottom() + 12.0 * k),
                Pos2::new(rect.left() + (18.0 + LEFT) * k, foot.top() - 10.0 * k),
            );
            let right = Rect::from_min_max(
                Pos2::new(left.right() + 20.0 * k, left.top()),
                Pos2::new(rect.right() - 18.0 * k, left.bottom()),
            );
            with_brush(ui.painter(), |b| {
                let x = b.snap(left.right() + 10.0 * k);
                b.vline(
                    x,
                    left.top(),
                    left.bottom(),
                    b.px(),
                    crate::kit::color::shade(s.t.panel, -0.12),
                );
            });
            let mut d = app.export.dialog.clone().unwrap_or_default();
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(left));
            child.add_enabled_ui(!busy, |ui| presets(ui, &mut d));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(right));
            egui::ScrollArea::vertical()
                .id_salt("cantus-export-body")
                .auto_shrink([false, false])
                .max_height(right.height())
                .show(&mut child, |ui| {
                    ui.set_width(right.width() - 12.0 * k);
                    ui.spacing_mut().item_spacing = Vec2::new(8.0 * k, 6.0 * k);
                    ui.add_enabled_ui(!busy, |ui| {
                        super::export_rows::body(app, ui, &mut d);
                    });
                });
            app.export.dialog = Some(d);
            if footer(app, ui, foot) {
                close = true;
            }
        });
    // Over the veil and the window's own menus, every frame; its lists are sublayers of
    // it (see `pick`), so they stay on top of it.
    ctx.move_to_top(layer());
    let escape = !list_open && ctx.input(|i| i.key_pressed(egui::Key::Escape));
    // Enter exports, as the button does — unless a tag is being typed or a list is open.
    let enter = !list_open
        && !typing
        && !ctx.egui_wants_keyboard_input()
        && ctx.input(|i| i.key_pressed(egui::Key::Enter));
    if enter && !app.export.busy() && app.export.dialog.is_some() {
        app.export.enter_pressed += 1;
        ex::choose_and_start(app);
    } else if (close || escape) && !app.export.busy() {
        ex::close(app);
    }
}

/// The dialog's layer.
fn layer() -> egui::LayerId {
    egui::LayerId::new(egui::Order::Foreground, Id::new("cantus-export"))
}

/// Where the dialog drew its controls this frame, by name: a row's control by its
/// caption key, a segment as `key#i`, the open list's items as `list#i`, the presets as
/// `preset#i`, the footer's buttons as `button.export` / `button.close` / `button.stop`.
/// The tests click through it.
type Geo = Vec<(String, Rect)>;

fn geo_id() -> Id {
    Id::new("cantus-export-geo")
}

/// Where `name` was drawn in the last frame of the dialog.
pub fn geo(ctx: &egui::Context, name: &str) -> Option<Rect> {
    ctx.data(|d| d.get_temp::<Geo>(geo_id()))
        .unwrap_or_default()
        .into_iter()
        .rev()
        .find(|(n, _)| n == name)
        .map(|(_, r)| r)
}

/// Every name drawn in the last frame of the dialog.
pub fn geo_names(ctx: &egui::Context) -> Vec<String> {
    ctx.data(|d| d.get_temp::<Geo>(geo_id()))
        .unwrap_or_default()
        .into_iter()
        .map(|(n, _)| n)
        .collect()
}

pub(super) fn put(ui: &Ui, name: impl Into<String>, r: Rect) {
    let name = name.into();
    ui.ctx()
        .data_mut(|d| d.get_temp_mut_or_default::<Geo>(geo_id()).push((name, r)));
}

/// The presets as cards, and *Custom* lit when the choices match none.
fn presets(ui: &mut Ui, d: &mut ExportSettings) {
    let s = Style::current();
    let k = s.k;
    band_row(ui, "export_audio.band.presets");
    let current = d.preset();
    let w = ui.available_width();
    for p in Preset::ALL {
        let (name, hint) = p.keys();
        let (r, _) = ui.allocate_exact_size(Vec2::new(w, 58.0 * k), Sense::hover());
        let resp = parts::choice_key(
            ui,
            r,
            Id::new(("export-preset", p as u8)),
            &t(name),
            &t(hint),
            Some(preset_dot(p)),
            current == Some(p),
        );
        put(ui, format!("preset#{}", p as u8), r);
        if resp.clicked() {
            *d = p.apply(d);
        }
        ui.add_space(6.0 * k);
    }
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, 40.0 * k), Sense::hover());
    let custom = t("export_audio.preset.custom");
    let hint = t("export_audio.preset.custom_hint");
    parts::choice_key(
        ui,
        r,
        Id::new("export-preset-custom"),
        &custom,
        &hint,
        None,
        current.is_none(),
    );
}

fn preset_dot(p: Preset) -> Tok {
    match p {
        Preset::Streaming => Tok::Selection,
        Preset::Cd => Tok::Led,
        Preset::MasteringStems => Tok::Brand,
        Preset::Podcast => Tok::Highlight,
    }
}

/// A band naming the rows under it.
pub(super) fn band_row(ui: &mut Ui, key: &str) {
    let s = Style::current();
    ui.add_space(4.0 * s.k);
    let (r, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), s.d(band::BAND)),
        Sense::hover(),
    );
    with_brush(ui.painter(), |b| {
        band::section(b, r, &s, State::REST, Src::Key(key), None, false)
    });
}

/// A caption on the left and the row's control to its right; returns the control's
/// response.
pub(super) fn row<R>(ui: &mut Ui, key: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    let s = Style::current();
    ui.horizontal(|ui| {
        w::row_label(ui, key, CAPTION, 24.0);
        ui.add_space(6.0 * s.k);
        add(ui)
    })
    .inner
}

/// A list of choices opened from a menu field; returns the one clicked.
pub(super) fn pick<T: Copy + PartialEq>(
    field: &Response,
    options: &[(T, String)],
    current: T,
) -> Option<T> {
    let mut chosen = None;
    egui::Popup::menu(field)
        .frame(egui::Frame::NONE)
        .show(|ui| {
            // The list is in the foreground like the dialog, which moves itself to the
            // top every frame: as a sublayer it is drawn — and clicked — just above it.
            ui.ctx().set_sublayer(layer(), ui.layer_id());
            let items: Vec<Item<'_>> = options
                .iter()
                .map(|(v, label)| Item::Action {
                    label: Src::Str(label),
                    check: Check::Radio(*v == current),
                    shortcut: None,
                    submenu: false,
                    enabled: true,
                })
                .collect();
            let width = field.rect.width().max(160.0);
            let (resp, clicks) = crate::kit::widgets::menu_list(ui, &items, width);
            let k = Style::current().d(1.0);
            for i in 0..items.len() {
                put(
                    ui,
                    format!("list#{i}"),
                    crate::kit::menu::row(resp.rect, &items, k, i),
                );
            }
            if let Some(i) = clicks.clicked {
                chosen = Some(options[i].0);
                ui.close();
            }
        });
    chosen
}

/// The footer: the progress (or how the last export ended) and the buttons. Returns
/// whether the dialog should close.
fn footer(app: &mut CantusApp, ui: &mut Ui, rect: Rect) -> bool {
    let s = Style::current();
    let k = s.k;
    let buttons = Rect::from_min_max(Pos2::new(rect.right() - 250.0 * k, rect.top()), rect.max);
    let status = Rect::from_min_max(
        rect.min,
        Pos2::new(buttons.left() - 16.0 * k, rect.bottom()),
    );
    let busy = app.export.busy();
    if let Some((phase, fraction)) = app.export.progress() {
        let bar = Rect::from_min_max(
            Pos2::new(status.left(), status.center().y - 11.0 * k),
            Pos2::new(status.right(), status.center().y + 11.0 * k),
        );
        let time = ui.input(|i| i.time);
        with_brush(ui.painter(), |b| {
            progress(b, bar, &s, &t(phase), fraction, time)
        });
    } else if let Some(last) = &app.export.last {
        let (msg, warn) = match last {
            Ok(m) => (m.as_str(), false),
            Err(m) => (m.as_str(), true),
        };
        with_brush(ui.painter(), |b| {
            let _ = text::paint(
                b,
                status,
                &Text::new(
                    msg,
                    Role::Body,
                    if warn { s.t.danger } else { s.t.text_dim },
                )
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::Middle)),
                s.tk(),
            );
        });
    }
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(buttons)
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    let mut close = false;
    if busy {
        let b = parts::button(&mut child, &t("export_audio.stop"), true, true, 96.0);
        put(&child, "button.stop", b.rect);
        if b.clicked() {
            ex::stop(app);
        }
    } else {
        let d = app.export.dialog.clone().unwrap_or_default();
        let label = if d.scope == Scope::Stems {
            t("export_audio.export_stems")
        } else {
            t("export_audio.export")
        };
        let b = parts::button(&mut child, &label, true, true, 110.0);
        put(&child, "button.export", b.rect);
        if b.clicked() {
            ex::choose_and_start(app);
        }
        child.add_space(8.0 * k);
        let close_label = if app.export.last.is_some() {
            t("export_audio.close")
        } else {
            t("export_audio.cancel")
        };
        let b = parts::button(&mut child, &close_label, false, true, 90.0);
        put(&child, "button.close", b.rect);
        close = b.clicked();
    }
    close
}

/// A progress bar in a well: filled to `fraction` in the selection colour with the phase
/// and the percentage over it, or — when the render cannot say how far it is — a lit
/// block gliding along it.
pub(super) fn progress(
    b: &mut crate::kit::Brush,
    r: Rect,
    s: &Style,
    phase: &str,
    fraction: Option<f32>,
    time: f64,
) {
    b.register(
        r,
        "export-progress",
        &[Tok::Well, Tok::Selection, Tok::Text],
        &[],
    );
    well::paint(b, r, s, State::REST, None);
    let inner = r.shrink(s.s(2.0));
    let fill = crate::kit::color::with_alpha(s.t.selection, 0.55);
    let label = match fraction {
        Some(f) => {
            let f = f.clamp(0.0, 1.0);
            let w = inner.width() * f;
            b.rrect(
                Rect::from_min_size(inner.min, Vec2::new(w, inner.height())),
                s.r(1.5),
                Fill::Solid(fill),
            );
            format!("{phase}  {:.0} %", f * 100.0)
        }
        None => {
            let span = inner.width() * 0.22;
            let phase_t = (time * 0.6).fract() as f32;
            let x = inner.left() + (inner.width() + span) * phase_t - span;
            let block = Rect::from_min_max(
                Pos2::new(x.max(inner.left()), inner.top()),
                Pos2::new((x + span).min(inner.right()), inner.bottom()),
            );
            if block.width() > 0.0 {
                b.rrect(block, s.r(1.5), Fill::Solid(fill));
            }
            phase.to_string()
        }
    };
    let _ = text::paint(
        b,
        r,
        &Text::new(&label, Role::Value, s.t.text).align(Align2::CENTER_CENTER),
        s.tk(),
    );
}
