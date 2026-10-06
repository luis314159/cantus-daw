//! The inspector's lists, drawn with the kit's menu (FL's rows, a check column, bands).
//! A list is built when it opens — never every frame — and what is chosen runs at once.

use super::{Inspector, Subject, WIDTH};
use crate::app::CantusApp;
use crate::kit::menu::{Check, Item};
use crate::kit::text::Src;
use crate::kit::{widgets, Style};
use cantus_core::{
    Command, EffectId, EffectKind, FirmusParam, InstrumentKind, InstrumentSpec, NativeSpec,
    PadField, PadVoice, Send, StripId, TrackId, Waveform,
};
use egui::{pos2, Id, Key, Order, Ui};

/// Which list is open.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum MenuId {
    /// The panel's own ▸ menu.
    Panel,
    Instrument(TrackId),
    Route(TrackId),
    Output(StripId),
    AddInsert(StripId),
    Slot(StripId, EffectId),
    AddSend(StripId),
    Send(StripId, StripId),
    Sidechain(StripId, EffectId),
    Wave(TrackId, FirmusParam),
    PadVoice(TrackId, usize),
    Choke(TrackId, usize),
    AddPad(TrackId),
}

/// What a row of a list does.
#[derive(Clone, Debug)]
enum Choice {
    Instrument(Box<InstrumentSpec>),
    Route(Option<StripId>),
    Output(Option<StripId>),
    AddEffect(EffectKind),
    Open,
    Up,
    Down,
    Bypass,
    Remove,
    AddSend(StripId),
    SendToggle,
    SendRemove,
    ShowStrip(StripId),
    Sidechain(Option<StripId>),
    Wave(Waveform),
    Voice(PadVoice),
    Choke(u8),
    AddPad(PadVoice),
    ResetWidth,
    Collapse,
    /// Round 20: open the device picker — swap this insert, or add from every effect.
    Swap,
    Browse,
}

/// One row, owning its words until the list closes.
enum Label {
    Key(&'static str),
    Text(String),
}

struct Entry {
    label: Label,
    check: Check,
    enabled: bool,
    band: bool,
    choice: Option<Choice>,
}

fn act(label: Label, check: Check, choice: Choice) -> Entry {
    Entry {
        label,
        check,
        enabled: true,
        band: false,
        choice: Some(choice),
    }
}

fn band(key: &'static str) -> Entry {
    Entry {
        label: Label::Key(key),
        check: Check::None,
        enabled: false,
        band: true,
        choice: None,
    }
}

fn radio(on: bool) -> Check {
    Check::Radio(on)
}

/// "3 · Reverb", or "Strip 3" for a strip with no name.
fn strip_entry(app: &CantusApp, id: StripId) -> String {
    let mut s = String::new();
    let n = app.project().strip_number(id).unwrap_or(0);
    let name = &app
        .project()
        .strip(id)
        .map(|x| x.name.clone())
        .unwrap_or_default();
    if name.is_empty() {
        super::strip_label(app, id, &mut s);
        if app.project().tracks_on_strip(id).next().is_some() {
            s = format!("{n} · {s}");
        }
    } else {
        s = format!("{n} · {name}");
    }
    s
}

fn build(id: MenuId, app: &CantusApp) -> Vec<Entry> {
    let p = app.project();
    let mut out = Vec::new();
    match id {
        MenuId::Panel => {
            out.push(act(
                Label::Key("inspector.reset_width"),
                Check::None,
                Choice::ResetWidth,
            ));
            out.push(act(
                Label::Key("inspector.collapse"),
                Check::None,
                Choice::Collapse,
            ));
        }
        MenuId::Instrument(t) => {
            let cur = p.track(t).map(|t| &t.instrument);
            let firmus = cur.is_some_and(|c| c.firmus().is_some());
            let kit = cur.is_some_and(|c| c.pulsus().is_some());
            let native = cur.and_then(|c| c.native().map(|n| n.kind));
            out.push(act(
                Label::Key("instrument.firmus"),
                radio(firmus),
                Choice::Instrument(Box::new(InstrumentSpec::Firmus(Default::default()))),
            ));
            out.push(act(
                Label::Key("instrument.pulsus"),
                radio(kit),
                Choice::Instrument(Box::new(InstrumentSpec::Pulsus(Default::default()))),
            ));
            // The native instruments (ADR 0023): the finished ones, and in a debug build
            // the scaffolds too, so an instrument can be tried while it is being built.
            let kinds: Vec<InstrumentKind> = if cfg!(debug_assertions) {
                InstrumentKind::ALL.to_vec()
            } else {
                InstrumentKind::ready_kinds().collect()
            };
            for kind in kinds {
                out.push(act(
                    Label::Key(kind.name_key()),
                    radio(native == Some(kind)),
                    Choice::Instrument(Box::new(InstrumentSpec::Native(NativeSpec::new(kind)))),
                ));
            }
        }
        MenuId::Route(t) => {
            let cur = p.track(t).and_then(|t| t.strip);
            out.push(act(
                Label::Key("mixer.master"),
                radio(cur.is_none()),
                Choice::Route(None),
            ));
            if let Some(free) = p.first_free_strip() {
                out.push(act(
                    Label::Key("track.route_free"),
                    Check::None,
                    Choice::Route(Some(free)),
                ));
            }
            for s in &p.strips {
                out.push(act(
                    Label::Text(strip_entry(app, s.id)),
                    radio(cur == Some(s.id)),
                    Choice::Route(Some(s.id)),
                ));
            }
        }
        MenuId::Output(me) => {
            let cur = p.strip(me).and_then(|s| s.output);
            out.push(act(
                Label::Key("mixer.master"),
                radio(cur.is_none()),
                Choice::Output(None),
            ));
            for s in p
                .strips
                .iter()
                .filter(|s| s.id != me && !p.routing_would_cycle(me, Some(s.id)))
            {
                out.push(act(
                    Label::Text(strip_entry(app, s.id)),
                    radio(cur == Some(s.id)),
                    Choice::Output(Some(s.id)),
                ));
            }
        }
        MenuId::AddInsert(_) => {
            out.push(act(Label::Key("swap.menu.browse_effects"), Check::None, Choice::Browse));
            out.push(band("inspector.effects"));
            for kind in EffectKind::ALL {
                out.push(act(
                    Label::Key(kind.name_key()),
                    Check::None,
                    Choice::AddEffect(kind),
                ));
            }
        }
        MenuId::Slot(strip, e) => {
            let Some(s) = p.effects_of(strip) else {
                return out;
            };
            let i = s.iter().position(|x| x.id == e).unwrap_or(0);
            let slot = s.get(i).filter(|x| x.id == e);
            if slot.is_some_and(|x| x.spec.kind.has_editor()) {
                out.push(act(Label::Key("eq.open"), Check::None, Choice::Open));
            }
            out.push(act(Label::Key("swap.menu.swap_effect"), Check::None, Choice::Swap));
            let mut up = act(Label::Key("effect.up"), Check::None, Choice::Up);
            up.enabled = i > 0;
            out.push(up);
            let mut down = act(Label::Key("effect.down"), Check::None, Choice::Down);
            down.enabled = i + 1 < s.len();
            out.push(down);
            let on = slot.is_some_and(|x| x.bypass);
            out.push(act(
                Label::Key("effect.bypass"),
                if on { Check::On } else { Check::Off },
                Choice::Bypass,
            ));
            out.push(act(
                Label::Key("effect.remove"),
                Check::None,
                Choice::Remove,
            ));
        }
        MenuId::AddSend(me) => {
            let Some(s) = p.strip(me) else { return out };
            for t in p.strips.iter().filter(|t| {
                t.id != me && s.send_to(t.id).is_none() && !p.routing_would_cycle(me, Some(t.id))
            }) {
                out.push(act(
                    Label::Text(strip_entry(app, t.id)),
                    Check::None,
                    Choice::AddSend(t.id),
                ));
            }
            if out.is_empty() {
                let mut none = act(Label::Key("mixer.send_none"), Check::None, Choice::Collapse);
                none.enabled = false;
                none.choice = None;
                out.push(none);
            }
        }
        MenuId::Send(me, to) => {
            let on = p
                .strip(me)
                .and_then(|s| s.send_to(to))
                .is_some_and(|s| s.enabled);
            out.push(act(
                Label::Key("inspector.send_on"),
                if on { Check::On } else { Check::Off },
                Choice::SendToggle,
            ));
            out.push(act(
                Label::Key("inspector.show_strip"),
                Check::None,
                Choice::ShowStrip(to),
            ));
            out.push(act(
                Label::Key("mixer.send_remove"),
                Check::None,
                Choice::SendRemove,
            ));
        }
        MenuId::Sidechain(me, e) => {
            let cur = p.effect(me, e).and_then(|x| x.sidechain);
            out.push(act(
                Label::Key("kit.inspector.none"),
                radio(cur.is_none()),
                Choice::Sidechain(None),
            ));
            for s in p
                .strips
                .iter()
                .filter(|s| s.id != me && !p.routing_would_cycle(s.id, Some(me)))
            {
                out.push(act(
                    Label::Text(strip_entry(app, s.id)),
                    radio(cur == Some(s.id)),
                    Choice::Sidechain(Some(s.id)),
                ));
            }
        }
        MenuId::Wave(t, param) => {
            let cur = p
                .track(t)
                .and_then(|t| t.instrument.firmus())
                .map(|f| param.get(f));
            for w in Waveform::ALL {
                out.push(act(
                    Label::Key(w.label_key()),
                    radio(cur == Some(w.index() as f32)),
                    Choice::Wave(w),
                ));
            }
        }
        MenuId::PadVoice(t, pad) => {
            let cur = p
                .track(t)
                .and_then(|t| t.instrument.pulsus())
                .and_then(|k| k.pad(pad))
                .map(|p| p.voice);
            for v in PadVoice::ALL {
                out.push(act(
                    Label::Key(v.label_key()),
                    radio(cur == Some(v)),
                    Choice::Voice(v),
                ));
            }
        }
        MenuId::Choke(t, pad) => {
            let cur = p
                .track(t)
                .and_then(|t| t.instrument.pulsus())
                .and_then(|k| k.pad(pad))
                .map(|p| p.choke);
            for g in 0u8..=4 {
                let label = if g == 0 {
                    Label::Key("pad.none")
                } else {
                    Label::Text(crate::i18n::tf("pad.group", &[("n", &g)]))
                };
                out.push(act(label, radio(cur == Some(g)), Choice::Choke(g)));
            }
        }
        MenuId::AddPad(_) => {
            for v in PadVoice::ALL {
                out.push(act(
                    Label::Key(v.label_key()),
                    Check::None,
                    Choice::AddPad(v),
                ));
            }
        }
    }
    out
}

/// Show the open list, if any, under its field; run what is chosen.
pub fn show(st: &mut Inspector, app: &mut CantusApp, ui: &mut Ui, s: &Style) {
    let Some(open) = st.menu else {
        st.menu_seen = None;
        return;
    };
    let ctx = ui.ctx().clone();
    if st.menu_seen != Some(open.id) {
        st.menu_seen = Some(open.id);
        st.menu_pass = ctx.cumulative_pass_nr();
    }
    let entries = build(open.id, app);
    let items: Vec<Item<'_>> = entries
        .iter()
        .map(|e| {
            let label = match &e.label {
                Label::Key(k) => Src::Key(k),
                Label::Text(t) => Src::Str(t),
            };
            if e.band {
                Item::Band(label)
            } else {
                Item::Action {
                    label,
                    check: e.check,
                    shortcut: None,
                    submenu: false,
                    enabled: e.enabled,
                }
            }
        })
        .collect();
    let width = open.anchor.width().max(s.s(180.0));
    let inserts = |app: &CantusApp| match open.id {
        MenuId::AddInsert(sid) => app.project().effects_of(sid).map(|x| x.len()),
        _ => None,
    };
    let before = inserts(app);
    let area = egui::Area::new(Id::new("inspector-menu"))
        .order(Order::Foreground)
        .fixed_pos(pos2(open.anchor.left(), open.anchor.bottom() + s.s(2.0)))
        .constrain(true)
        .show(&ctx, |ui| {
            let (_, clicks) = widgets::menu_list(ui, &items, width);
            if let MenuId::AddInsert(strip) = open.id {
                // Plug-ins made to the market's standards (ADR 0019): the host's own list.
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(width - s.s(14.0));
                    crate::views::plugins::insert_menu(app, ui, strip);
                });
            }
            clicks
        });
    let mut close = false;
    if let Some(i) = area.inner.clicked {
        if let Some(c) = entries.get(i).and_then(|e| e.choice.clone()) {
            choose(st, app, open.id, c);
        }
        close = true;
    }
    // A plug-in chosen from the host's list was added there and then.
    if inserts(app) != before {
        close = true;
    }
    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        close = true;
    }
    if area.response.clicked_elsewhere() && ctx.cumulative_pass_nr() > st.menu_pass {
        close = true;
    }
    if close && st.menu.is_some_and(|m| m.id == open.id) {
        st.menu = None;
    }
}

fn choose(st: &mut Inspector, app: &mut CantusApp, id: MenuId, c: Choice) {
    match (id, c) {
        (MenuId::Panel, Choice::ResetWidth) => st.width = WIDTH,
        (MenuId::Panel, Choice::Collapse) => st.collapsed = true,
        (MenuId::Instrument(t), Choice::Instrument(spec)) => {
            let same = app.project().track(t).is_some_and(|x| {
                std::mem::discriminant(&x.instrument) == std::mem::discriminant(&*spec)
                    && x.instrument.native().map(|n| n.kind) == spec.native().map(|n| n.kind)
            });
            if !same {
                app.set_instrument(t, *spec);
            }
        }
        (MenuId::Route(t), Choice::Route(strip)) => {
            if app.project().track(t).map(|x| x.strip) != Some(strip) {
                app.route_track(t, strip);
            }
        }
        (MenuId::Output(me), Choice::Output(output)) => {
            app.exec(Command::SetStripOutput { id: me, output });
        }
        (MenuId::AddInsert(strip), Choice::AddEffect(kind)) => app.add_effect(strip, kind),
        (MenuId::AddInsert(strip), Choice::Browse) => {
            app.open_picker(crate::picker::PickPurpose::AddEffect { strip })
        }
        (MenuId::Slot(strip, e), c) => {
            let i = app
                .project()
                .effects_of(strip)
                .and_then(|s| s.iter().position(|x| x.id == e))
                .unwrap_or(0);
            match c {
                Choice::Open => app.open_effect_editor(strip, e),
                Choice::Swap => {
                    app.open_picker(crate::picker::PickPurpose::ReplaceEffect { strip, effect: e })
                }
                Choice::Up if i > 0 => {
                    app.exec(Command::MoveEffect {
                        strip,
                        effect: e,
                        index: i - 1,
                    });
                }
                Choice::Down => {
                    app.exec(Command::MoveEffect {
                        strip,
                        effect: e,
                        index: i + 1,
                    });
                }
                Choice::Bypass => {
                    let on = app.project().effect(strip, e).is_some_and(|x| x.bypass);
                    app.exec(Command::SetEffectBypass {
                        strip,
                        effect: e,
                        bypass: !on,
                    });
                }
                Choice::Remove => app.remove_effect(strip, e),
                _ => {}
            }
        }
        (MenuId::AddSend(me), Choice::AddSend(to)) => {
            app.set_strip_send(me, Send::new(to));
            app.end_gesture();
        }
        (MenuId::Send(me, to), c) => match c {
            Choice::SendToggle => {
                if let Some(send) = app.project().strip(me).and_then(|s| s.send_to(to)).copied() {
                    app.set_strip_send(
                        me,
                        Send {
                            enabled: !send.enabled,
                            ..send
                        },
                    );
                    app.end_gesture();
                }
            }
            Choice::ShowStrip(s) => {
                app.select_strip(s);
                app.inspector.follow(Subject::Strip(s));
            }
            Choice::SendRemove => app.remove_strip_send(me, to),
            _ => {}
        },
        (MenuId::Sidechain(strip, effect), Choice::Sidechain(source)) => {
            app.exec(Command::SetEffectSidechain {
                strip,
                effect,
                source,
            });
        }
        (MenuId::Wave(t, param), Choice::Wave(w)) => {
            app.exec(Command::SetInstrumentParam {
                track: t,
                param: param.into(),
                value: w.index() as f32,
            });
        }
        (MenuId::PadVoice(t, pad), Choice::Voice(v)) => {
            if let Some(c) = super::track::pad_cmd(app, t, pad, PadField::Voice, v.index() as f32) {
                app.exec(c);
            }
        }
        (MenuId::Choke(t, pad), Choice::Choke(g)) => {
            if let Some(c) = super::track::pad_cmd(app, t, pad, PadField::Choke, g as f32) {
                app.exec(c);
            }
        }
        (MenuId::AddPad(t), Choice::AddPad(v)) => app.add_pad(t, v),
        _ => {}
    }
}

/// Which strip a track feeds, as a menu field with its list (the channel rack uses it).
pub fn route_picker(app: &mut CantusApp, ui: &mut Ui, id: TrackId, current: Option<StripId>) {
    let label = match current.and_then(|s| app.project().strip_number(s)) {
        Some(n) => crate::i18n::tf("track.route_strip", &[("n", &n)]),
        None => crate::i18n::t("mixer.master"),
    };
    let name = crate::i18n::t("track.route_tip");
    let (resp, _) = widgets::menu_field(ui, &label, false, false, &name, 96.0);
    let popup = egui::Popup::menu(&resp).id(Id::new(("insp-route-picker", id.0)));
    let entries = if popup.is_open() || resp.clicked() {
        build(MenuId::Route(id), app)
    } else {
        Vec::new()
    };
    let items: Vec<Item<'_>> = entries
        .iter()
        .map(|e| Item::Action {
            label: match &e.label {
                Label::Key(k) => Src::Key(k),
                Label::Text(t) => Src::Str(t),
            },
            check: e.check,
            shortcut: None,
            submenu: false,
            enabled: e.enabled,
        })
        .collect();
    let chosen = popup
        .show(|ui| widgets::menu_list(ui, &items, 180.0).1.clicked)
        .and_then(|r| r.inner);
    if let Some(Choice::Route(strip)) = chosen
        .and_then(|i| entries.get(i))
        .and_then(|e| e.choice.clone())
    {
        if strip != current {
            app.route_track(id, strip);
        }
    }
}
