//! The preset browser end to end (round 37, ADR 0101 §4): the application as it boots
//! (`CantusApp::assemble`, a manual engine), an instrument's editor, its preset menu's
//! *Browse presets…* clicked, a preset's ▶ clicked — the audition is rendered and **heard
//! from the engine's output while the project and its undo history stay as they were** —
//! then *Load* clicked: the preset is the instrument's, as **one step of undo**, and undo
//! gives the old sound back. Then the same for an insert, heard on the user's own clip.

use cantus_core::{EffectKind, InstrumentKind, InstrumentSpec, NativeSpec, Project};
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::preset_audition::{self as aud, HeardOn, Target};
use cantus_ui::preset_catalog::{Payload, Unit};
use cantus_ui::preset_tags::Role;
use cantus_ui::CantusApp;
use egui::{Pos2, Rect};
use std::time::Duration;

mod support;

const SCREEN: egui::Vec2 = egui::vec2(1600.0, 1000.0);

fn run(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
        events,
        ..Default::default()
    };
    ctx.run_ui(input, |ui| a.frame(ui)).textures_delta.clear();
}

fn click_at(a: &mut CantusApp, ctx: &egui::Context, at: Pos2) {
    let button = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    run(a, ctx, vec![egui::Event::PointerMoved(at)]);
    run(a, ctx, vec![button(true)]);
    run(a, ctx, vec![button(false)]);
}

/// Where a shell's preset well was drawn last frame (the shell's second header part).
fn well_of(ctx: &egui::Context, shell: egui::Id) -> Rect {
    ctx.read_response(shell.with(("part", 1usize)))
        .expect("the preset well is drawn")
        .rect
}

/// A project as a user has it: an Unda playing a line, with Fervor on its strip.
fn project() -> Project {
    let mut p = aud::phrase_project(
        InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Wavetable)),
        Role::Lead,
    );
    let strip = p.tracks[0].strip.expect("a strip");
    let slot = p.make_effect(EffectKind::Saturator);
    p.strip_mut(strip).unwrap().effects.push(slot);
    p
}

/// The loudest sample the engine's output carries over `blocks` blocks.
fn heard(proc: &mut cantus_engine::Processor, blocks: usize) -> f32 {
    let mut out = vec![0f32; 512 * 2];
    let mut most = 0f32;
    for _ in 0..blocks {
        out.iter_mut().for_each(|x| *x = 0.0);
        proc.process(&mut out, 2);
        most = out.iter().fold(most, |m, x| m.max(x.abs()));
    }
    most
}

#[test]
fn the_browser_opens_from_the_menu_auditions_without_a_change_and_loads_as_one_undo_step() {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    let ctx = support::ctx();
    let files = Files::read(data.path(), Some(cantus_ui::i18n::Locale::En));
    let (engine, mut proc) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::assemble(
        &ctx,
        files,
        Some(project()),
        engine,
        None,
        None,
        None,
        false,
    );
    // A fresh data folder opens on the first-run audio window and the start page, over
    // everything: closed, as a user closes them once.
    run(&mut app, &ctx, vec![]);
    app.audio_setup.first_run = None;
    app.workflow.start.close();
    let track = app.project().tracks[0].id;
    let strip = app.project().tracks[0].strip.unwrap();
    let fervor = app.project().strip(strip).unwrap().effects[0].id;
    heard(&mut proc, 8);

    // The editor; a click on its preset well opens the list, whose first row is Browse
    // presets….
    app.open_inst_editor(track);
    run(&mut app, &ctx, vec![]);
    run(&mut app, &ctx, vec![]);
    let well = well_of(
        &ctx,
        cantus_ui::kit::shell::Shell::new(("inst", track.0)).id,
    );
    click_at(&mut app, &ctx, well.center());
    assert_eq!(app.inst.popup, cantus_ui::views::inst::Popup::Presets);
    run(&mut app, &ctx, vec![]);
    let menu = ctx
        .memory(|m| m.area_rect(egui::Id::new(("inst-popup", track.0))))
        .expect("the preset menu is open");
    // The menu's first row: 4 points of frame, then a 22-point row.
    click_at(
        &mut app,
        &ctx,
        Pos2::new(menu.left() + 40.0, menu.top() + 15.0),
    );
    assert!(app.presets.open, "Browse presets… opened the browser");
    assert_eq!(
        app.presets.unit,
        Some(Unit::Instrument(InstrumentKind::Wavetable))
    );
    assert_eq!(app.presets.target, Target::Track(track));
    run(&mut app, &ctx, vec![]);
    assert!(
        app.presets.rows.len() >= 3,
        "rows on screen: {}",
        app.presets.rows.len()
    );

    // ▶ on the second row: rendered, heard, and nothing in the project moved.
    let before = app.project().clone();
    let steps = app.bus.history().len();
    let (revision, dirty) = (app.bus.revision(), app.bus.is_dirty());
    let (id, row) = app.presets.rows[1].clone();
    click_at(&mut app, &ctx, Pos2::new(row.left() + 18.0, row.center().y));
    assert!(app.presets.audition.rendering() || app.presets.audition.renders == 1);
    assert!(
        app.finish_preset_audition(Duration::from_secs(120)),
        "the audition renders"
    );
    let a = &app.presets.audition;
    assert_eq!(a.renders, 1);
    assert_eq!(a.heard.as_deref(), Some(id.as_str()));
    let sample = a.last.clone().expect("handed to the audition voice");
    assert!(
        sample.frames.iter().any(|x| x.abs() > 1e-3),
        "the render sounds"
    );
    assert!(a.peaks.contains_key(&id), "its row has its mini waveform");
    assert!(heard(&mut proc, 40) > 1e-3, "the engine's output plays it");
    assert_eq!(
        app.project(),
        &before,
        "an audition changes nothing in the project"
    );
    assert_eq!(app.bus.history().len(), steps, "nor the undo history");
    assert_eq!(app.bus.revision(), revision, "nor its revision");
    assert_eq!(app.bus.is_dirty(), dirty, "nor whether it needs saving");

    // Load: the preset is the instrument's, in one step of undo.
    app.presets.selected = Some(id.clone());
    run(&mut app, &ctx, vec![]);
    let load = app.presets.load.expect("the Load key is drawn");
    click_at(&mut app, &ctx, load.center());
    assert_eq!(app.presets.applied.as_deref(), Some(id.as_str()));
    assert_eq!(app.bus.history().len(), steps + 1, "one step of undo");
    let want = match &app.presets.entry(&id).unwrap().payload {
        Payload::Native(s) => s.clone(),
        other => panic!("an instrument preset, not {other:?}"),
    };
    assert_eq!(
        app.project().track(track).unwrap().instrument,
        InstrumentSpec::Native(want)
    );
    app.undo();
    assert_eq!(
        app.project().track(track).unwrap().instrument,
        before.track(track).unwrap().instrument,
        "undo gives the old sound back"
    );
    app.close_preset_browser();
    app.close_inst_editor();

    // An insert: its window's preset well, Browse presets…, a row's ▶ (heard on the user's
    // own clip), Load.
    app.open_effect_editor(strip, fervor);
    run(&mut app, &ctx, vec![]);
    run(&mut app, &ctx, vec![]);
    let well = well_of(
        &ctx,
        cantus_ui::views::fx::frame::shell_of(EffectKind::Saturator).id,
    );
    click_at(&mut app, &ctx, well.center());
    run(&mut app, &ctx, vec![]);
    // The list opens under the well; its first row is Browse presets….
    click_at(
        &mut app,
        &ctx,
        Pos2::new(well.left() + 40.0, well.bottom() + 15.0),
    );
    assert!(app.presets.open, "Browse presets… from the effect's window");
    assert_eq!(app.presets.unit, Some(Unit::Effect(EffectKind::Saturator)));
    run(&mut app, &ctx, vec![]);
    let before = app.project().clone();
    let steps = app.bus.history().len();
    let (id, row) = app.presets.rows[0].clone();
    click_at(&mut app, &ctx, Pos2::new(row.left() + 18.0, row.center().y));
    assert!(app.finish_preset_audition(Duration::from_secs(120)));
    assert_eq!(app.presets.audition.renders, 2);
    assert!(heard(&mut proc, 40) > 1e-3, "the insert's audition plays");
    assert!(
        matches!(
            app.presets.audition.on,
            Some(HeardOn::FirstClip | HeardOn::SelectedClip)
        ),
        "heard on the user's own clip"
    );
    assert_eq!(app.project(), &before);
    assert_eq!(app.bus.history().len(), steps);
    app.presets.selected = Some(id.clone());
    run(&mut app, &ctx, vec![]);
    let load = app.presets.load.expect("the Load key is drawn");
    click_at(&mut app, &ctx, load.center());
    assert_eq!(app.bus.history().len(), steps + 1);
    let Payload::Effect(values) = app.presets.entry(&id).unwrap().payload.clone() else {
        panic!("an effect preset");
    };
    let spec = &app.project().effect(strip, fervor).unwrap().spec;
    for (k, v) in &values {
        assert_eq!(spec.get(k), *v, "{k}");
    }
    // Esc closes it and stops what it plays.
    run(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    assert!(!app.presets.open, "Esc closes the browser");
}

#[test]
fn an_insert_with_no_clip_to_hear_is_auditioned_on_a_fixed_source_and_the_user_s_clip_otherwise() {
    let p = project();
    let strip = p.tracks[0].strip.unwrap();
    let fervor = p.strip(strip).unwrap().effects[0].id;
    let entry = cantus_ui::preset_catalog::entries(Unit::Effect(EffectKind::Saturator), false)
        .into_iter()
        .next()
        .expect("Fervor has presets");
    let on_clip = aud::prepare(
        &p,
        Target::Effect {
            strip,
            effect: fervor,
        },
        &entry,
        None,
        aud::TEMPO,
    );
    assert_eq!(
        on_clip.project.tracks.len(),
        1,
        "the user's project, cut down to the clip's track"
    );
    assert_eq!(on_clip.project.tracks[0].id, p.tracks[0].id);
    let Payload::Effect(values) = &entry.payload else {
        unreachable!()
    };
    let spec = &on_clip.project.effect(strip, fervor).unwrap().spec;
    for (k, v) in values {
        assert_eq!(spec.get(k), *v);
    }
    assert_eq!(
        p.effect(strip, fervor).unwrap().spec.params.get("drive"),
        None,
        "a copy"
    );
    let mut bare = p.clone();
    bare.tracks[0].clips.clear();
    let fixed = aud::prepare(
        &bare,
        Target::Effect {
            strip,
            effect: fervor,
        },
        &entry,
        None,
        aud::TEMPO,
    );
    assert!(!fixed.project.tracks.is_empty());
    assert_eq!(
        fixed.project.master.effects.len(),
        1,
        "the effect on the source's master"
    );
}

/// A Pulsus kit end to end (the review's item 12, once drums' *Browse presets…* row landed in
/// the kit's ⋯ menu): the row opens the browser on the kit's factory kits, a kit's ▶ is
/// clicked — rendered on its own groove and heard from the engine, the project untouched —
/// then *Load* clicked: the kit is the track's, in one step of undo, and the project needs
/// saving.
#[test]
fn a_kit_is_auditioned_on_its_groove_and_loaded_from_the_kits_menu_in_one_step() {
    use cantus_ui::views::pulsus_editor::{more, more_rows, MoreRow};
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    let ctx = support::ctx();
    let files = Files::read(data.path(), Some(cantus_ui::i18n::Locale::En));
    let (engine, mut proc) = EngineHandle::manual(48_000.0);
    let mut song = Project::new("Kit");
    let mut t = song.make_track("Drums");
    t.instrument = InstrumentSpec::Pulsus(cantus_core::PulsusParams::studio());
    song.tracks.push(t);
    let mut app = CantusApp::assemble(&ctx, files, Some(song), engine, None, None, None, false);
    run(&mut app, &ctx, vec![]);
    app.audio_setup.first_run = None;
    app.workflow.start.close();
    let track = app.project().tracks[0].id;
    heard(&mut proc, 8);

    let browse = more_rows()
        .into_iter()
        .find(|r| *r == MoreRow::Browse)
        .expect("the kit's menu has the row");
    more(&mut app, track, browse);
    assert!(app.presets.open);
    assert_eq!(app.presets.unit, Some(Unit::Kit));
    run(&mut app, &ctx, vec![]);
    assert!(app.presets.rows.len() >= 3, "kits on screen");

    let before = app.project().clone();
    let steps = app.bus.history().len();
    let dirty = app.bus.is_dirty();
    let (id, row) = app.presets.rows[2].clone();
    click_at(&mut app, &ctx, Pos2::new(row.left() + 18.0, row.center().y));
    assert!(
        app.finish_preset_audition(Duration::from_secs(120)),
        "the audition renders"
    );
    let a = &app.presets.audition;
    assert_eq!(a.heard.as_deref(), Some(id.as_str()));
    assert!(
        matches!(a.on, Some(HeardOn::Phrase(Role::Drums, _))),
        "a kit is heard on its groove: {:?}",
        a.on
    );
    let sample = a.last.clone().expect("handed to the audition voice");
    assert!(
        sample.frames.iter().any(|x| x.abs() > 1e-3),
        "the groove sounds"
    );
    assert!(heard(&mut proc, 40) > 1e-3, "the engine's output plays it");
    assert_eq!(app.project(), &before, "an audition changes nothing");
    assert_eq!(app.bus.history().len(), steps);

    app.presets.selected = Some(id.clone());
    run(&mut app, &ctx, vec![]);
    let load = app.presets.load.expect("the Load key is drawn");
    click_at(&mut app, &ctx, load.center());
    assert_eq!(app.presets.applied.as_deref(), Some(id.as_str()));
    assert_eq!(app.bus.history().len(), steps + 1, "one step of undo");
    assert!(app.bus.is_dirty() && !dirty, "the project needs saving");
    let Payload::Kit(kit) = &app.presets.entry(&id).unwrap().payload else {
        panic!("a kit")
    };
    let now = app
        .project()
        .track(track)
        .unwrap()
        .instrument
        .pulsus()
        .unwrap();
    assert_eq!(now.pads.len(), kit.pads.len());
    assert_eq!(
        now.pads.iter().map(|p| p.pitch).collect::<Vec<_>>(),
        kit.pads.iter().map(|p| p.pitch).collect::<Vec<_>>(),
        "the kit's pads"
    );
    assert_ne!(
        app.project().track(track).unwrap().instrument,
        before.track(track).unwrap().instrument
    );
    app.undo();
    assert_eq!(
        app.project().track(track).unwrap().instrument,
        before.track(track).unwrap().instrument,
        "undo gives the old kit back"
    );
}
