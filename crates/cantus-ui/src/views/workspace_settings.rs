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
use crate::kit::appearance::Mode;
use crate::kit::glyph::Glyph;
use crate::kit::knob::KnobSize;
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::widgets::{segmented, Param};
use crate::kit::{with_brush, Fill, Style};
use crate::settings::{AudioSettings, MAX_AUTOSAVE_SECONDS, MIN_AUTOSAVE_SECONDS, UI_SCALES};
use egui::{Align2, Context, Id, Pos2, Rect, Sense, Ui, Vec2};
use std::fmt::Write as _;

/// The label column's width, in points.
pub(super) const LABEL_W: f32 = 150.0;

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
    /// Agents and the MCP server (round 34, ADR 0081). // r34:mcp
    Ai, // r34:mcp
    /// The machine-learning models (round 34, ADR 0085). // r34:stems
    Models,
}

impl SettingsPage {
    pub const ALL: [SettingsPage; 11] = [
        // r34:mcp (10), r34:stems (11)
        SettingsPage::General,
        SettingsPage::Audio,
        SettingsPage::Midi,
        SettingsPage::Editing,
        SettingsPage::Appearance,
        SettingsPage::Saving,
        SettingsPage::Cache,
        SettingsPage::Updates,
        SettingsPage::Plugins,
        SettingsPage::Ai,     // r34:mcp
        SettingsPage::Models, // r34:stems
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
            SettingsPage::Ai => "ai.settings.page", // r34:mcp
            SettingsPage::Models => "stems.models.page", // r34:stems
        }
    }
}

impl SettingsPage {
    /// The page whose sidebar word is `key` (a stored choice), if any still is.
    pub fn from_key(key: &str) -> Option<SettingsPage> {
        SettingsPage::ALL.into_iter().find(|p| p.key() == key)
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
    /// Whether the window was shown last frame: the frame it opens it is raised over every
    /// other window in front (the start page's among them).
    pub(super) was_open: bool,
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

/// The settings window is not shown this frame.
pub fn closed(app: &mut CantusApp) {
    app.settings_page.was_open = false;
}

/// The settings window.
pub fn window(app: &mut CantusApp, ctx: &Context) {
    let mut open = app.show_settings;
    let s = Style::current();
    let screen = ctx.content_rect();
    scrim(ctx, "settings");
    // Below the toolbar, clear of the status bar.
    let size = Vec2::new(888.0, 590.0).min(screen.size() - Vec2::new(40.0, 100.0));
    let opening = !app.settings_page.was_open;
    // In front, with the kit's other dialogs (integration of round 39: opened over the start
    // page, a Middle window sat under it and its clicks went to the page).
    let shown = egui::Window::new("cantus-settings")
        .id(Id::new("cantus-settings"))
        .order(egui::Order::Foreground)
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
            let header = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 44.0 * s.k));
            if title(ui, header, &s, Glyph::Gear, "settings.title") {
                open = false;
            }
            // The foot (`dir-a`): what is running, and Done.
            let foot_r = Rect::from_min_max(
                Pos2::new(rect.left() + SIDEBAR_W * s.k, rect.bottom() - 52.0 * s.k),
                rect.max,
            );
            let status = crate::views::audio_chip::status_line(app, ctx);
            let (_, done) = foot(
                ui,
                foot_r,
                &s,
                Some((&status.0, status.1)),
                None,
                &t("topbar.done"),
            );
            if done {
                open = false;
            }
            // The pages' list at the left on the window's base, the chosen page at the
            // right on its surface (round 30; `dir-a`'s side list).
            let side = Rect::from_min_max(
                Pos2::new(rect.left(), header.bottom()),
                Pos2::new(rect.left() + SIDEBAR_W * s.k, rect.bottom()),
            );
            if let Some(p) = sidebar(ui, side, &s, app.settings_page.page) {
                app.settings_page.page = p;
                app.settings_page.body_scroll_top = true;
                // r39:ux-b — kept for the next session (ADR 0110).
                app.settings.settings_page = Some(p.key().to_string());
                app.store_settings();
            }
            let body = Rect::from_min_max(
                Pos2::new(side.right() + 24.0 * s.k, header.bottom() + 8.0 * s.k),
                Pos2::new(rect.right() - 12.0 * s.k, foot_r.top() - 4.0 * s.k),
            );
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
    if opening {
        if let Some(r) = &shown {
            ctx.move_to_top(r.response.layer_id);
        }
    }
    super::settings_audio::lists(app, ctx);
    // r39:ux-b — Esc closes Settings when it is what opened last (ADR 0110); a list or a
    // field of its own takes the key first.
    if open {
        crate::esc::open(ctx, crate::esc::Layer::Settings);
        if app.settings_page.list.is_none()
            && !crate::kit::field::active(ctx)
            && crate::esc::take(ctx, crate::esc::Layer::Settings)
        {
            open = false;
        }
    }
    app.show_settings = open;
    app.settings_page.was_open = open;
    if !open {
        app.settings_page.pending = None;
        app.settings_page.list = None;
    }
}

/// The sidebar's width at 100 %.
pub const SIDEBAR_W: f32 = 200.0;

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
        SettingsPage::Ai => crate::ai_mcp::settings_page::page(app, ui), // r34:mcp
        SettingsPage::Models => crate::stems_ui::models::page(app, ui),  // r34:stems
    }
}

/// The pages' list, FL's options sidebar: a word per row, the chosen one lit. Returns a
/// page clicked.
fn sidebar(ui: &mut Ui, r: Rect, s: &Style, now: SettingsPage) -> Option<SettingsPage> {
    use crate::kit::color::with_alpha;
    use crate::kit::Tok;
    let k = s.k;
    let row_h = 32.0 * k;
    let mut chosen = None;
    let mut rows = Vec::with_capacity(SettingsPage::ALL.len());
    for (i, p) in SettingsPage::ALL.iter().enumerate() {
        let row = Rect::from_min_size(
            Pos2::new(r.left() + 8.0 * k, r.top() + 12.0 * k + i as f32 * row_h),
            Vec2::new(r.width() - 16.0 * k, row_h - 2.0 * k),
        );
        let resp = ui.interact(row, Id::new(("settings-page", i)), Sense::click());
        // Which page it opens, on the help line (round 39, ADR 0109).
        let resp = super::hint::said_with(resp, || {
            crate::i18n::tf(
                "ux.hint.settings_page",
                &[("page", &crate::i18n::t(p.key()))],
            )
        });
        if resp.clicked() {
            chosen = Some(*p);
        }
        rows.push((*p, row, resp.hovered()));
    }
    with_brush(ui.painter(), |b| {
        b.register(
            r,
            "settings-sidebar",
            &[Tok::Panel, Tok::Text, Tok::TextDim, Tok::Highlight],
            &[],
        );
        side_ground(b, r, s);
        for (p, row, hovered) in rows {
            let on = p == now;
            if on {
                // A raised pill with a 2 × 14 accent bar.
                // r39:lapis2 — through the selection's tint (none in Slate).
                let pill = crate::kit::color::selected_fill(s.t.menu, s.t.selection_tint, 1.0);
                b.rrect(b.snap_rect(row), s.r(4.0), Fill::Solid(pill));
                let bar = Rect::from_center_size(
                    egui::pos2(row.left() + 1.0 * k, row.center().y),
                    Vec2::new(2.0, 14.0) * k,
                );
                b.rect(b.snap_rect(bar), Fill::Solid(s.t.highlight));
            } else if hovered {
                b.rrect(
                    b.snap_rect(row),
                    s.r(4.0),
                    Fill::Solid(with_alpha(s.t.text, 0.05)),
                );
            }
            let ink = if on || hovered {
                s.t.text
            } else {
                s.t.text_dim
            };
            let _ = text::paint(
                b,
                row.shrink2(Vec2::new(14.0 * k, 0.0)),
                &Text::key(p.key(), Role::Body, ink)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
        }
    });
    chosen
}

/// A floating window's face: in a recessed family the kit's one dialog face
/// ([`crate::kit::dialog::face`] — integration of round 39: `el-topbar`'s frame and
/// `el-controls`' became one), else [`frame_classic`].
pub fn frame(ui: &Ui, rect: Rect, s: &Style) {
    crate::kit::dialog::face(ui, rect, s);
}

/// A floating window's face in a Classic family (round 39, `dir-a`'s dialog frame,
/// `dlg_kit.py`): the bar's surface, corner 10, one shadow (14 down, blurred 22) and a lit
/// top edge.
pub fn frame_classic(ui: &Ui, rect: Rect, s: &Style) {
    use crate::kit::color::mix;
    let k = s.k;
    with_brush(ui.painter(), |b| {
        let r = s.r(10.0);
        b.rrect_soft(
            rect.translate(Vec2::new(0.0, 14.0 * k)),
            r,
            Fill::Solid(crate::kit::color::with_alpha(
                s.t.shadow,
                if s.t.dark { 0.9 } else { 0.45 },
            )),
            22.0 * k,
        );
        let rr = b.snap_rect(rect);
        b.rrect(rr, r, Fill::Solid(s.t.toolbar));
        let edge = if s.t.dark {
            mix(s.t.toolbar, s.t.ink, 0.6)
        } else {
            crate::kit::color::opaque(s.t.inner_shadow)
        };
        b.rrect_stroke(rr.shrink(b.px() * 0.5), r, b.px(), edge);
        let lit = if s.t.dark {
            mix(s.t.toolbar, s.t.text, 0.08)
        } else {
            mix(s.t.toolbar, s.t.light, 0.7)
        };
        b.hline(
            rr.left() + r,
            rr.right() - r,
            rr.top() + b.px() * 1.5,
            b.px(),
            lit,
        );
    });
}

/// A floating window's title row: in a recessed family the kit's dialog head
/// ([`crate::kit::dialog::head`]), else [`title_classic`]. Returns whether the close key
/// was clicked.
pub fn title(ui: &mut Ui, rect: Rect, s: &Style, g: Glyph, key: &str) -> bool {
    crate::kit::dialog::head(ui, rect, s, g, key)
}

/// A floating window's title row in a Classic family (`dir-a`: 15 Medium, a close key
/// 26 × 24 at the right). Returns whether the close key was clicked. `g`, the window's
/// glyph, is no longer drawn (the title says it).
pub fn title_classic(ui: &mut Ui, rect: Rect, s: &Style, g: Glyph, key: &str) -> bool {
    let _ = g;
    let k = s.k;
    let pad = 20.0 * k;
    with_brush(ui.painter(), |b| {
        let r = b.snap_rect(rect);
        let tr = Rect::from_min_max(
            Pos2::new(r.left() + pad, r.top()),
            Pos2::new(r.right() - 44.0 * k, r.bottom()),
        );
        let _ = text::paint(
            b,
            tr,
            &Text::key(key, Role::Title, s.t.text)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk() * 15.0 / Role::Title.size().max(1.0),
        );
    });
    let close = Rect::from_center_size(
        Pos2::new(rect.right() - 12.0 * k - 13.0 * k, rect.center().y),
        Vec2::new(26.0, 24.0) * k,
    );
    let resp = ui.interact(close, ui.id().with(("dialog-close", key)), Sense::click());
    let resp = super::hint::said_with(resp, || t("workspace.close"));
    let st = crate::kit::State::of(&resp, false);
    with_brush(ui.painter(), |b| {
        crate::kit::cap::well_key(
            b,
            close,
            s,
            st,
            crate::kit::cap::KeyFace::Glyph(Glyph::Close),
        );
    });
    resp.clicked()
}

/// A dialog's foot (`dir-a`): a line over it, `status` at the left with a green lamp when
/// `ok`, and the buttons at the right — `quiet` as words, `primary` an accent fill.
/// Returns (quiet clicked, primary clicked).
pub fn foot(
    ui: &mut Ui,
    rect: Rect,
    s: &Style,
    status: Option<(&str, bool)>,
    quiet: Option<&str>,
    primary: &str,
) -> (bool, bool) {
    use crate::kit::color::mix;
    let k = s.k;
    let pad = 20.0 * k;
    with_brush(ui.painter(), |b| {
        b.hline(
            rect.left(),
            rect.right(),
            b.snap_mid(rect.top()),
            b.px(),
            mix(s.t.toolbar, s.t.ink, 0.5),
        );
        if let Some((words, ok)) = status {
            let c = Pos2::new(rect.left() + pad + 3.0 * k, rect.center().y);
            let lamp = if ok { s.t.led } else { s.t.meter_high };
            b.circle(c, 3.0 * k, Fill::Solid(lamp));
            let tr = Rect::from_min_max(
                Pos2::new(c.x + 8.0 * k, rect.top()),
                Pos2::new(rect.center().x + 60.0 * k, rect.bottom()),
            );
            let _ = text::paint(
                b,
                tr,
                &Text::new(words, Role::Label, s.t.text_dim)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
        }
    });
    let pw = (text::measure(
        ui.ctx(),
        &Text::new(primary, Role::Control, s.t.text),
        s.tk(),
    )
    .x + 32.0 * k)
        .max(80.0 * k);
    let pr = Rect::from_min_size(
        Pos2::new(rect.right() - pad - pw, rect.center().y - 14.0 * k),
        Vec2::new(pw, 28.0 * k),
    );
    let presp = ui.interact(
        pr,
        ui.id().with(("dialog-primary", primary)),
        Sense::click(),
    );
    let presp = super::hint::said_with(presp, || primary.to_string());
    let mut quiet_clicked = false;
    if let Some(q) = quiet {
        let qw = text::measure(ui.ctx(), &Text::new(q, Role::Body, s.t.text), s.tk()).x + 24.0 * k;
        let qr = Rect::from_min_size(
            Pos2::new(pr.left() - 12.0 * k - qw, pr.top()),
            Vec2::new(qw, 28.0 * k),
        );
        let qresp = ui.interact(qr, ui.id().with(("dialog-quiet", q)), Sense::click());
        let qresp = super::hint::said_with(qresp, || q.to_string());
        quiet_clicked = qresp.clicked();
        // The kit's dialog button (integration of round 39: one button with `kit::dialog`).
        let st = crate::kit::State::of(&qresp, false);
        with_brush(ui.painter(), |b| {
            crate::kit::dialog::button(
                b,
                qr,
                s,
                st,
                crate::kit::dialog::Kind::Quiet,
                Src::Str(q),
                None,
            );
        });
    }
    let st = crate::kit::State::of(&presp, false);
    with_brush(ui.painter(), |b| {
        crate::kit::dialog::button(
            b,
            pr,
            s,
            st,
            crate::kit::dialog::Kind::Primary,
            Src::Str(primary),
            None,
        );
    });
    (quiet_clicked, presp.clicked())
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

// The external gear's bands (round 36, ADR 0094, 0095). r36:external
#[path = "external_bands.rs"] // r36:external
pub mod external_bands; // r36:external

fn midi(app: &mut CantusApp, ui: &mut Ui) {
    settings_midi::page(app, ui);
    external_bands::page(app, ui); // r36:external
}

/// Settings ▸ MIDI with these ports, for the screenshots (round 33).
pub fn demo_midi(app: &mut CantusApp, inputs: &[&str], outputs: &[&str]) {
    settings_midi::demo_ports(app, inputs, outputs);
    open(app, SettingsPage::Midi);
}

fn look(app: &mut CantusApp, ui: &mut Ui) {
    look_family(app, ui);
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
        // r39:el-topbar — its lines one under the other (in a row they ran side by side).
        ui.vertical(|ui| note(ui, &t("skin.settings.faces.hint"), false));
    });
    // The mixer's routing cables: Patch, Flow (the default) or Subway (r39:el-cables).
    ui.horizontal(|ui| {
        w::row_label(ui, "cables.style", LABEL_W, 24.0);
        use crate::settings::CableStyle;
        // The theme's own first, then each look (r39:el-cables).
        let mut sel = app.settings.cable_style.map_or(0, |c| {
            1 + CableStyle::ALL.iter().position(|x| *x == c).unwrap_or(0)
        });
        let mut labels = vec![Src::Key("cables.style.theme")];
        labels.extend(CableStyle::ALL.map(|c| Src::Key(c.key())));
        // Wide enough for "The theme's" whole beside the four looks (integration, round 39).
        if segmented(ui, &mut sel, &labels, &t("cables.style"), 400.0).changed() {
            app.settings.cable_style = sel.checked_sub(1).map(|i| CableStyle::ALL[i]);
            app.store_settings();
        }
    });
    ui.horizontal(|ui| {
        w::row_label(ui, "", LABEL_W, 18.0);
        // Its lines one under the other (in a row they ran side by side), as the faces'.
        ui.vertical(|ui| note(ui, &t("cables.style.hint"), false));
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
    look_icon(app, ui);
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

/// The scrim under a dialog (`dir-a`'s dialog frame): the window behind it dimmed — Night
/// toward its gutter at 58 %, Paper toward its ink at 34 %. Drawn on a layer under the
/// dialog that takes no clicks. // r39:el-topbar
pub fn scrim(ctx: &Context, id: &str) {
    let s = Style::current();
    let screen = ctx.content_rect();
    let fill = scrim_fill(&s);
    egui::Area::new(Id::new(("dialog-scrim", id)))
        .order(egui::Order::Middle)
        .interactable(false)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            with_brush(ui.painter(), |b| b.rect(screen, Fill::Solid(fill)));
        });
}

/// The scrim's colour (`dir-a`): Night toward its gutter at 58 %, Paper toward its ink at
/// 34 %. // r39:el-topbar
pub fn scrim_fill(s: &Style) -> egui::Color32 {
    // A recessed family's scrim is the kit's token, as `kit::dialog` lays it (one veil).
    if s.t.system.recessed() {
        s.t.scrim
    } else if s.t.dark {
        crate::kit::color::with_alpha(s.t.ink, 0.58)
    } else {
        crate::kit::color::with_alpha(s.t.text, 0.34)
    }
}

/// A preset on a tile: its id and its tokens.
type Face = (&'static str, fn() -> crate::kit::Tokens);

/// A tile on Settings ▸ Appearance: its name's key and its presets — a family's Night and
/// Paper, or a theme made in one light only (Ultraviolet, Lagoon, Bordeaux, Print), whose
/// other side is `None`.
struct Tile {
    key: &'static str,
    night: Option<Face>,
    paper: Option<Face>,
}

const fn pair(key: &'static str, night: Face, paper: Face) -> Tile {
    Tile {
        key,
        night: Some(night),
        paper: Some(paper),
    }
}

const fn dark_only(key: &'static str, night: Face) -> Tile {
    Tile {
        key,
        night: Some(night),
        paper: None,
    }
}

const fn light_only(key: &'static str, paper: Face) -> Tile {
    Tile {
        key,
        night: None,
        paper: Some(paper),
    }
}

use crate::theme_added as added;
use crate::theme_redrawn as redrawn;

/// Every shipped theme on Settings ▸ Appearance (ADR 0111 §2), in groups: round 39's
/// families — Slate the default; Slate Lapis (Slate with a lapis accent and gold details,
/// to go with the logo's metal), Graphite and gold and Studio ink as options —; the themes
/// of rounds 24–38, redrawn in round 39's system; the themes new in round 39; the
/// high-contrast pair. The tiles are numbered in this order across the groups (their ids,
/// `("look-family", n)`).
const GROUPS: [(&str, &[Tile]); 4] = [
    (
        "look.group.r39",
        &[
            pair(
                "topbar.look.family.slate",
                ("slate_night", crate::theme_slate::slate_night_tokens),
                ("slate_paper", crate::theme_slate::slate_paper_tokens),
            ),
            pair(
                "topbar.look.family.slate_lapis",
                (
                    "slate_lapis_night",
                    crate::theme_slate_lapis::slate_lapis_night_tokens,
                ),
                (
                    "slate_lapis_paper",
                    crate::theme_slate_lapis::slate_lapis_paper_tokens,
                ),
            ),
            pair(
                "topbar.look.family.graphite",
                (
                    "graphite_night",
                    crate::theme_graphite::graphite_night_tokens,
                ),
                (
                    "graphite_paper",
                    crate::theme_graphite::graphite_paper_tokens,
                ),
            ),
            pair(
                "topbar.look.family.studio_ink",
                (
                    "studio_ink_night",
                    crate::theme_studio_ink::studio_ink_night_tokens,
                ),
                (
                    "studio_ink_paper",
                    crate::theme_studio_ink::studio_ink_paper_tokens,
                ),
            ),
        ],
    ),
    (
        "look.group.redrawn",
        &[
            pair(
                "look.family.cantus",
                ("cantus_night", redrawn::cantus_night_tokens),
                ("cantus_paper", redrawn::cantus_paper_tokens),
            ),
            pair(
                "look.family.blue_hour",
                ("blue_hour", redrawn::blue_hour_tokens),
                ("daylight", redrawn::daylight_tokens),
            ),
            pair(
                "look.family.pewter",
                ("slate", redrawn::pewter_tokens),
                ("paper", redrawn::chalk_tokens),
            ),
            pair(
                "look.family.ink_gold",
                ("ink_gold", redrawn::ink_and_gold_tokens),
                ("parchment", redrawn::parchment_tokens),
            ),
            dark_only(
                "look.family.ultraviolet",
                ("ultraviolet", redrawn::ultraviolet_tokens),
            ),
            dark_only("look.family.lagoon", ("lagoon", redrawn::lagoon_tokens)),
        ],
    ),
    (
        "look.group.new",
        &[
            pair(
                "look.family.evergreen",
                ("evergreen_night", added::evergreen_night_tokens),
                ("evergreen_paper", added::evergreen_paper_tokens),
            ),
            dark_only("look.family.bordeaux", ("bordeaux", added::bordeaux_tokens)),
            light_only("look.family.print", ("print", added::print_tokens)),
        ],
    ),
    (
        "look.group.access",
        &[pair(
            "look.family.contrast",
            ("contrast", crate::theme_family::high_contrast_tokens),
            (
                "contrast_light",
                crate::theme_family::high_contrast_light_tokens,
            ),
        )],
    ),
];

/// Every tile, in its number's order.
fn tiles() -> impl Iterator<Item = &'static Tile> {
    GROUPS.iter().flat_map(|(_, g)| g.iter())
}

/// The number of the tile that shows preset `id` (its id is `("look-family", n)`).
pub fn tile_of(id: &str) -> Option<usize> {
    tiles().position(|t| [t.night, t.paper].iter().flatten().any(|f| f.0 == id))
}

/// Every preset a tile shows, by id, in the tiles' order.
pub fn tiled_presets() -> Vec<&'static str> {
    tiles()
        .flat_map(|t| [t.night, t.paper].into_iter().flatten().map(|f| f.0))
        .collect()
}

/// A tile's picture: a small window in each of its lights side by side (Night at the left,
/// Paper at the right; a theme in one light fills the tile) — the bar, three lanes of
/// clips and the playhead, a row of strips. A theme that tints its selection, colours its
/// readouts or edges its loop (Slate Lapis, r39:lapis2, and many of the others) shows it:
/// the second lane and strip chosen, the bar's clock, the loop over the lanes.
fn family_thumb(b: &mut crate::kit::Brush, r: Rect, faces: &[crate::kit::Tokens], k: f32) {
    let half = r.width() / faces.len().max(1) as f32;
    for (i, t) in faces.iter().enumerate() {
        let x0 = r.left() + i as f32 * half;
        let h = Rect::from_min_size(Pos2::new(x0, r.top()), Vec2::new(half, r.height()));
        b.rect(h, Fill::Solid(t.panel));
        let bar = Rect::from_min_size(h.min, Vec2::new(half, 9.0 * k));
        b.rect(bar, Fill::Solid(t.toolbar));
        b.rect(
            Rect::from_min_size(
                bar.min + Vec2::new(4.0 * k, 3.0 * k),
                Vec2::new(10.0 * k, 3.0 * k),
            ),
            Fill::Solid(t.highlight),
        );
        let tint = t.selection_tint;
        if t.readout.a() > 0 {
            // The clock: bar.beat in the first readout ink, the time in the second.
            let y = bar.top() + 3.0 * k;
            let big = Rect::from_min_size(
                Pos2::new(bar.right() - 30.0 * k, y),
                Vec2::new(14.0 * k, 3.0 * k),
            );
            b.rect(big, Fill::Solid(t.readout));
            let small = Rect::from_min_size(
                Pos2::new(big.right() + 3.0 * k, y),
                Vec2::new(8.0 * k, 3.0 * k),
            );
            b.rect(small, Fill::Solid(t.readout_or(t.text_dim, true)));
        }
        if tint.a() > 0 {
            // The chosen lane, under the second clip.
            let lane = Rect::from_min_size(
                Pos2::new(h.left(), h.top() + 21.5 * k),
                Vec2::new(half, 7.0 * k),
            );
            b.rect(
                lane,
                Fill::Solid(crate::kit::color::selected_fill(t.panel, tint, 1.0)),
            );
        }
        if t.loop_edge.a() > 0 {
            // The loop over the lanes, edged.
            b.hline(
                h.left() + half * 0.30,
                h.left() + half * 0.80,
                h.top() + 12.5 * k,
                k,
                t.loop_edge,
            );
        }
        for (j, c) in [t.hue_space, t.hue_tone, t.hue_mod].into_iter().enumerate() {
            let y = h.top() + (16.0 + j as f32 * 7.0) * k;
            let w = half * [0.42, 0.55, 0.36][j];
            b.rect(
                Rect::from_min_size(
                    Pos2::new(h.left() + (6.0 + 8.0 * j as f32) * k, y),
                    Vec2::new(w, 3.0 * k),
                ),
                Fill::Solid(crate::kit::color::mix(t.panel, c, 0.7)),
            );
        }
        // The playhead through the lanes, in each theme's own colour.
        b.vline(
            h.left() + half * 0.62,
            h.top() + 12.0 * k,
            h.bottom() - 32.0 * k,
            k,
            t.playhead,
        );
        let strips = Rect::from_min_max(Pos2::new(h.left(), h.bottom() - 30.0 * k), h.max);
        b.rect(strips, Fill::Solid(t.toolbar));
        let n = 6;
        let sw = (half - 8.0 * k) / n as f32;
        for j in 0..n {
            let sr = Rect::from_min_size(
                Pos2::new(h.left() + 4.0 * k + j as f32 * sw, strips.top() + 6.0 * k),
                Vec2::new(sw - 3.0 * k, 20.0 * k),
            );
            let fill = if j == 1 {
                crate::kit::color::selected_fill(t.strip_selected, tint, 2.0)
            } else {
                t.strip_selected
            };
            b.rect(sr, Fill::Solid(fill));
        }
    }
}

/// What a click on tile `n` does: a family sets both modes (its Night for dark, its Paper
/// for light) and leaves the mode as it is; a theme in one light takes that light's mode
/// and shows it, so the click is seen.
pub fn choose_tile(app: &mut CantusApp, n: usize) {
    let Some(tile) = tiles().nth(n) else {
        return;
    };
    match (tile.night, tile.paper) {
        (Some((night, _)), Some((paper, _))) => {
            app.appearance.assign(Mode::Dark, format!("preset:{night}"));
            app.appearance
                .assign(Mode::Light, format!("preset:{paper}"));
        }
        (Some((id, _)), None) => {
            app.appearance.assign(Mode::Dark, format!("preset:{id}"));
            app.appearance.mode = Mode::Dark;
        }
        (None, Some((id, _))) => {
            app.appearance.assign(Mode::Light, format!("preset:{id}"));
            app.appearance.mode = Mode::Light;
        }
        (None, None) => return,
    }
    app.apply_appearance();
    app.store_appearance();
}

/// Settings ▸ Appearance's top (`dir-a`): every theme as a picture, in its groups, four to
/// a row (fewer where the page is narrow), then the mode.
fn look_family(app: &mut CantusApp, ui: &mut Ui) {
    let s = Style::current();
    let k = s.k;
    w::band(ui, Src::Key("topbar.look.theme"));
    let dark_now = app.appearance.slot(Mode::Dark).base.clone();
    let light_now = app.appearance.slot(Mode::Light).base.clone();
    let gap = 14.0 * k;
    let avail = ui.available_width();
    let cols = (((avail + gap) / (132.0 * k + gap)).floor() as usize).clamp(2, 4);
    let tw = ((avail - (cols as f32 - 1.0) * gap) / cols as f32).min(200.0 * k);
    let th = (tw * 0.48).round();
    let mut n = 0usize;
    for (group, list) in GROUPS {
        let (cap, _) = ui.allocate_exact_size(Vec2::new(avail, 20.0 * k), Sense::hover());
        with_brush(ui.painter(), |b| {
            let _ = text::paint(
                b,
                cap,
                &Text::key(group, Role::Label, s.t.text_dim)
                    .align(Align2::LEFT_BOTTOM)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
        });
        for chunk in list.chunks(cols) {
            let (row, _) = ui.allocate_exact_size(Vec2::new(avail, th + 28.0 * k), Sense::hover());
            for (i, tile) in chunk.iter().enumerate() {
                let r = Rect::from_min_size(
                    Pos2::new(row.left() + i as f32 * (tw + gap), row.top() + 4.0 * k),
                    Vec2::new(tw, th),
                );
                let resp = ui.interact(r, Id::new(("look-family", n)), Sense::click());
                let resp = super::hint::said_with(resp, || t(tile.key));
                // Chosen: a family when the dark mode shows its Night (a theme in the light
                // only: when the light mode shows it).
                let chosen = match (tile.night, tile.paper) {
                    (Some((night, _)), _) => dark_now == format!("preset:{night}"),
                    (None, Some((paper, _))) => light_now == format!("preset:{paper}"),
                    (None, None) => false,
                };
                let faces: Vec<crate::kit::Tokens> = [tile.night, tile.paper]
                    .into_iter()
                    .flatten()
                    .map(|(_, make)| make())
                    .collect();
                with_brush(ui.painter(), |b| {
                    family_thumb(b, r, &faces, k);
                    if chosen {
                        b.rrect_stroke(r.expand(2.0 * k), s.r(4.0), 2.0 * k, s.t.highlight);
                    } else if resp.hovered() {
                        b.rrect_stroke(r.expand(1.0 * k), s.r(4.0), 1.0 * k, s.t.text_dim);
                    }
                    let lr = Rect::from_min_size(
                        Pos2::new(r.left(), r.bottom() + 4.0 * k),
                        Vec2::new(tw, 18.0 * k),
                    );
                    let _ = text::paint(
                        b,
                        lr,
                        &Text::key(tile.key, Role::Control, s.t.text)
                            .align(Align2::LEFT_CENTER)
                            .policy(Policy::Elide(Elide::End)),
                        s.tk(),
                    );
                });
                if resp.clicked() {
                    choose_tile(app, n);
                }
                n += 1;
            }
        }
    }
    ui.horizontal(|ui| {
        w::row_label(ui, "topbar.look.mode", LABEL_W, 24.0);
        let modes = [Mode::Dark, Mode::Light, Mode::System];
        let mut sel = modes
            .iter()
            .position(|m| *m == app.appearance.mode)
            .unwrap_or(0);
        let labels = [
            Src::Key("topbar.look.mode.night"),
            Src::Key("topbar.look.mode.paper"),
            Src::Key("topbar.look.mode.system"),
        ];
        if segmented(ui, &mut sel, &labels, &t("topbar.look.mode"), 340.0).changed() {
            app.appearance.mode = modes[sel];
            app.apply_appearance();
            app.store_appearance();
        }
    });
}

/// The taskbar icon's tints on Settings ▸ Appearance (`dir-a`): as designed, amber,
/// verdigris, blue, silver — each its colour beside its name; the appearance editor holds
/// the rest. // r39:el-topbar
fn look_icon(app: &mut CantusApp, ui: &mut Ui) {
    use crate::kit::appearance::{IconColour, IconPreset};
    let s = Style::current();
    let k = s.k;
    w::band(ui, Src::Key("topbar.look.icon"));
    let choices = [
        (IconColour::Auto, "topbar.look.icon.designed"),
        (
            IconColour::Preset(IconPreset::Amber),
            IconPreset::Amber.name_key(),
        ),
        (
            IconColour::Preset(IconPreset::Verdigris),
            IconPreset::Verdigris.name_key(),
        ),
        (
            IconColour::Preset(IconPreset::Blue),
            IconPreset::Blue.name_key(),
        ),
        (
            IconColour::Preset(IconPreset::Silver),
            IconPreset::Silver.name_key(),
        ),
    ];
    let (row, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0 * k), Sense::hover());
    let cw = (row.width() / choices.len() as f32).max(80.0 * k);
    for (i, (c, key)) in choices.into_iter().enumerate() {
        let r = Rect::from_min_size(
            Pos2::new(row.left() + i as f32 * cw, row.top()),
            Vec2::new(cw - 6.0 * k, 30.0 * k),
        );
        // The colour's name without its note ("Silver (for a dark taskbar)"): the note is
        // the hint's.
        let words = t(key);
        let short = words.split(" (").next().unwrap_or(&words);
        let resp = ui.interact(r, Id::new(("look-icon", i)), Sense::click());
        let resp = super::hint::said_with(resp, || t(key));
        let on = app.appearance.icon == c;
        let rgb = match c {
            IconColour::Preset(p) => p.rgb(),
            _ => IconPreset::Amber.rgb(),
        };
        with_brush(ui.painter(), |b| {
            let tile = Rect::from_min_size(
                Pos2::new(r.left(), r.top() + 3.0 * k),
                Vec2::splat(24.0 * k),
            );
            // The mark's own warm black tile, in either light.
            b.rrect(
                tile,
                s.r(5.0),
                Fill::Solid(crate::kit::color::mix(s.t.dark_shade, s.t.danger, 0.12)),
            );
            b.circle(
                tile.center(),
                7.0 * k,
                Fill::Solid(egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2])),
            );
            if on {
                b.rrect_stroke(tile.expand(2.0 * k), s.r(6.0), 2.0 * k, s.t.highlight);
            }
            let lr = Rect::from_min_max(Pos2::new(tile.right() + 8.0 * k, r.top()), r.max);
            let ink = if on || resp.hovered() {
                s.t.text
            } else {
                s.t.text_dim
            };
            let _ = text::paint(
                b,
                lr,
                &Text::new(short, Role::Label, ink)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
        });
        if resp.clicked() {
            app.appearance.icon = c;
            app.apply_appearance();
            app.store_appearance();
        }
    }
}

/// A dialog's side list ground (`dir-a`): the window's base, from under the title row to
/// the frame's foot, its lower left corner the frame's. // r39:el-topbar
pub fn side_ground(b: &mut crate::kit::Brush, r: Rect, s: &Style) {
    let rr = b.snap_rect(r);
    let c = s.r(10.0);
    b.rect(
        Rect::from_min_max(rr.min, egui::pos2(rr.right(), rr.bottom() - c)),
        Fill::Solid(s.t.panel),
    );
    b.rrect(
        Rect::from_min_max(egui::pos2(rr.left(), rr.bottom() - 2.0 * c), rr.max),
        c,
        Fill::Solid(s.t.panel),
    );
    b.rect(
        Rect::from_min_max(egui::pos2(rr.left() + c, rr.bottom() - 2.0 * c), rr.max),
        Fill::Solid(s.t.panel),
    );
}
