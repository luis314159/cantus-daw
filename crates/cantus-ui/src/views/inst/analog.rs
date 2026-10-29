//! Calor — the analog-modelling polysynth: its editor (ADR 0023), on the kit's device shell
//! (ADR 0024 §8). Owned by this instrument's builder.
//!
//! The panel reads as the signal path: oscillators → mixer → filter → amplifier, the
//! modulators under the stages they drive, then the voices and their sixteen cards. Each
//! section is a kit card in its function hue (Source for the oscillators and the mixer,
//! Tone for the filters, Dynamics for the envelopes and the amplifier, Mod for the LFOs and
//! the wheel, Space for the chorus, Output for the voices and the level), its numbers kit
//! cells, its switches segmented controls and LED buttons. Real units everywhere.
//!
//! - **Play**: the filter's response, drawn and dragged (across for cutoff, up for
//!   resonance), with the live cutoff and the self-oscillation mark, beside the sixteen
//!   voice cards lit as they play; the model and the slope as segmented controls; under
//!   them the cards a preset is played with — amp envelope, voice, chorus, output.
//! - **Edit**: every section, card by card; it scrolls when it is taller than the window.

use super::FactoryPreset;
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::knob::KnobSize;
use crate::kit::text::Src;
use crate::kit::{Function, Style};
use cantus_core::{Command, MergeKey, NativeSpec, TrackId};
use egui::{Pos2, Rect};

#[path = "analog_cards.rs"]
mod cards;
#[path = "analog_draw.rs"]
mod draw;
#[path = "analog_edit.rs"]
mod edit;
#[path = "analog_filter.rs"]
mod filter;
#[path = "analog_parts.rs"]
pub(crate) mod parts;
#[path = "analog_presets.rs"]
mod presets;

use parts::{Fmt, Item};
pub use presets::FACTORY_PRESETS;

/// Editor state that is not part of the project.
#[derive(Debug, Default)]
pub struct State {
    /// The voice card whose trims are shown.
    pub selected_card: Option<usize>,
    /// A drag on the filter display: the cutoff and resonance it started from.
    drag: Option<(f32, f32)>,
}

/// Telemetry slots (the DSP's `calor::tel`, from slot 8).
pub mod tel {
    pub use cantus_dsp::calor::tel::*;
}

/// Set several numbers as one gesture: the frames of one drag are one undo step.
fn set_values(app: &mut CantusApp, track: TrackId, gesture: &str, values: &[(&str, f32)]) {
    for (key, value) in values {
        app.exec_merging(
            Command::SetInstrumentValue {
                track,
                key: key.to_string(),
                value: *value,
            },
            MergeKey::of(&("calor", track.0, gesture)),
        );
    }
}

/// The filter display's gesture: cutoff and resonance together, merged frame to frame
/// into one undo step until `end_gesture`.
pub fn set_filter(app: &mut CantusApp, track: TrackId, cutoff: f32, reso: f32) {
    set_values(
        app,
        track,
        "filter",
        &[("vcf.cutoff", cutoff), ("vcf.reso", reso)],
    );
}

/// The CPU the voices cost, as the engine measures it (empty when it has not said).
fn cpu_text(tel: &cantus_engine::Telemetry) -> String {
    let ns = tel[tel::CPU_NS];
    if ns > 0.0 {
        format!(
            "{} {:.2} % \u{b7} {:.0} ns",
            t("calor.cpu_per_voice"),
            ns * 48_000.0 / 1e7,
            ns
        )
    } else {
        String::new()
    }
}

/// A micro caption in a slot.
fn slot_caption(ui: &egui::Ui, r: Rect, text: &str, s: &Style) {
    crate::kit::with_brush(&ui.painter_at(r), |b| {
        let _ = parts::words(b, r, &parts::caption_text(Src::Str(text), s), s);
    });
}

// ---- the views -------------------------------------------------------------------------------

/// The Edit view: every section, in the order the signal flows.
pub fn body(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    edit::show(app, ui, track, spec, s);
}

/// The filter's slope (or the morph's middle, or nothing: a Sallen-Key is two-pole) and
/// its circuit, as a column beside its cells; how many of the two there are.
pub(crate) fn filter_column(spec: &NativeSpec) -> ([Item<'static>; 2], usize) {
    let model = cantus_dsp::va::Model::from_index(spec.get("vcf.model"));
    let circuit = Item::Choice {
        key: "vcf.rev",
        labels: &edit::VCF_REV,
        first: 0.0,
        w: 180.0,
        caption: true,
    };
    let second = if model.four_pole() {
        Some(Item::Choice {
            key: "vcf.slope",
            labels: &edit::SLOPES,
            first: 1.0,
            w: 180.0,
            caption: true,
        })
    } else if model == cantus_dsp::va::Model::Svf {
        Some(Item::Choice {
            key: "vcf.mid",
            labels: &edit::MID,
            first: 0.0,
            w: 180.0,
            caption: true,
        })
    } else {
        None
    };
    match second {
        Some(x) => ([x, circuit], 2),
        None => ([circuit, circuit], 1),
    }
}

/// The Play view: the filter drawn and dragged beside the voice cards, and the cards a
/// preset is played with.
pub fn play(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let tel = app.inst_telemetry();
    let area = ui.max_rect();
    let k = s.k;
    let g = parts::gutter(s);

    // The bottom row: the cards a preset is played with.
    let chorus = [Item::Choice {
        key: "chorus.mode",
        labels: &edit::CHORUS,
        first: 0.0,
        w: 130.0,
        caption: false,
    }];
    let output_col = [
        Item::Choice {
            key: "quality",
            labels: &edit::QUALITIES,
            first: 0.0,
            w: 150.0,
            caption: true,
        },
        Item::Slot(150.0, 12.0),
    ];
    let amp: [Item; 4] = [
        Item::Cell("env2.a", KnobSize::M, Fmt::Unit),
        Item::Cell("env2.d", KnobSize::M, Fmt::Unit),
        Item::Cell("env2.s", KnobSize::M, Fmt::Unit),
        Item::Cell("env2.r", KnobSize::M, Fmt::Unit),
    ];
    let voice = [
        Item::Cell("glide.time", KnobSize::M, Fmt::Unit),
        Item::Cell("voice.stack", KnobSize::M, Fmt::Whole("")),
        Item::Cell("voice.stack_detune", KnobSize::M, Fmt::Unit),
    ];
    let space = [chorus[0], Item::Cell("chorus.mix", KnobSize::M, Fmt::Unit)];
    let output = [
        Item::Column(&output_col),
        Item::Cell("gain", KnobSize::M, Fmt::Unit),
    ];
    let fracs = [0.30, 0.24, 0.22, 0.24];
    let ws = parts::widths(area.width(), &fracs, s);
    let lists: [(&[Item], &str, Function); 4] = [
        (&amp, "calor.section.amp_env", Function::Dynamics),
        (&voice, "calor.section.voice", Function::Output),
        (&space, "calor.section.chorus", Function::Space),
        (&output, "calor.section.output", Function::Output),
    ];
    let hb = lists
        .iter()
        .zip(&ws)
        .map(|((items, _, _), w)| parts::card_height(items, *w, None, s))
        .fold(0.0f32, f32::max);
    let yb = area.bottom() - hb;
    let rects = parts::row(area, yb, hb, &fracs, s);
    for (((items, title, f), r), i) in lists.iter().zip(&rects).zip(0..) {
        let st = s.function(*f);
        let out = parts::section(
            app,
            ui,
            track,
            spec,
            &st,
            *r,
            Src::Key(title),
            None,
            0.0,
            items,
        );
        if let (3, Some((_, slots))) = (i, out) {
            if let Some(slot) = slots.first() {
                slot_caption(ui, *slot, &cpu_text(&tel), &st);
            }
        }
    }

    // The top row: the filter you drag beside the voice cards.
    let top = Rect::from_min_max(area.min, Pos2::new(area.right(), yb - g));
    let fr = [0.64, 0.36];
    let wa = parts::widths(area.width(), &fr, s);
    let (col, n) = filter_column(spec);
    let fitems = [
        Item::Cell("vcf.cutoff", KnobSize::M, Fmt::Unit),
        Item::Cell("vcf.reso", KnobSize::M, Fmt::Unit),
        Item::Cell("vcf.env", KnobSize::M, Fmt::Unit),
        Item::Cell("vcf.drive", KnobSize::M, Fmt::Unit),
        Item::Cell("vcf.bass_comp", KnobSize::M, Fmt::Unit),
        Item::Cell("vcf.key", KnobSize::M, Fmt::Unit),
        Item::Column(&col[..n]),
    ];
    let slots_col = [Item::Slot(200.0, 34.0), Item::Slot(200.0, 20.0)];
    let citems = [
        Item::Cell("analog.condition", KnobSize::M, Fmt::Unit),
        Item::Column(&slots_col),
    ];
    let body = parts::card_height(&fitems, wa[0], None, s)
        .max(parts::card_height(&citems, wa[1], None, s));
    let disp = ((top.height() - body) / k - 1.0).max(60.0);
    let rs = parts::row(area, top.top(), top.height(), &fr, s);
    let tone = s.function(Function::Tone);
    let model_w = 330.0;
    if let Some((cr, _)) = parts::section(
        app,
        ui,
        track,
        spec,
        &tone,
        rs[0],
        Src::Key("calor.section.filter"),
        Some(disp),
        model_w,
        &fitems,
    ) {
        let bar = parts::bar(&cr, &tone, model_w);
        parts::choice_at(
            app,
            ui,
            track,
            spec,
            "vcf.model",
            &edit::MODELS,
            0.0,
            bar,
            &tone,
        );
        if let Some(d) = cr.display {
            filter::display(
                app,
                ui,
                track,
                spec,
                &tel,
                d,
                &super::face::on_display(&tone),
            );
        }
    }
    let voices = s.function(Function::Output);
    if let Some((cr, slots)) = parts::section(
        app,
        ui,
        track,
        spec,
        &voices,
        rs[1],
        Src::Key("calor.section.cards"),
        Some(disp),
        0.0,
        &citems,
    ) {
        if let Some(d) = cr.display {
            cards::grid(
                app,
                ui,
                track,
                spec,
                &tel,
                d,
                &super::face::on_display(&voices),
                false,
            );
        }
        if let [words, tools, ..] = slots[..] {
            cards::condition_words(ui, words, spec, &voices);
            cards::tools(app, ui, track, spec, tools, &voices);
        }
    }
}

/// The inspector's small read-only picture: the filter curve and the model.
pub fn preview(ui: &mut egui::Ui, spec: &NativeSpec) -> egui::Response {
    filter::preview(ui, spec)
}

/// `--editor analog`: a warm pad-ish patch, so the display has something to show.
pub fn demo_setup(app: &mut CantusApp, track: TrackId) {
    for (k, v) in [
        ("vcf.cutoff", 1400.0),
        ("vcf.reso", 0.62),
        ("vcf.env", 2.5),
        ("analog.condition", 0.4),
        ("voice.stack", 2.0),
        ("mix.sub", 0.35),
        ("chorus.mode", 1.0),
    ] {
        app.set_inst_value(track, k, v, false);
    }
    app.set_inst_text(track, "macro1.name", Some(t("calor.macro.brightness")));
    app.connect_modulation(track, "macro1", "vcf.cutoff");
    // The screenshot tool has no engine: show what a playing one would report.
    demo_feed(app);
}

/// For screenshots and tests with no engine: five cards playing a chord, the filter's
/// cutoff moving, a healthy level into the filter.
pub fn demo_feed(app: &mut CantusApp) {
    let mut t = [0.0f32; cantus_engine::inst::TELEMETRY_LEN];
    t[super::TEL_VOICES] = 5.0;
    t[super::TEL_POLYPHONY] = 8.0;
    for (card, key, level) in [
        (2usize, 48u8, 0.9f32),
        (5, 55, 0.8),
        (7, 60, 0.75),
        (11, 64, 0.6),
        (12, 67, 0.3),
    ] {
        t[tel::CARD_LEVEL + card] = level;
        t[tel::CARD_KEY + card] = key as f32 + 1.0;
    }
    t[tel::IN_PEAK] = 1.3;
    t[tel::CUTOFF] = 2300.0;
    t[tel::ENV1] = 0.55;
    t[tel::ENV2] = 0.8;
    t[tel::FACTOR] = 2.0;
    t[tel::CPU_NS] = 460.0;
    t[tel::NEWEST_CARD] = 7.0;
    app.inst.injected = Some(t);
}
