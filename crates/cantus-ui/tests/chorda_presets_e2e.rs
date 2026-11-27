//! Chorda's voicing by hand and its character presets end to end (round 42, `chorda-presets`,
//! ADR 0098's eighth amendment), on the boot path: a Chorda track added the way the instrument
//! picker adds one, a character preset applied the way the browser applies it, its editor
//! opened on the **Voicing** view and one of the voicing's knobs dragged by the pointer the
//! way a hand drags it — then the real engine's offline render: the preset sounds other than
//! the shipped Chorda, the knob moved the sound, and undone, the render returns to the bit.
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::*;
use cantus_engine::{render, EngineHandle, RenderOptions};
use cantus_ui::app::Files;
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::raster::shoot_events;
use cantus_ui::views::inst::{self, View};
use cantus_ui::CantusApp;
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

fn rendered(app: &CantusApp) -> Vec<f32> {
    let p = app.project().clone();
    render(
        Arc::new(Snapshot::compile(&p, 1)),
        &RenderOptions {
            tail_seconds: 0.3,
            ..Default::default()
        },
    )
    .unwrap()
    .samples
}

fn spec(app: &CantusApp, track: TrackId) -> NativeSpec {
    app.project()
        .track(track)
        .and_then(|t| t.instrument.native())
        .cloned()
        .expect("a native Chorda")
}

/// The energy of `x` over 6 kHz against all of it (a one-pole split, both channels summed).
fn top_share(x: &[f32]) -> f64 {
    let a = (-std::f64::consts::TAU * 6_000.0 / 48_000.0f64).exp();
    let (mut lp, mut hi, mut all) = (0.0f64, 0.0f64, 0.0f64);
    for c in x.chunks(2) {
        let v = (c[0] + c[1]) as f64;
        lp = v + a * (lp - v);
        hi += (v - lp) * (v - lp);
        all += v * v;
    }
    hi / all.max(1e-30)
}

#[test]
fn a_character_preset_and_a_voicing_knob_dragged_reach_the_engine_and_undo_to_the_bit() {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let (engine, _proc) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::assemble(&ctx, files, None, engine, None, None, None, false);
    app.apply_pick(
        PickPurpose::AddInstrument { index: None },
        PickChoice::Instrument(InstrumentKind::Chorda),
    );
    let track = app
        .project()
        .tracks
        .iter()
        .find(|t| {
            t.instrument
                .native()
                .is_some_and(|n| n.kind == InstrumentKind::Chorda)
        })
        .map(|t| t.id)
        .expect("the new Chorda track");
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
    // A phrase over the keyboard: a bass note, a middle chord, a top run.
    let mut clip = app.bus.new_clip("Keys", Ticks::ZERO, Ticks::beats(8));
    for (beat, key, vel) in [
        (0, 36, 90),
        (0, 60, 70),
        (0, 64, 70),
        (1, 84, 80),
        (2, 96, 85),
    ] {
        clip.notes.push(
            app.bus
                .new_note(Ticks::beats(beat), Ticks::beats(2), key, vel),
        );
    }
    assert!(app.exec(Command::InsertClip {
        track,
        clip: Box::new(clip),
    }));
    let shipped = rendered(&app);
    assert!(shipped.iter().any(|x| *x != 0.0), "the Chorda is silent");

    // The character preset, as the browser applies it (one undo step).
    let dark = inst::chorda::FACTORY_PRESETS
        .iter()
        .find(|p| p.label_key == "chorda.preset.v_dark")
        .expect("the Dark Felt preset");
    let target = inst::preset_spec(InstrumentKind::Chorda, dark);
    app.apply_inst_spec(track, &target, "cmd.fx_preset");
    let s = spec(&app, track);
    assert_eq!(s.get("voicing.top_hz"), target.get("voicing.top_hz"));
    assert_ne!(s.get("voicing.top_hz"), cantus_core::inst::chorda::TOP_HZ);
    let preset = rendered(&app);
    assert!(preset != shipped, "the preset did not reach the engine");
    assert!(
        top_share(&preset) < 0.5 * top_share(&shipped),
        "Dark Felt is not darker: {:.2e} against {:.2e}",
        top_share(&preset),
        top_share(&shipped)
    );

    // Its editor on the Voicing view; the Tone card's top roll-off dragged 60 px up.
    app.inst.open = Some(track);
    app.inst.view = View::Voicing;
    app.fx.pos = Some(egui::pos2(0.0, 0.0));
    cantus_ui::kit::shell::Shell::new(("inst", track.0)).set_scale(&ctx, 1.0);
    let size = cantus_ui::kit::shell::INSTRUMENT;
    shoot_events(
        &ctx,
        size,
        1.0,
        6,
        None,
        |_, _| Vec::new(),
        |ui| inst::window_in(&mut app, ui),
    );
    let at = app
        .inst
        .chorda
        .placed
        .get("voicing.top_hz")
        .expect("the Voicing view draws the top roll-off")
        .center();
    let before = spec(&app, track).get("voicing.top_hz");
    let moves = 30;
    shoot_events(
        &ctx,
        size,
        1.0,
        moves + 6,
        None,
        |f, _| {
            use egui::{Event, PointerButton};
            let y = |k: usize| at.y - 2.0 * k.min(moves) as f32;
            match f {
                0 => vec![Event::PointerMoved(at)],
                1 => vec![Event::PointerButton {
                    pos: at,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                }],
                f if f < moves + 2 => vec![Event::PointerMoved(egui::pos2(at.x, y(f - 1)))],
                f if f == moves + 2 => vec![Event::PointerButton {
                    pos: egui::pos2(at.x, y(moves)),
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                }],
                _ => Vec::new(),
            }
        },
        |ui| inst::window_in(&mut app, ui),
    );
    let after = spec(&app, track).get("voicing.top_hz");
    assert!(
        after > before,
        "the drag left the roll-off at {before} ({after})"
    );
    let moved = rendered(&app);
    assert!(moved != preset, "the knob moved no sample");
    assert!(
        top_share(&moved) > top_share(&preset),
        "a higher roll-off is not brighter"
    );

    // One undo takes the drag back, to the bit; another the preset.
    app.undo();
    assert_eq!(spec(&app, track).get("voicing.top_hz"), before);
    assert!(rendered(&app) == preset, "undone, the drag left a trace");
    app.undo();
    assert!(rendered(&app) == shipped, "undone, the preset left a trace");
}

/// The Voicing view for the owner's eyes: Dark Felt loaded (its lines off the dashed voicing),
/// in one theme and language a run (`CHORDA_SHOT` = `<lapis|paper>-<en|es>`, `CHORDA_SHOT_SIZE`; the kit's caches are
/// the process's), at the editor's own size and whole (the macros' bands under it), into
/// `CHORDA_SHOTS` (`cargo test -p cantus-ui --test chorda_presets_e2e -- --ignored shoot`).
#[test]
#[ignore]
fn shoot_the_voicing_view() {
    use cantus_ui::{views, welcome_project};
    let dir =
        std::path::PathBuf::from(std::env::var("CHORDA_SHOTS").unwrap_or_else(|_| "shots".into()));
    std::fs::create_dir_all(&dir).unwrap();
    let which = std::env::var("CHORDA_SHOT").unwrap_or_else(|_| "lapis-en".into());
    let theme = if which.starts_with("paper") {
        cantus_ui::theme::slate_lapis_paper()
    } else {
        cantus_ui::theme::slate_lapis_night()
    };
    cantus_ui::i18n::set_thread_locale(if which.ends_with("es") {
        cantus_ui::i18n::Locale::Es
    } else {
        cantus_ui::i18n::Locale::En
    });
    for (size, what) in [
        (cantus_ui::kit::shell::INSTRUMENT, "window"),
        (
            inst::size_for(egui::vec2(0.0, inst::chorda::VOICING_H + 200.0)),
            "whole",
        ),
    ] {
        // One shot a process (`CHORDA_SHOT_SIZE` = `window` or `whole`).
        if std::env::var("CHORDA_SHOT_SIZE").unwrap_or_else(|_| "window".into()) != what {
            continue;
        }
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        cantus_ui::theme::apply(&theme, &ctx);
        let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
        app.demo_editor("chorda");
        views::inst::demo_feed(&mut app, InstrumentKind::Chorda);
        let track = app.inst.open.expect("Chorda's editor open");
        let dark = inst::chorda::FACTORY_PRESETS
            .iter()
            .find(|p| p.label_key == "chorda.preset.v_dark")
            .unwrap();
        app.apply_inst_spec(
            track,
            &inst::preset_spec(InstrumentKind::Chorda, dark),
            "cmd.fx_preset",
        );
        app.inst.view = View::Voicing;
        app.fx.pos = Some(egui::pos2(0.0, 0.0));
        cantus_ui::kit::shell::Shell::new(("inst", track.0)).set_scale(&ctx, 1.0);
        let out = dir.join(format!("voicing-{which}-{what}.png"));
        cantus_ui::raster::shoot(&ctx, size, 1.0, 8, Some(&out), |ui| {
            views::inst::window_in_sized(&mut app, ui, size)
        });
    }
}
