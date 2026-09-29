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
use cantus_core::{Command, MergeKey};
use egui::{pos2, Align2, Id, Rect, Sense, Ui, Vec2};

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
    if resp.dragged() {
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

    // The right-hand block: pan, the readouts, the switches.
    let x0 = fr.right() + k(10.0);
    let x1 = rect.right() - k(10.0);
    let bw = (x1 - x0).max(k(40.0));
    let knob_c = pos2(x0 + k(18.0), inner.top() + k(18.0));
    let kr = Rect::from_center_size(knob_c, Vec2::splat(k(30.0)));
    let pan_d = sheet::drag(
        ui,
        Id::new(("insp-pin-pan", tg.strip)),
        kr,
        tg.pan,
        &Range::lin(-1.0, 1.0, 0.0).bipolar(),
        false,
    );
    let pan_now = pan_d.value.unwrap_or(tg.pan);
    let hue = s.function(Function::Output);
    knob::paint(
        b,
        knob_c,
        &hue,
        State::of(&pan_d.resp, false),
        &KnobLook::new(KnobSize::S, (pan_now + 1.0) * 0.5, 0.5),
    );
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
        ui,
        s,
        r1,
        ("insp-pin-gain", tg.strip),
        "kit.param.volume",
        gain_now,
        &sheet::GAIN,
        true,
        sheet::db,
    );
    let pan_w = pinned_well(
        st,
        ui,
        s,
        r2,
        ("insp-pin-panw", tg.strip),
        "kit.param.pan",
        pan_now,
        &Range::lin(-1.0, 1.0, 0.0).bipolar(),
        false,
        sheet::pan,
    );
    let width_d = pinned_well(
        st,
        ui,
        s,
        r3,
        ("insp-pin-width", tg.strip),
        "kit.param.width",
        tg.width,
        &Range::lin(0.0, 2.0, 1.0),
        false,
        sheet::percent,
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
    ui: &mut Ui,
    s: &Style,
    r: Rect,
    salt: (&'static str, Option<cantus_core::StripId>),
    key: &'static str,
    v: f32,
    range: &Range,
    gain: bool,
    write: fn(f32, &mut String),
) -> sheet::Drag {
    let d = sheet::drag(ui, Id::new(salt), r, v, range, gain);
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
    let _ = text::paint(
        &mut st.brush,
        Rect::from_min_max(
            pos2(r.left() + s.s(8.0), r.top()),
            pos2(r.center().x, r.bottom()),
        ),
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
    d
}
