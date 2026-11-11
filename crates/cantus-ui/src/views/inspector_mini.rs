//! An insert's mini view (ADR 0024 §7): ▸ on a tile opens, under it, what the effect is
//! doing without opening its editor — its curve (Aequalis' response, Comprimo's transfer
//! curve) or its three main values — then its dry/wet and its sidechain. A hosted
//! plug-in shows its format, whether it loaded, and its latency.

use super::menus::MenuId;
use super::sheet::{self, Range, Sheet};
use super::Act;
use crate::app::CantusApp;
use crate::kit::color::{shade, with_alpha};
use crate::kit::{cache, Brush, Fill, Function, State, Style};
use crate::widgets::knob::Taper;
use cantus_core::{Command, EffectId, EffectKind, EffectSlot, MergeKey, StripId};
use egui::{pos2, Id, Pos2, Rect, Sense, Vec2};
use std::collections::HashMap;
use std::fmt::Write as _;

/// A curve's height at 100 %.
const CURVE_H: f32 = 56.0;
/// Points a mini curve is drawn with.
const N: usize = 64;

/// Each open insert's curve, and the project revision it was computed at.
#[derive(Default)]
pub struct Curves {
    map: HashMap<EffectId, (u64, [f32; N])>,
}

/// The kind of section an effect is, for its hue.
pub fn function_of(kind: EffectKind) -> Function {
    match kind {
        EffectKind::Eq => Function::Tone,
        EffectKind::Compressor
        | EffectKind::Limiter
        | EffectKind::Gate
        | EffectKind::DeEsser
        | EffectKind::Multiband => Function::Dynamics,
        EffectKind::Delay | EffectKind::Reverb | EffectKind::Space => Function::Space,
        EffectKind::Saturator => Function::Tone,
        EffectKind::Maximizer => Function::Dynamics,
        EffectKind::Stereo => Function::Space,
        EffectKind::External => Function::Source,
        EffectKind::Chorus
        | EffectKind::Flanger
        | EffectKind::Phaser
        | EffectKind::Tremolo
        | EffectKind::Vibrato
        | EffectKind::RingMod
        | EffectKind::FreqShift
        | EffectKind::VolumeShaper => Function::Mod,
        EffectKind::Clipper | EffectKind::Transient | EffectKind::TransientSplit => {
            Function::Dynamics
        }
        EffectKind::Analyzer => Function::Output,
        EffectKind::AutoTune => Function::Source,
        EffectKind::Aula => Function::Space, // r39:conv
    }
}

pub fn show(
    sh: &mut Sheet<'_>,
    curves: &mut Curves,
    app: &CantusApp,
    host: StripId,
    slot: &EffectSlot,
) {
    let s = sh.s;
    let e = slot.id;
    let kind = slot.spec.kind;
    let f = function_of(kind);
    sh.ui.add_space(s.d(2.0));
    let top = sh.ui.cursor().top();
    match kind {
        EffectKind::Compressor => {
            // des-mixer's mini view: Threshold, Ratio and the live Reduction as three
            // values on the row's panel.
            let params = kind.params();
            let find = |k: &str| params.iter().find(|p| p.key == k);
            columns(
                sh,
                app,
                host,
                slot,
                &[
                    find("thr").map(Col::Param).unwrap_or(Col::Empty),
                    find("ratio").map(Col::Param).unwrap_or(Col::Empty),
                    Col::Reduction,
                ],
            );
        }
        EffectKind::Eq => {
            let w = sh.width();
            let (r, resp) = sh
                .ui
                .allocate_exact_size(Vec2::new(w, s.s(CURVE_H)), Sense::click());
            let m = s.s(sheet::MARGIN);
            let rect =
                Rect::from_min_max(pos2(r.left() + m, r.top()), pos2(r.right() - m, r.bottom()));
            let rev = app.bus.revision();
            let entry = curves.map.entry(e).or_insert((u64::MAX, [0.0; N]));
            if entry.0 != rev {
                entry.0 = rev;
                compute(slot, &mut entry.1);
            }
            paint_curve(sh.b, rect, &s.function(f), kind, &entry.1);
            if resp.clicked() {
                sh.acts.push(Act::OpenEditor(host, e));
            }
            if resp.hovered() {
                let name = sh.fmt.tr("eq.open_tip");
                crate::kit::hint::set_text(sh.ui.ctx(), name, "", crate::kit::hint::Caps::NONE);
            }
            sh.ui.add_space(s.d(2.0));
        }
        EffectKind::External => hosted(sh, app, slot),
        _ => {
            // An effect with no editor of its own (Delay, Reverb) is edited only here:
            // its switches too (review of fix3: Delay's ping-pong was reachable nowhere).
            if !kind.has_editor() {
                for info in kind
                    .params()
                    .iter()
                    .filter(|p| p.key != "mix" && p.stepped && p.min == 0.0 && p.max == 1.0)
                {
                    let on = slot.spec.get(info.key) >= 0.5;
                    if sh.led_row_of(e, info.label_key, "fx.switch_on", on, f, false) {
                        sh.acts.push(Act::Exec(Command::SetEffectParam {
                            strip: host,
                            effect: e,
                            key: info.key.to_string(),
                            value: if on { 0.0 } else { 1.0 },
                        }));
                    }
                }
            }
            // des-mixer's mini view: its first three values on the row's panel; an effect
            // with no editor of its own (Delay, Reverb) is edited and modulated only
            // here, so every other parameter follows as a row (review 2: Reverb's
            // pre-delay had no row).
            let mut first = [Col::Empty; 3];
            for (c, info) in first.iter_mut().zip(
                kind.params()
                    .iter()
                    .filter(|p| p.key != "mix" && !p.stepped),
            ) {
                *c = Col::Param(info);
            }
            columns(sh, app, host, slot, &first);
            for info in kind
                .params()
                .iter()
                .filter(|p| p.key != "mix" && !p.stepped)
                .skip(3)
                .take(if kind.has_editor() { 0 } else { usize::MAX })
            {
                let range = Range {
                    min: info.min,
                    max: info.max,
                    default: info.default,
                    taper: if info.logarithmic {
                        Taper::Log
                    } else {
                        Taper::Linear
                    },
                    stepped: info.stepped,
                    bipolar: false,
                };
                let unit = info.unit;
                let key = info.key;
                // The effect's own parameter: automated and modulated from here too (an
                // effect with no editor of its own is edited only here, ADR 0057).
                super::modulators::param_row(
                    sh,
                    app,
                    info.label_key,
                    Id::new(("insp-fxp", e, key)),
                    slot.spec.get(key),
                    &range,
                    false,
                    |v, b| sheet::unit(v, unit, b),
                    f,
                    cantus_core::AutoTarget::Effect {
                        effect: e,
                        key: key.to_string(),
                    },
                    false,
                    |v| {
                        Act::Merge(
                            Command::SetEffectParam {
                                strip: host,
                                effect: e,
                                key: key.to_string(),
                                value: v,
                            },
                            MergeKey::of(&("fx", e.0, key)),
                        )
                    },
                );
            }
        }
    }
    let mix_row = sh.param_row(
        "effect.mix",
        Id::new(("insp-fx-mix", e)),
        slot.mix,
        &Range::lin(0.0, 1.0, 1.0),
        false,
        sheet::percent,
        f,
        false,
        |mix| {
            Act::Merge(
                Command::SetEffectMix {
                    strip: host,
                    effect: e,
                    mix,
                },
                MergeKey::of(&("fxmix", e.0)),
            )
        },
    );
    // The insert's mix is learnable here too (round 29, review 4 of ADR 0062, #21).
    let mix = crate::controllers::slot_learn_target(e, 1);
    let clip = sh.ui.clip_rect();
    if crate::controllers::learn_spot_view(app, sh.ui, mix_row.value, clip, &mix) {
        sh.acts.push(Act::Learn(mix));
    }
    // The key: another strip the detector listens to. Dimmed where the effect has none.
    let takes = slot.spec.takes_sidechain();
    let text = sh.fmt.get(
        cache::key(("insp-sc", e)),
        app.bus.revision() ^ ((crate::i18n::locale() as u64) << 60),
        |b| match slot.sidechain {
            Some(src) => super::strip_label(app, src, b),
            None => b.push_str(&crate::i18n::t("kit.inspector.none")),
        },
    );
    let text = sheet::name(text);
    let last = sh.menu_row(
        "inspector.sidechain",
        &text,
        MenuId::Sidechain(host, e),
        !takes,
    );
    let _ = (last, top);
    sh.ui.add_space(s.d(4.0));
}

/// A column of the mini view.
#[derive(Copy, Clone)]
enum Col<'p> {
    Empty,
    Param(&'p cantus_core::EffectParamInfo),
    /// A compressor's gain reduction now (the strip's meter).
    Reduction,
}

/// des-mixer's mini view (r39:lo-mixer): a 46-pt panel under the insert's row,
/// `mix(base, surface, .55)`, holding up to three values — the label Regular 10 ink 3, the
/// figure Regular 13 with its unit Regular 10 ink 3, a 3-pt bar under them; a compressor's
/// reduction in the solo colour with its bar from the right (0.0 in ink 3 at rest). A
/// value drags as any parameter (Shift for fine, a double-click resets) and is learnt,
/// automated and modulated from here as from its row.
fn columns(
    sh: &mut Sheet<'_>,
    app: &CantusApp,
    host: StripId,
    slot: &EffectSlot,
    cols: &[Col<'_>],
) {
    use crate::views::mixer::look::{self, Pal, W};
    let s = sh.s;
    let pal = Pal::of(&s);
    let w = sh.width();
    let (r, _) = sh
        .ui
        .allocate_exact_size(Vec2::new(w, s.s(48.0)), Sense::hover());
    let m = s.s(sheet::MARGIN);
    let panel = Rect::from_min_max(
        pos2(r.left() + m, r.top() - s.s(2.0)),
        pos2(r.right() - m, r.bottom() - s.s(2.0)),
    );
    sh.b.rrect(
        sh.b.snap_rect(panel),
        4.0,
        Fill::Solid(crate::kit::color::mix(pal.base, pal.surface, 0.55)),
    );
    let n = cols.len().max(1) as f32;
    let cw = (panel.width() - s.s(16.0)) / n;
    let e = slot.id;
    for (i, col) in cols.iter().enumerate() {
        let x = panel.left() + s.s(8.0) + cw * i as f32;
        let cell = Rect::from_min_max(
            pos2(x, panel.top()),
            pos2(x + cw - s.s(10.0), panel.bottom()),
        );
        let label_y = cell.top() + s.s(15.0);
        let value_y = cell.top() + s.s(32.0);
        let bar = Rect::from_min_max(
            pos2(cell.left(), cell.top() + s.s(38.0)),
            pos2(cell.right(), cell.top() + s.s(41.0)),
        );
        match *col {
            Col::Empty => {}
            Col::Reduction => {
                let gr = app.comp_gr_on_strip(host).flatten().unwrap_or(0.0).max(0.0);
                let live = gr > 0.05 && !slot.bypass;
                let label = sheet::name(sh.fmt.tr("inspector.reduction"));
                look::say(
                    sh.b,
                    &s,
                    &label,
                    cell.left(),
                    label_y,
                    egui::Align2::LEFT_BOTTOM,
                    cell.width(),
                    10.0,
                    W::Regular,
                    pal.ink3,
                );
                let text = sh.fmt.get(
                    cache::key(("insp-mini-gr", e)),
                    sheet::bits((gr * 10.0).round()) ^ u64::from(live),
                    |b| {
                        if live {
                            let _ = write!(b, "\u{2212}{gr:.1}");
                        } else {
                            b.push_str("0.0");
                        }
                    },
                );
                let vw = look::say(
                    sh.b,
                    &s,
                    text,
                    cell.left(),
                    value_y,
                    egui::Align2::LEFT_BOTTOM,
                    cell.width(),
                    13.0,
                    W::Regular,
                    if live { pal.solo } else { pal.ink3 },
                );
                look::say(
                    sh.b,
                    &s,
                    "dB",
                    cell.left() + vw + s.s(4.0),
                    value_y,
                    egui::Align2::LEFT_BOTTOM,
                    s.s(20.0),
                    10.0,
                    W::Regular,
                    pal.ink3,
                );
                sh.b.rrect(sh.b.snap_rect(bar), 1.5, Fill::Solid(pal.bar_track));
                if live {
                    let fw = (gr / 12.0).min(1.0) * bar.width();
                    let lit = Rect::from_min_max(pos2(bar.right() - fw, bar.top()), bar.max);
                    sh.b.rrect(sh.b.snap_rect(lit), 1.5, Fill::Solid(pal.solo));
                }
            }
            Col::Param(info) => {
                let range = Range {
                    min: info.min,
                    max: info.max,
                    default: info.default,
                    taper: if info.logarithmic {
                        Taper::Log
                    } else {
                        Taper::Linear
                    },
                    stepped: info.stepped,
                    bipolar: false,
                };
                let key = info.key;
                let v = slot.spec.get(key);
                let id = Id::new(("insp-fxp", e, key));
                let mut d = sheet::drag(sh.ui, id, cell, v, &range, false);
                let target = cantus_core::AutoTarget::Effect {
                    effect: e,
                    key: key.to_string(),
                };
                let live = super::modulators::wire_drag(
                    sh,
                    app,
                    info.label_key,
                    &mut d,
                    cell,
                    range.pos(v),
                    target,
                    true,
                );
                let shown = d.value.unwrap_or(v);
                if let Some(nv) = d.value {
                    sh.acts.push(Act::Merge(
                        Command::SetEffectParam {
                            strip: host,
                            effect: e,
                            key: key.to_string(),
                            value: nv,
                        },
                        MergeKey::of(&("fx", e.0, key)),
                    ));
                }
                if d.ended {
                    sh.acts.push(Act::EndGesture);
                }
                let hot = d.resp.hovered() || d.resp.dragged();
                let label = sheet::name(sh.fmt.tr(info.label_key));
                look::say(
                    sh.b,
                    &s,
                    &label,
                    cell.left(),
                    label_y,
                    egui::Align2::LEFT_BOTTOM,
                    cell.width(),
                    10.0,
                    W::Regular,
                    pal.ink3,
                );
                let unit = info.unit;
                let text = sh.fmt.get(
                    cache::key(("insp-mini-v", e, key)),
                    sheet::bits(shown),
                    |b| sheet::unit(shown, unit, b),
                );
                // The figure, then its unit smaller ("−18.0" "dB").
                let (fig, u) = match text.rsplit_once(' ') {
                    Some((f, u)) if !unit.is_empty() => (f, u),
                    _ => (text, ""),
                };
                let fig = sheet::name(fig);
                let u = sheet::name(u);
                let vw = look::say(
                    sh.b,
                    &s,
                    &fig,
                    cell.left(),
                    value_y,
                    egui::Align2::LEFT_BOTTOM,
                    cell.width(),
                    13.0,
                    W::Regular,
                    pal.ink1,
                );
                if !u.is_empty() {
                    look::say(
                        sh.b,
                        &s,
                        &u,
                        cell.left() + vw + s.s(3.0),
                        value_y,
                        egui::Align2::LEFT_BOTTOM,
                        (cell.width() - vw - s.s(3.0)).max(4.0),
                        10.0,
                        W::Regular,
                        pal.ink3,
                    );
                }
                let pos = range.pos(shown).clamp(0.0, 1.0);
                sh.b.rrect(sh.b.snap_rect(bar), 1.5, Fill::Solid(pal.bar_track));
                let fill =
                    Rect::from_min_max(bar.min, pos2(bar.left() + bar.width() * pos, bar.bottom()));
                if fill.width() > 0.5 {
                    sh.b.rrect(
                        sh.b.snap_rect(fill),
                        1.5,
                        Fill::Solid(if hot { pal.ink1 } else { pal.ink2 }),
                    );
                }
                if let Some(lv) = live {
                    let lx = bar.left() + bar.width() * lv.clamp(0.0, 1.0);
                    sh.b.vline(lx, bar.top() - 3.0, bar.bottom() + 3.0, 2.0, s.t.hue_mod);
                }
                if hot {
                    crate::kit::hint::set(
                        sh.ui.ctx(),
                        &label,
                        crate::kit::hint::Caps::PARAM,
                        d.resp.dragged(),
                        |b| b.write_str(text),
                    );
                }
            }
        }
    }
}

/// A hosted plug-in: its format, whether it is here, and the latency it adds.
fn hosted(sh: &mut Sheet<'_>, app: &CantusApp, slot: &EffectSlot) {
    let Some(p) = slot.spec.external.as_deref() else {
        return;
    };
    sh.text_row("inspector.format", p.format.badge(), false, false);
    let e = slot.id;
    // Stopped working (round 29, review #33): said in the danger colour, why in its tip.
    if app.plugins.crashed(e).is_some() {
        let v = sheet::name(sh.fmt.tr("isolation.stopped_card"));
        let row = sh.row();
        sh.label(row.label, "inspector.status", false);
        let s = sh.s.hue_of(crate::kit::Tok::Danger);
        crate::kit::well::readout(sh.b, row.value, &s, State::REST, &v, false);
        let cy = row.value.center().y;
        sh.b.dot(
            pos2(row.value.right() - s.s(10.0), cy),
            s.s(3.0),
            s.t.danger,
        );
        let r = sh
            .ui
            .interact(row.full, Id::new(("insp-mini-stopped", e)), Sense::hover());
        if r.hovered() {
            use crate::views::plugin_notice::{tip, Target};
            let tip = tip(app, Target::Insert(e));
            crate::kit::hint::set_text(sh.ui.ctx(), &v, &tip, crate::kit::hint::Caps::NONE);
        }
        return;
    }
    if app.plugins.missing(e).is_some() {
        let v = sheet::name(sh.fmt.tr("host.missing"));
        let row = sh.row();
        sh.label(row.label, "inspector.status", false);
        let s = sh.s.hue_of(crate::kit::Tok::Danger);
        crate::kit::well::readout(sh.b, row.value, &s, State::REST, &v, false);
        let cy = row.value.center().y;
        sh.b.dot(
            pos2(row.value.right() - s.s(10.0), cy),
            s.s(3.0),
            s.t.danger,
        );
        return;
    }
    let latency = app.plugins.hosted(e).map(|h| h.latency());
    let text = sh.fmt.get(
        cache::key(("insp-hosted-lat", e)),
        latency.map_or(u64::MAX, |l| l as u64),
        |b| match latency {
            Some(n) => {
                let _ = write!(b, "{n} smp");
            }
            None => b.push_str(&crate::i18n::t("host.preview_missing")),
        },
    );
    let text = sheet::name(text);
    sh.text_row(
        "kit.inspector.latency",
        &text,
        true,
        latency.is_none_or(|n| n == 0),
    );
}

/// Sample the effect's curve into `out`: an EQ's response in dB over 20 Hz … 20 kHz, or a
/// compressor's output level over −60 … 0 dB of input.
fn compute(slot: &EffectSlot, out: &mut [f32; N]) {
    match slot.spec.kind {
        EffectKind::Eq => {
            use cantus_dsp::aequalis::{self, Mode};
            let bands = cantus_engine::eq_bands(&slot.spec);
            let mode = Mode::from_index(slot.spec.get("mode").round() as usize);
            let sr = 48_000.0;
            let hz: [f32; N] =
                std::array::from_fn(|i| 20.0 * 1000f32.powf(i as f32 / (N - 1) as f32));
            let mut one = [0.0f32; N];
            out.fill(0.0);
            for b in bands.iter().filter(|b| b.active) {
                aequalis::band_curve_db(b, mode, sr, &hz, &mut one);
                for (t, o) in out.iter_mut().zip(&one) {
                    *t += o;
                }
            }
        }
        EffectKind::Compressor => {
            let c = cantus_engine::comp_settings(&slot.spec);
            for (i, o) in out.iter_mut().enumerate() {
                let d = -60.0 + 60.0 * i as f32 / (N - 1) as f32;
                *o = d - c.mix * cantus_dsp::comprimo::effective_gain_reduction_db(d, &c);
            }
        }
        _ => out.fill(0.0),
    }
}

/// Paint a mini curve on a display ground.
fn paint_curve(b: &mut Brush, rect: Rect, s: &Style, kind: EffectKind, pts: &[f32; N]) {
    let t = &s.t;
    b.register(
        rect,
        "mini_view",
        &[crate::kit::Tok::Ink, crate::kit::Tok::Grid],
        &[s.hue_tok],
    );
    let rect = b.snap_rect(rect);
    b.rrect(
        rect,
        s.r(3.0),
        Fill::V(shade(t.ink, if t.dark { 0.025 } else { 0.0 }), t.ink),
    );
    let plot = rect.shrink2(Vec2::new(s.s(4.0), s.s(5.0)));
    let px = b.px();
    let mut line = [Pos2::ZERO; N];
    let base;
    match kind {
        EffectKind::Compressor => {
            // Input across, output up, the unity diagonal for reference.
            let x_of = |d: f32| plot.left() + (d + 60.0) / 60.0 * plot.width();
            let y_of = |d: f32| plot.bottom() - ((d + 60.0) / 60.0).clamp(0.0, 1.0) * plot.height();
            b.segment(
                pos2(x_of(-60.0), y_of(-60.0)),
                pos2(x_of(0.0), y_of(0.0)),
                px,
                with_alpha(t.grid, 0.9),
                false,
            );
            for (i, p) in line.iter_mut().enumerate() {
                let d = -60.0 + 60.0 * i as f32 / (N - 1) as f32;
                *p = pos2(x_of(d), y_of(pts[i]));
            }
            base = plot.bottom();
        }
        _ => {
            let range = 18.0;
            let y_of =
                |db: f32| plot.center().y - (db / range).clamp(-1.0, 1.0) * plot.height() * 0.5;
            b.hline(
                plot.left(),
                plot.right(),
                b.snap_mid(y_of(0.0)),
                px,
                with_alpha(t.grid, 0.9),
            );
            for (i, p) in line.iter_mut().enumerate() {
                let x = plot.left() + plot.width() * i as f32 / (N - 1) as f32;
                *p = pos2(x, y_of(pts[i]));
            }
            base = y_of(0.0);
        }
    }
    b.area(
        &line,
        base,
        with_alpha(s.hue, if t.dark { 0.16 } else { 0.12 }),
    );
    b.polyline(&line, s.s(1.5), s.hue);
}
