//! The Partes (additive) and Grana (granular) engines' pictures, on the kit's brush: the
//! partials as bars where they sound (stretch moves them), the source with the grain
//! position and the spray's reach, and the grain window's shape; and their readouts.

use crate::i18n::t;
use crate::kit::color::with_alpha;
use crate::kit::text::Role;
use crate::kit::{Brush, Fill, Style};
use cantus_core::NativeSpec;
use cantus_dsp::unda::table::HARMONICS;
use cantus_dsp::unda::Table;
use egui::{Align2, Color32, Pos2, Rect, Vec2};
use std::fmt::Write as _;

/// Which engine oscillator `o` runs: 0 wavetable, 1 Partes, 2 Grana.
pub fn engine(spec: &NativeSpec, o: usize) -> usize {
    spec.get(&format!("osc{}.engine", o + 1)).round().max(0.0) as usize
}

fn get(spec: &NativeSpec, o: usize, f: &str) -> f32 {
    spec.get(&format!("osc{}.{f}", o + 1))
}

/// A caption at the display's top-left, on a plate.
fn caption(b: &mut Brush, d: Rect, s: &Style, text: &str) {
    super::parts::chip(
        b,
        Rect::from_min_size(
            d.min + Vec2::splat(s.s(6.0)),
            Vec2::new(d.width() * 0.7, s.s(16.0)),
        ),
        s,
        text,
        s.t.text,
        Align2::LEFT_CENTER,
    );
}

/// Partes: each partial a bar at its frequency ratio (log across the partials' span), its
/// height the frame's magnitude with tilt and odd/even, in dB over 60 dB so the quiet ones
/// show. The fundamental is drawn in the "now" colour.
pub fn draw_partes(
    b: &mut Brush,
    d: Rect,
    s: &Style,
    table: &Table,
    spec: &NativeSpec,
    o: usize,
    color: Color32,
) {
    let count = (get(spec, o, "partials").round() as usize).clamp(1, HARMONICS.min(256));
    let (tilt, oe, stretch) = (
        get(spec, o, "tilt"),
        get(spec, o, "oddeven"),
        get(spec, o, "stretch"),
    );
    let pos = get(spec, o, "pos");
    let frame = (pos.clamp(0.0, 1.0) * (table.frames().max(1) - 1) as f32).round() as usize;
    let top_ratio = (count as f32).powf(1.0 + stretch).max(2.0);
    let r = d.shrink2(Vec2::new(s.s(8.0), s.s(6.0)));
    let x_of = |ratio: f32| r.left() + r.width() * ratio.ln() / top_ratio.ln();
    let mut peak = 1e-9f32;
    let amp = |k: usize| {
        let (a, bb) = table.partial(frame, k);
        let kk = (k + 1) as f32;
        let tilt_g = 10f32.powf(tilt * kk.log2() / 20.0);
        let oe_g = if k == 0 {
            1.0
        } else if (k + 1) % 2 == 1 {
            1.0 - oe.max(0.0)
        } else {
            1.0 + oe.min(0.0)
        };
        (a * a + bb * bb).sqrt() * tilt_g * oe_g
    };
    for k in 0..count {
        peak = peak.max(amp(k));
    }
    let bar_w = s.s(1.5);
    for k in 0..count {
        let ratio = ((k + 1) as f32).powf(1.0 + stretch);
        let x = x_of(ratio);
        let db = 20.0 * (amp(k) / peak).max(1e-3).log10();
        let h = (r.height() - s.s(14.0)) * (1.0 + db / 60.0).clamp(0.0, 1.0);
        let c = if k == 0 { s.t.highlight } else { color };
        b.segment(
            Pos2::new(x, r.bottom()),
            Pos2::new(x, r.bottom() - h),
            bar_w,
            c,
            false,
        );
    }
    caption(b, d, s, &format!("{count} {}", t("unda.partials_unit")));
}

/// Grana: the source's level across its length (a table's frames, or the imported audio's
/// cycles), the position in the "now" colour and the spray's reach around it.
pub fn draw_grana(
    b: &mut Brush,
    d: Rect,
    s: &Style,
    table: &Table,
    spec: &NativeSpec,
    o: usize,
    color: Color32,
) {
    let n = table.frames().max(1);
    let r = d.shrink2(Vec2::new(s.s(8.0), s.s(4.0)));
    let mid = r.center().y;
    let w = r.width() / n as f32;
    for f in 0..n {
        let pk = table.cycle(f).iter().fold(0.0f32, |m, x| m.max(x.abs()));
        let x = r.left() + w * (f as f32 + 0.5);
        let h = (r.height() * 0.44) * pk;
        b.segment(
            Pos2::new(x, mid - h),
            Pos2::new(x, mid + h),
            w.clamp(s.s(1.0), s.s(6.0)),
            with_alpha(color, 0.7),
            false,
        );
    }
    let pos = get(spec, o, "pos").clamp(0.0, 1.0);
    let spray = get(spec, o, "spray");
    let has_file = spec
        .text(cantus_core::inst::wavetable::TABLE_KEYS[o])
        .is_some();
    // In the table the spray spans ×10 of its seconds as a fraction; for a file, its
    // seconds over the file's length are not known here, so it is drawn the same way.
    let reach = if has_file { spray * 2.0 } else { spray * 10.0 };
    let x0 = r.left() + r.width() * (pos - reach).clamp(0.0, 1.0);
    let x1 = r.left() + r.width() * (pos + reach).clamp(0.0, 1.0);
    b.rect(
        Rect::from_min_max(
            Pos2::new(x0, d.top()),
            Pos2::new(x1.max(x0 + 1.0), d.bottom()),
        ),
        Fill::Solid(with_alpha(s.t.highlight, 0.16)),
    );
    let x = r.left() + r.width() * pos;
    b.vline(x, d.top(), d.bottom(), s.s(2.0), s.t.highlight);
    let label = if has_file {
        t("unda.grana_file")
    } else {
        t("unda.grana_table")
    };
    caption(b, d, s, &label);
}

/// The grain window's shape (Hann, Gauss, Tukey, Expodec), filled, with its name.
pub fn draw_window(b: &mut Brush, d: Rect, s: &Style, spec: &NativeSpec, o: usize) {
    let w = get(spec, o, "gwin").round().max(0.0) as usize;
    let r = d.shrink2(Vec2::new(s.s(12.0), s.s(8.0)));
    let shape = |x: f32| -> f32 {
        use std::f32::consts::TAU;
        match w {
            1 => (-0.5 * ((x - 0.5) / 0.16).powi(2)).exp(),
            2 => {
                let a = 0.5;
                if x < a * 0.5 {
                    0.5 - 0.5 * (TAU * x / a).cos()
                } else if x > 1.0 - a * 0.5 {
                    0.5 - 0.5 * (TAU * (1.0 - x) / a).cos()
                } else {
                    1.0
                }
            }
            3 => {
                let rise = (x / 0.04).min(1.0);
                rise * (-5.0 * x).exp()
            }
            _ => 0.5 - 0.5 * (TAU * x).cos(),
        }
    };
    let mut pts = [Pos2::ZERO; 97];
    for (i, p) in pts.iter_mut().enumerate() {
        let x = i as f32 / 96.0;
        *p = Pos2::new(r.left() + r.width() * x, r.bottom() - r.height() * shape(x));
    }
    b.hline(
        r.left(),
        r.right(),
        b.snap_mid(r.bottom()),
        b.px(),
        s.t.grid,
    );
    b.area(
        &pts,
        r.bottom(),
        with_alpha(s.hue, if s.t.dark { 0.20 } else { 0.14 }),
    );
    b.polyline(&pts, s.s(1.8), s.hue);
    let names = cantus_core::inst::wavetable::GRAIN_WINDOWS;
    let name = t(&format!(
        "unda.gwin.{}",
        names.get(w).copied().unwrap_or("hann")
    ));
    super::parts::say(
        b,
        Rect::from_min_size(
            Pos2::new(d.right() - s.s(90.0), d.top() + s.s(4.0)),
            Vec2::new(s.s(84.0), s.s(16.0)),
        ),
        s,
        &name,
        Role::Label,
        s.t.text_dim,
        Align2::RIGHT_CENTER,
    );
}

/// Tilt, per octave.
pub fn tilt_text(v: f32, b: &mut String) {
    let _ = write!(b, "{v:+.0} dB/oct");
}

/// Odd / even: All, Odd 40, Even 20.
pub fn oddeven_text(v: f32, b: &mut String) {
    let p = (v * 100.0).round();
    let _ = if p == 0.0 {
        write!(b, "{}", t("unda.oddeven_all"))
    } else if p < 0.0 {
        write!(b, "{} {}", t("unda.odd"), -p)
    } else {
        write!(b, "{} {p}", t("unda.even"))
    };
}
