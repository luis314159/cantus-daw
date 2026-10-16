//! The channel's own strip, pinned at the bottom of the inspector (Bitwig's): its meter,
//! its fader, its pan, and the switches of its input stage — so a channel can be mixed
//! without the desk. With no strip (a track to the master, or nothing selected) it is the
//! master's.

use super::sheet::{self, Range};
use super::{strip_of, Act, Inspector, Subject};
use crate::app::CantusApp;
use crate::kit::color::shade;
use crate::kit::fader::{self, FaderLook};
use crate::kit::glyph::Glyph;
use crate::kit::hint::{self, Caps};
use crate::kit::knob::{self, KnobLook, KnobSize};
use crate::kit::meter::{self, Levels};
use crate::kit::text::{self, Role, Src, Text};
use crate::kit::{cache, led, Fill, Function, State, Style};
use crate::widgets::fader::{db_at, pos_of};
use cantus_core::{AutoTarget, Command, MergeKey};
use egui::{pos2, Align2, Id, Rect, Response, Sense, Ui, Vec2};

/// The meter's ballistics: what it shows, its peak hold, how long the hold has waited.
pub struct Meter {
    level: [f32; 2],
    hold: [f32; 2],
    held: [f64; 2],
}

impl Default for Meter {
    fn default() -> Self {
        Meter {
            level: [meter::MIN_DB; 2],
            hold: [meter::MIN_DB; 2],
            held: [0.0; 2],
        }
    }
}

/// Tick marks every 6 dB from +12 to −48, placed on the fader's own travel.
const TICKS_DB: [f32; 11] = [
    12.0, 6.0, 0.0, -6.0, -12.0, -18.0, -24.0, -30.0, -36.0, -42.0, -48.0,
];

fn to_db(lin: f32) -> f32 {
    if lin <= 1e-6 {
        meter::MIN_DB
    } else {
        (20.0 * lin.log10()).max(meter::MIN_DB)
    }
}

/// What the column is about.
#[derive(Copy, Clone)]
struct Target {
    strip: Option<cantus_core::StripId>,
    gain: f32,
    pan: f32,
    width: f32,
    phase: bool,
    mute: bool,
    solo: bool,
    swap: bool,
}

pub fn show(
    st: &mut Inspector,
    app: &CantusApp,
    ui: &mut Ui,
    rect: Rect,
    s: &Style,
    subject: Subject,
) {
    let p = app.project();
    let sid = strip_of(app, subject).filter(|id| p.strip(*id).is_some());
    let tg = match sid.and_then(|id| p.strip(id)) {
        Some(m) => Target {
            strip: Some(m.id),
            gain: m.gain_db,
            pan: m.pan,
            width: m.width,
            phase: m.phase_invert,
            mute: m.muted,
            solo: m.soloed,
            swap: m.swap_channels,
        },
        None => Target {
            strip: None,
            gain: p.master.gain_db,
            pan: p.master.pan,
            width: p.master.width,
            phase: p.master.phase_invert,
            mute: false,
            solo: false,
            swap: false,
        },
    };
    let t = &s.t;
    let b = &mut st.brush;
    b.register(rect, "pinned_strip", &[crate::kit::Tok::Strip], &[]);
    b.rect(b.snap_rect(rect), Fill::Solid(t.strip));
    b.hline(
        rect.left(),
        rect.right(),
        rect.top() + b.px() * 0.5,
        b.px(),
        shade(t.strip, -0.12),
    );
    let k = |v: f32| s.s(v);
    let pad = k(10.0);
    let inner = rect.shrink2(Vec2::new(0.0, pad));

    // The meter: a strip's post-fader peak, or the master's two sides.
    let raw = match tg.strip.and_then(|id| p.strip_index(id)) {
        Some(i) => {
            let v = app.engine.meters().strips.get(i).copied().unwrap_or(0.0);
            [v, v]
        }
        None => app.engine.meters().master,
    };
    let now = ui.input(|i| i.time);
    let dt = ui.input(|i| i.stable_dt).clamp(0.0, 0.1);
    let m = &mut st.pin;
    let mut moving = false;
    for (ch, lin) in raw.into_iter().enumerate() {
        let db = to_db(lin);
        // Up at once, down at 24 dB a second; the peak holds for a second and a half.
        m.level[ch] = if db >= m.level[ch] {
            db
        } else {
            (m.level[ch] - 24.0 * dt).max(db).max(meter::MIN_DB)
        };
        if db >= m.hold[ch] || now - m.held[ch] > 1.5 {
            m.hold[ch] = db.max(m.level[ch]);
            m.held[ch] = now;
        }
        moving |= m.level[ch] > meter::MIN_DB + 0.1 || m.hold[ch] > meter::MIN_DB + 0.1;
    }
    if moving {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(16));
    }
    let mr = Rect::from_min_size(
        pos2(rect.left() + k(14.0), inner.top()),
        Vec2::new(k(18.0), inner.height()),
    );
    meter::paint(
        b,
        mr,
        s,
        &Levels {
            level: m.level,
            hold: m.hold,
        },
    );
    let meter_resp = ui.interact(mr, Id::new("insp-pin-meter"), Sense::hover());
    if let Some(hp) = meter_resp.hover_pos() {
        let pos = (mr.bottom() - hp.y) / mr.height().max(1.0);
        let name = st.fmt.tr("kit.mixer.legend.meter");
        hint::set(ui.ctx(), name, Caps::NONE, false, |w| {
            std::fmt::Write::write_fmt(w, format_args!("{:.1} dB", meter::db_at(pos)))
        });
    }

    // The fader, on the same travel as the desk's; the wheel moves it only with Ctrl.
    let fr = Rect::from_min_size(
        pos2(mr.right() + k(6.0), inner.top() - k(2.0)),
        Vec2::new(k(30.0), inner.height() + k(4.0)),
    );
    let resp = ui.interact(
        fr,
        Id::new(("insp-pin-fader", tg.strip)),
        Sense::click_and_drag(),
    );
    let mut gp = pos_of(tg.gain);
    let before = gp;
    let targets = tg.targets();
    let fader_mod = PinMod::new(app, targets[0].clone(), gp, s);
    let fader_depth = fader_mod.input(st, ui, &resp, "kit.param.volume");
    let b = &mut st.brush;
    if resp.dragged() && !fader_depth {
        let fine = if ui.input(|i| i.modifiers.shift) {
            0.2
        } else {
            1.0
        };
        let travel = (fr.height() - fader::CAP.y * s.k).max(1.0);
        gp = (gp - resp.drag_delta().y * fine / travel).clamp(0.0, 1.0);
    }
    if resp.hovered() && ui.input(|i| i.modifiers.command) {
        let dy = ui.input(|i| i.smooth_scroll_delta.y);
        gp = (gp + dy / 400.0).clamp(0.0, 1.0);
    }
    let mut new_gain = ((gp - before).abs() > f32::EPSILON).then(|| db_at(gp));
    if resp.double_clicked() {
        new_gain = Some(0.0);
    }
    let mut ticks = [0.0f32; 11];
    for (d, v) in ticks.iter_mut().zip(TICKS_DB) {
        *d = pos_of(v);
    }
    fader::paint(
        b,
        fr,
        s,
        State {
            lit: tg.strip.is_some(),
            ..State::of(&resp, false)
        },
        &FaderLook {
            pos: gp,
            ticks: &ticks,
            unity: Some(pos_of(0.0)),
        },
    );
    fader_mod.paint(b, ui, &resp, fr, s, Live::Fader, "kit.param.volume");

    // The right-hand block: pan, the readouts, the switches.
    let x0 = fr.right() + k(10.0);
    let x1 = rect.right() - k(10.0);
    let bw = (x1 - x0).max(k(40.0));
    let knob_c = pos2(x0 + k(18.0), inner.top() + k(18.0));
    let kr = Rect::from_center_size(knob_c, Vec2::splat(k(30.0)));
    let mut pan_d = sheet::drag(
        ui,
        Id::new(("insp-pin-pan", tg.strip)),
        kr,
        tg.pan,
        &Range::lin(-1.0, 1.0, 0.0).bipolar(),
        false,
    );
    let pan_mod = PinMod::new(app, targets[1].clone(), (tg.pan + 1.0) * 0.5, s);
    if pan_mod.input_drag(st, ui, &mut pan_d, "kit.param.pan") {
        pan_d.value = None;
    }
    let b = &mut st.brush;
    let pan_now = pan_d.value.unwrap_or(tg.pan);
    let hue = s.function(Function::Output);
    knob::paint(
        b,
        knob_c,
        &hue,
        State::of(&pan_d.resp, false),
        &pan_mod.knob((pan_now + 1.0) * 0.5, 0.5),
    );
    pan_mod.paint(b, ui, &pan_d.resp, kr, s, Live::Knob, "kit.param.pan");
    let cap = Text::key("kit.param.pan", Role::Micro, t.text_dim);
    let _ = text::paint(
        b,
        Rect::from_center_size(
            pos2(knob_c.x, knob_c.y + k(21.0)),
            Vec2::new(k(40.0), k(11.0)),
        ),
        &cap,
        s.tk(),
    );
    let rx = x0 + k(42.0);
    let row_h = k(22.0);
    let r1 = Rect::from_min_max(pos2(rx, inner.top()), pos2(x1, inner.top() + row_h));
    let r2 = r1.translate(Vec2::new(0.0, row_h + k(6.0)));
    let r3 = Rect::from_min_max(
        pos2(x0, r2.bottom() + k(6.0)),
        pos2(x1, r2.bottom() + k(6.0) + row_h),
    );
    let gain_now = new_gain.unwrap_or(tg.gain);
    let gain_d = pinned_well(
        st,
        app,
        ui,
        s,
        r1,
        ("insp-pin-gain", tg.strip),
        "kit.param.volume",
        gain_now,
        &sheet::GAIN,
        true,
        sheet::db,
        targets[0].clone(),
    );
    let pan_w = pinned_well(
        st,
        app,
        ui,
        s,
        r2,
        ("insp-pin-panw", tg.strip),
        "kit.param.pan",
        pan_now,
        &Range::lin(-1.0, 1.0, 0.0).bipolar(),
        false,
        sheet::pan,
        targets[1].clone(),
    );
    let width_d = pinned_well(
        st,
        app,
        ui,
        s,
        r3,
        ("insp-pin-width", tg.strip),
        "kit.param.width",
        tg.width,
        &Range::lin(0.0, 2.0, 1.0),
        false,
        sheet::percent,
        targets[2].clone(),
    );
    let b = &mut st.brush;
    let gap = k(6.0);
    let lw = (bw - gap) * 0.5;
    let l1 = Rect::from_min_size(pos2(x0, r3.bottom() + gap), Vec2::new(lw, k(20.0)));
    let l2 = l1.translate(Vec2::new(lw + gap, 0.0));
    let l3 = Rect::from_min_size(pos2(x0, l1.bottom() + gap), Vec2::new(lw, k(20.0)));
    let key_w = ((lw - gap) * 0.5).max(k(20.0));
    let g1 = Rect::from_min_size(pos2(l2.left(), l3.top()), Vec2::new(key_w, k(20.0)));
    let g2 = Rect::from_min_size(pos2(g1.right() + gap, l3.top()), Vec2::new(key_w, k(20.0)));
    let dyn_s = s.function(Function::Dynamics);
    let out_s = s.function(Function::Output);
    let is_strip = tg.strip.is_some();
    let mono = tg.width <= 0.005;
    let buttons: [(Rect, &str, bool, bool, &Style); 3] = [
        (l1, "kit.mixer.mute", tg.mute, is_strip, &dyn_s),
        (l2, "kit.mixer.solo", tg.solo, is_strip, &dyn_s),
        (l3, "kit.mixer.mono", mono, true, &out_s),
    ];
    let mut clicked = [false; 5];
    for (i, (r, key, on, enabled, style)) in buttons.into_iter().enumerate() {
        let resp = ui.interact(r, Id::new(("insp-pin-led", key)), Sense::click());
        led::button(
            b,
            r,
            style,
            State {
                disabled: !enabled,
                ..State::of(&resp, on)
            },
            Src::Key(key),
        );
        clicked[i] = resp.clicked() && enabled;
    }
    for (i, (r, g, on, enabled, tip)) in [
        (g1, Glyph::Phase, tg.phase, true, "track.phase_tip"),
        (g2, Glyph::Swap, tg.swap, is_strip, "mixer.swap_tip"),
    ]
    .into_iter()
    .enumerate()
    {
        let resp = ui.interact(r, Id::new(("insp-pin-key", i)), Sense::click());
        sheet::glyph_key(
            b,
            r,
            &out_s,
            State {
                disabled: !enabled,
                ..State::of(&resp, on)
            },
            g,
        );
        clicked[3 + i] = resp.clicked() && enabled;
        if resp.hovered() {
            let name = st.fmt.tr(tip);
            hint::set_text(ui.ctx(), name, "", Caps::NONE);
        }
    }

    // What was done, as commands; a strip's fader goes through the ganged path.
    let acts = &mut st.acts;
    let gain = new_gain.or(gain_d.value);
    let pan = pan_d.value.or(pan_w.value);
    match tg.strip {
        Some(id) => {
            if let Some(g) = gain {
                acts.push(Act::StripGain(id, g));
            }
            if let Some(pan) = pan {
                acts.push(Act::Merge(
                    Command::SetStripPan { id, pan },
                    MergeKey::of(&("spans", id.0)),
                ));
            }
            if let Some(width) = width_d.value {
                acts.push(Act::Merge(
                    Command::SetStripWidth { id, width },
                    MergeKey::of(&("swidth", id.0)),
                ));
            }
            if clicked[0] {
                acts.push(Act::Exec(Command::SetStripMute {
                    id,
                    muted: !tg.mute,
                }));
            }
            if clicked[1] {
                acts.push(Act::ToggleSolo(id));
            }
            if clicked[2] {
                acts.push(Act::Exec(Command::SetStripWidth {
                    id,
                    width: if mono { 1.0 } else { 0.0 },
                }));
            }
            if clicked[3] {
                acts.push(Act::Exec(Command::SetStripPhaseInvert {
                    id,
                    invert: !tg.phase,
                }));
            }
            if clicked[4] {
                acts.push(Act::Exec(Command::SetStripSwapChannels {
                    id,
                    swap: !tg.swap,
                }));
            }
        }
        None => {
            if let Some(gain_db) = gain {
                acts.push(Act::Merge(
                    Command::SetMasterGain { gain_db },
                    MergeKey::of(&"master"),
                ));
            }
            if let Some(pan) = pan {
                acts.push(Act::Merge(
                    Command::SetMasterPan { pan },
                    MergeKey::of(&"master-pan"),
                ));
            }
            if let Some(width) = width_d.value {
                acts.push(Act::Merge(
                    Command::SetMasterWidth { width },
                    MergeKey::of(&"master-width"),
                ));
            }
            if clicked[2] {
                acts.push(Act::Exec(Command::SetMasterWidth {
                    width: if mono { 1.0 } else { 0.0 },
                }));
            }
            if clicked[3] {
                acts.push(Act::Exec(Command::SetMasterPhaseInvert {
                    invert: !tg.phase,
                }));
            }
        }
    }
    if resp.drag_stopped()
        || resp.double_clicked()
        || pan_d.ended
        || gain_d.ended
        || pan_w.ended
        || width_d.ended
    {
        acts.push(Act::EndGesture);
    }
}

/// A readout in the pinned block: the value right, its name small at the left, a level
/// bar — and a drag target like every other well.
#[allow(clippy::too_many_arguments)]
fn pinned_well(
    st: &mut Inspector,
    app: &CantusApp,
    ui: &mut Ui,
    s: &Style,
    r: Rect,
    salt: (&'static str, Option<cantus_core::StripId>),
    key: &'static str,
    v: f32,
    range: &Range,
    gain: bool,
    write: fn(f32, &mut String),
    target: AutoTarget,
) -> sheet::Drag {
    let mut d = sheet::drag(ui, Id::new(salt), r, v, range, gain);
    let base = if gain {
        sheet::gain_pos(v)
    } else {
        range.pos(v)
    };
    let pm = PinMod::new(app, target, base, s);
    pm.input_drag(st, ui, &mut d, key);
    let shown = d.value.unwrap_or(v);
    let text = st
        .fmt
        .get(cache::key(salt), sheet::bits(shown), |b| write(shown, b));
    let pos = if gain {
        sheet::gain_pos(shown)
    } else {
        range.pos(shown)
    };
    let hs = s.function(Function::Output);
    sheet::value_well(
        &mut st.brush,
        r,
        &hs,
        State::of(&d.resp, false),
        text,
        true,
        Some((pos, range.bipolar)),
    );
    let cap = Text::key(key, Role::Micro, s.t.text_dim)
        .align(Align2::LEFT_CENTER)
        .policy(text::Policy::Abbreviate);
    // The caption stops short of the value (last pass: "VOLUMEN+0.0 dB" at some widths);
    // where its whole does not fit there, its short form, else elided.
    let value_w = text::measure(ui.ctx(), &Text::new(text, Role::Value, s.t.text), s.tk()).x;
    let left = r.left() + s.s(8.0);
    let cap_right = (r.right() - s.s(6.0) - value_w - s.s(6.0))
        .min(r.center().x)
        .max(left);
    let _ = text::paint(
        &mut st.brush,
        Rect::from_min_max(pos2(left, r.top()), pos2(cap_right, r.bottom())),
        &cap,
        s.tk(),
    );
    if d.resp.hovered() || d.resp.dragged() {
        let text = sheet::name(text);
        let name = st.fmt.tr(key);
        hint::set(ui.ctx(), name, Caps::PARAM, d.resp.dragged(), |b| {
            std::fmt::Write::write_str(b, &text)
        });
    }
    pm.paint(&mut st.brush, ui, &d.resp, r, s, Live::Edge, key);
    d
}

impl Target {
    /// What its volume, pan and width are as automation targets: the strip's, else the
    /// master's.
    fn targets(&self) -> [AutoTarget; 3] {
        match self.strip {
            Some(id) => [
                AutoTarget::StripGain(id),
                AutoTarget::StripPan(id),
                AutoTarget::StripWidth(id),
            ],
            None => [
                AutoTarget::MasterGain,
                AutoTarget::MasterPan,
                AutoTarget::MasterWidth,
            ],
        }
    }
}

/// A pinned control's modulation (ADR 0057, review 2), as the desk's copies of the same
/// controls have it: the parameter's menu on a right-click (*Create automation clip*,
/// *Modulate…*), a modulator's handle dropped on it, Alt+drag (or any drag while a
/// modulator maps) for the depth — only the drag; a double-click still resets —, the
/// marks, the drop outline and where modulation has it now.
struct PinMod {
    target: AutoTarget,
    offer: crate::kit::automate::Offer,
    /// The modulations arriving, as the desk's knobs draw them (review of fix3: the pinned
    /// pan knob drew none).
    rings: [(f32, egui::Color32); crate::views::modulator::MAX_RINGS],
    n: usize,
}

/// Where a pinned control shows the live modulated value.
#[derive(Copy, Clone, Debug, PartialEq)]
enum Live {
    /// A tick at the left edge, at its height (the value wells).
    Edge,
    /// The desk's mark beside the fader's cap, where the cap would sit
    /// ([`fader::live_mark`]; review of fix3: the tick ran over the whole rect, up to half
    /// a cap off at the ends).
    Fader,
    /// The knob's own dot and rings.
    Knob,
}

impl Live {
    /// Its name for the probe (`automate::probe::note_live`).
    fn name(self) -> &'static str {
        match self {
            Live::Edge => "edge",
            Live::Fader => "fader",
            Live::Knob => "knob",
        }
    }

    /// Where its mark points at `live` in a control's `rect`: the edge tick's left end or
    /// the fader mark's tip; `None` for a knob (its dot is the knob's own).
    fn at(self, rect: Rect, s: &Style, live: f32) -> Option<egui::Pos2> {
        match self {
            Live::Edge => Some(pos2(
                rect.left() + s.s(1.0),
                rect.bottom() - rect.height() * live.clamp(0.0, 1.0),
            )),
            Live::Fader => Some(fader::live_mark_tip(rect, s, live)),
            Live::Knob => None,
        }
    }
}

impl PinMod {
    fn new(app: &CantusApp, target: AutoTarget, pos: f32, s: &Style) -> Self {
        let mut rings = [(0.0, egui::Color32::TRANSPARENT); crate::views::modulator::MAX_RINGS];
        let (m, n) = app.mod_offer(&target, pos, s, &mut rings);
        PinMod {
            offer: crate::kit::automate::Offer {
                automated: app.is_automated(&target),
                typing: false,
                modulation: Some(m),
            },
            target,
            rings,
            n,
        }
    }

    /// The knob's look with its rings and live dot.
    fn knob(&self, value: f32, default: f32) -> KnobLook<'_> {
        KnobLook {
            rings: &self.rings[..self.n],
            live: self.offer.modulation.and_then(|m| m.live),
            ..KnobLook::new(KnobSize::S, value, default)
        }
    }

    /// The control's modulation answered; `true` while its drags are the depth's.
    fn input(&self, st: &mut Inspector, ui: &Ui, resp: &Response, probe: &str) -> bool {
        use crate::kit::automate as a;
        a::probe::note(probe, true);
        let choice = a::modulation_choice(ui, resp, Some(self.offer))
            .or_else(|| a::menu(ui, resp, self.offer));
        if let Some(c) = choice {
            st.acts.push(Act::Mod(super::modulators::ModAct::Param(
                self.target.clone(),
                c,
                resp.rect,
            )));
        }
        a::depth_drag(ui, Some(self.offer))
    }

    /// [`Self::input`] for a value's drag: its new value and its end left out while the
    /// drag is the depth's. Whether it was.
    fn input_drag(&self, st: &mut Inspector, ui: &Ui, d: &mut sheet::Drag, probe: &str) -> bool {
        let depth =
            self.input(st, ui, &d.resp, probe) && (d.resp.dragged() || d.resp.drag_stopped());
        if depth {
            d.value = None;
            d.ended = false;
        }
        depth
    }

    /// The marks, the live value shown `how`, the drop outline; `probe` names the control
    /// for the tests' probe.
    #[allow(clippy::too_many_arguments)]
    fn paint(
        &self,
        b: &mut crate::kit::Brush,
        ui: &Ui,
        resp: &Response,
        rect: Rect,
        s: &Style,
        how: Live,
        probe: &str,
    ) {
        use crate::kit::automate as a;
        a::probe::note_live(probe, how.name());
        if self.offer.automated {
            a::mark(b, rect, s);
        }
        if self.n > 0 {
            a::mod_mark(b, rect, s);
            let live = self.offer.modulation.and_then(|m| m.live);
            match (how, live) {
                (Live::Edge, Some(live)) => {
                    if let Some(p) = how.at(rect, s, live) {
                        b.hline(p.x, p.x + s.s(6.0), p.y, s.s(2.0), s.t.hue_mod);
                    }
                }
                (Live::Fader, Some(live)) => fader::live_mark(b, rect, s, live),
                _ => {}
            }
        }
        if a::handle_in_flight(ui.ctx()) && resp.contains_pointer() {
            a::drop_target(b, rect, s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Review of fix3: the pinned fader's live mark sits where the cap would (the desk's
    /// rule), not a cap's half off at the ends; the pan knob draws its modulation rings.
    /// Review of fix4 (the test compared a function with its own body): the fader's way
    /// puts the mark where the desk's cap sits, which the edge tick does not; that the
    /// pinned fader is drawn the fader's way is checked where the inspector is drawn
    /// (`modulators::the_pinned_strip_shows_its_modulation_the_desks_way`).
    #[test]
    fn the_pinned_controls_show_their_modulation_as_the_desk_does() {
        let s = Style::current();
        let rect = Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(30.0, 200.0));
        for live in [0.0, 1.0] {
            let desk = fader::cap_rect(rect, &s, live).center().y;
            let at = Live::Fader.at(rect, &s, live).expect("a mark");
            assert_eq!(at.y, desk, "at {live}");
            assert!(at.x >= rect.right() - s.s(2.0), "beside the cap: {at:?}");
            let edge = Live::Edge.at(rect, &s, live).expect("a tick");
            assert!(
                (edge.y - desk).abs() > 1.0,
                "the edge tick is not the desk's"
            );
        }
        assert_eq!(Live::Knob.at(rect, &s, 0.5), None);
        let mut p = crate::starter_project();
        let strip = p.tracks[0].strip.unwrap();
        let t = p.tracks[0].id;
        let mut m = cantus_core::modulator::Modulator::new(
            cantus_core::ModulatorId(p.alloc_id()),
            cantus_core::modulator::ModKind::Lfo,
        );
        m.routes.push(cantus_core::modulator::ModRoute {
            target: AutoTarget::StripPan(strip),
            depth: 0.4,
        });
        p.modulators_mut(cantus_core::modulator::ModHost::Track(t))
            .unwrap()
            .push(m);
        let app = CantusApp::from_parts(Some(p), cantus_engine::EngineHandle::dummy(), false);
        let pm = PinMod::new(&app, AutoTarget::StripPan(strip), 0.5, &s);
        assert_eq!(pm.knob(0.5, 0.5).rings.len(), 1, "the pan knob's ring");
    }
}
