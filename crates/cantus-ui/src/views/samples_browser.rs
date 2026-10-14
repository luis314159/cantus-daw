//! The sample browser (ADR 0030): a dock tab listing the installed packs and the user's
//! own folders as a tree, with search. A click plays a sample through the audition voice;
//! a drag takes it to a Pulsus pad, Exemplar's keys, the channel rack or the arrangement.
//! When nothing is installed it offers the starter drum kit in one click.

use super::interop::form;
use super::samples_paint::{self as paint, Icon, RowLook};
use super::workspace_widgets as w;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::shade;
use crate::kit::glyph::Glyph;
use crate::kit::text::{Role, Src};
use crate::kit::{with_brush, Fill, Style};
use crate::samples::{BrowserGeometry, Drag, Root, RootKind, Row};
use cantus_content::library::Entry;
use egui::{pos2, vec2, Align2, Rect, Sense, Ui, UiBuilder};
use std::path::{Path, PathBuf};

/// The most search results listed.
const RESULTS: usize = 500;

#[derive(Clone, Debug, PartialEq)]
enum Kind {
    Section(&'static str),
    Hint(String),
    Root(RootKind),
    Dir,
    File,
}

#[derive(Clone, Debug)]
struct Item {
    kind: Kind,
    path: PathBuf,
    name: String,
    detail: String,
    depth: usize,
}

/// The rows to draw, in order: sections, roots, and the open folders' contents.
fn items(app: &mut CantusApp) -> Vec<Item> {
    let user = app.settings.sample_folders.clone();
    let roots = app.samples.roots(&user);
    let mut out = Vec::new();
    let query = app.samples.query.trim().to_string();
    if !query.is_empty() {
        match app.samples.search(&roots, RESULTS) {
            None => out.push(hint(t("samples.searching"))),
            Some(hits) if hits.is_empty() => {
                out.push(hint(tf("samples.no_results", &[("query", &query)])))
            }
            Some(hits) => {
                for p in hits {
                    let detail = roots
                        .iter()
                        .find(|r| p.starts_with(&r.path))
                        .map(|r| {
                            let rel = p.parent().and_then(|d| d.strip_prefix(&r.path).ok());
                            match rel {
                                Some(rel) if !rel.as_os_str().is_empty() => format!(
                                    "{} › {}",
                                    rel.to_string_lossy().replace('\\', " › "),
                                    r.label
                                ),
                                _ => r.label.clone(),
                            }
                        })
                        .unwrap_or_default();
                    out.push(Item {
                        kind: Kind::File,
                        name: stem(&p),
                        path: p,
                        detail,
                        depth: 0,
                    });
                }
            }
        }
        return out;
    }
    let (packs, folders): (Vec<Root>, Vec<Root>) = roots
        .into_iter()
        .partition(|r| matches!(r.kind, RootKind::Pack(_)));
    if !packs.is_empty() {
        out.push(section("samples.section.packs"));
        for r in packs {
            root(app, &mut out, r);
        }
    }
    out.push(section("samples.section.folders"));
    if folders.is_empty() {
        out.push(hint(t("samples.folders.empty")));
    }
    for r in folders {
        root(app, &mut out, r);
    }
    out
}

fn section(key: &'static str) -> Item {
    Item {
        kind: Kind::Section(key),
        path: PathBuf::new(),
        name: String::new(),
        detail: String::new(),
        depth: 0,
    }
}

fn hint(text: String) -> Item {
    Item {
        kind: Kind::Hint(text),
        path: PathBuf::new(),
        name: String::new(),
        detail: String::new(),
        depth: 0,
    }
}

/// A file's name as the browser shows it: without its extension (the format is a tag).
fn stem(p: &Path) -> String {
    p.file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The format tag of a file: `WAV`, `FLAC`, `AIFF`, `OGG`.
fn format_tag(p: &Path) -> String {
    match p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("wav" | "wave") => "WAV".into(),
        Some("aif" | "aiff" | "aifc") => "AIFF".into(),
        Some("ogg" | "oga") => "OGG".into(),
        Some(e) => e.to_ascii_uppercase(),
        None => String::new(),
    }
}

fn root(app: &mut CantusApp, out: &mut Vec<Item>, r: Root) {
    let open = app.samples.open.contains(&r.path);
    out.push(Item {
        kind: Kind::Root(r.kind.clone()),
        path: r.path.clone(),
        name: r.label,
        detail: String::new(),
        depth: 0,
    });
    if open {
        folder(app, out, &r.path, 1);
    }
}

/// A pack's licence folder holds no sounds: the browser leaves it out (the window shows it).
fn is_licences(e: &Entry, depth: usize) -> bool {
    depth == 1 && e.dir && e.name.eq_ignore_ascii_case("Licences")
}

fn folder(app: &mut CantusApp, out: &mut Vec<Item>, dir: &Path, depth: usize) {
    let entries: Vec<Entry> = app.samples.listing(dir).to_vec();
    for e in entries.into_iter().filter(|e| !is_licences(e, depth)) {
        if e.dir {
            let open = app.samples.open.contains(&e.path);
            out.push(Item {
                kind: Kind::Dir,
                path: e.path.clone(),
                name: e.name,
                detail: String::new(),
                depth,
            });
            if open {
                folder(app, out, &e.path, depth + 1);
            }
        } else {
            out.push(Item {
                kind: Kind::File,
                name: stem(&e.path),
                detail: format_tag(&e.path),
                path: e.path,
                depth,
            });
        }
    }
}

/// The browser, filling `ui`.
pub fn show(app: &mut CantusApp, ui: &mut Ui) {
    let s = Style::current();
    let k = s.k;
    let full = ui.max_rect();
    let header = Rect::from_min_size(full.min, vec2(full.width(), 32.0 * k));
    let search = Rect::from_min_size(
        pos2(full.left() + 8.0 * k, header.bottom() + 2.0 * k),
        vec2(full.width() - 16.0 * k, 26.0 * k),
    );
    let foot = Rect::from_min_max(pos2(full.left(), full.bottom() - 24.0 * k), full.max);
    with_brush(ui.painter(), |b| {
        b.register(
            full,
            "samples-browser",
            &[crate::kit::Tok::Panel, crate::kit::Tok::Text],
            &[],
        );
        b.rect(full, Fill::Solid(shade(s.t.panel, -0.035)));
        b.rect(header, Fill::Solid(s.t.panel));
        b.hline(
            full.left(),
            full.right(),
            header.bottom(),
            b.px(),
            shade(s.t.panel, -0.16),
        );
        b.hline(
            full.left(),
            full.right(),
            foot.top(),
            b.px(),
            shade(s.t.panel, -0.12),
        );
        let gr = Rect::from_center_size(
            pos2(header.left() + 16.0 * k, header.center().y),
            vec2(14.0, 14.0) * k,
        );
        crate::kit::glyph::paint(b, gr, Glyph::Browser, s.t.text_dim);
        paint::line(
            b,
            Rect::from_min_max(
                pos2(gr.right() + 8.0 * k, header.top()),
                pos2(header.right() - 64.0 * k, header.bottom()),
            ),
            &s,
            Src::Key("samples.title"),
            Role::Title,
            s.t.text,
            Align2::LEFT_CENTER,
        );
    });
    // The header's keys: add a folder of your own, and the packs window.
    let key = |i: f32| {
        Rect::from_center_size(
            pos2(header.right() - (14.0 + 26.0 * i) * k, header.center().y),
            vec2(22.0, 22.0) * k,
        )
    };
    let (content_key, add_key) = (key(0.0), key(1.0));
    let content_open = app.samples.window.open;
    if ui
        .scope_builder(UiBuilder::new().max_rect(content_key), |ui| {
            w::glyph_key(ui, Glyph::More, content_open, 22.0, &t("samples.content"))
        })
        .inner
        .clicked()
    {
        app.samples.window.open = !app.samples.window.open;
    }
    if ui
        .scope_builder(UiBuilder::new().max_rect(add_key), |ui| {
            w::glyph_key(ui, Glyph::Plus, false, 22.0, &t("samples.add_folder"))
        })
        .inner
        .clicked()
    {
        if let Some(dir) = rfd::FileDialog::new()
            .set_title(t("samples.add_folder"))
            .pick_folder()
        {
            app.add_sample_folder(dir);
        }
    }
    ui.scope_builder(UiBuilder::new().max_rect(search), |ui| {
        w::text_field(
            ui,
            egui::Id::new("samples-search"),
            &mut app.samples.query,
            "samples.search",
            search.width() / k,
        )
    });

    let mut top = search.bottom() + 8.0 * k;
    let mut starter_actions = None;
    let starter = starter_card(
        &mut starter_actions,
        app,
        ui,
        &s,
        Rect::from_min_max(
            pos2(full.left() + 8.0 * k, top),
            pos2(full.right() - 8.0 * k, foot.top()),
        ),
    );
    if let Some(r) = starter {
        top = r.bottom() + 10.0 * k;
    }
    let body = Rect::from_min_max(pos2(full.left(), top), pos2(full.right(), foot.top()));
    let rows = tree(app, ui, &s, body);
    foot_line(app, ui, &s, foot);
    app.samples.geometry = Some(BrowserGeometry {
        rect: full,
        search,
        rows,
        starter,
        starter_actions,
        content_key,
        add_folder_key: add_key,
    });
}

/// The tree (or the search results) in `body`, scrolled. Returns the rows drawn.
fn tree(app: &mut CantusApp, ui: &mut Ui, s: &Style, body: Rect) -> Vec<Row> {
    let k = s.k;
    let list = items(app);
    let row_h = 22.0 * k;
    let mut drawn = Vec::new();
    let mut clicked: Option<(Item, egui::Pos2)> = None;
    let mut dragged: Option<(PathBuf, egui::Pos2)> = None;
    let mut child = ui.new_child(UiBuilder::new().max_rect(body));
    egui::ScrollArea::vertical()
        .id_salt("samples-tree")
        .auto_shrink([false, false])
        .show(&mut child, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let width = ui.available_width();
            for item in &list {
                let h = match item.kind {
                    Kind::Section(_) => 24.0 * k,
                    _ => row_h,
                };
                let sense = match item.kind {
                    Kind::Section(_) | Kind::Hint(_) => Sense::hover(),
                    Kind::File => Sense::click_and_drag(),
                    _ => Sense::click(),
                };
                let (rect, resp) = ui.allocate_exact_size(vec2(width, h), sense);
                if !ui.is_rect_visible(rect) {
                    if matches!(item.kind, Kind::File | Kind::Dir | Kind::Root(_)) {
                        drawn.push(Row {
                            rect,
                            path: item.path.clone(),
                            dir: item.kind != Kind::File,
                            depth: item.depth,
                        });
                    }
                    continue;
                }
                let selected = app.samples.selected.as_deref() == Some(item.path.as_path())
                    && item.kind == Kind::File;
                let playing = item.kind == Kind::File && app.samples.preview.is_playing(&item.path);
                with_brush(ui.painter(), |b| match &item.kind {
                    Kind::Section(key) => paint::section(b, rect, s, key),
                    Kind::Hint(text) => paint::line(
                        b,
                        rect.shrink2(vec2(18.0 * k, 0.0)),
                        s,
                        Src::Str(text),
                        Role::Body,
                        s.t.text_dim,
                        Align2::LEFT_CENTER,
                    ),
                    kind => {
                        let open = app.samples.open.contains(&item.path);
                        let icon = match kind {
                            Kind::Root(RootKind::Pack(_)) => Icon::Pack,
                            Kind::File => Icon::Wave,
                            _ => Icon::Folder { open },
                        };
                        let look = RowLook {
                            hovered: resp.hovered(),
                            selected,
                            playing,
                            disclosure: (*kind != Kind::File).then_some(open),
                            dim: false,
                        };
                        paint::row(b, rect, s, item.depth, icon, &item.name, &item.detail, look);
                        if *kind == Kind::Root(RootKind::User) && resp.hovered() {
                            let x = Rect::from_center_size(
                                pos2(rect.right() - 12.0 * k, rect.center().y),
                                vec2(9.0, 9.0) * k,
                            );
                            crate::kit::glyph::paint(b, x, Glyph::Close, s.t.text_dim);
                        }
                    }
                });
                if resp.hovered() && item.kind == Kind::File {
                    crate::kit::hint::set_text(
                        ui.ctx(),
                        &item.name,
                        &t("samples.hint.file"),
                        crate::kit::hint::Caps::NONE,
                    );
                }
                if resp.clicked() {
                    clicked = Some((
                        item.clone(),
                        resp.interact_pointer_pos().unwrap_or(rect.center()),
                    ));
                }
                if resp.drag_started() && item.kind == Kind::File {
                    let from = ui
                        .ctx()
                        .input(|i| i.pointer.press_origin())
                        .unwrap_or(rect.center());
                    dragged = Some((item.path.clone(), from));
                }
                if matches!(item.kind, Kind::File | Kind::Dir | Kind::Root(_)) {
                    drawn.push(Row {
                        rect,
                        path: item.path.clone(),
                        dir: item.kind != Kind::File,
                        depth: item.depth,
                    });
                }
            }
            ui.allocate_space(vec2(width, 12.0 * k));
        });
    if let Some((item, at)) = clicked {
        let rect = drawn
            .iter()
            .find(|r| r.path == item.path)
            .map(|r| r.rect)
            .unwrap_or(Rect::NOTHING);
        on_click(app, item, at, rect, k);
    }
    if let Some((path, from)) = dragged {
        app.samples.selected = Some(path.clone());
        app.samples.drag = Some(Drag { path, from });
    }
    drawn
}

fn on_click(app: &mut CantusApp, item: Item, at: egui::Pos2, rect: Rect, k: f32) {
    match item.kind {
        Kind::Root(RootKind::User) if at.x > rect.right() - 22.0 * k => {
            app.remove_sample_folder(&item.path)
        }
        Kind::Root(_) | Kind::Dir => {
            if !app.samples.open.remove(&item.path) {
                app.samples.open.insert(item.path);
            }
        }
        Kind::File => {
            let again = app.samples.selected.as_deref() == Some(item.path.as_path());
            app.samples.selected = Some(item.path.clone());
            if again && app.samples.preview.is_playing(&item.path) {
                app.stop_audition();
            } else if app.settings.sample_audition {
                app.audition_sample(&item.path);
            }
        }
        _ => {}
    }
}

/// The one-click offer of the recommended pack, while no pack is installed.
fn starter_card(
    actions: &mut Option<Rect>,
    app: &mut CantusApp,
    ui: &mut Ui,
    s: &Style,
    area: Rect,
) -> Option<Rect> {
    if !app.samples.query.trim().is_empty() {
        return None;
    }
    let c = app.samples.content();
    if !c.installed().is_empty() {
        return None;
    }
    let pack = c.starter()?.clone();
    let status = c.status(&pack.id);
    let error = c.error(&pack.id).map(|e| e.to_string());
    let k = s.k;
    let lang = crate::i18n::locale().code();
    // The words wrap to the card's width; the card grows to hold them.
    let text_left = area.left() + 44.0 * k;
    let wrap = (area.right() - 12.0 * k - text_left).max(40.0);
    let mb = pack.download_size() as f64 / 1e6;
    let body = tf(
        "samples.starter.body",
        &[("pack", &pack.name.get(lang)), ("mb", &format!("{mb:.0}"))],
    );
    let body_g = ui
        .painter()
        .layout(body, Role::Body.font(s.tk()), s.t.text_dim, wrap);
    // What the licence asks, in a line: nothing (CC0), or credit to the author (CC BY,
    // whose full credit line Sound packs… shows before the download — ADR 0042).
    let licence = match (pack.licence_kind(), &pack.attribution) {
        (Some(l), Some(a)) if l.needs_credit() => tf(
            "samples.starter.licence_by",
            &[("licence", &l.name()), ("author", &a.author)],
        ),
        _ => t("samples.starter.licence"),
    };
    let lic_g = ui
        .painter()
        .layout(licence, Role::Micro.font(s.tk()), s.t.text_dim, wrap);
    let title_h = 26.0 * k;
    let text_h = title_h + body_g.size().y + 4.0 * k + lic_g.size().y;
    let actions_h = if matches!(status, cantus_content::Status::Busy(_)) {
        76.0 * k
    } else {
        40.0 * k
    };
    let rect = Rect::from_min_size(
        area.min,
        vec2(area.width(), 12.0 * k + text_h + 10.0 * k + actions_h),
    );
    with_brush(ui.painter(), |b| {
        crate::kit::light::raised(
            b,
            rect,
            s.r(5.0),
            crate::theme::tint(s.t.panel_raised, s.t.brand, 0.08),
            crate::kit::light::RaisedOpts {
                k,
                depth: s.t.depth,
                ..Default::default()
            },
        );
        let ir = Rect::from_min_size(rect.min + vec2(12.0, 16.0) * k, vec2(22.0, 18.0) * k);
        paint::paint_icon(b, ir, s, Icon::Pack, s.t.text_dim);
        paint::line(
            b,
            Rect::from_min_max(
                pos2(text_left, rect.top() + 10.0 * k),
                pos2(rect.right() - 10.0 * k, rect.top() + 10.0 * k + title_h),
            ),
            s,
            Src::Key("samples.starter.title"),
            Role::Title,
            s.t.text,
            Align2::LEFT_CENTER,
        );
    });
    let body_at = pos2(text_left, rect.top() + 10.0 * k + title_h);
    let lic_at = body_at + vec2(0.0, body_g.size().y + 4.0 * k);
    ui.painter().galley(body_at, body_g, s.t.text_dim);
    ui.painter().galley(lic_at, lic_g, s.t.text_dim);
    let inner = Rect::from_min_max(
        pos2(rect.left() + 12.0 * k, rect.bottom() - actions_h),
        pos2(rect.right() - 12.0 * k, rect.bottom() - 6.0 * k),
    );
    ui.scope_builder(UiBuilder::new().max_rect(inner), |ui| {
        ui.spacing_mut().item_spacing = vec2(6.0 * k, 6.0 * k);
        match &status {
            cantus_content::Status::Busy(phase) => {
                ui.horizontal(|ui| {
                    ui.set_width(inner.width());
                    if form::button(ui, &t("samples.cancel"), false, true) {
                        app.samples.content().cancel(&pack.id);
                    }
                });
                form::progress(ui, super::samples_content::fraction(phase.as_ref()));
                with_brush(ui.painter(), |b| {
                    let r = Rect::from_min_size(ui.cursor().min, vec2(inner.width(), 14.0 * k));
                    paint::line(
                        b,
                        r,
                        s,
                        Src::Str(&super::samples_content::phase_text(phase.as_ref())),
                        Role::Micro,
                        s.t.text_dim,
                        Align2::LEFT_CENTER,
                    );
                });
            }
            _ => {
                let row = ui.horizontal(|ui| {
                    if form::button(ui, &t("samples.starter.download"), true, true) {
                        app.samples.content().install(&pack.id);
                    }
                    if form::button(ui, &t("samples.more_packs"), false, true) {
                        app.samples.window.open = true;
                    }
                });
                *actions = Some(row.response.rect);
                if let Some(e) = &error {
                    with_brush(ui.painter(), |b| {
                        let r = Rect::from_min_size(ui.cursor().min, vec2(inner.width(), 14.0 * k));
                        paint::line(
                            b,
                            r,
                            s,
                            Src::Str(e),
                            Role::Micro,
                            s.t.danger,
                            Align2::LEFT_CENTER,
                        );
                    });
                }
            }
        }
    });
    Some(rect)
}

fn foot_line(app: &mut CantusApp, ui: &mut Ui, s: &Style, foot: Rect) {
    let text = app
        .samples
        .notice
        .clone()
        .or_else(|| app.samples.preview.error.clone());
    with_brush(ui.painter(), |b| {
        let r = foot.shrink2(vec2(10.0 * s.k, 0.0));
        match &text {
            Some(n) => paint::line(
                b,
                r,
                s,
                Src::Str(n),
                Role::Micro,
                s.t.text_dim,
                Align2::LEFT_CENTER,
            ),
            None => paint::line(
                b,
                r,
                s,
                Src::Key("samples.foot.hint"),
                Role::Micro,
                s.t.text_dim,
                Align2::LEFT_CENTER,
            ),
        }
    });
}

/// End of a frame: the dragged sample's chip and its target's ring, and a release
/// remembered for the next frame. Also the content window.
pub fn frame(app: &mut CantusApp, ctx: &egui::Context) {
    // A sound pack the open project needs and does not find: said, with what to do.
    super::samples_content::missing::frame(app, ctx);
    if app.samples.window.open {
        super::samples_content::window(app, ctx);
    }
    if let Some(drag) = app.samples.drag.clone() {
        let s = Style::current();
        let target = crate::samples::drop::hovered(app, ctx);
        if let Some(pos) = ctx.input(|i| i.pointer.latest_pos()) {
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Tooltip,
                egui::Id::new("samples-drag"),
            ));
            with_brush(&painter, |b| {
                if let Some((r, _)) = &target {
                    paint::target_ring(b, *r, &s);
                }
                paint::drag_chip(b, pos, &s, &stem(&drag.path), target.is_some());
            });
        }
        ctx.request_repaint();
    }
    crate::samples::drop::finish_drag(app, ctx);
}
