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
        EffectKind::Eq | EffectKind::Compressor => {
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
            for info in kind
                .params()
                .iter()
                .filter(|p| p.key != "mix" && !p.stepped)
                .take(3)
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
                sh.param_row(
                    info.label_key,
                    Id::new(("insp-fxp", e, key)),
                    slot.spec.get(key),
                    &range,
                    false,
                    |v, b| sheet::unit(v, unit, b),
                    f,
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
    sh.param_row(
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
    // A rule in the effect's hue down the left margin: these rows are the tile's.
    let x = last.full.left() + s.s(4.0);
    let hue = s.function(f).hue;
    sh.b.rect(
        Rect::from_min_max(
            pos2(x, top),
            pos2(x + s.s(2.0), last.full.bottom() - s.s(2.0)),
        ),
        Fill::Solid(with_alpha(hue, 0.7)),
    );
    sh.ui.add_space(s.d(4.0));
}

/// A hosted plug-in: its format, whether it is here, and the latency it adds.
fn hosted(sh: &mut Sheet<'_>, app: &CantusApp, slot: &EffectSlot) {
    let Some(p) = slot.spec.external.as_deref() else {
        return;
    };
    sh.text_row("inspector.format", p.format.badge(), false, false);
    let e = slot.id;
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
