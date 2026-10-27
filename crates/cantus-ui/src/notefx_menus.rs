//! The lists round 31's rows open (ADR 0069), drawn over everything beside what opened
//! them: a track's note-effect kinds, a clip's grooves, the song's key (roots beside
//! scales). Esc or a click outside closes one; so does a choice.

use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::menu::{Check, Item};
use crate::kit::text::Src;
use cantus_core::groove::{ClipGroove, GrooveSource};
use cantus_core::notefx::NoteFxType;
use cantus_core::scale::ROOT_NAMES;
use cantus_core::{Key, ScaleKind};
use egui::{Context, Id, Order, Rect};

/// Every list that is open, each frame.
pub fn frame(app: &mut CantusApp, ctx: &Context) {
    add_menu(app, ctx);
    groove_menu(app, ctx);
    key_menu(app, ctx);
    choice_menu(app, ctx);
}

/// A click outside `area` (and its anchor), or Esc.
fn closes(ctx: &Context, area: Rect, anchor: Rect) -> bool {
    let at = ctx.pointer_interact_pos().unwrap_or_default();
    ctx.input(|i| i.key_pressed(egui::Key::Escape))
        || (ctx.input(|i| i.pointer.any_pressed()) && !area.contains(at) && !anchor.contains(at))
}

/// The + of a track's Note effects band: arpeggiator, chords, scale.
fn add_menu(app: &mut CantusApp, ctx: &Context) {
    let Some((track, anchor)) = app.notefx.add_menu else {
        return;
    };
    let labels: Vec<String> = NoteFxType::ALL.iter().map(|k| t(k.label_key())).collect();
    let items: Vec<Item<'_>> = std::iter::once(Item::Band(Src::Key("notefx.add")))
        .chain(labels.iter().map(|l| Item::action(Src::Str(l))))
        .collect();
    let mut chosen = None;
    let area = egui::Area::new(Id::new("notefx-add-menu"))
        .order(Order::Foreground)
        .fixed_pos(anchor.left_bottom())
        .constrain(true)
        .show(ctx, |ui| {
            crate::kit::automate::frame().show(ui, |ui| {
                if let Some(i) = crate::kit::widgets::menu_list(ui, &items, 200.0).1.clicked {
                    chosen = i
                        .checked_sub(1)
                        .and_then(|k| NoteFxType::ALL.get(k).copied());
                }
            });
        });
    if closes(ctx, area.response.rect, anchor) {
        app.notefx.add_menu = None;
        return;
    }
    if let Some(kind) = chosen {
        app.notefx.add_menu = None;
        app.add_note_fx(track, kind);
    }
}

/// A note clip's grooves: none, the built-in swings, the pool's.
fn groove_menu(app: &mut CantusApp, ctx: &Context) {
    let Some((track, clip, anchor)) = app.notefx.groove_menu else {
        return;
    };
    let p = app.project();
    let current = p
        .track(track)
        .and_then(|t| t.clip(clip))
        .and_then(|c| c.groove);
    let choices = crate::notefx_ui::groove_choices(p);
    // Two columns: none and the built-in swings (a band before 8ths and 16ths), then the
    // project's own grooves (or a line saying where they come from).
    let none = t("notefx.groove.none");
    let empty = t("notefx.groove.pool_empty");
    // The pool's grooves to take out: each one no clip plays (review: the pool only grew).
    let pool: Vec<(cantus_core::GrooveId, String, bool)> = p
        .grooves
        .iter()
        .map(|g| (g.id, g.name.clone(), !cantus_core::groove::in_use(p, g.id)))
        .collect();
    let mut left: Vec<Item<'_>> = vec![Item::Action {
        label: Src::Str(&none),
        check: Check::Radio(current.is_none()),
        shortcut: None,
        submenu: false,
        enabled: true,
    }];
    let mut left_rows: Vec<Option<GrooveSource>> = vec![None];
    let mut right: Vec<Item<'_>> = vec![Item::Band(Src::Key("notefx.groove.pool"))];
    let mut right_rows: Vec<Option<GrooveSource>> = vec![None];
    let mut family = None;
    for (src, name) in &choices {
        let item = Item::Action {
            label: Src::Str(name),
            check: Check::Radio(current.is_some_and(|c| c.source == *src)),
            shortcut: None,
            submenu: false,
            enabled: true,
        };
        match src {
            GrooveSource::Pool { .. } => {
                right.push(item);
                right_rows.push(Some(*src));
            }
            GrooveSource::Swing { grid, .. } => {
                let f = match grid {
                    cantus_core::SwingGrid::Eighths => "notefx.groove.band8",
                    cantus_core::SwingGrid::Sixteenths => "notefx.groove.band16",
                };
                if family != Some(f) {
                    family = Some(f);
                    left.push(Item::Band(Src::Key(f)));
                    left_rows.push(None);
                }
                left.push(item);
                left_rows.push(Some(*src));
            }
        }
    }
    if right.len() == 1 {
        right.push(Item::Action {
            label: Src::Str(&empty),
            check: Check::None,
            shortcut: None,
            submenu: false,
            enabled: false,
        });
        right_rows.push(None);
    }
    let removable_from = right.len();
    if !pool.is_empty() {
        right.push(Item::Band(Src::Key("notefx.groove.remove_band")));
        right_rows.push(None);
        for (_, name, free) in &pool {
            right.push(Item::Action {
                label: Src::Str(name),
                check: Check::None,
                shortcut: None,
                submenu: false,
                enabled: *free,
            });
            right_rows.push(None);
        }
    }
    let mut removing = None;
    let mut chosen: Option<Option<GrooveSource>> = None;
    let area = egui::Area::new(Id::new("notefx-groove-menu"))
        .order(Order::Foreground)
        .fixed_pos(anchor.left_bottom())
        .constrain(true)
        .show(ctx, |ui| {
            crate::kit::automate::frame().show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    if let Some(i) = crate::kit::widgets::menu_list(ui, &left, 170.0).1.clicked {
                        chosen = Some(left_rows.get(i).copied().flatten());
                    }
                    if let Some(i) = crate::kit::widgets::menu_list(ui, &right, 210.0).1.clicked {
                        if i > removable_from {
                            removing = pool.get(i - removable_from - 1).map(|x| x.0);
                        } else if let Some(Some(s)) = right_rows.get(i) {
                            chosen = Some(Some(*s));
                        }
                    }
                });
            });
        });
    if closes(ctx, area.response.rect, anchor) {
        app.notefx.groove_menu = None;
        app.notefx.groove_pattern = false;
        return;
    }
    if let Some(id) = removing {
        // The list stays: what else to take out, or to play, is beside it.
        app.exec(cantus_core::Command::RemoveGroove { id });
        return;
    }
    if let Some(src) = chosen {
        app.notefx.groove_menu = None;
        // A new groove keeps the amounts the clip had (or starts at full timing).
        let groove = src.map(|s| match current {
            Some(c) => ClipGroove { source: s, ..c },
            None => ClipGroove::new(s),
        });
        if std::mem::take(&mut app.notefx.groove_pattern) {
            app.set_pattern_groove(track, clip, groove);
        } else {
            app.set_clip_groove(track, clip, groove, None);
        }
    }
}

/// The song's key: *No key* and the twelve roots on the left, the scales on the right.
fn key_menu(app: &mut CantusApp, ctx: &Context) {
    let Some(anchor) = app.notefx.key_menu else {
        return;
    };
    let key = app.project().key;
    let none = t("notefx.key.none");
    let mut left: Vec<Item<'_>> = vec![
        Item::Action {
            label: Src::Str(&none),
            check: Check::Radio(key.is_none()),
            shortcut: None,
            submenu: false,
            enabled: true,
        },
        Item::Band(Src::Key("notefx.key.root")),
    ];
    for (i, name) in ROOT_NAMES.iter().enumerate() {
        left.push(Item::Action {
            label: Src::Str(name),
            check: Check::Radio(key.is_some_and(|k| k.root as usize == i)),
            shortcut: None,
            submenu: false,
            enabled: true,
        });
    }
    let names: Vec<String> = ScaleKind::ALL.iter().map(|s| t(s.label_key())).collect();
    let mut right: Vec<Item<'_>> = vec![Item::Band(Src::Key("notefx.key.scale"))];
    for (s, name) in ScaleKind::ALL.iter().zip(&names) {
        right.push(Item::Action {
            label: Src::Str(name),
            check: Check::Radio(key.is_some_and(|k| k.scale == *s)),
            shortcut: None,
            submenu: false,
            enabled: true,
        });
    }
    let mut pick: Option<Option<Key>> = None;
    let base = key.unwrap_or_default();
    let area = egui::Area::new(Id::new("notefx-key-menu"))
        .order(Order::Foreground)
        .fixed_pos(anchor.left_bottom())
        .constrain(true)
        .show(ctx, |ui| {
            crate::kit::automate::frame().show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    if let Some(i) = crate::kit::widgets::menu_list(ui, &left, 110.0).1.clicked {
                        pick = Some(match i {
                            0 => None,
                            i => Some(Key::new((i - 2) as u8, base.scale)),
                        });
                    }
                    if let Some(i) = crate::kit::widgets::menu_list(ui, &right, 190.0).1.clicked {
                        if let Some(s) = i.checked_sub(1).and_then(|k| ScaleKind::ALL.get(k)) {
                            pick = Some(Some(Key::new(base.root, *s)));
                        }
                    }
                });
            });
        });
    if closes(ctx, area.response.rect, anchor) {
        app.notefx.key_menu = None;
        return;
    }
    if let Some(k) = pick {
        // A root or a scale chosen: the menu stays for the other half; *No key* closes it.
        if k.is_none() {
            app.notefx.key_menu = None;
        }
        app.set_key(k);
        // The piano roll's rows are the key's when folded: built again.
        crate::views::piano_roll::scale::forget_axis(app);
    }
}

/// A choice parameter's list: every value it takes, by its word, the current one checked
/// — in two columns past fourteen.
fn choice_menu(app: &mut CantusApp, ctx: &Context) {
    let Some((track, id, key, anchor)) = app.notefx.choice_menu else {
        return;
    };
    let Some(e) = cantus_core::notefx::find(app.project(), track, id) else {
        app.notefx.choice_menu = None;
        return;
    };
    let kind = e.kind.kind();
    let Some(p) = e.kind.param(key) else {
        app.notefx.choice_menu = None;
        return;
    };
    let now = e.kind.get(key).unwrap_or(p.default).round() as i32;
    let values: Vec<i32> = (p.min.round() as i32..=p.max.round() as i32).collect();
    let labels: Vec<String> = values
        .iter()
        .map(|v| {
            let mut s = String::new();
            crate::notefx_ui::write_value(kind, key, *v as f32, &mut s);
            s
        })
        .collect();
    let items: Vec<Item<'_>> = values
        .iter()
        .zip(&labels)
        .map(|(v, l)| Item::Action {
            label: Src::Str(l),
            check: Check::Radio(*v == now),
            shortcut: None,
            submenu: false,
            enabled: true,
        })
        .collect();
    let split = if items.len() > 14 {
        items.len().div_ceil(2)
    } else {
        items.len()
    };
    let mut chosen = None;
    let area = egui::Area::new(Id::new("notefx-choice-menu"))
        .order(Order::Foreground)
        .fixed_pos(anchor.left_bottom())
        .constrain(true)
        .show(ctx, |ui| {
            crate::kit::automate::frame().show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    // Short words (a rate, a root) in narrow columns.
                    let long = labels.iter().any(|l| l.chars().count() > 7);
                    let w = if long {
                        anchor.width().clamp(140.0, 200.0)
                    } else {
                        96.0
                    };
                    if let Some(i) = crate::kit::widgets::menu_list(ui, &items[..split], w)
                        .1
                        .clicked
                    {
                        chosen = Some(values[i]);
                    }
                    if split < items.len() {
                        if let Some(i) = crate::kit::widgets::menu_list(ui, &items[split..], w)
                            .1
                            .clicked
                        {
                            chosen = Some(values[split + i]);
                        }
                    }
                });
            });
        });
    if closes(ctx, area.response.rect, anchor) {
        app.notefx.choice_menu = None;
        return;
    }
    if let Some(v) = chosen {
        app.notefx.choice_menu = None;
        app.set_note_fx_param(track, id, key, v as f32, None);
    }
}
