//! Tabula's editor: the pedalboard (ADR 0134 §5) — native units as pedals on a dark board,
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

/// Factory presets: data (`presets/effects/board.toml`, ADR 0101).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[];

/// The macro rail's height and the side column's width, at 100 %.
const RAIL_H: f32 = 118.0;
const SIDE_W: f32 = 232.0;

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
    let side = SIDE_W * k;
    let total = rest.width() - gap;
    let cols: Vec<Rect> = g
        .row(rest.height(), &[(total - side) / total, side / total])
        .collect();
    if st.open.is_some() {
        pedal_editor_ui(host, st, ui, s, cols[0]);
    } else {
        // The board over the chosen pedal's controls.
        let strip_h = controls::card_height(Size::M) * k;
        let board = Rect::from_min_max(
            cols[0].min,
            pos2(cols[0].right(), cols[0].bottom() - strip_h - gap),
        );
        let strip = Rect::from_min_max(pos2(cols[0].left(), board.bottom() + gap), cols[0].max);
        board_ui(host, st, ui, s, board, tel);
        edit_strip_ui(host, st, ui, s, strip);
    }
    side_ui(host, st, ui, s, cols[1]);
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
    if kind == EffectKind::Pedal {
        let m = super::fragor::model_of(p.spec.get(cantus_core::fx::fragor::SLOTS[0])).unwrap_or(0);
        keys.extend(cantus_core::fx::fragor::KNOBS[m]);
    } else if let Some(f) = super::tabula_faces::of(kind) {
        keys.extend(f.knobs.iter().copied());
    }
    for i in kind.params() {
        if keys.len() >= 8 {
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
    controls::card(ui, s, r, Src::Str(&title), Some(p.on), |ui, _| {
        let Some(mut ph) = super::tabula_host::PedalHost::new(host, id, ptel) else {
            return;
        };
        let fragor_words = (kind == EffectKind::Pedal).then(|| {
            super::fragor::knob_labels(
                super::fragor::model_of(ph.spec().get(cantus_core::fx::fragor::SLOTS[0]))
                    .unwrap_or(0),
            )
        });
        for (j, key) in keys.iter().enumerate() {
            let Some(info) = kind.param(key) else {
                continue;
            };
            let label = match fragor_words {
                Some(w) if j < 3 => Src::Key(w[j]),
                _ => tb::knob_word(key).map_or(Src::Key(info.label_key), Src::Key),
            };
            let out = Cell::param(info, ph.spec().get(key))
                .size(Size::M)
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
        if controls::key(ui, s, Src::Key("tabula.pedal.open"), "", 170.0, false).clicked() {
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
    controls::card(ui, s, r, Src::Key("tabula.card.macros"), None, |ui, _| {
        for (i, key) in MACRO_KEYS.iter().enumerate() {
            let Some(info) = KIND.param(key) else {
                continue;
            };
            let c = Cell::param(info, spec.get(key))
                .size(Size::S)
                .width(86.0)
                .label(Src::Str(&names[i]));
            let out = c.show_in(ui, s, host, key);
            if let Some(v) = out.changed {
                host.set(key, v, Some(key));
            }
            if out.done {
                host.end_gesture();
            }
        }
        ui.add_space(s.d(10.0));
        controls::column(ui, s, 120.0, |ui| {
            if controls::key(ui, s, Src::Key("tabula.learn"), "", 116.0, learn).clicked() {
                st.learn = !st.learn;
            }
            let _ = controls::key(ui, s, Src::Key("tabula.randomise"), "", 116.0, false);
        });
    });
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
    let devices = crate::picker_catalog::build(std::iter::empty());
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
        for (id, rect) in &geo.pedals {
            let Some(p) = pedals.iter().find(|p| p.id == *id) else {
                continue;
            };
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
                    hot: None,
                    selected: selected == Some(*id),
                    bare: false,
                },
            );
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
        let mut cab = |b: &mut crate::kit::Brush, a: tb::End, z: tb::End, l: f32| {
            flowing |= l > 0.01;
            tb::cable(b, &sd, a, z, lift, Some(pd::Energy { level: l, time }));
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
        with_brush(&ui.painter_at(r), |b| {
            tb::picker(
                b,
                s,
                panel,
                Src::Key("tabula.picker.title"),
                &names,
                shelf,
                kinds,
                st.picker_hover,
            );
        });
    }
    if flowing && st.anim_time.is_none() {
        ui.ctx().request_repaint();
    }
    // Choose a pedal by a click; its full editor by a double click; the picker from the
    // place to add (the rest of the board's gestures: phase B).
    let resp = ui.interact(r, ui.id().with("tabula-board"), egui::Sense::click());
    if let Some(p) = resp.interact_pointer_pos() {
        if let Some((id, _)) = pedal_hits.iter().find(|(_, h)| h.body.contains(p)) {
            st.selected = Some(*id);
            if resp.double_clicked() {
                open_pedal(st, &spec, *id);
            }
        } else if resp.clicked() && geo.add.contains(p) {
            st.picker = if st.picker.is_some() { None } else { Some(0) };
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

/// The side column: the board's own levels, and the chosen pedal.
fn side_ui(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui, s: &Style, r: Rect) {
    let k = s.k;
    let mut g = Grid::new(r, s);
    let split = host
        .spec()
        .pedals()
        .iter()
        .any(|p| matches!(p.path, Path::A | Path::B));
    let h = controls::card_height(Size::S) * k;
    if let Some(top) = g.row(h, &[1.0]).next() {
        controls::card(ui, s, top, Src::Key("tabula.card.board"), None, |ui, _| {
            for key in ["in", "mix", "out"] {
                let Some(info) = KIND.param(key) else {
                    continue;
                };
                let out = Cell::param(info, host.spec().get(key))
                    .size(Size::S)
                    .width(66.0)
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
    let rest = g.rest();
    let spec = host.spec().clone();
    let chosen = st
        .selected
        .or_else(|| spec.pedals().first().map(|p| p.id))
        .and_then(|id| spec.pedals().iter().find(|p| p.id == id).cloned());
    let mut open = None;
    controls::card(ui, s, rest, Src::Key("tabula.card.pedal"), None, |ui, _| {
        let w = SIDE_W - 26.0;
        controls::column(ui, s, w, |ui| {
            let Some(p) = &chosen else {
                controls::caption(ui, s, Src::Key("tabula.pedal.none"), w);
                return;
            };
            let (name, what) = tb::names_of(&p.spec);
            let (name, what) = (src_text(name), src_text(what));
            controls::readout(ui, s, &name, false, &what, w);
            controls::caption(ui, s, Src::Str(&what), w);
            controls::caption(ui, s, Src::Key("tabula.pedal.size"), w);
            let shapes = [
                Src::Key("tabula.shape.mini"),
                Src::Key("tabula.shape.standard"),
                Src::Key("tabula.shape.wide"),
                Src::Key("tabula.shape.double"),
                Src::Key("tabula.shape.round"),
            ];
            let now = tb::shape_of(p) as usize;
            // Two rows of the five shapes; the row without the pedal's shows none lit.
            let first = if now < 3 { now } else { 9 };
            let second = now.checked_sub(3).unwrap_or(9);
            let _ = controls::segmented(ui, s, first, &shapes[..3], "", w);
            let _ = controls::segmented(ui, s, second, &shapes[3..], "", w);
            controls::caption(ui, s, Src::Key("tabula.pedal.switch"), w);
            let _ = controls::led(ui, s, p.on, Src::Key("tabula.pedal.on"), "", w);
            if split || p.path != Path::Pre {
                controls::caption(ui, s, Src::Key("tabula.pedal.lane"), w);
                let lanes = [
                    Src::Key("tabula.path.pre"),
                    Src::Key("tabula.lane.a"),
                    Src::Key("tabula.lane.b"),
                    Src::Key("tabula.path.post"),
                ];
                let at = match p.path {
                    Path::Pre => 0,
                    Path::A => 1,
                    Path::B => 2,
                    Path::Post => 3,
                };
                let _ = controls::segmented(ui, s, at, &lanes, "", w);
            }
            ui.add_space(s.d(6.0));
            if controls::key(ui, s, Src::Key("tabula.pedal.open"), "", w, false).clicked() {
                open = Some(p.id);
            }
            let _ = controls::key(ui, s, Src::Key("tabula.pedal.remove"), "", w, false);
        });
    });
    if let Some(id) = open {
        open_pedal(st, &spec, id);
    }
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
