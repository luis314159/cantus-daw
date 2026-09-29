//! The channel sheet (ADR 0024 §7): input → inserts as tiles → the strip EQ as a display
//! that is a control (`inspector_eq`) → sends → routing and delay compensation → output
//! (`inspector_sends`). A subject with no strip (a track that goes straight to the
//! master, or nothing selected) shows every section dimmed, never hidden.

use super::eq::{self, EqCurve};
use super::menus::MenuId;
use super::sheet::{self, Count, Sheet};
use super::{mini, sends, Act, MenuOpen, Subject};
use crate::app::CantusApp;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::tile::{self, TileLook};
use crate::kit::{cache, Fill, Function, State, Tok};
use cantus_core::{Command, EffectId, EffectSlot, MergeKey, MixerStrip, StripId, MAX_INSERTS};
use egui::{pos2, Align2, Id, Rect, Sense, Vec2};
use std::fmt::Write as _;

/// What the channel sheet keeps between frames.
#[derive(Default)]
pub struct Chan {
    /// Inserts whose mini view is open.
    pub open: Vec<EffectId>,
    /// The EQ band being dragged in the display.
    pub eq_drag: Option<usize>,
    pub eq: EqCurve,
    pub mini: mini::Curves,
    /// An insert being dragged to a new place: which, and where it started.
    moving: Option<(EffectId, usize)>,
}

pub fn show(
    sh: &mut Sheet<'_>,
    st: &mut Chan,
    app: &CantusApp,
    subject: Subject,
    strip: Option<StripId>,
) {
    let p = app.project();
    if matches!(
        subject,
        Subject::Track(_) | Subject::Clip(..) | Subject::Notes(..)
    ) {
        // Below a track, a clip or notes: where the channel's own sheet begins.
        sh.band("inspector.mixer", Function::Output, None, false, None);
    }
    input(sh, app, subject, strip);
    match strip.and_then(|id| p.strip(id)) {
        Some(ms) => {
            inserts(sh, st, app, ms);
            eq::show(sh, st, Some(ms));
            sends::sends(sh, app, Some(ms));
            sends::routing(sh, app, Some(ms));
        }
        None => {
            // No strip: every section in its place, dimmed.
            sh.band(
                "kit.inspector.inserts",
                Function::Dynamics,
                None,
                false,
                None,
            );
            for i in 0..MAX_INSERTS {
                let _ = empty_tile(sh, i, true);
            }
            eq::show(sh, st, None);
            sends::sends(sh, app, None);
            sends::routing(sh, app, None);
        }
    }
}

/// The input row: what feeds the strip. On a strip chosen on the desk, a click shows the
/// track that plays into it.
fn input(sh: &mut Sheet<'_>, app: &CantusApp, subject: Subject, strip: Option<StripId>) {
    let p = app.project();
    let s = sh.s;
    sh.ui.add_space(s.d(4.0));
    let row = sh.row();
    // FL's input row sits on the panel, above the lighter ground of the tiles.
    let ground = Rect::from_min_max(
        pos2(row.full.left(), row.full.top() - s.d(4.0)),
        pos2(row.full.right(), row.full.bottom() + s.d(2.0)),
    );
    sh.b.rect(ground, Fill::Solid(s.t.panel));
    let first = strip.and_then(|id| p.tracks_on_strip(id).next().map(|t| t.id));
    sh.label(row.label, "kit.inspector.input", first.is_none());
    let (Some(id), Some(first)) = (strip, first) else {
        sheet::placeholder_well(sh.b, row.value, &s, State::DISABLED, "kit.inspector.none");
        return;
    };
    let text = sh
        .fmt
        .get(cache::key(("insp-input", id)), app.bus.revision(), |b| {
            let mut names = p.tracks_on_strip(id);
            if let Some(t) = names.next() {
                b.push_str(&t.name);
            }
            let more = names.count();
            if more > 0 {
                let _ = write!(b, " + {more}");
            }
        });
    let clickable = matches!(subject, Subject::Strip(_));
    let r = sh
        .ui
        .interact(row.value, Id::new(("insp-input", id)), Sense::click());
    let st = if clickable {
        State::of(&r, false)
    } else {
        State::REST
    };
    crate::kit::well::readout(sh.b, row.value, &s, st, text, false);
    if clickable && r.hovered() {
        hint::set_text(sh.ui.ctx(), text, "", Caps::NONE);
    }
    if clickable && r.clicked() {
        sh.acts.push(Act::SelectTrack(first));
    }
}

/// The inserts: every slot drawn, empty ones as italic placeholders, so nothing jumps
/// when one is filled (FL). ▸ opens the slot's mini view; the name opens its editor; the
/// ring is its dry/wet; the LED its bypass; a drag moves it; right-click has the rest.
fn inserts(sh: &mut Sheet<'_>, st: &mut Chan, app: &CantusApp, ms: &MixerStrip) {
    let s = sh.s;
    let id = ms.id;
    let n = ms.effects.len();
    let count = Count {
        key: cache::key(("insp-fxcount", id)),
        stamp: n as u64,
        write: &|b: &mut String| {
            let _ = write!(b, "{n} / {MAX_INSERTS}");
        },
    };
    let fx = ms.fx_enabled;
    let band = sh.band(
        "kit.inspector.inserts",
        Function::Dynamics,
        Some(fx),
        false,
        Some(count),
    );
    if band.power {
        sh.acts
            .push(Act::Exec(Command::SetStripFxEnabled { id, enabled: !fx }));
    }
    let mut rects = [Rect::NOTHING; MAX_INSERTS];
    for (i, slot) in ms.effects.iter().enumerate() {
        rects[i] = insert_tile(sh, st, app, ms, i, slot);
        if st.open.contains(&slot.id) {
            mini::show(sh, &mut st.mini, app, ms, slot);
        }
    }
    for i in n..MAX_INSERTS {
        if let Some(anchor) = empty_tile(sh, i, false) {
            *sh.menu = Some(MenuOpen {
                id: MenuId::AddInsert(id),
                anchor,
            });
        }
    }
    // A tile being dragged: a line in the selection colour where it would land.
    let Some((effect, from)) = st.moving else {
        return;
    };
    let Some(p) = sh.ui.ctx().pointer_interact_pos() else {
        return;
    };
    let to = (0..n).find(|&i| p.y < rects[i].center().y).unwrap_or(n);
    let target = if to > from { to - 1 } else { to };
    if target != from && n > 0 {
        let y = if to < n {
            rects[to].top() - s.s(1.0)
        } else {
            rects[n - 1].bottom() + s.s(1.0)
        };
        let (x0, x1) = (rects[0].left(), rects[0].right());
        sh.b.rect(
            Rect::from_min_max(pos2(x0, y - s.s(1.0)), pos2(x1, y + s.s(1.0))),
            Fill::Solid(s.t.selection),
        );
    }
    if !sh.ui.input(|i| i.pointer.any_down()) {
        st.moving = None;
        if target != from {
            sh.acts.push(Act::Exec(Command::MoveEffect {
                strip: id,
                effect,
                index: target,
            }));
        }
    }
}

/// Allocate a tile's row (18 pt and a 1-pt gap) inside the margins.
fn tile_rect(sh: &mut Sheet<'_>) -> Rect {
    let s = sh.s;
    let w = sh.width();
    let (r, _) = sh
        .ui
        .allocate_exact_size(Vec2::new(w, s.d(tile::HEIGHT) + s.s(1.0)), Sense::hover());
    let m = s.s(sheet::MARGIN - 2.0);
    Rect::from_min_max(
        pos2(r.left() + m, r.top()),
        pos2(r.right() - m, r.bottom() - s.s(1.0)),
    )
}

/// One filled slot. Returns its rect (for the drop line of a drag).
fn insert_tile(
    sh: &mut Sheet<'_>,
    st: &mut Chan,
    app: &CantusApp,
    ms: &MixerStrip,
    i: usize,
    slot: &EffectSlot,
) -> Rect {
    let s = sh.s;
    let rect = tile_rect(sh);
    let parts = tile::rects(rect, &s);
    let e = slot.id;
    // The body first: the smaller parts on top of it take the pointer where they are.
    let body = sh
        .ui
        .interact(rect, Id::new(("insp-tile", e)), Sense::click_and_drag());
    let menu = sh
        .ui
        .interact(parts.menu, Id::new(("insp-tile-menu", e)), Sense::click());
    let lamp = sh.ui.interact(
        parts.led.expand(s.s(3.0)),
        Id::new(("insp-tile-led", e)),
        Sense::click(),
    );
    let ring = sheet::drag(
        sh.ui,
        Id::new(("insp-tile-mix", e)),
        parts.mix.expand(s.s(2.0)),
        slot.mix,
        &sheet::Range::lin(0.0, 1.0, 1.0),
        false,
    );
    if menu.clicked() {
        match st.open.iter().position(|x| *x == e) {
            Some(k) => {
                st.open.remove(k);
            }
            None => st.open.push(e),
        }
    }
    if lamp.clicked() {
        sh.acts.push(Act::Exec(Command::SetEffectBypass {
            strip: ms.id,
            effect: e,
            bypass: !slot.bypass,
        }));
    }
    if let Some(mix) = ring.value {
        sh.acts.push(Act::Merge(
            Command::SetEffectMix {
                strip: ms.id,
                effect: e,
                mix,
            },
            MergeKey::of(&("fxmix", e.0)),
        ));
    }
    if ring.ended {
        sh.acts.push(Act::EndGesture);
    }
    if body.clicked() {
        if slot.spec.kind.has_editor() {
            sh.acts.push(Act::OpenEditor(ms.id, e));
        } else if !st.open.contains(&e) {
            st.open.push(e);
        }
    }
    if body.drag_started() {
        st.moving = Some((e, i));
    }
    if body.secondary_clicked() {
        *sh.menu = Some(MenuOpen {
            id: MenuId::Slot(ms.id, e),
            anchor: rect,
        });
    }
    let hosted = slot.spec.external.as_deref();
    let missing = hosted.is_some() && app.plugins.missing(e).is_some();
    let open = st.open.contains(&e);
    let look = TileLook {
        name: Some(Src::Str("")),
        placeholder: Src::Str(""),
        mix: Some(ring.value.unwrap_or(slot.mix)),
        on: !slot.bypass && ms.fx_enabled,
    };
    let tst = State {
        lit: open || st.moving.is_some_and(|(m, _)| m == e),
        ..State::of(&body, false)
    };
    tile::paint(sh.b, rect, &s, tst, &look);
    // The name, with the chips a hosted plug-in carries (its format; missing).
    let mut right = parts.name.right();
    if let Some(p) = hosted {
        let cy = rect.center().y;
        if missing {
            let w = sheet::chip(
                sh.b,
                pos2(right, cy),
                &s,
                Src::Key("host.missing"),
                Tok::Danger,
                1.0,
            );
            right -= w + s.s(3.0);
        }
        let w = sheet::chip(
            sh.b,
            pos2(right, cy),
            &s,
            Src::Str(p.format.badge()),
            Tok::TextDim,
            1.0,
        );
        right -= w + s.s(4.0);
    }
    let ink = if missing { s.t.danger } else { s.t.text };
    let a = if slot.bypass || !ms.fx_enabled {
        0.6
    } else {
        1.0
    };
    let name = Text {
        src: insert_name(slot),
        role: Role::Body,
        caps: false,
        align: Align2::LEFT_CENTER,
        policy: Policy::Elide(Elide::Middle),
        color: ink.gamma_multiply(a),
    };
    let nr = Rect::from_min_max(
        parts.name.min,
        pos2(right.max(parts.name.left()), parts.name.bottom()),
    );
    let _ = text::paint(sh.b, nr, &name, s.tk());
    if body.hovered() || menu.hovered() || lamp.hovered() {
        let key = if lamp.hovered() {
            "effect.bypass"
        } else if menu.hovered() {
            "inspector.mini_tip"
        } else if slot.spec.kind.has_editor() {
            "inspector.slot_tip"
        } else {
            "inspector.slot_tip_plain"
        };
        let name = sh.fmt.tr(key);
        hint::set_text(
            sh.ui.ctx(),
            name,
            "",
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    if ring.resp.hovered() || ring.resp.dragged() {
        let v = ring.value.unwrap_or(slot.mix);
        let name = sheet::name(sh.fmt.tr("effect.mix"));
        let text = sh
            .fmt
            .get(cache::key(("insp-mix", e)), sheet::bits(v), |b| {
                sheet::percent(v, b)
            });
        hint::set_text(sh.ui.ctx(), &name, text, Caps::PARAM);
    }
    rect
}

/// One insert's name as its tile shows it.
fn insert_name(slot: &EffectSlot) -> Src<'_> {
    match slot.spec.external.as_deref() {
        Some(p) => Src::Str(&p.name),
        None => Src::Key(slot.spec.kind.name_key()),
    }
}

/// An empty slot: "Slot 6" in italics. A click returns its rect (the add menu hangs
/// from it).
pub fn empty_tile(sh: &mut Sheet<'_>, i: usize, dim: bool) -> Option<Rect> {
    let s = sh.s;
    let rect = tile_rect(sh);
    let r = sh
        .ui
        .interact(rect, Id::new(("insp-empty-slot", i)), Sense::click());
    let label = sh.fmt.get(
        cache::key(("insp-slot", i)),
        crate::i18n::locale() as u64,
        |b| b.push_str(&crate::i18n::tf("kit.inspector.slot", &[("n", &(i + 1))])),
    );
    let look = TileLook {
        name: None,
        placeholder: Src::Str(label),
        mix: Some(1.0),
        on: false,
    };
    let st = State {
        disabled: dim,
        ..State::of(&r, false)
    };
    tile::paint(sh.b, rect, &s, st, &look);
    if r.hovered() && !dim {
        let name = sh.fmt.tr("inspector.slot_add");
        hint::set_text(
            sh.ui.ctx(),
            name,
            "",
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    (!dim && (r.clicked() || r.secondary_clicked())).then_some(rect)
}
