//! Lapsus's editor: the time shaper (fx side round, ADR 0132 provisional).
//!
//! *Lapsus* is Latin for a slip, a glide — and the window is a deck at midnight: an amber
//! VFD over a petrol body under smoked glass. Its centre is **the timeline**: one cycle of the pattern,
//! the time curve above (how far behind the music it reads: flat at the top is the music
//! as it comes, a 45° fall is the tape stopped, a held step a repeat) over the cycle's
//! input drawn faintly, the level curve below it; while it plays, the playhead and **the
//! slip** — the line from where the song is to where Lapsus is reading it from, marked on
//! the ruler. Beside it **the banks** (as Gross Beat's, a time pattern and a volume pattern
//! chosen apart): thirty-six of each as pads in six rows, the bank shown chosen by a
//! switch that names what each plays, the playing pad lit, and the keyboard under them
//! with the two note ranges that play them. Under them the shown pattern's length and the
//! pencil's grid, the joins, the notes and the output.

use super::controls::{self, Size};
use super::frame;
use super::host::{DawHost, FxHost};
use super::modfx;
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::shell::Grid;
use crate::kit::text::Src;
use crate::kit::{Function, Style};
use cantus_core::fx::lapsus::{self as model, Lane};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use cantus_dsp::lapsus::{t, LapsusSettings, Pattern};

/// Where in the window the two banks' patterns are read from: the playing slot of each.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Playing {
    pub time: usize,
    pub vol: usize,
}

impl Playing {
    pub fn of(&self, lane: Lane) -> usize {
        match lane {
            Lane::Time => self.time,
            Lane::Vol => self.vol,
        }
    }
}
use egui::Rect;

#[path = "lapsus_bank.rs"]
mod bank;
#[path = "lapsus_edit.rs"]
mod edit;
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
    /// The bank the pads show and the pattern card edits.
    pub bank: Lane,
    /// A drag on the timeline, and the number that folds its steps into one undo step.
    drag: Option<edit::Drag>,
    gesture: u64,
    /// A pattern copied, to paste into any slot.
    clip: Option<model::Pattern>,
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
            bank: Lane::Time,
            drag: None,
            gesture: 0,
            clip: None,
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
    // A size up: the notes card's two note cells sit under their caption.
    let row2 = controls::card_height(Size::L) * k;
    let row1 = (area.height() - row2 - gap).max(300.0 * k);
    let bank_share = (BANK_W * k / area.width()).clamp(0.2, 0.4);
    let r1: Vec<Rect> = g.row(row1, &[1.0 - bank_share, bank_share]).collect();
    let playing = playing(tel, ls);
    grid::card(host, st, ui, s, r1[0], tel, playing);
    bank::card(host, st, ui, s, r1[1], tel, ls, playing);

    let w = [
        PATTERN_W + 16.0,
        controls::cells_width(&[Size::M, Size::M]) + 16.0,
        NOTES_W + 4.0 + controls::cells_width(&[Size::M, Size::M]) + 16.0,
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

/// The slots playing: the telemetry's while the unit runs, the knobs' otherwise.
fn playing(tel: &[f32; 64], ls: &LapsusSettings) -> Playing {
    let of = |i: usize| (tel[i].round().max(0.0) as usize).min(model::SLOTS - 1);
    if tel[cantus_dsp::modfx_clock::tel::BLOCKS] > 0.0 {
        Playing {
            time: of(t::SLOT),
            vol: of(t::VSLOT),
        }
    } else {
        Playing {
            time: ls.tslot,
            vol: ls.vslot,
        }
    }
}

/// The pattern in `slot` of `lane` of the host's banks, as the DSP plays it.
pub fn pattern_of(spec: &EffectSpec, lane: Lane, slot: usize) -> Pattern {
    let m = model::pattern_of(spec.lapsus.as_deref(), lane, slot);
    cantus_engine::fx::lapsus::pattern(lane, m)
}

/// The pattern card's column width at 100 %.
const PATTERN_W: f32 = 268.0;
/// The notes card's column width at 100 %.
const NOTES_W: f32 = 176.0;

/// The pattern playing: its length and the pencil's grid; how it is drawn.
fn pattern_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    playing: Playing,
) {
    let lane = st.bank;
    let slot = playing.of(lane);
    let len = model::pattern_of(host.spec().lapsus.as_deref(), lane, slot).len as usize;
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
                if let Some(i) =
                    controls::segmented(ui, s, len, &labels, frame::tr("lapsus.length"), half)
                {
                    edit::set_len(host, lane, slot, i as u8);
                }
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
                    let copy = controls::key(
                        ui,
                        s,
                        Src::Key("lapsus.edit.copy"),
                        frame::tr("lapsus.edit.copy"),
                        w2,
                        false,
                    );
                    if copy.clicked() {
                        let p = model::pattern_of(host.spec().lapsus.as_deref(), lane, slot);
                        st.clip = Some(p.clone());
                    }
                    let paste = ui.add_enabled_ui(st.clip.is_some(), |ui| {
                        controls::key(
                            ui,
                            s,
                            Src::Key("lapsus.edit.paste"),
                            frame::tr("lapsus.edit.paste"),
                            w2,
                            false,
                        )
                    });
                    if paste.inner.clicked() {
                        if let Some(p) = st.clip.clone() {
                            host.set_lapsus_pattern(lane, slot as u8, Some(p), None);
                        }
                    }
                });
            });
        },
    );
}

/// The time bank's factory names (`lapsus.tslot.{i}`).
pub const TSLOT_KEYS: [&str; model::SLOTS] = [
    "lapsus.tslot.0",
    "lapsus.tslot.1",
    "lapsus.tslot.2",
    "lapsus.tslot.3",
    "lapsus.tslot.4",
    "lapsus.tslot.5",
    "lapsus.tslot.6",
    "lapsus.tslot.7",
    "lapsus.tslot.8",
    "lapsus.tslot.9",
    "lapsus.tslot.10",
    "lapsus.tslot.11",
    "lapsus.tslot.12",
    "lapsus.tslot.13",
    "lapsus.tslot.14",
    "lapsus.tslot.15",
    "lapsus.tslot.16",
    "lapsus.tslot.17",
    "lapsus.tslot.18",
    "lapsus.tslot.19",
    "lapsus.tslot.20",
    "lapsus.tslot.21",
    "lapsus.tslot.22",
    "lapsus.tslot.23",
    "lapsus.tslot.24",
    "lapsus.tslot.25",
    "lapsus.tslot.26",
    "lapsus.tslot.27",
    "lapsus.tslot.28",
    "lapsus.tslot.29",
    "lapsus.tslot.30",
    "lapsus.tslot.31",
    "lapsus.tslot.32",
    "lapsus.tslot.33",
    "lapsus.tslot.34",
    "lapsus.tslot.35",
];

/// The volume bank's factory names (`lapsus.vslot.{i}`).
pub const VSLOT_KEYS: [&str; model::SLOTS] = [
    "lapsus.vslot.0",
    "lapsus.vslot.1",
    "lapsus.vslot.2",
    "lapsus.vslot.3",
    "lapsus.vslot.4",
    "lapsus.vslot.5",
    "lapsus.vslot.6",
    "lapsus.vslot.7",
    "lapsus.vslot.8",
    "lapsus.vslot.9",
    "lapsus.vslot.10",
    "lapsus.vslot.11",
    "lapsus.vslot.12",
    "lapsus.vslot.13",
    "lapsus.vslot.14",
    "lapsus.vslot.15",
    "lapsus.vslot.16",
    "lapsus.vslot.17",
    "lapsus.vslot.18",
    "lapsus.vslot.19",
    "lapsus.vslot.20",
    "lapsus.vslot.21",
    "lapsus.vslot.22",
    "lapsus.vslot.23",
    "lapsus.vslot.24",
    "lapsus.vslot.25",
    "lapsus.vslot.26",
    "lapsus.vslot.27",
    "lapsus.vslot.28",
    "lapsus.vslot.29",
    "lapsus.vslot.30",
    "lapsus.vslot.31",
    "lapsus.vslot.32",
    "lapsus.vslot.33",
    "lapsus.vslot.34",
    "lapsus.vslot.35",
];

/// Slot `i`'s factory name in `lane`, translated.
pub fn slot_name(lane: Lane, i: usize) -> &'static str {
    let i = i.min(model::SLOTS - 1);
    frame::tr(match lane {
        Lane::Time => TSLOT_KEYS[i],
        Lane::Vol => VSLOT_KEYS[i],
    })
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
            // The first note of each bank, under one caption: each cell names its bank
            // in a whole word (`.short`), the parameter keeps its full name for hosts.
            let kw = controls::cells_width(&[Size::M, Size::M]);
            controls::column(ui, &sn, kw, |ui| {
                controls::caption(ui, &sn, Src::Key("lapsus.first_notes"), kw);
                ui.horizontal(|ui| {
                    controls::param_with(ui, &sn, host, "tkey", Size::M, Some(&note_pill));
                    controls::param_with(ui, &sn, host, "vkey", Size::M, Some(&note_pill));
                });
            });
        },
    );
}

/// A note number as its name: 48 → C3.
pub fn note_pill(v: f32, b: &mut String) {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    use std::fmt::Write as _;
    let n = v.round().clamp(0.0, 127.0) as i32;
    let _ = write!(b, "{}{}", NAMES[(n % 12) as usize], n / 12 - 1);
}

/// The inspector's picture: the two playing patterns, small, with their names.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    grid::preview(ui, spec)
}

/// The settings `--editor lapsus` and the screenshot tool open it with: *Tape stop ¼* —
/// three beats as they come, the last one stopping on the bar — with *Fade the last beat*.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "lapsus").set_many(DEMO, "cmd.fx_preset");
}

/// The demo's settings.
pub const DEMO: &[(&str, f32)] = &[("tslot", 3.0), ("vslot", 21.0), ("mix", 1.0)];

/// What a playing engine would feed the editor, for screenshots with no engine: a drum
/// loop (kick, snare, hats) through the unit for a bar and a half, its telemetry, the
/// analyser's pairs and the cycle's waveform.
pub fn demo_feed(app: &mut CantusApp) {
    grid::demo_feed(app);
}
