//! The cable band's view options (round 25, the owner's request of 2026-10-11: "an option
//! to show only the current channel's sends, or more view options"): whose cables
//! (out / in / both), which kinds (outputs, sends, sidechain keys), how much detail (send
//! levels, the outputs into the master), from a menu beside the *Off · Selected · All*
//! switch; kept with the desk's preferences. Driven as a user drives the desk: frames
//! with pointer and key events.

use cantus_core::routing::{Route, RouteKind};
use cantus_core::{Command, Dock, EffectKind, Project, Send, SendTap, StripId};
use cantus_engine::EngineHandle;
use cantus_ui::kit::hint;
use cantus_ui::views::mixer::layout::{CableOpts, CablePreset, CableScope, CableView, WidthMode};
use cantus_ui::views::mixer::{self, DeskPrefs, DrawnCable};
use cantus_ui::{starter_project, views, CantusApp};
use egui::{vec2, Event, Modifiers, Pos2, Rect};

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
    ctx
}

struct Rig {
    ctx: egui::Context,
    app: CantusApp,
    size: egui::Vec2,
    frame: u64,
    pointer: Option<Pos2>,
}

impl Rig {
    fn new(project: Project, size: egui::Vec2) -> Rig {
        cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
        Rig {
            ctx: ctx(),
            app: CantusApp::from_parts(Some(project), EngineHandle::dummy(), false),
            size,
            frame: 0,
            pointer: None,
        }
    }

    /// One pass as the application runs it: the keymap first, then the desk; every
    /// painted text with where it lies.
    fn pass(&mut self, events: Vec<Event>) -> Vec<(String, Rect)> {
        self.frame += 1;
        let mut ev = Vec::new();
        if let Some(p) = self.pointer {
            ev.push(Event::PointerMoved(p));
        }
        ev.extend(events);
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            time: Some(self.frame as f64 / 60.0),
            predicted_dt: 1.0 / 60.0,
            events: ev,
            ..Default::default()
        };
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| {
            app.global_shortcuts(ui.ctx());
            egui::CentralPanel::default().show(ui, |ui| views::mixer::show(app, ui));
        });
        out.textures_delta.clear();
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, Rect)>) {
            match shape {
                egui::Shape::Text(t) => out.push((
                    t.galley.text().to_string(),
                    t.galley.rect.translate(t.pos.to_vec2()),
                )),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut v = Vec::new();
        for c in &out.shapes {
            walk(&c.shape, &mut v);
        }
        v
    }

    fn idle(&mut self, n: usize) -> Vec<(String, Rect)> {
        let mut t = Vec::new();
        for _ in 0..n {
            t = self.pass(Vec::new());
        }
        t
    }

    fn hover(&mut self, p: Pos2) {
        self.pointer = Some(p);
        self.pass(Vec::new());
    }

    fn click(&mut self, p: Pos2) {
        self.hover(p);
        for pressed in [true, false] {
            self.pass(vec![Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            }]);
        }
        self.pass(Vec::new());
    }

    fn opts(&mut self, f: impl FnOnce(&mut CableOpts)) {
        let mut p = self.app.mixer.prefs();
        f(&mut p.cable_opts);
        self.app.mixer.set_prefs(p);
    }

    fn view(&mut self, view: CableView) {
        let mut p = self.app.mixer.prefs();
        p.cables = view;
        self.app.mixer.set_prefs(p);
    }

    fn drawn(&self, route: Route) -> Option<DrawnCable> {
        self.app
            .mixer
            .cables()
            .iter()
            .find(|d| d.route == route)
            .copied()
    }

    fn delete(&mut self) {
        self.pass(vec![Event::Key {
            key: egui::Key::Delete,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]);
        self.idle(1);
    }
}

/// The desk this file plays on: strips 15 and 16 are returns at the right; strip 2 (the
/// one selected) goes to strip 10, sends pre-fader to 15 and keys a compressor on 7;
/// strip 4 sends to 2 and strip 6 goes to 2 (2's feeders). Returns the strip ids and the
/// key's route.
fn routed(r: &mut Rig) -> (Vec<StripId>, Route) {
    let ids: Vec<StripId> = r.app.project().strips.iter().map(|s| s.id).collect();
    for id in [ids[14], ids[15]] {
        r.app.exec(Command::SetStripDock {
            id,
            dock: Dock::Right,
        });
    }
    r.app.exec(Command::SetStripOutput {
        id: ids[1],
        output: Some(ids[9]),
    });
    r.app.exec(Command::SetStripOutput {
        id: ids[5],
        output: Some(ids[1]),
    });
    r.app.set_strip_send(
        ids[1],
        Send {
            tap: SendTap::Pre,
            level: 0.5,
            ..Send::new(ids[14])
        },
    );
    r.app.set_strip_send(ids[3], Send::new(ids[1]));
    r.app.add_effect(ids[6], EffectKind::Compressor);
    let fx = r
        .app
        .project()
        .strip(ids[6])
        .unwrap()
        .effects
        .last()
        .unwrap()
        .id;
    r.app.exec(Command::SetEffectSidechain {
        strip: ids[6],
        effect: fx,
        source: Some(ids[1]),
    });
    r.app.end_gesture();
    r.app.select_strip(ids[1]);
    r.app.focus = cantus_ui::app::EditFocus::Mixer;
    (ids.clone(), Route::key(ids[1], ids[6], fx))
}

fn wide_rig() -> (Rig, Vec<StripId>, Route) {
    let mut r = Rig::new(starter_project(), vec2(1600.0, 1000.0));
    mixer::set_width_mode(&mut r.app, WidthMode::Wide);
    let (ids, key) = routed(&mut r);
    r.idle(3);
    (r, ids, key)
}

/// A point on a drawn cable's flat run, away from its pill and its ends.
fn on_cable(d: &DrawnCable) -> Pos2 {
    let p = cantus_ui::kit::cable::path(d.from, d.to, d.depth);
    p.run_start + vec2(if d.to.x > d.from.x { 4.0 } else { -4.0 }, 0.0)
}

/// The options are kept with the desk's preferences: they survive a restart, and a desk
/// saved before them (or with only some of them) reads as the default — the band as
/// round 25 drew it.
#[test]
fn cable_options_round_trip_and_old_desks_read_as_today() {
    let opts = CableOpts {
        scope: CableScope::In,
        outputs: false,
        sends: true,
        keys: false,
        levels: false,
        master: false,
    };
    let p = DeskPrefs {
        cable_opts: opts,
        ..DeskPrefs::default()
    };
    let text = serde_json::to_string(&p).unwrap();
    let back: DeskPrefs = serde_json::from_str(&text).unwrap();
    assert_eq!(back, p);
    // A round-25 desk, as its mixer.json says it.
    let old = r#"{"width":"Wide","rows":65535,"ratio":0.38,"colourful":"Off","cables":"All","band":40.0}"#;
    let back: DeskPrefs = serde_json::from_str(old).unwrap();
    assert_eq!(back.cables, CableView::All);
    assert_eq!(back.width, WidthMode::Wide);
    assert_eq!(back.cable_opts, CableOpts::default(), "today's behaviour");
    // Only some of the options written: the rest default.
    let some = r#"{"cable_opts":{"sends":false}}"#;
    let back: DeskPrefs = serde_json::from_str(some).unwrap();
    assert!(!back.cable_opts.sends && back.cable_opts.outputs && back.cable_opts.levels);
    assert_eq!(back.cable_opts.scope, CableScope::Both);
    // Through the file the app keeps, across a restart.
    let dir = tempfile::tempdir().unwrap();
    let mut r = Rig::new(starter_project(), vec2(1400.0, 900.0));
    r.app.mixer.load_prefs(&dir.path().join("layouts.json"));
    r.idle(1);
    r.opts(|o| *o = opts);
    let mut again = Rig::new(starter_project(), vec2(1400.0, 900.0));
    again.app.mixer.load_prefs(&dir.path().join("layouts.json"));
    again.idle(1);
    assert_eq!(again.app.mixer.prefs().cable_opts, opts);
}

/// The owner's "only the current channel's sends": *Selected* with only Sends shown draws
/// the selection's sends and nothing else — no output, no key, no output into the master —
/// and a click where the hidden output ran selects nothing.
#[test]
fn only_sends_shown_draws_and_selects_no_output_or_key() {
    let (mut r, ids, key) = wide_rig();
    let out = r
        .drawn(Route::output(ids[1], ids[9]))
        .expect("the output, by default");
    assert!(r.drawn(key).is_some(), "the key, by default");
    r.opts(|o| *o = CablePreset::Sends.opts());
    r.idle(2);
    let drawn = r.app.mixer.cables().to_vec();
    assert!(!drawn.is_empty(), "the sends are drawn");
    for d in &drawn {
        assert_eq!(d.route.kind, RouteKind::Send, "only sends: {d:?}");
        assert_eq!(
            d.route.from, ids[1],
            "only the selection's own (out): {d:?}"
        );
    }
    assert!(r.drawn(Route::send(ids[1], ids[14])).is_some());
    // Where the output's cable ran: nothing to hit.
    r.click(on_cable(&out));
    assert_ne!(
        r.app.mixer.selected_cable(),
        Some(Route::output(ids[1], ids[9])),
        "a hidden output is not selectable"
    );
    // Keys by themselves.
    r.opts(|o| {
        *o = CableOpts {
            outputs: false,
            sends: false,
            ..CableOpts::default()
        }
    });
    r.idle(2);
    assert!(r
        .app
        .mixer
        .cables()
        .iter()
        .all(|d| matches!(d.route.kind, RouteKind::Key(_))));
    assert!(r.drawn(key).is_some());
    // All: the switches hold there too.
    r.view(CableView::All);
    r.opts(|o| *o = CablePreset::Sends.opts());
    r.idle(2);
    assert!(r
        .app
        .mixer
        .cables()
        .iter()
        .all(|d| d.route.kind == RouteKind::Send));
    assert!(
        r.drawn(Route::send(ids[3], ids[1])).is_some(),
        "All: every send"
    );
}

/// *In* draws what arrives at the selection — its feeders' outputs and sends, keys onto
/// its inserts — and not what leaves it; *Out* the reverse; *Both* (the default) all of
/// them. In *All*, the scope decides what is lit.
#[test]
fn the_in_scope_draws_the_feeders_cables() {
    let (mut r, ids, key) = wide_rig();
    let feeders = [Route::send(ids[3], ids[1]), Route::output(ids[5], ids[1])];
    let own = [
        Route::output(ids[1], ids[9]),
        Route::send(ids[1], ids[14]),
        key,
    ];
    for route in feeders.iter().chain(&own) {
        assert!(r.drawn(*route).is_some(), "both ways by default: {route:?}");
    }
    r.opts(|o| o.scope = CableScope::In);
    r.idle(2);
    for route in &feeders {
        let d = r.drawn(*route).expect("a feeder's cable");
        assert!(d.lit, "{route:?}");
    }
    for route in &own {
        assert!(
            r.drawn(*route).is_none(),
            "in: not what leaves it: {route:?}"
        );
    }
    r.opts(|o| o.scope = CableScope::Out);
    r.idle(2);
    for route in &feeders {
        assert!(r.drawn(*route).is_none(), "out: not its feeders: {route:?}");
    }
    for route in &own {
        assert!(r.drawn(*route).is_some(), "{route:?}");
    }
    // All, in: everything drawn, only the feeders lit.
    r.view(CableView::All);
    r.opts(|o| o.scope = CableScope::In);
    r.idle(2);
    for route in &feeders {
        assert!(r.drawn(*route).expect("drawn").lit, "{route:?}");
    }
    for route in &own {
        assert!(!r.drawn(*route).expect("drawn in All").lit, "{route:?}");
    }
}

/// Delete never removes a cable of a hidden kind: hiding the selected cable's kind lets
/// it go, and Delete then leaves the route alone.
#[test]
fn delete_on_a_hidden_kind_does_nothing() {
    let (mut r, ids, _) = wide_rig();
    let route = Route::output(ids[1], ids[9]);
    let out = r.drawn(route).expect("the output's cable");
    r.click(on_cable(&out));
    assert_eq!(r.app.mixer.selected_cable(), Some(route));
    r.opts(|o| o.outputs = false);
    r.idle(1);
    assert_eq!(r.app.mixer.deletable_cable(), None);
    assert_eq!(r.app.mixer.selected_cable(), None, "hidden: let go");
    r.delete();
    assert_eq!(r.app.project().strip(ids[1]).unwrap().output, Some(ids[9]));
    // Shown again and selected again, Delete removes it.
    r.opts(|o| o.outputs = true);
    r.idle(2);
    r.click(on_cable(&out));
    assert_eq!(r.app.mixer.selected_cable(), Some(route));
    r.delete();
    assert_eq!(r.app.project().strip(ids[1]).unwrap().output, None);
}

/// The detail switches: without send levels no pill (nor its PRE tag) is drawn or
/// grabbed; without the outputs into the master the selection's master route is gone.
#[test]
fn detail_switches_hide_pills_and_master_outputs() {
    let (mut r, ids, _) = wide_rig();
    // Strip 5 goes to the master: selected, its master route shows.
    r.app.select_strip(ids[4]);
    r.idle(2);
    assert!(r.drawn(Route::output(ids[4], StripId::MASTER)).is_some());
    r.opts(|o| o.master = false);
    r.idle(2);
    assert!(r.drawn(Route::output(ids[4], StripId::MASTER)).is_none());
    r.app.select_strip(ids[1]);
    let texts = r.idle(2);
    assert!(
        !r.app.mixer.cable_pills().is_empty(),
        "levels on by default"
    );
    assert!(texts.iter().any(|(t, _)| t == "PRE"), "the PRE tag");
    let pill = mixer::pill_rect(&r.ctx, ids[1], ids[14]).expect("the pill");
    r.opts(|o| o.levels = false);
    let texts = r.idle(2);
    assert!(r.app.mixer.cable_pills().is_empty(), "no pills");
    assert!(
        mixer::pill_rect(&r.ctx, ids[1], ids[14]).is_none(),
        "nothing to grab"
    );
    let band_top = pill.top() - 30.0;
    assert!(
        !texts
            .iter()
            .any(|(t, rect)| t == "PRE" && rect.center().y > band_top),
        "no PRE tag on the band"
    );
    assert!(
        r.drawn(Route::send(ids[1], ids[14])).is_some(),
        "the cable stays"
    );
}

/// The options menu, as a user opens it: a click on the key beside the view switch opens
/// it; a click on a kind switches it and keeps the menu open; a quick view sets the
/// options and closes it.
#[test]
fn the_options_menu_switches_kinds_and_resets() {
    let (mut r, _, _) = wide_rig();
    let key = mixer::title_rect(&r.ctx, "cable-options").expect("the options key");
    let switch = mixer::title_rect(&r.ctx, "cables").expect("the view switch");
    assert!(
        key.left() >= switch.right(),
        "beside the switch: {key:?} {switch:?}"
    );
    r.click(key.center());
    let texts = r.idle(1);
    let row = |texts: &[(String, Rect)], word: &str| {
        texts
            .iter()
            .find(|(t, _)| t == word)
            .map(|(_, r)| r.center())
            .unwrap_or_else(|| panic!("{word:?} is not in the menu: {texts:?}"))
    };
    let keys = cantus_ui::i18n::t("cables.kinds.keys");
    r.click(row(&texts, &keys));
    assert!(!r.app.mixer.prefs().cable_opts.keys, "the keys hidden");
    let texts = r.idle(1);
    let everything = cantus_ui::i18n::t("cables.preset.everything");
    r.click(row(&texts, &everything));
    assert_eq!(r.app.mixer.prefs().cable_opts, CableOpts::default());
    let texts = r.idle(1);
    assert!(
        !texts.iter().any(|(t, _)| *t == everything),
        "a quick view closes the menu"
    );
    // A quick view from the menu, the view off: only the selection's sends, shown.
    r.view(CableView::Off);
    r.idle(1);
    r.click(key.center());
    let texts = r.idle(1);
    r.click(row(&texts, &cantus_ui::i18n::t("cables.preset.sends")));
    assert_eq!(r.app.mixer.prefs().cable_opts, CablePreset::Sends.opts());
    assert_eq!(r.app.mixer.prefs().cables, CableView::Selected);
    // The key toggles its menu: a second click closes it.
    let quick = cantus_ui::i18n::t("cables.menu.quick").to_uppercase();
    let shows = |texts: &[(String, Rect)]| texts.iter().any(|(t, _)| t.to_uppercase() == quick);
    r.click(key.center());
    assert!(shows(&r.idle(1)), "open");
    r.click(key.center());
    assert!(!shows(&r.idle(1)), "a second click on the key closes it");
}

/// The legend never leaves the user wondering why a cable is missing: hovering it names
/// the kinds the options hide.
#[test]
fn the_legend_says_which_kinds_are_hidden() {
    let (mut r, _, _) = wide_rig();
    r.opts(|o| *o = CablePreset::Sends.opts());
    let texts = r.idle(2);
    let word = cantus_ui::i18n::t("cables.legend").to_uppercase();
    let at = texts
        .iter()
        .find(|(t, _)| *t == word)
        .map(|(_, r)| r.center())
        .expect("ROUTING in the legend");
    r.hover(at);
    let value = hint::current(&r.ctx).map(|h| h.value).unwrap_or_default();
    for key in ["cables.kinds.outputs", "cables.kinds.keys"] {
        let w = cantus_ui::i18n::t(key);
        assert!(value.contains(&w), "{w:?} named hidden: {value:?}");
    }
    assert!(!value.contains(&cantus_ui::i18n::t("cables.kinds.sends")));
}

/// The title row keeps its keys apart: at 1280 px, at 75 %, 100 % and 200 %, in English
/// and Spanish, with the desk-wide *Solo off* / *Unmute all* keys showing and a strip's
/// name in the title, no key overlaps another or runs off the row, and no word of the
/// row lies over a key it does not belong to.
#[test]
fn the_title_row_never_overlaps() {
    const KEYS: [&str; 8] = [
        "menu",
        "add",
        "width",
        "wave",
        "cables",
        "cable-options",
        "mixer.solo_off",
        "mixer.unmute_all",
    ];
    for locale in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        for (px, ppp) in [
            (1280.0f32, 0.75f32),
            (1280.0, 1.0),
            (1280.0, 2.0),
            // A desk in the lower zone beside the inspector, at 200 %: the switch folds
            // and the options key gives way to the desk's menu.
            (720.0, 2.0),
            (900.0, 2.0),
        ] {
            for filtered in [false, true] {
                let mut r = Rig::new(starter_project(), vec2(px, 720.0) / ppp);
                cantus_ui::i18n::set_locale(locale);
                r.ctx.set_pixels_per_point(ppp);
                let (ids, _) = routed(&mut r);
                r.app.exec(Command::SetStripName {
                    id: ids[1],
                    name: "Lead Vocal Doubles".into(),
                });
                r.app.toggle_strip_solo(ids[2]);
                r.app.toggle_strip_mute(ids[4]);
                if filtered {
                    r.opts(|o| *o = CablePreset::Feeders.opts());
                }
                let texts = r.idle(3);
                let what = format!("{locale:?} {px} px {ppp} filtered {filtered}");
                let keys: Vec<(&str, Rect)> = KEYS
                    .iter()
                    .filter_map(|k| mixer::title_rect(&r.ctx, k).map(|rect| (*k, rect)))
                    .collect();
                let must: &[&str] = if px >= 1280.0 {
                    &["menu", "add", "width", "cables", "cable-options"]
                } else {
                    &["menu", "add", "width", "cables"]
                };
                for &must in must {
                    assert!(
                        keys.iter().any(|(k, _)| *k == must),
                        "{what}: {must} is in the row"
                    );
                }
                let row = keys.iter().fold(Rect::NOTHING, |a, (_, b)| a.union(*b));
                let width = r.size.x;
                for (i, (ka, a)) in keys.iter().enumerate() {
                    assert!(
                        a.left() >= 0.0 && a.right() <= width,
                        "{what}: {ka} off the row {a:?}"
                    );
                    for (kb, b) in &keys[i + 1..] {
                        assert!(
                            !a.shrink(0.5).intersects(b.shrink(0.5)),
                            "{what}: {ka} {a:?} overlaps {kb} {b:?}"
                        );
                    }
                }
                // The row's words: inside their own key, or over none.
                for (t, tr) in &texts {
                    if tr.center().y < row.top() || tr.center().y > row.bottom() {
                        continue;
                    }
                    let home = keys.iter().find(|(_, k)| k.contains(tr.center()));
                    for (k, kr) in &keys {
                        if home.is_some_and(|(h, _)| h == k) {
                            continue;
                        }
                        assert!(
                            !tr.shrink(1.0).intersects(*kr),
                            "{what}: {t:?} {tr:?} lies over {k} {kr:?}"
                        );
                    }
                    assert!(
                        !t.contains('\u{2026}') || home.is_none(),
                        "{what}: {t:?} cut"
                    );
                }
            }
        }
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}

/// Every word of the options reads in both languages (the i18n test holds the twins;
/// here, that none is missing or falls back to English).
#[test]
fn the_options_words_exist_in_both_languages() {
    let mut keys: Vec<&str> = vec![
        "cables.menu.options",
        "cables.menu.scope",
        "cables.menu.kinds",
        "cables.menu.detail",
        "cables.menu.quick",
        "cables.detail.levels",
        "cables.detail.master",
        "cables.hint.options",
        "cables.legend.hidden",
    ];
    keys.extend(CableScope::ALL.iter().map(|s| s.key()));
    keys.extend(
        cantus_ui::views::mixer::layout::CableKindSel::ALL
            .iter()
            .map(|k| k.key()),
    );
    keys.extend(CablePreset::ALL.iter().map(|p| p.key()));
    for key in &keys {
        cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
        let en = cantus_ui::i18n::t(key);
        cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::Es);
        let es = cantus_ui::i18n::t(key);
        assert!(!en.is_empty() && en != *key, "{key}: no English");
        assert!(!es.is_empty() && es != *key, "{key}: no Spanish");
        assert_ne!(en, es, "{key}: the Spanish is the English");
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}

/// The desk's ▸ menu holds the cable options too (*Cable options ▸*), so they are in
/// reach where the title row has no room for their key; a switch there keeps the menu
/// open, as in the key's menu.
#[test]
fn the_desk_menu_holds_the_cable_options() {
    let (mut r, _, _) = wide_rig();
    mixer::open_menu(&mut r.app, None, egui::pos2(20.0, 300.0));
    let texts = r.idle(2);
    let label = cantus_ui::i18n::t("cables.menu.options");
    let at = texts
        .iter()
        .find(|(t, _)| *t == label)
        .map(|(_, r)| r.center())
        .expect("Cable options in the desk's menu");
    r.hover(at);
    let texts = r.idle(2);
    let sends = cantus_ui::i18n::t("cables.kinds.sends");
    let row = texts
        .iter()
        .find(|(t, _)| *t == sends)
        .map(|(_, r)| r.center())
        .expect("the submenu's kinds");
    // Across to the submenu along its row, then a click.
    r.hover(egui::pos2(row.x, at.y));
    r.click(row);
    assert!(!r.app.mixer.prefs().cable_opts.sends, "sends hidden");
    let texts = r.idle(1);
    assert!(
        texts.iter().any(|(t, _)| *t == sends),
        "the submenu stays open"
    );
}

/// The options menu reads whole — every heading and row, uncut — in both languages, at
/// 100 % and 200 %.
#[test]
fn the_options_menu_reads_whole_in_both_languages() {
    let bands = [
        "cables.menu.scope",
        "cables.menu.kinds",
        "cables.menu.detail",
        "cables.menu.quick",
    ];
    let mut rows: Vec<&str> = vec!["cables.detail.levels", "cables.detail.master"];
    rows.extend(CableScope::ALL.iter().map(|s| s.key()));
    rows.extend(
        cantus_ui::views::mixer::layout::CableKindSel::ALL
            .iter()
            .map(|k| k.key()),
    );
    rows.extend(CablePreset::ALL.iter().map(|p| p.key()));
    for locale in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        for ppp in [1.0f32, 2.0] {
            let mut r = Rig::new(starter_project(), vec2(1600.0, 1000.0) / ppp);
            cantus_ui::i18n::set_locale(locale);
            r.ctx.set_pixels_per_point(ppp);
            r.idle(2);
            mixer::open_cable_options(&mut r.app, &r.ctx.clone(), egui::pos2(300.0, 40.0));
            let texts = r.idle(2);
            for key in bands {
                let want = cantus_ui::i18n::t(key).to_uppercase();
                assert!(
                    texts.iter().any(|(t, _)| t.to_uppercase() == want),
                    "{locale:?} {ppp}: {want:?} whole in {texts:?}"
                );
            }
            for key in &rows {
                let want = cantus_ui::i18n::t(key);
                assert!(
                    texts.iter().any(|(t, _)| *t == want),
                    "{locale:?} {ppp}: {want:?} whole"
                );
            }
        }
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}
