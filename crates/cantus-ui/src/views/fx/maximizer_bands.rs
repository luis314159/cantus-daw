//! Culmen's bands on the spectrum they share, and the selected band's two-sided curve —
//! both of them controls.

use super::{key, State, BAND_KEYS};
use crate::kit::color::{safe_track, FillSize};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{Role, Src};
use crate::kit::{with_brush, Fill, Style};
use crate::views::fx::canvas::{self, Ink};
use crate::views::fx::controls;
use crate::views::fx::frame;
use crate::views::fx::host::FxHost;
use cantus_dsp::culmen::{MaxSettings, FULL, HI, LO, MID, MIN_GAP};
use cantus_dsp::multiplex::{Mode, Slope};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Sense};

/// What a drag holds.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Grab {
    /// The ceiling line on the history.
    Ceiling,
    /// A split on the band strip: 0 is `x1`, 1 is `x2`.
    Split(usize),
    /// A threshold point on the curve: the lower one when `true`.
    Thr(bool),
    /// A side's slope on the curve: the lower one when `true`.
    Ratio(bool),
}

const F_LO: f32 = 20.0;
const F_HI: f32 = 20_000.0;
/// The curve's scale, dB, on both axes.
pub const CURVE_LO: f32 = -60.0;
/// A band's gain bar on the strip holds this many dB either way.
const BAR_SPAN: f32 = 12.0;

/// A band's colour as the displays draw it: Multiplex's band colours, kept apart from the
/// device's hue, clamped to the theme's safe band.
pub fn band_ink(band: usize, s: &Style) -> Color32 {
    safe_track(
        crate::views::fx::multiband::band_color(band),
        s.t.dark,
        FillSize::Small,
    )
}

/// Where a frequency sits across `r`.
pub fn x_of(hz: f32, r: Rect) -> f32 {
    let t = (hz.clamp(F_LO, F_HI) / F_LO).ln() / (F_HI / F_LO).ln();
    r.left() + t * r.width()
}

fn hz_at(x: f32, r: Rect) -> f32 {
    let t = ((x - r.left()) / r.width().max(1.0)).clamp(0.0, 1.0);
    F_LO * (F_HI / F_LO).powf(t)
}

/// Where a level pair lands on the curve.
pub fn curve_pos(r: Rect, input_db: f32, output_db: f32) -> Pos2 {
    let span = -CURVE_LO;
    pos2(
        r.left() + ((input_db - CURVE_LO) / span).clamp(0.0, 1.0) * r.width(),
        r.bottom() - ((output_db - CURVE_LO) / span).clamp(0.0, 1.0) * r.height(),
    )
}

/// The bands in use, in frequency order, each with its edges in Hz.
fn layout(ms: &MaxSettings) -> Vec<(usize, f32, f32)> {
    let (x1, x2) = ms.crossovers();
    match ms.count {
        3 => vec![(LO, F_LO, x1), (MID, x1, x2), (HI, x2, F_HI)],
        2 => vec![(LO, F_LO, x1), (HI, x1, F_HI)],
        _ => Vec::new(),
    }
}

/// The band strip: the spectrum as the bands share it, each band's gain now as a bar, the
/// splits you drag, and the full band's column at the right.
pub fn strip(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    ms: &MaxSettings,
    tel: &[f32; 64],
    rect: Rect,
) {
    let k = s.k;
    let tk = s.tk();
    let ink = Ink::of(s);
    let full_w = 72.0 * k;
    let spec_r = Rect::from_min_max(
        rect.min,
        pos2(rect.right() - full_w - 4.0 * k, rect.bottom()),
    );
    let full_r = Rect::from_min_max(pos2(rect.right() - full_w, rect.top()), rect.max);
    st.geometry.strip = spec_r;
    st.geometry.full = full_r;
    let resp = ui.interact(rect, ui.id().with("max-strip"), Sense::click_and_drag());
    let hover = resp.hover_pos();
    let lay = layout(ms);
    let (x1, x2) = ms.crossovers();
    let splits: Vec<(usize, f32)> = match ms.count {
        3 => vec![(0, x1), (1, x2)],
        2 => vec![(0, x1)],
        _ => Vec::new(),
    };
    let near_split = |p: Pos2| {
        splits
            .iter()
            .find(|(_, hz)| spec_r.contains(p) && (p.x - x_of(*hz, spec_r)).abs() < 6.0 * k)
            .map(|(i, _)| *i)
    };
    let hot = hover.and_then(near_split).or(match st.drag {
        Some(Grab::Split(i)) => Some(i),
        _ => None,
    });
    let gains = st.history.back().map_or([0.0; 4], |c| c.bands_db);
    let mid_y = spec_r.center().y + 8.0 * k;
    let bar_y =
        |g: f32| mid_y - (g / BAR_SPAN).clamp(-1.0, 1.0) * (spec_r.height() * 0.5 - 22.0 * k);
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, spec_r, &ink, s.hue_tok);
        canvas::ground_b(b, full_r, &ink, s.hue_tok);
        let px = b.px();
        for f in [100.0f32, 1_000.0, 10_000.0] {
            b.vline(
                b.snap_mid(x_of(f, spec_r)),
                spec_r.top(),
                spec_r.bottom(),
                px,
                ink.grid,
            );
        }
        let columns: Vec<(usize, Rect)> = lay
            .iter()
            .map(|(band, lo, hi)| {
                let r =
                    Rect::from_x_y_ranges(x_of(*lo, spec_r)..=x_of(*hi, spec_r), spec_r.y_range());
                (*band, r)
            })
            .chain(std::iter::once((FULL, full_r)))
            .collect();
        for (band, r) in &columns {
            let color = band_ink(*band, s);
            let on = ms.bands[*band].on;
            b.rect(
                r.shrink(1.0),
                Fill::Solid(Ink::a(color, if on { 0.10 } else { 0.03 })),
            );
            if st.selected == *band {
                b.rrect_stroke(r.shrink(1.5 * k), 2.0 * k, 2.0 * k, color);
            }
            b.hline(
                r.left() + 4.0 * k,
                r.right() - 4.0 * k,
                b.snap_mid(mid_y),
                px,
                ink.grid_strong,
            );
            // The gain now, as a bar from the line: down for a cut, up for a lift.
            let g = gains[*band];
            let bw = (r.width() * 0.3).clamp(6.0 * k, 22.0 * k);
            let bar = Rect::from_x_y_ranges(
                r.center().x - bw * 0.5..=r.center().x + bw * 0.5,
                bar_y(g).min(mid_y)..=bar_y(g).max(mid_y),
            );
            if g != 0.0 {
                b.rect(bar, Fill::Solid(color));
            }
            let name = frame::tr(BAND_KEYS[*band]);
            canvas::put(
                b,
                pos2(r.center().x, r.top() + 4.0 * k),
                Align2::CENTER_TOP,
                &crate::kit::text::Text::new(name, Role::Micro, if on { color } else { ink.dim })
                    .caps(true),
                tk,
                None,
            );
            let state = if !on {
                Some("max.off")
            } else if ms.bands[*band].solo && *band != FULL {
                Some("max.solo_on")
            } else {
                None
            };
            if let Some(word) = state {
                canvas::put_key(
                    b,
                    pos2(r.center().x, r.bottom() - 4.0 * k),
                    Align2::CENTER_BOTTOM,
                    word,
                    Role::Micro,
                    ink.text,
                    tk,
                );
            } else {
                canvas::putf(
                    b,
                    pos2(r.center().x, r.bottom() - 4.0 * k),
                    Align2::CENTER_BOTTOM,
                    Role::Micro,
                    ink.text,
                    tk,
                    Some(ink.plate),
                    format_args!("{g:+.1} dB"),
                );
            }
        }
        for (i, hz) in &splits {
            let x = x_of(*hz, spec_r);
            let w = if hot == Some(*i) { 2.5 } else { 1.5 } * k;
            b.vline(x, spec_r.top(), spec_r.bottom(), w, ink.text);
            let handle =
                Rect::from_center_size(pos2(x, spec_r.top() + 18.0 * k), vec2(8.0 * k, 12.0 * k));
            b.rrect(handle, 2.0 * k, Fill::Solid(ink.text));
            canvas::putf(
                b,
                pos2(x + 6.0 * k, spec_r.top() + 18.0 * k),
                Align2::LEFT_CENTER,
                Role::Micro,
                ink.text,
                tk,
                Some(ink.plate),
                format_args!("{}", Hz(*hz)),
            );
        }
        for (f, word) in [(100.0f32, "100"), (1_000.0, "1k"), (10_000.0, "10k")] {
            canvas::putf(
                b,
                pos2(x_of(f, spec_r) + 3.0 * k, spec_r.bottom() - 18.0 * k),
                Align2::LEFT_BOTTOM,
                Role::Micro,
                ink.dim,
                tk,
                None,
                format_args!("{word}"),
            );
        }
        if lay.is_empty() {
            canvas::put_key(
                b,
                spec_r.center(),
                Align2::CENTER_CENTER,
                "max.bands.1",
                Role::Body,
                ink.dim,
                tk,
            );
        }
        canvas::edge_b(b, spec_r, &ink);
        canvas::edge_b(b, full_r, &ink);
    });
    let _ = tel;

    // Gestures: a split is dragged; a band is clicked to edit it, double-clicked for solo.
    if resp.drag_started() {
        let at = ui.input(|i| i.pointer.press_origin()).or(hover);
        st.drag = at.and_then(near_split).map(Grab::Split);
    }
    if let (Some(Grab::Split(i)), Some(p)) = (st.drag, resp.interact_pointer_pos()) {
        if resp.dragged() {
            let hz = hz_at(p.x, spec_r).round();
            let v = if i == 0 {
                let top = if ms.count >= 3 {
                    x2 / MIN_GAP
                } else {
                    F_HI / MIN_GAP
                };
                hz.clamp(F_LO, top)
            } else {
                hz.clamp(x1 * MIN_GAP, F_HI)
            };
            host.set(if i == 0 { "x1" } else { "x2" }, v, Some("split"));
        }
    }
    if resp.drag_stopped() && matches!(st.drag, Some(Grab::Split(_))) {
        st.drag = None;
        host.end_gesture();
    }
    let band_at = |p: Pos2| -> Option<usize> {
        if full_r.contains(p) {
            return Some(FULL);
        }
        let hz = hz_at(p.x, spec_r);
        lay.iter()
            .find(|(_, lo, hi)| hz >= *lo && hz <= *hi)
            .map(|(b, _, _)| *b)
    };
    if resp.clicked() {
        if let Some(b) = resp.interact_pointer_pos().and_then(band_at) {
            st.selected = b;
        }
    }
    if resp.double_clicked() {
        if let Some(b) = resp
            .interact_pointer_pos()
            .and_then(band_at)
            .filter(|b| *b != FULL)
        {
            let on = ms.bands[b].solo;
            host.set(key(b, "solo"), if on { 0.0 } else { 1.0 }, None);
        }
    }
    if hot.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("max.card.bands"),
            frame::tr("max.bands_tip"),
            Caps::NONE,
        );
    }
}

/// `1.2 kHz`, `150 Hz`.
struct Hz(f32);

impl std::fmt::Display for Hz {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 >= 1_000.0 {
            write!(f, "{:.1} kHz", self.0 / 1_000.0)
        } else {
            write!(f, "{:.0} Hz", self.0)
        }
    }
}

/// Under the strip: how many bands, how they are made, their slope.
pub fn strip_controls(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, width: f32) {
    let ms = cantus_engine::fx::maximizer::settings(host.spec());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = s.d(6.0);
        let wb = (width * 0.26).max(120.0);
        let wm = (width * 0.46).max(200.0);
        let ws = (width - wb - wm - 12.0).max(100.0);
        let counts = [
            Src::Key("max.bands.1"),
            Src::Key("max.bands.2"),
            Src::Key("max.bands.3"),
        ];
        if let Some(i) = controls::segmented_key(
            ui,
            s,
            host,
            "bands",
            ms.count - 1,
            &counts,
            frame::tr("fx.max.bands_tip"),
            wb,
        ) {
            host.set("bands", (i + 1) as f32, None);
        }
        let modes = [
            Src::Key("max.xmode.0"),
            Src::Key("max.xmode.1"),
            Src::Key("max.xmode.2"),
        ];
        if let Some(i) = controls::segmented_key(
            ui,
            s,
            host,
            "xmode",
            ms.mode.index(),
            &modes,
            frame::tr("fx.max.xmode_tip"),
            wm,
        ) {
            host.set("xmode", i as f32, None);
        }
        let slopes = [
            Src::Str("6"),
            Src::Str("12"),
            Src::Str("24"),
            Src::Str("48"),
        ];
        let now = Slope::ALL.iter().position(|x| *x == ms.slope).unwrap_or(2);
        if let Some(i) = controls::segmented_key(
            ui,
            s,
            host,
            "slope",
            now,
            &slopes,
            frame::tr("fx.max.slope"),
            ws,
        ) {
            host.set("slope", Slope::ALL[i].db(), None);
        }
        let _ = Mode::Dynamic;
    });
}

#[path = "maximizer_curve.rs"]
mod curve_view;
pub use curve_view::curve;
