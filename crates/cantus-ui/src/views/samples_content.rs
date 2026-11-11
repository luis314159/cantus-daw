//! The *Content* window (ADR 0030): every pack in the manifest, what it is, what it
//! weighs, its licence and who made it; install, update, remove, cancel, and install
//! from an archive already on disk. A CC BY pack shows its credit line before and after
//! the download, and what the licence means in plain words (ADR 0042).

#[path = "samples_missing.rs"]
pub mod missing;
#[path = "samples_sounds.rs"]
pub mod sounds;

use super::interop::form;
use super::samples_paint as paint;
use super::workspace_settings::{frame, title};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::with_alpha;
use crate::kit::glyph::Glyph;
use crate::kit::text::{self, Role, Src, Text};
use crate::kit::{with_brush, Fill, Style};
use cantus_content::{Attribution, Licence, Pack, Phase, Source, Status};
use egui::{pos2, vec2, Align2, Context, Id, Pos2, Rect, Sense, Ui, UiBuilder, Vec2};

/// How far an install has got, 0..1.
pub fn fraction(phase: Option<&Phase>) -> f32 {
    match phase {
        Some(Phase::Downloading { done, total }) => {
            (*done as f64 / (*total).max(1) as f64) as f32 * 0.9
        }
        Some(Phase::Verifying) => 0.9,
        Some(Phase::Unpacking { done, total }) => {
            0.9 + 0.1 * (*done as f64 / (*total).max(1) as f64) as f32
        }
        // A pack built at install unpacks to 95 % and mixes its kit in the rest.
        Some(Phase::Building { done, total }) => {
            0.95 + 0.05 * (*done as f64 / (*total).max(1) as f64) as f32
        }
        _ => 0.0,
    }
}

/// What an install is doing, in words.
pub fn phase_text(phase: Option<&Phase>) -> String {
    let mb = |b: u64| format!("{:.1}", b as f64 / 1e6);
    match phase {
        None => t("samples.phase.starting"),
        Some(Phase::Downloading { done, total }) => tf(
            "samples.phase.downloading",
            &[("done", &mb(*done)), ("total", &mb(*total))],
        ),
        Some(Phase::Retrying { error, wait }) => tf(
            "samples.phase.retrying",
            &[("why", error), ("s", &wait.as_secs())],
        ),
        Some(Phase::Verifying) => t("samples.phase.verifying"),
        Some(Phase::Unpacking { .. }) => t("samples.phase.unpacking"),
        Some(Phase::Building { done, total }) => tf(
            "samples.phase.building",
            &[("done", &done.to_string()), ("total", &total.to_string())],
        ),
    }
}

/// The details table's label column: wide enough for its longest label in the interface's
/// language (sound review 2: "Espacio libre para instalar" was cut), 110 pt at least.
pub fn label_width(ctx: &Context, s: &Style, labels: &[&str]) -> f32 {
    let widest = labels
        .iter()
        .map(|l| {
            crate::kit::text::measure(
                ctx,
                &crate::kit::text::Text::new(l, Role::Body, s.t.text_dim),
                s.tk(),
            )
            .x
        })
        .fold(0.0f32, f32::max);
    (110.0 * s.k).max(widest + 12.0 * s.k)
}

/// The keys of the details table's labels, in its order.
pub const DETAIL_LABELS: [&str; 6] = [
    "samples.detail.download",
    "samples.detail.on_disk",
    "samples.detail.space",
    "samples.detail.files",
    "samples.detail.licence",
    "samples.detail.checked",
];

/// The details table's label column as the table lays it out: [`label_width`] of every
/// label, in the interface's language.
pub fn detail_column(ctx: &Context, s: &Style) -> f32 {
    let words = DETAIL_LABELS.map(t);
    let labels = words.each_ref().map(String::as_str);
    label_width(ctx, s, &labels)
}

/// What went wrong installing a pack, in the interface's language where Cantus knows the
/// words (no space: the sizes in words); otherwise the error as it says itself.
pub fn install_error_text(e: &cantus_content::install::InstallError) -> String {
    match e {
        cantus_content::install::InstallError::NoSpace { need, free } => tf(
            "samples.error.no_space",
            &[("need", &size(*need)), ("free", &size(*free))],
        ),
        e => e.to_string(),
    }
}

pub fn size(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.1} GB", bytes as f64 / 1e9)
    } else {
        format!("{:.0} MB", (bytes as f64 / 1e6).max(1.0))
    }
}

/// The window.
pub fn window(app: &mut CantusApp, ctx: &Context) {
    let s = Style::current();
    let screen = ctx.content_rect();
    // r39:lo-rest — wide enough for three covers beside a pack's details.
    let want = Vec2::new(980.0, 620.0) * s.k;
    let size = want.min(screen.size() - Vec2::new(40.0, 100.0));
    let mut open = true;
    egui::Window::new("cantus-content")
        .id(Id::new("cantus-content"))
        .title_bar(false)
        .frame(egui::Frame::NONE)
        .resizable(true)
        .default_size(size)
        .min_size(Vec2::new(560.0, 360.0).min(size))
        .default_pos(Pos2::new(
            screen.center().x - size.x * 0.5,
            screen.top() + 70.0,
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
            let header = Rect::from_min_size(rect.min, vec2(rect.width(), 30.0 * s.k));
            if title(ui, header, &s, Glyph::Browser, "samples.content.title") {
                open = false;
            }
            body(
                app,
                ui,
                &s,
                Rect::from_min_max(pos2(rect.left(), header.bottom()), rect.max),
            );
        });
    if !open {
        app.samples.window.open = false;
    }
}

fn body(app: &mut CantusApp, ui: &mut Ui, s: &Style, area: Rect) {
    let k = s.k;
    let all: Vec<Pack> = app.samples.content().manifest().packs.clone();
    // r39:lo-rest — the page's head (counts, filters, order) and foot (credits, where).
    let head = Rect::from_min_size(
        area.min + vec2(10.0, 6.0) * k,
        vec2(area.width() - 20.0 * k, 48.0 * k),
    );
    let foot = Rect::from_min_max(
        pos2(area.left() + 10.0 * k, area.bottom() - 30.0 * k),
        pos2(area.right() - 10.0 * k, area.bottom() - 4.0 * k),
    );
    packs_head(app, ui, s, head, &all);
    packs_foot(app, ui, s, foot);
    let area = Rect::from_min_max(
        pos2(area.left(), head.bottom() + 2.0 * k),
        pos2(area.right(), foot.top()),
    );
    let lang = crate::i18n::locale().code();
    let (filter, sort) = (app.samples.window.filter, app.samples.window.sort);
    let installed: Vec<String> = all
        .iter()
        .filter(|p| app.samples.content().is_installed(&p.id))
        .map(|p| p.id.clone())
        .collect();
    let packs = shown_packs(
        &all,
        filter,
        sort,
        |id| installed.iter().any(|i| i == id),
        lang,
    );
    if app.samples.window.selected.is_none() {
        app.samples.window.selected = packs.first().map(|p| p.id.clone());
    }
    let list_w = (area.width() * 0.5).clamp(180.0 * k, 470.0 * k);
    let list = Rect::from_min_max(
        area.min + vec2(8.0, 8.0) * k,
        pos2(area.left() + list_w, area.bottom() - 8.0 * k),
    );
    let detail = Rect::from_min_max(
        pos2(list.right() + 12.0 * k, area.top() + 10.0 * k),
        area.max - vec2(14.0, 10.0) * k,
    );
    with_brush(ui.painter(), |b| {
        crate::kit::well::paint(b, list, s, crate::kit::State::REST, None);
    });
    let mut child = ui.new_child(UiBuilder::new().max_rect(list.shrink(2.0 * k)));
    egui::ScrollArea::vertical()
        .id_salt("content-list")
        .auto_shrink([false, false])
        .show(&mut child, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            grid(app, ui, s, &packs, lang, sort == PackSort::Category);
        });
    let Some(pack) = app
        .samples
        .window
        .selected
        .as_ref()
        .and_then(|id| all.iter().find(|p| &p.id == id))
        .cloned()
    else {
        return;
    };
    let mut child = ui.new_child(UiBuilder::new().max_rect(detail));
    egui::ScrollArea::vertical()
        .id_salt("content-detail")
        .auto_shrink([false, false])
        .show(&mut child, |ui| {
            ui.set_width(detail.width() - 12.0 * k);
            ui.spacing_mut().item_spacing = vec2(6.0 * k, 6.0 * k);
            details(app, ui, s, &pack, lang);
        });
}

/// The Packs page's filters (des-views): every pack, the installed ones, or one category.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum PackFilter {
    #[default]
    All,
    Installed,
    Category(cantus_content::manifest::Category),
}

/// How the Packs page orders its covers: by category under their titles, or by name, or
/// largest first.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum PackSort {
    #[default]
    Category,
    Name,
    Size,
}

impl PackSort {
    pub fn key(self) -> &'static str {
        match self {
            PackSort::Category => "samples.sort.category",
            PackSort::Name => "samples.sort.name",
            PackSort::Size => "samples.sort.size",
        }
    }

    /// The next order, as the Sort key cycles.
    pub fn next(self) -> PackSort {
        match self {
            PackSort::Category => PackSort::Name,
            PackSort::Name => PackSort::Size,
            PackSort::Size => PackSort::Category,
        }
    }
}

/// The filters the page offers for `packs`: All, Installed, then each category they have,
/// in the manifest's order of categories.
pub fn pack_filters(packs: &[Pack]) -> Vec<PackFilter> {
    let mut cats: Vec<cantus_content::manifest::Category> = Vec::new();
    for p in packs {
        if !cats.contains(&p.category) {
            cats.push(p.category);
        }
    }
    cats.sort_by_key(|c| *c as u8);
    [PackFilter::All, PackFilter::Installed]
        .into_iter()
        .chain(cats.into_iter().map(PackFilter::Category))
        .collect()
}

/// The packs `filter` keeps, in `sort`'s order (`installed` says which are here).
pub fn shown_packs(
    packs: &[Pack],
    filter: PackFilter,
    sort: PackSort,
    installed: impl Fn(&str) -> bool,
    lang: &str,
) -> Vec<Pack> {
    let mut out: Vec<Pack> = packs
        .iter()
        .filter(|p| match filter {
            PackFilter::All => true,
            PackFilter::Installed => installed(&p.id),
            PackFilter::Category(c) => p.category == c,
        })
        .cloned()
        .collect();
    match sort {
        // Grouped by category, in the manifest's order within each.
        PackSort::Category => out.sort_by_key(|p| p.category as u8),
        PackSort::Name => out.sort_by_key(|p| p.name.get(lang).to_lowercase()),
        PackSort::Size => out.sort_by_key(|p| std::cmp::Reverse(p.download_size())),
    }
    out
}

/// The page's head (des-views, *Packs*): how many packs, how many are here and what they
/// weigh on disk; the filters as one segmented control; the order at the right.
fn packs_head(app: &mut CantusApp, ui: &mut Ui, s: &Style, head: Rect, packs: &[Pack]) {
    let k = s.k;
    let room = super::room::Room::of(&s.t);
    let lang = crate::i18n::locale().code();
    let here: Vec<&Pack> = packs
        .iter()
        .filter(|p| app.samples.content().is_installed(&p.id))
        .collect();
    let on_disk: u64 = here.iter().map(|p| p.installed_size).sum();
    let summary = tf(
        "samples.packs_summary",
        &[
            ("n", &packs.len().to_string()),
            ("here", &here.len().to_string()),
            (
                "size",
                &if on_disk == 0 {
                    "0 MB".to_string()
                } else {
                    size(on_disk)
                },
            ),
        ],
    );
    let line = Rect::from_min_size(head.min, vec2(head.width(), 18.0 * k));
    let row = Rect::from_min_max(pos2(head.left(), line.bottom() + 6.0 * k), head.max);
    let filters = pack_filters(packs);
    let labels: Vec<String> = filters
        .iter()
        .map(|f| match f {
            PackFilter::All => t("samples.filter.all"),
            PackFilter::Installed => t("samples.state.installed"),
            PackFilter::Category(c) => t(c.key()),
        })
        .collect();
    let widest = labels
        .iter()
        .map(|l| text::measure(ui.ctx(), &Text::new(l, Role::Control, room.ink1), s.tk()).x)
        .fold(0.0f32, f32::max);
    let sort_w = 150.0 * k;
    let seg_w = ((widest + 20.0 * k) * filters.len() as f32).min(row.width() - sort_w - 16.0 * k);
    let seg = Rect::from_min_size(row.min, vec2(seg_w, row.height()));
    let sort = Rect::from_min_max(pos2(row.right() - sort_w, row.top()), row.max);
    let resp = ui.interact(seg, Id::new("packs-filters"), Sense::click());
    let hovered = resp
        .hover_pos()
        .and_then(|p| crate::kit::segmented::hit(seg, filters.len(), p.x));
    if resp.clicked() {
        if let Some(i) = hovered {
            app.samples.window.filter = filters[i];
        }
    }
    let chosen = filters
        .iter()
        .position(|f| *f == app.samples.window.filter)
        .unwrap_or(0);
    let sresp = ui.interact(sort, Id::new("packs-sort"), Sense::click());
    if sresp.clicked() {
        app.samples.window.sort = app.samples.window.sort.next();
    }
    if sresp.hovered() {
        crate::kit::hint::set_text(
            ui.ctx(),
            &t("samples.sort_tip"),
            "",
            crate::kit::hint::Caps::NONE,
        );
    }
    let order = t(app.samples.window.sort.key());
    let _ = lang;
    with_brush(ui.painter(), |b| {
        paint::line(
            b,
            line,
            s,
            Src::Str(&summary),
            Role::Label,
            room.ink3,
            Align2::LEFT_CENTER,
        );
        let srcs: Vec<Src<'_>> = labels.iter().map(|l| Src::Str(l)).collect();
        crate::kit::segmented::paint(
            b,
            seg,
            s,
            crate::kit::State::of(&resp, false),
            &srcs,
            chosen,
            hovered,
        );
        // "Sort  Category": the word in ink 3, the order in ink 1, a chevron.
        if sresp.hovered() {
            b.rrect(sort, s.r(3.0), Fill::Solid(room.well_hi));
        }
        let g = Rect::from_center_size(
            pos2(sort.right() - 10.0 * k, sort.center().y),
            vec2(9.0, 9.0) * k,
        );
        crate::kit::glyph::paint(b, g, Glyph::Down, room.ink2);
        let value = Text::new(&order, Role::Control, room.ink1)
            .caps(false)
            .align(Align2::RIGHT_CENTER);
        let painted = text::paint(
            b,
            Rect::from_min_max(sort.min, pos2(g.left() - 6.0 * k, sort.bottom())),
            &value,
            s.tk(),
        );
        let right = painted.map_or(g.left(), |p| p.rect.left()) - 6.0 * k;
        let _ = text::paint(
            b,
            Rect::from_min_max(sort.min, pos2(right, sort.bottom())),
            &Text::key("samples.sort", Role::Label, room.ink3)
                .caps(false)
                .align(Align2::RIGHT_CENTER),
            s.tk(),
        );
    });
}

/// The page's foot: where CC BY packs name their authors, and where packs are kept (a
/// click shows the folder).
fn packs_foot(app: &mut CantusApp, ui: &mut Ui, s: &Style, foot: Rect) {
    let k = s.k;
    let room = super::room::Room::of(&s.t);
    let link_w = text::measure(
        ui.ctx(),
        &Text::key("samples.where_kept", Role::Label, room.accent),
        s.tk(),
    )
    .x + 4.0 * k;
    let link = Rect::from_min_max(pos2(foot.right() - link_w, foot.top()), foot.max);
    let resp = ui.interact(link, Id::new("packs-where"), Sense::click());
    with_brush(ui.painter(), |b| {
        b.hline(foot.left(), foot.right(), foot.top(), b.px(), room.edge);
        paint::line(
            b,
            Rect::from_min_max(foot.min, pos2(link.left() - 12.0 * k, foot.bottom())),
            s,
            Src::Key("samples.cc_by_note"),
            Role::Label,
            room.ink3,
            Align2::LEFT_CENTER,
        );
        let ink = if resp.hovered() {
            room.ink1
        } else {
            room.accent
        };
        paint::line(
            b,
            link,
            s,
            Src::Key("samples.where_kept"),
            Role::Label,
            ink,
            Align2::RIGHT_CENTER,
        );
    });
    if resp.hovered() {
        let root = app.samples.content().root().display().to_string();
        crate::kit::hint::set_text(
            ui.ctx(),
            &t("samples.where_kept_tip"),
            &root,
            crate::kit::hint::Caps::NONE,
        );
    }
    if resp.clicked() {
        let root = app.samples.content().root().to_path_buf();
        let _ = std::fs::create_dir_all(&root);
        let _ = crate::app::reveal(&root);
    }
}

/// A cover cell's measures at 100 % (des-views, *Packs*): the 128-px cover, its name, what
/// it holds, and its state on one line under them.
pub const COVER: f32 = 128.0;
const CELL_GAP: f32 = 16.0;
const CELL_H: f32 = COVER + 8.0 + 16.0 + 14.0 + 8.0 + 20.0;

/// Where a cell's parts go: the cover, the name, the meta line and the state line.
pub fn cell_rects(cell: Rect, k: f32) -> [Rect; 4] {
    let cover = Rect::from_min_size(cell.min, vec2(COVER, COVER) * k);
    let name = Rect::from_min_size(
        pos2(cell.left(), cover.bottom() + 8.0 * k),
        vec2(cell.width(), 16.0 * k),
    );
    let meta = Rect::from_min_size(
        pos2(cell.left(), name.bottom()),
        vec2(cell.width(), 14.0 * k),
    );
    let state = Rect::from_min_size(
        pos2(cell.left(), meta.bottom() + 8.0 * k),
        vec2(cell.width(), 20.0 * k),
    );
    [cover, name, meta, state]
}

/// What a cell's state line does when clicked.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CellAction {
    Get,
    Cancel,
    /// An installed pack: open it in the browser.
    Open,
}

/// The packs as covers (r39:lo-rest): 128 px, r 8, drawn from what each holds; under
/// each its name, what it is with its size and licence, and its state — *Installed* (a
/// check on the cover), *Update · 4 MB*, a 4-px bar while it is fetched with *Cancel*, or
/// an accent well *Get · 140 MB*. A click on a cover shows its details beside the grid.
fn grid(app: &mut CantusApp, ui: &mut Ui, s: &Style, packs: &[Pack], lang: &str, grouped: bool) {
    let k = s.k;
    let width = ui.available_width();
    let cell_w = COVER * k;
    let gap = CELL_GAP * k;
    let cols = (((width - 12.0 * k + gap) / (cell_w + gap)).floor() as usize).max(1);
    let mut cells = Vec::new();
    let mut last_cat = None;
    let mut col = 0;
    let mut row_rect: Option<Rect> = None;
    for p in packs {
        if grouped && last_cat != Some(p.category) {
            last_cat = Some(p.category);
            let (r, _) = ui.allocate_exact_size(vec2(width, 26.0 * k), Sense::hover());
            with_brush(ui.painter(), |b| paint::section(b, r, s, p.category.key()));
            col = 0;
            row_rect = None;
        }
        if col == 0 {
            let (r, _) = ui.allocate_exact_size(vec2(width, CELL_H * k + gap), Sense::hover());
            row_rect = Some(r);
        }
        let Some(row) = row_rect else { continue };
        let cell = Rect::from_min_size(
            pos2(
                row.left() + 12.0 * k + col as f32 * (cell_w + gap),
                row.top(),
            ),
            vec2(cell_w, CELL_H * k),
        );
        cells.push((cell, p.clone()));
        col = (col + 1) % cols;
    }
    let mut chosen = None;
    let mut acted = None;
    app.samples.window.covers.clear();
    for (cell, p) in &cells {
        let [cover, .., state] = cell_rects(*cell, k);
        app.samples.window.covers.push((p.id.clone(), cover, state));
    }
    for (cell, p) in &cells {
        if !ui.is_rect_visible(*cell) {
            continue;
        }
        let [cover, ..] = cell_rects(*cell, k);
        let status = match &app.samples.window.shown_busy {
            Some((id, phase)) if *id == p.id => Status::Busy(Some(phase.clone())),
            _ => app.samples.content().status(&p.id),
        };
        let selected = app.samples.window.selected.as_deref() == Some(p.id.as_str());
        // The cover with its name and what it is: a click there shows its details.
        let [_, _, meta, _] = cell_rects(*cell, k);
        let resp = ui.interact(
            cover.union(meta),
            Id::new(("pack-cover", &p.id)),
            Sense::click(),
        );
        let state = cell_rects(*cell, k)[3];
        let act = match status {
            Status::NotInstalled | Status::Update { .. } => Some(CellAction::Get),
            Status::Busy(_) => Some(CellAction::Cancel),
            Status::Installed { .. } => Some(CellAction::Open),
        };
        let act_resp =
            act.map(|_| ui.interact(state, Id::new(("pack-state", &p.id)), Sense::click()));
        let act_hovered = act_resp.as_ref().is_some_and(|r| r.hovered());
        with_brush(ui.painter(), |b| {
            pack_cell(
                b,
                *cell,
                s,
                p,
                &status,
                (selected, resp.hovered(), act_hovered),
                lang,
            )
        });
        if resp.hovered() {
            crate::kit::hint::set_text(
                ui.ctx(),
                p.name.get(lang),
                &t("samples.pack_tip"),
                crate::kit::hint::Caps::NONE,
            );
        }
        if resp.clicked() {
            chosen = Some(p.id.clone());
        }
        if act_resp.is_some_and(|r| r.clicked()) {
            acted = act.map(|a| (a, p.id.clone()));
        }
    }
    if let Some(id) = chosen {
        app.samples.window.selected = Some(id);
        app.samples.window.confirm_remove = None;
    }
    match acted {
        Some((CellAction::Get, id)) => {
            app.samples.window.selected = Some(id.clone());
            app.samples.content().install(&id);
        }
        Some((CellAction::Cancel, id)) => app.samples.content().cancel(&id),
        Some((CellAction::Open, id)) => {
            // Its sounds in the browser's tree, the search let go of.
            let dir = app.samples.content().pack_dir(&id);
            app.samples.query.clear();
            app.samples.scope = None;
            app.samples.open.insert(dir);
            app.samples.window.open = false;
        }
        None => {}
    }
}

/// One cover cell: `look` is (selected, the cover hovered, the state line hovered).
fn pack_cell(
    b: &mut crate::kit::Brush,
    cell: Rect,
    s: &Style,
    p: &Pack,
    status: &Status,
    look: (bool, bool, bool),
    lang: &str,
) {
    let k = s.k;
    let room = super::room::Room::of(&s.t);
    let (selected, hovered, act_hovered) = look;
    let [cover, name, meta, state] = cell_rects(cell, k);
    paint::paint_icon(b, cover, s, paint::cover_of(&p.id), room.ink2);
    // The chosen pack: an accent ring round its cover; the pointer's, a light one.
    if selected || hovered {
        let ring = if selected {
            room.accent
        } else {
            with_alpha(room.ink1, 0.35)
        };
        b.rrect_stroke(cover.expand(2.0 * k), 10.0 * k, 1.5 * k, ring);
    }
    // Installed: a check in `play` on a disc in the corner.
    if matches!(status, Status::Installed { .. } | Status::Update { .. }) {
        let c = pos2(cover.right() - 14.0 * k, cover.top() + 14.0 * k);
        b.dot(c, 9.0 * k, with_alpha(room.gutter, 0.78));
        let g = Rect::from_center_size(c, vec2(12.0, 12.0) * k);
        crate::kit::glyph_browser::paint(b, g, crate::kit::glyph_browser::Glyph::Check, room.play);
    }
    paint::line(
        b,
        name,
        s,
        Src::Str(p.name.get(lang)),
        Role::Control,
        room.ink1,
        Align2::LEFT_CENTER,
    );
    // What it is, how many files where the manifest lists them, its licence.
    let what = match &p.source {
        Source::Files(f) => format!(
            "{} \u{b7} {} \u{b7} {}",
            t(p.category.key()),
            f.len(),
            p.licence
        ),
        _ => format!("{} \u{b7} {}", t(p.category.key()), p.licence),
    };
    let small = Text::new(&what, Role::Label, room.ink3)
        .caps(false)
        .align(Align2::LEFT_CENTER)
        .policy(crate::kit::text::Policy::Elide(
            crate::kit::text::Elide::End,
        ));
    let _ = crate::kit::text::paint(b, meta, &small, s.tk() * 10.0 / 11.0);
    let label =
        |b: &mut crate::kit::Brush, r: Rect, text: &str, ink: egui::Color32, align: Align2| {
            paint::line(b, r, s, Src::Str(text), Role::Label, ink, align)
        };
    match status {
        Status::Installed { .. } => {
            label(
                b,
                state,
                &t("samples.state.installed"),
                room.ink2,
                Align2::LEFT_CENTER,
            );
            let ink = if act_hovered { room.ink1 } else { room.accent };
            label(b, state, &t("samples.open_pack"), ink, Align2::RIGHT_CENTER);
        }
        Status::Update { .. } => {
            label(
                b,
                state,
                &t("samples.state.installed"),
                room.ink2,
                Align2::LEFT_CENTER,
            );
            let ink = if act_hovered { room.ink1 } else { room.accent };
            let words = tf("samples.update_size", &[("size", &size(p.download_size()))]);
            label(b, state, &words, ink, Align2::RIGHT_CENTER);
        }
        Status::Busy(phase) => {
            let f = fraction(phase.as_ref());
            let bar = Rect::from_min_size(state.min, vec2(state.width(), 4.0 * k));
            b.rrect(bar, 2.0 * k, Fill::Solid(room.well));
            b.rrect(
                Rect::from_min_size(bar.min, vec2(bar.width() * f, bar.height())),
                2.0 * k,
                Fill::Solid(room.accent),
            );
            let line = Rect::from_min_max(pos2(state.left(), bar.bottom() + 2.0 * k), state.max);
            let pct = format!("{:.0}", f * 100.0);
            let words = tf(
                "samples.getting",
                &[("pct", &pct), ("size", &size(p.download_size()))],
            );
            label(b, line, &words, room.ink3, Align2::LEFT_CENTER);
            let ink = if act_hovered { room.ink1 } else { room.ink2 };
            label(b, line, &t("samples.cancel"), ink, Align2::RIGHT_CENTER);
        }
        Status::NotInstalled => {
            let fill = crate::kit::color::mix(
                room.well,
                room.accent,
                if act_hovered { 0.30 } else { 0.18 },
            );
            b.rrect(state, s.r(3.0), Fill::Solid(fill));
            let words = tf("samples.get", &[("size", &size(p.download_size()))]);
            let ink = if act_hovered { room.ink1 } else { room.accent };
            label(b, state, &words, ink, Align2::CENTER_CENTER);
        }
    }
}

/// Text that wraps to the width it is given (descriptions, credits, errors).
fn text_block(ui: &mut Ui, s: &Style, src: Src<'_>, role: Role, color: egui::Color32, _lines: f32) {
    let text = match src {
        Src::Str(t) => t.to_string(),
        Src::Key(k) => t(k),
    };
    let width = ui.available_width();
    let galley = ui.painter().layout(text, role.font(s.tk()), color, width);
    let (r, _) = ui.allocate_exact_size(vec2(width, galley.size().y), Sense::hover());
    ui.painter().galley(r.min, galley, color);
}

/// What the licence lets the user do, in a few words.
fn licence_words(pack: &Pack) -> String {
    if pack.needs_credit() {
        t("samples.licence.cc_by")
    } else {
        t("samples.licence.cc0")
    }
}

/// A CC BY pack's credit (ADR 0042 §3–4): the credit line — the manifest's before the
/// download, the receipt's once installed — and what the licence means, in plain words,
/// with the author's own words about crediting when the manifest quotes them.
/// A credit as the reader sees it (ADR 0042 §3): the composed line in the interface's
/// language (`samples.credit.line`; the receipt and the notices keep the English one), or
/// the licensor's own line word for word — with the licence's name and link after it when
/// the line leaves the link out (CC BY 4.0 §3(a)(1)(C)).
pub fn credit_text(spdx: &str, licence_url: &str, a: &Attribution) -> String {
    let name: &str = match Licence::from_spdx(spdx) {
        Some(l) => l.name(),
        None => spdx,
    };
    if a.line.trim().is_empty() {
        // CC0 is a dedication to the public domain, not a licence: its own wording.
        let key = if Licence::from_spdx(spdx) == Some(Licence::Cc0) {
            "samples.credit.line_cc0"
        } else {
            "samples.credit.line"
        };
        tf(
            key,
            &[
                ("title", &a.title),
                ("author", &a.author),
                ("url", &a.url),
                ("licence", &name),
                ("licence_url", &licence_url),
            ],
        )
    } else if a.line.contains(licence_url) {
        a.line.clone()
    } else {
        tf(
            "samples.credit.licensor",
            &[
                ("line", &a.line),
                ("licence", &name),
                ("licence_url", &licence_url),
            ],
        )
    }
}

fn credit(app: &mut CantusApp, ui: &mut Ui, s: &Style, pack: &Pack) {
    let receipt = app
        .samples
        .content()
        .installed()
        .iter()
        .find(|r| r.id == pack.id)
        .cloned();
    let installed = receipt.as_ref().and_then(|r| {
        r.attribution
            .as_ref()
            .map(|a| credit_text(&r.licence, &r.licence_url, a))
    });
    let line = installed.clone().or_else(|| {
        pack.attribution
            .as_ref()
            .map(|a| credit_text(&pack.licence, &pack.licence_url, a))
    });
    form::band(ui, "samples.detail.credit");
    if let Some(line) = &line {
        text_block(ui, s, Src::Str(line), Role::Body, s.t.text, 2.0);
    }
    if !pack.needs_credit() {
        // CC0 with the credit its author asked for: a courtesy, never a condition.
        text_block(
            ui,
            s,
            Src::Key("samples.cc0.courtesy"),
            Role::Micro,
            s.t.text_dim,
            2.0,
        );
        return;
    }
    if installed.is_some() {
        text_block(
            ui,
            s,
            Src::Str(&tf("samples.ccby.kept", &[("file", &pack.licence_file)])),
            Role::Micro,
            s.t.text_dim,
            2.0,
        );
    }
    let name = pack
        .licence_kind()
        .map_or(pack.licence.as_str(), |l| l.name());
    text_block(
        ui,
        s,
        Src::Str(&tf("samples.ccby.plain", &[("licence", &name)])),
        Role::Micro,
        s.t.text_dim,
        3.0,
    );
    let note = pack
        .attribution
        .as_ref()
        .map(|a| a.note.trim())
        .unwrap_or_default();
    let judge = if note.is_empty() {
        format!("{} {}", t("samples.ccby.track"), t("samples.ccby.usually"))
    } else {
        format!(
            "{} {}",
            t("samples.ccby.track"),
            tf("samples.ccby.author_says", &[("note", &note)])
        )
    };
    text_block(ui, s, Src::Str(&judge), Role::Micro, s.t.text_dim, 2.0);
}

fn details(app: &mut CantusApp, ui: &mut Ui, s: &Style, pack: &Pack, lang: &str) {
    let status = app.samples.content().status(&pack.id);
    let error = app
        .samples
        .content()
        .error(&pack.id)
        .map(install_error_text);
    text_block(
        ui,
        s,
        Src::Str(pack.name.get(lang)),
        Role::Title,
        s.t.text,
        1.4,
    );
    // The instruments it plays as, once installed: the main thing to do next, in the
    // pack's header so it is in view without scrolling (round 26).
    let sounds: Vec<sounds::SoundChoice> = app
        .samples
        .content()
        .instruments()
        .into_iter()
        .filter(|i| i.pack == pack.id)
        .map(sounds::SoundChoice)
        .collect();
    if !matches!(status, Status::Busy(_)) && !sounds.is_empty() {
        ui.horizontal_wrapped(|ui| {
            for (i, sound) in sounds.iter().enumerate() {
                let label = tf("samples.sound.new_track", &[("name", &sound.name())]);
                if form::button(ui, &label, i == 0, true) {
                    app.apply_sound(
                        crate::picker::PickPurpose::AddInstrument { index: None },
                        sound,
                    );
                }
            }
        });
    }
    text_block(
        ui,
        s,
        Src::Str(pack.description.get(lang)),
        Role::Body,
        s.t.text_dim,
        3.0,
    );
    let files = match &pack.source {
        Source::Files(f) => f.len().to_string(),
        Source::Zip(_) | Source::TarGz(_) => "—".into(),
    };
    // In `DETAIL_LABELS`' order; a pack built at install needs much more while it
    // installs (round 28).
    let values = [
        (size(pack.download_size()), Role::Value),
        (size(pack.installed_size), Role::Value),
        (size(pack.space_needed()), Role::Value),
        (files, Role::Value),
        (
            format!("{} — {}", pack.licence, licence_words(pack)),
            Role::Body,
        ),
        (pack.checked.clone(), Role::Value),
    ];
    let lw = detail_column(ui.ctx(), s);
    let row_h = 20.0 * s.k;
    for (key, (value, role)) in DETAIL_LABELS.into_iter().zip(values) {
        let label = t(key);
        let width = ui.available_width();
        // Words (the licence's) wrap in the value column rather than being cut: the label
        // column grew to its longest label in Spanish (fix4: the licence's words lost
        // their end).
        let wrapped = (role == Role::Body).then(|| {
            ui.painter().layout(
                value.clone(),
                role.font(s.tk()),
                s.t.text,
                (width - lw).max(1.0),
            )
        });
        let h = wrapped
            .as_ref()
            .map_or(row_h, |g| row_h.max(g.size().y + 4.0 * s.k));
        let (r, _) = ui.allocate_exact_size(vec2(width, h), Sense::hover());
        let first = Rect::from_min_size(r.min, vec2(r.width(), row_h));
        with_brush(ui.painter(), |b| {
            paint::line(
                b,
                Rect::from_min_max(first.min, pos2(first.left() + lw, first.bottom())),
                s,
                Src::Str(&label),
                Role::Body,
                s.t.text_dim,
                Align2::LEFT_CENTER,
            );
            if wrapped.is_none() {
                paint::line(
                    b,
                    Rect::from_min_max(pos2(first.left() + lw, first.top()), first.max),
                    s,
                    Src::Str(&value),
                    role,
                    s.t.text,
                    Align2::LEFT_CENTER,
                );
            }
        });
        if let Some(g) = wrapped {
            let line = g.rows.first().map_or(row_h, |row| row.height());
            let at = pos2(r.left() + lw, r.top() + (row_h - line) * 0.5);
            ui.painter().galley(at, g, s.t.text);
        }
    }
    if pack.needs_credit() || pack.attribution.is_some() {
        credit(app, ui, s, pack);
    }
    form::band(ui, "samples.detail.credits");
    for c in &pack.credits {
        text_block(ui, s, Src::Str(&c.name), Role::Body, s.t.text, 1.0);
        text_block(ui, s, Src::Str(&c.url), Role::Micro, s.t.text_dim, 1.0);
    }
    text_block(
        ui,
        s,
        Src::Str(&tf(
            "samples.detail.licence_kept",
            &[("file", &pack.licence_file)],
        )),
        Role::Micro,
        s.t.text_dim,
        2.0,
    );
    // The instruments it plays as (round 26), named before the download too.
    if !pack.instruments.is_empty() || !sounds.is_empty() {
        let names = if sounds.is_empty() {
            pack.instruments
                .iter()
                .map(|i| format!("\u{201c}{}\u{201d}", i.name.get(lang)))
                .collect::<Vec<_>>()
        } else {
            sounds
                .iter()
                .map(|c| format!("\u{201c}{}\u{201d}", c.name()))
                .collect()
        };
        let key = if sounds.is_empty() {
            "samples.sound.offered"
        } else {
            "samples.sound.ready"
        };
        text_block(
            ui,
            s,
            Src::Str(&tf(key, &[("names", &names.join(", "))])),
            Role::Body,
            s.t.text,
            2.0,
        );
    }
    form::band(ui, "samples.detail.actions");
    match &status {
        Status::Busy(phase) => {
            form::progress(ui, fraction(phase.as_ref()));
            text_block(
                ui,
                s,
                Src::Str(&phase_text(phase.as_ref())),
                Role::Micro,
                s.t.text_dim,
                1.0,
            );
            if form::button(ui, &t("samples.cancel"), false, true) {
                app.samples.content().cancel(&pack.id);
            }
        }
        _ => {
            let installed = !matches!(status, Status::NotInstalled);
            ui.horizontal_wrapped(|ui| {
                let label = match status {
                    Status::Update { .. } => t("samples.update"),
                    Status::Installed { .. } => t("samples.reinstall"),
                    _ => t("samples.install"),
                };
                if form::button(
                    ui,
                    &label,
                    !installed || matches!(status, Status::Update { .. }),
                    true,
                ) {
                    app.samples.content().install(&pack.id);
                }
                if installed {
                    if app.samples.window.confirm_remove.as_deref() == Some(pack.id.as_str()) {
                        if form::button(ui, &t("samples.remove_confirm"), true, true) {
                            let dir = app.samples.content().pack_dir(&pack.id);
                            let r = app.samples.content().remove(&pack.id);
                            // Its decoded samples leave the sample cache too (ADR 0050).
                            app.sweep_sample_cache_removed(dir);
                            app.samples.window.confirm_remove = None;
                            app.samples.notice = Some(match r {
                                Ok(()) => {
                                    tf("samples.notice.removed", &[("pack", &pack.name.get(lang))])
                                }
                                Err(e) => e.to_string(),
                            });
                            app.samples.refresh();
                        }
                    } else if form::button(ui, &t("samples.remove"), false, true) {
                        app.samples.window.confirm_remove = Some(pack.id.clone());
                    }
                    if form::button(ui, &t("samples.show_folder"), false, true) {
                        let dir = app.samples.content().pack_dir(&pack.id);
                        let _ = crate::app::reveal(&dir);
                    }
                }
                let archive = match pack.source {
                    Source::Zip(_) => Some(("zip", &["zip"][..])),
                    Source::TarGz(_) => Some(("tar.gz", &["gz", "tgz"][..])),
                    Source::Files(_) => None,
                };
                if let Some((label, exts)) =
                    archive.filter(|_| form::button(ui, &t("samples.from_file"), false, true))
                {
                    if let Some(f) = rfd::FileDialog::new().add_filter(label, exts).pick_file() {
                        app.samples.content().install_from_file(&pack.id, &f);
                    }
                }
            });
            if let Some(e) = error {
                text_block(ui, s, Src::Str(&e), Role::Micro, s.t.danger, 2.0);
            }
        }
    }
}

#[cfg(test)]
mod space_tests {
    use crate::i18n::{set_thread_locale, t, Locale};

    /// Sound review 2: the details' labels fit their column in both languages, and a pack
    /// refused for want of space says so in the interface's language, in sizes.
    #[test]
    fn the_space_label_fits_and_the_no_space_error_reads_in_words() {
        let ctx = egui::Context::default();
        crate::design::install_fonts(&ctx);
        let s = crate::kit::Style::current();
        for locale in [Locale::En, Locale::Es] {
            set_thread_locale(locale);
            ctx.run_ui(egui::RawInput::default(), |_| {})
                .textures_delta
                .clear();
            // The column as the window draws it is checked where it is drawn
            // (`samples_browser::the_details_labels_are_drawn_whole_and_clear_of_their_values`;
            // review of fix4: this test measured the column's helper against itself). Here
            // only what makes that test bite: the old fixed 110 pt column is too narrow for
            // the Spanish space label with its padding.
            if locale == Locale::Es {
                let label = t("samples.detail.space");
                let text = crate::kit::text::measure(
                    &ctx,
                    &crate::kit::text::Text::new(&label, crate::kit::text::Role::Body, s.t.text),
                    s.tk(),
                )
                .x;
                assert!(text + 12.0 * s.k > 110.0 * s.k, "{label}: {text}");
            }
            let e = cantus_content::install::InstallError::NoSpace {
                need: 7_614_567_890,
                free: 3_100_000_000,
            };
            let said = super::install_error_text(&e);
            assert!(said.contains("7.6 GB") && said.contains("3.1 GB"), "{said}");
            assert!(!said.contains("7614567890"), "{said}");
            if locale == Locale::Es {
                assert!(said.contains("necesita"), "{said}");
            }
        }
        set_thread_locale(Locale::En);
    }
}

#[cfg(test)]
mod pack_page_tests {
    use super::*;

    /// r39:lo-rest — the Packs page's filters and orders, on the bundled manifest.
    #[test]
    fn the_filters_and_orders_keep_and_sort_the_packs() {
        let packs = cantus_content::Manifest::bundled().packs;
        assert!(packs.len() >= 3);
        let filters = pack_filters(&packs);
        assert_eq!(filters[..2], [PackFilter::All, PackFilter::Installed]);
        let cats: Vec<_> = filters[2..]
            .iter()
            .map(|f| match f {
                PackFilter::Category(c) => *c as u8,
                _ => panic!("{f:?}"),
            })
            .collect();
        let mut sorted = cats.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(cats, sorted, "each category once, in order");
        let first = packs[0].id.clone();
        let here = |id: &str| id == first;
        let all = shown_packs(&packs, PackFilter::All, PackSort::Category, here, "en");
        assert_eq!(all.len(), packs.len());
        assert!(all
            .windows(2)
            .all(|w| w[0].category as u8 <= w[1].category as u8));
        let installed = shown_packs(&packs, PackFilter::Installed, PackSort::Name, here, "en");
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].id, first);
        let cat = packs[0].category;
        let one = shown_packs(
            &packs,
            PackFilter::Category(cat),
            PackSort::Name,
            here,
            "en",
        );
        assert!(!one.is_empty() && one.iter().all(|p| p.category == cat));
        let names = shown_packs(&packs, PackFilter::All, PackSort::Name, here, "en");
        assert!(names
            .windows(2)
            .all(|w| w[0].name.en.to_lowercase() <= w[1].name.en.to_lowercase()));
        let sizes = shown_packs(&packs, PackFilter::All, PackSort::Size, here, "en");
        assert!(sizes
            .windows(2)
            .all(|w| w[0].download_size() >= w[1].download_size()));
        assert_eq!(PackSort::Size.next(), PackSort::Category);
    }
}
