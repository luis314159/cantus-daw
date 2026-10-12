//! **Receives** — what feeds the selected strip, between *Sends* and *Routing* in the
//! channel sheet (round 25, ADR 0041; design §4.4; Bitwig's send-receive pane): a row per
//! sender (its lamp, its name, its send level as the sheet's value well, a `PRE` chip on a
//! pre-fader send, a `×2` chip on a doubled one), a row per strip routed into it, a row
//! per keyed insert of its own; fed only by its tracks, a quiet line pointing at the Input
//! row; fed by nothing, a *Nothing feeds this strip — route here* well that opens the
//! route-here menu. A return can be mixed from its own sheet.
//!
//! Reads the project directly (no list is built a frame: the senders are found by walking
//! the strips, the names and the doubled flags are cached per revision); acts through the
//! inspector's merging `SetStripSend` gestures, and — for what needs the application
//! (selecting a sender, a row's route menu) — through `views::route_menu` under
//! [`route_menu::inspector_owner`].

use super::sheet::{self, Range, Sheet};
use super::Act;
use crate::app::CantusApp;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{cache, led, Function, State, Tok};
use crate::views::route_menu::{self, Pick};
use cantus_core::routing::Route;
use cantus_core::{Command, EffectId, MergeKey, MixerStrip, Send, SendTap, StripId};
use egui::{pos2, Align2, Id, Rect, Sense};

/// The section for `ms` (`None`: the master, or no strip — the section stands dimmed).
pub fn receives(sh: &mut Sheet<'_>, app: &CantusApp, ms: Option<&MixerStrip>) {
    sh.band("routing.receives.band", Function::Output, None, false, None);
    let Some(m) = ms else {
        // The master hears every strip; no strip, nothing: the section in its place.
        let row = sh.row();
        let r = full_value(sh, &row);
        let st = State {
            disabled: true,
            ..State::REST
        };
        sheet::placeholder_well(sh.b, r, &sh.s, st, "routing.receives.master");
        return;
    };
    let p = app.project();
    let host = m.id;
    let mut any = false;
    for src in &p.strips {
        if let Some(send) = src.send_to(host) {
            any = true;
            sender(sh, app, src.id, host, *send);
        }
    }
    for src in p.strips.iter().filter(|x| x.output == Some(host)) {
        any = true;
        routed(sh, app, src.id, host);
    }
    for fx in &m.effects {
        if let Some(from) = fx.sidechain {
            any = true;
            keyed(sh, app, from, host, fx.id);
        }
    }
    if any {
        return;
    }
    let row = sh.row();
    let r = full_value(sh, &row);
    if cantus_core::routing::is_fed(p, host) {
        // Only its tracks (or notes into an insert): they are the Input row above.
        let st = State {
            disabled: true,
            ..State::REST
        };
        sheet::placeholder_well(sh.b, r, &sh.s, st, "routing.receives.tracks_only");
        return;
    }
    let resp = sh
        .ui
        .interact(r, Id::new(("insp-recv-empty", host.0)), Sense::click());
    let s = sh.s.function(Function::Output);
    sheet::placeholder_well(
        sh.b,
        r,
        &s,
        State::of(&resp, false),
        "routing.receives.none",
    );
    if resp.hovered() {
        let name = sheet::name(sh.fmt.tr("routing.here.plate"));
        let tip = sh.fmt.tr("routing.here.tip");
        hint::set_text(sh.ui.ctx(), &name, tip, Caps::NONE);
    }
    if resp.clicked() {
        let at = resp.interact_pointer_pos().unwrap_or(r.left_bottom());
        route_menu::open_route_here(sh.ui.ctx(), route_menu::inspector_owner(), host, at);
    }
}

/// The row's whole width inside the margins, at the value's height.
fn full_value(sh: &Sheet<'_>, row: &sheet::Row) -> Rect {
    let m = sh.s.s(sheet::MARGIN);
    Rect::from_min_max(
        pos2(row.full.left() + m, row.value.top()),
        pos2(row.full.right() - m, row.value.bottom()),
    )
}

/// A strip's name, cached per revision and language.
fn name_of<'a>(sh: &'a mut Sheet<'_>, app: &CantusApp, host: StripId, from: StripId) -> &'a str {
    let stamp = app.bus.revision() ^ ((crate::i18n::locale() as u64) << 60);
    sh.fmt
        .get(cache::key(("insp-recv-name", host, from)), stamp, |b| {
            super::strip_label(app, from, b)
        })
}

/// The source's name as a clickable word: a click selects the source, a right-click
/// opens `route`'s menu. Returns the response.
fn name_cell(
    sh: &mut Sheet<'_>,
    app: &CantusApp,
    rect: Rect,
    host: StripId,
    from: StripId,
    route: Route,
    dim: bool,
) -> egui::Response {
    let resp = sh.ui.interact(
        rect,
        Id::new(("insp-recv-name", host.0, from.0, kind_tag(route))),
        Sense::click(),
    );
    let s = sh.s;
    let ink = if resp.hovered() {
        s.t.text
    } else if dim {
        s.t.text_dim.gamma_multiply(0.6)
    } else {
        s.t.text_dim
    };
    let name = sheet::name(name_of(sh, app, host, from));
    let tx = Text::new(&name, Role::Body, ink)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::Middle));
    let _ = text::paint(sh.b, rect, &tx, s.tk());
    if resp.hovered() {
        let what = sh.fmt.tr("routing.receives.select");
        hint::set_text(
            sh.ui.ctx(),
            &name,
            what,
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    if resp.clicked() {
        route_menu::defer(
            sh.ui.ctx(),
            route_menu::inspector_owner(),
            Pick::Select(from),
        );
    }
    if resp.secondary_clicked() {
        open_menu(sh, &resp, route);
    }
    resp
}

fn kind_tag(r: Route) -> u8 {
    match r.kind {
        cantus_core::routing::RouteKind::Output => 0,
        cantus_core::routing::RouteKind::Send => 1,
        cantus_core::routing::RouteKind::Key(_) => 2,
    }
}

fn open_menu(sh: &Sheet<'_>, resp: &egui::Response, route: Route) {
    let at = resp
        .interact_pointer_pos()
        .unwrap_or(resp.rect.left_bottom());
    route_menu::open(sh.ui.ctx(), route_menu::inspector_owner(), route, at);
}

/// A sender: lamp, name (and its chips), level.
fn sender(sh: &mut Sheet<'_>, app: &CantusApp, from: StripId, host: StripId, send: Send) {
    let s = sh.s.function(Function::Space);
    let row = sh.row();
    let r = full_value(sh, &row);
    let gap = s.s(4.0);
    let lamp = Rect::from_min_max(r.min, pos2(r.left() + s.s(16.0), r.bottom()));
    let level_w = (r.width() * 0.34).max(s.s(64.0));
    let level = Rect::from_min_max(pos2(r.right() - level_w, r.top()), r.max);
    let mut name = Rect::from_min_max(
        pos2(lamp.right() + gap, r.top()),
        pos2(level.left() - gap, r.bottom()),
    );
    let route = Route::send(from, host);
    // The lamp: the send on or off, its level kept.
    let lr = sh.ui.interact(
        lamp,
        Id::new(("insp-recv-on", host.0, from.0)),
        Sense::click(),
    );
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
        let n = sh.fmt.tr("inspector.send_on");
        hint::set_text(sh.ui.ctx(), n, "", Caps::NONE);
    }
    if lr.clicked() {
        let enabled = !send.enabled;
        sh.acts.push(set(from, Send { enabled, ..send }));
        sh.acts.push(Act::EndGesture);
    }
    // Chips at the name's right: PRE on a pre-fader send, ×2 on a doubled one.
    let doubled = {
        let stamp = app.bus.revision();
        let key = cache::key(("insp-recv-dbl", host, from));
        let f = sh.fmt.get(key, stamp, |b| {
            if cantus_core::routing::doubled(app.project(), from, host) {
                b.push('1');
            }
        });
        !f.is_empty()
    };
    let mid = pos2(name.right(), name.center().y);
    let mut right = mid;
    if doubled {
        let w = sheet::chip(
            sh.b,
            right,
            &s,
            Src::Key("routing.receives.twice"),
            Tok::Danger,
            1.0,
        );
        let chip = Rect::from_min_max(pos2(right.x - w, name.top()), pos2(right.x, name.bottom()));
        let cr = sh.ui.interact(
            chip,
            Id::new(("insp-recv-dbl", host.0, from.0)),
            Sense::hover(),
        );
        if cr.hovered() {
            let text =
                crate::routing_actions::doubled_text(app.project(), from, host).unwrap_or_default();
            let n = sheet::name(sh.fmt.tr("routing.receives.twice_name"));
            hint::set_text(
                sh.ui.ctx(),
                &n,
                &text,
                Caps {
                    warning: true,
                    ..Caps::NONE
                },
            );
        }
        right.x -= w + gap;
    }
    if send.tap == SendTap::Pre {
        let w = sheet::chip(
            sh.b,
            right,
            &s,
            Src::Key(send.tap.label_key()),
            Tok::Label,
            1.0,
        );
        right.x -= w + gap;
    }
    name.max.x = right.x.max(name.left() + s.s(12.0));
    name_cell(sh, app, name, host, from, route, !send.enabled);
    // The level, merged per gesture with the source's own sheet and the desk.
    let lname = sheet::name(sh.fmt.tr("inspector.send_level"));
    let d = sh.value(
        level,
        Id::new(("insp-recv-level", host.0, from.0)),
        send.level,
        &Range::lin(0.0, 1.0, 1.0),
        false,
        &lname,
        sheet::level_db,
        Function::Space,
        !send.enabled,
    );
    if let Some(v) = d.value {
        sh.acts.push(set(from, Send { level: v, ..send }));
    }
    if d.ended {
        sh.acts.push(Act::EndGesture);
    }
    if d.resp.secondary_clicked() {
        open_menu(sh, &d.resp, route);
    }
}

fn set(from: StripId, send: Send) -> Act {
    Act::Merge(
        Command::SetStripSend { id: from, send },
        MergeKey::of(&("send", from.0, send.to.0)),
    )
}

/// A strip whose output is this one: its name and "output" in the value's place.
fn routed(sh: &mut Sheet<'_>, app: &CantusApp, from: StripId, host: StripId) {
    let s = sh.s;
    let row = sh.row();
    let r = full_value(sh, &row);
    let w = (r.width() * 0.34).max(s.s(64.0));
    let value = Rect::from_min_max(pos2(r.right() - w, r.top()), r.max);
    let name = Rect::from_min_max(
        pos2(r.left() + s.s(20.0), r.top()),
        pos2(value.left() - s.s(4.0), r.bottom()),
    );
    let route = Route::output(from, host);
    name_cell(sh, app, name, host, from, route, false);
    let word = sheet::name(sh.fmt.tr("routing.receives.output"));
    let vr = sh.ui.interact(
        value,
        Id::new(("insp-recv-out", host.0, from.0)),
        Sense::click(),
    );
    crate::kit::well::readout(sh.b, value, &s, State::of(&vr, false), &word, false);
    if vr.hovered() {
        let tip = sh.fmt.tr("routing.receives.output_tip");
        hint::set_text(
            sh.ui.ctx(),
            &word,
            tip,
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    if vr.clicked() || vr.secondary_clicked() {
        open_menu(sh, &vr, route);
    }
}

/// An insert of this strip keyed from another: its name on the left (the source), and
/// "Comp, slot 2 ← key" — the chain line's short name first, so a narrow sheet cuts the
/// end, never the middle.
fn keyed(sh: &mut Sheet<'_>, app: &CantusApp, from: StripId, host: StripId, effect: EffectId) {
    let s = sh.s.function(Function::Dynamics);
    let row = sh.row();
    let r = full_value(sh, &row);
    let w = (r.width() * 0.44).max(s.s(80.0));
    let value = Rect::from_min_max(pos2(r.right() - w, r.top()), r.max);
    let name = Rect::from_min_max(
        pos2(r.left() + s.s(20.0), r.top()),
        pos2(value.left() - s.s(4.0), r.bottom()),
    );
    let route = Route::key(from, host, effect);
    name_cell(sh, app, name, host, from, route, false);
    let stamp = app.bus.revision() ^ ((crate::i18n::locale() as u64) << 60);
    let fx = sh
        .fmt
        .get(cache::key(("insp-recv-key", host, effect)), stamp, |b| {
            let p = app.project();
            if let Some((i, e)) = p
                .effects_of(host)
                .and_then(|v| v.iter().enumerate().find(|(_, e)| e.id == effect))
            {
                b.push_str(&crate::i18n::tf(
                    "routing.receives.key",
                    &[
                        ("n", &(i + 1)),
                        ("effect", &crate::views::mixer::desk::short_name(e)),
                    ],
                ));
            }
        });
    let fx = sheet::name(fx);
    let vr = sh.ui.interact(
        value,
        Id::new(("insp-recv-keyfx", host.0, effect.0)),
        Sense::click(),
    );
    let st = State::of(&vr, false);
    crate::kit::well::paint(sh.b, value, &s, st, None);
    let words = Text::new(&fx, Role::Body, s.t.text.gamma_multiply(st.alpha()))
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let _ = text::paint(
        sh.b,
        value.shrink2(egui::vec2(s.s(6.0), 0.0)),
        &words,
        s.tk(),
    );
    if vr.hovered() {
        let tip = sh.fmt.tr("routing.receives.key_tip");
        hint::set_text(
            sh.ui.ctx(),
            &fx,
            tip,
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    if vr.clicked() {
        sh.acts.push(Act::OpenEditor(host, effect));
    }
    if vr.secondary_clicked() {
        open_menu(sh, &vr, route);
    }
}
