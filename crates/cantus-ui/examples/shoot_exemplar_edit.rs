//! r42 fix3-suite: Exemplar's Edit view whole — its cards, the Legato card among them — in
//! Slate Lapis Night and Slate Lapis Paper, at the editor's 100 % and 150 %, at its own width
//! and at the narrowest window it allows (three quarters of the shell's width), in English and
//! Spanish: the look of what the round added, checked by eye (CLAUDE.md).
//!
//! `cargo run -p cantus-ui --example shoot_exemplar_edit [-- out_dir]`
//!
//! Writes `exemplar-edit-<theme>-<scale>-<width>-<lang>.png` into `out_dir` (default
//! `target/shoot-exemplar-edit`).

use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec};
use cantus_engine::EngineHandle;
use cantus_ui::kit::shell::{self, Shell};
use cantus_ui::views::inst::{self, View};
use cantus_ui::{raster, welcome_project, CantusApp};
use std::path::PathBuf;

/// The Edit view's height whole (its cards, the zone table, the log): no scrolling.
const TALL: f32 = 1520.0;

/// A theme by its constructor.
type ThemeFn = fn() -> cantus_ui::theme::Theme;

fn main() {
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/shoot-exemplar-edit")
        });
    std::fs::create_dir_all(&dir).expect("create the output directory");
    let themes: [(&str, ThemeFn); 2] = [
        ("lapis-night", cantus_ui::theme::slate_lapis_night),
        ("lapis-paper", cantus_ui::theme::slate_lapis_paper),
    ];
    for lang in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        cantus_ui::i18n::set_thread_locale(lang);
        for (theme_name, theme) in themes {
            for k in [1.0f32, 1.5] {
                for (width_name, width) in [
                    ("default", shell::INSTRUMENT.x),
                    ("narrowest", shell::INSTRUMENT.x * 0.75),
                ] {
                    let ctx = egui::Context::default();
                    cantus_ui::design::install_fonts(&ctx);
                    cantus_ui::design::install_style(&ctx);
                    cantus_ui::theme::apply(&theme(), &ctx);
                    let mut app = CantusApp::from_parts(
                        Some(welcome_project()),
                        EngineHandle::dummy(),
                        false,
                    );
                    let track = app.project().tracks[0].id;
                    app.set_instrument(
                        track,
                        InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Sampler)),
                    );
                    inst::demo_setup(&mut app, track, InstrumentKind::Sampler);
                    app.open_inst_editor(track);
                    app.inst.view = View::Edit;
                    Shell::new(("inst", track.0)).set_scale(&ctx, k);
                    let size = egui::vec2(width * k, TALL * k);
                    let path = dir.join(format!(
                        "exemplar-edit-{theme_name}-{:.0}-{width_name}-{}.png",
                        k * 100.0,
                        lang.code()
                    ));
                    raster::shoot_at(
                        &ctx,
                        size,
                        1.0,
                        8,
                        Some(&path),
                        |_| None,
                        |ui| inst::window_in_sized(&mut app, ui, size),
                    );
                    println!("{}", path.display());
                }
            }
        }
    }
}
