//! r39:el-arrange — the song map as the horizontal scrollbar (`des-arrange`, *Song map*):
//! the panel's 14-px foot, every track a line where it has clips (its colour at 55 %, the
//! selected one's at 90 %), the view a frame (drag it to scroll, its ends to zoom; press
//! elsewhere to go there), the playhead and the loop in the accent. The header column under
//! it reads the song's length. And the vertical scroll: a 3-px thumb at the lanes' right
//! edge, only when the rows overflow.

use super::slate::{say, Ink, W};
use super::{ticks_to_beats, PX_MAX, PX_MIN};
use crate::app::CantusApp;
use crate::kit::{self, Fill, Style};
use egui::{pos2, vec2, Align2, Id, Rect, Sense};

pub const MAP_H: f32 = 14.0;

/// What the map is dragging.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Grab {
    /// The frame, from where it was (in beats) at the press.
    Frame(f32),
    /// Its left or right end (zoom).
    Left,
    Right,
    /// The vertical thumb, from the scroll at the press.
    Thumb(f32),
}

/// The map's span in beats: the song, and the view if it reaches further.
fn total(app: &CantusApp, visible: f32) -> f32 {
    super::song_beats(app).max(app.arrangement.scroll_beats + visible)
}

/// Paint and handle the map in `rect`, and the song's length in `foot`.
pub(super) fn show(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    rect: Rect,
    foot: Rect,
    lanes: Rect,
    s: &Style,
) {
    let ink = Ink::of(s);
    let k = s.k;
    let visible = lanes.width() / app.arrangement.px_per_beat;
    let total = total(app, visible).max(1.0);
    let x_of = |beats: f32| rect.left() + beats / total * rect.width();
    let v0 = app.arrangement.scroll_beats;
    let frame = Rect::from_min_max(
        pos2(x_of(v0), rect.top() + 1.0 * k),
        pos2(
            x_of(v0 + visible).min(rect.right()),
            rect.bottom() - 1.0 * k,
        ),
    );
    {
        let a: &CantusApp = app;
        let p = a.project();
        kit::with_brush(&ui.painter().with_clip_rect(rect.union(foot)), |b| {
            b.register(
                rect,
                "song_map",
                &[kit::Tok::Toolbar, kit::Tok::Highlight],
                &[],
            );
            b.rect(b.snap_rect(rect), Fill::Solid(ink.surface));
            b.rect(b.snap_rect(foot), Fill::Solid(ink.surface));
            let words = &a.arrangement.words.foot;
            let _ = say(
                b,
                foot.shrink2(vec2(10.0 * k, 0.0)),
                words,
                W::SemiBold,
                9.0,
                ink.ink3,
                Align2::LEFT_CENTER,
                s,
            );
            let n = p.tracks.len().max(1) as f32;
            let h = ((MAP_H - 6.0) * k / n).max(b.px());
            let mut y = rect.top() + 3.0 * k;
            for t in &p.tracks {
                let sel = a.selected_track == Some(t.id);
                let c = ink.tint(
                    crate::theme::track_color(t.color),
                    if sel { 0.90 } else { 0.55 },
                );
                for clip in &t.clips {
                    let x0 = x_of(ticks_to_beats(clip.start));
                    let x1 = x_of(ticks_to_beats(clip.end())).max(x0 + b.px());
                    b.rect(
                        Rect::from_min_max(pos2(x0, y), pos2(x1, y + h)),
                        Fill::Solid(c),
                    );
                }
                y += h;
            }
            b.rrect(frame, s.r(3.0), Fill::Solid(Ink::a(ink.ink1, 0.06)));
            b.rrect_stroke(frame, s.r(3.0), b.px(), ink.ink3);
            if let (Some(l), true) = (&p.loop_region, p.loop_enabled) {
                let (x0, x1) = (x_of(ticks_to_beats(l.start)), x_of(ticks_to_beats(l.end)));
                b.hline(x0, x1, rect.top() + 0.5 * b.px(), b.px(), ink.accent);
            }
            let ph = x_of(ticks_to_beats(a.engine.position()));
            b.vline(ph, rect.top(), rect.bottom(), b.px(), ink.accent);
        });
    }
    // Input: the frame's ends zoom, the frame scrolls, elsewhere goes there.
    let resp = ui.interact(rect, Id::new("arr-song-map"), Sense::click_and_drag());
    let grip = 4.0 * k;
    let at_left = |x: f32| (x - frame.left()).abs() <= grip;
    let at_right = |x: f32| (x - frame.right()).abs() <= grip;
    if let Some(p) = resp.hover_pos() {
        if at_left(p.x) || at_right(p.x) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        crate::kit::hint::set_text(
            ui.ctx(),
            super::chrome::word("arrange.map.tip"),
            "",
            crate::kit::hint::Caps::NONE,
        );
    }
    let beats_at = |x: f32| (x - rect.left()) / rect.width() * total;
    if resp.drag_started() || resp.clicked() {
        if let Some(p) = ui.input(|i| i.pointer.press_origin()) {
            let g = if at_left(p.x) {
                Grab::Left
            } else if at_right(p.x) {
                Grab::Right
            } else if frame.contains(p) {
                Grab::Frame(v0)
            } else {
                // Elsewhere: the view centred there, then dragged from there.
                let c = (beats_at(p.x) - visible * 0.5).max(0.0);
                app.arrangement.scroll_beats = c;
                Grab::Frame(c)
            };
            app.arrangement.map_grab = Some(g);
        }
    }
    if resp.dragged() {
        if let (Some(g), Some(p), Some(o)) = (
            app.arrangement.map_grab,
            resp.interact_pointer_pos(),
            ui.input(|i| i.pointer.press_origin()),
        ) {
            let v = &mut app.arrangement;
            match g {
                Grab::Frame(from) => {
                    v.scroll_beats = (from + beats_at(p.x) - beats_at(o.x)).max(0.0);
                }
                Grab::Left => {
                    let end = v.scroll_beats + visible;
                    let start = beats_at(p.x).clamp(0.0, end - 0.25);
                    v.px_per_beat = (lanes.width() / (end - start)).clamp(PX_MIN, PX_MAX);
                    v.scroll_beats = (end - lanes.width() / v.px_per_beat).max(0.0);
                }
                Grab::Right => {
                    let end = beats_at(p.x).max(v.scroll_beats + 0.25);
                    v.px_per_beat = (lanes.width() / (end - v.scroll_beats)).clamp(PX_MIN, PX_MAX);
                }
                Grab::Thumb(_) => {}
            }
        }
    }
    if resp.drag_stopped() {
        app.arrangement.map_grab = None;
    }
}

/// The vertical scroll: a 3-px thumb at `lanes`' right edge, while the rows overflow.
pub(super) fn thumb(app: &mut CantusApp, ui: &mut egui::Ui, lanes: Rect, content: f32, s: &Style) {
    let k = s.k;
    let view = lanes.height();
    if content <= view + 0.5 {
        return;
    }
    let ink = Ink::of(s);
    let y0 = app.arrangement.scroll_y;
    let h = (view / content * view).max(20.0 * k);
    let top = lanes.top() + y0 / (content - view) * (view - h);
    let bar = Rect::from_min_size(pos2(lanes.right() - 5.0 * k, top), vec2(3.0 * k, h));
    let zone = Rect::from_min_max(pos2(lanes.right() - 9.0 * k, lanes.top()), lanes.max);
    let resp = ui.interact(zone, Id::new("arr-vthumb"), Sense::drag());
    if resp.hovered() {
        crate::kit::hint::set_text(
            ui.ctx(),
            super::chrome::word("arrange.thumb.tip"),
            "",
            crate::kit::hint::Caps::NONE,
        );
    }
    kit::with_brush(&ui.painter().with_clip_rect(lanes), |b| {
        let c = if resp.hovered() || resp.dragged() {
            ink.ink3
        } else {
            ink.ink4
        };
        b.rrect(bar, 1.5 * k, Fill::Solid(c));
    });
    if resp.drag_started() {
        app.arrangement.map_grab = Some(Grab::Thumb(y0));
    }
    if resp.dragged() {
        if let (Some(Grab::Thumb(from)), Some(p), Some(o)) = (
            app.arrangement.map_grab,
            resp.interact_pointer_pos(),
            ui.input(|i| i.pointer.press_origin()),
        ) {
            let per = (content - view) / (view - h).max(1.0);
            app.arrangement.scroll_y = (from + (p.y - o.y) * per).clamp(0.0, content - view);
        }
    }
    if resp.drag_stopped() {
        app.arrangement.map_grab = None;
    }
}
