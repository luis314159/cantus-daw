//! Tabula's editor: the pedalboard (ADR 0131 §5) — native units as pedals on a dark board,
//! the macro rail along its top, the A/B split's splitter and mixer, the pedal picker, and
//! each pedal's own full editor opened from the board.
//!
//! The board is the window's display (`tabula_board`): every pedal its unit's face, the
//! cables carrying the signal as bright as it is at that point, in the signal's direction.

use super::controls::{self, Cell, Size};
use super::frame;
use super::host::{DawHost, FxHost};
use super::pedal_draw as pd;
use super::tabula_board::{self as tb, Item};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::shell::Grid;
use crate::kit::text::Src;
use crate::kit::{with_brush, Style};
use cantus_core::fx::tabula::{Path, PedalId, MACROS, MACRO_KEYS};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use cantus_engine::fx::tabula::tel;
use cantus_engine::Telemetry;
use egui::{pos2, vec2, Rect};

const KIND: EffectKind = EffectKind::Board;

/// Factory presets: the factory's boards (`cantus_core::fx::tabula_presets`), each as the
/// flat map a board preset is (its pedals, their places and values, its macros); their
/// styles and roles are data (`presets/effects/board.toml`, ADR 0101).
pub fn factory_presets() -> &'static [FactoryPreset] {
    static ALL: std::sync::OnceLock<&'static [FactoryPreset]> = std::sync::OnceLock::new();
    ALL.get_or_init(|| {
        let leak = |s: String| -> &'static str { Box::leak(s.into_boxed_str()) };
        let v: Vec<FactoryPreset> = cantus_core::fx::tabula_presets::factory()
            .into_iter()
            .map(|(key, spec)| {
                let values: Vec<(&'static str, f32)> =
                    cantus_core::fx::tabula_presets::to_flat(&spec)
                        .into_iter()
                        .map(|(k, v)| (leak(k), v))
                        .collect();
                (key, &*Box::leak(values.into_boxed_slice()))
            })
            .collect();
        Box::leak(v.into_boxed_slice())
    })
}

/// What a board preset of `spec` holds: the flat map (its pedals and their places too).
pub fn preset_params(spec: &EffectSpec) -> std::collections::BTreeMap<String, f32> {
    cantus_core::fx::tabula_presets::to_flat(spec)
}

/// A board preset's flat map loaded on a board: the whole board replaced in one step
/// (`Command::ReplaceEffect`), its macros' names in the interface's words. `false` when the
/// map holds no board (it then only sets values).
pub fn load_board(
    app: &mut CantusApp,
    strip: StripId,
    effect: EffectId,
    params: &std::collections::BTreeMap<String, f32>,
) -> bool {
    let Some(mut spec) = cantus_core::fx::tabula_presets::from_flat(params) else {
        return false;
    };
    if let Some(b) = spec.board.as_deref_mut() {
        for m in &mut b.macros {
            if let Some(n) = m.name.as_mut() {
                if n.starts_with("tabula.macro.") {
                    *n = crate::i18n::t(n);
                }
            }
        }
    }
    let Some(mut slot) = app.project().effect(strip, effect).cloned() else {
        return false;
    };
    slot.spec = spec;
    app.exec(cantus_core::Command::ReplaceEffect {
        strip,
        effect,
        slot: Box::new(slot),
    });
    true
}

/// The macro rail's height and the side column's width, at 100 %.
const RAIL_H: f32 = 118.0;
/// The bottom row's pedal card and levels card, at 100 %.
const OPTIONS_W: f32 = 318.0;
const LEVELS_W: f32 = 236.0;

/// The full editor of one pedal, opened from the board.
pub struct Open {
    pub pedal: PedalId,
    pub editor: Box<super::Editor>,
}

impl std::fmt::Debug for Open {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Open").field("pedal", &self.pedal).finish()
    }
}

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    /// The effect this state belongs to.
    effect: Option<EffectId>,
    /// The device frame: the preset, its lists.
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    /// The pedal whose full editor shows in place of the board.
    pub open: Option<Open>,
    /// The pedal the side column describes.
    pub selected: Option<PedalId>,
    /// The picker, open on this shelf.
    pub picker: Option<usize>,
    /// The picker's pedal under the pointer (the screenshots set it).
    pub picker_hover: Option<usize>,
    /// The streaks' clock for screenshots (`None`: the frame's time).
    pub anim_time: Option<f64>,
    /// The LEDs' glow, smoothed, by pedal.
    glow: Vec<(PedalId, f32)>,
    /// How far a board wider than its window is scrolled, 0 … 1.
    pub scroll: f32,
    /// The macro rail's learn mode.
    pub learn: bool,
    /// What the open pedal's editor draws, for screenshots with no engine.
    pub pedal_tel: Option<Telemetry>,
    /// The picker's shelves: (category, its pedal kinds), built once.
    shelves: Vec<(crate::picker_catalog::Category, Vec<EffectKind>)>,
    /// The macro the learn mode maps the next touched knob to.
    pub armed: Option<u8>,
    /// A pedal lifted off the board, and where the hand holds it.
    drag: Option<(PedalId, egui::Vec2)>,
    /// A pedal's knob being turned: the pedal, its key, its position and the pointer's
    /// height when the turn began.
    turning: Option<(PedalId, &'static str, f32, f32)>,
    /// The knob or footswitch under the pointer: (pedal, knob — 9 the footswitch).
    hot: Option<(PedalId, usize)>,
    /// The board has the keyboard (a click on it), and the chosen pedal's knob the arrows
    /// turn.
    focused: bool,
    knob: usize,
    /// Randomise's next seed.
    seed: u64,
}

/// Tabula's editor in Cantus's own window: the host is the project.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.tabula);
    if st.effect != Some(effect) {
        // Another board: its own state (a state made for no board yet keeps what was set
        // on it — the screenshots' choices).
        if st.effect.is_some() {
            st = State::default();
        }
        st.effect = Some(effect);
    }
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "tabula"), &mut st, ui);
    app.fx.tabula = st;
    out
}

/// Tabula's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
/// another DAW.
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let tel = host.telemetry();
    let sr = host.sample_rate();
    let mut chips = std::mem::take(&mut st.chips);
    let chip = [chips.latency(tel[tel::LATENCY], sr)];
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

fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &Telemetry,
) {
    let k = s.k;
    let mut g = Grid::new(body, s);
    if let Some(rail) = g.row(RAIL_H * k, &[1.0]).next() {
        rail_ui(host, st, ui, s, rail);
    }
    let rest = g.rest();
    let gap = s.d(4.0);
    if st.open.is_some() {
        pedal_editor_ui(host, st, ui, s, rest);
        return;
    }
    // The board across the window, over a row: the chosen pedal's controls, its options,
    // the board's own levels.
    let strip_h = (controls::card_height(Size::M) * k).max(150.0 * k);
    let board = Rect::from_min_max(rest.min, pos2(rest.right(), rest.bottom() - strip_h - gap));
    let row = Rect::from_min_max(pos2(rest.left(), board.bottom() + gap), rest.max);
    board_ui(host, st, ui, s, board, tel);
    let levels = Rect::from_min_max(pos2(row.right() - LEVELS_W * k, row.top()), row.max);
    let options = Rect::from_min_max(
        pos2(levels.left() - gap - OPTIONS_W * k, row.top()),
        pos2(levels.left() - gap, row.bottom()),
    );
    let strip = Rect::from_min_max(row.min, pos2(options.left() - gap, row.bottom()));
    edit_strip_ui(host, st, ui, s, strip);
    options_ui(host, st, ui, s, options);
    levels_ui(host, ui, s, levels);
}

/// The chosen pedal's controls under the board: its face's knobs and its other main
/// parameters as the kit's cells (each with its menu: automate, modulate, learn), and the
/// way to its full editor.
fn edit_strip_ui(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui, s: &Style, r: Rect) {
    let spec = host.spec().clone();
    let Some(p) = st
        .selected
        .or_else(|| spec.pedals().first().map(|p| p.id))
        .and_then(|id| spec.pedals().iter().find(|p| p.id == id))
    else {
        controls::card(ui, s, r, Src::Key("tabula.card.pedal"), None, |ui, _| {
            controls::caption(ui, s, Src::Key("tabula.pedal.none"), 360.0);
        });
        return;
    };
    let kind = p.spec.kind;
    // The face's knobs first, then the unit's other continuous parameters.
    let mut keys: Vec<&'static str> = Vec::new();
    if let Some(m) = tb::fragor_circuit(&p.spec) {
        keys.extend(cantus_core::fx::fragor::KNOBS[m]);
    } else if let Some(f) = super::tabula_faces::of(kind) {
        keys.extend(f.knobs.iter().copied());
    }
    let faced = keys.len();
    // r41:pedals — one of Fragor's circuits as a pedal of its own has no window of its own:
    // everything it has is here — its gate and levels, its quality and channels, and its
    // circuit (in, or a wire).
    let whole = kind.fragor_circuit();
    if let Some(c) = whole {
        keys.extend(["gate", "in", "mix", "out", "os", "mono"]);
        keys.push(cantus_core::fx::fragor_pedal::switch(c));
    }
    let editor = super::Editor::of(kind).is_some(); // r41:pedals
    for i in kind.params() {
        if keys.len() >= 7 {
            break;
        }
        if !i.stepped && !keys.contains(&i.key) && !i.key.contains('.') {
            keys.push(i.key);
        }
    }
    let (name, what) = tb::names_of(&p.spec);
    let title = format!("{} · {}", src_text(name), src_text(what));
    let id = p.id;
    let ptel = st.pedal_tel.unwrap_or([0.0; cantus_engine::TELEMETRY_LEN]);
    let mut open = false;
    controls::card(ui, s, r, Src::Str(&title), Some(p.on), |ui, cr| {
        let Some(mut ph) = super::tabula_host::PedalHost::new(host, id, ptel) else {
            return;
        };
        let fragor_words = tb::fragor_circuit(ph.spec()).map(super::fragor::knob_labels);
        // fx3:saltus — a parameter of its own whose name is longer than a cell takes a
        // wider cell, and one that no longer fits before the button is left to the full
        // editor: nothing elided.
        let room = cr.body.width() / s.k - if editor { 182.0 } else { 8.0 }; // r41:pedals
        let mut used = 0.0;
        for (j, key) in keys.iter().enumerate() {
            let Some(info) = kind.param(key) else {
                continue;
            };
            // fx3:arca — every cell as wide as the word it prints (its face's word or the
            // parameter's own, in capitals), never its short form.
            let circuit = whole.is_some() && key.ends_with(".on"); // r41:pedals
            let word = match fragor_words {
                _ if circuit => "fragor_pedal.circuit",
                Some(w) if j < 3 => w[j],
                _ if j < faced => tb::knob_word(key).unwrap_or(info.label_key),
                _ => info.label_key,
            };
            let t = crate::kit::text::Text::key(word, crate::kit::text::Role::Label, s.t.text)
                .caps(true);
            let w = crate::kit::text::measure(ui.ctx(), &t, s.k).x / s.k * 1.15 + 12.0;
            let width = w.max(78.0);
            if j >= faced && used + width > room {
                break;
            }
            used += width + 4.0; // fx3:saltus
                                 // The face's knobs in its words; the unit's other parameters in their own.
            let label = match fragor_words {
                _ if circuit => Src::Key("fragor_pedal.circuit"),
                Some(w) if j < 3 => Src::Key(w[j]),
                _ if j < faced => tb::knob_word(key).map_or(Src::Key(info.label_key), Src::Key),
                _ => Src::Key(info.label_key),
            };
            let out = Cell::param(info, ph.spec().get(key))
                .size(Size::M)
                .width(width) // fx3:saltus
                .label(label)
                .show_in(ui, s, &mut ph, key);
            if let Some(v) = out.changed {
                ph.set(key, v, Some(key));
            }
            if out.done {
                ph.end_gesture();
            }
        }
        ui.add_space(s.d(12.0));
        // r41:pedals — offered only for a pedal with a window of its own.
        if editor && controls::key(ui, s, Src::Key("tabula.pedal.open"), "", 170.0, false).clicked()
        {
            open = true;
        }
    });
    if open {
        open_pedal(st, &spec, id);
    }
}

/// The macro rail: eight knobs along the board's top, each named for what it moves.
fn rail_ui(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui, s: &Style, r: Rect) {
    let spec = host.spec().clone();
    let board = cantus_core::fx::tabula::board(&spec);
    let names: Vec<String> = (0..MACROS)
        .map(|i| {
            board
                .macro_at(i)
                .and_then(|m| m.name.clone())
                .unwrap_or_else(|| crate::i18n::t(&format!("fx.tabula.m{}", i + 1)))
        })
        .collect();
    let learn = st.learn;
    let armed = st.armed;
    let pick = crate::i18n::t("tabula.learn.pick");
    let mut arm = None;
    let mut randomise = false;
    let mut toggle = false;
    controls::card(ui, s, r, Src::Key("tabula.card.macros"), None, |ui, _| {
        for (i, key) in MACRO_KEYS.iter().enumerate() {
            let Some(info) = KIND.param(key) else {
                continue;
            };
            // Armed, the macro asks for the knob it is to move.
            let label = if armed == Some(i as u8) {
                Src::Str(&pick)
            } else {
                Src::Str(&names[i])
            };
            let c = Cell::param(info, spec.get(key))
                .size(Size::S)
                .width(86.0)
                .label(label);
            let out = c.show_in(ui, s, host, key);
            if learn && (out.resp.clicked() || out.resp.drag_started()) {
                arm = Some(i as u8);
            }
            if let Some(v) = out.changed {
                host.set(key, v, Some(key));
            }
            if out.done {
                host.end_gesture();
            }
        }
        ui.add_space(s.d(10.0));
        controls::column(ui, s, 120.0, |ui| {
            toggle = controls::key(ui, s, Src::Key("tabula.learn"), "", 116.0, learn).clicked();
            randomise =
                controls::key(ui, s, Src::Key("tabula.randomise"), "", 116.0, false).clicked();
        });
    });
    if toggle {
        st.learn = !st.learn;
        st.armed = None;
    }
    if let Some(m) = arm {
        st.armed = Some(m);
        host.status(crate::i18n::t("tabula.learn.status"));
    }
    if randomise {
        st.seed = st.seed.wrapping_add(1).wrapping_mul(0x9E37_79B9) ^ host.revision();
        super::tabula_edit::randomise(host, st.seed);
    }
}

/// The board drawn in the dark theme's tokens: it is the window's display, dark in every
/// theme (the owner's rule for the central displays).
fn board_style(s: &Style) -> Style {
    if s.t.dark {
        return *s;
    }
    let mut t = crate::kit::tokens::Tokens::slate();
    t.text_scale = s.t.text_scale;
    t.density = s.t.density;
    Style { t, ..*s }
}

/// The pedal's entry in the board's telemetry: its input and output peaks.
fn peaks_of(t: &Telemetry, id: PedalId) -> Option<(f32, f32)> {
    let n = (t[tel::COUNT] as usize).min(16);
    (0..n).find_map(|i| {
        let at = tel::PEDALS + i * tel::STRIDE;
        (t[at] == f32::from(id.0) + 1.0).then(|| (t[at + 1], t[at + 2]))
    })
}

/// The picker's shelves: every pedal-allowed unit of Cantus's, on its primary shelf, in the
/// effect shelves' order.
fn shelves() -> Vec<(crate::picker_catalog::Category, Vec<EffectKind>)> {
    use crate::picker::PickChoice;
    use crate::picker_catalog::Category;
    // r41:contract — the board-only pedals too (ADR 0152).
    let devices = crate::picker_catalog::board_devices();
    let mut out: Vec<(Category, Vec<EffectKind>)> = Vec::new();
    for c in Category::EFFECTS {
        let kinds: Vec<EffectKind> = devices
            .iter()
            .filter(|d| d.primary() == c)
            .filter_map(|d| match d.choice {
                PickChoice::Effect(k) if cantus_core::fx::tabula::pedal_allowed(k) => Some(k),
                _ => None,
            })
            .collect();
        if !kinds.is_empty() {
            out.push((c, kinds));
        }
    }
    out
}

/// Where shelf `c` is in the picker's list (0 when it has no pedal).
pub fn picker_shelf(c: crate::picker_catalog::Category) -> usize {
    shelves().iter().position(|(x, _)| *x == c).unwrap_or(0)
}

fn board_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    r: Rect,
    tel: &Telemetry,
) {
    let k = s.k;
    let sd = board_style(s);
    let spec = host.spec().clone();
    let pedals = spec.pedals();
    let items: Vec<Item> = pedals
        .iter()
        .map(|p| Item {
            id: p.id,
            path: p.path,
            size: pd::size_of(tb::shape_of(p)),
        })
        .collect();
    let geo = tb::layout(r, k, &items, st.scroll);
    // The LEDs follow each pedal's output, smoothed (a lamp, not a meter).
    for p in pedals {
        let target = if p.on {
            peaks_of(tel, p.id).map_or(0.0, |(_, o)| tb::level(o))
        } else {
            0.0
        };
        match st.glow.iter_mut().find(|(id, _)| *id == p.id) {
            Some((_, g)) => *g += (target - *g) * 0.35,
            None => st.glow.push((p.id, target)),
        }
    }
    let glow = |id: PedalId| st.glow.iter().find(|(i, _)| *i == id).map_or(0.0, |g| g.1);
    if st.picker.is_some() && st.shelves.is_empty() {
        st.shelves = shelves();
    }
    let time = st.anim_time.unwrap_or_else(|| ui.input(|i| i.time));
    let selected = st.selected.or_else(|| pedals.first().map(|p| p.id));
    let mut flowing = false;
    let mut pedal_hits: Vec<(PedalId, pd::Hits)> = Vec::with_capacity(pedals.len());
    with_brush(&ui.painter_at(r), |b| {
        pd::board(b, &sd, r, true);
        tb::nameplate(b, &sd, r);
        let ink = pd::board_ink(&sd);
        pd::board_jack(b, &sd, geo.jack_in, Src::Key("tabula.jack.in"), ink);
        pd::board_jack(b, &sd, geo.jack_out, Src::Key("tabula.jack.out"), ink);
        let hot = st.hot;
        let lifted = st.drag.map(|d| d.0);
        let b_ = cantus_core::fx::tabula::board(&spec);
        for (id, rect) in &geo.pedals {
            let Some(p) = pedals.iter().find(|p| p.id == *id) else {
                continue;
            };
            if lifted == Some(*id) {
                // Lifted off the board: its place stays empty until it lands.
                pd::empty_slot(
                    b,
                    &sd,
                    rect.shrink2(vec2(6.0 * k, 0.0)),
                    Src::Str(""),
                    false,
                );
                pedal_hits.push((
                    *id,
                    pd::Hits {
                        body: *rect,
                        knobs: [Rect::NOTHING; pd::MAX_KNOBS],
                        n: 0,
                        switch: Rect::NOTHING,
                        jack_in: rect.left_top() + vec2(rect.width() * 0.25, 0.0),
                        jack_out: rect.right_top() - vec2(rect.width() * 0.25, 0.0),
                    },
                ));
                continue;
            }
            let h = tb::pedal(
                b,
                &sd,
                *rect,
                &tb::Look {
                    spec: &p.spec,
                    shape: tb::shape_of(p),
                    on: p.on,
                    glow: glow(*id),
                    lifted: false,
                    hot: hot.filter(|h| h.0 == *id).map(|h| h.1),
                    selected: selected == Some(*id),
                    bare: false,
                },
            );
            // A knob a macro moves wears its ring (ADR 0109's driven mark).
            let u = rect.width() / pd::size_of(tb::shape_of(p)).x;
            for (j, key) in tb::knob_keys(&p.spec).iter().enumerate().take(h.n) {
                if b_.macro_of(*id, key).is_some() {
                    let c = h.knobs[j].center() - vec2(0.0, 7.0 * u);
                    let kr = h.knobs[j].width() / 2.7;
                    b.circle_stroke(c, kr + 4.0 * u, 1.6 * k, sd.t.hue_mod);
                }
            }
            pedal_hits.push((*id, h));
        }
        let split = geo
            .splitter
            .map(|r| tb::split_box(b, &sd, r, Src::Key("tabula.split"), None, false));
        let mixer = geo.mixer.map(|r| {
            tb::split_box(
                b,
                &sd,
                r,
                Src::Key("tabula.mixer"),
                Some(spec.get("ab")),
                false,
            )
        });
        for (lane, path) in [(geo.lane_a, Path::A), (geo.lane_b, Path::B)] {
            if let Some(l) = lane.filter(|_| !pedals.iter().any(|p| p.path == path)) {
                pd::empty_slot(b, &sd, l, Src::Key("tabula.lane.empty"), false);
            }
        }
        pd::empty_slot(
            b,
            &sd,
            geo.add.shrink2(vec2(6.0 * k, 0.0)),
            Src::Key("tabula.add"),
            st.picker.is_some(),
        );
        // The cables: the track's jack → each pedal in turn → the output, the signal
        // flowing in them as loud as it is at that point.
        let lift = 30.0 * geo.scale;
        let hit = |id: PedalId| pedal_hits.iter().find(|(i, _)| *i == id).map(|h| h.1);
        let geo_ = &geo;
        let mut cab = |b: &mut crate::kit::Brush, a: tb::End, z: tb::End, l: f32| {
            flowing |= l > 0.01;
            let energy = Some(pd::Energy { level: l, time });
            match (a, z) {
                // A wrapped chain: the cable runs between the rows.
                (tb::End::Top(p), tb::End::Top(q))
                    if tb::row_of(geo_, p) != tb::row_of(geo_, q) =>
                {
                    tb::cable_wrapped(b, &sd, geo_, p, q, lift, energy)
                }
                _ => tb::cable(b, &sd, a, z, lift, energy),
            }
        };
        // Runs one lane's pedals from `from` at `level`: where it ends and how loud.
        let run = |b: &mut crate::kit::Brush,
                   cab: &mut dyn FnMut(&mut crate::kit::Brush, tb::End, tb::End, f32),
                   path: Path,
                   mut from: tb::End,
                   mut level: f32| {
            for p in pedals.iter().filter(|p| p.path == path) {
                let Some(h) = hit(p.id) else { continue };
                cab(b, from, tb::End::Top(h.jack_in), level);
                if p.on {
                    if let Some((_, o)) = peaks_of(tel, p.id) {
                        level = tb::level(o);
                    }
                }
                from = tb::End::Top(h.jack_out);
            }
            (from, level)
        };
        let (mut from, mut level) = run(
            b,
            &mut cab,
            Path::Pre,
            tb::End::Top(geo.jack_in),
            tb::level(tel[tel::IN]),
        );
        if let (Some(sj), Some(mj)) = (split, mixer) {
            cab(b, from, tb::End::Top(sj.main), level);
            let (fa, la) = run(b, &mut cab, Path::A, tb::End::Right(sj.a), level);
            cab(b, fa, tb::End::Left(mj.a), la);
            let (fb, lb) = run(b, &mut cab, Path::B, tb::End::Right(sj.b), level);
            cab(b, fb, tb::End::Left(mj.b), lb);
            from = tb::End::Top(mj.main);
            level = la.max(lb);
        }
        let (from, _) = run(b, &mut cab, Path::Post, from, level);
        cab(
            b,
            from,
            tb::End::Top(geo.jack_out),
            tb::level(tel[tel::OUT]),
        );
    });
    // The picker, over the board, by the place to add: drawn after the board (its words
    // too), so nothing of the board shows through.
    let mut picker_hits = None;
    if let Some(shelf) = st.picker {
        let size = vec2(720.0, 452.0) * k;
        let w = size.x.min(r.width() - 24.0 * k);
        let h = size.y.min(r.height() - 24.0 * k);
        let panel = Rect::from_min_size(
            pos2(
                (geo.add.center().x - w * 0.75)
                    .clamp(r.left() + 12.0 * k, r.right() - 12.0 * k - w),
                r.top() + 12.0 * k,
            ),
            vec2(w, h),
        );
        let names: Vec<(Src<'_>, usize)> = st
            .shelves
            .iter()
            .map(|(c, ks)| (Src::Key(c.key()), ks.len()))
            .collect();
        let kinds = st
            .shelves
            .get(shelf)
            .map_or(&[][..], |(_, ks)| ks.as_slice());
        picker_hits = Some(with_brush(&ui.painter_at(r), |b| {
            tb::picker(
                b,
                s,
                panel,
                Src::Key("tabula.picker.title"),
                &names,
                shelf,
                kinds,
                st.picker_hover,
            )
        }));
    }
    if (flowing || st.drag.is_some()) && st.anim_time.is_none() {
        ui.ctx().request_repaint();
    }
    // The pedal in the hand, over everything, and where it would land.
    if let (Some((id, grab)), Some(p)) = (st.drag, ui.ctx().pointer_latest_pos()) {
        if let Some(pedal) = pedals.iter().find(|x| x.id == id) {
            let size = pd::size_of(tb::shape_of(pedal)) * geo.scale;
            let band = geo.rows.get(tb::row_of(&geo, p)).copied().unwrap_or(r);
            with_brush(&ui.painter_at(r), |b| {
                pd::drop_marker(b, &sd, p.x, band.top(), band.bottom());
                tb::pedal(
                    b,
                    &sd,
                    Rect::from_min_size(p - grab, size),
                    &tb::Look {
                        spec: &pedal.spec,
                        shape: tb::shape_of(pedal),
                        on: pedal.on,
                        glow: glow(id),
                        lifted: true,
                        hot: None,
                        selected: true,
                        bare: false,
                    },
                );
            });
        }
    }
    interact(
        host,
        st,
        ui,
        r,
        &spec,
        &geo,
        &pedal_hits,
        picker_hits.as_ref(),
    );
}

/// What the board does with the pointer and the keyboard: choose a pedal, turn its knobs,
/// its footswitch, lift it and drop it elsewhere (or onto a lane), open its full editor, map
/// a knob to the armed macro, add a pedal from the picker — each one step of undo.
#[allow(clippy::too_many_arguments)]
fn interact(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    r: Rect,
    spec: &EffectSpec,
    geo: &tb::Geo,
    hits: &[(PedalId, pd::Hits)],
    picker: Option<&tb::PickerHits>,
) {
    let resp = ui.interact(
        r,
        ui.id().with("tabula-board"),
        egui::Sense::click_and_drag(),
    );
    let pedals = spec.pedals();
    let knob_at = |p: egui::Pos2| -> Option<(PedalId, usize)> {
        hits.iter().find_map(|(id, h)| {
            if h.switch.contains(p) {
                return Some((*id, 9));
            }
            (0..h.n).find(|j| h.knobs[*j].contains(p)).map(|j| (*id, j))
        })
    };
    let body_at = |p: egui::Pos2| hits.iter().find(|(_, h)| h.body.contains(p)).map(|x| x.0);
    let over_picker = |p: egui::Pos2| picker.is_some_and(|h| h.panel.contains(p));
    st.hot = resp
        .hover_pos()
        .filter(|p| !over_picker(*p))
        .and_then(knob_at);
    if resp.drag_started() {
        if let Some(p) = resp.interact_pointer_pos().filter(|p| !over_picker(*p)) {
            match knob_at(p) {
                Some((id, j)) if j < 9 => {
                    let ped = pedals.iter().find(|x| x.id == id);
                    let key = ped.and_then(|x| tb::knob_keys(&x.spec).get(j).copied());
                    if let (Some(ped), Some(key)) = (ped, key) {
                        st.turning = Some((id, key, tb::norm(&ped.spec, key), p.y));
                        st.selected = Some(id);
                    }
                }
                Some(_) => {}
                None => {
                    if let Some(id) = body_at(p) {
                        let rect = hits.iter().find(|x| x.0 == id).map(|x| x.1.body);
                        if let Some(rect) = rect {
                            st.drag = Some((id, p - rect.min));
                            st.selected = Some(id);
                        }
                    }
                }
            }
        }
    }
    if resp.dragged() {
        if let (Some((id, key, from, y0)), Some(p)) = (st.turning, resp.interact_pointer_pos()) {
            let fine = if ui.input(|i| i.modifiers.shift) {
                0.2
            } else {
                1.0
            };
            let x = from + (y0 - p.y) / 220.0 * fine;
            let pk = cantus_core::fx::tabula::pedal_key(id, key);
            if let Some(info) = spec.param(&pk) {
                host.set(&pk, tb::denorm(info, x), Some(&pk));
            }
        }
    }
    if resp.drag_stopped() {
        if st.turning.take().is_some() {
            host.end_gesture();
        }
        if let (Some((id, _)), Some(p)) = (st.drag.take(), resp.interact_pointer_pos()) {
            if r.contains(p) {
                let (index, path) = drop_target(geo, spec, id, p);
                let now = pedals.iter().position(|x| x.id == id);
                let same = now == Some(index)
                    && pedals.iter().find(|x| x.id == id).map(|x| x.path) == Some(path);
                if !same {
                    super::tabula_edit::move_to(host, id, index, path);
                }
            }
        }
    }
    if resp.clicked() {
        st.focused = true;
        if let Some(p) = resp.interact_pointer_pos() {
            if let Some(h) = picker.filter(|h| h.panel.contains(p)) {
                if let Some(i) = h.shelves.iter().position(|x| x.contains(p)) {
                    st.picker = Some(i);
                } else if let Some((kind, _)) = h.pedals.iter().find(|x| x.1.contains(p)) {
                    if let Some(id) = super::tabula_edit::add(host, *kind) {
                        st.selected = Some(id);
                    }
                    st.picker = None;
                }
                return;
            }
            st.picker = None;
            match knob_at(p) {
                Some((id, 9)) => {
                    if let Some(ped) = pedals.iter().find(|x| x.id == id) {
                        super::tabula_edit::footswitch(host, id, !ped.on);
                    }
                }
                Some((id, j)) => {
                    st.selected = Some(id);
                    let key = pedals
                        .iter()
                        .find(|x| x.id == id)
                        .and_then(|x| tb::knob_keys(&x.spec).get(j).copied());
                    if let (Some(m), Some(key)) = (st.armed, key) {
                        super::tabula_edit::learn(host, m, id, key);
                        st.armed = None;
                    }
                    st.knob = j;
                }
                None => {
                    if let Some(id) = body_at(p) {
                        st.selected = Some(id);
                    } else if geo.add.contains(p) {
                        if st.shelves.is_empty() {
                            st.shelves = shelves();
                        }
                        st.picker = Some(0);
                    }
                }
            }
        }
    }
    if resp.double_clicked() {
        if let Some(id) = resp.interact_pointer_pos().and_then(body_at) {
            open_pedal(st, spec, id);
        }
    }
    if resp.clicked_elsewhere() {
        st.focused = false;
    }
    if st.focused {
        keys(host, st, ui, spec);
    }
    // The screen reader's board: a list of pedals, each a toggle and its knobs sliders.
    for (id, h) in hits {
        let Some(ped) = pedals.iter().find(|x| x.id == *id) else {
            continue;
        };
        let name = src_text(tb::names_of(&ped.spec).0);
        let a = ui.interact(
            h.body,
            ui.id().with(("tabula-pedal", id.0)),
            egui::Sense::hover(),
        );
        a.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, ped.on, &name)
        });
        for (j, key) in tb::knob_keys(&ped.spec).iter().enumerate().take(h.n) {
            let kr = ui.interact(
                h.knobs[j],
                ui.id().with(("tabula-knob", id.0, j)),
                egui::Sense::hover(),
            );
            let v = f64::from(ped.spec.get(key));
            let label = format!("{name}: {}", src_text(knob_label(&ped.spec, j, key)));
            kr.widget_info(|| egui::WidgetInfo::slider(true, v, &label));
        }
    }
}

/// The word under a pedal's knob `j` (`key`).
fn knob_label(spec: &EffectSpec, j: usize, key: &str) -> Src<'static> {
    if let Some(m) = tb::fragor_circuit(spec) {
        return Src::Key(super::fragor::knob_labels(m)[j.min(2)]);
    }
    match tb::knob_word(key) {
        Some(w) => Src::Key(w),
        None => Src::Key(spec.kind.param(key).map_or("", |i| i.label_key)),
    }
}

/// Where a pedal dropped at `p` lands: its index in the signal order (the others' order
/// kept) and its lane.
fn drop_target(geo: &tb::Geo, spec: &EffectSpec, id: PedalId, p: egui::Pos2) -> (usize, Path) {
    let pedals = spec.pedals();
    let me = pedals
        .iter()
        .find(|x| x.id == id)
        .map_or(Path::Pre, |x| x.path);
    let near = |r: Option<Rect>| r.is_some_and(|r| r.expand(16.0).contains(p));
    let key = |q: egui::Pos2| (tb::row_of(geo, q), q.x);
    let path = if near(geo.lane_a) {
        Path::A
    } else if near(geo.lane_b) {
        Path::B
    } else if let Some(sp) = geo.splitter {
        if key(p) < key(sp.center()) {
            Path::Pre
        } else {
            Path::Post
        }
    } else if me == Path::Post {
        Path::Post
    } else {
        Path::Pre
    };
    let rect_of = |pid: PedalId| geo.pedals.iter().find(|x| x.0 == pid).map(|x| x.1);
    // The others in order, as the board will hold them once the pedal is lifted out.
    let others: Vec<(usize, PedalId, Path)> = pedals
        .iter()
        .filter(|x| x.id != id)
        .enumerate()
        .map(|(i, x)| (i, x.id, x.path))
        .collect();
    let on: Vec<&(usize, PedalId, Path)> = others.iter().filter(|x| x.2 == path).collect();
    let before = on
        .iter()
        .filter(|x| rect_of(x.1).is_some_and(|r| key(r.center()) < key(p)))
        .count();
    let index = match (on.get(before), on.last()) {
        (Some(x), _) => x.0,
        (None, Some(last)) => last.0 + 1,
        (None, None) if path == Path::Pre => 0,
        (None, None) => others.len(),
    };
    (index, path)
}

/// The keyboard on a board that has it: Tab and Shift+Tab choose the next pedal, Space its
/// footswitch, Enter its full editor, Delete takes it off, Ctrl+Left and Right move it,
/// Left and Right choose its knob and Up and Down turn it (Shift: finely).
fn keys(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui, spec: &EffectSpec) {
    use egui::{Key, Modifiers};
    let pedals = spec.pedals();
    if pedals.is_empty() {
        return;
    }
    let at = st
        .selected
        .and_then(|id| pedals.iter().position(|x| x.id == id))
        .unwrap_or(0);
    let p = &pedals[at];
    let consume = |ui: &mut egui::Ui, m: Modifiers, k: Key| ui.input_mut(|i| i.consume_key(m, k));
    if consume(ui, Modifiers::SHIFT, Key::Tab) {
        st.selected = Some(pedals[(at + pedals.len() - 1) % pedals.len()].id);
        st.knob = 0;
    } else if consume(ui, Modifiers::NONE, Key::Tab) {
        st.selected = Some(pedals[(at + 1) % pedals.len()].id);
        st.knob = 0;
    } else if consume(ui, Modifiers::NONE, Key::Space) {
        super::tabula_edit::footswitch(host, p.id, !p.on);
    } else if consume(ui, Modifiers::NONE, Key::Enter) {
        open_pedal(st, spec, p.id);
    } else if consume(ui, Modifiers::NONE, Key::Delete) {
        super::tabula_edit::remove(host, p.id);
        st.selected = None;
    } else if at > 0 && consume(ui, Modifiers::COMMAND, Key::ArrowLeft) {
        super::tabula_edit::move_to(host, p.id, at - 1, p.path);
    } else if at + 1 < pedals.len() && consume(ui, Modifiers::COMMAND, Key::ArrowRight) {
        super::tabula_edit::move_to(host, p.id, at + 1, p.path);
    } else if consume(ui, Modifiers::NONE, Key::ArrowLeft) {
        st.knob = st.knob.saturating_sub(1);
    } else if consume(ui, Modifiers::NONE, Key::ArrowRight) {
        st.knob = (st.knob + 1).min(tb::knob_keys(&p.spec).len().saturating_sub(1));
    } else {
        let fine = ui.input(|i| i.modifiers.shift);
        let m = if fine {
            Modifiers::SHIFT
        } else {
            Modifiers::NONE
        };
        let up = consume(ui, m, Key::ArrowUp);
        let down = !up && consume(ui, m, Key::ArrowDown);
        if up || down {
            let step = if fine { 0.005 } else { 0.02 } * if up { 1.0 } else { -1.0 };
            if let Some(key) = tb::knob_keys(&p.spec).get(st.knob) {
                let pk = cantus_core::fx::tabula::pedal_key(p.id, key);
                if let Some(info) = spec.param(&pk) {
                    host.set(&pk, tb::denorm(info, tb::norm(&p.spec, key) + step), None);
                    host.end_gesture();
                }
            }
        }
    }
}

/// Open a pedal's full editor in place of the board.
pub fn open_pedal(st: &mut State, spec: &EffectSpec, id: PedalId) {
    let Some(p) = spec.pedals().iter().find(|p| p.id == id) else {
        return;
    };
    if let Some(editor) = super::Editor::of(p.spec.kind) {
        st.open = Some(Open {
            pedal: id,
            editor: Box::new(editor),
        });
        st.selected = Some(id);
    }
}

/// A pedal's own full editor, where the board was: a bar to go back over it.
fn pedal_editor_ui(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui, s: &Style, r: Rect) {
    let k = s.k;
    let Some(open) = st.open.as_mut() else {
        return;
    };
    let id = open.pedal;
    let bar = Rect::from_min_size(r.min, vec2(r.width(), 30.0 * k));
    let body = Rect::from_min_max(pos2(r.left(), bar.bottom() + s.d(4.0)), r.max);
    let mut back = false;
    let spec = host.spec().clone();
    let kind = spec
        .pedals()
        .iter()
        .find(|p| p.id == id)
        .map(|p| p.spec.kind);
    let mut row = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(bar)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    if controls::key(&mut row, s, Src::Key("tabula.back"), "", 170.0, false).clicked() {
        back = true;
    }
    if let Some(kind) = kind {
        let p = spec.pedals().iter().find(|p| p.id == id);
        let (name, _) = p.map_or((Src::Key(kind.name_key()), Src::Str("")), |p| {
            tb::names_of(&p.spec)
        });
        let name = src_text(name);
        let words = crate::i18n::tf("tabula.editing", &[("name", &name)]);
        controls::caption(&mut row, s, Src::Str(&words), 420.0);
    }
    let ptel = st.pedal_tel.unwrap_or([0.0; cantus_engine::TELEMETRY_LEN]);
    match super::tabula_host::PedalHost::new(host, id, ptel) {
        Some(mut ph) => {
            let mut c = ui.new_child(egui::UiBuilder::new().max_rect(body));
            open.editor.show(&mut ph, &mut c);
        }
        None => back = true,
    }
    if back {
        st.open = None;
    }
}

/// The board's own levels: what goes in, the dry/wet, what comes out.
fn levels_ui(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, r: Rect) {
    controls::card(ui, s, r, Src::Key("tabula.card.board"), None, |ui, _| {
        for key in ["in", "mix", "out"] {
            let Some(info) = KIND.param(key) else {
                continue;
            };
            let out = Cell::param(info, host.spec().get(key))
                .size(Size::M)
                .width(70.0)
                .show_in(ui, s, host, key);
            if let Some(v) = out.changed {
                host.set(key, v, Some(key));
            }
            if out.done {
                host.end_gesture();
            }
        }
    });
}

/// The chosen pedal's place on the board: its size, its lane, its footswitch, and taking
/// it off.
fn options_ui(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui, s: &Style, r: Rect) {
    let spec = host.spec().clone();
    let chosen = chosen(st, &spec).cloned();
    controls::card(ui, s, r, Src::Key("tabula.card.place"), None, |ui, _| {
        let w = OPTIONS_W - 24.0;
        controls::column(ui, s, w, |ui| {
            let Some(p) = &chosen else {
                controls::caption(ui, s, Src::Key("tabula.pedal.none"), w);
                return;
            };
            controls::caption(ui, s, Src::Key("tabula.pedal.size"), w);
            let shapes = [
                Src::Key("tabula.shape.mini"),
                Src::Key("tabula.shape.standard"),
                Src::Key("tabula.shape.wide"),
                Src::Key("tabula.shape.double"),
                Src::Key("tabula.shape.round"),
            ];
            let now = tb::shape_of(p) as usize;
            if let Some(i) = controls::segmented(ui, s, now, &shapes, "", w) {
                use cantus_core::fx::tabula::PedalShape as S;
                let shape = [S::Mini, S::Standard, S::Wide, S::Double, S::Round][i];
                // The unit's own shape is no choice: it goes back to `None`.
                let natural = super::tabula_faces::of(p.spec.kind).map(|f| f.shape);
                let shape = (Some(shape) != natural).then_some(shape);
                super::tabula_edit::shape(host, p.id, shape);
            }
            controls::caption(ui, s, Src::Key("tabula.pedal.lane"), w);
            let lanes = [
                Src::Key("tabula.path.pre"),
                Src::Key("tabula.lane.a"),
                Src::Key("tabula.lane.b"),
                Src::Key("tabula.path.post"),
            ];
            let paths = [Path::Pre, Path::A, Path::B, Path::Post];
            let at = paths.iter().position(|x| *x == p.path).unwrap_or(0);
            if let Some(i) = controls::segmented(ui, s, at, &lanes, "", w) {
                let index = spec.pedals().iter().position(|x| x.id == p.id).unwrap_or(0);
                super::tabula_edit::move_to(host, p.id, index, paths[i]);
            }
            ui.horizontal(|ui| {
                let half = (w - 4.0) * 0.5;
                if controls::led(ui, s, p.on, Src::Key("tabula.pedal.on"), "", half).clicked() {
                    super::tabula_edit::footswitch(host, p.id, !p.on);
                }
                if controls::key(ui, s, Src::Key("tabula.pedal.remove"), "", half, false).clicked()
                {
                    super::tabula_edit::remove(host, p.id);
                    st.selected = None;
                }
            });
        });
    });
}

/// The pedal the window describes: the chosen one, else the first.
fn chosen<'a>(st: &State, spec: &'a EffectSpec) -> Option<&'a cantus_core::fx::tabula::Pedal> {
    st.selected
        .and_then(|id| spec.pedals().iter().find(|p| p.id == id))
        .or_else(|| spec.pedals().first())
}

/// The words of a source, translated.
fn src_text(s: Src<'_>) -> String {
    match s {
        Src::Key(k) => crate::i18n::t(k),
        Src::Str(s) => s.to_string(),
    }
}

/// The inspector's picture: the board in small — each pedal a case in its own paint, its
/// lamp lit when it is on.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let s = Style::current().function(frame::hue_of(spec.kind));
    let w = ui.available_width();
    let h = 46.0 * s.k;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), egui::Sense::hover());
    let pedals = spec.pedals();
    with_brush(&ui.painter_at(rect), |b| {
        pd::board(b, &board_style(&s), rect, true);
        let n = pedals.len().max(1) as f32;
        let gap = 6.0 * s.k;
        let pw = ((rect.width() - 16.0 * s.k - gap * (n - 1.0)) / n).min(30.0 * s.k);
        let mut x = rect.left() + 8.0 * s.k;
        for p in pedals {
            let f = tb::finish_of(p.spec.kind, &p.spec);
            let r = Rect::from_min_size(pos2(x, rect.top() + 7.0 * s.k), vec2(pw, h - 14.0 * s.k));
            b.rrect(r, 3.0 * s.k, crate::kit::Fill::Solid(f.body.rgb()));
            let led = if p.on { f.led.rgb() } else { f.cap.rgb() };
            b.circle(
                pos2(r.center().x, r.top() + 6.0 * s.k),
                2.2 * s.k,
                crate::kit::Fill::Solid(led),
            );
            x += pw + gap;
        }
    });
    resp
}

/// The settings `--editor tabula` and the screenshot tool open it with: the guitar rig (the
/// starting preset) with its pedals at mixed sizes — as a player sets them — and three
/// macros named and mapped.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    use cantus_core::fx::tabula::{MacroMap, PedalShape};
    use cantus_core::Command as C;
    let shapes = [
        (1, PedalShape::Mini),
        (2, PedalShape::Wide),
        (3, PedalShape::Double),
    ];
    for (id, shape) in shapes {
        app.exec(C::SetPedalShape {
            strip,
            effect,
            pedal: PedalId(id),
            shape: Some(shape),
        });
    }
    let map = |pedal: u16, key: &str, from: f32, to: f32| MacroMap {
        pedal: PedalId(pedal),
        key: key.into(),
        from,
        to,
    };
    let macros = [
        (
            "tabula.demo.drive",
            vec![map(0, "od.drive", 0.1, 0.8), map(0, "od.level", 0.6, 0.45)],
        ),
        (
            "tabula.demo.echo",
            vec![map(2, "mix", 0.0, 0.45), map(2, "feedback", 0.15, 0.6)],
        ),
        ("tabula.demo.space", vec![map(3, "mix", 0.05, 0.5)]),
    ];
    for (i, (name, maps)) in macros.into_iter().enumerate() {
        app.exec(C::SetMacroName {
            strip,
            effect,
            macro_: i as u8,
            name: Some(crate::i18n::t(name)),
        });
        app.exec(C::SetMacroMaps {
            strip,
            effect,
            macro_: i as u8,
            maps,
        });
    }
    DawHost::new(app, strip, effect, "tabula")
        .set_many(&[("m1", 0.3), ("m2", 0.4), ("m3", 0.3)], "cmd.fx_preset");
}

/// What a playing engine would report, for screenshots with no engine: a plucked chord at a
/// guitar's level through the board as set.
pub fn demo_feed(app: &mut CantusApp) {
    demo_feed_at(app, 0.4);
}

/// [`demo_feed`] with a pluck of peak `amp` (0: silence — the board at rest); the open
/// pedal's own editor is fed too.
pub fn demo_feed_at(app: &mut CantusApp, amp: f32) {
    let Some((strip, effect, _)) = app.fx.open else {
        return;
    };
    let Some(slot) = app.project().effect(strip, effect).cloned() else {
        return;
    };
    use cantus_engine::FxUnit;
    let sr = 48_000.0;
    let mut spec = slot.spec.clone();
    cantus_core::fx::tabula::ensure_macro_keys(&mut spec);
    cantus_core::fx::tabula::apply_macros(&mut spec);
    let mut u = cantus_engine::fx::tabula::Unit::build(&spec, sr);
    let n = 9_600;
    let (mut l, mut r): (Vec<f32>, Vec<f32>) = (0..n)
        .map(|i| {
            let t = i as f32 / sr;
            let env = amp * (-t * 3.0).exp();
            let v = [196.0f32, 246.9, 293.7]
                .iter()
                .enumerate()
                .map(|(j, f)| (std::f32::consts::TAU * f * t).sin() / (1.0 + j as f32))
                .sum::<f32>()
                * env
                * 0.6;
            (v, v)
        })
        .unzip();
    for (a, c) in l.chunks_mut(480).zip(r.chunks_mut(480)) {
        u.process(a, c, None);
    }
    app.fx.injected = Some(u.telemetry());
    let open = app.fx.tabula.open.as_ref().map(|o| o.pedal);
    app.fx.tabula.pedal_tel = open.and_then(|p| u.pedal_telemetry(p));
}
