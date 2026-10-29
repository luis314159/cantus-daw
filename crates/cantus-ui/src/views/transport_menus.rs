//! The top bar's left block (ADR 0052): the logo, the menus over the project's name and the
//! hint panel. The project's name is renamed in place: a click on it opens the kit's field
//! there (`title`), drawn after the bar's words so none shows under it.

use super::{title, Style};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::{shade, with_alpha};
use crate::kit::text::{self, Role, Text};
use crate::kit::{with_brush, Fill};
use crate::views;
use crate::views::workspace::Pane;
use egui::{Align2, Id, Pos2, Rect, Sense, Ui, Vec2};
use std::cell::RefCell;

/// The logo's place at 100 %.
pub const LOGO_W: f32 = 34.0;

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

fn menu_word_width(ctx: &egui::Context, s: &Style, key: &str) -> f32 {
    text::measure(
        ctx,
        &Text::key(key, Role::Control, s.t.text).caps(true),
        s.tk(),
    )
    .x + 14.0 * s.k
}

/// The menus' width in a row.
pub fn left_widths(ctx: &egui::Context, s: &Style) -> f32 {
    MENUS.iter().map(|k| menu_word_width(ctx, s, k)).sum()
}

/// The left block in `rect`: the logo, the menus over the project's name and the hint
/// panel. Returns the panel's rect (the name's field goes there, drawn last).
pub fn left_block(app: &mut CantusApp, ui: &mut Ui, rect: Rect, s: &Style) -> Option<Rect> {
    let k = s.k;
    let logo_r = Rect::from_center_size(
        Pos2::new(rect.left() + 14.0 * k, rect.center().y),
        Vec2::splat(28.0 * k),
    );
    views::workspace_widgets::paint_logo(ui.painter(), logo_r, app.appearance.logo, &s.t);
    let body = Rect::from_min_max(
        Pos2::new(rect.left() + LOGO_W * k, rect.top() + 3.0 * k),
        Pos2::new(rect.right(), rect.bottom() - 3.0 * k),
    );
    let menu_r = Rect::from_min_size(body.min, Vec2::new(body.width(), 17.0 * k));
    menus(app, ui, menu_r, s);
    let hint_r = Rect::from_min_max(Pos2::new(body.left(), menu_r.bottom() + 1.0 * k), body.max);
    if hint_r.width() < 60.0 * k {
        return None;
    }
    // While the project is renamed, its field stands where the name is: the panel leaves
    // the name out, so no word shows under the field.
    let naming = title::naming(app);
    NAME.with(|n| {
        let mut name = n.borrow_mut();
        name.clear();
        if !naming {
            name.push_str(&app.project().name);
            if app.bus.is_dirty() {
                name.push_str(" \u{2022}");
            }
        }
        with_brush(ui.painter(), |b| {
            // While renaming, the panel is the field's: no name and no idle hint under it.
            let idle = if naming {
                None
            } else {
                views::workspace_status::tool_hint_key(app)
            };
            views::hint::paint_panel(b, hint_r, s, &name, idle);
        });
    });
    // The idle hint in full, where the bar shows its essentials: the panel's tooltip.
    if let Some(key) = views::workspace_status::tool_hint_key(app) {
        let resp = ui.interact(hint_r, Id::new("cantus-hint-panel"), Sense::hover());
        if resp.hovered() {
            let _ = resp.on_hover_text(t(key));
        }
    }
    Some(hint_r)
}

/// The menu words: kit text in a box, each opening its menu.
fn menus(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let mut x = r.left();
    for (i, key) in MENUS.iter().enumerate() {
        let w = menu_word_width(ui.ctx(), s, key);
        let rect = Rect::from_min_size(Pos2::new(x, r.top()), Vec2::new(w, r.height()));
        x += w;
        if rect.right() > r.right() + 0.5 {
            break;
        }
        let resp = ui.interact(rect, Id::new(("toolbar-menu", i)), Sense::click());
        let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp));
        let build = open || resp.clicked();
        with_brush(ui.painter(), |b| {
            if resp.hovered() || open {
                b.rrect(
                    b.snap_rect(rect),
                    s.r(3.0),
                    Fill::Solid(with_alpha(s.t.text, if open { 0.14 } else { 0.08 })),
                );
            }
            let _ = text::paint(
                b,
                rect,
                &Text::key(
                    key,
                    Role::Control,
                    if resp.hovered() || open {
                        s.t.text
                    } else {
                        s.t.text_dim
                    },
                )
                .caps(true)
                .align(Align2::CENTER_CENTER),
                s.tk(),
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
    if ui.button(t("menu.new")).clicked() {
        ui.close();
        app.request_new();
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
    if ui.button(t("demos.menu")).clicked() {
        ui.close();
        views::demos::show(app);
    }
    if ui.button(t("menu.open")).clicked() {
        ui.close();
        app.request_open();
    }
    ui.separator();
    if ui.button(t("menu.save")).clicked() {
        ui.close();
        app.save();
    }
    if ui.button(t("menu.save_as")).clicked() {
        ui.close();
        app.save_as();
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
    if ui.button(t("audioclips.menu.collect")).clicked() {
        ui.close();
        app.save();
    }
    ui.separator();
    if ui.button(t("audioclips.menu.import")).clicked() {
        ui.close();
        app.import_audio_dialog();
    }
    if ui.button(t("menu.export_wav")).clicked() {
        ui.close();
        app.export_dialog();
    }
    views::interop::file_menu(app, ui);
    ui.separator();
    if ui.button(t("menu.quit")).clicked() {
        ui.close();
        app.request_quit = true;
    }
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
        .chain(std::iter::once(Pane::Score));
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
    if ui.checkbox(&mut shown, t("action.toggle-clips")).changed() {
        app.toggle_pane(Pane::Clips);
    }
    if ui.button(t("picker.menu.open_picker")).clicked() {
        ui.close();
        app.open_picker(crate::picker::PickPurpose::AddInstrument { index: None });
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
    if ui.button(t("settings.appearance")).clicked() {
        ui.close();
        app.show_theme_editor = true;
    }
    if ui.button(t("menu.settings")).clicked() {
        ui.close();
        app.show_settings = true;
    }
    // Round 28 (ADR 0052): which groups the top bar shows, and in what order.
    if ui.button(t("topbar.customise")).clicked() {
        ui.close();
        super::more::open_customise(app);
    }
    ui.checkbox(&mut app.keyboard_piano, t("menu.keyboard_piano"));
    ui.checkbox(&mut app.show_stats, t("menu.stats"));
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
    for (key, doc) in [
        ("about.menu.manual", crate::docs::Doc::Manual),
        ("about.menu.notes", crate::docs::Doc::ReleaseNotes),
    ] {
        if ui.button(t(key)).clicked() {
            ui.close();
            crate::docs::open(doc);
        }
    }
    if ui.button(t("menu.shortcuts")).clicked() {
        ui.close();
        app.show_shortcuts = true;
    }
    ui.separator();
    for (key, doc) in [
        ("about.menu.bug", crate::docs::Doc::ReportBug),
        ("about.menu.feature", crate::docs::Doc::RequestFeature),
    ] {
        if ui.button(t(key)).clicked() {
            ui.close();
            crate::docs::open(doc);
        }
    }
    ui.separator();
    if ui.button(t("update.menu.check")).clicked() {
        ui.close();
        views::update::check_now(app);
    }
    if ui.button(t("menu.about")).clicked() {
        ui.close();
        app.show_about = true;
    }
    // About, scrolled to the credits: the Bible translations and every sound pack (round 26).
    if ui.button(t("about.menu.credits")).clicked() {
        ui.close();
        let ctx = ui.ctx().clone();
        views::about::show_credits(app, &ctx);
    }
}

/// *View → Layout*: switch between saved window shapes, save the current one under a
/// name, or go back to the built-in one.
fn layout_menu(app: &mut CantusApp, ui: &mut Ui) {
    ui.menu_button(t("menu.layout"), |ui| {
        let current = app.layouts.current.name.clone();
        let saved: Vec<String> = app.layouts.saved.iter().map(|l| l.name.clone()).collect();
        for name in &saved {
            if ui.selectable_label(*name == current, name).clicked() {
                ui.close();
                app.apply_layout(name);
            }
        }
        if !saved.is_empty() {
            ui.separator();
        }
        if ui.button(t("layout.save_as")).clicked() {
            ui.close();
            // Offer the current name, so re-saving a layout is one keystroke.
            let seed = if current == crate::layout::DEFAULT_NAME {
                String::new()
            } else {
                current.clone()
            };
            app.naming_layout = Some(seed);
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
        if ui.button(t("layout.reset")).clicked() {
            ui.close();
            app.reset_layout();
        }
    });
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
        if ui.button(t("clip.color_reset")).clicked() {
            app.set_clip_color(None);
            ui.close();
        }
        ui.separator();
        if ui.button(t("clip.delete")).clicked() {
            app.delete_clip_selection();
            ui.close();
        }
    });
}
