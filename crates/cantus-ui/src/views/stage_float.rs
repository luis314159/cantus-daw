//! The stage over the workspace (ADR 0091 §1): every dancer whose `view.mode` is floating
//! stands over the workspace at its `x`/`y` share of it, frameless and see-through, in an
//! area of its own — above the floating panels when it is pinned (`view.top`: egui's
//! foreground order), else among them (it comes to the front when clicked).
//!
//! Dragging its body moves it (`x`, `y`: one undo step per drag); its round grip resizes
//! it about its feet (`size`); its pill pins it, docks it or hides it. The frame shows only
//! while the pointer is over it, and lingers a moment after.

use super::chrome::{self, Button, Chrome, Lit};
use super::geom::{self, Area, FLOAT_STANDARD};
use super::{source, Drag, StageDancer, StageState, Surface, ViewMode};
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::{with_brush, Style};
use crate::views::workspace_ticker as ticker;
use cantus_core::{Command, MergeKey, TrackId};
use egui::{Context, Id, Order, Rect, Sense, Ui};
use std::time::Duration;

/// How long the frame lingers after the pointer left, in seconds.
pub const LINGER: f64 = 0.6;

/// What a gesture on the stage asks of the project.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Action {
    /// Stand at this share of the area (a drag: merged into one undo step).
    Move { track: TrackId, x: f32, y: f32 },
    /// This size (a drag of the grip: merged).
    Size { track: TrackId, size: f32 },
    /// Show here (`view.mode`).
    Mode { track: TrackId, mode: ViewMode },
    /// Pin above the floating panels, or not (`view.top`).
    Top { track: TrackId, on: bool },
    /// A drag ended: its undo step is sealed.
    End,
}

/// Gather this frame's dancers once (the docked tab draws before the overlay does).
pub fn begin(app: &mut CantusApp, ctx: &Context) {
    let nr = ctx.cumulative_pass_nr();
    if app.stage.gathered == Some(nr) {
        return;
    }
    let mut st = std::mem::take(&mut app.stage);
    st.gathered = Some(nr);
    st.passes += 1;
    st.bodies.clear();
    source::gather(app, &mut st);
    app.stage = st;
}

/// The overlay, over `area` (the workspace): called once a frame, after the dock.
pub fn show(app: &mut CantusApp, ui: &Ui, area: Rect) {
    let ctx = ui.ctx().clone();
    begin(app, &ctx);
    let playing = app.engine.is_playing();
    let mut st = std::mem::take(&mut app.stage);
    let mut actions = Vec::new();
    let dancers = std::mem::take(&mut st.dancers);
    let hidden = app.settings.stage.hidden;
    for d in dancers
        .iter()
        .filter(|d| d.mode == ViewMode::Floating && !hidden)
    {
        dancer(&ctx, &mut st, area, d, playing, &mut actions);
    }
    st.dancers = dancers;
    st.sweep();
    wake(&ctx, &st, playing);
    app.stage = st;
    apply(app, &actions);
    super::companion::frame(app, &ctx);
}

/// Ask for the next frame the stage needs: the idle breath while it is stopped (playing,
/// the transport already asks for 60 a second), the frame's fade, nothing when nothing
/// shows or the window is minimised.
pub(crate) fn wake(ctx: &Context, st: &StageState, playing: bool) {
    if st.slots.is_empty() || ctx.input(|i| i.viewport().minimized == Some(true)) {
        return;
    }
    if st.drag.is_some() {
        ticker::moving(ctx, ticker::Reason::Drag);
    } else if !playing {
        ticker::wake_after(ctx, Duration::from_secs_f64(1.0 / super::IDLE_RATE));
    }
    if let Some((_, _, until)) = st.hover {
        let now = ctx.input(|i| i.time);
        if until > now {
            ticker::wake_after(ctx, Duration::from_secs_f64((until - now).max(0.016)));
        }
    }
}

fn dancer(
    ctx: &Context,
    st: &mut StageState,
    area: Rect,
    d: &StageDancer,
    playing: bool,
    actions: &mut Vec<Action>,
) {
    let ppp = ctx.pixels_per_point();
    let a = Area::of(area, ppp, FLOAT_STANDARD);
    // What can be seen: the workspace, within the screen (egui's provisional screen before
    // a first frame at a new scale is far larger).
    let seen = area.intersect(ctx.content_rect());
    let clip_px = Rect::from_min_max(
        (seen.min.to_vec2() * ppp).to_pos2(),
        (seen.max.to_vec2() * ppp).to_pos2(),
    );
    let Some(placed) = geom::place(&a, clip_px, d.character.as_ref(), &d.pose, ppp) else {
        return;
    };
    let key = (d.track, Surface::Float);
    st.bodies.push((d.track, Surface::Float, placed.body));
    let dragging = st.drag.is_some_and(|g| drag_track(g) == d.track);
    let rate = super::rate(playing, dragging);
    let Some(tex) = st.texture(
        ctx,
        key,
        d.character.as_ref(),
        &placed.pose,
        &placed.stage,
        rate,
    ) else {
        return;
    };
    let s = Style::current();
    let now = ctx.input(|i| i.time);
    let showing = st
        .hover
        .is_some_and(|(tr, sf, until)| tr == d.track && sf == Surface::Float && until > now)
        || dragging;
    let buttons = [Button::Pin, Button::Dock, Button::Hide];
    let frame = showing.then(|| chrome::layout(ctx, placed.body, &d.name, &buttons, &s, area));
    let extent = frame.as_ref().map_or(placed.body, Chrome::extent);
    let order = if d.top {
        Order::Foreground
    } else {
        Order::Middle
    };
    let id = Id::new(("cantus-stage-float", d.track.0));
    let tex_rect = placed.rect(ppp);
    egui::Area::new(id)
        .order(order)
        .fixed_pos(extent.min)
        .constrain(false)
        .fade_in(false)
        .interactable(true)
        .show(ctx, |ui| {
            // The area is the body and its frame: there it takes the pointer, and nowhere
            // else — the rest of the box is see-through to clicks as to the eye.
            let _ = ui.allocate_rect(extent, Sense::hover());
            let painter = ui.painter().with_clip_rect(area);
            with_brush(&painter, |b| b.foreign(tex, tex_rect));
            let out = sense(ui, st, d, &a, placed.body, frame.as_ref(), ppp, actions);
            if let Some(c) = &frame {
                let fade = if out.hovered || dragging {
                    1.0
                } else {
                    let left = st.hover.map_or(0.0, |(_, _, u)| u - now);
                    (left / LINGER).clamp(0.0, 1.0) as f32
                };
                let lit = Lit {
                    hot: out.hot,
                    grip_hot: out.grip_hot,
                    body_hot: out.hovered,
                    pinned: d.top,
                    fade,
                };
                with_brush(&painter, |b| chrome::paint(b, c, &d.name, &s, lit));
            }
        });
}

/// What the pointer did to one dancer this frame.
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct Sensed {
    pub hovered: bool,
    pub hot: Option<Button>,
    pub grip_hot: bool,
}

pub(crate) fn drag_track(g: Drag) -> TrackId {
    match g {
        Drag::Move { track, .. } | Drag::Size { track, .. } => track,
    }
}

/// Sense the body, the grip and the pill of `d` (in `ui`), turning gestures into actions.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sense(
    ui: &mut Ui,
    st: &mut StageState,
    d: &StageDancer,
    a: &Area,
    body: Rect,
    frame: Option<&Chrome>,
    ppp: f32,
    actions: &mut Vec<Action>,
) -> Sensed {
    let ctx = ui.ctx().clone();
    let surface = if d.mode == ViewMode::Docked {
        Surface::Dock
    } else {
        Surface::Float
    };
    let base = Id::new(("cantus-stage", d.track.0, surface == Surface::Dock));
    let mut out = Sensed::default();
    let mut any = false;
    // The pill's buttons first: they sit over the body when there is no room above it.
    if let Some(c) = frame {
        let bar = ui.interact(c.bar, base.with("bar"), Sense::hover());
        any |= bar.hovered();
        for (btn, r) in &c.buttons {
            let resp = ui.interact(*r, base.with(*btn), Sense::click());
            if resp.hovered() {
                out.hot = Some(*btn);
                any = true;
            }
            let resp = tip(resp, btn.tip_key(d.top));
            if resp.clicked() {
                actions.push(match btn {
                    Button::Pin => Action::Top {
                        track: d.track,
                        on: !d.top,
                    },
                    Button::Dock => Action::Mode {
                        track: d.track,
                        mode: ViewMode::Docked,
                    },
                    Button::Float => Action::Mode {
                        track: d.track,
                        mode: ViewMode::Floating,
                    },
                    Button::Hide => Action::Mode {
                        track: d.track,
                        mode: ViewMode::Hidden,
                    },
                });
            }
        }
        let grip = ui.interact(c.grip, base.with("grip"), Sense::drag());
        out.grip_hot = grip.hovered() || grip.dragged();
        any |= out.grip_hot;
        let grip = tip(grip, "stage.tip.size");
        if grip.drag_started() {
            st.drag = Some(Drag::Size { track: d.track });
        }
        if grip.dragged() {
            if let Some(p) = grip.interact_pointer_pos() {
                let px = p.y * ppp;
                let size = geom::size_for_top(a, d.character.as_ref(), &d.pose, px);
                actions.push(Action::Size {
                    track: d.track,
                    size: size.clamp(0.25, 4.0),
                });
            }
        }
        if grip.drag_stopped() {
            st.drag = None;
            actions.push(Action::End);
        }
    }
    let resp = ui.interact(body, base.with("body"), Sense::click_and_drag());
    let resp = tip(resp, "stage.tip.move");
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Other,
            true,
            crate::i18n::tf("stage.a11y.dancer", &[("track", &d.name)]),
        )
    });
    out.hovered = resp.hovered() || resp.dragged();
    any |= out.hovered;
    if resp.drag_started() {
        if let Some(p) = resp.interact_pointer_pos() {
            let feet = a.feet(&d.pose);
            st.drag = Some(Drag::Move {
                track: d.track,
                grab: egui::vec2(p.x * ppp - feet.x, p.y * ppp - feet.y),
            });
        }
    }
    if resp.dragged() {
        if let (Some(p), Some(Drag::Move { grab, .. })) = (resp.interact_pointer_pos(), st.drag) {
            let (x, y) = a.share(egui::pos2(p.x * ppp - grab.x, p.y * ppp - grab.y));
            actions.push(Action::Move {
                track: d.track,
                x: x.clamp(0.0, 1.0),
                y: y.clamp(0.0, 1.0),
            });
        }
    }
    if resp.drag_stopped() {
        st.drag = None;
        actions.push(Action::End);
    }
    if any {
        st.hover = Some((d.track, surface, ctx.input(|i| i.time) + LINGER));
    }
    out
}

/// A tooltip, its words looked up only while the pointer is there (a frame costs nothing
/// for it otherwise).
fn tip(resp: egui::Response, key: &str) -> egui::Response {
    if resp.hovered() {
        resp.on_hover_text(t(key))
    } else {
        resp
    }
}

/// Carry out the stage's actions as commands.
pub fn apply(app: &mut CantusApp, actions: &[Action]) {
    for a in actions {
        // A dancer on no track of the project (a guest) changes nothing.
        let track = match *a {
            Action::Move { track, .. }
            | Action::Size { track, .. }
            | Action::Mode { track, .. }
            | Action::Top { track, .. } => Some(track),
            Action::End => None,
        };
        if track.is_some_and(|t| app.project().track(t).is_none()) {
            continue;
        }
        match *a {
            Action::Move { track, x, y } => {
                let cmd = Command::Batch {
                    label_key: "cmd.stage.move".into(),
                    commands: vec![
                        Command::SetInstrumentValue {
                            track,
                            key: "x".into(),
                            value: x,
                        },
                        Command::SetInstrumentValue {
                            track,
                            key: "y".into(),
                            value: y,
                        },
                    ],
                };
                app.exec_merging(cmd, MergeKey::of(&("stage-move", track.0)));
            }
            Action::Size { track, size } => app.set_inst_value(track, "size", size, true),
            Action::Mode { track, mode } => {
                app.set_inst_value(track, "view.mode", mode.value(), false);
                if mode == ViewMode::Docked {
                    app.workspace.reveal(crate::views::workspace::Pane::Stage);
                }
            }
            Action::Top { track, on } => {
                app.set_inst_value(track, "view.top", if on { 1.0 } else { 0.0 }, false);
            }
            Action::End => app.end_gesture(),
        }
    }
}
