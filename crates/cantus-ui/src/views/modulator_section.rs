//! The **Modulators** section of the inspector (ADR 0057): a panel per host — the track,
//! its channel, the master — in the sheet's row grammar.
//!
//! Each modulator is a title row — its handle (drag it onto any knob to connect it), its
//! name and kind in the kind's hue, its live output, *map* (Bitwig's mapping mode: then a
//! drag on any control sets its depth there), on/off, remove — and, unfolded, its source's
//! rows (a picture of its movement drawn from the engine's own functions, then its
//! settings), its amount, and one row per parameter it drives: the target and the depth
//! (drag it; double-click resets to 0), *bake* and *disconnect*. The band's **+** adds one.
//!
//! Rows act after painting (`Act::Mod`), one command each, a drag merged into one step.

#[path = "modulator_rows.rs"]
mod rows;

use super::sheet::{self, Range, Sheet};
use super::Act;
use crate::app::CantusApp;
use crate::kit::automate::ModHandle;
use crate::kit::glyph::{self, Glyph};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{cache, led, Function, State, Tok};
use crate::views::modulator::{function_of, mod_name, paint};
use cantus_core::modulator::{ModHost, Modulator};
use cantus_core::{AutoTarget, MergeKey, ModulatorId};
use egui::{pos2, Align2, Id, Rect, Sense, Vec2};

/// What a row of the section asks for.
#[derive(Clone, Debug)]
pub enum ModAct {
    /// The band's +: the list of kinds, under `Rect`.
    AddMenu(ModHost, Rect),
    /// A modulator edited whole (merged under the key while a drag goes on).
    Set(Box<Modulator>, Option<MergeKey>),
    Remove(ModulatorId),
    Map(ModulatorId),
    Fold(ModulatorId),
    Depth(ModulatorId, AutoTarget, f32),
    Disconnect(ModulatorId, AutoTarget),
    Bake(ModulatorId, AutoTarget),
    /// A list for one of its settings (whose audio, whose notes), under `Rect`.
    List(ModulatorId, crate::views::modulator::ListKind, Rect),
    /// A parameter row's menu or modulation gesture (`param_row`).
    Param(AutoTarget, crate::kit::automate::Choice, Rect),
}

/// Run what a row asked for.
pub fn apply(app: &mut CantusApp, act: ModAct) {
    match act {
        ModAct::AddMenu(host, r) => app.modulators.add_menu = Some((host, r)),
        ModAct::Set(m, key) => {
            app.edit_modulator(*m, key);
        }
        ModAct::Remove(id) => {
            app.remove_modulator(id);
        }
        ModAct::Map(id) => app.toggle_mapping(id),
        ModAct::Fold(id) => {
            if !app.modulators.folded.remove(&id) {
                app.modulators.folded.insert(id);
            }
        }
        ModAct::Depth(id, target, d) => app.set_route_depth(id, &target, d, true),
        ModAct::Disconnect(id, target) => {
            app.disconnect(id, &target);
        }
        ModAct::Bake(id, target) => {
            app.bake_route(id, &target);
        }
        ModAct::List(id, kind, r) => app.modulators.list = Some((id, kind, r)),
        ModAct::Param(target, c, r) => match c {
            crate::kit::automate::Choice::Automate => app.automate(target),
            _ => app.modulation_choice(&target, c, r),
        },
    }
}

/// The hosts a subject shows modulators for: the track and its channel, a channel, the
/// master.
pub fn hosts(app: &CantusApp, subject: super::Subject) -> Vec<(ModHost, &'static str)> {
    let p = app.project();
    match subject {
        super::Subject::Track(t) | super::Subject::Clip(t, _) | super::Subject::Notes(t, _) => {
            let mut v = vec![(ModHost::Track(t), "mods.band.track")];
            if let Some(s) = p.track(t).and_then(|t| t.strip) {
                v.push((ModHost::Strip(s), "mods.band.strip"));
            }
            v
        }
        super::Subject::Strip(s) => vec![(ModHost::Strip(s), "mods.band.strip")],
        super::Subject::Master => vec![(ModHost::Master, "mods.band.master")],
    }
}

/// The section for `host`: its band, then each modulator.
pub fn show(sh: &mut Sheet<'_>, app: &CantusApp, host: ModHost, band_key: &'static str) {
    let Some(list) = app.project().modulators(host) else {
        return;
    };
    let room = list.len() < cantus_core::modulator::MAX_PER_HOST;
    let n = list.len();
    let count = sheet::Count {
        key: cache::key(("mod-count", host)),
        stamp: n as u64,
        write: &|b: &mut String| {
            if n > 0 {
                b.push_str(&n.to_string());
            }
        },
    };
    let band = sh.band(band_key, Function::Mod, None, room, Some(count));
    if let (true, Some(r)) = (band.plus, band.plus_rect) {
        sh.acts.push(Act::Mod(ModAct::AddMenu(host, r)));
    }
    if list.is_empty() {
        empty_row(sh, host);
        return;
    }
    for m in list {
        title(sh, app, m);
        if app.modulators.folded.contains(&m.id) {
            continue;
        }
        rows::source(sh, app, m);
        amount(sh, app, m);
        routes(sh, app, m);
    }
}

/// The amount row: every route at once, 0…100 % — and a parameter of its own (ADR 0057's
/// one level of nesting): its menu's *Modulate…*, another modulator's handle dropped on
/// it, the Alt-drag depth, *Create automation clip*.
fn amount(sh: &mut Sheet<'_>, app: &CantusApp, m: &Modulator) {
    let base = m.clone();
    let key = MergeKey::of(&("mod-field", m.id.0, "amount"));
    param_row(
        sh,
        app,
        "mods.amount",
        Id::new(("mod-num", m.id, "amount")),
        m.amount,
        &Range::lin(0.0, 1.0, 1.0),
        false,
        sheet::percent,
        function_of(m.source.kind()),
        AutoTarget::ModAmount(m.id),
        move |v| {
            let mut e = base;
            e.amount = v;
            Act::Mod(ModAct::Set(Box::new(e), Some(key)))
        },
    );
}

/// No modulator yet: the empty row says what would go there, and opens the kinds.
fn empty_row(sh: &mut Sheet<'_>, host: ModHost) {
    let s = sh.s.function(Function::Mod);
    let row = sh.row();
    let m = s.s(sheet::MARGIN);
    let r = Rect::from_min_max(
        pos2(row.full.left() + m, row.value.top()),
        pos2(row.full.right() - m, row.value.bottom()),
    );
    let resp = sh
        .ui
        .interact(r, Id::new(("mod-empty", host)), Sense::click());
    sheet::placeholder_well(sh.b, r, &s, State::of(&resp, false), "mods.empty");
    if resp.clicked() {
        sh.acts.push(Act::Mod(ModAct::AddMenu(host, r)));
    }
}

/// A modulator's title row: handle, fold, name, live output, map, on/off, remove.
fn title(sh: &mut Sheet<'_>, app: &CantusApp, m: &Modulator) {
    let kind = m.source.kind();
    let s = sh.s.function(function_of(kind));
    sh.ui.add_space(s.d(2.0));
    let row = sh.row();
    let mg = s.s(sheet::MARGIN);
    let (x0, x1) = (row.full.left() + mg, row.full.right() - mg);
    let (y0, y1) = (row.value.top(), row.value.bottom());
    let key = s.s(20.0);
    let gap = s.s(4.0);
    // From the right: remove, on/off, map.
    let remove = Rect::from_min_max(pos2(x1 - key, y0), pos2(x1, y1));
    let power = Rect::from_min_max(
        pos2(remove.left() - gap - key, y0),
        pos2(remove.left() - gap, y1),
    );
    let map = Rect::from_min_max(
        pos2(power.left() - gap - key, y0),
        pos2(power.left() - gap, y1),
    );
    let meter_w = s.s(36.0).min((map.left() - x0) * 0.25);
    let meter = Rect::from_min_max(
        pos2(map.left() - gap - meter_w, y0 + s.s(6.0)),
        pos2(map.left() - gap, y1 - s.s(6.0)),
    );
    let handle = Rect::from_min_max(pos2(x0, y0), pos2(x0 + s.s(14.0), y1));
    let name_r = Rect::from_min_max(pos2(handle.right() + gap, y0), pos2(meter.left() - gap, y1));
    // The handle: drag it onto a control to connect it there (Bitwig).
    let hr = sh
        .ui
        .interact(handle, Id::new(("mod-handle", m.id)), Sense::drag());
    if hr.drag_started() {
        egui::DragAndDrop::set_payload(sh.ui.ctx(), ModHandle(m.id.0));
    }
    if hr.hovered() || hr.dragged() {
        sh.ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        let name = sheet::name(sh.fmt.tr("mods.handle"));
        hint::set_text(sh.ui.ctx(), &name, "", Caps::NONE);
    }
    glyph::paint(
        sh.b,
        handle.shrink(s.s(1.0)),
        Glyph::Grip,
        if hr.hovered() || hr.dragged() {
            s.hue
        } else {
            s.t.text_dim
        },
    );
    // The name, in the kind's hue; a click folds it.
    let nr = sh
        .ui
        .interact(name_r, Id::new(("mod-name", m.id)), Sense::click());
    let name = sh.fmt.get(
        cache::key(("mod-name", m.id)),
        app.bus.revision() ^ ((crate::i18n::locale() as u64) << 60),
        |b| b.push_str(&mod_name(app.project(), m.id).unwrap_or_default()),
    );
    let name = sheet::name(name);
    let folded = app.modulators.folded.contains(&m.id);
    let chevron = Rect::from_min_size(
        pos2(name_r.left(), name_r.center().y - s.s(5.0)),
        Vec2::splat(s.s(10.0)),
    );
    glyph::paint(
        sh.b,
        chevron,
        if folded { Glyph::Menu } else { Glyph::Down },
        s.t.text_dim,
    );
    let tr = Rect::from_min_max(pos2(chevron.right() + s.s(4.0), y0), name_r.right_bottom());
    let ink = if m.muted { s.t.text_dim } else { s.hue };
    let t = Text::new(&name, Role::Body, ink)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let _ = text::paint(sh.b, tr, &t, s.tk());
    if nr.hovered() {
        let tip = sheet::name(sh.fmt.tr(if folded { "mods.unfold" } else { "mods.fold" }));
        hint::set_text(sh.ui.ctx(), &name, &tip, Caps::NONE);
    }
    if nr.clicked() {
        sh.acts.push(Act::Mod(ModAct::Fold(m.id)));
    }
    // Its output now.
    let live = app.engine.modulation().value(m.id).unwrap_or(0.0);
    let v = if m.amount > 0.0 { live / m.amount } else { 0.0 };
    paint::meter(sh.b, meter, &s, v, m.source.bipolar(), s.hue);
    // Map, on/off, remove.
    let mapping = app.modulators.mapping == Some(m.id);
    let keys = [
        (map, Glyph::Link, "mods.map", mapping),
        (power, Glyph::Power, "mods.on", !m.muted),
        (remove, Glyph::Close, "mods.remove", false),
    ];
    for (i, (r, g, tip, lit)) in keys.into_iter().enumerate() {
        let resp = sh
            .ui
            .interact(r, Id::new(("mod-key", m.id, i)), Sense::click());
        if i == 1 {
            led::paint(
                sh.b,
                r.center(),
                s.s(11.0),
                lit,
                &s,
                Tok::Led,
                Tok::PanelLight,
                1.0,
            );
        } else {
            sheet::glyph_key(sh.b, r, &s, State::of(&resp, lit), g);
        }
        if resp.hovered() {
            let name = sheet::name(sh.fmt.tr(tip));
            hint::set_text(sh.ui.ctx(), &name, "", Caps::NONE);
        }
        if resp.clicked() {
            sh.acts.push(Act::Mod(match i {
                0 => ModAct::Map(m.id),
                1 => {
                    let mut e = m.clone();
                    e.muted = !e.muted;
                    ModAct::Set(Box::new(e), None)
                }
                _ => ModAct::Remove(m.id),
            }));
        }
    }
}

/// One row per parameter the modulator drives: the target, its depth, bake, disconnect.
fn routes(sh: &mut Sheet<'_>, app: &CantusApp, m: &Modulator) {
    let s = sh.s.function(function_of(m.source.kind()));
    if m.routes.is_empty() {
        let row = sh.row();
        let mg = s.s(sheet::MARGIN);
        let r = Rect::from_min_max(
            pos2(row.full.left() + mg, row.value.top()),
            pos2(row.full.right() - mg, row.value.bottom()),
        );
        sheet::placeholder_well(sh.b, r, &s, State::REST, "mods.no_routes");
        return;
    }
    for route in &m.routes {
        let row = sh.row();
        let gap = s.s(4.0);
        let key = s.s(20.0);
        let mg = s.s(sheet::MARGIN);
        let x1 = row.full.right() - mg;
        let (y0, y1) = (row.value.top(), row.value.bottom());
        let close = Rect::from_min_max(pos2(x1 - key, y0), pos2(x1, y1));
        let bake = Rect::from_min_max(
            pos2(close.left() - gap - key, y0),
            pos2(close.left() - gap, y1),
        );
        let depth_w = s.s(64.0).min((bake.left() - row.full.left()) * 0.4);
        let depth = Rect::from_min_max(
            pos2(bake.left() - gap - depth_w, y0),
            pos2(bake.left() - gap, y1),
        );
        let label = Rect::from_min_max(
            pos2(row.full.left() + mg, row.full.top()),
            pos2(depth.left() - gap, row.full.bottom()),
        );
        let name = sh.fmt.get(
            cache::key(("mod-route", m.id, &route.target)),
            app.bus.revision() ^ ((crate::i18n::locale() as u64) << 60),
            |b| b.push_str(&app.target_name(&route.target)),
        );
        let name = sheet::name(name);
        let t = Text::new(&name, Role::Body, s.t.text)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::Middle));
        let _ = text::paint(sh.b, label, &t, s.tk());
        let lr = sh.ui.interact(
            label,
            Id::new(("mod-route-name", m.id, &route.target)),
            Sense::hover(),
        );
        if lr.hovered() {
            hint::set_text(sh.ui.ctx(), &name, "", Caps::NONE);
        }
        let dname = sheet::name(sh.fmt.tr("mods.depth"));
        let d = sh.value(
            depth,
            Id::new(("mod-depth", m.id, &route.target)),
            route.depth,
            &Range::lin(-1.0, 1.0, 0.0).bipolar(),
            false,
            &dname,
            |v, b| {
                let _ = std::fmt::Write::write_fmt(b, format_args!("{:+.0} %", v * 100.0));
            },
            function_of(m.source.kind()),
            m.muted,
        );
        if let Some(v) = d.value {
            sh.acts
                .push(Act::Mod(ModAct::Depth(m.id, route.target.clone(), v)));
        }
        if d.ended {
            sh.acts.push(Act::EndGesture);
        }
        for (r, g, tip, act) in [
            (
                bake,
                Glyph::Automate,
                "mods.bake",
                ModAct::Bake(m.id, route.target.clone()),
            ),
            (
                close,
                Glyph::Close,
                "mods.disconnect",
                ModAct::Disconnect(m.id, route.target.clone()),
            ),
        ] {
            let resp = sh.ui.interact(
                r,
                Id::new(("mod-route-key", m.id, &route.target, tip)),
                Sense::click(),
            );
            sheet::glyph_key(sh.b, r, &s, State::of(&resp, false), g);
            if resp.hovered() {
                let n = sheet::name(sh.fmt.tr(tip));
                hint::set_text(sh.ui.ctx(), &n, "", Caps::NONE);
            }
            if resp.clicked() {
                sh.acts.push(Act::Mod(act));
            }
        }
    }
}

/// A label | value row bound to a parameter that can be automated and modulated (ADR 0027,
/// 0057): [`Sheet::param_row`], with the parameter's menu on a right-click (*Create
/// automation clip*, *Modulate…*), Alt+drag for the depth of its modulation, a modulator's
/// handle dropped on it, and the marks and live position of what drives it.
#[allow(clippy::too_many_arguments)]
pub fn param_row(
    sh: &mut Sheet<'_>,
    app: &CantusApp,
    key: &'static str,
    id: Id,
    v: f32,
    range: &Range,
    gain: bool,
    write: impl FnOnce(f32, &mut String),
    f: Function,
    target: AutoTarget,
    act: impl FnOnce(f32) -> Act,
) {
    let row = sh.row();
    sh.label(row.label, key, false);
    crate::kit::automate::probe::note(key, true);
    let name = sheet::name(sh.fmt.tr(key));
    let base = if gain {
        sheet::gain_pos(v)
    } else {
        range.pos(v)
    };
    let mut rings = [(0.0, egui::Color32::TRANSPARENT); crate::views::modulator::MAX_RINGS];
    let (m, n) = app.mod_offer(&target, base, &sh.s, &mut rings);
    let offer = crate::kit::automate::Offer {
        automated: app.is_automated(&target),
        typing: false,
        modulation: Some(m),
    };
    let depth_drag = crate::kit::automate::depth_drag(sh.ui, Some(offer));
    let d = sh.value(row.value, id, v, range, gain, &name, write, f, false);
    if let Some(nv) = d.value.filter(|_| !depth_drag) {
        sh.acts.push(act(nv));
    }
    if d.ended && !depth_drag {
        sh.acts.push(Act::EndGesture);
    }
    let s = sh.s;
    if offer.automated {
        crate::kit::automate::mark(sh.b, row.value, &s);
    }
    if n > 0 {
        crate::kit::automate::mod_mark(sh.b, row.value, &s);
        // Where modulation has it now: a tick on the well's level bar.
        if let Some(live) = m.live {
            let x = row.value.left() + s.s(3.0);
            let y = row.value.bottom() - row.value.height() * live.clamp(0.0, 1.0);
            sh.b.hline(x - s.s(2.0), x + s.s(4.0), y, s.s(2.0), s.t.hue_mod);
        }
    }
    let rect = row.value;
    let choice = crate::kit::automate::modulation_choice(sh.ui, &d.resp, Some(offer))
        .or_else(|| crate::kit::automate::menu(sh.ui, &d.resp, offer));
    if let Some(c) = choice {
        sh.acts.push(Act::Mod(ModAct::Param(target, c, rect)));
    }
}
