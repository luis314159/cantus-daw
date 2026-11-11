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
use super::look::{self, W};
use super::menu::Menu;
use super::rows::{self, Col};
use super::{in_selection, DeskPrefs, Geo, MixerView, Renaming};
use crate::app::CantusApp;
use crate::kit::color::{mix, with_alpha};
use crate::kit::glyph_mixer::{Ink, MGlyph};
use crate::kit::hint::{self, Caps};
use crate::kit::meter;
use crate::kit::text::{self, Elide, Policy};
use crate::kit::{light, Brush, Fill, Style, Tok};
use cantus_core::{StripId, MIN_GAIN_DB};
use egui::{
    pos2, vec2, Align2, Color32, EventFilter, Id, Key, Modifiers, Rect, Response, Sense, Ui,
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
    /// The mixer's colours (round 39, des-mixer).
    pub pal: super::look::Pal,
    /// The strip whose destinations light (option C): under the pointer, else selected.
    pub lit: Option<StripId>,
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
            pal: super::look::Pal::of(s),
            lit: None,
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
    pub(super) effects: usize,
    pub(super) eq: [f32; 3],
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
            effects: m.effects.len(),
            eq: [0.0; 3],
            output: None,
        }
    }
}

pub(super) fn snap(app: &CantusApp, index: usize) -> Option<Snap> {
    let s = app.project().strips.get(index)?;
    let mut eq = [0.0; 3];
    for (i, band) in s.eq.bands.iter().take(3).enumerate() {
        eq[i] = band.gain_db;
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
        effects: s.effects.len(),
        eq,
        output: s.output,
    })
}

/// The colour a strip wears, clamped to the safe band.
pub(super) fn strip_color(v: &MixerView, s: &Style, index: usize) -> Option<Color32> {
    v.desk
        .info(index)
        .and_then(|i| i.color)
        .map(|c| crate::kit::color::track_mark(crate::theme::track_color(c), &s.t))
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
    let p = pass.pal;
    // The pitch's 2 pt after the body are the desk's: value between strips, not borders.
    let bw = w - 2.0;
    let selected = app.selected_strip == Some(id);
    let chosen = in_selection(app, id);
    let hue = v
        .desk
        .info(index)
        .and_then(|i| i.color)
        .map(crate::theme::track_color);
    let vca = v.desk.vca_marks.get(index).is_some_and(|m| m.is_vca);
    let silenced = v.desk.silenced.get(index).copied().unwrap_or(false);
    let frozen = crate::views::freeze_view::held(app).strips.contains(&id);
    let empty = !vca
        && !v.desk.fed(index)
        && sn.effects == 0
        && v.desk
            .info(index)
            .is_some_and(|i| !i.goes && i.label == super::desk::Label::None);
    let body = pass.body(x, bw);
    b.register(body, "mixer_strip", STRIP_TOKENS, &[]);
    let flat = if empty {
        Some(p.empty)
    } else if chosen {
        Some(hue.map_or(p.selected_plain(), |h| p.selected(h)))
    } else {
        None
    };
    look::body(b, body, &p, flat);
    if !empty {
        let stripe = if vca {
            None
        } else {
            let k = if selected {
                1.0
            } else if sn.muted || silenced {
                0.32
            } else if p.dark {
                0.72
            } else {
                0.70
            };
            Some(hue.map_or(p.ink4, |h| p.stripe(h, k))) // r39:fix2 — 3:1 on paper
        };
        match stripe {
            Some(c) => look::stripe(b, body, c),
            None => {
                // A VCA has no colour of its own: two hairlines.
                for dy in [0.0, 2.0] {
                    b.rect(
                        b.snap_rect(Rect::from_min_size(body.min + vec2(0.0, dy), vec2(bw, 1.0))),
                        Fill::Solid(p.ink3),
                    );
                }
            }
        }
    }
    // The column as a drop target while a drag is on (round 25).
    let target = super::drag::target(v, app, ui, Some(id), body);
    super::cables::column_hatch(b, pass.s, body, target);
    let c = Col {
        x,
        w: bw,
        sn,
        master: false,
        selected,
        chosen,
        hue,
        dim: sn.muted || silenced,
        frozen,
        vca,
        empty,
    };
    head(v, app, ui, b, pass, &c);
    rows::arm(v, app, ui, b, pass, &c);
    // A keyed insert's key under the pointer lights the strip it listens to.
    if v.key_shown == Some(id) {
        b.rrect_stroke(b.snap_rect(body).shrink(b.px() * 0.5), 3.0, b.px(), p.ink2);
    }
    if empty {
        // Its number, a groove, its socket (des-mixer) — and the places that act on it
        // quietly there: the inserts' `+`, the sends' `+`, the Route-here plate.
        if let (Some((a, _)), Some((_, z))) = (pass.row(Row::Pan), pass.row(Row::MuteSolo)) {
            let plate = Rect::from_min_max(pos2(x + 3.0, a), pos2(x + bw - 3.0, z));
            super::routehere::plate(v, app, ui, b, pass, id, plate);
        }
        rows::inserts(v, app, ui, b, pass, &c);
        rows::fader(v, app, ui, b, pass, &c);
        rows::sends(v, app, ui, b, pass, &c);
        super::parts::sends_row(v, app, ui, b, pass, &sn, x, bw);
        return;
    }
    if !vca {
        rows::inserts(v, app, ui, b, pass, &c);
        rows::eq(v, app, ui, b, pass, &c);
        rows::polarity(v, app, ui, b, pass, &c);
    }
    rows::pan(v, app, ui, b, pass, &c);
    rows::msr(v, app, ui, b, pass, &c);
    rows::fader(v, app, ui, b, pass, &c);
    rows::level(v, ui, b, pass, &c);
    rows::sends(v, app, ui, b, pass, &c);
    rows::output(v, app, ui, b, pass, &c);
    super::parts::sends_row(v, app, ui, b, pass, &sn, x, bw);
    if frozen {
        // The sound block is a render: hatched, cold, still readable.
        let top = pass.row(Row::Chain).map(|r| r.0);
        let end = pass
            .row(Row::PhaseSwap)
            .or(pass.row(Row::Eq))
            .or(pass.row(Row::Chain))
            .map(|r| r.1);
        if let (Some(a), Some(z)) = (top, end) {
            let r = Rect::from_min_max(pos2(x + 1.0, a - 2.0), pos2(x + bw - 1.0, z + 2.0));
            look::hatch(b, r.intersect(pass.clip), &p);
        }
    }
}

/// The tokens a strip's body paints with (its parts register their own).
const STRIP_TOKENS: &[Tok] = &[Tok::Strip, Tok::StripSelected, Tok::Text, Tok::Label];

/// The head (number and name; Large its kind): one zone that selects, renames
/// (double-click), drags the strip along the desk and opens its menu (right-click).
/// Wide and Large: the number Regular 10 ink 3 over the name Regular 12 (13), Medium when
/// selected; a name too wide at 11 takes both lines after the number; then the ellipsis.
/// Compact: the number, and the name up the strip. A return wears its hook, a frozen strip
/// its snowflake, a VCA its tag; a strip a VCA holds, "VCA 1" after its number.
fn head(v: &mut MixerView, app: &mut CantusApp, ui: &mut Ui, b: &mut Brush, pass: &Pass, c: &Col) {
    let s = pass.s;
    let p = pass.pal;
    let Some((na, nz)) = pass.row(Row::Number) else {
        return;
    };
    let sn = c.sn;
    let (x, bw) = (c.x, c.w);
    let cx = c.cx();
    let zone = Rect::from_min_max(pos2(x, na), pos2(x + bw, nz));
    let resp = sense(ui, pass, zone, pid(Some(sn.id), Part::Head), Sense::click());
    if let Some(r) = &resp {
        super::drag::source(v, app, ui, r, super::drag::Source::Strip(sn.id));
    }
    let hot = resp.as_ref().is_some_and(|r| r.hovered());
    if hot && !c.selected {
        b.rrect(
            b.snap_rect(zone),
            3.0,
            Fill::Solid(with_alpha(p.ink1, 0.04)),
        );
    }
    let renaming = matches!(v.rename, Some((Renaming::Strip(r), _)) if r == sn.id);
    if renaming {
        v.rename_at = Some(zone);
    }
    let mode = pass.mode();
    let name = v.desk.name(app, sn.index);
    let glyph = v.desk.info(sn.index).and_then(|i| i.glyph);
    let name_ink = if c.empty {
        p.ink4
    } else if c.dim {
        p.ink3
    } else {
        p.ink1
    };
    let weight = if c.selected { W::Medium } else { W::Regular };
    let mark = v.desk.vca_marks.get(sn.index).copied().unwrap_or_default();
    let returns = v.desk.sends_in.get(sn.index).copied().unwrap_or(0) > 0
        && app
            .project()
            .strip(sn.id)
            .is_some_and(|st| st.sends.is_empty());
    v.buf.clear();
    let _ = write!(v.buf, "{}", sn.index + 1);
    let num = std::mem::take(&mut v.buf);
    let num_ink = if c.empty { p.ink4 } else { p.ink3 };
    if mode == WidthMode::Compact {
        look::say(
            b,
            s,
            &num,
            cx,
            na + 14.0,
            Align2::CENTER_BOTTOM,
            bw,
            10.0,
            W::Regular,
            num_ink,
        );
        if c.vca {
            look::say(
                b,
                s,
                "VCA",
                cx,
                na + 26.0,
                Align2::CENTER_BOTTOM,
                bw,
                8.0,
                W::SemiBold,
                p.ink2,
            );
        }
        if c.frozen {
            look::glyph(
                b,
                s,
                pos2(cx, na + 28.0),
                12.0,
                MGlyph::Freeze,
                Ink::Solid(p.accent),
                p.surface,
            );
        }
        // The track's own icon (12 under the number and the arm dot, above the name).
        let iconed = !c.empty && !c.vca && glyph.is_some();
        if let Some(g) = glyph.filter(|_| iconed) {
            let at = Rect::from_center_size(pos2(cx, na + 41.0), vec2(12.0, 12.0));
            head_icon(v, b, pass, c, g, at);
        }
        if !renaming && !name.is_empty() && !c.empty {
            let room = (nz - na) - if iconed { 53.0 } else { 26.0 };
            let (t, k) = look::words(name, 12.0, weight, name_ink, s.tk());
            let t = t
                .align(Align2::CENTER_BOTTOM)
                .policy(Policy::Rotate90(Elide::End));
            let nr = Rect::from_min_max(pos2(cx - 8.0, nz - 4.0 - room), pos2(cx + 8.0, nz - 4.0));
            let _ = text::paint(b, nr, &t, k);
        }
    } else {
        let ny = na + 15.0;
        let room = bw - 6.0;
        let size = if mode == WidthMode::Xl { 13.0 } else { 12.0 };
        let ctx = ui.ctx().clone();
        let fits = |sz: f32| look::width_of(&ctx, s, name, sz, weight) <= room;
        let wraps = !c.vca && !name.is_empty() && !fits(11.0) && name.contains(' ');
        // The track's own icon (ADR 0113 §4; the icons sheet: 14 before the name, the two
        // centred as one group). A name that would not fit beside it keeps its room and
        // the icon goes beside the number; a name on two lines takes it before its second.
        let glyph = glyph.filter(|_| !c.vca && !c.empty && !renaming && !name.is_empty());
        let beside = |sz: f32| look::width_of(&ctx, s, name, sz, weight) <= room - ICON - 3.0;
        let icon_at = match glyph {
            None => IconAt::None,
            Some(_) if wraps => IconAt::LineTwo,
            Some(_) if beside(size) => IconAt::Name(size),
            Some(_) if beside(size - 1.0) => IconAt::Name(size - 1.0),
            Some(_) if mark.governed == 0 => IconAt::Number,
            Some(_) => IconAt::None,
        };
        if mark.governed > 0 && !c.vca {
            // "2 VCA 1": the number and the VCA's tag, one centred group.
            let tag = &v.words.vca;
            let nw = look::width_of(&ctx, s, &num, 10.0, W::Regular);
            let tw = look::width_of(&ctx, s, tag, 9.0, W::SemiBold);
            let gx = cx - (nw + 4.0 + tw) * 0.5;
            look::say(
                b,
                s,
                &num,
                gx,
                ny,
                Align2::LEFT_BOTTOM,
                nw + 2.0,
                10.0,
                W::Regular,
                num_ink,
            );
            look::say(
                b,
                s,
                tag,
                gx + nw + 4.0,
                ny,
                Align2::LEFT_BOTTOM,
                tw + 2.0,
                9.0,
                W::SemiBold,
                p.ink2,
            );
        } else if let (IconAt::Number, Some(g)) = (icon_at, glyph) {
            // The number and the icon, one centred group.
            let nw = look::width_of(&ctx, s, &num, 10.0, W::Regular);
            let gx = cx - (nw + 3.0 + ICON) * 0.5;
            look::say(
                b,
                s,
                &num,
                gx,
                ny,
                Align2::LEFT_BOTTOM,
                nw + 2.0,
                10.0,
                W::Regular,
                num_ink,
            );
            let at = Rect::from_center_size(
                pos2(gx + nw + 3.0 + ICON * 0.5, ny - 3.6),
                vec2(ICON, ICON),
            );
            head_icon(v, b, pass, c, g, at);
        } else if !wraps {
            look::say(
                b,
                s,
                &num,
                cx,
                ny,
                Align2::CENTER_BOTTOM,
                room,
                10.0,
                W::Regular,
                num_ink,
            );
        }
        if returns {
            look::glyph(
                b,
                s,
                pos2(x + 9.0, ny - 3.5),
                12.0,
                MGlyph::Ret,
                Ink::Solid(p.ink3),
                p.surface,
            );
        }
        if c.frozen {
            look::glyph(
                b,
                s,
                pos2(x + bw - 9.0, ny - 3.5),
                12.0,
                MGlyph::Freeze,
                Ink::Solid(p.accent),
                p.surface,
            );
        }
        if c.vca {
            look::say(
                b,
                s,
                &v.words.vca,
                x + 5.0,
                ny,
                Align2::LEFT_BOTTOM,
                24.0,
                9.0,
                W::SemiBold,
                p.ink2,
            );
        }
        if !renaming && !c.empty {
            if wraps {
                // Two lines: the number leads the first.
                let nw = look::width_of(&ctx, s, &num, 10.0, W::Regular);
                let mut split = 0;
                for (i, ch) in name.char_indices() {
                    if ch == ' ' {
                        let w = look::width_of(&ctx, s, &name[..i], 11.0, weight);
                        if nw + 3.0 + w <= room || split == 0 {
                            split = i;
                        }
                    }
                }
                let (l1, l2) = (&name[..split], name[split..].trim_start());
                let w1 = look::width_of(&ctx, s, l1, 11.0, weight).min(room - nw - 3.0);
                let gx = cx - (nw + 3.0 + w1) * 0.5;
                look::say(
                    b,
                    s,
                    &num,
                    gx,
                    ny,
                    Align2::LEFT_BOTTOM,
                    nw + 2.0,
                    10.0,
                    W::Regular,
                    num_ink,
                );
                look::say(
                    b,
                    s,
                    l1,
                    gx + nw + 3.0,
                    ny,
                    Align2::LEFT_BOTTOM,
                    room - nw - 3.0,
                    11.0,
                    weight,
                    name_ink,
                );
                let w2 = look::width_of(&ctx, s, l2, 11.0, weight);
                match glyph {
                    Some(g) if w2 <= room - ICON - 3.0 => {
                        let gx = cx - (ICON + 3.0 + w2) * 0.5;
                        let at = Rect::from_center_size(
                            pos2(gx + ICON * 0.5, ny + 14.0 - 4.0),
                            vec2(ICON, ICON),
                        );
                        head_icon(v, b, pass, c, g, at);
                        look::say(
                            b,
                            s,
                            l2,
                            gx + ICON + 3.0,
                            ny + 14.0,
                            Align2::LEFT_BOTTOM,
                            w2 + 2.0,
                            11.0,
                            weight,
                            name_ink,
                        );
                    }
                    _ => {
                        look::say(
                            b,
                            s,
                            l2,
                            cx,
                            ny + 14.0,
                            Align2::CENTER_BOTTOM,
                            room,
                            11.0,
                            weight,
                            name_ink,
                        );
                    }
                }
            } else {
                let base = na + if mode == WidthMode::Xl { 31.0 } else { 30.0 };
                match (icon_at, glyph) {
                    (IconAt::Name(sz), Some(g)) => {
                        let nw = look::width_of(&ctx, s, name, sz, weight);
                        let gx = cx - (ICON + 3.0 + nw) * 0.5;
                        let at = Rect::from_center_size(
                            pos2(gx + ICON * 0.5, base - sz * 0.36),
                            vec2(ICON, ICON),
                        );
                        head_icon(v, b, pass, c, g, at);
                        look::say(
                            b,
                            s,
                            name,
                            gx + ICON + 3.0,
                            base,
                            Align2::LEFT_BOTTOM,
                            nw + 2.0,
                            sz,
                            weight,
                            name_ink,
                        );
                    }
                    _ => {
                        let sz = if fits(size) { size } else { size - 1.0 };
                        look::say(
                            b,
                            s,
                            name,
                            cx,
                            base,
                            Align2::CENTER_BOTTOM,
                            room,
                            sz,
                            weight,
                            name_ink,
                        );
                    }
                }
            }
            if mode == WidthMode::Xl {
                let mut kind = std::mem::take(&mut v.hint_buf);
                kind.clear();
                let ins = v.desk.outputs_in.get(sn.index).copied().unwrap_or(0) as usize;
                let fed = v.desk.sends_in.get(sn.index).copied().unwrap_or(0) as usize;
                if c.vca {
                    rows::fill_n(&mut kind, &v.words.strips_n, mark.members as usize);
                } else if returns {
                    rows::fill_n(&mut kind, &v.words.kind_return, fed);
                } else if ins > 0 {
                    rows::fill_n(&mut kind, &v.words.kind_bus, ins);
                } else {
                    kind.push_str(&v.words.kind_track);
                }
                look::say(
                    b,
                    s,
                    &kind,
                    cx,
                    na + 42.0,
                    Align2::CENTER_BOTTOM,
                    room,
                    10.0,
                    W::Regular,
                    p.ink3,
                );
                v.hint_buf = kind;
            }
        }
    }
    v.buf = num;
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
    let resp = resp.on_hover_ui(|ui| {
        ui.label(crate::routing_actions::feeders_text(app.project(), sn.id));
        let vcas = super::vca_ui::names(app.project(), sn.id); // r36:mix
        if !vcas.is_empty() {
            ui.label(crate::i18n::tf("mix.vca.follows", &[("names", &vcas)])); // r36:mix
        }
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
        crate::quick_devices::warm(app); // r39:ux-b — the recent effects it lists
        v.popups.open(Menu::Strip(sn.id), at);
    }
}

/// A track icon's side on a Wide or Large head (the icons sheet: 14 before the name).
const ICON: f32 = 14.0;

/// Where a Wide or Large head puts its track's icon.
#[derive(Copy, Clone, PartialEq)]
enum IconAt {
    None,
    /// Before the name, the name at this size.
    Name(f32),
    /// Beside the number (the name needs the head's whole width).
    Number,
    /// Before a two-line name's second line.
    LineTwo,
}

/// Paint the track's own icon `g` in `at`, in the strip's colour as the icons sheet inks
/// it (ADR 0113 §4): the colour itself when selected, toward ink 2 at rest, ink 4 when
/// muted; its second ink half-way to the head's ground.
fn head_icon(
    v: &mut MixerView,
    b: &mut Brush,
    pass: &Pass,
    c: &Col,
    g: crate::views::track_glyphs::TrackGlyph,
    at: Rect,
) {
    let p = pass.pal;
    let hue = c.hue.unwrap_or(p.ink2);
    let ground = if c.chosen {
        c.hue.map_or(p.surface, |h| p.selected(h))
    } else {
        p.surface
    };
    let main = if c.dim {
        p.ink4
    } else if c.selected {
        hue
    } else {
        mix(hue, p.ink2, 0.35)
    };
    let mut inks = crate::views::track_glyphs::inks(pass.s, main, ground);
    inks.second = mix(main, ground, 0.5);
    if crate::views::track_glyphs::paint_with(b, at, g, &inks) {
        v.icons_drawn.push((c.sn.id, at, g.id));
    }
}

/// The level under the pointer, in the hint bar.
pub(super) fn meter_hint(v: &mut MixerView, ui: &Ui, resp: &Response, mr: Rect) {
    if let Some(p) = resp.hover_pos() {
        let f = ((p.y - mr.top() - 8.0) / (mr.height() - 11.0).max(1.0)).clamp(0.0, 1.0);
        hint::set(ui.ctx(), &v.words.level, Caps::NONE, false, |b| {
            write!(b, "{:.1} dB", super::look::law::db_at(f).max(-96.0))
        });
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

/// A switch's tip: its name and whether it is on (the hint and the screen reader read it).
/// // r37:finish
pub(super) fn state_tip(name: &str, on: bool) -> String {
    let state = crate::i18n::t(if on { "kit.state.on" } else { "kit.state.off" });
    format!("{name}: {state}")
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
        if crate::kit::automate::probe::noting_learn() {
            crate::kit::automate::probe::note_learn(&format!("desk:{target:?}"), m.learn.is_some());
        }
        let automated = app.is_automated(&target);
        DeskMod {
            target,
            offer: Some(crate::kit::automate::Offer {
                automated,
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
        // The signs of what drives it (round 39, ADR 0109): a clip, a controller (round 29,
        // review 1 of ADR 0062: not in learn mode, whose outline says it), a VCA; its rings
        // and live mark are the modulators'.
        if let Some(o) = self.offer {
            let mut d = o.drivers();
            d.modulated = false;
            d.mapped &= !o.learn().is_some_and(|l| l.mode);
            crate::kit::automate::signs(b, rect, s, d);
        }
        if resp.is_some_and(|r| r.hovered()) {
            crate::kit::automate::hint_drive(ui.ctx(), self.offer);
        }
    }
}
