//! Chorda's scene end to end (round 37, ADR 0104), on the boot path: a Chorda added the way
//! the picker adds one, its editor open in the Play view, drawn frame by frame as the app
//! draws it — turning Lid, Mics and Room moves the scene's geometry, a preset changes the
//! piano; every factory preset shows a piano its sound can stand for; the look touches no
//! sample, live or offline, even changed while the piano sounds.
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::inst::chorda_look::{self as look, Body, Finish, BODY_KEY, FINISH_KEY};
use cantus_core::*;
use cantus_dsp::chorda::design::Tuning;
use cantus_engine::{render, EngineHandle, Processor, RenderOptions};
use cantus_ui::app::Files;
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::views::inst::chorda::{scene, FACTORY_PRESETS};
use cantus_ui::CantusApp;
use std::collections::BTreeSet;
use std::sync::Arc;

fn data() -> &'static std::path::Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
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

/// The first Chorda's scene (the one the test opened first).
fn scene_a(app: &CantusApp) -> &scene::Scene {
    app.project()
        .tracks
        .iter()
        .find_map(|t| app.inst.chorda.scene_of(t.id))
        .expect("a Chorda's scene drawn")
}

/// Frames of the app as it runs, the editor maximised; the scene's probe after them.
fn frames(ctx: &egui::Context, app: &mut CantusApp, n: usize) -> scene::Probe {
    cantus_ui::raster::shoot(ctx, egui::vec2(1500.0, 940.0), 1.0, n, None, |root| {
        app.frame(root)
    });
    scene_a(app).probe
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
    assert_eq!(
        finishes.len(),
        Finish::ALL.len(),
        "every finish: {finishes:?}"
    );
    // And the rule says no where it must: an open lid's grand strings are no upright.
    let grand = NativeSpec::new(InstrumentKind::Chorda);
    assert!(!look::fits(Body::Upright, |k| grand.get(k)));
}

#[test]
fn turning_lid_mics_and_room_moves_the_scene_and_a_preset_changes_the_piano() {
    // The scene bakes on the painting thread here, so each frame shows what it asks for
    // (the worker's own path is `chorda_scene_budget.rs`'s).
    scene::set_blocking(true);
    let ctx = egui::Context::default();
    let (mut app, _proc) = assembled(&ctx);
    let track = add_chorda(&mut app);
    app.inst.view = cantus_ui::views::inst::View::Play;
    app.open_instrument_window(track);
    let first = frames(&ctx, &mut app, 8);
    assert_eq!(
        first.body,
        Some(Body::Concert),
        "a new Chorda: a concert grand"
    );
    // Each control moves what it should, once the glide has settled.
    let mut turn = |key: &str, v: f32| {
        app.set_inst_value(track, key, v, false);
        frames(&ctx, &mut app, 30)
    };
    let shut = turn("lid", 0.0);
    assert!(
        shut.lid_tip.y > first.lid_tip.y + 5.0,
        "the lid comes down: {:?} → {:?}",
        first.lid_tip,
        shut.lid_tip
    );
    let close = turn("persp", 0.0);
    let far = turn("persp", 1.0);
    assert!(
        (close.mics[0] - far.mics[0]).length() > 40.0,
        "the microphones move: {:?} → {:?}",
        close.mics,
        far.mics
    );
    let narrow = turn("width", 0.0);
    let wide = turn("width", 1.0);
    assert!(
        (wide.mics[0] - wide.mics[1]).length() > (narrow.mics[0] - narrow.mics[1]).length() + 2.0,
        "the pair spreads with the width"
    );
    let small = turn("roomsize", 0.1);
    let big = turn("roomsize", 1.0);
    assert!(
        (small.corner - big.corner).length() > 10.0 && small.digest != big.digest,
        "the room grows: {:?} → {:?}",
        small.corner,
        big.corner
    );
    let dry = turn("room", 0.0);
    assert!(!dry.room_mics, "no room in the mix, no room pair");
    let live = turn("room", 0.8);
    assert!(live.room_mics && live.digest != dry.digest);
    // A preset: the felt piano is an upright.
    let felt = FACTORY_PRESETS
        .iter()
        .find(|p| p.label_key == "chorda.preset.felt")
        .unwrap();
    let spec = cantus_ui::views::inst::preset_spec(InstrumentKind::Chorda, felt);
    app.apply_inst_spec(track, &spec, "cmd.fx_preset");
    let up = frames(&ctx, &mut app, 30);
    assert_eq!(up.body, Some(Body::Upright));
    assert_ne!(up.digest, live.digest);
    // The look picked by hand is one undo step, named as the look; undone, the upright again.
    app.set_inst_text(track, BODY_KEY, Some("parlour".into()));
    assert_eq!(frames(&ctx, &mut app, 4).body, Some(Body::Parlour));
    app.undo();
    assert_eq!(frames(&ctx, &mut app, 4).body, Some(Body::Upright));
    // In steady state (the window's own layout settled: the status line's note gone) the
    // scene is not rebuilt.
    // Once the picture shown is the one asked for (the worker's bake has landed: seconds in
    // a debug build), nothing more is asked for while nothing moves.
    // Settled: the glide over, the picture shown the one asked for, nothing new asked for
    // over a run of frames.
    let mut builds = scene_a(&app).builds;
    for _ in 0..20 {
        frames(&ctx, &mut app, 6);
        let now = scene_a(&app).builds;
        if now == builds && scene_a(&app).ready() {
            break;
        }
        builds = now;
    }
    assert!(scene_a(&app).ready(), "the bake came: {:?}", scene_a(&app));
    let builds = scene_a(&app).builds;
    let before = format!("{:?}", scene_a(&app));
    frames(&ctx, &mut app, 10);
    assert_eq!(
        scene_a(&app).builds,
        builds,
        "no rebuild while nothing moves: {before}
{:?}",
        scene_a(&app)
    );
    // Review #3: another Chorda opened shows its own piano at once — never the last one's,
    // nor a glide from it (its first painted frame already shows its own controls).
    let b = add_chorda(&mut app);
    app.open_instrument_window(b);
    // Its window's first frames (it opens, lays out once unseen, then paints).
    frames(&ctx, &mut app, 3);
    let spec_b = spec_of(&app, b);
    let want_b = scene::Inputs::of(&spec_b, true);
    let (shown, _) = app
        .inst
        .chorda
        .scene_of(b)
        .and_then(|s| s.showing())
        .expect("B's own scene, with a picture");
    assert_eq!(shown.look, want_b.look, "B's own look");
    assert_eq!(
        (shown.lid, shown.persp, shown.room, shown.size),
        (want_b.lid, want_b.persp, want_b.room, want_b.size),
        "B's own controls, no glide from A's"
    );
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
    cantus_engine::inst::chorda::ensure(Tuning::default(), 48_000.0);
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
