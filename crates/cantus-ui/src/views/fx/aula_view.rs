//! Aula's picture: the response's outline in decibels over time, after the pre-delay;
//! the inspector's small strip of it; and the demo set-up the screenshot tool uses.

use super::super::canvas::{self, Ink};
use super::super::frame;
use super::super::host::{DawHost, FxHost};
use super::{State, IR_KEYS};
use crate::app::CantusApp;
use crate::kit::text::{Role, Text};
use crate::kit::{with_brush, Style};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use cantus_engine::fx::aula as unit;
use cantus_engine::fx::aula_store::{self, Outline};
use egui::{pos2, vec2, Align2, Rect, Sense};
use std::fmt::Write as _;

/// The floor of the picture, dB under the response's peak.
const FLOOR_DB: f32 = 72.0;

/// The locale key of a format's name, by its channel count.
pub fn format_key(channels: usize) -> &'static str {
    match channels {
        1 => "aula.format.1",
        2 => "aula.format.2",
        4 => "aula.format.4",
        _ => "aula.format.0",
    }
}

/// The outline for `spec`, from the editor's state when it was drawn for the same source
/// and shape.
fn outline_of(st: &mut State, spec: &EffectSpec) -> Outline {
    let want = unit::want(spec);
    match &st.outline {
        Some((k, o)) if *k == want => o.clone(),
        _ => {
            let o = aula_store::outline(want.0, want.1);
            st.outline = Some((want, o.clone()));
            o
        }
    }
}

/// A round span of seconds that holds `s`.
fn axis(s: f32) -> f32 {
    [
        0.25f32, 0.5, 1.0, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 16.0, 24.0, 32.0,
    ]
    .into_iter()
    .find(|a| *a >= s)
    .unwrap_or(32.0)
}

/// The response's name as the source gives it.
fn name_of(spec: &EffectSpec) -> String {
    match &spec.ir {
        Some(f) => f.name().to_string(),
        None => {
            let i = (spec.get("ir").round().max(0.0) as usize).min(IR_KEYS.len() - 1);
            frame::tr(IR_KEYS[i]).to_string()
        }
    }
}

pub fn display(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    d: Rect,
    tel: &[f32; 64],
) {
    let spec = host.spec().clone();
    let o = outline_of(st, &spec);
    let k = s.k;
    let ink = Ink::of(s);
    let pre = spec.get("predelay") / 1000.0;
    let span = axis(pre + o.seconds.max(0.05));
    let plot = Rect::from_min_max(
        d.min + vec2(10.0 * k, 26.0 * k),
        d.max - vec2(10.0 * k, 18.0 * k),
    );
    let x = |t: f32| plot.left() + plot.width() * (t / span).clamp(0.0, 1.0);
    let y = |db: f32| plot.top() + plot.height() * (-db).clamp(0.0, FLOOR_DB) / FLOOR_DB;
    let words_key = (
        unit::want(&spec).0,
        o.seconds.to_bits(),
        o.paths,
        crate::i18n::locale(),
    );
    if st.words.as_ref().is_none_or(|w| w.0 != words_key) {
        let mut w = name_of(&spec);
        if o.found {
            let _ = write!(
                w,
                " \u{b7} {} \u{b7} {:.2} s",
                frame::tr(format_key(o.paths)),
                o.seconds
            );
        }
        st.words = Some((words_key, w));
    }
    let words = st.words.as_ref().map_or("", |w| w.1.as_str());
    let peak = o.peaks.iter().fold(0.0f32, |m, v| m.max(*v)).max(1e-12);
    with_brush(&ui.painter_at(d), |b| {
        canvas::ground_b(b, d, &ink, s.hue_tok);
        for db in [24.0f32, 48.0] {
            b.hline(
                plot.left(),
                plot.right(),
                b.snap_mid(y(-db)),
                b.px(),
                ink.grid,
            );
        }
        // The seconds under the plot, at quarters of the span.
        for q in 0..=4 {
            let t = span * q as f32 / 4.0;
            let xx = x(t);
            b.segment(
                pos2(xx, plot.bottom()),
                pos2(xx, plot.bottom() + 3.0 * k),
                b.px(),
                ink.grid_strong,
                false,
            );
            let mut label = String::new();
            super::super::controls::write_readout(t, "s", &mut label);
            let t = Text::new(&label, Role::Micro, ink.dim).caps(false);
            let align = match q {
                0 => Align2::LEFT_TOP,
                4 => Align2::RIGHT_TOP,
                _ => Align2::CENTER_TOP,
            };
            canvas::put(b, pos2(xx, plot.bottom() + 4.0 * k), align, &t, k, None);
        }
        // The pre-delay: nothing is heard yet.
        if pre > 0.0 {
            let r = Rect::from_min_max(plot.left_top(), pos2(x(pre), plot.bottom()));
            canvas::hatch(b, r, 6.0 * k, b.px(), ink.grid);
            if r.width() > 60.0 * k {
                let t = Text::key("aula.predelay_mark", Role::Micro, ink.dim).caps(true);
                canvas::put(b, r.center(), Align2::CENTER_CENTER, &t, k, Some(ink.plate));
            }
        }
        // The outline: a column per point from the floor to its level, and its edge.
        if o.found && o.seconds > 0.0 {
            let n = o.peaks.len().max(1);
            let mut pts = Vec::with_capacity(n);
            for (i, p) in o.peaks.iter().enumerate() {
                let t = pre + o.seconds * (i as f32 + 0.5) / n as f32;
                let db = 20.0 * cantus_dsp::math::log10((p / peak).max(1e-9));
                let top = pos2(x(t), y(db));
                b.segment(
                    pos2(top.x, plot.bottom()),
                    top,
                    (plot.width() / n as f32).max(b.px()),
                    ink.faint,
                    false,
                );
                pts.push(top);
            }
            canvas::trace(b, &pts, 1.5 * k, ink.hue, &ink);
        }
        let t = Text::new(words, Role::Value, ink.text);
        canvas::put(
            b,
            d.min + vec2(8.0 * k, 5.0 * k),
            Align2::LEFT_TOP,
            &t,
            k,
            Some(ink.plate),
        );
        // While the engine builds the kernel the controls asked for.
        let alive = tel[cantus_dsp::aula::T_PATHS] > 0.0;
        if alive && tel[unit::T_READY] == 0.0 {
            let t = Text::key("aula.loading", Role::Micro, ink.text).caps(false);
            canvas::put(
                b,
                d.right_top() + vec2(-8.0 * k, 6.0 * k),
                Align2::RIGHT_TOP,
                &t,
                k,
                Some(ink.plate),
            );
        }
        canvas::edge_b(b, d, &ink);
    });
    if !o.found {
        super::super::modfx::idle_note(ui, s, d, "aula.drop");
    } else if spec.get("mix") == 0.0 {
        super::super::modfx::idle_note(ui, s, d, "aula.idle");
    }
}

thread_local! {
    static PREVIEW: std::cell::RefCell<Option<(EffectSpec, Outline, String)>> =
        const { std::cell::RefCell::new(None) };
}

/// The inspector's picture: the outline and a line of words (name, length, mix).
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let st = Style::current().function(frame::hue_of(EffectKind::Aula));
    let ink = Ink::of(&st);
    let k = st.k;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 56.0 * k), Sense::click());
    PREVIEW.with(|cell| {
        let mut c = cell.borrow_mut();
        if c.as_ref().is_none_or(|p| p.0 != *spec) {
            let (src, shape) = unit::want(spec);
            let o = aula_store::outline(src, shape);
            let words = format!(
                "{} \u{b7} {:.1} s \u{b7} {:.0} %",
                name_of(spec),
                o.seconds + spec.get("predelay") / 1000.0,
                spec.get("mix") * 100.0
            );
            *c = Some((spec.clone(), o, words));
        }
        let Some((_, o, words)) = c.as_ref() else {
            return;
        };
        let plot = rect.shrink2(vec2(6.0 * k, 6.0 * k));
        let peak = o.peaks.iter().fold(0.0f32, |m, v| m.max(*v)).max(1e-12);
        with_brush(&ui.painter_at(rect), |b| {
            canvas::ground_b(b, rect, &ink, st.hue_tok);
            let n = o.peaks.len().max(1);
            let pts: Vec<_> = o
                .peaks
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let db = 20.0 * cantus_dsp::math::log10((p / peak).max(1e-9));
                    pos2(
                        plot.left() + plot.width() * i as f32 / n as f32,
                        plot.top() + plot.height() * (-db).clamp(0.0, FLOOR_DB) / FLOOR_DB,
                    )
                })
                .collect();
            if o.found {
                canvas::trace(b, &pts, 1.2 * k, ink.hue, &ink);
            }
            let t = Text::new(words, Role::Label, ink.text);
            canvas::put(
                b,
                rect.min + vec2(6.0 * k, 4.0 * k),
                Align2::LEFT_TOP,
                &t,
                k,
                Some(ink.plate),
            );
            canvas::edge_b(b, rect, &ink);
        });
    });
    resp
}

/// The settings `--editor aula` and the screenshot tool open it with: Cantus's large hall,
/// a little late, its highs damped, a third wet.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "aula").set_many(
        &[
            ("ir", 8.0),
            ("predelay", 35.0),
            ("damp", 0.35),
            ("low", 90.0),
            ("mix", 0.3),
        ],
        "cmd.fx_preset",
    );
}

/// What a playing engine would report, for screenshots with no engine: the response
/// asked for is the one playing.
pub fn demo_feed(app: &mut CantusApp) {
    let Some((strip, effect, _)) = app.fx.open else {
        return;
    };
    let Some(slot) = app.project().effect(strip, effect).cloned() else {
        return;
    };
    use cantus_engine::FxUnit;
    let mut u = unit::Unit::new(48_000.0);
    u.configure(&slot.spec);
    if let Some(need) = u.memory_need() {
        let _ = u.supply(cantus_engine::EffectMemory::build(need, 48_000.0));
    }
    let n = 24_000;
    let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
    l[0] = 0.8;
    r[0] = 0.8;
    u.process(&mut l, &mut r, None);
    app.fx.injected = Some(u.telemetry());
}
