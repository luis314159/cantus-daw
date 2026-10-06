//! The rest of the desk's pieces: FL's send row, the insert slots of Extra large, the
//! master strip, a group's separator and the `+` after the last strip.

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
use cantus_core::{Command, GroupId, MergeKey, Send};
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
        let resp = sense(ui, pass, r, pid(Some(sn.id), Part::Send), Sense::click());
        let hovered = resp.as_ref().is_some_and(|r| r.hovered());
        let open = v.popups.is_open(Menu::Route(sn.id));
        if hovered || open {
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
            if resp.clicked() || resp.secondary_clicked() {
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
        if let Some(resp) = &resp {
            if let Some(p) = knob_input(v, ui, pass, resp, pos, 1.0) {
                pos = p;
                app.set_strip_send(sel, Send { level: p, ..send });
            }
            if resp.drag_stopped() {
                app.end_gesture();
            }
            if resp.hovered() {
                hint::set(&ctx, &v.words.send_knob, Caps::PARAM, resp.dragged(), |b| {
                    write!(b, "{:.0} %", pos * 100.0)
                });
            }
            if resp.secondary_clicked() {
                let at = resp.interact_pointer_pos().unwrap_or(c);
                v.popups.open(Menu::Arrow(sn.id), at);
            }
        }
        knob::paint(
            b,
            c,
            &ks.function(crate::kit::Function::Space),
            st,
            &KnobLook::new(KnobSize::Mini, pos, 1.0),
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
    if hovered {
        hint::set_text(
            &ctx,
            tip,
            "",
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
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
        } else if !loops {
            if pass.mods.command {
                app.exec(Command::SetStripOutput {
                    id: sel,
                    output: Some(sn.id),
                });
            } else {
                app.set_strip_send(sel, Send::new(sn.id));
                app.end_gesture();
            }
        }
    }
}

/// The insert slots (Extra large): a loaded slot is a raised tile — its lamp (a click
/// bypasses it), its name (a click opens its editor) and a drag moves it to another slot;
/// an empty slot is a darker well (a click lists the effects to put in it). Right-click:
/// the slot's menu. The master's slots are the same code (ADR 0026).
#[allow(clippy::too_many_arguments)]
pub(super) fn inserts(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    sn: &Snap,
    x: f32,
    bw: f32,
) {
    let Some((a, _)) = pass.row(Row::Inserts) else {
        return;
    };
    let s = pass.s;
    let t = &s.t;
    let ctx = ui.ctx().clone();
    let loaded = app.project().effects_of(sn.id).map_or(0, |e| e.len());
    for k in 0..pass.fit.slots {
        let y = a + 2.0 + k as f32 * layout::SLOT;
        let r = Rect::from_min_max(pos2(x + 3.0, y), pos2(x + bw - 3.0, y + layout::SLOT - 2.0));
        let resp = sense(
            ui,
            pass,
            r,
            pid(Some(sn.id), Part::Slot(k as u8)),
            if k < loaded {
                Sense::click_and_drag()
            } else {
                Sense::click()
            },
        );
        let hovered = resp.as_ref().is_some_and(|r| r.hovered());
        // Through the resolver, so the master's slots are the same code (ADR 0026).
        let slot = app.project().effects_of(sn.id).and_then(|e| e.get(k));
        b.register(r, "insert_slot", SLOT_TOKENS, &[]);
        let Some(slot) = slot else {
            let base = shade(t.well, if t.dark { -0.05 } else { -0.03 });
            light::sunken(
                b,
                r,
                s.r(2.0),
                if hovered { shade(base, 0.03) } else { base },
                1.0,
                t.depth,
            );
            if let Some(resp) = resp {
                if hovered {
                    hint::set_text(&ctx, &v.words.slot_empty, "", Caps::NONE);
                }
                let add = crate::picker::PickPurpose::AddEffect { strip: sn.id };
                crate::views::device_chrome::drop_target(app, ui, &resp, add);
                let resp = resp.on_hover_text(&v.words.slot_empty);
                // Round 20: a click opens the device picker on the effects; a right-click,
                // the quick list.
                if resp.clicked() {
                    app.open_picker(crate::picker::PickPurpose::AddEffect { strip: sn.id });
                } else if resp.secondary_clicked() {
                    v.popups
                        .open(Menu::AddEffect(sn.id), pos2(r.left(), r.bottom()));
                }
            }
            continue;
        };
        let (effect, bypass) = (slot.id, slot.bypass);
        let base = shade(t.panel_raised, if hovered { 0.03 } else { 0.0 });
        light::raised(
            b,
            r,
            s.r(2.0),
            base,
            light::RaisedOpts {
                drop: false,
                depth: t.depth,
                ..Default::default()
            },
        );
        let lc = pos2(r.left() + 7.0, r.center().y);
        crate::kit::led::paint(b, lc, s.s(7.0), !bypass, s, Tok::Led, Tok::PanelRaised, 1.0);
        let nr = Rect::from_min_max(pos2(lc.x + 6.0, r.top()), pos2(r.right() - 3.0, r.bottom()));
        let ink = if bypass {
            with_alpha(t.text, 0.5)
        } else {
            t.text
        };
        let tt = match slot.spec.external.as_deref() {
            Some(p) => Text::new(&p.name, Role::Label, ink),
            None => Text::key(slot.spec.kind.name_key(), Role::Label, ink),
        }
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, nr, &tt, s.tk());
        let Some(resp) = resp else {
            continue;
        };
        let swap = crate::picker::PickPurpose::ReplaceEffect {
            strip: sn.id,
            effect,
        };
        crate::views::device_chrome::drop_target(app, ui, &resp, swap);
        if hovered {
            hint::set_text(
                &ctx,
                &v.words.slot,
                "",
                Caps {
                    menu: true,
                    ..Caps::NONE
                },
            );
        }
        let on_lamp = resp
            .interact_pointer_pos()
            .is_some_and(|p| p.x < lc.x + 6.0);
        if resp.drag_started() {
            v.slot_drag = Some((sn.id, effect, k));
        }
        // Round 20: the swap badge at the slot's right end while the pointer is on it.
        let purpose = crate::picker::PickPurpose::ReplaceEffect {
            strip: sn.id,
            effect,
        };
        let swapped = crate::views::device_chrome::swap_badge(
            app,
            ui,
            b,
            r,
            ui.id().with(("slot-swap", effect.0)),
            1.0,
            purpose,
            "swap.tip.effect",
        );
        let resp = resp.on_hover_text(&v.words.slot);
        if swapped {
            // The badge took the click: the picker is open.
        } else if resp.secondary_clicked() {
            let at = resp.interact_pointer_pos().unwrap_or(r.left_bottom());
            v.popups.open(Menu::Slot(sn.id, effect), at);
        } else if resp.clicked() {
            if on_lamp {
                app.exec(Command::SetEffectBypass {
                    strip: sn.id,
                    effect,
                    bypass: !bypass,
                });
            } else {
                app.open_effect_editor(sn.id, effect);
            }
        }
    }
    slot_drop(v, app, ui, b, pass, sn, x, bw, a, loaded);
}

/// A slot being dragged on this strip: a line in the selection colour where it would
/// land, and the move when the button is let go.
#[allow(clippy::too_many_arguments)]
fn slot_drop(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    sn: &Snap,
    x: f32,
    bw: f32,
    top: f32,
    loaded: usize,
) {
    let Some((strip, effect, from)) = v.slot_drag else {
        return;
    };
    if strip != sn.id || loaded == 0 {
        return;
    }
    let Some(p) = ui.ctx().pointer_interact_pos() else {
        return;
    };
    let row = ((p.y - top - 2.0) / layout::SLOT).floor().max(0.0) as usize;
    let to = row.min(loaded - 1);
    if to != from {
        // Above the slot it lands on when moving up, below it when moving down.
        let k = if to > from { to + 1 } else { to };
        let y = top + 1.0 + k as f32 * layout::SLOT;
        let r = Rect::from_min_max(pos2(x + 3.0, y - 1.0), pos2(x + bw - 3.0, y + 1.0));
        b.rect(r, Fill::Solid(pass.s.t.selection));
    }
    if !ui.input(|i| i.pointer.any_down()) {
        v.slot_drag = None;
        if to != from {
            app.exec(Command::MoveEffect {
                strip,
                effect,
                index: to,
            });
        }
    }
}

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
        if let Some(resp) = &resp {
            if let Some(p) = knob_input(v, ui, pass, resp, pos, 0.5) {
                pos = p;
                app.exec_merging(
                    Command::SetMasterPan { pan: p * 2.0 - 1.0 },
                    MergeKey::of(&"master-pan"),
                );
            }
            if resp.drag_stopped() {
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
            &knob_style(pass),
            st,
            &KnobLook::new(KnobSize::Mini, pos, 0.5),
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
        if let Some(resp) = &resp {
            if let Some(g) = fader_input(v, ui, pass, resp, fr, gain) {
                gain = g;
                app.exec_merging(
                    Command::SetMasterGain { gain_db: g },
                    MergeKey::of(&"master"),
                );
            }
            if resp.drag_stopped() {
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
        if let Some(resp) = &resp {
            if let Some(p) = knob_input(v, ui, pass, resp, pos, 0.5) {
                pos = p;
                app.exec_merging(
                    Command::SetMasterWidth { width: p * 2.0 },
                    MergeKey::of(&"master-width"),
                );
            }
            if resp.drag_stopped() {
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
            &knob_style(pass).function(crate::kit::Function::Space),
            st,
            &KnobLook::new(KnobSize::Mini, pos, 0.5),
        );
    }
    let sn = Snap::master(&m);
    fx_rec(v, app, ui, b, pass, &sn, cx, CaptureTarget::Master);
    // Its insert slots, the same code as a strip's (ADR 0026).
    inserts(v, app, ui, b, pass, &sn, x, bw);
    // The pan law.
    if let Some(sy) = pass.mid(Row::Sends) {
        let law = app.project().pan_law;
        let r = Rect::from_center_size(pos2(cx, sy), vec2(bw - 6.0, 16.0));
        let resp = sense(ui, pass, r, pid(None, Part::Send), Sense::click());
        let open = v.popups.is_open(Menu::PanLaw);
        let st = resp.as_ref().map_or(State::REST, |r| State::of(r, open));
        crate::kit::well::paint(b, r, s, st, None);
        let tt = Text::new(law.id(), Role::Micro, t.text)
            .caps(false)
            .align(Align2::CENTER_CENTER);
        let _ = text::paint(b, r, &tt, s.tk());
        if let Some(resp) = resp {
            if resp.hovered() {
                hint::set_text(
                    &ctx,
                    &v.words.pan_law,
                    law.id(),
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
const SLOT_TOKENS: &[Tok] = &[Tok::Well, Tok::PanelRaised, Tok::Text];
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
