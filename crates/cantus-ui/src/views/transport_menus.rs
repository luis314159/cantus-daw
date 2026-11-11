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

fn menu_body(app: &mut CantusApp, ui: &mut Ui, i: usize) {
    match i {
        0 => file_menu(app, ui),
        1 => edit_menu(app, ui),
        2 => clip_menu(app, ui),
        3 => view_menu(app, ui),
        4 => options_menu(app, ui),
        _ => help_menu(app, ui),
    }
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
fn file_menu(app: &mut CantusApp, ui: &mut Ui) {
    use crate::keymap::Action;
    use crate::menu_commands::MenuCommand as M;
    // r39:ux-b — every row that is an action shows its key (ADR 0110); the rest run the
    // commands the palette offers under the same words.
    let command = |app: &mut CantusApp, ui: &mut Ui, c: M| {
        if views::hint::button(ui, t(c.key())).clicked() {
            ui.close();
            let ctx = ui.ctx().clone();
            c.run(app, &ctx);
        }
    };
    if views::arrangement::action_item(app, ui, Action::NewProject, "menu.new") {
        ui.close();
    }
    // Round 31 (ADR 0072): the start page, with its templates and the recent projects.
    if views::arrangement::action_item(
        app,
        ui,
        crate::keymap::Action::NewFromTemplate,
        "action.new-from-template",
    ) {
        ui.close();
    }
    for (action, key) in [
        (Action::DemoSongs, "demos.menu"),
        (Action::Open, "menu.open"),
    ] {
        if views::arrangement::action_item(app, ui, action, key) {
            ui.close();
        }
    }
    ui.separator();
    for (action, key) in [
        (Action::Save, "menu.save"),
        (Action::SaveAs, "menu.save_as"),
    ] {
        if views::arrangement::action_item(app, ui, action, key) {
            ui.close();
        }
    }
    for (action, key) in [
        (
            crate::keymap::Action::SaveAsTemplate,
            "action.save-as-template",
        ),
        (
            crate::keymap::Action::SaveTrackTemplate,
            "action.save-track-template",
        ),
    ] {
        if views::arrangement::action_item(app, ui, action, key) {
            ui.close();
        }
    }
    // Audio (ADR 0025): files in as clips, and every file of the project into its folder.
    command(app, ui, M::CollectAndSave);
    ui.separator();
    command(app, ui, M::ImportAudio);
    crate::dance_import::file_menu_item(app, ui); // r35:import
    if views::arrangement::action_item(app, ui, Action::ExportWav, "menu.export_wav") {
        ui.close();
    }
    crate::video_export::menu_item(app, ui); // r35:video — File ▸ Export video…
    views::interop::file_menu(app, ui);
    ui.separator();
    command(app, ui, M::Quit);
}

fn edit_menu(app: &mut CantusApp, ui: &mut Ui) {
    let undo = app
        .bus
        .undo_label_key()
        .map(|k| tf("menu.undo_named", &[("what", &t(k))]));
    let redo = app
        .bus
        .redo_label_key()
        .map(|k| tf("menu.redo_named", &[("what", &t(k))]));
    if ui
        .add_enabled(
            undo.is_some(),
            egui::Button::new(undo.unwrap_or_else(|| t("menu.undo"))),
        )
        .clicked()
    {
        ui.close();
        app.undo();
    }
    if ui
        .add_enabled(
            redo.is_some(),
            egui::Button::new(redo.unwrap_or_else(|| t("menu.redo"))),
        )
        .clicked()
    {
        ui.close();
        app.redo();
    }
    ui.separator();
    // Through `run_action`, not straight to the note methods: these mean one thing in the
    // piano roll and another in the arrangement (ADR 0010).
    use crate::keymap::Action;
    for (action, key) in [
        (Action::Cut, "menu.cut"),
        (Action::Copy, "menu.copy"),
        (Action::Paste, "menu.paste"),
        (Action::Duplicate, "menu.duplicate"),
    ] {
        if views::arrangement::action_item(app, ui, action, key) {
            ui.close();
        }
    }
    ui.separator();
    for (action, key) in [
        (Action::SelectAll, "menu.select_all"),
        (Action::Delete, "menu.delete"),
        (Action::Quantize, "menu.quantize"),
    ] {
        if views::arrangement::action_item(app, ui, action, key) {
            ui.close();
        }
    }
    // The selected strip's route menu (round 25): with its key, as every item here.
    ui.separator();
    if views::arrangement::action_item(app, ui, Action::RouteMenu, "routing.menu.route_menu") {
        ui.close();
    }
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

fn view_menu(app: &mut CantusApp, ui: &mut Ui) {
    use crate::keymap::Action;
    // Every area, with its key: the same toggles as the toolbar's window keys, and the
    // Routing tab after the mixer (round 25).
    let areas = std::iter::once(Pane::Browser)
        .chain(Pane::ZONES)
        .chain(std::iter::once(Pane::Routing))
        // The controller mappings (round 29, ADR 0062).
        .chain(std::iter::once(Pane::Controllers))
        // The undo history (round 31, ADR 0072).
        .chain(std::iter::once(Pane::History))
        // The Score pane (round 32, ADR 0075).
        .chain(std::iter::once(Pane::Score)) // r33:launcher (its `;` moved below)
        .chain(std::iter::once(Pane::Session)); // r33:launcher
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
            Pane::Session => Action::ToggleSessionView, // r33:launcher
            _ => Action::ToggleInspector,
        };
        let mut shown = app.workspace.contains(pane);
        let shortcut = app
            .keymap
            .bindings_for(action)
            .first()
            .map(|b| b.display())
            .unwrap_or_default();
        if ui
            .checkbox(&mut shown, format!("{}    {shortcut}", t(pane.title_key())))
            .changed()
        {
            app.toggle_pane(pane);
        }
    }
    // The arrangement's tempo track (round 29, ADR 0060).
    crate::tempo_ui::view_menu_entry(app, ui);
    crate::stage::view_menu(app, ui); // r35:stage — View ▸ Stage, Companion window, Streaming mode
                                      // The devices panel (round 20) and the full device picker.
    let mut shown = app.workspace.contains(Pane::Devices);
    if ui
        .checkbox(&mut shown, t("picker.menu.view_devices"))
        .changed()
    {
        app.toggle_pane(Pane::Devices);
    }
    // The clip browser (round 26): link groups and clips to drag onto the arrangement.
    let mut shown = app.workspace.contains(Pane::Clips);
    if views::hint::said(
        ui.checkbox(&mut shown, t("action.toggle-clips")),
        &t("action.toggle-clips"),
    )
    .changed()
    {
        app.toggle_pane(Pane::Clips);
    }
    if views::hint::button(ui, t("picker.menu.open_picker")).clicked() {
        ui.close();
        let ctx = ui.ctx().clone();
        crate::menu_commands::MenuCommand::DevicePicker.run(app, &ctx);
    }
    // The command palette (round 31, ADR 0072).
    if views::arrangement::action_item(app, ui, Action::CommandPalette, "action.command-palette") {
        ui.close();
    }
    ui.separator();
    layout_menu(app, ui);
    ui.separator();
    for (action, key) in [
        (Action::ZoomIn, "workspace.zoom_in"),
        (Action::ZoomOut, "workspace.zoom_out"),
        (Action::ZoomReset, "workspace.zoom_reset"),
        (Action::PickElement, "theme.inspect"),
        (Action::CloseFloating, "workspace.close_floating"),
    ] {
        if views::arrangement::action_item(app, ui, action, key) {
            ui.close();
        }
    }
    ui.separator();
    if views::hint::button(ui, t("settings.appearance")).clicked() {
        ui.close();
        app.show_theme_editor = true;
    }
    if views::hint::button(ui, t("menu.settings")).clicked() {
        ui.close();
        app.show_settings = true;
    }
    // Round 28 (ADR 0052): which groups the top bar shows, and in what order.
    if views::hint::button(ui, t("topbar.customise")).clicked() {
        ui.close();
        super::more::open_customise(app);
    }
    views::hint::said(
        ui.checkbox(&mut app.keyboard_piano, t("menu.keyboard_piano")),
        &t("menu.keyboard_piano"),
    );
    views::hint::said(
        ui.checkbox(&mut app.show_stats, t("menu.stats")),
        &t("menu.stats"),
    );
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
    let command = |app: &mut CantusApp, ui: &mut Ui, c: M| {
        if views::hint::button(ui, t(c.key())).clicked() {
            ui.close();
            let ctx = ui.ctx().clone();
            c.run(app, &ctx);
        }
    };
    command(app, ui, M::Manual);
    command(app, ui, M::ReleaseNotes);
    if views::hint::button(ui, t("menu.shortcuts")).clicked() {
        ui.close();
        app.show_shortcuts = true;
    }
    ui.separator();
    command(app, ui, M::ReportBug);
    command(app, ui, M::SuggestFeature);
    ui.separator();
    command(app, ui, M::CheckForUpdates);
    if views::hint::button(ui, t("menu.about")).clicked() {
        ui.close();
        app.show_about = true;
    }
    // About, scrolled to the credits: the Bible translations and every sound pack (round 26).
    command(app, ui, M::Credits);
}

/// *View → Layout*: switch between saved window shapes, save the current one under a
/// name, or go back to the built-in one.
fn layout_menu(app: &mut CantusApp, ui: &mut Ui) {
    let layout = ui
        .menu_button(t("menu.layout"), |ui| {
            let current = app.layouts.current.name.clone();
            let saved: Vec<String> = app.layouts.saved.iter().map(|l| l.name.clone()).collect();
            for name in &saved {
                if views::hint::said(ui.selectable_label(*name == current, name), name).clicked() {
                    ui.close();
                    app.apply_layout(name);
                }
            }
            if !saved.is_empty() {
                ui.separator();
            }
            if views::hint::button(ui, t("layout.save_as")).clicked() {
                ui.close();
                let ctx = ui.ctx().clone();
                crate::menu_commands::MenuCommand::SaveLayoutAs.run(app, &ctx);
            }
            let is_saved = saved.contains(&current);
            if ui
                .add_enabled(is_saved, egui::Button::new(t("layout.delete")))
                .clicked()
            {
                ui.close();
                let ctx = ui.ctx().clone();
                app.delete_layout(&current, &ctx);
            }
            if views::hint::button(ui, t("layout.reset")).clicked() {
                ui.close();
                app.reset_layout();
            }
        })
        .response;
    views::hint::said(layout, &t("menu.layout"));
}

/// The **Clip** menu: everything goes through `run_action`, and every entry shows the key
/// bound to it, so the menu teaches the shortcut instead of replacing it.
fn clip_menu(app: &mut CantusApp, ui: &mut Ui) {
    use crate::keymap::Action;
    // FL's Tools ▸ Last tweaked ▸ Create automation clip, here where clips are made
    // (ADR 0027): enabled once a parameter has been moved, and saying which.
    let tweaked = app
        .automation
        .last_tweaked
        .clone()
        .map(|t| crate::automation::target_name(app.project(), &t));
    ui.add_enabled_ui(tweaked.is_some(), |ui| {
        if views::arrangement::action_item(
            app,
            ui,
            Action::AutomateLastTweaked,
            "auto.menu.last_tweaked",
        ) {
            ui.close();
        }
        // FL's *Link to controller* for the same parameter (round 29, review 1 of ADR
        // 0062): the way into a hosted plug-in's own window from the mouse.
        if views::arrangement::action_item(
            app,
            ui,
            Action::LinkLastTweaked,
            "action.link-last-tweaked",
        ) {
            ui.close();
        }
    });
    if let Some(name) = tweaked {
        let _ = crate::kit::widgets::menu_list(
            ui,
            &[crate::kit::menu::Item::Band(crate::kit::text::Src::Str(
                &name,
            ))],
            240.0,
        );
    }
    ui.separator();
    let selection = app.clip_selection();
    let (muted, looping) = selection
        .first()
        .and_then(|(t, c)| app.project().track(*t)?.clip(*c))
        .map(|c| (c.muted, c.content_length.is_some()))
        .unwrap_or((false, false));
    // The whole menu is about the clip selection; with none there is nothing to enable.
    ui.add_enabled_ui(!selection.is_empty(), |ui| {
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
            if views::arrangement::action_item(app, ui, action, key) {
                ui.close();
            }
        }
        ui.separator();
        let mute_key = if muted { "clip.unmute" } else { "clip.mute" };
        if views::arrangement::action_item(app, ui, Action::ToggleClipMute, mute_key) {
            ui.close();
        }
        let loop_key = if looping {
            "clip.loop_off"
        } else {
            "clip.loop_on"
        };
        if views::arrangement::action_item(app, ui, Action::ToggleClipLoop, loop_key) {
            ui.close();
        }
        ui.separator();
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
        let ctx = ui.ctx().clone();
        for c in [crate::menu_commands::MenuCommand::ClipTrackColour] {
            if views::hint::button(ui, t(c.key())).clicked() {
                c.run(app, &ctx);
                ui.close();
            }
        }
        ui.separator();
        let c = crate::menu_commands::MenuCommand::DeleteClips;
        if views::hint::button(ui, t(c.key())).clicked() {
            c.run(app, &ctx);
            ui.close();
        }
    });
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
