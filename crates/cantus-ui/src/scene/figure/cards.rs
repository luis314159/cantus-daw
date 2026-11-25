//! r42:arcus-upper — the realistic figures' cards (ADR 0159; the owner's review,
//! 2026-11-24: hair, brows and lashes that read as hair, not paint): MakeHuman's CC0 hair,
//! brow and lash proxies are alpha-tested cards in the Photographic bake. Their textures
//! (`assets/figures/<who>_<hair|brow|lash>.png`, grey and alpha, made by
//! `tools/arcus-figures/build_figures.py`) are decoded once, the first time a picture with
//! a realistic figure is baked, and kept. A card is `who × 3 + kind` (hair 0, brows 1,
//! lashes 2).

use std::sync::OnceLock;

static PNGS: [&[u8]; 6] = [
    include_bytes!("../../../assets/figures/woman_hair.png"),
    include_bytes!("../../../assets/figures/woman_brow.png"),
    include_bytes!("../../../assets/figures/woman_lash.png"),
    include_bytes!("../../../assets/figures/man_hair.png"),
    include_bytes!("../../../assets/figures/man_brow.png"),
    include_bytes!("../../../assets/figures/man_lash.png"),
];

/// One level of a card's texture: its side and each texel's `[grey, coverage, way's cos 2θ,
/// way's sin 2θ]` (0…255; the way 128 where it has none).
pub struct Level {
    pub w: usize,
    pub h: usize,
    pub texels: Vec<[u8; 4]>,
}

/// A card's texture: its levels (the first the texture as stored, each next half its side:
/// a strand pattern finer than a sample is taken as its coverage and mean, not sampled at
/// random — the owner's review of sheet v8: hair as noise and camouflage).
pub struct Card {
    pub w: usize,
    pub h: usize,
    pub levels: Vec<Level>,
    /// r42:arcus-hair — a hair card's maps (`tools/arcus-figures/hair_maps.py`): the grey is
    /// the strands' own detail over 128 (the texture's large-scale light taken away), the
    /// way the strands run is kept; a brow's or a lash's is grey and coverage.
    pub hair: bool,
    /// The mean grey where it covers (its strands' light is taken against it, so a card's
    /// colour is the figure's hair colour on average, as the scalp under it is).
    pub mean: f32,
}

fn decode(bytes: &[u8], mips: bool) -> Option<Card> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (frame.width as usize, frame.height as usize);
    let (ch, hair) = match frame.color_type {
        png::ColorType::GrayscaleAlpha => (2, false),
        png::ColorType::Rgba => (4, true),
        png::ColorType::Grayscale => (1, false),
        png::ColorType::Rgb => (3, false),
        _ => return None,
    };
    let texels = buf[..w * h * ch]
        .chunks(ch)
        .map(|c| match ch {
            2 => [c[0], c[1], 128, 128],
            4 => [c[0], c[3], c[1], c[2]],
            1 => [c[0], 255, 128, 128],
            _ => [
                ((c[0] as u16 + c[1] as u16 + c[2] as u16) / 3) as u8,
                255,
                128,
                128,
            ],
        })
        .collect::<Vec<[u8; 4]>>();
    let (sum, n) = texels
        .iter()
        .filter(|t| t[1] >= 128)
        .fold((0.0f32, 0usize), |(s, n), t| (s + t[0] as f32, n + 1));
    let mean = if n > 0 { sum / n as f32 } else { 128.0 }.max(8.0);
    let mut levels = vec![Level { w, h, texels }];
    while let Some(l) = levels.last().filter(|l| mips && l.w >= 32 && l.h >= 32) {
        let next = half(l);
        levels.push(next);
    }
    Some(Card {
        w,
        h,
        levels,
        hair,
        mean,
    })
}

/// A level at half the side: coverage averaged; the grey and the way averaged over what
/// covers (a strand's colour is not thinned by the gaps beside it).
fn half(l: &Level) -> Level {
    let (w, h) = (l.w / 2, l.h / 2);
    let mut texels = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let q = [
                l.texels[2 * y * l.w + 2 * x],
                l.texels[2 * y * l.w + 2 * x + 1],
                l.texels[(2 * y + 1) * l.w + 2 * x],
                l.texels[(2 * y + 1) * l.w + 2 * x + 1],
            ];
            let a: u32 = q.iter().map(|t| t[1] as u32).sum();
            let wsum = q.iter().map(|t| t[1] as u32 + 1).sum::<u32>();
            let avg = |k: usize| {
                let s: u32 = q.iter().map(|t| t[k] as u32 * (t[1] as u32 + 1)).sum();
                ((s + wsum / 2) / wsum) as u8
            };
            texels.push([avg(0), ((a + 2) / 4) as u8, avg(2), avg(3)]);
        }
    }
    Level { w, h, texels }
}

static SKINS: [&[u8]; 2] = [
    include_bytes!("../../../assets/figures/woman_skin.png"),
    include_bytes!("../../../assets/figures/man_skin.png"),
];

/// The skin's fine detail at texture coordinates (`u` 0…1 hers, 2…3 his): the MakeHuman
/// skin's luminance over its own blur (pores, the brows' hairs, the lips' lines; about
/// 0.6 … 1.4), bilinear.
pub fn skin_detail(u: f32, v: f32) -> f32 {
    // (Kept as one grey byte a texel: a skin is never a card.)
    static TEX: OnceLock<Vec<Option<(usize, usize, Vec<u8>)>>> = OnceLock::new();
    let t = TEX.get_or_init(|| {
        SKINS
            .iter()
            .map(|b| {
                let c = decode(b, false)?;
                Some((c.w, c.h, c.levels[0].texels.iter().map(|t| t[0]).collect()))
            })
            .collect()
    });
    let (k, u) = if u >= 2.0 { (1, u - 2.0) } else { (0, u) };
    let Some(Some((w, h, grey))) = t.get(k) else {
        return 1.0;
    };
    let (w, h) = (*w, *h);
    let x = (u.clamp(0.0, 1.0) * w as f32 - 0.5).max(0.0);
    let y = ((1.0 - v.clamp(0.0, 1.0)) * h as f32 - 0.5).max(0.0);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (x0, y0) = (x0.min(w - 1), y0.min(h - 1));
    let (tx, ty) = (x - x0 as f32, y - y0 as f32);
    let g = |x: usize, y: usize| grey[y * w + x] as f32;
    let a = g(x0, y0) * (1.0 - tx) + g(x1, y0) * tx;
    let b = g(x0, y1) * (1.0 - tx) + g(x1, y1) * tx;
    ((a * (1.0 - ty) + b * ty) / 128.0).clamp(0.6, 1.4)
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// r42:arcus-hair — how much of a hair card's coverage stands at `line` degrees in from
/// figure `who`'s hairline (0 her, 1 him): the strands thin out over the front edge of the
/// hair, so it meets the forehead softly (the owner's review of sheet v8: a hard edge).
pub fn feather(who: u8, line: f32) -> f32 {
    if who == 0 {
        smooth(-6.0, 2.0, line)
    } else {
        // (His short hair's edge is its cards' own, as it read well on sheet v8.)
        1.0
    }
}

/// r42:arcus-hair — a strand's coverage `alpha` where the hair keeps `fade` of itself: the
/// fainter parts of the strands go first and the cores last (multiplied, every strand
/// faded alike and the coverage's dither read as a sand of dots at 200 %). The texture's
/// soft edges are drawn in toward the strands' cores (a strand's faint fringe over the
/// skin, a quarter of a pixel's samples, read as a dot).
pub fn thin(alpha: f32, fade: f32) -> f32 {
    let alpha = smooth(0.2, 0.8, alpha);
    if fade >= 1.0 {
        return alpha;
    }
    ((alpha - (1.0 - fade)) / fade.max(0.05)).clamp(0.0, 1.0)
}

/// r42:arcus-hair — how a figure's head of hair takes the light in the Photographic bake.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct HairLook {
    /// Its albedo against the Drawn picture's colour (seen per pixel under the key her
    /// blonde read near white, its strands and its sheen clipped away).
    pub albedo: f32,
    /// The narrow, near white highlight along the strands: its sharpness and strength.
    pub shine: (f32, f32),
    /// The broad one tinted by the hair.
    pub sheen: (f32, f32),
    /// How much of the strands' own detail shows (1 the maps' as stored).
    pub detail: f32,
    /// The scalp's colour under the cards against the Drawn picture's hair colour: the
    /// deeper hair in the gaps between the cards (his short cards leave wide ones, and a
    /// scalp as light as his hair read as pale patches over the crown).
    pub scalp: f32,
}

/// `who`'s (0 her, 1 him) hair. His short hair runs every way (its curls' way changes
/// from strand to strand) and is dark (the key's highlight dwarfs its diffuse): a sheen
/// that suits her blonde read on him as pale glossy blotches, so his stays faint.
pub fn hair_look(who: u8) -> HairLook {
    if who == 0 {
        HairLook {
            albedo: 0.6,
            shine: (60.0, 0.3),
            sheen: (10.0, 0.25),
            detail: 1.0,
            scalp: 0.57,
        }
    } else {
        HairLook {
            albedo: 1.0,
            shine: (60.0, 0.004),
            sheen: (12.0, 0.006),
            detail: 0.35,
            scalp: 0.55,
        }
    }
}

/// r42:arcus-hair — how much of the scalp under the cards stands at `line` degrees in from
/// `who`'s hairline, `across` its strands' across coordinate (the angle round the head, × 9):
/// it thins out at its edge, unevenly along it as a hairline does, so the skin shows
/// between the feathered strands and no band of scalp is drawn.
pub fn scalp_cover(who: u8, line: f32, across: f32) -> f32 {
    if who == 0 {
        let wave = (across * 7.0).sin() * 0.6 + (across * 2.3 * 7.0).sin() * 0.4;
        // (Over six degrees, about two millimetres a degree: a narrower ramp read as a
        // jagged staircase at 200 %, the coordinator's review.)
        smooth(-3.5, 2.5, line + wave)
    } else {
        1.0
    }
}

static EYES: [&[u8]; 2] = [
    include_bytes!("../../../assets/figures/woman_eye.png"),
    include_bytes!("../../../assets/figures/man_eye.png"),
];

/// r42:arcus-hair — an eye's albedo (linear) at its texture coordinates (`u` 0…1 hers, 2…3
/// his): MakeHuman's CC0 iris, its pupil and the white round it, bilinear; outside the
/// crop the white at its edge.
pub fn eye(uv: [f32; 2]) -> [f32; 3] {
    static TEX: OnceLock<Vec<Option<(usize, usize, Vec<[f32; 3]>)>>> = OnceLock::new();
    let t = TEX.get_or_init(|| {
        EYES.iter()
            .map(|b| {
                let mut dec = png::Decoder::new(std::io::Cursor::new(*b));
                dec.set_transformations(png::Transformations::normalize_to_color8());
                let mut reader = dec.read_info().ok()?;
                let mut buf = vec![0u8; reader.output_buffer_size()?];
                let f = reader.next_frame(&mut buf).ok()?;
                let ch = match f.color_type {
                    png::ColorType::Rgb => 3,
                    png::ColorType::Rgba => 4,
                    _ => return None,
                };
                let lin = |c: u8| (c as f32 / 255.0).powf(2.2);
                let px = buf[..f.width as usize * f.height as usize * ch]
                    .chunks(ch)
                    .map(|c| [lin(c[0]), lin(c[1]), lin(c[2])])
                    .collect();
                Some((f.width as usize, f.height as usize, px))
            })
            .collect()
    });
    let (k, u) = if uv[0] >= 1.5 {
        (1, uv[0] - 2.0)
    } else {
        (0, uv[0])
    };
    let Some(Some((w, h, px))) = t.get(k) else {
        return [0.6; 3];
    };
    let (w, h) = (*w, *h);
    let x = (u.clamp(0.0, 1.0) * w as f32 - 0.5).clamp(0.0, (w - 1) as f32);
    let y = ((1.0 - uv[1].clamp(0.0, 1.0)) * h as f32 - 0.5).clamp(0.0, (h - 1) as f32);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (tx, ty) = (x - x0 as f32, y - y0 as f32);
    let rgb: [f32; 3] = std::array::from_fn(|i| {
        let a = px[y0 * w + x0][i] * (1.0 - tx) + px[y0 * w + x1][i] * tx;
        let b = px[y1 * w + x0][i] * (1.0 - tx) + px[y1 * w + x1][i] * tx;
        a * (1.0 - ty) + b * ty
    });
    // (Deeper and greyer than the texture — in the bake's light a blue-green iris read as
    // a small saturated doll's dot, the coordinator's review: a natural grey-blue — and the
    // upper lid's shadow over the top of the iris, as an eye under a lid has it.)
    let y = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
    let lid = 1.0 - 0.6 * smooth(0.5, 0.78, uv[1]);
    rgb.map(|c| (c * 0.5 + y * 0.5) * 0.62 * lid)
}

/// Card `k`'s texture (none past the six).
pub fn card(k: u8) -> Option<&'static Card> {
    static CARDS: OnceLock<Vec<Option<Card>>> = OnceLock::new();
    CARDS
        .get_or_init(|| PNGS.iter().map(|b| decode(b, true)).collect())
        .get(k as usize)?
        .as_ref()
}

impl Level {
    /// The texel at `uv` (the proxy's coordinates: v up), nearest.
    fn at(&self, uv: [f32; 2]) -> [u8; 4] {
        let x = (uv[0].rem_euclid(1.0) * self.w as f32) as usize;
        let y = ((1.0 - uv[1].rem_euclid(1.0)) * self.h as f32) as usize;
        self.texels[y.min(self.h - 1) * self.w + x.min(self.w - 1)]
    }

    /// The four channels at `uv`, bilinear (wrapping), as 0…255 floats.
    fn bilinear(&self, uv: [f32; 2]) -> [f32; 4] {
        let x = uv[0].rem_euclid(1.0) * self.w as f32 - 0.5;
        let y = (1.0 - uv[1].rem_euclid(1.0)) * self.h as f32 - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (tx, ty) = (x - x0, y - y0);
        let wrap = |v: f32, n: usize| (v as isize).rem_euclid(n as isize) as usize;
        let (xa, xb) = (wrap(x0, self.w), wrap(x0 + 1.0, self.w));
        let (ya, yb) = (wrap(y0, self.h), wrap(y0 + 1.0, self.h));
        let t = |x: usize, y: usize| self.texels[y * self.w + x];
        let (a, b, c, d) = (t(xa, ya), t(xb, ya), t(xa, yb), t(xb, yb));
        std::array::from_fn(|k| {
            let top = a[k] as f32 * (1.0 - tx) + b[k] as f32 * tx;
            let bot = c[k] as f32 * (1.0 - tx) + d[k] as f32 * tx;
            top * (1.0 - ty) + bot * ty
        })
    }
}

impl Card {
    /// The texel at `uv` (the proxy's coordinates: v up), nearest, on the first level.
    pub fn at(&self, uv: [f32; 2]) -> [u8; 4] {
        self.levels[0].at(uv)
    }

    fn level(&self, lod: f32) -> &Level {
        let i = (lod.max(0.0) + 0.25) as usize;
        &self.levels[i.min(self.levels.len() - 1)]
    }

    /// The level to read for a triangle with texture coordinates `uv` covering `area`
    /// samples on screen: texels a sample, as a power of two.
    pub fn lod(&self, uv: [[f32; 2]; 3], area: f32) -> f32 {
        let (a, b) = (
            [uv[1][0] - uv[0][0], uv[1][1] - uv[0][1]],
            [uv[2][0] - uv[0][0], uv[2][1] - uv[0][1]],
        );
        let texels = (a[0] * b[1] - a[1] * b[0]).abs() * 0.5 * (self.w * self.h) as f32;
        let area = area.abs().max(1e-6);
        if texels <= area {
            return 0.0;
        }
        0.5 * (texels / area).log2()
    }

    /// Its strands' light at `uv` against its mean, held near it (0.85 … 1.18: the
    /// texture's darker regions read as patches of another colour — the review). A brow's
    /// or a lash's.
    pub fn shade(&self, uv: [f32; 2]) -> f32 {
        (self.at(uv)[0] as f32 / self.mean).clamp(0.85, 1.18)
    }

    /// Its coverage at `uv` (0…1) at level `lod`, bilinear.
    pub fn alpha(&self, uv: [f32; 2], lod: f32) -> f32 {
        self.level(lod).bilinear(uv)[1] / 255.0
    }

    /// r42:arcus-hair — a hair card's strand at `uv`, level `lod`: its detail (about
    /// 0.55 … 1.45, 1 on average) and the way its strands run in the texture's coordinates
    /// (`du`, `dv`; a line: its sign means nothing), unit, or `None` where it has none.
    pub fn strand(&self, uv: [f32; 2], lod: f32) -> (f32, Option<[f32; 2]>) {
        let t = self.level(lod).bilinear(uv);
        let detail = if self.hair {
            (t[0] / 128.0).clamp(0.5, 1.5)
        } else {
            1.0
        };
        let (c2, s2) = ((t[2] - 128.0) / 127.0, (t[3] - 128.0) / 127.0);
        let m = (c2 * c2 + s2 * s2).sqrt();
        if !self.hair || m < 0.05 {
            return (detail, None);
        }
        let (c2, s2) = (c2 / m, s2 / m);
        // The half angle: cos θ ≥ 0, sin θ with sin 2θ's sign (image space, y down; the
        // texture's v runs up).
        let c = ((1.0 + c2) * 0.5).max(0.0).sqrt();
        let s = ((1.0 - c2) * 0.5).max(0.0).sqrt().copysign(s2);
        (detail, Some([c, -s]))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_card_texture_decodes_with_coverage_and_gaps() {
        for k in 0..6 {
            let c = super::card(k).expect("a card's texture");
            let t = &c.levels[0].texels;
            let covered = t.iter().filter(|t| t[1] >= 128).count();
            assert!(covered > 0 && covered < t.len(), "card {k}: {covered}");
        }
    }

    /// r42:arcus-hair — the owner's camouflage, in the data: a hair card's detail has no
    /// large-scale light left (its mean over every covered 64-texel block within a few
    /// per cent of 1), and nearly every covered texel knows its strands' way.
    #[test]
    fn hair_maps_carry_no_large_scale_light_and_know_the_strands_way() {
        for k in [0u8, 3] {
            let c = super::card(k).expect("a hair card");
            assert!(c.hair, "card {k} is stored as its maps");
            let l = &c.levels[0];
            let mut means = Vec::new();
            for by in (0..l.h).step_by(64) {
                for bx in (0..l.w).step_by(64) {
                    let (mut sum, mut n) = (0.0f32, 0usize);
                    for y in by..(by + 64).min(l.h) {
                        for x in bx..(bx + 64).min(l.w) {
                            let t = l.texels[y * l.w + x];
                            if t[1] >= 200 {
                                sum += t[0] as f32 / 128.0;
                                n += 1;
                            }
                        }
                    }
                    if n > 1024 {
                        means.push(sum / n as f32);
                    }
                }
            }
            assert!(means.len() > 10, "card {k}: {} blocks", means.len());
            for m in &means {
                assert!((m - 1.0).abs() < 0.08, "card {k}: a block's mean {m}");
            }
            let (mut way, mut n) = (0usize, 0usize);
            for y in (0..l.h).step_by(7) {
                for x in (0..l.w).step_by(7) {
                    if l.texels[y * l.w + x][1] < 200 {
                        continue;
                    }
                    n += 1;
                    let uv = [
                        (x as f32 + 0.5) / l.w as f32,
                        1.0 - (y as f32 + 0.5) / l.h as f32,
                    ];
                    if let (_, Some(d)) = c.strand(uv, 0.0) {
                        assert!((d[0] * d[0] + d[1] * d[1] - 1.0).abs() < 1e-3);
                        way += 1;
                    }
                }
            }
            assert!(way * 10 >= n * 9, "card {k}: the way at {way} of {n}");
        }
        for k in [1u8, 2, 4, 5] {
            assert!(!super::card(k).expect("a card").hair);
        }
    }

    /// Each level halves the side, down to 16 texels, and keeps the coverage's mean.
    #[test]
    fn card_levels_halve_and_keep_the_coverage() {
        let c = super::card(0).expect("her hair");
        let mean = |l: &super::Level| {
            l.texels.iter().map(|t| t[1] as f64).sum::<f64>() / l.texels.len() as f64
        };
        let m0 = mean(&c.levels[0]);
        for w in c.levels.windows(2) {
            assert_eq!(w[1].w * 2, w[0].w);
            assert!((mean(&w[1]) - m0).abs() < 2.0, "{} vs {m0}", mean(&w[1]));
        }
        assert_eq!(c.levels.last().map(|l| l.w), Some(16));
    }

    /// The hairline: her cards whole well inside it and gone well out over the skin, the
    /// strands' cores kept longest; the scalp's edge thinning the same way.
    #[test]
    fn the_hair_thins_out_over_the_hairline() {
        use super::{feather, scalp_cover, thin};
        assert_eq!(feather(0, 10.0), 1.0);
        assert_eq!(feather(0, -10.0), 0.0);
        assert_eq!(feather(1, -30.0), 1.0);
        let mut last = 0.0;
        for i in 0..=40 {
            let f = feather(0, -8.0 + i as f32 * 0.3);
            assert!(f >= last);
            last = f;
        }
        // Half the hair kept: a strand's core stays, its faint fringe goes.
        assert!(thin(1.0, 0.5) > 0.99);
        assert_eq!(thin(0.3, 0.5), 0.0);
        assert_eq!(thin(0.9, 0.0), 0.0);
        for a in [0.0, 0.4, 1.0] {
            assert!(scalp_cover(0, 10.0, a) > 0.99 && scalp_cover(0, -5.0, a) < 0.01);
        }
    }

    /// The eyes: a dark pupil in the middle of each texture, the white lighter round it.
    #[test]
    fn the_eyes_have_a_pupil_and_a_white() {
        for u0 in [0.0f32, 2.0] {
            let y = |c: [f32; 3]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
            let pupil = y(super::eye([u0 + 0.5, 0.5]));
            let white = y(super::eye([u0 + 0.05, 0.5]));
            assert!(pupil * 4.0 < white, "{u0}: pupil {pupil}, white {white}");
        }
    }
}
