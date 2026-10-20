//! The rest of the desk's pieces: FL's send row, Extra large's chain line, output chip and
//! own send rows (round 25: the insert slots left the desk for the inspector), the master
//! strip, a group's separator and the `+` after the last strip.

use super::layout::{self, Row, WidthMode};
use super::menu::Menu;
use super::strip::{
    fader_input, fx_rec, knob_input, knob_style, meter_hint, meter_rects, peak_readout, pid, sense,
    wave, write_db, write_pan, Part, Pass, Snap,
};
use super::{MixerView, Renaming};
use crate::app::CantusApp;
use crate::kit::color::{on_fill, safe_track, shade, with_alpha, FillSize};
use crate::kit::fader::{self as kfader, FaderLook};
use crate::kit::glyph::{self, Glyph};
use crate::kit::hint::{self, Caps};
use crate::kit::knob::{self, KnobLook, KnobSize};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{light, Brush, Fill, State, Tok};
use crate::widgets::fader::{pos_of, UNITY_POS};
use cantus_core::{Command, GroupId, MergeKey, Send, StripId};
use cantus_engine::CaptureTarget;
use egui::{pos2, vec2, Align2, Rect, Sense, Ui};
use std::fmt::Write as _;

/// FL's send row. On the selected strip: where it goes (▼ and the destination's number; a
/// click lists every destination). On every other strip, the answer to "can the selected
/// strip go here?": a knob where it already sends (drag: the level; dimmed while the send
/// is off), a lit ▲ where its output already goes (a click sends it back to the master), a
/// hollow ▲ where it would close a loop, and a white ▲ otherwise — a click sends it here,
/// Ctrl+click routes it here only. Right-click: that route's menu.
#[allow(clippy::too_many_arguments)]
pub(super) fn sends_row(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    sn: &Snap,
    x: f32,
    bw: f32,
) {
    let Some(sy) = pass.mid(Row::Sends) else {
        return;
    };
    let Some(sel) = app.selected_strip else {
        return;
    };
    let s = pass.s;
    let t = &s.t;
    let cx = x + bw * 0.5;
    let ctx = ui.ctx().clone();
    if sel == sn.id {
        // Where the selected strip goes.
        let r = Rect::from_center_size(pos2(cx, sy), vec2((bw - 4.0).min(40.0), 16.0));
        let resp = sense(
            ui,
            pass,
            r,
            pid(Some(sn.id), Part::Send),
            Sense::click_and_drag(),
        );
        if let Some(resp) = &resp {
            // A drag from the chip makes a cable, as from the port (round 25).
            super::drag::source(v, app, ui, resp, super::drag::Source::Port(sn.id));
        }
        let hovered = resp.as_ref().is_some_and(|r| r.hovered());
        let open = v.popups.is_open(Menu::Route(sn.id));
        let armed = super::drag::armed(v) == Some(sn.id);
        if armed {
            // Click mode: a route armed from here waits for the click on its destination.
            b.rrect(r, s.r(3.0), Fill::Solid(with_alpha(t.selection, 0.30)));
            b.rrect_stroke(r.shrink(b.px() * 0.5), s.r(3.0), b.px() * 1.5, t.selection);
        } else if hovered || open {
            b.rrect(r, s.r(3.0), Fill::Solid(with_alpha(t.selection, 0.18)));
        }
        let g = Rect::from_center_size(pos2(r.left() + 6.0, sy), vec2(9.0, 9.0));
        glyph::paint(b, g, Glyph::Down, t.selection);
        v.buf.clear();
        match sn.output.and_then(|o| app.project().strip_number(o)) {
            Some(n) => {
                let _ = write!(v.buf, "{n}");
            }
            None => v.buf.push('M'),
        }
        let nr = Rect::from_min_max(pos2(g.right() + 1.0, r.top()), pos2(r.right(), r.bottom()));
        let _ = text::paint(
            b,
            nr,
            &Text::new(&v.buf, Role::Micro, t.selection)
                .caps(false)
                .align(Align2::CENTER_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
        if let Some(resp) = resp {
            if hovered {
                hint::set_text(
                    &ctx,
                    &v.words.route,
                    &v.buf,
                    Caps {
                        menu: true,
                        ..Caps::NONE
                    },
                );
            }
            let resp = resp.on_hover_text(&v.words.route);
            // A click arms a route from here (click mode, `drag::source`); right-click
            // lists every destination.
            if resp.secondary_clicked() {
                v.popups
                    .open(Menu::Route(sn.id), pos2(r.left(), r.bottom()));
            }
        }
        return;
    }
    let (send, routes_here) = match app.project().strip(sel) {
        Some(from) => (from.send_to(sn.id).copied(), from.output == Some(sn.id)),
        None => return,
    };
    let loops = v.desk.would_cycle.get(sn.index).copied().unwrap_or(true);
    if let Some(send) = send {
        // A linked send is a knob: its level.
        let ks = knob_style(pass).scaled(if pass.mode() == WidthMode::Compact {
            0.72
        } else {
            0.8
        });
        let c = pos2(cx, sy);
        let mut pos = send.level.clamp(0.0, 1.0);
        let resp = sense(
            ui,
            pass,
            Rect::from_center_size(c, vec2(18.0, 18.0)),
            pid(Some(sn.id), Part::Send),
            Sense::click_and_drag(),
        );
        let mut st = resp.as_ref().map_or(State::REST, |r| State::of(r, false));
        st.disabled = !send.enabled;
        // The level is a parameter (review 2): the handle's drop, the Alt-drag depth,
        // the rings; its right-click menu offers automation and *Modulate…*.
        let dm = super::strip::DeskMod::new(
            app,
            cantus_core::AutoTarget::Send {
                strip: sel,
                to: sn.id,
            },
            pos,
            &ks,
        );
        if let Some(resp) = &resp {
            let depth = dm.input(app, ui, resp);
            if let Some(p) = knob_input(v, ui, pass, resp, pos, 1.0, !depth) {
                pos = p;
                app.set_strip_send(sel, Send { level: p, ..send });
            }
            if resp.drag_stopped() && !depth {
                app.end_gesture();
            }
            if resp.hovered() {
                hint::set(&ctx, &v.words.send_knob, Caps::PARAM, resp.dragged(), |b| {
                    write!(b, "{:.0} %", pos * 100.0)
                });
            }
            if resp.secondary_clicked() {
                let at = resp.interact_pointer_pos().unwrap_or(c);
                // An existing send: the one route menu (round 25), POST | PRE and all.
                let route = cantus_core::routing::Route::send(sel, sn.id);
                crate::views::route_menu::open(&ctx, super::route_owner(), route, at);
            }
        }
        knob::paint(
            b,
            c,
            &ks.function(crate::kit::Function::Space),
            st,
            &KnobLook {
                live: dm.live,
                rings: dm.rings(),
                ..KnobLook::new(KnobSize::Mini, pos, 1.0)
            },
        );
        dm.paint_drop(
            b,
            ui,
            resp.as_ref(),
            Rect::from_center_size(c, vec2(18.0, 18.0)),
            s,
        );
        return;
    }
    let g = Rect::from_center_size(pos2(cx, sy), vec2(12.0, 12.0));
    let resp = sense(
        ui,
        pass,
        g.expand(2.0),
        pid(Some(sn.id), Part::Send),
        Sense::click(),
    );
    let hovered = resp.as_ref().is_some_and(|r| r.hovered());
    if routes_here {
        glyph::paint(b, g, Glyph::Send, t.led);
    } else if loops {
        // Hollow: the outline of the arrow.
        let u = g.width() / 12.0;
        let c = g.center();
        let pts = [
            c + vec2(-4.0 * u, 2.5 * u),
            c + vec2(0.0, -2.5 * u),
            c + vec2(4.0 * u, 2.5 * u),
        ];
        let ink = with_alpha(t.text_dim, 0.5);
        for i in 0..3 {
            b.segment(pts[i], pts[(i + 1) % 3], b.px() * 1.2, ink, false);
        }
    } else {
        let ink = if hovered { t.selection } else { t.text };
        glyph::paint(b, g, Glyph::Send, ink);
    }
    let Some(resp) = resp else {
        return;
    };
    let tip = if routes_here {
        &v.words.send_lit
    } else if loops {
        &v.words.send_loop
    } else {
        &v.words.send_free
    };
    let said = v
        .arrow_said
        .as_ref()
        .filter(|(id, _)| *id == sn.id)
        .map(|(_, w)| w.as_str());
    if hovered {
        // A refused click says why (a loop, no room), else what a click does.
        hint::set_text(
            &ctx,
            tip,
            said.unwrap_or(""),
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    } else if said.is_some() {
        v.arrow_said = None;
    }
    let resp = resp.on_hover_text(tip);
    if resp.secondary_clicked() {
        let at = resp.interact_pointer_pos().unwrap_or(g.center());
        v.popups.open(Menu::Arrow(sn.id), at);
    } else if resp.clicked() {
        if routes_here {
            app.exec(Command::SetStripOutput {
                id: sel,
                output: None,
            });
        } else {
            // The desk's one gesture (round 25): the whole selection sends (Ctrl: routes
            // its output here, Alt: keys), one undo step; a refusal is said, not
            // swallowed.
            use crate::routing_actions::{refusal_text, Gesture, Outcome};
            let at = resp.interact_pointer_pos().unwrap_or(g.center());
            let owner = super::route_owner();
            match app.route_gesture(sel, sn.id, Gesture::from_modifiers(pass.mods)) {
                Outcome::Done => v.arrow_said = None,
                Outcome::Refused(r) => {
                    v.arrow_said = Some((sn.id, refusal_text(app.project(), sel, r)));
                }
                Outcome::ChooseKey(choices) => {
                    crate::views::route_menu::open_key_choice(&ctx, owner, sel, sn.id, choices, at);
                }
                Outcome::NothingToKey => {
                    crate::views::route_menu::open_nothing_to_key(&ctx, owner, sel, sn.id, at);
                }
            }
        }
    }
}

/// Show a strip's (or, `None`, the master's) chain where it lives: the strip chosen and
/// the inspector open (round 25: the inserts left the desk for it).
pub(super) fn open_chain(app: &mut CantusApp, strip: Option<StripId>) {
    match strip {
        Some(id) => {
            if app.selected_strip != Some(id) {
                app.select_strip(id);
            }
        }
        None => app.select_master(),
    }
    // Drawn inside the mixer's tab: the dock is checked out, so the request waits for
    // it (a toggle here would change a placeholder and be lost).
    app.show_pane(crate::views::workspace::Pane::Inspector);
}

/// The chain in one line (Extra large; round 25, the owner's answer 7): "EQ · Comp · +1"
/// in a well, dimmed while the chain is switched out. A click opens the inspector at the
/// strip, where the inserts live; an empty chain's "+ Effect" opens the effect picker.
/// Right-click: the menu of the insert under the pointer (or the quick list to add one).
#[allow(clippy::too_many_arguments)]
pub(super) fn chain_row(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    sn: &Snap,
    x: f32,
    bw: f32,
) {
    let Some((a, z)) = pass.row(Row::Chain) else {
        return;
    };
    let s = pass.s;
    let t = &s.t;
    let master = sn.id.is_master();
    let r = Rect::from_min_max(pos2(x + 3.0, a), pos2(x + bw - 3.0, z));
    let id = if master { None } else { Some(sn.id) };
    let resp = sense(ui, pass, r, pid(id, Part::Chain), Sense::click());
    let hovered = resp.as_ref().is_some_and(|r| r.hovered());
    b.register(r, "chain_line", CHAIN_TOKENS, &[]);
    let base = shade(t.well, if t.dark { -0.04 } else { -0.02 });
    light::sunken(
        b,
        b.snap_rect(r),
        s.r(2.0),
        if hovered { shade(base, 0.04) } else { base },
        1.0,
        t.depth,
    );
    let line = v.desk.chain(if master { None } else { Some(sn.index) });
    let empty = line.is_empty();
    // Inserts whose plug-in stopped working (round 29, review #17): the line in the danger
    // colour, and their tips with the chain's.
    let stopped = stopped_inserts(app, sn.id);
    let inner = r.shrink2(vec2(4.0, 0.0));
    if empty {
        // An icon, not a word forty times across the desk: the hint bar says it.
        let ink = with_alpha(t.text_dim, if hovered { 0.9 } else { 0.45 });
        let g = Rect::from_center_size(r.center(), vec2(9.0, 9.0));
        glyph::paint(b, g, Glyph::Plus, ink);
    } else {
        let ink = if !stopped.is_empty() {
            t.danger
        } else if sn.fx_on {
            t.text
        } else {
            with_alpha(t.text, 0.5)
        };
        let tt = Text::new(line, Role::Label, ink)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, inner, &tt, s.tk() * super::desk::CHAIN_TEXT);
    }
    let Some(resp) = resp else {
        return;
    };
    if hovered {
        let tip = if empty {
            &v.words.chain_empty
        } else {
            &v.words.chain
        };
        let body = chain_body(line, &stopped);
        hint::set_text(
            ui.ctx(),
            tip,
            &body,
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    if resp.clicked() {
        if empty {
            open_chain(app, id);
            app.open_picker(crate::picker::PickPurpose::AddEffect { strip: sn.id });
        } else {
            open_chain(app, id);
        }
    }
    if resp.secondary_clicked() {
        let at = resp.interact_pointer_pos().unwrap_or(r.left_bottom());
        let menu = match chain_effect_at(app, ui, s, sn.id, inner, at.x) {
            Some(fx) => Menu::Slot(sn.id, fx),
            None => Menu::AddEffect(sn.id),
        };
        v.popups.open(menu, at);
    }
}

/// The insert whose name in the chain line is under `x` (built on a right-click only:
/// it measures the names it shows).
fn chain_effect_at(
    app: &CantusApp,
    ui: &Ui,
    s: &crate::kit::Style,
    strip: StripId,
    inner: Rect,
    x: f32,
) -> Option<cantus_core::EffectId> {
    let effects = app.project().effects_of(strip)?;
    let mut left = inner.left();
    let sep = text::measure(
        ui.ctx(),
        &Text::new(super::desk::CHAIN_SEP, Role::Label, s.t.text),
        s.tk() * super::desk::CHAIN_TEXT,
    )
    .x;
    for e in effects.iter().take(2) {
        let name = super::desk::short_name(e);
        let w = text::measure(
            ui.ctx(),
            &Text::new(&name, Role::Label, s.t.text),
            s.tk() * super::desk::CHAIN_TEXT,
        )
        .x;
        if x < left + w + sep * 0.5 {
            return Some(e.id);
        }
        left += w + sep;
    }
    None
}

/// Where the strip's output goes (Wide and up): a menu-field chip "→ 12 Bus" ("→ M" for
/// the master); a click lists every destination. The master's own output is the device.
#[allow(clippy::too_many_arguments)]
pub(super) fn output_row(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    sn: &Snap,
    x: f32,
    bw: f32,
) {
    let Some((a, z)) = pass.row(Row::Output) else {
        return;
    };
    if sn.id.is_master() {
        return;
    }
    let s = pass.s;
    let r = Rect::from_min_max(pos2(x + 3.0, a), pos2(x + bw - 3.0, z));
    let resp = sense(ui, pass, r, pid(Some(sn.id), Part::Output), Sense::click());
    let open = v.popups.is_open(Menu::Route(sn.id));
    let st = resp.as_ref().map_or(State::REST, |r| State::of(r, open));
    v.buf.clear();
    v.buf.push_str("\u{2192} ");
    let p = app.project();
    match sn.output.and_then(|o| p.strip_index(o)) {
        Some(i) => {
            let _ = write!(v.buf, "{}", i + 1);
            if pass.mode() == WidthMode::Xl {
                let name = v.desk.name(app, i);
                if !name.is_empty() {
                    v.buf.push(' ');
                    v.buf.push_str(name);
                }
            }
        }
        None => v.buf.push('M'),
    }
    // A chip, not a menu field: its ▾ would leave no room for "→ 12" on a Wide strip.
    crate::kit::well::paint(b, r, s, st, None);
    let ink = if sn.output.is_some() {
        s.t.text
    } else {
        s.t.text_dim
    };
    let tt = Text::new(&v.buf, Role::Label, ink)
        .align(Align2::CENTER_CENTER)
        .policy(Policy::Elide(Elide::End));
    let _ = text::paint(b, r.shrink2(vec2(3.0, 0.0)), &tt, s.tk() * 0.92);
    let Some(resp) = resp else {
        return;
    };
    if st.hovered {
        hint::set_text(
            ui.ctx(),
            &v.words.output,
            &v.buf,
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    if resp.clicked() || resp.secondary_clicked() {
        v.popups
            .open(Menu::Route(sn.id), pos2(r.left(), r.bottom()));
    }
}

/// The strip's own sends (Extra large; design §4.5): one row each — its switch, the
/// destination's number and name, the `POST | PRE` switch (answer 4) and the level as a
/// bar along the row's foot. Drag on a row: the level (the house rules; double-click is
/// 0 dB); a click on the name selects the destination; right-click: the route's menu. A
/// doubled signal (answer 5) carries the warning mark. The row after the last send is
/// "+ Send" (a click lists the destinations; a drag from it makes a cable), and a strip
/// with more sends than rows ends in "+n", which opens the inspector.
#[allow(clippy::too_many_arguments)]
pub(super) fn send_list(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    sn: &Snap,
    x: f32,
    bw: f32,
) {
    let Some((a, _)) = pass.row(Row::SendList) else {
        return;
    };
    if sn.id.is_master() {
        return;
    }
    let s = pass.s;
    let t = &s.t;
    let rows = pass.fit.send_rows;
    let count = app.project().strip(sn.id).map_or(0, |st| st.sends.len());
    // Every row but the last holds a send; the last holds a send only when it is the
    // strip's last, else "+ Send" or "+n".
    let shown = if count > rows { rows - 1 } else { count };
    for k in 0..rows {
        let y = a + 1.0 + k as f32 * layout::SEND_ROW;
        let r = Rect::from_min_max(
            pos2(x + 2.0, y),
            pos2(x + bw - 2.0, y + layout::SEND_ROW - 1.0),
        );
        if k < shown {
            let Some(send) = app
                .project()
                .strip(sn.id)
                .and_then(|st| st.sends.get(k))
                .copied()
            else {
                continue;
            };
            send_row(v, app, ui, b, pass, sn, k, send, r);
        } else if k == shown && count > rows {
            v.buf.clear();
            let _ = write!(v.buf, "+{}", count - shown);
            let resp = sense(ui, pass, r, pid(Some(sn.id), Part::SendAdd), Sense::click());
            let hovered = resp.as_ref().is_some_and(|r| r.hovered());
            let ink = if hovered { t.text } else { t.text_dim };
            let tt = Text::new(&v.buf, Role::Label, ink).align(Align2::CENTER_CENTER);
            let _ = text::paint(b, r, &tt, s.tk() * 0.9);
            if let Some(resp) = resp {
                if hovered {
                    hint::set_text(ui.ctx(), &v.words.send_more, &v.buf, Caps::NONE);
                }
                if resp.clicked() {
                    open_chain(app, Some(sn.id));
                }
            }
        } else if k == shown && count < cantus_core::MAX_SENDS {
            send_add(v, app, ui, b, pass, sn, r);
        }
    }
}

/// One send row of [`send_list`].
#[allow(clippy::too_many_arguments)]
fn send_row(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    sn: &Snap,
    k: usize,
    send: Send,
    r: Rect,
) {
    let s = pass.s;
    let t = &s.t;
    let ctx = ui.ctx().clone();
    let id = pid(Some(sn.id), Part::SendRow(k as u8));
    let resp = sense(ui, pass, r, id, Sense::click_and_drag());
    let hovered = resp.as_ref().is_some_and(|r| r.hovered());
    b.register(r, "send_row", SEND_TOKENS, &[Some(Tok::HueSpace)]);
    if hovered || resp.as_ref().is_some_and(|r| r.dragged()) {
        b.rrect(r, s.r(2.0), Fill::Solid(with_alpha(t.text, 0.06)));
    }
    // The switch.
    let lc = pos2(r.left() + 5.0, r.center().y - 0.5);
    let lamp_r = Rect::from_center_size(lc, vec2(10.0, r.height()));
    crate::kit::led::paint(b, lc, s.s(7.0), send.enabled, s, Tok::Led, Tok::Strip, 1.0);
    // The tap at the right: one chip sized to its word ("PRE" lit, "POST" dim; a click
    // switches it), so the word is never cut to fit two segments.
    let tap_k = s.tk() * 0.88;
    let tap_w = crate::kit::tap::chip_width(&ctx, s, send.tap, tap_k);
    let tap_r = Rect::from_min_max(
        pos2(r.right() - tap_w, r.top() + 1.0),
        pos2(r.right(), r.bottom() - 3.0),
    );
    let doubled = v.desk.send_doubled(sn.index, k);
    // The destination: its name, or its number when it has none (both are in the hint
    // bar; a 76-pt strip has room for one).
    let p = app.project();
    let to = p.strip_index(send.to);
    v.buf.clear();
    if let Some(i) = to {
        let name = v.desk.name(app, i);
        if name.is_empty() {
            let _ = write!(v.buf, "{}", i + 1);
        } else {
            v.buf.push_str(name);
        }
    }
    let mut nr = Rect::from_min_max(
        pos2(lc.x + 6.0, r.top()),
        pos2(tap_r.left() - 2.0, r.bottom() - 2.0),
    );
    if doubled {
        let dc = pos2(nr.right() - 4.0, nr.center().y);
        super::cables::doubled_mark_small(b, s, dc);
        nr.max.x -= 9.0;
    }
    let ink = if send.enabled {
        t.text
    } else {
        with_alpha(t.text, 0.5)
    };
    let tt = Text::new(&v.buf, Role::Label, ink)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let _ = text::paint(b, nr, &tt, s.tk() * 0.88);
    // The level, along the foot, in the space hue (sends are space).
    let foot = Rect::from_min_max(
        pos2(nr.left(), r.bottom() - 2.0),
        pos2(r.right(), r.bottom()),
    );
    let mut level = send.level.clamp(0.0, 1.0);
    // The level is a parameter (review 2): a modulator's handle dropped on the row
    // connects it, an Alt-drag (or a mapping modulator) edits the depth; its right-click
    // menu offers automation and *Modulate…*.
    let dm = super::strip::DeskMod::new(
        app,
        cantus_core::AutoTarget::Send {
            strip: sn.id,
            to: send.to,
        },
        level,
        s,
    );
    if let Some(resp) = &resp {
        let depth = dm.input(app, ui, resp);
        if let Some(p) = knob_input(v, ui, pass, resp, level, 1.0, !depth) {
            level = p;
            app.set_strip_send(sn.id, Send { level: p, ..send });
        }
        if resp.drag_stopped() && !depth {
            app.end_gesture();
        }
    }
    dm.paint_drop(b, ui, resp.as_ref(), r, s);
    let fb = b.snap_rect(foot);
    b.rect(fb, Fill::Solid(with_alpha(t.text_dim, 0.18)));
    if level > 0.0 {
        let lit = Rect::from_min_max(fb.min, pos2(fb.left() + fb.width() * level, fb.bottom()));
        let c = if send.enabled {
            t.hue_space
        } else {
            with_alpha(t.hue_space, 0.45)
        };
        b.rect(b.snap_rect(lit), Fill::Solid(c));
    }
    // Where modulation has the level now: a tick on the foot, in the modulation hue.
    if let Some(live) = dm.live {
        let x = fb.left() + fb.width() * live.clamp(0.0, 1.0);
        b.rect(
            Rect::from_min_max(pos2(x - 1.0, fb.top() - 2.0), pos2(x + 1.0, fb.bottom())),
            Fill::Solid(t.hue_mod),
        );
    }
    if let Some(tap) =
        crate::kit::tap::chip(ui, b, s, tap_r, pass.clip, id.with("tap"), send.tap, tap_k)
    {
        app.set_send_tap(sn.id, send.to, tap);
    }
    let Some(resp) = resp else {
        return;
    };
    let on_lamp = resp
        .interact_pointer_pos()
        .is_some_and(|p| lamp_r.contains(p));
    if hovered || resp.dragged() {
        let name = v.buf.as_str();
        let tap = if send.tap.is_post() {
            &v.words.tap[0]
        } else {
            &v.words.tap[1]
        };
        hint::set(&ctx, &v.words.send_row, Caps::PARAM, resp.dragged(), |w| {
            write!(w, "{name} \u{b7} ")?;
            write_level(w, level);
            write!(w, " \u{b7} {tap}")
        });
    }
    if resp.double_clicked() {
        // knob_input took it: 0 dB.
    } else if resp.clicked() {
        if on_lamp {
            app.set_strip_send(
                sn.id,
                Send {
                    enabled: !send.enabled,
                    ..send
                },
            );
            app.end_gesture();
        } else if to.is_some() {
            app.select_strip(send.to);
        }
    }
    if resp.secondary_clicked() {
        let at = resp.interact_pointer_pos().unwrap_or(r.left_bottom());
        let route = cantus_core::routing::Route::send(sn.id, send.to);
        crate::views::route_menu::open(&ctx, super::route_owner(), route, at);
    }
    if doubled && hovered {
        let p = app.project();
        if let Some(text) = crate::routing_actions::doubled_text(p, sn.id, send.to) {
            let _ = resp.on_hover_text(text);
        }
    }
}

/// "−6.0 dB" for a linear send level, written without allocating.
pub(super) fn write_level(w: &mut String, level: f32) {
    if level <= 0.0 {
        w.push_str("\u{2212}\u{221e} dB");
    } else {
        write_db(w, cantus_core::linear_to_db(level), true);
    }
}

/// The "+ Send" row: a click lists the destinations (with *New return ▸*), a drag from
/// it makes a cable (as from the strip's port).
#[allow(clippy::too_many_arguments)]
fn send_add(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    sn: &Snap,
    r: Rect,
) {
    let s = pass.s;
    let t = &s.t;
    let resp = sense(
        ui,
        pass,
        r,
        pid(Some(sn.id), Part::SendAdd),
        Sense::click_and_drag(),
    );
    let hovered = resp.as_ref().is_some_and(|r| r.hovered());
    let ink = with_alpha(t.text_dim, if hovered { 0.9 } else { 0.45 });
    b.rrect_stroke(
        b.snap_rect(r.shrink(1.0)),
        s.r(2.0),
        b.px(),
        with_alpha(t.text_dim, if hovered { 0.7 } else { 0.25 }),
    );
    let g = Rect::from_center_size(r.center(), vec2(8.0, 8.0));
    glyph::paint(b, g, Glyph::Plus, ink);
    let Some(resp) = resp else {
        return;
    };
    super::drag::source(v, app, ui, &resp, super::drag::Source::Port(sn.id));
    if hovered {
        hint::set_text(ui.ctx(), &v.words.send_add, "", Caps::NONE);
    }
    if resp.clicked() {
        let at = pos2(r.left(), r.bottom());
        crate::views::route_menu::open_add_send(ui.ctx(), super::route_owner(), sn.id, at);
    }
}

const CHAIN_TOKENS: &[Tok] = &[Tok::Well, Tok::Text, Tok::TextDim];
const SEND_TOKENS: &[Tok] = &[Tok::Led, Tok::Text, Tok::TextDim];

/// The master: a strip like the others, with a stereo meter, a balance (the pan law does
/// not apply to it, ADR 0012), no mute, solo, swap, EQ or sends — its sends row holds the
/// project's pan law instead, which is where somebody mixing looks for it.
#[allow(clippy::too_many_arguments)]
pub(super) fn master(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    x: f32,
    w: f32,
) {
    let s = pass.s;
    let t = &s.t;
    let bw = w - 1.0;
    let cx = x + bw * 0.5;
    let ctx = ui.ctx().clone();
    let mut m = app.project().master.clone();
    // An automated master shows what its clips give it (ADR 0027).
    for (t, val) in [
        (cantus_core::AutoTarget::MasterGain, &mut m.gain_db),
        (cantus_core::AutoTarget::MasterPan, &mut m.pan),
        (cantus_core::AutoTarget::MasterWidth, &mut m.width),
    ] {
        if let Some(Some(a)) = app.automated_value_where(|x| *x == t) {
            *val = a;
        }
    }
    let body = pass.body(x, bw);
    b.rect(b.snap_rect(body), Fill::Solid(t.strip));
    let target = super::drag::target(v, app, ui, None, body);
    super::cables::column_hatch(b, s, body, target);
    // The master is chosen like a strip (ADR 0026): a click on its number or name shows
    // its chain in the inspector, a right-click is its menu.
    let chosen = crate::views::inspector::subject(app) == crate::views::inspector::Subject::Master;
    let mut head = Rect::NOTHING;
    if let Some((a, z)) = pass.row(Row::Number) {
        let cell = b.snap_rect(Rect::from_min_max(pos2(x, a), pos2(x + bw, z)));
        head = head.union(cell);
        let fill = t.header_cell;
        b.rect(cell, Fill::V(shade(fill, 0.03), shade(fill, -0.03)));
        if chosen {
            b.rrect_stroke(cell.shrink(b.px() * 1.5), 0.0, b.px() * 1.5, t.selection);
        }
        let _ = text::paint(b, cell, &Text::new("M", Role::Label, on_fill(fill)), s.tk());
    }
    if let Some((a, z)) = pass.row(Row::Name) {
        let nr = Rect::from_min_max(pos2(x + 2.0, a), pos2(x + bw - 2.0, z - 2.0));
        head = head.union(nr);
        let ink = if chosen { t.text } else { t.label };
        let tt = Text::key("mixer.master", Role::Label, ink)
            .caps(true)
            .align(Align2::CENTER_BOTTOM)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, nr, &tt, s.tk());
    }
    if head.is_positive() {
        if let Some(resp) = sense(ui, pass, head, pid(None, Part::Head), Sense::click()) {
            if resp.hovered() {
                hint::set_text(
                    &ctx,
                    &v.words.master_head,
                    "",
                    Caps {
                        menu: true,
                        ..Caps::NONE
                    },
                );
            }
            let resp = resp.on_hover_text(&v.words.master_head);
            if resp.clicked() {
                app.select_master();
            }
            if resp.secondary_clicked() {
                let at = resp.interact_pointer_pos().unwrap_or(head.left_bottom());
                v.popups.open(Menu::Master, at);
            }
        }
    }
    if let Some((a, z)) = pass.row(Row::Meter) {
        let levels = v
            .meters
            .master(app.engine.meters().master, pass.now, pass.wave);
        let mut top = a;
        if pass.mode() != WidthMode::Compact && z - a >= 72.0 {
            let pr = Rect::from_min_max(pos2(x + 3.0, a), pos2(x + bw - 3.0, a + 15.0));
            let held = v.meters.master_held();
            peak_readout(v, ui, b, pass, pr, held, None, None);
            top = a + 18.0;
        }
        let (mr, _) = meter_rects(pass.mode(), x, bw, top, z, false);
        let mr = mr.expand2(vec2(4.0, 0.0));
        if pass.wave {
            wave(b, s, mr, Some(v.meters.master_wave()), None);
        } else {
            pass.meters.paint(b, mr, &levels);
        }
        if let Some(resp) = sense(ui, pass, mr, pid(None, Part::Meter), Sense::hover()) {
            meter_hint(v, ui, &resp, mr);
        }
    }
    // Balance.
    if let Some(py) = pass.mid(Row::Pan) {
        let c = pos2(cx, py);
        let mut pos = (m.pan.clamp(-1.0, 1.0) + 1.0) * 0.5;
        let resp = sense(
            ui,
            pass,
            Rect::from_center_size(c, vec2(22.0, 22.0)),
            pid(None, Part::Pan),
            Sense::click_and_drag(),
        );
        let st = resp.as_ref().map_or(State::REST, |r| State::of(r, false));
        let ks = knob_style(pass);
        let dm = super::strip::DeskMod::new(app, cantus_core::AutoTarget::MasterPan, pos, &ks);
        if let Some(resp) = &resp {
            let depth = dm.input(app, ui, resp);
            if let Some(p) = knob_input(v, ui, pass, resp, pos, 0.5, !depth) {
                pos = p;
                app.exec_merging(
                    Command::SetMasterPan { pan: p * 2.0 - 1.0 },
                    MergeKey::of(&"master-pan"),
                );
            }
            if resp.drag_stopped() && !depth {
                app.end_gesture();
            }
            if st.hovered {
                hint::set(&ctx, &v.words.balance, Caps::PARAM, resp.dragged(), |b| {
                    write_pan(b, pos * 2.0 - 1.0);
                    Ok(())
                });
            }
        }
        knob::paint(
            b,
            c,
            &ks,
            st,
            &KnobLook {
                live: dm.live,
                rings: dm.rings(),
                ..KnobLook::new(KnobSize::Mini, pos, 0.5)
            },
        );
        dm.paint_drop(
            b,
            ui,
            resp.as_ref(),
            Rect::from_center_size(c, vec2(22.0, 22.0)),
            s,
        );
    }
    // The fader, and its value.
    if let Some((a, z)) = pass.row(Row::Fader) {
        let half = if pass.mode() == WidthMode::Compact {
            11.0
        } else {
            14.0
        };
        let fr = Rect::from_min_max(pos2(cx - half, a), pos2(cx + half, z));
        let mut gain = m.gain_db;
        let resp = sense(
            ui,
            pass,
            fr,
            pid(None, Part::Fader),
            Sense::click_and_drag(),
        );
        let st = resp.as_ref().map_or(State::REST, |r| State::of(r, false));
        let dm =
            super::strip::DeskMod::new(app, cantus_core::AutoTarget::MasterGain, pos_of(gain), s);
        if let Some(resp) = &resp {
            let depth = dm.input(app, ui, resp);
            if let Some(g) = fader_input(v, ui, pass, resp, fr, gain, !depth) {
                gain = g;
                app.exec_merging(
                    Command::SetMasterGain { gain_db: g },
                    MergeKey::of(&"master"),
                );
            }
            if resp.drag_stopped() && !depth {
                app.end_gesture();
            }
            if st.hovered {
                hint::set(&ctx, &v.words.master, Caps::PARAM, resp.dragged(), |b| {
                    write_db(b, gain, true);
                    Ok(())
                });
            }
        }
        kfader::paint(
            b,
            fr,
            s,
            st,
            &FaderLook {
                pos: pos_of(gain),
                ticks: super::paint::ticks(),
                unity: Some(UNITY_POS),
            },
        );
        dm.fader_mark(b, fr, s);
        dm.paint_drop(b, ui, resp.as_ref(), fr, s);
        if let Some((ra, rz)) = pass.row(Row::Readout) {
            let rr = Rect::from_min_max(pos2(x + 3.0, ra), pos2(x + bw - 3.0, rz));
            v.buf.clear();
            write_db(&mut v.buf, gain, false);
            crate::kit::well::readout(b, rr, s, State::REST, &v.buf, true);
        }
    }
    // Polarity.
    if let Some(py) = pass.mid(Row::PhaseSwap) {
        let r = Rect::from_center_size(pos2(cx, py), vec2(11.0, 11.0));
        let resp = sense(
            ui,
            pass,
            r.expand(1.0),
            pid(None, Part::Phase),
            Sense::click(),
        );
        let hovered = resp.as_ref().is_some_and(|r| r.hovered());
        let c = if m.phase_invert {
            t.highlight
        } else if hovered {
            t.text
        } else {
            with_alpha(t.text_dim, 0.62)
        };
        glyph::paint(b, r, Glyph::Phase, c);
        if let Some(resp) = resp {
            if hovered {
                hint::set_text(&ctx, &v.words.phase, "", Caps::NONE);
            }
            if resp.on_hover_text(&v.words.phase).clicked() {
                app.exec(Command::SetMasterPhaseInvert {
                    invert: !m.phase_invert,
                });
            }
        }
    }
    // Width.
    if let Some(wy) = pass.mid(Row::Width) {
        let c = pos2(cx, wy);
        let mut pos = (m.width / 2.0).clamp(0.0, 1.0);
        let resp = sense(
            ui,
            pass,
            Rect::from_center_size(c, vec2(22.0, 22.0)),
            pid(None, Part::Width),
            Sense::click_and_drag(),
        );
        let st = resp.as_ref().map_or(State::REST, |r| State::of(r, false));
        let ks = knob_style(pass);
        let dm = super::strip::DeskMod::new(app, cantus_core::AutoTarget::MasterWidth, pos, &ks);
        if let Some(resp) = &resp {
            let depth = dm.input(app, ui, resp);
            if let Some(p) = knob_input(v, ui, pass, resp, pos, 0.5, !depth) {
                pos = p;
                app.exec_merging(
                    Command::SetMasterWidth { width: p * 2.0 },
                    MergeKey::of(&"master-width"),
                );
            }
            if resp.drag_stopped() && !depth {
                app.end_gesture();
            }
            if st.hovered {
                hint::set(&ctx, &v.words.width, Caps::PARAM, resp.dragged(), |b| {
                    write!(b, "{:.0} %", pos * 200.0)
                });
            }
        }
        knob::paint(
            b,
            c,
            &ks.function(crate::kit::Function::Space),
            st,
            &KnobLook {
                live: dm.live,
                rings: dm.rings(),
                ..KnobLook::new(KnobSize::Mini, pos, 0.5)
            },
        );
        dm.paint_drop(
            b,
            ui,
            resp.as_ref(),
            Rect::from_center_size(c, vec2(22.0, 22.0)),
            s,
        );
    }
    let sn = Snap::master(&m);
    fx_rec(v, app, ui, b, pass, &sn, cx, CaptureTarget::Master);
    // Its chain in one line (Extra large), as a strip's (ADR 0026; round 25).
    chain_row(v, app, ui, b, pass, &sn, x, bw);
    // The pan law: in the output chip's row from Wide up (the master's output is the
    // device), else in FL's send row.
    if let Some(sy) = pass.mid(Row::Output).or(pass.mid(Row::Sends)) {
        let law = app.project().pan_law;
        let r = Rect::from_center_size(pos2(cx, sy), vec2(bw - 6.0, 16.0));
        let resp = sense(ui, pass, r, pid(None, Part::Send), Sense::click());
        let open = v.popups.is_open(Menu::PanLaw);
        let st = resp.as_ref().map_or(State::REST, |r| State::of(r, open));
        crate::kit::well::paint(b, r, s, st, None);
        // "−3 dB law" — never a bare number under OUTPUT; "−3 dB" where that is all that
        // fits (Compact's narrow master).
        for template in [&v.words.pan_law_chip, &v.words.pan_law_short] {
            v.buf.clear();
            let (before, after) = template.split_once("{law}").unwrap_or((template, ""));
            v.buf.push_str(before);
            for ch in law.id().chars() {
                v.buf.push(if ch == '-' { '\u{2212}' } else { ch });
            }
            v.buf.push_str(after);
            let tt = Text::new(&v.buf, Role::Micro, t.text).caps(false);
            if text::measure(&ctx, &tt, s.tk()).x <= r.width() - 6.0 {
                break;
            }
        }
        let tt = Text::new(&v.buf, Role::Micro, t.text)
            .caps(false)
            .align(Align2::CENTER_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, r.shrink2(vec2(3.0, 0.0)), &tt, s.tk());
        if let Some(resp) = resp {
            if resp.hovered() {
                hint::set_text(
                    &ctx,
                    &v.words.pan_law,
                    &v.buf,
                    Caps {
                        menu: true,
                        ..Caps::NONE
                    },
                );
            }
            if resp.on_hover_text(&v.words.pan_law).clicked() {
                v.popups.open(Menu::PanLaw, pos2(r.left(), r.bottom()));
            }
        }
    }
}

/// A group's separator: a column in the group's colour with its name running up it, a
/// chevron at the top and the member count at the foot. A click folds the group (its
/// strips leave the desk; the separator stays); a double-click renames it; right-click is
/// the group's menu.
#[allow(clippy::too_many_arguments)]
pub(super) fn separator(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    gid: GroupId,
    x: f32,
    w: f32,
) {
    let s = pass.s;
    let t = &s.t;
    let Some(group) = app.project().strip_group(gid) else {
        return;
    };
    let (collapsed, color) = (group.collapsed, group.color);
    let members = app.project().strips_in_group(gid).count();
    let bw = w - 1.0;
    let body = pass.body(x, bw);
    let resp = sense(
        ui,
        pass,
        body,
        egui::Id::new(("mixer-group", gid.0)),
        Sense::click(),
    );
    let hovered = resp.as_ref().is_some_and(|r| r.hovered());
    let c = safe_track(crate::theme::track_color(color), t.dark, FillSize::Large);
    let fill = crate::kit::color::mix(t.strip, c, if hovered { 0.55 } else { 0.4 });
    b.register(body, "mixer_group", SEPARATOR_TOKENS, &[]);
    b.rect(b.snap_rect(body), Fill::Solid(fill));
    b.rect(
        b.snap_rect(Rect::from_min_max(
            body.min,
            pos2(body.left() + 3.0, body.bottom()),
        )),
        Fill::Solid(safe_track(
            crate::theme::track_color(color),
            t.dark,
            FillSize::Small,
        )),
    );
    let ink = on_fill(fill);
    if let Some((a, z)) = pass.row(Row::Number) {
        let g =
            Rect::from_center_size(pos2(body.center().x + 1.0, (a + z) * 0.5), vec2(10.0, 10.0));
        glyph::paint(b, g, if collapsed { Glyph::Menu } else { Glyph::Down }, ink);
    }
    let renaming = matches!(v.rename, Some((Renaming::Group(g), _)) if g == gid);
    if renaming {
        if let Some((a, _)) = pass.row(Row::Number) {
            v.rename_at = Some(Rect::from_min_size(pos2(x, a), vec2(bw, 16.0)));
        }
    }
    let top = pass.row(Row::Name).or(pass.row(Row::Meter)).map(|r| r.0);
    let bottom = pass.row(Row::Fader).map(|r| r.1);
    if let (Some(a), Some(z), false) = (top, bottom, renaming) {
        if let Some(group) = app.project().strip_group(gid) {
            let nr = Rect::from_min_max(pos2(x + 3.0, a + 2.0), pos2(x + bw, z));
            let tt = Text::new(&group.name, Role::Label, ink)
                .align(Align2::CENTER_TOP)
                .policy(Policy::Rotate90(Elide::Middle));
            let _ = text::paint(b, nr, &tt, s.tk());
        }
    }
    if let Some(y) = pass.mid(Row::FxRec).or(pass.mid(Row::Sends)) {
        v.buf.clear();
        let _ = write!(v.buf, "{members}");
        let cr = Rect::from_center_size(pos2(body.center().x + 1.0, y), vec2(bw, 12.0));
        let _ = text::paint(
            b,
            cr,
            &Text::new(&v.buf, Role::Micro, with_alpha(ink, 0.8)).caps(false),
            s.tk(),
        );
    }
    let Some(resp) = resp else {
        return;
    };
    if hovered {
        if let Some(group) = app.project().strip_group(gid) {
            v.hint_buf.clear();
            let _ = write!(v.hint_buf, "{} · {members}", group.name);
            hint::set_text(
                ui.ctx(),
                &v.hint_buf,
                &v.words.group,
                Caps {
                    menu: true,
                    ..Caps::NONE
                },
            );
        }
    }
    let resp = resp.on_hover_text(&v.words.group);
    // The separator drags the whole group, and is a drop target (round 25).
    super::drag::source(v, app, ui, &resp, super::drag::Source::Group(gid));
    let target = super::drag::target_group(v, app, ui, gid, body);
    super::cables::column_hatch(b, s, body, target);
    if resp.double_clicked() {
        v.begin_group_rename(app, gid);
    } else if resp.clicked() {
        app.exec(Command::SetStripGroupCollapsed {
            id: gid,
            collapsed: !collapsed,
        });
    }
    if resp.secondary_clicked() {
        let at = resp.interact_pointer_pos().unwrap_or(body.center());
        v.popups.open(Menu::Group(gid), at);
    }
}

const SEPARATOR_TOKENS: &[Tok] = &[Tok::Strip];
const ADD_TOKENS: &[Tok] = &[Tok::Text, Tok::TextDim];

/// The `+` after the last strip of the middle (FL 2025's add-a-track slot).
pub(super) fn add_button(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &Pass,
    rect: Rect,
) {
    let s = pass.s;
    let t = &s.t;
    let resp = sense(
        ui,
        pass,
        rect,
        egui::Id::new("mixer-add-strip"),
        Sense::click(),
    );
    let hovered = resp.as_ref().is_some_and(|r| r.hovered());
    let r = b.snap_rect(rect.shrink2(vec2(1.0, 0.0)));
    b.register(r, "add_strip", ADD_TOKENS, &[]);
    if hovered {
        b.rrect(r, s.r(3.0), Fill::Solid(with_alpha(t.text, 0.06)));
    }
    b.rrect_stroke(
        r.shrink(b.px() * 0.5),
        s.r(3.0),
        b.px(),
        with_alpha(t.text_dim, if hovered { 0.8 } else { 0.35 }),
    );
    let y = pass.mid(Row::Number).unwrap_or(r.top() + 8.0);
    glyph::paint(
        b,
        Rect::from_center_size(pos2(r.center().x, y), vec2(12.0, 12.0)),
        Glyph::Plus,
        if hovered { t.text } else { t.text_dim },
    );
    if let Some(resp) = resp {
        if hovered {
            hint::set_text(ui.ctx(), &v.words.add_strip, "", Caps::NONE);
        }
        if resp.on_hover_text(&v.words.add_strip).clicked() {
            app.add_strip();
        }
    }
}

/// The polarity switch as a control of its own (the inspector's strip uses it): Ø, lit
/// in the highlight colour while the polarity is inverted.
pub fn phase_button(ui: &mut Ui, inverted: bool) -> egui::Response {
    let s = crate::kit::Style::current();
    let (rect, resp) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
    let st = State::of(&resp, inverted);
    let c = if inverted {
        s.t.highlight
    } else if st.hovered {
        s.t.text
    } else {
        s.t.text_dim
    };
    crate::kit::with_brush(ui.painter(), |b| {
        if st.hovered {
            b.rrect(rect, s.r(3.0), Fill::Solid(with_alpha(s.t.text, 0.06)));
        }
        glyph::paint(b, rect.shrink(4.0), Glyph::Phase, c);
    });
    let tip = crate::i18n::t("track.phase_tip");
    if st.hovered {
        hint::set_text(ui.ctx(), &tip, "", Caps::NONE);
    }
    resp.on_hover_text(tip)
}

/// A strip's inserts whose plug-in stopped working, one line each: its name and why, in
/// the interface's words (round 29, review #17, #33).
pub fn stopped_inserts(app: &CantusApp, strip: cantus_core::StripId) -> Vec<String> {
    app.project()
        .effects_of(strip)
        .map(|fx| {
            fx.iter()
                .filter(|e| app.plugins.crashed(e.id).is_some())
                .map(|e| {
                    let name = super::desk::short_name(e);
                    let why = app.plugins.crashed(e.id).unwrap_or_default();
                    crate::i18n::tf(
                        "isolation.reason",
                        &[
                            ("name", &name),
                            ("reason", &crate::views::plugin_notice::stopped_words(why)),
                        ],
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// What a chain line's hint says: the chain, then each stopped insert and how to bring it
/// back.
pub fn chain_body(line: &str, stopped: &[String]) -> String {
    if stopped.is_empty() {
        return line.to_string();
    }
    let mut lines = vec![line.to_string()];
    lines.extend(stopped.iter().cloned());
    lines.push(crate::i18n::t("isolation.tip.reload_menu"));
    lines.join("\n")
}

/// The hint of `strip`'s chain line: what a pointer over it reads (review isolation-4 #42
/// asked for it to be seen).
pub fn chain_hint(app: &CantusApp, strip: cantus_core::StripId, line: &str) -> String {
    chain_body(line, &stopped_inserts(app, strip))
}
