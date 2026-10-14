//! **Link to…** (ADR 0037, round 26): link the selected clips to a clip chosen *by name*.
//!
//! A floating panel lists the project's link groups — each once, by its shared name, with
//! how many clips share it (×N), what it holds (notes, audio, automation) and where its
//! first member sits ("Drums · 5.1", which tells two groups of one name apart) — and then
//! the clips of their own, which a first link needs (*Linked only* hides them; the switch
//! is kept while the program runs). A filter narrows the list by name or track as one
//! types (case and accents ignored). A row that would link nothing — another kind of clip,
//! automation of another control, the selection itself — stays in the list, dimmed,
//! saying why (`cantus_core::links::would_link`). Choosing a row links the selection with
//! `links::link_to`, one undo step (`CantusApp::link_clips_to`).
//!
//! Opened by the action *Link to…* — **Alt+N** (the keymap's clip section), the clip
//! menu, the menu bar's Clip menu, the inspector's link row; ↑ ↓ walk the rows that link,
//! Enter chooses, Esc or a click outside closes.
//!
//! Cost (the review of round 26): the list is built once per project revision and
//! selection — `groups` once, the selection summarised once (`links::Selection`), every
//! row answered from the summary and its strings made then — a keystroke only filters it,
//! and only the rows in view are laid out and painted, so a still frame costs the same
//! with 2 000 rows as with 5.

use crate::app::CantusApp;
use crate::clips::ClipRef;
use crate::links::{PickList, PickRow};
use egui::{Context, Pos2, Rect};

/// The panel's width at 100 %.
pub const WIDTH: f32 = 380.0;
/// One row: the name over its hint.
const ROW: f32 = 40.0;
/// A section band over the groups and over the clips of their own.
const BAND: f32 = 22.0;
/// Rows shown before the list scrolls.
const VISIBLE: usize = 8;

/// The picker's state: what it links, the filter, the rows, and where things were drawn.
#[derive(Default)]
pub struct LinkPicker {
    open: bool,
    /// The text the rows are narrowed by.
    pub filter: String,
    /// Only the link groups (kept across openings while the program runs).
    groups_only: bool,
    /// The clips being linked (the selection when it opened).
    sel: Vec<ClipRef>,
    /// Where it was asked to hang from; else the pointer.
    anchor: Option<Pos2>,
    /// Its top-left, fixed on the first frame (from the anchor or the pointer) so it does not
    /// move while one types; `None`: centred near the top of the window.
    at: Option<Pos2>,
    /// The list's height, fixed on the first frame from the whole list.
    list_h: f32,
    /// Every row for this selection, and the revision and locale it was built for.
    list: PickList,
    built: Option<(u64, u8)>,
    /// The rows the filter leaves (indices into `list`), and what they were filtered by.
    shown: Vec<usize>,
    shown_for: Option<(String, bool)>,
    /// The lead line, made for one width, locale and revision.
    lead: String,
    lead_for: Option<(u32, u8, u64)>,
    /// The rows shown changed (the filter, the switch): bring the highlight into view.
    reveal: bool,
    /// The list's scroll offset last frame, and where the keys want it this frame.
    offset: f32,
    scroll_to: Option<f32>,
    /// Whether the list overflowed its well last frame (its rows leave the scrollbar room).
    overflow: bool,
    /// The row Enter would choose: always one that links.
    highlight: Option<ClipRef>,
    /// The rows in view this frame (their index and whether hovered), painted together.
    painted: Vec<(usize, bool)>,
    /// Where each row in view was drawn last frame.
    rects: Vec<(ClipRef, Rect)>,
    panel: Option<Rect>,
    /// The frame it opened in: the click or key that opened it is not a click outside.
    fresh: bool,
}

impl LinkPicker {
    /// Open for linking `sel`, hanging from `anchor` (or the pointer).
    pub fn open(&mut self, sel: Vec<ClipRef>, anchor: Option<Pos2>) {
        *self = LinkPicker {
            open: true,
            sel,
            anchor,
            fresh: true,
            groups_only: self.groups_only,
            ..Default::default()
        };
    }

    /// Close; the *Linked only* switch stays as it was.
    pub fn close(&mut self) {
        *self = LinkPicker {
            groups_only: self.groups_only,
            ..Default::default()
        };
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Whether only the link groups are listed.
    pub fn groups_only(&self) -> bool {
        self.groups_only
    }

    /// The rows the list shows now (after the filter and the switch).
    pub fn rows(&self) -> Vec<&PickRow> {
        let all = self.list.rows();
        self.shown.iter().map(|&i| &all[i]).collect()
    }

    /// Where each row in view was drawn, by the clip choosing it would link to.
    pub fn row_rects(&self) -> &[(ClipRef, Rect)] {
        &self.rects
    }

    /// The row Enter would choose.
    pub fn highlighted(&self) -> Option<ClipRef> {
        self.highlight
    }

    /// Where the panel was drawn last frame.
    pub fn panel_rect(&self) -> Rect {
        self.panel.unwrap_or(Rect::NOTHING)
    }

    fn row(&self, i: usize) -> &PickRow {
        &self.list.rows()[self.shown[i]]
    }

    /// Build the list again when the project or the language changed, filter it again when
    /// the filter or the switch changed; keep the highlight on a row that links (the first
    /// one, when it is gone). A still frame compares, and makes nothing.
    fn rebuild(&mut self, app: &CantusApp) {
        let key = (app.bus.revision(), crate::i18n::locale() as u8);
        if self.built != Some(key) {
            self.list = PickList::build(app.project(), &self.sel);
            self.built = Some(key);
            self.shown_for = None;
            self.lead_for = None;
        }
        let same = self
            .shown_for
            .as_ref()
            .is_some_and(|(f, g)| *f == self.filter && *g == self.groups_only);
        if same {
            return;
        }
        self.shown = self.list.matching(&self.filter, self.groups_only);
        self.reveal = true;
        self.shown_for = Some((self.filter.clone(), self.groups_only));
        let rows = self.list.rows();
        let linking = |i: &&usize| rows[**i].refusal.is_none();
        let kept = self.highlight.filter(|h| {
            self.shown
                .iter()
                .filter(linking)
                .any(|&i| rows[i].group.first == *h)
        });
        self.highlight = kept.or_else(|| {
            self.shown
                .iter()
                .find(linking)
                .map(|&i| rows[i].group.first)
        });
    }

    /// Move the highlight `by` rows that link (up is negative), stopping at either end.
    fn step(&mut self, by: i32) {
        let rows = self.list.rows();
        let linking: Vec<ClipRef> = self
            .shown
            .iter()
            .map(|&i| &rows[i])
            .filter(|r| r.refusal.is_none())
            .map(|r| r.group.first)
            .collect();
        if linking.is_empty() {
            return;
        }
        let now = self
            .highlight
            .and_then(|h| linking.iter().position(|x| *x == h));
        let next = match now {
            None => 0,
            Some(i) => (i as i32 + by).clamp(0, linking.len() as i32 - 1) as usize,
        };
        self.highlight = Some(linking[next]);
    }
}

/// Link the selection to `target` and close: one undo step.
fn choose(app: &mut CantusApp, target: ClipRef) {
    let sel = std::mem::take(&mut app.link_picker.sel);
    app.link_picker.close();
    if app.clip_selection() != sel {
        app.set_clip_selection(sel);
    }
    app.link_clips_to(Some(target));
}

pub(crate) use paint::search_field;

#[path = "link_picker_paint.rs"]
mod paint;

/// Draw the picker when it is open, and act on what is chosen. Once a frame.
pub fn frame(app: &mut CantusApp, ctx: &Context) {
    if !app.link_picker.open {
        return;
    }
    let mut st = std::mem::take(&mut app.link_picker);
    let out = paint::show(&mut st, app, ctx);
    st.fresh = false;
    app.link_picker = st;
    match out {
        paint::Out::Stay => {}
        paint::Out::Close => app.link_picker.close(),
        paint::Out::Choose(target) => choose(app, target),
    }
}
