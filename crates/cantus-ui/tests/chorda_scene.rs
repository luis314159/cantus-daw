//! Chorda's scene end to end (round 37, ADR 0104; round 40 phase 3, ADR 0126: the scene
//! module's projected vectors), on the boot path: a Chorda added the way the picker adds
//! one stands on the stage; its editor open in the Play view, drawn frame by frame as the
//! app draws it — a click on the lid opens it, turning Perspective and Room moves the
//! microphones, the room picked is the room drawn, a preset changes the piano; every
//! factory preset shows a piano its sound can stand for; the look touches no sample, live or
//! offline, even changed while the piano sounds.
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::inst::chorda_look::{self as look, Body, Finish, BODY_KEY, FINISH_KEY};
use cantus_core::inst::room_look::{self, RoomKind, ROOM_KEY};
use cantus_core::*;
use cantus_engine::{render, EngineHandle, Processor, RenderOptions};
use cantus_ui::app::Files;
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::scene::{self, display::Part, Picked, Room, Subject};
use cantus_ui::views::inst::chorda::FACTORY_PRESETS;
use cantus_ui::CantusApp;
use std::collections::BTreeSet;
use std::sync::Arc;

fn data() -> &'static std::path::Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        // A configured copy: no first-run audio setup, no start page over the editor.
        std::fs::write(
            d.path().join("settings.json"),
            r#"{"audio_setup":{"setup_done":true},"start_page_at_start":false}"#,
        )
        .unwrap();
        d
    })
    .path()
}

fn assembled(ctx: &egui::Context) -> (CantusApp, Processor) {
    cantus_ui::design::install_fonts(ctx);
    cantus_ui::design::install_style(ctx);
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let (engine, proc_) = EngineHandle::manual(48_000.0);
    let app = CantusApp::assemble(ctx, files, None, engine, None, None, None, false);
    (app, proc_)
}

/// A Chorda track added the way the instrument picker adds one.
fn add_chorda(app: &mut CantusApp) -> TrackId {
    app.apply_pick(
        PickPurpose::AddInstrument { index: None },
        PickChoice::Instrument(InstrumentKind::Chorda),
    );
    app.project()
        .tracks
        .iter()
        .rev()
        .find(|t| {
            t.instrument
                .native()
                .is_some_and(|n| n.kind == InstrumentKind::Chorda)
        })
        .map(|t| t.id)
        .expect("the new Chorda track")
}

fn spec_of(app: &CantusApp, track: TrackId) -> NativeSpec {
    app.project()
        .track(track)
        .and_then(|t| t.instrument.native())
        .cloned()
        .expect("a native instrument")
}

/// `track`'s scene.
fn scene_of(app: &CantusApp, track: TrackId) -> &scene::Scene {
    app.inst
        .chorda
        .scene_of(track)
        .expect("a Chorda's scene drawn")
}

const WINDOW: egui::Vec2 = egui::vec2(1500.0, 940.0);

/// Frames of the app as it runs, `events` sent on the first; time always moves forward (a
/// quarter second a frame, so every glide and fade has finished by the last).
fn frames_with(ctx: &egui::Context, app: &mut CantusApp, n: usize, events: Vec<egui::Event>) {
    thread_local! {
        static T: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) };
    }
    let mut events = Some(events);
    for _ in 0..n.max(1) {
        let t = T.with(|t| {
            t.set(t.get() + 0.25);
            t.get()
        });
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, WINDOW)),
            time: Some(t),
            events: events.take().unwrap_or_default(),
            ..Default::default()
        };
        ctx.run_ui(input, |root| app.frame(root))
            .textures_delta
            .clear();
    }
}

fn frames(ctx: &egui::Context, app: &mut CantusApp, n: usize) {
    frames_with(ctx, app, n, Vec::new());
}

/// The top of the lid's outline on screen (smaller: higher up).
fn lid_top(app: &CantusApp, track: TrackId) -> f32 {
    let b = scene_of(app, track).built().expect("a picture");
    let o = b.outlines.iter().find(|o| o.0 == Part::Lid).expect("a lid");
    o.2.iter().map(|p| p.y).fold(f32::MAX, f32::min)
}

/// Where a microphone part's head is on screen.
fn mic_at(app: &CantusApp, track: TrackId, part: Part) -> Option<egui::Pos2> {
    scene_of(app, track)
        .built()
        .and_then(|b| b.mics.iter().find(|m| m.0 == part).map(|m| m.1))
}

fn click(p: egui::Pos2) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(p),
        egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        },
        egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        },
    ]
}

#[test]
fn every_factory_preset_shows_a_piano_its_sound_can_stand_for() {
    let mut bodies = BTreeSet::new();
    let mut finishes = BTreeSet::new();
    for p in FACTORY_PRESETS {
        let spec = cantus_ui::views::inst::preset_spec(InstrumentKind::Chorda, p);
        // Said explicitly, so a later default never changes a preset's picture.
        assert!(
            spec.text(BODY_KEY).and_then(Body::from_key).is_some()
                && spec.text(FINISH_KEY).and_then(Finish::from_key).is_some(),
            "{}: its look is declared",
            p.label_key
        );
        let l = look::of(&spec);
        assert!(
            look::fits(l.body, |k| spec.get(k)),
            "{}: a {:?} its sound is not (lid {}, stiffness {}, unison {})",
            p.label_key,
            l.body,
            spec.get("lid"),
            spec.get("stiffness"),
            spec.get("unison")
        );
        bodies.insert(l.body.key());
        finishes.insert(l.finish.key());
    }
    assert_eq!(
        bodies.len(),
        Body::ALL.len(),
        "every body shown: {bodies:?}"
    );
    // The four round-37 finishes; maple (round 40) is the user's to pick.
    assert!(
        [
            Finish::Black,
            Finish::White,
            Finish::Walnut,
            Finish::Rosewood
        ]
        .iter()
        .all(|f| finishes.contains(f.key())),
        "every finish: {finishes:?}"
    );
    // And the rule says no where it must: an open lid's grand strings are no upright.
    let grand = NativeSpec::new(InstrumentKind::Chorda);
    assert!(!look::fits(Body::Upright, |k| grand.get(k)));
}

#[test]
fn a_new_chorda_stands_on_the_stage_and_its_lid_opens_with_a_click() {
    // The scene builds on the painting thread here, so each frame shows what it asks for
    // (the worker's own path is `scene_vector.rs`'s).
    scene::set_blocking(true);
    let ctx = egui::Context::default();
    let (mut app, _proc) = assembled(&ctx);
    let track = add_chorda(&mut app);
    assert_eq!(
        room_look::of(&spec_of(&app, track)).room,
        Some(RoomKind::Stage),
        "a new Chorda stands on the stage"
    );
    app.inst.view = cantus_ui::views::inst::View::Play;
    app.open_instrument_window(track);
    app.set_inst_value(track, "lid", 0.0, false);
    frames(&ctx, &mut app, 8);
    // The editor maximised, as a player works in it (nothing else over the scene).
    let key = app
        .instrument_window_key(track)
        .expect("the editor's window");
    cantus_ui::views::workspace_device::toggle_maximise(&ctx, key);
    frames(&ctx, &mut app, 30);
    let (subject, room) = scene_of(&app, track).showing().expect("a picture");
    assert_eq!(room, Room::Stage);
    assert!(
        matches!(subject, Subject::Piano { look, lid, .. } if look.body == Body::Concert && *lid == 0.0)
    );
    let shut = lid_top(&app, track);
    // A click on the closed lid opens it to the full stick (one undo step).
    let at = scene_of(&app, track)
        .point_on(Picked::Lid)
        .expect("the lid on screen");
    let [moved, press, release] = <[egui::Event; 3]>::try_from(click(at)).unwrap();
    frames_with(&ctx, &mut app, 1, vec![moved, press]);
    frames_with(&ctx, &mut app, 1, vec![release]);
    frames(&ctx, &mut app, 30);
    assert_eq!(
        spec_of(&app, track).get("lid"),
        1.0,
        "the click opened the lid"
    );
    let open = lid_top(&app, track);
    assert!(
        open < shut - 20.0,
        "the lid rises on screen: {shut} → {open}"
    );
    app.undo();
    frames(&ctx, &mut app, 30);
    assert_eq!(
        spec_of(&app, track).get("lid"),
        0.0,
        "one undo step closes it again"
    );

    // The microphones, shown: Perspective moves the pair, Room adds the room pair.
    app.inst.chorda.show_mics(track, true);
    let turn = |app: &mut CantusApp, key: &str, v: f32| {
        app.set_inst_value(track, key, v, false);
        frames(&ctx, app, 30);
    };
    turn(&mut app, "persp", 0.0);
    let close = mic_at(&app, track, Part::Pair).expect("the pair");
    turn(&mut app, "persp", 1.0);
    let far = mic_at(&app, track, Part::Pair).expect("the pair");
    assert!(
        (close - far).length() > 40.0,
        "the pair moves: {close:?} → {far:?}"
    );
    turn(&mut app, "room", 0.0);
    assert!(
        mic_at(&app, track, Part::RoomPair).is_none(),
        "no room in the mix, no room pair"
    );
    turn(&mut app, "room", 0.8);
    assert!(mic_at(&app, track, Part::RoomPair).is_some());

    // The room picked is the room drawn; an older song's Chorda (no room kept) draws the
    // room its Room size drew.
    app.set_inst_text(track, ROOM_KEY, Some("church".into()));
    frames(&ctx, &mut app, 4);
    assert_eq!(scene_of(&app, track).showing().unwrap().1, Room::Church);
    app.set_inst_text(track, ROOM_KEY, None);
    app.set_inst_value(track, "roomsize", 0.1, false);
    frames(&ctx, &mut app, 30);
    assert_eq!(scene_of(&app, track).showing().unwrap().1, Room::Booth);

    // A preset: the felt piano is an upright; the look picked by hand is one undo step.
    let felt = FACTORY_PRESETS
        .iter()
        .find(|p| p.label_key == "chorda.preset.felt")
        .unwrap();
    let spec = cantus_ui::views::inst::preset_spec(InstrumentKind::Chorda, felt);
    app.apply_inst_spec(track, &spec, "cmd.fx_preset");
    frames(&ctx, &mut app, 30);
    let body = |app: &CantusApp| match scene_of(app, track).showing().unwrap().0 {
        Subject::Piano { look, .. } => look.body,
        _ => unreachable!(),
    };
    assert_eq!(body(&app), Body::Upright);
    app.set_inst_text(track, BODY_KEY, Some("parlour".into()));
    frames(&ctx, &mut app, 4);
    assert_eq!(body(&app), Body::Parlour);
    app.undo();
    frames(&ctx, &mut app, 4);
    assert_eq!(body(&app), Body::Upright);

    // At rest nothing is rebuilt.
    frames(&ctx, &mut app, 10);
    let builds = scene_of(&app, track).builds;
    frames(&ctx, &mut app, 10);
    assert_eq!(
        scene_of(&app, track).builds,
        builds,
        "no rebuild while nothing moves"
    );

    // Another Chorda opened shows its own piano at once, never the last one's.
    let b = add_chorda(&mut app);
    app.open_instrument_window(b);
    frames(&ctx, &mut app, 3);
    let spec_b = spec_of(&app, b);
    match scene_of(&app, b).showing().expect("B's own picture").0 {
        Subject::Piano { look, lid, .. } => {
            assert_eq!(*look, look::of(&spec_b), "B's own look");
            assert_eq!(*lid, spec_b.get("lid"), "B's own lid, no glide from A's");
        }
        _ => unreachable!(),
    }
}

fn offline(p: &Project, frames: usize) -> Vec<f32> {
    let r = render(
        Arc::new(Snapshot::compile(p, 1)),
        &RenderOptions {
            tail_seconds: 1.0,
            ..Default::default()
        },
    )
    .unwrap();
    r.samples[..frames * 2].to_vec()
}

/// The null test: the look is never heard. A piano playing live through the engine the
/// app drives has its look changed twice while it sounds; what it plays equals, to the
/// bit, the offline render of the song with no look at all and with others.
#[test]
fn the_look_touches_no_sample_live_or_offline() {
    let ctx = egui::Context::default();
    let (mut app, mut proc_) = assembled(&ctx);
    // A new Chorda's keyboard (model 2's own since `DSP_VERSION` 7), designed first.
    cantus_engine::inst::chorda::ensure(cantus_engine::inst::chorda::reference(), 48_000.0);
    let track = add_chorda(&mut app);
    let mut clip = app.bus.new_clip("Phrase", Ticks::ZERO, Ticks::beats(4));
    for (b, k, v) in [(0i64, 48u8, 90u8), (0, 55, 80), (1, 64, 100), (2, 72, 70)] {
        clip.notes
            .push(app.bus.new_note(Ticks::beats(b), Ticks::beats(2), k, v));
    }
    assert!(app.exec(Command::InsertClip {
        track,
        clip: Box::new(clip),
    }));
    let others: Vec<TrackId> = app
        .project()
        .tracks
        .iter()
        .map(|t| t.id)
        .filter(|t| *t != track)
        .collect();
    for id in others {
        assert!(app.exec(Command::RemoveTrack { id }));
    }
    let plain = app.project().clone();
    app.sync_engine();
    proc_.settle();
    app.toggle_play();
    let total = 48_000;
    let mut live = Vec::with_capacity(total * 2);
    let mut i = 0;
    while live.len() / 2 < total {
        let n = [256usize, 333, 512][i % 3].min(total - live.len() / 2);
        let mut buf = vec![0f32; n * 2];
        proc_.process(&mut buf, 2);
        live.extend_from_slice(&buf);
        // The look changed twice while the chord rings.
        if i == 20 || i == 50 {
            let (b, f) = if i == 20 {
                ("upright", "walnut")
            } else {
                ("parlour", "rosewood")
            };
            app.set_inst_text(track, BODY_KEY, Some(b.into()));
            app.set_inst_text(track, FINISH_KEY, Some(f.into()));
            app.sync_engine();
        }
        i += 1;
    }
    assert!(live.iter().any(|x| x.abs() > 0.01), "the piano sounds");
    assert_eq!(look::of(&spec_of(&app, track)).body, Body::Parlour);
    let reference = offline(&plain, total);
    assert!(
        live == reference,
        "the live take equals the song with no look"
    );
    for (b, f) in [
        (Body::Upright, Finish::White),
        (Body::Grand, Finish::Walnut),
    ] {
        let mut p = plain.clone();
        if let Some(n) = p.track_mut(track).and_then(|t| t.instrument.native_mut()) {
            n.set_text(BODY_KEY, Some(b.key().into())).unwrap();
            n.set_text(FINISH_KEY, Some(f.key().into())).unwrap();
        }
        assert!(offline(&p, total) == reference, "{b:?} {f:?} is heard");
    }
}
