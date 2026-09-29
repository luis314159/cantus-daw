//! Unda — the wavetable / hybrid flagship synth: its editor, on the kit's device shell
//! (ADR 0024 §8). Owned by this instrument's builder (ADR 0023). What the benchmark
//! research found (`research/instruments/wavetable.md` §5, §8.6) and what this does:
//!
//! - **The display is the control.** The selected oscillator's table leads both views,
//!   bleeding to its card's edges: drag up and down to scrub the position, 3D | 2D in its
//!   title bar. Every card has its own picture you drag — an oscillator's cycle, a filter's
//!   curve (cutoff × resonance) with its modulation ghost, an envelope's corners, a drawn
//!   LFO. Every gesture is one step of the project's undo.
//! - **Cards by function** (ADR 0024 §2): OSC 1–3, SUB and NOISE in the source hue, the
//!   filters in tone, the envelopes in dynamics, the LFOs in modulation, the voice in
//!   output. Values sit in kit cells (pill, knob, label); modes are menu fields and
//!   segmented switches — nothing hides in a right-click (Serum 2's most repeated
//!   criticism), no word floats loose.
//! - **Modulation you can see**: rings on every modulated cell, the modulated cutoff drawn
//!   as a ghost over the filter curve and its reach as a band, the frame the newest voice
//!   plays lit in the table, each envelope's and LFO's live value at its display's edge.
//! - **Real units** everywhere: Hz, ms, s, dB, cents, semitones, frames.
//!
//! **Play** is the table, large, with the position as the one XL cell, and the filter and
//! the amplifier beside it. **Edit** is every section, top to bottom; it scrolls.

#[path = "wavetable_engines.rs"]
mod engines;
#[path = "wavetable_filter.rs"]
mod filter_view;
#[path = "wavetable_import.rs"]
pub mod import;
#[path = "wavetable_mod.rs"]
mod mod_view;
#[path = "wavetable_osc.rs"]
mod osc_view;
#[path = "wavetable_parts.rs"]
pub(crate) mod parts;
#[path = "wavetable_presets.rs"]
mod presets;

pub use osc_view::{import_table, table_for};
pub use presets::FACTORY_PRESETS;

use super::FactoryPreset;
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::{Function, Style};
use cantus_core::{Command, MergeKey, NativeSpec, TrackId};
use egui::{Rect, Sense, Vec2};
use parts::Rows;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Arc;

/// Editor state that is not part of the project.
#[derive(Debug, Default)]
pub struct State {
    /// The oscillator the big table and the detail card show (0-based).
    pub osc: usize,
    /// The envelope and the LFO their cards show (0-based).
    pub env: usize,
    pub lfo: usize,
    /// Per oscillator: the table drawn flat (2D, with its warps) instead of as a stack.
    pub flat: [bool; 3],
    /// Imported tables decoded for drawing, by (path, frame size). `None`: unreadable.
    tables: HashMap<(PathBuf, usize), Option<Arc<cantus_dsp::unda::Table>>>,
    /// The last import's outcome, for the status line.
    pub import_note: Option<(usize, String)>,
}

/// The names of a list of model values, through the locale: `prefix.name`.
pub fn names(prefix: &str, list: &[&str]) -> Vec<String> {
    list.iter().map(|n| t(&format!("{prefix}.{n}"))).collect()
}

/// Set several numbers as one undo step that a drag keeps extending (`gesture`).
pub fn set_values(app: &mut CantusApp, track: TrackId, gesture: &str, values: &[(&str, f32)]) {
    for (key, value) in values {
        app.exec_merging(
            Command::SetInstrumentValue {
                track,
                key: (*key).to_string(),
                value: *value,
            },
            MergeKey::of(&("unda", track.0, gesture)),
        );
    }
}

/// Whether the engine is playing (the demo's or a real voice): live markers show then.
pub fn live(app: &CantusApp) -> bool {
    let tel = app.inst_telemetry();
    tel[cantus_engine::inst::wavetable::tel::ALIVE] > 0.0 && tel[super::TEL_VOICES] > 0.0
}

// ---- readouts ------------------------------------------------------------------------------

/// A rate: `2.40 Hz`, `12.5 Hz`.
pub fn rate_text(v: f32, b: &mut String) {
    let _ = if v < 10.0 {
        write!(b, "{v:.2} Hz")
    } else {
        write!(b, "{v:.1} Hz")
    };
}

/// A whole count.
pub fn count_text(v: f32, b: &mut String) {
    let _ = write!(b, "{}", v.round() as i64);
}

/// Pan as a cell prints it: L 40, C, R 12.
pub fn pan_text(v: f32, b: &mut String) {
    let p = (v * 100.0).round();
    let _ = if p == 0.0 {
        write!(b, "{}", t("unda.pan_centre"))
    } else if p < 0.0 {
        write!(b, "L {}", -p)
    } else {
        write!(b, "R {p}")
    };
}

/// A dB value in a cell: one decimal below ±10 dB.
pub fn db_text(v: f32, b: &mut String) {
    let _ = if v.abs() >= 10.0 {
        write!(b, "{v:+.0} dB")
    } else {
        write!(b, "{v:+.1} dB")
    };
}

/// A dB value in a small (S) cell's pill: whole decibels.
pub fn db_s(v: f32, b: &mut String) {
    let _ = write!(b, "{v:+.0} dB");
}

/// A frequency in a small cell's pill: `350 Hz`, `2.4k`, `12k`.
pub fn hz_s(v: f32, b: &mut String) {
    parts::house_s(v, "Hz", false, b);
}

/// A rate in a small cell's pill: `0.5 Hz`, `12 Hz`.
pub fn rate_s(v: f32, b: &mut String) {
    let _ = if v < 10.0 {
        write!(b, "{v:.1} Hz")
    } else {
        write!(b, "{v:.0} Hz")
    };
}

// ---- the views -----------------------------------------------------------------------------

/// The Edit view: every section, in rows of cards that follow the signal — the selected
/// oscillator's table and its warps, the three oscillators, sub, noise and the filters,
/// the envelopes and the LFOs, the voice. Taller than the window: it scrolls.
pub fn body(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let k = s.k;
    let heights = [236.0, 306.0, 206.0, 214.0, 112.0];
    let total: f32 = heights.iter().sum::<f32>() * k + 4.0 * k * (heights.len() - 1) as f32;
    let w = ui.available_width();
    let (area, _) = ui.allocate_exact_size(Vec2::new(w, total), Sense::hover());
    let mut rows = Rows::new(area, s);
    let src = s.function(Function::Source);
    let o = app.inst.wavetable.osc.min(2);

    let r = rows.take(heights[0] * k);
    let cols = parts::columns(r, s, &[0.58, 0.42]);
    osc_view::table_card(app, ui, track, spec, &src, cols[0], o, false);
    osc_view::detail_card(app, ui, track, spec, &src, cols[1], o);

    let r = rows.take(heights[1] * k);
    for (i, c) in parts::columns(r, s, &[1.0 / 3.0; 3])
        .into_iter()
        .enumerate()
    {
        osc_view::osc_card(app, ui, track, spec, &src, c, i);
    }

    let r = rows.take(heights[2] * k);
    let cols = parts::columns(r, s, &[0.13, 0.15, 0.31, 0.10, 0.31]);
    osc_view::sub_card(app, ui, track, spec, &src, cols[0]);
    osc_view::noise_card(app, ui, track, spec, &src, cols[1]);
    let tone = s.function(Function::Tone);
    filter_view::filter_card(app, ui, track, spec, &tone, cols[2], 0);
    filter_view::routing_card(app, ui, track, spec, &tone, cols[3]);
    filter_view::filter_card(app, ui, track, spec, &tone, cols[4], 1);

    let r = rows.take(heights[3] * k);
    let cols = parts::columns(r, s, &[0.5, 0.5]);
    mod_view::env_card(
        app,
        ui,
        track,
        spec,
        &s.function(Function::Dynamics),
        cols[0],
    );
    mod_view::lfo_card(app, ui, track, spec, &s.function(Function::Mod), cols[1]);

    let r = rows.take(heights[4] * k);
    let cols = parts::columns(r, s, &[0.72, 0.28]);
    let out = s.function(Function::Output);
    mod_view::voice_card(app, ui, track, spec, &out, cols[0]);
    mod_view::output_card(app, ui, track, spec, &out, cols[1]);
}

/// The Play view: the selected oscillator's table, large and alive, with its position as
/// the one XL cell; beside it the first filter and the amplifier's envelope.
pub fn play(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let area = ui.max_rect();
    let cols = parts::columns(area, s, &[0.6, 0.4]);
    let o = app.inst.wavetable.osc.min(2);
    osc_view::table_card(
        app,
        ui,
        track,
        spec,
        &s.function(Function::Source),
        cols[0],
        o,
        true,
    );
    let right = cols[1];
    let g = s.d(4.0);
    let top_h = ((right.height() - g) * 0.53).round();
    let top = Rect::from_min_size(right.min, Vec2::new(right.width(), top_h));
    let bottom = Rect::from_min_max(egui::pos2(right.left(), top.bottom() + g), right.max);
    filter_view::filter_card(app, ui, track, spec, &s.function(Function::Tone), top, 0);
    mod_view::amp_card(
        app,
        ui,
        track,
        spec,
        &s.function(Function::Dynamics),
        bottom,
    );
}

/// The inspector's small read-only picture: oscillator 1's current frame.
pub fn preview(ui: &mut egui::Ui, spec: &NativeSpec) -> egui::Response {
    osc_view::preview(ui, spec)
}

/// `--editor wavetable`: a preset that shows the displays off, and a live engine's
/// numbers so the picture is alive.
pub fn demo_setup(app: &mut CantusApp, track: TrackId) {
    if let Some(p) = FACTORY_PRESETS
        .iter()
        .find(|p| p.label_key == "unda.preset.glass_choir")
    {
        let spec = super::preset_spec(cantus_core::InstrumentKind::Wavetable, p);
        app.apply_inst_spec(track, &spec, "cmd.fx_preset");
    }
    // `CANTUS_SHOOT_VIEW=edit|lfo|matrix` for the screenshot tool, which otherwise shows
    // the Play view.
    match std::env::var("CANTUS_SHOOT_VIEW").as_deref() {
        Ok("edit") => app.inst.view = super::View::Edit,
        Ok("lfo") => {
            app.inst.view = super::View::Edit;
            app.inst.wavetable.lfo = 0;
            app.set_inst_value(track, "lfo1.wave", 7.0, false);
            app.set_inst_text(
                track,
                "lfo1.shape",
                Some("0,0.5;0.12,0.95;0.3,0.7;0.5,0.2;0.8,0.05;1,0.5".into()),
            );
        }
        Ok("matrix") => app.inst.view = super::View::Matrix,
        _ => {}
    }
    demo_feed(app);
}

/// For screenshots and tests with no engine: what a playing engine would report.
pub fn demo_feed(app: &mut CantusApp) {
    use cantus_engine::inst::wavetable::tel;
    let mut tel_v = [0.0f32; cantus_engine::TELEMETRY_LEN];
    tel_v[super::TEL_VOICES] = 4.0;
    tel_v[super::TEL_POLYPHONY] = 16.0;
    tel_v[tel::POS..tel::POS + 3].copy_from_slice(&[0.42, 0.3, 0.0]);
    tel_v[tel::ENV..tel::ENV + 4].copy_from_slice(&[0.86, 0.55, 0.2, 0.0]);
    for (i, v) in [0.6, -0.3, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0].iter().enumerate() {
        tel_v[tel::LFO + i] = *v;
    }
    tel_v[tel::CUTOFF..tel::CUTOFF + 2].copy_from_slice(&[3_400.0, 9_000.0]);
    tel_v[tel::RES..tel::RES + 2].copy_from_slice(&[0.35, 0.1]);
    tel_v[tel::PEAK..tel::PEAK + 2].copy_from_slice(&[0.4, 0.38]);
    tel_v[tel::LANES] = 56.0;
    tel_v[tel::ALIVE] = 1.0;
    tel_v[tel::OVERSAMPLE] = 2.0;
    app.inst.injected = Some(tel_v);
}

/// The factory preset list, for the frame (and tests).
pub fn presets() -> &'static [FactoryPreset] {
    FACTORY_PRESETS
}
