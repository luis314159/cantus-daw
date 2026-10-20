//! The key, in the tone hue: where it comes from and its knobs; its six-band EQ drawn over
//! its spectrum, with the selected band's numbers under it; and — in the dynamics hue —
//! the response graph: what this compressor does to a burst.

use super::*;
use crate::kit::{Fill, State as KitState};
use crate::views::fx::canvas::{self, Ink};
use crate::widgets::knob::{pos_of, value_at, Taper};
use cantus_dsp::aequalis::{design as design_band, Coefs, MAX_SECTIONS};
use cantus_dsp::comprimo::step_response;
use std::cell::RefCell;

/// One key-EQ band as the editor reads it from the spec.
#[derive(Copy, Clone, Debug)]
pub(super) struct KeyBandView {
    pub used: bool,
    pub on: bool,
    pub shape: EqShape,
    pub freq: f32,
    pub gain: f32,
    pub q: f32,
    pub slope: f32,
}

/// The six bands, read through the parameter table's keys: nothing is formatted.
pub(super) fn key_bands(spec: &EffectSpec) -> [KeyBandView; KEY_BANDS] {
    std::array::from_fn(|b| {
        let g = |f: usize| spec.get(band_key(b, f));
        KeyBandView {
            used: g(F_USED) >= 0.5,
            on: g(F_ON) >= 0.5,
            shape: EqShape::from_index(g(F_SHAPE).round().max(0.0) as usize),
            freq: g(F_FREQ),
            gain: g(F_GAIN),
            q: g(F_Q),
            slope: (g(F_SLOPE) / 6.0).round().clamp(1.0, 16.0) * 6.0,
        }
    })
}

/// The key EQ's curve at its display points: every band in use and on, as the detector
/// hears it — each band designed once.
fn key_curve(bands: &[KeyBandView; KEY_BANDS], out: &mut [f32; KEY_POINTS]) {
    let span = (KEY_HI_HZ / KEY_LO_HZ).ln();
    let mut coefs = [[Coefs::IDENTITY; MAX_SECTIONS]; KEY_BANDS];
    let mut n = [0usize; KEY_BANDS];
    for (i, b) in bands.iter().enumerate() {
        if b.used && b.on {
            n[i] = design_band(
                b.shape,
                DISPLAY_SR,
                b.freq,
                b.gain,
                b.q,
                b.slope,
                &mut coefs[i],
            );
        }
    }
    for (p, o) in out.iter_mut().enumerate() {
        let hz = KEY_LO_HZ * (span * p as f32 / (KEY_POINTS - 1) as f32).exp();
        *o = (0..KEY_BANDS)
            .map(|i| {
                coefs[i][..n[i]]
                    .iter()
                    .map(|c| c.magnitude_db(DISPLAY_SR, hz))
                    .sum::<f32>()
            })
            .sum();
    }
}

/// `1.2 kHz`, `350 Hz`.
struct Hz(f32);

impl std::fmt::Display for Hz {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 >= 1_000.0 {
            write!(f, "{}k", (self.0 / 1_000.0).round())
        } else {
            write!(f, "{:.0}", self.0)
        }
    }
}

/// The key: where it comes from (the desk's strips, or the plug-in's sidechain), its
/// level, its high-pass and the stereo link.
pub(super) fn key_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
) {
    let tone = s.function(Function::Tone);
    let s = &tone;
    controls::card(ui, s, rect, Src::Key("comp.key"), None, |ui, _| {
        // What it listens to: a caption over the menu field, level with the knobs.
        col(ui, s, KEY_COLUMN, false, |ui| {
            centre(ui, s, 14.0 + 5.0 + controls::ROW);
            controls::caption(ui, s, Src::Key("comp.key_from"), KEY_COLUMN);
            key_source(host, st, ui, s, KEY_COLUMN);
        });
        controls::param(ui, s, host, "key_gain", Size::M);
        controls::param_with(ui, s, host, "key_hpf", Size::M, Some(&hpf_pill));
        controls::param(ui, s, host, "link", Size::M);
    });
}

/// The key's EQ: its lamp puts it in or out (the bands are kept); ‹ Band 2 › steps
/// through the bands in use and ⋯ is the selected band's menu; the display is the
/// control; under it, the selected band's shape and numbers.
pub(super) fn key_eq_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    cs: &CompSettings,
    rect: Rect,
) {
    let tone = s.function(Function::Tone);
    let s = &tone;
    let k = s.k;
    let bands = key_bands(host.spec());
    let sel = st.key_band.filter(|b| bands[*b].used);
    let lang = crate::i18n::locale();
    let label = sel.map(|b| {
        cached(&mut st.words.band, (b, lang), || {
            tf("comp.key_band", &[("n", &(b + 1))])
        })
    });
    let row_h = 8.0 + controls::ROW + 8.0;
    let look = band::CardLook {
        title: Src::Key("comp.key_eq_title"),
        switchable: true,
        selector: label,
        menu: sel.is_some(),
        display: Some((rect.height() / k - band::TITLE - 1.0 - row_h).max(20.0)),
    };
    let lit = KitState {
        lit: cs.key_eq,
        ..KitState::REST
    };
    let rects = with_brush(&ui.painter_at(rect.expand(4.0)), |b| {
        band::card(b, rect, s, lit, &look)
    });
    let id = ui.id().with("comp-key-eq");
    if let Some(led) = rects.led {
        let r = ui.interact(led.expand(4.0 * k), id.with("led"), Sense::click());
        if r.hovered() {
            hint::set_text(
                ui.ctx(),
                frame::tr("comp.key_eq"),
                frame::tr("comp.key_eq_tip"),
                Caps::NONE,
            );
        }
        // The key EQ's On is a parameter (round 29, review 2 of ADR 0062, #44).
        let learnt = crate::views::fx::controls::stepped_offer(
            ui,
            s,
            host,
            "key_eq",
            frame::tr("comp.key_eq"),
            &r,
        );
        if r.clicked() && !learnt {
            host.set("key_eq", if cs.key_eq { 0.0 } else { 1.0 }, None);
        }
    }
    if let (Some(sr), Some(b)) = (rects.selector, sel) {
        let r = ui.interact(sr, id.with("selector"), Sense::click());
        if r.hovered() {
            hint::set_text(ui.ctx(), frame::tr("comp.key_band_step"), "", Caps::NONE);
        }
        if let (true, Some(p)) = (r.clicked(), r.interact_pointer_pos()) {
            let used: [bool; KEY_BANDS] = std::array::from_fn(|i| bands[i].used);
            let step = if p.x < sr.center().x {
                KEY_BANDS - 1
            } else {
                1
            };
            let mut i = b;
            for _ in 0..KEY_BANDS {
                i = (i + step) % KEY_BANDS;
                if used[i] {
                    break;
                }
            }
            st.key_band = Some(i);
        }
    }
    if let Some(m) = rects.menu {
        let r = ui.interact(m.expand(2.0 * k), id.with("menu"), Sense::click());
        if r.hovered() {
            hint::set_text(
                ui.ctx(),
                frame::tr("comp.key_band_menu"),
                "",
                Caps {
                    menu: true,
                    ..Caps::NONE
                },
            );
        }
        if r.clicked() {
            st.band_menu = match st.band_menu {
                Some(_) => None,
                None => Some((pos2(m.right() - 200.0 * k, m.bottom()), m)),
            };
        }
    }
    if let Some(d) = rects.display {
        key_eq_display(host, st, ui, s, cs, &bands, d);
    }
    band_row(host, st, ui, s, &bands, rects.body);
    band_menu(host, st, ui, s, &bands);
}
/// The key's EQ over the key's spectrum. Double-click to add a band (a cut at the edges,
/// a bell elsewhere), drag a dot, the wheel for Q (slope on a cut), Alt+click to bypass,
/// right-click for its menu, Delete to remove it.
fn key_eq_display(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    cs: &CompSettings,
    bands: &[KeyBandView; KEY_BANDS],
    rect: Rect,
) {
    let k = s.k;
    let tk = s.tk();
    let ink = Ink::of(s);
    let resp = ui.interact(
        rect,
        ui.id().with("comp-key-eq-display"),
        Sense::click_and_drag(),
    );
    st.geometry.key_eq = Some(rect);
    let span = (KEY_HI_HZ / KEY_LO_HZ).ln();
    let x_of = |hz: f32| rect.left() + (hz / KEY_LO_HZ).ln() / span * rect.width();
    let hz_of =
        |px: f32| KEY_LO_HZ * (span * ((px - rect.left()) / rect.width()).clamp(0.0, 1.0)).exp();
    let y_of = |d: f32| rect.center().y - (d / KEY_DB).clamp(-1.0, 1.0) * rect.height() * 0.45;
    let db_of = |py: f32| (rect.center().y - py) / (rect.height() * 0.45) * KEY_DB;
    let spec_y = |d: f32| rect.bottom() - ((d + 90.0) / 90.0).clamp(0.0, 1.0) * rect.height();
    let rev = host.revision();
    if st.key_curve.as_ref().is_none_or(|(r, _)| *r != rev) {
        let mut c = [0.0; KEY_POINTS];
        key_curve(bands, &mut c);
        st.key_curve = Some((rev, c));
    }
    let curve = st.key_curve.as_ref().map_or(&[0.0; KEY_POINTS], |(_, c)| c);
    // The key's spectrum: its own input when the key is this strip; the key itself (the
    // output) while you listen to it. A key from elsewhere is not tapped otherwise.
    let internal = match st.key.as_ref().and_then(|(_, k)| k.as_ref()) {
        Some(KeySource::Strips { current, .. }) => current.is_none(),
        Some(KeySource::Switch(on)) => !on,
        None => true,
    };
    let source = if cs.listen == Listen::Key {
        Some(1)
    } else if internal {
        Some(0)
    } else {
        None
    };
    let an = st.analyser.as_ref().filter(|a| a.has_data());
    let dot = |v: &KeyBandView| {
        pos2(
            x_of(v.freq.clamp(KEY_LO_HZ, KEY_HI_HZ)),
            y_of(if v.shape.has_gain() { v.gain } else { 0.0 }),
        )
    };
    let hit = |pt: Pos2| {
        (0..KEY_BANDS)
            .filter(|b| bands[*b].used)
            .map(|b| (b, dot(&bands[b]).distance(pt)))
            .filter(|(_, d)| *d < 10.0 * k)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(b, _)| b)
    };
    let hovered = resp.hover_pos().and_then(hit);
    let pressed_on = ui.input(|i| i.pointer.press_origin()).and_then(hit);
    let selected = st.key_band;
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, s.hue_tok);
        let px = b.px();
        for hz in [
            50.0f32, 100.0, 200.0, 500.0, 1_000.0, 2_000.0, 5_000.0, 10_000.0,
        ] {
            b.vline(
                b.snap_mid(x_of(hz)),
                rect.top(),
                rect.bottom(),
                px,
                ink.grid,
            );
        }
        for d in [-12.0f32, 12.0] {
            b.hline(rect.left(), rect.right(), b.snap_mid(y_of(d)), px, ink.grid);
        }
        b.hline(
            rect.left(),
            rect.right(),
            b.snap_mid(y_of(0.0)),
            px,
            ink.grid_strong,
        );
        let mut pts = [Pos2::ZERO; KEY_POINTS];
        if let (Some(which), Some(an)) = (source, an) {
            for (i, p) in pts.iter_mut().enumerate() {
                let hz = KEY_LO_HZ * (span * i as f32 / (KEY_POINTS - 1) as f32).exp();
                // Tilted 4.5 dB/octave about 1 kHz, as the EQ's analyser: music reads
                // level.
                let d = an.level_db(which, DISPLAY_SR, hz, 1.0 / 6.0) + 4.5 * (hz / 1_000.0).log2();
                *p = pos2(x_of(hz), spec_y(d));
            }
            b.area(&pts, rect.bottom(), Ink::a(ink.dim, 0.22));
            if which == 0 && cs.key_eq {
                // What the detector hears: the input through the EQ.
                for (p, c) in pts.iter_mut().zip(curve.iter()) {
                    let d = (rect.bottom() - p.y) / rect.height() * 90.0 - 90.0;
                    p.y = spec_y(d + c);
                }
                b.polyline(&pts, 1.0 * k, Ink::a(ink.text, 0.55));
            }
        }
        // The curve: solid when the EQ is in, dashed when it is out.
        for (i, p) in pts.iter_mut().enumerate() {
            let hz = KEY_LO_HZ * (span * i as f32 / (KEY_POINTS - 1) as f32).exp();
            *p = pos2(x_of(hz), y_of(curve[i]));
        }
        if cs.key_eq {
            b.polyline(&pts, 2.0 * k, ink.hue);
        } else {
            for w in pts.windows(2).step_by(2) {
                b.segment(w[0], w[1], 1.25 * k, ink.hue, false);
            }
        }
        // The dots, numbered: a band keeps its number for life.
        let num = crate::kit::color::ensure_contrast(ink.ground, ink.hue, 3.0);
        for (i, v) in bands.iter().enumerate() {
            if !v.used {
                continue;
            }
            let at = dot(v);
            let big = selected == Some(i) || hovered == Some(i);
            let r = if big { 8.0 } else { 7.0 } * k;
            if v.on {
                b.circle(at, r, Fill::Solid(ink.hue));
            } else {
                b.circle(at, r, Fill::Solid(ink.ground));
                b.circle_stroke(at, r - 0.75 * k, 1.5 * k, ink.hue);
            }
            if selected == Some(i) {
                b.circle_stroke(at, r + 2.5 * k, 1.5 * k, ink.text);
            }
            canvas::putf(
                b,
                at,
                Align2::CENTER_CENTER,
                Role::Micro,
                if v.on { num } else { ink.hue },
                tk,
                None,
                format_args!("{}", i + 1),
            );
        }
        // Words: the frequencies, the state.
        let mut ruler = canvas::Ruler::new();
        for hz in [
            50.0f32, 100.0, 200.0, 500.0, 1_000.0, 2_000.0, 5_000.0, 10_000.0,
        ] {
            let at = pos2(x_of(hz) + 2.0 * k, rect.bottom() - 2.0 * k);
            if !ruler.fits(at.x, 4.0 * k) {
                continue;
            }
            let r = canvas::putf(
                b,
                at,
                Align2::LEFT_BOTTOM,
                Role::Micro,
                ink.dim,
                tk,
                None,
                format_args!("{}", Hz(hz)),
            );
            ruler.placed(r);
        }
        if !cs.key_eq {
            canvas::put(
                b,
                pos2(rect.left() + 6.0 * k, rect.top() + 4.0 * k),
                Align2::LEFT_TOP,
                &Text::key("comp.bypassed", Role::Micro, ink.dim),
                tk,
                Some(ink.plate),
            );
        }
        if source.is_none() {
            canvas::put(
                b,
                pos2(rect.right() - 6.0 * k, rect.top() + 4.0 * k),
                Align2::RIGHT_TOP,
                &Text::key("comp.key_spectrum_hint", Role::Micro, ink.dim),
                tk,
                Some(ink.plate),
            );
        }
        if !bands.iter().any(|b| b.used) {
            let box_ = rect.shrink2(vec2(12.0 * k, 0.0));
            let t = Text::key("comp.key_eq_empty", Role::Body, ink.dim)
                .caps(false)
                .align(Align2::CENTER_CENTER)
                .policy(Policy::Elide(Elide::End));
            let _ = ktext::paint(b, box_, &t, tk);
        }
    });
    key_eq_gestures(
        host,
        st,
        ui,
        &resp,
        bands,
        (hovered, pressed_on),
        (&hz_of, &db_of),
    );
}

/// The key EQ's gestures: select, Alt+click to bypass, double-click to add, drag, wheel,
/// Delete, and the band's menu on a right-click.
fn key_eq_gestures(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    resp: &egui::Response,
    bands: &[KeyBandView; KEY_BANDS],
    (hovered, pressed_on): (Option<usize>, Option<usize>),
    (hz_of, db_of): (&dyn Fn(f32) -> f32, &dyn Fn(f32) -> f32),
) {
    let (alt, shift) = ui.input(|i| (i.modifiers.alt, i.modifiers.shift));
    if resp.clicked() {
        match hovered {
            Some(b) if alt => {
                let on = if bands[b].on { 0.0 } else { 1.0 };
                host.set(band_key(b, F_ON), on, None);
            }
            Some(b) => st.key_band = Some(b),
            None => st.key_band = None,
        }
    }
    if resp.secondary_clicked() {
        if let (Some(b), Some(p)) = (hovered, resp.interact_pointer_pos()) {
            st.key_band = Some(b);
            st.band_menu = Some((p, Rect::from_center_size(p, vec2(2.0, 2.0))));
        }
    }
    if resp.double_clicked() && hovered.is_none() {
        let free = (0..KEY_BANDS).find(|b| !bands[*b].used);
        if let (Some(b), Some(pt)) = (free, resp.interact_pointer_pos()) {
            let frac = (pt.x - resp.rect.left()) / resp.rect.width();
            let shape = if frac < 0.12 {
                EqShape::LowCut
            } else if frac > 0.88 {
                EqShape::HighCut
            } else {
                EqShape::Bell
            };
            let last = if shape.has_gain() {
                (band_key(b, F_GAIN), db_of(pt.y).clamp(-30.0, 30.0))
            } else {
                (band_key(b, F_SLOPE), 24.0)
            };
            host.set_many(
                &[
                    (band_key(b, F_USED), 1.0),
                    (band_key(b, F_ON), 1.0),
                    (band_key(b, F_SHAPE), shape.index() as f32),
                    (band_key(b, F_FREQ), hz_of(pt.x).clamp(KEY_LO_HZ, KEY_HI_HZ)),
                    last,
                ],
                "cmd.comp_key_band",
            );
            st.key_band = Some(b);
        }
    }
    if resp.drag_started() {
        st.key_drag = pressed_on;
        if pressed_on.is_some() {
            st.key_band = pressed_on;
        }
    }
    if let (Some(b), Some(pt)) = (st.key_drag, resp.interact_pointer_pos()) {
        if resp.dragged() {
            let freq = (
                band_key(b, F_FREQ),
                Some(hz_of(pt.x).clamp(KEY_LO_HZ, KEY_HI_HZ)),
            );
            let gain = (band_key(b, F_GAIN), Some(db_of(pt.y).clamp(-30.0, 30.0)));
            let edits = [freq, gain];
            let n = if bands[b].shape.has_gain() { 2 } else { 1 };
            host.apply(
                &edits[..n],
                "cmd.comp_key_band",
                Some(BAND_GESTURE + b as u64),
            );
        }
    }
    if resp.drag_stopped() {
        st.key_drag = None;
        host.end_gesture();
    }
    // The wheel: Q, or the slope of a cut in 6 dB steps.
    if let Some(b) = hovered.or(st.key_band).filter(|b| bands[*b].used) {
        let wheel = if resp.hovered() {
            ui.input(|i| i.smooth_scroll_delta.y)
        } else {
            0.0
        };
        if wheel != 0.0 {
            let v = bands[b];
            let g = Some(BAND_GESTURE + 16 + b as u64);
            if v.shape.has_slope() {
                let slope = (v.slope + 6.0 * wheel.signum()).clamp(6.0, 96.0);
                host.apply(
                    &[(band_key(b, F_SLOPE), Some(slope))],
                    "cmd.comp_key_band",
                    g,
                );
            } else {
                let fine = if shift { 0.2 } else { 1.0 };
                let q = (v.q * (wheel * 0.004 * fine).exp()).clamp(0.1, 18.0);
                host.apply(&[(band_key(b, F_Q), Some(q))], "cmd.comp_key_band", g);
            }
            st.wheel_at = Some(st.clock);
        }
    }
    let delete =
        ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace));
    if delete && resp.hovered() {
        if let Some(b) = st.key_band {
            remove_band(host, b);
            st.key_band = None;
        }
    }
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("comp.key_eq_title"),
            frame::tr("comp.key_eq_display_tip"),
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
}
thread_local! {
    static BUF: RefCell<String> = RefCell::new(String::with_capacity(32));
}

/// A band's number in a well: drag it (up or right for more; Shift for fine), the wheel,
/// double-click for its default, right-click to type. Returns the value it moved to and
/// whether the gesture ended.
fn value_well(
    ui: &mut egui::Ui,
    s: &Style,
    key: &'static str,
    value: f32,
    fmt: &dyn Fn(f32, &mut String),
    w: f32,
) -> (Option<f32>, bool) {
    let Some(info) = EffectKind::Compressor.param(key) else {
        return (None, false);
    };
    let (rect, resp) =
        ui.allocate_exact_size(vec2(w * s.k, s.d(controls::ROW)), Sense::click_and_drag());
    let taper = if info.logarithmic {
        Taper::Log
    } else {
        Taper::Linear
    };
    let (min, max) = (info.min, info.max);
    let p0 = pos_of(value, min, max, taper);
    let mut p = p0;
    let fine = if ui.input(|i| i.modifiers.shift) {
        0.2
    } else {
        1.0
    };
    if resp.dragged() {
        let d = resp.drag_delta();
        p += (d.x - d.y) * fine / 200.0;
    }
    if resp.hovered() {
        let dy = ui.input(|i| i.smooth_scroll_delta.y);
        p += dy * fine / 400.0;
    }
    let mut v = if p != p0 {
        value_at(p.clamp(0.0, 1.0), min, max, taper)
    } else {
        value
    };
    let mut done = resp.drag_stopped();
    if resp.double_clicked() {
        v = info.default;
        done = true;
    }
    if let Some(x) = controls::typing(ui, &resp, rect, s, info.unit, |b| fmt(value, b)) {
        v = x;
        done = true;
    }
    if info.unit == "dB/oct" {
        v = (v / 6.0).round() * 6.0;
    } else if info.stepped {
        v = v.round();
    }
    let v = v.clamp(min, max);
    let st = KitState::of(&resp, false);
    if st.hovered {
        hint::set(
            ui.ctx(),
            frame::tr(info.label_key),
            Caps::PARAM,
            resp.dragged(),
            |b| {
                fmt(v, b);
                Ok(())
            },
        );
    }
    if ui.is_rect_visible(rect) {
        BUF.with(|buf| {
            let mut buf = buf.borrow_mut();
            buf.clear();
            fmt(v, &mut buf);
            with_brush(ui.painter(), |b| {
                crate::kit::well::readout(b, rect, s, st, &buf, true)
            });
        });
    }
    ((v != value).then_some(v), done)
}

fn freq_well(v: f32, b: &mut String) {
    controls::write_readout(v, "Hz", b);
}

fn gain_well(v: f32, b: &mut String) {
    let _ = write!(b, "{v:+.1} dB");
}

fn q_well(v: f32, b: &mut String) {
    let _ = write!(b, "{v:.2} Q");
}

fn slope_well(v: f32, b: &mut String) {
    let _ = write!(b, "{v:.0} dB/oct");
}

/// The selected band under the display: its shape, frequency, gain, Q or slope, on/off,
/// remove — every band reachable from the keyboard, its state on screen.
fn band_row(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    bands: &[KeyBandView; KEY_BANDS],
    rect: Rect,
) {
    let mut ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let ui = &mut ui;
    gap_x(ui, s);
    let w = rect.width() / s.k;
    let Some(b) = st.key_band.filter(|b| bands[*b].used) else {
        words_line(
            ui,
            s,
            frame::tr("comp.key_band_none"),
            s.t.text_dim,
            frame::tr("comp.key_eq_title"),
            w,
        );
        return;
    };
    let v = bands[b];
    let id = ui.id().with(("comp-key-shape", b));
    let shape = v.shape.index().min(8);
    if let Some(at) = controls::field(
        ui,
        s,
        id,
        frame::tr(SHAPE_KEYS[shape]),
        frame::tr("fx.comp.sc.shape"),
        96.0,
    ) {
        let items: [Item<'_>; 9] =
            std::array::from_fn(|i| frame::radio(Src::Key(SHAPE_KEYS[i]), i == shape));
        if let Some(i) = controls::list(ui, s, id, at, 140.0, &items) {
            host.set(band_key(b, F_SHAPE), i as f32, None);
        }
    }
    let g = BAND_GESTURE + 32 + b as u64;
    let edit = |host: &mut dyn FxHost, f: usize, (to, done): (Option<f32>, bool)| {
        if let Some(x) = to {
            host.apply(
                &[(band_key(b, f), Some(x))],
                "cmd.comp_key_band",
                Some(g + f as u64),
            );
        }
        if done {
            host.end_gesture();
        }
    };
    let r = value_well(ui, s, band_key(b, F_FREQ), v.freq, &freq_well, 80.0);
    edit(host, F_FREQ, r);
    if v.shape.has_gain() {
        let r = value_well(ui, s, band_key(b, F_GAIN), v.gain, &gain_well, 72.0);
        edit(host, F_GAIN, r);
    }
    if v.shape.has_slope() {
        let r = value_well(ui, s, band_key(b, F_SLOPE), v.slope, &slope_well, 88.0);
        edit(host, F_SLOPE, r);
    } else {
        let r = value_well(ui, s, band_key(b, F_Q), v.q, &q_well, 68.0);
        edit(host, F_Q, r);
    }
    if controls::led(
        ui,
        s,
        v.on,
        Src::Key("comp.band_on"),
        frame::tr("eq.bypass_band"),
        68.0,
    )
    .clicked()
    {
        host.set(band_key(b, F_ON), if v.on { 0.0 } else { 1.0 }, None);
    }
    if controls::key(
        ui,
        s,
        Src::Key("comp.remove_band"),
        frame::tr("eq.remove_band"),
        80.0,
        false,
    )
    .clicked()
    {
        remove_band(host, b);
        st.key_band = None;
    }
}

/// The selected band's menu: its shape, bypass, remove.
fn band_menu(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    bands: &[KeyBandView; KEY_BANDS],
) {
    let Some((at, opener)) = st.band_menu else {
        return;
    };
    let Some(b) = st.key_band.filter(|b| bands[*b].used) else {
        st.band_menu = None;
        return;
    };
    let v = bands[b];
    let shape = v.shape.index().min(8);
    let label = st.words.band.as_ref().map_or("", |(_, t)| t.as_str());
    let items: [Item<'_>; 13] = std::array::from_fn(|i| match i {
        0 => Item::Band(Src::Str(label)),
        1..=9 => frame::radio(Src::Key(SHAPE_KEYS[i - 1]), i - 1 == shape),
        10 => Item::Separator,
        11 => frame::check(Src::Key("eq.bypass_band"), !v.on),
        _ => frame::action(Src::Key("eq.remove_band")),
    });
    let mut close = false;
    let chosen = frame::popup_list(
        ui.ctx(),
        ui.id().with("comp-band-menu"),
        at,
        opener,
        s,
        &items,
        200.0 * s.k,
        &mut close,
    );
    match chosen {
        Some(i @ 1..=9) => host.set(band_key(b, F_SHAPE), (i - 1) as f32, None),
        Some(11) => host.set(band_key(b, F_ON), if v.on { 0.0 } else { 1.0 }, None),
        Some(12) => {
            remove_band(host, b);
            st.key_band = None;
        }
        _ => {}
    }
    if close || chosen.is_some() {
        st.band_menu = None;
    }
}

/// How this compressor answers a burst: the reduction over time for a step 12 dB over the
/// threshold held 400 ms, then 20 dB under it — run through the real DSP with the settings
/// as they are, with the attack and release it measures. It is what "attack" means in this
/// style, drawn.
pub(super) fn response_card(
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    cs: &CompSettings,
    rect: Rect,
) {
    let k = s.k;
    let disp = (rect.height() / k - band::TITLE - 1.0).max(20.0);
    controls::card_with(
        ui,
        s,
        rect,
        Src::Key("comp.response"),
        None,
        Some(disp),
        |ui, cr| {
            if let Some(d) = cr.display {
                response(st, ui, s, cs, d);
            }
        },
    );
}

fn response(st: &mut State, ui: &mut egui::Ui, s: &Style, cs: &CompSettings, rect: Rect) {
    let k = s.k;
    let tk = s.tk();
    let ink = Ink::of(s);
    let resp = ui.interact(rect, ui.id().with("comp-response"), Sense::hover());
    if st.response.as_ref().is_none_or(|(c, _)| c != cs) {
        st.response = Some((*cs, step_response(cs, RESPONSE_SR, RESPONSE_POINTS)));
        st.words.response = None;
    }
    let Some((_, r)) = st.response.as_ref() else {
        return;
    };
    let lang = crate::i18n::locale();
    let line = cached(&mut st.words.response, lang, || {
        let ms = |v: f32| {
            let mut b = String::new();
            controls::write_readout(v, "ms", &mut b);
            b
        };
        match (r.attack_ms, r.release_ms) {
            (Some(a), Some(rel)) => tf(
                "comp.response_times",
                &[
                    ("att", &ms(a)),
                    ("rel", &ms(rel)),
                    ("gr", &format!("{:.1}", r.settled_db)),
                ],
            ),
            (Some(a), None) => tf("comp.response_held", &[("att", &ms(a))]),
            _ => t("comp.response_none"),
        }
    });
    let plot = Rect::from_min_max(
        rect.min + vec2(8.0 * k, 22.0 * k),
        rect.max - vec2(8.0 * k, 16.0 * k),
    );
    let x = |ms: f32| plot.left() + (ms / r.length_ms).clamp(0.0, 1.0) * plot.width();
    let scale = (r.settled_db * 1.3).max(6.0);
    let y = |gr: f32| plot.top() + (gr / scale).clamp(0.0, 1.0) * plot.height();
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, s.hue_tok);
        let px = b.px();
        // The burst, as a band behind the curve.
        b.rect(
            Rect::from_x_y_ranges(x(r.rise_ms)..=x(r.fall_ms), plot.y_range()),
            Fill::Solid(Ink::a(ink.dim, if ink.dark { 0.14 } else { 0.10 })),
        );
        for ms in [0.0f32, 250.0, 500.0, 750.0, 1_000.0] {
            b.vline(b.snap_mid(x(ms)), plot.top(), plot.bottom(), px, ink.grid);
        }
        canvas::dashed(
            b,
            pos2(plot.left(), y(r.settled_db)),
            pos2(plot.right(), y(r.settled_db)),
            px,
            ink.grid_strong,
            3.0 * k,
            3.0 * k,
        );
        let n = r.gr_db.len().min(RESPONSE_POINTS);
        let dt = r.length_ms / r.gr_db.len().max(1) as f32;
        let mut pts = [Pos2::ZERO; RESPONSE_POINTS];
        for (i, (p, g)) in pts.iter_mut().zip(&r.gr_db).enumerate() {
            *p = pos2(x(i as f32 * dt), y(*g));
        }
        b.area(
            &pts[..n],
            plot.top(),
            Ink::a(ink.danger, if ink.dark { 0.32 } else { 0.22 }),
        );
        b.polyline(&pts[..n], 1.5 * k, ink.danger);
        // The measured points: 63 % in, 63 % out.
        if let Some(a) = r.attack_ms {
            b.circle_stroke(
                pos2(x(r.step_ms + a), y(r.settled_db * 0.632)),
                3.5 * k,
                1.5 * k,
                ink.text,
            );
        }
        if let Some(rel) = r.release_ms {
            b.circle_stroke(
                pos2(x(r.drop_ms + rel), y(r.settled_db * 0.368)),
                3.5 * k,
                1.5 * k,
                ink.text,
            );
        }
        let mut ruler = canvas::Ruler::new();
        for ms in [0.0f32, 250.0, 500.0, 750.0, 1_000.0] {
            let at = pos2(x(ms) + 2.0 * k, rect.bottom() - 2.0 * k);
            if !ruler.fits(at.x, 4.0 * k) || at.x > rect.right() - 24.0 * k {
                continue;
            }
            let lr = canvas::putf(
                b,
                at,
                Align2::LEFT_BOTTOM,
                Role::Micro,
                ink.dim,
                tk,
                None,
                format_args!("{ms:.0}"),
            );
            ruler.placed(lr);
        }
        let head = Rect::from_min_max(
            rect.min + vec2(8.0 * k, 2.0 * k),
            pos2(rect.right() - 8.0 * k, rect.top() + 20.0 * k),
        );
        let t = Text::new(line, Role::Body, ink.danger)
            .caps(false)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = ktext::paint(b, head, &t, tk);
    });
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("comp.response"),
            frame::tr("comp.response_tip"),
            Caps::NONE,
        );
    }
}
