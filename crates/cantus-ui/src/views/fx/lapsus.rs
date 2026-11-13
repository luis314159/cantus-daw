//! Lapsus's editor: the time shaper (fx side round, ADR 0132 provisional).
//!
//! *Lapsus* is Latin for a slip, a glide — and the window is a deck at midnight: an amber
//! VFD over a petrol-dark body. Its centre is **the timeline**: one cycle of the pattern,
//! the time curve above (how far behind the music it reads: flat at the top is the music
//! as it comes, a 45° fall is the tape stopped, a held step a repeat) over the cycle's
//! input drawn faintly, the level curve below it; while it plays, the playhead and **the
//! slip** — the line from where the song is to where Lapsus is reading it from, marked on
//! the ruler. Beside it **the bank**: thirty-six patterns as pads in six rows (stops,
//! speed, repeats, reverse, scratch, level), each drawn small, the playing one lit, the
//! keyboard under it naming the note that plays each. Under them the pattern's length and
//! grid, the joins, the notes and the output.

use super::controls::{self, Size};
use super::frame;
use super::host::{DawHost, FxHost};
use super::modfx;
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::shell::Grid;
use crate::kit::text::Src;
use crate::kit::{Function, Style};
use cantus_core::fx::lapsus as model;
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use cantus_dsp::lapsus::{t, LapsusSettings, Pattern};
use egui::Rect;

#[path = "lapsus_bank.rs"]
mod bank;
#[path = "lapsus_grid.rs"]
mod grid;

pub(crate) const KIND: EffectKind = EffectKind::TimeShaper;

/// Factory presets: data (`presets/effects/timeshaper.toml`, ADR 0101).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[];

/// The pencil's grids: the note value a step snaps to, in quarter notes.
pub const GRIDS: [(&str, f64); 4] = [("1/4", 1.0), ("1/8", 0.5), ("1/16", 0.25), ("1/32", 0.125)];

/// What the editor remembers between frames.
#[derive(Debug)]
pub struct State {
    /// The effect this state belongs to.
    effect: Option<EffectId>,
    /// The device frame: the preset, its lists.
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    /// The pencil's grid (an index of [`GRIDS`]).
    pub grid: usize,
    /// The drawing tool: 0 points, 1 the pencil.
    pub tool: usize,
    /// The input's level across the cycle, under the time curve.
    wave: grid::Wave,
    /// Where the timeline's lanes were drawn last frame (tests point at them).
    pub lanes: Option<(Rect, Rect)>,
    /// Where the bank's pads were drawn last frame.
    pub pads: Option<Rect>,
}

impl Default for State {
    fn default() -> Self {
        State {
            effect: None,
            chrome: frame::Chrome::default(),
            chips: frame::ChipText::default(),
            grid: 2,
            tool: 0,
            wave: grid::Wave::default(),
            lanes: None,
            pads: None,
        }
    }
}

/// Lapsus's editor in Cantus's own window: the host is the project.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.lapsus);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "lapsus"), &mut st, ui);
    app.fx.lapsus = st;
    out
}

/// Lapsus's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
/// another DAW.
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let tel = host.telemetry();
    let sr = host.sample_rate();
    let ls = cantus_engine::fx::lapsus::settings(host.spec());
    let mut chips = std::mem::take(&mut st.chips);
    let latency = (ls.ahead_ms * 0.001 * sr).round();
    let chip = [chips.latency(latency, sr)];
    let look = frame::Look {
        kind: KIND,
        chips: &chip,
        meter: modfx::out_meter(&tel),
        more: &[],
    };
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, st, ui, s, body, &tel, &ls)
    });
    st.chrome = chrome;
    st.chips = chips;
    out
}

/// The bank card's width at 100 % (its share of the top row).
const BANK_W: f32 = 318.0;

fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &[f32; 64],
    ls: &LapsusSettings,
) {
    let s = &modfx::body(s);
    let k = s.k;
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let gap = s.d(4.0);
    let row2 = controls::card_height(Size::M) * k;
    let row1 = (area.height() - row2 - gap).max(300.0 * k);
    let bank_share = (BANK_W * k / area.width()).clamp(0.2, 0.4);
    let r1: Vec<Rect> = g.row(row1, &[1.0 - bank_share, bank_share]).collect();
    let playing = playing_slot(tel, ls);
    grid::card(host, st, ui, s, r1[0], tel, playing);
    bank::card(host, st, ui, s, r1[1], tel, ls, playing);

    let w = [
        PATTERN_W + 16.0,
        controls::cells_width(&[Size::M, Size::M]) + 16.0,
        NOTES_W + 4.0 + controls::cells_width(&[Size::M]) + 16.0,
        controls::cells_width(&[Size::M, Size::M, Size::M]) + 16.0,
    ];
    let sum: f32 = w.iter().sum();
    let r2: Vec<Rect> = g.row(row2, &w.map(|x| x / sum)).collect();
    pattern_card(host, st, ui, s, r2[0], playing);
    let sj = modfx::own(s, Function::Mod);
    controls::card(
        ui,
        &sj,
        r2[1],
        Src::Key("lapsus.card.joins"),
        None,
        |ui, _| {
            controls::param_with(ui, &sj, host, "xfade", Size::M, Some(&modfx::ms_pill));
            controls::param_with(ui, &sj, host, "smooth", Size::M, Some(&modfx::ms_pill));
        },
    );
    notes_card(host, ui, s, r2[2]);
    let so = modfx::own(s, Function::Output);
    controls::card(
        ui,
        &so,
        r2[3],
        Src::Key("lapsus.card.output"),
        None,
        |ui, _| {
            controls::param_with(ui, &so, host, "ahead", Size::M, Some(&modfx::ms_pill));
            controls::param_with(ui, &so, host, "mix", Size::M, Some(&modfx::pct_pill));
            controls::param_with(ui, &so, host, "out", Size::M, Some(&modfx::db_pill));
        },
    );
}

/// The slot playing: the telemetry's while the unit runs, the knob's otherwise.
fn playing_slot(tel: &[f32; 64], ls: &LapsusSettings) -> usize {
    if tel[cantus_dsp::modfx_clock::tel::BLOCKS] > 0.0 {
        (tel[t::SLOT].round().max(0.0) as usize).min(model::SLOTS - 1)
    } else {
        ls.slot
    }
}

/// The pattern in `slot` of the host's bank, as the DSP plays it.
pub fn pattern_of(spec: &EffectSpec, slot: usize) -> Pattern {
    cantus_engine::fx::lapsus::pattern(model::pattern_of(spec.lapsus.as_deref(), slot))
}

/// The pattern card's column width at 100 %.
const PATTERN_W: f32 = 268.0;
/// The notes card's column width at 100 %.
const NOTES_W: f32 = 196.0;

/// The pattern playing: its length and the pencil's grid; how it is drawn.
fn pattern_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    slot: usize,
) {
    let len = model::pattern_of(host.spec().lapsus.as_deref(), slot).len as usize;
    controls::card(
        ui,
        s,
        rect,
        Src::Key("lapsus.card.pattern"),
        None,
        |ui, _| {
            let w = PATTERN_W;
            let half = (w - 4.0) * 0.5;
            controls::column(ui, s, half, |ui| {
                controls::caption(ui, s, Src::Key("lapsus.length"), half);
                let labels = ["\u{bc}", "\u{bd}", "1", "2", "4"].map(Src::Str);
                let _ = controls::segmented(ui, s, len, &labels, frame::tr("lapsus.length"), half);
                controls::caption(ui, s, Src::Key("lapsus.grid"), half);
                let labels = GRIDS.map(|(g, _)| Src::Str(g));
                if let Some(i) =
                    controls::segmented(ui, s, st.grid, &labels, frame::tr("lapsus.grid"), half)
                {
                    st.grid = i;
                }
            });
            controls::column(ui, s, half, |ui| {
                controls::caption(ui, s, Src::Key("lapsus.draw"), half);
                let labels = [
                    Src::Key("lapsus.tool.points"),
                    Src::Key("lapsus.tool.pencil"),
                ];
                if let Some(i) =
                    controls::segmented(ui, s, st.tool, &labels, frame::tr("lapsus.draw"), half)
                {
                    st.tool = i;
                }
                controls::caption(ui, s, Src::Key("lapsus.edit"), half);
                // Actions, not a choice: a key each (phase B: what they do).
                ui.horizontal(|ui| {
                    let w2 = (half - 4.0) / 2.0;
                    for key in ["lapsus.edit.copy", "lapsus.edit.paste"] {
                        let _ = controls::key(ui, s, Src::Key(key), frame::tr(key), w2, false);
                    }
                });
            });
        },
    );
}

/// The bank's factory names (`lapsus.slot.{i}`).
pub const SLOT_KEYS: [&str; model::SLOTS] = [
    "lapsus.slot.0",
    "lapsus.slot.1",
    "lapsus.slot.2",
    "lapsus.slot.3",
    "lapsus.slot.4",
    "lapsus.slot.5",
    "lapsus.slot.6",
    "lapsus.slot.7",
    "lapsus.slot.8",
    "lapsus.slot.9",
    "lapsus.slot.10",
    "lapsus.slot.11",
    "lapsus.slot.12",
    "lapsus.slot.13",
    "lapsus.slot.14",
    "lapsus.slot.15",
    "lapsus.slot.16",
    "lapsus.slot.17",
    "lapsus.slot.18",
    "lapsus.slot.19",
    "lapsus.slot.20",
    "lapsus.slot.21",
    "lapsus.slot.22",
    "lapsus.slot.23",
    "lapsus.slot.24",
    "lapsus.slot.25",
    "lapsus.slot.26",
    "lapsus.slot.27",
    "lapsus.slot.28",
    "lapsus.slot.29",
    "lapsus.slot.30",
    "lapsus.slot.31",
    "lapsus.slot.32",
    "lapsus.slot.33",
    "lapsus.slot.34",
    "lapsus.slot.35",
];

/// Slot `i`'s factory name, translated.
pub fn slot_name(i: usize) -> &'static str {
    frame::tr(SLOT_KEYS[i.min(model::SLOTS - 1)])
}

/// The lengths' names (`lapsus.len.{i}`).
const LEN_KEYS: [&str; 5] = [
    "lapsus.len.0",
    "lapsus.len.1",
    "lapsus.len.2",
    "lapsus.len.3",
    "lapsus.len.4",
];

/// What a note does, whether it starts its pattern again, and the note of the first slot.
fn notes_card(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect) {
    let sn = modfx::own(s, Function::Mod);
    controls::card(
        ui,
        &sn,
        rect,
        Src::Key("lapsus.card.notes"),
        None,
        |ui, _| {
            let w = NOTES_W;
            controls::column(ui, &sn, w, |ui| {
                controls::caption(ui, &sn, Src::Key("lapsus.each_note"), w);
                let modes = [
                    Src::Key("lapsus.notes.0"),
                    Src::Key("lapsus.notes.1"),
                    Src::Key("lapsus.notes.2"),
                ];
                controls::segmented_param(ui, &sn, host, "notes", &modes, w);
                controls::led_param(ui, &sn, host, "retrig", Src::Key("fx.lapsus.retrig"), w);
            });
            controls::param_with(ui, &sn, host, "key", Size::M, Some(&note_pill));
        },
    );
}

/// A note number as its name: 48 → C3.
pub fn note_pill(v: f32, b: &mut String) {
    const NAMES: [&str; 12] = [
        "C", "C\u{266f}", "D", "D\u{266f}", "E", "F", "F\u{266f}", "G", "G\u{266f}", "A",
        "A\u{266f}", "B",
    ];
    use std::fmt::Write as _;
    let n = v.round().clamp(0.0, 127.0) as i32;
    let _ = write!(b, "{}{}", NAMES[(n % 12) as usize], n / 12 - 1);
}

/// The inspector's picture: the playing pattern's two curves, small, with its name.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    grid::preview(ui, spec)
}

/// The settings `--editor lapsus` and the screenshot tool open it with: *Tape stop ¼* —
/// three beats as they come, the last one stopping on the bar.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "lapsus").set_many(DEMO, "cmd.fx_preset");
}

/// The demo's settings.
pub const DEMO: &[(&str, f32)] = &[("slot", 3.0), ("mix", 1.0)];

/// What a playing engine would feed the editor, for screenshots with no engine: a drum
/// loop (kick, snare, hats) through the unit for a bar and a half, its telemetry, the
/// analyser's pairs and the cycle's waveform.
pub fn demo_feed(app: &mut CantusApp) {
    grid::demo_feed(app);
}
