//! Sends, one row each — the send's lamp, its destination, its `POST | PRE` tap and its
//! level — then routing: the chain's delay compensation, the engine's output latency and
//! where the strip goes (ADR 0024 §7). The tap switch is the kit's (`kit::tap`, round 25):
//! a click issues `cmd.set_send_tap`, one undo step.

use super::menus::MenuId;
use super::sheet::{self, Range, Sheet};
use super::{Act, MenuOpen};
use crate::app::CantusApp;
use crate::kit::hint::{self, Caps};
use crate::kit::{cache, led, tap, Function, State, Tok};
use cantus_core::routing::plan;
use cantus_core::{Command, MergeKey, MixerStrip, Send, MAX_SENDS};
use egui::{pos2, Id, Rect, Sense};
use std::fmt::Write as _;

pub fn sends(sh: &mut Sheet<'_>, app: &CantusApp, ms: Option<&MixerStrip>) {
    let s = sh.s.function(Function::Space);
    let room = ms.is_some_and(|m| m.sends.len() < MAX_SENDS);
    let band = sh.band("kit.inspector.sends", Function::Space, None, room, None);
    if let (true, Some(m), Some(anchor)) = (band.plus, ms, band.plus_rect) {
        *sh.menu = Some(MenuOpen {
            id: MenuId::AddSend(m.id),
            anchor,
        });
    }
    let Some(m) = ms.filter(|m| !m.sends.is_empty()) else {
        // No sends yet: the empty row says what would go there, and opens the list.
        let row = sh.row();
        let m_ = s.s(sheet::MARGIN);
        let r = Rect::from_min_max(
            pos2(row.full.left() + m_, row.value.top()),
            pos2(row.full.right() - m_, row.value.bottom()),
        );
        let dim = ms.is_none();
        let resp = sh
            .ui
            .interact(r, Id::new("insp-send-empty"), Sense::click());
        let st = State {
            disabled: dim,
            ..State::of(&resp, false)
        };
        sheet::placeholder_well(sh.b, r, &s, st, "inspector.sends_add");
        if let (true, Some(m)) = (resp.clicked() && !dim, ms) {
            *sh.menu = Some(MenuOpen {
                id: MenuId::AddSend(m.id),
                anchor: r,
            });
        }
        return;
    };
    let id = m.id;
    for send in &m.sends {
        let row = sh.row();
        let mg = s.s(sheet::MARGIN);
        let gap = s.s(4.0);
        let (x0, x1) = (row.full.left() + mg, row.full.right() - mg);
        let (y0, y1) = (row.value.top(), row.value.bottom());
        let total = x1 - x0;
        let lamp_w = s.s(16.0);
        // Wide enough that POST and PRE are read whole, in both languages (round 25).
        let seg_w = s.s(84.0);
        let lamp = Rect::from_min_max(pos2(x0, y0), pos2(x0 + lamp_w, y1));
        let (dest, seg, level) = if total >= s.s(250.0) {
            // One line: lamp, destination, pre/post, level.
            let level_w = (total * 0.30).max(s.s(64.0));
            let dest_w = (total - lamp_w - seg_w - level_w - 3.0 * gap).max(s.s(24.0));
            let dest = Rect::from_min_max(
                pos2(lamp.right() + gap, y0),
                pos2(lamp.right() + gap + dest_w, y1),
            );
            let seg = Rect::from_min_max(
                pos2(dest.right() + gap, y0),
                pos2(dest.right() + gap + seg_w, y1),
            );
            (
                dest,
                seg,
                Rect::from_min_max(pos2(seg.right() + gap, y0), pos2(x1, y1)),
            )
        } else {
            // A narrow panel: the destination has the first line to itself, pre/post and
            // the level the second — nothing is cut to fit.
            let next = sh.row();
            let (z0, z1) = (next.value.top(), next.value.bottom());
            let dest = Rect::from_min_max(pos2(lamp.right() + gap, y0), pos2(x1, y1));
            let seg = Rect::from_min_max(pos2(dest.left(), z0), pos2(dest.left() + seg_w, z1));
            (
                dest,
                seg,
                Rect::from_min_max(pos2(seg.right() + gap, z0), pos2(x1, z1)),
            )
        };
        // The lamp: the send on or off, its level kept.
        let lr = sh
            .ui
            .interact(lamp, Id::new(("insp-send-on", id, send.to)), Sense::click());
        led::paint(
            sh.b,
            lamp.center(),
            s.s(11.0),
            send.enabled,
            &s,
            Tok::Led,
            Tok::PanelLight,
            1.0,
        );
        if lr.hovered() {
            let name = sh.fmt.tr("inspector.send_on");
            hint::set_text(sh.ui.ctx(), name, "", Caps::NONE);
        }
        if lr.clicked() {
            sh.acts.push(set(
                id,
                Send {
                    enabled: !send.enabled,
                    ..*send
                },
            ));
            sh.acts.push(Act::EndGesture);
        }
        let name = sh.fmt.get(
            cache::key(("insp-send-dest", id, send.to)),
            app.bus.revision(),
            |b| super::strip_label(app, send.to, b),
        );
        let name = sheet::name(name);
        sh.menu_field(dest, &name, MenuId::Send(id, send.to), false);
        // Where the send takes its copy: after the fader (and the pan law), or before
        // them. The switch says which in its own hue and explains itself on hover.
        let picked = tap::switch(
            sh.ui,
            sh.b,
            &s,
            seg,
            Id::new(("insp-send-tap", id, send.to)),
            send.tap,
        );
        if let Some(cmd) = picked.and_then(|t| plan::set_send_tap(app.project(), id, send.to, t)) {
            sh.acts.push(Act::Exec(cmd));
        }
        let lname = sheet::name(sh.fmt.tr("inspector.send_level"));
        let d = sh.value(
            level,
            Id::new(("insp-send-level", id, send.to)),
            send.level,
            &Range::lin(0.0, 1.0, 1.0),
            false,
            &lname,
            sheet::level_db,
            Function::Space,
            !send.enabled,
        );
        if let Some(v) = d.value {
            sh.acts.push(set(id, Send { level: v, ..*send }));
        }
        if d.ended {
            sh.acts.push(Act::EndGesture);
        }
    }
}

fn set(id: cantus_core::StripId, send: Send) -> Act {
    Act::Merge(
        Command::SetStripSend { id, send },
        MergeKey::of(&("send", id.0, send.to.0)),
    )
}

/// Routing: how late the strip's own chain makes it (the delay compensation the engine
/// plans for it), the whole output's latency, and where the strip goes.
pub fn routing(
    sh: &mut Sheet<'_>,
    app: &CantusApp,
    ms: Option<&MixerStrip>,
    inserts: Option<super::channel::Chain<'_>>,
) {
    sh.band("kit.inspector.routing", Function::Output, None, false, None);
    let sr = (app.engine.sample_rate as f32).max(1.0);
    let chain: usize = inserts.map_or(0, |m| {
        m.effects
            .iter()
            .filter_map(|e| app.plugins.hosted(e.id).map(|h| h.latency()))
            .sum()
    });
    let text = sh.fmt.get(cache::key("insp-delay"), chain as u64, |b| {
        let _ = write!(b, "{:.1} ms", chain as f32 / sr * 1_000.0);
    });
    let text = sheet::name(text);
    sh.text_row("kit.inspector.delay", &text, true, chain == 0);
    let total = app.engine.stats().latency_samples;
    let text = sh.fmt.get(cache::key("insp-latency"), total, |b| {
        let _ = write!(b, "{total} smp");
    });
    let text = sheet::name(text);
    sh.text_row("kit.inspector.latency", &text, true, total == 0);
    let out = ms.and_then(|m| m.output);
    let master = inserts.is_some_and(|c| c.id.is_master());
    let text = sh.fmt.get(
        cache::key(("insp-output", ms.map(|m| m.id), master)),
        app.bus.revision() ^ ((crate::i18n::locale() as u64) << 60),
        |b| match out {
            Some(o) => super::strip_label(app, o, b),
            // The master goes to the audio device.
            None if master => b.push_str(&crate::i18n::t("master.output")),
            None => b.push_str(&crate::i18n::t("mixer.master")),
        },
    );
    let text = sheet::name(text);
    match ms {
        Some(m) => {
            sh.menu_row("kit.inspector.output", &text, MenuId::Output(m.id), false);
        }
        None => {
            sh.text_row("kit.inspector.output", &text, false, !master);
        }
    }
}
