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

/// Settings' pages, listed in the sidebar (round 30, the owner's friction: one long page
/// hid *Plug-ins* at its bottom), as FL's options have them.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum SettingsPage {
    #[default]
    General,
    Audio,
    Midi,
    /// Crossfades and effects on insert (round 31, ADRs 0070, 0071).
    Editing,
    Appearance,
    Saving,
    Cache,
    Updates,
    Plugins,
}

impl SettingsPage {
    pub const ALL: [SettingsPage; 9] = [
        SettingsPage::General,
        SettingsPage::Audio,
        SettingsPage::Midi,
        SettingsPage::Editing,
        SettingsPage::Appearance,
        SettingsPage::Saving,
        SettingsPage::Cache,
        SettingsPage::Updates,
        SettingsPage::Plugins,
    ];

    /// The sidebar's word.
    pub fn key(self) -> &'static str {
        match self {
            SettingsPage::General => "ship.settings.general",
            SettingsPage::Audio => "ship.settings.audio",
            SettingsPage::Midi => "ship.settings.midi",
            SettingsPage::Editing => "clipfx.settings.page",
            SettingsPage::Appearance => "ship.settings.appearance",
            SettingsPage::Saving => "ship.settings.saving",
            SettingsPage::Cache => "ship.settings.cache",
            SettingsPage::Updates => "ship.settings.updates",
            SettingsPage::Plugins => "ship.settings.plugins",
        }
    }
}

/// Open Settings on `page` (the Options menu, the picker's *Find more plug-ins…*).
pub fn open(app: &mut CantusApp, page: SettingsPage) {
    app.settings_page.page = page;
    app.settings_page.body_scroll_top = true;
    app.show_settings = true;
}

/// The window's own state.
#[derive(Default)]
pub struct Page {
    /// The page the sidebar chose (kept while Cantus runs).
    pub page: SettingsPage,
    /// Scroll the page to its top on the next frame (a page was chosen).
    pub(super) body_scroll_top: bool,
    /// The audio choice not yet applied.
    pub(super) pending: Option<AudioSettings>,
    pub(super) list: Option<(super::settings_audio::List, Pos2)>,
    /// Open the cores list where its field is drawn next (the shots).
    pub(super) open_cores: bool,
    /// The Audio page's own (ADR 0025): the device scan, the loopback measurement.
    pub audio: super::settings_audio::State,
    /// The MIDI page's list of outputs, and when it was taken (round 33). r33:sync
    pub(crate) midi_outputs: (Option<std::time::Instant>, Vec<String>),
}

/// The settings window.
pub fn window(app: &mut CantusApp, ctx: &Context) {
    let mut open = app.show_settings;
    let s = Style::current();
    let screen = ctx.content_rect();
    // Below the toolbar, clear of the status bar.
    let size = Vec2::new(780.0, 760.0).min(screen.size() - Vec2::new(40.0, 100.0));
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
            // The pages' sidebar at the left, the chosen page at the right (round 30).
            let side = Rect::from_min_max(
                Pos2::new(rect.left() + 8.0 * s.k, header.bottom() + 8.0 * s.k),
                Pos2::new(rect.left() + SIDEBAR_W * s.k, rect.bottom() - 10.0 * s.k),
            );
            if let Some(p) = sidebar(ui, side, &s, app.settings_page.page) {
                app.settings_page.page = p;
                app.settings_page.body_scroll_top = true;
            }
            let body = Rect::from_min_max(
                Pos2::new(side.right() + 12.0 * s.k, header.bottom() + 8.0 * s.k),
                Pos2::new(rect.right() - 6.0 * s.k, rect.bottom() - 10.0 * s.k),
            );
            // The page's panel fills the window's height beside the sidebar, flat like it,
            // whatever the page's length (the owner's integrator, round 30: a page short of
            // the window left the frame's shading showing under it).
            let panel = Rect::from_min_max(
                Pos2::new(side.right() + 6.0 * s.k, side.top()),
                Pos2::new(rect.right() - 8.0 * s.k, side.bottom()),
            );
            with_brush(ui.painter(), |b| {
                b.register(panel, "settings-page", &[crate::kit::Tok::Panel], &[]);
                b.rrect(panel, s.r(4.0), Fill::Solid(s.t.panel));
            });
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
            let mut area = egui::ScrollArea::vertical()
                .id_salt(("cantus-settings-body", app.settings_page.page as u8))
                .auto_shrink([false, false]);
            if std::mem::take(&mut app.settings_page.body_scroll_top) {
                area = area.vertical_scroll_offset(0.0);
            }
            area.show(&mut child, |ui| {
                ui.set_width(body.width() - 14.0 * s.k);
                ui.spacing_mut().item_spacing = Vec2::new(8.0 * s.k, 6.0 * s.k);
                page_body(app, ui);
            });
        });
    super::settings_audio::lists(app, ctx);
    app.show_settings = open;
    if !open {
        app.settings_page.pending = None;
        app.settings_page.list = None;
    }
}

/// The sidebar's width at 100 %.
pub const SIDEBAR_W: f32 = 160.0;

/// What one page holds.
fn page_body(app: &mut CantusApp, ui: &mut Ui) {
    match app.settings_page.page {
        SettingsPage::General => {
            super::verse::settings(app, ui);
            // The start page at start (round 31, ADR 0072).
            super::start_page::settings(app, ui);
        }
        SettingsPage::Audio => super::settings_audio::page(app, ui),
        SettingsPage::Midi => midi(app, ui),
        SettingsPage::Editing => super::clipfx_settings::page(app, ui),
        SettingsPage::Appearance => look(app, ui),
        SettingsPage::Saving => saving(app, ui),
        SettingsPage::Cache => sample_cache(app, ui),
        SettingsPage::Updates => super::update::settings_section(app, ui),
        SettingsPage::Plugins => {
            w::band(ui, Src::Key("host.manager.title"));
            super::plugins::manager(app, ui);
        }
    }
}

/// The pages' list, FL's options sidebar: a word per row, the chosen one lit. Returns a
/// page clicked.
fn sidebar(ui: &mut Ui, r: Rect, s: &Style, now: SettingsPage) -> Option<SettingsPage> {
    use crate::kit::color::with_alpha;
    use crate::kit::Tok;
    let row_h = 28.0 * s.k;
    let mut chosen = None;
    let mut rows = Vec::with_capacity(SettingsPage::ALL.len());
    for (i, p) in SettingsPage::ALL.iter().enumerate() {
        let row = Rect::from_min_size(
            Pos2::new(
                r.left() + 4.0 * s.k,
                r.top() + 4.0 * s.k + i as f32 * (row_h + 2.0 * s.k),
            ),
            Vec2::new(r.width() - 8.0 * s.k, row_h),
        );
        let resp = ui.interact(row, Id::new(("settings-page", i)), Sense::click());
        if resp.clicked() {
            chosen = Some(*p);
        }
        rows.push((*p, row, resp.hovered()));
    }
    with_brush(ui.painter(), |b| {
        b.register(
            r,
            "settings-sidebar",
            &[Tok::Strip, Tok::Text, Tok::TextDim, Tok::Selection],
            &[],
        );
        b.rrect(r, s.r(4.0), Fill::Solid(s.t.strip));
        for (p, row, hovered) in rows {
            let on = p == now;
            if on {
                b.rrect(row, s.r(3.0), Fill::Solid(s.t.selection));
            } else if hovered {
                b.rrect(row, s.r(3.0), Fill::Solid(with_alpha(s.t.text, 0.07)));
            }
            let ink = if on || hovered {
                s.t.text
            } else {
                s.t.text_dim
            };
            let _ = text::paint(
                b,
                row.shrink2(Vec2::new(10.0 * s.k, 0.0)),
                &Text::key(p.key(), Role::Body, ink)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
        }
    });
    chosen
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
    let color = if warn { st.t.danger } else { st.t.text_dim };
    let width = ui.available_width();
    wrapped(ui, s, Role::Label, color, width);
}

/// `s` broken into lines no wider than `width` points, at word boundaries, one 18-pt row
/// each — a long note or warning reads whole at any window width (round 30: a Spanish
/// warning in Settings ▸ Plug-ins was cut with "…"). A single word wider than the row is
/// the only thing still elided.
pub fn wrapped(ui: &mut Ui, s: &str, role: Role, color: egui::Color32, width: f32) {
    let st = Style::current();
    let tk = st.tk();
    let fits = |t: &str| text::measure(ui.ctx(), &Text::new(t, role, color), tk).x <= width;
    let mut lines: Vec<String> = Vec::new();
    for para in s.split('\n') {
        let mut cur = String::new();
        for word in para.split_whitespace() {
            let cand = if cur.is_empty() {
                word.to_owned()
            } else {
                format!("{cur} {word}")
            };
            if !cur.is_empty() && !fits(&cand) {
                lines.push(std::mem::replace(&mut cur, word.to_owned()));
            } else {
                cur = cand;
            }
        }
        lines.push(cur);
    }
    for line in lines {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 18.0 * st.k), Sense::hover());
        if ui.is_rect_visible(rect) {
            with_brush(ui.painter(), |b| {
                let _ = text::paint(
                    b,
                    rect,
                    &Text::new(&line, role, color)
                        .align(Align2::LEFT_CENTER)
                        .policy(Policy::Elide(Elide::End)),
                    tk,
                );
            });
        }
    }
}

// The MIDI page: the ports' table and sync (round 33, ADR 0078–0079).
#[path = "settings_midi.rs"]
mod settings_midi;
pub use settings_midi::{inputs_layout as midi_inputs_layout, InputsLayout}; // r33:launcher

fn midi(app: &mut CantusApp, ui: &mut Ui) {
    settings_midi::page(app, ui);
}

/// Settings ▸ MIDI with these ports, for the screenshots (round 33).
pub fn demo_midi(app: &mut CantusApp, inputs: &[&str], outputs: &[&str]) {
    settings_midi::demo_ports(app, inputs, outputs);
    open(app, SettingsPage::Midi);
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
    // Units wear their own faces (round 32, ADR 0074).
    ui.horizontal(|ui| {
        w::row_label(ui, "skin.settings.faces", LABEL_W, 24.0);
        let mut sel = usize::from(!app.settings.unit_faces);
        let labels = [
            Src::Key("skin.settings.faces.on"),
            Src::Key("skin.settings.faces.off"),
        ];
        if segmented(ui, &mut sel, &labels, &t("skin.settings.faces"), 160.0).changed() {
            app.settings.unit_faces = sel == 0;
            app.apply_unit_faces();
            app.store_settings();
        }
    });
    ui.horizontal(|ui| {
        w::row_label(ui, "", LABEL_W, 18.0);
        note(ui, &t("skin.settings.faces.hint"), false);
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
