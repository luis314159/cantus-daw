//! Lamina's two physical pictures, drawn in millimetres with the kit's brush into a card's
//! display:
//!
//! - **The pickup**: its transfer curve (flux for the tine, capacitance for the reed)
//!   across the tip's travel, the stretch the tip actually swings through lit from the
//!   engine's report, and below it the geometry at true scale — the tip, the pole piece (or
//!   the reed's slot) `gap` mm away and `offset` mm off the rest line. Drag the pole: sideways
//!   sets the offset, up and down the gap. The display is the control.
//! - **The tine (or reed)** from the side: its length for the key shown (from the cantilever's
//!   physics, `lamina::geometry`), where the hammer meets it, the tone bar beside it and the
//!   pickup at its tip, with a 10 mm scale.

use super::parts::label;
use super::set;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::{mix, shade, with_alpha};
use crate::kit::hint::{self, Caps};
use crate::kit::light::{self, RaisedOpts};
use crate::kit::text::{self as ktext, Elide, Policy, Role, Text};
use crate::kit::{with_brush, Brush, Fill, Style};
use cantus_core::TrackId;
use cantus_dsp::lamina::geometry::{reed_length_mm, tine_length_mm, transfer, Engine};
use cantus_dsp::lamina::LaminaSettings;
use cantus_engine::inst::epiano::tel;
use egui::{Align2, Pos2, Rect, Sense, Vec2};

/// Half the tip travel the curve shows, mm.
const SPAN_MM: f32 = 3.0;

/// The pickup in the display `d`: the header, the curve above, the geometry below, the pole
/// draggable.
pub fn pickup(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    ls: &LaminaSettings,
    tele: &cantus_engine::Telemetry,
    d: Rect,
    s: &Style,
) {
    let (gap, align, gap_key, align_key) = match ls.engine {
        Engine::Tine => (ls.tine_gap_mm, ls.tine_align_mm, "tine.gap", "tine.align"),
        Engine::Reed => (ls.reed_gap_mm, ls.reed_align_mm, "reed.gap", "reed.align"),
    };
    let alive = tele[tel::ALIVE] > 0.5;
    let (lo, hi, x_now) = if alive {
        (tele[tel::SWING_LO], tele[tel::SWING_HI], tele[tel::X])
    } else {
        (0.0, 0.0, 0.0)
    };
    let head = s.s(36.0);
    let inner = Rect::from_min_max(
        Pos2::new(d.left() + s.s(14.0), d.top() + head),
        Pos2::new(d.right() - s.s(14.0), d.bottom() - s.s(8.0)),
    );
    let px_mm = inner.width() / (2.0 * SPAN_MM);
    let cx = inner.center().x;
    let x_of = |mm: f32| cx + mm * px_mm;
    let curve = Rect::from_min_max(
        inner.min,
        Pos2::new(inner.right(), inner.top() + inner.height() * 0.48),
    );
    let y_of = |v: f32| curve.bottom() - v.clamp(0.0, 1.0) * curve.height();
    // The geometry: the tip on its rest line and the pole below it, both axes at one true
    // scale of their own (fitted so the pole shows at any gap), with a 1 mm bar.
    let geo_top = curve.bottom() + s.s(18.0);
    let geo_h = inner.bottom() - geo_top;
    let g = px_mm.min((geo_h - s.s(10.0)) / (gap + 1.0)).max(1.0);
    let gx = |mm: f32| cx + mm * g;
    let rest_y = geo_top + s.s(6.0);
    let pole_top = rest_y + gap * g;
    let pole_h = (0.6 * g).clamp(s.s(8.0), s.s(20.0));
    let pole_w = if ls.engine == Engine::Tine { 2.4 } else { 3.6 } * g;
    let pole = Rect::from_min_size(
        Pos2::new(gx(align) - pole_w * 0.5, pole_top),
        Vec2::new(pole_w, pole_h),
    );
    // The pole first: a drag moves it this frame.
    let resp = ui.interact(
        pole.expand(s.s(4.0)),
        egui::Id::new(("lamina-pole", track.0)),
        Sense::drag(),
    );
    app.inst.epiano.pole = Some(pole);
    if resp.dragged() {
        let dd = resp.drag_delta();
        if dd.x != 0.0 {
            let v = (align + dd.x / g).clamp(-2.0, 2.0);
            set(app, track, align_key, (v * 100.0).round() / 100.0, true);
        }
        if dd.y != 0.0 {
            let v = (gap + dd.y / g).clamp(0.3, 5.0);
            set(app, track, gap_key, (v * 100.0).round() / 100.0, true);
        }
    }
    if resp.drag_stopped() {
        app.end_gesture();
    }
    let hot = resp.hovered() || resp.dragged();
    if hot {
        hint::set_text(ui.ctx(), &t("epiano.pole_tip"), "", Caps::PARAM);
    }

    with_brush(&ui.painter_at(d), |b| {
        let tk = &s.t;
        // Header: what the curve is, the gap and offset; the swing below it.
        let what = t(if ls.engine == Engine::Tine {
            "epiano.flux"
        } else {
            "epiano.capacitance"
        });
        let title = tf(
            "epiano.pickup_title",
            &[
                ("what", &what),
                ("gap", &format!("{gap:.2}")),
                ("off", &format!("{align:+.2}")),
            ],
        );
        let row = s.s(15.0);
        let x0 = d.left() + s.s(10.0);
        let w = d.width() - s.s(20.0);
        label(
            b,
            Rect::from_min_size(Pos2::new(x0, d.top() + s.s(3.0)), Vec2::new(w, row)),
            &title,
            Role::Label,
            tk.text,
            s,
        );
        let swing = if hi > lo {
            tf(
                "epiano.swing",
                &[("mm", &format!("{:.2}", (hi - lo) * 0.5))],
            )
        } else {
            t("epiano.swing_idle")
        };
        label(
            b,
            Rect::from_min_size(Pos2::new(x0, d.top() + s.s(3.0) + row), Vec2::new(w, row)),
            &swing,
            Role::Label,
            tk.text_dim,
            s,
        );
        // Grid: millimetres.
        const MM: [&str; 7] = ["-3", "-2", "-1", "0", "+1", "+2", "+3"];
        for (i, mm) in (-3..=3).enumerate() {
            let x = b.snap_mid(x_of(mm as f32));
            b.vline(
                x,
                curve.top(),
                curve.bottom(),
                b.px(),
                if mm == 0 { tk.grid_strong } else { tk.grid },
            );
            let lr = Rect::from_center_size(
                Pos2::new(x, curve.bottom() + s.s(8.0)),
                Vec2::new(s.s(24.0), s.s(12.0)),
            );
            let _ = ktext::paint(b, lr, &Text::new(MM[i], Role::Micro, tk.text_dim), s.tk());
        }
        b.hline(
            curve.left(),
            curve.right(),
            curve.bottom(),
            b.px(),
            tk.grid_strong,
        );
        let mut pts = [Pos2::ZERO; 161];
        for (k, p) in pts.iter_mut().enumerate() {
            let mm = -SPAN_MM + 2.0 * SPAN_MM * k as f32 / 160.0;
            *p = Pos2::new(x_of(mm), y_of(transfer(ls.engine, mm, gap, align)));
        }
        b.area(
            &pts,
            curve.bottom(),
            with_alpha(s.hue, if tk.dark { 0.12 } else { 0.10 }),
        );
        b.polyline(&pts, s.s(1.5), mix(tk.text_dim, s.hue, 0.35));
        // The stretch the tip swings through, lit.
        if hi > lo {
            let mut seg = [Pos2::ZERO; 49];
            for (k, p) in seg.iter_mut().enumerate() {
                let mm = lo + (hi - lo) * k as f32 / 48.0;
                *p = Pos2::new(x_of(mm), y_of(transfer(ls.engine, mm, gap, align)));
            }
            b.polyline(&seg, s.s(4.0), s.hue);
            let now = Pos2::new(x_of(x_now), y_of(transfer(ls.engine, x_now, gap, align)));
            b.circle(now, s.s(5.0), Fill::Solid(s.hue));
            b.circle(now, s.s(2.0), Fill::Solid(tk.text));
        }
        // The rest line, the swing on it, the tip.
        b.hline(inner.left(), inner.right(), rest_y, b.px(), tk.grid_strong);
        if hi > lo {
            let band = Rect::from_min_max(
                Pos2::new(gx(lo), rest_y - s.s(3.0)),
                Pos2::new(gx(hi), rest_y + s.s(3.0)),
            );
            b.rrect(band, s.r(2.0), Fill::Solid(with_alpha(s.hue, 0.45)));
        }
        b.circle(Pos2::new(gx(x_now), rest_y), s.s(5.0), Fill::Solid(tk.text));
        // The centre line through the tip's rest, down past the pole.
        b.vline(
            b.snap_mid(cx),
            rest_y - s.s(8.0),
            pole.bottom() + s.s(4.0),
            b.px(),
            with_alpha(tk.text_dim, 0.5),
        );
        // The pole piece, or the reed's slot: two plates either side of the reed's path.
        let base = mix(tk.panel_raised, s.hue, 0.25);
        let plates: [Option<Rect>; 2] = match ls.engine {
            Engine::Tine => [Some(pole), None],
            Engine::Reed => {
                let w = pole.width() * 0.42;
                [
                    Some(Rect::from_min_size(pole.min, Vec2::new(w, pole.height()))),
                    Some(Rect::from_min_size(
                        Pos2::new(pole.max.x - w, pole.min.y),
                        Vec2::new(w, pole.height()),
                    )),
                ]
            }
        };
        for r in plates.into_iter().flatten() {
            light::raised(
                b,
                r,
                s.r(2.0),
                if hot { shade(base, 0.06) } else { base },
                RaisedOpts {
                    k: s.k,
                    depth: tk.depth,
                    ..Default::default()
                },
            );
        }
        if hot {
            b.rrect_stroke(pole.expand(s.s(2.0)), s.r(3.0), s.s(1.5), s.hue);
        }
        // The gap, dimensioned beside the pole.
        let dim_x = pole.right() + s.s(10.0);
        b.segment(
            Pos2::new(dim_x, rest_y),
            Pos2::new(dim_x, pole_top),
            b.px() * 1.5,
            tk.text_dim,
            false,
        );
        for y in [rest_y, pole_top] {
            b.hline(dim_x - s.s(3.0), dim_x + s.s(3.0), y, b.px(), tk.text_dim);
        }
        let mut v = String::with_capacity(10);
        super::parts::readout("", "mm", gap, &mut v);
        let gl = Rect::from_min_size(
            Pos2::new(dim_x + s.s(6.0), (rest_y + pole_top) * 0.5 - s.s(7.0)),
            Vec2::new(s.s(64.0), s.s(14.0)),
        );
        label(b, gl, &v, Role::Label, tk.text, s);
        // A 1 mm bar.
        let bar_y = inner.bottom() - s.s(2.0);
        b.segment(
            Pos2::new(inner.left(), bar_y),
            Pos2::new(inner.left() + g, bar_y),
            s.s(2.0),
            tk.text,
            false,
        );
        label(
            b,
            Rect::from_min_size(
                Pos2::new(inner.left() + g + s.s(5.0), bar_y - s.s(8.0)),
                Vec2::new(s.s(40.0), s.s(14.0)),
            ),
            "1 mm",
            Role::Label,
            tk.text,
            s,
        );
        if align.abs() < 0.1 {
            let r = Rect::from_min_max(
                Pos2::new(d.center().x, bar_y - s.s(8.0)),
                Pos2::new(inner.right(), bar_y + s.s(6.0)),
            );
            let _ = ktext::paint(
                b,
                r,
                &Text::key("epiano.octave_hint", Role::Label, s.hue)
                    .caps(false)
                    .align(Align2::RIGHT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
        }
    });
}

/// The tine (or reed) from the side, to scale, for the key shown, in the display `d`.
pub fn geometry(b: &mut Brush, ls: &LaminaSettings, key: u8, d: Rect, s: &Style) {
    let tk = &s.t;
    let hz = ls.a4_hz * 2f32.powf((key as f32 - 69.0) / 12.0);
    let len = match ls.engine {
        Engine::Tine => tine_length_mm(hz),
        Engine::Reed => reed_length_mm(hz),
    };
    let what = if ls.engine == Engine::Tine {
        "epiano.tine_geo"
    } else {
        "epiano.reed_geo"
    };
    let title = tf(
        what,
        &[
            ("key", &cantus_core::pitch_name(key)),
            ("hz", &format!("{hz:.1}")),
            ("len", &format!("{len:.0}")),
        ],
    );
    let strike = tf(
        "epiano.strike_geo",
        &[
            ("strike", &format!("{:.1}", ls.strike * len)),
            ("pct", &format!("{:.0}", ls.strike * 100.0)),
        ],
    );
    let head = s.s(20.0);
    label(
        b,
        Rect::from_min_size(
            Pos2::new(d.left() + s.s(10.0), d.top() + s.s(3.0)),
            Vec2::new(d.width() - s.s(20.0), s.s(15.0)),
        ),
        &title,
        Role::Label,
        tk.text,
        s,
    );
    // Where the hammer meets it, written at the bottom left (the 10 mm bar is at the right).
    let foot = s.s(18.0);
    label(
        b,
        Rect::from_min_size(
            Pos2::new(d.left() + s.s(10.0), d.bottom() - foot),
            Vec2::new(d.width() * 0.72, s.s(15.0)),
        ),
        &strike,
        Role::Label,
        tk.text_dim,
        s,
    );
    // Scale: the longest bass tine (about 200 mm) fits.
    let inner = Rect::from_min_max(
        Pos2::new(d.left() + s.s(12.0), d.top() + head),
        Pos2::new(d.right() - s.s(12.0), d.bottom() - foot),
    );
    let clamp_w = s.s(14.0);
    let px_mm = (inner.width() - clamp_w - s.s(30.0)) / 205.0;
    let x0 = inner.left() + clamp_w;
    let y = inner.center().y;
    let block = Rect::from_min_max(
        Pos2::new(inner.left(), y - s.s(14.0)),
        Pos2::new(x0, y + s.s(10.0)),
    );
    light::raised(
        b,
        block,
        s.r(2.0),
        tk.panel_raised,
        RaisedOpts {
            k: s.k,
            depth: tk.depth,
            ..Default::default()
        },
    );
    let tip = x0 + len * px_mm;
    match ls.engine {
        Engine::Tine => {
            // The tone bar above, twisted brass (the face's second accent), from the
            // same block.
            let brass = crate::views::inst::face::voice2(s, s.hue);
            let bar = Rect::from_min_max(
                Pos2::new(x0, y - s.s(12.0)),
                Pos2::new(x0 + len * 0.85 * px_mm, y - s.s(6.0)),
            );
            b.rrect(
                bar,
                s.r(2.0),
                Fill::V(
                    mix(tk.panel_raised, brass, 0.5),
                    mix(tk.panel_raised, brass, 0.3),
                ),
            );
            b.segment(Pos2::new(x0, y), Pos2::new(tip, y), s.s(2.5), tk.text, true);
        }
        Engine::Reed => {
            b.rrect(
                Rect::from_min_max(Pos2::new(x0, y - s.s(2.0)), Pos2::new(tip, y + s.s(2.0))),
                s.r(1.0),
                Fill::Solid(tk.text),
            );
            b.circle(Pos2::new(tip - s.s(3.0), y), s.s(4.5), Fill::Solid(s.hue));
        }
    }
    // The pickup at the tip.
    let gap = if ls.engine == Engine::Tine {
        ls.tine_gap_mm
    } else {
        ls.reed_gap_mm
    };
    let pk = Rect::from_min_size(
        Pos2::new(tip + gap.max(0.8) * px_mm * 2.0, y - s.s(9.0)),
        Vec2::new(s.s(8.0), s.s(18.0)),
    );
    b.rrect(pk, s.r(2.0), Fill::Solid(mix(tk.panel_raised, s.hue, 0.25)));
    b.rrect_stroke(pk, s.r(2.0), b.px(), tk.text_dim);
    // Where the hammer meets it.
    let hx = x0 + ls.strike * len * px_mm;
    b.polygon(
        &[
            Pos2::new(hx, y + s.s(3.0)),
            Pos2::new(hx - s.s(5.0), y + s.s(12.0)),
            Pos2::new(hx + s.s(5.0), y + s.s(12.0)),
        ],
        s.hue,
    );
    // A 10 mm bar.
    let sx = inner.right() - 10.0 * px_mm;
    let sy = d.bottom() - s.s(6.0);
    b.segment(
        Pos2::new(sx, sy),
        Pos2::new(inner.right(), sy),
        s.s(2.0),
        tk.text,
        false,
    );
    let _ = ktext::paint(
        b,
        Rect::from_min_max(
            Pos2::new(sx - s.s(48.0), d.bottom() - foot),
            Pos2::new(sx - s.s(4.0), d.bottom() - foot + s.s(15.0)),
        ),
        &Text::new("10 mm", Role::Label, tk.text)
            .caps(false)
            .align(Align2::RIGHT_CENTER),
        s.tk(),
    );
}
