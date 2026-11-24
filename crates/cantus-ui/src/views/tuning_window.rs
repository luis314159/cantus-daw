//! The Tuning window's frame (ADR 0024: the kit's panel over a dimmed room, as the export
//! dialog): its title row, the controls down the left (`tuning_rows.rs`, scrolling when
//! taller than the window), the circle of fifths and the degrees on the right, and a footer
//! with the summary and the keys. Every control publishes where it was drawn (`geo`), so
//! the tests click through it as a musician does. r42:tuning-face

use super::{circle, Target};
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::glyph::Glyph;
use crate::kit::menu::{Check, Item};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{band, with_brush, Fill, State, Style};
use crate::views::audio_parts as parts;
use egui::{Align2, Id, Pos2, Rect, Response, Sense, Ui, Vec2};

/// The window at 100 %.
pub const SIZE: Vec2 = Vec2::new(1000.0, 740.0);
/// The controls' column beside the pictures.
const LEFT: f32 = 500.0;
/// The pictures' side beside the controls: what their legend and the degrees' headings
/// need whole in either language (measured: the Spanish legend and "Tercera, batidos").
const RIGHT: f32 = 440.0;

/// How the window lays out its two halves.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Layout {
    /// The controls at the left, the pictures at the right.
    Beside,
    /// One column that scrolls: the controls, then the circle, its legend and the degrees
    /// under them at the column's width (a narrow screen, or 200 % on a small one — r42
    /// fix-ui, review ui#18: side by side, the pictures' side shrank to 136 px and below
    /// ~560 points inverted).
    Stacked,
}

/// The controls' rect, the pictures' and the layout in the window's `body` (the room
/// between its title and its footer) at scale `k`: side by side while both keep their
/// width ([`LEFT`] and [`RIGHT`]) — the full window — else one scrolling column, the
/// controls' rect the whole body and the pictures' empty (they are drawn inside it).
pub fn split(body: Rect, k: f32) -> (Rect, Rect, Layout) {
    let gap = 24.0 * k;
    if body.width() + 0.5 >= (LEFT + RIGHT) * k + gap {
        let left = Rect::from_min_size(body.min, Vec2::new(LEFT * k, body.height()));
        let right = Rect::from_min_max(Pos2::new(left.right() + gap, body.top()), body.max);
        (left, right, Layout::Beside)
    } else {
        let none = Rect::from_min_size(Pos2::new(body.left(), body.bottom()), Vec2::ZERO);
        (body, none, Layout::Stacked)
    }
}

/// How tall the pictures are at `width` when drawn in the scrolling column: the circle (a
/// 12-note tuning), its legend, the degrees whole.
fn pictures_height(app: &CantusApp, target: Target, width: f32, k: f32) -> f32 {
    let tu = super::effective(&super::tuning_of(app, target), app.project().key);
    let rows = match &tu.system {
        cantus_core::tuning::TuningSystem::Scale(sc)
            if !(sc.degrees.len() == 12 && tu.map.is_none()) =>
        {
            return 24.0 * k + (sc.degrees.len() as f32 + 1.0) * 18.0 * k + 8.0 * k;
        }
        _ => 12.0,
    };
    let circle = width.min(330.0 * k) + 6.0 * k + 36.0 * k + 8.0 * k + 24.0 * k;
    circle + 24.0 * k + rows * 18.0 * k + 8.0 * k
}
/// A row's caption column.
pub(super) const CAPTION: f32 = 124.0;

/// The window, when open.
pub fn show(app: &mut CantusApp, ctx: &egui::Context) {
    let Some(target) = app.tuning.open else {
        return;
    };
    // A track deleted while its tuning was open: the window goes.
    if let Target::Track(id) = target {
        if app.project().track(id).is_none() {
            super::close(app);
            return;
        }
    }
    ctx.data_mut(|d| d.remove::<Geo>(geo_id()));
    if std::mem::take(&mut app.tuning.fresh) {
        egui::Popup::close_all(ctx);
    }
    let list_open = egui::Popup::is_any_open(ctx);
    let typing = ctx.egui_wants_keyboard_input() || crate::kit::field::active(ctx);
    let s = Style::current();
    let k = s.k;
    let screen = ctx.content_rect();
    egui::Area::new(Id::new("cantus-tuning-veil"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (r, _) = ui.allocate_exact_size(screen.size(), Sense::click());
            with_brush(ui.painter(), |b| {
                b.rect(
                    r,
                    Fill::Solid(crate::views::workspace_settings::scrim_fill(&s)),
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
            let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 44.0 * k));
            let title = match target {
                Target::Project => "tuning.title",
                Target::Track(_) => "tuning.title_track",
            };
            if crate::views::workspace_settings::title(ui, head, &s, Glyph::Key, title) {
                close = true;
            }
            put(
                ui,
                "close",
                Rect::from_min_max(Pos2::new(head.right() - 44.0 * k, head.top()), head.max),
            );
            let foot = Rect::from_min_max(
                Pos2::new(rect.left() + 18.0 * k, rect.bottom() - 52.0 * k),
                Pos2::new(rect.right() - 18.0 * k, rect.bottom() - 14.0 * k),
            );
            let body = Rect::from_min_max(
                Pos2::new(rect.left() + 18.0 * k, head.bottom() + 8.0 * k),
                Pos2::new(rect.right() - 18.0 * k, foot.top() - 10.0 * k),
            );
            let (left, right, side) = split(body, k);
            if side == Layout::Beside {
                // Side by side: the pictures' ground from the column to the edge.
                with_brush(ui.painter(), |b| {
                    let side = Rect::from_min_max(
                        Pos2::new(right.left() - 12.0 * k, head.bottom()),
                        Pos2::new(rect.right(), foot.top() - 4.0 * k),
                    );
                    crate::views::workspace_settings::side_ground(b, side, &s);
                });
            }
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(left));
            egui::ScrollArea::vertical()
                .id_salt("cantus-tuning-body")
                .auto_shrink([false, false])
                .max_height(left.height())
                .show(&mut child, |ui| {
                    let w = left.width() - 12.0 * k;
                    ui.set_width(w);
                    ui.spacing_mut().item_spacing = Vec2::new(8.0 * k, 6.0 * k);
                    super::rows::body(app, ui, target);
                    if side == Layout::Stacked {
                        // The pictures under the controls, in the same scroll.
                        ui.add_space(12.0 * k);
                        let h = pictures_height(app, target, w, k);
                        let (r, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
                        pictures(app, ui, r, target);
                    }
                });
            if side == Layout::Beside {
                pictures(app, ui, right, target);
            }
            if footer(app, ui, foot, target) {
                close = true;
            }
        });
    ctx.move_to_top(layer());
    let escape = !list_open && !typing && ctx.input(|i| i.key_pressed(egui::Key::Escape));
    if close || escape {
        super::close(app);
    }
}

/// The window's layer.
pub(super) fn layer() -> egui::LayerId {
    egui::LayerId::new(egui::Order::Foreground, Id::new("cantus-tuning"))
}

/// Where the window drew its controls this frame, by name (`system#2`, `temperament`,
/// `key`, `follow`, `a4`, `preset#0`, `import`, `export`, `master`, `audition`, `reset`,
/// `close`, `track#1`, a list's `list#i`, the circle's notes `note#pc`).
type Geo = Vec<(String, Rect)>;

fn geo_id() -> Id {
    Id::new("cantus-tuning-geo")
}

/// Where `name` was drawn in the window's last frame.
pub fn geo(ctx: &egui::Context, name: &str) -> Option<Rect> {
    ctx.data(|d| d.get_temp::<Geo>(geo_id()))
        .unwrap_or_default()
        .into_iter()
        .rev()
        .find(|(n, _)| n == name)
        .map(|(_, r)| r)
}

/// Every name drawn in the window's last frame.
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

/// A band naming the rows under it (as the export dialog's).
pub(super) fn band_row(ui: &mut Ui, key: &str) {
    let s = Style::current();
    ui.add_space(4.0 * s.k);
    let (r, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), s.d(band::BAND)),
        Sense::hover(),
    );
    with_brush(ui.painter(), |b| {
        if s.t.system.recessed() {
            let ink = crate::kit::color::ensure_contrast_all(
                s.t.placeholder,
                &[s.t.toolbar, s.t.panel],
                4.5,
            );
            let _ = text::paint(
                b,
                r,
                &Text::key(key, Role::Section, ink).align(Align2::LEFT_BOTTOM),
                s.tk(),
            );
        } else {
            band::section(b, r, &s, State::REST, Src::Key(key), None, false)
        }
    });
}

/// A caption on the left and the row's control to its right.
pub(super) fn row<R>(ui: &mut Ui, key: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    let s = Style::current();
    ui.horizontal(|ui| {
        crate::views::workspace_widgets::row_label(ui, key, CAPTION, 24.0);
        ui.add_space(6.0 * s.k);
        add(ui)
    })
    .inner
}

/// A list of choices opened from a menu field; returns the one clicked.
pub(super) fn pick<T: Copy + PartialEq>(
    field: &Response,
    options: &[(T, String)],
    current: Option<T>,
) -> Option<T> {
    let mut chosen = None;
    egui::Popup::menu(field)
        .frame(egui::Frame::NONE)
        .show(|ui| {
            ui.ctx().set_sublayer(layer(), ui.layer_id());
            let items: Vec<Item<'_>> = options
                .iter()
                .map(|(v, label)| Item::Action {
                    label: Src::Str(label),
                    check: Check::Radio(Some(*v) == current),
                    shortcut: None,
                    submenu: false,
                    enabled: true,
                })
                .collect();
            let width = field.rect.width().max(180.0);
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

/// The right side: the circle of fifths, its legend and the degrees — or, for a scale that
/// is not 12 to the octave, its degrees alone.
fn pictures(app: &CantusApp, ui: &mut Ui, r: Rect, target: Target) {
    let s = Style::current();
    let k = s.k;
    let tu = super::effective(&super::tuning_of(app, target), app.project().key);
    let table = super::table_of(app, target);
    let scale = match &tu.system {
        cantus_core::tuning::TuningSystem::Scale(sc)
            if !(sc.degrees.len() == 12 && tu.map.is_none()) =>
        {
            Some(sc.clone())
        }
        _ => None,
    };
    let twelve = scale.is_none() && circle::twelve_to_the_octave(&table);
    let tonic = match tu.system {
        cantus_core::tuning::TuningSystem::Temperament { key, .. } => key,
        _ => app.project().key.map_or(0, |k| k.root),
    };
    // Stacked under the controls the room is wide and short: the circle and its legend at
    // the left, the degrees beside them (r42 fix-ui, review ui#18).
    let wide = twelve && r.width() > r.height() * 1.3;
    let (pic, r) = if wide {
        let cw = (r.height() - 50.0 * k).min(r.width() * 0.45).max(160.0 * k);
        (
            Rect::from_min_size(r.min, Vec2::new(cw, r.height())),
            Rect::from_min_max(Pos2::new(r.left() + cw + 16.0 * k, r.top()), r.max),
        )
    } else {
        (r, r)
    };
    let mut y = r.top();
    if twelve {
        let side = (pic.width().min(330.0 * k)).min(if wide {
            pic.height() - 50.0 * k
        } else {
            pic.height() * 0.52
        });
        let cr = Rect::from_center_size(
            Pos2::new(pic.center().x, pic.top() + side * 0.5),
            Vec2::splat(side),
        );
        let centres = with_brush(ui.painter(), |b| circle::paint(b, cr, &s, &table, tonic));
        // The circle's centre: the tonic it is tuned to, and the diapason under it.
        let mid = Rect::from_center_size(cr.center(), Vec2::new(side * 0.4, 52.0 * k));
        let (top, bottom) = mid.split_top_bottom_at_fraction(0.6);
        with_brush(ui.painter(), |b| {
            let _ = text::paint(
                b,
                top,
                &Text::new(super::words::key_name(tonic), Role::Hero, s.t.text)
                    .align(Align2::CENTER_BOTTOM),
                s.tk(),
            );
            let _ = text::paint(
                b,
                bottom,
                &Text::new(&super::words::a4_short(tu.a4_hz), Role::Label, s.t.text_dim)
                    .align(Align2::CENTER_TOP),
                s.tk(),
            );
        });
        for (pc, c) in centres.iter().enumerate() {
            let hit = Rect::from_center_size(*c, Vec2::splat(26.0 * k));
            put(ui, format!("note#{pc}"), hit);
            let resp = ui.interact(hit, Id::new(("tuning-note", pc)), Sense::hover());
            if resp.hovered() {
                crate::kit::hint::set_text(
                    ui.ctx(),
                    &circle::hover_words(&table, pc as u8),
                    "",
                    crate::kit::hint::Caps::NONE,
                );
            }
        }
        let ly = cr.bottom() + 6.0 * k;
        let lr = Rect::from_min_size(Pos2::new(pic.left(), ly), Vec2::new(pic.width(), 36.0 * k));
        with_brush(ui.painter(), |b| circle::legend(b, lr, &s));
        if !wide {
            y = lr.bottom() + 8.0 * k;
        }
    }
    let (rows, heads): (Vec<circle::Row>, [&str; 5]) = match &scale {
        Some(sc) => (
            circle::scale_rows(sc),
            [
                "tuning.table.degree",
                "tuning.table.cents",
                "tuning.table.from_12",
                "tuning.table.written",
                "",
            ],
        ),
        None => (
            circle::twelve_rows(&table, tu.a4_hz),
            [
                "tuning.table.note",
                "tuning.table.cents",
                "tuning.table.from_equal",
                "tuning.table.fifth",
                "tuning.table.third",
            ],
        ),
    };
    if !twelve && scale.is_none() {
        // A keyboard map leaving keys of the middle octave unmapped: the circle cannot say.
        let nr = Rect::from_min_size(Pos2::new(r.left(), y), Vec2::new(r.width(), 20.0 * k));
        with_brush(ui.painter(), |b| {
            let _ = text::paint(
                b,
                nr,
                &Text::key("tuning.circle.unmapped", Role::Label, s.t.text_dim)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Abbreviate),
                s.tk(),
            );
        });
        y = nr.bottom() + 4.0 * k;
    }
    let tr = Rect::from_min_max(Pos2::new(r.left(), y), r.max);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(tr));
    degree_table(&mut child, tr, &rows, &heads, scale.is_none());
}

/// The degrees as a table: a header, then one 18-pt row each, scrolling when longer.
fn degree_table(ui: &mut Ui, r: Rect, rows: &[circle::Row], heads: &[&str; 5], beats: bool) {
    let s = Style::current();
    let k = s.k;
    let cols: [f32; 5] = [0.12, 0.2, 0.2, 0.24, 0.24];
    let col_rect = |line: Rect, i: usize| {
        let x0 = line.left() + line.width() * cols[..i].iter().sum::<f32>();
        Rect::from_min_size(
            Pos2::new(x0, line.top()),
            Vec2::new(line.width() * cols[i] - 6.0 * k, line.height()),
        )
    };
    let head = Rect::from_min_size(r.min, Vec2::new(r.width(), 20.0 * k));
    with_brush(ui.painter(), |b| {
        for (i, key) in heads.iter().enumerate() {
            if key.is_empty() {
                continue;
            }
            let align = if i == 0 {
                Align2::LEFT_CENTER
            } else {
                Align2::RIGHT_CENTER
            };
            let mut hr = col_rect(head, i);
            // What the deviation is measured from, on the heading's hint.
            if i == 2 {
                let id = Id::new(("tuning-table-head", i));
                if ui.interact(hr, id, Sense::hover()).hovered() {
                    let why = if beats {
                        "tuning.table.from_equal_hint"
                    } else {
                        "tuning.table.from_12_hint"
                    };
                    crate::kit::hint::set_text(ui.ctx(), &t(why), "", crate::kit::hint::Caps::NONE);
                }
            }
            if beats && (i == 3 || i == 4) {
                hr.max.x -= 14.0 * k; // over the number, not its dot
            }
            let _ = text::paint(
                b,
                hr,
                &Text::key(key, Role::Label, s.t.text_dim)
                    .align(align)
                    .policy(Policy::Abbreviate),
                s.tk(),
            );
        }
        b.hline(head.left(), head.right(), head.bottom(), 1.0 * k, s.t.edge);
    });
    let body = Rect::from_min_max(Pos2::new(r.left(), head.bottom() + 2.0 * k), r.max);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
    egui::ScrollArea::vertical()
        .id_salt("cantus-tuning-degrees")
        .auto_shrink([false, false])
        .max_height(body.height())
        .show(&mut child, |ui| {
            let w = body.width() - 12.0 * k;
            ui.spacing_mut().item_spacing.y = 0.0;
            for (n, row) in rows.iter().enumerate() {
                let (line, _) = ui.allocate_exact_size(Vec2::new(w, 18.0 * k), Sense::hover());
                with_brush(ui.painter(), |b| {
                    if n % 2 == 1 {
                        b.rect(
                            line,
                            Fill::Solid(crate::kit::color::with_alpha(s.t.text, 0.04)),
                        );
                    }
                    let words = [
                        &row.name,
                        &row.cents,
                        &row.from_equal,
                        &row.fifth,
                        &row.third,
                    ];
                    for (i, w) in words.iter().enumerate() {
                        let cr = col_rect(line, i);
                        let align = if i == 0 {
                            Align2::LEFT_CENTER
                        } else {
                            Align2::RIGHT_CENTER
                        };
                        let ink = if i == 0 { s.t.text } else { s.t.readout };
                        // A beat rate's colour dot after its number, at the column's end.
                        let cr = if beats && (i == 3 || i == 4) {
                            let x = if i == 3 { row.fifth_x } else { row.third_x };
                            let dot = Pos2::new(cr.right() - 4.0 * k, cr.center().y);
                            b.circle(dot, 4.0 * k, Fill::Solid(circle::beat_colour(&s, x)));
                            Rect::from_min_max(
                                cr.min,
                                Pos2::new(cr.right() - 14.0 * k, cr.bottom()),
                            )
                        } else {
                            cr
                        };
                        let _ = text::paint(
                            b,
                            cr,
                            &Text::new(w, if i == 0 { Role::Body } else { Role::Value }, ink)
                                .align(align)
                                .policy(Policy::Elide(Elide::End)),
                            s.tk(),
                        );
                    }
                });
            }
        });
}

/// The footer: the summary at the left, *Equal temperament* and *Close* at the right.
/// Returns whether the window should close.
fn footer(app: &mut CantusApp, ui: &mut Ui, rect: Rect, target: Target) -> bool {
    let s = Style::current();
    let k = s.k;
    let reset_key = match target {
        Target::Project => "tuning.reset",
        Target::Track(_) => "tuning.reset_track",
    };
    // The buttons as wide as their words (a narrow window gives the summary the rest).
    let bw = |key: &str| {
        let words = t(key);
        let tx = Text::new(&words, Role::Control, s.t.text);
        (text::measure(ui.ctx(), &tx, s.tk()).x + 32.0 * k + 2.0).max(90.0 * k)
    };
    let room = bw("tuning.close") + bw(reset_key) + 8.0 * k + 8.0 * k;
    let buttons = Rect::from_min_max(
        Pos2::new(
            rect.right() - room.max(200.0 * k).min(330.0 * k),
            rect.top(),
        ),
        rect.max,
    );
    let status = Rect::from_min_max(
        rect.min,
        Pos2::new(buttons.left() - 16.0 * k, rect.bottom()),
    );
    let tu = super::tuning_of(app, target);
    let fits = |w: &str| {
        let tx = Text::new(w, Role::Body, s.t.text);
        text::measure(ui.ctx(), &tx, s.tk()).x <= status.width()
    };
    // Short of room, the default's short form; a tuning's parts go whole from the end.
    let words = if tu.is_default() {
        let long = t("tuning.summary.default");
        if fits(&long) {
            long
        } else {
            t("tuning.summary.default_short")
        }
    } else {
        let full = super::words::summary_of(&tu, app.project().key);
        let first = full.find(" \u{b7} ").unwrap_or(full.len());
        crate::views::transport::menus::fit_summary(&full, first, fits).to_string()
    };
    with_brush(ui.painter(), |b| {
        let _ = text::paint(
            b,
            status,
            &Text::new(&words, Role::Body, s.t.text)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
    });
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(buttons)
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    let b = parts::button(&mut child, &t("tuning.close"), true, true, 90.0);
    put(&child, "button.close", b.rect);
    let close = b.clicked();
    child.add_space(8.0 * k);
    let reset_enabled = match target {
        Target::Project => !tu.is_default(),
        Target::Track(_) => !super::track_mode(app, target).is_some_and(|m| m.is_follow()),
    };
    let b = parts::button(&mut child, &t(reset_key), false, reset_enabled, 90.0);
    put(&child, "reset", b.rect);
    if b.clicked() && reset_enabled {
        match target {
            Target::Project => {
                super::set(app, target, cantus_core::tuning::Tuning::default());
            }
            Target::Track(id) => {
                super::set_track_mode(app, id, cantus_core::tuning::TrackTuning::Follow);
            }
        }
    }
    close
}
