//! The strip's rows as des-mixer draws them (round 39; `docs/round39/design/des-mixer/`,
//! approved by the owner), in signal order: **the sound** (the inserts, the EQ curve,
//! polarity · swap · width), **the mix** (pan, M · S · arm, the fader with its meter on one
//! scale, the level), **the route** (the sends, the output). The head and the socket are
//! `mixer_strip`'s and `mixer_cables`'; every control here senses as before (the same part
//! ids, the same commands), only its drawing and its place are des-mixer's.

use super::desk::Insert;
use super::layout::{Row, WidthMode};
use super::look::{self, law, Bars, Pal, W};
use super::menu::Menu;
use super::strip::{knob_input, pid, sense, touch, DeskMod, Part, Pass, Snap};
use super::MixerView;
use crate::app::CantusApp;
use crate::kit::color::{mix, with_alpha};
use crate::kit::glyph_mixer::{Ink, MGlyph};
use crate::kit::hint::{self, Caps};
use crate::kit::knob::{self, KnobLook, KnobSize};
use crate::kit::{Brush, Fill, State};
use cantus_core::{Command, MergeKey, StripId};
use egui::{pos2, vec2, Align2, Color32, Rect, Sense, Ui};
use std::fmt::Write as _;

/// One column of the desk as its rows see it.
#[derive(Copy, Clone, Debug)]
pub(super) struct Col {
    /// The body's left edge and width.
    pub x: f32,
    pub w: f32,
    pub sn: Snap,
    pub master: bool,
    pub selected: bool,
    pub chosen: bool,
    /// The user's colour, as chosen (the stripe and the selected tint take it).
    pub hue: Option<Color32>,
    /// Muted, or silenced by another strip's solo: name, stripe and meter quiet.
    pub dim: bool,
    pub frozen: bool,
    pub vca: bool,
    /// Nothing routed into it, nothing in it: its number, a groove, its socket.
    pub empty: bool,
}

impl Col {
    pub fn id(&self) -> Option<StripId> {
        (!self.master).then_some(self.sn.id)
    }
    pub fn cx(&self) -> f32 {
        self.x + self.w * 0.5
    }
    fn mode(&self, pass: &Pass) -> WidthMode {
        pass.mode()
    }
}

/// The inserts (Chain row): Wide and Large a list in a quiet panel — a dot (hollow when
/// bypassed) and the kind (Large: the unit's name), the first empty row a faint `+`, "+2"
/// when more than fit; folded, one summary line; Compact, pips. A click opens the strip's
/// chain in the inspector (on the `+`: the effect picker); right-click, the insert's menu.
pub(super) fn inserts(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    c: &Col,
) {
    let Some((a, z)) = pass.row(Row::Chain) else {
        return;
    };
    let s = pass.s;
    let p = &pass.pal;
    let mode = c.mode(pass);
    let list: &[Insert] = if c.master {
        &v.desk.master_inserts
    } else {
        v.desk.inserts.get(c.sn.index).map_or(&[], Vec::as_slice)
    };
    let n = list.len();
    let x = c.x;
    let r = Rect::from_min_max(pos2(x + 3.0, a), pos2(x + c.w - 3.0, z));
    let resp = sense(ui, pass, r, pid(c.id(), Part::Chain), Sense::click());
    let hovered = resp.as_ref().is_some_and(|r| r.hovered());
    let quiet = c.frozen;
    // Where the pointer's row is, for the click and the menu.
    let (rows, rh) = if mode == WidthMode::Xl {
        (5, 18.0)
    } else {
        (4, 16.0)
    };
    let row_at = |py: f32| -> usize { ((py - (a + 2.0)) / rh).floor().max(0.0) as usize };
    if mode == WidthMode::Compact {
        let pips = n.min(5);
        let x0 = c.cx() - (pips as f32 * 5.0 - 2.0) * 0.5 + 1.5;
        for (k, ins) in list.iter().take(5).enumerate() {
            let pc = pos2(x0 + k as f32 * 5.0, a + 5.0);
            if ins.on {
                b.circle(pc, 1.5, Fill::Solid(if quiet { p.ink4 } else { p.ink3 }));
            } else {
                b.circle_stroke(pc, 1.25, 0.75, p.ink4);
            }
        }
    } else if pass.fit.folded {
        v.buf.clear();
        for ins in list.iter().filter(|i| i.on) {
            if !v.buf.is_empty() {
                v.buf.push_str(" \u{b7} ");
            }
            v.buf.push_str(&ins.kind);
        }
        let ink = if quiet { p.ink4 } else { p.ink3 };
        let buf = std::mem::take(&mut v.buf);
        look::say(
            b,
            s,
            &buf,
            c.cx(),
            a + 11.0,
            Align2::CENTER_BOTTOM,
            c.w - 8.0,
            10.0,
            W::Regular,
            ink,
        );
        v.buf = buf;
    } else if c.empty && !hovered {
        // An empty strip: nothing at rest.
    } else {
        look::panel(
            b,
            Rect::from_min_size(r.min, vec2(r.width(), rows as f32 * rh + 4.0)),
            p,
        );
        if hovered {
            b.rrect(b.snap_rect(r), 3.0, Fill::Solid(with_alpha(p.ink1, 0.03)));
        }
        let large = mode == WidthMode::Xl;
        let size = if large { 11.0 } else { 10.0 };
        let mut key_hover: Option<usize> = None;
        for k in 0..rows {
            let cy = a + 2.0 + k as f32 * rh + rh * 0.5;
            if let Some(ins) = list.get(k) {
                let dc = pos2(r.left() + 7.0, cy);
                if ins.on {
                    b.circle(dc, 2.0, Fill::Solid(if quiet { p.ink3 } else { p.ink2 }));
                } else {
                    b.circle_stroke(dc, 2.0, b.px(), p.ink4);
                }
                let label = if large { &ins.unit } else { &ins.kind };
                let ink = if !ins.on || quiet {
                    p.ink4
                } else if large {
                    p.ink1
                } else {
                    p.ink2
                };
                // Large names the key's source beside the key (the owner's choice 4).
                let src_name = ins
                    .key_src
                    .filter(|_| large)
                    .map_or("", |i| v.desk.name(app, i));
                let src_w = if src_name.is_empty() {
                    0.0
                } else {
                    look::width_of(ui.ctx(), s, src_name, 10.0, W::Regular).min(40.0) + 4.0
                };
                let room = r.width() - 16.0 - if ins.keyed { 12.0 + src_w } else { 0.0 };
                look::say(
                    b,
                    s,
                    label,
                    r.left() + 13.0,
                    cy + size * 0.36,
                    Align2::LEFT_BOTTOM,
                    room,
                    size,
                    W::Regular,
                    ink,
                );
                if ins.keyed {
                    // The key: "← Kick" on hover, and the source strip lit.
                    let kc = pos2(r.right() - 9.0 - src_w, cy);
                    let kr = Rect::from_center_size(kc, vec2(14.0, 14.0));
                    let kresp = sense(
                        ui,
                        pass,
                        kr,
                        pid(c.id(), Part::Chain).with(("key", k)),
                        Sense::click(),
                    );
                    let khot = kresp.is_some_and(|r| r.hovered());
                    if khot {
                        key_hover = ins.key_src;
                    }
                    look::glyph(
                        b,
                        s,
                        kc,
                        12.0,
                        MGlyph::Key,
                        Ink::Solid(if khot { p.ink1 } else { p.ink2 }),
                        p.panel,
                    );
                    if !src_name.is_empty() {
                        look::say(
                            b,
                            s,
                            src_name,
                            r.right() - 4.0,
                            cy + 3.6,
                            Align2::RIGHT_BOTTOM,
                            src_w,
                            10.0,
                            W::Regular,
                            p.ink3,
                        );
                    }
                }
            } else if k == n && !quiet {
                look::plus(
                    b,
                    pos2(r.left() + 7.5, cy),
                    if hovered { p.ink3 } else { p.ink4 },
                );
            }
        }
        if let Some(src) = key_hover {
            let id = app.project().strips.get(src).map(|st| st.id);
            v.key_lit = id;
            let mut line = String::new();
            let name = v.desk.name(app, src);
            match v.words.key_from.split_once("{name}") {
                Some((x, y)) => {
                    line.push_str(x);
                    line.push_str(name);
                    line.push_str(y);
                }
                None => line.push_str(name),
            }
            hint::set_text(ui.ctx(), &line, "", Caps::NONE);
        }
        if n > rows {
            v.buf.clear();
            let _ = write!(v.buf, "+{}", n - rows);
            let buf = std::mem::take(&mut v.buf);
            look::say(
                b,
                s,
                &buf,
                r.right() - 4.0,
                a + rows as f32 * rh + 1.0,
                Align2::RIGHT_BOTTOM,
                24.0,
                9.0,
                W::SemiBold,
                p.ink3,
            );
            v.buf = buf;
        }
    }
    let Some(resp) = resp else {
        return;
    };
    if hovered && v.key_lit.is_none() {
        let line = v.desk.chain(if c.master { None } else { Some(c.sn.index) });
        let tip = if n == 0 {
            &v.words.chain_empty
        } else {
            &v.words.chain
        };
        let stopped = super::parts::stopped_inserts(app, c.sn.id);
        let body = super::parts::chain_body(line, &stopped);
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
    let at_row = resp.interact_pointer_pos().map(|p| row_at(p.y));
    if resp.clicked() {
        super::parts::open_chain(app, c.id());
        let on_add =
            n == 0 || (mode != WidthMode::Compact && !pass.fit.folded && at_row == Some(n));
        if on_add && !c.master {
            app.open_picker(crate::picker::PickPurpose::AddEffect { strip: c.sn.id });
        }
    }
    if resp.secondary_clicked() && !c.master {
        let at = resp.interact_pointer_pos().unwrap_or(r.left_bottom());
        let fx = at_row
            .filter(|_| mode != WidthMode::Compact && !pass.fit.folded)
            .and_then(|k| {
                app.project()
                    .effects_of(c.sn.id)
                    .and_then(|e| e.get(k))
                    .map(|e| e.id)
            });
        let menu = match fx {
            Some(fx) => Menu::Slot(c.sn.id, fx),
            None => Menu::AddEffect(c.sn.id),
        };
        v.popups.open(menu, at);
    }
}

/// The EQ (Wide 22, Large 36): the strip's three bands as a curve in its panel, the area
/// under it lightly in the accent; flat, one line in ink 4; Large, the bands' dots. A
/// click opens the strip in the inspector, where the EQ is edited.
pub(super) fn eq(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    c: &Col,
) {
    let Some((a, z)) = pass.row(Row::Eq) else {
        return;
    };
    let r = Rect::from_min_max(pos2(c.x + 3.0, a), pos2(c.x + c.w - 3.0, z));
    let mut pts = std::mem::take(&mut v.pts);
    let thumb = if c.master {
        v.desk.master_eq
    } else {
        v.desk.eq.get(c.sn.index).copied().unwrap_or_default()
    };
    look::eq_curve(
        b,
        r,
        &thumb.db,
        thumb.shaped,
        (pass.mode() == WidthMode::Xl).then_some(&thumb.dots),
        c.frozen,
        &pass.pal,
        &mut pts,
    );
    v.pts = pts;
    let Some(resp) = sense(ui, pass, r, pid(c.id(), Part::Eq(0)), Sense::click()) else {
        return;
    };
    if resp.hovered() {
        let (lo, mi, hi) = (c.sn.eq[0], c.sn.eq[1], c.sn.eq[2]);
        hint::set(ui.ctx(), &v.words.eq[1], Caps::NONE, false, |w| {
            write!(w, "{lo:+.1} \u{b7} {mi:+.1} \u{b7} {hi:+.1} dB")
        });
    }
    if resp.clicked() {
        super::parts::open_chain(app, c.id());
    }
}

/// One segment of a pill of keys (the owner's option C, 2026-11-09): its part, what it
/// shows, whether it is on and in what colour.
#[derive(Copy, Clone)]
struct Seg {
    part: Part,
    face: Face,
    on: Option<Color32>,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Face {
    Glyph(MGlyph),
    Letter(&'static str),
}

/// A pill of keys (option C): one recessed well split into `segs`, each target its share of
/// the pill and the pill's height (24 and up); a lit segment filled in its state's colour.
/// Returns each segment's response.
#[allow(clippy::too_many_arguments)]
fn pill<const N: usize>(
    ui: &mut Ui,
    b: &mut Brush,
    pass: &Pass,
    c: &Col,
    r: Rect,
    segs: [Seg; N],
    ghost_first: bool,
    size: f32,
) -> [Option<egui::Response>; N] {
    let s = pass.s;
    let p = pass.pal;
    look::well(b, r, &p, false);
    let n = N as f32;
    let horizontal = r.width() >= r.height() * 1.5 || N == 1;
    let cell = |i: usize| -> Rect {
        let i = i as f32;
        if horizontal {
            let w = r.width() / n;
            Rect::from_min_max(
                pos2(r.left() + i * w, r.top()),
                pos2(r.left() + (i + 1.0) * w, r.bottom()),
            )
        } else {
            let h = r.height() / n;
            Rect::from_min_max(
                pos2(r.left(), r.top() + i * h),
                pos2(r.right(), r.top() + (i + 1.0) * h),
            )
        }
    };
    let mut out: [Option<egui::Response>; N] = std::array::from_fn(|_| None);
    for (i, sg) in segs.iter().enumerate() {
        let cr = cell(i);
        let resp = sense(ui, pass, cr, pid(c.id(), sg.part), Sense::click());
        let hot = resp.as_ref().is_some_and(|r| r.hovered());
        if let Some(on) = sg.on {
            b.rrect(
                b.snap_rect(cr.shrink(1.5)),
                2.5,
                Fill::Solid(if hot { mix(on, p.ink1, 0.12) } else { on }),
            );
        } else if hot {
            b.rrect(b.snap_rect(cr.shrink(1.5)), 2.5, Fill::Solid(p.well_hi));
        }
        if i == 0 && ghost_first && sg.on.is_none() {
            let ring = b.snap_rect(cr.shrink(2.0));
            b.rrect_stroke(ring, 2.0, b.px(), with_alpha(p.solo, 0.45));
        }
        if i > 0 {
            let px = b.px();
            if horizontal {
                b.vline(
                    b.snap(cr.left()) + px * 0.5,
                    cr.top() + 4.0,
                    cr.bottom() - 4.0,
                    px,
                    p.edge,
                );
            } else {
                b.hline(
                    cr.left() + 4.0,
                    cr.right() - 4.0,
                    b.snap(cr.top()) + px * 0.5,
                    px,
                    p.edge,
                );
            }
        }
        let ink = match sg.on {
            Some(_) => p.on_ink,
            None if hot => p.ink1,
            None => p.ink3,
        };
        match sg.face {
            Face::Letter(l) => {
                look::say(
                    b,
                    s,
                    l,
                    cr.center().x,
                    cr.center().y + size * 0.36,
                    Align2::CENTER_BOTTOM,
                    cr.width(),
                    size,
                    W::SemiBold,
                    ink,
                );
            }
            Face::Glyph(g) => {
                let gi = match sg.on {
                    Some(_) => Ink::Solid(p.on_ink),
                    None if hot => Ink::Hover,
                    None => Ink::Rest,
                };
                let ground = sg.on.unwrap_or(p.well);
                look::glyph(
                    b,
                    s,
                    pos2(cr.center().x, cr.center().y - 0.5),
                    size + 6.0,
                    g,
                    gi,
                    ground,
                );
            }
        }
        out[i] = resp;
    }
    out
}

/// Polarity and swap in one pill (option C; Large adds mono, the master has polarity and
/// mono); the width knob moved to the pan row.
pub(super) fn polarity(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    c: &Col,
) {
    let Some((a, _)) = pass.row(Row::PhaseSwap) else {
        return;
    };
    let p = pass.pal;
    let large = pass.mode() == WidthMode::Xl;
    let sid = c.sn.id;
    let mono = c.sn.width <= 0.005;
    let tint = |on: bool| on.then_some(p.accent);
    let phase = Seg {
        part: Part::Phase,
        face: Face::Glyph(MGlyph::Phase),
        on: tint(c.sn.phase),
    };
    let swap = Seg {
        part: Part::Swap,
        face: Face::Glyph(MGlyph::Swap),
        on: tint(c.sn.swap),
    };
    let monos = Seg {
        part: Part::Mono,
        face: Face::Glyph(MGlyph::Mono),
        on: tint(mono),
    };
    let r = Rect::from_min_size(pos2(c.x + 3.0, a), vec2(c.w - 6.0, 24.0));
    let mut resps: [(Part, bool, Option<egui::Response>); 3] = [
        (Part::Phase, false, None),
        (Part::Swap, false, None),
        (Part::Mono, false, None),
    ];
    if c.master {
        let [x, y] = pill(ui, b, pass, c, r, [phase, monos], false, 10.0);
        resps[0] = (Part::Phase, c.sn.phase, x);
        resps[2] = (Part::Mono, mono, y);
    } else if large {
        let [x, y, z] = pill(ui, b, pass, c, r, [phase, swap, monos], false, 12.0);
        resps = [
            (Part::Phase, c.sn.phase, x),
            (Part::Swap, c.sn.swap, y),
            (Part::Mono, mono, z),
        ];
    } else {
        let [x, y] = pill(ui, b, pass, c, r, [phase, swap], false, 10.0);
        resps[0] = (Part::Phase, c.sn.phase, x);
        resps[1] = (Part::Swap, c.sn.swap, y);
    }
    for (part, on, resp) in resps {
        let Some(resp) = resp else { continue };
        let name = match part {
            Part::Phase => &v.words.phase,
            Part::Swap => &v.words.swap,
            _ => &v.words.mono,
        };
        resp.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::Checkbox,
                true,
                on,
                super::strip::state_tip(name, on),
            )
        });
        let resp = if resp.hovered() {
            let tip = super::strip::state_tip(name, on);
            hint::set_text(ui.ctx(), &tip, "", Caps::NONE);
            resp.on_hover_text(tip)
        } else {
            resp
        };
        if resp.clicked() {
            let cmd = match (part, c.master) {
                (Part::Phase, false) => Command::SetStripPhaseInvert {
                    id: sid,
                    invert: !on,
                },
                (Part::Phase, true) => Command::SetMasterPhaseInvert { invert: !on },
                (Part::Swap, _) => Command::SetStripSwapChannels { id: sid, swap: !on },
                (_, false) => Command::SetStripWidth {
                    id: sid,
                    width: if on { 1.0 } else { 0.0 },
                },
                (_, true) => Command::SetMasterWidth {
                    width: if on { 1.0 } else { 0.0 },
                },
            };
            app.exec(cmd);
        }
    }
}

/// What a desk knob sets.
#[derive(Copy, Clone, PartialEq, Eq)]
pub(super) enum KnobWhat {
    Pan,
    Width,
}

/// A knob of the strip or the master: its gestures (drag, Ctrl+wheel, arrows,
/// double-click), its modulation, its hint, drawn with the kit's knob at des-mixer's
/// radius.
#[allow(clippy::too_many_arguments)]
pub(super) fn knob_row(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    c: &Col,
    kc: egui::Pos2,
    r: f32,
    what: KnobWhat,
) {
    let s = pass.s;
    let sid = c.sn.id;
    let (part, target, mut pos, origin) = match (what, c.master) {
        (KnobWhat::Pan, false) => (
            Part::Pan,
            cantus_core::AutoTarget::StripPan(sid),
            (c.sn.pan.clamp(-1.0, 1.0) + 1.0) * 0.5,
            0.5,
        ),
        (KnobWhat::Pan, true) => (
            Part::Pan,
            cantus_core::AutoTarget::MasterPan,
            (c.sn.pan.clamp(-1.0, 1.0) + 1.0) * 0.5,
            0.5,
        ),
        (KnobWhat::Width, false) => (
            Part::Width,
            cantus_core::AutoTarget::StripWidth(sid),
            (c.sn.width / 2.0).clamp(0.0, 1.0),
            0.0,
        ),
        (KnobWhat::Width, true) => (
            Part::Width,
            cantus_core::AutoTarget::MasterWidth,
            (c.sn.width / 2.0).clamp(0.0, 1.0),
            0.0,
        ),
    };
    let hit = Rect::from_center_size(kc, vec2(2.0 * r + 8.0, 2.0 * r + 8.0).max(vec2(24.0, 24.0)));
    let resp = sense(ui, pass, hit, pid(c.id(), part), Sense::click_and_drag());
    let st = resp.as_ref().map_or(State::REST, |r| State::of(r, false));
    let ks = s.scaled(s.k * r / 10.0);
    let dm = DeskMod::new(app, target, pos, &ks);
    if let Some(resp) = &resp {
        if !c.master {
            touch(app, sid, resp);
        }
        let depth = dm.input(app, ui, resp);
        // Pan's centre; width's 100 % (the middle of 0…200 %).
        if let Some(np) = knob_input(v, ui, pass, resp, pos, 0.5, !depth) {
            pos = np;
            let cmd = match (what, c.master) {
                (KnobWhat::Pan, false) => (
                    Command::SetStripPan {
                        id: sid,
                        pan: np * 2.0 - 1.0,
                    },
                    MergeKey::of(&("spans", sid.0)),
                ),
                (KnobWhat::Pan, true) => (
                    Command::SetMasterPan {
                        pan: np * 2.0 - 1.0,
                    },
                    MergeKey::of(&"master-pan"),
                ),
                (KnobWhat::Width, false) => (
                    Command::SetStripWidth {
                        id: sid,
                        width: np * 2.0,
                    },
                    MergeKey::of(&("swidth", sid.0)),
                ),
                (KnobWhat::Width, true) => (
                    Command::SetMasterWidth { width: np * 2.0 },
                    MergeKey::of(&"master-width"),
                ),
            };
            app.exec_merging(cmd.0, cmd.1);
        }
        if resp.drag_stopped() && !depth {
            app.end_gesture();
        }
        if st.hovered {
            let ctx = ui.ctx().clone();
            match what {
                KnobWhat::Pan => {
                    let words = if c.master {
                        &v.words.balance
                    } else {
                        &v.words.pan
                    };
                    hint::set(&ctx, words, Caps::PARAM, resp.dragged(), |w| {
                        super::strip::write_pan(w, pos * 2.0 - 1.0);
                        Ok(())
                    })
                }
                KnobWhat::Width => {
                    hint::set(&ctx, &v.words.width, Caps::PARAM, resp.dragged(), |w| {
                        write!(w, "{:.0} %", pos * 200.0)
                    })
                }
            }
        }
    }
    knob::paint(
        b,
        kc,
        &ks,
        st,
        &KnobLook {
            live: dm.live,
            rings: dm.rings(),
            ..KnobLook::new(KnobSize::Mini, pos, origin)
        },
    );
    dm.paint_drop(b, ui, resp.as_ref(), hit, s);
}

/// Pan, and beside it the width knob (option C: width left the polarity row); Compact pan
/// alone; Large with the pan's value under it ("C", "16 R").
pub(super) fn pan(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    c: &Col,
) {
    let Some((a, z)) = pass.row(Row::Pan) else {
        return;
    };
    if c.vca {
        return;
    }
    let mode = pass.mode();
    let (r, wr) = match mode {
        WidthMode::Compact => (7.0, 0.0),
        WidthMode::Wide => (9.0, 7.0),
        WidthMode::Xl => (11.0, 9.0),
    };
    let cy = if mode == WidthMode::Xl {
        a + 15.0
    } else {
        (a + z) * 0.5
    };
    let (px, wx) = if mode == WidthMode::Compact {
        (c.cx(), 0.0)
    } else {
        (c.x + c.w * 0.32, c.x + c.w * 0.72)
    };
    knob_row(v, app, ui, b, pass, c, pos2(px, cy), r, KnobWhat::Pan);
    if mode != WidthMode::Compact {
        knob_row(v, app, ui, b, pass, c, pos2(wx, cy), wr, KnobWhat::Width);
    }
    if mode == WidthMode::Xl {
        let pan = c.sn.pan;
        v.buf.clear();
        if pan.abs() < 0.005 {
            v.buf.push('C');
        } else {
            let _ = write!(
                v.buf,
                "{:.0} {}",
                pan.abs() * 100.0,
                if pan < 0.0 { 'L' } else { 'R' }
            );
        }
        let buf = std::mem::take(&mut v.buf);
        look::say(
            b,
            pass.s,
            &buf,
            px,
            z - 1.0,
            Align2::CENTER_BOTTOM,
            c.w * 0.5,
            10.0,
            W::Regular,
            pass.pal.ink3,
        );
        v.buf = buf;
    }
}

/// Mute and solo as one pill (option C): Wide 26 × 24 a key, Large 43 × 24, Compact the two
/// stacked 26 × 23; on, the state's colour (mute ink, solo amber); a strip silenced by
/// another's solo keeps its M a well ringed faintly in the solo colour. Right-click or
/// Ctrl+click on M solos, as in FL. The arm is a dot in the head (`mixer_strip`).
pub(super) fn msr(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    c: &Col,
) {
    let Some((a, _)) = pass.row(Row::MuteSolo) else {
        return;
    };
    if c.master {
        return;
    }
    let p = pass.pal;
    let mode = pass.mode();
    let sid = c.sn.id;
    let silenced = v.desk.silenced.get(c.sn.index).copied().unwrap_or(false);
    let (r, size) = match mode {
        WidthMode::Compact => (
            Rect::from_min_size(pos2(c.x + (c.w - 26.0) * 0.5, a), vec2(26.0, 48.0)),
            9.0,
        ),
        WidthMode::Wide => (
            Rect::from_min_size(pos2(c.x + 3.0, a), vec2(c.w - 6.0, 24.0)),
            10.0,
        ),
        WidthMode::Xl => (
            Rect::from_min_size(pos2(c.x + 4.0, a), vec2(c.w - 8.0, 24.0)),
            11.0,
        ),
    };
    let m = Seg {
        part: Part::Mute,
        face: Face::Letter("M"),
        on: c.sn.muted.then_some(p.mute_on),
    };
    let so = Seg {
        part: Part::Solo,
        face: Face::Letter("S"),
        on: c.sn.soloed.then_some(p.solo),
    };
    let [mr, sr] = pill(ui, b, pass, c, r, [m, so], silenced && !c.sn.muted, size);
    let ctx = ui.ctx().clone();
    for (k, resp) in [(0, mr), (1, sr)] {
        let Some(resp) = resp else { continue };
        let target = if k == 0 {
            cantus_core::controller::MappingTarget::StripMute(sid)
        } else {
            cantus_core::controller::MappingTarget::StripSolo(sid)
        };
        crate::controllers::learn_spot_in(app, ui, resp.rect, pass.clip, target);
        let words = if k == 0 { &v.words.mute } else { &v.words.solo };
        if resp.hovered() {
            hint::set_text(&ctx, words, "", Caps::NONE);
        }
        let resp = resp.on_hover_text(words);
        if k == 0 && (resp.secondary_clicked() || (resp.clicked() && pass.mods.command)) {
            app.exec(Command::SetStripSolo {
                id: sid,
                soloed: !c.sn.soloed,
            });
        } else if k == 0 && resp.clicked() {
            app.exec(Command::SetStripMute {
                id: sid,
                muted: !c.sn.muted,
            });
        } else if k == 1 && resp.clicked() {
            app.exec(Command::SetStripSolo {
                id: sid,
                soloed: !c.sn.soloed,
            });
        }
    }
}

/// The arm (option C): a 24 × 24 dot in the strip's head beside its number — grey at rest,
/// the record red while armed — on every strip a track plays into; a click arms its tracks
/// (or disarms them when all are armed), one undo step.
pub(super) fn arm(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    c: &Col,
) {
    if c.master || c.vca || c.empty {
        return;
    }
    if !v.desk.armable.get(c.sn.index).copied().unwrap_or(false) {
        return;
    }
    let Some((na, _)) = pass.row(Row::Number) else {
        return;
    };
    let p = pass.pal;
    let sid = c.sn.id;
    let at = if pass.mode() == WidthMode::Compact {
        pos2(c.cx(), na + 27.0)
    } else {
        pos2(c.x + 12.0, na + 11.0)
    };
    let hit = Rect::from_center_size(at, vec2(24.0, 24.0));
    let resp = sense(ui, pass, hit, pid(c.id(), Part::Rec), Sense::click());
    let hot = resp.as_ref().is_some_and(|r| r.hovered());
    let armed = {
        let mut on = app.project().tracks_on_strip(sid).peekable();
        on.peek().is_some() && on.all(|t| t.armed)
    };
    crate::controllers::learn_spot_in(
        app,
        ui,
        hit,
        pass.clip,
        cantus_core::controller::MappingTarget::StripArm(sid),
    );
    if armed {
        b.circle(at, 6.0, Fill::Solid(with_alpha(p.rec, 0.25)));
        b.circle(at, 4.0, Fill::Solid(p.rec));
    } else {
        if hot {
            b.circle(at, 7.0, Fill::Solid(p.well_hi));
        }
        b.circle(at, 4.0, Fill::Solid(if hot { p.ink2 } else { p.ink3 }));
    }
    let Some(resp) = resp else {
        return;
    };
    let tip = crate::views::clips_chrome::word("rec.arm_strip_tip");
    if hot {
        hint::set_text(ui.ctx(), tip, "", Caps::NONE);
    }
    if resp.on_hover_text(tip).clicked() {
        let commands: Vec<Command> = app
            .project()
            .tracks_on_strip(sid)
            .filter(|t| t.armed == armed)
            .map(|t| Command::SetTrackArmed {
                id: t.id,
                armed: !armed,
            })
            .collect();
        if !commands.is_empty() {
            app.exec(Command::Batch {
                label_key: "cmd.set_track_armed".into(),
                commands,
            });
        }
    }
}

/// The meter's levels for a column this frame (`None`: stopped, the grooves alone).
pub(super) fn bars_of(levels: crate::kit::meter::Levels) -> Option<Bars> {
    let silent = levels
        .level
        .iter()
        .chain(levels.hold.iter())
        .all(|d| *d <= crate::kit::meter::MIN_DB);
    (!silent).then_some(Bars {
        level: levels.level,
        hold: levels.hold,
    })
}

/// The fader and, beside it, its meter on the same law (README: one tick reads both):
/// the groove, the ticks (Wide) or the scale's numbers (the master, Large), the cap at the
/// level (its centre line in the strip's colour when selected), the two bars with their
/// held peaks and the clip cell. Drag, a click in the groove, Ctrl+wheel, the arrows and
/// a double-click (0 dB) move it, as every fader.
pub(super) fn fader(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    c: &Col,
) {
    let Some((ta, tz)) = pass.fit.travel() else {
        return;
    };
    let (top, bot) = (pass.y0 + ta, pass.y0 + tz);
    let s = pass.s;
    let p = pass.pal;
    let mode = pass.mode();
    let g = look::geom(mode, c.master);
    let gx = c.x + g.groove_x;
    let sid = c.sn.id;
    if !c.vca {
        let levels = if c.master {
            v.meters
                .master(app.engine.meters().master, pass.now, pass.wave)
        } else {
            let peak = app
                .engine
                .meters()
                .strips
                .get(c.sn.index)
                .copied()
                .unwrap_or(0.0);
            v.meters.strip(c.sn.index, peak, pass.now, pass.wave)
        };
        let held = if c.master {
            v.meters.master_held()
        } else {
            v.meters.held(c.sn.index)
        };
        let mx = c.x + g.meter_x;
        let mr = Rect::from_min_max(
            pos2(mx, top - 8.0),
            pos2(mx + 2.0 * g.bar_w + 1.0, bot + 3.0),
        );
        if c.empty {
            // An empty strip: no meter drawn (des-mixer), its column still senses.
        } else if pass.wave {
            let w = if c.master {
                Some(v.meters.master_wave())
            } else {
                v.meters.wave(c.sn.index)
            };
            super::strip::wave(b, s, mr.with_min_y(top), w, None);
        } else {
            look::meter(
                b,
                mx,
                top,
                bot,
                g.bar_w,
                bars_of(levels),
                held >= 0.0,
                c.dim,
                &p,
            );
        }
        if let Some(resp) = sense(
            ui,
            pass,
            mr.expand2(vec2(2.0, 0.0)),
            pid(c.id(), Part::Meter),
            Sense::click(),
        ) {
            super::strip::meter_hint(v, ui, &resp, mr);
            if resp.clicked() {
                v.meters
                    .reset_hold(if c.master { None } else { Some(c.sn.index) });
                if !c.master {
                    super::strip::click_select(app, sid, pass.mods);
                }
            }
        }
    }
    if mode != WidthMode::Compact && (c.master || mode == WidthMode::Xl) {
        let (x, al) = if c.master {
            (gx - g.cap.x * 0.5 - 3.0, Align2::RIGHT_BOTTOM)
        } else {
            (
                c.x + (g.groove_x + g.cap.x * 0.5 + g.meter_x) * 0.5 + 1.0,
                Align2::CENTER_BOTTOM,
            )
        };
        look::scale_numbers(b, s, x, al, top, bot, &p);
    }
    look::groove(b, gx, top, bot, &p);
    if mode == WidthMode::Wide && !c.master && !c.empty {
        look::ticks(b, gx + g.cap.x * 0.5 + 3.0, top, bot, &p);
    }
    let mut gain = c.sn.gain;
    let fr = Rect::from_min_max(
        pos2(gx - g.cap.x * 0.5 - 3.0, top - super::layout::OVERHANG),
        pos2(gx + g.cap.x * 0.5 + 3.0, bot + super::layout::OVERHANG),
    );
    let resp = sense(
        ui,
        pass,
        fr,
        pid(c.id(), Part::Fader),
        Sense::click_and_drag(),
    );
    let st = resp
        .as_ref()
        .map_or(State::REST, |r| State::of(r, c.chosen))
        .lit(c.chosen);
    let target = if c.master {
        cantus_core::AutoTarget::MasterGain
    } else {
        cantus_core::AutoTarget::StripGain(sid)
    };
    let dm = DeskMod::new(app, target, 1.0 - law::frac(gain), s);
    if let Some(resp) = &resp {
        if !c.master {
            touch(app, sid, resp);
        }
        let depth = dm.input(app, ui, resp);
        if let Some(g2) = fader_input(v, ui, pass, resp, (top, bot), g.cap.y, gain, !depth) {
            gain = g2;
            if c.master {
                app.exec_merging(
                    Command::SetMasterGain { gain_db: g2 },
                    MergeKey::of(&"master"),
                );
            } else {
                app.set_strip_gain(sid, g2);
            }
        }
        if resp.drag_stopped() && !depth {
            app.end_gesture();
        }
        if st.hovered {
            let words = if c.master {
                &v.words.master
            } else {
                &v.words.volume
            };
            hint::set(ui.ctx(), words, Caps::PARAM, resp.dragged(), |w| {
                super::strip::write_db(w, gain, true);
                Ok(())
            });
        }
    }
    // A VCA follower: the dashed ghost cap with its V where the VCAs put the fader, on this
    // fader's law (des-controls' sheet; `kit::automate::paint_vca_ghost`, the kit fader's).
    if let Some(off) = (!c.master)
        .then(|| v.desk.vca_offset.get(c.sn.index).copied().flatten())
        .flatten()
    {
        let ty = b.snap(top + (bot - top) * law::frac(gain + off));
        let ghost = Rect::from_center_size(pos2(gx, ty), g.cap);
        crate::kit::automate::paint_vca_ghost(b, b.snap_rect(ghost), s, p.ink3);
    }
    let cy = top + (bot - top) * law::frac(gain);
    let centre = match (c.selected, c.hue) {
        (true, Some(h)) => h,
        _ => p.ink1,
    };
    if c.empty && !st.hovered && !st.active {
        look::cap_ghost(b, gx, cy, g.cap.x - 2.0, &p);
    } else {
        look::cap(b, gx, cy, g.cap, &p, centre, st.hovered || st.active);
    }
    if let Some(live) = dm.live {
        let ly = top + (bot - top) * (1.0 - live.clamp(0.0, 1.0));
        b.rect(
            Rect::from_min_size(pos2(gx - g.cap.x * 0.5 - 4.0, ly - 0.5), vec2(2.0, 1.0)),
            Fill::Solid(p.auto),
        );
    }
    dm.paint_drop(b, ui, resp.as_ref(), fr, s);
}

/// A fader's gestures on the desk's law: drag, a click in the groove off the cap jumps
/// there, Ctrl+wheel (1 dB a notch), the arrows (1 dB), a double-click to 0 dB.
#[allow(clippy::too_many_arguments)]
fn fader_input(
    v: &MixerView,
    ui: &Ui,
    pass: &Pass,
    resp: &egui::Response,
    (top, bot): (f32, f32),
    cap_h: f32,
    db: f32,
    drags: bool,
) -> Option<f32> {
    let fine = if pass.mods.shift { 0.2 } else { 1.0 };
    let travel = (bot - top).max(1.0);
    let (lo, hi) = (cantus_core::MIN_GAIN_DB, cantus_core::MAX_GAIN_DB);
    let start = db.clamp(lo, hi);
    let mut f = law::frac(start);
    let mut moved = false;
    if resp.dragged() {
        if drags && resp.drag_delta().y != 0.0 {
            f += resp.drag_delta().y * fine / travel;
            moved = true;
        }
    } else if resp.clicked() {
        if let Some(pp) = resp.interact_pointer_pos() {
            let cy = top + travel * f;
            if (pp.y - cy).abs() > cap_h * 0.5 + 1.0 {
                f = (pp.y - top) / travel;
                moved = true;
            }
        }
    }
    let mut value = if moved { law::db_at(f) } else { start };
    if resp.hovered() && pass.mods.command && v.ctrl_notches != 0.0 {
        value = (value + v.ctrl_notches * fine).clamp(lo, hi);
    }
    let steps = super::strip::arrows(ui, resp);
    if steps != 0 {
        value = (value + steps as f32 * fine).clamp(lo, hi);
    }
    if resp.double_clicked() {
        value = 0.0;
    }
    (value != start).then_some(value)
}

/// The level in numbers (Readout row): Regular 11 ink 2 (ink 1 selected), centred; once
/// the strip went over 0 dBFS, its peak in the clip colour on a pill until clicked. Large:
/// the level at the left, the held peak at the right. Nothing in silence (no "−∞").
pub(super) fn level(v: &mut MixerView, ui: &mut Ui, b: &mut Brush, pass: &mut Pass, c: &Col) {
    let Some((a, z)) = pass.row(Row::Readout) else {
        return;
    };
    let s = pass.s;
    let p = pass.pal;
    let held = if c.master {
        v.meters.master_held()
    } else {
        v.meters.held(c.sn.index)
    };
    let col = if c.selected { p.ink1 } else { p.ink2 };
    let r = Rect::from_min_max(pos2(c.x + 4.0, a), pos2(c.x + c.w - 4.0, z));
    let large = pass.mode() == WidthMode::Xl;
    let mut buf = std::mem::take(&mut v.buf);
    buf.clear();
    if large && !c.vca {
        look::write_db(&mut buf, c.sn.gain);
        look::say(
            b,
            s,
            &buf,
            c.x + 6.0,
            a + 13.0,
            Align2::LEFT_BOTTOM,
            c.w * 0.5,
            11.0,
            W::Regular,
            col,
        );
        if held > crate::kit::meter::MIN_DB {
            buf.clear();
            look::write_db(&mut buf, held);
            let ink = if held > 0.0 { p.clip } else { p.ink3 };
            look::say(
                b,
                s,
                &buf,
                c.x + c.w - 6.0,
                a + 13.0,
                Align2::RIGHT_BOTTOM,
                c.w * 0.45,
                11.0,
                W::Regular,
                ink,
            );
        }
    } else if held >= 0.0 && !c.vca {
        b.rrect(
            b.snap_rect(r),
            3.0,
            Fill::Solid(mix(p.surface, p.clip, 0.22)),
        );
        look::write_db(&mut buf, held);
        look::say(
            b,
            s,
            &buf,
            c.cx(),
            a + 12.0,
            Align2::CENTER_BOTTOM,
            c.w - 8.0,
            11.0,
            W::SemiBold,
            p.clip,
        );
    } else {
        look::write_db(&mut buf, c.sn.gain);
        look::say(
            b,
            s,
            &buf,
            c.cx(),
            a + 12.0,
            Align2::CENTER_BOTTOM,
            c.w - 6.0,
            11.0,
            W::Regular,
            col,
        );
    }
    v.buf = buf;
    if let Some(resp) = sense(ui, pass, r, pid(c.id(), Part::Peak), Sense::click()) {
        if resp.hovered() {
            v.hint_buf.clear();
            look::write_db(&mut v.hint_buf, held);
            hint::set_text(ui.ctx(), &v.words.peak, &v.hint_buf, Caps::NONE);
        }
        if resp.clicked() {
            v.meters
                .reset_hold(if c.master { None } else { Some(c.sn.index) });
        }
    }
}

/// The sends (SendList row). A strip with sends: one row each — the return's name and a
/// thin bar of its level (Large: the value in dB at the right, the bar across under the
/// name); off, both in ink 4; the first free row a faint `+`; more than fit, "+n". A
/// return that only receives says who feeds it ("5 sends in"; Large: "← Clap"); a VCA,
/// how many strips it holds. Drag on a row: its level (double-click 0 dB); a click
/// switches it; Ctrl+click selects the return; right-click is the route's menu.
pub(super) fn sends(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    c: &Col,
) {
    let Some((a, _)) = pass.row(Row::SendList) else {
        return;
    };
    if c.master {
        return;
    }
    if pass.mode() == WidthMode::Compact {
        compact_sends(v, app, ui, b, pass, c, a);
        return;
    }
    let s = pass.s;
    let p = pass.pal;
    let large = pass.mode() == WidthMode::Xl;
    let rows = pass.fit.send_rows.max(1);
    let rh = super::layout::send_row(pass.mode());
    let count = app.project().strip(c.sn.id).map_or(0, |st| st.sends.len());
    let mut buf = std::mem::take(&mut v.buf);
    if c.vca {
        let members = v.desk.vca_marks.get(c.sn.index).map_or(0, |m| m.members);
        buf.clear();
        fill_n(&mut buf, &v.words.strips_n, members as usize);
        look::say(
            b,
            s,
            &buf,
            c.cx(),
            a + 11.0,
            Align2::CENTER_BOTTOM,
            c.w - 6.0,
            10.0,
            W::Regular,
            p.ink3,
        );
        v.buf = buf;
        return;
    }
    let fed = v.desk.sends_in.get(c.sn.index).copied().unwrap_or(0);
    if count == 0 && fed > 0 {
        if large {
            let mut k = 0;
            let id = c.sn.id;
            for (i, from) in app.project().strips.iter().enumerate() {
                if !from.sends.iter().any(|x| x.to == id) {
                    continue;
                }
                if k == 2 && fed > 3 {
                    buf.clear();
                    let _ = write!(buf, "+{}", fed - 2);
                    look::say(
                        b,
                        s,
                        &buf,
                        c.x + c.w - 6.0,
                        a + 12.0 + 2.0 * 20.0,
                        Align2::RIGHT_BOTTOM,
                        30.0,
                        9.0,
                        W::SemiBold,
                        p.ink3,
                    );
                }
                if k >= 3 || (k == 2 && fed > 3) {
                    break;
                }
                buf.clear();
                buf.push_str("\u{2190} ");
                buf.push_str(v.desk.name(app, i));
                look::say(
                    b,
                    s,
                    &buf,
                    c.x + 6.0,
                    a + 12.0 + k as f32 * 20.0,
                    Align2::LEFT_BOTTOM,
                    c.w - 12.0,
                    11.0,
                    W::Regular,
                    p.ink3,
                );
                k += 1;
            }
        } else {
            buf.clear();
            fill_n(&mut buf, &v.words.sends_in, fed as usize);
            look::say(
                b,
                s,
                &buf,
                c.cx(),
                a + 11.0,
                Align2::CENTER_BOTTOM,
                c.w - 4.0,
                10.0,
                W::Regular,
                p.ink3,
            );
        }
        v.buf = buf;
        return;
    }
    v.buf = buf;
    let shown = if count > rows { rows - 1 } else { count };
    let (x0, w) = if large {
        (c.x + 6.0, c.w - 12.0)
    } else {
        (c.x + 5.0, c.w - 10.0)
    };
    for k in 0..rows {
        let ry = a + k as f32 * rh;
        let r = Rect::from_min_max(pos2(c.x + 2.0, ry), pos2(c.x + c.w - 2.0, ry + rh));
        if k < shown {
            let Some(send) = app
                .project()
                .strip(c.sn.id)
                .and_then(|st| st.sends.get(k))
                .copied()
            else {
                continue;
            };
            send_row(v, app, ui, b, pass, c, k, send, r, x0, w);
        } else if k == shown && count > rows {
            let mut buf = std::mem::take(&mut v.buf);
            buf.clear();
            let _ = write!(buf, "+{}", count - shown);
            let resp = sense(ui, pass, r, pid(c.id(), Part::SendAdd), Sense::click());
            let hot = resp.as_ref().is_some_and(|r| r.hovered());
            look::say(
                b,
                s,
                &buf,
                x0,
                ry + rh * 0.5 + 3.5,
                Align2::LEFT_BOTTOM,
                w,
                9.0,
                W::SemiBold,
                if hot { p.ink2 } else { p.ink3 },
            );
            if let Some(resp) = resp {
                if hot {
                    hint::set_text(ui.ctx(), &v.words.send_more, &buf, Caps::NONE);
                }
                if resp.clicked() {
                    super::parts::open_chain(app, c.id());
                }
            }
            v.buf = buf;
        } else if k == shown && count < cantus_core::MAX_SENDS {
            let rest = Rect::from_min_max(r.min, pos2(r.right(), a + rows as f32 * rh));
            send_add(v, app, ui, b, pass, c, rest, pos2(x0 + 3.0, ry + rh * 0.5));
        }
    }
}

/// One send row (Wide and Large; the owner's choice 3): the return's name and a small knob
/// of its level at the right — the knob a driven-aware control (its automation's sign, a
/// modulator's rings); the value in dB in the name's place while the pointer is on the
/// row or drags it. Drag: the level (double-click 0 dB); a click switches the send; Ctrl+
/// click selects the return; right-click is the route's menu.
#[allow(clippy::too_many_arguments)]
fn send_row(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    c: &Col,
    k: usize,
    send: cantus_core::Send,
    r: Rect,
    x0: f32,
    w: f32,
) {
    let s = pass.s;
    let p = pass.pal;
    let large = pass.mode() == WidthMode::Xl;
    let ctx = ui.ctx().clone();
    let id = pid(c.id(), Part::SendRow(k as u8));
    let resp = sense(ui, pass, r, id, Sense::click_and_drag());
    let hot = resp.as_ref().is_some_and(|r| r.hovered() || r.dragged());
    let to = app.project().strip_index(send.to);
    let mut level = send.level.clamp(0.0, 1.0);
    let kr = if large { 7.0 } else { 5.5 };
    let kc = pos2(x0 + w - kr - 1.0, r.center().y);
    let ks = s.scaled(s.k * kr / 10.0);
    let dm = DeskMod::new(
        app,
        cantus_core::AutoTarget::Send {
            strip: c.sn.id,
            to: send.to,
        },
        level,
        &ks,
    );
    if let Some(resp) = &resp {
        let depth = dm.input(app, ui, resp);
        if let Some(np) = knob_input(v, ui, pass, resp, level, 1.0, !depth) {
            level = np;
            app.set_strip_send(c.sn.id, cantus_core::Send { level: np, ..send });
        }
        if resp.drag_stopped() && !depth {
            app.end_gesture();
        }
    }
    let on = send.enabled;
    let mut buf = std::mem::take(&mut v.buf);
    buf.clear();
    if hot {
        // The value in the name's place while pointed at or dragged.
        super::parts::write_level(&mut buf, level);
    } else if let Some(i) = to {
        let name = v.desk.name(app, i);
        if name.is_empty() {
            let _ = write!(buf, "{}", i + 1);
        } else {
            buf.push_str(name);
        }
    }
    let size = if large { 11.0 } else { 10.0 };
    let ink = if hot {
        p.ink1
    } else if on {
        p.ink2
    } else {
        p.ink4
    };
    look::say(
        b,
        s,
        &buf,
        x0,
        r.center().y + size * 0.36,
        Align2::LEFT_BOTTOM,
        w - 2.0 * kr - 4.0,
        size,
        W::Regular,
        ink,
    );
    let st = resp.as_ref().map_or(State::REST, |r| State::of(r, false));
    knob::paint(
        b,
        kc,
        &ks,
        State {
            disabled: !on,
            ..st
        },
        &KnobLook {
            live: dm.live,
            rings: dm.rings(),
            ..KnobLook::new(KnobSize::Mini, level, 0.0)
        },
    );
    dm.paint_drop(
        b,
        ui,
        resp.as_ref(),
        Rect::from_center_size(kc, vec2(2.0 * kr + 6.0, 2.0 * kr + 6.0)),
        s,
    );
    let Some(resp) = resp else {
        v.buf = buf;
        return;
    };
    if hot {
        let name = to.map_or("", |i| v.desk.name(app, i));
        let tap = if send.tap.is_post() {
            &v.words.tap[0]
        } else {
            &v.words.tap[1]
        };
        hint::set(&ctx, &v.words.send_row, Caps::PARAM, resp.dragged(), |wr| {
            write!(wr, "{name} \u{b7} ")?;
            super::parts::write_level(wr, level);
            write!(wr, " \u{b7} {tap}")
        });
    }
    v.buf = buf;
    if resp.clicked() && !resp.double_clicked() {
        if pass.mods.command {
            if to.is_some() {
                app.select_strip(send.to);
            }
        } else {
            app.set_strip_send(
                c.sn.id,
                cantus_core::Send {
                    enabled: !on,
                    ..send
                },
            );
            app.end_gesture();
        }
    }
    if resp.secondary_clicked() {
        let at = resp.interact_pointer_pos().unwrap_or(r.left_bottom());
        let route = cantus_core::routing::Route::send(c.sn.id, send.to);
        crate::views::route_menu::open(&ctx, super::route_owner(), route, at);
    }
}

/// Compact's sends (the owner's choice 3): one dot per send (the accent while it plays, a
/// ring while it is off); a click opens its knob above the dots, a click elsewhere closes it.
fn compact_sends(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    c: &Col,
    a: f32,
) {
    let p = pass.pal;
    // The first three sends, copied out (a send is a few numbers: nothing allocated).
    let mut sends = [None; 3];
    if let Some(st) = app.project().strip(c.sn.id) {
        for (k, x) in st.sends.iter().take(3).enumerate() {
            sends[k] = Some(*x);
        }
    }
    let n = sends.iter().flatten().count();
    if n == 0 {
        return;
    }
    let x0 = c.cx() - (n as f32 - 1.0) * 4.0;
    for (k, send) in sends.iter().flatten().enumerate() {
        let dc = pos2(x0 + k as f32 * 8.0, a + 6.0);
        let hit = Rect::from_center_size(dc, vec2(8.0, 12.0));
        let resp = sense(
            ui,
            pass,
            hit,
            pid(c.id(), Part::SendRow(k as u8)).with("dot"),
            Sense::click(),
        );
        let hot = resp.as_ref().is_some_and(|r| r.hovered());
        if send.enabled {
            b.circle(dc, if hot { 3.0 } else { 2.5 }, Fill::Solid(p.accent));
        } else {
            b.circle_stroke(dc, 2.5, b.px(), p.ink4);
        }
        if let Some(resp) = resp {
            if hot {
                hint::set_text(ui.ctx(), &v.words.send_level, "", Caps::NONE);
            }
            if resp.clicked() {
                v.send_pop = Some((c.sn.id, k));
            }
        }
    }
    // The open knob: on a raised pill above the dots.
    let Some((sid, k)) = v.send_pop.filter(|(s, _)| *s == c.sn.id) else {
        return;
    };
    let Some(send) = sends.get(k).copied().flatten() else {
        v.send_pop = None;
        return;
    };
    let pr = Rect::from_center_size(pos2(c.cx(), a - 18.0), vec2(28.0, 30.0));
    b.rrect(b.snap_rect(pr), 4.0, Fill::Solid(p.raised));
    b.rrect_stroke(b.snap_rect(pr), 4.0, b.px(), p.edge);
    let row = Rect::from_center_size(pr.center(), vec2(28.0, 28.0));
    let id = pid(Some(sid), Part::SendRow(k as u8));
    let resp = sense(ui, pass, row, id, Sense::click_and_drag());
    let s = pass.s;
    let ks = s.scaled(s.k * 0.9);
    let mut level = send.level.clamp(0.0, 1.0);
    let dm = DeskMod::new(
        app,
        cantus_core::AutoTarget::Send {
            strip: sid,
            to: send.to,
        },
        level,
        &ks,
    );
    if let Some(resp) = &resp {
        let depth = dm.input(app, ui, resp);
        if let Some(np) = knob_input(v, ui, pass, resp, level, 1.0, !depth) {
            level = np;
            app.set_strip_send(sid, cantus_core::Send { level: np, ..send });
        }
        if resp.drag_stopped() && !depth {
            app.end_gesture();
        }
        if resp.hovered() || resp.dragged() {
            hint::set(
                ui.ctx(),
                &v.words.send_level,
                Caps::PARAM,
                resp.dragged(),
                |w| {
                    super::parts::write_level(w, level);
                    Ok(())
                },
            );
        }
    }
    let st = resp.as_ref().map_or(State::REST, |r| State::of(r, false));
    knob::paint(
        b,
        pr.center(),
        &ks,
        st,
        &KnobLook {
            live: dm.live,
            rings: dm.rings(),
            ..KnobLook::new(KnobSize::Mini, level, 0.0)
        },
    );
    dm.paint_drop(b, ui, resp.as_ref(), row, s);
    // A press anywhere else closes it.
    let (pressed, at) = ui.input(|i| (i.pointer.any_pressed(), i.pointer.interact_pos()));
    if pressed
        && at.is_some_and(|q| {
            !pr.expand(2.0).contains(q)
                && !Rect::from_center_size(pos2(c.cx(), a + 6.0), vec2(c.w, 12.0)).contains(q)
        })
    {
        v.send_pop = None;
    }
}

/// The first free send row: a faint `+`; a click lists the destinations (with *New return
/// ▸*), a drag from it makes a cable (as from the socket).
#[allow(clippy::too_many_arguments)]
fn send_add(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    c: &Col,
    r: Rect,
    at: egui::Pos2,
) {
    let p = pass.pal;
    let resp = sense(
        ui,
        pass,
        r,
        pid(c.id(), Part::SendAdd),
        Sense::click_and_drag(),
    );
    let hot = resp.as_ref().is_some_and(|r| r.hovered());
    look::plus(b, at, if hot { p.ink2 } else { p.ink4 });
    let Some(resp) = resp else {
        return;
    };
    super::drag::source(v, app, ui, &resp, super::drag::Source::Port(c.sn.id));
    if hot {
        hint::set_text(ui.ctx(), &v.words.send_add, "", Caps::NONE);
    }
    if resp.clicked() {
        crate::views::route_menu::open_add_send(
            ui.ctx(),
            super::route_owner(),
            c.sn.id,
            pos2(r.left(), r.bottom()),
        );
    }
}

/// Where the strip goes (Output row). To the master it says so quietly ("Master", ink 4);
/// to a bus, the output glyph and the bus's name in ink 2; Compact, the bus's colour as a
/// dot. The master's row: "→ Out 1–2" (Large: the short-term loudness). A click lists every
/// destination.
pub(super) fn output(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    c: &Col,
) {
    let Some((a, z)) = pass.row(Row::Output) else {
        return;
    };
    if c.vca {
        return;
    }
    let s = pass.s;
    let p = pass.pal;
    let mode = pass.mode();
    let r = Rect::from_min_max(pos2(c.x + 3.0, a), pos2(c.x + c.w - 3.0, z));
    let base = a + if mode == WidthMode::Xl { 13.0 } else { 10.0 };
    let mut buf = std::mem::take(&mut v.buf);
    buf.clear();
    if c.master {
        if mode == WidthMode::Xl {
            let lufs = app.engine.meters().master_loudness.short_term;
            if lufs.is_finite() && lufs > -70.0 {
                if lufs < 0.0 {
                    buf.push('\u{2212}');
                }
                let _ = write!(buf, "{:.1}", lufs.abs());
            } else {
                buf.push('\u{2014}');
            }
            let unit = &v.words.lufs;
            let vw = look::width_of(ui.ctx(), s, &buf, 11.0, W::Regular);
            let uw = look::width_of(ui.ctx(), s, unit, 9.0, W::Regular);
            let x0 = c.cx() - (vw + 4.0 + uw) * 0.5;
            look::say(
                b,
                s,
                &buf,
                x0,
                base,
                Align2::LEFT_BOTTOM,
                vw + 2.0,
                11.0,
                W::Regular,
                if lufs.is_finite() { p.ink1 } else { p.ink4 },
            );
            look::say(
                b,
                s,
                unit,
                x0 + vw + 4.0,
                base,
                Align2::LEFT_BOTTOM,
                uw + 2.0,
                9.0,
                W::Regular,
                p.ink3,
            );
        } else if mode == WidthMode::Wide {
            out_label(b, s, &p, c, &v.words.out_main, base, ui.ctx());
        }
        v.buf = buf;
        return;
    }
    let dest = c.sn.output.and_then(|o| app.project().strip_index(o));
    let lit = reach(app, pass.lit, c);
    let routes = routes_out(v, c);
    if let Some(how) = lit {
        v.buf = buf;
        dest_tag(v, b, pass, c, how);
        buf = std::mem::take(&mut v.buf);
        buf.clear();
    }
    match (dest, mode) {
        _ if lit.is_some() && mode != WidthMode::Compact => {}
        (_, m) if routes >= 2 && m != WidthMode::Compact => {
            fill_n(&mut buf, &v.words.routes, routes);
            look::say(
                b,
                s,
                &buf,
                c.cx(),
                base,
                Align2::CENTER_BOTTOM,
                c.w - 6.0,
                10.0,
                W::Regular,
                p.ink3,
            );
        }
        (Some(i), WidthMode::Compact) => {
            let hue = v
                .desk
                .info(i)
                .and_then(|i| i.color)
                .map(crate::theme::track_color);
            let dot = hue.map_or(p.ink3, |h| p.hue(h, 0.9));
            b.circle(pos2(c.cx(), a + 6.0), 2.5, Fill::Solid(dot));
        }
        (Some(i), _) => {
            buf.push_str(v.desk.name(app, i));
            if buf.is_empty() {
                let _ = write!(buf, "{}", i + 1);
            }
            out_label(b, s, &p, c, &buf, base, ui.ctx());
        }
        (None, WidthMode::Compact) => {}
        (None, _) => {
            let ink = if p.dark { p.ink4 } else { p.ink3 };
            look::say(
                b,
                s,
                &v.words.master_short,
                c.cx(),
                base,
                Align2::CENTER_BOTTOM,
                c.w - 6.0,
                10.0,
                W::Regular,
                ink,
            );
        }
    }
    v.buf = buf;
    let Some(resp) = sense(ui, pass, r, pid(c.id(), Part::Output), Sense::click()) else {
        return;
    };
    if resp.hovered() {
        v.hint_buf.clear();
        match dest {
            Some(i) => {
                let _ = write!(v.hint_buf, "{} {}", i + 1, v.desk.name(app, i));
            }
            None => v.hint_buf.push_str(&v.words.master_short),
        }
        hint::set_text(
            ui.ctx(),
            &v.words.output,
            &v.hint_buf,
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    if resp.clicked() || resp.secondary_clicked() {
        v.popups
            .open(Menu::Route(c.sn.id), pos2(r.left(), r.bottom()));
    }
}

/// "→ name": the output glyph (12, ink 3) and the name (Regular 10, ink 2), centred.
fn out_label(
    b: &mut Brush,
    s: &crate::kit::Style,
    p: &Pal,
    c: &Col,
    name: &str,
    base: f32,
    ctx: &egui::Context,
) {
    let room = c.w - 14.0;
    let lw = look::width_of(ctx, s, name, 10.0, W::Regular).min(room);
    let x0 = c.cx() - (lw + 12.0) * 0.5;
    look::glyph(
        b,
        s,
        pos2(x0 + 4.0, base - 3.6),
        12.0,
        MGlyph::Out,
        Ink::Solid(p.ink3),
        p.surface,
    );
    look::say(
        b,
        s,
        name,
        x0 + 11.0,
        base,
        Align2::LEFT_BOTTOM,
        room,
        10.0,
        W::Regular,
        p.ink2,
    );
}

/// `template` with its "{n}" as `n`, written into `out` without allocating.
pub(super) fn fill_n(out: &mut String, template: &str, n: usize) {
    match template.split_once("{n}") {
        Some((a, z)) => {
            out.push_str(a);
            let _ = write!(out, "{n}");
            out.push_str(z);
        }
        None => out.push_str(template),
    }
}

/// How the lit strip (the one under the pointer, else the selected one) reaches strip `c`:
/// its output, a send (its level in dB) or a key into one of `c`'s inserts.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(super) enum Reach {
    Out,
    Send(f32),
    Key,
}

pub(super) fn reach(app: &CantusApp, lit: Option<StripId>, c: &Col) -> Option<Reach> {
    let from = lit.filter(|l| *l != c.sn.id && !c.master)?;
    let p = app.project();
    let f = p.strip(from)?;
    if f.output == Some(c.sn.id) {
        return Some(Reach::Out);
    }
    if let Some(sd) = f.sends.iter().find(|x| x.to == c.sn.id) {
        let db = if sd.level <= 0.0 {
            cantus_core::MIN_GAIN_DB
        } else {
            cantus_core::linear_to_db(sd.level)
        };
        return Some(Reach::Send(db));
    }
    let keyed = p
        .strip(c.sn.id)
        .is_some_and(|me| me.effects.iter().any(|e| e.sidechain == Some(from)));
    keyed.then_some(Reach::Key)
}

/// A destination of the lit strip (option C): the strip outlined in the accent, and in its
/// output row a tag of the route's kind — "> out", "> send −12", "> key" (Compact: the
/// outline alone).
pub(super) fn dest_tag(v: &mut MixerView, b: &mut Brush, pass: &Pass, c: &Col, how: Reach) {
    let p = pass.pal;
    let body = pass.body(c.x, c.w);
    b.rrect_stroke(
        b.snap_rect(body).shrink(b.px() * 0.5),
        3.0,
        b.px(),
        p.accent,
    );
    if pass.mode() == WidthMode::Compact {
        return;
    }
    let Some((a, z)) = pass.row(Row::Output).or(pass.row(Row::Readout)) else {
        return;
    };
    let mut buf = std::mem::take(&mut v.buf);
    buf.clear();
    buf.push_str("> ");
    match how {
        Reach::Out => buf.push_str(&v.words.tag_out),
        Reach::Key => buf.push_str(&v.words.tag_key),
        Reach::Send(db) => {
            buf.push_str(&v.words.tag_send);
            buf.push(' ');
            if db <= cantus_core::MIN_GAIN_DB + 0.01 {
                buf.push('\u{2212}');
                buf.push('\u{221e}');
            } else if db < -0.5 {
                let _ = write!(buf, "\u{2212}{:.0}", -db);
            } else {
                let _ = write!(buf, "{:+.0}", db);
            }
        }
    }
    let r = Rect::from_min_max(pos2(c.x + 4.0, a - 1.0), pos2(c.x + c.w - 4.0, z + 1.0));
    b.rrect(
        b.snap_rect(r),
        3.0,
        Fill::Solid(mix(p.surface, p.accent, 0.18)),
    );
    b.rrect_stroke(b.snap_rect(r), 3.0, b.px(), with_alpha(p.accent, 0.6));
    look::say(
        b,
        pass.s,
        &buf,
        r.center().x,
        r.center().y + 3.6,
        Align2::CENTER_BOTTOM,
        r.width() - 4.0,
        10.0,
        W::Regular,
        p.accent,
    );
    v.buf = buf;
}

/// How many routes leave strip `c`: its output to a strip, its sends, its keys into other
/// strips' inserts.
pub(super) fn routes_out(v: &MixerView, c: &Col) -> usize {
    v.desk.routes_out.get(c.sn.index).copied().unwrap_or(0) as usize
}
