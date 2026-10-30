//! Where everything of the Session pane sits (ADR 0077), pure arithmetic so the painters,
//! the input and the tests agree: the pane's header bar; the track heads over the columns;
//! the grid of slots (scrolling both ways); the stop row under it; the master column at
//! the right with the scene launchers and *stop all* (fixed while the columns scroll, as
//! Live's); the clip panel at the bottom when a clip is chosen.

use cantus_core::session::{SceneId, SlotRef};
use cantus_core::TrackId;
use egui::{pos2, vec2, Pos2, Rect, Vec2};

/// Sizes at 100 %, in points.
pub const HEADER_H: f32 = 30.0;
pub const HEAD_H: f32 = 44.0;
pub const ROW_H: f32 = 34.0;
pub const COL_W: f32 = 112.0;
pub const MIN_COL_W: f32 = 72.0;
pub const MAX_COL_W: f32 = 220.0;
pub const MASTER_W: f32 = 132.0;
pub const STOP_H: f32 = 26.0;
pub const PANEL_H: f32 = 252.0;
pub const MIN_PANEL_H: f32 = 210.0;
/// The gap between two cells.
pub const GAP: f32 = 2.0;
/// The strip at the grid's right and bottom edges for the scrollbars.
pub const BAR: f32 = 6.0;

/// What a point of the pane is over.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Hit {
    Slot(SlotRef),
    /// A track's head (its name, its switches).
    Head(TrackId),
    /// A track's stop button, under its column.
    Stop(TrackId),
    /// A scene's launcher in the master column.
    Scene(SceneId),
    StopAll,
    /// The empty row under the last scene: a double-click adds one.
    NewScene,
    /// The column divider at a track's right: a drag sets every column's width.
    Divider,
    /// The clip panel's top edge: a drag sets its height.
    PanelEdge,
}

/// The pane's geometry this frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    pub k: f32,
    pub rect: Rect,
    pub header: Rect,
    /// The track heads' band (scrolls sideways with the grid).
    pub heads: Rect,
    /// The slots' viewport (scrolls both ways).
    pub grid: Rect,
    /// The stop buttons' band (scrolls sideways).
    pub stops: Rect,
    /// The master column: its head, the scene launchers (scroll up and down with the grid),
    /// stop all.
    pub master: Rect,
    /// The clip panel, when one shows.
    pub panel: Option<Rect>,
    pub col_w: f32,
    pub row_h: f32,
    pub scroll: Vec2,
    pub tracks: Vec<TrackId>,
    pub scenes: Vec<SceneId>,
}

impl Geometry {
    /// Lay the pane out in `rect` at scale `k` for `tracks` × `scenes`, scrolled by `scroll`
    /// (clamped here to what there is), with a clip panel `panel_h` tall when it shows.
    pub fn new(
        rect: Rect,
        k: f32,
        tracks: Vec<TrackId>,
        scenes: Vec<SceneId>,
        col_w: f32,
        scroll: Vec2,
        panel_h: Option<f32>,
    ) -> Geometry {
        let header = Rect::from_min_size(rect.min, vec2(rect.width(), HEADER_H * k));
        // The grid keeps its heads, the stop row and three rows of slots: the panel takes
        // what is left up to its height, and steps aside when that is too little for it.
        let room = rect.height() - header.height() - (HEAD_H + STOP_H + 3.0 * ROW_H) * k;
        let panel = panel_h
            .map(|h| h.max(MIN_PANEL_H * k).min(room))
            .filter(|h| *h >= MIN_PANEL_H * k)
            .map(|h| Rect::from_min_max(pos2(rect.left(), rect.bottom() - h), rect.max));
        let body_bottom = panel.map_or(rect.bottom(), |p| p.top());
        let master_w = MASTER_W * k;
        let left = rect.left();
        let right = (rect.right() - master_w).max(left + 40.0 * k);
        let heads = Rect::from_min_max(
            pos2(left, header.bottom()),
            pos2(right, header.bottom() + HEAD_H * k),
        );
        let stops = Rect::from_min_max(
            pos2(left, (body_bottom - STOP_H * k).max(heads.bottom())),
            pos2(right, body_bottom),
        );
        let grid = Rect::from_min_max(pos2(left, heads.bottom()), pos2(right, stops.top()));
        let master = Rect::from_min_max(
            pos2(right, header.bottom()),
            pos2(rect.right(), body_bottom),
        );
        let mut g = Geometry {
            k,
            rect,
            header,
            heads,
            grid,
            stops,
            master,
            panel,
            col_w: col_w.clamp(MIN_COL_W, MAX_COL_W) * k,
            row_h: ROW_H * k,
            scroll: Vec2::ZERO,
            tracks,
            scenes,
        };
        g.scroll = g.clamp_scroll(scroll);
        g
    }

    /// The whole grid's size (every column, every row and the new-scene row).
    pub fn content(&self) -> Vec2 {
        vec2(
            self.tracks.len() as f32 * self.col_w,
            (self.scenes.len() + 1) as f32 * self.row_h,
        )
    }

    /// How far the grid can scroll.
    pub fn max_scroll(&self) -> Vec2 {
        let c = self.content();
        vec2(
            (c.x - self.grid.width() + BAR * self.k).max(0.0),
            (c.y - self.grid.height() + BAR * self.k).max(0.0),
        )
    }

    pub fn clamp_scroll(&self, s: Vec2) -> Vec2 {
        let m = self.max_scroll();
        vec2(s.x.clamp(0.0, m.x), s.y.clamp(0.0, m.y))
    }

    /// Column `i`'s left edge on screen.
    pub fn col_x(&self, i: usize) -> f32 {
        self.grid.left() + i as f32 * self.col_w - self.scroll.x
    }

    /// Row `j`'s top edge on screen.
    pub fn row_y(&self, j: usize) -> f32 {
        self.grid.top() + j as f32 * self.row_h - self.scroll.y
    }

    /// Slot (`i`, `j`)'s cell, inset by the gap.
    pub fn cell(&self, i: usize, j: usize) -> Rect {
        let g = GAP * self.k;
        Rect::from_min_size(
            pos2(self.col_x(i) + g * 0.5, self.row_y(j) + g * 0.5),
            vec2(self.col_w - g, self.row_h - g),
        )
    }

    /// Track `i`'s head.
    pub fn head(&self, i: usize) -> Rect {
        Rect::from_min_max(
            pos2(self.col_x(i) + GAP * self.k * 0.5, self.heads.top()),
            pos2(
                self.col_x(i) + self.col_w - GAP * self.k * 0.5,
                self.heads.bottom() - GAP * self.k,
            ),
        )
    }

    /// Track `i`'s stop button.
    pub fn stop(&self, i: usize) -> Rect {
        let g = GAP * self.k;
        Rect::from_min_max(
            pos2(self.col_x(i) + g * 0.5, self.stops.top() + g),
            pos2(
                self.col_x(i) + self.col_w - g * 0.5,
                self.stops.bottom() - g,
            ),
        )
    }

    /// Scene `j`'s launcher in the master column.
    pub fn scene(&self, j: usize) -> Rect {
        let g = GAP * self.k;
        Rect::from_min_max(
            pos2(self.master.left() + 4.0 * self.k, self.row_y(j) + g * 0.5),
            pos2(
                self.master.right() - 4.0 * self.k,
                self.row_y(j) + self.row_h - g * 0.5,
            ),
        )
    }

    /// The master column's head (over the scene launchers).
    pub fn master_head(&self) -> Rect {
        Rect::from_min_max(
            pos2(self.master.left() + 4.0 * self.k, self.heads.top()),
            pos2(
                self.master.right() - 4.0 * self.k,
                self.heads.bottom() - GAP * self.k,
            ),
        )
    }

    /// *Stop all* under the scene launchers.
    pub fn stop_all(&self) -> Rect {
        Rect::from_min_max(
            pos2(
                self.master.left() + 4.0 * self.k,
                self.stops.top() + GAP * self.k,
            ),
            pos2(
                self.master.right() - 4.0 * self.k,
                self.stops.bottom() - GAP * self.k,
            ),
        )
    }

    /// Whether the panel was asked for but the pane is too short for it.
    pub fn panel_squeezed(&self, asked: bool) -> bool {
        asked && self.panel.is_none()
    }

    /// The scene launchers' viewport (the grid's rows, in the master column).
    pub fn scene_view(&self) -> Rect {
        Rect::from_min_max(
            pos2(self.master.left(), self.grid.top()),
            pos2(self.master.right(), self.grid.bottom()),
        )
    }

    /// The columns and rows in view (half-open ranges).
    pub fn visible(&self) -> (std::ops::Range<usize>, std::ops::Range<usize>) {
        let c0 = (self.scroll.x / self.col_w).floor().max(0.0) as usize;
        let c1 = (((self.scroll.x + self.grid.width()) / self.col_w).ceil() as usize)
            .min(self.tracks.len());
        let r0 = (self.scroll.y / self.row_h).floor().max(0.0) as usize;
        let r1 = (((self.scroll.y + self.grid.height()) / self.row_h).ceil() as usize)
            .min(self.scenes.len() + 1);
        (c0.min(c1)..c1, r0.min(r1)..r1)
    }

    /// The column under `x`, among the tracks.
    pub fn col_at(&self, x: f32) -> Option<usize> {
        let f = (x - self.grid.left() + self.scroll.x) / self.col_w;
        (f >= 0.0 && (f as usize) < self.tracks.len()).then_some(f as usize)
    }

    /// The row under `y`: a scene's index, or `scenes.len()` for the new-scene row.
    pub fn row_at(&self, y: f32) -> Option<usize> {
        let f = (y - self.grid.top() + self.scroll.y) / self.row_h;
        (f >= 0.0 && (f as usize) <= self.scenes.len()).then_some(f as usize)
    }

    /// What `p` is over.
    pub fn hit(&self, p: Pos2) -> Option<Hit> {
        if let Some(panel) = self.panel {
            if (p.y - panel.top()).abs() <= 3.0 * self.k && panel.x_range().contains(p.x) {
                return Some(Hit::PanelEdge);
            }
            if panel.contains(p) {
                return None;
            }
        }
        if self.heads.contains(p) {
            let i = self.col_at(p.x)?;
            let edge = self.col_x(i) + self.col_w;
            if (edge - p.x).abs() <= 3.0 * self.k {
                return Some(Hit::Divider);
            }
            return Some(Hit::Head(self.tracks[i]));
        }
        if self.grid.contains(p) {
            let i = self.col_at(p.x)?;
            let j = self.row_at(p.y)?;
            return Some(match self.scenes.get(j) {
                Some(s) => Hit::Slot(SlotRef::new(self.tracks[i], *s)),
                None => Hit::NewScene,
            });
        }
        if self.stops.contains(p) {
            let i = self.col_at(p.x)?;
            return Some(Hit::Stop(self.tracks[i]));
        }
        if self.scene_view().contains(p) {
            let j = self.row_at(p.y)?;
            return Some(match self.scenes.get(j) {
                Some(s) => Hit::Scene(*s),
                None => Hit::NewScene,
            });
        }
        if self.stop_all().contains(p) {
            return Some(Hit::StopAll);
        }
        None
    }

    /// Where slot `s` is, if it is in the grid (on screen or not).
    pub fn index_of(&self, s: SlotRef) -> Option<(usize, usize)> {
        let i = self.tracks.iter().position(|t| *t == s.track)?;
        let j = self.scenes.iter().position(|x| *x == s.scene)?;
        Some((i, j))
    }

    /// The scroll that shows slot (`i`, `j`) whole, from `self.scroll`.
    pub fn scroll_to_show(&self, i: usize, j: usize) -> Vec2 {
        let mut s = self.scroll;
        let (x0, x1) = (i as f32 * self.col_w, (i + 1) as f32 * self.col_w);
        let (y0, y1) = (j as f32 * self.row_h, (j + 1) as f32 * self.row_h);
        let (w, h) = (
            self.grid.width() - BAR * self.k,
            self.grid.height() - BAR * self.k,
        );
        if x0 < s.x {
            s.x = x0;
        } else if x1 > s.x + w {
            s.x = x1 - w;
        }
        if y0 < s.y {
            s.y = y0;
        } else if y1 > s.y + h {
            s.y = y1 - h;
        }
        self.clamp_scroll(s)
    }
}
