//! Sends, one row each — the send's lamp, its destination, its `POST | PRE` tap and its
//! level — then routing: the chain's delay compensation, the engine's output latency and
//! where the strip goes (ADR 0024 §7). The tap switch is the kit's (`kit::tap`, round 25):
//! a click issues `cmd.set_send_tap`, one undo step.

use super::menus::MenuId;
use super::sheet::{self, Range, Sheet};
use super::Act;
use crate::app::CantusApp;
use crate::kit::hint::{self, Caps};
use crate::kit::{cache, Function, State};
use cantus_core::{Command, MergeKey, MixerStrip, Send, MAX_SENDS};
use egui::{pos2, Id, Rect, Sense};
use std::fmt::Write as _;

pub fn sends(sh: &mut Sheet<'_>, app: &CantusApp, ms: Option<&MixerStrip>) {
    let s = sh.s.function(Function::Space);
    let room = ms.is_some_and(|m| m.sends.len() < MAX_SENDS);
    let band = sh.band("kit.inspector.sends", Function::Space, None, room, None);
    if let (true, Some(m), Some(anchor)) = (band.plus, ms, band.plus_rect) {
        add_send(sh, m.id, anchor);
    }
    let Some(m) = ms.filter(|m| !m.sends.is_empty()) else {
        // No sends yet (des-mixer's empty section): "+ Add a send", which opens the list.
        let w = sh.width();
        let (r0, _) = sh
            .ui
            .allocate_exact_size(egui::Vec2::new(w, s.s(26.0)), Sense::hover());
        let dim = ms.is_none();
        let resp = sh
            .ui
            .interact(r0, Id::new("insp-send-empty"), Sense::click());
        if dim {
            let _ = st_dim(sh, r0);
        } else {
            add_row(sh, r0, &resp, "inspector.sends_add");
        }
        if let (true, Some(m)) = (resp.clicked() && !dim, ms) {
            add_send(sh, m.id, r0);
        }
        return;
    };
    let id = m.id;
    for send in &m.sends {
        send_row(sh, app, id, send);
    }
    // "+ Add a send" (des-mixer): the desk's add-send list.
    if m.sends.len() < MAX_SENDS {
        let w = sh.width();
        let (r, resp) = sh
            .ui
            .allocate_exact_size(egui::Vec2::new(w, s.s(26.0)), Sense::click());
        add_row(sh, r, &resp, "inspector.sends_add");
        if resp.clicked() {
            add_send(sh, id, r);
        }
    }
}

/// The empty row of a sheet with no strip: "+ Add a send" quiet, doing nothing.
fn st_dim(sh: &mut Sheet<'_>, r: Rect) -> Rect {
    use crate::views::mixer::look::{self, W};
    let s = sh.s;
    let p = look::Pal::of(&s);
    let label = sh.fmt.tr("inspector.sends_add");
    look::say(
        sh.b,
        &s,
        label,
        r.left() + s.s(sheet::MARGIN) + 14.0,
        r.center().y + 4.0,
        egui::Align2::LEFT_BOTTOM,
        r.width() - 40.0,
        11.0,
        W::Regular,
        p.ink4,
    );
    r
}

/// A "+ word" row: the add glyph and its word in ink 3 (ink 1 under the pointer).
fn add_row(sh: &mut Sheet<'_>, r: Rect, resp: &egui::Response, key: &str) {
    use crate::kit::glyph_mixer::{Ink, MGlyph};
    use crate::views::mixer::look::{self, W};
    let s = sh.s;
    let p = look::Pal::of(&s);
    let hot = resp.hovered();
    let ink = if hot { p.ink1 } else { p.ink3 };
    let x = r.left() + s.s(sheet::MARGIN);
    look::glyph(
        sh.b,
        &s,
        pos2(x + 6.0, r.center().y),
        10.0,
        MGlyph::Add,
        Ink::Solid(ink),
        p.base,
    );
    let label = sh.fmt.tr(key);
    if hot {
        let name = sheet::name(label);
        hint::set_text(
            sh.ui.ctx(),
            &name,
            "",
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    let label = sh.fmt.tr(key);
    look::say(
        sh.b,
        &s,
        label,
        x + 14.0,
        r.center().y + 4.0,
        egui::Align2::LEFT_BOTTOM,
        r.width() - 40.0,
        11.0,
        W::Regular,
        ink,
    );
}

/// One send (des-mixer): its switch (a 14-pt well, the accent's dot while it plays), the
/// return's name (Regular 12; a click lists what the send can do, its tap among them), a
/// 64-pt level bar with its handle (drag: the level; double-click: 0 dB) and the level
/// in dB at the right.
fn send_row(sh: &mut Sheet<'_>, app: &CantusApp, id: cantus_core::StripId, send: &Send) {
    use crate::views::mixer::look::{self, law, W};
    let s = sh.s;
    let p = look::Pal::of(&s);
    let w = sh.width();
    let (row, _) = sh
        .ui
        .allocate_exact_size(egui::Vec2::new(w, s.s(26.0)), Sense::hover());
    let m = s.s(sheet::MARGIN);
    let cy = row.center().y;
    let base = cy + 4.0;
    let lamp = Rect::from_center_size(pos2(row.left() + m + 7.0, cy), egui::Vec2::splat(14.0));
    let lr = sh.ui.interact(
        lamp.expand(4.0),
        Id::new(("insp-send-on", id, send.to)),
        Sense::click(),
    );
    look::well(sh.b, lamp, &p, lr.hovered());
    if send.enabled {
        sh.b.circle(lamp.center(), 3.0, crate::kit::Fill::Solid(p.accent));
    } else {
        sh.b.circle_stroke(lamp.center(), 3.0, sh.b.px(), p.ink4);
    }
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
    // The name: a click lists the send's choices.
    let name_x = lamp.right() + 8.0;
    let name_r = Rect::from_min_max(pos2(name_x, row.top()), pos2(name_x + 74.0, row.bottom()));
    let nr = sh.ui.interact(
        name_r,
        Id::new(("insp-send-dest", id, send.to)),
        Sense::click(),
    );
    let name = sh.fmt.get(
        cache::key(("insp-send-dest", id, send.to)),
        app.bus.revision(),
        |b| super::strip_label(app, send.to, b),
    );
    let ink = if send.enabled { p.ink1 } else { p.ink4 };
    look::say(
        sh.b,
        &s,
        name,
        name_x,
        base,
        egui::Align2::LEFT_BOTTOM,
        74.0,
        12.0,
        W::Medium,
        ink,
    );
    if nr.clicked() || nr.secondary_clicked() {
        *sh.menu = Some(super::MenuOpen {
            id: MenuId::Send(id, send.to),
            anchor: name_r,
        });
    }
    // The level bar and its value.
    let bar = Rect::from_min_size(
        pos2(name_x + 74.0 + 8.0, cy - 1.5),
        egui::Vec2::new(64.0, 3.0),
    );
    let hit = bar.expand2(egui::Vec2::new(6.0, 9.0));
    let mut d = sheet::drag(
        sh.ui,
        Id::new(("insp-send-level", id, send.to)),
        hit,
        send.level,
        &Range::lin(0.0, 1.0, 1.0),
        false,
    );
    // The level is a parameter: learnt, automated and modulated from here too (review 2;
    // the bar of des-mixer keeps the wiring the value well had).
    let live = super::modulators::wire_drag(
        sh,
        app,
        "inspector.send_level",
        &mut d,
        hit,
        send.level.clamp(0.0, 1.0),
        cantus_core::AutoTarget::Send {
            strip: id,
            to: send.to,
        },
        true,
    );
    let level = d.value.unwrap_or(send.level).clamp(0.0, 1.0);
    if let Some(v) = d.value {
        sh.acts.push(set(id, Send { level: v, ..*send }));
    }
    if d.ended {
        sh.acts.push(Act::EndGesture);
    }
    let db = if level <= 0.0 {
        cantus_core::MIN_GAIN_DB
    } else {
        cantus_core::linear_to_db(level)
    };
    let fill = 1.0 - law::frac(db);
    let on = if send.enabled { p.ink2 } else { p.ink4 };
    sh.b.rrect(
        sh.b.snap_rect(bar),
        1.5,
        crate::kit::Fill::Solid(p.bar_track),
    );
    sh.b.rrect(
        sh.b.snap_rect(Rect::from_min_size(
            bar.min,
            egui::Vec2::new((64.0 * fill).round(), 3.0),
        )),
        1.5,
        crate::kit::Fill::Solid(on),
    );
    let hx = bar.left() + 64.0 * fill;
    sh.b.circle(pos2(hx, cy), 4.0, crate::kit::Fill::Solid(p.raised));
    sh.b.circle_stroke(pos2(hx, cy), 4.0, sh.b.px(), p.edge);
    // Where modulation has it now: a tick across the bar.
    if let Some(live) = live {
        let lv = live.clamp(0.0, 1.0);
        let ldb = if lv <= 0.0 {
            cantus_core::MIN_GAIN_DB
        } else {
            cantus_core::linear_to_db(lv)
        };
        let lx = bar.left() + 64.0 * (1.0 - law::frac(ldb));
        sh.b.vline(lx, cy - 4.0, cy + 4.0, 2.0, s.t.hue_mod);
    }
    let value = sh.fmt.get(
        cache::key(("insp-send-db", id, send.to)),
        sheet::bits(level),
        |b| sheet::level_db(level, b),
    );
    look::say(
        sh.b,
        &s,
        value,
        row.right() - m,
        base,
        egui::Align2::RIGHT_BOTTOM,
        48.0,
        11.0,
        W::Regular,
        if send.enabled { p.ink1 } else { p.ink4 },
    );
    if d.resp.hovered() || d.resp.dragged() {
        let lname = sheet::name(sh.fmt.tr("inspector.send_level"));
        let v = sh.fmt.get(
            cache::key(("insp-send-db", id, send.to)),
            sheet::bits(level),
            |b| sheet::level_db(level, b),
        );
        hint::set_text(sh.ui.ctx(), &lname, v, Caps::PARAM);
    }
}

/// *Add send*: the desk's own add-send list (`route_menu::open_add_send` — *New return ▸*
/// first, then every strip by dock), under the inspector's route-menu owner, so the two
/// places offer one list through one code path (round 26).
fn add_send(sh: &mut Sheet<'_>, from: cantus_core::StripId, anchor: Rect) {
    super::super::route_menu::open_add_send(
        sh.ui.ctx(),
        super::super::route_menu::inspector_owner(),
        from,
        anchor.left_bottom(),
    );
}

fn set(id: cantus_core::StripId, send: Send) -> Act {
    Act::Merge(
        Command::SetStripSend { id, send },
        MergeKey::of(&("send", id.0, send.to.0)),
    )
}

/// Routing (des-mixer): the Output field (where the strip goes; a list), the Input field
/// (who plays into it: a click shows its first track), the Channel's polarity, swap and
/// mono as glyph and word (an accent pill when on; the pinned strip's keys, r39:lo-mixer),
/// then the chain's delay compensation and the engine's output latency.
pub fn routing(
    sh: &mut Sheet<'_>,
    app: &CantusApp,
    ms: Option<&MixerStrip>,
    inserts: Option<super::channel::Chain<'_>>,
) {
    use super::look::Lead;
    use crate::kit::glyph_mixer::MGlyph;
    sh.band("kit.inspector.routing", Function::Output, None, false, None);
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
            sh.menu_row_with(
                "kit.inspector.output",
                Lead::Mixer(MGlyph::Out),
                &text,
                "",
                MenuId::Output(m.id),
                false,
            );
            input_row(sh, app, m);
        }
        None => {
            let row = sh.field_row();
            sh.label(row.label, "kit.inspector.output", !master);
            let st = State {
                disabled: !master,
                ..State::REST
            };
            super::look::field(
                sh.b,
                &sh.s,
                row.value,
                st,
                Lead::Mixer(MGlyph::Out),
                &text,
                "",
                false,
            );
        }
    }
    channel_row(sh, app, ms, master);
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
}

/// The Input field: the first track (and how many more), or the strips routed in.
fn input_row(sh: &mut Sheet<'_>, app: &CantusApp, m: &MixerStrip) {
    use crate::kit::glyph_mixer::MGlyph;
    let p = app.project();
    let first = p.tracks_on_strip(m.id).next().map(|t| t.id);
    let stamp = app.bus.revision() ^ ((crate::i18n::locale() as u64) << 60);
    let value = sheet::name(sh.fmt.get(cache::key(("insp-in", m.id)), stamp, |b| {
        let mut tracks = p.tracks_on_strip(m.id);
        if let Some(t) = tracks.next() {
            b.push_str(&t.name);
            let more = tracks.count();
            if more > 0 {
                let _ = write!(b, " + {more}");
            }
            return;
        }
        let mut strips = p.strips.iter().filter(|x| x.output == Some(m.id));
        match strips.next() {
            Some(x) => {
                super::strip_label(app, x.id, b);
                let more = strips.count();
                if more > 0 {
                    let _ = write!(b, " + {more}");
                }
            }
            None => b.push_str(&crate::i18n::t("inspector.none")),
        }
    }));
    let detail = sheet::name(sh.fmt.get(cache::key(("insp-in-d", m.id)), stamp, |b| {
        if first.is_some() {
            b.push_str(&crate::i18n::t("lo_mixer.track_kind"));
        } else if p.strips.iter().any(|x| x.output == Some(m.id)) {
            b.push_str(&crate::i18n::t("lo_mixer.strips_kind"));
        }
    }));
    let row = sh.field_row();
    sh.label(row.label, "lo_mixer.input", false);
    let r = sh
        .ui
        .interact(row.value, Id::new(("insp-route-in", m.id)), Sense::click());
    let st = if first.is_some() {
        State::of(&r, false)
    } else {
        State::REST
    };
    super::look::field(
        sh.b,
        &sh.s,
        row.value,
        st,
        super::look::Lead::Mixer(MGlyph::Into),
        &value,
        &detail,
        false,
    );
    if let Some(t) = first {
        if r.hovered() {
            hint::set_text(sh.ui.ctx(), &value, "", Caps::NONE);
        }
        if r.clicked() {
            sh.acts.push(Act::SelectTrack(t));
        }
    }
}

/// Channel: polarity, swap and mono as glyph and word (des-mixer: no well at rest, an
/// `accent` 20 % pill when on), wrapping under the label when they do not fit beside it.
fn channel_row(sh: &mut Sheet<'_>, app: &CantusApp, ms: Option<&MixerStrip>, master: bool) {
    use crate::kit::glyph_mixer::{Ink, MGlyph};
    use crate::views::mixer::look::{self, W};
    let p = app.project();
    let (phase, swap, width) = match ms {
        Some(m) => (m.phase_invert, Some(m.swap_channels), m.width),
        None if master => (p.master.phase_invert, None, p.master.width),
        None => return,
    };
    let mono = width <= 0.0;
    let s = sh.s;
    let pal = look::Pal::of(&s);
    let ctx = sh.ui.ctx().clone();
    let keys: [(&str, MGlyph, bool, bool); 3] = [
        ("lo_mixer.polarity", MGlyph::Phase, phase, true),
        (
            "lo_mixer.swap",
            MGlyph::Swap,
            swap.unwrap_or(false),
            swap.is_some(),
        ),
        ("lo_mixer.mono", MGlyph::Mono, mono, true),
    ];
    // Lay the three out: beside the label when they fit, else on a line under it.
    let words: Vec<(sheet::NameBuf, f32)> = keys
        .iter()
        .map(|(k, ..)| {
            let w = sheet::name(sh.fmt.tr(k));
            let width = look::width_of(&ctx, &s, &w, 10.0, W::Regular) + s.s(26.0);
            (w, width)
        })
        .collect();
    let total: f32 = keys
        .iter()
        .zip(&words)
        .filter(|((.., shown), _)| *shown)
        .map(|(_, (_, w))| *w + s.s(4.0))
        .sum();
    let row = sh.row();
    sh.label(row.label, "lo_mixer.channel", false);
    let line = if total <= row.value.width() {
        row.value
    } else {
        let r2 = sh.row();
        Rect::from_min_max(
            pos2(r2.full.left() + s.s(sheet::MARGIN), r2.value.top()),
            r2.value.max,
        )
    };
    let mut x = line.left();
    for ((key, g, on, shown), (word, w)) in keys.iter().zip(&words) {
        if !shown {
            continue;
        }
        let r = Rect::from_min_size(pos2(x, line.top()), egui::vec2(*w, line.height()));
        x += w + s.s(4.0);
        let resp = sh.ui.interact(
            r,
            Id::new(("insp-chan", *key, ms.map(|m| m.id))),
            Sense::click(),
        );
        let hot = resp.hovered();
        if *on {
            sh.b.rrect(
                sh.b.snap_rect(r),
                4.0,
                crate::kit::Fill::Solid(crate::kit::color::with_alpha(pal.accent, 0.2)),
            );
        } else if hot {
            sh.b.rrect(sh.b.snap_rect(r), 4.0, crate::kit::Fill::Solid(pal.well_hi));
        }
        let ink = if *on {
            pal.accent
        } else if hot {
            pal.ink1
        } else {
            pal.ink3
        };
        let cy = r.center().y;
        look::glyph(
            sh.b,
            &s,
            pos2(r.left() + s.s(10.0), cy),
            12.0,
            *g,
            Ink::Solid(ink),
            pal.base,
        );
        look::say(
            sh.b,
            &s,
            word,
            r.left() + s.s(19.0),
            cy + 4.0,
            egui::Align2::LEFT_BOTTOM,
            w - s.s(19.0),
            10.0,
            W::Regular,
            if *on { pal.ink1 } else { pal.ink2 },
        );
        if hot {
            hint::set_text(sh.ui.ctx(), word, "", Caps::NONE);
        }
        if !resp.clicked() {
            continue;
        }
        let cmd = match (*key, ms) {
            ("lo_mixer.polarity", Some(m)) => Command::SetStripPhaseInvert {
                id: m.id,
                invert: !phase,
            },
            ("lo_mixer.polarity", None) => Command::SetMasterPhaseInvert { invert: !phase },
            ("lo_mixer.swap", Some(m)) => Command::SetStripSwapChannels {
                id: m.id,
                swap: !m.swap_channels,
            },
            ("lo_mixer.mono", Some(m)) => Command::SetStripWidth {
                id: m.id,
                width: if mono { 1.0 } else { 0.0 },
            },
            ("lo_mixer.mono", None) => Command::SetMasterWidth {
                width: if mono { 1.0 } else { 0.0 },
            },
            _ => continue,
        };
        sh.acts.push(Act::Exec(cmd));
    }
}
