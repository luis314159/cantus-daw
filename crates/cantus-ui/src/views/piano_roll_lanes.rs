//! The piano roll's **event lanes** under the notes (ADR 0068): FL's event editor,
//! Ableton's clip envelopes, Cubase's controller lanes. Beside the velocity lane that was
//! there, a clip shows the lanes it holds — pitch bend, channel pressure, any CC 0…119 —
//! and the release velocity of its notes; each is added from the **+** in the velocity
//! lane's cell and removed by its own **×**, drawn with the pencil, line and curve tools
//! (the keys in its cell), its points dragged, Shift-dragged into a selection, deleted
//! with Delete or a right-click, and copied, pasted and moved with the notes (*lanes follow
//! the notes*, on by default).
//!
//! The rows share the area under the splitter; when they do not fit at
//! [`ROW_MIN`] each, the wheel over their cells scrolls them.

use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::glyph::{self, Glyph};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Role, Text};
use crate::kit::{self, Brush, Fill, Style};
use cantus_core::clip_lanes::{ClipLane, LaneKind, MAX_CC, NAMED};
use cantus_core::{Clip, ClipId, Command, TrackId};
use egui::{pos2, vec2, Align2, Rect, Sense, Ui};

#[path = "piano_roll_lanes_edit.rs"]
mod edit;
pub use edit::delete_selected;
pub(super) use edit::{ctl_input, paint_ctl};

/// The least height a row is drawn at; below it the rows scroll.
pub const ROW_MIN: f32 = 40.0;

/// One row of the lane area: the velocity lane that was always there, the notes' release
/// velocities, or a clip's controller lane.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum LaneRow {
    Velocity,
    Release,
    Ctl(LaneKind),
    /// The selected notes' per-note expression of one kind (ADR 0080).
    Expr(cantus_core::model::note_expr::ExprKind),
}

/// How a controller lane is drawn on.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum LaneTool {
    /// Freehand: the stroke as drawn, thinned.
    #[default]
    Pencil,
    /// A straight line from the press to the release.
    Line,
    /// A curve: the line bent towards its start (Alt bends it the other way).
    Curve,
}

impl LaneTool {
    pub const ALL: [LaneTool; 3] = [LaneTool::Pencil, LaneTool::Line, LaneTool::Curve];

    fn glyph(self) -> Glyph {
        match self {
            LaneTool::Pencil => Glyph::LanePencil,
            LaneTool::Line => Glyph::LaneLine,
            LaneTool::Curve => Glyph::LaneCurve,
        }
    }

    fn label_key(self) -> &'static str {
        match self {
            LaneTool::Pencil => "automodes.tool.pencil",
            LaneTool::Line => "automodes.tool.line",
            LaneTool::Curve => "automodes.tool.curve",
        }
    }

    fn hint_key(self) -> &'static str {
        match self {
            LaneTool::Pencil => "automodes.tool.pencil_tip",
            LaneTool::Line => "automodes.tool.line_tip",
            LaneTool::Curve => "automodes.tool.curve_tip",
        }
    }
}

/// The lane area's own state.
#[derive(Debug)]
pub struct LanesView {
    /// The release velocity lane is shown (a view of the notes; the other lanes are the
    /// clip's own and shown when it has them).
    pub release: bool,
    /// Moving, copying and pasting notes carries the controllers under them (Cubase's
    /// *controllers follow events*). On by default.
    pub follow: bool,
    pub tool: LaneTool,
    /// How far the rows are scrolled, in points, when more are shown than fit.
    pub scroll: f32,
    /// The points selected, per lane: indices into the lane's points.
    pub sel: Vec<(LaneKind, Vec<usize>)>,
    /// A stroke, a line or a move under way.
    pub(super) drag: Option<edit::LaneDrag>,
    /// The add menu's page: `None` its first, `Some(k)` the controllers `20k…20k+19`.
    page: Option<u8>,
    /// Strokes so far: each its own undo step.
    pub(super) stroke: u64,
    /// The per-note expression rows shown (bend, pressure, timbre — ADR 0080).
    pub expr: [bool; 3],
    /// What the notes show of their expression: the bend as a line through the note,
    /// pressure as the line's weight, timbre as its colour.
    pub on_notes: [bool; 3],
    /// An expression stroke under way.
    pub(super) expr_drag: Option<super::expr::Stroke>,
}

impl Default for LanesView {
    fn default() -> Self {
        LanesView {
            release: false,
            follow: true,
            tool: LaneTool::Pencil,
            scroll: 0.0,
            sel: Vec::new(),
            drag: None,
            page: None,
            stroke: 0,
            expr: [false; 3],
            on_notes: [true; 3],
            expr_drag: None,
        }
    }
}

impl LanesView {
    /// The points selected in `kind`'s lane.
    pub fn selected(&self, kind: LaneKind) -> &[usize] {
        self.sel
            .iter()
            .find(|(k, _)| *k == kind)
            .map_or(&[], |(_, v)| v.as_slice())
    }

    pub fn has_selection(&self) -> bool {
        self.sel.iter().any(|(_, v)| !v.is_empty())
    }

    pub fn set_selected(&mut self, kind: LaneKind, idx: Vec<usize>) {
        self.sel.retain(|(k, _)| *k != kind);
        if !idx.is_empty() {
            self.sel.push((kind, idx));
        }
    }
}

/// The rows a clip shows: velocity, the release when asked for, then its lanes in order.
pub fn rows(v: &LanesView, clip: &Clip) -> Vec<LaneRow> {
    let mut out = Vec::with_capacity(2 + clip.lanes.len());
    out.push(LaneRow::Velocity);
    if v.release {
        out.push(LaneRow::Release);
    }
    for (k, on) in cantus_core::model::note_expr::ExprKind::ALL
        .iter()
        .zip(v.expr)
    {
        if on {
            out.push(LaneRow::Expr(*k));
        }
    }
    out.extend(clip.lanes.iter().map(|l| LaneRow::Ctl(l.kind)));
    out
}

/// One row laid out: its body under the grid and its cell beside it.
#[derive(Copy, Clone, Debug)]
pub struct RowRects {
    pub row: LaneRow,
    pub body: Rect,
    pub cell: Rect,
}

/// The rows in the area (`body` under the grid, `cells` beside it), each at least
/// [`ROW_MIN`] tall, scrolled by `scroll` when they do not fit: the visible ones, cut to
/// the area, and how far they can scroll.
pub fn layout(
    body: Rect,
    cells: Rect,
    rows: &[LaneRow],
    scroll: f32,
    s: &Style,
) -> (Vec<RowRects>, f32) {
    let n = rows.len().max(1) as f32;
    let h = (body.height() / n).max(s.s(ROW_MIN));
    let total = h * n;
    let max_scroll = (total - body.height()).max(0.0);
    let scroll = scroll.clamp(0.0, max_scroll);
    let mut out = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        let top = body.top() + i as f32 * h - scroll;
        let r = Rect::from_min_max(pos2(body.left(), top), pos2(body.right(), top + h));
        if r.bottom() <= body.top() || r.top() >= body.bottom() {
            continue;
        }
        let c = Rect::from_min_max(pos2(cells.left(), r.top()), pos2(cells.right(), r.bottom()));
        out.push(RowRects {
            row: *row,
            body: r.intersect(body),
            cell: c.intersect(cells),
        });
    }
    (out, max_scroll)
}

/// A lane's name, in the interface's language: "Mod wheel", "CC 20", "Pitch bend".
pub fn lane_name(kind: LaneKind) -> String {
    match kind {
        LaneKind::Cc(n) if kind.name_key() == "automodes.cc.n" => {
            crate::i18n::tf("automodes.cc.n", &[("n", &n)])
        }
        k => t(k.name_key()),
    }
}

fn row_name(row: LaneRow) -> String {
    match row {
        LaneRow::Velocity => t("pianoroll.velocity"),
        LaneRow::Release => t("automodes.lane.release"),
        LaneRow::Ctl(k) => lane_name(k),
        LaneRow::Expr(k) => t(k.key()),
    }
}

/// Where a cell's parts go: its name on top, the × (a lane) or the + (the velocity row) at
/// its right, the tool keys under the name (a lane, when the row is tall enough).
struct CellParts {
    name: Rect,
    action: Option<Rect>,
    tools: Option<[Rect; 3]>,
}

fn cell_parts(cell: Rect, row: LaneRow, s: &Style) -> CellParts {
    let pad = s.s(4.0);
    let line = s.s(18.0);
    let top = Rect::from_min_size(
        cell.min + vec2(pad, pad),
        vec2(cell.width() - 2.0 * pad, line),
    );
    let key = s.s(14.0);
    let action = match row {
        LaneRow::Release => None,
        _ => Some(Rect::from_center_size(
            pos2(top.right() - key * 0.5, top.center().y),
            vec2(key, key),
        )),
    };
    let name = match action {
        Some(a) => Rect::from_min_max(top.min, pos2(a.left() - s.s(2.0), top.bottom())),
        None => top,
    };
    let tools = match row {
        LaneRow::Ctl(_) | LaneRow::Expr(_) if cell.height() >= line + key + 3.0 * pad => {
            let y = top.bottom() + pad + key * 0.5;
            let w = key + s.s(3.0);
            Some([0, 1, 2].map(|i| {
                Rect::from_center_size(
                    pos2(top.left() + key * 0.5 + i as f32 * w, y),
                    vec2(key, key),
                )
            }))
        }
        _ => None,
    };
    CellParts {
        name,
        action,
        tools,
    }
}

/// Paint the cells: each row's name in a well, its × or +, a lane's tool keys.
pub fn paint_cells(b: &mut Brush, v: &LanesView, rows: &[RowRects], s: &Style) {
    let t = &s.t;
    for rr in rows {
        b.register(rr.cell, "lane_target", &[kit::Tok::Panel], &[]);
        b.rect(b.snap_rect(rr.cell), Fill::Solid(t.panel));
        // A hairline between rows.
        b.hline(
            rr.cell.left(),
            rr.body.right(),
            rr.cell.top(),
            b.px(),
            t.grid,
        );
        let parts = cell_parts(rr.cell, rr.row, s);
        if parts.name.height() > rr.cell.height() {
            continue;
        }
        if let Some(a) = parts.action {
            let g = if rr.row == LaneRow::Velocity {
                Glyph::Plus
            } else {
                Glyph::Close
            };
            glyph::paint(b, a.shrink(s.s(2.0)), g, t.text_dim);
        }
        if let Some(keys) = parts.tools {
            for (k, tool) in keys.iter().zip(LaneTool::ALL) {
                let lit = v.tool == tool;
                if lit {
                    b.rrect(
                        *k,
                        s.r(2.0),
                        Fill::Solid(kit::color::with_alpha(t.highlight, 0.35)),
                    );
                }
                let c = if lit { t.highlight_glyph } else { t.text_dim };
                glyph::paint(b, k.shrink(s.s(1.5)), tool.glyph(), c);
            }
        }
    }
}

/// The cells as controls: a lane's × removes it (one undo step), its tool keys pick the
/// tool, the velocity row's + opens the add menu; the wheel over the cells scrolls the
/// rows when they overflow.
pub fn cell_input(
    app: &mut CantusApp,
    ui: &mut Ui,
    rows: &[RowRects],
    max_scroll: f32,
    cells: Rect,
    track: TrackId,
    clip: ClipId,
) {
    let s = Style::current();
    if max_scroll > 0.0 && ui.rect_contains_pointer(cells) {
        let dy = ui.input(|i| i.smooth_scroll_delta.y);
        if dy != 0.0 {
            let v = &mut app.piano_roll.lanes;
            v.scroll = (v.scroll - dy).clamp(0.0, max_scroll);
        }
    }
    for rr in rows {
        let parts = cell_parts(rr.cell, rr.row, &s);
        let name = ui.interact(
            parts.name.intersect(rr.cell),
            ui.id().with(("lane-cell", rr.row)),
            Sense::hover(),
        );
        if name.hovered() {
            hint::set_text(
                ui.ctx(),
                &row_name(rr.row),
                &t(row_hint(rr.row)),
                Caps::NONE,
            );
        }
        if let Some(a) = parts.action.filter(|a| rr.cell.contains_rect(*a)) {
            let resp = ui.interact(a, ui.id().with(("lane-act", rr.row)), Sense::click());
            match rr.row {
                LaneRow::Velocity => add_menu(app, ui, &resp, track, clip),
                LaneRow::Ctl(kind) => {
                    if resp.hovered() {
                        hint::set_text(ui.ctx(), &t("automodes.lane.remove"), "", Caps::NONE);
                    }
                    if resp.clicked() {
                        remove_lane(app, track, clip, kind);
                    }
                }
                LaneRow::Expr(kind) => {
                    if resp.hovered() {
                        hint::set_text(ui.ctx(), &t("mpe.lane.hide"), "", Caps::NONE);
                    }
                    if resp.clicked() {
                        app.piano_roll.lanes.expr[kind as usize] = false;
                    }
                }
                LaneRow::Release => {}
            }
        }
        if let Some(keys) = parts.tools {
            for (k, tool) in keys.iter().zip(LaneTool::ALL) {
                if !rr.cell.contains_rect(*k) {
                    continue;
                }
                let resp = ui.interact(
                    *k,
                    ui.id().with(("lane-tool", rr.row, tool.label_key())),
                    Sense::click(),
                );
                if resp.hovered() {
                    hint::set_text(
                        ui.ctx(),
                        &t(tool.label_key()),
                        &t(tool.hint_key()),
                        Caps::NONE,
                    );
                }
                if resp.clicked() {
                    app.piano_roll.lanes.tool = tool;
                }
            }
        }
    }
}

fn row_hint(row: LaneRow) -> &'static str {
    match row {
        LaneRow::Velocity => "pianoroll.velocity_tip",
        LaneRow::Release => "automodes.lane.release_tip",
        LaneRow::Ctl(LaneKind::PitchBend) => "automodes.lane.bend_tip",
        LaneRow::Ctl(_) => "automodes.lane.ctl_tip",
        LaneRow::Expr(_) => "mpe.lane.tip",
    }
}

/// Take `kind`'s lane off the clip: one undo step.
fn remove_lane(app: &mut CantusApp, track: TrackId, clip: ClipId, kind: LaneKind) {
    let Some(c) = app.project().track(track).and_then(|t| t.clip(clip)) else {
        return;
    };
    let lanes: Vec<ClipLane> = c.lanes.iter().filter(|l| l.kind != kind).cloned().collect();
    app.piano_roll.lanes.set_selected(kind, Vec::new());
    app.exec(Command::SetClipLanes { track, clip, lanes });
}

/// Give the clip a lane of `kind`, at its rest value: one undo step (its first stroke is
/// another).
pub fn add_lane(app: &mut CantusApp, track: TrackId, clip: ClipId, kind: LaneKind) -> bool {
    let Some(c) = app.project().track(track).and_then(|t| t.clip(clip)) else {
        return false;
    };
    if c.lanes.iter().any(|l| l.kind == kind) {
        return false;
    }
    let mut lanes = c.lanes.clone();
    let start = c.offset;
    lanes.push(ClipLane::new(
        kind,
        vec![cantus_core::AutoPoint::new(start, kind.rest())],
    ));
    let ok = app.exec(Command::SetClipLanes { track, clip, lanes });
    if ok {
        // Room for it: the area grows by a row, up to its most.
        let v = &mut app.piano_roll;
        v.vel_h = (v.vel_h + ROW_MIN * 1.5).min(super::VEL_MAX);
    }
    ok
}

/// The + menu: the release velocity lane, the lanes the clip does not have yet, *Other
/// controller ▸* (every number in pages of twenty), and *Lanes follow the notes*.
fn add_menu(app: &mut CantusApp, ui: &mut Ui, resp: &egui::Response, track: TrackId, clip: ClipId) {
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            &t("automodes.lane.add"),
            &t("automodes.lane.add_tip"),
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    if resp.clicked() {
        app.piano_roll.lanes.page = None;
    }
    let have: Vec<LaneKind> = app
        .project()
        .track(track)
        .and_then(|t| t.clip(clip))
        .map(|c| c.lanes.iter().map(|l| l.kind).collect())
        .unwrap_or_default();
    let mut picked: Option<Pick> = None;
    egui::Popup::menu(resp)
        .frame(crate::views::clips_chrome::menu_frame())
        .show(|ui| {
            let page = app.piano_roll.lanes.page;
            let (items, picks) = menu_items(&app.piano_roll.lanes, &have, page);
            let refs: Vec<kit::menu::Item<'_>> = items.iter().map(|i| i.item()).collect();
            if let Some(i) = kit::widgets::menu_list(ui, &refs, 230.0).1.clicked {
                picked = picks.get(i).copied().flatten();
                if !matches!(picked, Some(Pick::Page(_))) {
                    ui.close();
                }
            }
        });
    match picked {
        Some(Pick::Release) => {
            let v = &mut app.piano_roll.lanes;
            v.release = !v.release;
        }
        Some(Pick::Follow) => {
            let v = &mut app.piano_roll.lanes;
            v.follow = !v.follow;
        }
        Some(Pick::Page(p)) => app.piano_roll.lanes.page = p,
        Some(Pick::Lane(kind)) => {
            add_lane(app, track, clip, kind);
        }
        Some(Pick::Expr(i)) => {
            let v = &mut app.piano_roll.lanes;
            v.expr[i] = !v.expr[i];
        }
        None => {}
    }
}

#[derive(Copy, Clone, Debug)]
enum Pick {
    Release,
    Follow,
    Page(Option<u8>),
    Lane(LaneKind),
    /// A per-note expression row (ADR 0080), by kind.
    Expr(usize),
}

/// A menu row owning its words.
struct Row {
    words: String,
    check: kit::menu::Check,
    band: bool,
    submenu: bool,
}

impl Row {
    fn item(&self) -> kit::menu::Item<'_> {
        if self.band {
            return kit::menu::Item::Band(text::Src::Str(&self.words));
        }
        kit::menu::Item::Action {
            label: text::Src::Str(&self.words),
            check: self.check,
            shortcut: None,
            submenu: self.submenu,
            enabled: true,
        }
    }
}

fn menu_items(v: &LanesView, have: &[LaneKind], page: Option<u8>) -> (Vec<Row>, Vec<Option<Pick>>) {
    use kit::menu::Check;
    let mut rows = Vec::new();
    let mut picks = Vec::new();
    let mut push = |words: String, check: Check, band: bool, submenu: bool, pick: Option<Pick>| {
        rows.push(Row {
            words,
            check,
            band,
            submenu,
        });
        picks.push(pick);
    };
    match page {
        None => {
            push(
                t("automodes.lane.band_notes"),
                Check::None,
                true,
                false,
                None,
            );
            push(
                t("automodes.lane.release"),
                if v.release { Check::On } else { Check::Off },
                false,
                false,
                Some(Pick::Release),
            );
            push(
                t("automodes.lane.follow"),
                if v.follow { Check::On } else { Check::Off },
                false,
                false,
                Some(Pick::Follow),
            );
            // Per-note expression (ADR 0080): the selected notes' curves, each a row.
            push(t("mpe.lane.band"), Check::None, true, false, None);
            for (i, k) in cantus_core::model::note_expr::ExprKind::ALL
                .iter()
                .enumerate()
            {
                push(
                    t(k.key()),
                    if v.expr[i] { Check::On } else { Check::Off },
                    false,
                    false,
                    Some(Pick::Expr(i)),
                );
            }
            push(t("automodes.lane.band_ctl"), Check::None, true, false, None);
            for kind in [LaneKind::PitchBend, LaneKind::Pressure]
                .into_iter()
                .chain(NAMED.iter().map(|n| LaneKind::Cc(*n)))
                .filter(|k| !have.contains(k))
            {
                push(
                    lane_name(kind),
                    Check::None,
                    false,
                    false,
                    Some(Pick::Lane(kind)),
                );
            }
            push(
                t("automodes.lane.other"),
                Check::None,
                false,
                true,
                Some(Pick::Page(Some(0))),
            );
        }
        Some(p) => {
            push(
                t("automodes.lane.back"),
                Check::None,
                false,
                false,
                Some(Pick::Page(None)),
            );
            let pages = (MAX_CC as u16 + 1).div_ceil(20) as u8;
            // The pages themselves, then this page's numbers.
            for q in 0..pages {
                let (a, b) = (q as u16 * 20, (q as u16 * 20 + 19).min(MAX_CC as u16));
                let words = crate::i18n::tf("automodes.lane.page", &[("a", &a), ("b", &b)]);
                let check = if q == p {
                    Check::Radio(true)
                } else {
                    Check::Radio(false)
                };
                push(words, check, false, false, Some(Pick::Page(Some(q))));
            }
            push(t("automodes.lane.band_ctl"), Check::None, true, false, None);
            let (a, b) = (p as u16 * 20, (p as u16 * 20 + 19).min(MAX_CC as u16));
            for n in a..=b {
                let kind = LaneKind::Cc(n as u8);
                if have.contains(&kind) {
                    continue;
                }
                let words = format!("{n} · {}", lane_name(kind));
                push(words, Check::None, false, false, Some(Pick::Lane(kind)));
            }
        }
    }
    (rows, picks)
}

/// A row's name, whole, at its body's top left on a wash of the ground (the cells beside
/// are the keys' width: too narrow for "Release velocity").
pub fn paint_name(b: &mut Brush, row: LaneRow, body: Rect, s: &Style) {
    let t = &s.t;
    let words = row_name(row);
    let tx = Text::new(&words, Role::Micro, t.text).align(Align2::LEFT_CENTER);
    let h = s.s(15.0);
    if body.height() < h + s.s(4.0) {
        return;
    }
    let w = text::measure(b.ctx(), &tx, s.tk()).x + s.s(10.0);
    let r = Rect::from_min_size(body.min + vec2(s.s(4.0), s.s(3.0)), vec2(w, h));
    b.rrect(
        r,
        s.r(2.0),
        Fill::Solid(kit::color::with_alpha(t.panel, 0.85)),
    );
    let _ = text::paint(b, r.shrink2(vec2(s.s(5.0), 0.0)), &tx, s.tk());
}
