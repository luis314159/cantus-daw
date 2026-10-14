//! **The clip browser** (ADR 0037, round 26): FL Studio's playlist *picker panel* in
//! Cantus — a pane beside the arrangement listing every link group once and every clip of
//! its own, as tiles in the clip's colour: the name and ×N on the tile's title strip, a
//! miniature of what it holds (its notes, its curve, its waveform) under it. A
//! filter narrows the tiles by name or track. **Dragging a tile onto the arrangement
//! places a linked copy** where it is let go — the row under the pointer, the time
//! snapped to the grid as the arrangement snaps — in one undo step
//! (`CantusApp::place_linked_copy`, through `Linker::copy`); a clip of its own and its copy
//! become a group. An automation tile lands under the track it automates. A click on a
//! tile makes it the arrangement's **paint source** (FL's picked pattern; the paint tool is
//! B in the arrangement: click places a copy, drag paints copies along the row, right-drag
//! or Alt erases them; Esc puts it away). A double-click on a
//! tile selects its clips (the group's every copy). Audio tiles show their waveform, from
//! the peaks the arrangement reads. Alt+F5 shows or hides it (F5 is the
//! arrangement, FL's playlist).
//!
//! The list is the *Link to…* picker's (`links::PickList`, built once per project
//! revision, filtered per keystroke), and only the tiles in view are laid out and painted.

use crate::app::CantusApp;
use crate::clips::ClipRef;

use crate::links::{PickList, PickRow};
use cantus_core::{links, Clip, Ticks, TrackId};
use egui::{pos2, Pos2, Rect, Ui};

/// The pane's width when it opens, in points.
pub const WIDTH: f32 = 200.0;
/// One tile, and the gap under it.
const TILE: f32 = 46.0;
const TILE_GAP: f32 = 4.0;
/// A band over the groups and over the clips of their own.
const BAND: f32 = 20.0;
const PAD: f32 = 6.0;
const FIELD: f32 = 24.0;

/// An audio tile's waveform as kept: the revision and column count it was made for, and
/// its min/max columns.
type Wave = (u64, usize, Vec<(f32, f32)>);

/// The browser's state: its filter, its list, a tile being dragged, the tile chosen as the
/// arrangement's paint source, a paint stroke in progress, and the audio tiles' waveforms.
#[derive(Default)]
pub struct ClipBrowser {
    /// The text the tiles are narrowed by.
    pub filter: String,
    list: PickList,
    built: Option<(u64, u8)>,
    shown: Vec<usize>,
    shown_for: Option<String>,
    /// Where each tile in view was drawn last frame.
    rects: Vec<(ClipRef, Rect)>,
    /// The entry being dragged (by the member a copy is made of).
    drag: Option<ClipRef>,
    /// The paint source (FL's selected pattern): the clip the paint tool copies.
    brush: Option<ClipRef>,
    /// The arrangement's paint tool is out (B).
    painting: bool,
    /// A paint or erase stroke in progress on the lanes.
    stroke: Option<paint_tool::Stroke>,
    /// Each audio tile's waveform columns, made from the peak store once per clip,
    /// revision and width: (revision, columns, the pairs).
    waves: std::collections::HashMap<ClipRef, Wave>,
    /// Scratch for the store's columns before they are kept.
    wave_scratch: Vec<(f32, f32)>,
    /// Where the paint tool's ghosts were drawn last frame, and the browser drag's.
    paint_ghosts: Vec<Rect>,
    drop_ghost: Option<Rect>,
    /// The scrollbar's lane while the tiles overflow, and where a drag on it took them.
    lane: Option<Rect>,
    scroll_to: Option<f32>,
}

impl ClipBrowser {
    /// Where each tile in view was drawn, by the clip a copy would be made of.
    pub fn tile_rects(&self) -> &[(ClipRef, Rect)] {
        &self.rects
    }

    /// The entry being dragged, if any.
    pub fn dragging(&self) -> Option<ClipRef> {
        self.drag
    }

    /// The paint source: the tile chosen (a click on it; a click again lets it go).
    pub fn brush(&self) -> Option<ClipRef> {
        self.brush
    }

    pub fn set_brush(&mut self, brush: Option<ClipRef>) {
        self.brush = brush;
    }

    /// Where the ghosts of what a paint stroke or a drag would place were drawn.
    pub fn ghosts(&self) -> Vec<Rect> {
        let mut v = self.paint_ghosts.clone();
        v.extend(self.drop_ghost);
        v
    }

    /// The scrollbar's lane, while the tiles overflow the pane.
    pub fn scroll_lane(&self) -> Option<Rect> {
        self.lane
    }

    /// Whether the arrangement's paint tool is out.
    pub fn painting(&self) -> bool {
        self.painting
    }

    pub fn set_painting(&mut self, on: bool) {
        self.painting = on;
        if !on {
            self.stroke = None;
        }
    }

    /// The waveform an audio tile shows, once the peaks have arrived: min/max pairs, one
    /// per column, reversed for a reversed clip.
    pub fn wave_of(&self, r: ClipRef) -> Option<&[(f32, f32)]> {
        self.waves.get(&r).map(|(_, _, w)| w.as_slice())
    }

    fn rebuild(&mut self, app: &CantusApp) {
        let key = (app.bus.revision(), crate::i18n::locale() as u8);
        if self.built != Some(key) {
            self.list = PickList::build(app.project(), &[]);
            self.built = Some(key);
            self.shown_for = None;
        }
        if self.shown_for.as_deref() == Some(self.filter.as_str()) {
            return;
        }
        self.shown = self.list.matching(&self.filter, false);
        self.shown_for = Some(self.filter.clone());
    }

    fn row(&self, i: usize) -> &PickRow {
        &self.list.rows()[self.shown[i]]
    }

    /// The waveform of audio clip `r` at `n` columns: kept until the project changes or the
    /// tile's width does; made from the peaks when they are ready (no decoding here — the
    /// store reads files on its own thread, and asks `ctx` for a frame when they arrive).
    /// `None` while they are not.
    fn wave(
        &mut self,
        app: &CantusApp,
        ctx: &egui::Context,
        r: ClipRef,
        n: usize,
    ) -> Option<&[(f32, f32)]> {
        let rev = app.bus.revision();
        let fresh = self
            .waves
            .get(&r)
            .is_some_and(|(at, cols, _)| *at == rev && *cols == n);
        if !fresh {
            let (clip, region) = crate::audio_clips::region(app.project(), r)?;
            let src = app.project().audio_source(region.source)?;
            let crate::waveform::Lookup::Ready(peaks) =
                app.peaks.lookup(src, app.audio_base(), Some(ctx))
            else {
                return None;
            };
            let (from, to) =
                crate::audio_clips::window_frames(app.project(), clip, src.sample_rate);
            let per = ((to - from) / n.max(1) as f64).max(1e-9);
            peaks.columns(
                crate::waveform::Which::Mono,
                from,
                per,
                n,
                &mut self.wave_scratch,
            );
            if region.reverse {
                self.wave_scratch.reverse();
            }
            let entry = self.waves.entry(r).or_default();
            entry.0 = rev;
            entry.1 = n;
            entry.2.clear();
            entry.2.extend_from_slice(&self.wave_scratch);
        }
        self.waves.get(&r).map(|(_, _, w)| w.as_slice())
    }
}

#[path = "clip_browser_paint_tool.rs"]
mod paint_tool;
pub use paint_tool::paint_tool;

/// Where a tile let go of at `p` lands: the track of the row under the pointer (the
/// entry's own track for automation) and the time under it, snapped as the arrangement
/// snaps; and the rect a copy of `clip` would cover there, for the ghost. `None` off the
/// arrangement's rows.
fn drop_at(app: &CantusApp, p: Pos2, src: ClipRef, clip: &Clip) -> Option<(TrackId, Ticks, Rect)> {
    let g = app.arrangement.geometry()?;
    let inside = p.x >= g.left && p.x <= g.left + g.width && p.y >= g.body_top;
    if !inside {
        return None;
    }
    let under = g.row_at(p.y)?;
    // An automation clip lands under the track it automates: its ghost is drawn there.
    let row = if clip.automation.is_some() {
        app.project().track_index(src.0)?
    } else {
        under
    };
    let track = app.project().tracks.get(row)?.id;
    let r = *g.rows.get(row)?;
    let mut at = crate::views::beats_to_ticks(g.beats(p.x)).max(Ticks::ZERO);
    if app.arrangement.snap {
        at = at.snap_floor(app.arrangement.grid.max(Ticks(1)));
    }
    let x0 = g.x(crate::views::ticks_to_beats(at));
    let x1 = g.x(crate::views::ticks_to_beats(at + clip.length));
    // Only what shows of it: the timeline's width, never over the panes beside it.
    let (lo, hi) = (g.left, g.left + g.width);
    let ghost = Rect::from_min_max(
        pos2(x0.clamp(lo, hi), r.top + 1.0),
        pos2(x1.clamp(lo, hi), r.top + r.height - 1.0),
    );
    Some((track, at, ghost))
}

#[path = "clip_browser_paint.rs"]
mod paint;

/// The pane: the filter, the tiles, and a drag in progress (its chip, the ghost where it
/// would land, the copy placed when it is let go).
pub fn show(app: &mut CantusApp, ui: &mut Ui) {
    let mut st = std::mem::take(&mut app.clip_browser);
    st.rebuild(app);
    let picked = paint::pane(&mut st, app, ui);
    if let Some(entry) = picked.choose {
        // A click chooses the paint source (FL's picked pattern); on the chosen one, lets
        // it go. The tool itself is the arrangement's (B).
        let same = st.brush.is_some_and(|b| same_entry(app, b, entry));
        st.brush = (!same).then_some(entry);
        // The browser is the arrangement's picker panel: choosing in it is an arrangement
        // gesture, so B then acts on the arrangement.
        app.focus_arrangement();
    }
    if let Some(entry) = picked.select {
        st.brush = Some(entry);
        select_entry(app, &st, entry);
        app.focus_arrangement();
    }
    st.rebuild(app);
    st.drop_ghost = None;
    if let Some(src) = st.drag {
        let ctx = ui.ctx().clone();
        let (down, pos) = ctx.input(|i| (i.pointer.primary_down(), i.pointer.latest_pos()));
        let clip = crate::clips::clip_of(app.project(), src).cloned();
        let landing = clip
            .as_ref()
            .zip(pos)
            .and_then(|(c, p)| drop_at(app, p, src, c));
        st.drop_ghost = landing.map(|l| l.2);
        if let (Some(c), Some(p)) = (&clip, pos) {
            paint::drag_feedback(&ctx, app, c, p, landing.map(|l| l.2));
        }
        if !down {
            st.drag = None;
            if let Some((track, at, _)) = landing {
                app.place_linked_copy(src, track, at);
            }
        }
        ctx.request_repaint();
    }
    app.clip_browser = st;
}

/// A click on a tile: the entry's clips — the group's every copy — become the selection.
fn select_entry(app: &mut CantusApp, st: &ClipBrowser, entry: ClipRef) {
    let Some(row) = st.list.rows().iter().find(|r| r.group.first == entry) else {
        return;
    };
    let mut sel = vec![entry];
    if row.group.link.is_some() {
        sel.extend(links::siblings(app.project(), entry));
    }
    app.select_clip(entry.0, entry.1);
    app.set_clip_selection(sel);
}

/// The clip a tile shows, and its colour (its own, else its track's).
fn clip_and_colour(app: &CantusApp, r: ClipRef) -> Option<(&Clip, egui::Color32)> {
    let track = app.project().track(r.0)?;
    let clip = track.clip(r.1)?;
    let c = clip.color.unwrap_or(track.color);
    Some((clip, crate::theme::track_color(c)))
}

/// Whether two clips are one entry of the list: the same clip, or copies of one group.
pub(crate) fn same_entry(app: &CantusApp, a: ClipRef, b: ClipRef) -> bool {
    if a == b {
        return true;
    }
    let link = |r: ClipRef| crate::clips::clip_of(app.project(), r).and_then(|c| c.link);
    link(a).is_some() && link(a) == link(b)
}
