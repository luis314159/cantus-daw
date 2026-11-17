//! Comparatio's editor (ADR 0146, provisional; design sheet v1): the reference comparator —
//! a **two-deck console** on a felt desk. Between two cheeks, the faceplate holds your mix's
//! deck on the left and the reference's on the right (each its jewel lamp and name, its
//! loudness in a big readout — short term — and integrated, momentary, true peak and
//! loudness range under it, a level column on its outer side), the dark screen between them
//! (tonal balance, the difference, dynamics, stereo), and along the foot the reference's
//! strip (its name, length and overview with the playhead), the **big selector** that
//! switches between your mix and the reference at the same loudness, and the **blind
//! drawer** (A, B, X, the answers and the score with the chance of guessing it). The cards
//! hold the match, the playback, the match EQ and the display. Every number from the
//! engine's telemetry (`cantus_engine::fx::comparatio::t`).
//!
//! **Phase B** (the owner's verdict on sheet v1): the reference's strip holds **the list of
//! references** — a numbered memory key for each (the one compared lit), *Load reference…*
//! and *Remove*; a file dropped on the console is added and compared; each reference keeps
//! its own analysis (`comparatio_store::Summary`) and a file that changed or went missing
//! since it was loaded is said so. **Your mix's tonal curve is live** from the slot's
//! analyser tap ([`picture::Live`]). **The match EQ's suggestion** is drawn over the
//! difference before anything is sent, and *Send to Aequalis* puts it on an Aequalis just
//! before this unit (or updates the one it went to), one step of undo.
//!
//! Comparatio never changes your mix: the mix heard is the input to the bit; a render (an
//! export, a stem, a freeze) hears the mix, never the reference (ADR 0146).

use super::controls::{self, Size};
use super::frame;
use super::fxa_cells::{self as cells, Item};
use super::host::{DawHost, FxHost};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::color::{mix, with_alpha};
use crate::kit::hint::{self, Caps};
use crate::kit::shell::Grid;
use crate::kit::skin::comparator as pal;
use crate::kit::text::{Role, Text};
use crate::kit::{with_brush, Brush, Fill, Style};
use cantus_core::fx::comparatio::{Reference, References, MAX_REFERENCES};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use cantus_engine::fx::comparatio::{
    R_INTEGRATED, R_MOMENTARY, R_RANGE, R_SECONDS, R_SHORT, R_TRUE_PEAK,
};
use cantus_engine::fx::comparatio_store::{self as store, RefStatus, Summary};
use std::path::PathBuf;
use std::sync::Arc;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Sense, Vec2};

#[path = "comparatio_art.rs"]
mod art;
#[path = "comparatio_demo.rs"]
mod demo;
#[path = "comparatio_picture.rs"]
pub mod picture;
#[path = "comparatio_screen.rs"]
pub mod screen;

use picture::{num, Heard, Picture};

const KIND: EffectKind = EffectKind::Reference;

/// Factory presets: data (`presets/effects/reference.toml`, ADR 0101).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[];

/// Rows of cards in a language (the window grows a row for each more; the console keeps
/// its size).
pub fn card_rows(lang: crate::i18n::Locale) -> usize {
    match lang {
        crate::i18n::Locale::En => 1,
        crate::i18n::Locale::Es => 1,
    }
}

/// The designed size at 100 % in this thread's language.
pub fn designed_size() -> Vec2 {
    let rows = card_rows(crate::i18n::locale()) as f32;
    let row = controls::card_height(Size::M) + 4.0;
    Vec2::new(1400.0, 900.0 + (rows - 1.0) * row)
}

/// The states the design sheets and `fxc_whole` draw it in.
pub const DESIGN_STATES: &[(&str, &[(&str, f32)])] = &[
    ("start", &[]),
    ("reference", &[("listen", 1.0)]),
    (
        "short",
        &[
            ("listen", 1.0),
            ("match", 1.0),
            ("offset", -1.0),
            ("res", 1.0),
        ],
    ),
    ("loop", &[("sync", 2.0), ("lstart", 3.0), ("lend", 9.0)]),
    ("blind", &[("listen", 2.0), ("seed", 7.0), ("trial", 6.0)]),
];

/// The blind test's answers so far (the editor's; the unit knows only the seed and trial).
#[derive(Debug, Default, Clone)]
pub struct Blind {
    pub answers: Vec<bool>,
}

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    effect: Option<EffectId>,
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    pub pic: Box<Picture>,
    pub blind: Blind,
    words: Words,
    /// Where the console was drawn last frame (tests point at it).
    pub display: Option<Rect>,
    pub rows: usize,
    /// The console's controls last frame (tests press them).
    pub spots: Vec<Spot>,
    /// Your mix's tonal curves, measured from the slot's analyser tap.
    pub live: Option<Box<picture::Live>>,
    /// The summary of the reference compared, and what the curves were last made from
    /// (the summary, the resolution, the match EQ's two keys).
    summary: Option<Arc<Summary>>,
    made: Option<(usize, usize, u32, u32)>,
    /// A loop region being dragged on the overview: where it started and where the pointer
    /// is, seconds into the reference.
    pub dragging: Option<(f32, f32)>,
    /// Where the reference's overview was drawn last frame (tests drag across it).
    pub overview: Option<Rect>,
}

#[derive(Debug, Default)]
struct Words {
    match_line: String,
    reference: String,
    /// What became of the file: loading, missing, changed since it was loaded (empty: in).
    status: String,
    /// Whether `status` is a warning (drawn in the warning's colour).
    warn: bool,
    score: String,
    chance: String,
}

impl Words {
    fn update(&mut self, spec: &EffectSpec, pic: &Picture, blind: &Blind) {
        let g = pic.gain;
        self.match_line = if !pic.loaded() || !g.is_finite() {
            frame::tr("comparatio.match.none").to_string()
        } else if g >= 0.05 {
            crate::i18n::tf("comparatio.match.up", &[("db", &format!("{g:.1}"))])
        } else if g <= -0.05 {
            crate::i18n::tf("comparatio.match.down", &[("db", &format!("{:.1}", -g))])
        } else {
            frame::tr("comparatio.match.level").to_string()
        };
        let summary = spec.ir.as_deref().and_then(|f| store::summary(&f.path));
        (self.status, self.warn) = match spec.ir.as_deref() {
            Some(f) => {
                let entry = spec
                    .refs
                    .as_deref()
                    .and_then(|r| r.find(&f.path).map(|i| &r.list[i]));
                status_words(&f.path, entry, summary.as_deref())
            }
            None => (String::new(), false),
        };
        self.reference = match spec.ir.as_deref() {
            Some(f) if pic.loaded() || summary.is_some() => {
                let secs = if pic.loaded() {
                    pic.ref_len.max(0.0) as u32
                } else {
                    summary.as_ref().map_or(0, |x| x.seconds as u32)
                };
                let len = format!("{}:{:02}", secs / 60, secs % 60);
                match summary.as_ref().and_then(|x| x.lufs) {
                    Some(lufs) => crate::i18n::tf(
                        "comparatio.ref.length_loudness",
                        &[
                            ("name", &f.name()),
                            ("len", &len),
                            ("lufs", &format!("{lufs:.1}")),
                        ],
                    ),
                    None => crate::i18n::tf(
                        "comparatio.ref.length",
                        &[("name", &f.name()), ("len", &len)],
                    ),
                }
            }
            Some(f) if self.warn => f.name().to_string(),
            Some(f) => crate::i18n::tf("comparatio.ref.loading", &[("name", &f.name())]),
            None => frame::tr("comparatio.ref.none").to_string(),
        };
        let n = blind.answers.len() as u32;
        let right = blind.answers.iter().filter(|a| **a).count() as u32;
        self.score = if n == 0 {
            frame::tr("comparatio.blind.start").to_string()
        } else {
            crate::i18n::tf("comparatio.blind.score", &[("right", &right), ("n", &n)])
        };
        self.chance = if n == 0 {
            String::new()
        } else {
            let p = cantus_dsp::comparatio::blind::p_value(right, n) * 100.0;
            crate::i18n::tf("comparatio.blind.chance", &[("p", &format!("{p:.1}"))])
        };
    }
}

pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.comparatio);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(
        &mut DawHost::new(app, strip, effect, "comparatio"),
        &mut st,
        ui,
    );
    app.fx.comparatio = st;
    out
}

pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let tel = host.telemetry();
    let sr = host.sample_rate();
    let mut chips = std::mem::take(&mut st.chips);
    let chip = [chips.latency(0.0, sr)];
    let look = frame::Look {
        kind: KIND,
        chips: &chip,
        meter: None,
        more: &[],
    };
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, st, ui, s, body, &tel)
    });
    st.chrome = chrome;
    st.chips = chips;
    out
}

const MATCH_MODES: &[&str] = &[
    "fx.comparatio.match.0",
    "fx.comparatio.match.1",
    "fx.comparatio.match.2",
];
const REFL: &[&str] = &["fx.comparatio.refl.0", "fx.comparatio.refl.1"];
const SYNC: &[&str] = &[
    "fx.comparatio.sync.0",
    "fx.comparatio.sync.1",
    "fx.comparatio.sync.2",
];
const RES: &[&str] = &["comparatio.l.thirds", "comparatio.l.sixths"];
const MATCH: &[Item] = &[
    Item::Choice("match", "comparatio.l.match", MATCH_MODES),
    Item::Knob("offset", "comparatio.l.offset"),
    Item::Choice("refl", "comparatio.l.refl", REFL),
    Item::Knob("fade", "comparatio.l.fade"),
];
const PLAYBACK: &[Item] = &[
    Item::Choice("sync", "comparatio.l.sync", SYNC),
    Item::Pill("soff", "comparatio.l.ahead", seconds_pill),
    Item::Pill("lstart", "comparatio.l.start", seconds_pill),
    Item::Pill("lend", "comparatio.l.end", seconds_pill),
];
const DISPLAY: &[Item] = &[Item::Switch("res", "comparatio.l.res", RES)];
const MATCH_EQ: &[Item] = &[
    Item::Knob("eqamt", "comparatio.l.eqamt"),
    Item::Knob("eqmax", "comparatio.l.eqmax"),
];

fn seconds_pill(v: f32, b: &mut String) {
    use std::fmt::Write as _;
    let _ = write!(b, "{v:.1} s");
}

fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &[f32; 64],
) {
    let k = s.k;
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let row = controls::card_height(Size::M) * k;
    let gap = s.d(4.0);
    let cards: [cells::Card<'_>; 4] = [
        ("comparatio.card.match", MATCH),
        ("comparatio.card.playback", PLAYBACK),
        ("comparatio.card.match_eq", MATCH_EQ),
        ("comparatio.card.display", DISPLAY),
    ];
    let widths: Vec<f32> = cards
        .iter()
        .map(|(_, it)| k * cells::card_width(ui, it))
        .collect();
    let rows = cells::breaks(&widths, 6.0 * k, area.width());
    st.rows = rows.len();
    let top = (area.height() - rows.len() as f32 * (row + gap)).max(640.0 * k);
    let r1 = g.full(top);
    st.display = Some(r1);
    st.pic.update(tel);
    // A file dropped on the console: a reference, compared from now on.
    if let Some(files) = take_drop(ui, r1) {
        add_references(host, &files);
    }
    update_curves(host, st);
    display(host, st, ui, s, r1);
    for &(i, j) in &rows {
        let sum: f32 = widths[i..j].iter().sum::<f32>().max(1.0);
        let shares: Vec<f32> = widths[i..j].iter().map(|w| w / sum).collect();
        let rects: Vec<Rect> = g.row(row, &shares).collect();
        for ((title, items), rect) in cards[i..j].iter().zip(rects) {
            cells::card(host, ui, s, rect, title, items);
        }
    }
}

/// The audio files dropped on the console this frame (taken, so the window's own handler
/// does not import them as clips too).
fn take_drop(ui: &egui::Ui, rect: Rect) -> Option<Vec<PathBuf>> {
    let (files, at) = ui.ctx().input(|i| {
        (
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .filter(|p| !p.as_os_str().is_empty())
                .collect::<Vec<_>>(),
            i.pointer.latest_pos(),
        )
    });
    if files.is_empty() || !at.is_none_or(|p| rect.contains(p)) {
        return None;
    }
    let audio: Vec<PathBuf> = files
        .into_iter()
        .filter(|f| super::aula::is_audio(&f.to_string_lossy()))
        .collect();
    if audio.is_empty() {
        return None;
    }
    ui.ctx().input_mut(|i| i.raw.dropped_files.clear());
    Some(audio)
}

/// Your mix's curves from the tap, the reference's from its summary, their difference and
/// the match EQ's suggestion — made again only when something they come from changed (an
/// editor at rest makes nothing).
fn update_curves(host: &mut dyn FxHost, st: &mut State) {
    let (res, opts, path_key) = {
        let spec = host.spec();
        (
            spec.get("res").round().clamp(0.0, 1.0) as usize,
            picture::match_options(spec),
            spec.ir.as_deref().map(|f| store::summary(&f.path)),
        )
    };
    let sr = host.sample_rate();
    let live = st
        .live
        .get_or_insert_with(|| Box::new(picture::Live::new(sr)));
    live.pull(host, st.pic.mix[R_SECONDS]);
    let fresh = live.take_fresh();
    let summary = path_key.flatten();
    let at = summary.as_ref().map_or(0, |x| Arc::as_ptr(x) as usize);
    let made = (at, res, opts.amount.to_bits(), opts.max_db.to_bits());
    if !fresh && st.made == Some(made) {
        return;
    }
    st.made = Some(made);
    st.summary = summary.clone();
    let mix = live.curve(res);
    match (mix, summary) {
        (Some(m), Some(x)) => {
            let r = picture::reference_of(&mut st.pic, &x, res);
            st.pic.set_all(m, r, &opts);
        }
        (None, Some(x)) => {
            let r = picture::reference_of(&mut st.pic, &x, res);
            st.pic.clear_curves();
            st.pic.ref_curve = Some(Box::new(r));
        }
        (Some(m), None) => {
            st.pic.clear_curves();
            st.pic.overview.clear();
            st.pic.mix_curve = Some(Box::new(m));
        }
        (None, None) => {
            st.pic.clear_curves();
            st.pic.overview.clear();
        }
    }
}

/// How many references the strip lists and which is compared — no copy (every frame).
fn count_at(spec: &EffectSpec) -> (usize, Option<usize>) {
    let list = spec.refs.as_deref().map_or(&[][..], |r| &r.list[..]);
    match spec.ir.as_deref() {
        Some(f) => match list.iter().position(|r| r.path == f.path) {
            Some(i) => (list.len(), Some(i)),
            None => (list.len() + 1, Some(list.len())),
        },
        None => (list.len(), None),
    }
}

/// The references the strip lists and which is compared.
fn listed(spec: &EffectSpec) -> (Vec<Reference>, Option<usize>) {
    let list = cantus_core::fx::comparatio::listed(spec.refs.as_deref(), spec.ir.as_deref());
    let at = spec
        .ir
        .as_deref()
        .and_then(|f| list.iter().position(|r| r.path == f.path));
    (list, at)
}

/// What became of a reference's file, in words, and whether it is a warning.
fn status_words(path: &str, entry: Option<&Reference>, summary: Option<&Summary>) -> (String, bool) {
    match store::status(path) {
        RefStatus::Failed(e) => {
            let key = if e.contains("not there") {
                "comparatio.ref.missing"
            } else {
                "comparatio.ref.failed"
            };
            (frame::tr(key).to_string(), true)
        }
        _ => match summary {
            Some(x)
                if entry.is_some_and(|e| {
                    !e.hash.is_empty() && (x.hash != e.hash || x.bytes != e.bytes)
                }) =>
            {
                (frame::tr("comparatio.ref.changed").to_string(), true)
            }
            _ => (String::new(), false),
        },
    }
}

/// Add `files` as references (one step): each appended (one already listed takes the size
/// and hash it has now), the last of them compared. At most [`MAX_REFERENCES`]: a file past
/// them is left, said in the status line.
pub fn add_references(host: &mut dyn FxHost, files: &[PathBuf]) -> bool {
    let spec = host.spec();
    let (mut list, _) = listed(spec);
    let eq = spec.refs.as_deref().and_then(|r| r.eq);
    let mut chosen = None;
    let mut full = false;
    for f in files {
        let path = host.store_reference(f);
        let (bytes, hash) = store::fingerprint(f).unwrap_or((0, String::new()));
        match list.iter().position(|r| r.path == path) {
            Some(i) => {
                list[i].bytes = bytes;
                list[i].hash = hash;
                chosen = Some(i);
            }
            None if list.len() < MAX_REFERENCES => {
                list.push(Reference { path, bytes, hash });
                chosen = Some(list.len() - 1);
            }
            None => full = true,
        }
    }
    if full {
        host.status(crate::i18n::tf(
            "comparatio.status.full",
            &[("n", &MAX_REFERENCES)],
        ));
    }
    let Some(i) = chosen else {
        return false;
    };
    let selected = list[i].file();
    host.set_references(Some(References { list, eq }), Some(selected))
}

/// Compare the `i`th reference of the list (one step).
fn select(host: &mut dyn FxHost, i: usize) {
    let spec = host.spec();
    let (list, at) = listed(spec);
    if at == Some(i) || i >= list.len() {
        return;
    }
    let eq = spec.refs.as_deref().and_then(|r| r.eq);
    let selected = list[i].file();
    host.set_references(Some(References { list, eq }), Some(selected));
}

/// Take the reference compared off the list (one step): the next one compared in its place,
/// or the one before, or none.
fn remove(host: &mut dyn FxHost) {
    let spec = host.spec();
    let (mut list, at) = listed(spec);
    let Some(i) = at else {
        return;
    };
    let eq = spec.refs.as_deref().and_then(|r| r.eq);
    list.remove(i);
    let selected = list.get(i.min(list.len().saturating_sub(1))).map(Reference::file);
    let refs = References { list, eq };
    host.set_references((!refs.is_empty()).then_some(refs), selected);
}

/// The file the user picks for *Load reference…*: a native dialog (or, for the tests, the
/// file they set with [`pick_for_tests`]).
fn pick_files() -> Vec<PathBuf> {
    if let Some(f) = PICK.with(|p| p.borrow_mut().take()) {
        return f;
    }
    rfd::FileDialog::new()
        .set_title(crate::i18n::t("comparatio.pick.title"))
        .add_filter(
            crate::i18n::t("comparatio.pick.audio"),
            &super::aula::EXTENSIONS,
        )
        .pick_files()
        .unwrap_or_default()
}

thread_local! {
    static PICK: std::cell::RefCell<Option<Vec<PathBuf>>> = const { std::cell::RefCell::new(None) };
}

/// For the tests: what the next *Load reference…* picks, instead of a dialog.
pub fn pick_for_tests(files: Vec<PathBuf>) {
    PICK.with(|p| *p.borrow_mut() = Some(files));
}

/// The match EQ's suggestion as an Aequalis: fitted into editable bands (at most six, in
/// minimum phase — Aequalis's own choice to change), its output gain keeping the mix's
/// loudness where it was. `None` with nothing to send.
pub fn match_eq_spec(pic: &Picture, sample_rate: f32) -> Option<EffectSpec> {
    let m = pic.match_eq.as_deref()?;
    let fit = cantus_dsp::aequalis::fit::fit(
        m.points(),
        &cantus_dsp::aequalis::fit::FitOptions::default(),
        sample_rate.max(8_000.0),
    );
    if fit.bands.is_empty() {
        return None;
    }
    let mut spec = EffectSpec::new(EffectKind::Eq);
    for (n, b) in fit.bands.iter().enumerate() {
        for (k, v) in crate::views::eq_editor::band_fields(n, b) {
            let _ = spec.set(k, v);
        }
    }
    let _ = spec.set("out", m.makeup_db.clamp(-24.0, 24.0));
    Some(spec)
}

/// Put `eq` before Comparatio `effect` on `strip` (one step): in place of the Aequalis it
/// was sent to last, while that one is still on the strip; else a new Aequalis just before
/// the unit, its id kept in the unit's references.
pub fn send_match_eq(app: &mut CantusApp, strip: StripId, effect: EffectId, eq: EffectSpec) -> bool {
    use cantus_core::Command;
    let Some(chain) = app.project().effects_of(strip) else {
        return false;
    };
    let Some(at) = chain.iter().position(|e| e.id == effect) else {
        return false;
    };
    let me = chain[at].spec.clone();
    let sent = me.refs.as_deref().and_then(|r| r.eq).and_then(|id| {
        chain
            .iter()
            .find(|e| e.id == id && e.spec.kind == EffectKind::Eq)
            .cloned()
    });
    let ok = match sent {
        Some(mut slot) => {
            let id = slot.id;
            slot.spec = eq;
            app.exec(Command::ReplaceEffect {
                strip,
                effect: id,
                slot: Box::new(slot),
            })
        }
        None => {
            let mut slot = app.bus.new_effect(EffectKind::Eq);
            slot.spec = eq;
            let id = slot.id;
            let mut refs = me.refs.as_deref().cloned().unwrap_or_default();
            refs.eq = Some(id);
            let mut keep = References {
                list: cantus_core::fx::comparatio::listed(Some(&refs), me.ir.as_deref()),
                eq: Some(id),
            };
            keep.list.truncate(MAX_REFERENCES);
            app.exec(Command::Batch {
                label_key: "cmd.comparatio_send_eq".into(),
                commands: vec![
                    Command::InsertEffect {
                        strip,
                        index: at,
                        slot: Box::new(slot),
                    },
                    Command::SetReferences {
                        strip,
                        effect,
                        refs: Some(Box::new(keep)),
                        selected: me.ir.clone(),
                    },
                ],
            })
        }
    };
    if ok {
        app.set_status(frame::tr("comparatio.status.sent").to_string());
    }
    ok
}

/// What a press on the console does.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Act {
    /// Hear this (`listen`'s value).
    Listen(f32),
    /// The big selector: from the mix to the reference and back.
    Swap,
    /// The blind test's answer: X is your mix (`false`) or the reference (`true`).
    Answer(bool),
    /// A new blind test: a new order, the answers cleared.
    Restart,
    /// Compare the `n`th reference of the list.
    Select(usize),
    /// *Load reference…*: pick files, add them, compare the last.
    Load,
    /// Take the reference compared off the list.
    Remove,
    /// *Send to Aequalis*: the match EQ's suggestion before this unit.
    SendEq,
}

#[derive(Copy, Clone, Debug)]
pub struct Spot {
    pub rect: Rect,
    pub act: Act,
    pub tip: &'static str,
}

/// The console's layout.
struct Layout {
    console: Rect,
    cheeks: [Rect; 2],
    plate: Rect,
    decks: [Rect; 2],
    screen: screen::Plot,
    strip: Rect,
    selector: Pos2,
    sel_rad: f32,
    lamps: [Pos2; 2],
    drawer: Rect,
    keys: [Rect; 3],
    answers: [Rect; 2],
    restart: Rect,
}

fn lay(r: Rect, k: f32, ctx: &egui::Context) -> Layout {
    let console = Rect::from_min_max(
        pos2(r.left() + 10.0 * k, r.top() + 10.0 * k),
        pos2(r.right() - 10.0 * k, r.bottom() - 16.0 * k),
    );
    let cw = 26.0 * k;
    let cheeks = [
        Rect::from_min_max(console.min, pos2(console.left() + cw, console.bottom())),
        Rect::from_min_max(pos2(console.right() - cw, console.top()), console.max),
    ];
    let plate = Rect::from_min_max(
        pos2(cheeks[0].right() + 2.0 * k, console.top() + 6.0 * k),
        pos2(cheeks[1].left() - 2.0 * k, console.bottom() - 6.0 * k),
    );
    let inner = plate.shrink2(vec2(26.0 * k, 24.0 * k));
    let foot_h = 150.0 * k;
    let upper = Rect::from_min_max(
        pos2(inner.left(), inner.top() + 8.0 * k),
        pos2(inner.right(), inner.bottom() - foot_h - 14.0 * k),
    );
    let deck_w = 236.0 * k;
    let decks = [
        Rect::from_min_max(upper.min, pos2(upper.left() + deck_w, upper.bottom())),
        Rect::from_min_max(pos2(upper.right() - deck_w, upper.top()), upper.max),
    ];
    let glass = Rect::from_min_max(
        pos2(decks[0].right() + 18.0 * k, upper.top()),
        pos2(decks[1].left() - 18.0 * k, upper.bottom()),
    )
    .shrink(3.0 * k);
    let screen = screen::Plot::lay(glass, k);
    let foot = Rect::from_min_max(pos2(inner.left(), upper.bottom() + 14.0 * k), inner.max);
    let sel_rad = 40.0 * k;
    let selector = pos2(foot.center().x, foot.center().y + 22.0 * k);
    let lamps = [
        selector + vec2(-70.0 * k, -50.0 * k),
        selector + vec2(70.0 * k, -50.0 * k),
    ];
    let strip = Rect::from_min_max(foot.min, pos2(selector.x - 200.0 * k, foot.bottom()));
    let drawer = Rect::from_min_max(pos2(selector.x + 200.0 * k, foot.top()), foot.max);
    let key = 44.0 * k;
    let ky = drawer.top() + 30.0 * k;
    let keys = [0, 1, 2].map(|i| {
        Rect::from_min_size(
            pos2(drawer.left() + 14.0 * k + i as f32 * (key + 10.0 * k), ky),
            vec2(key, key),
        )
    });
    let small_w = |key: &str| {
        let t = Text::key(key, Role::Micro, Color32::TRANSPARENT).caps(true);
        crate::kit::text::measure(ctx, &t, k).x + 24.0 * k
    };
    let ax = keys[2].right() + 18.0 * k;
    let a0 = Rect::from_min_size(
        pos2(ax, ky),
        vec2(small_w("comparatio.blind.x_is_a"), 20.0 * k),
    );
    let a1 = Rect::from_min_size(
        pos2(ax, ky + 24.0 * k),
        vec2(small_w("comparatio.blind.x_is_b"), 20.0 * k),
    );
    let rw = small_w("comparatio.blind.restart");
    let restart = Rect::from_min_size(
        pos2(drawer.right() - 12.0 * k - rw, drawer.top() + 8.0 * k),
        vec2(rw, 20.0 * k),
    );
    Layout {
        console,
        cheeks,
        plate,
        decks,
        screen,
        strip,
        selector,
        sel_rad,
        lamps,
        drawer,
        keys,
        answers: [a0, a1],
        restart,
    }
}

fn display(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui, s: &Style, r: Rect) {
    let k = s.k;
    let ctx = ui.ctx().clone();
    let l = lay(r, k, &ctx);
    let mut spots = vec![
        Spot {
            rect: Rect::from_center_size(l.selector, Vec2::splat(l.sel_rad * 2.2)),
            act: Act::Swap,
            tip: "comparatio.tip.selector",
        },
        Spot {
            rect: Rect::from_center_size(l.lamps[0] - vec2(50.0 * k, 0.0), vec2(120.0, 30.0) * k),
            act: Act::Listen(0.0),
            tip: "comparatio.tip.mix",
        },
        Spot {
            rect: Rect::from_center_size(l.lamps[1] + vec2(50.0 * k, 0.0), vec2(120.0, 30.0) * k),
            act: Act::Listen(1.0),
            tip: "comparatio.tip.reference",
        },
    ];
    for (i, (v, tip)) in [
        (0.0, "comparatio.tip.a"),
        (1.0, "comparatio.tip.b"),
        (2.0, "comparatio.tip.x"),
    ]
    .iter()
    .enumerate()
    {
        spots.push(Spot {
            rect: l.keys[i],
            act: Act::Listen(*v),
            tip,
        });
    }
    spots.push(Spot {
        rect: l.answers[0],
        act: Act::Answer(false),
        tip: "comparatio.tip.answer",
    });
    spots.push(Spot {
        rect: l.answers[1],
        act: Act::Answer(true),
        tip: "comparatio.tip.answer",
    });
    spots.push(Spot {
        rect: l.restart,
        act: Act::Restart,
        tip: "comparatio.tip.restart",
    });
    // Phase B: the reference's strip (its numbered keys, Load and Remove) and Send to
    // Aequalis — after the console's own, so their numbers stay.
    let (count, at) = count_at(host.spec());
    let strip = strip_lay(&ctx, k, l.strip, count, at.is_some());
    spots.push(Spot {
        rect: strip.load,
        act: Act::Load,
        tip: "comparatio.tip.load",
    });
    if let Some(r) = strip.remove {
        spots.push(Spot {
            rect: r,
            act: Act::Remove,
            tip: "comparatio.tip.remove",
        });
    }
    for (i, r) in strip.keys.iter().enumerate() {
        spots.push(Spot {
            rect: *r,
            act: Act::Select(i),
            tip: "comparatio.tip.select",
        });
    }
    let send = (st.pic.match_eq.is_some() && host.can_send_match_eq())
        .then(|| screen::send_rect(&ctx, k, &l.screen));
    if let Some(r) = send {
        spots.push(Spot {
            rect: r,
            act: Act::SendEq,
            tip: "comparatio.tip.send",
        });
    }
    let mut hot = None;
    for (i, sp) in spots.iter().enumerate() {
        let resp = ui.interact(
            sp.rect,
            ui.id().with(("comparatio-spot", i)),
            Sense::click(),
        );
        // What is heard offers automation and MIDI learn on each of its keys (a
        // footswitch for A/B); the blind test's order and trials are no control's.
        let name = frame::tr("fx.comparatio.listen");
        let learnt = match sp.act {
            Act::Listen(v) => {
                let rest = if v == 0.0 { 1.0 } else { 0.0 };
                controls::stepped_offer_one(ui, s, host, "listen", name, &resp, rest, v)
            }
            Act::Swap => controls::stepped_offer_one(ui, s, host, "listen", name, &resp, 0.0, 1.0),
            _ => false,
        };
        if resp.clicked() && !learnt {
            press(host, &mut st.blind, &st.pic, sp.act);
        }
        if resp.hovered() {
            hot = Some(i);
            hint::set_text(
                ui.ctx(),
                frame::tr("effect.comparatio"),
                frame::tr(sp.tip),
                Caps::NONE,
            );
        }
    }
    st.overview = Some(strip.window);
    drag_loop(host, st, ui, k, strip.window);
    let spec = host.spec().clone();
    st.words.update(&spec, &st.pic, &st.blind);
    // The loop region shown on the overview: the one being dragged, or the one set.
    let len = ref_seconds(&st.pic, st.summary.as_deref());
    let region = st.dragging.or_else(|| {
        (spec.get("sync").round() as i32 == 2).then(|| {
            let (a, b) = (spec.get("lstart"), spec.get("lend"));
            (a, if b > a { b } else { len })
        })
    });
    let region = region
        .filter(|_| len > 0.0)
        .map(|(a, b)| {
            (
                overview_x(strip.window, k, a.min(b), len),
                overview_x(strip.window, k, a.max(b), len),
            )
        });
    st.spots.clone_from(&spots);
    let res = spec.get("res").round() as usize;
    let (mv, rv) = art::voices(s);
    with_brush(&ui.painter_at(r), |b| {
        art::desk(b, s, r);
        b.rrect_soft(
            l.console.translate(vec2(0.0, 10.0 * k)),
            12.0 * k,
            Fill::Solid(with_alpha(art::shadow(), 0.6)),
            16.0 * k,
        );
        art::faceplate(b, s, l.plate);
        art::cheek(b, s, l.cheeks[0], true);
        art::cheek(b, s, l.cheeks[1], false);
        art::window(b, s, l.screen.glass.expand(3.0 * k), 6.0 * k);
        let hot_act = hot.map(|i| spots[i].act);
        screen::paint(b, s, &l.screen, &st.pic, res, send, hot_act == Some(Act::SendEq));
        deck(b, s, l.decks[0], &st.pic, false, mv, &st.words);
        deck(b, s, l.decks[1], &st.pic, true, rv, &st.words);
        reference_strip(b, s, &strip, &st.pic, &st.words, rv, at, hot_act, region);
        selector(b, s, &l, &st.pic, hot == Some(0), mv, rv);
        drawer(b, s, &l, &spec, &st.blind, &st.words, hot, mv, rv);
        let model = Text::key("comparatio.model", Role::Micro, Color32::TRANSPARENT).caps(true);
        art::print(
            b,
            s,
            pos2(l.plate.center().x, l.plate.top() + 14.0 * k),
            Align2::CENTER_CENTER,
            &model,
            1.0,
        );
    });
}

fn press(host: &mut dyn FxHost, blind: &mut Blind, pic: &Picture, act: Act) {
    let spec = host.spec();
    match act {
        Act::Listen(v) => host.set("listen", v, None),
        Act::Swap => {
            let now = spec.get("listen");
            let to = if (0.5..1.5).contains(&now) { 0.0 } else { 1.0 };
            host.set("listen", to, None);
        }
        Act::Answer(says_reference) => {
            let seed = spec.get("seed") as u64;
            let trial = spec.get("trial") as u32;
            let right =
                cantus_dsp::comparatio::blind::x_is_reference(seed, trial) == says_reference;
            blind.answers.push(right);
            host.set("trial", (trial + 1) as f32, None);
        }
        Act::Restart => {
            blind.answers.clear();
            let seed = fresh_seed(spec.get("seed") as u32);
            host.set_many(
                &[("seed", seed as f32), ("trial", 0.0), ("listen", 2.0)],
                "cmd.fx_preset",
            );
        }
        Act::Select(i) => select(host, i),
        Act::Load => {
            let files = pick_files();
            if !files.is_empty() {
                add_references(host, &files);
            }
        }
        Act::Remove => remove(host),
        Act::SendEq => {
            if let Some(eq) = match_eq_spec(pic, host.sample_rate()) {
                host.send_match_eq(eq);
            }
        }
    }
}

/// A new blind test's order: from the clock, never the last one's — so no listener can learn
/// an order (a preset or a reload puts the seed back to 0; the next test is not seed 1).
fn fresh_seed(old: u32) -> u32 {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos() ^ (d.as_secs() as u32).rotate_left(16));
    let s = (t ^ old.wrapping_mul(0x9E37_79B9)) % 65_536;
    if s == old {
        (s + 1) % 65_536
    } else {
        s
    }
}

fn put(b: &mut Brush, k: f32, at: Pos2, align: Align2, t: &Text<'_>) -> Rect {
    super::canvas::put(b, at, align, t, k, None)
}

/// A deck: its lamp and name, the big short-term readout, the other readings in rows, the
/// level column on its outer side. The reference's levels are as heard (its match gain on).
fn deck(
    b: &mut Brush,
    s: &Style,
    r: Rect,
    pic: &Picture,
    is_ref: bool,
    voice: Color32,
    words: &Words,
) {
    let k = s.k;
    let readings = if is_ref { &pic.reference } else { &pic.mix };
    let lit = match pic.heard {
        Heard::Fading => {
            if is_ref {
                pic.fade
            } else {
                1.0 - pic.fade
            }
        }
        Heard::Mix => f32::from(u8::from(!is_ref)),
        Heard::Reference => f32::from(u8::from(is_ref)),
        Heard::Hidden => 0.0,
    };
    let lamp = pos2(r.left() + 10.0 * k, r.top() + 12.0 * k);
    art::jewel(b, s, lamp, 5.0 * k, lit, voice);
    let name_key = if is_ref {
        "comparatio.deck.reference"
    } else {
        "comparatio.deck.mix"
    };
    let name = Text::key(name_key, Role::Label, Color32::TRANSPARENT).caps(true);
    art::print(
        b,
        s,
        pos2(lamp.x + 14.0 * k, lamp.y),
        Align2::LEFT_CENTER,
        &name,
        1.1,
    );
    let win = Rect::from_min_max(
        pos2(r.left(), r.top() + 30.0 * k),
        pos2(r.right(), r.bottom() - 30.0 * k),
    );
    let g = art::window(b, s, win, 6.0 * k);
    let col_w = 24.0 * k;
    let col = if is_ref {
        Rect::from_min_max(
            pos2(g.right() - col_w - 8.0 * k, g.top() + 14.0 * k),
            pos2(g.right() - 8.0 * k, g.bottom() - 14.0 * k),
        )
    } else {
        Rect::from_min_max(
            pos2(g.left() + 8.0 * k, g.top() + 14.0 * k),
            pos2(g.left() + 8.0 * k + col_w, g.bottom() - 14.0 * k),
        )
    };
    let y_of = |lufs: f32| col.bottom() - ((lufs + 48.0) / 48.0).clamp(0.0, 1.0) * col.height();
    b.rrect(col, 2.0 * k, Fill::Solid(art::c(pal::GLASS_FOOT)));
    // Each source's own loudness (the reference's before its match gain: the gain is the
    // line under the deck).
    let gain = 0.0;
    let m = readings[R_MOMENTARY] + gain;
    for i in 0..48 {
        let lufs = -48.0 + i as f32 + 0.5;
        let seg = Rect::from_min_max(
            pos2(col.left() + 3.0 * k, y_of(lufs + 0.5) + 0.6 * k),
            pos2(col.right() - 3.0 * k, y_of(lufs - 0.5) - 0.6 * k),
        );
        let on = m.is_finite() && lufs <= m;
        b.rect(
            seg,
            Fill::Solid(with_alpha(voice, if on { 0.85 } else { 0.08 })),
        );
    }
    let dim = art::c(pal::SCREEN_DIM);
    let ink = art::c(pal::SCREEN_INK);
    for (lufs, w) in [
        (-6.0f32, "-6"),
        (-14.0, "-14"),
        (-23.0, "-23"),
        (-36.0, "-36"),
    ] {
        let (x, al) = if is_ref {
            (col.left() - 4.0 * k, Align2::RIGHT_CENTER)
        } else {
            (col.right() + 4.0 * k, Align2::LEFT_CENTER)
        };
        put(
            b,
            k,
            pos2(x, y_of(lufs)),
            al,
            &Text::new(w, Role::Micro, dim).caps(false),
        );
    }
    let short = readings[R_SHORT] + gain;
    if short.is_finite() {
        let y = y_of(short);
        b.segment(
            pos2(col.left() - 2.0 * k, y),
            pos2(col.right() + 2.0 * k, y),
            1.6 * k,
            mix(voice, art::c(pal::LAMP_HOT), 0.5),
            false,
        );
    }
    let txt = if is_ref {
        Rect::from_min_max(
            g.min + vec2(12.0 * k, 0.0),
            pos2(col.left() - 30.0 * k, g.bottom()),
        )
    } else {
        Rect::from_min_max(
            pos2(col.right() + 32.0 * k, g.top()),
            g.max - vec2(12.0 * k, 0.0),
        )
    };
    let mut buf = String::new();
    num(short, 1, &mut buf);
    put(
        b,
        k,
        pos2(txt.right(), txt.top() + 34.0 * k),
        Align2::RIGHT_CENTER,
        &Text::new(&buf, Role::Hero, voice).caps(false),
    );
    put(
        b,
        k,
        pos2(txt.right(), txt.top() + 62.0 * k),
        Align2::RIGHT_CENTER,
        &Text::key("comparatio.r.short", Role::Micro, dim).caps(false),
    );
    let rows: [(&str, usize, &str); 4] = [
        ("comparatio.r.integrated", R_INTEGRATED, "LUFS"),
        ("comparatio.r.momentary", R_MOMENTARY, "LUFS"),
        ("comparatio.r.true_peak", R_TRUE_PEAK, "dBTP"),
        ("comparatio.r.range", R_RANGE, "LU"),
    ];
    let mut y = txt.top() + 94.0 * k;
    for (key, idx, unit) in rows {
        let add = if idx == R_RANGE { 0.0 } else { gain };
        put(
            b,
            k,
            pos2(txt.left(), y),
            Align2::LEFT_CENTER,
            &Text::key(key, Role::Micro, ink).caps(false),
        );
        num(readings[idx] + add, 1, &mut buf);
        buf.push(' ');
        buf.push_str(unit);
        put(
            b,
            k,
            pos2(txt.right(), y + 16.0 * k),
            Align2::RIGHT_CENTER,
            &Text::new(&buf, Role::Value, voice).caps(false),
        );
        b.hline(
            txt.left(),
            txt.right(),
            y + 29.0 * k,
            0.5 * k,
            with_alpha(dim, 0.35),
        );
        y += 40.0 * k;
    }
    art::glass_sheen(b, s, g);
    let under = if is_ref {
        Text::new(&words.match_line, Role::Micro, Color32::TRANSPARENT).caps(false)
    } else if pic.heard == Heard::Hidden {
        Text::key("comparatio.heard.hidden", Role::Micro, Color32::TRANSPARENT).caps(false)
    } else {
        Text::key(
            "comparatio.heard.untouched",
            Role::Micro,
            Color32::TRANSPARENT,
        )
        .caps(false)
    };
    art::print(
        b,
        s,
        pos2(r.center().x, r.bottom() - 13.0 * k),
        Align2::CENTER_CENTER,
        &under,
        1.0,
    );
}

/// Where the overview draws a second `t` of a reference `len` long, and back.
fn overview_x(window: Rect, k: f32, t: f32, len: f32) -> f32 {
    let g = window.shrink(3.0 * k);
    g.left() + 4.0 * k + (g.width() - 8.0 * k) * (t / len.max(1e-3)).clamp(0.0, 1.0)
}

fn overview_t(window: Rect, k: f32, x: f32, len: f32) -> f32 {
    let g = window.shrink(3.0 * k);
    ((x - g.left() - 4.0 * k) / (g.width() - 8.0 * k).max(1.0)).clamp(0.0, 1.0) * len
}

/// The reference's length in seconds: as the unit plays it, or as its file was read.
fn ref_seconds(pic: &Picture, summary: Option<&Summary>) -> f32 {
    if pic.ref_len > 0.0 {
        pic.ref_len
    } else {
        summary.map_or(0.0, |x| x.seconds as f32)
    }
}

/// A drag across the overview loops that stretch of the reference: playback Loop, its start
/// and end — one step of undo when it is let go (shown as it is dragged).
fn drag_loop(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui, k: f32, window: Rect) {
    let len = ref_seconds(&st.pic, st.summary.as_deref());
    let resp = ui.interact(
        window,
        ui.id().with("comparatio-overview"),
        Sense::drag(),
    );
    if len <= 0.0 {
        st.dragging = None;
        return;
    }
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("effect.comparatio"),
            frame::tr("comparatio.tip.loop"),
            Caps::NONE,
        );
    }
    let at = |p: Pos2| (overview_t(window, k, p.x, len) * 10.0).round() / 10.0;
    if resp.drag_started() {
        if let Some(p) = resp.interact_pointer_pos() {
            let t = at(p);
            st.dragging = Some((t, t));
        }
    }
    if resp.dragged() {
        if let (Some((a, _)), Some(p)) = (st.dragging, resp.interact_pointer_pos()) {
            st.dragging = Some((a, at(p)));
        }
    }
    if resp.drag_stopped() {
        if let Some((a, b)) = st.dragging.take() {
            let (lo, hi) = (a.min(b), a.max(b));
            if hi - lo >= 0.1 {
                host.set_many(
                    &[("sync", 2.0), ("lstart", lo), ("lend", hi)],
                    "cmd.comparatio_loop",
                );
            }
        }
    }
}

/// The reference's strip laid out: the title row with *Load reference…* and *Remove*, a
/// numbered memory key per reference, the name line, the overview's window.
struct StripLay {
    r: Rect,
    load: Rect,
    remove: Option<Rect>,
    keys: Vec<Rect>,
    name_y: f32,
    window: Rect,
}

fn strip_lay(ctx: &egui::Context, k: f32, r: Rect, n: usize, compared: bool) -> StripLay {
    let w = |key: &str| {
        let t = Text::key(key, Role::Micro, Color32::TRANSPARENT).caps(true);
        crate::kit::text::measure(ctx, &t, k).x + 24.0 * k
    };
    let lw = w("comparatio.ref.load");
    let load = Rect::from_min_size(pos2(r.right() - lw, r.top()), vec2(lw, 20.0 * k));
    let remove = compared.then(|| {
        let rw = w("comparatio.ref.remove");
        Rect::from_min_size(pos2(load.left() - 8.0 * k - rw, r.top()), vec2(rw, 20.0 * k))
    });
    let key = 22.0 * k;
    let keys = (0..n.min(MAX_REFERENCES))
        .map(|i| {
            Rect::from_min_size(
                pos2(r.left() + i as f32 * (key + 5.0 * k), r.top() + 28.0 * k),
                vec2(key, key),
            )
        })
        .collect();
    StripLay {
        r,
        load,
        remove,
        keys,
        name_y: r.top() + 64.0 * k,
        window: Rect::from_min_max(
            pos2(r.left(), r.top() + 78.0 * k),
            pos2(r.right(), r.bottom() - 6.0 * k),
        ),
    }
}

/// The reference's strip: the list as numbered memory keys (the one compared lit), *Load
/// reference…* and *Remove*, the one compared's name, length and loudness, what became of
/// its file, its overview with the playhead.
#[allow(clippy::too_many_arguments)]
fn reference_strip(
    b: &mut Brush,
    s: &Style,
    l: &StripLay,
    pic: &Picture,
    words: &Words,
    rv: Color32,
    at: Option<usize>,
    hot: Option<Act>,
    region: Option<(f32, f32)>,
) {
    let k = s.k;
    let r = l.r;
    let title = Text::key("comparatio.ref.title", Role::Micro, Color32::TRANSPARENT).caps(true);
    art::print(
        b,
        s,
        pos2(r.left(), l.load.center().y),
        Align2::LEFT_CENTER,
        &title,
        1.0,
    );
    small_key(b, s, l.load, "comparatio.ref.load", hot == Some(Act::Load));
    if let Some(rm) = l.remove {
        small_key(b, s, rm, "comparatio.ref.remove", hot == Some(Act::Remove));
    }
    let mut digits = String::new();
    for (i, key) in l.keys.iter().enumerate() {
        use std::fmt::Write as _;
        digits.clear();
        let _ = write!(digits, "{}", i + 1);
        memory_key(b, s, *key, &digits, at == Some(i), hot == Some(Act::Select(i)), rv);
    }
    if !words.status.is_empty() {
        let x = l.keys.last().map_or(r.left(), |k2| k2.right() + 12.0 * k);
        let col = if words.warn {
            art::c(pal::WRONG)
        } else {
            art::c(art::console(s).print)
        };
        let t = Text::new(&words.status, Role::Micro, col).caps(false);
        put(b, k, pos2(x, r.top() + 39.0 * k), Align2::LEFT_CENTER, &t);
    }
    let name = Text::new(&words.reference, Role::Label, Color32::TRANSPARENT).caps(false);
    art::print(
        b,
        s,
        pos2(r.left(), l.name_y),
        Align2::LEFT_CENTER,
        &name,
        1.0,
    );
    let g = art::window(b, s, l.window, 4.0 * k);
    if let Some((x0, x1)) = region {
        let band = Rect::from_min_max(pos2(x0, g.top() + 1.0 * k), pos2(x1, g.bottom() - 1.0 * k));
        b.rect(band, Fill::Solid(with_alpha(art::c(pal::LAMP_HOT), 0.14)));
        for x in [x0, x1] {
            b.vline(x, g.top() + 1.0 * k, g.bottom() - 1.0 * k, 1.0 * k, art::c(pal::SCREEN_INK));
        }
    }
    let n = pic.overview.len();
    if n > 1 {
        let cy = g.center().y;
        let h = g.height() * 0.40;
        let span = g.width() - 8.0 * k;
        let top: Vec<Pos2> = pic
            .overview
            .iter()
            .enumerate()
            .map(|(i, v)| {
                pos2(
                    g.left() + 4.0 * k + span * i as f32 / (n - 1) as f32,
                    cy - v * h,
                )
            })
            .collect();
        let bot: Vec<Pos2> = top.iter().map(|p| pos2(p.x, 2.0 * cy - p.y)).collect();
        b.between(&top, &bot, with_alpha(rv, 0.55));
        b.hline(
            g.left() + 4.0 * k,
            g.right() - 4.0 * k,
            cy,
            0.5 * k,
            with_alpha(rv, 0.8),
        );
        if pic.ref_at.is_finite() && pic.ref_len > 0.0 {
            let x = g.left() + 4.0 * k + span * (pic.ref_at / pic.ref_len).clamp(0.0, 1.0);
            b.vline(
                x,
                g.top() + 2.0 * k,
                g.bottom() - 2.0 * k,
                1.4 * k,
                art::c(pal::LAMP_HOT),
            );
        }
    } else {
        let t = Text::key("comparatio.ref.drop", Role::Micro, art::c(pal::SCREEN_DIM)).caps(false);
        put(b, k, g.center(), Align2::CENTER_CENTER, &t);
    }
    art::glass_sheen(b, s, g);
}

/// A numbered memory key: a small keycap, its number lit in the reference's voice when it
/// is the one compared.
fn memory_key(b: &mut Brush, s: &Style, r: Rect, digits: &str, lit: bool, hot: bool, voice: Color32) {
    let k = s.k;
    let f = art::console(s);
    b.rrect(
        r.translate(vec2(0.0, 2.0 * k)),
        4.0 * k,
        Fill::Solid(with_alpha(art::shadow(), 0.5)),
    );
    let cap = if lit { r.translate(vec2(0.0, 1.0 * k)) } else { r };
    b.rrect(cap, 4.0 * k, Fill::V(art::c(pal::KEY_TOP), art::c(pal::KEY_BODY)));
    let ink = if lit { voice } else { art::c(pal::KEY_INK) };
    let t = Text::new(digits, Role::Label, ink).caps(false);
    put(b, k, cap.center(), Align2::CENTER_CENTER, &t);
    if lit {
        b.rrect_soft(cap, 4.0 * k, Fill::Solid(with_alpha(voice, 0.20)), 5.0 * k);
    }
    if hot {
        b.rrect_stroke(r.expand(1.5 * k), 5.0 * k, 0.8 * k, art::c(f.edge));
    }
}

/// The big selector between its two positions, its lamps lit by what is heard (hidden in a
/// blind test: both dark, the pointer upright).
fn selector(
    b: &mut Brush,
    s: &Style,
    l: &Layout,
    pic: &Picture,
    hot: bool,
    mv: Color32,
    rv: Color32,
) {
    let k = s.k;
    let x = match pic.heard {
        Heard::Mix => 0.0,
        Heard::Reference => 1.0,
        Heard::Fading => pic.fade,
        Heard::Hidden => 0.5,
    };
    let angle = (-0.75 + 1.5 * x) * 0.9;
    let f = art::console(s);
    b.arc(
        l.selector,
        l.sel_rad * 1.36,
        -2.2,
        -0.94,
        1.0 * k,
        art::c(f.print),
        false,
    );
    let lamps = [
        (l.lamps[0], "comparatio.sel.mix", mv),
        (l.lamps[1], "comparatio.sel.reference", rv),
    ];
    for (i, (lp, key, voice)) in lamps.iter().enumerate() {
        let lit = if pic.heard == Heard::Hidden {
            0.0
        } else if i == 0 {
            1.0 - x
        } else {
            x
        };
        art::jewel(b, s, *lp, 6.0 * k, lit, *voice);
        let t = Text::key(key, Role::Label, Color32::TRANSPARENT).caps(true);
        let (al, dx) = if i == 0 {
            (Align2::RIGHT_CENTER, -14.0 * k)
        } else {
            (Align2::LEFT_CENTER, 14.0 * k)
        };
        art::print(b, s, *lp + vec2(dx, 0.0), al, &t, 1.0);
    }
    art::selector(b, s, l.selector, l.sel_rad, angle, hot);
}

/// The blind drawer: its title, the keycaps A (your mix), B (the reference), X (hidden),
/// the answers, a lamp per trial, the score and the chance of guessing it.
#[allow(clippy::too_many_arguments)]
fn drawer(
    b: &mut Brush,
    s: &Style,
    l: &Layout,
    spec: &EffectSpec,
    blind: &Blind,
    words: &Words,
    hot: Option<usize>,
    mv: Color32,
    rv: Color32,
) {
    let k = s.k;
    let f = art::console(s);
    b.rrect(
        l.drawer,
        6.0 * k,
        Fill::V(
            mix(art::c(f.foot), art::shadow(), 0.75),
            mix(art::c(f.foot), art::shadow(), 0.55),
        ),
    );
    b.rrect_stroke(l.drawer, 6.0 * k, 0.8 * k, art::c(f.edge));
    let title = Text::key("comparatio.blind.title", Role::Micro, art::c(pal::KEY_INK)).caps(true);
    put(
        b,
        k,
        pos2(l.drawer.left() + 14.0 * k, l.drawer.top() + 16.0 * k),
        Align2::LEFT_CENTER,
        &title,
    );
    let listen = spec.get("listen").round() as i32;
    let hot_lamp = art::c(pal::LAMP_HOT);
    for (i, letter) in ["A", "B", "X"].iter().enumerate() {
        let col = [mv, rv, hot_lamp][i];
        art::keycap(
            b,
            s,
            l.keys[i],
            letter,
            listen == i as i32,
            hot == Some(3 + i),
            col,
        );
        let cap = [
            "comparatio.blind.a",
            "comparatio.blind.b",
            "comparatio.blind.x",
        ][i];
        let t = Text::key(cap, Role::Micro, art::c(pal::KEY_INK)).caps(false);
        put(
            b,
            k,
            pos2(l.keys[i].center().x, l.keys[i].bottom() + 10.0 * k),
            Align2::CENTER_CENTER,
            &t,
        );
    }
    for (i, key) in ["comparatio.blind.x_is_a", "comparatio.blind.x_is_b"]
        .iter()
        .enumerate()
    {
        small_key(b, s, l.answers[i], key, hot == Some(6 + i));
    }
    small_key(b, s, l.restart, "comparatio.blind.restart", hot == Some(8));
    let y = l.drawer.bottom() - 36.0 * k;
    let mut x = l.answers[1].left() + 4.0 * k;
    for a in blind.answers.iter().rev().take(16).rev() {
        let col = art::c(if *a { pal::RIGHT } else { pal::WRONG });
        b.circle(pos2(x, y), 3.5 * k, Fill::Solid(col));
        x += 10.0 * k;
    }
    let t = Text::new(&words.score, Role::Micro, art::c(pal::KEY_INK)).caps(false);
    let sr = put(
        b,
        k,
        pos2(l.drawer.left() + 14.0 * k, l.drawer.bottom() - 16.0 * k),
        Align2::LEFT_CENTER,
        &t,
    );
    if !words.chance.is_empty() {
        let t = Text::new(&words.chance, Role::Micro, art::c(pal::SCREEN_DIM)).caps(false);
        put(
            b,
            k,
            pos2(sr.right() + 12.0 * k, sr.center().y),
            Align2::LEFT_CENTER,
            &t,
        );
    }
}

fn small_key(b: &mut Brush, s: &Style, r: Rect, key: &str, hot: bool) {
    let k = s.k;
    let f = art::console(s);
    b.rrect(
        r.translate(vec2(0.0, 1.5 * k)),
        4.0 * k,
        Fill::Solid(with_alpha(art::shadow(), 0.45)),
    );
    b.rrect(
        r,
        4.0 * k,
        Fill::V(art::c(pal::KEY_TOP), art::c(pal::KEY_BODY)),
    );
    let t = Text::key(key, Role::Micro, art::c(pal::KEY_INK)).caps(true);
    put(b, k, r.center(), Align2::CENTER_CENTER, &t);
    if hot {
        b.rrect_stroke(r.expand(1.5 * k), 5.0 * k, 0.8 * k, art::c(f.edge));
    }
}

/// The inspector's picture: what is heard and how it is matched ("Your mix · Integrated").
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let s = Style::current().function(frame::hue_of(spec.kind));
    let pick = |key: &str, n: f32| {
        crate::i18n::t(&format!(
            "fx.comparatio.{key}.{}",
            spec.get(key).round().clamp(0.0, n)
        ))
    };
    let w = format!("{} \u{b7} {}", pick("listen", 4.0), pick("match", 2.0));
    controls::readout(
        ui,
        &s,
        &w,
        false,
        frame::tr(spec.kind.name_key()),
        ui.available_width(),
    )
}

/// The settings `--editor comparatio` opens it with: its defaults (the reference is the
/// demo's, set by `demo_feed`).
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    let _ = (app, strip, effect);
}

/// What a playing engine would feed the editor, for screenshots with no engine: the real
/// unit given the demo's master as its reference, run over the demo's mix ([`demo`]); the
/// mix's middle fed to the editor's live curves as the tap feeds them. The list holds three
/// references, the demo's master compared.
pub fn demo_feed(app: &mut CantusApp) {
    let Some((strip, effect, _)) = app.fx.open else {
        return;
    };
    if super::modfx::open_spec(app).is_some_and(|s| s.ir.is_none()) {
        let list = vec![
            Reference::new(demo::PATH),
            Reference::new("references/Album master.wav"),
            Reference::new("references/Radio single.flac"),
        ];
        app.exec(cantus_core::Command::SetReferences {
            strip,
            effect,
            refs: Some(Box::new(References { list, eq: None })),
            selected: Some(Box::new(cantus_core::IrFile::new(demo::PATH))),
        });
    }
    let Some(spec) = super::modfx::open_spec(app) else {
        return;
    };
    let (tel, pic, mid) = demo::picture(&spec);
    app.fx.injected = Some(tel);
    if app.fx.comparatio.effect != Some(effect) {
        app.fx.comparatio = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    *app.fx.comparatio.pic = pic;
    let mut live = picture::Live::new(48_000.0);
    live.feed(&mid);
    app.fx.comparatio.live = Some(Box::new(live));
    app.fx.comparatio.made = None;
    // A blind test under way: six trials answered, five right.
    if spec.get("listen").round() as i32 == 2 {
        app.fx.comparatio.blind.answers = vec![true, true, false, true, true, true];
    }
}
