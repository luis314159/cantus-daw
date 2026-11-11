//! The interface's size follows the screen until the user picks one (round 39, ADR 0111 §7;
//! the owner, 2026-11-09: *"the default interface scale on screens 1920 px wide is 125 %;
//! a user's saved scale is kept"*).
//!
//! **The rule.** Slate was drawn at 1 280 × 720; FL Studio's "100 %" on a 1 920-px screen
//! matches our 150 % (`dir-a`'s comparison), and the owner chose 125 % there. So the whole
//! interface — the operating system's scaling and ours together — is drawn at
//! **1.25 × (the screen's width in physical pixels ÷ 1 920)**, never under 100 % nor over
//! 200 %, and our part of it is the nearest of the zoom steps Ctrl + and Ctrl − walk:
//!
//! | Screen (physical) | OS scaling | Whole | Ours |
//! |---|---|---|---|
//! | 1 280 / 1 366 wide | 100 % | 100 % | 100 % |
//! | 1 920 | 100 % | 125 % | 125 % |
//! | 1 920 | 125 % | 125 % | 100 % |
//! | 2 560 | 100 % | 167 % | 175 % |
//! | 3 840 | 150 % | 200 % (cap) | 125 % |
//!
//! A user's own choice (Ctrl +, Ctrl −, Ctrl 0, the settings) stops the following, and a
//! settings file written before round 39 keeps the scale it holds.

use crate::settings::{self, Settings};

/// The whole interface's scale at 1 920 physical pixels across.
pub const AT_1920: f32 = 1.25;

/// Our zoom for a screen `width_px` physical pixels wide whose system already scales by
/// `native_ppp`: the rule above, snapped to the nearest zoom step.
pub fn for_screen(width_px: f32, native_ppp: f32) -> f32 {
    if !(width_px.is_finite() && width_px > 0.0) {
        return settings::DEFAULT_UI_SCALE;
    }
    let whole = (AT_1920 * width_px / 1920.0).clamp(1.0, 2.0);
    let ours = whole / native_ppp.max(0.5);
    let mut best = settings::DEFAULT_UI_SCALE;
    for s in settings::UI_SCALES {
        if (s - ours).abs() < (best - ours).abs() - 1e-4 {
            best = s;
        }
    }
    best.clamp(settings::MIN_UI_SCALE, settings::MAX_UI_SCALE)
}

/// While the settings follow the screen, the zoom the screen under the window asks for;
/// `None` when the user chose their own or the screen is not known yet.
pub fn wanted(s: &Settings, ctx: &egui::Context) -> Option<f32> {
    if !s.ui_scale_follows_screen {
        return None;
    }
    let (monitor, native) = ctx.input(|i| {
        let v = i.viewport();
        (v.monitor_size, v.native_pixels_per_point)
    });
    let native = native.unwrap_or(1.0);
    // egui gives the monitor in points of the system's scale: back to physical pixels.
    let width = monitor?.x * native;
    Some(for_screen(width, native))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rule_puts_125_percent_at_1920_and_scales_with_the_screen() {
        assert_eq!(for_screen(1920.0, 1.0), 1.25);
        assert_eq!(
            for_screen(1920.0, 1.25),
            1.0,
            "the system's 125 % is the whole"
        );
        assert_eq!(for_screen(1280.0, 1.0), 1.0, "never under 100 %");
        assert_eq!(for_screen(1366.0, 1.0), 1.0);
        assert_eq!(for_screen(2560.0, 1.0), 1.75);
        assert_eq!(for_screen(3840.0, 1.5), 1.25, "200 % whole at most");
        assert_eq!(for_screen(3840.0, 2.0), 1.0);
        assert_eq!(for_screen(f32::NAN, 1.0), settings::DEFAULT_UI_SCALE);
    }

    #[test]
    fn a_fresh_install_follows_the_screen_and_an_old_file_keeps_its_scale() {
        assert!(Settings::default().ui_scale_follows_screen);
        let old: Settings = serde_json::from_str(r#"{"ui_scale":1.0}"#).unwrap();
        assert!(!old.ui_scale_follows_screen, "a saved scale is kept");
        assert_eq!(old.ui_scale(), 1.0);
        let back: Settings =
            serde_json::from_str(&serde_json::to_string(&Settings::default()).unwrap()).unwrap();
        assert!(back.ui_scale_follows_screen);
    }

    #[test]
    fn nothing_is_wanted_until_the_screen_is_known_or_once_the_user_chose() {
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .textures_delta
            .clear();
        assert_eq!(wanted(&Settings::default(), &ctx), None);
        let mut input = egui::RawInput::default();
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .monitor_size = Some(egui::vec2(1920.0, 1080.0));
        ctx.run_ui(input, |_| {}).textures_delta.clear();
        assert_eq!(wanted(&Settings::default(), &ctx), Some(1.25));
        let chosen = Settings {
            ui_scale_follows_screen: false,
            ..Settings::default()
        };
        assert_eq!(wanted(&chosen, &ctx), None);
    }
}
