//! The preset list's starting-preset rows read whole (ADR 0138, provisional; the house rule:
//! nothing elided in English or Spanish at any scale): Saltus's editor with the person's
//! start chosen (the longest names the factory list has), its preset list open, drawn at
//! 75 %, 100 % and 200 % in both languages — the kit's text probe finds no word cut inside
//! the open list. // fx3:start

use cantus_engine::EngineHandle;
use cantus_ui::i18n::Locale;
use cantus_ui::kit::text::probe;
use cantus_ui::{views, welcome_project, CantusApp};

const KIND: cantus_core::EffectKind = cantus_core::EffectKind::PitchShift;

fn context() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
    ctx
}

#[test]
fn the_start_rows_are_never_cut() {
    for lang in [Locale::En, Locale::Es] {
        for k in [0.75f32, 1.0, 2.0] {
            cantus_ui::i18n::set_thread_locale(lang);
            let ctx = context();
            let mut app =
                CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
            app.demo_editor("saltus");
            let (_, effect, _) = app.fx.open.expect("Saltus is open");
            // The editor's state belongs to this effect (as a playing engine's feed makes it).
            views::fx::demo_feed(&mut app, KIND);
            app.settings
                .starting
                .push(cantus_ui::starting_choice::StartingChoice {
                    label: cantus_ui::i18n::t("preset.saltus.octave_down_live"),
                    spec: cantus_core::EffectSpec::new(KIND),
                });
            views::fx::frame::shell_of(KIND).set_scale(&ctx, k);
            let size = views::fx::frame::designed_size(KIND) * k;
            // The list opens under the header's preset field.
            let c = size.x * 0.5;
            app.fx.saltus.chrome.reset_for(effect.0);
            app.fx.saltus.chrome.open_presets_at(
                egui::pos2(c - 120.0 * k, 27.0 * k),
                egui::Rect::from_min_max(
                    egui::pos2(c - 120.0 * k, 4.0 * k),
                    egui::pos2(c + 120.0 * k, 26.0 * k),
                ),
            );
            let mut lists = Vec::new();
            probe::start();
            cantus_ui::raster::shoot(&ctx, size, 1.0, 3, None, |root| {
                let ctx = root.ctx().clone();
                views::fx::window(&mut app, &ctx);
                lists = ctx
                    .memory(|m| m.areas().visible_layer_ids())
                    .into_iter()
                    .filter(|l| l.order == egui::Order::Foreground)
                    .filter_map(|l| ctx.memory(|m| m.area_rect(l.id)))
                    .filter(|r| r.width() > 40.0 && r.height() > 40.0)
                    .collect();
            });
            let cuts: Vec<String> = probe::take()
                .into_iter()
                .filter(|c| lists.iter().any(|r: &egui::Rect| r.intersects(c.rect)))
                .map(|c| format!("{:?} -> {:?}", c.full, c.shown))
                .collect();
            assert!(!lists.is_empty(), "{lang:?} at {k}: the list is open");
            assert!(cuts.is_empty(), "{lang:?} at {k}: words cut: {cuts:?}");
        }
    }
}
