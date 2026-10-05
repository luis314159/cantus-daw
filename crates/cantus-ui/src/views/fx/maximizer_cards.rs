//! Culmen's rows of cells: the selected band's, the brickwall's, and the output's
//! switches.

use super::{display, key, meters, BAND_KEYS};
use crate::kit::text::Src;
use crate::kit::Style;
use crate::views::fx::controls::{self, Size};
use crate::views::fx::host::FxHost;
use cantus_dsp::culmen::MaxSettings;
use egui::Rect;
use std::fmt::Write as _;

/// The width of a card's column of switches, at 100 %.
pub const COLUMN_W: f32 = 128.0;
/// The band card's cells, at 100 %: both sides, the knee, the times, the gain.
pub const BAND_CELLS: [Size; 10] = [Size::S; 10];

/// The band card's width at 100 %.
pub fn band_card_width() -> f32 {
    controls::cells_width(&BAND_CELLS) + 4.0 + COLUMN_W + 16.0
}

/// The selected band's cells: its two sides, its knee and times, its gain, and what part
/// of the image it works on.
pub fn band_card(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    b: usize,
    ms: &MaxSettings,
) {
    let in_use = ms.band_in_use(b);
    controls::card(ui, s, rect, Src::Key(BAND_KEYS[b]), None, |ui, _| {
        if !in_use {
            ui.disable();
        }
        let ratio = |v: f32, out: &mut String| display::write_ratio(v, out);
        controls::param_with(ui, s, host, key(b, "thr"), Size::S, Some(&num_pill));
        controls::param_with(ui, s, host, key(b, "ratio"), Size::S, Some(&ratio));
        controls::param_with(ui, s, host, key(b, "range"), Size::S, Some(&num_pill));
        controls::param_with(ui, s, host, key(b, "thr_lo"), Size::S, Some(&num_pill));
        controls::param_with(ui, s, host, key(b, "ratio_lo"), Size::S, Some(&ratio));
        controls::param_with(ui, s, host, key(b, "range_lo"), Size::S, Some(&num_pill));
        controls::param_with(ui, s, host, key(b, "knee"), Size::S, Some(&num_pill));
        controls::param_with(ui, s, host, key(b, "att"), Size::S, Some(&ms_pill));
        controls::param_with(ui, s, host, key(b, "rel"), Size::S, Some(&ms_pill));
        controls::param_with(ui, s, host, key(b, "gain"), Size::S, Some(&gain_pill));
        controls::column(ui, s, COLUMN_W, |ui| {
            controls::caption(ui, s, Src::Key("fx.max.place"), COLUMN_W);
            let labels = [
                Src::Key("mb.place.0"),
                Src::Key("mb.place.1"),
                Src::Key("mb.place.2"),
            ];
            controls::segmented_param(ui, s, host, key(b, "place"), &labels, COLUMN_W);
            if b != cantus_dsp::culmen::FULL {
                controls::led_param(
                    ui,
                    s,
                    host,
                    key(b, "solo"),
                    Src::Key("fx.max.solo"),
                    COLUMN_W,
                );
            }
        });
    });
}

/// The brickwall: the drive and the ceiling, then how far it looks ahead, how it lets go,
/// how it links the sides, and the clipper in front.
pub fn wall_card(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect) {
    controls::card(
        ui,
        s,
        rect,
        Src::Key("max.card.brickwall"),
        None,
        |ui, _| {
            controls::param(ui, s, host, "gain", Size::L);
            controls::param(ui, s, host, "ceiling", Size::M);
            controls::param_with(ui, s, host, "look", Size::S, Some(&ms_pill));
            controls::param_with(ui, s, host, "lim_att", Size::S, Some(&ms_pill));
            controls::param_with(ui, s, host, "lim_rel", Size::S, Some(&ms_pill));
            controls::param(ui, s, host, "link_t", Size::S);
            controls::param(ui, s, host, "link_r", Size::S);
            controls::param(ui, s, host, "clip", Size::S);
        },
    );
}

/// What you hear and what goes out: the monitor and the delta; the brickwall's character,
/// true peak and DC; dither and its shape.
pub fn output_card(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect) {
    controls::card(ui, s, rect, Src::Key("max.card.output"), None, |ui, _| {
        controls::column(ui, s, COLUMN_W, |ui| {
            controls::caption(ui, s, Src::Key("fx.max.monitor"), COLUMN_W);
            let labels = [
                Src::Key("max.monitor.0"),
                Src::Key("max.monitor.1"),
                Src::Key("max.monitor.2"),
            ];
            controls::segmented_param(ui, s, host, "monitor", &labels, COLUMN_W);
            controls::led_param(ui, s, host, "delta", Src::Key("fx.max.delta"), COLUMN_W);
            controls::led_param(ui, s, host, "tp", Src::Key("fx.max.tp"), COLUMN_W);
            controls::led_param(ui, s, host, "dc", Src::Key("fx.max.dc"), COLUMN_W);
        });
        controls::column(ui, s, COLUMN_W, |ui| {
            controls::caption(ui, s, Src::Key("fx.max.style"), COLUMN_W);
            meters::style_field(host, ui, s, COLUMN_W);
            controls::caption(ui, s, Src::Key("fx.max.dither"), COLUMN_W);
            meters::dither_field(host, ui, s, COLUMN_W);
            meters::shape_field(host, ui, s, COLUMN_W);
        });
    });
}

// ---- pills that fit a small cell: the number, its unit on the help line ----------------

/// `-14.0`, `12.0`: a level or a range in dB.
fn num_pill(v: f32, out: &mut String) {
    let _ = write!(out, "{v:.1}");
}

/// `+1.5`: a gain, signed.
fn gain_pill(v: f32, out: &mut String) {
    let _ = write!(out, "{v:+.1}");
}

/// `2.0 ms`, `150 ms`, `1.2 s`.
fn ms_pill(v: f32, out: &mut String) {
    let _ = if v >= 1_000.0 {
        write!(out, "{:.1} s", v / 1_000.0)
    } else if v >= 10.0 {
        write!(out, "{v:.0} ms")
    } else {
        write!(out, "{v:.1} ms")
    };
}
