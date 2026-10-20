//! *Modulate…* (ADR 0057): the list a parameter's menu opens beside its control.
//!
//! - **The modulators already there**, checked when they drive this parameter: a click
//!   connects one (at a quarter of the travel) or disconnects it — the host's own first
//!   (the track the parameter is on, its strip, the master), then every other.
//! - **New modulator**: an LFO, an envelope, a follower, a step sequence or a random source
//!   on the parameter's own host, already driving it — one undo step, as fast as Bitwig's
//!   drag of a handle.
//! - **Bake to automation**, for each modulator that drives it.
//!
//! Esc or a click outside closes it; so does a choice.

use super::{host_for, mod_name};
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::menu::{Check, Item};
use crate::kit::text::Src;
use crate::kit::Style;
use cantus_core::modulator::{ModHost, ModKind};
use cantus_core::{AutoTarget, ModulatorId};
use egui::{Context, Id, Order};

/// What a row does.
#[derive(Clone, Debug, PartialEq)]
enum Row {
    Toggle(ModulatorId),
    New(ModKind),
    Bake(ModulatorId),
    Nothing,
}

/// One row of the popup: what it does, its label (owned: names are the project's), its
/// check, and whether it is a band.
type PopRow = (Row, String, Check, bool);

/// The rows for `target` in three parts: the modulators to connect (scrolled when many),
/// the kinds to make, the routes to bake (always in view).
fn rows(app: &CantusApp, target: &AutoTarget) -> [Vec<PopRow>; 3] {
    let p = app.project();
    let home = host_for(p, target, app.selected_track);
    let mut mine = Vec::new();
    let mut others = Vec::new();
    for (host, m) in p.all_modulators() {
        // A modulator's own amount is not a target it can take.
        if *target == AutoTarget::ModAmount(m.id) {
            continue;
        }
        let on = m.route(target).is_some();
        let label = match mod_name(p, m.id) {
            Some(n) if host == home => n,
            Some(n) => format!("{n} \u{b7} {}", host_label(app, host)),
            None => continue,
        };
        let row = (Row::Toggle(m.id), label, Check::Radio(on), false);
        if host == home {
            mine.push(row);
        } else {
            others.push(row);
        }
    }
    let mut connect = vec![(Row::Nothing, t("mods.popup.connect"), Check::None, true)];
    if mine.is_empty() && others.is_empty() {
        connect.push((Row::Nothing, t("mods.popup.none"), Check::None, false));
    }
    connect.extend(mine);
    connect.extend(others);
    let mut new = vec![(Row::Nothing, t("mods.popup.new"), Check::None, true)];
    for k in ModKind::ALL {
        new.push((Row::New(k), t(k.name_key()), Check::None, false));
    }
    let mut bake = Vec::new();
    let baking: Vec<_> = p.routes_into(target).map(|(m, _)| m.id).collect::<Vec<_>>();
    if !baking.is_empty() {
        bake.push((Row::Nothing, t("mods.popup.bake"), Check::None, true));
        for id in baking {
            if let Some(n) = mod_name(p, id) {
                bake.push((Row::Bake(id), n, Check::None, false));
            }
        }
    }
    [connect, new, bake]
}

/// The items of some rows, with an optional title band first.
fn items<'a>(title: Option<&'a str>, rows: &'a [PopRow]) -> Vec<Item<'a>> {
    title
        .map(|t| Item::Band(Src::Str(t)))
        .into_iter()
        .chain(rows.iter().map(|(row, label, check, band)| {
            if *band {
                Item::Band(Src::Str(label))
            } else {
                Item::Action {
                    label: Src::Str(label),
                    check: *check,
                    shortcut: None,
                    submenu: false,
                    enabled: *row != Row::Nothing,
                }
            }
        }))
        .collect()
}

/// A host in words: the track's or strip's name, or the master.
pub fn host_label(app: &CantusApp, host: ModHost) -> String {
    let p = app.project();
    match host {
        ModHost::Track(id) => p.track(id).map(|x| x.name.clone()).unwrap_or_default(),
        ModHost::Strip(id) => crate::automation::target_parts(p, &AutoTarget::StripGain(id)).0,
        ModHost::Master => t("kit.mixer.master"),
    }
}

/// The popup, while one is open: drawn over everything, beside the control it is for.
pub fn frame(app: &mut CantusApp, ctx: &Context) {
    app.validate_modulators();
    app.poll_bake();
    add_menu(app, ctx);
    setting_list(app, ctx);
    let Some((target, mut anchor)) = app.modulators.popup.clone() else {
        return;
    };
    if anchor == egui::Rect::NOTHING {
        // Opened from a menu: beside the pointer, where it was.
        let at = ctx.pointer_latest_pos().unwrap_or_default();
        anchor = egui::Rect::from_min_size(at, egui::Vec2::ZERO);
        app.modulators.popup = Some((target.clone(), anchor));
    }
    let s = Style::current();
    let [connect, new, bake] = rows(app, &target);
    let title = app.target_name(&target);
    let width = 240.0;
    let mut clicked = None;
    // The modulators to connect scroll when there are many (every track's, the strips',
    // the master's); the title, the kinds to make and the bakes stay in view.
    let screen = ctx.content_rect().height();
    let fixed = crate::kit::menu::height(&items(Some(&title), &[]), s.d(1.0))
        + crate::kit::menu::height(&items(None, &new), s.d(1.0))
        + crate::kit::menu::height(&items(None, &bake), s.d(1.0));
    let room = (screen - fixed - s.s(48.0)).max(s.s(120.0));
    // Room for all of it, not egui's default area size (400 points tall): see `mixer_menu`.
    let area = egui::Area::new(Id::new("mod-popup"))
        .order(Order::Foreground)
        .fixed_pos(egui::pos2(anchor.right() + s.s(4.0), anchor.top()))
        .default_size(egui::vec2(s.s(width) + s.s(16.0), screen))
        .constrain(true)
        .show(ctx, |ui| {
            crate::kit::automate::frame().show(ui, |ui| {
                let head = items(Some(&title), &[]);
                crate::kit::widgets::menu_list(ui, &head, width);
                egui::ScrollArea::vertical()
                    .id_salt("mod-popup-list")
                    .max_height(room)
                    .show(ui, |ui| {
                        let list = items(None, &connect);
                        if let Some(i) = crate::kit::widgets::menu_list(ui, &list, width).1.clicked
                        {
                            clicked = connect.get(i).map(|x| x.0.clone());
                        }
                    });
                for part in [&new, &bake] {
                    if part.is_empty() {
                        continue;
                    }
                    let list = items(None, part);
                    if let Some(i) = crate::kit::widgets::menu_list(ui, &list, width).1.clicked {
                        clicked = part.get(i).map(|x| x.0.clone());
                    }
                }
            });
        });
    let outside = ctx.input(|i| i.pointer.any_pressed())
        && !area
            .response
            .rect
            .contains(ctx.pointer_interact_pos().unwrap_or_default())
        && !anchor.contains(ctx.pointer_interact_pos().unwrap_or_default());
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) || outside {
        app.modulators.popup = None;
        return;
    }
    let Some(row) = clicked else {
        return;
    };
    app.modulators.popup = None;
    match row {
        Row::Toggle(id) => {
            let on = app
                .project()
                .modulator(id)
                .is_some_and(|(_, _, m)| m.route(&target).is_some());
            if on {
                app.disconnect(id, &target);
            } else {
                app.connect(id, target);
            }
        }
        Row::New(kind) => {
            let host = host_for(app.project(), &target, app.selected_track);
            if app.add_modulator(host, kind, Some(target)).is_some() {
                app.reveal_modulators(host);
            }
        }
        Row::Bake(id) => {
            app.bake_route(id, &target);
        }
        Row::Nothing => {}
    }
}

/// The **+** of a host's Modulators band: the kinds, under it.
pub fn add_menu(app: &mut CantusApp, ctx: &Context) {
    let Some((host, anchor)) = app.modulators.add_menu else {
        return;
    };
    let labels: Vec<String> = ModKind::ALL.iter().map(|k| t(k.name_key())).collect();
    let items: Vec<Item<'_>> = std::iter::once(Item::Band(Src::Key("mods.popup.new")))
        .chain(labels.iter().map(|l| Item::action(Src::Str(l))))
        .collect();
    let mut chosen = None;
    let area = egui::Area::new(Id::new("mod-add-menu"))
        .order(Order::Foreground)
        .fixed_pos(anchor.left_bottom())
        .constrain(true)
        .show(ctx, |ui| {
            crate::kit::automate::frame().show(ui, |ui| {
                if let Some(i) = crate::kit::widgets::menu_list(ui, &items, 200.0).1.clicked {
                    chosen = i.checked_sub(1).and_then(|k| ModKind::ALL.get(k).copied());
                }
            });
        });
    if closes(ctx, area.response.rect, anchor) {
        app.modulators.add_menu = None;
        return;
    }
    if let Some(kind) = chosen {
        app.modulators.add_menu = None;
        app.add_modulator(host, kind, None);
    }
}

/// A click outside `area` (and its anchor), or Esc.
fn closes(ctx: &Context, area: egui::Rect, anchor: egui::Rect) -> bool {
    let at = ctx.pointer_interact_pos().unwrap_or_default();
    ctx.input(|i| i.key_pressed(egui::Key::Escape))
        || (ctx.input(|i| i.pointer.any_pressed()) && !area.contains(at) && !anchor.contains(at))
}

/// An edit a list's row makes.
type Edit = Box<dyn Fn(&mut cantus_core::Modulator)>;

/// A setting's list: whose audio a follower (or an audio envelope) hears, whose notes an
/// envelope (or a retriggered LFO) hears.
pub fn setting_list(app: &mut CantusApp, ctx: &Context) {
    use super::ListKind;
    use cantus_core::modulator::{EnvTrigger, Listen, ModSource};
    let Some((id, kind, anchor)) = app.modulators.list else {
        return;
    };
    let Some((_, _, m)) = app.project().modulator(id) else {
        app.modulators.list = None;
        return;
    };
    let m = m.clone();
    let p = app.project();
    let mut rows: Vec<(String, Edit)> = Vec::new();
    match kind {
        ListKind::Listen => {
            let set = |l: Listen| -> Edit {
                Box::new(move |e: &mut cantus_core::Modulator| match &mut e.source {
                    ModSource::Follower(f) => f.listen = l,
                    ModSource::Envelope(x) => {
                        if let EnvTrigger::Audio { listen, .. } = &mut x.trigger {
                            *listen = l;
                        }
                    }
                    _ => {}
                })
            };
            rows.push((t("mods.listen.host"), set(Listen::Host)));
            rows.push((t("kit.mixer.master"), set(Listen::Master)));
            for s in &p.strips {
                let name = crate::automation::target_parts(p, &AutoTarget::StripGain(s.id)).0;
                rows.push((name, set(Listen::Strip(s.id))));
            }
        }
        ListKind::Notes => {
            let set = |tr: Option<cantus_core::TrackId>| -> Edit {
                Box::new(move |e: &mut cantus_core::Modulator| match &mut e.source {
                    ModSource::Lfo(l) => l.notes = tr,
                    ModSource::Envelope(x) => {
                        if let EnvTrigger::Notes { track } = &mut x.trigger {
                            *track = tr;
                        }
                    }
                    _ => {}
                })
            };
            rows.push((t("mods.notes.host"), set(None)));
            for tr in &p.tracks {
                rows.push((tr.name.clone(), set(Some(tr.id))));
            }
        }
    }
    let items: Vec<Item<'_>> = rows
        .iter()
        .map(|(l, _)| Item::action(Src::Str(l)))
        .collect();
    let mut chosen = None;
    let width = anchor.width().max(180.0);
    let area = egui::Area::new(Id::new(("mod-list", id.0)))
        .order(Order::Foreground)
        .fixed_pos(anchor.left_bottom())
        .constrain(true)
        .show(ctx, |ui| {
            crate::kit::automate::frame().show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        chosen = crate::kit::widgets::menu_list(ui, &items, width).1.clicked;
                    });
            });
        });
    if closes(ctx, area.response.rect, anchor) {
        app.modulators.list = None;
        return;
    }
    if let Some((_, f)) = chosen.and_then(|i| rows.get(i)) {
        let mut e = m;
        f(&mut e);
        app.modulators.list = None;
        if e.is_valid() {
            app.edit_modulator(e, None);
        }
    }
}
