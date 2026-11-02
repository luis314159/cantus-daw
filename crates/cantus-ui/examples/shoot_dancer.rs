//! Saltator's window (round 35, ADR 0087), photographed without a screen:
//! `cargo run -p cantus-ui --example shoot_dancer -- <out-dir>`. A song plays through a
//! real processor (`EngineHandle::manual`) so the stage dances what the engine publishes:
//! Play and Edit, Slate and Paper, 100 % and 150 %, English and Spanish, and a sequence of
//! the stage over two bars.

use cantus_core::inst::dancer::{self, Move};
use cantus_core::*;
use cantus_engine::{EngineHandle, Processor};
use cantus_ui::views::inst::View;
use cantus_ui::CantusApp;
use std::path::{Path, PathBuf};

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::font_fallback::enable_music();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    ctx
}

/// The starter song with its first track a dancer (the party preset, a note map clip),
/// the engine playing it.
fn setup() -> (CantusApp, Processor, TrackId) {
    let (engine, proc) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::from_parts(Some(cantus_ui::starter_project()), engine, false);
    let track = app.project().tracks[0].id;
    let mut spec = NativeSpec::new(InstrumentKind::Dancer);
    spec.set_text(
        dancer::CHARACTER_KEY,
        Some(dancer::DEFAULT_CHARACTER.into()),
    )
    .unwrap();
    spec.set("energy", 0.95).unwrap();
    app.set_instrument(track, InstrumentSpec::Native(spec));
    // A drum-like part for its ears: React to the second track's channel.
    if let Some(strip) = app.project().tracks.get(1).and_then(|t| t.strip) {
        cantus_ui::views::inst::dancer::controls::react_to(
            &mut app,
            track,
            Some(cantus_core::modulator::Listen::Strip(strip)),
        );
    }
    app.open_inst_editor(track);
    app.sync_engine();
    (app, proc, track)
}

fn run(app: &mut CantusApp, proc: &mut Processor, blocks: usize) {
    let mut buf = vec![0f32; 512 * 2];
    for _ in 0..blocks {
        proc.process(&mut buf, 2);
        app.engine.poll();
    }
}

fn shot(
    app: &mut CantusApp,
    dir: &Path,
    name: &str,
    theme: &cantus_ui::theme::Theme,
    k: f32,
    wall: f64,
) {
    let c = ctx();
    cantus_ui::theme::apply(theme, &c);
    // A new context each shot: the textures are drawn again in it.
    app.inst.dancer = Default::default();
    cantus_ui::dance_live::DanceLive::update(app, wall);
    let size = cantus_ui::kit::shell::INSTRUMENT;
    let path = dir.join(format!("{name}.png"));
    cantus_ui::raster::shoot(&c, size, k, 4, Some(&path), |ui| {
        cantus_ui::views::inst::window_in(app, ui)
    });
    println!("wrote {}", path.display());
}

fn main() {
    let dir = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "shots-dancer".into()),
    );
    std::fs::create_dir_all(&dir).expect("out dir");
    for (lang, locale) in [
        ("en", cantus_ui::i18n::Locale::En),
        ("es", cantus_ui::i18n::Locale::Es),
    ] {
        cantus_ui::i18n::set_locale(locale);
        cantus_ui::i18n::set_thread_locale(locale);
        let (mut app, mut proc, _) = setup();
        app.engine.play();
        run(&mut app, &mut proc, 300);
        for (tname, theme) in [
            ("slate", cantus_ui::theme::slate()),
            ("paper", cantus_ui::theme::paper()),
        ] {
            for (view, vname) in [(View::Play, "play"), (View::Edit, "edit")] {
                app.inst.view = view;
                for k in [1.0f32, 1.5] {
                    if lang == "es" && (k != 1.0 || tname != "slate") {
                        continue;
                    }
                    let name = format!("dancer-{vname}-{tname}-{}-{lang}", (k * 100.0) as u32);
                    shot(&mut app, &dir, &name, &theme, k, 3.0);
                }
            }
        }
    }
    // The stage through two bars at 120 BPM: a frame every half beat, notes on the map.
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let (mut app, mut proc, track) = setup();
    app.inst.view = View::Play;
    let jump = dancer::BASE_KEY + Move::Jump.index() as u8;
    app.engine.play();
    for i in 0..16 {
        if i == 6 {
            app.engine.note_on(track, jump, 120);
        }
        if i == 8 {
            app.engine.note_off(track, jump);
        }
        run(&mut app, &mut proc, 24);
        shot(
            &mut app,
            &dir,
            &format!("dancer-seq-{i:02}"),
            &cantus_ui::theme::slate(),
            1.0,
            f64::from(i) * 0.25,
        );
    }
}
