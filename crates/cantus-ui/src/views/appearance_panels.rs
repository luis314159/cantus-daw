//! The appearance page's three tabs and its popups (see `appearance.rs`).

use super::appearance::{change, editing_mode};
use super::workspace_widgets as w;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::appearance::{self as appearance, Mode, PRESETS};
use crate::kit::knob::KnobSize;
use crate::kit::logo::{self, Logo, Tint};
use crate::kit::menu::Item;
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::tokens::{self, Density};
use crate::kit::widgets::{menu_field, segmented, Param};
use crate::kit::{with_brush, Group, Style, Tok};
use egui::{Align2, Context, Id, Order, Pos2, Rect, Sense, Ui, Vec2};
use std::fmt::Write as _;

/// The role pickers (FL's Theme page): the colours that carry a meaning, by name.
const ROLES: [(&str, Tok); 12] = [
    ("kit.token.accent.selection", Tok::Selection),
    ("kit.token.accent.highlight", Tok::Highlight),
    ("kit.token.accent.focus", Tok::Focus),
    ("kit.token.accent.led_on", Tok::Led),
    ("kit.token.accent.record", Tok::Danger),
    ("kit.token.accent.brand", Tok::Brand),
    ("kit.token.steps.odd_lit", Tok::StepOddLit),
    ("kit.token.steps.even_lit", Tok::StepEvenLit),
    ("kit.token.meters.low", Tok::MeterLow),
    ("kit.token.meters.high", Tok::MeterHigh),
    ("kit.token.meters.clip", Tok::MeterClip),
    ("kit.token.text.primary", Tok::Text),
];

/// The width of a label column in the page's rows.
const LABEL_W: f32 = 132.0;

fn base_name(app: &CantusApp, base: &str) -> String {
    if let Some(id) = base.strip_prefix("preset:") {
        if !PRESETS.iter().any(|(p, _)| *p == id) {
            return id.to_string();
        }
        return t(&appearance::preset_name_key(id));
    }
    let _ = app;
    base.strip_prefix("user:").unwrap_or(base).to_string()
}

/// **Look**: mode, the base theme of each mode, the knobs, the form, the role pickers.
pub fn look(app: &mut CantusApp, ui: &mut Ui) {
    let s = Style::current();
    let k = s.k;
    w::band(ui, Src::Key("theme.mode"));
    let mut sel = match app.appearance.mode {
        Mode::Dark => 0,
        Mode::Light => 1,
        Mode::System => 2,
    };
    let labels = [
        Src::Key("theme.mode.dark"),
        Src::Key("theme.mode.light"),
        Src::Key("theme.mode.system"),
    ];
    let width = ui.available_width() / k;
    if segmented(ui, &mut sel, &labels, &t("theme.mode"), width).changed() {
        let m = [Mode::Dark, Mode::Light, Mode::System][sel];
        change(app, |a| a.mode = m);
    }
    for (mode, key) in [
        (Mode::Dark, "workspace.appearance.base_dark"),
        (Mode::Light, "workspace.appearance.base_light"),
    ] {
        ui.horizontal(|ui| {
            w::row_label(ui, key, LABEL_W, 24.0);
            let name = base_name(app, &app.appearance.slot(mode).base);
            let fw = (ui.available_width() / k - 8.0).max(80.0);
            let (resp, clicks) = menu_field(ui, &name, false, false, &t(key), fw);
            if clicks.open {
                app.appearance_page.base_menu = Some((mode, resp.rect.left_bottom()));
            }
        });
    }
    let mode = editing_mode(app);
    let showing = base_name(app, &app.appearance.slot(mode).base);
    let mode_name = t(match mode {
        Mode::Dark => "theme.mode.dark",
        _ => "theme.mode.light",
    });
    caption(
        ui,
        &tf(
            "workspace.appearance.editing",
            &[("mode", &mode_name), ("theme", &showing)],
        ),
    );

    // FL's knobs: they move the whole ladder at once.
    w::band(ui, Src::Key("workspace.appearance.adjust"));
    let base = app
        .appearance
        .base_theme(&app.appearance.slot(mode).base, mode);
    let mut knobs = app.appearance.slot(mode).knobs.unwrap_or(base.knobs);
    let before = knobs;
    ui.horizontal_wrapped(|ui| {
        knob(
            ui,
            &mut knobs.hue,
            -180.0..=180.0,
            "theme.knobs.hue",
            |v, b| {
                let _ = write!(b, "{v:+.0}°");
            },
        );
        let pct = |v: f32, b: &mut String| {
            let _ = write!(b, "{:+.0} %", v * 100.0);
        };
        knob(
            ui,
            &mut knobs.saturation,
            -1.0..=1.0,
            "theme.knobs.saturation",
            pct,
        );
        knob(
            ui,
            &mut knobs.brightness,
            -1.0..=1.0,
            "theme.knobs.brightness",
            pct,
        );
        knob(
            ui,
            &mut knobs.contrast,
            -1.0..=1.0,
            "theme.knobs.contrast",
            pct,
        );
        knob(ui, &mut knobs.text, -1.0..=1.0, "theme.knobs.text", pct);
    });
    if knobs != before {
        change(app, |a| a.slot_mut(mode).knobs = Some(knobs));
    }

    w::band(ui, Src::Key("workspace.appearance.form"));
    let mut form = app.appearance.slot(mode).form.unwrap_or(base.form);
    let before = form;
    ui.horizontal_wrapped(|ui| {
        let pct = |v: f32, b: &mut String| {
            let _ = write!(b, "{:.0} %", v * 100.0);
        };
        knob(ui, &mut form.depth, 0.0..=1.5, "theme.form.depth", pct);
        knob(
            ui,
            &mut form.roundness,
            0.0..=2.5,
            "theme.form.roundness",
            pct,
        );
        knob(
            ui,
            &mut form.text_size,
            0.8..=1.4,
            "theme.form.text_size",
            pct,
        );
    });
    ui.horizontal(|ui| {
        w::row_label(ui, "theme.form.density", LABEL_W, 24.0);
        let mut d = match form.density {
            Density::Compact => 0,
            Density::Normal => 1,
            Density::Comfortable => 2,
        };
        let labels = [
            Src::Key("theme.form.density.compact"),
            Src::Key("theme.form.density.normal"),
            Src::Key("theme.form.density.comfortable"),
        ];
        let fw = (ui.available_width() / k - 8.0).max(120.0);
        if segmented(ui, &mut d, &labels, &t("theme.form.density"), fw).changed() {
            form.density = [Density::Compact, Density::Normal, Density::Comfortable][d];
        }
    });
    if form != before {
        change(app, |a| a.slot_mut(mode).form = Some(form));
    }

    w::band(ui, Src::Key("workspace.appearance.roles"));
    let tokens = tokens::current();
    let overrides = app.appearance.slot(mode).overrides.clone();
    // The names' column as wide as the longest name in this language, two pairs a row
    // where they fit, else one (final review: Spanish names were cut with room beside).
    let label_w = ROLES
        .iter()
        .map(|(key, _)| text::measure(ui.ctx(), &Text::key(key, Role::Body, s.t.text), s.tk()).x)
        .fold(0.0f32, f32::max)
        / k
        + 4.0;
    let pair = (26.0 + 8.0 + label_w + 8.0) * k;
    let two = 2.0 * pair <= ui.available_width();
    egui::Grid::new("cantus-roles")
        .num_columns(if two { 4 } else { 2 })
        .spacing(Vec2::new(8.0 * k, 6.0 * k))
        .show(ui, |ui| {
            for (i, (key, tok)) in ROLES.iter().enumerate() {
                let name = t(key);
                let r = w::swatch(
                    ui,
                    tokens.get(*tok),
                    Vec2::new(26.0, 20.0),
                    overrides.contains_key(tok.key()),
                    &name,
                );
                if r.clicked() {
                    app.appearance_page.editing = Some((*tok, r.rect.right_top()));
                }
                w::row_label_left(ui, key, label_w, 20.0);
                if !two || i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
    ui.add_space(4.0 * k);
    ui.horizontal(|ui| {
        if w::button(
            ui,
            Src::Key("theme.reset_all"),
            0.0,
            !app.appearance.slot(mode).is_pristine(),
        )
        .clicked()
        {
            change(app, |a| a.reset_all(mode));
        }
    });
}

/// A theme knob as a cell: value pill, knob, label.
fn knob(
    ui: &mut Ui,
    v: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    key: &'static str,
    fmt: impl Fn(f32, &mut String),
) {
    let default = if key.starts_with("theme.form") {
        1.0
    } else {
        0.0
    };
    let name = t(key);
    Param::new(v, range, default)
        .format(fmt)
        .size(KnobSize::S)
        .cell(Src::Key(key))
        .name(&name)
        .show(ui);
}

/// A dim line of explanation, in a box.
fn caption(ui: &mut Ui, s: &str) {
    let st = Style::current();
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0 * st.k), Sense::hover());
    with_brush(ui.painter(), |b| {
        let _ = text::paint(
            b,
            rect,
            &Text::new(s, Role::Label, st.t.text_dim)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            st.tk(),
        );
    });
}

/// **Colours**: every token, searchable and grouped; or the picked element's.
pub fn colours(app: &mut CantusApp, ui: &mut Ui) {
    let s = Style::current();
    let k = s.k;
    let mode = editing_mode(app);
    ui.horizontal(|ui| {
        let fw = (ui.available_width() / k - 118.0).max(120.0);
        let mut q = std::mem::take(&mut app.appearance_page.search);
        w::text_field(
            ui,
            egui::Id::new("theme-search"),
            &mut q,
            "theme.search",
            fw,
        );
        app.appearance_page.search = q;
        if w::button(ui, Src::Key("theme.inspect"), 110.0, true).clicked() {
            app.toggle_pick();
        }
    });
    let picked: Option<Vec<Tok>> = app
        .appearance_page
        .picked
        .as_ref()
        .map(|item| item.all_tokens().collect());
    if let Some(item) = app.appearance_page.picked {
        ui.horizontal(|ui| {
            caption_inline(
                ui,
                &tf("workspace.appearance.picked", &[("part", &item.part)]),
            );
            if w::button(ui, Src::Key("workspace.appearance.show_all"), 0.0, true).clicked() {
                app.appearance_page.picked = None;
            }
        });
    }
    let found: Vec<Tok> = match &picked {
        Some(list) => list.clone(),
        None => tokens::search(&app.appearance_page.search),
    };
    let current = tokens::current();
    let overrides = app.appearance.slot(mode).overrides.clone();
    for group in Group::ALL {
        let in_group: Vec<Tok> = found
            .iter()
            .copied()
            .filter(|t| t.group() == group)
            .collect();
        if in_group.is_empty() {
            continue;
        }
        let band = w::band(ui, Src::Key(group_key(group)));
        // "Reset this group" at the band's right end.
        let any = in_group.iter().any(|t| overrides.contains_key(t.key()));
        if any {
            let r = Rect::from_min_max(
                Pos2::new(band.right() - 110.0 * k, band.top() + 1.0),
                Pos2::new(band.right() - 4.0 * k, band.bottom() - 1.0),
            );
            let resp = ui.interact(r, Id::new(("reset-group", group)), Sense::click());
            with_brush(ui.painter(), |b| {
                let _ = text::paint(
                    b,
                    r,
                    &Text::key(
                        "theme.reset_group",
                        Role::Label,
                        if resp.hovered() {
                            s.t.text
                        } else {
                            s.t.text_dim
                        },
                    )
                    .align(Align2::RIGHT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                    s.tk(),
                );
            });
            if resp.clicked() {
                change(app, |a| a.reset_group(mode, group));
            }
        }
        for tok in in_group {
            token_row(
                app,
                ui,
                tok,
                current.get(tok),
                overrides.contains_key(tok.key()),
                mode,
            );
        }
    }
    if found.is_empty() {
        caption(ui, &t("workspace.appearance.none_found"));
    }
}

fn group_key(g: Group) -> &'static str {
    match g {
        Group::Surfaces => "kit.group.surface",
        Group::Lines => "kit.group.lines",
        Group::Text => "kit.group.text",
        Group::Accents => "kit.group.accent",
        Group::Meters => "kit.group.meters",
        Group::Mixer => "kit.group.mixer",
        Group::Knobs => "kit.group.knob",
        Group::Devices => "kit.group.device",
        Group::Steps => "kit.group.steps",
        Group::Keys => "kit.group.keys",
        Group::Menus => "kit.group.menu",
        Group::Hues => "kit.group.hue",
        Group::Depth => "kit.group.depth",
        Group::Editors => "kit.group.editor",
    }
}

fn caption_inline(ui: &mut Ui, s: &str) {
    let st = Style::current();
    let w = text::measure(ui.ctx(), &Text::new(s, Role::Label, st.t.text_dim), st.tk()).x;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w + 4.0, 22.0 * st.k), Sense::hover());
    with_brush(ui.painter(), |b| {
        let _ = text::paint(
            b,
            rect,
            &Text::new(s, Role::Label, st.t.text_dim).align(Align2::LEFT_CENTER),
            st.tk(),
        );
    });
}

/// One token: its swatch, its name, its key, and its reset when it is the user's.
fn token_row(
    app: &mut CantusApp,
    ui: &mut Ui,
    tok: Tok,
    c: egui::Color32,
    changed: bool,
    mode: Mode,
) {
    let s = Style::current();
    let k = s.k;
    let name = t(&tok.name_key());
    ui.horizontal(|ui| {
        let r = w::swatch(ui, c, Vec2::new(30.0, 20.0), changed, &name);
        if r.clicked() {
            app.appearance_page.editing = Some((tok, r.rect.right_top()));
        }
        let avail = ui.available_width();
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(avail - 28.0 * k, 22.0 * k), Sense::hover());
        with_brush(ui.painter(), |b| {
            let key_w = (rect.width() * 0.42).min(170.0 * k);
            let nr = Rect::from_min_max(
                rect.min,
                Pos2::new(rect.right() - key_w - 6.0 * k, rect.bottom()),
            );
            let kr = Rect::from_min_max(Pos2::new(rect.right() - key_w, rect.top()), rect.max);
            let _ = text::paint(
                b,
                nr,
                &Text::new(&name, Role::Body, s.t.text)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
            let _ = text::paint(
                b,
                kr,
                &Text::new(tok.key(), Role::Micro, s.t.text_dim)
                    .caps(false)
                    .align(Align2::RIGHT_CENTER)
                    .policy(Policy::Elide(Elide::Middle)),
                s.tk(),
            );
        });
        if changed
            && w::glyph_key(
                ui,
                crate::kit::glyph::Glyph::Undo,
                false,
                20.0,
                &t("theme.reset_token"),
            )
            .clicked()
        {
            change(app, |a| a.reset_token(mode, tok));
        }
    });
}

/// **Themes and logo**: save, duplicate, delete, share and import; the logo and its tint.
pub fn themes(app: &mut CantusApp, ui: &mut Ui) {
    let s = Style::current();
    let k = s.k;
    let mode = editing_mode(app);
    w::band(ui, Src::Key("workspace.appearance.save"));
    ui.horizontal(|ui| {
        let fw = (ui.available_width() / k - 100.0).max(120.0);
        let mut name = std::mem::take(&mut app.appearance_page.name);
        w::text_field(
            ui,
            egui::Id::new("theme-name"),
            &mut name,
            "workspace.appearance.theme_name",
            fw,
        );
        let ok = !name.trim().is_empty();
        if w::button(ui, Src::Key("theme.save_as"), 92.0, ok).clicked() {
            let n = name.trim().to_string();
            change(app, |a| {
                a.save_as(mode, &n);
            });
            name.clear();
        }
        app.appearance_page.name = name;
    });
    w::band(ui, Src::Key("workspace.appearance.yours"));
    let names: Vec<String> = app
        .appearance
        .themes
        .iter()
        .map(|t| t.name.clone())
        .collect();
    if names.is_empty() {
        caption(ui, &t("workspace.appearance.no_themes"));
    }
    for name in names {
        theme_row(app, ui, &name, &format!("user:{name}"), true);
    }
    w::band(ui, Src::Key("workspace.appearance.presets"));
    for (id, _) in PRESETS {
        let base = format!("preset:{id}");
        let label = base_name(app, &base);
        theme_row(app, ui, &label, &base, false);
    }
    ui.horizontal(|ui| {
        if w::button(ui, Src::Key("theme.import"), 0.0, true).clicked() {
            import(app);
        }
        if w::button(ui, Src::Key("theme.export"), 0.0, true).clicked() {
            let theme = app.appearance.resolve_mode(mode);
            export(app, &theme);
        }
        if w::button(ui, Src::Key("menu.open_theme_file"), 0.0, true).clicked() {
            app.reveal_theme_file();
        }
    });
    if let Some((m, err)) = app.appearance_page.message.clone() {
        let st = Style::current();
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0 * k), Sense::hover());
        with_brush(ui.painter(), |b| {
            let _ = text::paint(
                b,
                rect,
                &Text::new(
                    &m,
                    Role::Label,
                    if err { st.t.danger } else { st.t.text_dim },
                )
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
                st.tk(),
            );
        });
    }

    w::band(ui, Src::Key("theme.logo"));
    let choice = app.appearance.logo;
    let tokens = tokens::current();
    let side = 56.0 * k;
    ui.horizontal_wrapped(|ui| {
        for logo in Logo::all() {
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(side + 8.0 * k), Sense::click());
            let on = choice.number == logo.0;
            with_brush(ui.painter(), |b| {
                if on || resp.hovered() {
                    b.rrect(
                        rect,
                        s.r(6.0),
                        crate::kit::Fill::Solid(crate::kit::color::with_alpha(
                            if on { s.t.selection } else { s.t.text },
                            if on { 0.22 } else { 0.08 },
                        )),
                    );
                }
                if on {
                    b.rrect_stroke(rect, s.r(6.0), 1.5 * k, s.t.selection);
                }
            });
            let this = logo::LogoChoice {
                number: logo.0,
                tint: choice.tint,
            };
            w::paint_logo(ui.painter(), rect.shrink(4.0 * k), this, &tokens);
            let resp = resp.on_hover_text(t(logo.name_key()));
            if resp.clicked() {
                change(app, |a| a.logo.number = logo.0);
            }
        }
    });
    ui.horizontal(|ui| {
        w::row_label(ui, "theme.logo.tint", LABEL_W, 24.0);
        const TINTS: [Tint; 3] = [Tint::Original, Tint::Theme, Tint::Icon];
        let mut sel = TINTS.iter().position(|x| *x == choice.tint).unwrap_or(0);
        let labels = [
            Src::Key("kit.logo.tint.original"),
            Src::Key("kit.logo.tint.theme"),
            Src::Key("kit.logo.tint.icon"),
        ];
        let fw = (ui.available_width() / k - 8.0).max(200.0);
        if segmented(ui, &mut sel, &labels, &t("theme.logo.tint"), fw).changed() {
            let tint = TINTS[sel];
            change(app, |a| a.logo.tint = tint);
        }
    });

    super::appearance_look::icon_section(app, ui);
}

/// One theme: its name, and what can be done with it.
fn theme_row(app: &mut CantusApp, ui: &mut Ui, label: &str, base: &str, own: bool) {
    let s = Style::current();
    let k = s.k;
    let preview = super::appearance_look::preview_tokens(app, base);
    ui.horizontal(|ui| {
        let used_dark = app.appearance.dark.base == base;
        let used_light = app.appearance.light.base == base;
        let (chip, resp) = ui.allocate_exact_size(Vec2::new(64.0 * k, 38.0 * k), Sense::hover());
        with_brush(ui.painter(), |b| {
            super::appearance_look::paint_preview(b, chip, &preview, k);
        });
        resp.on_hover_text(tf("look.preview", &[("name", &label)]));
        let (rect, _) = ui.allocate_exact_size(Vec2::new(150.0 * k, 38.0 * k), Sense::hover());
        with_brush(ui.painter(), |b| {
            let _ = text::paint(
                b,
                rect,
                &Text::new(label, Role::Body, s.t.text)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::Middle)),
                s.tk(),
            );
        });
        if w::button(ui, Src::Key("theme.mode.dark"), 64.0, !used_dark).clicked() {
            let b = base.to_string();
            change(app, |a| a.assign(Mode::Dark, b));
        }
        if w::button(ui, Src::Key("theme.mode.light"), 64.0, !used_light).clicked() {
            let b = base.to_string();
            change(app, |a| a.assign(Mode::Light, b));
        }
        if w::button(ui, Src::Key("theme.duplicate"), 0.0, true).clicked() {
            let mut n = 2;
            let stem = label.to_string();
            let mut name = tf("workspace.appearance.copy_of", &[("name", &stem)]);
            while app.appearance.themes.iter().any(|t| t.name == name) {
                name = format!(
                    "{} {n}",
                    tf("workspace.appearance.copy_of", &[("name", &stem)])
                );
                n += 1;
            }
            let b = base.to_string();
            change(app, |a| {
                a.duplicate(&b, &name);
            });
        }
        if own && w::button(ui, Src::Key("theme.delete"), 0.0, true).clicked() {
            let name = label.to_string();
            change(app, |a| a.delete(&name));
        }
    });
}

fn import(app: &mut CantusApp) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("JSON", &["json"])
        .pick_file()
    else {
        return;
    };
    let result = std::fs::read_to_string(&path).map_err(|e| e.to_string());
    let mut out = None;
    change(app, |a| {
        out = Some(result.and_then(|json| a.import(&json)));
    });
    app.appearance_page.message = match out {
        Some(Ok(name)) => Some((
            tf("workspace.appearance.imported", &[("name", &name)]),
            false,
        )),
        Some(Err(e)) => Some((e, true)),
        None => None,
    };
}

fn export(app: &mut CantusApp, theme: &crate::theme::Theme) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("JSON", &["json"])
        .set_file_name(format!("{}.json", theme.name))
        .save_file()
    else {
        return;
    };
    app.appearance_page.message = Some(
        match std::fs::write(&path, appearance::export_theme(theme)) {
            Ok(()) => (
                tf(
                    "workspace.appearance.exported",
                    &[("path", &path.display())],
                ),
                false,
            ),
            Err(e) => (e.to_string(), true),
        },
    );
}

/// The colour picker of the token being edited, and the base theme menu.
pub fn popups(app: &mut CantusApp, ctx: &Context) {
    let s = Style::current();
    let mode = editing_mode(app);
    if let Some((tok, at)) = app.appearance_page.editing {
        let mut c = tokens::current().get(tok);
        let mut close = false;
        let area = egui::Area::new(Id::new("cantus-colour-picker"))
            .order(Order::Foreground)
            .fixed_pos(at + Vec2::new(8.0, 0.0))
            .constrain(true)
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(s.t.menu)
                    .corner_radius(s.r(6.0))
                    .inner_margin(10.0 * s.k)
                    .show(ui, |ui| {
                        ui.set_max_width(260.0 * s.k);
                        let name = t(&tok.name_key());
                        caption(ui, &name);
                        if egui::widgets::color_picker::color_picker_color32(
                            ui,
                            &mut c,
                            egui::widgets::color_picker::Alpha::Opaque,
                        ) {
                            change(app, |a| a.set_token(mode, tok, c));
                        }
                        ui.horizontal(|ui| {
                            if w::button(ui, Src::Key("theme.reset_token"), 0.0, true).clicked() {
                                change(app, |a| a.reset_token(mode, tok));
                            }
                            if w::button(ui, Src::Key("workspace.close"), 0.0, true).clicked() {
                                close = true;
                            }
                        });
                    });
            });
        let outside = ctx.input(|i| i.pointer.any_pressed())
            && !ctx
                .pointer_interact_pos()
                .is_some_and(|p| area.response.rect.contains(p));
        if close || outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            app.appearance_page.editing = None;
        }
    }
    if let Some((for_mode, at)) = app.appearance_page.base_menu {
        let mut bases: Vec<String> = PRESETS
            .iter()
            .map(|(id, _)| format!("preset:{id}"))
            .collect();
        bases.extend(
            app.appearance
                .themes
                .iter()
                .map(|t| format!("user:{}", t.name)),
        );
        let labels: Vec<String> = bases.iter().map(|b| base_name(app, b)).collect();
        let current = app.appearance.slot(for_mode).base.clone();
        let items: Vec<Item<'_>> = labels
            .iter()
            .zip(&bases)
            .map(|(l, b)| Item::Action {
                label: Src::Str(l),
                check: crate::kit::menu::Check::Radio(*b == current),
                shortcut: None,
                submenu: false,
                enabled: true,
            })
            .collect();
        // Each row carries its theme's swatch at the right, painted with that theme's own
        // tokens as the Themes tab's rows are (round 26); the menu is as wide as its
        // longest name plus the swatch, so no name runs under one.
        let previews: Vec<crate::kit::Tokens> = bases
            .iter()
            .map(|b| super::appearance_look::preview_tokens(app, b))
            .collect();
        let k = s.k;
        let sw = Vec2::new(40.0 * k, 16.0 * k);
        let names_w = labels
            .iter()
            .map(|l| text::measure(ctx, &Text::new(l, Role::Body, s.t.menu_text), s.tk()).x)
            .fold(0.0f32, f32::max);
        let width = (22.0 * k + names_w + 16.0 * k + sw.x + 10.0 * k).max(220.0 * k);
        let mut chosen = None;
        let mut swatches: Vec<(String, Rect)> = Vec::with_capacity(bases.len());
        let area = egui::Area::new(Id::new("cantus-base-menu"))
            .order(Order::Foreground)
            .fixed_pos(at)
            .constrain(true)
            .show(ctx, |ui| {
                let (resp, c) = crate::kit::widgets::menu_list(ui, &items, width);
                chosen = c.clicked;
                with_brush(ui.painter(), |b| {
                    for (i, t) in previews.iter().enumerate() {
                        let row = crate::kit::menu::row(resp.rect, &items, s.d(1.0), i);
                        let r = Rect::from_center_size(
                            Pos2::new(row.right() - 6.0 * k - sw.x * 0.5, row.center().y),
                            sw,
                        );
                        super::appearance_look::paint_preview(b, r, t, k * 0.5);
                        swatches.push((bases[i].clone(), r));
                    }
                });
            });
        ctx.data_mut(|d| d.insert_temp(swatches_id(), swatches));
        let outside = ctx.input(|i| i.pointer.any_pressed())
            && !ctx
                .pointer_interact_pos()
                .is_some_and(|p| area.response.rect.contains(p));
        if let Some(i) = chosen {
            let b = bases[i].clone();
            change(app, |a| a.assign(for_mode, b));
        }
        if chosen.is_some() || outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            app.appearance_page.base_menu = None;
        }
    }
}

fn swatches_id() -> Id {
    Id::new("cantus-base-menu-swatches")
}

/// Where the open base-theme menu painted each theme's swatch last frame, by its base
/// (`preset:<id>` / `user:<name>`) — for tests and tools.
pub fn base_menu_swatches(ctx: &Context) -> Vec<(String, Rect)> {
    ctx.data(|d| d.get_temp::<Vec<(String, Rect)>>(swatches_id()))
        .unwrap_or_default()
}
