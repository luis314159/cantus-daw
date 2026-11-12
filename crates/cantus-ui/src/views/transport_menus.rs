//! The top bar's menus block (ADR 0052; round 39, `dir-a`'s phase B): the mark, the menus
//! over the project's name and its summary, in four forms as the bar narrows — full,
//! compact (no summary), minimal (the mark, a ≡ key that holds every menu, the name on the
//! bar's centre line) and bare (the mark and ≡). The project's name is renamed in place: a
//! click on it opens the kit's field there (`title`), drawn after the bar's words.

use super::layout::Brand;
use super::paint::{self as p, ink1, ink2, ink3};
use super::{title, Style};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::{shade, with_alpha};
use crate::kit::glyph::Glyph;
use crate::kit::text::Role;
use crate::kit::{with_brush, Fill, State};
use crate::views;
use crate::views::workspace::Pane;
use egui::{Align2, Id, Pos2, Rect, Sense, Ui, Vec2};
use std::cell::RefCell;

/// The mark's side, and where the menus start after it, at 100 %.
pub const MARK: f32 = 24.0;
pub const LOGO_W: f32 = 34.0;
/// The space between two menus' words, and the ≡ key's width.
const MENU_GAP: f32 = 13.0;
const MENU_KEY: f32 = 28.0;
/// The most the name and the summary take before they elide (their whole in the hint).
const NAME_MAX: f32 = 200.0;
const SUMMARY_MAX: f32 = 180.0;

thread_local! {
    /// The project's name and its dirty mark, rebuilt in place each frame.
    static NAME: RefCell<String> = RefCell::new(String::with_capacity(64));
}

const MENUS: [&str; 6] = [
    "menu.file",
    "menu.edit",
    "menu.clip",
    "menu.view",
    "ship.menu.options",
    "menu.help",
];

/// The menus' words: 12 Regular in ink 2, in title case (`dir-a`).
const MENU_PX: f32 = 12.0;

fn menu_word_width(ctx: &egui::Context, s: &Style, key: &str) -> f32 {
    p::measure_key(ctx, key, Role::Body, MENU_PX, s)
}

/// The menus' width in a row, their gaps between them.
pub fn left_widths(ctx: &egui::Context, s: &Style) -> f32 {
    let words: f32 = MENUS.iter().map(|k| menu_word_width(ctx, s, k)).sum();
    words + MENU_GAP * s.k * (MENUS.len() - 1) as f32
}

/// The name as the bar shows it (with the unsaved mark) into `out`.
fn name_into(app: &CantusApp, out: &mut String) {
    out.clear();
    out.push_str(&app.project().name);
    if app.bus.is_dirty() {
        out.push_str(" \u{2022}");
    }
}

/// The summary under the name, made again only when the song's facts change.
fn with_summary<R>(app: &CantusApp, f: impl FnOnce(&str) -> R) -> R {
    SUMMARY.with(|c| {
        let mut c = c.borrow_mut();
        let pr = app.project();
        let key = (pr.tracks.len(), pr.strips.len(), pr.content_end().0);
        if c.0 != Some(key) {
            c.0 = Some(key);
            c.1 = summary(app);
        }
        f(&c.1)
    })
}

/// The menus block's width in each form ([`Brand`] order: full, compact, minimal, bare).
pub fn brand_widths(app: &CantusApp, ctx: &egui::Context, s: &Style) -> [f32; 4] {
    let k = s.k;
    let menus = left_widths(ctx, s);
    let name = NAME.with(|n| {
        let mut n = n.borrow_mut();
        name_into(app, &mut n);
        p::measure(ctx, &n, Role::Title, 13.0, s).min(NAME_MAX * k)
    });
    let summary =
        with_summary(app, |w| p::measure(ctx, w, Role::Label, 11.0, s)).min(SUMMARY_MAX * k);
    let mark = (MARK + 8.0 + MENU_KEY + 8.0) * k;
    [
        (LOGO_W * k + menus.max(name + 8.0 * k + summary)).ceil(),
        (LOGO_W * k + menus.max(name)).ceil(),
        (mark + name).ceil(),
        (mark - 8.0 * k).ceil(),
    ]
}

/// The menus block in `rect` (the bar's height) in `form`. Returns the name's line, where
/// the rename field goes (drawn last).
pub fn left_block(
    app: &mut CantusApp,
    ui: &mut Ui,
    rect: Rect,
    form: Brand,
    s: &Style,
) -> Option<Rect> {
    let k = s.k;
    let top = rect.top();
    let logo_r = Rect::from_min_size(
        Pos2::new(rect.left(), top + (rect.height() - MARK * k) * 0.5),
        Vec2::splat(MARK * k),
    );
    views::workspace_widgets::paint_logo(ui.painter(), logo_r, app.appearance.logo, &s.t);
    let mid = rect.center().y;
    match form {
        Brand::Full | Brand::Compact => {
            // Menus on the upper line (baseline 19 of 44), the name under them (36).
            let x = rect.left() + LOGO_W * k;
            let menu_r = Rect::from_min_max(
                Pos2::new(x, mid - 17.0 * k),
                Pos2::new(rect.right(), mid - 1.0 * k),
            );
            menus(app, ui, menu_r, s);
            let line = Rect::from_min_max(
                Pos2::new(x, mid + 1.0 * k),
                Pos2::new(rect.right(), mid + 17.0 * k),
            );
            name_line(app, ui, line, form == Brand::Full, s);
            Some(line)
        }
        Brand::Minimal | Brand::Bare => {
            let kr = Rect::from_min_size(
                Pos2::new(logo_r.right() + 8.0 * k, mid - 12.0 * k),
                Vec2::new(MENU_KEY * k, 24.0 * k),
            );
            menu_key(app, ui, kr, s);
            if form == Brand::Bare {
                return None;
            }
            let line = Rect::from_min_max(
                Pos2::new(kr.right() + 8.0 * k, mid - 9.0 * k),
                Pos2::new(rect.right(), mid + 9.0 * k),
            );
            name_line(app, ui, line, false, s);
            Some(line)
        }
    }
}

/// The name (13 Medium, ink 1) and, when `summary`, the song's summary after it (11, ink 3).
fn name_line(app: &CantusApp, ui: &Ui, line: Rect, summary: bool, s: &Style) {
    if title::naming(app) {
        return;
    }
    let ctx = ui.ctx().clone();
    NAME.with(|n| {
        let mut n = n.borrow_mut();
        name_into(app, &mut n);
        let nw = p::measure(&ctx, &n, Role::Title, 13.0, s).min(line.width());
        with_brush(ui.painter(), |b| {
            let nr = Rect::from_min_size(line.min, Vec2::new(nw + 1.0, line.height()));
            p::words(
                b,
                nr,
                &n,
                Role::Title,
                13.0,
                ink1(s),
                Align2::LEFT_CENTER,
                s,
            );
            if summary {
                let sr =
                    Rect::from_min_max(Pos2::new(nr.right() + 8.0 * s.k, line.top()), line.max);
                if sr.width() > 20.0 * s.k {
                    with_summary(app, |w| {
                        p::words(b, sr, w, Role::Label, 11.0, ink3(s), Align2::LEFT_CENTER, s)
                    });
                }
            }
        });
    });
}

/// The menu words: each opening its menu, the open one and the hovered one lifted.
fn menus(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let k = s.k;
    let mut x = r.left() - MENU_GAP * k * 0.5;
    for (i, key) in MENUS.iter().enumerate() {
        let w = menu_word_width(ui.ctx(), s, key) + MENU_GAP * k;
        let rect = Rect::from_min_size(Pos2::new(x, r.top()), Vec2::new(w, r.height()));
        x += w;
        if rect.right() > r.right() + MENU_GAP * k {
            break;
        }
        let resp = ui.interact(rect, Id::new(("toolbar-menu", i)), Sense::click());
        // The help line (round 39, ADR 0109): which menu this word opens.
        if resp.hovered() {
            crate::kit::hint::set_text(
                ui.ctx(),
                &tf("ux.hint.menu", &[("menu", &t(key))]),
                "",
                crate::kit::hint::Caps {
                    menu: true,
                    ..crate::kit::hint::Caps::NONE
                },
            );
        }
        let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp));
        let build = open || resp.clicked();
        with_brush(ui.painter(), |b| {
            if resp.hovered() || open {
                b.rrect(
                    b.snap_rect(rect.shrink2(Vec2::new(2.0 * k, 0.0))),
                    s.r(3.0),
                    Fill::Solid(with_alpha(s.t.text, if open { 0.12 } else { 0.07 })),
                );
            }
            let ink = if resp.hovered() || open {
                ink1(s)
            } else {
                ink2(s)
            };
            p::words_key(
                b,
                rect,
                key,
                Role::Body,
                MENU_PX,
                ink,
                Align2::CENTER_CENTER,
                s,
            );
        });
        // A closed menu costs nothing: its popup is built only while it shows.
        if build {
            egui::Popup::menu(&resp)
                .style(menu_look)
                .show(|ui| menu_body(app, ui, i));
        }
    }
}

/// The ≡ key of the minimal form: every menu as a submenu.
fn menu_key(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let resp = ui.interact(r, Id::new("toolbar-menus-all"), Sense::click());
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp));
    let st = State::of(&resp, false);
    with_brush(ui.painter(), |b| {
        p::well(b, r, s, None);
        p::tint(b, r, s, st, s.t.highlight, 0.2);
        let st = if open {
            State {
                hovered: true,
                ..st
            }
        } else {
            st
        };
        p::icon(b, r, s, Glyph::Menu, st, s.t.highlight, 20.0);
    });
    if resp.hovered() {
        crate::kit::hint::set_text(
            ui.ctx(),
            &t("topbar.menus_all"),
            "",
            crate::kit::hint::Caps {
                menu: true,
                ..crate::kit::hint::Caps::NONE
            },
        );
    }
    if open || resp.clicked() {
        egui::Popup::menu(&resp).style(menu_look).show(|ui| {
            for (i, key) in MENUS.iter().enumerate() {
                ui.menu_button(t(key), |ui| menu_body(app, ui, i));
            }
        });
    }
}

/// One of the bar's menus, kept inside the window: one taller than it scrolls (the owner,
/// 2026-11-11: View ran off the foot of a 1080-px screen with nothing to scroll).
fn menu_body(app: &mut CantusApp, ui: &mut Ui, i: usize) {
    crate::kit::menu::fit(ui, |ui| match i {
        0 => file_menu(app, ui),
        1 => edit_menu(app, ui),
        2 => clip_menu(app, ui),
        3 => view_menu(app, ui),
        4 => options_menu(app, ui),
        _ => help_menu(app, ui),
    });
}

/// The menus' look (ADR 0024 §4, FL 2026): the kit's menu face, 22-pt rows, the hover
/// in the menu's own colour, text in the body role.
pub(super) fn menu_look(style: &mut egui::Style) {
    egui::containers::menu::menu_style(style);
    let s = Style::current();
    let t = s.t;
    let v = &mut style.visuals;
    v.window_fill = t.menu;
    v.panel_fill = t.menu;
    v.window_stroke = egui::Stroke::new(1.0, shade(t.menu, -0.22));
    v.menu_corner_radius = egui::CornerRadius::same(s.r(4.0).round() as u8);
    v.override_text_color = Some(t.menu_text);
    v.weak_text_color = Some(with_alpha(t.menu_text, 0.72));
    for w in [&mut v.widgets.inactive, &mut v.widgets.noninteractive] {
        w.weak_bg_fill = egui::Color32::TRANSPARENT;
        w.bg_fill = shade(t.menu, -0.06);
        w.fg_stroke.color = t.menu_text;
        w.bg_stroke = egui::Stroke::new(1.0, shade(t.menu, -0.16));
    }
    for w in [
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.weak_bg_fill = t.menu_hover;
        w.bg_fill = t.menu_hover;
        w.fg_stroke.color = t.menu_text_hover;
        w.bg_stroke = egui::Stroke::NONE;
    }
    style.spacing.button_padding = Vec2::new(10.0, 3.0);
    style.spacing.interact_size.y = 22.0;
    style.spacing.menu_margin = egui::Margin::same(4);
    style.override_font_id = Some(Role::Body.font(s.tk()));
}
/// What a row of one of the bar's menus does.
#[derive(Clone)]
enum Do {
    Run(crate::keymap::Action),
    Command(crate::menu_commands::MenuCommand),
    Undo,
    Redo,
    Shortcuts,
    About,
    ExportVideo,
    Pane(Pane),
    TempoLane,
    Layout(String),
    Appearance,
    Settings,
    Customise,
    KeyboardPiano,
    Stats,
    Band,
    Separator,
}

/// One row of a bar menu: its words, its check, the action whose key it shows, whether it
/// can be chosen now, and what it does.
struct Row {
    words: Words,
    check: crate::kit::menu::Check,
    key: Option<crate::keymap::Action>,
    enabled: bool,
    what: Do,
}

/// A row's words: a string key, or text made now (an undo's name, a layout's name).
enum Words {
    Key(&'static str),
    Text(String),
}

impl Row {
    fn new(words: &'static str, what: Do) -> Row {
        Row {
            words: Words::Key(words),
            check: crate::kit::menu::Check::None,
            key: match what {
                Do::Run(a) => Some(a),
                _ => None,
            },
            enabled: true,
            what,
        }
    }
    fn run(action: crate::keymap::Action, words: &'static str) -> Row {
        Row::new(words, Do::Run(action))
    }
    fn command(c: crate::menu_commands::MenuCommand) -> Row {
        Row::new(c.key(), Do::Command(c))
    }
    fn separator() -> Row {
        Row::new("", Do::Separator)
    }
    fn band(words: Words) -> Row {
        Row {
            words,
            ..Row::new("", Do::Band)
        }
    }
    fn checked(mut self, on: bool) -> Row {
        self.check = if on {
            crate::kit::menu::Check::On
        } else {
            crate::kit::menu::Check::Off
        };
        self
    }
    fn key(mut self, action: crate::keymap::Action) -> Row {
        self.key = Some(action);
        self
    }
    fn enabled(mut self, on: bool) -> Row {
        self.enabled = on;
        self
    }
    fn text(mut self, words: String) -> Row {
        self.words = Words::Text(words);
        self
    }
}

/// A bar menu's rows as one kit menu (round 39's light rows, one frame, each action's key
/// on the right) — every bar menu alike. They were egui's own buttons and check boxes and a
/// kit menu per row: rows of two heights and boxed entries, until the owner's screen showed
/// them (2026-11-11). A click closes the menu and does what the row says.
fn bar_menu(app: &mut CantusApp, ui: &mut Ui, rows: &[Row]) {
    bar_menu_at(app, ui, rows, 240.0);
}

/// [`bar_menu`] at least `width` wide (a second list under the first, as wide as it).
fn bar_menu_at(app: &mut CantusApp, ui: &mut Ui, rows: &[Row], width: f32) {
    let Some(what) = bar_list(app, ui, rows, width) else {
        return;
    };
    ui.close();
    let ctx = ui.ctx().clone();
    run_row(app, &ctx, what);
}

/// [`bar_menu`]'s list alone: the row chosen, if one was.
fn bar_list(app: &CantusApp, ui: &mut Ui, rows: &[Row], width: f32) -> Option<Do> {
    use crate::kit::menu::Item;
    use crate::kit::text::Src;
    let keys: Vec<Option<String>> = rows
        .iter()
        .map(|r| {
            r.key
                .and_then(|a| app.keymap.bindings_for(a).first().map(|b| b.display()))
        })
        .collect();
    let items: Vec<Item<'_>> = rows
        .iter()
        .zip(&keys)
        .map(|(r, k)| {
            let label = match &r.words {
                Words::Key(key) => Src::Key(key),
                Words::Text(text) => Src::Str(text),
            };
            match r.what {
                Do::Band => Item::Band(label),
                Do::Separator => Item::Separator,
                _ => Item::Action {
                    label,
                    check: r.check,
                    shortcut: k.as_deref(),
                    submenu: false,
                    enabled: r.enabled,
                },
            }
        })
        .collect();
    let clicked = crate::kit::widgets::menu_list(ui, &items, width)
        .1
        .clicked?;
    let row = &rows[clicked];
    row.enabled.then(|| row.what.clone())
}

fn run_row(app: &mut CantusApp, ctx: &egui::Context, what: Do) {
    match what {
        Do::Run(a) => app.run_action(a),
        Do::Command(c) => c.run(app, ctx),
        Do::Undo => app.undo(),
        Do::Redo => app.redo(),
        Do::Shortcuts => app.show_shortcuts = true,
        Do::About => app.show_about = true,
        Do::ExportVideo => crate::video_export::open(app),
        Do::Pane(pane) => app.toggle_pane(pane),
        Do::TempoLane => app.toggle_tempo_lane(),
        Do::Layout(name) => app.apply_layout(&name),
        Do::Appearance => app.show_theme_editor = true,
        Do::Settings => app.show_settings = true,
        Do::Customise => super::more::open_customise(app),
        Do::KeyboardPiano => app.keyboard_piano = !app.keyboard_piano,
        Do::Stats => app.show_stats = !app.show_stats,
        Do::Band | Do::Separator => {}
    }
}

fn file_menu(app: &mut CantusApp, ui: &mut Ui) {
    use crate::keymap::Action;
    use crate::menu_commands::MenuCommand as M;
    // r39:ux-b — every row that is an action shows its key (ADR 0110); the rest run the
    // commands the palette offers under the same words.
    let rows = [
        Row::run(Action::NewProject, "menu.new"),
        // Round 31 (ADR 0072): the start page, with its templates and the recent projects.
        Row::run(Action::NewFromTemplate, "action.new-from-template"),
        Row::run(Action::DemoSongs, "demos.menu"),
        Row::run(Action::Open, "menu.open"),
        Row::separator(),
        Row::run(Action::Save, "menu.save"),
        Row::run(Action::SaveAs, "menu.save_as"),
        Row::run(Action::SaveAsTemplate, "action.save-as-template"),
        Row::run(Action::SaveTrackTemplate, "action.save-track-template"),
        // Audio (ADR 0025): every file of the project into its folder.
        Row::command(M::CollectAndSave),
        Row::separator(),
        Row::command(M::ImportAudio),
        // r35:import — for the selected dancer; with none, the status says what to do.
        Row::command(M::ImportCharacter),
        Row::run(Action::ExportWav, "menu.export_wav"),
        // r35:video
        Row::new("video.menu", Do::ExportVideo),
        // Round 17: MIDI, stems (ADR 0033) and DAWproject.
        Row::separator(),
        Row::command(M::ImportMidi),
        Row::command(M::ExportMidi),
        Row::command(M::ExportStems).enabled(!app.export.busy()),
        Row::separator(),
        Row::command(M::ImportDawProject),
        Row::command(M::ExportDawProject),
        Row::separator(),
        Row::command(M::Quit),
    ];
    bar_menu(app, ui, &rows);
}

fn edit_menu(app: &mut CantusApp, ui: &mut Ui) {
    use crate::keymap::Action;
    let undo = app.bus.undo_label_key();
    let redo = app.bus.redo_label_key();
    // Through `run_action`, not straight to the note methods: these mean one thing in the
    // piano roll and another in the arrangement (ADR 0010).
    let rows = [
        Row::new("menu.undo", Do::Undo)
            .text(undo.map_or_else(
                || t("menu.undo"),
                |k| tf("menu.undo_named", &[("what", &t(k))]),
            ))
            .key(Action::Undo)
            .enabled(undo.is_some()),
        Row::new("menu.redo", Do::Redo)
            .text(redo.map_or_else(
                || t("menu.redo"),
                |k| tf("menu.redo_named", &[("what", &t(k))]),
            ))
            .key(Action::Redo)
            .enabled(redo.is_some()),
        Row::separator(),
        Row::run(Action::Cut, "menu.cut"),
        Row::run(Action::Copy, "menu.copy"),
        Row::run(Action::Paste, "menu.paste"),
        Row::run(Action::Duplicate, "menu.duplicate"),
        Row::separator(),
        Row::run(Action::SelectAll, "menu.select_all"),
        Row::run(Action::Delete, "menu.delete"),
        Row::run(Action::Quantize, "menu.quantize"),
        // The selected strip's route menu (round 25).
        Row::separator(),
        Row::run(Action::RouteMenu, "routing.menu.route_menu"),
    ];
    bar_menu(app, ui, &rows);
}

/// The area a View menu key shows or hides (what its check says), for a controller's
/// light (round 29, review 3 of ADR 0062, #48).
pub fn pane_of(action: crate::keymap::Action) -> Option<Pane> {
    use crate::keymap::Action;
    Some(match action {
        Action::ViewArrangement => Pane::Arrangement,
        Action::ViewChannelRack => Pane::ChannelRack,
        Action::ViewPianoRoll => Pane::PianoRoll,
        Action::ViewMixer => Pane::Mixer,
        Action::ToggleBrowser => Pane::Browser,
        Action::ViewRouting => Pane::Routing,
        Action::ViewControllers => Pane::Controllers,
        Action::ViewHistory => Pane::History,
        Action::ViewScore => Pane::Score,
        Action::ToggleSessionView => Pane::Session, // r33:launcher
        Action::ToggleInspector => Pane::Inspector,
        // The clips list, which the View menu checks too (review 4, #26).
        Action::ToggleClips => Pane::Clips,
        _ => return None,
    })
}

/// The **View** menu: every area with its check and its key, the tempo track, the stage, the
/// devices and the clip browser, the layouts in a band of their own (no submenu), the
/// interface's zoom, and the settings.
fn view_menu(app: &mut CantusApp, ui: &mut Ui) {
    use crate::keymap::Action;
    use crate::menu_commands::MenuCommand as M;
    let mut rows: Vec<Row> = Vec::new();
    // Every area, with its key: the same toggles as the toolbar's window keys, the Routing
    // tab after the mixer (round 25), the controller mappings (round 29, ADR 0062), the undo
    // history (round 31, ADR 0072), the score (round 32, ADR 0075), the session view
    // (round 33).
    let areas = std::iter::once(Pane::Browser).chain(Pane::ZONES).chain([
        Pane::Routing,
        Pane::Controllers,
        Pane::History,
        Pane::Score,
        Pane::Session,
    ]);
    for pane in areas {
        let action = match pane {
            Pane::Arrangement => Action::ViewArrangement,
            Pane::ChannelRack => Action::ViewChannelRack,
            Pane::PianoRoll => Action::ViewPianoRoll,
            Pane::Mixer => Action::ViewMixer,
            Pane::Browser => Action::ToggleBrowser,
            Pane::Routing => Action::ViewRouting,
            Pane::Controllers => Action::ViewControllers,
            Pane::History => Action::ViewHistory,
            Pane::Score => Action::ViewScore,
            Pane::Session => Action::ToggleSessionView,
            _ => Action::ToggleInspector,
        };
        rows.push(
            Row::new(pane.title_key(), Do::Pane(pane))
                .checked(app.workspace.contains(pane))
                .key(action),
        );
    }
    // The arrangement's tempo track (round 29, ADR 0060).
    rows.push(
        Row::new("tempo.view_menu", Do::TempoLane)
            .checked(app.tempo_lane_shown())
            .key(Action::ToggleTempoTrack),
    );
    // The stage, its companion window and streaming (round 35).
    let stage = &app.settings.stage;
    rows.push(Row::run(Action::ToggleStage, "stage.view.stage").checked(!stage.hidden));
    rows.push(
        Row::run(Action::ToggleCompanion, "stage.view.companion").checked(stage.companion_open),
    );
    rows.push(Row::run(Action::ToggleStreaming, "stage.view.streaming").checked(stage.streaming));
    // The devices panel (round 20), the clip browser (round 26), the full device picker and
    // the command palette (round 31, ADR 0072).
    rows.push(
        Row::new("picker.menu.view_devices", Do::Pane(Pane::Devices))
            .checked(app.workspace.contains(Pane::Devices)),
    );
    rows.push(
        Row::new("action.toggle-clips", Do::Pane(Pane::Clips))
            .checked(app.workspace.contains(Pane::Clips)),
    );
    rows.push(Row::command(M::DevicePicker));
    rows.push(Row::run(Action::CommandPalette, "action.command-palette"));
    // The layouts (round 30): the saved ones to choose, then save, delete and reset.
    rows.push(Row::separator());
    rows.push(Row::band(Words::Key("menu.layout")));
    let current = app.layouts.current.name.clone();
    for layout in &app.layouts.saved {
        let mut row = Row::new("", Do::Layout(layout.name.clone())).text(layout.name.clone());
        row.check = crate::kit::menu::Check::Radio(layout.name == current);
        rows.push(row);
    }
    let is_saved = app.layouts.saved.iter().any(|l| l.name == current);
    rows.push(Row::command(M::SaveLayoutAs));
    rows.push(Row::command(M::DeleteLayout).enabled(is_saved));
    rows.push(Row::command(M::ResetLayout));
    rows.push(Row::separator());
    for (action, key) in [
        (Action::ZoomIn, "workspace.zoom_in"),
        (Action::ZoomOut, "workspace.zoom_out"),
        (Action::ZoomReset, "workspace.zoom_reset"),
        (Action::PickElement, "theme.inspect"),
        (Action::CloseFloating, "workspace.close_floating"),
    ] {
        rows.push(Row::run(action, key));
    }
    rows.push(Row::separator());
    rows.push(Row::new("settings.appearance", Do::Appearance));
    rows.push(Row::new("menu.settings", Do::Settings));
    // Round 28 (ADR 0052): which groups the top bar shows, and in what order.
    rows.push(Row::new("topbar.customise", Do::Customise));
    rows.push(Row::new("menu.keyboard_piano", Do::KeyboardPiano).checked(app.keyboard_piano));
    rows.push(Row::new("menu.stats", Do::Stats).checked(app.show_stats));
    bar_menu(app, ui, &rows);
}

/// Open the top bar's menu `i` (0 File … 4 Options, 5 Help) — the screenshots.
pub fn open_menu(ctx: &egui::Context, i: usize) {
    egui::Popup::open_id(ctx, Id::new(("toolbar-menu", i)).with("popup"));
}

/// FL's *Options* (round 30): the entries of the menu model the Mac's bar shows too
/// (`crate::macos::menu::options`), so the two never differ.
fn options_menu(app: &mut CantusApp, ui: &mut Ui) {
    use crate::kit::menu::{Check, Item};
    use crate::kit::text::Src;
    use crate::macos::menu::{self as model, Entry, Pick};
    let menu = model::options(&app.keymap);
    // One kit menu, as the editors' (FL's light rows), each action's key at the right.
    let keys: Vec<Option<String>> = menu
        .entries
        .iter()
        .map(|e| match e {
            Entry::Item {
                pick: Pick::Action(a),
                ..
            } => app.keymap.bindings_for(*a).first().map(|b| b.display()),
            _ => None,
        })
        .collect();
    let items: Vec<Item<'_>> = menu
        .entries
        .iter()
        .zip(&keys)
        .map(|(e, k)| match e {
            Entry::Item { label, .. } | Entry::System { label, .. } => Item::Action {
                label: Src::Str(label),
                check: Check::None,
                shortcut: k.as_deref(),
                submenu: false,
                enabled: true,
            },
            Entry::Separator => Item::Separator,
        })
        .collect();
    let clicked = crate::kit::widgets::menu_list(ui, &items, 230.0).1.clicked;
    if let Some(Entry::Item { pick, .. }) = clicked.map(|i| &menu.entries[i]) {
        let pick = *pick;
        ui.close();
        let ctx = ui.ctx().clone();
        model::run(app, &ctx, pick);
    }
}

fn help_menu(app: &mut CantusApp, ui: &mut Ui) {
    // The manual (the copy installed beside the program, in the interface's language),
    // the release notes and GitHub's forms for a bug or an idea (round 22).
    use crate::menu_commands::MenuCommand as M;
    let rows = [
        Row::command(M::Manual),
        Row::command(M::ReleaseNotes),
        Row::new("menu.shortcuts", Do::Shortcuts),
        Row::separator(),
        Row::command(M::ReportBug),
        Row::command(M::SuggestFeature),
        Row::separator(),
        Row::command(M::CheckForUpdates),
        Row::new("menu.about", Do::About),
        // About, scrolled to the credits: the Bible translations and every sound pack
        // (round 26).
        Row::command(M::Credits),
    ];
    bar_menu(app, ui, &rows);
}

/// The **Clip** menu: everything goes through `run_action`, and every entry shows the key
/// bound to it, so the menu teaches the shortcut instead of replacing it. Two kit lists
/// round the clip colours' swatches.
fn clip_menu(app: &mut CantusApp, ui: &mut Ui) {
    use crate::keymap::Action;
    use crate::menu_commands::MenuCommand as M;
    // FL's Tools ▸ Last tweaked ▸ Create automation clip, here where clips are made
    // (ADR 0027): enabled once a parameter has been moved, and saying which; FL's *Link to
    // controller* for the same parameter (round 29, review 1 of ADR 0062).
    let tweaked = app
        .automation
        .last_tweaked
        .clone()
        .map(|t| crate::automation::target_name(app.project(), &t));
    let selection = app.clip_selection();
    let any = !selection.is_empty();
    let (muted, looping) = selection
        .first()
        .and_then(|(t, c)| app.project().track(*t)?.clip(*c))
        .map(|c| (c.muted, c.content_length.is_some()))
        .unwrap_or((false, false));
    let mut rows = vec![
        Row::run(Action::AutomateLastTweaked, "auto.menu.last_tweaked").enabled(tweaked.is_some()),
        Row::run(Action::LinkLastTweaked, "action.link-last-tweaked").enabled(tweaked.is_some()),
    ];
    if let Some(name) = tweaked {
        rows.push(Row::band(Words::Text(name)));
    }
    rows.push(Row::separator());
    // The whole menu is about the clip selection; with none there is nothing to enable.
    for (action, key) in [
        (Action::SplitClip, "clip.split"),
        (Action::GlueClips, "clip.glue"),
        (Action::Duplicate, "clip.duplicate"),
        // Linked clips (ADR 0037).
        (Action::MakeUnique, "clip.make_unique"),
        (Action::LinkClips, "action.link-clips"),
        (Action::SelectLinked, "clip.select_linked"),
        (Action::LinkTo, "clip.link_to"),
    ] {
        rows.push(Row::run(action, key).enabled(any));
    }
    rows.push(Row::separator());
    let mute_key = if muted { "clip.unmute" } else { "clip.mute" };
    rows.push(Row::run(Action::ToggleClipMute, mute_key).enabled(any));
    let loop_key = if looping {
        "clip.loop_off"
    } else {
        "clip.loop_on"
    };
    rows.push(Row::run(Action::ToggleClipLoop, loop_key).enabled(any));
    rows.push(Row::separator());
    bar_menu(app, ui, &rows);
    let width = ui.min_rect().width();
    // The clip's colour: the palette's swatches, then the track's own.
    ui.add_enabled_ui(any, |ui| {
        ui.horizontal(|ui| {
            let name = t("clip.color");
            for c in cantus_core::Color::PALETTE {
                let col = crate::theme::track_color(c);
                let r = views::workspace_widgets::swatch(ui, col, Vec2::splat(16.0), false, &name);
                if r.clicked() {
                    app.set_clip_color(Some(c));
                    ui.close();
                }
            }
        });
    });
    let rows = [
        Row::command(M::ClipTrackColour).enabled(any),
        Row::separator(),
        Row::command(M::DeleteClips).enabled(any),
    ];
    bar_menu_at(app, ui, &rows, width);
}

/// What a summary line was made from: the tracks, the strips, the song's end.
type SummaryKey = Option<(usize, usize, i64)>;

thread_local! {
    /// The summary line and what it was made from: made again only when that changes (no
    /// allocation a frame).
    static SUMMARY: RefCell<(SummaryKey, String)> = const { RefCell::new((None, String::new())) };
}

/// The song in a line under its name: its tracks, its bars and its
/// length. // r37:finish
pub fn summary(app: &CantusApp) -> String {
    let p = app.project();
    let end = p.content_end();
    let bar = views::bars::bar_len(p, cantus_core::Ticks(0)).0.max(1);
    let bars = (end.0 + bar - 1) / bar;
    let secs = views::bars::seconds_between(p, cantus_core::Ticks(0), end).max(0.0) as u64;
    crate::i18n::tf(
        "topbar.summary_line",
        &[
            ("tracks", &p.tracks.len().to_string()),
            ("bars", &bars.to_string()),
            ("time", &format!("{}:{:02}", secs / 60, secs % 60)),
        ],
    )
}

#[cfg(test)]
mod summary_tests {
    /// The song's summary line under its name: its tracks, bars and length (`dir-a`:
    /// "9 tracks · 64 bars · 2:03").
    #[test]
    fn the_summary_counts_the_song() {
        crate::i18n::set_thread_locale(crate::i18n::Locale::En);
        let app = crate::CantusApp::from_parts(
            Some(crate::starter_project()),
            cantus_engine::EngineHandle::dummy(),
            false,
        );
        let line = super::summary(&app);
        let p = app.project();
        assert!(
            line.starts_with(&format!("{} tracks \u{b7} ", p.tracks.len())),
            "{line}"
        );
        assert!(
            line.contains(" bars \u{b7} ") && line.contains(':'),
            "{line}"
        );
    }
}
