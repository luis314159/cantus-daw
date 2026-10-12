//! The Cantus **mark** (ADR 0046): the C of waves and the playhead with no tile behind
//! them — the website's favicon (`web/assets/favicon.svg`), drawn as data so it can be
//! rendered at any size and in any colour. It is the window's and the taskbar's icon and
//! the `.exe`'s and the installer's.
//!
//! In a file of its own with no dependency beyond `std`, like `logo_small.rs`, so the
//! application's build script can include it (`#[path]`) and bake the `.exe`'s icon from
//! the same drawing the running application recolours.
//!
//! **Sizes.** At 40 px and up it is the favicon exactly: eleven spokes of a wave round an
//! open C, the playhead with its crossbar. Below that the spokes would melt into a ring, so
//! the small sizes are redrawn for the pixel grid: below 24 px the C is one solid arc
//! carrying the wave's gradient, and below 40 px the playhead's line and bar sit on whole
//! pixels.
//!
//! **Colour.** The spokes run a three-stop gradient round the C (pale at the bottom, deep
//! at the top) and the playhead is one colour: [`MarkColours`]. The original is the logo's
//! amber → copper; any other is made from one colour by [`MarkColours::from_one`].

/// The mark's colours: the wave's three stops (bottom, middle, top of the C) and the
/// playhead's.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct MarkColours {
    pub wave: [[u8; 3]; 3],
    pub playhead: [u8; 3],
}

impl MarkColours {
    /// As designed: amber at the bottom of the C, copper at the top, an amber playhead.
    pub const ORIGINAL: MarkColours = MarkColours {
        wave: [[0xff, 0xd9, 0x8a], [0xeb, 0x80, 0x30], [0xbc, 0x40, 0x10]],
        playhead: [0xff, 0xd2, 0x5e],
    };

    /// A mark in one colour: the wave from a lighter tint of it to a deeper shade, the
    /// playhead a light tint — the original's shape of light, in another hue.
    pub fn from_one(c: [u8; 3]) -> MarkColours {
        let (h, s, l) = hsl(c);
        let at = |dl: f32, ds: f32| rgb(h, (s + ds).clamp(0.0, 1.0), (l + dl).clamp(0.04, 0.96));
        MarkColours {
            wave: [at(0.2, -0.05), c, at(-0.16, 0.05)],
            playhead: at(0.16, 0.0),
        }
    }
}

/// The spokes as the favicon draws them, in its 64-unit box: the angle of each round the
/// C's centre (degrees, clockwise from three o'clock) and its outer radius (the wave).
const SPOKES: [(f32, f32); 11] = [
    (38.1, 19.0),
    (66.5, 22.3),
    (94.9, 24.3),
    (123.3, 24.35),
    (151.7, 22.95),
    (180.0, 25.2),
    (208.3, 25.8),
    (236.7, 24.35),
    (265.1, 21.5),
    (293.5, 22.3),
    (321.9, 21.8),
];

/// The C's centre and the spokes' inner radius, in the 64-unit box.
const CENTRE: (f32, f32) = (30.0, 32.0);
const INNER: f32 = 10.0;

/// The SVG of the mark drawn for `px` pixels (see the module docs for the small sizes).
pub fn mark_svg(px: u32, c: &MarkColours) -> String {
    let n = px.max(8) as f32;
    let u = n / 64.0;
    let (cx, cy) = (CENTRE.0 * u, CENTRE.1 * u);
    let mut body = String::new();
    if px < 24 {
        // A solid C: one arc through the middle of the spokes, the wave's gradient run
        // from the bottom of the C to its top.
        let rm = (INNER + 22.0) * 0.5 * u;
        let w = ((22.0 - INNER) * 0.62 * u).max(2.0);
        let (a0, a1) = (SPOKES[0].0.to_radians(), SPOKES[10].0.to_radians());
        let (x0, y0) = (cx + rm * a0.cos(), cy + rm * a0.sin());
        let (x1, y1) = (cx + rm * a1.cos(), cy + rm * a1.sin());
        body.push_str(&format!(
            r##"<defs><linearGradient id="w" gradientUnits="userSpaceOnUse" x1="0" y1="{y0b:.2}" x2="0" y2="{y1b:.2}"><stop offset="0" stop-color="{s0}" data-role="wave"/><stop offset="0.5" stop-color="{s1}" data-role="wave"/><stop offset="1" stop-color="{s2}" data-role="wave"/></linearGradient></defs>
<path d="M{x0:.2},{y0:.2} A{rm:.2},{rm:.2} 0 1 1 {x1:.2},{y1:.2}" fill="none" stroke="url(#w)" stroke-width="{w:.2}" stroke-linecap="round"/>
"##,
            y0b = cy + rm,
            y1b = cy - rm,
            s0 = hex(c.wave[0]),
            s1 = hex(c.wave[1]),
            s2 = hex(c.wave[2]),
        ));
    } else {
        let width = (5.6 * u).max(1.8);
        let last = (SPOKES.len() - 1) as f32;
        for (i, &(deg, outer)) in SPOKES.iter().enumerate() {
            let a = deg.to_radians();
            let (x0, y0) = (cx + INNER * u * a.cos(), cy + INNER * u * a.sin());
            let (x1, y1) = (cx + outer * u * a.cos(), cy + outer * u * a.sin());
            let colour = gradient(&c.wave, i as f32 / last);
            body.push_str(&format!(
                r##"<line x1="{x0:.2}" y1="{y0:.2}" x2="{x1:.2}" y2="{y1:.2}" stroke="{}" stroke-width="{width:.2}" stroke-linecap="round" data-role="wave"/>"##,
                hex(colour)
            ));
            body.push('\n');
        }
    }
    // The playhead: a line down the right edge and a crossbar near its top. On the pixel
    // grid below 40 px: whole-pixel widths, the line on a whole column.
    let ph = hex(c.playhead);
    let (lx, lw, top, bottom, bx, bw, by, bh, r) = if px >= 40 {
        (
            54.6 * u,
            6.0 * u,
            3.5 * u,
            60.5 * u,
            47.6 * u,
            16.0 * u,
            8.0 * u,
            5.4 * u,
            3.0 * u,
        )
    } else {
        let lw = (6.0 * u).round().max(2.0);
        let lx = (n - lw - (n / 32.0).round()).max(0.0);
        // The bar reaches further left of the line than right of it, as designed, and is
        // as thick as the line at 16 px so it still reads as a cross.
        let bx = (lx - (7.0 * u).round().max(2.0)).max(0.0);
        let bw = (lx + lw + (3.0 * u).round().max(1.0)).min(n) - bx;
        let bh = (5.4 * u).round().max(lw.min(2.0));
        let by = (8.0 * u).round().max(1.0);
        (
            lx,
            lw,
            (3.5 * u).floor(),
            n - (3.5 * u).floor(),
            bx,
            bw,
            by,
            bh,
            0.0,
        )
    };
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {n} {n}" width="{n}" height="{n}">
{body}<rect x="{lx:.2}" y="{top:.2}" width="{lw:.2}" height="{h:.2}" rx="{rl:.2}" fill="{ph}" data-role="playhead"/>
<rect x="{bx:.2}" y="{by:.2}" width="{bw:.2}" height="{bh:.2}" rx="{rb:.2}" fill="{ph}" data-role="playhead"/>
</svg>"##,
        h = bottom - top,
        rl = r.min(lw * 0.5),
        rb = (bh * 0.5).min(r.max(0.0) * 0.9),
    )
}

/// The colour `t` (0…1) of the way along three stops.
fn gradient(stops: &[[u8; 3]; 3], t: f32) -> [u8; 3] {
    let (a, b, f) = if t <= 0.5 {
        (stops[0], stops[1], t * 2.0)
    } else {
        (stops[1], stops[2], (t - 0.5) * 2.0)
    };
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * f).round() as u8;
    [mix(a[0], b[0]), mix(a[1], b[1]), mix(a[2], b[2])]
}

pub fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// Hue (degrees), saturation and lightness of an sRGB colour.
fn hsl(c: [u8; 3]) -> (f32, f32, f32) {
    let [r, g, b] = c.map(|v| v as f32 / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) * 0.5;
    let d = max - min;
    if d < 1e-6 {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        60.0 * (((g - b) / d).rem_euclid(6.0))
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    (h, s, l)
}

fn rgb(h: f32, s: f32, l: f32) -> [u8; 3] {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c * 0.5;
    [r, g, b].map(|v| ((v + m).clamp(0.0, 1.0) * 255.0).round() as u8)
}
