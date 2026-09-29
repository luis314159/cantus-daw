//! The workspace's share of egui's style (ADR 0024 §10, §11): scroll bars always shown,
//! 6 pt, solid; the plain wheel scrolls an area that scrolls only sideways; the keyboard
//! zoom is the application's own (Ctrl + / Ctrl − / Ctrl 0 through the keymap, clamped to
//! 75–200 % and saved), not egui's.

use crate::kit::chrome;
use egui::style::ScrollStyle;
use egui::Context;

/// Install it. Called once at startup; the theme's colours are applied separately.
pub fn install(ctx: &Context) {
    ctx.options_mut(|o| o.zoom_with_keyboard = false);
    ctx.all_styles_mut(|style| {
        style.always_scroll_the_only_direction = true;
        style.spacing.scroll = ScrollStyle {
            floating: false,
            bar_width: chrome::SCROLLBAR,
            handle_min_length: 16.0,
            bar_inner_margin: 2.0,
            bar_outer_margin: 0.0,
            ..ScrollStyle::solid()
        };
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_bars_are_solid_six_points_and_the_only_direction_scrolls() {
        let ctx = Context::default();
        install(&ctx);
        let s = ctx.global_style();
        assert!(s.always_scroll_the_only_direction);
        assert!(
            !s.spacing.scroll.floating,
            "always visible, never hover-only"
        );
        assert_eq!(s.spacing.scroll.bar_width, 6.0);
        assert!(!ctx.options(|o| o.zoom_with_keyboard));
    }
}
