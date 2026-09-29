//! The mock screens: each paints a whole image with the kit's parts only.

use cantus_ui::kit::{Brush, Style};
use egui::{Painter, Vec2};

pub type Paint = fn(&mut Brush, &Painter, &Style, Vec2);
pub type PaintUi = fn(&mut egui::Ui, &Style);

/// Screens painted straight through a brush.
pub const SCREENS: [(&str, Vec2, Paint); 4] = [
    ("a-mixer", crate::mixer::SIZE, crate::mixer::paint),
    (
        "b-inspector",
        crate::inspector::SIZE,
        crate::inspector::paint,
    ),
    ("d-splash", crate::splash::SPLASH, crate::splash::splash),
    ("e-logos", crate::splash::SHEET, crate::splash::sheet),
];

/// Screens built from widgets (the device shell).
pub const UI_SCREENS: [(&str, Vec2, PaintUi); 2] = [
    ("c-device", crate::device::SIZE, crate::device::aequalis),
    (
        "c-instrument",
        crate::device::INSTRUMENT,
        crate::device::calor,
    ),
];
