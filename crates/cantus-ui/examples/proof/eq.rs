//! A mock EQ display, drawn with the kit's brush: grid, analyser, curve, band tokens.

use cantus_ui::kit::color::{shade, with_alpha};
use cantus_ui::kit::text::{self, Role, Text};
use cantus_ui::kit::{Brush, Fill, Style};
use egui::{Align2, Pos2, Rect, Vec2};

#[derive(Copy, Clone)]
pub enum Kind {
    Bell,
    LowShelf,
    HighShelf,
    LowCut,
    HighCut,
}

#[derive(Copy, Clone)]
pub struct Band {
    pub freq: f32,
    pub gain: f32,
    pub q: f32,
    pub kind: Kind,
}

const fn band(freq: f32, gain: f32, q: f32, kind: Kind) -> Band {
    Band {
        freq,
        gain,
        q,
        kind,
    }
}

pub const CHANNEL: [Band; 3] = [
    band(90.0, 2.5, 0.7, Kind::LowShelf),
    band(1200.0, -3.5, 1.1, Kind::Bell),
    band(8000.0, 3.0, 0.7, Kind::HighShelf),
];

pub const AEQUALIS: [Band; 6] = [
    band(38.0, 0.0, 0.7, Kind::LowCut),
    band(120.0, 2.5, 0.7, Kind::LowShelf),
    band(380.0, -4.0, 1.4, Kind::Bell),
    band(2400.0, 4.5, 0.9, Kind::Bell),
    band(6200.0, -2.5, 3.0, Kind::Bell),
    band(12000.0, 2.0, 0.7, Kind::HighShelf),
];

const F_MIN: f32 = 20.0;
const F_MAX: f32 = 20000.0;

fn x_of(f: f32, r: Rect) -> f32 {
    r.left() + r.width() * ((f / F_MIN).ln() / (F_MAX / F_MIN).ln())
}

fn f_at(x: f32, r: Rect) -> f32 {
    F_MIN * (F_MAX / F_MIN).powf(((x - r.left()) / r.width()).clamp(0.0, 1.0))
}

/// The summed response in dB (a smooth sketch of each filter's shape).
fn response(bands: &[Band], f: f32) -> f32 {
    bands
        .iter()
        .map(|b| {
            let o = (f / b.freq).log2();
            match b.kind {
                Kind::Bell => b.gain * (-(o * b.q * 1.6).powi(2)).exp(),
                Kind::LowShelf => b.gain / (1.0 + (o * 2.2 * b.q).exp()),
                Kind::HighShelf => b.gain / (1.0 + (-o * 2.2 * b.q).exp()),
                Kind::LowCut => -24.0 / (1.0 + (o * 5.0).exp()),
                Kind::HighCut => -24.0 / (1.0 + (-o * 4.0).exp()),
            }
        })
        .sum()
}

/// Paint the display in `rect`. `selected` lights one band's token; `big` adds the
/// analyser and the band-name ruler.
pub fn display(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    bands: &[Band],
    selected: Option<usize>,
    big: bool,
) {
    display_with(b, rect, s, bands, selected, big, true);
}

/// The display, with the band tokens optional (a filter curve has none).
#[allow(clippy::too_many_arguments)]
pub fn display_with(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    bands: &[Band],
    selected: Option<usize>,
    big: bool,
    tokens: bool,
) {
    let t = &s.t;
    let rect = b.snap_rect(rect);
    let range = if big { 18.0 } else { 12.0 };
    b.rect(
        rect,
        Fill::V(shade(t.ink, if t.dark { 0.025 } else { 0.0 }), t.ink),
    );
    let plot = rect.shrink2(Vec2::new(0.0, if big { 18.0 } else { 6.0 }));
    let y_of = |db: f32| plot.center().y - db / range * plot.height() * 0.5;
    let minor = if t.dark {
        with_alpha(t.grid, 0.9)
    } else {
        t.grid
    };
    let major = if t.dark {
        shade(t.grid, 0.07)
    } else {
        t.grid_strong
    };
    for f in [
        30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0, 800.0,
        900.0, 2000.0, 3000.0, 4000.0, 5000.0, 6000.0, 7000.0, 8000.0, 9000.0,
    ] {
        b.vline(
            b.snap_mid(x_of(f, plot)),
            plot.top(),
            plot.bottom(),
            b.px(),
            with_alpha(minor, 0.55),
        );
    }
    for f in [100.0, 1000.0, 10000.0] {
        b.vline(
            b.snap_mid(x_of(f, plot)),
            rect.top(),
            rect.bottom(),
            b.px(),
            major,
        );
    }
    let step = 6.0;
    let mut db = -range + step;
    while db < range {
        let c = if db.abs() < 0.1 {
            shade(major, 0.05)
        } else {
            minor
        };
        b.hline(plot.left(), plot.right(), b.snap_mid(y_of(db)), b.px(), c);
        db += step;
    }
    let n = (plot.width() / 3.0) as usize;
    let mut pts = [Pos2::ZERO; 400];
    let n = n.clamp(8, pts.len());
    if big {
        // The analyser: a spectrum under the curve.
        for (i, p) in pts.iter_mut().take(n).enumerate() {
            let x = plot.left() + plot.width() * i as f32 / (n - 1) as f32;
            let f = f_at(x, plot);
            let o = (f / 180.0).log2();
            let tilt = -3.2 * o.max(0.0) - 6.0 * (-o).max(0.0).powi(2);
            let wob = 1.2 * (i as f32 * 0.21).sin()
                + 0.8 * (i as f32 * 0.067).cos()
                + 0.5 * (i as f32 * 0.9).sin();
            let db = (-30.0 + tilt + wob).clamp(-72.0, 0.0);
            *p = Pos2::new(x, plot.bottom() - (db + 72.0) / 72.0 * plot.height() * 0.62);
        }
        b.area(
            &pts[..n],
            plot.bottom(),
            with_alpha(if t.dark { t.text_dim } else { t.label }, 0.09),
        );
        b.polyline(&pts[..n], b.px(), with_alpha(t.text_dim, 0.30));
    }
    for (i, p) in pts.iter_mut().take(n).enumerate() {
        let x = plot.left() + plot.width() * i as f32 / (n - 1) as f32;
        *p = Pos2::new(x, y_of(response(bands, f_at(x, plot)).clamp(-range, range)));
    }
    b.area(
        &pts[..n],
        y_of(0.0),
        with_alpha(s.hue, if t.dark { 0.16 } else { 0.12 }),
    );
    b.polyline(&pts[..n], 2.0, s.hue);
    // Band tokens.
    let digits = ["1", "2", "3", "4", "5", "6"];
    for (i, band) in bands.iter().enumerate() {
        if !tokens {
            break;
        }
        let x = x_of(band.freq, plot);
        let gain = if matches!(band.kind, Kind::LowCut) {
            0.0
        } else {
            band.gain
        };
        let c = Pos2::new(x, y_of(gain));
        let r = if big { 8.0 } else { 6.0 };
        let sel = selected == Some(i);
        if sel {
            cantus_ui::kit::light::glow(b, c, r * 2.2, s.hue, 0.8);
            b.circle(c, r, Fill::Solid(s.hue));
            let _ = text::paint(
                b,
                Rect::from_center_size(c, Vec2::splat(r * 2.0)),
                &Text::new(digits[i], Role::Micro, t.ink),
                1.0,
            );
        } else {
            b.circle(c, r, Fill::Solid(with_alpha(t.ink, 0.85)));
            b.circle_stroke(c, r - 0.75, 1.5, s.hue);
            let _ = text::paint(
                b,
                Rect::from_center_size(c, Vec2::splat(r * 2.0)),
                &Text::new(digits[i], Role::Micro, s.hue),
                1.0,
            );
        }
    }
    // Axis labels on plates, inside the display.
    let lab = |b: &mut Brush, c: Pos2, s2: &str| {
        let r = Rect::from_center_size(c, Vec2::new(28.0, 12.0));
        let _ = text::paint(
            b,
            r,
            &Text::new(s2, Role::Micro, t.text_dim).caps(false),
            1.0,
        );
    };
    for (f, l) in [(100.0, "100"), (1000.0, "1k"), (10000.0, "10k")] {
        lab(b, Pos2::new(x_of(f, plot) + 12.0, rect.bottom() - 7.0), l);
    }
    if big {
        let names = [
            "kit.eq.sub",
            "kit.eq.bass",
            "kit.eq.low_mid",
            "kit.eq.mid",
            "kit.eq.high_mid",
            "kit.eq.presence",
            "kit.eq.treble",
        ];
        let at = [35.0, 110.0, 350.0, 1100.0, 2800.0, 5500.0, 12000.0];
        for (k, f) in names.iter().zip(at) {
            let r = Rect::from_center_size(
                Pos2::new(x_of(f, plot), rect.top() + 9.0),
                Vec2::new(80.0, 12.0),
            );
            let _ = text::paint(
                b,
                r,
                &Text::key(k, Role::Micro, t.text_dim).align(Align2::CENTER_CENTER),
                1.0,
            );
        }
        for db in [12.0f32, 0.0, -12.0] {
            let l = if db > 0.0 {
                "+12"
            } else if db < 0.0 {
                "−12"
            } else {
                "0"
            };
            let r = Rect::from_min_size(
                Pos2::new(plot.left() + 4.0, y_of(db) - 13.0),
                Vec2::new(26.0, 12.0),
            );
            let _ = text::paint(
                b,
                r,
                &Text::new(l, Role::Micro, t.text_dim)
                    .caps(false)
                    .align(Align2::LEFT_CENTER),
                1.0,
            );
        }
    }
}
