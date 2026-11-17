//! Fractus's lanes under the pointer (ADR 0137, phase B) — the display is the sequencer, so
//! it is painted on as one:
//!
//! - **Press on a step and drag along the lane**: a block from the step pressed to the step
//!   under the pointer (either way), **as tall as the pointer's height in the row** — its
//!   amount, the pad lit from its floor to there (5 % steps). What the block covers is its
//!   own (`cantus_core::fx::fractus::paint`); the lane's other blocks stay.
//! - **Shift and drag**: a stroke for **chance** — every block the stroke passes on the lane
//!   takes the chance the pointer's height reads, along the stroke.
//! - **Right-click a block**: it goes.
//! - **The Frozen lamp** (over the cloud): a click freezes or thaws the cloud.
//!
//! Each gesture is one step of undo (`FxHost::set_fractus_lane`, merged by the gesture); a
//! plug-in host gets the lanes as content in its state.

use super::super::frame::tr;
use super::super::host::FxHost;
use super::display::Laid;
use cantus_core::fx::fractus::{self as model, Block, LANES};
use egui::{pos2, Pos2, Rect, Sense};
use std::sync::atomic::{AtomicU64, Ordering};

/// Gestures are numbered across every editor and context: two strokes never merge.
static NEXT: AtomicU64 = AtomicU64::new(1);

/// What the pointer is doing on the lanes, kept between frames.
#[derive(Debug, Default)]
pub struct Stroke {
    /// Where the display put its lanes last frame (the tests point at them).
    pub laid: Option<Laid>,
    /// The Frozen switch's place last frame.
    pub frozen: Option<Rect>,
    held: Option<Held>,
}

#[derive(Debug, Clone)]
struct Held {
    lane: usize,
    /// The step pressed.
    from: usize,
    /// Shift was held: a chance stroke.
    chance: bool,
    /// The lane as it was when the press came.
    base: Vec<Block>,
    gesture: u64,
    /// A chance stroke's last step and value: the blocks between take the line between.
    last: Option<(usize, f32)>,
    /// What was sent last (nothing new, nothing sent).
    sent: Option<Vec<Block>>,
}

impl Stroke {
    /// Whether a stroke is being painted.
    pub fn painting(&self) -> bool {
        self.held.is_some()
    }
}

/// Lane `lane`'s row across the grid.
pub fn row(laid: &Laid, lane: usize) -> Rect {
    let rh = laid.grid.height() / LANES as f32;
    let y0 = laid.grid.top() + lane as f32 * rh;
    Rect::from_min_max(pos2(laid.grid.left(), y0), pos2(laid.grid.right(), y0 + rh))
}

/// The pads' well in a row: the amount is read from its floor (0) to its top (1).
fn well(laid: &Laid, row: Rect) -> (f32, f32) {
    let inset = laid.inset;
    (row.bottom() - inset, (row.height() - 2.0 * inset).max(1.0))
}

/// Step `step`'s column's centre.
pub fn step_x(laid: &Laid, step: usize) -> f32 {
    laid.grid.left() + (step as f32 + 0.5) * laid.grid.width() / laid.steps.max(1) as f32
}

/// The height in lane `lane`'s row where the pointer reads `amount`.
pub fn amount_y(laid: &Laid, lane: usize, amount: f32) -> f32 {
    let (floor, h) = well(laid, row(laid, lane));
    floor - amount * h
}

/// The step under `x`, inside the cycle.
fn step_of(laid: &Laid, x: f32) -> usize {
    let t = (x - laid.grid.left()) / laid.grid.width().max(1.0);
    ((t * laid.steps as f32).floor().max(0.0) as usize).min(laid.steps.saturating_sub(1))
}

/// The lane under `y`.
fn lane_of(laid: &Laid, y: f32) -> usize {
    let t = (y - laid.grid.top()) / laid.grid.height().max(1.0);
    ((t * LANES as f32).floor().max(0.0) as usize).min(LANES - 1)
}

/// What the pointer's height in lane `lane`'s row reads: 5 % steps, 5 % … 100 %.
pub fn amount_at(laid: &Laid, lane: usize, y: f32) -> f32 {
    let (floor, h) = well(laid, row(laid, lane));
    let a = ((floor - y) / h).clamp(0.0, 1.0);
    ((a * 20.0).round() / 20.0).max(0.05)
}

/// The pointer on the lanes and the Frozen switch, this frame.
pub fn pointer(
    host: &mut dyn FxHost,
    st: &mut Stroke,
    ui: &mut egui::Ui,
    laid: &Laid,
    frozen: Rect,
) {
    // The Frozen switch: the lamp and its word.
    if frozen.is_positive() {
        let resp = ui.interact(frozen, ui.id().with("fractus.freeze"), Sense::click());
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            crate::kit::hint::fallback(
                ui.ctx(),
                tr("fractus.d.frozen"),
                tr("fractus.h.freeze"),
                crate::kit::hint::Caps::NONE,
            );
        }
        if resp.clicked() {
            let on = host.spec().get("cfreeze") >= 0.5;
            host.set("cfreeze", if on { 0.0 } else { 1.0 }, None);
        }
    }
    if !laid.grid.is_positive() || laid.steps == 0 {
        return;
    }
    let resp = ui.interact(
        laid.grid,
        ui.id().with("fractus.grid"),
        Sense::click_and_drag(),
    );
    if resp.hovered() && st.held.is_none() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        crate::kit::hint::fallback(
            ui.ctx(),
            tr("fractus.d.lanes"),
            tr("fractus.h.grid"),
            crate::kit::hint::Caps::NONE,
        );
    }
    // Shift as the frame's modifiers say, or as the press itself carried it.
    let (pos, pressed, down, shift) = ui.input(|i| {
        let pressed_shift = i.events.iter().any(|e| {
            matches!(
                e,
                egui::Event::PointerButton {
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers,
                    ..
                } if modifiers.shift
            )
        });
        (
            i.pointer.interact_pos(),
            i.pointer.primary_pressed(),
            i.pointer.primary_down(),
            i.modifiers.shift || pressed_shift,
        )
    });
    // A right-click takes the block under it.
    if resp.secondary_clicked() {
        if let Some(p) = pos.filter(|p| laid.grid.contains(*p)) {
            let (lane, step) = (lane_of(laid, p.y), step_of(laid, p.x));
            let now = lane_blocks(host, lane);
            let left = model::erase(&now, step);
            if left != now {
                host.set_fractus_lane(lane as u8, left, None);
            }
        }
    }
    if st.held.is_none() && pressed {
        if let Some(p) = pos.filter(|p| laid.grid.contains(*p) && resp.contains_pointer()) {
            let lane = lane_of(laid, p.y);
            st.held = Some(Held {
                lane,
                from: step_of(laid, p.x),
                chance: shift,
                base: lane_blocks(host, lane),
                gesture: NEXT.fetch_add(1, Ordering::Relaxed),
                last: None,
                sent: None,
            });
        }
    }
    let Some(h) = st.held.as_mut() else {
        return;
    };
    if let Some(p) = pos {
        let blocks = if h.chance {
            chance_stroke(laid, h, p)
        } else {
            let to = step_of(laid, p.x);
            let (a, b) = (h.from.min(to), h.from.max(to));
            let block = Block {
                amount: amount_at(laid, h.lane, p.y),
                ..Block::new(a as u16, (b - a + 1) as u16)
            };
            model::paint(&h.base, block)
        };
        if h.sent.as_ref() != Some(&blocks) {
            host.set_fractus_lane(h.lane as u8, blocks.clone(), Some(h.gesture));
            h.sent = Some(blocks);
        }
    }
    if !down {
        st.held = None;
        host.end_gesture();
    }
}

/// A chance stroke reaching `p`: every block between the last step and this one takes the
/// chance along the line between their heights.
fn chance_stroke(laid: &Laid, h: &mut Held, p: Pos2) -> Vec<Block> {
    let step = step_of(laid, p.x);
    let v = amount_at(laid, h.lane, p.y);
    let (s0, v0) = h.last.unwrap_or((step, v));
    h.last = Some((step, v));
    let (lo, hi) = (s0.min(step), s0.max(step));
    let mut blocks = h.sent.clone().unwrap_or_else(|| h.base.clone());
    for b in &mut blocks {
        let (from, to) = (usize::from(b.at), b.end());
        if to <= lo || from > hi {
            continue;
        }
        // The block's own step on the stroke: where the stroke crossed it nearest its start.
        let at = from.clamp(lo, hi);
        let t = if hi == lo {
            1.0
        } else {
            (at as f32 - s0 as f32) / (step as f32 - s0 as f32)
        };
        let c = v0 + (v - v0) * t.clamp(0.0, 1.0);
        b.prob = ((c * 20.0).round() / 20.0).clamp(0.05, 1.0);
    }
    blocks
}

/// The blocks lane `lane` holds now.
fn lane_blocks(host: &dyn FxHost, lane: usize) -> Vec<Block> {
    host.spec()
        .fractus
        .as_deref()
        .map(|p| p.lane(lane).to_vec())
        .unwrap_or_default()
}
