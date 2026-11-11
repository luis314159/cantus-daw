//! The driven signs (round 39, `des-controls`, ADR 0109 drawn): when an automation clip,
//! a modulator, a hardware controller or a VCA drives a value, the control says so with a
//! shape **and** a colour, never colour alone (`docs/round39/design/des-controls/src/signs.py`):
//!
//! | sign | shape | colour |
//! |---|---|---|
//! | automated | a lane: low, a ramp, high; a node at each bend | `auto` |
//! | modulated | one period of a sine | `modulated` |
//! | MIDI learned | a socket: a ring, its pin, a dot | the accent |
//! | VCA | a V | ink 2 |
//!
//! A sign is 1.8 × its height wide. On a knob it sits centred in the groove's open quarter
//! just below the cap (a dot on the small knob); on a fader 7 px left of the cap; on a
//! value bar or a field before the number; anywhere else at the control's upper right.
//! Every sign goes through [`super::automate::signs`] — the one entry point (ADR 0109).

use super::automate::Drivers;
use super::brush::{Brush, Fill};
use super::color::with_alpha;
use super::knob;
use super::Style;
use egui::{Color32, Pos2, Rect, Vec2};

/// What drives a value, as the sign it wears.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Sign {
    Automated,
    Modulated,
    Learned,
    Vca,
}

impl Sign {
    pub const ALL: [Sign; 4] = [Sign::Automated, Sign::Modulated, Sign::Learned, Sign::Vca];

    /// The sign's colour in `s`.
    pub fn color(self, s: &Style) -> Color32 {
        match self {
            Sign::Automated => s.t.auto,
            Sign::Modulated => s.t.modulated,
            Sign::Learned => s.t.focus,
            Sign::Vca => s.t.text_dim,
        }
    }

    /// Whether `d` holds this sign.
    pub fn of(self, d: Drivers) -> bool {
        match self {
            Sign::Automated => d.clip,
            Sign::Modulated => d.modulated,
            Sign::Learned => d.mapped,
            Sign::Vca => d.vca,
        }
    }
}

/// The signs `d` holds, in the order they are drawn.
pub fn of(d: Drivers) -> impl Iterator<Item = Sign> {
    Sign::ALL.into_iter().filter(move |g| g.of(d))
}

/// The strongest sign of `d` (the clip over the modulator over the controller over the VCA).
pub fn first(d: Drivers) -> Option<Sign> {
    of(d).next()
}

/// The stroke and node of a sign `h` physical pixels tall: each size drawn for itself.
fn weights(h_px: f32) -> (f32, f32) {
    if h_px <= 5.5 {
        (1.0, 1.0)
    } else if h_px <= 7.5 {
        (1.25, 1.3)
    } else if h_px <= 10.5 {
        (1.5, 1.6)
    } else {
        (2.0, 2.1)
    }
}

/// Paint `sign` centred on `c`, `h` points tall, in `color`; `halo` knocks it out of what
/// it sits on with a 1-px ring of that colour.
pub fn paint_sign(
    b: &mut Brush,
    c: Pos2,
    h: f32,
    sign: Sign,
    color: Color32,
    halo: Option<Color32>,
) {
    let px = b.px();
    let (sw_px, node_px) = weights(h / px);
    let (sw, nr) = (sw_px * px, node_px * px);
    let w = (h * 1.8 / px).round() * px;
    let c = Pos2::new(b.snap(c.x), b.snap(c.y));
    let (x0, y0) = (c.x - w * 0.5, c.y - h * 0.5);
    match sign {
        Sign::Automated => {
            let pts = [
                Pos2::new(x0, y0 + h - sw * 0.5),
                Pos2::new(x0 + w * 0.26, y0 + h - sw * 0.5),
                Pos2::new(x0 + w * 0.74, y0 + sw * 0.5),
                Pos2::new(x0 + w, y0 + sw * 0.5),
            ];
            if let Some(bg) = halo {
                b.polyline(&pts, sw + 2.0 * px, bg);
                for p in [pts[1], pts[2]] {
                    b.circle(p, nr + px, Fill::Solid(bg));
                }
            }
            b.polyline(&pts, sw, color);
            for p in [pts[1], pts[2]] {
                b.circle(p, nr, Fill::Solid(color));
            }
        }
        Sign::Modulated => {
            let a = h * 0.5 - sw * 0.5;
            let bez = |p0: Pos2, p1: Pos2, p2: Pos2, p3: Pos2, t: f32| {
                let u = 1.0 - t;
                let v = p0.to_vec2() * (u * u * u)
                    + p1.to_vec2() * (3.0 * u * u * t)
                    + p2.to_vec2() * (3.0 * u * t * t)
                    + p3.to_vec2() * (t * t * t);
                v.to_pos2()
            };
            let (top, bot) = (c.y - a * 1.32, c.y + a * 1.32);
            let first = [
                Pos2::new(x0, c.y),
                Pos2::new(x0 + w * 0.18, top),
                Pos2::new(x0 + w * 0.32, top),
                c,
            ];
            let second = [
                c,
                Pos2::new(x0 + w * 0.68, bot),
                Pos2::new(x0 + w * 0.82, bot),
                Pos2::new(x0 + w, c.y),
            ];
            let n = 8;
            let mut pts = [Pos2::ZERO; 17];
            for i in 0..=n {
                let t = i as f32 / n as f32;
                pts[i] = bez(first[0], first[1], first[2], first[3], t);
                pts[n + i] = bez(second[0], second[1], second[2], second[3], t);
            }
            if let Some(bg) = halo {
                b.polyline(&pts, sw + 2.0 * px, bg);
            }
            b.polyline(&pts, sw, color);
        }
        Sign::Learned => {
            let r = h * 0.5 - sw * 0.5;
            if let Some(bg) = halo {
                b.circle(c, r + sw * 0.5 + px, Fill::Solid(bg));
            }
            b.circle_stroke(c, r, sw, color);
            b.circle(c, (h * 0.14).max(0.8 * px), Fill::Solid(color));
            b.segment(
                Pos2::new(c.x, c.y - r - sw * 0.5),
                Pos2::new(c.x, c.y - r + sw * 1.2),
                sw,
                color,
                false,
            );
        }
        Sign::Vca => {
            let hw = h * 0.55;
            let pts = [
                Pos2::new(c.x - hw, y0),
                Pos2::new(c.x, y0 + h),
                Pos2::new(c.x + hw, y0),
            ];
            if let Some(bg) = halo {
                b.polyline(&pts, sw + 2.0 * px, bg);
            }
            b.polyline(&pts, sw, color);
        }
    }
}

/// Paint the signs of `d` in a row centred on `c`, each `h` tall, 2 points apart.
pub fn row(b: &mut Brush, c: Pos2, h: f32, d: Drivers, s: &Style, halo: Option<Color32>) {
    let n = of(d).count();
    if n == 0 {
        return;
    }
    let w = h * 1.8;
    let gap = s.s(2.0);
    let total = w * n as f32 + gap * (n as f32 - 1.0);
    let mut x = c.x - total * 0.5 + w * 0.5;
    for g in of(d) {
        paint_sign(b, Pos2::new(x, c.y), h, g, g.color(s), halo);
        x += w + gap;
    }
}

/// What a kit painter drew, for [`place`] to find (the last one painted on this thread).
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Painted {
    /// A knob centred on `c`: its groove's radius and width, the cap's radius, the
    /// pointer's width and where it starts (a fraction of the cap's radius), the sign's
    /// height, the value and where its arc starts (0…1), whether it is the small knob, and
    /// the signs its painter drew itself.
    Knob {
        c: Pos2,
        groove_r: f32,
        ring: f32,
        cap_r: f32,
        ptr: f32,
        p0: f32,
        sign_h: f32,
        value: f32,
        from: f32,
        mini: bool,
        drew: Drivers,
    },
    /// A fader whose cap is at `cap`.
    Fader { cap: Rect, drew: Drivers },
    /// A value bar or a field whose number starts at `x` on the line `y`.
    Value {
        x: f32,
        y: f32,
        h: f32,
        drew: Drivers,
    },
}

impl Painted {
    fn anchor(&self) -> Pos2 {
        match *self {
            Painted::Knob { c, .. } => c,
            Painted::Fader { cap, .. } => cap.center(),
            Painted::Value { x, y, .. } => Pos2::new(x, y),
        }
    }

    fn drew(&self) -> Drivers {
        match *self {
            Painted::Knob { drew, .. }
            | Painted::Fader { drew, .. }
            | Painted::Value { drew, .. } => drew,
        }
    }
}

/// The signs of `d` on a control at `rect` that [`super::automate::signs`] was asked for,
/// where its shape says (the control `painted` just before, when it lies in `rect`).
pub fn place(b: &mut Brush, rect: Rect, s: &Style, d: Drivers, painted: Option<Painted>) {
    let painted = painted.filter(|p| rect.expand(s.s(2.0)).contains(p.anchor()));
    // What the painter did not draw itself.
    let drew = painted.map_or(Drivers::default(), |p| p.drew());
    let left = Drivers {
        clip: d.clip && !drew.clip,
        modulated: d.modulated && !drew.modulated,
        mapped: d.mapped && !drew.mapped,
        vca: d.vca && !drew.vca,
    };
    if left.none() {
        return;
    }
    match painted {
        Some(Painted::Knob {
            c,
            groove_r,
            ring,
            cap_r,
            ptr,
            p0,
            sign_h,
            value,
            from,
            mini,
            ..
        }) => {
            if left.clip {
                // Painted before anyone said: the arc and the pointer take the clip's colour.
                let (a, z) = (from.min(value), from.max(value));
                if z - a > 0.002 {
                    b.arc(
                        c,
                        groove_r,
                        knob::angle(a),
                        knob::angle(z),
                        ring,
                        s.t.auto,
                        true,
                    );
                }
                let dir = Vec2::angled(knob::angle(value));
                b.segment(
                    c + dir * (cap_r * p0),
                    c + dir * (cap_r - ptr * 0.5 - s.s(1.5)),
                    ptr,
                    s.t.auto,
                    true,
                );
            }
            knob_signs(b, c, (groove_r, ring, cap_r), sign_h, mini, left, s);
        }
        Some(Painted::Fader { cap, .. }) => fader_signs(b, cap, left, s),
        Some(Painted::Value { x, y, h, .. }) => {
            let w = h * 1.8;
            let mut cx = x - s.s(4.0) - w * 0.5;
            for g in of(left) {
                paint_sign(b, Pos2::new(cx, y), h, g, g.color(s), None);
                cx -= w + s.s(3.0);
            }
        }
        None => {
            // A control the kit did not draw: its signs at its upper right, small, knocked
            // out of whatever is under them.
            let h = s.s(5.0);
            let w = h * 1.8;
            let mut cx = rect.right() - s.s(2.0) - w * 0.5;
            let cy = rect.top() + s.s(2.0) + h * 0.5;
            let halo = Some(with_alpha(s.t.panel, 1.0));
            for g in of(left) {
                paint_sign(b, Pos2::new(cx, cy), h, g, g.color(s), halo);
                cx -= w + s.s(3.0);
            }
        }
    }
}

/// A knob's signs in its groove's open quarter (a dot on the small knob, in the strongest
/// sign's colour).
pub fn knob_signs(
    b: &mut Brush,
    c: Pos2,
    (groove_r, _ring, cap_r): (f32, f32, f32),
    sign_h: f32,
    mini: bool,
    d: Drivers,
    s: &Style,
) {
    let Some(g) = first(d) else {
        return;
    };
    if mini {
        let at = Pos2::new(c.x, c.y + groove_r + s.s(0.5));
        b.circle(at, sign_h * 0.5, Fill::Solid(g.color(s)));
        return;
    }
    let y = c.y + cap_r + s.s(1.5) + sign_h * 0.5;
    row(b, Pos2::new(c.x, y), sign_h, d, s, None);
}

/// A fader's signs 7 points left of its cap, one under the other.
pub fn fader_signs(b: &mut Brush, cap: Rect, d: Drivers, s: &Style) {
    let h = s.s(6.0);
    let w = h * 1.8;
    let cx = cap.left() - s.s(7.0) - w * 0.5;
    let mut y = cap.center().y;
    for g in of(d) {
        paint_sign(b, Pos2::new(cx, y), h, g, g.color(s), Some(s.t.strip));
        y += h + s.s(3.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::knob::{KnobLook, KnobSize};
    use crate::kit::{gallery, State};

    fn night() -> Style {
        Style::new(crate::theme_slate::slate_night_tokens())
    }

    /// Where the sign inked: a mask of the pixels it touched.
    fn mask(g: Sign, h: f32) -> Vec<bool> {
        let s = night();
        let c = gallery::render(Vec2::new(32.0, 32.0), 1.0, |b, _| {
            paint_sign(b, Pos2::new(16.0, 16.0), h, g, s.t.text, None)
        });
        (0..32 * 32)
            .map(|i| c.pixel(i % 32, i / 32)[0] > 40)
            .collect()
    }

    /// Never colour alone (`des-controls`): every sign has a shape of its own at every
    /// height — drawn in one ink, no two masks agree.
    #[test]
    fn every_sign_is_a_shape_of_its_own_at_every_height() {
        for h in [5.0, 7.0, 9.0, 12.0] {
            let masks: Vec<Vec<bool>> = Sign::ALL.iter().map(|g| mask(*g, h)).collect();
            for (i, a) in masks.iter().enumerate() {
                assert!(
                    a.iter().any(|p| *p),
                    "{:?} at {h} inks nothing",
                    Sign::ALL[i]
                );
                for (j, b) in masks.iter().enumerate().skip(i + 1) {
                    let differ = a.iter().zip(b).filter(|(x, y)| x != y).count();
                    assert!(
                        differ >= 4,
                        "{:?} and {:?} look alike at {h}",
                        Sign::ALL[i],
                        Sign::ALL[j]
                    );
                }
            }
        }
    }

    /// The number of pixels within a few steps of `c`.
    fn count_near(canvas: &crate::raster::Canvas, c: egui::Color32) -> usize {
        let mut n = 0;
        for y in 0..canvas.height {
            for x in 0..canvas.width {
                let p = canvas.pixel(x, y);
                let d = (p[0] as i32 - c.r() as i32).abs()
                    + (p[1] as i32 - c.g() as i32).abs()
                    + (p[2] as i32 - c.b() as i32).abs();
                if d < 24 {
                    n += 1;
                }
            }
        }
        n
    }

    /// An automated knob turns its arc, its pointer and its sign the clip's colour — told
    /// before it is painted (`automate::driving`) or after (`automate::signs` over a knob a
    /// view painted itself); a knob nothing drives shows none of it.
    #[test]
    fn an_automated_knob_wears_the_clips_colour_however_it_was_told() {
        let s = night();
        let look = KnobLook::new(KnobSize::M, 0.7, 0.0);
        let clip = Drivers {
            clip: true,
            ..Drivers::default()
        };
        let area = Rect::from_center_size(Pos2::new(30.0, 30.0), Vec2::splat(44.0));
        let draw = |before: Drivers, after: Drivers| {
            gallery::render(Vec2::new(60.0, 60.0), 1.0, |b, _| {
                crate::kit::automate::driving(before);
                crate::kit::knob::paint(b, Pos2::new(30.0, 30.0), &s, State::REST, &look);
                crate::kit::automate::signs(b, area, &s, after);
            })
        };
        let none = count_near(&draw(Drivers::default(), Drivers::default()), s.t.auto);
        let told = count_near(&draw(clip, clip), s.t.auto);
        let late = count_near(&draw(Drivers::default(), clip), s.t.auto);
        assert_eq!(none, 0, "a knob nothing drives wears no violet");
        assert!(told > 40, "told before: {told} violet pixels");
        assert!(late > 40, "told after: {late} violet pixels");
        assert_eq!(
            crate::kit::automate::current_driving(),
            Drivers::default(),
            "signs() ends the driving"
        );
    }
}
