//! Settings ▸ AI (ADR 0081 §10): the one switch for the whole AI area; external agents —
//! the door, its port, the token, *Copy setup for…* each client and a Claude Desktop
//! extension; what agents may do — edits without asking, files and their folders, tools
//! refused by name; the AI activity. Off, the page shows its switch alone.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::text::{Role, Src};
use crate::kit::widgets::segmented;
use crate::kit::Style;
use crate::views::workspace_settings::wrapped;
use crate::views::workspace_widgets as w;
use egui::{Id, Sense, Ui, Vec2};

use super::setup::{self, Client};

/// The label column's width at 100 % (Settings' own).
const LABEL_W: f32 = 140.0;

/// The page's fields and its last word.
#[derive(Default)]
pub struct PageState {
    pub port_text: Option<String>,
    pub folder_text: String,
    pub tool_text: String,
    /// "Copied the setup for Cursor…", or an error.
    pub message: Option<(String, bool)>,
}

fn note(ui: &mut Ui, text: &str, warn: bool) {
    let s = Style::current();
    ui.horizontal(|ui| {
        w::row_label(ui, "", LABEL_W, 18.0);
        let color = if warn { s.t.danger } else { s.t.text_dim };
        let width = ui.available_width();
        ui.vertical(|ui| wrapped(ui, text, Role::Label, color, width));
    });
}

/// A two-way choice in a row; true when changed.
fn choice(ui: &mut Ui, label: &str, on: &mut bool, off_key: &str, on_key: &str) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        w::row_label(ui, label, LABEL_W, 24.0);
        let mut sel = usize::from(*on);
        let labels = [Src::Key(off_key), Src::Key(on_key)];
        if segmented(ui, &mut sel, &labels, &t(label), 260.0).changed() {
            *on = sel == 1;
            changed = true;
        }
    });
    changed
}

/// A one-line field; the words when Enter keeps them.
fn field(ui: &mut Ui, id: &str, text: &mut String, hint: &str, width: f32) -> bool {
    let s = Style::current();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width * s.k, s.d(24.0)), Sense::hover());
    let hint = t(hint);
    let out = crate::kit::field::Field::new(Id::new(("ai-settings", id)))
        .hint(&hint)
        .show(ui, rect, text);
    matches!(out.event, Some(crate::kit::field::Event::Commit)) && out.enter
}

/// The page.
pub fn page(app: &mut CantusApp, ui: &mut Ui) {
    w::band(ui, Src::Key("ai.settings.title"));
    let mut on = app.settings.ai.enabled;
    if choice(
        ui,
        "ai.settings.area",
        &mut on,
        "ai.settings.off",
        "ai.settings.on",
    ) {
        app.settings.ai.enabled = on;
        app.store_settings();
    }
    note(ui, &t("ai.settings.area_hint"), false);
    if !app.settings.ai.enabled {
        return;
    }
    external(app, ui);
    permissions(app, ui);
    w::band(ui, Src::Key("ai.activity.title"));
    ui.horizontal(|ui| {
        w::row_label(ui, "", LABEL_W, 24.0);
        if w::button(ui, Src::Key("ai.settings.open_activity"), 0.0, true).clicked() {
            app.ai.activity_open = true;
        }
    });
}

fn external(app: &mut CantusApp, ui: &mut Ui) {
    w::band(ui, Src::Key("ai.settings.external"));
    let mut on = app.settings.ai.external_agents;
    if choice(
        ui,
        "ai.settings.allow_external",
        &mut on,
        "ai.settings.off",
        "ai.settings.on",
    ) {
        app.settings.ai.external_agents = on;
        app.store_settings();
    }
    note(ui, &t("ai.settings.external_hint"), false);
    if !app.settings.ai.external_agents {
        return;
    }
    // The port.
    let port_now = app.settings.ai.port;
    let mut text = app
        .ai
        .page
        .port_text
        .clone()
        .unwrap_or_else(|| port_now.to_string());
    let mut kept = false;
    ui.horizontal(|ui| {
        w::row_label(ui, "ai.settings.port", LABEL_W, 24.0);
        kept = field(ui, "port", &mut text, "ai.settings.port", 90.0);
    });
    if kept {
        match text.trim().parse::<u16>() {
            Ok(p) if p >= 1024 => {
                app.settings.ai.port = p;
                app.store_settings();
                app.ai.page.port_text = None;
            }
            _ => {
                app.ai.page.message = Some((t("ai.settings.port_bad"), true));
                app.ai.page.port_text = None;
            }
        }
    } else {
        app.ai.page.port_text = Some(text);
    }
    let status = match (app.ai.door.port(), &app.ai.door.error) {
        (_, Some(e)) => (e.clone(), true),
        (Some(p), None) => (
            tf("ai.settings.listening", &[("url", &setup::url(p))]),
            false,
        ),
        (None, None) => (t("ai.settings.opening"), false),
    };
    note(ui, &status.0, status.1);
    // The token.
    let home = app.ai.door.home();
    ui.horizontal(|ui| {
        w::row_label(ui, "ai.settings.token", LABEL_W, 24.0);
        if w::button(ui, Src::Key("ai.settings.copy_token"), 0.0, true).clicked() {
            match super::door::token(&home) {
                Some(tok) => {
                    ui.ctx().copy_text(tok);
                    app.ai.page.message = Some((t("ai.settings.token_copied"), false));
                }
                None => app.ai.page.message = Some((t("ai.settings.no_token"), true)),
            }
        }
        if w::button(ui, Src::Key("ai.settings.regenerate"), 0.0, true).clicked() {
            app.ai.page.message = Some(match super::door::regenerate(app) {
                Ok(_) => (t("ai.settings.regenerated"), false),
                Err(e) => (tf("ai.settings.no_token_why", &[("error", &e)]), true),
            });
        }
    });
    note(ui, &t("ai.settings.token_hint"), false);
    // Copy setup for…
    let port = app.ai.door.port().unwrap_or(app.settings.ai.port);
    let bridge = setup::bridge_path();
    let launch = setup::this_launch();
    // Two rows of three, under the value column.
    for (row, clients) in Client::ALL.chunks(3).enumerate() {
        ui.horizontal(|ui| {
            let label = if row == 0 { "ai.settings.setup" } else { "" };
            w::row_label(ui, label, LABEL_W, 24.0);
            for &c in clients {
                if w::button(ui, Src::Str(c.name()), 0.0, true).clicked() {
                    let token = super::door::client_token(app, c.name());
                    app.ai.page.message = Some(match token {
                        Ok(tok) => {
                            ui.ctx().copy_text(setup::snippet(c, port, &tok, &launch));
                            let place = t(c.where_key());
                            (
                                tf(
                                    "ai.settings.copied",
                                    &[("client", &c.name()), ("where", &place)],
                                ),
                                false,
                            )
                        }
                        Err(e) => (tf("ai.settings.no_token_why", &[("error", &e)]), true),
                    });
                }
            }
        });
    }
    ui.horizontal(|ui| {
        w::row_label(ui, "ai.settings.extension", LABEL_W, 24.0);
        if w::button(ui, Src::Key("ai.settings.save_mcpb"), 0.0, bridge.is_file()).clicked() {
            save_mcpb(app, &bridge, &home);
        }
    });
    note(
        ui,
        &tf("ai.settings.bridge", &[("path", &launch.line())]),
        false,
    );
    if let Some((m, warn)) = app.ai.page.message.clone() {
        note(ui, &m, warn);
    }
}

fn save_mcpb(app: &mut CantusApp, bridge: &std::path::Path, home: &std::path::Path) {
    let Some(path) = rfd::FileDialog::new()
        .set_file_name("Cantus.mcpb")
        .add_filter("Claude Desktop extension", &["mcpb"])
        .save_file()
    else {
        return;
    };
    let written = setup::mcpb(crate::app::APP_VERSION, bridge, home)
        .and_then(|bytes| std::fs::write(&path, bytes));
    app.ai.page.message = Some(match written {
        Ok(()) => (
            tf("ai.settings.mcpb_saved", &[("file", &path.display())]),
            false,
        ),
        Err(e) => (tf("ai.settings.mcpb_failed", &[("error", &e)]), true),
    });
}

fn permissions(app: &mut CantusApp, ui: &mut Ui) {
    w::band(ui, Src::Key("ai.settings.may"));
    let mut edits = app.settings.ai.allow_edits;
    if choice(
        ui,
        "ai.settings.edits",
        &mut edits,
        "ai.settings.ask",
        "ai.settings.no_ask",
    ) {
        app.settings.ai.allow_edits = edits;
        app.store_settings();
    }
    note(ui, &t("ai.settings.edits_hint"), false);
    let mut files = app.settings.ai.allow_files;
    if choice(
        ui,
        "ai.settings.files",
        &mut files,
        "ai.settings.off",
        "ai.settings.ask_each",
    ) {
        app.settings.ai.allow_files = files;
        app.store_settings();
    }
    note(ui, &t("ai.settings.files_hint"), false);
    if app.settings.ai.allow_files {
        let mut remove = None;
        for (i, f) in app.settings.ai.folders.iter().enumerate() {
            let label = f.display().to_string();
            ui.horizontal(|ui| {
                w::row_label(ui, "", LABEL_W, 24.0);
                let width = ui.available_width() - 120.0 * Style::current().k;
                let s = Style::current();
                ui.allocate_ui(Vec2::new(width.max(80.0), s.d(24.0)), |ui| {
                    wrapped(ui, &label, Role::Body, s.t.text, width.max(80.0));
                });
                if w::button(ui, Src::Key("ai.settings.remove"), 0.0, true).clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            app.settings.ai.folders.remove(i);
            app.store_settings();
        }
        let mut text = std::mem::take(&mut app.ai.page.folder_text);
        let mut add = false;
        ui.horizontal(|ui| {
            w::row_label(ui, "ai.settings.folders", LABEL_W, 24.0);
            add |= field(ui, "folder", &mut text, "ai.settings.folder_hint", 260.0);
            add |= w::button(
                ui,
                Src::Key("ai.settings.add"),
                0.0,
                !text.trim().is_empty(),
            )
            .clicked();
            if w::button(ui, Src::Key("ai.settings.browse"), 0.0, true).clicked() {
                if let Some(d) = rfd::FileDialog::new().pick_folder() {
                    text = d.display().to_string();
                    add = true;
                }
            }
        });
        let dir = std::path::PathBuf::from(text.trim());
        if add && dir.is_dir() && !app.settings.ai.folders.contains(&dir) {
            app.settings.ai.folders.push(dir);
            app.store_settings();
            text.clear();
        } else if add {
            app.ai.page.message = Some((t("ai.settings.not_a_folder"), true));
        }
        app.ai.page.folder_text = text;
    }
    // Tools refused by name.
    let mut remove = None;
    for (i, tool) in app.settings.ai.denied_tools.iter().enumerate() {
        let tool = tool.clone();
        ui.horizontal(|ui| {
            w::row_label(
                ui,
                if i == 0 { "ai.settings.denied" } else { "" },
                LABEL_W,
                24.0,
            );
            let s = Style::current();
            ui.allocate_ui(Vec2::new(200.0 * s.k, s.d(24.0)), |ui| {
                wrapped(ui, &tool, Role::Body, s.t.text, 200.0 * s.k);
            });
            if w::button(ui, Src::Key("ai.settings.remove"), 0.0, true).clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        app.settings.ai.denied_tools.remove(i);
        app.store_settings();
    }
    let mut text = std::mem::take(&mut app.ai.page.tool_text);
    let mut add = false;
    ui.horizontal(|ui| {
        let label = if app.settings.ai.denied_tools.is_empty() {
            "ai.settings.denied"
        } else {
            ""
        };
        w::row_label(ui, label, LABEL_W, 24.0);
        add |= field(ui, "tool", &mut text, "ai.settings.tool_hint", 340.0);
        add |= w::button(
            ui,
            Src::Key("ai.settings.refuse"),
            0.0,
            !text.trim().is_empty(),
        )
        .clicked();
    });
    let name = text.trim().to_string();
    let known = crate::ai_tools::find(&name).is_some();
    if add && known && !app.settings.ai.denied_tools.contains(&name) {
        app.settings.ai.denied_tools.push(name);
        app.store_settings();
        text.clear();
    } else if add {
        app.ai.page.message = Some((tf("ai.settings.no_such_tool", &[("name", &name)]), true));
    }
    app.ai.page.tool_text = text;
}
