//! fxd — the X-Y pad's face (ADR 0147, design sheet v1): a touch surface of dark glass in a
//! machined frame — a fine grid, the axes engraved on the rails with the parameters each
//! drives, the puck (a lit glass disc in a metal ring) where it is, the hand's crosshair
//! where it heads, its inertia trail fading behind it, the path drawn as a lit loop with its
//! direction and where the song is on it; beside it the object of character, the inertia
//! flywheel (its rim heavier the more inertia, its spokes blurred as the puck travels), and
//! the links — each target with its axis and its range drawn as a bar. Cards under them.

use super::fxa_cells::Item;
use super::fxd_art::{self as art, Card, Moved, Value};
use crate::kit::color::{mix, with_alpha};
use crate::kit::glyph::Glyph;
use crate::kit::text::{Role, Src};
use crate::kit::{Brush, Fill, Style};
use cantus_core::modulator::{RouteRange, XySource};
use cantus_dsp::math::{cos, sin};
use egui::{pos2, vec2, Align2, Pos2, Rect, Vec2};
use std::f32::consts::TAU;

/// What the face shows of the engine (the puck as it plays).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Live {
    /// Where the puck is, and where it heads (0…1 each, y up).
    pub puck: [f32; 2],
    pub heads: [f32; 2],
    /// Where it was, oldest first (its inertia trail).
    pub trail: Vec<[f32; 2]>,
    /// Where the song is along the path, 0…1 of its loop, while it plays.
    pub path_at: Option<f32>,
    /// What a drag on the pad does now (the hand, drawing a path, recording one).
    pub mode: PadMode,
    /// The path being drawn or recorded, while the drag goes on (0…1 each, y up).
    pub stroke: Vec<[f32; 2]>,
    /// The first row the links list shows (it scrolls when there are more than fit).
    pub links_first: usize,
    /// The loop a path is drawn over while the pad has none (the Loop card's value; 0 is
    /// the default four beats).
    pub loop_beats: f32,
}

/// What a drag on the pad does.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum PadMode {
    /// Moves the puck (the pad's X and Y).
    #[default]
    Hand,
    /// Draws a path: the drag's points over one loop.
    Draw,
    /// Records a path while the song plays: the drag in song time.
    Record,
}

/// Where the face's parts were drawn this frame (the window's tests, its hints).
#[derive(Clone, Debug)]
pub struct Rects {
    /// The touch surface.
    pub pad: Rect,
    /// The *Draw path* and *Record path* keys on the glass.
    pub draw: Rect,
    pub record: Rect,
    /// The links list.
    pub links: Rect,
    /// The cards' items.
    pub items: art::ItemRects,
}

impl Default for Rects {
    fn default() -> Self {
        Rects {
            pad: Rect::NOTHING,
            draw: Rect::NOTHING,
            record: Rect::NOTHING,
            links: Rect::NOTHING,
            items: Vec::new(),
        }
    }
}

/// One parameter the pad drives, as the links list shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    pub name: String,
    pub axis: u8,
    pub range: Option<RouteRange>,
    pub depth: f32,
}

/// What the user did on the face.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Action {
    /// The hand on the pad (x, y), and its end.
    Pad(f32, f32),
    PadDone,
    /// A card's control.
    Card(Moved),
    /// *Draw path* or *Record path* switched (to `Hand` when it was on).
    Mode(PadMode),
    /// The links list scrolled to show from this row.
    LinksAt(usize),
}

/// The cards: the pad (its hand, its polarity, its amount) and its motion.
pub const CARDS: [Card<'static>; 2] = [
    Card {
        title: "fxd.xy.card.pad",
        items: &[
            Item::Knob("x", "fxd.xy.card.x"),
            Item::Knob("y", "fxd.xy.card.y"),
            Item::Switch(
                "pol",
                "fxd.card.polarity",
                &["fxd.card.bipolar", "fxd.card.unipolar"],
            ),
            Item::Knob("amount", "fxd.card.amount"),
        ],
    },
    Card {
        title: "fxd.xy.card.motion",
        items: &[
            Item::Knob("inertia", "fxd.xy.inertia"),
            Item::Switch(
                "path",
                "fxd.xy.path",
                &["fxd.card.off", "fxd.xy.path.plays"],
            ),
            Item::Knob("loop", "fxd.xy.loop"),
            Item::Switch(
                "line",
                "fxd.xy.line",
                &["fxd.xy.line.straight", "fxd.xy.line.smooth"],
            ),
        ],
    },
];

/// The display and its column, 100 %.
pub const PAD: f32 = 470.0;
pub const SIDE: f32 = 500.0;
const RAIL: f32 = 26.0;
/// Parameters an axis names on its rail; the rest are counted.
const RAIL_NAMES: usize = 2;
const MARGIN: f32 = 14.0;

/// The body's size at 100 % for the words of the current language: the window grows when
/// the cards take more rows, the display never shrinks.
pub fn body_size(ui: &egui::Ui) -> Vec2 {
    let w = PAD + 2.0 * RAIL + MARGIN * 3.0 + SIDE;
    let cards = art::cards_height(ui, &CARDS, w - 2.0 * MARGIN);
    vec2(w, MARGIN * 3.0 + PAD + 2.0 * RAIL + cards)
}

/// The face in `rect`: what the user did, and where its parts were drawn.
pub fn body(
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    src: &XySource,
    amount: f32,
    links: &[Link],
    live: &Live,
) -> (Vec<Action>, Rects) {
    let k = s.k;
    let mut out = Vec::new();
    let mut rects = Rects::default();
    let outer = Rect::from_min_size(
        rect.min + vec2(MARGIN, MARGIN) * k,
        Vec2::splat((PAD + 2.0 * RAIL) * k),
    );
    let side = Rect::from_min_max(
        pos2(outer.right() + MARGIN * k, outer.top()),
        pos2(rect.right() - MARGIN * k, outer.bottom()),
    );
    let mut b = Brush::default();
    b.begin(ui.ctx());
    let open = art::frame(&mut b, s, outer, RAIL * k);
    // The maker's line engraved on the top rail.
    let top = Rect::from_min_max(
        pos2(outer.left() + 30.0 * k, outer.top() + 2.0 * k),
        pos2(outer.right() - 30.0 * k, open.top() - 4.0 * k),
    );
    art::label(
        &mut b,
        s,
        top,
        Src::Key("fxd.xy.engraving"),
        Role::Micro,
        art::metal(s).engrave,
        Align2::CENTER_CENTER,
    );
    surface(&mut b, s, open, src, live);
    rails(&mut b, s, outer, open, links);
    // The side: the links sized to their rows (scrolling past the most that fit), the
    // flywheel taking the rest.
    let list_h = art::links_height(links.len(), side.height() / k * 0.5) * k;
    let (wheel, list) = side.split_top_bottom_at_y(side.bottom() - list_h);
    flywheel(&mut b, s, wheel.shrink2(vec2(0.0, 4.0 * k)), src, live);
    let list = list.shrink2(vec2(0.0, 4.0 * k));
    let fit = links_panel(ui.ctx(), &mut b, s, list, links, true, live.links_first);
    rects.links = list;
    // The keys on the glass: draw a path, record one.
    let (draw, record) = mode_keys(open, k);
    let hot = |r: Rect| ui.rect_contains_pointer(r);
    mode_key(
        &mut b,
        s,
        draw,
        Glyph::LanePencil,
        "fxd.xy.draw",
        live.mode == PadMode::Draw,
        hot(draw),
        false,
    );
    mode_key(
        &mut b,
        s,
        record,
        Glyph::Record,
        "fxd.xy.record",
        live.mode == PadMode::Record,
        hot(record),
        true,
    );
    b.flush(ui.painter());
    rects.pad = open;
    rects.draw = draw;
    rects.record = record;
    // The hand on the pad (a press on the keys is theirs).
    let resp = ui.interact(
        open,
        ui.id().with("fxd-xy-pad"),
        egui::Sense::click_and_drag(),
    );
    let on_keys = ui
        .input(|i| i.pointer.press_origin())
        .is_some_and(|p| draw.contains(p) || record.contains(p));
    if !on_keys {
        if let Some(p) = resp
            .interact_pointer_pos()
            .filter(|_| resp.dragged() || resp.clicked())
        {
            let x = ((p.x - open.left()) / open.width()).clamp(0.0, 1.0);
            let y = ((open.bottom() - p.y) / open.height()).clamp(0.0, 1.0);
            out.push(Action::Pad(x, y));
        }
        if resp.drag_stopped() || resp.clicked() {
            out.push(Action::PadDone);
        }
    }
    for (r, mode, id) in [
        (draw, PadMode::Draw, "fxd-xy-draw"),
        (record, PadMode::Record, "fxd-xy-record"),
    ] {
        let kr = ui.interact(r, ui.id().with(id), egui::Sense::click());
        if kr.hovered() {
            let key = if mode == PadMode::Draw {
                "fxd.xy.draw"
            } else {
                "fxd.xy.record"
            };
            crate::kit::hint::set_text(
                ui.ctx(),
                super::frame::tr(key),
                super::frame::tr(if mode == PadMode::Draw {
                    "fxd.xy.draw.tip"
                } else {
                    "fxd.xy.record.tip"
                }),
                crate::kit::hint::Caps::NONE,
            );
        }
        if kr.clicked() {
            out.push(Action::Mode(if live.mode == mode {
                PadMode::Hand
            } else {
                mode
            }));
        }
    }
    // The wheel over the list scrolls it a row at a time.
    if let Some(at) = scroll_links(ui, list, links.len(), fit, live.links_first) {
        out.push(Action::LinksAt(at));
    }
    // The cards under.
    let cards = Rect::from_min_max(
        pos2(rect.left() + MARGIN * k, outer.bottom() + MARGIN * k),
        pos2(rect.right() - MARGIN * k, rect.bottom() - MARGIN * k),
    );
    let path = src.path.as_ref();
    let pct = |v: f32, b: &mut String| {
        use std::fmt::Write as _;
        let _ = write!(b, "{:.0} %", v * 100.0);
    };
    let ms = |v: f32, b: &mut String| {
        use std::fmt::Write as _;
        let _ = write!(b, "{v:.0} ms");
    };
    let beats = |v: f32, b: &mut String| {
        use std::fmt::Write as _;
        let _ = write!(b, "{v}");
    };
    let values = |c: usize, i: usize| -> Value {
        match (c, i) {
            (0, 0) => Value::Knob {
                v: src.x,
                lo: 0.0,
                hi: 1.0,
                def: 0.5,
                fmt: &pct,
            },
            (0, 1) => Value::Knob {
                v: src.y,
                lo: 0.0,
                hi: 1.0,
                def: 0.5,
                fmt: &pct,
            },
            (0, 2) => Value::Choice(usize::from(src.unipolar)),
            (0, _) => Value::Knob {
                v: amount,
                lo: 0.0,
                hi: 1.0,
                def: 1.0,
                fmt: &pct,
            },
            (1, 0) => Value::Knob {
                v: src.inertia_ms,
                lo: 0.0,
                hi: 2_000.0,
                def: 0.0,
                fmt: &ms,
            },
            (1, 1) => Value::Choice(usize::from(src.play_path)),
            (1, 2) => Value::Knob {
                v: path.map_or(
                    if live.loop_beats > 0.0 {
                        live.loop_beats
                    } else {
                        4.0
                    },
                    |p| p.beats,
                ),
                lo: 0.25,
                hi: 64.0,
                def: 4.0,
                fmt: &beats,
            },
            _ => Value::Choice(usize::from(path.is_none_or(|p| p.smooth))),
        }
    };
    out.extend(
        art::lay_cards(ui, s, cards, &CARDS, &values, &mut rects.items)
            .into_iter()
            .map(Action::Card),
    );
    (out, rects)
}

/// A point of the pad (0…1, y up) in `r`.
fn at(r: Rect, p: [f32; 2]) -> Pos2 {
    pos2(r.left() + p[0] * r.width(), r.bottom() - p[1] * r.height())
}

/// The touch surface: the glass, its grid, the path, the trail, the crosshair, the puck.
fn surface(b: &mut Brush, s: &Style, r: Rect, src: &XySource, live: &Live) {
    let k = s.k;
    let (lit, lit2) = (art::lit(s), art::lit2(s));
    b.rrect(r, 3.0 * k, Fill::Solid(art::ground(s)));
    // The glow under the puck, wide and faint: the glass lit from where the hand is.
    let pc = at(r, live.puck);
    b.radial(
        r,
        pc,
        Vec2::splat(r.width() * 0.45),
        &[(0.0, with_alpha(lit, 0.16)), (1.0, with_alpha(lit, 0.0))],
        8,
    );
    // The grid: tenths, the halves stronger, a fine sub-grid of fortieths near the puck.
    for i in 1..40 {
        let f = i as f32 / 40.0;
        let (major, half) = (i % 4 == 0, i == 20);
        let a: f32 = if half {
            0.30
        } else if major {
            0.14
        } else {
            0.0
        };
        let near = |c: f32| (1.0 - (c - f).abs() * 6.0).clamp(0.0, 1.0);
        let ax = a.max(0.07 * near(live.puck[0]));
        let ay = a.max(0.07 * near(live.puck[1]));
        let w = if half { 0.9 } else { 0.6 } * k;
        if ax > 0.0 {
            let x = r.left() + f * r.width();
            b.vline(x, r.top(), r.bottom(), w, with_alpha(lit, ax));
        }
        if ay > 0.0 {
            let y = r.bottom() - f * r.height();
            b.hline(r.left(), r.right(), y, w, with_alpha(lit, ay));
        }
    }
    // The centre: the parameters' own values on a bipolar pad, a small ring.
    let centre = if src.unipolar {
        at(r, [0.0, 0.0])
    } else {
        r.center()
    };
    b.circle_stroke(centre, 5.0 * k, 0.8 * k, with_alpha(lit, 0.45));
    // The path: a lit loop, its direction marked, where the song is a bright bead.
    if let Some(path) = &src.path {
        let n = 160;
        let pts: Vec<Pos2> = (0..=n)
            .map(|i| at(r, path.at(f64::from(path.beats) * i as f64 / n as f64)))
            .collect();
        let a = if src.play_path { 0.85 } else { 0.35 };
        if src.play_path {
            b.glow_polyline(
                &pts,
                1.3 * k,
                with_alpha(lit2, a),
                with_alpha(lit2, 0.18),
                5.0 * k,
            );
        } else {
            b.dashed(&pts, 1.0 * k, with_alpha(lit2, a), 5.0 * k, 4.0 * k);
        }
        for q in [0.125f32, 0.375, 0.625, 0.875] {
            let p0 = at(r, path.at(f64::from(path.beats * q)));
            let p1 = at(r, path.at(f64::from(path.beats * (q + 0.01))));
            let d = (p1 - p0).normalized();
            let n2 = vec2(-d.y, d.x);
            let tip = p0 + d * 5.0 * k;
            b.polygon(
                &[
                    tip,
                    p0 - d * 2.0 * k + n2 * 3.5 * k,
                    p0 - d * 2.0 * k - n2 * 3.5 * k,
                ],
                with_alpha(lit2, a),
            );
        }
        if let (Some(t), true) = (live.path_at, src.play_path) {
            let p = at(r, path.at(f64::from(path.beats * t)));
            b.circle(p, 3.2 * k, Fill::Solid(lit2));
            b.circle_stroke(p, 6.0 * k, 0.8 * k, with_alpha(lit2, 0.5));
        }
    }
    // The path being drawn or recorded, as the hand lays it (recording in record red).
    if live.stroke.len() > 1 {
        let pts: Vec<Pos2> = live.stroke.iter().map(|p| at(r, *p)).collect();
        let c = if live.mode == PadMode::Record {
            s.t.danger
        } else {
            lit2
        };
        b.glow_polyline(&pts, 1.6 * k, c, with_alpha(c, 0.25), 6.0 * k);
    }
    // The inertia trail: older is fainter and thinner.
    let m = live.trail.len();
    if m > 1 {
        for (i, w) in live.trail.windows(2).enumerate() {
            let f = (i + 1) as f32 / m as f32;
            b.segment(
                at(r, w[0]),
                at(r, w[1]),
                (0.6 + 2.6 * f) * k,
                with_alpha(lit, 0.08 + 0.5 * f * f),
                true,
            );
        }
    }
    // Where the hand set it: a crosshair, joined to the puck by a fine line while it glides.
    let h = at(r, live.heads);
    if (h - pc).length() > 1.0 {
        b.segment(pc, h, 0.6 * k, with_alpha(s.t.light, 0.35), false);
    }
    let cl = with_alpha(s.t.light, 0.75);
    for d in [
        vec2(1.0, 0.0),
        vec2(-1.0, 0.0),
        vec2(0.0, 1.0),
        vec2(0.0, -1.0),
    ] {
        b.segment(h + d * 4.0 * k, h + d * 10.0 * k, 0.9 * k, cl, false);
    }
    puck(b, s, pc);
    // Its coordinates, in the corner, mono.
    let txt = format!("X {:.2}   Y {:.2}", live.puck[0], live.puck[1]);
    let c = Rect::from_min_size(
        r.right_top() + vec2(-150.0 * k, 8.0 * k),
        vec2(140.0 * k, 16.0 * k),
    );
    art::value(
        b,
        s,
        c,
        &txt,
        Role::Micro,
        with_alpha(lit, 0.85),
        Align2::RIGHT_CENTER,
    );
    art::glass(b, s, r);
}

/// The puck: a lit glass disc in a knurled metal ring, its shadow on the glass.
fn puck(b: &mut Brush, s: &Style, p: Pos2) {
    let k = s.k;
    let lit = art::lit(s);
    let m = art::metal(s);
    b.radial(
        Rect::from_center_size(p, Vec2::splat(80.0 * k)),
        p,
        Vec2::splat(34.0 * k),
        &[(0.0, with_alpha(lit, 0.45)), (1.0, with_alpha(lit, 0.0))],
        6,
    );
    b.circle(
        p + vec2(1.5, 3.0) * k,
        15.0 * k,
        Fill::Solid(with_alpha(s.t.dark_shade, 0.5)),
    );
    b.circle(p, 15.0 * k, Fill::V(m.top, m.foot));
    // Knurling: fine ticks round the ring.
    for i in 0..48 {
        let a = i as f32 / 48.0 * TAU;
        let d = vec2(cos(a), sin(a));
        b.segment(
            p + d * 12.6 * k,
            p + d * 14.6 * k,
            0.5 * k,
            with_alpha(m.edge, 0.8),
            false,
        );
    }
    b.circle(
        p,
        11.0 * k,
        Fill::V(mix(lit, s.t.light, 0.35), mix(lit, s.t.dark_shade, 0.35)),
    );
    b.circle_stroke(p, 11.0 * k, 0.8 * k, with_alpha(m.edge, 0.9));
    b.circle(
        p + vec2(-3.5, -4.0) * k,
        3.0 * k,
        Fill::Solid(with_alpha(s.t.light, 0.45)),
    );
    b.circle(p, 1.6 * k, Fill::Solid(with_alpha(s.t.light, 0.9)));
}

/// The rails: an engraved scale on the foot and the left side, and which parameters each
/// axis drives, engraved along it.
fn rails(b: &mut Brush, s: &Style, outer: Rect, open: Rect, links: &[Link]) {
    let k = s.k;
    let m = art::metal(s);
    for i in 0..=20 {
        let f = i as f32 / 20.0;
        let len = if i % 10 == 0 {
            7.0
        } else if i % 2 == 0 {
            4.5
        } else {
            2.5
        } * k;
        let x = open.left() + f * open.width();
        b.vline(
            x,
            open.bottom() + 4.0 * k,
            open.bottom() + 4.0 * k + len,
            0.7 * k,
            m.engrave,
        );
        let y = open.bottom() - f * open.height();
        b.hline(
            open.left() - 4.0 * k - len,
            open.left() - 4.0 * k,
            y,
            0.7 * k,
            m.engrave,
        );
    }
    // The first two an axis drives by name, the rest counted ("+3 more"): the rail is
    // never too short for its words.
    let names = |axis: u8| -> String {
        let v: Vec<&str> = links
            .iter()
            .filter(|l| l.axis == axis)
            .map(|l| l.name.as_str())
            .collect();
        if v.len() <= RAIL_NAMES {
            v.join(" · ")
        } else {
            let more = crate::i18n::tf(
                "fxd.rail.more",
                &[("n", &(v.len() - RAIL_NAMES).to_string())],
            );
            format!("{} · {more}", v[..RAIL_NAMES].join(" · "))
        }
    };
    let foot = Rect::from_min_max(
        pos2(open.left() + 10.0 * k, open.bottom() + 11.0 * k),
        pos2(open.right() - 10.0 * k, outer.bottom() - 2.0 * k),
    );
    let x = format!("X — {}", names(0));
    art::value(
        b,
        s,
        foot,
        &x,
        Role::Micro,
        m.engrave,
        Align2::CENTER_CENTER,
    );
    // The left rail's words read upward.
    let y = format!("Y — {}", names(1));
    let side = Rect::from_center_size(
        pos2(outer.left() + RAIL * 0.42 * k, open.center().y),
        vec2(14.0 * k, open.height() - 20.0 * k),
    );
    let t = crate::kit::text::Text::new(&y, Role::Micro, m.engrave)
        .caps(false)
        .align(Align2::CENTER_CENTER)
        .policy(crate::kit::text::Policy::Rotate90(
            crate::kit::text::Elide::End,
        ));
    let _ = crate::kit::text::paint(b, side, &t, s.tk());
}

/// The object: the inertia flywheel. Its rim's weights grow with the inertia; while the
/// puck travels, the spokes blur along their turn.
fn flywheel(b: &mut Brush, s: &Style, r: Rect, src: &XySource, live: &Live) {
    let k = s.k;
    art::well(b, s, r);
    let m = art::metal(s);
    let lit = art::lit(s);
    art::label(
        b,
        s,
        Rect::from_min_size(
            r.min + vec2(10.0, 6.0) * k,
            vec2(r.width() - 20.0 * k, 14.0 * k),
        ),
        Src::Key("fxd.xy.flywheel"),
        Role::Micro,
        s.t.text_dim,
        Align2::LEFT_CENTER,
    );
    let c = pos2(r.left() + r.width() * 0.38, r.center().y + 8.0 * k);
    let rad = (r.height() * 0.38).min(r.width() * 0.3);
    let heavy = (src.inertia_ms / 2_000.0).clamp(0.0, 1.0).sqrt();
    let speed = live.trail.windows(2).last().map_or(0.0, |w| {
        ((w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]) * 40.0).min(1.0)
    });
    let turn = (live.puck[0] * 3.0 + live.puck[1] * 2.0) * TAU;
    // The axle's bracket, then the wheel: a rim, its weights, six spokes, the hub.
    b.rrect(
        Rect::from_center_size(c + vec2(0.0, rad * 0.62), vec2(rad * 0.5, rad * 1.1)),
        4.0 * k,
        Fill::V(m.face, m.foot),
    );
    b.circle(
        c + vec2(2.0, 4.0) * k,
        rad * 1.04,
        Fill::Solid(with_alpha(s.t.dark_shade, 0.4)),
    );
    b.circle(c, rad, Fill::V(m.top, m.foot));
    b.circle(
        c,
        rad * (0.84 - 0.08 * heavy),
        Fill::Solid(mix(s.t.panel, s.t.dark_shade, 0.4)),
    );
    b.circle_stroke(c, rad * 0.92, 0.6 * k, with_alpha(m.edge, 0.8));
    for i in 0..12 {
        let a = turn + i as f32 / 12.0 * TAU;
        let d = vec2(cos(a), sin(a));
        let w = (2.0 + 5.0 * heavy) * k;
        b.circle(c + d * rad * 0.9, w * 0.5, Fill::V(m.top, m.edge));
    }
    for blur in 0..4 {
        let back = blur as f32 * 0.06 * speed;
        let alpha = if blur == 0 { 1.0 } else { 0.25 * speed };
        for i in 0..6 {
            let a = turn - back + i as f32 / 6.0 * TAU;
            let d = vec2(cos(a), sin(a));
            b.segment(
                c + d * rad * 0.2,
                c + d * rad * (0.78 - 0.08 * heavy),
                3.0 * k,
                with_alpha(m.face, alpha),
                true,
            );
            b.segment(
                c + d * rad * 0.2,
                c + d * rad * (0.78 - 0.08 * heavy),
                0.7 * k,
                with_alpha(m.top, alpha * 0.8),
                false,
            );
        }
    }
    b.circle(c, rad * 0.2, Fill::V(m.top, m.foot));
    b.circle(c, rad * 0.08, Fill::Solid(with_alpha(lit, 0.9)));
    // Its reading beside it.
    let tx = Rect::from_min_max(
        pos2(c.x + rad + 14.0 * k, r.top() + 24.0 * k),
        pos2(r.right() - 10.0 * k, r.bottom() - 10.0 * k),
    );
    let (top, rest) = tx.split_top_bottom_at_fraction(0.5);
    art::value(
        b,
        s,
        top,
        &format!("{:.0} ms", src.inertia_ms),
        Role::Display,
        lit_on_panel(s),
        Align2::LEFT_BOTTOM,
    );
    let key = if src.inertia_ms == 0.0 {
        "fxd.xy.inertia.none"
    } else {
        "fxd.xy.inertia.glides"
    };
    art::label(
        b,
        s,
        rest,
        Src::Key(key),
        Role::Micro,
        s.t.text_dim,
        Align2::LEFT_TOP,
    );
}

/// The accent where it sits on the panel (not on a display): the theme's own value.
fn lit_on_panel(s: &Style) -> egui::Color32 {
    s.hue
}

/// The keys on the glass, top left: *Draw path*, *Record path*.
fn mode_keys(open: Rect, k: f32) -> (Rect, Rect) {
    let h = 20.0 * k;
    let w = 30.0 * k;
    let draw = Rect::from_min_size(open.min + vec2(8.0, 8.0) * k, vec2(w, h));
    let record = draw.translate(vec2(w + 6.0 * k, 0.0));
    (draw, record)
}

/// A key on the glass: a dark chip with its glyph, lit while its mode is on (recording in
/// the theme's record red).
#[allow(clippy::too_many_arguments)]
fn mode_key(
    b: &mut Brush,
    s: &Style,
    r: Rect,
    g: Glyph,
    _key: &str,
    on: bool,
    hot: bool,
    rec: bool,
) {
    let k = s.k;
    let lit = if rec { s.t.danger } else { art::lit(s) };
    let ground = art::ground(s);
    let face = if on {
        mix(ground, lit, 0.35)
    } else if hot {
        mix(ground, s.t.light, 0.10)
    } else {
        mix(ground, s.t.light, 0.04)
    };
    b.rrect(r, 4.0 * k, Fill::Solid(face));
    b.rrect_stroke(
        r,
        4.0 * k,
        0.8 * k,
        with_alpha(
            if on { lit } else { s.t.light },
            if on { 0.9 } else { 0.25 },
        ),
    );
    let ink = if on {
        mix(lit, s.t.light, 0.5)
    } else {
        with_alpha(s.t.light, if hot { 0.9 } else { 0.6 })
    };
    let gr = Rect::from_center_size(r.center(), Vec2::splat(12.0 * k));
    if rec {
        // The record key: a round lamp.
        b.circle(
            gr.center(),
            4.5 * k,
            Fill::Solid(if on { lit } else { ink }),
        );
    } else {
        crate::kit::glyph::paint(b, gr, g, ink);
    }
}

/// The wheel over a links list: the row to show from, when it moved.
pub fn scroll_links(
    ui: &egui::Ui,
    list: Rect,
    n: usize,
    fit: usize,
    first: usize,
) -> Option<usize> {
    if n <= fit || !ui.rect_contains_pointer(list) {
        return None;
    }
    // The wheel's own steps (not the smoothed scroll): one row a notch.
    let dy: f32 = ui.input(|i| {
        i.events
            .iter()
            .map(|e| match e {
                egui::Event::MouseWheel { delta, .. } => delta.y,
                _ => 0.0,
            })
            .sum()
    });
    if dy == 0.0 {
        return None;
    }
    let max = n - fit;
    let at = if dy < 0.0 {
        (first + 1).min(max)
    } else {
        first.saturating_sub(1).min(max)
    };
    (at != first).then_some(at)
}

/// The links: each parameter, its axis, its range drawn as a bar over the axis's travel;
/// from row `first` on, as many as fit, a thin scroll bar at the side when not all do.
/// Returns how many rows fit.
#[allow(clippy::too_many_arguments)]
pub fn links_panel(
    ctx: &egui::Context,
    b: &mut Brush,
    s: &Style,
    r: Rect,
    links: &[Link],
    axes: bool,
    first: usize,
) -> usize {
    let k = s.k;
    art::well(b, s, r);
    art::label(
        b,
        s,
        Rect::from_min_size(
            r.min + vec2(10.0, 6.0) * k,
            vec2(r.width() - 20.0 * k, 14.0 * k),
        ),
        Src::Key("fxd.links"),
        Role::Micro,
        s.t.text_dim,
        Align2::LEFT_CENTER,
    );
    let row = art::LINK_ROW * k;
    let fit =
        (((r.height() - art::LINKS_HEAD * k - art::LINKS_FOOT * k) / row).floor() as usize).max(1);
    let first = first.min(links.len().saturating_sub(fit));
    let scrolls = links.len() > fit;
    let bar_room = if scrolls { 10.0 * k } else { 0.0 };
    for (i, l) in links.iter().enumerate().skip(first).take(fit) {
        let y = r.top() + art::LINKS_HEAD * k + (i - first) as f32 * row;
        let line = Rect::from_min_size(
            pos2(r.left() + 10.0 * k, y),
            vec2(r.width() - 20.0 * k - bar_room, row - 4.0 * k),
        );
        let axis = if l.axis == 0 { "X" } else { "Y" };
        let tag = Rect::from_min_size(line.min, vec2(if axes { 20.0 } else { 0.0 } * k, 16.0 * k));
        if axes {
            b.rrect(tag, 3.0 * k, Fill::Solid(with_alpha(s.hue, 0.25)));
            art::value(
                b,
                s,
                tag,
                axis,
                Role::Micro,
                s.t.text,
                Align2::CENTER_CENTER,
            );
        }
        let name = Rect::from_min_max(
            pos2(tag.right() + 6.0 * k, line.top()),
            pos2(line.right(), line.top() + 16.0 * k),
        );
        art::value(
            b,
            s,
            name,
            &l.name,
            Role::Label,
            s.t.text,
            Align2::LEFT_CENTER,
        );
        // The bar: the parameter's travel, the share the route sweeps, its bend.
        let bar = Rect::from_min_max(
            pos2(tag.right() + 6.0 * k, line.top() + 19.0 * k),
            pos2(line.right(), line.top() + 23.0 * k),
        );
        b.rrect(
            bar,
            2.0 * k,
            Fill::Solid(mix(s.t.panel, s.t.dark_shade, 0.4)),
        );
        let g = l.range.unwrap_or(RouteRange {
            from: -1.0,
            to: 1.0,
            curve: 0.0,
        });
        let to_x = |v: f32| bar.left() + (0.5 + 0.5 * (v * l.depth).clamp(-1.0, 1.0)) * bar.width();
        let (a, z) = (to_x(g.from), to_x(g.to));
        b.rrect(
            Rect::from_min_max(pos2(a.min(z), bar.top()), pos2(a.max(z), bar.bottom())),
            2.0 * k,
            Fill::Solid(s.hue),
        );
        b.vline(
            bar.center().x,
            bar.top() - 2.0 * k,
            bar.bottom() + 2.0 * k,
            0.7 * k,
            s.t.text_dim,
        );
    }
    if scrolls {
        // The scroll bar: a fine track, the thumb the share shown.
        let track = Rect::from_min_max(
            pos2(r.right() - 9.0 * k, r.top() + art::LINKS_HEAD * k),
            pos2(r.right() - 6.0 * k, r.bottom() - art::LINKS_FOOT * k),
        );
        b.rrect(
            track,
            1.5 * k,
            Fill::Solid(mix(s.t.panel, s.t.dark_shade, 0.35)),
        );
        let n = links.len() as f32;
        let t0 = track.top() + track.height() * first as f32 / n;
        let t1 = track.top() + track.height() * (first + fit).min(links.len()) as f32 / n;
        b.rrect(
            Rect::from_min_max(pos2(track.left(), t0), pos2(track.right(), t1)),
            1.5 * k,
            Fill::Solid(with_alpha(s.hue, 0.8)),
        );
    }
    if links.is_empty() {
        // What to do, in as many lines as the well's width needs.
        let room = Rect::from_min_max(
            pos2(r.left() + 12.0 * k, r.top() + art::LINKS_HEAD * k),
            pos2(r.right() - 12.0 * k, r.bottom() - art::LINKS_FOOT * k),
        );
        let words = crate::i18n::t("fxd.links.none");
        let lines = crate::kit::prose::wrap(ctx, &words, Role::Body, k, room.width());
        let _ = crate::kit::prose::paint(
            b,
            room,
            &lines,
            Role::Body,
            k,
            s.t.text_dim,
            Align2::CENTER_CENTER,
        );
    }
    fit
}
