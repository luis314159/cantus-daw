//! Culmen's loudness card — every reading at once, the target, the verdict, *Learn* — and
//! the menu fields of the brickwall and the output.

use super::display::{pair, verdict, Lu};
use super::{learn, learn_words, target_of, State, TARGETS};
use crate::i18n::{t, tf};
use crate::kit::hint::{self, Caps};
use crate::kit::menu::Item;
use crate::kit::text::{self as ktext, Elide, Policy, Role, Src, Text};
use crate::kit::{with_brush, Fill, Style};
use crate::views::fx::canvas::{self, Ink};
use crate::views::fx::controls;
use crate::views::fx::frame;
use crate::views::fx::host::FxHost;
use cantus_dsp::culmen::MaxSettings;
use egui::{pos2, vec2, Align2, Rect, Sense};

/// The verdict's words, per (integrated, true peak, target, ceiling) and language.
type VerdictKey = (Option<i64>, i32, u32, u32);

/// Words formatted once per change of what they say.
#[derive(Debug, Default)]
pub struct Words {
    verdict: Option<(VerdictKey, crate::i18n::Locale, bool, String)>,
    target: Option<(u32, crate::i18n::Locale, String)>,
}

/// One row of the card, `h` points tall, the whole width.
fn row(ui: &mut egui::Ui, s: &Style, w: f32, h: f32) -> (Rect, egui::Response) {
    ui.allocate_exact_size(vec2(w * s.k, h * s.k), Sense::hover())
}

/// The loudness since the last reset, against the target; the target; *Learn*.
pub fn loudness(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    ms: &MaxSettings,
) {
    let k = s.k;
    let w = body.width() / k;
    let target = host.spec().get("target");
    let ceiling = ms.limiter.ceiling_db;
    let integrated = st.integrated();
    let lang = crate::i18n::locale();
    let vkey: VerdictKey = (
        integrated.map(|v| (v * 20.0).round() as i64),
        (st.max_tp.max(-1_000.0) * 200.0).round() as i32,
        target.to_bits(),
        ceiling.to_bits(),
    );
    if st
        .words
        .verdict
        .as_ref()
        .is_none_or(|v| v.0 != vkey || v.1 != lang)
    {
        let (pass, text) = verdict(integrated, st.max_tp, target, ceiling);
        st.words.verdict = Some((vkey, lang, pass, text));
    }
    let preset = target_of(target, ceiling);
    let tkey = preset.map_or(u32::MAX, |i| i as u32) ^ target.to_bits();
    if st
        .words
        .target
        .as_ref()
        .is_none_or(|v| v.0 != tkey || v.1 != lang)
    {
        let name = match preset {
            Some(i) => t(TARGETS[i].key),
            None => format!("{} {target:.1}", t("max.target.custom")),
        };
        st.words.target = Some((tkey, lang, name));
    }
    let ink = Ink::of(s);
    let plate = s.t.text;
    controls::column(ui, s, w, |ui| {
        ui.spacing_mut().item_spacing.y = s.d(3.0);
        // The integrated loudness: the device's one hero number.
        let (r, resp) = row(ui, s, w, 28.0);
        with_brush(ui.painter(), |b| {
            pair(
                b,
                s,
                r,
                Src::Key("max.integrated"),
                Role::Hero,
                plate,
                format_args!("{} LUFS", Lu(integrated)),
            );
        });
        if resp.hovered() {
            hint::set_text(ui.ctx(), frame::tr("max.integrated"), "", Caps::NONE);
        }
        // The verdict: a filled disc when it passes, a hollow triangle when not.
        let (r, _) = row(ui, s, w, 18.0);
        if let Some((_, _, pass, text)) = &st.words.verdict {
            with_brush(ui.painter(), |b| {
                let c = pos2(r.left() + 6.0 * k, r.center().y);
                if *pass {
                    b.circle(c, 4.5 * k, Fill::Solid(ink.ok));
                } else {
                    let pts = [
                        pos2(c.x, c.y - 5.0 * k),
                        pos2(c.x + 5.0 * k, c.y + 4.0 * k),
                        pos2(c.x - 5.0 * k, c.y + 4.0 * k),
                        pos2(c.x, c.y - 5.0 * k),
                    ];
                    b.polyline(&pts, 1.5 * k, ink.danger);
                }
                let tr = Rect::from_min_max(pos2(r.left() + 16.0 * k, r.top()), r.max);
                let t = Text::new(text, Role::Body, s.t.text)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::End));
                let _ = ktext::paint(b, tr, &t, s.tk());
            });
        }
        // Three rows of two readings.
        let half = (w - 6.0) * 0.5;
        let m = st.loud.momentary;
        let sh = st.loud.short_term;
        let lra = st.loud.range();
        let tp = (st.max_tp > -99.0).then_some(st.max_tp as f64);
        let plr = match (tp, integrated) {
            (Some(p), Some(i)) => Some(p - i),
            _ => None,
        };
        let input = st.loud_in.integrated();
        let rows: [[(&str, Option<f64>, &str); 2]; 3] = [
            [("max.momentary", Some(m), ""), ("max.short", Some(sh), "")],
            [("max.range", lra, " LU"), ("max.tp_max", tp, " dB")],
            [("max.plr", plr, " dB"), ("max.input", input, "")],
        ];
        for pairs in rows {
            let (r, resp) = row(ui, s, w, 20.0);
            let left = Rect::from_min_size(r.min, vec2(half * k, r.height()));
            let right = Rect::from_min_max(pos2(r.right() - half * k, r.top()), r.max);
            with_brush(ui.painter(), |b| {
                for ((label, v, unit), rr) in pairs.iter().zip([left, right]) {
                    pair(
                        b,
                        s,
                        rr,
                        Src::Key(label),
                        Role::Value,
                        s.t.text,
                        format_args!("{}{unit}", Lu(*v)),
                    );
                }
            });
            if let Some(p) = resp.hover_pos() {
                let (label, _, _) = if p.x < right.left() {
                    pairs[0]
                } else {
                    pairs[1]
                };
                let tip = match label {
                    "max.plr" => frame::tr("max.plr_tip"),
                    "max.input" => frame::tr("max.input_tip"),
                    _ => "",
                };
                hint::set_text(ui.ctx(), frame::tr(label), tip, Caps::NONE);
            }
        }
        // The target: the usual deliveries, or the knob's own value.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = s.d(6.0);
            let name = st.words.target.as_ref().map_or("", |v| v.2.as_str());
            let id = ui.id().with("max-target");
            if let Some(at) = controls::field(ui, s, id, name, frame::tr("fx.max.target_tip"), half)
            {
                let items: Vec<Item<'_>> = TARGETS
                    .iter()
                    .enumerate()
                    .map(|(i, x)| frame::radio(Src::Key(x.key), preset == Some(i)))
                    .collect();
                if let Some(i) = controls::list(ui, s, id, at, half, &items) {
                    let x = TARGETS[i];
                    host.set_many(
                        &[("target", x.lufs), ("ceiling", x.ceiling)],
                        "cmd.max_target",
                    );
                }
            }
            drag_value(ui, s, host, "target", half);
        });
        // Learn, and reset.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = s.d(6.0);
            let learning = st.learn.is_some();
            let label = if learning {
                Src::Key("max.learn.stopped")
            } else {
                Src::Key("max.learn")
            };
            let r = controls::key(ui, s, label, frame::tr("max.learn_tip"), half, learning);
            if r.clicked() {
                learn::toggle(host, st);
            }
            if controls::key(
                ui,
                s,
                Src::Key("max.reset"),
                frame::tr("max.reset_tip"),
                half,
                false,
            )
            .clicked()
            {
                st.reset_loudness();
            }
        });
        // What learning says, or the target's rule.
        let (r, _) = row(ui, s, w, 18.0);
        let words = learn_words(st).or_else(|| preset.map(|i| t(TARGETS[i].rule)));
        if let Some(words) = words {
            with_brush(ui.painter(), |b| {
                let t = Text::new(&words, Role::Body, s.t.text_dim)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::End));
                let _ = ktext::paint(b, r, &t, s.tk());
            });
        }
        let _ = tf;
    });
}

/// A stepped parameter as a menu field: its value's words, a list of every choice.
fn choice_field(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    param: &'static str,
    words: &dyn Fn(usize) -> String,
    count: usize,
    width: f32,
) {
    let now = host.spec().get(param).round().max(0.0) as usize;
    let name = host
        .spec()
        .kind
        .param(param)
        .map_or("", |i| frame::tr(i.label_key));
    let id = ui.id().with(("max-field", param));
    let value = words(now);
    if let Some(at) = controls::field_key(ui, s, host, param, id, &value, name, width) {
        let labels: Vec<String> = (0..count).map(words).collect();
        let items: Vec<Item<'_>> = labels
            .iter()
            .enumerate()
            .map(|(i, l)| frame::radio(Src::Str(l), i == now))
            .collect();
        if let Some(i) = controls::list(ui, s, id, at, width, &items) {
            if i != now {
                host.set(param, i as f32, None);
            }
        }
    }
}

/// The brickwall's character: Terminus's styles, by name.
pub fn style_field(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, width: f32) {
    const KEYS: [&str; 5] = [
        "lim.style.0",
        "lim.style.1",
        "lim.style.2",
        "lim.style.3",
        "lim.style.4",
    ];
    choice_field(host, ui, s, "style", &|i| t(KEYS[i.min(4)]), 5, width);
}

/// Dither: off, or a word length.
pub fn dither_field(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, width: f32) {
    let words = |i: usize| match cantus_dsp::culmen::dither_bits(i) {
        0 => t("export.off"),
        bits => tf("export.bits", &[("n", &bits)]),
    };
    choice_field(host, ui, s, "dither", &words, 6, width);
}

/// Where the dither's noise goes.
pub fn shape_field(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, width: f32) {
    const KEYS: [&str; 3] = ["lim.shape.0", "lim.shape.1", "lim.shape.2"];
    choice_field(host, ui, s, "shape", &|i| t(KEYS[i.min(2)]), 3, width);
}

/// A value in a well that is dragged up and down (0.1 per point, Shift finer) and reset by
/// a double-click: for a number that has no room for a knob. A drag is one undo step.
fn drag_value(
    ui: &mut egui::Ui,
    s: &Style,
    host: &mut dyn FxHost,
    param: &'static str,
    width: f32,
) {
    let Some(info) = host.spec().kind.param(param) else {
        return;
    };
    let (rect, resp) = ui.allocate_exact_size(
        vec2(width * s.k, s.d(controls::ROW)),
        Sense::click_and_drag(),
    );
    let v = host.spec().get(param);
    if resp.dragged() {
        let fine = if ui.input(|i| i.modifiers.shift) {
            0.2
        } else {
            1.0
        };
        let dy = -resp.drag_delta().y / s.k * 0.1 * fine;
        let nv = ((v + dy) * 10.0).round() / 10.0;
        let nv = nv.clamp(info.min, info.max);
        if nv != v {
            host.set(param, nv, Some(param));
        }
    }
    if resp.drag_stopped() {
        host.end_gesture();
    }
    if resp.double_clicked() {
        host.set(param, info.default, None);
    }
    let st = crate::kit::State::of(&resp, false);
    with_brush(ui.painter(), |b| {
        crate::kit::well::paint(b, rect, s, st, None);
        canvas::line_in(
            b,
            rect.shrink2(vec2(6.0 * s.k, 0.0)),
            Role::Value,
            s.t.text,
            s.tk(),
            format_args!("{v:.1} {}", info.unit),
        );
    });
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        hint::set_text(
            ui.ctx(),
            frame::tr(info.label_key),
            frame::tr("fx.max.target_tip"),
            Caps::NONE,
        );
    }
}
