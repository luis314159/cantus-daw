//! Project ▸ Tuning reads whole (the house rule, ADR 0109: nothing elided, dropped,
//! abbreviated or painted past its place, in English or Spanish), as `r41_whole` holds
//! round 41's windows: the window in each of its states — a new song, a temperament in a
//! key, one following the song's key, a Scala scale of 31 notes read from outside bytes, a
//! track's own — in Slate Lapis Night and Paper at 100 %, and Night at 75, 150 and 200 %,
//! with the kit's clip-aware probe on; and the keys the tests click are where they are drawn,
//! none overlapping another. r42:tuning-face

use cantus_core::scale::{Key, ScaleKind};
use cantus_core::tuning::{Temperament, TrackTuning, Tuning, TuningSystem};
use cantus_core::Command;
use cantus_engine::EngineHandle;
use cantus_ui::i18n::{self, Locale};
use cantus_ui::kit::text::probe;
use cantus_ui::views::tuning::{self, Target};
use cantus_ui::{welcome_project, CantusApp};
use std::path::PathBuf;

const STATES: [&str; 5] = ["equal", "temperament", "follow", "scale", "track"];

const SCALES: [(f32, bool); 5] = [
    (1.0, true),
    (1.0, false),
    (0.75, true),
    (1.5, true),
    (2.0, true),
];

fn context(dark: bool) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let theme = if dark {
        cantus_ui::theme::default_theme()
    } else {
        cantus_ui::theme::default_light_theme()
    };
    cantus_ui::theme::apply(&theme, &ctx);
    ctx
}

fn seen() -> Vec<String> {
    let shorts = probe::take_shorts();
    let mut out: Vec<String> = probe::take()
        .into_iter()
        .chain(shorts)
        .map(|c| format!("{:?} -> {:?} ({})", c.full, c.shown, c.how))
        .collect();
    out.sort();
    out.dedup();
    out
}

fn app_in(state: &str) -> (CantusApp, Target) {
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    let temper = |id, key, follow| Tuning {
        a4_hz: 415.0,
        system: TuningSystem::Temperament {
            id,
            key,
            follow_song_key: follow,
        },
        map: None,
    };
    let target = match state {
        "temperament" => {
            // The longest name in either language.
            tuning::set(
                &mut app,
                Target::Project,
                temper(Temperament::MeantoneQuarter, 3, false),
            );
            Target::Project
        }
        "follow" => {
            app.exec(Command::SetProjectKey {
                key: Some(Key::new(2, ScaleKind::Minor)),
            });
            tuning::set(
                &mut app,
                Target::Project,
                temper(Temperament::Barbershop7, 0, true),
            );
            Target::Project
        }
        "scale" => {
            let scl = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../research/tuning-library-tests/31edo.scl");
            let said = tuning::io::import(&mut app, Target::Project, &scl, None);
            assert!(said.is_ok(), "{said:?}");
            app.tuning.last = Some(said);
            Target::Project
        }
        "track" => {
            let id = app.project().tracks[0].id;
            app.exec(Command::SetTrackTuning {
                track: id,
                tuning: TrackTuning::Own {
                    tuning: Tuning {
                        a4_hz: 442.0,
                        ..Tuning::default()
                    },
                },
            });
            Target::Track(id)
        }
        _ => Target::Project,
    };
    tuning::open(&mut app, target);
    (app, target)
}

/// The window drawn over an empty room at the interface's zoom `k` (pixels per point):
/// every word not read whole, and the keys the tests click.
fn probe_window(
    app: &mut CantusApp,
    k: f32,
    dark: bool,
) -> (Vec<String>, Vec<(String, egui::Rect)>) {
    let ctx = context(dark);
    let room = tuning::SIZE + egui::vec2(80.0, 80.0);
    let mut keys = Vec::new();
    probe::start_clipped();
    cantus_ui::raster::shoot(&ctx, room, k, 3, None, |root| {
        let ctx = root.ctx().clone();
        tuning::show(app, &ctx);
        keys = tuning::geo_names(&ctx)
            .into_iter()
            .filter_map(|n| tuning::geo(&ctx, &n).map(|r| (n, r)))
            .collect();
    });
    (seen(), keys)
}

#[test]
fn the_tuning_window_reads_whole_in_every_state_and_both_languages() {
    let _turn = theme_turn();
    let mut bad = Vec::new();
    for lang in [Locale::En, Locale::Es] {
        i18n::set_thread_locale(lang);
        for state in STATES {
            for (k, dark) in SCALES {
                if k != 1.0 && state != "temperament" {
                    continue;
                }
                let (mut app, _) = app_in(state);
                let (words, keys) = probe_window(&mut app, k, dark);
                for c in words {
                    bad.push(format!("{state} {lang:?} {k} dark {dark}: {c}"));
                }
                // The controls the tests click do not overlap one another.
                let controls: Vec<&(String, egui::Rect)> = keys
                    .iter()
                    .filter(|(n, _)| !n.contains('#') && !n.starts_with("note"))
                    .collect();
                for (i, (a, ra)) in controls.iter().enumerate() {
                    for (b, rb) in &controls[i + 1..] {
                        let overlap = ra.intersect(*rb);
                        if overlap.width() > 1.0 && overlap.height() > 1.0 {
                            bad.push(format!("{state} {lang:?} {k}: {a} overlaps {b}"));
                        }
                    }
                }
            }
        }
    }
    i18n::set_thread_locale(Locale::En);
    bad.sort();
    bad.dedup();
    assert!(bad.is_empty(), "not read whole:\n{}", bad.join("\n"));
}

/// This binary's tests take turns (docs/RELEASING.md L5): `theme::apply` sets the palette
/// for the whole process.
fn theme_turn() -> std::sync::MutexGuard<'static, ()> {
    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    cantus_ui::theme::apply(
        &cantus_ui::theme::default_theme(),
        &egui::Context::default(),
    );
    turn
}

/// The top bar of a song in a tuning of its own says it whole: the name, the song's line and
/// the tuning after it ("4 tracks · 4 bars · 0:08 · Werckmeister III in Eb · A415"), in
/// both themes and both languages, on a 1440-point bar.
#[test]
fn the_top_bar_says_a_tuned_songs_line_whole() {
    let _turn = theme_turn();
    let mut bad = Vec::new();
    for lang in [Locale::En, Locale::Es] {
        i18n::set_thread_locale(lang);
        for dark in [true, false] {
            let (mut app, _) = app_in("temperament");
            tuning::close(&mut app);
            let ctx = context(dark);
            probe::start_clipped();
            let mut full = 0.0;
            cantus_ui::raster::shoot(&ctx, egui::vec2(1440.0, 44.0), 1.0, 4, None, |ui| {
                cantus_ui::views::transport::show(&mut app, ui);
                full = cantus_ui::views::transport::room(ui.ctx(), 1440.0).left_max;
            });
            for c in seen() {
                if c.contains("A415") || c.contains("Meso") || c.contains("meantone") {
                    bad.push(format!("{lang:?} dark {dark} (menus block {full:.1}): {c}"));
                }
            }
        }
    }
    i18n::set_thread_locale(Locale::En);
    assert!(bad.is_empty(), "not read whole:\n{}", bad.join("\n"));
}

/// Chorda's Temperament card reads whole (the owner's ask of 2026-11-22): Chorda's Edit view
/// with its own *Werckmeister III* in E, and following the project's tuning, in both themes
/// at 100 % and dark at 75, 150 and 200 %, in both languages — every word cut there that is
/// not cut in Chorda's Edit view of a song with no tuning (what the card added).
#[test]
fn chordas_temperament_card_reads_whole_in_both_languages() {
    use cantus_core::InstrumentKind;
    use cantus_ui::kit::shell::Shell;
    use cantus_ui::views::inst::View;
    let _turn = theme_turn();
    let kind = InstrumentKind::Chorda;
    cantus_ui::views::inst::chorda::scene::set_blocking(true);
    let mut bad = Vec::new();
    for lang in [Locale::En, Locale::Es] {
        i18n::set_thread_locale(lang);
        for (k, dark) in SCALES {
            let seen_in = |state: &str| {
                let ctx = context(dark);
                let mut app =
                    CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
                app.demo_editor("chorda");
                let track = app.inst.open.expect("Chorda is open");
                match state {
                    "own" => {
                        let spec = app
                            .project()
                            .track(track)
                            .and_then(|t| t.instrument.native().cloned())
                            .unwrap();
                        let p = tuning::chorda::PRESETS
                            .iter()
                            .find(|p| p.name == "tuning.temper.meantone_quarter")
                            .unwrap();
                        app.exec(tuning::chorda::preset_command(&spec, track, p, 4));
                    }
                    "project" => {
                        tuning::set(
                            &mut app,
                            Target::Project,
                            Tuning {
                                a4_hz: 415.0,
                                system: TuningSystem::Temperament {
                                    id: Temperament::Barbershop7,
                                    key: 3,
                                    follow_song_key: false,
                                },
                                map: None,
                            },
                        );
                    }
                    _ => {}
                }
                app.inst.view = View::Edit;
                Shell::new(("inst", track.0)).set_scale(&ctx, k);
                let size = cantus_ui::views::inst::editor_size(kind) * k;
                probe::start_clipped();
                cantus_ui::raster::shoot(&ctx, size, 1.0, 3, None, |ui| {
                    cantus_ui::views::inst::window_in(&mut app, ui);
                });
                seen()
            };
            let plain = seen_in("plain");
            for state in ["own", "project"] {
                for c in seen_in(state) {
                    if !plain.contains(&c) {
                        bad.push(format!("chorda {state} {lang:?} {k} dark {dark}: {c}"));
                    }
                }
            }
        }
    }
    i18n::set_thread_locale(Locale::En);
    assert!(bad.is_empty(), "not read whole:\n{}", bad.join("\n"));
}
