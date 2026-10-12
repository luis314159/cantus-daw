//! The cable band under the strips (round 25, ADR 0041; design §3.1–3.2, §6): every
//! strip's port, the routes as cables (`kit::cable`), level pills, edge chips for ends
//! scrolled out of view, the hit test; and the hatching of a column a drag cannot drop
//! on.
//!
//! **What is drawn.** The desk's cable view (the title row's switch) decides: *Selected*
//! (the default, the owner's answer 1) draws the cables into and out of every selected
//! strip; *All* draws every route but the implied ones to the master, those away from the
//! selection faint; *Off* hides the band. Hovering a strip or a cable lights it in either
//! mode, and the hint bar names it ("3 Keys → 15 Reverb · send · −6.0 dB · pre-fader").
//! A send carries its level on a pill (Wide and Extra large; Compact on hover) with *PRE*
//! in the cable's colour when it is pre-fader; a doubled signal its warning mark.
//!
//! **What it does.** A port starts a drag (`mixer_drag`: plain = send, Ctrl = output,
//! Alt = key); a lit cable's end mark moves that route; a click selects a cable (Delete
//! removes it, Esc lets it go); right-click opens the route menu; dragging a pill sets the
//! send's level (double-click: 0 dB); an edge chip scrolls its strip into view.
//!
//! **Cost** (§6): the routes, the places and the doubled flags come from the desk's
//! per-revision cache; a frame visits the edges once, strokes into the desk's one brush,
//! keeps its lists' capacity and formats into the view's buffers — no allocation, no draw
//! call of its own.

use super::desk::{Place, MASTER_END};
use super::drag::{self, Source, Target};
use super::layout::{CableView, Row, WidthMode};
use super::strip::{knob_input, pid, sense, strip_color, Part, Pass};
use super::{in_selection, route_owner, Geo, MixerView};
use crate::app::CantusApp;
use crate::kit::cable::{self, CableKind, CableLook};
use crate::kit::color::{on_fill, shade, with_alpha};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Role, Text};
use crate::kit::{Brush, Fill, Style, Tok};
use cantus_core::routing::{Edge, EdgeKind, End, Route};
use cantus_core::{Dock, Send, StripId};
use egui::{pos2, vec2, Align2, Key, Pos2, Rect, Sense, Ui};
use std::fmt::Write as _;

/// A port's centre under the band's top edge, and its radius, at 100 %.
const PORT_Y: f32 = 6.0;
const PORT_R: f32 = 3.5;
/// The size of a port's hit area.
const PORT_HIT: egui::Vec2 = vec2(14.0, 14.0);
/// A pill's height: a cable hangs no lower than half of it above the band's foot.
const PILL_H: f32 = 13.0;
/// An edge chip's size.
const CHIP: egui::Vec2 = vec2(30.0, 13.0);
/// How near the pointer must come for a cable to be hovered.
const HIT: f32 = 4.0;

/// One cable as the band drew it this frame (`MixerView::cables` shows them to tests).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Drawn {
    pub route: Route,
    /// Its two ends on screen (a scrolled-out end: its edge chip).
    pub from: Pos2,
    pub to: Pos2,
    pub depth: f32,
    pub lit: bool,
    /// The source's and the destination's end is an edge chip.
    pub chips: [bool; 2],
    /// Its index in the desk's edges.
    pub edge: u32,
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
    /// Scratch for the lit cables' lanes: their order by span, and each one's lane.
    order: Vec<u32>,
    lane: Vec<u8>,
}

impl Band {
    /// Where this frame's pills went.
    pub fn pills(&self) -> &[Rect] {
        &self.pills
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

/// Where the ports are this frame: each dock's first item, the master's port, the strip
/// pitch, and the middle's visible span.
struct Ports {
    left: f32,
    middle: f32,
    right: f32,
    master: f32,
    pitch: f32,
    shown: (f32, f32),
}

impl Ports {
    fn new(v: &MixerView, geo: &Geo) -> Ports {
        Ports {
            left: geo.left.left() + geo.master_w,
            middle: geo.middle.left() - v.scroll_x,
            right: geo.right.left(),
            master: geo.left.left() + (geo.master_w - 1.0) * 0.5,
            pitch: geo.pitch,
            shown: (geo.middle.left() + 2.0, geo.middle.right() - 2.0),
        }
    }

    /// A port's x, and whether it shows (pinned docks always do; `None` is the master).
    fn at(&self, place: Option<Place>) -> (f32, bool) {
        let Some(p) = place else {
            return (self.master, true);
        };
        let origin = match p.dock {
            Dock::Left => self.left,
            Dock::Middle => self.middle,
            Dock::Right => self.right,
        };
        let x = origin + p.x(self.pitch);
        let shows = p.dock != Dock::Middle || (x >= self.shown.0 && x <= self.shown.1);
        (x, shows)
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

/// The band: ports, cables, pills, chips, across the whole desk width.
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
    v.band.pass = ui.ctx().cumulative_pass_nr();
    let Some((a, z)) = pass.row(Row::Cables) else {
        v.band.selected = None;
        return;
    };
    let view = pass.prefs.cables;
    if view == CableView::Off {
        // Nothing shows: nothing stays selected (Delete must not remove a hidden cable).
        v.band.selected = None;
        return;
    }
    let s = pass.s;
    let area =
        Rect::from_min_max(pos2(geo.left.left(), a), pos2(geo.rows.right(), z)).intersect(geo.rows);
    if area.height() <= 1.0 {
        return;
    }
    b.register(area, "cable_band", BAND_TOKENS, &[Some(Tok::HueDynamics)]);
    let port_y = a + PORT_Y;
    let room = (z - port_y - PILL_H * 0.5 - 1.0).max(4.0);
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
    collect(v, app, &ports, view, hovered_strip, port_y, room);
    if let Some(sel) = v.band.selected {
        // A selected cable scrolled out of sight (or no longer drawn) is let go.
        if !v.band.drawn.iter().any(|d| d.route == sel) {
            v.band.selected = None;
        }
    }
    lanes(&mut v.band, room);

    // The cable under the pointer: the nearest within reach, lit ones first.
    let in_band = pointer.filter(|p| area.contains(*p) && !dragging);
    let hot = in_band.and_then(|p| {
        let mut best: Option<(usize, f32)> = None;
        for (k, d) in v.band.drawn.iter().enumerate() {
            let dist = cable::hit(d.from, d.to, d.depth, p) - if d.lit { 1.0 } else { 0.0 };
            if dist < HIT && best.is_none_or(|(_, b)| dist < b) {
                best = Some((k, dist));
            }
        }
        best.map(|(k, _)| k)
    });
    if let Some(k) = hot {
        v.band.drawn[k].lit = true;
    }
    // The band itself first, so the ports, pills, ends and chips sensed after it are on
    // top of it.
    let band = ui.interact(area, egui::Id::new("mixer-cable-band"), Sense::click());

    // Faint cables under lit ones.
    for lit in [false, true] {
        for k in 0..v.band.drawn.len() {
            let d = v.band.drawn[k];
            if d.lit != lit {
                continue;
            }
            let look = look_of(v, s, &d);
            cable::paint(b, s, d.from, d.to, d.depth, &look);
        }
    }
    // The ports first: the chips and pills sensed after them are on top.
    port_row(v, app, ui, b, pass, geo, &ports, port_y);
    chips(v, app, ui, b, pass, geo, port_y, z);
    pills(
        v,
        app,
        ui,
        b,
        pass,
        hot,
        port_y,
        (area.left(), area.right()),
    );
    ends(v, app, ui, b, pass);
    interact(v, app, ui, &band, hot);
}

/// The cables this frame draws, and where: the selection's (and the hovered strip's and
/// the selected cable) lit, the rest faint in *All*; an output to the master only for the
/// selection, as FL does (sixty identical cables into the master say nothing).
fn collect(
    v: &mut MixerView,
    app: &CantusApp,
    ports: &Ports,
    view: CableView,
    hovered: Option<StripId>,
    port_y: f32,
    room: f32,
) {
    let selected = v.band.selected;
    for (i, e) in v.desk.edges.iter().enumerate() {
        let Some(&(fi, ti)) = v.desk.ends.get(i) else {
            continue;
        };
        if fi == MASTER_END {
            continue;
        }
        let route = route_of(e);
        let to_master = ti == MASTER_END;
        let touches = in_selection(app, e.from) || (!to_master && in_selection(app, e.to));
        let hov = hovered.is_some_and(|h| h == e.from || (!to_master && h == e.to));
        let chosen = selected == Some(route);
        let lit = touches || hov || chosen;
        let show = if e.is_master_output() {
            in_selection(app, e.from) || hov || chosen
        } else {
            match view {
                CableView::Off => false,
                CableView::Selected => lit,
                CableView::All => true,
            }
        };
        if !show {
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
        let (mut fx, fshow) = ports.at(Some(fp));
        let (mut tx, tshow) = ports.at(tp);
        // Both ends scrolled out on the same side: nothing of it shows.
        if !fshow && !tshow && (fx < ports.shown.0) == (tx < ports.shown.0) {
            continue;
        }
        if !fshow {
            fx = ports.chip_x(fx);
        }
        if !tshow {
            tx = ports.chip_x(tx);
        }
        if (tx - fx).abs() < 1.0 {
            continue;
        }
        let (from, to) = (pos2(fx, port_y), pos2(tx, port_y));
        v.band.drawn.push(Drawn {
            route,
            from,
            to,
            depth: cable::depth_for(from, to, room) * depth_share(&e.kind),
            lit,
            chips: [!fshow, !tshow],
            edge: i as u32,
        });
    }
}

/// The lit cables' depths: a lane each, so no two lit cables share a flat run (in a
/// 32-pt band every long cable would otherwise hang at the cap, on top of the others).
/// Shortest first, a cable takes the lane under every lit cable its span overlaps, so a
/// cable nested in another hangs above it (nested cables never cross, `kit::cable`) and
/// an output and a send between the same strips are two lines; the lanes then share the
/// band's depth. A lone lit cable keeps the depth its span asks for. Faint cables keep
/// theirs. Nothing allocates once the scratch lists have grown.
fn lanes(band: &mut Band, room: f32) {
    band.order.clear();
    band.order.extend(
        band.drawn
            .iter()
            .enumerate()
            .filter(|(_, d)| d.lit)
            .map(|(i, _)| i as u32),
    );
    if band.order.len() < 2 {
        return;
    }
    let span = |d: &Drawn| (d.to.x - d.from.x).abs();
    let drawn = &band.drawn;
    band.order
        .sort_unstable_by(|a, b| span(&drawn[*a as usize]).total_cmp(&span(&drawn[*b as usize])));
    band.lane.clear();
    band.lane.resize(band.drawn.len(), 0);
    let mut most = 1u8;
    for k in 0..band.order.len() {
        let d = band.drawn[band.order[k] as usize];
        let (lo, hi) = (d.from.x.min(d.to.x), d.from.x.max(d.to.x));
        let mut lane = 1u8;
        for j in 0..k {
            let o = band.drawn[band.order[j] as usize];
            let (olo, ohi) = (o.from.x.min(o.to.x), o.from.x.max(o.to.x));
            if olo < hi - 0.5 && ohi > lo + 0.5 {
                lane = lane.max(band.lane[band.order[j] as usize].saturating_add(1));
            }
        }
        band.lane[band.order[k] as usize] = lane;
        most = most.max(lane);
    }
    if most < 2 {
        return;
    }
    let floor = cable::MIN_DEPTH.min(room);
    for &i in &band.order {
        let lane = band.lane[i as usize];
        band.drawn[i as usize].depth =
            floor + (room - floor) * f32::from(lane - 1) / f32::from(most - 1);
    }
}

/// How deep a kind hangs, of the depth its span asks for: an output lowest, a send above
/// it, a key above both — so an output and a send between the same two strips (a doubled
/// signal, answer 5) or a key beside them are two cables, not one.
fn depth_share(kind: &EdgeKind) -> f32 {
    match kind {
        EdgeKind::Output => 1.0,
        EdgeKind::Send { .. } => 0.78,
        EdgeKind::Key { .. } => 0.6,
    }
}

/// How a drawn cable looks: its kind from its edge, its source's colour.
fn look_of(v: &MixerView, s: &Style, d: &Drawn) -> CableLook {
    let i = d.edge as usize;
    let (fi, _) = v.desk.ends[i];
    let e = v.desk.edges[i];
    let kind = match e.kind {
        EdgeKind::Output if e.to.is_master() => CableKind::Master,
        EdgeKind::Output => CableKind::Output,
        EdgeKind::Send { enabled, tap, .. } => CableKind::Send {
            on: enabled,
            pre: !tap.is_post(),
        },
        EdgeKind::Key { .. } => CableKind::Key,
    };
    CableLook {
        kind,
        color: strip_color(v, s, fi as usize).unwrap_or(s.t.label),
        lit: d.lit,
        selected: v.band.selected == Some(d.route),
        doubled: d.lit && v.desk.doubled.get(i).copied().unwrap_or(false),
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
        if !d.lit || !(!compact || hot == Some(k) || chosen) {
            continue;
        }
        let path = cable::path(d.from, d.to, d.depth);
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
        let r = Rect::from_center_size(path.low, vec2(vw + tw + 14.0, PILL_H - 1.0));
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
        if whole.top() < port_y + s.s(PORT_R) + 1.0
            || whole.left() < clip.0
            || whole.right() > clip.1
            || v.band.pills.iter().any(|p| p.intersects(whole.expand(2.0)))
        {
            continue;
        }
        v.band.pills.push(whole);
        let look = look_of(v, s, &d);
        let ink = cable::ink(s, &look);
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
        if let (Some(send), Some(p)) = (send, knob_input(v, ui, pass, &resp, level, 1.0)) {
            app.set_strip_send(d.route.from, Send { level: p, ..send });
        }
        if resp.drag_stopped() {
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

/// Every port on screen: the master's, each strip's, a collapsed group's at its
/// separator (where its members' cables gather).
#[allow(clippy::too_many_arguments)]
fn port_row(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &Ui,
    b: &mut Brush,
    pass: &Pass,
    geo: &Geo,
    ports: &Ports,
    port_y: f32,
) {
    let dragging = drag::active(v);
    port(
        v,
        app,
        ui,
        b,
        pass,
        None,
        pos2(ports.master, port_y),
        dragging,
    );
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
            let c = pos2(x + (w - 1.0) * 0.5, port_y);
            x += w;
            if dock == Dock::Middle && (c.x < ports.shown.0 || c.x > ports.shown.1) {
                continue;
            }
            match item {
                super::desk::Item::Strip(id, index) => {
                    let lit = dragging || in_selection(app, id);
                    port(v, app, ui, b, pass, Some((id, index)), c, lit);
                }
                super::desk::Item::Separator(g) => {
                    let collapsed = app.project().strip_group(g).is_some_and(|g| g.collapsed);
                    if collapsed {
                        let s = pass.s;
                        b.circle_stroke(c, s.s(PORT_R), s.s(1.2), with_alpha(s.t.text, 0.6));
                    }
                }
            }
        }
    }
}

/// One port: a ring in the strip's colour, filled when the strip goes somewhere other
/// than the master; larger while a drag is on or the strip is chosen. A press starts a
/// cable (`mixer_drag`). The master's ring is a destination only.
#[allow(clippy::too_many_arguments)]
fn port(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &Ui,
    b: &mut Brush,
    pass: &Pass,
    strip: Option<(StripId, usize)>,
    c: Pos2,
    lit: bool,
) {
    let s = pass.s;
    let t = &s.t;
    let resp = strip.and_then(|(id, _)| {
        let r = Rect::from_center_size(c, PORT_HIT);
        sense(
            ui,
            pass,
            r,
            pid(Some(id), Part::Port),
            Sense::click_and_drag(),
        )
    });
    let hovered = resp.as_ref().is_some_and(|r| r.hovered());
    let (color, goes) = match strip {
        Some((_, i)) => (
            strip_color(v, s, i).unwrap_or(t.label),
            v.desk.info(i).is_some_and(|x| x.goes),
        ),
        None => (t.text_dim, false),
    };
    let r = s.s(PORT_R) * if lit || hovered { 1.2 } else { 1.0 };
    let w = s.s(1.4);
    b.dot(c, r + w * 0.5, t.strip);
    if goes {
        b.dot(c, r + w * 0.5, color);
    } else {
        b.circle_stroke(c, r, w, color);
    }
    if hovered {
        b.circle_stroke(c, r + s.s(2.5), b.px(), with_alpha(t.text, 0.6));
    }
    if strip.is_some_and(|(id, _)| drag::armed(v) == Some(id)) {
        // Click mode: armed — a ring in the selection's colour around the port, and its
        // core lit, until the click on a destination (or Esc) lets it go.
        b.circle_stroke(c, r + s.s(3.0), s.s(1.6), t.selection);
        b.dot(c, r * 0.55, t.selection);
    }
    let (Some(resp), Some((id, _))) = (resp, strip) else {
        return;
    };
    drag::source(v, app, ui, &resp, Source::Port(id));
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        hint::set_text(ui.ctx(), &v.words.port, "", Caps::NONE);
    }
}

/// The ends of lit cables, as drag sources: the destination's mark moves where the route
/// goes; the selected cable's source end (a dot on its first corner) moves where it comes
/// from — the one way a key moves (a key belongs to its insert).
fn ends(v: &mut MixerView, app: &mut CantusApp, ui: &Ui, b: &mut Brush, pass: &Pass) {
    let s = pass.s;
    for k in 0..v.band.drawn.len() {
        let d = v.band.drawn[k];
        if !d.lit {
            continue;
        }
        let path = cable::path(d.from, d.to, d.depth);
        let chosen = v.band.selected == Some(d.route);
        for (end, at, chip) in [
            (End::Destination, path.mark, d.chips[1]),
            (End::Source, path.tail, d.chips[0]),
        ] {
            if chip || (end == End::Source && !chosen) {
                continue;
            }
            if end == End::Source {
                let look = look_of(v, s, &d);
                b.dot(at, s.s(2.4), cable::ink(s, &look));
            }
            let mut r = Rect::from_center_size(at, vec2(10.0, 10.0));
            // Never over the port beside it: a press on a port always starts a new cable
            // (and a click on it arms one), even where a lit cable ends.
            let port_at = if end == End::Destination {
                d.to
            } else {
                d.from
            };
            let port = Rect::from_center_size(port_at, PORT_HIT);
            if r.intersects(port) {
                // Just under the port instead, where the cable leaves it: still there to
                // grab whatever lane the cable takes.
                r = Rect::from_min_size(pos2(r.left(), port.max.y), vec2(r.width(), 8.0));
            }
            let id = egui::Id::new(("mixer-cable-end", d.edge, end == End::Source));
            if let Some(resp) = sense(ui, pass, r, id, Sense::drag()) {
                drag::source(
                    v,
                    app,
                    ui,
                    &resp,
                    Source::End {
                        route: d.route,
                        end,
                    },
                );
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                    hint::set_text(ui.ctx(), &v.words.cable, "", Caps::NONE);
                }
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
    if esc {
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

/// The hint bar's words for cable `k`: its two strips into `hint_buf` ("3 Keys → 15
/// Reverb"), what it is into `buf` ("send · −6.0 dB · pre-fader").
fn describe(v: &mut MixerView, app: &CantusApp, k: usize) {
    let d = v.band.drawn[k];
    let i = d.edge as usize;
    let (fi, ti) = v.desk.ends[i];
    let e = v.desk.edges[i];
    let mut names = std::mem::take(&mut v.hint_buf);
    names.clear();
    write_strip(&mut names, v, app, fi);
    names.push_str(" \u{2192} ");
    write_strip(&mut names, v, app, ti);
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

/// "3 Keys" (the number and the desk's name), or "M" for the master.
fn write_strip(out: &mut String, v: &MixerView, app: &CantusApp, index: u32) {
    if index == MASTER_END {
        out.push_str(&v.words.master);
        return;
    }
    let _ = write!(out, "{}", index + 1);
    let name = v.desk.name(app, index as usize);
    if !name.is_empty() {
        out.push(' ');
        out.push_str(name);
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

/// The doubled-signal mark at the size of a send row.
pub(super) fn doubled_mark_small(b: &mut Brush, s: &Style, center: Pos2) {
    cable::doubled_mark(b, s, center, s.s(9.0));
}

const BAND_TOKENS: &[Tok] = &[
    Tok::Strip,
    Tok::Panel,
    Tok::Text,
    Tok::Danger,
    Tok::Selection,
];

#[cfg(test)]
mod tests {
    //! The band's geometry without a frame: the lanes of lit cables (integration finding
    //! 25).
    use super::*;

    fn cable(i: u32, from: f32, to: f32, lit: bool) -> Drawn {
        Drawn {
            route: Route::send(StripId(100 + i as u64), StripId(200 + i as u64)),
            from: pos2(from, 6.0),
            to: pos2(to, 6.0),
            depth: cable::depth_for(pos2(from, 6.0), pos2(to, 6.0), 30.0),
            lit,
            chips: [false, false],
            edge: i,
        }
    }

    /// Two lit cables whose spans overlap never hang at one depth (they would share a
    /// flat run and read as one line), in any order and nesting; a cable nested in another
    /// hangs above it; faint cables keep the depth their span asks for.
    #[test]
    fn no_two_lit_cables_share_a_flat_run() {
        let spans: [(f32, f32); 7] = [
            (10.0, 400.0),
            (40.0, 120.0),
            (60.0, 300.0),
            (400.0, 10.0),
            (500.0, 900.0),
            (100.0, 140.0),
            (120.0, 600.0),
        ];
        for rot in 0..spans.len() {
            let mut band = Band::default();
            for k in 0..spans.len() {
                let (a, z) = spans[(k + rot) % spans.len()];
                band.drawn.push(cable(k as u32, a, z, true));
            }
            let faint = cable(99, 20.0, 700.0, false);
            band.drawn.push(faint);
            lanes(&mut band, 30.0);
            let d = &band.drawn;
            for i in 0..d.len() {
                for j in i + 1..d.len() {
                    if !(d[i].lit && d[j].lit) {
                        continue;
                    }
                    let (alo, ahi) = (d[i].from.x.min(d[i].to.x), d[i].from.x.max(d[i].to.x));
                    let (blo, bhi) = (d[j].from.x.min(d[j].to.x), d[j].from.x.max(d[j].to.x));
                    if alo < bhi - 0.5 && ahi > blo + 0.5 {
                        assert!(
                            (d[i].depth - d[j].depth).abs() > 0.5,
                            "rot {rot}: {:?} and {:?} share a flat run at {}",
                            (alo, ahi),
                            (blo, bhi),
                            d[i].depth
                        );
                        // Nested: the inner one hangs above (less deep).
                        if alo >= blo && ahi <= bhi {
                            assert!(d[i].depth < d[j].depth, "rot {rot}: nested below");
                        } else if blo >= alo && bhi <= ahi {
                            assert!(d[j].depth < d[i].depth, "rot {rot}: nested below");
                        }
                    }
                }
            }
            let f = band.drawn.last().unwrap();
            assert_eq!(f.depth, faint.depth, "a faint cable keeps its depth");
            assert!(
                band.drawn.iter().all(|d| d.depth <= 30.0 + 1e-3),
                "in the band"
            );
        }
    }
}
