//! The operator table on the kit (Operator's best idea, taken to eight rows): every
//! operator's wave, ratio or fixed frequency (a segmented switch and a value pill), detune,
//! level (a knob and its pill), velocity, output and feedback, each a value you drag, and
//! its envelope drawn; and the voice's own cards — the easy macros, the voice, the LFOs,
//! the envelopes of pitch and modulation, the quality.

use super::parts::{self, c, C};
use super::{bake, batch, EASY};
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::color::{ensure_contrast, mix, shade};
use crate::kit::knob::KnobSize;
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{with_brush, Brush, Fill, State, Style};
use cantus_core::inst::fm::{
    GUARDS, LFO_WAVES, MODES, NAMES, OPS, OP_KEYS, OUT_OFF, QUALITIES, WAVES,
};
use cantus_core::{NativeSpec, TrackId};
use egui::{Align2, Color32, Pos2, Rect, Sense, Vec2};
use std::fmt::Write as _;

/// A value pill: pale, its number in the value face, lit in the hue while hot, outlined in
/// a modulation source's colour when one reaches it.
pub(super) fn paint_pill(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    txt: &str,
    hot: bool,
    ring: Option<Color32>,
) {
    let fill = if hot {
        mix(s.t.value_pill, s.hue, 0.40)
    } else {
        s.t.value_pill
    };
    let ink = ensure_contrast(s.t.value_pill_text, fill, 4.5);
    let r = b.snap_rect(rect);
    b.rrect(
        r,
        s.r(3.0),
        Fill::V(
            shade(fill, 0.02 * s.t.depth),
            shade(fill, -0.02 * s.t.depth),
        ),
    );
    if let Some(c) = ring {
        b.rrect_stroke(r, s.r(3.0), s.s(1.5), c);
    }
    let _ = text::paint(
        b,
        r.shrink2(Vec2::new(s.s(3.0), 0.0)),
        &Text::new(txt, Role::Value, ink).policy(Policy::Elide(Elide::End)),
        s.tk(),
    );
}

/// A number in a value pill you drag: up or right to increase (logarithmic keys move by
/// ratios, stepped ones a step per 8 pt), Shift for fine, double-click for the default. A
/// drag is one undo step. `range` and `per_px` override the parameter's own travel.
#[allow(clippy::too_many_arguments)]
pub(super) fn pill_num(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    key: &'static str,
    rect: Rect,
    s: &Style,
    txt: &str,
    ring: Option<Color32>,
    range: Option<(f32, f32)>,
    per_px: Option<f32>,
) -> egui::Response {
    // A key can have a pill in the graph and another in the table: the place tells them
    // apart (whole points, so a drag does not change it).
    let place = (
        (rect.center().x / s.k) as i32,
        (rect.center().y / s.k) as i32,
    );
    let resp = ui.interact(
        rect,
        ui.id().with(("fm-pill", key, place)),
        Sense::click_and_drag(),
    );
    let Some(info) = spec.kind.param(key) else {
        return resp;
    };
    let (lo, hi) = range.unwrap_or((info.min, info.max));
    if resp.dragged() {
        let fine = if ui.input(|i| i.modifiers.shift) {
            0.2
        } else {
            1.0
        };
        let d = resp.drag_delta();
        let px = (d.x - d.y) * fine / s.k;
        let v = spec.get(key);
        let nv = if let Some(p) = per_px {
            v + px * p
        } else if info.logarithmic {
            v * (px * (info.max / info.min).log2() / 400.0).exp2()
        } else if info.stepped {
            let steps = ui.data_mut(|m| {
                let a = m.get_temp_mut_or_default::<f32>(resp.id);
                *a += px;
                let whole = (*a / 8.0).trunc();
                *a -= whole * 8.0;
                whole
            });
            v + steps
        } else {
            v + px * (info.max - info.min) / 400.0
        };
        let nv = nv.clamp(lo, hi);
        if nv != v {
            app.set_inst_value(track, key, nv, true);
        }
    }
    if resp.drag_stopped() {
        app.end_gesture();
    }
    if resp.double_clicked() {
        batch(
            app,
            track,
            spec,
            "cmd.set_instrument_value",
            &[(key, info.default)],
        );
    }
    let hot = resp.hovered() || resp.dragged();
    if hot {
        crate::kit::hint::set_text(
            ui.ctx(),
            &t(info.label_key),
            txt,
            crate::kit::hint::Caps::PARAM,
        );
    }
    if let Some(src) = resp.dnd_release_payload::<&'static str>() {
        app.connect_modulation(track, *src, key);
    }
    with_brush(ui.painter(), |b| paint_pill(b, rect, s, txt, hot, ring));
    resp
}

/// A small drawing of an operator's envelope on a display ground.
pub(super) fn env_thumb(
    b: &mut Brush,
    spec: &NativeSpec,
    o: usize,
    rect: Rect,
    s: &Style,
    hot: bool,
) {
    b.rrect(rect, s.r(2.0), Fill::V(s.t.ink, shade(s.t.ink, -0.02)));
    if hot {
        b.rrect_stroke(rect, s.r(2.0), s.s(1.0), s.hue);
    }
    let pts = super::env::shape(spec, o, rect.shrink(s.s(3.0)));
    b.polyline(&pts, s.s(1.2), s.hue);
}

/// The table's columns: legend key and relative width.
const COLS: [(&str, f32); 10] = [
    ("fm.col.op", 34.0),
    ("fm.col.wave", 44.0),
    ("fm.col.mode", 46.0),
    ("fm.col.freq", 70.0),
    ("fm.col.detune", 58.0),
    ("fm.col.level", 84.0),
    ("fm.col.vel", 46.0),
    ("fm.col.out", 60.0),
    ("fm.col.fb", 46.0),
    ("fm.col.env", 60.0),
];

/// The operator table: a legend row, then eight rows of kit parts.
pub(super) fn table_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
) {
    let cr = parts::card(ui, rect, s, Src::Key("fm.ops_title"), None, None, None);
    let body = Rect::from_min_max(
        cr.body.min - Vec2::new(0.0, s.s(2.0)),
        cr.body.max + Vec2::new(0.0, s.s(4.0)),
    );
    let gap = s.s(4.0);
    let total: f32 = COLS.iter().map(|c| c.1).sum();
    let unit = (body.width() - gap * (COLS.len() - 1) as f32) / total;
    let mut xs = [0.0f32; 11];
    let mut x = body.left();
    for (i, (_, w)) in COLS.iter().enumerate() {
        xs[i] = x;
        x += w * unit + gap;
    }
    xs[10] = body.right() + gap;
    let col = |i: usize, top: f32, h: f32| {
        Rect::from_min_max(Pos2::new(xs[i], top), Pos2::new(xs[i + 1] - gap, top + h))
    };
    let head_h = s.s(14.0);
    with_brush(ui.painter(), |b| {
        for (i, (key, _)) in COLS.iter().enumerate() {
            let _ = text::paint(
                b,
                col(i, body.top(), head_h),
                &Text::key(key, Role::Micro, s.t.text_dim)
                    .caps(true)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Abbreviate),
                s.tk(),
            );
        }
    });
    let rows_top = body.top() + head_h + s.s(2.0);
    let row_h = ((body.bottom() - rows_top) / OPS as f32).min(s.s(28.0));
    let routed: Vec<(String, String)> = cantus_core::inst::modulation::connections(spec)
        .filter(|c| c.on)
        .map(|c| (c.dst.to_string(), c.src.to_string()))
        .collect();
    let ring = |key: &str| {
        routed
            .iter()
            .find(|(d, _)| d == key)
            .map(|(_, src)| crate::views::inst::source_color(src))
    };
    for o in 0..OPS {
        let k = &OP_KEYS[o];
        let top = rows_top + row_h * o as f32;
        let ch = row_h - s.s(4.0);
        let cy = top + (row_h - ch) * 0.5;
        let heard = spec.get(k.out) > OUT_OFF;
        let on = spec.get(k.on) >= 0.5;
        let sel = app.inst.fm.selected == o;
        // The tile: a lamp (on), the letter (selects); filled when heard.
        let tile = col(0, cy, ch);
        let tresp = ui.interact(tile, ui.id().with(("fm-tile", o)), Sense::click());
        let lamp = Rect::from_center_size(
            Pos2::new(tile.left() + s.s(7.0), tile.center().y),
            Vec2::splat(s.s(10.0)),
        );
        // The lamp is the operator's On, a parameter: a control of its own over the tile —
        // its menu, its mark, the learn mode (round 29, review 2 of ADR 0062, #44).
        let lamp_hit = Rect::from_min_max(tile.min, Pos2::new(lamp.right() + s.s(2.0), tile.max.y));
        let lresp = ui.interact(lamp_hit, ui.id().with(("fm-tile-lamp", o)), Sense::click());
        let learning = crate::controllers::stepped_inst(app, ui, track, spec, k.on, &lresp);
        if lresp.clicked() && !learning {
            app.set_inst_value(track, k.on, if on { 0.0 } else { 1.0 }, false);
        }
        if tresp.clicked() {
            app.inst.fm.selected = o;
        }
        if tresp.hovered() {
            crate::kit::hint::set_text(
                ui.ctx(),
                &crate::i18n::tf("fm.node.tip", &[("op", &NAMES[o])]),
                "",
                crate::kit::hint::Caps::NONE,
            );
        }
        with_brush(ui.painter(), |b| {
            let face = if heard {
                mix(s.t.deck_title, s.hue, 0.30)
            } else {
                s.t.deck_title
            };
            b.rrect(tile, s.r(3.0), Fill::Solid(face));
            if sel {
                b.rrect_stroke(tile, s.r(3.0), s.s(1.5), s.hue);
            }
            crate::kit::band::lamp(b, lamp.center(), s, on, s.hue, 1.0);
            parts::say(
                b,
                Rect::from_min_max(Pos2::new(lamp.right() + s.s(2.0), tile.top()), tile.max),
                s,
                NAMES[o],
                Role::Section,
                if on { s.t.text } else { s.t.text_dim },
                Align2::CENTER_CENTER,
            );
        });
        // Wave: a click steps through, the wheel too.
        let wr = col(1, cy, ch);
        let w = spec.get(k.wave).round() as usize;
        let wresp = ui.interact(wr, ui.id().with(("fm-wave", o)), Sense::click());
        let wave_label = t(&format!("{}_short", WAVES[w.min(3)]));
        with_brush(ui.painter(), |b| {
            crate::kit::well::readout(b, wr, s, State::of(&wresp, false), &wave_label, false)
        });
        if wresp.hovered() {
            crate::kit::hint::set_text(
                ui.ctx(),
                &t("fm.op.wave"),
                &t(WAVES[w.min(3)]),
                crate::kit::hint::Caps::NONE,
            );
        }
        // The wave's readout is its parameter's control (review 2 of ADR 0062, #44).
        let learning = crate::controllers::stepped_inst(app, ui, track, spec, k.wave, &wresp);
        if wresp.clicked() && !learning {
            app.set_inst_value(track, k.wave, ((w + 1) % 4) as f32, false);
        }
        // Ratio or fixed: a segmented switch, then the number.
        let fixed = spec.get(k.freq_mode) >= 0.5;
        let modes = [
            Src::Key("fm.freq.ratio_short"),
            Src::Key("fm.freq.fixed_short"),
        ];
        if let Some(i) = parts::seg_param(
            app,
            ui,
            ui.id().with(("fm-mode", o)),
            col(2, cy, ch),
            s,
            &modes,
            fixed as usize,
            &t("fm.op.freq_mode"),
            track,
            spec,
            k.freq_mode,
        ) {
            app.set_inst_value(track, k.freq_mode, i as f32, false);
        }
        let fr = col(3, cy, ch);
        if fixed {
            let mut b = String::new();
            crate::views::inst::write_readout(spec.get(k.fixed), "Hz", &mut b);
            pill_num(
                app,
                ui,
                track,
                spec,
                k.fixed,
                fr,
                s,
                &b,
                ring(k.fixed),
                None,
                None,
            );
        } else {
            let txt = format!("\u{00d7}{:.3}", spec.get(k.ratio));
            pill_num(
                app,
                ui,
                track,
                spec,
                k.ratio,
                fr,
                s,
                &txt,
                ring(k.ratio),
                None,
                None,
            );
        }
        let txt = format!("{:+.1} ct", spec.get(k.detune));
        pill_num(
            app,
            ui,
            track,
            spec,
            k.detune,
            col(4, cy, ch),
            s,
            &txt,
            ring(k.detune),
            None,
            None,
        );
        // Level: a knob and its pill.
        let lr = col(5, cy, ch);
        let kd = (KnobSize::Mini.diameter() + 6.0) * s.k;
        parts::knob_at(
            app,
            ui,
            track,
            spec,
            c(k.level),
            s,
            KnobSize::Mini,
            Pos2::new(lr.left() - s.s(3.0), lr.center().y - kd * 0.5),
        );
        let mut lt = String::new();
        super::super::wavetable::db_text(spec.get(k.level), &mut lt);
        let lp = Rect::from_min_max(Pos2::new(lr.left() + kd - s.s(4.0), lr.top()), lr.max);
        pill_num(
            app,
            ui,
            track,
            spec,
            k.level,
            lp,
            s,
            &lt,
            ring(k.level),
            None,
            None,
        );
        let txt = format!("{:.0} %", spec.get(k.vel) * 100.0);
        pill_num(
            app,
            ui,
            track,
            spec,
            k.vel,
            col(6, cy, ch),
            s,
            &txt,
            ring(k.vel),
            None,
            None,
        );
        let out = spec.get(k.out);
        let out_txt = if out <= OUT_OFF {
            t("fm.off")
        } else {
            let mut b = String::new();
            super::super::wavetable::db_text(out, &mut b);
            b
        };
        pill_num(
            app,
            ui,
            track,
            spec,
            k.out,
            col(7, cy, ch),
            s,
            &out_txt,
            ring(k.out),
            None,
            None,
        );
        let txt = format!("{:.2}", spec.get(k.fb));
        pill_num(
            app,
            ui,
            track,
            spec,
            k.fb,
            col(8, cy, ch),
            s,
            &txt,
            ring(k.fb),
            None,
            None,
        );
        // The envelope, drawn: a click opens its page.
        let er = col(9, cy, ch);
        let eresp = ui.interact(er, ui.id().with(("fm-env-thumb", o)), Sense::click());
        with_brush(ui.painter(), |b| {
            env_thumb(b, spec, o, er, s, eresp.hovered())
        });
        if eresp.clicked() {
            app.inst.fm.selected = o;
            app.inst.fm.detail = super::Detail::Envelope;
        }
    }
}

// ---- the voice's cards ---------------------------------------------------------------------

/// A segmented switch at the right of a card's title bar.
/// The play mode (Poly · Mono · Legato) as a segmented switch at the right of a voice
/// card's title bar.
fn mode_switch(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    bar: Rect,
) {
    let labels = mode_labels();
    let h = s.s(18.0);
    let w = s.s(58.0) * labels.len() as f32;
    let r = Rect::from_min_size(
        Pos2::new(bar.right() - s.s(6.0) - w, bar.center().y - h * 0.5),
        Vec2::new(w, h),
    );
    parts::seg_key(app, ui, track, spec, "mode", r, s, &labels);
}

/// The easy macros — offsets on the whole patch — and Bake, which writes them in.
pub(super) fn easy_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
    size: KnobSize,
) {
    let cr = parts::card(ui, rect, s, Src::Key("fm.easy"), None, None, None);
    let moved = EASY.iter().any(|k| spec.get(k) != 0.0);
    let bw = s.s(76.0);
    let br = Rect::from_min_size(
        Pos2::new(
            cr.title.right() - s.s(6.0) - bw,
            cr.title.center().y - s.s(9.0),
        ),
        Vec2::new(bw, s.s(18.0)),
    );
    if parts::action(
        ui,
        ui.id().with("fm-bake"),
        br,
        s,
        Src::Key("fm.bake"),
        &t("fm.bake.tip"),
        moved,
    ) {
        bake(app, track, spec);
    }
    let list: Vec<C<'_>> = EASY.iter().map(|k| c(k)).collect();
    let size = if parts::row_width(list.len(), size, s) <= cr.body.width() {
        size
    } else {
        KnobSize::S
    };
    parts::cells_spread(app, ui, track, spec, s, size, cr.body, &list);
}

fn mode_labels() -> [Src<'static>; 3] {
    [Src::Key(MODES[0]), Src::Key(MODES[1]), Src::Key(MODES[2])]
}

/// The voice (Edit): play mode in the title bar, and every number of the voice.
pub(super) fn voice_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
) {
    let cr = parts::card(ui, rect, s, Src::Key("fm.sec.voice"), None, None, None);
    mode_switch(app, ui, track, spec, s, cr.title);
    let count = |v: f32, b: &mut String| {
        let _ = write!(b, "{}", v.round() as i64);
    };
    parts::cells_spread(
        app,
        ui,
        track,
        spec,
        s,
        KnobSize::S,
        cr.body,
        &[
            c("voices").fmt(&count),
            c("glide"),
            c("unison").fmt(&count),
            c("uni.detune"),
            c("uni.spread"),
            c("transpose"),
            c("tune"),
            c("pb_range"),
            c("drift"),
        ],
    );
}

/// The voice (Play): play mode, glide, transpose, tune and the output gain.
pub(super) fn play_voice_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
) {
    let cr = parts::card(ui, rect, s, Src::Key("fm.sec.voice"), None, None, None);
    mode_switch(app, ui, track, spec, s, cr.title);
    parts::cells_spread(
        app,
        ui,
        track,
        spec,
        s,
        KnobSize::M,
        cr.body,
        &[
            c("glide"),
            c("transpose"),
            c("tune"),
            c("gain")
                .fmt(&super::super::wavetable::db_text)
                .label(Src::Key("fm.gain")),
        ],
    );
}

/// A LFO: its wave in the title's selector, its numbers, key sync.
pub(super) fn lfo_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
    l: usize,
) {
    let p = if l == 0 { "lfo1" } else { "lfo2" };
    let key = |f: &str| format!("{p}.{f}");
    let wave_key = key("wave");
    let wi = spec.get(&wave_key).round().max(0.0) as usize;
    let waves: Vec<String> = LFO_WAVES.iter().map(|k| t(k)).collect();
    let title = if l == 0 { "fm.sec.lfo1" } else { "fm.sec.lfo2" };
    let cr = parts::card(
        ui,
        rect,
        s,
        Src::Key(title),
        None,
        waves.get(wi).map(String::as_str),
        None,
    );
    let id = ui.id().with(("fm-lfo-wave", l));
    let pick = parts::selector_key(app, ui, id, &cr, &t("fm.lfo.wave"), track, spec, &wave_key);
    if let Some(i) = parts::step(wi, waves.len(), pick) {
        app.set_inst_value(track, &wave_key, i as f32, false);
    }
    if pick.open {
        parts::toggle_popup(ui, id.with("menu"));
    }
    if let Some(sr) = cr.selector {
        if let Some(i) = parts::popup(ui, id.with("menu"), sr, s, &parts::radio_items(&waves, wi)) {
            app.set_inst_value(track, &wave_key, i as f32, false);
        }
    }
    let fields: &[&str] = if l == 0 {
        &["rate", "delay", "pitch", "amp"]
    } else {
        &["rate", "delay"]
    };
    let keys: Vec<String> = fields.iter().map(|f| key(f)).collect();
    let list: Vec<C<'_>> = keys.iter().map(|k| c(k)).collect();
    let body = cr.body;
    let lw = s.s(64.0);
    let cells_r = Rect::from_min_max(
        body.min,
        Pos2::new(body.right() - lw - s.s(6.0), body.bottom()),
    );
    parts::cells_spread(app, ui, track, spec, s, KnobSize::S, cells_r, &list);
    let lr = Rect::from_min_size(
        Pos2::new(body.right() - lw, body.center().y - s.s(10.0)),
        Vec2::new(lw, s.s(20.0)),
    );
    parts::led_key(app, ui, track, spec, &key("sync"), lr, s);
}

/// The pitch envelope's seven numbers.
pub(super) fn penv_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
) {
    let cr = parts::card(ui, rect, s, Src::Key("fm.sec.penv"), None, None, None);
    let keys = [
        "penv.l0", "penv.t1", "penv.l1", "penv.t2", "penv.l2", "penv.t3", "penv.l3",
    ];
    let list: Vec<C<'_>> = keys.iter().map(|k| c(k)).collect();
    parts::cells_spread(app, ui, track, spec, s, KnobSize::S, cr.body, &list);
}

/// The modulation envelope's four stages.
pub(super) fn menv_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
) {
    let cr = parts::card(ui, rect, s, Src::Key("fm.sec.menv"), None, None, None);
    parts::cells_spread(
        app,
        ui,
        track,
        spec,
        s,
        KnobSize::S,
        cr.body,
        &[c("menv.a"), c("menv.d"), c("menv.s"), c("menv.r")],
    );
}

/// Quality: the oversampling, the alias guard, the anti-hunt average and the gain.
pub(super) fn quality_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
) {
    let cr = parts::card(ui, rect, s, Src::Key("fm.sec.quality"), None, None, None);
    let body = cr.body;
    let cw = parts::cell_size(KnobSize::S, s).x;
    parts::cells(
        app,
        ui,
        track,
        spec,
        s,
        KnobSize::S,
        Rect::from_min_size(body.min, Vec2::new(cw, body.height())),
        &[c("gain")
            .fmt(&super::super::wavetable::db_s)
            .label(Src::Key("fm.gain"))],
    );
    let col = Rect::from_min_max(
        Pos2::new(body.left() + cw + s.s(10.0), body.top()),
        body.max,
    );
    let fh = s.s(20.0);
    let lw = s.s(52.0);
    let y0 = col.center().y - (fh * 3.0 + s.s(8.0)) * 0.5;
    let row = |i: usize| {
        Rect::from_min_size(
            Pos2::new(col.left(), y0 + i as f32 * (fh + s.s(4.0))),
            Vec2::new(col.width(), fh),
        )
    };
    let field = |r: Rect| Rect::from_min_max(Pos2::new(r.left() + lw, r.top()), r.max);
    with_brush(ui.painter(), |b| {
        parts::row_label(
            b,
            Rect::from_min_size(row(0).min, Vec2::new(lw - s.s(6.0), fh)),
            s,
            "fm.quality",
        );
        parts::row_label(
            b,
            Rect::from_min_size(row(1).min, Vec2::new(lw - s.s(6.0), fh)),
            s,
            "fm.guard",
        );
    });
    let q: Vec<String> = QUALITIES.iter().map(|k| t(k)).collect();
    parts::choice_key(app, ui, track, spec, "quality", field(row(0)), s, &q);
    let g = [
        Src::Key(GUARDS[0]),
        Src::Key(GUARDS[1]),
        Src::Key(GUARDS[2]),
    ];
    parts::seg_key(app, ui, track, spec, "guard", field(row(1)), s, &g);
    parts::led_key(app, ui, track, spec, "fb_avg", field(row(2)), s);
}
