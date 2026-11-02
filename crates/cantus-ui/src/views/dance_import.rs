//! The import card (round 35, ADR 0089 §8): the character dancing to the song's tempo,
//! what the file is and holds, its licence (stated, or chosen here), what was ignored, and
//! the mapping of our sixteen moves onto its animations — each a list, its beats a
//! segmented choice — then *Use* (or, editing, *Save*). All of it the kit.

use super::{Card, Failed};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::glyph::Glyph;
use crate::kit::menu::{Check, Item};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::widgets::{menu_field, menu_list, segmented};
use crate::kit::{with_brush, Fill, Style};
use crate::views::interop::form;
use crate::views::workspace_settings as ws;
use crate::views::workspace_widgets as w;
use cantus_dance::import::{package, Filter};
use cantus_dance::render::{Background, Stage};
use cantus_dance::{pose_at, Character, DanceFrame, Licence, Move};
use egui::{pos2, vec2, Align2, Context, Id, Rect, Response, Sense, Ui, Vec2};

/// The card's size at 100 %.
const SIZE: Vec2 = Vec2::new(860.0, 620.0);
/// The left column (preview, licence, facts).
const LEFT: f32 = 300.0;
const HEAD: f32 = 34.0;
const PAD: f32 = 18.0;
/// The beats a move may span.
const BEATS: [f32; 4] = [1.0, 2.0, 4.0, 8.0];

fn layer() -> egui::LayerId {
    egui::LayerId::new(egui::Order::Foreground, Id::new("cantus-dance-import"))
}

/// Where the card drew its controls last frame, by name (the tests click through it):
/// `use`, `cancel`, `choose`, `move#<i>` (a move's list), `beats#<i>`, `licence#<i>`,
/// `pixel`, `own_speed`, `list#<i>` (the open list's items).
type Geo = Vec<(String, Rect)>;

fn geo_id() -> Id {
    Id::new("cantus-dance-import-geo")
}

fn put(ui: &Ui, name: impl Into<String>, r: Rect) {
    let name = name.into();
    ui.ctx()
        .data_mut(|d| d.get_temp_mut_or_default::<Geo>(geo_id()).push((name, r)));
}

/// Where `name` was drawn in the card's last frame.
pub fn geo(ctx: &Context, name: &str) -> Option<Rect> {
    ctx.data(|d| {
        d.get_temp::<Geo>(geo_id())
            .and_then(|g| g.iter().find(|(n, _)| n == name).map(|(_, r)| *r))
    })
}

/// The card, or the words of a failure, while there is one. Once a frame.
pub fn show(app: &mut CantusApp, ctx: &Context) {
    ctx.data_mut(|d| d.remove::<Geo>(geo_id()));
    if app.dance_import.failed.is_some() {
        failed(app, ctx);
        return;
    }
    if app.dance_import.card.is_none() {
        return;
    }
    let list_open = egui::Popup::is_any_open(ctx);
    let typing = ctx.egui_wants_keyboard_input() || crate::kit::field::active(ctx);
    let s = Style::current();
    let k = s.k;
    let screen = ctx.content_rect();
    veil(ctx, screen, &s);
    let size = (SIZE * k).min(screen.size() - Vec2::splat(24.0));
    let rect = Rect::from_center_size(screen.center(), size);
    let tempo = app.project().tempo_bpm;
    let mut act = None;
    egui::Area::new(layer().id)
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            ui.set_min_size(size);
            ws::frame(ui, rect, &s);
            let Some(card) = app.dance_import.card.as_mut() else {
                return;
            };
            let head = Rect::from_min_size(rect.min, vec2(rect.width(), HEAD * k));
            let title_key = if card.edit.is_some() {
                "dance_import.title_edit"
            } else {
                "dance_import.title"
            };
            if ws::title(ui, head, &s, Glyph::StarFilled, title_key) {
                act = Some(false);
            }
            let foot = Rect::from_min_max(
                pos2(rect.left() + PAD * k, rect.bottom() - 50.0 * k),
                pos2(rect.right() - PAD * k, rect.bottom() - 14.0 * k),
            );
            let left = Rect::from_min_max(
                pos2(rect.left() + PAD * k, head.bottom() + 12.0 * k),
                pos2(
                    rect.left() + (PAD + LEFT).min(size.x / k * 0.42) * k,
                    foot.top() - 10.0 * k,
                ),
            );
            let right = Rect::from_min_max(
                pos2(left.right() + 22.0 * k, left.top()),
                pos2(rect.right() - PAD * k, left.bottom()),
            );
            with_brush(ui.painter(), |b| {
                let x = b.snap(left.right() + 11.0 * k);
                b.vline(
                    x,
                    left.top(),
                    left.bottom(),
                    b.px(),
                    crate::kit::color::shade(s.t.panel, -0.12),
                );
            });
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(left));
            egui::ScrollArea::vertical()
                .id_salt("cantus-dance-import-left")
                .auto_shrink([false, false])
                .max_height(left.height())
                .show(&mut child, |ui| {
                    ui.set_width(left.width() - 10.0 * k);
                    ui.spacing_mut().item_spacing = vec2(6.0 * k, 4.0 * k);
                    preview(ui, card, tempo, &s);
                    about(ui, card, &s);
                });
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(right));
            card.showing = None;
            egui::ScrollArea::vertical()
                .id_salt("cantus-dance-import-moves")
                .auto_shrink([false, false])
                .max_height(right.height())
                .show(&mut child, |ui| {
                    ui.set_width(right.width() - 12.0 * k);
                    ui.spacing_mut().item_spacing = vec2(8.0 * k, 5.0 * k);
                    moves(ui, card, &s);
                    playback(ui, card, &s);
                });
            if let Some(a) = footer(ui, card, foot, &s) {
                act = Some(a);
            }
        });
    ctx.move_to_top(layer());
    ctx.request_repaint();
    let escape = !list_open && !typing && ctx.input(|i| i.key_pressed(egui::Key::Escape));
    let enter = !list_open && !typing && ctx.input(|i| i.key_pressed(egui::Key::Enter));
    if enter {
        act = Some(true);
    } else if escape {
        act = Some(false);
    }
    match act {
        Some(true) => {
            super::use_card(app);
        }
        Some(false) => super::close(app),
        None => {}
    }
}

fn veil(ctx: &Context, screen: Rect, s: &Style) {
    egui::Area::new(Id::new("cantus-dance-import-veil"))
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
}

/// A line of words.
fn line(ui: &mut Ui, words: &str, role: Role, color: egui::Color32, h: f32) -> Rect {
    let s = Style::current();
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), h * s.k), Sense::hover());
    with_brush(ui.painter(), |b| {
        let t = Text::new(words, role, color)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, r, &t, s.tk());
    });
    r
}

/// Words that wrap over as many lines as they need.
fn prose(ui: &mut Ui, words: &str, color: egui::Color32) {
    let w = ui.available_width();
    ws::wrapped(ui, words, Role::Label, color, w);
}

/// The character dancing: the move under the pointer, else its routine, at the song's
/// tempo on the wall clock — on a checker, so its transparency shows.
fn preview(ui: &mut Ui, card: &mut Card, tempo: f64, s: &Style) {
    let k = s.k;
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, w.min(300.0 * k)), Sense::hover());
    let ppp = ui.ctx().pixels_per_point();
    let px = (r.width() * ppp).round().max(1.0) as u32;
    let py = (r.height() * ppp).round().max(1.0) as u32;
    let seconds = card.opened.elapsed().as_secs_f64();
    let beats = seconds * tempo.clamp(20.0, 400.0) / 60.0;
    let mut f = DanceFrame {
        beats,
        seconds,
        tempo_bpm: tempo as f32,
        playing: true,
        bar: (beats / 4.0).floor() as i32,
        bar_start_beats: (beats / 4.0).floor() * 4.0,
        ..DanceFrame::default()
    };
    let set = |f: &mut DanceFrame, key: &str, v: f32| {
        if let Some(i) = cantus_core::inst::dancer::KEYS
            .iter()
            .position(|k| *k == key)
        {
            f.values[i] = v;
        }
    };
    set(&mut f, "mood", 0.0);
    if let Some(m) = card.showing {
        set(&mut f, "move", (m.index() + 1) as f32);
    }
    let mut stage = Stage::new(px, py, Background::Transparent);
    stage.standard_height = 0.7;
    let ch: &dyn Character = &card.preview;
    let mut pose = pose_at(&f, ch, seconds);
    pose.y = 0.92;
    let tex = card.texture.draw(ui.ctx(), &stage, &[(ch, pose)]);
    with_brush(ui.painter(), |b| {
        b.rrect(r, s.r(4.0), Fill::Solid(s.t.ink));
        // A soft checker under the stage: transparency shows as such.
        let cell = 12.0 * k;
        let mut y = r.top();
        let mut row = 0;
        while y < r.bottom() {
            let mut x = r.left() + if row % 2 == 0 { 0.0 } else { cell };
            while x < r.right() {
                let c = Rect::from_min_max(
                    pos2(x, y),
                    pos2((x + cell).min(r.right()), (y + cell).min(r.bottom())),
                );
                b.rect(c, Fill::Solid(crate::kit::color::with_alpha(s.t.well, 0.6)));
                x += cell * 2.0;
            }
            y += cell;
            row += 1;
        }
        if let Some(id) = tex {
            b.foreign(id, r);
        }
        b.rrect_stroke(
            r,
            s.r(4.0),
            b.px(),
            crate::kit::color::shade(s.t.panel, -0.2),
        );
    });
    put(ui, "preview", r);
    let showing = match card.showing {
        Some(m) => t(m.label_key()),
        None => t("dance_import.preview.routine"),
    };
    line(ui, &showing, Role::Micro, s.t.text_dim, 16.0);
}

/// What it is: name, format and author, its licence (stated, or chosen here), what it
/// holds, what it costs to draw, what was ignored.
fn about(ui: &mut Ui, card: &mut Card, s: &Style) {
    let k = s.k;
    ui.add_space(4.0 * k);
    line(ui, &card.name, Role::Title, s.t.text, 24.0);
    let by = if card.author.is_empty() {
        tf("dance_import.format", &[("format", &card.format)])
    } else {
        tf(
            "dance_import.format_by",
            &[("format", &card.format), ("author", &card.author)],
        )
    };
    line(ui, &by, Role::Label, s.t.text_dim, 18.0);
    form::band(ui, "dance_import.band.licence");
    if let Licence::Stated { text, source } = &card.stated {
        let words = if source == package::CHOSEN {
            text.clone()
        } else {
            tf(
                "dance_import.licence.stated",
                &[("licence", text), ("source", source)],
            )
        };
        prose(ui, &words, s.t.text);
    } else {
        prose(ui, &t("dance_import.licence.choose"), s.t.text_dim);
        let mut chosen = card.choice;
        for (i, (_, key)) in package::CHOICES.iter().enumerate() {
            let r = ui.vertical(|ui| form::choice(ui, chosen == Some(i), true, &t(key)));
            put(ui, format!("licence#{i}"), r.response.rect);
            if r.inner {
                chosen = Some(i);
            }
        }
        let r = ui.vertical(|ui| {
            form::choice(
                ui,
                chosen.is_none(),
                true,
                &t("dance_import.licence.unknown"),
            )
        });
        put(ui, "licence#unknown", r.response.rect);
        if r.inner {
            chosen = None;
        }
        card.choice = chosen;
        if card
            .choice
            .and_then(|i| package::CHOICES.get(i))
            .is_some_and(|c| c.0 == "other")
        {
            let w = ui.available_width() / k;
            let _ = w::text_field(
                ui,
                Id::new("cantus-dance-import-licence-other"),
                &mut card.other,
                "dance_import.licence.other_hint",
                w,
            );
        }
    }
    for key in package::licence_asks(&card.licence()) {
        let warn = key == "dance_import.asks.non_commercial" || key == "dance_import.asks.unknown";
        prose(ui, &t(key), if warn { s.t.danger } else { s.t.text_dim });
    }
    form::band(ui, "dance_import.band.found");
    let moves = card.clips.len();
    prose(
        ui,
        &tf("dance_import.fact.animations", &[("count", &moves)]),
        s.t.text,
    );
    for (key, value) in &card.facts {
        prose(ui, &tf(key, &[("value", value)]), s.t.text_dim);
    }
    let moods: Vec<String> = cantus_dance::Mood::ALL
        .iter()
        .filter(|m| card.settings.mapping.mood_clip(**m, moves).is_some())
        .map(|m| t(m.label_key()))
        .collect();
    if moods.is_empty() {
        prose(ui, &t("dance_import.fact.no_moods"), s.t.text_dim);
    } else {
        prose(
            ui,
            &tf("dance_import.fact.moods", &[("moods", &moods.join(", "))]),
            s.t.text_dim,
        );
    }
    // What it costs: measured once the card is up (a few frames of its routine).
    match card.cost_ms {
        Some(ms) => {
            let words = tf(
                "dance_import.fact.cost",
                &[("ms", &format!("{ms:.2}")), ("budget", &"1.5")],
            );
            prose(ui, &words, if ms > 1.5 { s.t.danger } else { s.t.text_dim });
            if ms > 1.5 {
                prose(ui, &t("dance_import.fact.heavy"), s.t.text_dim);
            }
        }
        None => prose(ui, &t("dance_import.fact.measuring"), s.t.text_dim),
    }
    if !card.notes.is_empty() {
        form::band(ui, "dance_import.band.ignored");
        for n in &card.notes {
            prose(ui, &tf(n.key, &[("arg", &n.arg)]), s.t.text_dim);
        }
    }
}

/// Our sixteen moves, each given one of the file's animations and its beats.
fn moves(ui: &mut Ui, card: &mut Card, s: &Style) {
    let k = s.k;
    form::band(ui, "dance_import.band.moves");
    prose(ui, &t("dance_import.moves.lead"), s.t.text_dim);
    let n = card.clips.len();
    let mut settings = card.settings.clone();
    let mut changed = false;
    let label_w = 120.0;
    let beats_w = 150.0;
    for m in Move::ALL {
        let i = m.index();
        let row = ui.horizontal(|ui| {
            w::row_label(ui, m.label_key(), label_w, 24.0);
            ui.add_space(6.0 * k);
            let field_w = (ui.available_width() / k - beats_w - 14.0).max(120.0);
            let value = match settings.mapping.moves.get(i).copied().flatten() {
                Some(c) => {
                    let name = card.clips.get(c).map(|c| c.name.as_str()).unwrap_or("");
                    if settings.mapping.guessed.get(i).copied().unwrap_or(false) {
                        tf("dance_import.moves.guessed", &[("name", &name)])
                    } else {
                        name.to_string()
                    }
                }
                None => {
                    let falls = settings
                        .mapping
                        .clip_for(m, n)
                        .and_then(|c| card.clips.get(c))
                        .map(|c| c.name.clone())
                        .unwrap_or_default();
                    tf("dance_import.moves.fallback", &[("name", &falls)])
                }
            };
            let (resp, _) = menu_field(ui, &value, false, false, m.label_key(), field_w);
            put(ui, format!("move#{i}"), resp.rect);
            let mut options: Vec<(Option<usize>, String)> =
                vec![(None, t("dance_import.moves.none"))];
            options.extend(card.clips.iter().enumerate().map(|(c, clip)| {
                (
                    Some(c),
                    tf(
                        "dance_import.moves.clip",
                        &[
                            ("name", &clip.name),
                            ("seconds", &format!("{:.2}", clip.seconds)),
                        ],
                    ),
                )
            }));
            let current = settings.mapping.moves.get(i).copied().flatten();
            if let Some(v) = pick(&resp, &options, current) {
                if let Some(slot) = settings.mapping.moves.get_mut(i) {
                    *slot = v;
                }
                if let Some(g) = settings.mapping.guessed.get_mut(i) {
                    *g = false;
                }
                if let Some(c) = v.and_then(|c| card.clips.get(c)) {
                    if let Some(b) = settings.mapping.beats.get_mut(i) {
                        *b = cantus_dance::import::mapping::natural_beats(c.seconds);
                    }
                }
                changed = true;
            }
            ui.add_space(6.0 * k);
            let at = BEATS
                .iter()
                .position(|b| (b - settings.mapping.beats_of(m)).abs() < 1e-3)
                .unwrap_or(0);
            let labels = [Src::Str("1"), Src::Str("2"), Src::Str("4"), Src::Str("8")];
            let mut sel = at;
            let r = segmented(ui, &mut sel, &labels, "dance_import.moves.beats", beats_w);
            put(ui, format!("beats#{i}"), r.rect);
            if sel != at {
                if let Some(b) = settings.mapping.beats.get_mut(i) {
                    *b = BEATS[sel];
                }
                changed = true;
            }
        });
        if row.response.contains_pointer() || ui.rect_contains_pointer(row.response.rect) {
            card.showing = Some(m);
        }
    }
    if changed {
        card.set_settings(settings);
    }
}

/// How it plays: its pixels sharp or smooth, its own pace or the song's.
fn playback(ui: &mut Ui, card: &mut Card, s: &Style) {
    let _ = s;
    form::band(ui, "dance_import.band.playback");
    let mut settings = card.settings.clone();
    let sharp = settings.filter == Filter::Nearest;
    let r = ui.vertical(|ui| form::choice(ui, sharp, true, &t("dance_import.playback.pixel")));
    put(ui, "pixel", r.response.rect);
    let mut changed = false;
    if r.inner {
        settings.filter = if sharp {
            Filter::Bilinear
        } else {
            Filter::Nearest
        };
        changed = true;
    }
    let r = ui.vertical(|ui| {
        form::choice(
            ui,
            settings.own_speed,
            true,
            &t("dance_import.playback.own_speed"),
        )
    });
    put(ui, "own_speed", r.response.rect);
    if r.inner {
        settings.own_speed = !settings.own_speed;
        changed = true;
    }
    prose(
        ui,
        &t("dance_import.playback.lead"),
        Style::current().t.text_dim,
    );
    if changed {
        card.set_settings(settings);
    }
}

/// A list opened from a field; the item clicked.
fn pick<T: Copy + PartialEq>(field: &Response, options: &[(T, String)], current: T) -> Option<T> {
    let mut chosen = None;
    egui::Popup::menu(field)
        .frame(egui::Frame::NONE)
        .show(|ui| {
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
            let width = field.rect.width().max(180.0);
            let (resp, clicks) = menu_list(ui, &items, width);
            let k = Style::current().d(1.0);
            for i in 0..items.len() {
                let r = crate::kit::menu::row(resp.rect, &items, k, i);
                put(ui, format!("list#{i}"), r);
            }
            if let Some(i) = clicks.clicked {
                chosen = options.get(i).map(|o| o.0);
                ui.close();
            }
        });
    chosen
}

/// The footer: what *Use* does, and the buttons. `Some(true)` to use, `Some(false)` to
/// close.
fn footer(ui: &mut Ui, card: &Card, rect: Rect, s: &Style) -> Option<bool> {
    let k = s.k;
    let mut act = None;
    let words = if card.busy() {
        t("dance_import.foot.copying")
    } else if card.edit.is_some() {
        t("dance_import.foot.edit")
    } else {
        t("dance_import.foot.use")
    };
    let note = Rect::from_min_max(rect.min, pos2(rect.right() - 260.0 * k, rect.bottom()));
    with_brush(ui.painter(), |b| {
        let tx = Text::new(&words, Role::Label, s.t.text_dim)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, note, &tx, s.tk());
    });
    let buttons = Rect::from_min_max(pos2(rect.right() - 260.0 * k, rect.top()), rect.max);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(buttons));
    form::buttons(&mut child, |ui| {
        let label = if card.edit.is_some() {
            t("dance_import.save")
        } else {
            t("dance_import.use")
        };
        let before = ui.cursor().min;
        if form::button(ui, &label, true, !card.busy()) {
            act = Some(true);
        }
        let r = Rect::from_two_pos(before, ui.cursor().min + vec2(0.0, 26.0 * k));
        put(ui, "use", r);
        let before = ui.cursor().min;
        if form::button(ui, &t("dance_import.cancel"), false, true) {
            act = Some(false);
        }
        put(
            ui,
            "cancel",
            Rect::from_two_pos(before, ui.cursor().min + vec2(0.0, 26.0 * k)),
        );
    });
    act
}

/// A file that could not be read: what went wrong, in words, and what to do.
fn failed(app: &mut CantusApp, ctx: &Context) {
    let Some(f) = app.dance_import.failed.as_ref() else {
        return;
    };
    let s = Style::current();
    let k = s.k;
    let screen = ctx.content_rect();
    veil(ctx, screen, &s);
    let size = vec2((480.0 * k).min(screen.width() - 24.0), 160.0 * k);
    let at = pos2(
        screen.center().x - size.x * 0.5,
        screen.top() + screen.height() * 0.25,
    );
    let rect = Rect::from_min_size(at, size);
    let (track, file) = (f.track, f.path.clone());
    let words = failed_words(f);
    let mut choose = false;
    let mut close = false;
    egui::Area::new(Id::new("cantus-dance-import-failed"))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .show(ctx, |ui| {
            ui.set_min_size(size);
            ws::frame(ui, rect, &s);
            let head = Rect::from_min_size(rect.min, vec2(rect.width(), HEAD * k));
            if ws::title(ui, head, &s, Glyph::Warning, "dance_import.failed.title") {
                close = true;
            }
            let inner = Rect::from_min_max(
                pos2(rect.left() + PAD * k, head.bottom() + 10.0 * k),
                pos2(rect.right() - PAD * k, rect.bottom() - 14.0 * k),
            );
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(inner)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            let name = file
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            line(&mut child, &name, Role::Control, s.t.text, 20.0);
            prose(&mut child, &words, s.t.text_dim);
            form::buttons(&mut child, |ui| {
                let before = ui.cursor().min;
                if form::button(ui, &t("dance_import.failed.choose"), true, true) {
                    choose = true;
                }
                put(
                    ui,
                    "choose",
                    Rect::from_two_pos(before, ui.cursor().min + vec2(0.0, 26.0 * k)),
                );
                if form::button(ui, &t("dance_import.cancel"), false, true) {
                    close = true;
                }
            });
        });
    ctx.move_to_top(egui::LayerId::new(
        egui::Order::Foreground,
        Id::new("cantus-dance-import-failed"),
    ));
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        close = true;
    }
    if close || choose {
        app.dance_import.failed = None;
    }
    if choose {
        super::open(app, track);
    }
}

/// The words of a failure: what kind, and the parser's detail.
pub fn failed_words(f: &Failed) -> String {
    let lead = t(f.key);
    if f.detail.is_empty() {
        lead
    } else {
        tf(
            "dance_import.failed.detail",
            &[("lead", &lead), ("detail", &f.detail)],
        )
    }
}
