//! Modulators in the application (ADR 0057): a new modulator on a parameter, connecting and
//! disconnecting, a depth dragged as one undo step, *Bake to automation* as one step,
//! automation recorded from knobs while a pass records, the controls' depth drag and drop,
//! the Modulators section drawn, and the popup living in the shipped window's frame.

use cantus_core::modulator::*;
use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::kit::automate::{Choice, ModHandle, ModOffer, Offer};
use cantus_ui::views::modulator::host_for;
use cantus_ui::{starter_project, CantusApp};

fn app(project: Project) -> CantusApp {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    CantusApp::from_parts(Some(project), EngineHandle::dummy(), false)
}

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    ctx
}

/// The starter project's first track's strip pan: a target every project has.
fn pan(a: &CantusApp) -> (TrackId, StripId, AutoTarget) {
    let t = &a.project().tracks[0];
    let s = t.strip.unwrap();
    (t.id, s, AutoTarget::StripPan(s))
}

#[test]
fn a_new_modulator_on_a_parameter_is_one_undo_step_on_its_own_host() {
    let mut a = app(starter_project());
    let (_, s, target) = pan(&a);
    let before = a.project().clone();
    let steps = a.bus.history().len();
    let host = host_for(a.project(), &target, None);
    assert_eq!(host, ModHost::Strip(s));
    let id = a
        .add_modulator(host, ModKind::Lfo, Some(target.clone()))
        .unwrap();
    assert_eq!(a.bus.history().len(), steps + 1);
    assert!(a.is_modulated(&target));
    let (h, _, m) = a.project().modulator(id).unwrap();
    assert_eq!(h, ModHost::Strip(s));
    assert_eq!(m.routes[0].depth, cantus_ui::views::modulator::NEW_DEPTH);
    a.undo();
    let mut back = a.project().clone();
    back.next_id = before.next_id;
    assert_eq!(&back, &before);
}

#[test]
fn connecting_disconnecting_and_a_dragged_depth_are_one_step_each() {
    let mut a = app(starter_project());
    let (t, _, target) = pan(&a);
    let id = a
        .add_modulator(ModHost::Track(t), ModKind::Step, None)
        .unwrap();
    let steps = a.bus.history().len();
    assert!(a.connect(id, target.clone()));
    assert!(!a.connect(id, target.clone()), "connected once");
    assert_eq!(a.bus.history().len(), steps + 1);
    let connected = a.project().clone();
    // A drag: many depths, one step holding the last.
    for k in 0..20 {
        a.set_route_depth(id, &target, -0.05 * k as f32, true);
    }
    a.end_gesture();
    assert_eq!(a.bus.history().len(), steps + 2);
    let depth = a.project().modulator(id).unwrap().2.routes[0].depth;
    assert!((depth + 0.95).abs() < 1e-6);
    a.undo();
    assert_eq!(a.project(), &connected);
    a.redo();
    assert!(a.disconnect(id, &target));
    assert!(!a.is_modulated(&target));
    assert_eq!(a.bus.history().len(), steps + 3);
}

#[test]
fn a_depth_choice_edits_the_mapping_modulator_and_connects_it() {
    let mut a = app(starter_project());
    let (t, _, target) = pan(&a);
    let lfo = a
        .add_modulator(ModHost::Track(t), ModKind::Lfo, Some(target.clone()))
        .unwrap();
    let rnd = a
        .add_modulator(ModHost::Track(t), ModKind::Random, None)
        .unwrap();
    // Not mapping: the first route's depth.
    a.modulation_choice(&target, Choice::Depth(0.6), egui::Rect::NOTHING);
    a.modulation_choice(&target, Choice::DepthDone, egui::Rect::NOTHING);
    assert_eq!(a.project().modulator(lfo).unwrap().2.routes[0].depth, 0.6);
    // Mapping the random source: a depth drag connects it here.
    a.toggle_mapping(rnd);
    a.modulation_choice(&target, Choice::Depth(-0.3), egui::Rect::NOTHING);
    assert_eq!(
        a.project()
            .modulator(rnd)
            .unwrap()
            .2
            .route(&target)
            .map(|r| r.depth),
        Some(-0.3)
    );
    // A handle dropped: connected at the new-route depth.
    a.toggle_mapping(rnd);
    let env = a
        .add_modulator(ModHost::Track(t), ModKind::Envelope, None)
        .unwrap();
    a.modulation_choice(&target, Choice::Connect(env.0), egui::Rect::NOTHING);
    assert!(a
        .project()
        .modulator(env)
        .unwrap()
        .2
        .route(&target)
        .is_some());
    // Modulate… opens the list beside the control.
    let r = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(30.0, 30.0));
    a.modulation_choice(&target, Choice::Modulate, r);
    assert_eq!(a.modulators.popup, Some((target, r)));
}

#[test]
fn bake_is_one_step_that_keeps_the_sound_and_drops_the_route() {
    let mut a = app(starter_project());
    let (t, _, target) = pan(&a);
    let id = a
        .add_modulator(ModHost::Track(t), ModKind::Step, Some(target.clone()))
        .unwrap();
    a.set_time_selection(Ticks::ZERO, Ticks::beats(4));
    let before = a.project().clone();
    let steps = a.bus.history().len();
    assert!(a.bake_route(id, &target));
    assert_eq!(a.bus.history().len(), steps + 1, "one undo step");
    assert!(!a.is_modulated(&target));
    let clip = a
        .project()
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .find(|c| c.automation.as_ref().is_some_and(|x| x.target == target))
        .expect("a clip of the parameter");
    assert_eq!((clip.start, clip.length), (Ticks::ZERO, Ticks::beats(4)));
    assert!(clip.automation.as_ref().unwrap().is_valid());
    a.undo();
    let mut back = a.project().clone();
    back.next_id = before.next_id;
    assert_eq!(&back, &before);
}

#[test]
fn knobs_moved_while_a_pass_records_become_automation_clips() {
    let mut a = app(starter_project());
    let (_, s, target) = pan(&a);
    let steps = a.bus.history().len();
    a.engine.seek(Ticks::beats(1));
    a.start_recording();
    assert!(a.is_recording());
    for (beat, p) in [(1, -0.5f32), (2, 0.0), (3, 0.8)] {
        a.engine.seek(Ticks::beats(beat));
        a.exec(Command::SetStripPan { id: s, pan: p });
    }
    a.engine.seek(Ticks::beats(5));
    a.stop_recording();
    let clip = a
        .project()
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .find(|c| c.automation.as_ref().is_some_and(|x| x.target == target))
        .expect("the pan was recorded");
    let curve = clip.automation.as_ref().unwrap();
    assert_eq!(clip.start, Ticks::beats(1));
    let at = |beat: i64| {
        curve
            .value_at((Ticks::beats(beat) - clip.start).0 as f64)
            .unwrap()
    };
    assert!((at(1) - 0.25).abs() < 1e-3, "{}", at(1));
    assert!((at(3) - 0.9).abs() < 1e-3, "{}", at(3));
    // The pan moves, the take (none: nothing armed), the recorded automation: one more.
    assert_eq!(a.bus.history().len(), steps + 4);
    // Off in the record options: nothing is written.
    let mut rs = a.project().recording.clone();
    rs.record_automation = false;
    a.set_record_settings(rs);
    let clips = a
        .project()
        .tracks
        .iter()
        .map(|t| t.clips.len())
        .sum::<usize>();
    a.start_recording();
    a.exec(Command::SetStripPan { id: s, pan: 0.1 });
    a.stop_recording();
    assert_eq!(
        a.project()
            .tracks
            .iter()
            .map(|t| t.clips.len())
            .sum::<usize>(),
        clips
    );
}

/// The kit's controls: Alt+drag on a modulated knob reports a depth (the value stays), a
/// plain drag moves the value; a mapping modulator makes a plain drag a depth drag.
#[test]
fn alt_drag_on_a_modulated_knob_moves_the_depth_not_the_value() {
    let run = |offer: Offer, mods: egui::Modifiers| -> (f32, Option<Choice>) {
        let ctx = ctx();
        let mut v = 0.5f32;
        let mut last = None;
        let at = egui::pos2(26.0, 58.0);
        let frames: Vec<Vec<egui::Event>> = vec![
            vec![egui::Event::PointerMoved(at)],
            vec![egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: mods,
            }],
            vec![egui::Event::PointerMoved(at + egui::vec2(0.0, -20.0))],
            vec![egui::Event::PointerMoved(at + egui::vec2(0.0, -40.0))],
            vec![egui::Event::PointerButton {
                pos: at + egui::vec2(0.0, -40.0),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: mods,
            }],
        ];
        for mut events in frames {
            events.insert(0, egui::Event::ModifiersChanged(mods));
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(200.0, 200.0),
                )),
                events,
                ..Default::default()
            };
            ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.add_space(30.0);
                    let mut p = cantus_ui::kit::widgets::Param::new(&mut v, 0.0..=1.0, 0.5)
                        .automation(offer.automated);
                    if let Some(m) = offer.modulation {
                        p = p.modulation(m);
                    }
                    let (_, c) = p.show_full(ui);
                    if let Some(Choice::Depth(d)) = c {
                        last = Some(Choice::Depth(d));
                    }
                });
            })
            .textures_delta
            .clear();
        }
        (v, last)
    };
    let modulated = Offer {
        modulation: Some(ModOffer {
            depth: Some(0.2),
            mapping: false,
            live: Some(0.6),
            learn: None,
        }),
        ..Default::default()
    };
    let (v, c) = run(modulated, egui::Modifiers::ALT);
    assert_eq!(v, 0.5, "the value stays");
    assert!(matches!(c, Some(Choice::Depth(d)) if d > 0.2), "{c:?}");
    let (v, c) = run(modulated, egui::Modifiers::NONE);
    assert!(v > 0.5, "a plain drag moves the value");
    assert!(c.is_none());
    let mapping = Offer {
        modulation: Some(ModOffer {
            depth: None,
            mapping: true,
            live: None,
            learn: None,
        }),
        ..Default::default()
    };
    let (v, c) = run(mapping, egui::Modifiers::NONE);
    assert_eq!(v, 0.5);
    assert!(matches!(c, Some(Choice::Depth(d)) if d > 0.0), "{c:?}");
    let _ = ModHandle(1);
}

/// The Modulators section draws every kind, folded and unfolded, in both languages: the
/// track's and its channel's bands for a track, the master's for the master — each
/// modulator's name in its title, its amount a parameter row that offers modulation
/// (nesting, ADR 0057), a folded one its title alone.
#[test]
fn the_modulators_section_draws_every_kind() {
    use cantus_ui::kit::automate::probe;
    let mut p = starter_project();
    let t = p.tracks[0].id;
    let s = p.tracks[0].strip.unwrap();
    let mut ids = Vec::new();
    for (host, k) in [
        (ModHost::Track(t), ModKind::Lfo),
        (ModHost::Track(t), ModKind::Envelope),
        (ModHost::Strip(s), ModKind::Follower),
        (ModHost::Strip(s), ModKind::Step),
        (ModHost::Master, ModKind::Random),
    ] {
        let mut m = Modulator::new(ModulatorId(p.alloc_id()), k);
        m.name = format!("Mod {}", ids.len() + 1);
        m.routes.push(ModRoute {
            target: AutoTarget::StripPan(s),
            depth: 0.3,
        });
        ids.push(m.id);
        p.modulators_mut(host).unwrap().push(m);
    }
    let mut a = app(p);
    let ctx = ctx();
    let draw = |a: &mut CantusApp| -> (String, usize) {
        probe::start();
        let mut text = String::new();
        for _ in 0..2 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(360.0, 2400.0),
                )),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| cantus_ui::views::inspector::show(a, ui));
            });
            out.textures_delta.clear();
            text = String::new();
            for c in &out.shapes {
                if let egui::Shape::Text(t) = &c.shape {
                    text.push_str(t.galley.text());
                    text.push('|');
                }
            }
        }
        let amounts = probe::take()
            .iter()
            .filter(|(n, m)| n == "mods.amount" && *m)
            .count();
        (text, amounts / 2)
    };
    for locale in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        cantus_ui::i18n::set_locale(locale);
        a.select_track(t);
        let (text, amounts) = draw(&mut a);
        for n in ["Mod 1", "Mod 2", "Mod 3", "Mod 4"] {
            assert!(text.contains(n), "{locale:?}: {n} is drawn: {text}");
        }
        assert!(!text.contains("Mod 5"), "the master's is not the track's");
        assert_eq!(amounts, 4, "each one's amount row offers modulation");
        // Folded: its title stays, its rows go.
        a.modulators.folded.insert(ids[1]);
        let (text, amounts) = draw(&mut a);
        assert!(text.contains("Mod 2"));
        assert_eq!(amounts, 3);
        a.modulators.folded.clear();
        // The master: its own band, the random source's rows.
        a.select_master();
        let (text, amounts) = draw(&mut a);
        assert!(
            text.contains("Mod 5"),
            "{locale:?}: the master's band: {text}"
        );
        assert_eq!(amounts, 1);
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}

/// Nesting from the interface (ADR 0057, review 1; review 2: a real drop): another
/// modulator's handle carried from its title row and let go over a modulator's amount row
/// in the inspector connects it there — one undo step — and the amount's *Modulate…* list
/// leaves the modulator itself out.
#[test]
fn a_handle_dropped_on_an_amount_nests_one_modulator_in_another() {
    use egui::{pos2, Event, Modifiers, PointerButton, Pos2};
    let mut p = starter_project();
    let t = p.tracks[0].id;
    let s = p.tracks[0].strip.unwrap();
    let mut ids = Vec::new();
    for (k, name) in [(ModKind::Lfo, "Wobble"), (ModKind::Random, "Dice")] {
        let mut m = Modulator::new(ModulatorId(p.alloc_id()), k);
        m.name = name.into();
        if k == ModKind::Lfo {
            m.routes.push(ModRoute {
                target: AutoTarget::StripPan(s),
                depth: 0.3,
            });
        }
        ids.push(m.id);
        p.modulators_mut(ModHost::Track(t)).unwrap().push(m);
    }
    let (lfo, rnd) = (ids[0], ids[1]);
    let mut a = app(p);
    a.select_track(t);
    let steps = a.bus.history().len();
    let amount = AutoTarget::ModAmount(lfo);
    let ctx = ctx();
    let size = egui::vec2(360.0, 2400.0);
    let frame = |a: &mut CantusApp, events: Vec<Event>| -> String {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, size)),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| cantus_ui::views::inspector::show(a, ui));
            cantus_ui::views::modulator::popup::frame(a, ui.ctx());
        });
        out.textures_delta.clear();
        let mut text = String::new();
        for c in &out.shapes {
            if let egui::Shape::Text(t) = &c.shape {
                text.push_str(t.galley.text());
                text.push('|');
            }
        }
        text
    };
    frame(&mut a, vec![]);
    frame(&mut a, vec![]);
    let row = ctx
        .read_response(egui::Id::new(("mod-num", lfo, "amount")))
        .expect("the LFO's amount row")
        .rect;
    let button = |p, pressed| Event::PointerButton {
        pos: p,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    // Picked up away from any control, carried over the amount, let go there.
    let away = pos2(size.x - 2.0, size.y - 2.0);
    frame(&mut a, vec![Event::PointerMoved(away), button(away, true)]);
    egui::DragAndDrop::set_payload(&ctx, ModHandle(rnd.0));
    frame(&mut a, vec![Event::PointerMoved(row.center())]);
    frame(&mut a, vec![button(row.center(), false)]);
    assert_eq!(a.bus.history().len(), steps + 1, "one undo step");
    let route = |a: &CantusApp| {
        a.project()
            .modulator(rnd)
            .and_then(|(_, _, m)| m.route(&amount).map(|r| r.depth))
    };
    assert!(route(&a).is_some(), "the drop nested it");
    // The amount's Modulate… list: the random source, never the LFO itself.
    a.modulators.popup = Some((amount.clone(), row));
    frame(&mut a, vec![]);
    let text = frame(&mut a, vec![]);
    // The list's Connect part: between its band and the New band.
    let connect = text
        .split_once("|CONNECT|")
        .and_then(|(_, rest)| rest.split_once("|NEW MODULATOR|"))
        .map(|(c, _)| c.to_string())
        .unwrap_or_else(|| panic!("the list is drawn: {text}"));
    assert!(connect.contains("Dice"), "{connect}");
    assert!(
        !connect.contains("Wobble"),
        "never its own amount: {connect}"
    );
    a.modulators.popup = None;
    a.undo();
    assert!(route(&a).is_none());
}

/// Reach: the shipped window's frame (`boot::Shell`, whose frames are `CantusApp`'s own)
/// runs the modulators' popup — a mapping left naming a modulator that is gone is let go.
#[test]
fn the_shipped_frame_runs_the_modulators_popup() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(starter_project());
    a.settings_path = dir.path().join("settings.json");
    a.layouts_path = dir.path().join("layouts.json");
    a.appearance_path = dir.path().join("appearance.json");
    a.keymap_path = dir.path().join("keymap.toml");
    a.modulators.mapping = Some(ModulatorId(987_654));
    let mut shell = cantus_ui::boot::Shell::Running(Box::new(a));
    let ctx = ctx();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1280.0, 720.0),
        )),
        ..Default::default()
    };
    ctx.run_ui(input, |root| {
        let mut frame = eframe::Frame::_new_kittest();
        eframe::App::ui(&mut shell, root, &mut frame);
    })
    .textures_delta
    .clear();
    let cantus_ui::boot::Shell::Running(a) = &shell else {
        panic!("still running");
    };
    assert_eq!(a.modulators.mapping, None);
}

/// Review 1: the desk's controls take a modulator too — its handle dropped on a strip's
/// fader connects it there; an Alt-drag on a modulated fader edits the depth, never the
/// gain. (The strip's pan and width, and the master's three, go through the same code.)
#[test]
fn a_handle_dropped_on_a_desk_fader_connects_and_alt_drag_edits_its_depth() {
    use cantus_ui::views::mixer::{self, Control};
    use egui::{pos2, Event, Modifiers, PointerButton, Pos2};
    let mut a = app(starter_project());
    let (_, s, _) = pan(&a);
    let fader = AutoTarget::StripGain(s);
    a.add_modulator(ModHost::Strip(s), ModKind::Lfo, None);
    let id = a.project().modulators(ModHost::Strip(s)).unwrap()[0].id;
    let ctx = ctx();
    cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
    let size = egui::vec2(1400.0, 800.0);
    let frame = |a: &mut CantusApp, events: Vec<Event>, mods: Modifiers| {
        let mut ev = vec![Event::ModifiersChanged(mods)];
        ev.extend(events);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, size)),
            events: ev,
            ..Default::default()
        };
        ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| cantus_ui::views::mixer::show(a, ui));
        })
        .textures_delta
        .clear();
    };
    frame(&mut a, vec![], Modifiers::NONE);
    frame(&mut a, vec![], Modifiers::NONE);
    let fr = mixer::control_rect(&ctx, Some(s), Control::Fader).expect("the strip's fader");
    let button = |p, pressed, modifiers| Event::PointerButton {
        pos: p,
        button: PointerButton::Primary,
        pressed,
        modifiers,
    };
    // The handle picked up away from any control, carried to the fader, let go there.
    let away = pos2(fr.center().x, size.y - 4.0);
    frame(
        &mut a,
        vec![
            Event::PointerMoved(away),
            button(away, true, Modifiers::NONE),
        ],
        Modifiers::NONE,
    );
    egui::DragAndDrop::set_payload(&ctx, ModHandle(id.0));
    frame(
        &mut a,
        vec![Event::PointerMoved(fr.center())],
        Modifiers::NONE,
    );
    let gain = a.project().strip(s).unwrap().gain_db;
    frame(
        &mut a,
        vec![button(fr.center(), false, Modifiers::NONE)],
        Modifiers::NONE,
    );
    let route = |a: &CantusApp| {
        a.project()
            .modulator(id)
            .and_then(|(_, _, m)| m.route(&fader).map(|r| r.depth))
    };
    let depth = route(&a).expect("the drop connected the fader");
    assert_eq!(
        a.project().strip(s).unwrap().gain_db,
        gain,
        "the gain stayed"
    );
    // Alt-drag up on the fader: the depth moves, the gain does not.
    let alt = Modifiers {
        alt: true,
        ..Modifiers::NONE
    };
    let c = fr.center();
    frame(
        &mut a,
        vec![Event::PointerMoved(c), button(c, true, alt)],
        alt,
    );
    for k in 1..=6 {
        frame(
            &mut a,
            vec![Event::PointerMoved(pos2(c.x, c.y - 8.0 * k as f32))],
            alt,
        );
    }
    frame(&mut a, vec![button(pos2(c.x, c.y - 48.0), false, alt)], alt);
    assert_eq!(
        a.project().strip(s).unwrap().gain_db,
        gain,
        "the gain stayed"
    );
    let now = route(&a).unwrap();
    assert!(now > depth, "the depth rose: {depth} → {now}");
}

/// The test CLAP plug-in, built from source where the host's tests build it.
fn test_plugin() -> std::path::PathBuf {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let target = root.join("target").join("test-plugins");
    let status =
        std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args(["build", "-p", "cantus-test-clap", "--target-dir"])
            .arg(&target)
            .current_dir(&root)
            .status()
            .expect("cargo");
    assert!(status.success());
    target.join("debug").join(if cfg!(windows) {
        "cantus_test_clap.dll"
    } else if cfg!(target_os = "macos") {
        "libcantus_test_clap.dylib"
    } else {
        "libcantus_test_clap.so"
    })
}

/// Review 1: a plug-in window that moves two parameters in one frame (an XY pad, a macro)
/// records both while a pass records — not only the last one the frame ran.
#[test]
fn two_parameters_a_plugin_window_moves_in_one_frame_are_both_recorded() {
    use cantus_engine::fx::FxParamEvent;
    let mut a = app(starter_project());
    cantus_ui::views::plugins::demo_setup(&mut a, &test_plugin(), true);
    let effect = a
        .project()
        .strips
        .iter()
        .flat_map(|s| s.effects.iter())
        .find(|e| e.spec.external.is_some())
        .map(|e| e.id)
        .expect("the hosted slot");
    // The test plug-in's gain and its (stepped) gesture parameter.
    let (gain, key) = (0u32, 3u32);
    a.engine.seek(Ticks::beats(1));
    a.start_recording();
    for (beat, g, k) in [(1, -6.0, 20.0), (3, -12.0, 70.0)] {
        a.engine.seek(Ticks::beats(beat));
        // One frame of the plug-in's window: both parameters in one gesture each.
        for (id, v) in [(gain, g), (key, k)] {
            a.plugins
                .record(&mut a.bus, effect, FxParamEvent::Begin(id));
            a.plugins
                .record(&mut a.bus, effect, FxParamEvent::Value(id, v));
        }
        a.host_tweaks();
        for id in [gain, key] {
            a.plugins.record(&mut a.bus, effect, FxParamEvent::End(id));
        }
    }
    a.engine.seek(Ticks::beats(5));
    a.stop_recording();
    for id in [gain, key] {
        let target = AutoTarget::Plugin { effect, id };
        assert!(
            a.project()
                .tracks
                .iter()
                .flat_map(|t| t.clips.iter())
                .any(|c| c.automation.as_ref().is_some_and(|x| x.target == target)),
            "parameter {id} was recorded"
        );
    }
}

/// A frame of the desk (`mixer`) or of the inspector, with `events` and `mods` held.
fn desk_frame(
    ctx: &egui::Context,
    a: &mut CantusApp,
    inspector: bool,
    events: Vec<egui::Event>,
    mods: egui::Modifiers,
) {
    let mut ev = vec![egui::Event::ModifiersChanged(mods)];
    ev.extend(events);
    let size = if inspector {
        egui::vec2(360.0, 2400.0)
    } else {
        egui::vec2(1400.0, 800.0)
    };
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        events: ev,
        ..Default::default()
    };
    ctx.run_ui(input, |ui| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                if inspector {
                    cantus_ui::views::inspector::show(a, ui)
                } else {
                    cantus_ui::views::mixer::show(a, ui)
                }
            });
    })
    .textures_delta
    .clear();
}

fn press(p: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos: p,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

/// Carry modulator `id`'s handle (picked up at `away`) and let it go at `at`.
fn drop_handle(
    ctx: &egui::Context,
    a: &mut CantusApp,
    inspector: bool,
    id: ModulatorId,
    away: egui::Pos2,
    at: egui::Pos2,
) {
    let none = egui::Modifiers::NONE;
    desk_frame(
        ctx,
        a,
        inspector,
        vec![egui::Event::PointerMoved(away), press(away, true)],
        none,
    );
    egui::DragAndDrop::set_payload(ctx, ModHandle(id.0));
    desk_frame(ctx, a, inspector, vec![egui::Event::PointerMoved(at)], none);
    desk_frame(ctx, a, inspector, vec![press(at, false)], none);
}

/// Review 2: while a modulator maps, a desk control's drag is the depth's — but only the
/// drag: a double-click still resets the fader to unity, as every other control does.
#[test]
fn a_desk_fader_double_clicked_while_a_modulator_maps_goes_to_unity() {
    use cantus_ui::views::mixer::{self, Control};
    let mut a = app(starter_project());
    let (_, s, _) = pan(&a);
    a.set_strip_gain(s, -6.0);
    a.end_gesture();
    let id = a
        .add_modulator(ModHost::Strip(s), ModKind::Lfo, None)
        .unwrap();
    a.toggle_mapping(id);
    let ctx = ctx();
    cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
    let none = egui::Modifiers::NONE;
    desk_frame(&ctx, &mut a, false, vec![], none);
    desk_frame(&ctx, &mut a, false, vec![], none);
    let fr = mixer::control_rect(&ctx, Some(s), Control::Fader).expect("the strip's fader");
    // Below the cap, so a plain click in the groove would move it too.
    let at = egui::pos2(fr.center().x, fr.bottom() - 6.0);
    let mut events = vec![egui::Event::PointerMoved(at)];
    desk_frame(&ctx, &mut a, false, std::mem::take(&mut events), none);
    for _ in 0..2 {
        desk_frame(&ctx, &mut a, false, vec![press(at, true)], none);
        desk_frame(&ctx, &mut a, false, vec![press(at, false)], none);
    }
    assert_eq!(a.project().strip(s).unwrap().gain_db, 0.0, "unity");
}

/// Review 2: a send's level takes a modulator like every parameter — its handle dropped
/// on the desk's send knob (the selected strip's send, FL's send row) connects the send.
#[test]
fn a_handle_dropped_on_a_desk_send_knob_connects_the_send() {
    use cantus_ui::views::mixer::{self, Control};
    let mut a = app(starter_project());
    let (t, s, _) = pan(&a);
    a.add_strip();
    let ret = a.project().strips.last().unwrap().id;
    assert!(a.add_sends(&[s], ret));
    a.select_track(t);
    a.select_strip(s);
    let id = a
        .add_modulator(ModHost::Strip(s), ModKind::Lfo, None)
        .unwrap();
    let ctx = ctx();
    cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
    let none = egui::Modifiers::NONE;
    desk_frame(&ctx, &mut a, false, vec![], none);
    desk_frame(&ctx, &mut a, false, vec![], none);
    let knob = mixer::control_rect(&ctx, Some(ret), Control::Send).expect("the send knob");
    let steps = a.bus.history().len();
    drop_handle(
        &ctx,
        &mut a,
        false,
        id,
        egui::pos2(4.0, 796.0),
        knob.center(),
    );
    let send = AutoTarget::Send { strip: s, to: ret };
    assert!(
        a.project()
            .modulator(id)
            .is_some_and(|(_, _, m)| m.route(&send).is_some()),
        "the drop connected the send"
    );
    assert_eq!(a.bus.history().len(), steps + 1);
}

/// Review 2: the inspector's pinned strip takes a modulator as the desk's copy does — a
/// handle dropped on its fader connects the channel's volume.
#[test]
fn a_handle_dropped_on_the_pinned_fader_connects_the_volume() {
    let mut a = app(starter_project());
    let (t, s, _) = pan(&a);
    a.select_track(t);
    let id = a
        .add_modulator(ModHost::Strip(s), ModKind::Lfo, None)
        .unwrap();
    let ctx = ctx();
    cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
    let none = egui::Modifiers::NONE;
    desk_frame(&ctx, &mut a, true, vec![], none);
    desk_frame(&ctx, &mut a, true, vec![], none);
    let fader = ctx
        .read_response(egui::Id::new(("insp-pin-fader", Some(s))))
        .expect("the pinned fader")
        .rect;
    drop_handle(&ctx, &mut a, true, id, egui::pos2(2.0, 2.0), fader.center());
    assert!(
        a.project()
            .modulator(id)
            .is_some_and(|(_, _, m)| m.route(&AutoTarget::StripGain(s)).is_some()),
        "the drop connected the volume"
    );
}

/// Review of fix4: the inspector's pinned strip shows where modulation has its controls
/// the desk's way — the fader with the desk's mark beside its cap (not the value wells'
/// edge tick), the pan knob with its own dot and rings, width and the wells with the edge
/// tick. Review of fix5: the probe notes a mark only where one is drawn — nothing
/// modulated, nothing noted; only what is modulated, only its mark.
#[test]
fn the_pinned_strip_shows_its_modulation_the_desks_way() {
    use cantus_ui::kit::automate::probe;
    // The marks the pinned strip draws, by control, with these routes modulated.
    let marks = |routes: &[fn(StripId) -> AutoTarget]| -> Vec<(String, &'static str)> {
        let mut p = starter_project();
        let t = p.tracks[0].id;
        let s = p.tracks[0].strip.unwrap();
        if !routes.is_empty() {
            let mut m = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Lfo);
            for target in routes {
                m.routes.push(ModRoute {
                    target: target(s),
                    depth: 0.3,
                });
            }
            p.modulators_mut(ModHost::Track(t)).unwrap().push(m);
        }
        let mut a = app(p);
        a.select_track(t);
        let ctx = ctx();
        probe::start();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(360.0, 2400.0),
            )),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| cantus_ui::views::inspector::show(&mut a, ui));
        });
        out.textures_delta.clear();
        let _ = probe::take();
        probe::take_live()
    };
    let how = |live: &[(String, &'static str)], name: &str| -> Vec<&'static str> {
        live.iter()
            .filter(|(n, _)| n == name)
            .map(|(_, h)| *h)
            .collect()
    };
    let live = marks(&[
        AutoTarget::StripGain,
        AutoTarget::StripPan,
        AutoTarget::StripWidth,
    ]);
    // The volume's fader the desk's way and the pan knob its own dot, their readout wells
    // the edge tick; the width's well the edge tick.
    let sorted = |name: &str| {
        let mut v = how(&live, name);
        v.sort_unstable();
        v
    };
    assert_eq!(
        sorted("kit.param.volume"),
        vec!["edge", "fader"],
        "{live:?}"
    );
    assert_eq!(sorted("kit.param.pan"), vec!["edge", "knob"], "{live:?}");
    assert_eq!(how(&live, "kit.param.width"), vec!["edge"], "{live:?}");
    // Nothing modulated: no mark anywhere.
    let none = marks(&[]);
    assert!(none.is_empty(), "no modulation, no mark: {none:?}");
    // Only the width: only its mark.
    let width = marks(&[AutoTarget::StripWidth]);
    assert_eq!(
        width,
        vec![("kit.param.width".to_string(), "edge")],
        "only what is modulated"
    );
}
