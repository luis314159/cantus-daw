//! The **More** key at the bar's right end and what hangs from it (ADR 0052; round 39,
//! `dir-a`'s phase B):
//!
//! - **The More panel** (`panel`): built from what the bar folded — each large group a
//!   section, the small folds one-line rows under *Also here* — usable there as on the bar.
//!   The key carries the number of folded groups in a small pill. A click inside the panel,
//!   or inside a popover it opened (the metronome's options, the grid list…), keeps it open;
//!   a click elsewhere, Esc or the More key closes it.
//! - **Customise top bar…** (also in *View* and on a right-click of the bar): every group
//!   in the bar's order — tick to show, drag by its grip to reorder, *Reset* for the
//!   default. Kept in `Settings::topbar`.

use super::layout::{self, Group, Layout};
use super::paint as p;
use super::{strip_tip, Style};
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::glyph::{self, Glyph};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::tokens::Tok;
use crate::kit::{led, with_brush, Fill, State};
use egui::{vec2, Align2, Id, Order, Pos2, Rect, Sense, Ui};

/// The customise panel's state.
#[derive(Debug, Default)]
pub struct Customise {
    pub open: bool,
    /// The overflow panel is open (its own state: see [`key`]).
    pub overflow: bool,
    /// The row being dragged, and where it would land.
    drag: Option<(usize, usize)>,
}

/// Open the overflow panel (screenshots and tests).
pub fn open_overflow(app: &mut CantusApp) {
    app.topbar.customise.overflow = true;
}

/// Whether the overflow panel is open.
pub fn overflow_open(app: &CantusApp) -> bool {
    app.topbar.customise.overflow
}

/// Open the customise panel.
pub fn open_customise(app: &mut CantusApp) {
    app.topbar.customise.open = true;
}

/// The More key in `r`, the number of folded groups in a raised pill at its top right, and
/// the panel while it is open.
///
/// The panel's open state is the bar's own, not egui's one popup slot: the folded groups'
/// own menus and popovers (the grid list, the metronome's options, the metre's editor, the
/// zoom steps…) are popups of their own, and opening one must not close the panel that
/// holds the group (review 1).
pub fn key(app: &mut CantusApp, ui: &mut Ui, r: Rect, l: &Layout, s: &Style) {
    let k = s.k;
    let open = app.topbar.customise.overflow;
    let n = l.folded().len();
    let resp = ui.interact(r, Id::new(("toolbar-key", "overflow")), Sense::click());
    let st = State::of(&resp, false);
    with_brush(ui.painter(), |b| {
        b.register(r, "toolbar_key", crate::kit::cap::WELL_KEY_TOKENS, &[]);
        let fill = open.then(|| crate::kit::color::opaque(s.t.inner_shadow));
        p::well(b, r, s, fill);
        p::tint(b, r, s, st, s.t.highlight, 0.2);
        let gst = State {
            hovered: st.hovered || open,
            ..st
        };
        p::icon(b, r, s, Glyph::More, gst, s.t.highlight, 24.0);
        if n > 0 {
            // The count in a 13×11 raised pill at the key's top right.
            let pill = Rect::from_min_size(
                Pos2::new(r.left() + 21.0 * k, r.top() - 3.0 * k),
                vec2(13.0 * k, 11.0 * k),
            );
            let bad = l.folded().contains(&Group::Device)
                && crate::views::audio_chip::health(app) != crate::views::audio_chip::Health::Good;
            let fill = if bad { s.t.meter_high } else { s.t.menu };
            b.rrect(b.snap_rect(pill), 5.5 * k, crate::kit::Fill::Solid(fill));
            b.rrect_stroke(
                b.snap_rect(pill).shrink(b.px() * 0.5),
                5.0 * k,
                b.px(),
                crate::kit::color::opaque(s.t.inner_shadow),
            );
            let mut w = [0u8; 2];
            let digit = (b'0' + n.min(9) as u8) as char;
            let word = digit.encode_utf8(&mut w);
            let ink = if bad { s.t.ink } else { p::ink2(s) };
            p::words(
                b,
                pill,
                word,
                Role::Micro,
                8.5,
                ink,
                Align2::CENTER_CENTER,
                s,
            );
        }
    });
    let resp = strip_tip(ui, resp, || {
        if n == 0 {
            t("topbar.more_none")
        } else {
            crate::i18n::tf_count("topbar.more_tip", n, &[])
        }
    });
    if resp.clicked() {
        app.topbar.customise.overflow = !open;
    }
    if !app.topbar.customise.overflow {
        return;
    }
    let ctx = ui.ctx().clone();
    // Closed by Esc (when no popover of its own is open), or by a click on the window
    // itself — not inside the panel, nor inside a popover it opened (a foreground layer),
    // nor on the More key.
    let id = Id::new("toolbar-overflow");
    let last = ctx.data(|d| d.get_temp::<Rect>(id.with("rect")));
    let (click, at) = ctx.input(|i| (i.pointer.any_click(), i.pointer.interact_pos()));
    let outside = click
        && !resp.hovered()
        && at.is_some_and(|p| {
            !last.is_some_and(|r| r.contains(p))
                && ctx
                    .layer_id_at(p)
                    .is_none_or(|l| !matches!(l.order, Order::Foreground | Order::Tooltip))
        });
    let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !egui::Popup::is_any_open(&ctx);
    if (outside || esc) && !resp.clicked() {
        app.topbar.customise.overflow = false;
        return;
    }
    let rect = super::panel::show(app, &ctx, r, l, s);
    ctx.data_mut(|d| d.insert_temp(id.with("rect"), rect));
}

// ---- customise -----------------------------------------------------------------------

const ROW: f32 = 24.0;
const PANEL_W: f32 = 300.0;

/// The customise panel, under the bar's right end, while open.
pub fn customise_panel(app: &mut CantusApp, ctx: &egui::Context, bar: Rect) {
    if !app.topbar.customise.open {
        return;
    }
    let s = Style::current();
    let k = s.k;
    let order = layout::resolve(&app.settings.topbar.order);
    let head_h = 32.0 * k;
    let foot_h = 40.0 * k;
    let size = vec2(
        PANEL_W * k,
        head_h + order.len() as f32 * ROW * k + foot_h + 8.0 * k,
    );
    let screen = ctx.content_rect();
    let size = size.min(screen.size() - vec2(16.0, 16.0));
    let pos = Pos2::new(
        (bar.right() - size.x - 8.0 * k).max(screen.left() + 8.0),
        bar.bottom() + 4.0 * k,
    );
    let size = vec2(size.x, size.y.min(screen.bottom() - pos.y - 8.0));
    let mut close = false;
    let mut changed = false;
    let mut topbar = app.settings.topbar.clone();
    egui::Area::new(Id::new("cantus-topbar-customise"))
        .order(Order::Foreground)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            let rect = Rect::from_min_size(pos, size);
            ui.set_min_size(size);
            crate::views::audio_parts::panel(ui, rect, &s);
            let head = Rect::from_min_size(rect.min, vec2(rect.width(), head_h));
            if crate::views::workspace_settings::title(
                ui,
                head,
                &s,
                Glyph::Gear,
                "topbar.customise_title",
            ) {
                close = true;
            }
            let body = Rect::from_min_max(
                Pos2::new(rect.left() + 8.0 * k, head.bottom() + 4.0 * k),
                Pos2::new(rect.right() - 8.0 * k, rect.bottom() - foot_h),
            );
            // The rows scroll when the window is shorter than the list.
            let mut list = ui.new_child(egui::UiBuilder::new().max_rect(body));
            egui::ScrollArea::vertical()
                .max_height(body.height())
                .auto_shrink([false, true])
                .show(&mut list, |ui| {
                    let h = order.len() as f32 * ROW * k;
                    let (rows_r, _) = ui.allocate_exact_size(vec2(body.width(), h), Sense::hover());
                    changed |= rows(app, ui, rows_r, &order, &mut topbar, &s);
                });
            let foot =
                Rect::from_min_max(Pos2::new(body.left(), body.bottom() + 6.0 * k), rect.max);
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(foot)
                    .layout(egui::Layout::left_to_right(egui::Align::Min)),
            );
            if crate::views::workspace_widgets::button(
                &mut child,
                Src::Key("topbar.reset"),
                0.0,
                true,
            )
            .clicked()
            {
                topbar.order.clear();
                topbar.hidden.clear();
                topbar.shown.clear(); // r33:sync
                changed = true;
            }
            child.add_space(6.0 * k);
            if crate::views::workspace_widgets::button(
                &mut child,
                Src::Key("topbar.done"),
                0.0,
                true,
            )
            .clicked()
            {
                close = true;
            }
        });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        close = true;
    }
    if changed {
        app.settings.topbar = topbar;
        app.store_settings();
    }
    if close {
        app.topbar.customise.open = false;
        app.topbar.customise.drag = None;
    }
}

/// The groups' rows: grip, lamp (shown), name. Returns whether anything changed.
fn rows(
    app: &mut CantusApp,
    ui: &mut Ui,
    body: Rect,
    order: &[Group],
    topbar: &mut crate::settings::TopBarSettings,
    s: &Style,
) -> bool {
    let k = s.k;
    let row_h = ROW * k;
    let mut changed = false;
    let row_at = |i: usize| {
        Rect::from_min_size(
            Pos2::new(body.left(), body.top() + i as f32 * row_h),
            vec2(body.width(), row_h),
        )
    };
    let drag = app.topbar.customise.drag;
    for (i, g) in order.iter().enumerate() {
        let r = row_at(i);
        let in_use = app.settings.sync.source != super::sync_service::SyncSource::None; // r33:sync
        let shown = g.is_shown(&topbar.hidden, &topbar.shown, in_use); // r33:sync
        let grip = Rect::from_min_size(r.min, vec2(22.0 * k, row_h));
        let lamp = Rect::from_min_size(Pos2::new(grip.right(), r.top()), vec2(22.0 * k, row_h));
        let gresp = ui.interact(grip, Id::new(("topbar-grip", i)), Sense::drag());
        let lresp = ui.interact(
            Rect::from_min_max(lamp.min, r.max),
            Id::new(("topbar-show", i)),
            Sense::click(),
        );
        if gresp.hovered() || gresp.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            crate::kit::hint::set_text(
                ui.ctx(),
                &t("topbar.customise_drag"),
                "",
                crate::kit::hint::Caps::NONE,
            );
        }
        if gresp.drag_started() {
            app.topbar.customise.drag = Some((i, i));
        }
        if let (Some((from, _)), Some(p)) =
            (app.topbar.customise.drag, gresp.interact_pointer_pos())
        {
            if from == i {
                let to =
                    (((p.y - body.top()) / row_h).floor().max(0.0) as usize).min(order.len() - 1);
                app.topbar.customise.drag = Some((from, to));
            }
        }
        if gresp.drag_stopped() {
            if let Some((from, to)) = app.topbar.customise.drag.take() {
                if from != to {
                    let mut ids: Vec<Group> = order.to_vec();
                    let g = ids.remove(from);
                    ids.insert(to, g);
                    topbar.order = ids.iter().map(|g| g.id().to_string()).collect();
                    changed = true;
                }
            }
        }
        if lresp.clicked() {
            if shown {
                topbar.hidden.push(g.id().to_string());
                topbar.shown.retain(|h| h != g.id()); // r33:sync
            } else {
                topbar.hidden.retain(|h| h != g.id());
                if g.hidden_by_default() {
                    // r33:sync
                    topbar.shown.push(g.id().to_string()); // r33:sync
                } // r33:sync
            }
            changed = true;
        }
        if lresp.hovered() {
            crate::kit::hint::set_text(
                ui.ctx(),
                &t("topbar.customise_show"),
                "",
                crate::kit::hint::Caps::NONE,
            );
        }
        let dragging = drag.is_some_and(|(from, _)| from == i);
        let target = drag.is_some_and(|(from, to)| from != to && to == i);
        let below = drag.is_some_and(|(from, to)| from < to);
        with_brush(ui.painter(), |b| {
            if lresp.hovered() || dragging {
                b.rrect(
                    b.snap_rect(r),
                    s.r(3.0),
                    Fill::Solid(crate::kit::color::with_alpha(s.t.text, 0.08)),
                );
            }
            if target {
                b.hline(
                    r.left(),
                    r.right(),
                    // Where the row lands: under the target when it moves down (it goes
                    // after it), over it when it moves up.
                    b.snap_mid(if below { r.bottom() } else { r.top() }),
                    2.0 * b.px(),
                    s.t.focus,
                );
            }
            let gr = Rect::from_center_size(grip.center(), vec2(12.0 * k, 12.0 * k));
            glyph::paint(b, gr, Glyph::Grip, s.t.text_dim);
            led::paint(
                b,
                lamp.center(),
                9.0 * k,
                shown,
                s,
                Tok::Led,
                Tok::Panel,
                1.0,
            );
            let nr = Rect::from_min_max(Pos2::new(lamp.right() + 4.0 * k, r.top()), r.max);
            let ink = if shown { s.t.text } else { s.t.text_dim };
            let _ = text::paint(
                b,
                nr,
                &Text::key(g.name_key(), Role::Body, ink)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
        });
    }
    changed
}
