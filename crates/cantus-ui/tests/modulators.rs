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
        }),
        ..Default::default()
    };
    let (v, c) = run(mapping, egui::Modifiers::NONE);
    assert_eq!(v, 0.5);
    assert!(matches!(c, Some(Choice::Depth(d)) if d > 0.0), "{c:?}");
    let _ = ModHandle(1);
}

/// The Modulators section draws every kind, folded and unfolded, without a fault, in both
/// languages; the inspector shows the track's, its channel's and the master's.
#[test]
fn the_modulators_section_draws_every_kind() {
    let mut p = starter_project();
    let t = p.tracks[0].id;
    let s = p.tracks[0].strip.unwrap();
    for (host, k) in [
        (ModHost::Track(t), ModKind::Lfo),
        (ModHost::Track(t), ModKind::Envelope),
        (ModHost::Strip(s), ModKind::Follower),
        (ModHost::Strip(s), ModKind::Step),
        (ModHost::Master, ModKind::Random),
    ] {
        let mut m = Modulator::new(ModulatorId(p.alloc_id()), k);
        m.routes.push(ModRoute {
            target: AutoTarget::StripPan(s),
            depth: 0.3,
        });
        p.modulators_mut(host).unwrap().push(m);
    }
    let mut a = app(p);
    a.select_track(t);
    let ctx = ctx();
    for locale in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        cantus_ui::i18n::set_locale(locale);
        for _ in 0..2 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(360.0, 1800.0),
                )),
                ..Default::default()
            };
            ctx.run_ui(input, |ui| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| cantus_ui::views::inspector::show(&mut a, ui));
            })
            .textures_delta
            .clear();
        }
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
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
