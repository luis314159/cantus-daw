//! The design system: typography, the 4-px spacing grid, elevation and focus rings.
//!
//! [`theme`](crate::theme) owns *colour* — which is user data, editable in `theme.json`.
//! This module owns *form*: the type scale, the spacing rhythm, how deep a surface sits
//! and how keyboard focus is shown. Those are product decisions, not user settings, so
//! they live in code and stay consistent across every theme.
//!
//! Three rules hold the look together:
//!
//! 1. **Everything lands on a 4-px grid.** Use [`space`] constants, never a bare literal.
//! 2. **Numbers are monospaced.** Bars, beats, dB, hertz and milliseconds must not shift
//!    horizontally as they count, so they use [`mono`] / [`readout`], not the body font.
//! 3. **Focus is always visible.** egui renders a keyboard-focused widget with the same
//!    visuals as a pressed one, so [`paint_focus_ring`] draws an accent ring around
//!    whatever holds focus, once per frame, for every widget in the app.

use crate::theme::Palette;
use egui::{Context, CornerRadius, FontFamily, FontId, Margin, Shadow, Stroke, TextStyle, Vec2};
use std::sync::Arc;
use std::sync::OnceLock;

// ---- spacing ------------------------------------------------------------------------

/// The 4-px grid. Every margin, gap and control height in Cantus is a multiple of it.
pub mod space {
    /// The base unit. All other spacing is a whole multiple of this.
    pub const GRID: f32 = 4.0;

    /// `GRID` × 1 — hairline gaps, the space inside a dense control.
    pub const S1: f32 = GRID;
    /// `GRID` × 2 — the default gap between related controls.
    pub const S2: f32 = GRID * 2.0;
    /// `GRID` × 3 — padding inside a card.
    pub const S3: f32 = GRID * 3.0;
    /// `GRID` × 4 — padding inside a panel.
    pub const S4: f32 = GRID * 4.0;
    /// `GRID` × 6 — the gap between sections.
    pub const S6: f32 = GRID * 6.0;
    /// `GRID` × 8 — the gap between major regions.
    pub const S8: f32 = GRID * 8.0;

    /// Height of a standard interactive row (button, drag value, combo).
    pub const ROW: f32 = GRID * 6.0;
    /// Height of a compact row used in dense strips (mixer, track headers).
    pub const ROW_DENSE: f32 = GRID * 5.0;

    /// Snap a length to the grid, so runtime-computed sizes stay in rhythm.
    pub fn snap(v: f32) -> f32 {
        (v / GRID).round() * GRID
    }
}

/// Size of an icon-only button, on the grid.
pub const ICON_BUTTON: Vec2 = Vec2::new(space::S8, space::ROW);

// ---- typography ---------------------------------------------------------------------

/// `font_data` keys for the bundled IBM Plex faces (SIL OFL 1.1, see
/// `assets/fonts/LICENSE-OFL.txt`).
const SANS: &str = "cantus-sans";
const SANS_STRONG: &str = "cantus-sans-strong";
const MONO: &str = "cantus-mono";
const MONO_STRONG: &str = "cantus-mono-strong";

/// Named [`FontFamily`] for semibold sans — headings and emphasis.
pub fn family_strong() -> FontFamily {
    static F: OnceLock<FontFamily> = OnceLock::new();
    F.get_or_init(|| FontFamily::Name("strong".into())).clone()
}

/// Named [`FontFamily`] for medium mono — numeric readouts that must carry weight.
pub fn family_mono_strong() -> FontFamily {
    static F: OnceLock<FontFamily> = OnceLock::new();
    F.get_or_init(|| FontFamily::Name("mono-strong".into()))
        .clone()
}

fn named(name: &'static str) -> TextStyle {
    TextStyle::Name(Arc::from(name))
}

/// Dense metadata: port names, counts, hints. Smallest legible size.
pub fn caption() -> TextStyle {
    named("caption")
}

/// Body text at semibold — section labels and the emphasised half of a pair.
pub fn strong() -> TextStyle {
    named("strong")
}

/// Monospace at caption size — inline numbers inside dense strips.
pub fn mono_small() -> TextStyle {
    named("mono-small")
}

/// The hero numeric display: the transport position. Monospace so digits never shift.
pub fn readout() -> TextStyle {
    named("readout")
}

/// A secondary numeric display: tempo, dB values in the mixer.
pub fn readout_small() -> TextStyle {
    named("readout-small")
}

/// Every named style this module installs. Views must only ask for these; the unit test
/// asserts the list matches what [`install`] actually puts in the style map.
pub fn named_styles() -> Vec<TextStyle> {
    vec![
        caption(),
        strong(),
        mono_small(),
        readout(),
        readout_small(),
    ]
}

/// The type scale. Sizes are chosen against IBM Plex's x-height, not picked at random:
/// each step is far enough from the last to read as a different level at 100 % scaling.
fn text_styles() -> std::collections::BTreeMap<TextStyle, FontId> {
    use FontFamily::{Monospace, Proportional};
    [
        (named("caption"), FontId::new(10.5, Proportional)),
        (TextStyle::Small, FontId::new(11.0, Proportional)),
        (TextStyle::Body, FontId::new(13.0, Proportional)),
        (TextStyle::Button, FontId::new(13.0, Proportional)),
        (named("strong"), FontId::new(13.0, family_strong())),
        (TextStyle::Heading, FontId::new(16.0, family_strong())),
        (TextStyle::Monospace, FontId::new(12.0, Monospace)),
        (named("mono-small"), FontId::new(10.5, Monospace)),
        (
            named("readout-small"),
            FontId::new(13.0, family_mono_strong()),
        ),
        (named("readout"), FontId::new(19.0, family_mono_strong())),
    ]
    .into_iter()
    .collect()
}

/// Install the bundled fonts. Keeps egui's emoji faces as a fallback so a stray symbol
/// still resolves to something rather than a tofu box.
pub fn install_fonts(ctx: &Context) {
    let mut fonts = egui::FontDefinitions::default();
    let mut add = |key: &str, bytes: &'static [u8]| {
        fonts
            .font_data
            .insert(key.to_owned(), Arc::new(egui::FontData::from_static(bytes)));
    };
    add(
        SANS,
        include_bytes!("../assets/fonts/IBMPlexSans-Regular.ttf"),
    );
    add(
        SANS_STRONG,
        include_bytes!("../assets/fonts/IBMPlexSans-SemiBold.ttf"),
    );
    add(
        MONO,
        include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf"),
    );
    add(
        MONO_STRONG,
        include_bytes!("../assets/fonts/IBMPlexMono-Medium.ttf"),
    );

    // egui's defaults left in place after ours act as the fallback chain.
    let fallback: Vec<String> = fonts
        .families
        .get(&FontFamily::Proportional)
        .map(|v| v.iter().filter(|n| n.contains("emoji")).cloned().collect())
        .unwrap_or_default();
    let chain = |first: &str| {
        let mut v = vec![first.to_owned()];
        v.extend(fallback.iter().cloned());
        v
    };
    fonts.families.insert(FontFamily::Proportional, chain(SANS));
    fonts.families.insert(FontFamily::Monospace, chain(MONO));
    fonts.families.insert(family_strong(), chain(SANS_STRONG));
    fonts
        .families
        .insert(family_mono_strong(), chain(MONO_STRONG));
    ctx.set_fonts(fonts);
}

/// Fonts for text painted straight onto a canvas with a [`Painter`](egui::Painter) —
/// rulers, clip names, note labels. Those call sites have no widget to resolve a
/// [`TextStyle`] against, so they name a font here instead of inventing a size.
pub mod paint {
    use super::{family_mono_strong, FontFamily, FontId};

    /// Bar numbers and pitch names. Monospaced: a ruler whose digits change width as it
    /// scrolls reads as jitter.
    pub fn ruler() -> FontId {
        FontId::new(10.0, FontFamily::Monospace)
    }

    /// A name drawn inside a shape: a clip, a track header.
    pub fn label() -> FontId {
        FontId::new(11.0, FontFamily::Proportional)
    }

    /// A lane's own caption, drawn in its corner.
    pub fn caption() -> FontId {
        FontId::new(10.0, FontFamily::Proportional)
    }

    /// Empty-state text drawn in the middle of a canvas.
    pub fn empty_state() -> FontId {
        FontId::new(13.0, family_mono_strong())
    }
}

// ---- style --------------------------------------------------------------------------

/// Install the type scale and the spacing grid. Colour comes separately from
/// [`theme::apply`](crate::theme::apply), which is called on every theme reload; this is
/// called once at startup and whenever the fonts change.
pub fn install_style(ctx: &Context) {
    // The type scale and the grid are the same in every theme — only colour swaps — so
    // this writes both of egui's light and dark styles.
    ctx.all_styles_mut(|style| {
        style.text_styles = text_styles();

        let s = &mut style.spacing;
        s.item_spacing = Vec2::new(space::S2, space::S1);
        s.button_padding = Vec2::new(space::S2, space::S1);
        s.menu_margin = Margin::same(space::S1 as i8);
        s.indent = space::S4;
        s.interact_size = Vec2::new(space::S8, space::ROW);
        s.slider_width = space::GRID * 32.0;
        s.combo_width = space::GRID * 24.0;
        s.text_edit_width = space::GRID * 45.0;
        s.icon_width = space::S4;
        s.icon_width_inner = space::S2;
        s.icon_spacing = space::S1;
        s.window_margin = Margin::same(space::S3 as i8);
        s.scroll.bar_width = space::S2;
        s.scroll.bar_inner_margin = space::S1;
        s.scroll.bar_outer_margin = 0.0;

        // A DAW is read at a glance: tooltips should not make the user wait.
        style.interaction.tooltip_delay = 0.35;
        style.interaction.show_tooltips_only_when_still = false;
    });
}

// ---- elevation ----------------------------------------------------------------------

/// How far a surface sits above the window's ground. Higher levels get a lighter fill and
/// a stronger edge — never a drop shadow inside the layout, which would smear the dense
/// grids the piano roll and arrangement draw.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Elevation {
    /// The ground: the canvas that the piano roll and arrangement paint on.
    Canvas,
    /// A chrome panel: the transport, the status bar, the inspector.
    Panel,
    /// A card inside a panel: an effect slot, a mixer strip, a settings group.
    Card,
}

impl Elevation {
    /// The frame for this surface, in the given palette.
    pub fn frame(self, pal: &Palette) -> egui::Frame {
        match self {
            Elevation::Canvas => egui::Frame::NONE.fill(pal.ink),
            Elevation::Panel => egui::Frame::NONE
                .fill(pal.panel)
                .inner_margin(Margin::symmetric(space::S2 as i8, space::S1 as i8)),
            Elevation::Card => egui::Frame::NONE
                .fill(pal.panel_raised)
                .stroke(Stroke::new(1.0, pal.grid_strong))
                .corner_radius(CornerRadius::same(pal.corner_radius))
                .inner_margin(Margin::same(space::S2 as i8))
                .outer_margin(Margin::symmetric(0, space::S1 as i8)),
        }
    }
}

/// A hairline separating two surfaces. Painting it as part of a panel's frame — rather
/// than as an `ui.separator()` inside it — keeps it flush with the panel edge.
pub fn hairline(pal: &Palette) -> Stroke {
    Stroke::new(1.0, pal.grid_strong)
}

/// The shadow used for surfaces that float over the layout: windows and menus.
pub fn overlay_shadow(pal: &Palette) -> Shadow {
    Shadow {
        offset: [0, 2],
        blur: 12,
        spread: 0,
        color: pal.shadow,
    }
}

// ---- focus --------------------------------------------------------------------------

/// Width of the focus ring's stroke.
const FOCUS_WIDTH: f32 = 2.0;
/// How far the ring sits outside the widget, so it reads as a ring and not a border.
const FOCUS_OFFSET: f32 = 1.5;

/// The rectangle the focus ring is drawn on for a widget occupying `rect`.
pub fn focus_ring_rect(rect: egui::Rect) -> egui::Rect {
    rect.expand(FOCUS_OFFSET)
}

/// Draw a ring around whatever widget currently holds keyboard focus.
///
/// egui's [`Widgets::style`](egui::style::Widgets::style) returns the *active* visuals for
/// a focused widget, which makes a tabbed-to button look pressed and an untabbed one look
/// the same as it did — so keyboard navigation is invisible without this. Painting it once
/// per frame on a foreground layer, from the focused id's stored response, covers every
/// widget in the app including ones added later, with no call-site changes.
///
/// Call at the end of the frame, after every panel has been laid out, so the response for
/// the focused widget exists in this frame's memory.
pub fn paint_focus_ring(ctx: &Context, pal: &Palette) {
    let Some(id) = ctx.memory(|m| m.focused()) else {
        return;
    };
    let Some(resp) = ctx.read_response(id) else {
        return;
    };
    // Text edits draw their own focus indication and are usually flush against a label;
    // an extra ring around them reads as a rendering bug.
    if !resp.sense.interactive() || resp.rect.width() <= 0.0 {
        return;
    }
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("cantus-focus-ring"),
    ));
    painter.rect_stroke(
        focus_ring_rect(resp.rect),
        CornerRadius::same(pal.corner_radius + 2),
        Stroke::new(FOCUS_WIDTH, pal.focus),
        egui::StrokeKind::Outside,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_named_style_is_installed() {
        let ctx = Context::default();
        install_style(&ctx);
        let style = ctx.style_of(egui::Theme::Dark);
        for name in named_styles() {
            assert!(
                style.text_styles.contains_key(&name),
                "views can ask for {name:?} but install_style does not define it"
            );
        }
        // And the built-in styles keep their Cantus sizes rather than egui's defaults.
        assert_eq!(style.text_styles[&TextStyle::Body].size, 13.0);
        assert_eq!(
            style.text_styles[&TextStyle::Heading].family,
            family_strong()
        );
    }

    #[test]
    fn spacing_lands_on_the_grid() {
        let ctx = Context::default();
        install_style(&ctx);
        let style = ctx.style_of(egui::Theme::Dark);
        let s = &style.spacing;
        let on_grid = |v: f32| (v / space::GRID).fract() == 0.0;
        for (what, v) in [
            ("item_spacing.x", s.item_spacing.x),
            ("item_spacing.y", s.item_spacing.y),
            ("button_padding.x", s.button_padding.x),
            ("button_padding.y", s.button_padding.y),
            ("indent", s.indent),
            ("interact_size.y", s.interact_size.y),
            ("slider_width", s.slider_width),
            ("icon_width", s.icon_width),
            ("scroll.bar_width", s.scroll.bar_width),
        ] {
            assert!(on_grid(v), "{what} = {v} is off the 4-px grid");
        }
        assert_eq!(space::snap(13.0), 12.0);
        assert_eq!(space::snap(14.0), 16.0);
    }

    #[test]
    fn fonts_load_and_resolve_latin_and_accents() {
        let ctx = Context::default();
        install_fonts(&ctx);
        install_style(&ctx);
        // Fonts are only realized once a frame has run; no GPU here, so acknowledge the
        // font-atlas upload epaint expects a renderer to take.
        ctx.run_ui(Default::default(), |_| {})
            .textures_delta
            .clear();
        // Spanish needs the accented forms; a missing face would lay out as zero width.
        for family in [
            FontFamily::Proportional,
            FontFamily::Monospace,
            family_strong(),
            family_mono_strong(),
        ] {
            let w = ctx.fonts_mut(|f| {
                f.layout_no_wrap(
                    "Añadir efecto 1234".into(),
                    FontId::new(13.0, family.clone()),
                    egui::Color32::WHITE,
                )
                .size()
                .x
            });
            assert!(w > 50.0, "{family:?} laid out suspiciously narrow: {w}");
        }
    }

    /// Every rect stroked in `color`, however deeply nested in the shape tree.
    fn strokes_of(shapes: &[egui::epaint::ClippedShape], color: egui::Color32) -> usize {
        fn walk(shape: &egui::Shape, color: egui::Color32, n: &mut usize) {
            match shape {
                egui::Shape::Rect(r) if r.stroke.color == color => *n += 1,
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, color, n)),
                _ => {}
            }
        }
        let mut n = 0;
        for s in shapes {
            walk(&s.shape, color, &mut n);
        }
        n
    }

    #[test]
    fn a_focused_widget_gets_a_ring_and_an_unfocused_one_does_not() {
        // egui paints a focused widget with its *active* visuals, so without this ring
        // keyboard navigation is invisible. This is the test that says it is not.
        let ctx = Context::default();
        install_style(&ctx);
        let pal = crate::theme::ink_and_gold_palette();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(400.0, 200.0),
            )),
            ..Default::default()
        };

        // Nothing focused: no ring.
        let mut out = ctx.run_ui(input.clone(), |ui| {
            let _ = ui.button("focus me");
            paint_focus_ring(ui.ctx(), &pal);
        });
        out.textures_delta.clear();
        assert_eq!(
            strokes_of(&out.shapes, pal.focus),
            0,
            "a ring was painted with nothing focused"
        );

        // Focus the button, the way Tab would.
        let mut out = ctx.run_ui(input.clone(), |ui| {
            ui.button("focus me").request_focus();
        });
        out.textures_delta.clear();

        // Now it wears the ring.
        let mut out = ctx.run_ui(input, |ui| {
            let _ = ui.button("focus me");
            paint_focus_ring(ui.ctx(), &pal);
        });
        out.textures_delta.clear();
        assert_eq!(
            strokes_of(&out.shapes, pal.focus),
            1,
            "the focused widget should wear exactly one focus ring"
        );
    }

    #[test]
    fn focus_ring_sits_outside_the_widget() {
        let r = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(40.0, 24.0));
        let ring = focus_ring_rect(r);
        assert!(ring.contains_rect(r));
        assert!(ring.min.x < r.min.x && ring.max.y > r.max.y);
    }
}
