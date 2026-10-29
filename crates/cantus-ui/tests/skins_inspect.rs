//! Inspect mode names the token a unit's header is painted in (round 32, the review of the
//! skin's delivery 2): in a face with an accent a modulation unit's header is painted in
//! the accent, and the eyedropper must name the accent's token, not `hue.modulation`.

use cantus_ui::i18n::{default_thread_locale, Locale};
use cantus_ui::kit::brush::Brush;
use cantus_ui::kit::color::Hsl;
use cantus_ui::kit::shell::{self, Header, Hot};
use cantus_ui::kit::skin::{Pair, Skin};
use cantus_ui::kit::{gallery, inspect, Function, Style, Tokens};
use egui::{Pos2, Rect, Vec2};

static FACE: Skin = Skin {
    accent: Some(Pair::both(Hsl {
        h: 30.0,
        s: 0.8,
        l: 0.55,
    })),
    ..Skin::plain("test.header")
};

#[test]
fn the_header_registers_the_token_it_is_painted_in() {
    default_thread_locale(Locale::En);
    let ctx = gallery::offscreen_context();
    inspect::set_enabled(&ctx, true);
    for sheet in [&FACE, &cantus_ui::kit::skin::PLAIN] {
        for f in Function::ALL {
            let s = Style::new(Tokens::slate()).skinned(sheet);
            let h = Header {
                name: "Unit",
                context: "",
                hue: f,
                preset: None,
                ab: None,
                undo: None,
                views: None,
                bypassed: None,
                help: false,
                name_w: 0.0,
                window: None,
            };
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 60.0))),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                let mut b = Brush::default();
                b.begin(ui.ctx());
                let _ = shell::paint_header(
                    &mut b,
                    Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 32.0)),
                    &s,
                    &h,
                    Hot::None,
                );
                b.flush(ui.painter());
            });
            out.textures_delta.clear();
            let items = inspect::items(&ctx);
            let header = items
                .iter()
                .find(|i| i.part == "shell_header")
                .expect("the header registered");
            let tok = header.extra[0].expect("the header's hue token");
            assert_eq!(
                s.t.get(tok),
                s.function_hue(f),
                "{} {f:?}: the eyedropper names {tok:?}, not the hue painted",
                sheet.key
            );
        }
    }
}
