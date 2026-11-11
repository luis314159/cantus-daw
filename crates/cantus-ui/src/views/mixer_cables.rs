//! The cable band under the strips (round 25, ADR 0041; round 39's look, des-mixer's
//! `options-cables-*`): every strip's two jacks, the routes as cables in the user's style
//! (`Settings::cable_style`: *Patch*, *Flow* — the default — or *Subway*, `mixer_wires`),
//! level pills, edge chips for ends scrolled out of view, the hit test; and the hatching
//! of a column a drag cannot drop on.
//!
//! **What is drawn.** Nothing at rest (the owner's option C); the header's Cables key
//! shows the selection's cables, in and out; *Show all* draws every route — the others at
//! 45 % — and a strip that receives (a bus, a return) its way to the master, so a chain
//! reads end to end. Hovering a cable lights its whole route to the master (Kick → Drum
//! bus → Master), drops everything else to 22 % and names the route by the pointer and in
//! the hint bar. A send's width says its level; a key is amber and dashed.
//!
//! **Flow.** While the transport plays and a cable's source has signal, a gleam travels
//! along it, its speed and brightness by the level; its place comes from the transport's
//! time. It asks for no frame of its own: it rides the meters' (none at rest).
//!
//! **What it does.** An output jack starts a drag (`mixer_drag`: plain = send, Ctrl =
//! output, Alt = key); an input jack a lit cable plugs into moves that route's end; a click
//! selects a cable (Delete removes it, Esc lets it go); right-click opens the route menu;
//! dragging a pill sets the send's level (double-click: 0 dB); an edge chip scrolls its
//! strip into view.
//!
//! **Cost.** The routes, the places and the doubled flags come from the desk's
//! per-revision cache; a frame flattens each drawn cable once into the band's point list
//! (24 segments a curve), strokes into the desk's one brush, keeps its lists' capacity and
//! formats into the view's buffers — no allocation, no draw call of its own.

use super::desk::{Place, MASTER_END};
use super::drag::{self, Source, Target};
use super::layout::{CableOpts, CableView, Row, WidthMode};
use super::strip::{knob_input, pid, sense, strip_color, Part, Pass};
use super::wires::{self, Kind, Tokens};
use super::{in_selection, route_owner, Geo, MixerView};
use crate::app::CantusApp;
use crate::kit::color::{on_fill, shade, with_alpha};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Role, Text};
use crate::kit::{Brush, Fill, Style, Tok};
use crate::settings::CableStyle;
use cantus_core::routing::{Edge, EdgeKind, End, Route};
use cantus_core::{Dock, Send, StripId};
use egui::{pos2, vec2, Align2, Key, Pos2, Rect, Sense, Ui};
use std::fmt::Write as _;

/// A pill's height.
const PILL_H: f32 = 13.0;
/// An edge chip's size.
const CHIP: egui::Vec2 = vec2(30.0, 13.0);
/// A jack's hit area's half width and height.
const JACK_HIT: f32 = 6.0;
/// The other strips' cables with *Show all* on, and everything off a hovered route.
const OTHERS: f32 = 0.45;
const DIMMED: f32 = 0.22;
/// The longest chain a hovered route follows to the master.
const CHAIN_MAX: usize = 16;

/// One cable as the band drew it this frame (`MixerView::cables` shows them to tests).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Drawn {
    pub route: Route,
    /// Its two ends on screen: the source's output jack and the target's input jack (a
    /// scrolled-out end: its edge chip).
    pub from: Pos2,
    pub to: Pos2,
    /// Patch and Flow: the sag; Subway: the lane's depth below the jacks.
    pub depth: f32,
    /// The selection's (or the hovered strip's, or the selected cable).
    pub lit: bool,
    /// On the hovered route (the hovered cable and its way to the master).
    pub chain: bool,
    /// Drawn only because of the pointer — the hovered strip's, or on the hovered route
    /// (it takes no lane of its own: Subway's lanes never move under the pointer).
    pub hover_only: bool,
    /// How opaque it was drawn: 1, 0.45 (another strip's, *Show all*), 0.22 (off a
    /// hovered route).
    pub alpha: f32,
    pub kind: Kind,
    /// Its stroke's width.
    pub width: f32,
    /// Its signal now, 0…1 (the source's meter, and a send's level).
    pub level: f32,
    /// The source's and the destination's end is an edge chip.
    pub chips: [bool; 2],
    /// Its index in the desk's edges.
    pub edge: u32,
    /// Its points in the band's list (`MixerView::cable_path`).
    pub path: (u32, u32),
}

/// The band's state between frames.
#[derive(Debug, Default)]
pub(crate) struct Band {
    /// The cable a click selected.
    pub selected: Option<Route>,
    /// What was drawn this frame (capacity kept).
    pub drawn: Vec<Drawn>,
    /// The egui pass the band was last drawn on (a hidden desk draws nothing: its
    /// selected cable is not something Delete may remove).
    pub pass: u64,
    /// The pass the keymap last ran on (set by `CantusApp::global_shortcuts`).
    pub keys_pass: u64,
    /// Where pills went this frame, so none overlaps another.
    pills: Vec<Rect>,
    /// Every drawn cable's points, flattened.
    pub pts: Vec<egui::Pos2>,
    /// The cable under the pointer last frame, whose route to the master is drawn.
    pub hot: Option<Route>,
    /// The strips on that route after its target, to the master.
    chain: Vec<StripId>,
    /// The gleam's pieces drawn this frame (Flow; tests read it).
    pub gleams: u32,
    /// The channel's depth the cables asked for (Subway's lanes), for the next layout.
    pub channel: f32,
    /// Scratch: the subway's spans, order, sub-lanes and lanes; each jack's colour rank;
    /// the strips that receive; the input jacks sensed.
    spans: Vec<(Kind, f32, f32)>,
    order: Vec<u32>,
    sub: Vec<u16>,
    lanes: Vec<f32>,
    jack: Vec<[u8; 2]>,
    receives: Vec<bool>,
    sensed: Vec<f32>,
    /// D: each route's catenary parameter, by its ends and sag (solved once, kept while
    /// they hold); the routes a drop just made, settling since when; the routes before a
    /// drag; whether a drag was on last frame; a shifted copy of a cord (its shadow).
    cat: Vec<(Route, [u32; 4], f32)>,
    settle: Vec<(Route, f64)>,
    known: Vec<Route>,
    was_dragging: bool,
    tmp: Vec<Pos2>,
}

impl Band {
    /// Where this frame's pills went.
    pub fn pills(&self) -> &[Rect] {
        &self.pills
    }

    /// A drawn cable's points.
    pub fn path(&self, d: &Drawn) -> &[Pos2] {
        self.pts
            .get(d.path.0 as usize..d.path.1 as usize)
            .unwrap_or(&[])
    }
}

/// The route an edge is.
pub(crate) fn route_of(e: &Edge) -> Route {
    match e.kind {
        EdgeKind::Output => Route::output(e.from, e.to),
        EdgeKind::Send { .. } => Route::send(e.from, e.to),
        EdgeKind::Key { effect } => Route::key(e.from, e.to, effect),
    }
}

fn kind_of(e: &Edge) -> Kind {
    match e.kind {
        EdgeKind::Output => Kind::Output,
        EdgeKind::Send { .. } => Kind::Send,
        EdgeKind::Key { .. } => Kind::Key,
    }
}

/// Where the strips are this frame: each dock's first item, the master's centre and
/// width, the strip pitch, and the middle's visible span.
struct Ports {
    left: f32,
    middle: f32,
    right: f32,
    master: f32,
    master_w: f32,
    pitch: f32,
    shown: (f32, f32),
}

impl Ports {
    fn new(v: &MixerView, geo: &Geo) -> Ports {
        Ports {
            left: geo.left.left() + geo.master_w,
            middle: geo.middle.left() - v.scroll_x,
            right: geo.right.left(),
            master: geo.left.left() + geo.master_w * 0.5,
            master_w: geo.master_w,
            pitch: geo.pitch,
            shown: (geo.middle.left() + 2.0, geo.middle.right() - 2.0),
        }
    }

    /// A strip's centre, its two jacks' x, and whether it shows (pinned docks always do;
    /// `None` is the master). A folded group's members share one point at its separator.
    fn at(&self, place: Option<Place>) -> (f32, (f32, f32), bool) {
        let Some(p) = place else {
            return (self.master, wires::jacks(self.master, self.master_w), true);
        };
        let origin = match p.dock {
            Dock::Left => self.left,
            Dock::Middle => self.middle,
            Dock::Right => self.right,
        };
        let x = origin + p.x(self.pitch);
        let shows = p.dock != Dock::Middle || (x >= self.shown.0 && x <= self.shown.1);
        let j = if p.folded {
            (x, x)
        } else {
            wires::jacks(x, self.pitch)
        };
        (x, j, shows)
    }

    /// Where an end scrolled out of the middle shows: a chip at the side it went.
    fn chip_x(&self, x: f32) -> f32 {
        if x < self.shown.0 {
            self.shown.0 + CHIP.x * 0.5
        } else {
            self.shown.1 - CHIP.x * 0.5
        }
    }
}

/// The band: the channel, cables, jacks, pills, chips, across the whole desk width.
pub(super) fn band(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &Pass,
    geo: &Geo,
) {
    v.band.drawn.clear();
    v.band.pills.clear();
    v.band.pts.clear();
    v.band.gleams = 0;
    v.band.pass = ui.ctx().cumulative_pass_nr();
    let Some((a, z)) = pass.row(Row::Cables) else {
        v.band.selected = None;
        v.band.hot = None;
        return;
    };
    let view = pass.prefs.cables;
    if view == CableView::Off {
        // Nothing shows: nothing stays selected (Delete must not remove a hidden cable).
        v.band.selected = None;
        v.band.hot = None;
        return;
    }
    let s = pass.s;
    let style = app.settings.cable_style_in(&s.t.system);
    let jack_style = style == CableStyle::Jack;
    let area =
        Rect::from_min_max(pos2(geo.left.left(), a), pos2(geo.inner_right, z)).intersect(geo.rows);
    if area.height() <= 1.0 {
        return;
    }
    b.register(area, "cable_band", BAND_TOKENS, &[]);
    let tok = wires::tokens(&pass.pal, s.t.ink);
    // The jacks: in the strip's foot row (A–C), on its bottom edge (D, whose plugs hang
    // below it); the cables end at the jack, or at a seated plug's boot.
    let jy = if jack_style {
        a + wires::JACK_ROW
    } else {
        a + wires::JACK_Y
    };
    let ey = if jack_style { jy + wires::PLUG_LEN } else { jy };
    let now = ui.input(|i| i.time);
    // The channel under the strips: the cables' ground.
    let channel = Rect::from_min_max(
        pos2(area.left() + 2.0, a + wires::JACK_ROW + 2.0),
        pos2(area.right() - 2.0, z - 2.0),
    );
    if channel.height() > 2.0 {
        b.rrect(b.snap_rect(channel), 4.0, Fill::Solid(tok.channel));
    }
    let ports = Ports::new(v, geo);
    if let Some(sel) = v.band.selected {
        if !v.desk.edges.iter().any(|e| route_of(e) == sel) {
            v.band.selected = None;
        }
    }
    let pointer = ui.ctx().pointer_hover_pos();
    let dragging = drag::active(v);
    let hovered_strip = pointer
        .filter(|p| geo.rows.contains(*p))
        .and_then(|p| super::paint::strip_at(v, geo, p));
    let opts = pass.prefs.cable_opts;
    settling(v, dragging, now);
    chain_of(v, app);
    collect(v, app, &ports, view, opts, hovered_strip, ey, jack_style);
    if let Some(sel) = v.band.selected {
        // A selected cable scrolled out of sight (or no longer drawn) is let go.
        if !v.band.drawn.iter().any(|d| d.route == sel) {
            v.band.selected = None;
        }
    }
    shape(v, style, ey, z - 3.0, now);

    // The cable under the pointer: the nearest within reach; the one drawn on top wins a
    // tie. Its whole route is drawn from the next frame on (asked for once, on a change).
    let in_band = pointer.filter(|p| area.contains(*p) && !dragging);
    let hot = in_band.and_then(|p| hit(v, p));
    let hot_route = hot.map(|k| v.band.drawn[k].route);
    if hot_route != v.band.hot {
        v.band.hot = hot_route;
        ui.ctx().request_repaint();
    }
    // How opaque each is: a hovered route lit and the rest dimmed; else the selection's
    // whole and the others' (Show all) at 45 %.
    for k in 0..v.band.drawn.len() {
        let d = &mut v.band.drawn[k];
        let on_route = hot == Some(k) || (hot.is_some() && d.chain);
        d.alpha = match hot {
            Some(_) if on_route => 1.0,
            Some(_) => DIMMED,
            None if d.lit || d.chain => 1.0,
            None => OTHERS,
        };
        if hot.is_some() && on_route {
            d.width += 0.5;
        }
    }
    let band = ui.interact(area, egui::Id::new("mixer-cable-band"), Sense::click());
    paint(v, app, b, &tok, style, hot);
    // The jacks first: the chips and pills sensed after them are on top.
    jack_row(v, app, ui, b, pass, geo, &ports, &tok, jy, jack_style);
    chips(v, app, ui, b, pass, geo, jy, z);
    // The send levels are a detail the options can leave out: then no pill is drawn or
    // grabbed (the level stays in the hint bar, the send row and the inspector).
    if opts.levels {
        pills(v, app, ui, b, pass, hot, jy, (area.left(), area.right()));
    }
    ends(v, app, ui, pass, jack_style);
    interact(v, app, ui, &band, hot);
    if let (Some(k), Some(p)) = (hot, in_band) {
        route_tip(v, app, b, s, geo, k, p);
    }
    // D: a dropped cord settles for 300 ms, a frame each tick, then nothing moves.
    if jack_style && !v.band.settle.is_empty() {
        crate::views::workspace_ticker::moving(
            ui.ctx(),
            crate::views::workspace_ticker::Reason::Animation,
        );
    }
}

/// The routes a drop just made start settling (D): the desk's routes are noted when a
/// drag starts, and those new when it ends settle from then on, for 300 ms.
fn settling(v: &mut MixerView, dragging: bool, now: f64) {
    let band = &mut v.band;
    if dragging && !band.was_dragging {
        band.known.clear();
        band.known.extend(v.desk.edges.iter().map(route_of));
    } else if !dragging && band.was_dragging {
        for e in &v.desk.edges {
            let r = route_of(e);
            if !band.known.contains(&r) {
                band.settle.push((r, now));
            }
        }
    }
    band.was_dragging = dragging;
    band.settle
        .retain(|(_, t0)| ((now - t0) * 1000.0) < f64::from(wires::SETTLE_MS));
}

/// The strips on the hovered route after its target, to the master (`Band::chain`).
fn chain_of(v: &mut MixerView, app: &CantusApp) {
    v.band.chain.clear();
    let Some(r) = v.band.hot else {
        return;
    };
    let p = app.project();
    let mut at = r.to;
    while !at.is_master() && v.band.chain.len() < CHAIN_MAX && !v.band.chain.contains(&at) {
        v.band.chain.push(at);
        match p.strip(at).map(|s| s.output) {
            Some(Some(next)) => at = next,
            _ => break,
        }
    }
}

/// The cables this frame draws, and where: the selection's (and the hovered strip's and
/// the selected cable) lit, the rest at 45 % in *All*; an output to the master only for
/// the selection, a strip that receives (*All*) and the hovered route — sixty identical
/// cables into the master say nothing.
///
/// The view's options filter first: a hidden kind (or the master outputs, when left
/// out) is never drawn, so never hit-tested, selected or removed by Delete; the scope
/// decides which of the selection's cables count as its own — what leaves it, what
/// arrives at it, or both.
#[allow(clippy::too_many_arguments)]
fn collect(
    v: &mut MixerView,
    app: &CantusApp,
    ports: &Ports,
    view: CableView,
    opts: CableOpts,
    hovered: Option<StripId>,
    jy: f32,
    jack: bool,
) {
    let selected = v.band.selected;
    let scope = opts.scope;
    // Which strips receive: their way to the master is part of a chain (*All*).
    v.band.receives.clear();
    v.band.receives.resize(v.desk.place.len(), false);
    for &(fi, ti) in &v.desk.ends {
        if fi != MASTER_END {
            if let Some(r) = v.band.receives.get_mut(ti as usize) {
                *r = true;
            }
        }
    }
    let playing = app.engine.is_playing();
    for (i, e) in v.desk.edges.iter().enumerate() {
        let Some(&(fi, ti)) = v.desk.ends.get(i) else {
            continue;
        };
        if fi == MASTER_END {
            continue;
        }
        let kind_shown = match e.kind {
            EdgeKind::Output => opts.outputs,
            EdgeKind::Send { .. } => opts.sends,
            EdgeKind::Key { .. } => opts.keys,
        };
        let route = route_of(e);
        let chain = v.band.hot == Some(route)
            || (matches!(e.kind, EdgeKind::Output) && v.band.chain.contains(&e.from));
        // A hidden kind is never drawn, not even on a hovered route (so never hit, selected
        // or removed); a hovered route shows its way to the master all the same.
        if !kind_shown || (!chain && e.is_master_output() && !opts.master) {
            continue;
        }
        let to_master = ti == MASTER_END;
        let touches = scope.takes(
            in_selection(app, e.from),
            !to_master && in_selection(app, e.to),
        );
        let hov = hovered.is_some_and(|h| scope.takes(h == e.from, !to_master && h == e.to));
        let chosen = selected == Some(route);
        let lit = touches || hov || chosen;
        // Shown for `lit`: a master output only from its source's side (and in *All* from a
        // strip that receives: the end of a chain), the rest by the view.
        let receives = v.band.receives.get(fi as usize).copied().unwrap_or(false);
        let shows = |lit: bool| {
            if e.is_master_output() {
                lit || (view == CableView::All && receives)
            } else {
                match view {
                    CableView::Off => false,
                    CableView::Selected => lit,
                    CableView::All => true,
                }
            }
        };
        let shown = shows(lit);
        // What the pointer alone brings (a hovered strip's cables, the hovered route) takes
        // no lane of its own: Subway's lanes never move under the pointer.
        let settled = shows(touches || chosen);
        if !shown && !chain {
            continue;
        }
        let Some(fp) = v.desk.place.get(fi as usize).copied() else {
            continue;
        };
        let tp = if to_master {
            None
        } else {
            match v.desk.place.get(ti as usize) {
                Some(p) => Some(*p),
                None => continue,
            }
        };
        let (fc, (_, mut fx), fshow) = ports.at(Some(fp));
        let (tc, (mut tx, _), tshow) = ports.at(tp);
        // Both ends scrolled out on the same side: nothing of it shows.
        if !fshow && !tshow && (fc < ports.shown.0) == (tc < ports.shown.0) {
            continue;
        }
        if !fshow {
            fx = ports.chip_x(fc);
        }
        if !tshow {
            tx = ports.chip_x(tc);
        }
        if (tx - fx).abs() < 1.0 {
            continue;
        }
        let kind = kind_of(e);
        // The signal: the source's meter (as the strip shows it), and a send's level.
        let src_db = v.meters.level_db(fi as usize);
        let (db, send_db) = match e.kind {
            EdgeKind::Send { level, enabled, .. } => {
                let g = if enabled { level } else { 0.0 };
                let sdb = 20.0 * g.max(1e-6).log10();
                (src_db + sdb, sdb)
            }
            _ => (src_db, 0.0),
        };
        let level = if playing {
            ((db + 48.0) / 48.0).clamp(0.0, 1.0)
        } else {
            0.0
        };
        v.band.drawn.push(Drawn {
            route,
            from: pos2(fx, jy),
            to: pos2(tx, jy),
            depth: 0.0,
            lit,
            chain,
            hover_only: !settled,
            alpha: 1.0,
            kind,
            width: if jack {
                wires::jack_width(kind, send_db)
            } else {
                wires::width(kind, send_db)
            },
            level,
            chips: [!fshow, !tshow],
            edge: i as u32,
            path: (0, 0),
        });
    }
}

/// Each drawn cable's shape in the user's style, into the band's point list: a hanging
/// curve (Patch, Flow; no lower than `floor`), or a subway route in its lane. The
/// channel's depth the lanes need is kept for the next layout.
fn shape(v: &mut MixerView, style: CableStyle, jy: f32, floor: f32, now: f64) {
    let band = &mut v.band;
    let room = (floor - jy).max(4.0);
    if style == CableStyle::Jack {
        // D: a catenary whose parameter is solved once per route (its ends and sag), a
        // dropped cord's sag ringing down for 300 ms.
        band.channel = wires::JACK_CHANNEL;
        if band.cat.len() > 4 * band.drawn.len() + 16 {
            band.cat.clear();
        }
        for k in 0..band.drawn.len() {
            let d = band.drawn[k];
            let s0 = wires::jack_sag(d.to.x - d.from.x).min((room - 5.0).max(4.0));
            let s = match band.settle.iter().find(|(r, _)| *r == d.route) {
                Some((_, t0)) => wires::settle(s0, ((now - t0) * 1000.0) as f32),
                None => s0,
            };
            let key = [
                d.from.x.to_bits(),
                d.to.x.to_bits(),
                d.from.y.to_bits(),
                s.to_bits(),
            ];
            let ca = match band
                .cat
                .iter()
                .find(|(r, kk, _)| *r == d.route && *kk == key)
            {
                Some((_, _, ca)) => *ca,
                None => {
                    let ca = wires::catenary_a(d.to.x - d.from.x, s);
                    band.cat.retain(|(r, _, _)| *r != d.route);
                    band.cat.push((d.route, key, ca));
                    ca
                }
            };
            let start = band.pts.len() as u32;
            wires::jack_cord(&mut band.pts, d.from, d.to, s, ca);
            let dd = &mut band.drawn[k];
            dd.depth = s;
            dd.path = (start, band.pts.len() as u32);
        }
    } else if style == CableStyle::Subway {
        // The lanes of what the view shows; a cable drawn only for the hovered route runs
        // below them all, so nothing under the pointer moves and the channel keeps its depth.
        band.spans.clear();
        for d in band.drawn.iter().filter(|d| !d.hover_only) {
            band.spans.push((d.kind, d.from.x, d.to.x));
        }
        let deepest = wires::lanes(&band.spans, &mut band.order, &mut band.sub, &mut band.lanes);
        band.channel =
            (deepest + 2.0 * wires::CORNER + wires::JACK_Y - wires::JACK_ROW).max(wires::CHANNEL);
        let (mut shown, mut extra) = (0usize, 0.0f32);
        for k in 0..band.drawn.len() {
            let d = band.drawn[k];
            let lane = if d.hover_only {
                extra += 1.0;
                deepest.max(wires::LANE[2]) + wires::SUB * extra
            } else {
                shown += 1;
                band.lanes[shown - 1]
            }
            .min(room);
            let start = band.pts.len() as u32;
            wires::subway(&mut band.pts, d.from, d.to, jy + lane);
            let dd = &mut band.drawn[k];
            dd.depth = lane;
            dd.path = (start, band.pts.len() as u32);
        }
    } else {
        band.channel = wires::CHANNEL;
        for d in band.drawn.iter_mut() {
            let k = wires::sag(d.to.x - d.from.x).min(room / 0.75);
            let start = band.pts.len() as u32;
            wires::patch(&mut band.pts, d.from, d.to, k);
            d.depth = k;
            d.path = (start, band.pts.len() as u32);
        }
    }
}

/// The cable under `p`: the nearest within reach of its stroke; a tie goes to the one
/// drawn on top (the selection's), then the shorter.
fn hit(v: &MixerView, p: Pos2) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (k, d) in v.band.drawn.iter().enumerate() {
        let dist = wires::dist(v.band.path(d), p) - d.width * 0.5;
        if dist > wires::HIT {
            continue;
        }
        let better = match best {
            None => true,
            Some((j, bd)) => {
                let o = &v.band.drawn[j];
                if (dist - bd).abs() > 0.05 {
                    dist < bd
                } else if layer(d, None) != layer(o, None) {
                    layer(d, None) > layer(o, None)
                } else {
                    (d.to.x - d.from.x).abs() < (o.to.x - o.from.x).abs()
                }
            }
        };
        if better {
            best = Some((k, dist));
        }
    }
    best.map(|(k, _)| k)
}

/// The layer a cable is painted in: the others', the selection's, the hovered route.
fn layer(d: &Drawn, on_route: Option<bool>) -> u8 {
    match on_route {
        Some(true) => 2,
        Some(false) => 0,
        None => u8::from(d.lit || d.chain),
    }
}

/// Every cable: its darker under-stroke, its stroke (a key, or a send switched off,
/// dashed), and in *Flow* the gleam while its source plays; the others first, the hovered
/// route on top. Each jack takes the colour of its strongest cable drawn at 45 % or more
/// (output, then send, then key).
fn paint(
    v: &mut MixerView,
    app: &CantusApp,
    b: &mut Brush,
    tok: &Tokens,
    style: CableStyle,
    hot: Option<usize>,
) {
    let flowing = style.gleams() && app.engine.is_playing();
    let jack = style == CableStyle::Jack;
    // The transport's time, in seconds: where every gleam is.
    let t = if flowing {
        let bpm = app.project().tempo_bpm.max(1.0);
        app.engine.position().as_beats_f64() * 60.0 / bpm
    } else {
        0.0
    };
    let n = v.band.drawn.len();
    let jacks = v.desk.place.len() + 1;
    v.band.jack.clear();
    v.band.jack.resize(jacks, [u8::MAX; 2]);
    for pass in 0..3u8 {
        for k in 0..n {
            let d = v.band.drawn[k];
            let l = match hot {
                Some(_) => layer(&d, Some(hot == Some(k) || d.chain)),
                None => layer(&d, None),
            };
            if l != pass {
                continue;
            }
            let selected = v.band.selected == Some(d.route);
            let col = tok.of(d.kind);
            let alpha = d.alpha;
            let w = d.width + if selected { 0.75 } else { 0.0 };
            let band = &mut v.band;
            let pts = &band.pts[d.path.0 as usize..d.path.1 as usize];
            if pts.len() < 2 {
                continue;
            }
            let under = with_alpha(tok.under(col), alpha);
            let ink = with_alpha(col, alpha);
            let off = matches!(
                v.desk.edges[d.edge as usize].kind,
                EdgeKind::Send { enabled: false, .. }
            );
            if jack {
                let lift = hot.is_some() && l == 2;
                cord(
                    b,
                    tok,
                    &mut band.tmp,
                    pts,
                    col,
                    w,
                    d.kind == Kind::Key,
                    alpha,
                    lift,
                );
            } else if d.kind == Kind::Key || off {
                b.dashed(pts, w + 2.0, under, 4.0, 3.0);
                b.dashed(pts, w, ink, 4.0, 3.0);
            } else {
                // Both ends lie under their jack's ring: no cap is seen, none is drawn.
                b.polyline(pts, w + 2.0, under);
                b.polyline(pts, w, ink);
            }
            if flowing && d.level > 0.0 && !off {
                let gi = if jack {
                    crate::kit::color::mix(col, egui::Color32::WHITE, 0.75)
                } else {
                    tok.gleam(col)
                };
                let g = with_alpha(gi, wires::gleam_alpha(d.level) * alpha);
                let o = wires::gleam_offset(t, d.level);
                let dash = if jack {
                    wires::JACK_GLEAM
                } else {
                    wires::GLEAM
                };
                let mut count = 0u32;
                wires::gleams_of(pts, o, dash, |a, z| {
                    b.segment(a, z, w, g, false);
                    count += 1;
                });
                band.gleams += count;
            }
            if alpha >= OTHERS - 1e-3 {
                let (fi, ti) = v.desk.ends[d.edge as usize];
                let rank = d.kind.rank() as u8;
                let slot = |i: u32| {
                    if i == MASTER_END {
                        jacks - 1
                    } else {
                        i as usize
                    }
                };
                for (i, side) in [(fi, 1usize), (ti, 0usize)] {
                    if let Some(j) = v.band.jack.get_mut(slot(i)) {
                        j[side] = j[side].min(rank);
                    }
                }
            }
        }
    }
}

/// The colour a jack's ring takes: its strongest cable's, or the ring at rest.
fn jack_ink(v: &MixerView, tok: &Tokens, slot: usize, side: usize) -> Option<egui::Color32> {
    let r = v.band.jack.get(slot)?[side];
    match r {
        0 => Some(tok.out),
        1 => Some(tok.send),
        2 => Some(tok.key),
        _ => None,
    }
}

/// One jack: a dark hole of radius 4 with a 1.25-px ring (its cable's colour when one is
/// plugged in); the output carries a dot. A drop target's halo, a refused one's red ring
/// and slash.
fn jack(
    b: &mut Brush,
    tok: &Tokens,
    p: &super::look::Pal,
    c: Pos2,
    out: bool,
    ink: Option<egui::Color32>,
    target: Target,
) {
    if target == Target::Valid {
        b.circle(c, 8.0, Fill::Solid(with_alpha(tok.out, 0.22)));
    }
    let ring = match target {
        Target::Refused => tok.refused,
        _ => ink.unwrap_or(tok.ring),
    };
    b.circle(c, wires::JACK_R, Fill::Solid(tok.hole));
    b.circle_stroke(c, wires::JACK_R - 0.6, 1.25, ring);
    if out {
        b.dot(c, 1.5, ink.unwrap_or(p.ink3));
    }
    if target == Target::Refused {
        b.segment(
            c - vec2(2.0, 2.0),
            c + vec2(2.0, 2.0),
            1.2,
            tok.refused,
            false,
        );
    }
}

/// The words by the pointer over a hovered cable: its whole route ("Kick → Drum bus →
/// Master"), also in the hint bar with what it is.
#[allow(clippy::too_many_arguments)]
fn route_tip(
    v: &mut MixerView,
    app: &CantusApp,
    b: &mut Brush,
    s: &Style,
    geo: &Geo,
    k: usize,
    p: Pos2,
) {
    let mut words = std::mem::take(&mut v.hint_buf);
    route_words(v, app, k, &mut words);
    let t = &s.t;
    let tx = Text::new(&words, Role::Label, t.text).caps(false);
    let size = text::measure(b.ctx(), &tx, s.tk());
    let (w, h) = (size.x + 14.0, 18.0);
    let mut r = Rect::from_min_size(pos2(p.x - w * 0.5, p.y - 40.0), vec2(w, h));
    r = r.translate(vec2(
        (geo.full.left() + 4.0 - r.left()).max(0.0) - (r.right() - geo.full.right() + 4.0).max(0.0),
        0.0,
    ));
    let r = b.snap_rect(r);
    b.rrect(r, 4.0, Fill::Solid(t.panel_raised));
    b.rect(
        Rect::from_min_size(r.min, vec2(r.width(), 1.0)),
        Fill::Solid(crate::kit::color::mix(t.panel_raised, t.text, 0.08)),
    );
    let _ = text::paint(b, r, &tx.align(Align2::CENTER_CENTER), s.tk());
    v.hint_buf = words;
}

/// Cable `k`'s whole route by name: its source, its target, and on to the master
/// ("Kick → Drum bus → Master") when it is the hovered one.
fn route_words(v: &MixerView, app: &CantusApp, k: usize, out: &mut String) {
    let d = v.band.drawn[k];
    out.clear();
    let name = |out: &mut String, id: StripId| {
        if id.is_master() {
            out.push_str(&v.words.master);
        } else if let Some(i) = app.project().strip_index(id) {
            out.push_str(v.desk.name(app, i));
        }
    };
    name(out, d.route.from);
    out.push_str(" \u{2192} ");
    name(out, d.route.to);
    // The chain is the hovered cable's (`Band::chain`, from its target on).
    if v.band.hot == Some(d.route) {
        for &id in v.band.chain.iter().skip(1) {
            out.push_str(" \u{2192} ");
            name(out, id);
        }
        if !v.band.chain.is_empty() {
            out.push_str(" \u{2192} ");
            name(out, StripId::MASTER);
        }
    }
}

/// The edge chips of lit cables whose end is scrolled out of the middle: "‹ 37" / "37 ›"
/// at the side it went, stacked when several go the same way; a click scrolls to it.
#[allow(clippy::too_many_arguments)]
fn chips(
    v: &mut MixerView,
    app: &CantusApp,
    ui: &Ui,
    b: &mut Brush,
    pass: &Pass,
    geo: &Geo,
    port_y: f32,
    bottom: f32,
) {
    let s = pass.s;
    let t = &s.t;
    let mut stack = [0u8; 2];
    for k in 0..v.band.drawn.len() {
        let d = v.band.drawn[k];
        if !d.lit {
            continue;
        }
        for (end, at, far) in [(0usize, d.from, d.route.from), (1, d.to, d.route.to)] {
            if !d.chips[end] {
                continue;
            }
            let side = usize::from(at.x >= geo.middle.center().x);
            let n = stack[side];
            stack[side] = n.saturating_add(1);
            let r = Rect::from_center_size(pos2(at.x, port_y + n as f32 * (CHIP.y + 1.0)), CHIP);
            if r.bottom() > bottom {
                continue;
            }
            let Some(index) = app.project().strip_index(far) else {
                continue;
            };
            let color = strip_color(v, s, index).unwrap_or(t.label);
            let id = egui::Id::new(("mixer-cable-chip", d.edge, end));
            let resp = sense(ui, pass, r, id, Sense::click());
            let hovered = resp.as_ref().is_some_and(|r| r.hovered());
            let fill = shade(t.panel, if hovered { 0.10 } else { 0.05 });
            b.rrect(r, s.r(3.0), Fill::Solid(fill));
            b.rrect_stroke(r.shrink(b.px() * 0.5), s.r(3.0), b.px(), color);
            v.buf.clear();
            if side == 0 {
                let _ = write!(v.buf, "\u{2039} {}", index + 1);
            } else {
                let _ = write!(v.buf, "{} \u{203a}", index + 1);
            }
            let tt = Text::new(&v.buf, Role::Micro, t.text)
                .caps(false)
                .align(Align2::CENTER_CENTER);
            let _ = text::paint(b, r, &tt, s.tk());
            let Some(resp) = resp else {
                continue;
            };
            if hovered {
                let name = v.desk.name(app, index);
                hint::set_text(ui.ctx(), &v.words.chip, name, Caps::NONE);
            }
            if resp.clicked() {
                reveal(v, geo, index);
            }
        }
    }
}

/// Scroll the middle so the strip at `index` shows (an edge chip's click).
fn reveal(v: &mut MixerView, geo: &Geo, index: usize) {
    if let Some(p) = v.desk.place.get(index).filter(|p| p.dock == Dock::Middle) {
        v.scroll_x = (p.x(geo.pitch) - geo.middle.width() * 0.5).max(0.0);
    }
}

/// The level pills of the lit sends: "−6.0 dB", with a *PRE* tag in the cable's colour
/// for a pre-fader send (answer 4). Wide and Extra large show them on every lit send,
/// Compact on the hovered or selected one; a pill that would cover the ports or another
/// pill is left out (its value is in the hint bar, the send row and the inspector). Drag:
/// the level (double-click: 0 dB); click: select the cable; right-click: the route menu.
/// The selected send's pill has the `POST | PRE` switch beside it.
#[allow(clippy::too_many_arguments)]
fn pills(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &Ui,
    b: &mut Brush,
    pass: &Pass,
    hot: Option<usize>,
    port_y: f32,
    clip: (f32, f32),
) {
    let s = pass.s;
    let t = &s.t;
    let compact = pass.mode() == WidthMode::Compact;
    let ctx = ui.ctx().clone();
    let k_text = s.tk() * 0.85;
    for k in 0..v.band.drawn.len() {
        let d = v.band.drawn[k];
        let i = d.edge as usize;
        let EdgeKind::Send {
            level,
            enabled,
            tap,
        } = v.desk.edges[i].kind
        else {
            continue;
        };
        let chosen = v.band.selected == Some(d.route);
        let lit = d.lit || hot == Some(k);
        if !lit || !(!compact || hot == Some(k) || chosen) {
            continue;
        }
        let low = wires::lowest(v.band.path(&d));
        v.buf.clear();
        super::parts::write_level(&mut v.buf, level);
        let value = Text::new(&v.buf, Role::Value, t.text);
        let vw = text::measure(&ctx, &value, k_text).x;
        // The selected send's tap chip beside the pill names its tap (and switches it):
        // the pill's own PRE tag would only say it twice.
        let pre = !tap.is_post() && !chosen;
        // The label role, not the micro one: the tag must read at 75 %.
        let tag = Text::new(&v.words.pre, Role::Label, t.text).caps(false);
        let tw = if pre {
            text::measure(&ctx, &tag, k_text).x + 6.0
        } else {
            0.0
        };
        let r = Rect::from_center_size(low, vec2(vw + tw + 14.0, PILL_H - 1.0));
        let r = b.snap_rect(r);
        // The selected send's tap chip rides beside its pill: placed, clipped and kept off
        // the other pills with it.
        let chip = chosen.then(|| {
            let w = crate::kit::tap::chip_width(&ctx, s, tap, k_text);
            b.snap_rect(Rect::from_min_size(
                pos2(r.right() + 3.0, r.top()),
                vec2(w, r.height()),
            ))
        });
        let whole = chip.map_or(r, |c| r.union(c));
        if whole.top() < port_y + wires::JACK_R + 1.0
            || whole.left() < clip.0
            || whole.right() > clip.1
            || v.band.pills.iter().any(|p| p.intersects(whole.expand(2.0)))
        {
            continue;
        }
        v.band.pills.push(whole);
        let ink = wires::tokens(&pass.pal, t.ink).of(d.kind);
        let id = egui::Id::new(("mixer-cable-pill", d.route.from.0, d.route.to.0));
        let resp = sense(ui, pass, r, id, Sense::click_and_drag());
        let hovered = resp.as_ref().is_some_and(|r| r.hovered() || r.dragged());
        let radius = r.height() * 0.5;
        b.rrect(
            r,
            radius,
            Fill::Solid(shade(t.panel, if hovered { 0.10 } else { 0.04 })),
        );
        b.rrect_stroke(r.shrink(b.px() * 0.5), radius, b.px() * 1.2, ink);
        let mut vr = r.shrink2(vec2(5.0, 0.0));
        if pre {
            let tr = Rect::from_min_max(r.min, pos2(r.left() + tw + 2.0, r.bottom()));
            // Lit in the pre-fader hue, as the tap chip and the matrix's cell are.
            let hue = t.hue(crate::kit::tap::function(tap));
            b.rrect(tr.shrink(b.px() * 1.5), radius, Fill::Solid(hue));
            let tag = Text::new(&v.words.pre, Role::Label, on_fill(hue))
                .caps(false)
                .align(Align2::CENTER_CENTER);
            let _ = text::paint(b, tr.translate(vec2(1.0, 0.0)), &tag, k_text);
            vr.min.x = tr.right() + 2.0;
        }
        let vink = if enabled {
            t.text
        } else {
            with_alpha(t.text, 0.55)
        };
        let value = Text::new(&v.buf, Role::Value, vink).align(Align2::CENTER_CENTER);
        let _ = text::paint(b, vr, &value, k_text);
        if let Some(cr) = chip {
            if let Some(tap) =
                crate::kit::tap::chip(ui, b, s, cr, pass.clip, id.with("tap"), tap, k_text)
            {
                app.set_send_tap(d.route.from, d.route.to, tap);
            }
        }
        let Some(resp) = resp else {
            continue;
        };
        let send = app
            .project()
            .strip(d.route.from)
            .and_then(|st| st.send_to(d.route.to))
            .copied();
        // The level is a parameter (review 2): the handle's drop, the Alt-drag depth.
        let dm = super::strip::DeskMod::new(
            app,
            cantus_core::AutoTarget::Send {
                strip: d.route.from,
                to: d.route.to,
            },
            level,
            s,
        );
        let depth = dm.input(app, ui, &resp);
        // Modulated, as the desk's send row and knob show it (review of fix3: the pill took
        // modulation and showed none): its outline in the modulation hue, and a tick on its
        // foot where modulation has the level now.
        if !dm.rings().is_empty() {
            b.rrect_stroke(r.shrink(b.px() * 0.5), radius, b.px() * 1.6, t.hue_mod);
            if let Some(live) = dm.live {
                let x = r.left() + radius + (r.width() - 2.0 * radius) * live.clamp(0.0, 1.0);
                b.rect(
                    Rect::from_min_max(pos2(x - 1.0, r.bottom() - 3.0), pos2(x + 1.0, r.bottom())),
                    Fill::Solid(t.hue_mod),
                );
            }
        }
        dm.paint_drop(b, ui, Some(&resp), r, s);
        if let (Some(send), Some(p)) = (send, knob_input(v, ui, pass, &resp, level, 1.0, !depth)) {
            app.set_strip_send(d.route.from, Send { level: p, ..send });
        }
        if resp.drag_stopped() && !depth {
            app.end_gesture();
        }
        if hovered {
            hint::set(&ctx, &v.words.pill, Caps::PARAM, resp.dragged(), |w| {
                super::parts::write_level(w, level);
                Ok(())
            });
        }
        if resp.clicked() {
            v.band.selected = Some(d.route);
        }
        if resp.secondary_clicked() {
            let at = resp.interact_pointer_pos().unwrap_or(r.left_bottom());
            v.band.selected = Some(d.route);
            crate::views::route_menu::open(&ctx, route_owner(), d.route, at);
        }
    }
}

/// Every jack on screen: the master's two, each strip's input and output, a collapsed
/// group's one at its separator (where its members' cables gather). The output jack
/// starts a cable (`mixer_drag`); while one is dragged, the input jack under it shows
/// whether a drop there works.
#[allow(clippy::too_many_arguments)]
fn jack_row(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &Ui,
    b: &mut Brush,
    pass: &Pass,
    geo: &Geo,
    ports: &Ports,
    tok: &Tokens,
    jy: f32,
    d: bool,
) {
    let p = pass.pal;
    let over = drag::over(v);
    let target = |id: Option<StripId>| match over {
        Some((s, t)) if s == id => t,
        _ => Target::Idle,
    };
    let master_slot = v.desk.place.len();
    let (mi, mo) = wires::jacks(ports.master, ports.master_w);
    let ink = jack_ink(v, tok, master_slot, 0);
    if d {
        nut(b, tok, pos2(mi, jy), ink, target(None));
        nut(b, tok, pos2(mo, jy), None, Target::Idle);
    } else {
        jack(b, tok, &p, pos2(mi, jy), false, ink, target(None));
        jack(b, tok, &p, pos2(mo, jy), true, None, Target::Idle);
    }
    let pitch = geo.pitch;
    for (dock, origin) in [
        (Dock::Left, ports.left),
        (Dock::Middle, ports.middle),
        (Dock::Right, ports.right),
    ] {
        let len = match dock {
            Dock::Left => v.desk.left.len(),
            Dock::Middle => v.desk.middle.len(),
            Dock::Right => v.desk.right.len(),
        };
        let mut x = origin;
        for k in 0..len {
            let item = match dock {
                Dock::Left => v.desk.left[k],
                Dock::Middle => v.desk.middle[k],
                Dock::Right => v.desk.right[k],
            };
            let w = super::desk::item_width(&item, pitch);
            let c = x + (w - 2.0) * 0.5;
            x += w;
            if dock == Dock::Middle && (c < ports.shown.0 || c > ports.shown.1) {
                continue;
            }
            match item {
                super::desk::Item::Strip(id, index) => {
                    let (ix, ox) = wires::jacks(c, pitch);
                    let ink = jack_ink(v, tok, index, 0);
                    if d {
                        nut(b, tok, pos2(ix, jy), ink, target(Some(id)));
                    } else {
                        jack(b, tok, &p, pos2(ix, jy), false, ink, target(Some(id)));
                    }
                    out_jack(v, app, ui, b, pass, tok, (id, index), pos2(ox, jy), d);
                }
                super::desk::Item::Separator(g) => {
                    let collapsed = app.project().strip_group(g).is_some_and(|g| g.collapsed);
                    if collapsed && d {
                        nut(b, tok, pos2(c, jy), None, Target::Idle);
                    } else if collapsed {
                        jack(b, tok, &p, pos2(c, jy), false, None, Target::Idle);
                    }
                }
            }
        }
    }
}

/// A strip's output jack: its strongest cable's colour on the ring and the dot; under the
/// pointer a ring round it; armed (click mode) a ring in the selection's colour. A press
/// starts a cable (`mixer_drag`).
#[allow(clippy::too_many_arguments)]
fn out_jack(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &Ui,
    b: &mut Brush,
    pass: &Pass,
    tok: &Tokens,
    (id, index): (StripId, usize),
    c: Pos2,
    d: bool,
) {
    let s = pass.s;
    let t = &s.t;
    let r = jack_rect(c, d);
    let resp = sense(
        ui,
        pass,
        r,
        pid(Some(id), Part::Port),
        Sense::click_and_drag(),
    );
    let hovered = resp.as_ref().is_some_and(|r| r.hovered());
    // The selected strip's own output lights in the accent, as des-mixer's socket did.
    let ink = jack_ink(v, tok, index, 1).or_else(|| {
        (app.selected_strip == Some(id) && v.band.drawn.iter().any(|d| d.route.from == id))
            .then_some(tok.out)
    });
    if d {
        nut(b, tok, c, ink, Target::Idle);
    } else {
        jack(b, tok, &pass.pal, c, true, ink, Target::Idle);
    }
    if hovered {
        b.circle_stroke(c, wires::JACK_R + 2.5, b.px(), with_alpha(t.text, 0.6));
    }
    if drag::armed(v) == Some(id) {
        // Click mode: armed — a ring in the selection's colour around the jack, until the
        // click on a destination (or Esc) lets it go.
        b.circle_stroke(c, wires::JACK_R + 3.0, 1.6, t.selection);
    }
    let Some(resp) = resp else {
        return;
    };
    drag::source(v, app, ui, &resp, Source::Port(id));
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        hint::set_text(ui.ctx(), &v.words.port, "", Caps::NONE);
    }
}

/// The input jacks lit cables plug into, as drag sources: pulling the plug moves where
/// that route goes (the selected cable's first where several share a jack).
fn ends(v: &mut MixerView, app: &mut CantusApp, ui: &Ui, pass: &Pass, jack: bool) {
    v.band.sensed.clear();
    let first = v
        .band
        .selected
        .and_then(|r| v.band.drawn.iter().position(|d| d.route == r));
    let n = v.band.drawn.len();
    for k in first.into_iter().chain(0..n) {
        let d = v.band.drawn[k];
        if !d.lit || d.chips[1] || v.band.sensed.iter().any(|x| (x - d.to.x).abs() < 0.5) {
            continue;
        }
        v.band.sensed.push(d.to.x);
        // D: the cord ends at its plug's boot; the jack is 22 above.
        let at = if jack {
            d.to - vec2(0.0, wires::PLUG_LEN)
        } else {
            d.to
        };
        let r = jack_rect(at, jack);
        let id = egui::Id::new(("mixer-cable-end", d.edge, false));
        if let Some(resp) = sense(ui, pass, r, id, Sense::drag()) {
            drag::source(
                v,
                app,
                ui,
                &resp,
                Source::End {
                    route: d.route,
                    end: End::Destination,
                },
            );
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                hint::set_text(ui.ctx(), &v.words.cable, "", Caps::NONE);
            }
        }
    }
}

/// The band's own clicks and keys: a click selects the cable under the pointer (or lets
/// the selected one go), right-click opens its route menu, Esc lets it go and Delete
/// removes it; hovering a cable names it in the hint bar.
fn interact(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &Ui,
    resp: &egui::Response,
    hot: Option<usize>,
) {
    let ctx = ui.ctx().clone();
    let hot_route = hot.and_then(|k| v.band.drawn.get(k)).map(|d| d.route);
    if resp.clicked() {
        v.band.selected = hot_route;
    }
    if resp.secondary_clicked() {
        if let Some(route) = hot_route {
            v.band.selected = Some(route);
            let at = resp.interact_pointer_pos().unwrap_or(resp.rect.center());
            crate::views::route_menu::open(&ctx, route_owner(), route, at);
        }
    }
    // r39:ux-a — the band's own words where no cable is pointed at (ADR 0109).
    if hot.is_none() && resp.hovered() {
        hint::fallback(&ctx, &crate::i18n::t("ux.hint.cable_band"), "", Caps::NONE);
    }
    if let (Some(k), true) = (hot, resp.hovered()) {
        describe(v, app, k);
        hint::set_text(
            &ctx,
            &v.hint_buf,
            &v.buf,
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    let Some(route) = v.band.selected else {
        return;
    };
    let (esc, del) = ctx.input(|i| {
        (
            i.key_pressed(Key::Escape),
            i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace),
        )
    });
    // Esc lets the cable go only when nothing above the band took it: a field being
    // typed in, or a route menu (the right-click that selected the cable opened it).
    let taken = ctx.egui_wants_keyboard_input()
        || crate::kit::field::active(&ctx)
        || crate::views::route_menu::is_open(&ctx, route_owner());
    if esc && !taken {
        v.band.selected = None;
    } else if del
        && !ctx.egui_wants_keyboard_input()
        && app.focus == crate::app::EditFocus::Mixer
        && app.selected_notes.is_empty()
    {
        // The keymap's Delete runs first and removes selected notes; a cable goes only
        // when that took nothing (round 25: the integrator may route Delete here).
        app.remove_route(route);
        v.band.selected = None;
    }
}

/// The hint bar's words for cable `k`: its whole route into `hint_buf` ("Kick → Drum
/// bus → Master"), what it is into `buf` ("send · −6.0 dB · pre-fader").
fn describe(v: &mut MixerView, app: &CantusApp, k: usize) {
    let d = v.band.drawn[k];
    let i = d.edge as usize;
    let e = v.desk.edges[i];
    let mut names = std::mem::take(&mut v.hint_buf);
    route_words(v, app, k, &mut names);
    v.hint_buf = names;
    v.buf.clear();
    match e.kind {
        EdgeKind::Output => v.buf.push_str(&v.words.kind[0]),
        EdgeKind::Send {
            level,
            enabled,
            tap,
        } => {
            v.buf.push_str(&v.words.kind[1]);
            v.buf.push_str(" \u{b7} ");
            super::parts::write_level(&mut v.buf, level);
            v.buf.push_str(" \u{b7} ");
            v.buf.push_str(&v.words.tap[usize::from(!tap.is_post())]);
            if !enabled {
                v.buf.push_str(" \u{b7} ");
                v.buf.push_str(&v.words.send_off);
            }
            if v.desk.doubled.get(i).copied().unwrap_or(false) {
                if let Some(text) =
                    crate::routing_actions::doubled_text(app.project(), e.from, e.to)
                {
                    v.buf.push_str(" \u{b7} ");
                    v.buf.push_str(&text);
                }
            }
        }
        EdgeKind::Key { .. } => v.buf.push_str(&v.words.kind[2]),
    }
}

/// A column under a drag: lit when a drop would work, hatched when it is refused.
pub(super) fn column_hatch(b: &mut Brush, s: &Style, column: Rect, target: Target) {
    let t = &s.t;
    match target {
        Target::Idle => {}
        Target::Valid => {
            b.rect(
                b.snap_rect(column),
                Fill::Solid(with_alpha(t.selection, 0.10)),
            );
            b.rrect_stroke(
                b.snap_rect(column).shrink(b.px()),
                0.0,
                b.px() * 1.5,
                with_alpha(t.selection, 0.8),
            );
        }
        Target::Refused => {
            b.rect(b.snap_rect(column), Fill::Solid(with_alpha(t.danger, 0.06)));
            let (l, r, top, bottom) =
                (column.left(), column.right(), column.top(), column.bottom());
            let ink = with_alpha(t.danger, 0.30);
            let (step, w) = (s.s(8.0), b.px() * 1.2);
            // Lines going down to the right, x = c + (y − top), clipped to the column.
            let mut c = l - column.height();
            while c < r {
                let y0 = top.max(top + (l - c));
                let y1 = bottom.min(top + (r - c));
                if y1 > y0 {
                    b.segment(
                        pos2(c + (y0 - top), y0),
                        pos2(c + (y1 - top), y1),
                        w,
                        ink,
                        false,
                    );
                }
                c += step;
            }
        }
    }
}

const BAND_TOKENS: &[Tok] = &[
    Tok::Strip,
    Tok::Panel,
    Tok::Text,
    Tok::Danger,
    Tok::Selection,
];

// ---- D, "Jack": the cord, the plugs and the jacks' nuts (README, "Cable style D") -----

/// A cord, back to front: a two-pass shadow on the desk (offset 1.5, 2.5; lifted 3, 5 when
/// hovered), a darker edge, the body, a key's spiral marks, a narrow highlight (a tube lit
/// from above). `tmp` holds the shifted copies.
#[allow(clippy::too_many_arguments)]
pub(super) fn cord(
    b: &mut Brush,
    tok: &Tokens,
    tmp: &mut Vec<Pos2>,
    pts: &[Pos2],
    col: egui::Color32,
    w: f32,
    key: bool,
    alpha: f32,
    lift: bool,
) {
    use crate::kit::color::mix;
    if pts.len() < 2 {
        return;
    }
    let m = wires::material(tok.dark());
    let (off, fade) = if lift {
        (vec2(3.0, 5.0), (0.75, 0.8))
    } else {
        (vec2(1.5, 2.5), (1.0, 1.0))
    };
    let (sa, sb) = (
        f32::from(m.shadow_a.0) / 255.0 * fade.0,
        f32::from(m.shadow_a.1) / 255.0 * fade.1,
    );
    tmp.clear();
    tmp.extend(pts.iter().map(|p| *p + off));
    b.polyline(tmp, w + 6.0, with_alpha(m.shadow, sb * alpha));
    b.polyline(tmp, w + 3.0, with_alpha(m.shadow, sa * alpha));
    let edge = with_alpha(tok.jack_edge(col), alpha);
    let body = with_alpha(col, alpha);
    b.polyline(pts, w + 1.5, edge);
    b.polyline(pts, w, body);
    if key {
        let mark = with_alpha(mix(col, egui::Color32::BLACK, 0.45), 0.8 * alpha);
        b.dashed(pts, w, mark, 1.4, 3.2);
    }
    tmp.clear();
    tmp.extend(pts.iter().map(|p| *p + vec2(-0.3, -0.22 * w)));
    let hi = mix(col, egui::Color32::WHITE, 0.6);
    b.polyline(
        tmp,
        0.35 * w,
        with_alpha(hi, if lift { 0.85 } else { 0.6 } * alpha),
    );
}

/// A plug from the layer atlas: seated (its nut at `at`, hanging 22 below) or free (its tip
/// at `at`, 36 long, pointing up), its kind band in `band`, its boot `rec` when refused.
/// Drawn once per (shape, colours, scale) by the SVG of des-mixer's parts.
pub(super) fn plug(
    b: &mut Brush,
    tok: &Tokens,
    at: Pos2,
    band: egui::Color32,
    boot: Option<egui::Color32>,
    free: bool,
    alpha: f32,
) {
    let m = wires::material(tok.dark());
    let ppp = b.ppp();
    let (w, h) = wires::plug_size(free);
    let (pw, ph) = ((w * ppp).ceil(), (h * ppp).ceil());
    let key = crate::kit::cache::key(("cables39-plug", free, band, boot, m, pw as u32, ph as u32));
    let Some(uv) = b.layer(key, pw as usize, ph as usize, |pix| {
        raster(&wires::plug_svg(&m, band, boot, free, ppp), pix)
    }) else {
        return;
    };
    let snap = |v: f32| (v * ppp).round() / ppp;
    let r = Rect::from_min_size(
        pos2(snap(at.x - w * 0.5), snap(at.y)),
        vec2(pw / ppp, ph / ppp),
    );
    b.layer_quad(r, uv, alpha);
}

fn raster(doc: &str, pix: &mut tiny_skia::Pixmap) {
    use resvg::usvg;
    let Ok(tree) = usvg::Tree::from_str(doc, &usvg::Options::default()) else {
        return;
    };
    let Some(mut out) = resvg::tiny_skia::Pixmap::new(pix.width(), pix.height()) else {
        return;
    };
    resvg::render(&tree, Default::default(), &mut out.as_mut());
    pix.data_mut().copy_from_slice(out.data());
}

fn ellipse(b: &mut Brush, c: Pos2, rx: f32, ry: f32, col: egui::Color32) {
    let mut pts = [Pos2::ZERO; 24];
    for (i, p) in pts.iter_mut().enumerate() {
        let a = i as f32 / 24.0 * std::f32::consts::TAU;
        *p = pos2(c.x + rx * a.cos(), c.y + ry * a.sin());
    }
    b.polygon(&pts, col);
}

fn ellipse_ring(b: &mut Brush, c: Pos2, rx: f32, ry: f32, w: f32, col: egui::Color32) {
    let mut pts = [Pos2::ZERO; 25];
    for (i, p) in pts.iter_mut().enumerate() {
        let a = i as f32 / 24.0 * std::f32::consts::TAU;
        *p = pos2(c.x + rx * a.cos(), c.y + ry * a.sin());
    }
    b.polyline(&pts, w, col);
}

/// A D jack on the strip's bottom edge: a nut 12 × 6 with its lit top 10 × 4; empty, its
/// hole; a plug seated in it when a cable is (`ink`, its kind band). A drop target's halo
/// (r 10, 25 %), a refused one's red ring 14 × 7.6.
fn nut(b: &mut Brush, tok: &Tokens, c: Pos2, ink: Option<egui::Color32>, target: Target) {
    use crate::kit::color::mix;
    let m = wires::material(tok.dark());
    if target == Target::Valid {
        b.circle(c, 10.0, Fill::Solid(with_alpha(tok.out, 0.25)));
    }
    ellipse(b, c, 6.0, 3.0, m.nut);
    ellipse(
        b,
        c - vec2(0.0, 0.6),
        5.0,
        2.0,
        mix(m.nut, egui::Color32::WHITE, 0.25),
    );
    match ink {
        Some(band) => plug(b, tok, c, band, None, false, 1.0),
        None => ellipse(b, c, 3.0, 1.4, m.hole),
    }
    if target == Target::Refused {
        ellipse_ring(b, c, 7.0, 3.8, 1.3, tok.refused);
    }
}

/// A jack's sensed area: around the ring (A–C), or around the nut, reaching most of the
/// way down its seated plug (D) — centred on the jack, where a dragged cable starts.
fn jack_rect(c: Pos2, jack: bool) -> Rect {
    if jack {
        Rect::from_center_size(c, vec2(2.0 * JACK_HIT, 32.0))
    } else {
        Rect::from_center_size(c, vec2(2.0 * JACK_HIT, 2.0 * JACK_HIT + 2.0))
    }
}
