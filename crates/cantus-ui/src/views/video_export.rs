//! The video export dialog (round 35, ADR 0088), drawn with the kit like *Export audio*:
//! a floating panel over a dimmed room; on the left the **live preview** — the frame the
//! video will have at the scrubbed time, drawn by the same renderer at the preview's own
//! pixels, the title and background as the file will have them — with its scrub bar; on
//! the right the choices in banded rows; the footer says what it will take, then the
//! progress and its time left, and how it ended.

#[path = "video_export_rows.rs"]
mod rows;

use super::audio_parts as parts;
use super::export_dialog::progress;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::glyph::Glyph;
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{well, with_brush, Fill, State, Style};
use crate::video_export::{self as vx, job, VideoSettings};
use cantus_dance::render::Stage;
use cantus_video::dance::{Dancer, SongFrames};
use egui::{Align2, Id, Pos2, Rect, Sense, Ui, Vec2};

/// The window at 100 %.
pub const SIZE: Vec2 = Vec2::new(900.0, 720.0);
/// The preview's column.
const LEFT: f32 = 300.0;

/// What the dialog drew where this frame, by name (the tests click through it).
type Geo = Vec<(String, Rect)>;

fn geo_id() -> Id {
    Id::new("cantus-video-geo")
}

/// Where `name` was drawn in the dialog's last frame.
pub fn geo(ctx: &egui::Context, name: &str) -> Option<Rect> {
    ctx.data(|d| d.get_temp::<Geo>(geo_id()))
        .unwrap_or_default()
        .into_iter()
        .rev()
        .find(|(n, _)| n == name)
        .map(|(_, r)| r)
}

pub(crate) fn put(ui: &Ui, name: impl Into<String>, r: Rect) {
    let name = name.into();
    ui.ctx()
        .data_mut(|d| d.get_temp_mut_or_default::<Geo>(geo_id()).push((name, r)));
}

fn layer() -> egui::LayerId {
    egui::LayerId::new(egui::Order::Foreground, Id::new("cantus-video"))
}

/// The dialog, when open.
pub fn show(app: &mut CantusApp, ctx: &egui::Context) {
    if app.video.dialog.is_none() {
        return;
    }
    ctx.data_mut(|d| d.remove::<Geo>(geo_id()));
    if std::mem::take(&mut app.video.fresh) {
        egui::Popup::close_all(ctx);
    }
    let list_open = egui::Popup::is_any_open(ctx);
    let s = Style::current();
    let k = s.k;
    let screen = ctx.content_rect();
    let busy = app.video.busy();
    if busy || app.video.openh264_fetch.is_some() {
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }
    egui::Area::new(Id::new("cantus-video-veil"))
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
            if crate::kit::dialog::head(ui, head, &s, Glyph::Send, "video.title") {
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
            let mut d = app.video.dialog.clone().unwrap_or_default();
            preview(app, ui, left, &d);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(right));
            egui::ScrollArea::vertical()
                .id_salt("cantus-video-body")
                .auto_shrink([false, false])
                .max_height(right.height())
                .show(&mut child, |ui| {
                    ui.set_width(right.width() - 12.0 * k);
                    ui.spacing_mut().item_spacing = Vec2::new(8.0 * k, 6.0 * k);
                    ui.add_enabled_ui(!busy, |ui| rows::body(app, ui, &mut d));
                });
            app.video.dialog = Some(d);
            if footer(app, ui, foot) {
                close = true;
            }
        });
    ctx.move_to_top(layer());
    let typing = ctx.egui_wants_keyboard_input();
    let escape = !list_open && !typing && ctx.input(|i| i.key_pressed(egui::Key::Escape));
    let enter = !list_open && !typing && ctx.input(|i| i.key_pressed(egui::Key::Enter));
    if enter && !app.video.busy() {
        vx::choose_and_start(app);
    } else if (close || escape) && !app.video.busy() {
        vx::close(app);
    }
}

/// The video's length in seconds for the dialog's range (0 when there is none).
pub fn length(app: &CantusApp, d: &VideoSettings) -> f64 {
    vx::range(app, d.range).map_or(0.0, |(a, b)| {
        let map = app.project().tempo_map();
        map.seconds_at(b.0 as f64) - map.seconds_at(a.0 as f64)
    })
}

/// The preview at the scrubbed time, and the scrub bar under it.
fn preview(app: &mut CantusApp, ui: &mut Ui, area: Rect, d: &VideoSettings) {
    let s = Style::current();
    let k = s.k;
    super::export_dialog::band_row(
        &mut ui.new_child(egui::UiBuilder::new().max_rect(area)),
        "video.band.preview",
    );
    let top = area.top() + s.d(crate::kit::band::BAND) + 10.0 * k;
    let bar_h = 22.0 * k;
    let room = Rect::from_min_max(
        Pos2::new(area.left(), top),
        Pos2::new(area.right(), area.bottom() - bar_h - 34.0 * k),
    );
    let (w, h) = d.size();
    let scale = (room.width() / w as f32).min(room.height() / h as f32);
    let shown = Rect::from_center_size(
        Pos2::new(room.center().x, room.top() + h as f32 * scale / 2.0),
        Vec2::new(w as f32 * scale, h as f32 * scale),
    );
    let ppp = ui.ctx().pixels_per_point();
    let px = [
        (shown.width() * ppp).round().max(2.0) as u32,
        (shown.height() * ppp).round().max(2.0) as u32,
    ];
    let length = length(app, d);
    app.video.scrub = app.video.scrub.clamp(0.0, length.max(0.0));
    let (tex, state) = preview_texture(app, ui.ctx(), d, px, length);
    with_brush(ui.painter(), |b| {
        b.register(shown, "video-preview", &[crate::kit::Tok::Well], &[]);
        well::paint(b, shown.expand(2.0 * k), &s, State::REST, None);
        if d.transparent() {
            checker(b, shown, &s);
        }
        if let Some(tex) = tex {
            b.foreign(tex, shown);
        }
    });
    // A small chip while the moment's own render is on its way, or why it is not.
    let chip = match &state {
        vx::PreviewState::Rendering => Some((t("video.preview.rendering"), false)),
        vx::PreviewState::Failed(e) => Some((tf("video.preview.unavailable", &[("why", e)]), true)),
        vx::PreviewState::Ready(_) => None,
    };
    if let Some((words, warn)) = chip {
        let r = Rect::from_min_size(
            shown.left_top() + Vec2::splat(6.0 * k),
            Vec2::new((shown.width() - 12.0 * k).max(0.0), 20.0 * k),
        );
        with_brush(ui.painter(), |b| {
            let size = text::measure(ui.ctx(), &Text::new(&words, Role::Label, s.t.text), s.tk());
            let pill = Rect::from_min_size(
                r.min,
                Vec2::new((size.x + 12.0 * k).min(r.width()), r.height()),
            );
            b.rrect(
                pill,
                s.r(3.0),
                Fill::Solid(crate::kit::color::with_alpha(s.t.panel, 0.85)),
            );
            let _ = text::paint(
                b,
                pill,
                &Text::new(
                    &words,
                    Role::Label,
                    if warn { s.t.danger } else { s.t.text_dim },
                )
                .align(Align2::CENTER_CENTER)
                .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
        });
        put(ui, "preview.chip", r);
    }
    put(ui, "preview", shown);
    // The scrub bar.
    let bar = Rect::from_min_size(
        Pos2::new(area.left(), shown.bottom() + 12.0 * k),
        Vec2::new(area.width(), bar_h),
    );
    let resp = ui.interact(bar, Id::new("cantus-video-scrub"), Sense::click_and_drag());
    if let Some(p) = resp.interact_pointer_pos() {
        let f = ((p.x - bar.left()) / bar.width()).clamp(0.0, 1.0) as f64;
        app.video.scrub = f * length;
    }
    put(ui, "scrub", bar);
    let f = if length > 0.0 {
        (app.video.scrub / length) as f32
    } else {
        0.0
    };
    let label = format!("{} / {}", clock(app.video.scrub), clock(length));
    with_brush(ui.painter(), |b| {
        well::paint(b, bar, &s, State::of(&resp, false), None);
        let inner = bar.shrink(s.s(2.0));
        let fill = crate::kit::color::with_alpha(s.t.selection, 0.45);
        b.rrect(
            Rect::from_min_size(inner.min, Vec2::new(inner.width() * f, inner.height())),
            s.r(1.5),
            Fill::Solid(fill),
        );
        let _ = text::paint(
            b,
            bar,
            &Text::new(&label, Role::Value, s.t.text).align(Align2::CENTER_CENTER),
            s.tk(),
        );
    });
    let hint = Rect::from_min_size(
        Pos2::new(area.left(), bar.bottom() + 6.0 * k),
        Vec2::new(area.width(), 18.0 * k),
    );
    with_brush(ui.painter(), |b| {
        let _ = text::paint(
            b,
            hint,
            &Text::new(&t("video.preview.hint"), Role::Label, s.t.text_dim)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
    });
}

/// `m:ss.t`.
pub fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0);
    format!("{}:{:04.1}", (s / 60.0).floor() as u64, s % 60.0)
}

/// A checkerboard of the theme's two panel shades: what "transparent" looks like.
fn checker(b: &mut crate::kit::Brush, r: Rect, s: &Style) {
    let cell = (12.0 * s.k).max(4.0);
    let (a, c) = (s.t.panel, crate::kit::color::shade(s.t.panel, 0.12));
    b.rect(r, Fill::Solid(a));
    let (nx, ny) = (
        (r.width() / cell).ceil() as i32,
        (r.height() / cell).ceil() as i32,
    );
    for y in 0..ny {
        for x in 0..nx {
            if (x + y) % 2 == 1 {
                let min = Pos2::new(r.left() + x as f32 * cell, r.top() + y as f32 * cell);
                let q = Rect::from_min_size(min, Vec2::splat(cell)).intersect(r);
                b.rect(q, Fill::Solid(c));
            }
        }
    }
}

/// The preview's picture: drawn again only when what it shows changed.
fn preview_texture(
    app: &mut CantusApp,
    ctx: &egui::Context,
    d: &VideoSettings,
    px: [u32; 2],
    length: f64,
) -> (Option<egui::TextureId>, vx::PreviewState) {
    // What the picture shows: the choices, its size, the moment, the project — and
    // whether the moment's own render and every character are in yet (review: the preview
    // is the file's frame, drawn again when those arrive).
    let state = vx::preview_timelines(app, d);
    let loaded = !matches!(job::resolve_dancers(app), job::Cast::Loading);
    // Drawn again while something is on its way — never for a render that failed.
    if state == vx::PreviewState::Rendering || !loaded {
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }
    let real = match &state {
        vx::PreviewState::Ready(t) => Some(t.clone()),
        _ => None,
    };
    let key = format!(
        "{d:?}|{px:?}|{:.3}|{}|{}|{loaded}",
        app.video.scrub,
        app.bus.revision(),
        real.is_some()
    );
    if let Some((k, tex)) = &app.video.preview {
        if *k == key {
            return (Some(tex.id()), state);
        }
    }
    let frames = preview_frames(app, d, px, length, real);
    let Some(mut pm) = tiny_skia::Pixmap::new(px[0], px[1]) else {
        return (None, state);
    };
    let n = (app.video.scrub * d.fps as f64).floor() as u32;
    cantus_video::export::FrameSource::draw(&frames, n, &mut pm);
    let image =
        egui::ColorImage::from_rgba_premultiplied([px[0] as usize, px[1] as usize], pm.data());
    let tex = match app.video.preview.take() {
        Some((_, mut t)) => {
            t.set(image, egui::TextureOptions::LINEAR);
            t
        }
        None => ctx.load_texture("video-preview", image, egui::TextureOptions::LINEAR),
    };
    let id = tex.id();
    app.video.preview = Some((key, tex));
    (Some(id), state)
}

/// The song's frames as the preview draws them: every dancer from the moment's own render
/// (`real`: automation, notes and modulators as in the file) — until it is in, to the beat
/// with its own numbers — with the title and the background as the file will have them.
fn preview_frames(
    app: &mut CantusApp,
    d: &VideoSettings,
    px: [u32; 2],
    length: f64,
    real: Option<crate::video_export::Timelines>,
) -> SongFrames {
    let (from, _) = vx::range(app, d.range).unwrap_or_default();
    let map = app.project().tempo_map();
    let beat = job::beat_timeline(&map, from, length.max(0.1));
    let specs = job::dancers(app);
    let mut dancers: Vec<Dancer> = specs
        .iter()
        .map(|spec| {
            let mut tl = beat.clone();
            if let Some(n) = app
                .project()
                .track(spec.track)
                .and_then(|t| t.instrument.native())
            {
                for f in &mut tl {
                    for (v, key) in f.values.iter_mut().zip(cantus_core::inst::dancer::KEYS) {
                        *v = n.get(key);
                    }
                }
            }
            let rendered = real
                .as_ref()
                .and_then(|r| r.iter().find(|(t, _)| *t == spec.track))
                .map(|(_, tl)| tl.clone());
            Dancer {
                character: spec.character.clone(),
                timeline: rendered.unwrap_or(tl),
            }
        })
        .collect();
    if dancers.is_empty() {
        let dir = crate::dance_import::project_dir(app);
        let (character, _) = app
            .dance_live
            .character(cantus_core::inst::dancer::DEFAULT_CHARACTER, &dir);
        dancers.push(Dancer {
            character,
            timeline: beat,
        });
    }
    let c = Style::current().t.panel;
    let stage = Stage::new(
        px[0],
        px[1],
        job::background_for(d, [c.r(), c.g(), c.b(), 255]),
    );
    SongFrames {
        dancers,
        stage,
        start_seconds: map.seconds_at(from.0 as f64),
        fps: d.fps,
        overlay: crate::stage::vis_video::preview(app, d, job::title_overlay(app, d)), // r35:stage
    }
}

/// The footer: what it will take (or the progress, or how it ended) and the buttons.
fn footer(app: &mut CantusApp, ui: &mut Ui, rect: Rect) -> bool {
    let s = Style::current();
    let k = s.k;
    let buttons = Rect::from_min_max(Pos2::new(rect.right() - 250.0 * k, rect.top()), rect.max);
    let status = Rect::from_min_max(
        rect.min,
        Pos2::new(buttons.left() - 16.0 * k, rect.bottom()),
    );
    let busy = app.video.busy();
    if let Some((phase, fraction, left)) = app.video.progress() {
        let bar = Rect::from_min_max(
            Pos2::new(status.left(), status.center().y - 11.0 * k),
            Pos2::new(status.right(), status.center().y + 11.0 * k),
        );
        let mut words = t(phase);
        if let Some(left) = left {
            words = tf("video.left", &[("phase", &words), ("time", &clock(left))]);
        }
        let time = ui.input(|i| i.time);
        with_brush(ui.painter(), |b| {
            progress(b, bar, &s, &words, Some(fraction), time)
        });
    } else {
        let (msg, warn) = match &app.video.last {
            Some(Ok(m)) => (m.clone(), false),
            Some(Err(m)) => (m.clone(), true),
            None => (rows::estimate(app), false),
        };
        with_brush(ui.painter(), |b| {
            let _ = text::paint(
                b,
                status,
                &Text::new(
                    &msg,
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
        let b = parts::button(&mut child, &t("video.stop"), true, true, 96.0);
        put(&child, "button.stop", b.rect);
        if b.clicked() {
            vx::stop(app);
        }
    } else {
        let can = rows::refusal(app).is_none();
        let b = parts::button(&mut child, &t("video.export"), true, can, 110.0);
        put(&child, "button.export", b.rect);
        if b.clicked() && can {
            vx::choose_and_start(app);
        }
        child.add_space(8.0 * k);
        let label = if app.video.last.is_some() {
            t("video.close")
        } else {
            t("video.cancel")
        };
        let b = parts::button(&mut child, &label, false, true, 90.0);
        put(&child, "button.close", b.rect);
        close = b.clicked();
    }
    close
}
