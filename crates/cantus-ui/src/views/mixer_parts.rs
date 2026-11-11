//! The rest of the desk's pieces: FL's send row, Extra large's chain line, output chip and
//! own send rows (round 25: the insert slots left the desk for the inspector), the master
//! strip, a group's separator and the `+` after the last strip.

use super::layout::{self, Row, WidthMode};
use super::look::{self, W};
use super::menu::Menu;
use super::rows::Col;
use super::strip::{knob_input, pid, sense, write_db, Part, Pass, Snap};
use super::{MixerView, Renaming};
use crate::app::CantusApp;
use crate::kit::color::{mix, on_fill, safe_track, with_alpha, FillSize};
use crate::kit::glyph::{self, Glyph};
use crate::kit::hint::{self, Caps};
use crate::kit::knob::{self, KnobLook, KnobSize};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{Brush, Fill, State, Tok};
use cantus_core::{Command, GroupId, Send, StripId};
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
        // r37:finish — a 24-pt target.
        let r = Rect::from_center_size(
            pos2(cx, sy),
            vec2((bw - 4.0).clamp(layout::TARGET, 40.0), layout::TARGET),
        );
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
        let g = Rect::from_center_size(pos2(r.left() + 7.0, sy), vec2(12.0, 12.0)); // r37:finish
        v.buf.clear();
        match sn.output.and_then(|o| app.project().strip_number(o)) {
            Some(n) => {
                let _ = write!(v.buf, "{n}");
            }
            None => v.buf.push('M'),
        }
        let mut nr =
            Rect::from_min_max(pos2(g.right() + 1.0, r.top()), pos2(r.right(), r.bottom()));
        // The arrow where the number still fits beside it; else the number alone, whole
        // (round 39, ADR 0109: "10" in a compact strip was cut to an ellipsis).
        let nw = text::measure(&ctx, &Text::new(&v.buf, Role::Label, t.selection), s.tk()).x;
        if nw <= nr.width() {
            glyph::paint(b, g, Glyph::Down, t.selection);
        } else {
            nr = r;
        }
        let _ = text::paint(
            b,
            nr,
            &Text::new(&v.buf, Role::Label, t.selection) // r37:finish (was Micro)
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
        let ks = pass.s.scaled(if pass.mode() == WidthMode::Compact {
            0.72
        } else {
            0.8
        });
        let c = pos2(cx, sy);
        let mut pos = send.level.clamp(0.0, 1.0);
        let resp = sense(
            ui,
            pass,
            Rect::from_center_size(c, vec2(layout::TARGET, layout::TARGET)), // r37:finish
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
    let g = Rect::from_center_size(pos2(cx, sy), vec2(16.0, 16.0)); // r37:finish (12 read small)
    let resp = sense(
        ui,
        pass,
        Rect::from_center_size(g.center(), vec2(layout::TARGET, layout::TARGET)), // r37:finish
        pid(Some(sn.id), Part::Send),
        Sense::click(),
    );
    let hovered = resp.as_ref().is_some_and(|r| r.hovered());
    if routes_here {
        glyph::paint(b, g, Glyph::Send, t.led);
    } else if loops {
        // Hollow: the outline of the arrow (the kit's glyph, round 37). // r37:finish
        glyph::paint(b, g, Glyph::SendOpen, with_alpha(t.text_dim, 0.5)); // r37:finish
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

/// "−6.0 dB" for a linear send level, written without allocating.
pub(super) fn write_level(w: &mut String, level: f32) {
    if level <= 0.0 {
        w.push_str("\u{2212}\u{221e} dB");
    } else {
        write_db(w, cantus_core::linear_to_db(level), true);
    }
}

/// The master (des-mixer): a strip like the others at the desk's left — "M" and
/// "Master" in its head, its inserts, its polarity and width, the balance, mono, the fader
/// with its scale's numbers and its two channels, its level, "→ Out 1–2" (Large: the
/// short-term loudness) and its socket. The pan law is in its menu.
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
    let p = pass.pal;
    let bw = w;
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
    // The master is chosen like a strip (ADR 0026): a click on its head shows its chain
    // in the inspector, a right-click is its menu.
    let chosen = crate::views::inspector::subject(app) == crate::views::inspector::Subject::Master;
    look::body(b, body, &p, chosen.then(|| mix(p.surface, p.ink1, 0.05)));
    look::stripe(b, body, p.ink3);
    let target = super::drag::target(v, app, ui, None, body);
    super::cables::column_hatch(b, s, body, target);
    let sn = Snap::master(&m);
    let c = Col {
        x,
        w: bw,
        sn,
        master: true,
        selected: chosen,
        chosen,
        hue: None,
        dim: false,
        frozen: false,
        vca: false,
        empty: false,
    };
    if let Some((a, z)) = pass.row(Row::Number) {
        let head = Rect::from_min_max(pos2(x, a), pos2(x + bw, z));
        let cx = x + bw * 0.5;
        let name = &v.words.master_short;
        let weight = W::Medium;
        if pass.mode() == WidthMode::Compact {
            look::say(
                b,
                s,
                "M",
                cx,
                a + 14.0,
                Align2::CENTER_BOTTOM,
                bw,
                10.0,
                W::SemiBold,
                p.ink3,
            );
            let room = (z - a) - 26.0;
            let (t, k) = look::words(name, 12.0, weight, p.ink1, s.tk());
            let t = t
                .align(Align2::CENTER_BOTTOM)
                .policy(Policy::Rotate90(Elide::End));
            let nr = Rect::from_min_max(pos2(cx - 8.0, z - 4.0 - room), pos2(cx + 8.0, z - 4.0));
            let _ = text::paint(b, nr, &t, k);
        } else {
            let large = pass.mode() == WidthMode::Xl;
            look::say(
                b,
                s,
                "M",
                cx,
                a + 15.0,
                Align2::CENTER_BOTTOM,
                bw,
                10.0,
                W::SemiBold,
                p.ink3,
            );
            let (size, base) = if large { (13.0, 31.0) } else { (12.0, 30.0) };
            look::say(
                b,
                s,
                name,
                cx,
                a + base,
                Align2::CENTER_BOTTOM,
                bw - 6.0,
                size,
                weight,
                p.ink1,
            );
            if large {
                look::say(
                    b,
                    s,
                    &v.words.out_main,
                    cx,
                    a + 42.0,
                    Align2::CENTER_BOTTOM,
                    bw - 6.0,
                    10.0,
                    W::Regular,
                    p.ink3,
                );
            }
        }
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
    super::rows::inserts(v, app, ui, b, pass, &c);
    super::rows::eq(v, app, ui, b, pass, &c);
    super::rows::polarity(v, app, ui, b, pass, &c);
    super::rows::pan(v, app, ui, b, pass, &c);
    super::rows::msr(v, app, ui, b, pass, &c);
    super::rows::fader(v, app, ui, b, pass, &c);
    super::rows::level(v, ui, b, pass, &c);
    super::rows::output(v, app, ui, b, pass, &c);
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
        Fill::Solid(crate::kit::color::track_mark(
            crate::theme::track_color(color),
            t,
        )),
    );
    let ink = on_fill(fill);
    // des-mixer's head: the chevron stands where a strip's number does (Compact's number
    // sits on the head's first 14 points; Wide and Large centre it in a shorter head).
    let head = pass.row(Row::Number);
    if let Some((a, z)) = head {
        let cy = if z - a > 48.0 { a + 9.0 } else { (a + z) * 0.5 };
        let g = Rect::from_center_size(pos2(body.center().x + 1.0, cy), vec2(10.0, 10.0));
        glyph::paint(b, g, if collapsed { Glyph::Menu } else { Glyph::Down }, ink);
    }
    let renaming = matches!(v.rename, Some((Renaming::Group(g), _)) if g == gid);
    if renaming {
        if let Some((a, _)) = pass.row(Row::Number) {
            v.rename_at = Some(Rect::from_min_size(pos2(x, a), vec2(bw, 16.0)));
        }
    }
    // The name runs up the column from below the chevron to the fader's foot, the way a
    // Compact strip's name runs up its head (des-mixer's desk has no name or meter row).
    let top = head.map(|(a, z)| if z - a > 48.0 { a + 18.0 } else { z });
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
    // The member count at the foot, in the strips' output row (or the last row there is).
    let foot = [
        Row::Output,
        Row::SendList,
        Row::Readout,
        Row::Sends,
        Row::FxRec,
    ]
    .into_iter()
    .find_map(|r| pass.mid(r));
    if let Some(y) = foot {
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
    // r37:finish — a "+" key at the top of the column, the column itself drawn only under
    // the pointer (the judge: an empty full-height column read as dead space).
    if hovered {
        b.rrect(r, s.r(3.0), Fill::Solid(with_alpha(t.text, 0.06)));
        b.rrect_stroke(
            r.shrink(b.px() * 0.5),
            s.r(3.0),
            b.px(),
            with_alpha(t.text_dim, 0.6),
        );
    }
    let y = pass
        .mid(Row::Number)
        .unwrap_or(r.top() + 12.0)
        .max(r.top() + 12.0);
    let side = r.width().clamp(18.0, 24.0);
    // r39:el-mixer — des-mixer: the header's `+` adds a strip; here a faint `+` at rest,
    // the key only under the pointer.
    if !hovered {
        look::plus(b, pos2(r.center().x, y), look::Pal::of(s).ink4);
    } else {
        crate::kit::cap::row_key(
            b,
            Rect::from_center_size(pos2(r.center().x, y), vec2(side, side)),
            s,
            State {
                hovered,
                ..State::REST
            },
            crate::kit::cap::KeyFace::Glyph(Glyph::Plus),
        );
    }
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
