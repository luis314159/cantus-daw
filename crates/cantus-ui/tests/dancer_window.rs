//! Saltator's window and the ways in (round 35, ADR 0087): the picker's Visual shelf and a
//! new dancer (its name, icon and character), the window in both views and both themes —
//! the stage drawing the character, every number in its unit, the move map's pads playing
//! live notes, the character picker choosing in one step, *React to* in one step — a
//! preset keeping who dances and where, MIDI learn and an instrument choice's words.

mod support;
use cantus_core::controller::*;
use cantus_core::inst::dancer::{self, Move, CHARACTER_KEY};
use cantus_core::*;
use cantus_ui::picker::PickChoice;
use cantus_ui::views::inst::View;
use cantus_ui::CantusApp;
use egui::{Pos2, Rect};
use support::*;

fn dancer_app() -> (CantusApp, TrackId) {
    let mut a = app(cantus_ui::starter_project());
    assert!(a.add_instrument_track(None, &PickChoice::Instrument(InstrumentKind::Dancer)));
    let track = a.project().tracks.last().unwrap().id;
    (a, track)
}

fn spec(a: &CantusApp, track: TrackId) -> NativeSpec {
    a.project()
        .track(track)
        .unwrap()
        .instrument
        .native()
        .unwrap()
        .clone()
}

const SIZE: egui::Vec2 = egui::vec2(1400.0, 900.0);

/// One frame of the editor with `events`.
fn frame(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) -> String {
    cantus_ui::dance_live::DanceLive::update(a, 1.0);
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)),
        events,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| cantus_ui::views::inst::window_in(a, ui));
    out.textures_delta.clear();
    painted_text(&out.shapes)
}

fn press(at: Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}

fn click(a: &mut CantusApp, ctx: &egui::Context, at: Pos2) {
    frame(a, ctx, vec![egui::Event::PointerMoved(at)]);
    frame(a, ctx, vec![press(at, true)]);
    frame(a, ctx, vec![press(at, false)]);
}

#[test]
fn the_picker_files_it_on_the_visual_shelf_and_a_new_one_names_who_dances() {
    use cantus_ui::picker_catalog::Category;
    let natives = cantus_ui::picker_catalog::build(std::iter::empty());
    let d = natives
        .iter()
        .find(|d| d.choice == PickChoice::Instrument(InstrumentKind::Dancer))
        .expect("Saltator is offered");
    assert_eq!(d.primary(), Category::Visual);
    assert_eq!(d.name, "Saltator");
    let (a, track) = dancer_app();
    let t = a.project().track(track).unwrap();
    assert_eq!(t.name, "Saltator");
    assert_eq!(t.icon(), TrackIcon::Dance);
    assert_eq!(
        spec(&a, track).text(CHARACTER_KEY),
        Some(dancer::DEFAULT_CHARACTER)
    );
    assert_eq!(a.inst.open, Some(track), "its window opens");
}

#[test]
fn the_window_paints_both_views_in_both_themes_with_the_character_dancing() {
    let (mut a, _) = dancer_app();
    for theme in [cantus_ui::theme::slate(), cantus_ui::theme::paper()] {
        for view in [View::Play, View::Edit] {
            a.inst.view = view;
            let c = ctx();
            cantus_ui::theme::apply(&theme, &c);
            let text = frame(&mut a, &c, vec![]);
            let text = frame(&mut a, &c, vec![]) + &text;
            for w in ["SALTATOR", "To the beat", "In place", "1\u{d7}", "Happy"] {
                assert!(
                    text.to_uppercase().contains(&w.to_uppercase()),
                    "{view:?}: {w:?} not painted in {text}"
                );
            }
            if view == View::Play {
                assert!(
                    text.contains("Arms up") || text.contains("Clap"),
                    "the pads"
                );
                assert!(text.contains("Fuga") || text.contains("Reacts to"));
            } else {
                assert!(text.contains("Floating"), "where it shows");
            }
        }
    }
    assert!(
        a.inst.dancer.stage.draws > 0,
        "the stage drew the character"
    );
    assert!(a.inst.dancer.pick.draws > 0, "the picker drew its previews");
}

/// *Export video…* in the stage card opens the video export's dialog.
#[test]
fn export_video_opens_the_video_export_from_the_window() {
    let (mut a, _) = dancer_app();
    a.inst.view = View::Play;
    let c = ctx();
    let text = frame(&mut a, &c, vec![]);
    assert!(text.to_uppercase().contains("EXPORT VIDEO"), "{text}");
    let r = a.inst.dancer.placed["export_video"];
    click(&mut a, &c, r.center());
    assert!(a.video.dialog.is_some(), "the dialog opened");
}

#[test]
fn a_pad_plays_its_move_live_while_held() {
    let (mut a, track) = dancer_app();
    a.inst.view = View::Play;
    let c = ctx();
    frame(&mut a, &c, vec![]);
    let pads = a.inst.dancer.placed["pads"];
    let rects = cantus_ui::views::inst::dancer::moves::pads(pads, 3.0);
    let at = rects[Move::Clap.index()].center();
    frame(&mut a, &c, vec![egui::Event::PointerMoved(at)]);
    frame(&mut a, &c, vec![press(at, true)]);
    assert_eq!(
        a.inst.dancer.pad,
        Some(dancer::BASE_KEY + Move::Clap.index() as u8)
    );
    frame(&mut a, &c, vec![press(at, false)]);
    frame(&mut a, &c, vec![]);
    assert_eq!(a.inst.dancer.pad, None);
    let _ = track;
}

#[test]
fn react_to_steps_through_the_channels_in_one_undo_step_each() {
    let (mut a, track) = dancer_app();
    a.inst.view = View::Play;
    let c = ctx();
    frame(&mut a, &c, vec![]);
    let field = a.inst.dancer.placed["react.to"];
    let before = a.project().clone();
    // The field's middle goes on to the next choice: the whole song.
    click(&mut a, &c, field.center());
    assert_eq!(
        dancer::reacting_to(a.project(), track),
        Some(cantus_core::modulator::Listen::Master)
    );
    assert_eq!(spec(&a, track).get(dancer::REACT_KEY), dancer::REACT_DEPTH);
    assert!(a.bus.undo().is_some());
    let mut back = a.project().clone();
    back.next_id = before.next_id;
    assert_eq!(&back, &before, "one step");
}

#[test]
fn choosing_a_character_is_one_step_and_a_preset_keeps_who_dances_and_where() {
    let (mut a, track) = dancer_app();
    a.set_inst_text(
        track,
        CHARACTER_KEY,
        Some("builtin:placeholder-test".into()),
    );
    a.set_inst_value(track, "view.mode", 1.0, false);
    a.set_inst_value(track, "x", 0.3, false);
    let party = cantus_ui::views::inst::factory_presets(InstrumentKind::Dancer)
        .iter()
        .find(|p| p.label_key == "dance.preset.party")
        .unwrap();
    let target = cantus_ui::views::inst::preset_spec(InstrumentKind::Dancer, party);
    a.apply_inst_spec(track, &target, "cmd.fx_preset");
    let s = spec(&a, track);
    assert_eq!(s.text(CHARACTER_KEY), Some("builtin:placeholder-test"));
    assert_eq!((s.get("view.mode"), s.get("x")), (1.0, 0.3));
    assert_eq!(s.get("energy"), 1.0, "the preset's dance");
    // In the picker: a click on the first tile chooses the cast's first character.
    a.inst.view = View::Edit;
    let c = ctx();
    frame(&mut a, &c, vec![]);
    let first = cantus_dance::cast::builtin()[0].info().id.clone();
    let tile = a.inst.dancer.placed["pick.first"];
    click(&mut a, &c, tile.center());
    assert_eq!(spec(&a, track).text(CHARACTER_KEY), Some(first.as_str()));
    assert!(a.bus.undo().is_some());
    assert_eq!(
        spec(&a, track).text(CHARACTER_KEY),
        Some("builtin:placeholder-test")
    );
}

#[test]
fn midi_learn_reaches_its_move_and_names_its_choices() {
    use cantus_ui::views::controllers::value_words;
    let (mut a, track) = dancer_app();
    a.midi_port_names = vec!["Keys".into()];
    let target = MappingTarget::Param(AutoTarget::Instrument {
        track,
        key: "move".into(),
    });
    let m = Mapping::new(
        MappingId(9),
        MidiSource {
            port: PortRef {
                name: "Keys".into(),
                ordinal: 0,
            },
            channel: Some(0),
            control: Control::Cc(21),
        },
        target.clone(),
        MapMode::Absolute,
    );
    let mut m = m;
    m.takeover = Takeover::Jump;
    assert!(a.add_mapping(cantus_ui::controllers::Scope::Project, m));
    let cc = |v: u8| cantus_engine::midi::Heard {
        port: 0,
        event: cantus_engine::MidiEvent {
            status: 0xB0,
            data1: 21,
            data2: v,
        },
        at: std::time::Instant::now(),
    };
    a.controller_heard(&[cc(127)]);
    assert_eq!(spec(&a, track).get("move"), 16.0, "the last move: Freeze");
    a.controller_heard(&[cc(0)]);
    assert_eq!(spec(&a, track).get("move"), 0.0);
    assert_eq!(value_words(&a, &target, 0.0), "To the beat");
    // The pane's words, at a normalised position of the travel.
    assert_eq!(
        value_words(&a, &target, dancer::move_value(Move::Clap) / 16.0),
        "Clap"
    );
    // Learning: armed on `mood`, the next controller is mapped to it.
    let target = MappingTarget::Param(AutoTarget::Instrument {
        track,
        key: "mood".into(),
    });
    a.learn_target(target.clone());
    // A pad pressed (a button learns at once).
    a.controller_heard(&[cantus_engine::midi::Heard {
        port: 0,
        event: cantus_engine::MidiEvent {
            status: 0x90,
            data1: 60,
            data2: 100,
        },
        at: std::time::Instant::now(),
    }]);
    assert!(a
        .project()
        .controllers
        .mappings
        .iter()
        .any(|m| m.source.control == Control::Note(60) && m.target == target));
}

/// What the window draws is drawn again only when it moved: the stage when the pose
/// changed, a preview at most 30 times a second.
#[test]
fn the_window_draws_again_only_what_moved() {
    let (mut a, _) = dancer_app();
    a.inst.view = View::Play;
    let c = ctx();
    let input = |t: f64| egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)),
        time: Some(t),
        ..Default::default()
    };
    let run = |a: &mut CantusApp, wall: f64| {
        cantus_ui::dance_live::DanceLive::update(a, wall);
        let mut out = c.run_ui(input(wall), |ui| cantus_ui::views::inst::window_in(a, ui));
        out.textures_delta.clear();
    };
    run(&mut a, 1.0);
    let (stage, pick) = (a.inst.dancer.stage.draws, a.inst.dancer.pick.draws);
    assert!(stage > 0 && pick > 0);
    // The same moment again (stopped, the same wall clock): nothing drawn again.
    run(&mut a, 1.0);
    run(&mut a, 1.0);
    assert_eq!(a.inst.dancer.stage.draws, stage);
    assert_eq!(a.inst.dancer.pick.draws, pick);
    // A second of 144 Hz frames: the previews at 30 a second at most.
    for i in 1..=144 {
        run(&mut a, 1.0 + f64::from(i) / 144.0);
    }
    let previews = a.inst.dancer.pick.draws - pick;
    let tiles = cantus_dance::cast::builtin().len() as u64;
    assert!(
        previews <= 31 * tiles,
        "{previews} preview draws for {tiles} tiles"
    );
}

/// The window's cost with the character dancing (release: `cargo test --release -p
/// cantus-ui --test dancer_window -- --ignored --nocapture`): the median frame of the Play
/// view, every frame a new pose, at 100 %.
#[test]
#[ignore]
fn the_window_timing() {
    let (mut a, _) = dancer_app();
    a.inst.view = View::Play;
    let c = ctx();
    let mut times = Vec::new();
    for i in 0..240 {
        let wall = 1.0 + f64::from(i) / 60.0;
        cantus_ui::dance_live::DanceLive::update(&mut a, wall);
        let t0 = std::time::Instant::now();
        let mut out = c.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)),
                time: Some(wall),
                ..Default::default()
            },
            |ui| cantus_ui::views::inst::window_in(&mut a, ui),
        );
        out.textures_delta.clear();
        if i >= 40 {
            times.push(t0.elapsed().as_secs_f64() * 1e3);
        }
    }
    times.sort_by(f64::total_cmp);
    let median = times[times.len() / 2];
    let p95 = times[times.len() * 95 / 100];
    println!("dancer window: median {median:.2} ms, p95 {p95:.2} ms a frame");
    assert!(median < 8.0, "median {median:.2} ms");
}

/// Every character of the cast (and *Import…*) fits the picker's grid, in the Play view's
/// card and the Edit view's, without a tile on the words under them.
#[test]
fn every_character_fits_the_picker() {
    use cantus_ui::views::inst::dancer::pick::{fitting_side, tiles};
    let n = cantus_dance::cast::builtin().len() + 1;
    for grid in [
        Rect::from_min_size(Pos2::ZERO, egui::vec2(380.0, 128.0)),
        Rect::from_min_size(Pos2::ZERO, egui::vec2(620.0, 200.0)),
    ] {
        let side = fitting_side(grid, n, 6.0, 36.0, 96.0);
        assert!(side >= 44.0, "{side}");
        for t in tiles(grid, n, side, 6.0) {
            assert!(
                t.bottom() + side * 0.22 <= grid.bottom() + 8.0,
                "{t:?} in {grid:?}"
            );
        }
    }
}
