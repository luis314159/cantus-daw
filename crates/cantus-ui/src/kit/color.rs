//! Colour arithmetic for the kit: HSL in gamma space, WCAG contrast, the lighting
//! recipe's shades, and the safe band for track colours (ADR 0024 §2).
//!
//! Everything here is plain `f32` math on [`Color32`]: no allocation, no tables, cheap
//! enough to run inside a painter. HSL (not OKLCH) because FL's theme and the spec in
//! `research/ui/fl-studio.md` §12 are written in it, and the ladder it shifts is nearly
//! grey, where the two spaces agree.

use egui::Color32;

/// Hue (degrees, 0…360), saturation and lightness (0…1) of an opaque colour.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Hsl {
    pub h: f32,
    pub s: f32,
    pub l: f32,
}

impl Hsl {
    pub fn of(c: Color32) -> Hsl {
        let [r, g, b, _] = c.to_srgba_unmultiplied();
        let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) * 0.5;
        let d = max - min;
        if d < 1e-6 {
            return Hsl { h: 0.0, s: 0.0, l };
        }
        let s = if l > 0.5 {
            d / (2.0 - max - min)
        } else {
            d / (max + min)
        };
        let h = if max == r {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if max == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        Hsl { h: h * 60.0, s, l }
    }

    /// Back to an opaque colour.
    pub fn rgb(self) -> Color32 {
        let (h, s, l) = (
            self.h.rem_euclid(360.0) / 360.0,
            self.s.clamp(0.0, 1.0),
            self.l.clamp(0.0, 1.0),
        );
        if s <= 0.0 {
            let v = (l * 255.0).round() as u8;
            return Color32::from_rgb(v, v, v);
        }
        let q = if l < 0.5 {
            l * (1.0 + s)
        } else {
            l + s - l * s
        };
        let p = 2.0 * l - q;
        let ch = |mut t: f32| {
            if t < 0.0 {
                t += 1.0;
            }
            if t > 1.0 {
                t -= 1.0;
            }
            let v = if t < 1.0 / 6.0 {
                p + (q - p) * 6.0 * t
            } else if t < 0.5 {
                q
            } else if t < 2.0 / 3.0 {
                p + (q - p) * (2.0 / 3.0 - t) * 6.0
            } else {
                p
            };
            (v * 255.0).round().clamp(0.0, 255.0) as u8
        };
        Color32::from_rgb(ch(h + 1.0 / 3.0), ch(h), ch(h - 1.0 / 3.0))
    }
}

/// `c` with its HSL lightness moved by `dl` (−1…1, so `0.06` is "+6 % L").
///
/// This is the one way the lighting recipe makes a colour lighter or darker, so a lit
/// part and a plain one step by the same amounts.
pub fn shade(c: Color32, dl: f32) -> Color32 {
    let a = c.a();
    let mut hsl = Hsl::of(opaque(c));
    hsl.l = (hsl.l + dl).clamp(0.0, 1.0);
    with_alpha(hsl.rgb(), a as f32 / 255.0)
}

/// The colour without its alpha (un-premultiplied), for arithmetic on its hue.
pub fn opaque(c: Color32) -> Color32 {
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    Color32::from_rgb(r, g, b)
}

/// An opaque colour at `alpha` (0…1), premultiplied as epaint wants it.
pub fn with_alpha(c: Color32, alpha: f32) -> Color32 {
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(r, g, b, (alpha.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// Linear interpolation in gamma space, `t` = 0 → `a`, 1 → `b`.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    a.lerp_to_gamma(b, t.clamp(0.0, 1.0))
}

/// Relative luminance, WCAG 2.1.
pub fn luminance(c: Color32) -> f32 {
    let ch = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let c = opaque(c);
    0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
}

/// WCAG contrast ratio of two opaque colours (1…21).
pub fn contrast(a: Color32, b: Color32) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// `fg` with its lightness pushed away from `bg`, one percent at a time, until the pair
/// reaches `need`:1. The floor under every derived text colour — FL's own tints land at
/// 3.2–4.1:1, Cantus's must not (`research/ui/fl-studio.md` §12.1).
pub fn ensure_contrast(fg: Color32, bg: Color32, need: f32) -> Color32 {
    ensure_contrast_all(fg, &[bg], need)
}

/// `fg` moved in lightness (hue and saturation kept) until it reaches `need`:1 against
/// **every** colour in `bgs`: first away from them, then the other way if that cannot
/// get there; failing both, the lightness that comes closest.
pub fn ensure_contrast_all(fg: Color32, bgs: &[Color32], need: f32) -> Color32 {
    let worst = |c: Color32| bgs.iter().map(|b| contrast(c, *b)).fold(f32::MAX, f32::min);
    if worst(fg) >= need {
        return fg;
    }
    let start = Hsl::of(opaque(fg));
    let mean_bg = bgs.iter().map(|b| luminance(*b)).sum::<f32>() / bgs.len().max(1) as f32;
    let first_up = luminance(fg) >= mean_bg;
    let (mut best, mut best_score) = (fg, worst(fg));
    for up in [first_up, !first_up] {
        let mut h = start;
        for _ in 0..101 {
            h.l = (h.l + if up { 0.01 } else { -0.01 }).clamp(0.0, 1.0);
            let c = h.rgb();
            let w = worst(c);
            if w >= need {
                return c;
            }
            if w > best_score {
                best = c;
                best_score = w;
            }
            if h.l <= 0.0 || h.l >= 1.0 {
                break;
            }
        }
    }
    best
}

/// Text for a coloured fill: the fill's own hue, pale on a dark fill and deep on a light
/// one, then pushed to 4.5:1 (FL's rule 5: "text on a coloured item is a tint of that
/// colour, not white").
pub fn on_fill(fill: Color32) -> Color32 {
    let hsl = Hsl::of(opaque(fill));
    let dark_fill = luminance(fill) < 0.2;
    let tint = Hsl {
        h: hsl.h,
        s: (hsl.s * 1.6).clamp(0.25, 0.8),
        l: if dark_fill { 0.88 } else { 0.18 },
    }
    .rgb();
    ensure_contrast(tint, fill, 4.5)
}

/// How big a coloured fill is, which decides how light a track colour may be.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FillSize {
    /// Headers, mixer number cells, chips: FL's pastel band.
    Small,
    /// Clips, lane headers, whole-strip tints: the same hues, darker.
    Large,
}

/// The "safe colours" clamp (FL's *Lock to safe colours*): the colour's hue kept, its
/// saturation and lightness narrowed to the band that lets every track sit at one
/// lightness, so no track shouts (`fl-studio.md` §1.3, §12.1–12.2).
///
/// Near-greys keep their low saturation: a grey the user chose stays grey.
pub fn safe_track(c: Color32, dark_theme: bool, size: FillSize) -> Color32 {
    let hsl = Hsl::of(opaque(c));
    let (s_band, l_band) = match (dark_theme, size) {
        (true, FillSize::Small) => ((0.25, 0.40), (0.58, 0.64)),
        (true, FillSize::Large) => ((0.25, 0.40), (0.36, 0.46)),
        (false, FillSize::Small) => ((0.35, 0.45), (0.50, 0.58)),
        (false, FillSize::Large) => ((0.30, 0.42), (0.62, 0.72)),
    };
    let s = if hsl.s < 0.08 {
        hsl.s
    } else {
        hsl.s.clamp(s_band.0, s_band.1)
    };
    Hsl {
        h: hsl.h,
        s,
        l: hsl.l.clamp(l_band.0, l_band.1),
    }
    .rgb()
}

/// Whether `c` already sits in the safe band (the picker shows both side by side).
pub fn is_safe_track(c: Color32, dark_theme: bool, size: FillSize) -> bool {
    let s = safe_track(c, dark_theme, size);
    let d = |a: u8, b: u8| a.abs_diff(b) <= 2;
    d(s.r(), c.r()) && d(s.g(), c.g()) && d(s.b(), c.b())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsl_round_trips() {
        for c in [
            Color32::from_rgb(0x1f, 0x28, 0x2d),
            Color32::from_rgb(0xc3, 0xff, 0x5a),
            Color32::from_rgb(0xff, 0x61, 0x35),
            Color32::from_rgb(0x80, 0x80, 0x80),
            Color32::WHITE,
            Color32::BLACK,
        ] {
            let back = Hsl::of(c).rgb();
            assert!(
                back.r().abs_diff(c.r()) <= 1
                    && back.g().abs_diff(c.g()) <= 1
                    && back.b().abs_diff(c.b()) <= 1,
                "{c:?} came back as {back:?}"
            );
        }
        // FL's measured strip: 202° / 10 % / 30 %.
        let s = Hsl::of(Color32::from_rgb(0x45, 0x4f, 0x55));
        assert!((s.h - 202.0).abs() < 2.0 && (s.l - 0.30).abs() < 0.01);
    }

    #[test]
    fn contrast_matches_wcag() {
        assert!((contrast(Color32::WHITE, Color32::BLACK) - 21.0).abs() < 0.01);
        // Measured in the FL report: lime on the strip is about 7:1.
        let lime = Color32::from_rgb(0xc3, 0xff, 0x5a);
        let strip = Color32::from_rgb(0x45, 0x4f, 0x55);
        assert!(contrast(lime, strip) > 6.5);
    }

    #[test]
    fn ensure_contrast_reaches_the_floor_either_way() {
        let dark = Color32::from_rgb(0x45, 0x4f, 0x55);
        let light = Color32::from_rgb(0xdf, 0xe4, 0xe7);
        let grey = Color32::from_rgb(0x80, 0x88, 0x8c);
        assert!(contrast(ensure_contrast(grey, dark, 4.5), dark) >= 4.5);
        assert!(contrast(ensure_contrast(grey, light, 4.5), light) >= 4.5);
    }

    #[test]
    fn text_on_a_fill_is_its_own_tint_and_legible() {
        for fill in [
            Color32::from_rgb(0x40, 0x78, 0x40),
            Color32::from_rgb(0xbb, 0x7f, 0xc4),
            Color32::from_rgb(0xa4, 0x4a, 0x48),
            Color32::from_rgb(0xdf, 0xe4, 0xe7),
        ] {
            let t = on_fill(fill);
            assert!(contrast(t, fill) >= 4.5, "{t:?} on {fill:?}");
        }
        // FL's green header carries a green-tinted name, not white.
        let t = Hsl::of(on_fill(Color32::from_rgb(0x40, 0x78, 0x40)));
        assert!((t.h - 120.0).abs() < 15.0, "hue {}", t.h);
    }

    #[test]
    fn the_safe_clamp_lands_in_fl_s_band() {
        let loud = Color32::from_rgb(0xff, 0x00, 0x40);
        let s = Hsl::of(safe_track(loud, true, FillSize::Small));
        assert!((0.24..=0.41).contains(&s.s) && (0.57..=0.65).contains(&s.l));
        let big = Hsl::of(safe_track(loud, true, FillSize::Large));
        assert!(big.l < s.l, "large fills are darker");
        // A grey stays grey.
        let g = Hsl::of(safe_track(
            Color32::from_rgb(0x70, 0x70, 0x70),
            true,
            FillSize::Small,
        ));
        assert!(g.s < 0.05);
        // Idempotent, and the picker can tell a safe colour from an unsafe one.
        let once = safe_track(loud, false, FillSize::Small);
        assert!(is_safe_track(once, false, FillSize::Small));
        assert!(!is_safe_track(loud, false, FillSize::Small));
    }

    #[test]
    fn shade_moves_lightness_only() {
        let base = Color32::from_rgb(0x45, 0x4f, 0x55);
        let up = Hsl::of(shade(base, 0.06));
        let b = Hsl::of(base);
        assert!((up.l - b.l - 0.06).abs() < 0.01);
        assert!((up.h - b.h).abs() < 3.0);
    }
}
