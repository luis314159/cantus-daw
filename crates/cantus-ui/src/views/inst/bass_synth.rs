//! The Synth engine's display: the filter's response with one handle (drag across for the
//! cutoff, up and down for the resonance; double-click resets both), the live cutoff while
//! a note plays (envelope, key tracking and the 303's accent sweep included), the unison
//! voices on Szabo's curves with the outer detune in cents, and the accent meter — the
//! sweep's charge. Drawn with the kit's brush in the Tone hue.
//!
//! The response drawn is the analog prototype of the filter chosen, with the bass
//! compensation applied — so you see the passband stay put as the resonance rises, which
//! is what the control is for.

use super::{micro, Grab};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::{shade, with_alpha};
use crate::kit::meter;
use crate::kit::{self, light, Brush, Fill, Style, Tok};
use cantus_core::{NativeSpec, TrackId};
use cantus_dsp::gravis::filter::{diode_response, DiodeCal, LADDER_K_MAX};
use cantus_dsp::gravis::synth::{mix_gains, outer_cents, UNISON_OFFSETS};
use cantus_engine::inst::bass::tel;
use egui::{Align2, Pos2, Rect, Sense, Vec2};

const LO_HZ: f32 = 20.0;
const HI_HZ: f32 = 20_000.0;
const TOP_DB: f32 = 24.0;
const BOTTOM_DB: f32 = -48.0;
const POINTS: usize = 180;
/// The tokens the display paints with, beside the hue (for inspect mode).
const TOKENS: &[Tok] = &[
    Tok::Ink,
    Tok::Grid,
    Tok::GridStrong,
    Tok::Text,
    Tok::TextDim,
];

#[derive(Copy, Clone)]
struct Geo {
    rect: Rect,
}

impl Geo {
    fn x(&self, hz: f32) -> f32 {
        let f = (hz.max(1.0) / LO_HZ).ln() / (HI_HZ / LO_HZ).ln();
        self.rect.left() + f * self.rect.width()
    }
    fn hz(&self, x: f32) -> f32 {
        let f = ((x - self.rect.left()) / self.rect.width()).clamp(0.0, 1.0);
        LO_HZ * (HI_HZ / LO_HZ).powf(f)
    }
    fn y(&self, db: f32) -> f32 {
        let f = (db - BOTTOM_DB) / (TOP_DB - BOTTOM_DB);
        self.rect.bottom() - f.clamp(-0.02, 1.02) * self.rect.height()
    }
}

/// The filter's magnitude in dB at `hz`, for its kind (0 ladder, 1 diode, 2 SVF LP, 3 SVF
/// BP), cutoff, resonance and bass compensation.
pub fn response_db(kind: usize, hz: f32, cutoff: f32, reso: f32, comp: f32, cal: &DiodeCal) -> f32 {
    let w = (hz / cutoff) as f64;
    let reso = reso.clamp(0.0, 1.0) as f64;
    let comp = comp.clamp(0.0, 1.0) as f64;
    let cmul = |a: (f64, f64), b: (f64, f64)| (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0);
    let cdiv = |a: (f64, f64), b: (f64, f64)| {
        let d = b.0 * b.0 + b.1 * b.1;
        ((a.0 * b.0 + a.1 * b.1) / d, (a.1 * b.0 - a.0 * b.1) / d)
    };
    let mag = |z: (f64, f64)| (z.0 * z.0 + z.1 * z.1).sqrt();
    let h = match kind {
        0 | 1 => {
            let (open, k) = if kind == 0 {
                let g = cdiv((1.0, 0.0), (1.0, w));
                let g2 = cmul(g, g);
                (cmul(g2, g2), reso * LADDER_K_MAX)
            } else {
                (diode_response(w * cal.w180), reso * cal.k_max * 1.02)
            };
            let num = (open.0 * (1.0 + comp * k), open.1 * (1.0 + comp * k));
            mag(cdiv(num, (1.0 + k * open.0, k * open.1)))
        }
        _ => {
            let k = 2.0 - 1.96 * reso;
            let den = (1.0 - w * w, w * k);
            if kind == 2 {
                mag(cdiv((1.0, 0.0), den))
            } else {
                mag(cdiv((0.0, w * k), den))
            }
        }
    };
    (20.0 * h.max(1e-9).log10()) as f32
}

fn curve(
    geo: &Geo,
    kind: usize,
    cutoff: f32,
    reso: f32,
    comp: f32,
    cal: &DiodeCal,
) -> [Pos2; POINTS] {
    std::array::from_fn(|i| {
        let hz = LO_HZ * (HI_HZ / LO_HZ).powf(i as f32 / (POINTS - 1) as f32);
        Pos2::new(
            geo.x(hz),
            geo.y(response_db(kind, hz, cutoff, reso, comp, cal)),
        )
    })
}

/// The display in `d`, painted, with its handle. `s` is in the Tone hue.
pub fn display(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    d: Rect,
    s: &Style,
) {
    let k = s.k;
    let resp = ui.interact(
        d,
        ui.id().with(("gravis-synth", track.0)),
        Sense::click_and_drag(),
    );
    let side_w = (d.width() * 0.26).max(180.0 * k);
    let plot = Rect::from_min_max(
        d.min + Vec2::new(12.0 * k, 22.0 * k),
        Pos2::new(d.right() - side_w - 40.0 * k, d.bottom() - 18.0 * k),
    );
    let geo = Geo { rect: plot };
    let kind = spec.get("syn.filter").round() as usize;
    let cutoff = spec.get("syn.cutoff");
    let reso = spec.get("syn.reso");
    let comp = spec.get("syn.bass_comp");
    let cal = DiodeCal::compute();
    let at_cutoff = response_db(kind, cutoff, cutoff, reso, comp, &cal).clamp(BOTTOM_DB, TOP_DB);
    let handle = Pos2::new(geo.x(cutoff), geo.y(at_cutoff));
    let hot = app.inst.bass.grab == Some(Grab::Filter)
        || resp
            .hover_pos()
            .is_some_and(|p| p.distance(handle) < 18.0 * k);
    let tele = app.inst_telemetry();
    let live = (tele[super::super::TEL_VOICES] > 0.0
        && tele[tel::ENGINE] == 1.0
        && tele[tel::CUTOFF] > 0.0)
        .then_some(tele[tel::CUTOFF]);
    let sweep = tele[tel::SWEEP].clamp(0.0, 1.0);
    kit::with_brush(&ui.painter_at(d), |b| {
        b.register(d, "gravis.synth", TOKENS, &[s.hue_tok]);
        let t = &s.t;
        for decade in [100.0f32, 1_000.0, 10_000.0] {
            for m in [1.0f32, 2.0, 5.0] {
                let hz = decade * m;
                if hz > HI_HZ {
                    continue;
                }
                let x = geo.x(hz);
                b.vline(
                    b.snap_mid(x),
                    plot.top(),
                    plot.bottom(),
                    b.px(),
                    if m == 1.0 { t.grid_strong } else { t.grid },
                );
                let mut text = String::with_capacity(8);
                crate::views::inst::write_readout(hz, "Hz", &mut text);
                let text = text.replace(".00", "");
                let lr = Rect::from_min_size(
                    Pos2::new(x + 2.0 * k, plot.bottom() + 3.0 * k),
                    Vec2::new(44.0 * k, 12.0 * k),
                );
                micro(b, lr, &text, t.text_dim, Align2::LEFT_CENTER, s);
            }
        }
        for db in [-36.0f32, -24.0, -12.0, 0.0, 12.0] {
            let y = geo.y(db);
            b.hline(plot.left(), plot.right(), b.snap_mid(y), b.px(), t.grid);
            let lr = Rect::from_min_size(
                Pos2::new(plot.right() + 4.0 * k, y - 6.0 * k),
                Vec2::new(30.0 * k, 12.0 * k),
            );
            micro(
                b,
                lr,
                &format!("{db:+.0}"),
                t.text_dim,
                Align2::LEFT_CENTER,
                s,
            );
        }
        let pts = curve(&geo, kind, cutoff, reso, comp, &cal);
        b.area(
            &pts,
            plot.bottom(),
            with_alpha(s.hue, if t.dark { 0.16 } else { 0.12 }),
        );
        b.polyline(&pts, 2.0 * k, s.hue);
        // The cutoff while a note plays: the envelope, key tracking and the accent sweep.
        if let Some(live) = live {
            let pts = curve(&geo, kind, live, reso, comp, &cal);
            for (i, w) in pts.windows(2).enumerate() {
                if i % 3 != 2 {
                    b.segment(w[0], w[1], k, t.text, false);
                }
            }
        }
        // The handle sits on the curve at the cutoff: across is the cutoff, up the resonance.
        if hot {
            light::glow(b, handle, 18.0 * k, s.hue, 0.8);
        }
        b.circle(handle, 7.0 * k, Fill::Solid(s.hue));
        b.circle_stroke(handle, 7.0 * k, k, shade(s.hue, -0.25));
        let mut text = String::with_capacity(24);
        crate::views::inst::write_readout(cutoff, "Hz", &mut text);
        let text = format!("{text} \u{b7} {:.0} %", reso * 100.0);
        let lr = Rect::from_min_size(
            handle + Vec2::new(11.0 * k, -18.0 * k),
            Vec2::new(110.0 * k, 13.0 * k),
        );
        micro(b, lr, &text, t.text, Align2::LEFT_CENTER, s);
        let name = t_of(match kind {
            1 => "bass.filter.diode",
            2 => "bass.filter.svf_lp",
            3 => "bass.filter.svf_bp",
            _ => "bass.filter.ladder",
        });
        let lr = Rect::from_min_size(
            d.min + Vec2::new(10.0 * k, 4.0 * k),
            Vec2::new(plot.width(), 13.0 * k),
        );
        micro(b, lr, &name, t.text_dim, Align2::LEFT_CENTER, s);

        // Beside it: the unison voices, and the accent meter.
        let side = Rect::from_min_max(
            Pos2::new(d.right() - side_w, d.top() + 22.0 * k),
            Pos2::new(d.right() - 12.0 * k, d.bottom() - 18.0 * k),
        );
        unison(b, side, spec, s);
        let m = Rect::from_min_max(
            Pos2::new(side.left(), side.bottom() - 10.0 * k),
            Pos2::new(side.right(), side.bottom()),
        );
        let db = if sweep > 0.0 {
            -48.0 * (1.0 - sweep)
        } else {
            -120.0
        };
        meter::paint(
            b,
            m,
            s,
            &meter::Levels {
                level: [db, db],
                hold: [db, db],
            },
        );
        let lr = Rect::from_min_size(
            Pos2::new(m.left(), m.top() - 15.0 * k),
            Vec2::new(m.width(), 13.0 * k),
        );
        let text = tf(
            "bass.sweep_now",
            &[("pct", &format!("{:.0}", sweep * 100.0))],
        );
        micro(b, lr, &text, t.text, Align2::LEFT_CENTER, s);
    });
    if resp.hovered() {
        crate::kit::hint::set_text(
            ui.ctx(),
            &t("bass.syn.cutoff"),
            &t("bass.syn.drag_tip"),
            crate::kit::hint::Caps::PARAM,
        );
    }
    if resp.drag_started() {
        let origin = ui.input(|i| i.pointer.press_origin());
        app.inst.bass.grab = origin
            .or(resp.interact_pointer_pos())
            .filter(|pt| pt.distance(handle) < 18.0 * k)
            .map(|_| Grab::Filter);
    }
    if resp.dragged() && app.inst.bass.grab == Some(Grab::Filter) {
        if let Some(pt) = resp.interact_pointer_pos() {
            let hz = geo.hz(pt.x).clamp(20.0, 20_000.0);
            // Resonance follows the vertical motion: the whole height is 0 → 100 %.
            let r = (reso - resp.drag_delta().y / (plot.height() * 0.8)).clamp(0.0, 1.0);
            // Both axes in one command under one key: the whole drag is one undo step.
            let set = |key: &str, value: f32| cantus_core::Command::SetInstrumentValue {
                track,
                key: key.into(),
                value,
            };
            app.exec_merging(
                cantus_core::Command::Batch {
                    label_key: "cmd.set_instrument_value".into(),
                    commands: vec![set("syn.cutoff", hz), set("syn.reso", r)],
                },
                cantus_core::MergeKey::of(&("gravis-filter", track.0)),
            );
        }
    }
    if resp.drag_stopped() {
        app.inst.bass.grab = None;
        app.end_gesture();
    }
    if resp.double_clicked()
        && resp
            .interact_pointer_pos()
            .is_some_and(|pt| pt.distance(handle) < 18.0 * k)
    {
        for key in ["syn.cutoff", "syn.reso"] {
            if let Some(info) = spec.kind.param(key) {
                app.set_inst_value(track, key, info.default, false);
            }
        }
    }
}

fn t_of(key: &str) -> String {
    t(key)
}

/// The unison voices as ticks at their detune, as tall as their level.
fn unison(b: &mut Brush, r: Rect, spec: &NativeSpec, s: &Style) {
    let k = s.k;
    let n = spec.get("syn.unison").round().clamp(1.0, 7.0) as usize;
    let detune = spec.get("syn.detune") as f64;
    let spread = spec.get("syn.spread") as f64;
    let cents = outer_cents(n, detune);
    let span = cents.max(10.0);
    let area = Rect::from_min_max(
        r.min + Vec2::new(0.0, 20.0 * k),
        Pos2::new(r.right(), r.bottom() - 34.0 * k),
    );
    let cx = area.center().x;
    b.hline(
        area.left(),
        area.right(),
        b.snap_mid(area.bottom()),
        b.px(),
        s.t.grid_strong,
    );
    let (cg, sg) = if n == 1 {
        (1.0, 0.0)
    } else {
        mix_gains(spread)
    };
    let set: &[usize] = match n {
        1 => &[3],
        2 => &[2, 4],
        3 => &[2, 3, 4],
        4 => &[1, 2, 4, 5],
        5 => &[1, 2, 3, 4, 5],
        6 => &[0, 1, 2, 4, 5, 6],
        _ => &[0, 1, 2, 3, 4, 5, 6],
    };
    let curve = cantus_dsp::gravis::synth::detune_curve(detune);
    for i in set {
        let c = 1200.0 * (1.0 + UNISON_OFFSETS[*i] * curve).log2();
        let x = cx + (c / span) as f32 * area.width() * 0.45;
        let g = if *i == 3 { cg } else { sg } as f32;
        let bar = Rect::from_min_max(
            Pos2::new(x - 1.5 * k, area.bottom() - g * area.height()),
            Pos2::new(x + 1.5 * k, area.bottom()),
        );
        b.rect(b.snap_rect(bar), Fill::Solid(s.hue));
    }
    let lr = Rect::from_min_size(r.min, Vec2::new(r.width(), 13.0 * k));
    let text = tf(
        "bass.syn.unison_caption",
        &[("n", &format!("{n}")), ("ct", &format!("{cents:.0}"))],
    );
    micro(b, lr, &text, s.t.text, Align2::LEFT_CENTER, s);
}
