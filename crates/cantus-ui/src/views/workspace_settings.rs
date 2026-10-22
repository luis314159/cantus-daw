//! The settings window, in the kit's look (ADR 0024 §3's row grammar): section bands, and
//! label | value rows of 24 pt with the label right-aligned in a fixed column and every
//! value in a well, a menu field or a segmented control.
//!
//! Audio changes are deliberately *not* applied as they are chosen: reopening the device
//! on every choice would stutter the sound. The window keeps a pending choice and applies
//! it when *Apply* is pressed.

use super::workspace_widgets as w;
use crate::app::CantusApp;
use crate::i18n::{t, tf, Locale};
use crate::kit::glyph::Glyph;
use crate::kit::knob::KnobSize;
use crate::kit::light::{self, RaisedOpts};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::widgets::{segmented, Param};
use crate::kit::{with_brush, Fill, Style};
use crate::settings::{AudioSettings, MAX_AUTOSAVE_SECONDS, MIN_AUTOSAVE_SECONDS, UI_SCALES};
use egui::{Align2, Context, Id, Pos2, Rect, Sense, Ui, Vec2};
use std::fmt::Write as _;

/// The label column's width, in points.
pub(super) const LABEL_W: f32 = 140.0;

/// The window's own state.
#[derive(Default)]
pub struct Page {
    /// The audio choice not yet applied.
    pub(super) pending: Option<AudioSettings>,
    pub(super) list: Option<(super::settings_audio::List, Pos2)>,
    /// The Audio page's own (ADR 0025): the device scan, the loopback measurement.
    pub audio: super::settings_audio::State,
}

/// The settings window.
pub fn window(app: &mut CantusApp, ctx: &Context) {
    let mut open = app.show_settings;
    let s = Style::current();
    let screen = ctx.content_rect();
    // Below the toolbar, clear of the status bar.
    let size = Vec2::new(600.0, 800.0).min(screen.size() - Vec2::new(40.0, 100.0));
    egui::Window::new("cantus-settings")
        .id(Id::new("cantus-settings"))
        .title_bar(false)
        .frame(egui::Frame::NONE)
        .resizable(true)
        .default_size(size)
        .min_size(Vec2::new(420.0, 320.0).min(size))
        .default_pos(Pos2::new(
            screen.center().x - size.x * 0.5,
            screen.top() + 64.0,
        ))
        .constrain(true)
        .show(ctx, |ui| {
            let rect = ui.max_rect();
            // The window claims what it paints: its contents sit in child UIs, which
            // leave the window's own area empty, and egui hands the wheel only to the layer
            // whose area is under the pointer (the owner, 2026-10-22: Settings would not
            // scroll to its Plug-ins).
            ui.expand_to_include_rect(rect);
            frame(ui, rect, &s);
            let header = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 30.0 * s.k));
            if title(ui, header, &s, Glyph::Gear, "settings.title") {
                open = false;
            }
            let body = Rect::from_min_max(
                Pos2::new(rect.left() + 12.0 * s.k, header.bottom() + 8.0 * s.k),
                Pos2::new(rect.right() - 6.0 * s.k, rect.bottom() - 10.0 * s.k),
            );
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
            egui::ScrollArea::vertical()
                .id_salt("cantus-settings-body")
                .auto_shrink([false, false])
                .show(&mut child, |ui| {
                    ui.set_width(body.width() - 14.0 * s.k);
                    ui.spacing_mut().item_spacing = Vec2::new(8.0 * s.k, 6.0 * s.k);
                    super::verse::settings(app, ui);
                    super::settings_audio::page(app, ui);
                    midi(app, ui);
                    look(app, ui);
                    saving(app, ui);
                    sample_cache(app, ui);
                    super::update::settings_section(app, ui);
                    w::band(ui, Src::Key("host.manager.title"));
                    super::plugins::manager(app, ui);
                });
        });
    super::settings_audio::lists(app, ctx);
    app.show_settings = open;
    if !open {
        app.settings_page.pending = None;
        app.settings_page.list = None;
    }
}

/// A floating window's face: a soft shadow and the raised panel.
pub fn frame(ui: &Ui, rect: Rect, s: &Style) {
    with_brush(ui.painter(), |b| {
        b.rrect_soft(
            rect.translate(Vec2::new(0.0, 4.0)),
            s.r(6.0) + 4.0,
            Fill::Solid(crate::kit::color::with_alpha(s.t.shadow, 0.9)),
            16.0,
        );
        light::raised(
            b,
            rect,
            s.r(6.0),
            s.t.panel,
            RaisedOpts {
                drop: false,
                k: s.k,
                depth: s.t.depth,
                ..Default::default()
            },
        );
    });
}

/// A floating window's title row: a glyph, the title, ✕. Returns whether ✕ was clicked.
pub fn title(ui: &mut Ui, rect: Rect, s: &Style, g: Glyph, key: &str) -> bool {
    let k = s.k;
    with_brush(ui.painter(), |b| {
        let r = b.snap_rect(rect);
        b.hline(
            r.left() + 1.0,
            r.right() - 1.0,
            r.bottom() - b.px() * 0.5,
            b.px(),
            crate::kit::color::shade(s.t.panel, -0.16),
        );
        let gr = Rect::from_center_size(
            Pos2::new(r.left() + 18.0 * k, r.center().y),
            Vec2::splat(14.0 * k),
        );
        crate::kit::glyph::paint(b, gr, g, s.t.text_dim);
        let tr = Rect::from_min_max(
            Pos2::new(gr.right() + 8.0 * k, r.top()),
            Pos2::new(r.right() - 40.0 * k, r.bottom()),
        );
        let _ = text::paint(
            b,
            tr,
            &Text::key(key, Role::Title, s.t.text)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
    });
    let keys = Rect::from_min_max(
        Pos2::new(rect.right() - 36.0 * k, rect.top() + 4.0 * k),
        Pos2::new(rect.right() - 8.0 * k, rect.bottom() - 4.0 * k),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(keys)
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    w::glyph_key(&mut child, Glyph::Close, false, 22.0, &t("workspace.close")).clicked()
}

/// A dim line in a box.
pub(super) fn note(ui: &mut Ui, s: &str, warn: bool) {
    let st = Style::current();
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0 * st.k), Sense::hover());
    with_brush(ui.painter(), |b| {
        let _ = text::paint(
            b,
            rect,
            &Text::new(
                s,
                Role::Label,
                if warn { st.t.danger } else { st.t.text_dim },
            )
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End)),
            st.tk(),
        );
    });
}

fn midi(app: &mut CantusApp, ui: &mut Ui) {
    w::band(ui, Src::Key("settings.midi"));
    ui.horizontal(|ui| {
        w::row_label(ui, "", LABEL_W, 24.0);
        if w::button(ui, Src::Key("settings.rescan_midi"), 0.0, true).clicked() {
            app.rescan_midi();
        }
        note(
            ui,
            &tf("status.midi", &[("n", &app.midi_port_names.len())]),
            false,
        );
    });
    if app.midi_port_names.is_empty() {
        note(ui, &t("settings.no_midi"), false);
    }
    let names = app.midi_port_names.clone();
    for name in names {
        ui.horizontal(|ui| {
            w::row_label(ui, "", LABEL_W, 18.0);
            note(ui, &name, false);
        });
    }
}

fn look(app: &mut CantusApp, ui: &mut Ui) {
    let s = Style::current();
    let k = s.k;
    w::band(ui, Src::Key("settings.appearance"));
    ui.horizontal(|ui| {
        w::row_label(ui, "menu.language", LABEL_W, 24.0);
        let mut sel = Locale::ALL
            .iter()
            .position(|l| *l == crate::i18n::locale())
            .unwrap_or(0);
        let labels: Vec<Src<'_>> = Locale::ALL
            .iter()
            .map(|l| Src::Str(l.native_name()))
            .collect();
        let width = (ui.available_width() / k - 8.0).max(160.0);
        if segmented(ui, &mut sel, &labels, &t("menu.language"), width).changed() {
            app.set_locale(Locale::ALL[sel]);
        }
    });
    ui.horizontal(|ui| {
        w::row_label(ui, "settings.ui_scale", LABEL_W, 24.0);
        const LABELS: [&str; 9] = ["75", "90", "100", "110", "125", "150", "175", "190", "200"];
        let now = app.settings.ui_scale();
        let mut sel = UI_SCALES
            .iter()
            .enumerate()
            .min_by(|a, b| (a.1 - now).abs().total_cmp(&(b.1 - now).abs()))
            .map_or(2, |(i, _)| i);
        let labels: Vec<Src<'_>> = LABELS.iter().map(|l| Src::Str(l)).collect();
        let width = (ui.available_width() / k - 8.0).max(200.0);
        if segmented(ui, &mut sel, &labels, &t("settings.ui_scale"), width).changed() {
            let ctx = ui.ctx().clone();
            app.apply_ui_scale(&ctx, UI_SCALES[sel]);
        }
    });
    ui.horizontal(|ui| {
        w::row_label(ui, "settings.colours", LABEL_W, 24.0);
        if w::button(ui, Src::Key("settings.edit_colours"), 0.0, true).clicked() {
            app.show_theme_editor = true;
        }
    });
    ui.horizontal(|ui| {
        w::row_label(ui, "keymap.title", LABEL_W, 24.0);
        if w::button(ui, Src::Key("settings.edit_shortcuts"), 0.0, true).clicked() {
            app.show_shortcuts = true;
        }
    });
}

fn saving(app: &mut CantusApp, ui: &mut Ui) {
    w::band(ui, Src::Key("settings.saving"));
    let mut secs = app.settings.autosave_seconds as f32;
    let before = secs;
    ui.horizontal(|ui| {
        w::row_label(ui, "settings.autosave", LABEL_W, 70.0);
        let name = t("settings.autosave");
        Param::new(
            &mut secs,
            MIN_AUTOSAVE_SECONDS as f32..=MAX_AUTOSAVE_SECONDS as f32,
            crate::settings::DEFAULT_AUTOSAVE_SECONDS as f32,
        )
        .format(|v, b| {
            let v = v.round() as u64;
            let _ = if v >= 60 && v.is_multiple_of(60) {
                write!(b, "{} min", v / 60)
            } else {
                write!(b, "{v} s")
            };
        })
        .log(true)
        .size(KnobSize::S)
        .cell(Src::Key("workspace.autosave.cell"))
        .name(&name)
        .show(ui);
    });
    note(ui, &t("settings.autosave_hint"), false);
    // Whole seconds, and a round number once past a minute.
    let snapped = if secs >= 60.0 {
        (secs / 30.0).round() * 30.0
    } else {
        (secs / 5.0).round() * 5.0
    };
    if (secs - before).abs() > f32::EPSILON && snapped as u64 != app.settings.autosave_seconds {
        app.settings.autosave_seconds = snapped as u64;
        app.store_settings();
    }
}

/// The sample cache (ADR 0050, review 11): its size limit — automatic, or chosen — the
/// disk it is on, what it holds now, and *Clear*.
pub fn sample_cache(app: &mut CantusApp, ui: &mut Ui) {
    use crate::settings::{DEFAULT_SAMPLE_CACHE_GB, MAX_SAMPLE_CACHE_GB, MIN_SAMPLE_CACHE_GB};
    const GB: f64 = (1u64 << 30) as f64;
    let gb_of = |b: u64| format!("{:.1}", b as f64 / GB);
    w::band(ui, Src::Key("smp.cache.band"));
    let room = app.sample_cache_room();
    let mut gb = app
        .settings
        .sample_cache_gb
        .unwrap_or((room.limit as f64 / GB) as f32)
        .clamp(MIN_SAMPLE_CACHE_GB, MAX_SAMPLE_CACHE_GB);
    let before = gb;
    let mut auto = false;
    // Automatic: the knob says so, and the line below gives the limit in force (third
    // review #10: never a rounded or clamped figure beside the real one).
    let auto_now = app.settings.sample_cache_gb.is_none();
    let auto_word = t("smp.cache.auto_cell");
    ui.horizontal(|ui| {
        w::row_label(ui, "smp.cache.limit", LABEL_W, 70.0);
        let name = t("smp.cache.limit");
        Param::new(
            &mut gb,
            MIN_SAMPLE_CACHE_GB..=MAX_SAMPLE_CACHE_GB,
            DEFAULT_SAMPLE_CACHE_GB,
        )
        .format(|v, b| {
            if auto_now {
                b.push_str(&auto_word);
            } else {
                let _ = write!(b, "{} GB", v.round() as u64);
            }
        })
        .log(true)
        .size(KnobSize::S)
        .cell(Src::Key("smp.cache.limit.cell"))
        .name(&name)
        .show(ui);
        let chosen = app.settings.sample_cache_gb.is_some();
        auto = w::button(ui, Src::Key("smp.cache.auto"), 0.0, chosen).clicked();
        if w::button(ui, Src::Key("smp.cache.clear"), 0.0, true).clicked() {
            app.clear_sample_cache();
        }
    });
    // The limit in force and the rule it comes from, then the disk's numbers — two lines,
    // so neither is cut in any language (second review #10, third #10).
    if room.auto {
        note(
            ui,
            &tf("smp.cache.room.auto", &[("limit", &gb_of(room.limit))]),
            false,
        );
    }
    if let Some(s) = room.space {
        note(
            ui,
            &tf(
                "smp.cache.room",
                &[("total", &gb_of(s.total)), ("free", &gb_of(s.free))],
            ),
            false,
        );
    }
    note(
        ui,
        &tf("smp.cache.held", &[("size", &gb_of(room.held))]),
        false,
    );
    let changed = if auto {
        app.settings.sample_cache_gb = None;
        true
    } else if (gb - before).abs() > f32::EPSILON {
        app.settings.sample_cache_gb = Some(gb.round().max(MIN_SAMPLE_CACHE_GB));
        true
    } else {
        false
    };
    if changed {
        app.settings.apply_sample_cache_limit();
        app.inst.sampler.cache_size = None;
        app.store_settings();
    }
}
