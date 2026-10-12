//! **The Routing tab**: the routing graph as a matrix (round 25, ADR 0041; design §5) —
//! strips → strips (sources down, destinations across, the master first, dock dividers),
//! tracks → strips beneath (a radio per track; the tracks block also shows notes into
//! effects, the owner's answer 8), the *All · In use* filter, the click / Ctrl / Alt
//! grammar of the desk, the keys of §5, the route menu (`views::route_menu`) on a
//! right-click. Opened with Alt+F9 (answer 9) and *View ▸ Routing*.
//!
//! **The grammar** — the desk's, cell by cell:
//!
//! - click an empty cell: a **send** at 0 dB, post-fader; **Ctrl**+click: the column
//!   becomes the row's **only output**; **Alt**+click: **key** a sidechain (one keyable
//!   insert is keyed at once, several open a menu, none offer a ducking compressor or a
//!   gate). The master column takes the output. A refused cell (a loop, no room) is
//!   hatched and says why in the hint bar; a click on it issues nothing.
//! - a send cell: drag up or down for its level (200 pt the whole travel, Shift fine),
//!   double-click for 0 dB, Ctrl+click to make it the output, right-click for the route
//!   menu; **Shift+drag down or up the column copies the send** (same destination, level,
//!   switch and tap) to the strips it crosses — one undo step.
//! - a track row is a radio: a click plays the track into that column.
//! - keys, while the matrix has focus (a click in it gives it): arrows move the focused
//!   cell, Home / End the first / last column, **Enter** adds or removes a send,
//!   **Ctrl+Enter** routes the output, **Alt+Enter** keys, **+ / −** the level by 1 dB
//!   (Shift 0.1), **Delete** removes the route, **Esc** gives the focus back. No letters
//!   and not Space: the transport and every other shortcut keep working.
//!
//! Every mutation goes through `crate::routing_actions` (one undo step each), except the
//! level (the desk's merging `set_strip_send`) and the column copy, a batch of
//! `SetStripSend` built here (`cmd.copy_send`).
//!
//! The grid is derived once per revision (`routing_cells.rs`); a frame paints the visible
//! cells only, into one brush (`routing_paint.rs`).

use super::clips_chrome::{self as chrome, Span, Toolbar, BAR};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::hint::{self, Caps};
use crate::kit::menu::{self, Check, Item};
use crate::kit::text::Src;
use crate::kit::{self, segmented, State, Style};
use crate::routing_actions::{level_text, refusal_text, Gesture as Grammar};
use cantus_core::routing::{Route, RouteKind};
use cantus_core::{db_to_linear, linear_to_db, Command, EffectId, Send, StripId, TrackId};
use egui::{pos2, vec2, Id, Key, Modifiers, Pos2, Rect, Sense, Vec2};

#[path = "routing_cells.rs"]
mod cells;
#[path = "routing_paint.rs"]
mod paint;

pub use cells::{Cell, Col, Grid, Head, Refused, Row, SendMark};

/// A cell's side at 100 %.
pub const CELL: f32 = 18.0;
/// The row headers' width: the number cell and the name.
pub const ROW_HEAD_W: f32 = 172.0;
/// The number cell's width in a row header.
pub const NUM_W: f32 = 26.0;
/// The column headers' height: the names on their side, the number chip under them.
pub const COL_HEAD_H: f32 = 108.0;
/// The band between the strips block and the tracks block.
pub const BAND_H: f32 = 20.0;
/// Above this many strips the matrix opens on *In use* (design §5).
pub const IN_USE_ABOVE: usize = 32;
/// A send's level travel, in points (the house knob rule); Shift is ten times finer.
pub const TRAVEL: f32 = 200.0;

/// The tab's own state (scroll, focused cell, filter), kept on the app.
#[derive(Debug, Default)]
pub struct RoutingView {
    grid: Grid,
    /// *In use* (`Some(true)`), *All* (`Some(false)`), or not chosen yet: *In use* above
    /// [`IN_USE_ABOVE`] strips.
    filter: Option<bool>,
    /// How far the body is scrolled, in points.
    scroll: Vec2,
    /// The focused cell, by what it is (so it survives a rebuild).
    focus: Option<(Row, Col)>,
    gesture: Option<Gesture>,
    key_menu: Option<KeyMenu>,
    geometry: Option<Geometry>,
    /// The hint for the cell under the pointer, and what it was written for.
    hint: (Option<HintKey>, String, String),
    /// The toolbar's texts, and what they were written for.
    summary: (Option<(u64, bool, crate::i18n::Locale)>, String, String),
}

/// What a cell's hint was written for: (revision and locale, row, column, keys held).
type HintKey = (u64, usize, usize, u8);

/// A drag in the body.
#[derive(Copy, Clone, Debug, PartialEq)]
enum Gesture {
    /// A send's level: the send as it was, and the travel so far.
    Level {
        from: StripId,
        send: Send,
        travel: f32,
    },
    /// Shift+drag along a column: the send of row `start` copied to every strip row
    /// between it and `end`.
    Copy {
        col: usize,
        start: usize,
        end: usize,
    },
}

/// The menu Alt+click opens when a key needs a choice.
#[derive(Clone, Debug, PartialEq)]
struct KeyMenu {
    from: StripId,
    host: StripId,
    at: Pos2,
    /// The inserts to choose from; empty: offer a ducking compressor or a gate.
    choices: Vec<EffectId>,
    /// The frame it opened in: the click that opened it does not close it.
    opened: u64,
}

/// Where the matrix was drawn last frame, in screen coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    pub body: Rect,
    pub row_head: Rect,
    pub col_head: Rect,
    pub cell: f32,
    pub band: f32,
    pub scroll: Vec2,
    pub rows: usize,
    pub cols: usize,
    pub strip_rows: usize,
}

impl Geometry {
    pub fn row_top(&self, r: usize) -> f32 {
        let band = if r >= self.strip_rows { self.band } else { 0.0 };
        self.body.top() - self.scroll.y + r as f32 * self.cell + band
    }

    pub fn col_left(&self, c: usize) -> f32 {
        self.body.left() - self.scroll.x + c as f32 * self.cell
    }

    /// The cell of row `r`, column `c`.
    pub fn cell_rect(&self, r: usize, c: usize) -> Rect {
        Rect::from_min_size(
            pos2(self.col_left(c), self.row_top(r)),
            vec2(self.cell, self.cell),
        )
    }

    /// The band between the strips and the tracks, across the headers and the body.
    pub fn band_rect(&self) -> Rect {
        let y = self.row_top(self.strip_rows) - self.band;
        Rect::from_min_max(
            pos2(self.row_head.left(), y),
            pos2(self.body.right(), y + self.band),
        )
    }

    /// The content's size: every column and row, and the band.
    pub fn content(&self) -> Vec2 {
        vec2(
            self.cols as f32 * self.cell,
            self.rows as f32 * self.cell + self.band,
        )
    }

    /// The row at a height on the screen (the band is none).
    pub fn row_at(&self, y: f32) -> Option<usize> {
        let y = y - (self.body.top() - self.scroll.y);
        let split = self.strip_rows as f32 * self.cell;
        let r = if y < split {
            y / self.cell
        } else if y < split + self.band {
            return None;
        } else {
            (y - self.band) / self.cell
        };
        (r >= 0.0 && (r as usize) < self.rows).then_some(r as usize)
    }

    /// The column at an x on the screen.
    pub fn col_at(&self, x: f32) -> Option<usize> {
        let c = (x - (self.body.left() - self.scroll.x)) / self.cell;
        (c >= 0.0 && (c as usize) < self.cols).then_some(c as usize)
    }

    /// The cell under `p`, inside the body.
    pub fn hit(&self, p: Pos2) -> Option<(usize, usize)> {
        if !self.body.contains(p) {
            return None;
        }
        Some((self.row_at(p.y)?, self.col_at(p.x)?))
    }

    /// The rows and columns at least partly in view.
    pub fn visible(&self) -> (std::ops::Range<usize>, std::ops::Range<usize>) {
        let r0 = (0..self.rows)
            .find(|r| self.row_top(*r) + self.cell > self.body.top())
            .unwrap_or(self.rows);
        let r1 = (r0..self.rows)
            .find(|r| self.row_top(*r) >= self.body.bottom())
            .unwrap_or(self.rows);
        let c0 = ((self.scroll.x / self.cell).floor().max(0.0) as usize).min(self.cols);
        let c1 = (((self.scroll.x + self.body.width()) / self.cell).ceil() as usize).min(self.cols);
        (r0..r1, c0..c1)
    }

    /// The scroll that brings cell `(r, c)` into view, from `scroll`.
    pub fn reveal(&self, r: usize, c: usize, scroll: Vec2) -> Vec2 {
        let mut s = scroll;
        let y = self.row_top(r) - self.body.top() + self.scroll.y;
        let x = c as f32 * self.cell;
        if y < s.y {
            s.y = y;
        } else if y + self.cell > s.y + self.body.height() {
            s.y = y + self.cell - self.body.height();
        }
        if x < s.x {
            s.x = x;
        } else if x + self.cell > s.x + self.body.width() {
            s.x = x + self.cell - self.body.width();
        }
        s
    }
}

impl RoutingView {
    /// Where the matrix was drawn last frame (`None` before the first), for the tests.
    pub fn geometry(&self) -> Option<&Geometry> {
        self.geometry.as_ref()
    }

    /// The grid shown last frame.
    pub fn grid(&self) -> &Grid {
        &self.grid
    }

    /// The focused cell.
    pub fn focus(&self) -> Option<(Row, Col)> {
        self.focus
    }

    /// Whether *In use* is on (chosen, or by the strip count).
    pub fn in_use(&self, strips: usize) -> bool {
        self.filter.unwrap_or(strips > IN_USE_ABOVE)
    }

    /// Choose *All* (`false`) or *In use* (`true`).
    pub fn set_in_use(&mut self, on: bool) {
        self.filter = Some(on);
        self.scroll = Vec2::ZERO;
    }
}

/// The route menu's owner id for the matrix.
pub fn owner() -> Id {
    Id::new("routing-matrix-menu")
}

/// The body's id: what holds the keyboard focus.
pub fn body_id() -> Id {
    Id::new("routing-matrix-body")
}

/// Draw the tab.
pub fn show(app: &mut CantusApp, ui: &mut egui::Ui) {
    let s = Style::current();
    let rev = app.bus.revision();
    let strips = app.project().strips.len();
    let in_use = app.routing_view.in_use(strips);
    let mut grid = std::mem::take(&mut app.routing_view.grid);
    grid.refresh(app.project(), rev, in_use);
    app.routing_view.grid = grid;
    toolbar(app, ui, rev, in_use, &s);
    let full = ui.available_rect_before_wrap();
    let bar = s.s(BAR);
    let least = vec2(
        s.s(ROW_HEAD_W + 3.0 * CELL) + bar,
        s.s(COL_HEAD_H + 2.0 * CELL) + bar,
    );
    if full.width() < least.x || full.height() < least.y {
        app.routing_view.geometry = None;
        return;
    }
    let corner = Rect::from_min_size(full.min, vec2(s.s(ROW_HEAD_W), s.s(COL_HEAD_H)));
    let body = Rect::from_min_max(corner.max, pos2(full.right() - bar, full.bottom() - bar));
    let v = &app.routing_view;
    let mut geo = Geometry {
        body,
        row_head: Rect::from_min_max(
            pos2(full.left(), body.top()),
            pos2(body.left(), body.bottom()),
        ),
        col_head: Rect::from_min_max(
            pos2(body.left(), full.top()),
            pos2(body.right(), body.top()),
        ),
        cell: s.s(CELL),
        band: if v.grid.rows.len() > v.grid.strip_rows {
            s.s(BAND_H)
        } else {
            0.0
        },
        scroll: v.scroll,
        rows: v.grid.rows.len(),
        cols: v.grid.cols.len(),
        strip_rows: v.grid.strip_rows,
    };
    let resp = ui.interact(body, body_id(), Sense::click_and_drag());
    navigate(app, ui, &mut geo, full, bar);
    heads(app, ui, &geo);
    let hover = ui
        .input(|i| i.pointer.hover_pos())
        .filter(|_| resp.hovered() || resp.dragged())
        .and_then(|p| geo.hit(p));
    body_input(app, ui, &resp, &geo, hover);
    keys(app, ui, &resp, &mut geo);
    {
        let a: &CantusApp = app;
        let v = &a.routing_view;
        let focused = resp.has_focus();
        let focus = v
            .focus
            .and_then(|(r, c)| Some((v.grid.row_of(r)?, v.grid.col_of(c)?)));
        let ring = focus.filter(|_| focused);
        kit::with_brush(&ui.painter().with_clip_rect(geo.body), |b| {
            paint::body(b, v, &geo, hover, ring, &s)
        });
        kit::with_brush(&ui.painter().with_clip_rect(geo.row_head), |b| {
            paint::row_heads(b, &v.grid, &geo, hover, focus, &s)
        });
        kit::with_brush(&ui.painter().with_clip_rect(geo.col_head), |b| {
            paint::col_heads(b, &v.grid, &geo, hover, focus, &s)
        });
        kit::with_brush(ui.painter(), |b| {
            paint::corner(b, corner, &s);
            for c in [
                Rect::from_min_max(
                    pos2(full.right() - bar, full.top()),
                    pos2(full.right(), body.top()),
                ),
                Rect::from_min_max(
                    pos2(full.left(), body.bottom()),
                    pos2(body.left(), full.bottom()),
                ),
                Rect::from_min_max(body.max, full.max),
            ] {
                chrome::corner(b, c, &s);
            }
        });
    }
    if hover.is_some() || app.routing_view.gesture.is_some() {
        write_hint(app, ui, hover);
    } else if resp.has_focus() {
        hint::set_text(ui.ctx(), chrome::word("matrix.keys"), "", Caps::NONE);
    }
    key_menu(app, ui, &s);
    super::route_menu::show(app, ui, owner());
    app.routing_view.geometry = Some(geo);
}

/// The toolbar: the title with what the matrix holds, the *All · In use* filter, and how
/// many strips the filter hides.
fn toolbar(app: &mut CantusApp, ui: &mut egui::Ui, rev: u64, in_use: bool, s: &Style) {
    let mut bar = Toolbar::new(ui);
    let v = &mut app.routing_view;
    let key = (rev, in_use, crate::i18n::locale());
    if v.summary.0 != Some(key) {
        let g = &v.grid;
        v.summary.1 = tf(
            "matrix.summary",
            &[("strips", &g.strips), ("routes", &g.routes)],
        );
        v.summary.2 = if g.hidden > 0 {
            tf("matrix.hidden", &[("n", &g.hidden)])
        } else {
            String::new()
        };
        v.summary.0 = Some(key);
    }
    bar.title(ui, Src::Key("matrix.title"), Some(&v.summary.1));
    if let Some(r) = bar.take_left(140.0, 22.0) {
        let labels = [
            Src::Key("matrix.filter.all"),
            Src::Key("matrix.filter.in_use"),
        ];
        let resp = ui.interact(r, Id::new("routing-filter"), Sense::click());
        let hovered = resp
            .hover_pos()
            .and_then(|p| segmented::hit(r, labels.len(), p.x));
        if let (true, Some(i)) = (resp.clicked(), hovered) {
            v.set_in_use(i == 1);
        }
        let st = State::of(&resp, false);
        if st.hovered {
            hint::set_text(ui.ctx(), chrome::word("matrix.filter.hint"), "", Caps::NONE);
        }
        let sel = usize::from(v.in_use(v.grid.strips));
        let fs = s.function(kit::Function::Output);
        kit::with_brush(ui.painter(), |b| {
            segmented::paint(b, r, &fs, st, &labels, sel, hovered)
        });
    }
    if !v.summary.2.is_empty() {
        bar.readout_right(ui, &v.summary.2, 72.0, false);
    }
}

/// The wheel (Shift+wheel across), the middle button and the two bars.
fn navigate(app: &mut CantusApp, ui: &mut egui::Ui, geo: &mut Geometry, full: Rect, bar: f32) {
    let v = &mut app.routing_view;
    let area = Rect::from_min_max(full.min, geo.body.max);
    if ui.rect_contains_pointer(area) {
        let w = chrome::wheel(ui);
        if w.scroll != Vec2::ZERO {
            v.scroll -= w.scroll;
        }
    }
    let middle = ui.input(|i| {
        let mine = i.pointer.press_origin().is_some_and(|o| area.contains(o));
        (i.pointer.middle_down() && mine).then(|| i.pointer.delta())
    });
    if let Some(d) = middle {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        v.scroll -= d;
    }
    let content = geo.content();
    let most = vec2(
        (content.x - geo.body.width()).max(0.0),
        (content.y - geo.body.height()).max(0.0),
    );
    v.scroll = v.scroll.clamp(Vec2::ZERO, most);
    let hbar = Rect::from_min_max(
        pos2(geo.body.left(), full.bottom() - bar),
        pos2(geo.body.right(), full.bottom()),
    );
    let vbar = Rect::from_min_max(
        pos2(full.right() - bar, geo.body.top()),
        pos2(full.right(), geo.body.bottom()),
    );
    let span = Span::new(v.scroll.x, geo.body.width(), content.x);
    if let Some(n) = chrome::bar(
        ui,
        hbar,
        Id::new("routing-hbar"),
        span,
        None,
        "views.nav.hbar",
    ) {
        v.scroll.x = n.start;
    }
    let span = Span::new(v.scroll.y, geo.body.height(), content.y);
    if let Some(n) = chrome::bar(
        ui,
        vbar,
        Id::new("routing-vbar"),
        span,
        None,
        "views.nav.vbar",
    ) {
        v.scroll.y = n.start;
    }
    v.scroll = v.scroll.clamp(Vec2::ZERO, most);
    geo.scroll = v.scroll;
}

/// A click on a row's or a column's header selects that strip (the desk and the inspector
/// follow), or that track.
fn heads(app: &mut CantusApp, ui: &mut egui::Ui, geo: &Geometry) {
    let rr = ui.interact(geo.row_head, Id::new("routing-row-heads"), Sense::click());
    if rr.clicked() {
        let row = rr
            .interact_pointer_pos()
            .and_then(|p| geo.row_at(p.y))
            .map(|r| app.routing_view.grid.rows[r]);
        match row {
            Some(Row::Strip(id)) => app.select_strip(id),
            Some(Row::Track(id)) => app.select_track(id),
            None => {}
        }
    }
    if rr.hovered() {
        hint::set_text(ui.ctx(), chrome::word("matrix.head.hint"), "", Caps::NONE);
    }
    let cr = ui.interact(geo.col_head, Id::new("routing-col-heads"), Sense::click());
    if cr.clicked() {
        let col = cr
            .interact_pointer_pos()
            .and_then(|p| geo.col_at(p.x))
            .map(|c| app.routing_view.grid.cols[c]);
        if let Some(Col::Strip(id)) = col {
            app.select_strip(id);
        }
    }
    if cr.hovered() {
        hint::set_text(ui.ctx(), chrome::word("matrix.head.hint"), "", Caps::NONE);
    }
}

/// The pointer on the body: a press focuses the cell; a click acts with the grammar; a
/// drag on a send moves its level (Shift: fine, as on every knob of the house); a
/// Ctrl+drag copies it along the column (integration: Shift was both); a double-click is
/// 0 dB; a right-click opens the route menu.
fn body_input(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    resp: &egui::Response,
    geo: &Geometry,
    hover: Option<(usize, usize)>,
) {
    let mods = ui.input(|i| i.modifiers);
    let origin = ui
        .input(|i| i.pointer.press_origin())
        .and_then(|p| geo.hit(p));
    // The pointer's clicks only: Space and Enter on the focused body are the keymap's
    // and the matrix's keys, not a click on whatever cell the pointer happens to be over.
    let clicked = resp.clicked_by(egui::PointerButton::Primary);
    if clicked || resp.drag_started_by(egui::PointerButton::Primary) {
        resp.request_focus();
        if let Some((r, c)) = origin.or(hover) {
            let g = &app.routing_view.grid;
            app.routing_view.focus = Some((g.rows[r], g.cols[c]));
        }
    }
    if resp.drag_started_by(egui::PointerButton::Primary) {
        app.routing_view.gesture = origin.and_then(|(r, c)| {
            let (from, to) = send_at(app, r, c)?;
            let send = *app.project().strip(from)?.send_to(to)?;
            Some(if mods.command {
                Gesture::Copy {
                    col: c,
                    start: r,
                    end: r,
                }
            } else {
                Gesture::Level {
                    from,
                    send,
                    travel: 0.0,
                }
            })
        });
    }
    if resp.dragged() {
        let fine = if mods.shift { 0.1 } else { 1.0 };
        let dy = resp.drag_delta().y;
        let pointer = ui.input(|i| i.pointer.hover_pos());
        let strip_rows = app.routing_view.grid.strip_rows;
        match &mut app.routing_view.gesture {
            Some(Gesture::Level { from, send, travel }) => {
                *travel -= dy * fine;
                let level = (send.level + *travel / TRAVEL).clamp(0.0, 1.0);
                let (from, send) = (*from, *send);
                let now = app
                    .project()
                    .strip(from)
                    .and_then(|s| s.send_to(send.to))
                    .map(|x| x.level);
                if now.is_some_and(|l| l != level) {
                    app.set_strip_send(from, Send { level, ..send });
                }
            }
            Some(Gesture::Copy { end, .. }) => {
                if let Some(p) = pointer {
                    let below = p.y > geo.row_top(strip_rows.saturating_sub(1));
                    let r = geo.row_at(p.y).filter(|r| *r < strip_rows);
                    *end = match r {
                        Some(r) => r,
                        None if below => strip_rows.saturating_sub(1),
                        None => 0,
                    };
                }
            }
            None => {}
        }
    }
    if resp.drag_stopped() {
        match app.routing_view.gesture.take() {
            Some(Gesture::Level { .. }) => app.end_gesture(),
            Some(Gesture::Copy { col, start, end }) => {
                copy_send(app, col, start, end);
            }
            None => {}
        }
    }
    let at = ui
        .input(|i| i.pointer.interact_pos())
        .unwrap_or(geo.body.center());
    if resp.double_clicked() {
        if let Some((from, to)) = hover.and_then(|(r, c)| send_at(app, r, c)) {
            let send = app
                .project()
                .strip(from)
                .and_then(|s| s.send_to(to))
                .copied();
            if let Some(send) = send.filter(|s| s.level != 1.0) {
                app.set_strip_send(from, Send { level: 1.0, ..send });
                app.end_gesture();
            }
        }
    } else if clicked {
        if let Some((r, c)) = hover {
            act(app, r, c, Grammar::from_modifiers(mods), at, false);
        }
    }
    if resp.secondary_clicked() {
        if let Some((r, c)) = hover {
            open_menu(app, ui, r, c, at);
        }
    }
}

/// The send a strip cell holds, as (from, to).
fn send_at(app: &CantusApp, r: usize, c: usize) -> Option<(StripId, StripId)> {
    let g = &app.routing_view.grid;
    match (g.rows[r], g.cols[c]) {
        (Row::Strip(from), Col::Strip(to)) if g.cell(r, c).send.is_some() => Some((from, to)),
        _ => None,
    }
}

/// What a click (or Enter, `toggle`) on cell `(r, c)` does with `grammar`.
fn act(app: &mut CantusApp, r: usize, c: usize, grammar: Grammar, at: Pos2, toggle: bool) {
    let g = &app.routing_view.grid;
    let (row, col, cell) = (g.rows[r], g.cols[c], *g.cell(r, c));
    let from = match row {
        Row::Track(track) => {
            if !cell.output {
                let to = match col {
                    Col::Master => None,
                    Col::Strip(s) => Some(s),
                };
                app.route_tracks_here(&[track], to);
            }
            return;
        }
        Row::Strip(from) => from,
    };
    if cell.itself {
        return;
    }
    let to = col.id();
    match (col, grammar) {
        (_, Grammar::Key) => start_key(app, from, to, at),
        (Col::Master, _) => {
            if !cell.output {
                app.replace_output(&[from], to);
            }
        }
        (Col::Strip(_), Grammar::Output) => {
            if cell.send.is_some() {
                app.send_to_output(from, to);
            } else if !cell.output {
                match app.project().route_check(from, to, RouteKind::Output) {
                    Ok(()) => {
                        app.replace_output(&[from], to);
                    }
                    Err(e) => refuse(app, from, e),
                }
            }
        }
        (Col::Strip(_), Grammar::Send) => {
            if cell.send.is_some() {
                if toggle {
                    app.remove_route(Route::send(from, to));
                }
            } else if !cell.output || toggle {
                match app.project().route_check(from, to, RouteKind::Send) {
                    Ok(()) => {
                        app.add_sends(&[from], to);
                    }
                    Err(e) => refuse(app, from, e),
                }
            }
        }
    }
}

/// Say why a route was not made, in the status bar (the hint bar said it already).
fn refuse(app: &mut CantusApp, from: StripId, e: cantus_core::routing::RouteRefusal) {
    let text = refusal_text(app.project(), from, e);
    app.set_status(text);
}

/// Alt: key a sidechain on `host` from `from` — one keyable insert at once, several in a
/// menu, none: the ducking compressor or gate offers.
fn start_key(app: &mut CantusApp, from: StripId, host: StripId, at: Pos2) {
    if let Err(e) = app.project().route_check(from, host, RouteKind::Key(None)) {
        refuse(app, from, e);
        return;
    }
    let choices = cantus_core::routing::key_choices(app.project(), from, host);
    if let [only] = choices[..] {
        app.key_sidechain(from, host, only);
        return;
    }
    app.routing_view.key_menu = Some(KeyMenu {
        from,
        host,
        at,
        choices,
        opened: 0,
    });
}

/// The route menu of a cell: its send, its output or its key — or, on an empty strip
/// cell, the add-send list.
fn open_menu(app: &mut CantusApp, ui: &egui::Ui, r: usize, c: usize, at: Pos2) {
    let g = &app.routing_view.grid;
    let (Row::Strip(from), col, cell) = (g.rows[r], g.cols[c], *g.cell(r, c)) else {
        return;
    };
    let to = col.id();
    let route = if cell.send.is_some() {
        Some(Route::send(from, to))
    } else if cell.output {
        Some(Route::output(from, to))
    } else if cell.keys > 0 {
        first_key(app, from, to).map(|e| Route::key(from, to, e))
    } else {
        None
    };
    match route {
        Some(route) => super::route_menu::open(ui.ctx(), owner(), route, at),
        None if !cell.itself && col != Col::Master => {
            super::route_menu::open_add_send(ui.ctx(), owner(), from, at)
        }
        None => {}
    }
}

/// The first insert of `host` keyed from `from`.
fn first_key(app: &CantusApp, from: StripId, host: StripId) -> Option<EffectId> {
    let p = app.project();
    let chain = if host.is_master() {
        &p.master.effects
    } else {
        &p.strip(host)?.effects
    };
    chain
        .iter()
        .find(|e| e.sidechain == Some(from))
        .map(|e| e.id)
}

/// Shift+drag along a column: the send of row `start` copied to every other strip row
/// from `start` to `end` that can take it (or already sends there: its level, switch and
/// tap become the copy's). One undo step, `cmd.copy_send`.
fn copy_send(app: &mut CantusApp, col: usize, start: usize, end: usize) -> bool {
    let g = &app.routing_view.grid;
    let (Row::Strip(src), Col::Strip(to)) = (g.rows[start], g.cols[col]) else {
        return false;
    };
    let rows: Vec<StripId> = (start.min(end)..=start.max(end))
        .filter(|r| *r != start && *r < g.strip_rows)
        .filter_map(|r| match g.rows[r] {
            Row::Strip(s) => Some(s),
            Row::Track(_) => None,
        })
        .collect();
    match copy_batch(app.project(), src, to, &rows) {
        Some(cmd) => app.exec(cmd),
        None => false,
    }
}

/// The batch that copies `src`'s send to `to` onto every one of `rows` that can take it.
pub fn copy_batch(
    p: &cantus_core::Project,
    src: StripId,
    to: StripId,
    rows: &[StripId],
) -> Option<Command> {
    let send = *p.strip(src)?.send_to(to)?;
    let commands: Vec<Command> = rows
        .iter()
        .filter(|id| {
            matches!(
                p.route_check(**id, to, RouteKind::Send),
                Ok(()) | Err(cantus_core::routing::RouteRefusal::AlreadyThere)
            )
        })
        .map(|id| Command::SetStripSend { id: *id, send })
        .collect();
    (!commands.is_empty()).then(|| Command::Batch {
        label_key: "cmd.copy_send".to_string(),
        commands,
    })
}

/// Remove the route a cell holds: its send, else its output (back to the master), else
/// its first key.
fn remove(app: &mut CantusApp, r: usize, c: usize) {
    let g = &app.routing_view.grid;
    let (Row::Strip(from), col, cell) = (g.rows[r], g.cols[c], *g.cell(r, c)) else {
        return;
    };
    let to = col.id();
    if cell.send.is_some() {
        app.remove_route(Route::send(from, to));
    } else if cell.output && col != Col::Master {
        app.remove_route(Route::output(from, to));
    } else if cell.keys > 0 {
        if let Some(e) = first_key(app, from, to) {
            app.remove_route(Route::key(from, to, e));
        }
    }
}

/// A send's level by `db` (from silence, −60 dB is the first step up). One undo step.
fn step_level(app: &mut CantusApp, from: StripId, to: StripId, db: f32) {
    let Some(send) = app
        .project()
        .strip(from)
        .and_then(|s| s.send_to(to))
        .copied()
    else {
        return;
    };
    let now = if send.level > 0.0 {
        linear_to_db(send.level)
    } else {
        -60.0 - db.abs()
    };
    let level = db_to_linear(now + db).clamp(0.0, 1.0);
    if level != send.level {
        app.set_strip_send(from, Send { level, ..send });
        app.end_gesture();
    }
}

/// The keys the matrix takes while it has the focus, with the modifiers it reads them
/// with (anything else held leaves the key to the keymap).
fn mine(key: Key, m: Modifiers) -> bool {
    let bare = !m.command && !m.alt && !m.shift;
    match key {
        Key::ArrowUp | Key::ArrowDown | Key::ArrowLeft | Key::ArrowRight => bare,
        Key::Home | Key::End | Key::Delete | Key::Backspace | Key::Escape => bare,
        Key::Enter => !m.shift && !(m.command && m.alt),
        Key::Plus | Key::Equals | Key::Minus => !m.command && !m.alt,
        _ => false,
    }
}

/// The keyboard, while the body holds the focus (design §5's keys). Every other
/// shortcut is passed on to the keymap — the transport, undo, the views — since a
/// focused widget keeps the application's own dispatch away.
fn keys(app: &mut CantusApp, ui: &mut egui::Ui, resp: &egui::Response, geo: &mut Geometry) {
    if !resp.has_focus() {
        return;
    }
    let id = body_id();
    ui.memory_mut(|m| {
        m.set_focus_lock_filter(
            id,
            egui::EventFilter {
                horizontal_arrows: true,
                vertical_arrows: true,
                // egui drops the focus on Esc before the body sees it: keep it, so the
                // matrix's own Esc (give the focus back) is the one that runs.
                escape: true,
                ..Default::default()
            },
        )
    });
    crate::widgets::claim_arrow_keys(ui.ctx(), id);
    let (m, pressed) = ui.input(|i| {
        let keys: Vec<Key> = i
            .events
            .iter()
            .filter_map(|e| match e {
                egui::Event::Key {
                    key, pressed: true, ..
                } => Some(*key),
                _ => None,
            })
            .collect();
        (i.modifiers, keys)
    });
    let taken: Vec<Key> = pressed.iter().copied().filter(|k| mine(*k, m)).collect();
    // Everything else goes to the keymap, as if nothing held the focus — with the typing
    // piano, the paste and the bare M / S rule (one helper for both paths).
    app.dispatch_keys(ui.ctx(), true, &|km, a| {
        km.bindings_for(a).iter().any(|b| {
            taken.contains(&b.key) && b.command == m.command && b.alt == m.alt && b.shift == m.shift
        })
    });
    if taken.is_empty() {
        return;
    }
    let g = &app.routing_view.grid;
    let (rows, cols) = (g.rows.len(), g.cols.len());
    if rows == 0 || cols == 0 {
        return;
    }
    let (mut r, mut c) = app
        .routing_view
        .focus
        .and_then(|(r, c)| Some((g.row_of(r)?, g.col_of(c)?)))
        .unwrap_or((0, 0));
    let at = geo.cell_rect(r, c).center();
    for key in taken {
        match key {
            Key::ArrowUp => r = r.saturating_sub(1),
            Key::ArrowDown => r = (r + 1).min(rows - 1),
            Key::ArrowLeft => c = c.saturating_sub(1),
            Key::ArrowRight => c = (c + 1).min(cols - 1),
            Key::Home => c = 0,
            Key::End => c = cols - 1,
            Key::Escape => ui.memory_mut(|mem| mem.surrender_focus(id)),
            Key::Delete | Key::Backspace => remove(app, r, c),
            Key::Enter => act(app, r, c, Grammar::from_modifiers(m), at, true),
            Key::Plus | Key::Equals | Key::Minus => {
                let step = if m.shift { 0.1 } else { 1.0 };
                let db = if key == Key::Minus { -step } else { step };
                if let Some((from, to)) = send_at(app, r, c) {
                    step_level(app, from, to, db);
                }
            }
            _ => {}
        }
    }
    let g = &app.routing_view.grid;
    if r < g.rows.len() && c < g.cols.len() {
        app.routing_view.focus = Some((g.rows[r], g.cols[c]));
        let to = geo.reveal(r, c, app.routing_view.scroll);
        app.routing_view.scroll = to;
        geo.scroll = to;
    }
}

/// The hint bar for the cell under the pointer (or the drag going on): which route it
/// is, what a click does with the keys held, or why it cannot be made.
fn write_hint(app: &mut CantusApp, ui: &egui::Ui, hover: Option<(usize, usize)>) {
    let m = ui.input(|i| i.modifiers);
    let rev = app.bus.revision();
    match app.routing_view.gesture {
        Some(Gesture::Copy { start, end, .. }) => {
            let n = start.max(end) - start.min(end);
            let text = tf("matrix.copy", &[("n", &n)]);
            hint::set_text(ui.ctx(), &text, "", Caps::NONE);
            return;
        }
        Some(Gesture::Level { from, send, .. }) => {
            let level = app
                .project()
                .strip(from)
                .and_then(|s| s.send_to(send.to))
                .map_or(send.level, |x| x.level);
            let p = app.project();
            let name = tf(
                "matrix.cell.pair",
                &[
                    ("from", &crate::routing_actions::strip_name(p, from)),
                    ("to", &crate::routing_actions::strip_name(p, send.to)),
                ],
            );
            hint::set_text(ui.ctx(), &name, &level_text(level), Caps::NONE);
            return;
        }
        None => {}
    }
    let Some((r, c)) = hover else {
        return;
    };
    let bits = u8::from(m.command) | (u8::from(m.alt) << 1) | (u8::from(m.shift) << 2);
    let key = (rev ^ ((crate::i18n::locale() as u64) << 60), r, c, bits);
    if app.routing_view.hint.0 != Some(key) {
        let (name, value) = describe(app, r, c, m);
        app.routing_view.hint = (Some(key), name, value);
    }
    let v = &app.routing_view;
    let caps = Caps {
        menu: true,
        warning: v.grid.cell(r, c).refused.is_some()
            || v.grid.cell(r, c).send.is_some_and(|s| s.doubled),
        ..Caps::NONE
    };
    hint::set_text(ui.ctx(), &v.hint.1, &v.hint.2, caps);
}

/// What the hint says about cell `(r, c)` with the modifiers `m` held: the route and
/// what a click does (or why not) in the name, a send's level as the value (the hint bar
/// sets values in figures).
pub fn describe(app: &CantusApp, r: usize, c: usize, m: Modifiers) -> (String, String) {
    let (pair, prose) = prose(app, r, c, m);
    let name = if prose.is_empty() {
        pair
    } else {
        format!("{pair} · {prose}")
    };
    let value = app
        .routing_view
        .grid
        .cell(r, c)
        .send
        .map_or(String::new(), |s| level_text(s.level));
    (name, value)
}

/// The route a cell stands for, and what it holds and does, in words.
fn prose(app: &CantusApp, r: usize, c: usize, m: Modifiers) -> (String, String) {
    let p = app.project();
    let g = &app.routing_view.grid;
    let (row, col, cell) = (g.rows[r], g.cols[c], *g.cell(r, c));
    let name = tf(
        "matrix.cell.pair",
        &[("from", &g.row_heads[r].name), ("to", &g.col_heads[c].name)],
    );
    let mut value = String::new();
    let mut add = |s: String| {
        if !s.is_empty() {
            if !value.is_empty() {
                value.push_str(" · ");
            }
            value.push_str(&s);
        }
    };
    let from = match row {
        Row::Track(_) => {
            add(t(if cell.output {
                "matrix.cell.track_on"
            } else {
                "matrix.cell.track_off"
            }));
            if cell.notes > 0 {
                add(tf("matrix.cell.notes", &[("n", &cell.notes)]));
            }
            return (name, value);
        }
        Row::Strip(from) => from,
    };
    let to = col.id();
    if cell.itself {
        add(t("routing.refusal.itself"));
        return (name, value);
    }
    if let Some(s) = cell.send {
        let mut what = tf("matrix.cell.send", &[("tap", &t(s.tap.label_key()))]);
        if !s.enabled {
            what.push(' ');
            what.push_str(&t("routing.feeders.off"));
        }
        add(what);
        if let Some(d) = crate::routing_actions::doubled_text(p, from, to) {
            add(d);
        }
        add(t("matrix.cell.send_do"));
    } else if cell.output {
        add(t(if col == Col::Master {
            "matrix.cell.master_output"
        } else {
            "matrix.cell.output"
        }));
    }
    if cell.keys > 0 {
        add(tf("matrix.cell.keys", &[("n", &cell.keys)]));
    }
    if cell.send.is_none() && !cell.output {
        let grammar = Grammar::from_modifiers(m);
        let kind = match (col, grammar) {
            (Col::Master, Grammar::Key) => RouteKind::Key(None),
            (Col::Master, _) => RouteKind::Output,
            (_, g) => g.kind(),
        };
        match p.route_check(from, to, kind) {
            Err(e) => add(refusal_text(p, from, e)),
            Ok(()) if col == Col::Master => add(t("matrix.cell.master_empty")),
            Ok(()) => add(t(match grammar {
                Grammar::Send => "matrix.cell.empty",
                Grammar::Output => "matrix.cell.empty_output",
                Grammar::Key => "matrix.cell.empty_key",
            })),
        }
    }
    (name, value)
}

/// The menu Alt+click opens when the key needs a choice: the keyable inserts, or the
/// ducking compressor and gate offers.
fn key_menu(app: &mut CantusApp, ui: &egui::Ui, s: &Style) {
    let Some(km) = app.routing_view.key_menu.clone() else {
        return;
    };
    let frame = ui.ctx().cumulative_frame_nr();
    if km.opened == 0 {
        if let Some(k) = app.routing_view.key_menu.as_mut() {
            k.opened = frame;
        }
    }
    let p = app.project();
    let from_name = crate::routing_actions::strip_name(p, km.from);
    let mut labels: Vec<String> = Vec::new();
    if km.choices.is_empty() {
        labels.push(tf("matrix.key_menu.duck", &[("from", &from_name)]));
        labels.push(tf("matrix.key_menu.gate", &[("from", &from_name)]));
    } else {
        let chain = if km.host.is_master() {
            Some(&p.master.effects)
        } else {
            p.strip(km.host).map(|h| &h.effects)
        };
        for e in &km.choices {
            let pos = chain.and_then(|c| c.iter().position(|x| x.id == *e));
            let name = chain
                .and_then(|c| c.iter().find(|x| x.id == *e))
                .map(|x| t(x.spec.kind.name_key()))
                .unwrap_or_default();
            labels.push(format!("{} · {name}", pos.map_or(0, |i| i + 1)));
        }
    }
    let band = if km.choices.is_empty() {
        "matrix.key_menu.none"
    } else {
        "matrix.key_menu.title"
    };
    let mut items = vec![Item::Band(Src::Key(band))];
    items.extend(labels.iter().map(|l| Item::Action {
        label: Src::Str(l),
        check: Check::None,
        shortcut: None,
        submenu: false,
        enabled: true,
    }));
    let k = s.d(1.0);
    let width = s.s(240.0);
    let h = menu::height(&items, k);
    let area = egui::Area::new(Id::new("routing-key-menu"))
        .order(egui::Order::Foreground)
        .fixed_pos(km.at + vec2(s.s(4.0), s.s(4.0)))
        .show(ui.ctx(), |ui| {
            let (rect, resp) = ui.allocate_exact_size(vec2(width, h), Sense::click());
            let hovered = resp.hover_pos().and_then(|p| {
                (0..items.len())
                    .find(|&i| items[i].is_action() && menu::row(rect, &items, k, i).contains(p))
            });
            kit::with_brush(ui.painter(), |b| menu::paint(b, rect, s, &items, hovered));
            (resp, hovered)
        });
    let (resp, hovered) = area.inner;
    let escape = ui.input(|i| i.key_pressed(Key::Escape));
    if let (true, Some(i)) = (resp.clicked(), hovered) {
        app.routing_view.key_menu = None;
        let pick = i - 1;
        if km.choices.is_empty() {
            if pick == 0 {
                app.duck_strip_from(km.host, km.from);
            } else {
                app.gate_strip_from(km.host, km.from);
            }
        } else if let Some(e) = km.choices.get(pick) {
            app.key_sidechain(km.from, km.host, *e);
        }
    } else if escape || (resp.clicked_elsewhere() && km.opened != 0 && frame > km.opened) {
        app.routing_view.key_menu = None;
    }
}

/// Whether the key menu is open (for the tests).
pub fn key_menu_open(app: &CantusApp) -> bool {
    app.routing_view.key_menu.is_some()
}

/// The track a track row stands for, for the tests.
pub fn track_row(grid: &Grid, r: usize) -> Option<TrackId> {
    match grid.rows.get(r)? {
        Row::Track(t) => Some(*t),
        Row::Strip(_) => None,
    }
}
