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

/// The monitor's width in physical pixels from egui's `monitor_size`, which egui-winit
/// gives in points of **the system's scale times the zoom in effect before the pass**
/// (`egui_winit::update_viewport_info` divides by `egui_winit::pixels_per_point`, read
/// before `run`; a zoom set during a pass becomes active at the start of the next one).
///
/// GitHub issue #1 (ADR 0149, provisional number): this was read as points of the system's
/// scale alone, so the zoom chosen changed the width measured, which changed the zoom —
/// 1.0 ↔ 1.25 every other frame on a 1 920-px screen at 100 %, the whole interface
/// flickering along both axes from the first start of a fresh installation.
pub fn monitor_px(monitor_points: f32, zoom_used: f32, native_ppp: f32) -> f32 {
    monitor_points * zoom_used * native_ppp
}

/// The screen-following scale's state (ADR 0111 §7, ADR 0149): one decision per screen,
/// measured only while the zoom has held for a pass, so the zoom it sets can never change
/// what it measures.
#[derive(Clone, Debug, Default)]
pub struct Follow {
    /// The zoom in effect in the previous pass: the one egui-winit measured this pass's
    /// monitor with, when it equals this pass's.
    prev_zoom: Option<f32>,
    /// The screen the last decision was made for: its width in physical pixels and the
    /// system's scale (as bits).
    decided: Option<(u32, u32)>,
    /// A scale fixed for the session (`--scale`, `--safe-graphics`): set once instead of
    /// following the screen, never stored; Ctrl + and Ctrl − still change it.
    pinned: Option<f32>,
    pin_set: bool,
    /// The window's watchdog tripped (`window_watch`): nothing more is followed.
    stopped: bool,
}

impl Follow {
    /// A scale for this session only, whatever the screen and the settings say.
    pub fn pin(&mut self, scale: f32) {
        if scale.is_finite() {
            self.pinned = Some(scale.clamp(settings::MIN_UI_SCALE, settings::MAX_UI_SCALE));
        }
    }

    pub fn pinned(&self) -> Option<f32> {
        self.pinned
    }

    /// Stop following: the zoom in effect stays (the watchdog's way to break a loop).
    pub fn stop(&mut self) {
        self.stopped = true;
    }

    pub fn stopped(&self) -> bool {
        self.stopped
    }

    /// Once a frame: the zoom to set now, if any. A pinned scale once; else, while the
    /// settings follow the screen, the zoom the screen under the window asks for, decided
    /// once per screen; `None` when the user chose their own, the screen is not known yet,
    /// or the zoom changed since the previous pass (the monitor was measured with the
    /// other one: wait a pass).
    pub fn wanted(&mut self, s: &Settings, ctx: &egui::Context) -> Option<f32> {
        let zoom = ctx.zoom_factor();
        let prev = self.prev_zoom.replace(zoom);
        if self.stopped {
            return None;
        }
        if let Some(p) = self.pinned {
            if self.pin_set {
                return None;
            }
            self.pin_set = true;
            return Some(p);
        }
        if !s.ui_scale_follows_screen || prev != Some(zoom) {
            return None;
        }
        let (monitor, native) = ctx.input(|i| {
            let v = i.viewport();
            (v.monitor_size, v.native_pixels_per_point)
        });
        let native = native.unwrap_or(1.0);
        let width = monitor_px(monitor?.x, zoom, native);
        if !(width.is_finite() && width > 0.0) {
            return None;
        }
        let key = (width.round() as u32, native.to_bits());
        if self.decided == Some(key) {
            return None;
        }
        self.decided = Some(key);
        Some(for_screen(width, native))
    }
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
        let mut f = Follow::default();
        assert_eq!(f.wanted(&Settings::default(), &ctx), None);
        let mut input = egui::RawInput::default();
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .monitor_size = Some(egui::vec2(1920.0, 1080.0));
        ctx.run_ui(input, |_| {}).textures_delta.clear();
        // The zoom held for a pass: the screen is measured, decided once.
        assert_eq!(f.wanted(&Settings::default(), &ctx), Some(1.25));
        assert_eq!(f.wanted(&Settings::default(), &ctx), None, "decided once");
        let chosen = Settings {
            ui_scale_follows_screen: false,
            ..Settings::default()
        };
        assert_eq!(Follow::default().wanted(&chosen, &ctx), None);
    }

    /// Issue #1: frames fed as egui-winit feeds them (the monitor in points of the zoom in
    /// effect before the pass): the zoom settles on the rule's scale within a few frames
    /// and never moves again, on every screen and system scale — where the old reading
    /// (the monitor taken as points of the system's scale alone) went 1.0 ↔ 1.25 forever
    /// on a 1 920-px screen at 100 %.
    #[test]
    fn the_followed_zoom_settles_and_never_measures_itself() {
        for width in [
            1280.0, 1366.0, 1920.0, 2048.0, 2560.0, 2880.0, 3200.0, 3840.0,
        ] {
            for native in [1.0_f32, 1.25, 1.5, 1.75, 2.0] {
                let ctx = egui::Context::default();
                let mut f = Follow::default();
                let s = Settings::default();
                let mut zooms = Vec::new();
                for _ in 0..40 {
                    let ppp = ctx.zoom_factor() * native;
                    let mut input = egui::RawInput::default();
                    let vp = input.viewports.entry(egui::ViewportId::ROOT).or_default();
                    vp.native_pixels_per_point = Some(native);
                    vp.monitor_size = Some(egui::vec2(width, 1080.0) / ppp);
                    ctx.run_ui(input, |ui| {
                        if let Some(z) = f.wanted(&s, ui.ctx()) {
                            ui.ctx().set_zoom_factor(z);
                        }
                    })
                    .textures_delta
                    .clear();
                    zooms.push(ctx.zoom_factor());
                }
                let want = for_screen(width, native);
                assert!(
                    zooms[6..].iter().all(|z| *z == want),
                    "{width} px at {native}: {zooms:?}, want {want}"
                );
            }
        }
    }
}
