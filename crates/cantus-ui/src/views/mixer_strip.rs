//! One strip on the desk — and the master, a group's separator, the `+` — painted with
//! the kit and sensed control by control, only while it is on screen.
//!
//! Every control follows the house rules (`widgets::knob`, `widgets::fader`): 200 px of
//! drag for the whole travel, Shift for fine control, a double-click resets, the arrows
//! move it while it has keyboard focus (and are claimed, so they do not also nudge the
//! notes), and the wheel moves it **only with Ctrl held** (ADR 0024 §6). Each writes the
//! hint bar's line while hovered.

use super::desk::Item;
use super::layout::{Fit, Row, WidthMode};
use super::menu::Menu;
use super::{in_selection, DeskPrefs, Geo, MixerView, Renaming};
use crate::app::CantusApp;
use crate::kit::color::{mix, on_fill, safe_track, shade, with_alpha, FillSize};
use crate::kit::fader::{self as kfader, FaderLook};
use crate::kit::glyph::{self, Glyph};
use crate::kit::hint::{self, Caps};
use crate::kit::knob::{self, KnobLook, KnobSize};
use crate::kit::meter::{self, MeterBatch};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{led, light, Brush, Fill, State, Style, Tok};
use crate::widgets::fader::{db_at, pos_of, UNITY_POS};
use cantus_core::{Command, EqField, MergeKey, StripId, EQ_MAX_GAIN_DB, MAX_GAIN_DB, MIN_GAIN_DB};
use cantus_engine::CaptureTarget;
use egui::{
    pos2, vec2, Align2, Color32, EventFilter, Id, Key, Modifiers, Pos2, Rect, Response, Sense, Ui,
};
use std::fmt::Write as _;

/// A drag this long covers a control's whole travel.
const TRAVEL_PX: f32 = 200.0;
/// Shift: a fifth of the distance.
const FINE: f32 = 0.2;

/// What every strip of a frame shares.
pub(super) struct Pass<'a> {
    pub s: &'a Style,
    pub fit: &'a Fit,
    /// The screen y of the top of the first row.
    pub y0: f32,
    /// Where strips are visible: nothing is sensed outside it.
    pub clip: Rect,
    /// egui's frame time: the meters' clock.
    pub now: f64,
    pub prefs: DeskPrefs,
    pub wave: bool,
    pub mods: Modifiers,
    pub meters: MeterBatch,
}

impl<'a> Pass<'a> {
    pub fn new(
        s: &'a Style,
        fit: &'a Fit,
        geo: &Geo,
        ui: &Ui,
        prefs: DeskPrefs,
        wave: bool,
    ) -> Pass<'a> {
        let (now, mods) = ui.input(|i| (i.time, i.modifiers));
        Pass {
            s,
            fit,
            y0: geo.y0,
            clip: geo.rows,
            now,
            prefs,
            wave,
            mods,
            meters: MeterBatch::new(s),
        }
    }

    /// A row's screen span.
    pub(super) fn row(&self, r: Row) -> Option<(f32, f32)> {
        self.fit.row(r).map(|(a, z)| (self.y0 + a, self.y0 + z))
    }

    pub(super) fn mid(&self, r: Row) -> Option<f32> {
        self.row(r).map(|(a, z)| (a + z) * 0.5)
    }

    /// The strip's whole height on screen.
    pub(super) fn body(&self, x: f32, w: f32) -> Rect {
        Rect::from_min_max(pos2(x, self.y0), pos2(x + w, self.y0 + self.fit.height))
    }

    pub(super) fn mode(&self) -> WidthMode {
        self.prefs.width
    }
}

/// The parts of a strip that are sensed, for their ids.
#[derive(Copy, Clone, Debug, Hash)]
pub(super) enum Part {
    Head,
    Meter,
    Peak,
    Mute,
    Solo,
    Pan,
    Fader,
    Phase,
    Swap,
    Mono,
    Width,
    Eq(u8),
    Fx,
    Rec,
    Send,
    Gr,
    // Round 25.
    Chain,
    Output,
    SendRow(u8),
    SendAdd,
    Port,
}

/// The desk's controls, as tests and tools name them.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Control {
    /// The number cell and the name.
    Head,
    Meter,
    Mute,
    Solo,
    Pan,
    Fader,
    Phase,
    Swap,
    Mono,
    Width,
    /// The low (0), mid (1) or high (2) EQ bar.
    Eq(u8),
    Fx,
    Record,
    /// The sends row: an arrow, a send knob, or the selected strip's route.
    Send,
    /// The chain in one line (Extra large, round 25).
    Chain,
    /// The output chip (Wide and up, round 25).
    Output,
    /// The strip's own send row `n` (0-based; Extra large, round 25).
    SendRow(u8),
    /// The "+ Send" row after the last send (Extra large, round 25).
    SendAdd,
    /// The strip's port in the cable band (round 25).
    Port,
}

impl Control {
    pub(super) fn part(self) -> Part {
        match self {
            Control::Head => Part::Head,
            Control::Meter => Part::Meter,
            Control::Mute => Part::Mute,
            Control::Solo => Part::Solo,
            Control::Pan => Part::Pan,
            Control::Fader => Part::Fader,
            Control::Phase => Part::Phase,
            Control::Swap => Part::Swap,
            Control::Mono => Part::Mono,
            Control::Width => Part::Width,
            Control::Eq(i) => Part::Eq(i),
            Control::Fx => Part::Fx,
            Control::Record => Part::Rec,
            Control::Send => Part::Send,
            Control::Chain => Part::Chain,
            Control::Output => Part::Output,
            Control::SendRow(n) => Part::SendRow(n),
            Control::SendAdd => Part::SendAdd,
            Control::Port => Part::Port,
        }
    }
}

pub(super) fn pid(strip: Option<StripId>, part: Part) -> Id {
    Id::new(("mixer-strip", strip.map(|s| s.0), part))
}

/// Sense `rect` (clipped to where strips show); `None` when none of it shows.
pub(super) fn sense(ui: &Ui, pass: &Pass, rect: Rect, id: Id, sense: Sense) -> Option<Response> {
    let r = rect.intersect(pass.clip);
    (r.width() > 0.5 && r.height() > 0.5).then(|| ui.interact(r, id, sense))
}

/// A control's keyboard: claim the arrows while focused, and return the steps pressed
/// (up/right positive).
pub(super) fn arrows(ui: &Ui, resp: &Response) -> i32 {
    if !resp.has_focus() {
        return 0;
    }
    ui.memory_mut(|m| {
        m.set_focus_lock_filter(
            resp.id,
            EventFilter {
                horizontal_arrows: true,
                vertical_arrows: true,
                ..Default::default()
            },
        )
    });
    crate::widgets::claim_arrow_keys(ui.ctx(), resp.id);
    ui.input(|i| {
        (i.num_presses(Key::ArrowUp) + i.num_presses(Key::ArrowRight)) as i32
            - (i.num_presses(Key::ArrowDown) + i.num_presses(Key::ArrowLeft)) as i32
    })
}

/// A knob's gestures on a position (0…1): drag, Ctrl+wheel, arrows, double-click to
/// `default`. Returns the new position when it moved. `drags` false while a drag is a
/// modulation's depth (Alt, or a modulator mapping): only the drag is left out, as in the
/// kit's `Param` (review 2).
pub(super) fn knob_input(
    v: &MixerView,
    ui: &Ui,
    pass: &Pass,
    resp: &Response,
    pos: f32,
    default: f32,
    drags: bool,
) -> Option<f32> {
    let fine = if pass.mods.shift { FINE } else { 1.0 };
    let mut p = pos;
    if resp.dragged() && drags {
        let d = resp.drag_delta();
        p += (d.x - d.y) * fine / TRAVEL_PX;
    }
    if resp.hovered() && pass.mods.command && v.ctrl_notches != 0.0 {
        p += v.ctrl_notches * 0.02 * fine;
    }
    let steps = arrows(ui, resp);
    if steps != 0 {
        p += steps as f32 * if fine < 1.0 { 0.001 } else { 0.01 };
    }
    if resp.double_clicked() {
        p = default;
    }
    let p = p.clamp(0.0, 1.0);
    ((p - pos).abs() > f32::EPSILON).then_some(p)
}

/// A fader's gestures on a gain in dB: drag, a click in the groove jumps there, Ctrl+wheel
/// (1 dB a notch), arrows (1 dB), double-click to unity. Returns the new gain when it
/// moved. `drags` as in [`knob_input`].
pub(super) fn fader_input(
    v: &MixerView,
    ui: &Ui,
    pass: &Pass,
    resp: &Response,
    rect: Rect,
    db: f32,
    drags: bool,
) -> Option<f32> {
    let s = pass.s;
    let fine = if pass.mods.shift { FINE } else { 1.0 };
    let start = db.clamp(MIN_GAIN_DB, MAX_GAIN_DB);
    let mut pos = pos_of(start);
    let travel = (rect.height() - kfader::CAP.y * s.k).max(1.0);
    if resp.dragged() {
        if drags {
            pos -= resp.drag_delta().y * fine / travel;
        }
    } else if resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            let cap = kfader::cap_rect(rect, s, pos);
            if !cap.expand(1.0).contains(p) {
                pos = kfader::pos_at(rect, s, p.y);
            }
        }
    }
    let mut value = db_at(pos.clamp(0.0, 1.0));
    if resp.hovered() && pass.mods.command && v.ctrl_notches != 0.0 {
        value = (value + v.ctrl_notches * fine).clamp(MIN_GAIN_DB, MAX_GAIN_DB);
    }
    let steps = arrows(ui, resp);
    if steps != 0 {
        value = (value + steps as f32 * fine).clamp(MIN_GAIN_DB, MAX_GAIN_DB);
    }
    if resp.double_clicked() {
        value = 0.0;
    }
    (value != start).then_some(value)
}

/// "−6.0 dB", "−∞ dB", "+1.5 dB" into `b`.
pub(super) fn write_db(b: &mut String, db: f32, unit: bool) {
    if db <= MIN_GAIN_DB {
        b.push_str("\u{2212}\u{221e}");
    } else if db < -0.05 {
        let _ = write!(b, "\u{2212}{:.1}", -db);
    } else {
        let _ = write!(b, "{:+.1}", db.max(0.0));
    }
    if unit {
        b.push_str(" dB");
    }
}

/// "C", "L30", "R25".
pub(super) fn write_pan(b: &mut String, pan: f32) {
    if pan.abs() < 0.005 {
        b.push('C');
    } else if pan < 0.0 {
        let _ = write!(b, "L{:.0}", -pan * 100.0);
    } else {
        let _ = write!(b, "R{:.0}", pan * 100.0);
    }
}

/// Paint a lamp's hit area: the lamp itself.
pub(super) fn lamp(b: &mut Brush, s: &Style, c: Pos2, lit: bool, color: Tok, hovered: bool) {
    led::paint(b, c, s.s(9.0), lit, s, color, Tok::Strip, 1.0);
    if hovered {
        b.circle_stroke(c, s.s(6.0), b.px(), with_alpha(s.t.text, 0.5));
    }
}

/// A glyph toggle: dim at rest, the highlight when on, full text when hovered.
pub(super) fn glyph_toggle(
    b: &mut Brush,
    s: &Style,
    rect: Rect,
    g: Glyph,
    on: bool,
    hovered: bool,
    on_color: Color32,
) {
    let c = if on {
        on_color
    } else if hovered {
        s.t.text
    } else {
        with_alpha(s.t.text_dim, 0.62)
    };
    glyph::paint(b, rect, g, c);
}

/// Draw one desk item.
#[allow(clippy::too_many_arguments)]
pub(super) fn item(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    item: Item,
    x: f32,
    w: f32,
) {
    match item {
        Item::Separator(g) => super::parts::separator(v, app, ui, b, pass, g, x, w),
        Item::Strip(id, index) => {
            v.laid_out += 1;
            strip(v, app, ui, b, pass, id, index, x, w);
        }
    }
}

/// The values a strip shows, copied out of the project so its controls can issue commands
/// while the strip is painted.
#[derive(Copy, Clone, Debug)]
pub(super) struct Snap {
    pub(super) id: StripId,
    pub(super) index: usize,
    pub(super) gain: f32,
    pub(super) pan: f32,
    pub(super) width: f32,
    pub(super) phase: bool,
    pub(super) swap: bool,
    pub(super) muted: bool,
    pub(super) soloed: bool,
    pub(super) fx_on: bool,
    pub(super) effects: usize,
    pub(super) eq: [f32; 3],
    pub(super) eq_hz: [f32; 3],
    pub(super) output: Option<StripId>,
}

impl Snap {
    /// The master's values in a strip's shape, for the parts they share.
    pub(super) fn master(m: &cantus_core::Master) -> Snap {
        Snap {
            id: StripId::MASTER,
            index: usize::MAX,
            gain: m.gain_db,
            pan: m.pan,
            width: m.width,
            phase: m.phase_invert,
            swap: false,
            muted: false,
            soloed: false,
            fx_on: m.fx_enabled,
            effects: m.effects.len(),
            eq: [0.0; 3],
            eq_hz: [0.0; 3],
            output: None,
        }
    }
}

pub(super) fn snap(app: &CantusApp, index: usize) -> Option<Snap> {
    let s = app.project().strips.get(index)?;
    let mut eq = [0.0; 3];
    let mut eq_hz = [0.0; 3];
    for (i, band) in s.eq.bands.iter().take(3).enumerate() {
        eq[i] = band.gain_db;
        eq_hz[i] = band.freq_hz;
    }
    // An automated fader, pan or width shows what its clip gives it (ADR 0027).
    let auto = |t: cantus_core::AutoTarget, own: f32| {
        app.automated_value_where(|x| *x == t)
            .flatten()
            .unwrap_or(own)
    };
    Some(Snap {
        id: s.id,
        index,
        gain: auto(cantus_core::AutoTarget::StripGain(s.id), s.gain_db),
        pan: auto(cantus_core::AutoTarget::StripPan(s.id), s.pan),
        width: auto(cantus_core::AutoTarget::StripWidth(s.id), s.width),
        phase: s.phase_invert,
        swap: s.swap_channels,
        muted: s.muted,
        soloed: s.soloed,
        fx_on: s.fx_enabled,
        effects: s.effects.len(),
        eq,
        eq_hz,
        output: s.output,
    })
}

/// The colour a strip wears, clamped to the safe band.
pub(super) fn strip_color(v: &MixerView, s: &Style, index: usize) -> Option<Color32> {
    v.desk
        .info(index)
        .and_then(|i| i.color)
        .map(|c| safe_track(crate::theme::track_color(c), s.t.dark, FillSize::Small))
}

/// Select on a click, FL's way: Ctrl adds or takes out, Shift selects the run from the
/// selected strip, a plain click selects this one.
pub(super) fn click_select(app: &mut CantusApp, id: StripId, mods: Modifiers) {
    if mods.command {
        app.toggle_strip_selected(id);
    } else if mods.shift {
        let p = app.project();
        let (Some(from), Some(to)) = (
            app.selected_strip.and_then(|s| p.strip_index(s)),
            p.strip_index(id),
        ) else {
            app.select_strip(id);
            return;
        };
        let run: Vec<StripId> = p.strips[from.min(to)..=from.max(to)]
            .iter()
            .map(|s| s.id)
            .collect();
        app.select_strip(id);
        app.selected_strips = run;
    } else {
        app.select_strip(id);
    }
}

/// Grabbing a control of a strip outside the selection selects it; inside a
/// multi-selection it keeps the selection, so the faders move together.
pub(super) fn touch(app: &mut CantusApp, id: StripId, resp: &Response) {
    if (resp.drag_started() || resp.clicked()) && !in_selection(app, id) {
        app.select_strip(id);
    }
}

#[allow(clippy::too_many_arguments)]
fn strip(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    id: StripId,
    index: usize,
    x: f32,
    w: f32,
) {
    let Some(sn) = snap(app, index).filter(|s| s.id == id) else {
        return;
    };
    let s = pass.s;
    let t = &s.t;
    let bw = w - 1.0;
    let selected = app.selected_strip == Some(id);
    let chosen = in_selection(app, id);
    let color = strip_color(v, s, index);
    let body = pass.body(x, bw);
    b.register(body, "mixer_strip", STRIP_TOKENS, &[]);
    b.rect(
        b.snap_rect(body),
        Fill::Solid(if chosen { t.strip_selected } else { t.strip }),
    );
    let a = pass.prefs.colourful.alpha();
    if let (Some(c), true) = (color, a > 0.0) {
        b.rect(
            b.snap_rect(body),
            Fill::V(with_alpha(c, a), with_alpha(c, a * 0.45)),
        );
    }
    // The column as a drop target while a drag is on (round 25: `mixer_drag` says how it
    // looks, `mixer_cables` paints it).
    let target = super::drag::target(v, app, ui, Some(id), body);
    super::cables::column_hatch(b, s, body, target);
    head(v, app, ui, b, pass, &sn, x, bw, color, selected, chosen);
    meter_row(v, app, ui, b, pass, &sn, x, bw, color);
    controls(v, app, ui, b, pass, &sn, x, bw, chosen);
    super::parts::chain_row(v, app, ui, b, pass, &sn, x, bw);
    super::parts::output_row(v, app, ui, b, pass, &sn, x, bw);
    super::parts::send_list(v, app, ui, b, pass, &sn, x, bw);
    super::parts::sends_row(v, app, ui, b, pass, &sn, x, bw);
}

/// The tokens a strip's body paints with (its parts register their own).
const STRIP_TOKENS: &[Tok] = &[
    Tok::Strip,
    Tok::StripSelected,
    Tok::HeaderCell,
    Tok::Text,
    Tok::Label,
];

/// The number cell and the name: one zone that selects, renames (double-click) and opens
/// the strip's menu (right-click).
#[allow(clippy::too_many_arguments)]
fn head(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &Pass,
    sn: &Snap,
    x: f32,
    bw: f32,
    color: Option<Color32>,
    selected: bool,
    chosen: bool,
) {
    let s = pass.s;
    let t = &s.t;
    let Some((na, nz)) = pass.row(Row::Number) else {
        return;
    };
    let cell = Rect::from_min_max(pos2(x, na), pos2(x + bw, nz));
    let name_row = pass.row(Row::Name);
    let zone = Rect::from_min_max(cell.min, pos2(x + bw, name_row.map_or(nz, |r| r.1)));
    let resp = sense(ui, pass, zone, pid(Some(sn.id), Part::Head), Sense::click());
    if let Some(r) = &resp {
        // The number cell drags the strip (or the selection) along the desk (round 25).
        super::drag::source(v, app, ui, r, super::drag::Source::Strip(sn.id));
    }
    let fill = color.unwrap_or(t.header_cell);
    let cr = b.snap_rect(cell);
    b.rect(cr, Fill::V(shade(fill, 0.03), shade(fill, -0.03)));
    if selected || chosen {
        let edge = if selected { t.text } else { t.selection };
        b.rrect_stroke(cr.shrink(b.px() * 1.5), 0.0, b.px() * 1.5, edge);
    } else if resp.as_ref().is_some_and(|r| r.hovered()) {
        b.rrect_stroke(cr.shrink(b.px()), 0.0, b.px(), with_alpha(t.text, 0.4));
    }
    v.buf.clear();
    let _ = write!(v.buf, "{}", sn.index + 1);
    let _ = text::paint(
        b,
        cr,
        &Text::new(&v.buf, Role::Label, on_fill(fill)),
        s.tk(),
    );
    let renaming = matches!(v.rename, Some((Renaming::Strip(r), _)) if r == sn.id);
    if renaming {
        v.rename_at = Some(cell);
    }
    let name = v.desk.name(app, sn.index);
    if let (Some((a, z)), false) = (name_row, renaming) {
        let ink = if selected { t.text } else { t.label };
        let nt = if pass.fit.rotated {
            Text::new(name, Role::Label, ink)
                .align(Align2::CENTER_BOTTOM)
                .policy(Policy::Rotate90(Elide::Middle))
        } else {
            Text::new(name, Role::Label, ink).policy(Policy::Elide(Elide::Middle))
        };
        let nr = Rect::from_min_max(pos2(x + 2.0, a + 1.0), pos2(x + bw - 2.0, z - 1.0));
        let _ = text::paint(b, nr, &nt, s.tk());
    }
    let Some(resp) = resp else {
        return;
    };
    if resp.hovered() {
        v.hint_buf.clear();
        let _ = write!(v.hint_buf, "{} {}", v.words.name, sn.index + 1);
        hint::set_text(
            ui.ctx(),
            &v.hint_buf,
            name,
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    // Who feeds the strip — tracks, outputs, sends, keys, notes (round 25: a return fed
    // only by sends no longer says nothing feeds it). Built only while hovered.
    let resp = resp.on_hover_ui(|ui| {
        ui.label(crate::routing_actions::feeders_text(app.project(), sn.id));
    });
    if resp.double_clicked() {
        let own = app
            .project()
            .strip(sn.id)
            .map(|s| s.name.clone())
            .unwrap_or_default();
        v.rename = Some((Renaming::Strip(sn.id), own));
    } else if resp.clicked() {
        click_select(app, sn.id, pass.mods);
    }
    if resp.secondary_clicked() {
        let at = resp.interact_pointer_pos().unwrap_or(zone.left_bottom());
        v.popups.open(Menu::Strip(sn.id), at);
    }
}

/// The meter width per width mode.
pub(super) fn meter_w(mode: WidthMode) -> f32 {
    match mode {
        WidthMode::Compact => 16.0,
        WidthMode::Wide => 22.0,
        WidthMode::Xl => 28.0,
    }
}

/// The meter (or the wave view), the gain-reduction lane of a strip with a compressor,
/// and — from Wide up — the held peak above it (a click forgets it, as in FL).
#[allow(clippy::too_many_arguments)]
fn meter_row(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    sn: &Snap,
    x: f32,
    bw: f32,
    color: Option<Color32>,
) {
    let Some((a, z)) = pass.row(Row::Meter) else {
        return;
    };
    let s = pass.s;
    let peak = app
        .engine
        .meters()
        .strips
        .get(sn.index)
        .copied()
        .unwrap_or(0.0);
    let levels = v.meters.strip(sn.index, peak, pass.now, pass.wave);
    let compressor = v.desk.info(sn.index).is_some_and(|i| i.compressor);
    let mut top = a;
    if pass.mode() != WidthMode::Compact && z - a >= 72.0 {
        let pr = Rect::from_min_max(pos2(x + 3.0, a), pos2(x + bw - 3.0, a + 15.0));
        let held = v.meters.held(sn.index);
        peak_readout(v, ui, b, pass, pr, held, Some(sn.index), Some(sn.id));
        top = a + 18.0;
    }
    let (mr, lane) = meter_rects(pass.mode(), x, bw, top, z, compressor);
    if pass.wave {
        wave(b, s, mr, v.meters.wave(sn.index), color);
    } else {
        pass.meters.paint(b, mr, &levels);
    }
    if !v.desk.fed(sn.index) {
        // Nothing feeds it: the Route here plate in the silent well (round 25).
        super::routehere::plate(v, app, ui, b, pass, sn.id, mr);
    }
    if let Some(lane) = lane {
        let gr = app.comp_gr_on_strip(sn.id).flatten();
        if gr_lane(b, s, lane, gr) {
            v.lanes_filling += 1;
        }
        if let Some(resp) = sense(ui, pass, lane, pid(Some(sn.id), Part::Gr), Sense::hover()) {
            if resp.hovered() {
                hint::set(ui.ctx(), &v.words.gr, Caps::NONE, false, |b| match gr {
                    Some(g) => write!(b, "{g:.1} dB"),
                    None => Ok(()),
                });
            }
        }
    }
    if let Some(resp) = sense(ui, pass, mr, pid(Some(sn.id), Part::Meter), Sense::click()) {
        meter_hint(v, ui, &resp, mr);
        if resp.clicked() {
            click_select(app, sn.id, pass.mods);
        }
    }
}

/// Where the meter and the gain-reduction lane go, centred in the strip.
pub(super) fn meter_rects(
    mode: WidthMode,
    x: f32,
    bw: f32,
    top: f32,
    bottom: f32,
    lane: bool,
) -> (Rect, Option<Rect>) {
    let mw = meter_w(mode);
    let lw = if lane { 5.0 } else { 0.0 };
    let total = mw + lw;
    let left = x + (bw - total) * 0.5;
    let mr = Rect::from_min_max(pos2(left, top), pos2(left + mw, bottom));
    let lr = lane
        .then(|| Rect::from_min_max(pos2(mr.right() + 1.0, top), pos2(mr.right() + lw, bottom)));
    (mr, lr)
}

/// The level under the pointer, in the hint bar.
pub(super) fn meter_hint(v: &mut MixerView, ui: &Ui, resp: &Response, mr: Rect) {
    if let Some(p) = resp.hover_pos() {
        let pos = (mr.bottom() - p.y) / mr.height().max(1.0);
        hint::set(ui.ctx(), &v.words.level, Caps::NONE, false, |b| {
            write!(b, "{:.1} dB", meter::db_at(pos))
        });
    }
}

/// The held peak as a number: `−∞` for silence, one decimal otherwise, in the record
/// colour once it has touched full scale — how a strip that clipped once is found without
/// watching all of them.
#[allow(clippy::too_many_arguments)]
pub(super) fn peak_readout(
    v: &mut MixerView,
    ui: &Ui,
    b: &mut Brush,
    pass: &mut Pass,
    rect: Rect,
    held: f32,
    index: Option<usize>,
    id: Option<StripId>,
) {
    let s = pass.s;
    v.buf.clear();
    if held <= meter::MIN_DB {
        v.buf.push_str("\u{2212}\u{221e}");
    } else if held < 0.0 {
        let _ = write!(v.buf, "\u{2212}{:.1}", -held);
    } else {
        let _ = write!(v.buf, "+{:.1}", held);
    }
    let ink = if held >= 0.0 {
        s.t.danger
    } else {
        s.t.text_dim
    };
    let tt = Text::new(&v.buf, Role::Value, ink).policy(Policy::Elide(Elide::End));
    let _ = text::paint(b, rect, &tt, s.tk() * 0.85);
    if let Some(resp) = sense(ui, pass, rect, pid(id, Part::Peak), Sense::click()) {
        if resp.hovered() {
            hint::set_text(ui.ctx(), &v.words.peak, &v.buf, Caps::NONE);
        }
        if resp.clicked() {
            v.meters.reset_hold(index);
        }
    }
}

/// FL's meter-wave view: the recent level as bars across a sunken well, newest at the
/// bottom, as wide as it was loud.
pub(super) fn wave(
    b: &mut Brush,
    s: &Style,
    rect: Rect,
    w: Option<&super::meters::Wave>,
    _color: Option<Color32>,
) {
    let t = &s.t;
    b.register(rect, "meter_wave", meter::TOKENS, &[]);
    light::sunken(b, rect, 0.0, t.meter_well, 1.0, t.depth);
    let Some(w) = w else {
        return;
    };
    if w.is_empty() {
        return;
    }
    let n = w.len();
    let inner = rect.shrink(b.px() * 2.0);
    let dy = inner.height() / super::meters::WAVE_LEN as f32;
    let cx = inner.center().x;
    for i in 0..n {
        let db = w.get(i);
        if db <= meter::MIN_DB {
            continue;
        }
        let half = meter::pos_of_db(db) * inner.width() * 0.5;
        let y1 = inner.bottom() - (n - 1 - i) as f32 * dy;
        let c = if db >= 0.0 {
            t.meter_clip
        } else if db >= -6.0 {
            t.meter_high
        } else {
            t.meter_low
        };
        b.rect(
            Rect::from_min_max(pos2(cx - half, y1 - dy), pos2(cx + half, y1)),
            Fill::Solid(c),
        );
    }
}

/// A compressor's gain reduction, hanging from the top in the record colour over 0–24 dB
/// (ticks every 6 dB). Returns whether it is showing reduction.
pub(super) fn gr_lane(b: &mut Brush, s: &Style, rect: Rect, gr: Option<f32>) -> bool {
    let t = &s.t;
    b.register(rect, "gr_lane", GR_TOKENS, &[]);
    light::sunken(b, rect, 0.0, t.meter_well, 1.0, t.depth);
    let mut filling = false;
    if let Some(g) = gr {
        let h = (g / 24.0).clamp(0.0, 1.0) * rect.height();
        if h > 0.5 {
            b.rect(
                b.snap_rect(Rect::from_min_size(rect.min, vec2(rect.width(), h)).shrink(b.px())),
                Fill::Solid(t.danger),
            );
            filling = true;
        }
    }
    for db in [6.0f32, 12.0, 18.0] {
        let y = b.snap_mid(rect.top() + db / 24.0 * rect.height());
        b.hline(
            rect.left(),
            rect.right(),
            y,
            b.px(),
            with_alpha(t.text_dim, 0.4),
        );
    }
    filling
}

const GR_TOKENS: &[Tok] = &[Tok::MeterWell, Tok::Danger, Tok::TextDim];
const EQ_TOKENS: &[Tok] = &[Tok::Well, Tok::TextDim];

/// The style a strip's knobs are drawn at: a touch smaller on a 24-pt strip, so the value
/// arc stays inside it.
pub(super) fn knob_style(pass: &Pass) -> Style {
    if pass.mode() == WidthMode::Compact {
        pass.s.scaled(0.9)
    } else {
        *pass.s
    }
}

/// Mute and solo, pan, the fader and its value, phase / swap / mono, width, the EQ bars,
/// the FX bulb and record.
#[allow(clippy::too_many_arguments)]
fn controls(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    sn: &Snap,
    x: f32,
    bw: f32,
    chosen: bool,
) {
    let s = pass.s;
    let t = &s.t;
    let id = sn.id;
    let cx = x + bw * 0.5;
    let compact = pass.mode() == WidthMode::Compact;
    let ctx = ui.ctx().clone();

    // Mute and solo: FL's lamps — green while the strip plays, orange while soloed.
    // Right-click or Ctrl+click on the mute lamp solos, as in FL.
    if let Some(my) = pass.mid(Row::MuteSolo) {
        let dx = if compact { 5.5 } else { 8.0 };
        let (mc, sc) = (pos2(cx - dx, my), pos2(cx + dx, my));
        let hit = |c: Pos2| Rect::from_center_size(c, vec2(11.0, 13.0));
        let m = sense(ui, pass, hit(mc), pid(Some(id), Part::Mute), Sense::click());
        let so = sense(ui, pass, hit(sc), pid(Some(id), Part::Solo), Sense::click());
        let mh = m.as_ref().is_some_and(|r| r.hovered());
        let sh = so.as_ref().is_some_and(|r| r.hovered());
        lamp(b, s, mc, !sn.muted, Tok::Led, mh);
        lamp(b, s, sc, sn.soloed, Tok::Highlight, sh);
        if let Some(m) = m {
            if mh {
                hint::set_text(&ctx, &v.words.mute, "", Caps::NONE);
            }
            let m = m.on_hover_text(&v.words.mute);
            if m.secondary_clicked() || (m.clicked() && pass.mods.command) {
                app.exec(Command::SetStripSolo {
                    id,
                    soloed: !sn.soloed,
                });
            } else if m.clicked() {
                app.exec(Command::SetStripMute {
                    id,
                    muted: !sn.muted,
                });
            }
        }
        if let Some(so) = so {
            if sh {
                hint::set_text(&ctx, &v.words.solo, "", Caps::NONE);
            }
            if so.on_hover_text(&v.words.solo).clicked() {
                app.exec(Command::SetStripSolo {
                    id,
                    soloed: !sn.soloed,
                });
            }
        }
    }

    // Pan.
    if let Some(py) = pass.mid(Row::Pan) {
        let ks = knob_style(pass);
        let c = pos2(cx, py);
        let mut pos = (sn.pan.clamp(-1.0, 1.0) + 1.0) * 0.5;
        let resp = sense(
            ui,
            pass,
            Rect::from_center_size(c, vec2(22.0, 22.0)),
            pid(Some(id), Part::Pan),
            Sense::click_and_drag(),
        );
        let st = resp.as_ref().map_or(State::REST, |r| State::of(r, false));
        let dm = DeskMod::new(app, cantus_core::AutoTarget::StripPan(id), pos, &ks);
        if let Some(resp) = &resp {
            touch(app, id, resp);
            let depth = dm.input(app, ui, resp);
            if let Some(p) = knob_input(v, ui, pass, resp, pos, 0.5, !depth) {
                pos = p;
                app.exec_merging(
                    Command::SetStripPan {
                        id,
                        pan: p * 2.0 - 1.0,
                    },
                    MergeKey::of(&("spans", id.0)),
                );
            }
            if resp.drag_stopped() && !depth {
                app.end_gesture();
            }
            if st.hovered {
                hint::set(&ctx, &v.words.pan, Caps::PARAM, resp.dragged(), |b| {
                    write_pan(b, pos * 2.0 - 1.0);
                    Ok(())
                });
            }
        }
        // Where a modulator has it now (ADR 0057).
        knob::paint(
            b,
            c,
            &ks,
            st,
            &KnobLook {
                live: dm.live,
                rings: dm.rings(),
                ..KnobLook::new(KnobSize::Mini, pos, 0.5)
            },
        );
        dm.paint_drop(
            b,
            ui,
            resp.as_ref(),
            Rect::from_center_size(c, vec2(22.0, 22.0)),
            s,
        );
    }

    // The fader: lime while its strip is in the selection.
    if let Some((a, z)) = pass.row(Row::Fader) {
        let half = if compact { 11.0 } else { 14.0 };
        let fr = Rect::from_min_max(pos2(cx - half, a), pos2(cx + half, z));
        let mut gain = sn.gain;
        let resp = sense(
            ui,
            pass,
            fr,
            pid(Some(id), Part::Fader),
            Sense::click_and_drag(),
        );
        let st = resp
            .as_ref()
            .map_or(State::REST, |r| State::of(r, chosen))
            .lit(chosen);
        let dm = DeskMod::new(app, cantus_core::AutoTarget::StripGain(id), pos_of(gain), s);
        if let Some(resp) = &resp {
            touch(app, id, resp);
            let depth = dm.input(app, ui, resp);
            if let Some(g) = fader_input(v, ui, pass, resp, fr, gain, !depth) {
                gain = g;
                app.set_strip_gain(id, g);
            }
            if resp.drag_stopped() && !depth {
                app.end_gesture();
            }
            if st.hovered {
                hint::set(&ctx, &v.words.volume, Caps::PARAM, resp.dragged(), |b| {
                    write_db(b, gain, true);
                    Ok(())
                });
            }
        }
        kfader::paint(
            b,
            fr,
            s,
            st,
            &FaderLook {
                pos: pos_of(gain),
                ticks: super::paint::ticks(),
                unity: Some(UNITY_POS),
            },
        );
        // Where a modulator has the fader now (ADR 0057): a mark beside the track.
        dm.fader_mark(b, fr, s);
        dm.paint_drop(b, ui, resp.as_ref(), fr, s);
        // The value, from Wide up.
        if let Some((ra, rz)) = pass.row(Row::Readout) {
            let rr = Rect::from_min_max(pos2(x + 3.0, ra), pos2(x + bw - 3.0, rz));
            v.buf.clear();
            write_db(&mut v.buf, gain, false);
            crate::kit::well::readout(b, rr, s, State::REST, &v.buf, true);
        }
    }

    // Phase, swap and (from Wide up) mono: the input stage, in the order the signal meets
    // it. Mono is the width's end stop, so its switch is a shortcut to it.
    if let Some(py) = pass.mid(Row::PhaseSwap) {
        let mono = sn.width <= 0.005;
        let all = [
            (Part::Phase, Glyph::Phase, sn.phase),
            (Part::Swap, Glyph::Swap, sn.swap),
            (Part::Mono, Glyph::Record, mono),
        ];
        // Mono needs a third place: from Wide up.
        let parts = &all[..if compact { 2 } else { 3 }];
        let step = if compact { 11.0 } else { 13.0 };
        let x0 = cx - step * (parts.len() - 1) as f32 * 0.5;
        for (i, (part, g, on)) in parts.iter().enumerate() {
            let c = pos2(x0 + step * i as f32, py);
            let r = Rect::from_center_size(c, vec2(11.0, 11.0));
            let resp = sense(
                ui,
                pass,
                r.expand(1.0),
                pid(Some(id), *part),
                Sense::click(),
            );
            let hovered = resp.as_ref().is_some_and(|r| r.hovered());
            if matches!(part, Part::Mono) {
                mono_glyph(b, s, r, *on, hovered);
            } else {
                glyph_toggle(b, s, r, *g, *on, hovered, t.highlight);
            }
            let Some(resp) = resp else {
                continue;
            };
            let tip = match part {
                Part::Phase => &v.words.phase,
                Part::Swap => &v.words.swap,
                _ => &v.words.mono,
            };
            if hovered {
                hint::set_text(&ctx, tip, "", Caps::NONE);
            }
            if resp.on_hover_text(tip).clicked() {
                app.exec(match part {
                    Part::Phase => Command::SetStripPhaseInvert {
                        id,
                        invert: !sn.phase,
                    },
                    Part::Swap => Command::SetStripSwapChannels { id, swap: !sn.swap },
                    _ => Command::SetStripWidth {
                        id,
                        width: if mono { 1.0 } else { 0.0 },
                    },
                });
            }
        }
    }

    // Width: 0 mono … 1 as recorded … 2 twice as wide.
    if let Some(wy) = pass.mid(Row::Width) {
        let ks = knob_style(pass);
        let c = pos2(cx, wy);
        let mut pos = (sn.width / 2.0).clamp(0.0, 1.0);
        let resp = sense(
            ui,
            pass,
            Rect::from_center_size(c, vec2(22.0, 22.0)),
            pid(Some(id), Part::Width),
            Sense::click_and_drag(),
        );
        let st = resp.as_ref().map_or(State::REST, |r| State::of(r, false));
        let dm = DeskMod::new(app, cantus_core::AutoTarget::StripWidth(id), pos, &ks);
        if let Some(resp) = &resp {
            touch(app, id, resp);
            let depth = dm.input(app, ui, resp);
            if let Some(p) = knob_input(v, ui, pass, resp, pos, 0.5, !depth) {
                pos = p;
                app.exec_merging(
                    Command::SetStripWidth { id, width: p * 2.0 },
                    MergeKey::of(&("swidth", id.0)),
                );
            }
            if resp.drag_stopped() && !depth {
                app.end_gesture();
            }
            if st.hovered {
                hint::set(&ctx, &v.words.width, Caps::PARAM, resp.dragged(), |b| {
                    write!(b, "{:.0} %", pos * 200.0)
                });
            }
        }
        knob::paint(
            b,
            c,
            &ks.function(crate::kit::Function::Space),
            st,
            &KnobLook {
                live: dm.live,
                rings: dm.rings(),
                ..KnobLook::new(KnobSize::Mini, pos, 0.5)
            },
        );
        dm.paint_drop(
            b,
            ui,
            resp.as_ref(),
            Rect::from_center_size(c, vec2(22.0, 22.0)),
            s,
        );
    }

    eq_bars(v, app, ui, b, pass, sn, x, bw);
    fx_rec(v, app, ui, b, pass, sn, cx, CaptureTarget::Strip(id));
}

/// Mono: two rings folded into one.
fn mono_glyph(b: &mut Brush, s: &Style, r: Rect, on: bool, hovered: bool) {
    let c = if on {
        s.t.highlight
    } else if hovered {
        s.t.text
    } else {
        with_alpha(s.t.text_dim, 0.62)
    };
    let u = r.width() / 12.0;
    b.circle_stroke(r.center() - vec2(1.6 * u, 0.0), 3.2 * u, 1.2 * u, c);
    b.circle_stroke(r.center() + vec2(1.6 * u, 0.0), 3.2 * u, 1.2 * u, c);
}

/// The strip's own EQ as three bipolar bars — low, mid, high — each filling out from 0 dB
/// in the tone hue. The frequencies and Qs are in the inspector; here the three moves a
/// mix engineer makes most are one gesture each.
#[allow(clippy::too_many_arguments)]
fn eq_bars(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    sn: &Snap,
    x: f32,
    bw: f32,
) {
    let Some((a, z)) = pass.row(Row::Eq) else {
        return;
    };
    let s = pass.s;
    let t = &s.t;
    let hue = t.hue_tone;
    let ctx = ui.ctx().clone();
    let (bh, gap) = (5.0, 3.0);
    let top = (a + z) * 0.5 - (3.0 * bh + 2.0 * gap) * 0.5;
    for i in 0..3 {
        let y = top + i as f32 * (bh + gap);
        let r = Rect::from_min_max(pos2(x + 4.0, y), pos2(x + bw - 4.0, y + bh));
        let mut pos = (sn.eq[i] / EQ_MAX_GAIN_DB * 0.5 + 0.5).clamp(0.0, 1.0);
        let resp = sense(
            ui,
            pass,
            r.expand2(vec2(0.0, 1.0)),
            pid(Some(sn.id), Part::Eq(i as u8)),
            Sense::click_and_drag(),
        );
        let st = resp.as_ref().map_or(State::REST, |r| State::of(r, false));
        if let Some(resp) = &resp {
            if let Some(p) = knob_input(v, ui, pass, resp, pos, 0.5, true) {
                pos = p;
                app.exec_merging(
                    Command::SetStripEq {
                        id: sn.id,
                        band: i as u8,
                        field: EqField::Gain,
                        value: (p - 0.5) * 2.0 * EQ_MAX_GAIN_DB,
                    },
                    MergeKey::of(&("seq", sn.id.0, i)),
                );
            }
            if resp.drag_stopped() {
                app.end_gesture();
            }
            if st.hovered {
                let (gain, hz) = ((pos - 0.5) * 2.0 * EQ_MAX_GAIN_DB, sn.eq_hz[i]);
                hint::set(&ctx, &v.words.eq[i], Caps::PARAM, resp.dragged(), |b| {
                    write!(b, "{gain:+.1} dB · ")?;
                    if hz >= 1_000.0 {
                        write!(b, "{:.1} kHz", hz / 1_000.0)
                    } else {
                        write!(b, "{hz:.0} Hz")
                    }
                });
            }
        }
        let r = b.snap_rect(r);
        b.register(r, "eq_bar", EQ_TOKENS, &[Some(Tok::HueTone)]);
        light::sunken(b, r, 1.0, t.well, 1.0, t.depth);
        let cxm = b.snap(r.center().x);
        let vx = r.left() + r.width() * pos;
        if (pos - 0.5).abs() > 0.004 {
            let fill = Rect::from_min_max(
                pos2(cxm.min(vx), r.top() + b.px()),
                pos2(cxm.max(vx), r.bottom() - b.px()),
            );
            let c = if st.hovered { shade(hue, 0.06) } else { hue };
            b.rect(b.snap_rect(fill), Fill::Solid(c));
        }
        // 0 dB: a notch in the well's lip, so a flat band is quiet.
        b.vline(
            cxm,
            r.top(),
            r.top() + b.px() * 2.0,
            b.px(),
            with_alpha(t.text_dim, if st.hovered { 0.9 } else { 0.5 }),
        );
    }
}

/// The FX bulb (lit while inserts are in; hollow while the chain is switched out; dark
/// with no inserts) and the record dot (red and pulsing while it records).
#[allow(clippy::too_many_arguments)]
pub(super) fn fx_rec(
    v: &mut MixerView,
    app: &mut CantusApp,
    ui: &mut Ui,
    b: &mut Brush,
    pass: &mut Pass,
    sn: &Snap,
    cx: f32,
    target: CaptureTarget,
) {
    let Some(fy) = pass.mid(Row::FxRec) else {
        return;
    };
    let s = pass.s;
    let t = &s.t;
    let ctx = ui.ctx().clone();
    let compact = pass.mode() == WidthMode::Compact;
    let dx = if compact { 5.5 } else { 8.0 };
    let master = matches!(target, CaptureTarget::Master);
    {
        let br = Rect::from_center_size(pos2(cx - dx, fy), vec2(12.0, 12.0));
        let fx_id = if master { None } else { Some(sn.id) };
        let resp = sense(ui, pass, br, pid(fx_id, Part::Fx), Sense::click());
        let hovered = resp.as_ref().is_some_and(|r| r.hovered());
        let c = if sn.effects == 0 {
            with_alpha(t.text_dim, if hovered { 0.6 } else { 0.35 })
        } else if sn.fx_on {
            t.fx_on
        } else {
            mix(t.fx_on, t.strip, 0.65)
        };
        glyph::paint(b, br, Glyph::Bulb, c);
        if let Some(resp) = resp {
            let tip = if sn.fx_on {
                &v.words.fx
            } else {
                &v.words.fx_off
            };
            if hovered {
                hint::set_text(&ctx, tip, "", Caps::NONE);
            }
            if resp.on_hover_text(tip).clicked() {
                if master {
                    app.exec(Command::SetMasterFxEnabled { enabled: !sn.fx_on });
                } else {
                    app.exec(Command::SetStripFxEnabled {
                        id: sn.id,
                        enabled: !sn.fx_on,
                    });
                }
            }
        }
    }
    let rc = cx + dx;
    let rr = Rect::from_center_size(pos2(rc, fy), vec2(11.0, 11.0));
    let id = if master { None } else { Some(sn.id) };
    let resp = sense(ui, pass, rr, pid(id, Part::Rec), Sense::click());
    let hovered = resp.as_ref().is_some_and(|r| r.hovered());
    // A strip whose tracks have an input: the dot arms them (FL's mixer record arm, ADR
    // 0025); the take of what leaves the fader moves to the strip's menu.
    let (inputs, armed) = if master {
        (false, false)
    } else {
        super::menu::rec_state(app, sn.id)
    };
    if inputs {
        let live = app.rec.pass.as_ref().is_some_and(|l| {
            app.project()
                .tracks_on_strip(sn.id)
                .any(|t| l.records(t.id))
        });
        if live {
            let pulse = 0.65 + 0.35 * (pass.now as f32 * 5.0).sin().abs();
            glyph::paint(b, rr, Glyph::Record, with_alpha(t.danger, pulse));
        } else if armed {
            glyph::paint(b, rr, Glyph::Record, t.danger);
        } else {
            let c = if hovered {
                with_alpha(t.danger, 0.8)
            } else {
                with_alpha(t.danger, 0.4)
            };
            glyph::paint(b, rr.shrink(1.0), Glyph::Record, c);
        }
        if let Some(resp) = resp {
            let tip = crate::views::clips_chrome::word("rec.arm_strip_tip");
            if hovered {
                hint::set_text(&ctx, tip, "", Caps::NONE);
            }
            if resp.on_hover_text(tip).clicked() {
                super::menu::arm_strip(app, sn.id);
            }
        }
        return;
    }
    let on = app.capturing(target);
    if on {
        let pulse = 0.75 + 0.25 * (pass.now as f32 * 3.0).sin();
        glyph::paint(b, rr, Glyph::Record, with_alpha(t.danger, pulse));
    } else {
        let c = if hovered {
            t.danger
        } else {
            with_alpha(t.text_dim, 0.6)
        };
        glyph::paint(b, rr.shrink(1.0), Glyph::Record, c);
    }
    if let Some(resp) = resp {
        let tip = if on {
            &v.words.stop
        } else if master {
            &v.words.record_mix
        } else {
            &v.words.record
        };
        if hovered {
            hint::set_text(&ctx, tip, "", Caps::NONE);
        }
        if resp.on_hover_text(tip).clicked() {
            app.toggle_capture(target);
        }
    }
}

/// The rings a desk knob wears at most: a mini knob grows 3 pt per ring.
const DESK_RINGS: usize = 2;

/// A desk control's modulation (ADR 0057, review 1): its rings and where modulation has it
/// now, and what it does with a modulator's handle dropped on it or an Alt-drag (or any
/// drag while a modulator maps) — the depth of its route, not its value. The strip's and
/// the master's pan, fader and width all go through here.
pub(super) struct DeskMod {
    target: cantus_core::AutoTarget,
    offer: Option<crate::kit::automate::Offer>,
    rings: [(f32, egui::Color32); crate::views::modulator::MAX_RINGS],
    n: usize,
    pub live: Option<f32>,
}

impl DeskMod {
    pub(super) fn new(
        app: &CantusApp,
        target: cantus_core::AutoTarget,
        pos: f32,
        s: &crate::kit::Style,
    ) -> Self {
        let mut rings = [(0.0, egui::Color32::TRANSPARENT); crate::views::modulator::MAX_RINGS];
        let (m, n) = app.mod_offer(&target, pos, s, &mut rings);
        DeskMod {
            target,
            offer: Some(crate::kit::automate::Offer {
                automated: false,
                typing: false,
                modulation: Some(m),
            }),
            rings,
            n,
            live: m.live,
        }
    }

    /// The modulators' rings, as many as a desk knob wears.
    pub(super) fn rings(&self) -> &[(f32, egui::Color32)] {
        &self.rings[..self.n.min(DESK_RINGS)]
    }

    /// What the control's response did of its modulation, answered; `true` while its
    /// drags are the depth's (the value must not move).
    pub(super) fn input(&self, app: &mut CantusApp, ui: &Ui, resp: &Response) -> bool {
        if let Some(c) = crate::kit::automate::modulation_choice(ui, resp, self.offer) {
            app.modulation_choice(&self.target, c, resp.rect);
        }
        crate::kit::automate::depth_drag(ui, self.offer)
    }

    /// While a modulator's handle is dragged over it, the outline that says it takes it.
    pub(super) fn paint_drop(
        &self,
        b: &mut Brush,
        ui: &Ui,
        resp: Option<&Response>,
        rect: Rect,
        s: &crate::kit::Style,
    ) {
        if crate::kit::automate::handle_in_flight(ui.ctx())
            && resp.is_some_and(|r| r.contains_pointer())
        {
            crate::kit::automate::drop_target(b, rect, s);
        }
    }

    /// The modulated mark beside the fader where modulation has it now.
    pub(super) fn fader_mark(&self, b: &mut Brush, fr: Rect, s: &crate::kit::Style) {
        if let Some(live) = self.live {
            let c = crate::kit::fader::cap_rect(fr, s, live).center();
            let x = fr.right() - 1.0 * s.k;
            let d = 4.0 * s.k;
            b.polygon(
                &[pos2(x, c.y), pos2(x + d, c.y - d), pos2(x + d, c.y + d)],
                s.t.hue_mod,
            );
        }
    }
}
