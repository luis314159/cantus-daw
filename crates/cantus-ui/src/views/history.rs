//! **The undo history** (round 31, ADR 0072): Photoshop's and Studio One's history, as an
//! area of the workspace that docks, stacks and floats like the others (*View ▸ History*,
//! **Alt+H**; beside the inspector by default).
//!
//! Every step by its name (`history_text`: *Move 3 notes*, *Insert Concentus on Lead*),
//! oldest first, under a first row for the project as it was opened or begun. The current
//! position is marked (the accent bar, the row lit); the steps after it — the ones Redo
//! would bring back — are dimmed; the step the file holds is tagged *Saved*, and the steps
//! since the last save carry a dot. **A click on a row goes there**: each step between is
//! undone (or redone) as Undo and Redo do it, one after the other, so the history stays as
//! it was and a later click goes back. Long histories scroll; only the rows in view are
//! named and painted.

use crate::app::CantusApp;
use crate::history_text;
use crate::i18n::t;
use crate::kit::color::{mix, shade, with_alpha};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{with_brush, Brush, Fill, Style};
use egui::{pos2, vec2, Align2, Id, Rect, Sense, Ui};

/// One row.
pub const ROW: f32 = 26.0;
/// The line over the list.
const HEAD: f32 = 30.0;

/// The pane's own state.
#[derive(Default)]
pub struct HistoryView {
    offset: f32,
    scroll_to: Option<f32>,
    overflow: bool,
    /// The state shown as current last frame: a new one brings its row into view.
    shown_current: Option<u64>,
    /// Where each row in view was drawn last frame, by its state.
    pub rects: Vec<(u64, Rect)>,
}

/// One row of the list, as the bus holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// The state the row stands for.
    pub step: u64,
    pub name: String,
    /// It would be redone (after the current position).
    pub ahead: bool,
    /// The current position.
    pub current: bool,
    /// The state the file holds.
    pub saved: bool,
    /// A step the file does not hold yet (after the save, up to the current position).
    pub unsaved: bool,
}

/// The rows of the list: the start, every step to undo (oldest first), every step to redo.
/// `name_rows` names only rows in `range` (the rest keep an empty name): a long history
/// costs nothing for the rows out of view.
pub fn rows(app: &CantusApp, range: std::ops::Range<usize>) -> Vec<Row> {
    let bus = &app.bus;
    let project = bus.project();
    let undo = bus.history();
    let redo = bus.redo_history();
    let current = bus.current_step();
    // A project never saved has no file: no step is the file's (review 6).
    let saved = bus.saved_step().filter(|_| app.path.is_some());
    let n = 1 + undo.len() + redo.len();
    // Where the saved state sits among the rows (None: out of reach).
    let saved_at = saved.and_then(|s| {
        if s == bus.base_step() {
            Some(0)
        } else {
            undo.iter()
                .position(|e| e.step() == s)
                .map(|i| i + 1)
                .or_else(|| {
                    redo.iter()
                        .rev()
                        .position(|e| e.step() == s)
                        .map(|i| 1 + undo.len() + i)
                })
        }
    });
    let current_at = undo.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let (step, name) = if i == 0 {
            let name = if range.contains(&0) {
                t("workflow.history.start")
            } else {
                String::new()
            };
            (bus.base_step(), name)
        } else if i <= undo.len() {
            let e = &undo[i - 1];
            let name = if range.contains(&i) {
                let name = history_text::describe(&e.label_key, &e.command, &e.inverse, project);
                crate::ai_mcp::history_name(app, e, name) // r34:mcp — "AI: …"
            } else {
                String::new()
            };
            (e.step(), name)
        } else {
            let e = &redo[redo.len() - 1 - (i - 1 - undo.len())];
            let name = if range.contains(&i) {
                let name = history_text::describe(&e.label_key, &e.command, &e.inverse, project);
                crate::ai_mcp::history_name(app, e, name) // r34:mcp — "AI: …"
            } else {
                String::new()
            };
            (e.step(), name)
        };
        let unsaved = match saved_at {
            Some(s) => (i > s && i <= current_at) || (i <= s && i > current_at),
            None => i > 0 && i <= current_at,
        };
        out.push(Row {
            step,
            name,
            ahead: i > current_at,
            current: step == current,
            saved: saved_at == Some(i),
            unsaved,
        });
    }
    out
}

/// Go to the state `step`: undo or redo each step between, as the keys do, until it is
/// there (or a step refuses, said by the undo itself). Returns whether it got there.
pub fn go_to(app: &mut CantusApp, step: u64) -> bool {
    let below = app.bus.base_step() == step || app.bus.history().iter().any(|e| e.step() == step);
    let above = app.bus.redo_history().iter().any(|e| e.step() == step);
    if !below && !above {
        return false;
    }
    while app.bus.current_step() != step {
        let before = app.bus.current_step();
        if below {
            app.undo();
        } else {
            app.redo();
        }
        if app.bus.current_step() == before {
            return false;
        }
    }
    true
}

/// The pane.
pub fn show(app: &mut CantusApp, ui: &mut Ui) {
    let s = Style::current();
    let k = s.k;
    let full = ui.max_rect();
    with_brush(ui.painter(), |b| b.rect(full, Fill::Solid(s.t.ink)));
    let head = Rect::from_min_size(full.min, vec2(full.width(), HEAD * k));
    let undo_n = app.bus.history().len();
    let unsaved = if app.path.is_none() {
        t("workflow.history.never_saved")
    } else if app.bus.is_dirty() {
        t("workflow.history.unsaved")
    } else {
        t("workflow.history.all_saved")
    };
    let line = crate::i18n::tf_count(
        "workflow.history.count",
        undo_n,
        &[("redo", &app.bus.redo_history().len())],
    );
    with_brush(ui.painter(), |b| {
        b.rect(head, Fill::Solid(s.t.panel));
        // The count first; the save's word in what is left (cut, never over the count).
        let r = head.shrink2(vec2(10.0 * k, 0.0));
        let t = Text::new(&line, Role::Label, s.t.text)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let lw = text::measure(b.ctx(), &t, s.tk()).x.min(r.width());
        let _ = text::paint(b, r, &t, s.tk());
        let rest = Rect::from_min_max(pos2(r.left() + lw + 12.0 * k, r.top()), r.max);
        if rest.width() > 24.0 * k {
            let right = Text::new(&unsaved, Role::Label, s.t.text_dim)
                .align(Align2::RIGHT_CENTER)
                .policy(Policy::Elide(Elide::End));
            let _ = text::paint(b, rest, &right, s.tk());
        }
    });
    let body = Rect::from_min_max(pos2(full.left(), head.bottom()), full.max);
    let mut view = std::mem::take(&mut app.workflow.history);
    let clicked = list(app, ui, &mut view, &s, body);
    app.workflow.history = view;
    if let Some(step) = clicked {
        go_to(app, step);
    }
}

fn list(app: &CantusApp, ui: &mut Ui, st: &mut HistoryView, s: &Style, body: Rect) -> Option<u64> {
    let k = s.k;
    let row_h = ROW * k;
    let n = 1 + app.bus.history().len() + app.bus.redo_history().len();
    let current_at = app.bus.history().len();
    let total = n as f32 * row_h;
    let lane = Rect::from_min_max(
        pos2(body.right() - 6.0 * k, body.top() + 2.0 * k),
        pos2(body.right() - 1.0 * k, body.bottom() - 2.0 * k),
    );
    let rows_rect = if st.overflow {
        Rect::from_min_max(body.min, pos2(lane.left() - 2.0 * k, body.bottom()))
    } else {
        body
    };
    let current = app.bus.current_step();
    if st.shown_current != Some(current) {
        st.shown_current = Some(current);
        let y = current_at as f32 * row_h;
        if y < st.offset || y + row_h > st.offset + rows_rect.height() {
            st.scroll_to = Some((y + row_h - rows_rect.height() * 0.5).max(0.0));
        }
    }
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rows_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(rows_rect.intersect(ui.clip_rect()));
    let mut scroll = egui::ScrollArea::vertical()
        .id_salt("cantus-history-list")
        .auto_shrink([false, false])
        .max_height(rows_rect.height())
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
    if let Some(y) = st.scroll_to.take() {
        scroll = scroll.vertical_scroll_offset(y);
    }
    let mut chosen = None;
    st.rects.clear();
    let out = scroll.show_viewport(&mut child, |ui, view| {
        ui.set_height(total);
        let (top, left, w) = (
            ui.max_rect().top(),
            ui.max_rect().left(),
            ui.max_rect().width(),
        );
        let from = ((view.min.y / row_h).floor().max(0.0) as usize).min(n);
        let to = ((view.max.y / row_h).ceil() as usize + 1).min(n);
        let all = rows(app, from..to);
        let mut painted = Vec::with_capacity(to - from);
        for (i, row) in all.iter().enumerate().take(to).skip(from) {
            let rect = Rect::from_min_size(pos2(left, top + i as f32 * row_h), vec2(w, row_h));
            let resp = ui.interact(rect, Id::new(("cantus-history-row", i)), Sense::click());
            if resp.clicked() && !row.current {
                chosen = Some(row.step);
            }
            if resp.hovered() {
                let what = if row.current {
                    t("workflow.history.tip_current")
                } else if row.ahead {
                    t("workflow.history.tip_redo")
                } else {
                    t("workflow.history.tip_undo")
                };
                crate::kit::hint::set_text(
                    ui.ctx(),
                    &row.name,
                    &what,
                    crate::kit::hint::Caps::NONE,
                );
            }
            painted.push((i, rect, resp.hovered()));
            st.rects.push((row.step, rect));
        }
        let painter = ui.painter().clone();
        with_brush(&painter, |b| {
            for &(i, rect, hover) in &painted {
                paint_row(b, rect, s, i, &all[i], hover);
            }
        });
    });
    st.offset = out.state.offset.y;
    let content = out.content_size.y.max(1.0);
    let len = (out.inner_rect.height() / content).min(1.0);
    st.overflow = len < 1.0;
    if st.overflow {
        let mut start = out.state.offset.y / content;
        if crate::kit::widgets::scrollbar(
            ui,
            lane,
            Id::new("cantus-history-scroll"),
            &mut start,
            len,
        )
        .dragged()
        {
            st.scroll_to = Some(start * content);
        }
    }
    chosen
}

/// One row: its number, the dot of a step not saved, the name, the *Saved* tag.
fn paint_row(b: &mut Brush, rect: Rect, s: &Style, i: usize, row: &Row, hover: bool) {
    let k = s.k;
    if row.current {
        b.rect(rect, Fill::Solid(mix(s.t.ink, s.t.selection, 0.24)));
        let bar = Rect::from_min_size(rect.min, vec2(3.0 * k, rect.height()));
        b.rect(bar, Fill::Solid(s.t.selection));
    } else if hover {
        b.rect(
            rect,
            Fill::Solid(shade(s.t.ink, if s.t.dark { 0.06 } else { -0.04 })),
        );
    }
    b.hline(
        rect.left(),
        rect.right(),
        rect.bottom() - b.px() * 0.5,
        b.px(),
        shade(s.t.ink, -0.08),
    );
    let ink = if row.ahead {
        with_alpha(s.t.text, 0.42)
    } else {
        s.t.text
    };
    let body = rect.shrink2(vec2(10.0 * k, 0.0));
    let num_r = Rect::from_min_size(body.min, vec2(30.0 * k, body.height()));
    if i > 0 {
        let n = i.to_string();
        let num =
            Text::new(&n, Role::Value, with_alpha(s.t.text_dim, 0.8)).align(Align2::RIGHT_CENTER);
        let _ = text::paint(b, num_r, &num, s.tk());
    }
    let dot = pos2(num_r.right() + 9.0 * k, body.center().y);
    if row.unsaved {
        b.circle(
            dot,
            2.5 * k,
            Fill::Solid(with_alpha(s.t.led, if row.ahead { 0.4 } else { 1.0 })),
        );
    }
    let mut right = body.right();
    if row.saved {
        let tag = Text {
            caps: true,
            ..Text::key("workflow.history.saved", Role::Section, s.t.label)
                .align(Align2::RIGHT_CENTER)
        };
        let w = text::measure(b.ctx(), &tag, s.tk()).x;
        let _ = text::paint(b, body, &tag, s.tk());
        right -= w + 10.0 * k;
    }
    let name_r = Rect::from_min_max(
        pos2(dot.x + 9.0 * k, body.top()),
        pos2(right, body.bottom()),
    );
    let nt = Text::new(&row.name, Role::Body, ink)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let _ = text::paint(b, name_r, &nt, s.tk());
}
