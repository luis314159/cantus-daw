//! fxd — the formula controller's face (ADR 0145, design sheet v1): a scientific
//! calculator's case. On top, the formula typed on a vacuum-fluorescent line behind green
//! glass (the caret where it is typed, the error under it with the character it points at);
//! under it a plotting screen with its graticule — the formula's value over the last four
//! beats and the next four at the knobs as they are, "now" a cursor, the value a lit bead.
//! Beside them the object of character, the keypad: the language's words on keys (what a
//! click types), the knobs a, b and c as three dials on the case; the targets it drives
//! listed with their ranges. Cards under them.

use super::fxa_cells::Item;
use super::fxd_art::{self as art, Card, Moved, Value};
use crate::kit::color::{mix, with_alpha};
use crate::kit::text::{Role, Src};
use crate::kit::{Brush, Fill, Style};
use cantus_core::modulator::formula::{FormulaError, Program};
use cantus_core::modulator::FormulaSource;
use egui::{pos2, vec2, Align2, Pos2, Rect, Vec2};

/// What the face shows of the engine and of the typing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Live {
    /// The song position, beats.
    pub beats: f64,
    pub tempo: f64,
    /// The text being typed when it differs from the formula playing, and why it does not
    /// compile (it keeps playing the last good one).
    pub draft: Option<String>,
    /// The caret, a byte offset into what is shown, while typing.
    pub caret: Option<usize>,
    /// The display holds the keyboard (a typing session).
    pub focused: bool,
    /// The first row the links list shows.
    pub links_first: usize,
    /// What the knobs a, b, c and the amount offer their controls (the kit's parameter
    /// menu, MIDI learn, the signs): from the app each frame; `None` on the design sheets.
    pub params: art::Params,
}

/// Where the face's parts were drawn this frame (the window's tests, its hints).
#[derive(Clone, Debug)]
pub struct Rects {
    /// The fluorescent line the formula is typed on.
    pub display: Rect,
    /// The keypad's keys, in [`KEYS`]' order.
    pub keys: Vec<Rect>,
    /// The dials a, b and c.
    pub dials: [Rect; 3],
    pub links: Rect,
    pub items: art::ItemRects,
}

impl Default for Rects {
    fn default() -> Self {
        Rects {
            display: Rect::NOTHING,
            keys: Vec::new(),
            dials: [Rect::NOTHING; 3],
            links: Rect::NOTHING,
            items: Vec::new(),
        }
    }
}

pub use super::fxd_xy::Link;

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// A key of the keypad typed its word.
    Type(&'static str),
    Card(Moved),
    /// A click on the display: type there, the caret before character `n`.
    Focus(usize),
    /// Dial `i` (a, b, c) dragged by this share of its travel, its drag ended, or reset
    /// by a double-click.
    Dial(usize, f32),
    DialDone(usize),
    DialReset(usize),
    /// The links list scrolled to show from this row.
    LinksAt(usize),
    /// A link's range edited in the list.
    Link(super::fxd_xy::LinkEdit),
    /// A dial answered the kit's parameter menu or modulation (slot 0…2: a, b, c).
    Param(usize, crate::kit::automate::Choice, Rect),
}

pub const CARDS: [Card<'static>; 2] = [
    Card {
        title: "fxd.formula.card.knobs",
        items: &[
            Item::Knob("a", "fxd.formula.a"),
            Item::Knob("b", "fxd.formula.b"),
            Item::Knob("c", "fxd.formula.c"),
        ],
    },
    Card {
        title: "fxd.card.output",
        items: &[
            Item::Switch(
                "pol",
                "fxd.card.polarity",
                &["fxd.card.unipolar", "fxd.card.bipolar"],
            ),
            Item::Knob("amount", "fxd.card.amount"),
        ],
    },
];

/// The keypad's keys: what each shows and what it types.
pub const KEYS: [(&str, &str); 24] = [
    ("sin", "Sin("),
    ("cos", "Cos("),
    ("tri", "tri("),
    ("saw", "saw("),
    ("frac", "Frac("),
    ("round", "Round("),
    ("int", "Int("),
    ("rand", "Rand"),
    ("a", "a"),
    ("b", "b"),
    ("c", "c"),
    ("π", "Pi"),
    ("beat", "SongTime"),
    ("t", "t"),
    ("(", "("),
    (")", ")"),
    ("+", "+"),
    ("−", "-"),
    ("×", "*"),
    ("÷", "/"),
    ("^", "^"),
    ("if", "If("),
    ("max", "Max("),
    ("min", "Min("),
];

pub const SCREEN_W: f32 = 560.0;
const VFD_H: f32 = 64.0;
const PLOT_H: f32 = 300.0;
const SIDE: f32 = 330.0;
const RAIL: f32 = 18.0;
const MARGIN: f32 = 14.0;
/// The keypad's least height at 100 %: its dials and four rows of keys.
const KEYPAD_MIN: f32 = 300.0;

pub fn body_size(ui: &egui::Ui) -> Vec2 {
    let w = SCREEN_W + 2.0 * RAIL + MARGIN * 3.0 + SIDE;
    let cards = art::cards_height(ui, &CARDS, w - 2.0 * MARGIN);
    vec2(w, MARGIN * 4.0 + VFD_H + PLOT_H + 4.0 * RAIL + 24.0 + cards)
}

/// The face in `rect`: what the user did, and where its parts were drawn.
#[allow(clippy::too_many_arguments)]
pub fn body(
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    src: &FormulaSource,
    amount: f32,
    links: &[Link],
    live: &Live,
) -> (Vec<Action>, Rects) {
    let k = s.k;
    let mut out = Vec::new();
    let mut rects = Rects::default();
    let col = rect.left() + MARGIN * k;
    let vfd_outer = Rect::from_min_size(
        pos2(col, rect.top() + MARGIN * k),
        vec2((SCREEN_W + 2.0 * RAIL) * k, (VFD_H + 2.0 * RAIL) * k),
    );
    let err_line = Rect::from_min_size(
        vfd_outer.left_bottom() + vec2(RAIL * k, 2.0 * k),
        vec2(SCREEN_W * k, 20.0 * k),
    );
    let plot_outer = Rect::from_min_size(
        pos2(col, err_line.bottom() + 4.0 * k),
        vec2((SCREEN_W + 2.0 * RAIL) * k, (PLOT_H + 2.0 * RAIL) * k),
    );
    let side = Rect::from_min_max(
        pos2(vfd_outer.right() + MARGIN * k, vfd_outer.top()),
        pos2(rect.right() - MARGIN * k, plot_outer.bottom()),
    );
    let typed = live.draft.as_deref().unwrap_or(&src.text);
    let error: Option<FormulaError> = live
        .draft
        .as_deref()
        .and_then(|d| Program::compile(d).err());
    let program = Program::compile(&src.text).ok();
    let mut b = Brush::default();
    b.begin(ui.ctx());
    let vfd = art::frame(&mut b, s, vfd_outer, RAIL * k);
    let line = fluorescent(
        ui.ctx(),
        &mut b,
        s,
        vfd,
        typed,
        live.caret.filter(|_| live.focused),
        error.as_ref(),
        live.focused,
    );
    let plot = art::frame(&mut b, s, plot_outer, RAIL * k);
    screen(&mut b, s, plot, src, program.as_ref(), live);
    message(&mut b, s, err_line, error.as_ref());
    // The side: the links sized to their rows, the keypad taking the rest (never less
    // than its dials and four rows of keys).
    let list_h = art::links_height(links.len(), side.height() / k - KEYPAD_MIN) * k;
    let (pad, list) = side.split_top_bottom_at_y(side.bottom() - list_h);
    let (keys, dials) = keypad(&mut b, s, pad.shrink2(vec2(0.0, 2.0 * k)), src);
    // The signs of what drives each dial (a clip, a controller, a modulator).
    for (r, p) in dials.iter().zip(&live.params) {
        if let Some(p) = p {
            let mut d = p.offer.drivers();
            d.modulated |= p.n > 0;
            crate::kit::automate::signs(&mut b, *r, s, d);
        }
    }
    let list = list.shrink2(vec2(0.0, 4.0 * k));
    let fit = super::fxd_xy::links_panel(ui.ctx(), &mut b, s, list, links, false, live.links_first);
    b.flush(ui.painter());
    // The display: a click types there.
    let dr = ui.interact(vfd, super::fxd_edit::display_id(), egui::Sense::click());
    if dr.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
    }
    if let (true, Some(p)) = (dr.clicked(), dr.interact_pointer_pos()) {
        let byte = caret_at(ui.ctx(), s, typed, line, p.x);
        out.push(Action::Focus(typed[..byte].chars().count()));
    }
    for (i, r) in keys.iter().enumerate() {
        let resp = ui.interact(*r, ui.id().with(("fxd-key", i)), egui::Sense::click());
        if resp.clicked() {
            out.push(Action::Type(KEYS[i].1));
        }
    }
    // The dials: dragged up or down (150 points the whole travel), a double-click resets;
    // each the kit's parameter menu on a right-click (automate, MIDI learn, forget, reset;
    // Modulate…), a click in the learn mode learns it, Alt+drag sets a modulation's depth.
    for (i, r) in dials.iter().enumerate() {
        let resp = ui.interact(
            *r,
            ui.id().with(("fxd-dial", i)),
            egui::Sense::click_and_drag(),
        );
        if resp.hovered() || resp.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
        let param = live.params[i].as_ref();
        let name = ["fxd.formula.a", "fxd.formula.b", "fxd.formula.c"][i];
        let offer = param.map(|p| p.offer);
        let not_turned = crate::kit::automate::depth_drag(ui, offer);
        if let Some(c) = art::param_spot(ui, &resp, name, param) {
            out.push(Action::Param(i, c, *r));
        }
        if not_turned {
            // The learn mode's click learns and an Alt-drag sets a depth: the dial stays.
            continue;
        }
        if resp.double_clicked() {
            out.push(Action::DialReset(i));
        } else if resp.dragged() {
            let dy = resp.drag_delta().y;
            if dy != 0.0 {
                out.push(Action::Dial(i, -dy / (150.0 * k)));
            }
        }
        if resp.drag_stopped() {
            out.push(Action::DialDone(i));
        }
    }
    if let Some(at) = super::fxd_xy::scroll_links(ui, list, links.len(), fit, live.links_first) {
        out.push(Action::LinksAt(at));
    }
    out.extend(
        super::fxd_xy::link_edits(ui, s.k, list, links, false, live.links_first)
            .into_iter()
            .map(Action::Link),
    );
    rects.display = vfd;
    rects.keys = keys;
    rects.dials = dials;
    rects.links = list;
    let cards = Rect::from_min_max(
        pos2(col, plot_outer.bottom() + MARGIN * k),
        pos2(rect.right() - MARGIN * k, rect.bottom() - MARGIN * k),
    );
    let pct = |v: f32, b: &mut String| {
        use std::fmt::Write as _;
        let _ = write!(b, "{v:.2}");
    };
    let amt = |v: f32, b: &mut String| {
        use std::fmt::Write as _;
        let _ = write!(b, "{:.0} %", v * 100.0);
    };
    let values = |c: usize, i: usize| -> Value {
        match (c, i) {
            (0, i) => Value::Knob {
                v: src.inputs[i.min(2)],
                lo: 0.0,
                hi: 1.0,
                def: 0.5,
                fmt: &pct,
                unit: "",
                taper: crate::widgets::knob::Taper::Linear,
                param: Some(i.min(2)),
            },
            (_, 0) => Value::Choice(usize::from(src.bipolar)),
            _ => Value::Knob {
                v: amount,
                lo: 0.0,
                hi: 1.0,
                def: 1.0,
                fmt: &amt,
                unit: "%",
                taper: crate::widgets::knob::Taper::Linear,
                param: Some(art::AMOUNT),
            },
        }
    };
    out.extend(
        art::lay_cards(
            ui,
            s,
            cards,
            &CARDS,
            &values,
            &live.params,
            &mut rects.items,
        )
        .into_iter()
        .map(Action::Card),
    );
    (out, rects)
}

/// The message under the line: why a draft is refused (with its character), or that the
/// formula compiled and plays.
fn message(b: &mut Brush, s: &Style, r: Rect, error: Option<&FormulaError>) {
    use cantus_core::modulator::formula::ErrorKind;
    match error {
        Some(e) => {
            let what = match &e.kind {
                ErrorKind::Unexpected(c) => c.to_string(),
                ErrorKind::UnknownName(n)
                | ErrorKind::NotDeterministic(n)
                | ErrorKind::NotSupported(n) => n.clone(),
                ErrorKind::Arguments { name, .. } => name.clone(),
                _ => String::new(),
            };
            let n = match &e.kind {
                ErrorKind::Arguments { wants, .. } => *wants,
                _ => 0,
            };
            let at = e.at + 1;
            let args: [(&str, &dyn std::fmt::Display); 3] =
                [("at", &at), ("what", &what), ("n", &n)];
            let msg = crate::i18n::tf_in(crate::i18n::locale(), e.kind.key(), &args);
            art::value(b, s, r, &msg, Role::Label, s.t.danger, Align2::LEFT_CENTER);
        }
        None => art::label(
            b,
            s,
            r,
            Src::Key("fxd.formula.compiled"),
            Role::Micro,
            s.t.text_dim,
            Align2::LEFT_CENTER,
        ),
    }
}

/// The fluorescent line: the formula in the display's mono, a caret, the error's character
/// underlined, a lit edge while it holds the keyboard. Returns where its text was painted
/// (a click's caret).
#[allow(clippy::too_many_arguments)]
fn fluorescent(
    ctx: &egui::Context,
    b: &mut Brush,
    s: &Style,
    r: Rect,
    text: &str,
    caret: Option<usize>,
    error: Option<&FormulaError>,
    focused: bool,
) -> Line {
    let k = s.k;
    let lit = art::lit(s);
    b.rrect(r, 3.0 * k, Fill::Solid(mix(art::ground(s), lit, 0.05)));
    // The tube's filament grid: fine horizontal wires.
    for i in 0..3 {
        let y = r.top() + r.height() * (0.25 + 0.25 * i as f32);
        b.hline(
            r.left() + 4.0 * k,
            r.right() - 4.0 * k,
            y,
            0.4 * k,
            with_alpha(lit, 0.06),
        );
    }
    let line = r.shrink2(vec2(14.0 * k, 0.0));
    let face = crate::kit::text::Face {
        mono: true,
        ..crate::kit::text::Face::ROLE
    };
    // The line's letters: the display's size while the formula fits, smaller down to 60 %
    // for a longer one, and past that a window of it that keeps the caret (or the error)
    // in sight, a mark at each side where more is hidden. Mono: every character as wide.
    let tk = s.tk();
    let width = |t: &str, kt: f32| {
        let m = crate::kit::text::Text::new(t, Role::Display, lit).caps(false);
        crate::kit::text::measure_face(ctx, &m, kt, face).x
    };
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let n = chars.len();
    let whole = width(text, tk);
    let shrink = (line.width() / whole.max(1.0)).clamp(0.6, 1.0);
    let kt = tk * shrink;
    let index = |byte: usize| text[..byte.min(text.len())].chars().count();
    let (first, last) = if whole * shrink <= line.width() {
        (0, n)
    } else {
        let avg = whole * shrink / n.max(1) as f32;
        let fit = ((line.width() / avg).floor() as usize).max(4);
        let keep = caret.or(error.map(|e| e.at)).map_or(0, index);
        let mut first = keep
            .saturating_sub(fit.saturating_sub(4))
            .min(n.saturating_sub(fit));
        let mut last = (first + fit).min(n);
        let byte = |i: usize| chars.get(i).map_or(text.len(), |c| c.0);
        // The estimate measured: a character less at the side away from the caret.
        while last > first + 1 && width(&text[byte(first)..byte(last)], kt) > line.width() {
            if last.saturating_sub(keep) <= 3 {
                first += 1;
            } else {
                last -= 1;
            }
        }
        (first, last)
    };
    let from = chars.get(first).map_or(text.len(), |c| c.0);
    let to = chars.get(last).map_or(text.len(), |c| c.0);
    let shown = &text[from..to];
    let t = crate::kit::text::Text::new(shown, Role::Display, lit)
        .caps(false)
        .align(Align2::LEFT_CENTER);
    // A glow under the glyphs, then the glyphs.
    b.rrect_soft(
        line.shrink2(vec2(0.0, line.height() * 0.3)),
        6.0 * k,
        Fill::Solid(with_alpha(lit, 0.06)),
        10.0 * k,
    );
    let _ = crate::kit::text::paint_face(b, line, &t, kt, face);
    for (more, x, d) in [
        (first > 0, line.left() - 7.0 * k, -1.0),
        (last < n, line.right() + 7.0 * k, 1.0),
    ] {
        if more {
            let c = pos2(x, line.center().y);
            b.polygon(
                &[
                    c + vec2(3.0 * d, 0.0) * k,
                    c + vec2(-2.0 * d, -4.0) * k,
                    c + vec2(-2.0 * d, 4.0) * k,
                ],
                with_alpha(lit, 0.7),
            );
        }
    }
    // Where a character begins: the shown text before it, measured as it is painted.
    let x_of = |byte: usize| line.left() + width(&text[from..byte.clamp(from, to)], kt);
    let char_w = width(shown, kt) / (last - first).max(1) as f32;
    let half = 13.0 * k * shrink.max(0.8);
    if let Some(c) = caret {
        let x = x_of(c);
        b.vline(
            x,
            line.center().y - half,
            line.center().y + half,
            1.6 * k,
            lit,
        );
    }
    if let Some(e) = error.filter(|e| (first..=last).contains(&index(e.at))) {
        let x = x_of(e.at);
        b.hline(
            x,
            x + char_w.max(6.0 * k),
            line.center().y + half + 2.0 * k,
            1.6 * k,
            s.t.danger,
        );
    }
    art::label(
        b,
        s,
        Rect::from_min_size(r.min + vec2(8.0, 3.0) * k, vec2(200.0 * k, 12.0 * k)),
        Src::Key("fxd.formula.line"),
        Role::Micro,
        with_alpha(lit, 0.55),
        Align2::LEFT_CENTER,
    );
    art::glass(b, s, r);
    if focused {
        b.rrect_stroke(r.expand(1.0 * k), 3.5 * k, 1.4 * k, with_alpha(lit, 0.8));
    }
    Line {
        left: line.left(),
        k: kt,
        from,
        to,
    }
}

/// Where the fluorescent line's text was painted: its left, its scale, the bytes shown.
#[derive(Copy, Clone, Debug)]
struct Line {
    left: f32,
    k: f32,
    from: usize,
    to: usize,
}

/// The caret a click at `x` puts in `text` on `line`: before the character nearest it (a
/// byte offset).
fn caret_at(ctx: &egui::Context, s: &Style, text: &str, line: Line, x: f32) -> usize {
    let face = crate::kit::text::Face {
        mono: true,
        ..crate::kit::text::Face::ROLE
    };
    let shown = &text[line.from..line.to];
    let mut best = (f32::MAX, line.to);
    for (i, _) in shown
        .char_indices()
        .chain(std::iter::once((shown.len(), ' ')))
    {
        let m = crate::kit::text::Text::new(&shown[..i], Role::Display, art::lit(s)).caps(false);
        let w = crate::kit::text::measure_face(ctx, &m, line.k, face).x;
        let d = (line.left + w - x).abs();
        if d < best.0 {
            best = (d, line.from + i);
        }
    }
    best.1
}

/// The plotting screen: the graticule, the formula over eight beats, the cursor at now.
fn screen(
    b: &mut Brush,
    s: &Style,
    r: Rect,
    src: &FormulaSource,
    program: Option<&Program>,
    live: &Live,
) {
    let k = s.k;
    let (lit, lit2) = (art::lit(s), art::lit2(s));
    b.rrect(r, 3.0 * k, Fill::Solid(art::ground(s)));
    let plot = r.shrink2(vec2(34.0 * k, 22.0 * k));
    for i in 0..=8 {
        let x = plot.left() + plot.width() * i as f32 / 8.0;
        b.vline(
            x,
            plot.top(),
            plot.bottom(),
            if i == 4 { 0.9 } else { 0.5 } * k,
            with_alpha(lit, if i == 4 { 0.0 } else { 0.12 }),
        );
        for j in 1..5 {
            let xx = x + plot.width() / 40.0 * j as f32;
            if i < 8 {
                b.vline(
                    xx,
                    plot.center().y - 2.0 * k,
                    plot.center().y + 2.0 * k,
                    0.5 * k,
                    with_alpha(lit, 0.18),
                );
            }
        }
    }
    for j in 0..=4 {
        let y = plot.bottom() - plot.height() * j as f32 / 4.0;
        b.hline(
            plot.left(),
            plot.right(),
            y,
            0.5 * k,
            with_alpha(lit, if j == 2 { 0.2 } else { 0.12 }),
        );
        let v = j as f32 / 4.0;
        let lab = if src.bipolar {
            format!("{:+.1}", 2.0 * v - 1.0)
        } else {
            format!("{v:.2}")
        };
        art::value(
            b,
            s,
            Rect::from_min_size(
                pos2(r.left() + 2.0 * k, y - 7.0 * k),
                vec2(30.0 * k, 14.0 * k),
            ),
            &lab,
            Role::Micro,
            with_alpha(lit, 0.6),
            Align2::RIGHT_CENTER,
        );
    }
    for (i, key) in [
        (0usize, "fxd.formula.ago"),
        (4, "fxd.formula.now"),
        (8, "fxd.formula.ahead"),
    ] {
        let x = plot.left() + plot.width() * i as f32 / 8.0;
        let align = match i {
            0 => Align2::LEFT_CENTER,
            8 => Align2::RIGHT_CENTER,
            _ => Align2::CENTER_CENTER,
        };
        let rr =
            Rect::from_center_size(pos2(x, plot.bottom() + 11.0 * k), vec2(150.0 * k, 14.0 * k));
        let rr = match i {
            0 => rr.translate(vec2(75.0 * k, 0.0)),
            8 => rr.translate(vec2(-75.0 * k, 0.0)),
            _ => rr,
        };
        art::label(
            b,
            s,
            rr,
            Src::Key(key),
            Role::Micro,
            with_alpha(lit, 0.6),
            align,
        );
    }
    let Some(p) = program else { return };
    let env = |beats: f64| cantus_core::modulator::formula::Env {
        a: f64::from(src.inputs[0]),
        b: f64::from(src.inputs[1]),
        c: f64::from(src.inputs[2]),
        beats,
        seconds: beats * 60.0 / live.tempo.max(1.0),
        tempo: live.tempo,
    };
    let n = 400;
    let y_of = |v: f32| plot.bottom() - v * plot.height();
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let beats = live.beats - 4.0 + 8.0 * i as f64 / n as f64;
            let x = plot.left() + plot.width() * i as f32 / n as f32;
            pos2(x, y_of(p.output(&env(beats.max(0.0)))))
        })
        .collect();
    let (past, future) = pts.split_at(n / 2 + 1);
    b.area(past, plot.bottom(), with_alpha(lit2, 0.10));
    b.glow_polyline(past, 1.4 * k, lit2, with_alpha(lit2, 0.25), 5.0 * k);
    let mut fut = vec![past[past.len() - 1]];
    fut.extend_from_slice(future);
    b.dashed(&fut, 1.0 * k, with_alpha(lit2, 0.6), 4.0 * k, 3.0 * k);
    let now = pts[n / 2];
    b.vline(
        now.x,
        plot.top(),
        plot.bottom(),
        1.0 * k,
        with_alpha(lit, 0.8),
    );
    b.circle(now, 7.0 * k, Fill::Solid(with_alpha(lit2, 0.25)));
    b.circle(now, 3.4 * k, Fill::Solid(lit2));
    let v = p.output(&env(live.beats));
    let shown = if src.bipolar {
        format!("{:+.3}", 2.0 * v - 1.0)
    } else {
        format!("{v:.3}")
    };
    art::value(
        b,
        s,
        Rect::from_min_size(
            pos2(now.x + 8.0 * k, plot.top() + 2.0 * k),
            vec2(90.0 * k, 16.0 * k),
        ),
        &shown,
        Role::Label,
        lit2,
        Align2::LEFT_CENTER,
    );
    art::glass(b, s, r);
}

/// The keypad: the case's keys in a bezel, the three knob dials above them. Returns the
/// keys' rects (clicks type their words).
fn keypad(b: &mut Brush, s: &Style, r: Rect, src: &FormulaSource) -> (Vec<Rect>, [Rect; 3]) {
    let k = s.k;
    let m = art::metal(s);
    art::well(b, s, r);
    art::label(
        b,
        s,
        Rect::from_min_size(
            r.min + vec2(10.0, 6.0) * k,
            vec2(r.width() - 20.0 * k, 14.0 * k),
        ),
        Src::Key("fxd.formula.keypad"),
        Role::Micro,
        s.t.text_dim,
        Align2::LEFT_CENTER,
    );
    // The three dials: a, b, c as the case shows them (the cards hold the knobs).
    let dial_r = 20.0 * k;
    let mut dials = [Rect::NOTHING; 3];
    for (i, name) in ["a", "b", "c"].iter().enumerate() {
        let c = pos2(
            r.left() + r.width() * (0.2 + 0.3 * i as f32),
            r.top() + 50.0 * k,
        );
        dials[i] = Rect::from_center_size(c, Vec2::splat(dial_r * 2.4));
        b.circle(
            c + vec2(1.0, 2.5) * k,
            dial_r,
            Fill::Solid(with_alpha(s.t.dark_shade, 0.4)),
        );
        b.circle(c, dial_r, Fill::V(m.top, m.foot));
        for t in 0..=10 {
            let a = (0.75 + 1.5 * t as f32 / 10.0) * std::f32::consts::PI;
            let d = vec2(cantus_dsp::math::cos(a), cantus_dsp::math::sin(a));
            b.segment(
                c + d * (dial_r + 2.0 * k),
                c + d * (dial_r + if t % 5 == 0 { 6.0 } else { 4.0 } * k),
                0.6 * k,
                s.t.text_dim,
                false,
            );
        }
        let a = (0.75 + 1.5 * src.inputs[i]) * std::f32::consts::PI;
        let d = vec2(cantus_dsp::math::cos(a), cantus_dsp::math::sin(a));
        b.segment(
            c + d * dial_r * 0.25,
            c + d * dial_r * 0.85,
            2.0 * k,
            s.hue,
            true,
        );
        b.circle(c, dial_r * 0.22, Fill::V(m.foot, m.top));
        art::value(
            b,
            s,
            Rect::from_center_size(c + vec2(0.0, dial_r + 14.0 * k), vec2(60.0 * k, 14.0 * k)),
            &format!("{name} {:.2}", src.inputs[i]),
            Role::Micro,
            s.t.text,
            Align2::CENTER_CENTER,
        );
    }
    let grid = Rect::from_min_max(
        pos2(r.left() + 10.0 * k, r.top() + 100.0 * k),
        pos2(r.right() - 10.0 * k, r.bottom() - 10.0 * k),
    );
    let (cols, rows) = (6usize, 4usize);
    let (kw, kh) = (grid.width() / cols as f32, grid.height() / rows as f32);
    let mut rects = Vec::with_capacity(KEYS.len());
    for (i, (show, types)) in KEYS.iter().enumerate() {
        let (cx, cy) = (i % cols, i / cols);
        let key = Rect::from_min_size(
            pos2(grid.left() + cx as f32 * kw, grid.top() + cy as f32 * kh),
            vec2(kw, kh),
        )
        .shrink(2.5 * k);
        // A key: a shadow, the cap rising to its face, a lit legend; the operators the second voice.
        let op = i >= 14;
        let cap = if op { mix(m.face, s.hue, 0.22) } else { m.face };
        b.rrect(
            key.translate(vec2(0.0, 1.8 * k)),
            4.0 * k,
            Fill::Solid(with_alpha(s.t.dark_shade, 0.45)),
        );
        b.rrect(
            key,
            4.0 * k,
            Fill::V(mix(cap, s.t.light, 0.18), mix(cap, s.t.dark_shade, 0.18)),
        );
        b.rrect(
            key.shrink(2.0 * k),
            3.0 * k,
            Fill::V(mix(cap, s.t.light, 0.08), cap),
        );
        b.rrect_stroke(key, 4.0 * k, 0.6 * k, m.edge);
        // The legend, and above it in the second voice what the key types when it is more.
        let (top, low) = key.split_top_bottom_at_fraction(0.42);
        let same = types.trim_end_matches('(').eq_ignore_ascii_case(show);
        if !same && types.len() > 1 {
            art::value(
                b,
                s,
                top.translate(vec2(0.0, 2.0 * k)),
                types,
                Role::Micro,
                mix(s.t.text_dim, s.hue, 0.5),
                Align2::CENTER_CENTER,
            );
            art::value(
                b,
                s,
                low.translate(vec2(0.0, -3.0 * k)),
                show,
                Role::Label,
                s.t.text,
                Align2::CENTER_CENTER,
            );
        } else {
            art::value(
                b,
                s,
                key,
                show,
                Role::Label,
                s.t.text,
                Align2::CENTER_CENTER,
            );
        }
        rects.push(key);
    }
    (rects, dials)
}
