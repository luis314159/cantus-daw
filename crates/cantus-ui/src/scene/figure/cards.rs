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

/// A card's texture: its side and each texel's grey and coverage (0…255).
pub struct Card {
    pub w: usize,
    pub h: usize,
    pub texels: Vec<[u8; 2]>,
    /// The mean grey where it covers (its strands' light is taken against it, so a card's
    /// colour is the figure's hair colour on average, as the scalp under it is).
    pub mean: f32,
}

fn decode(bytes: &[u8]) -> Option<Card> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (frame.width as usize, frame.height as usize);
    let ch = match frame.color_type {
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgba => 4,
        png::ColorType::Grayscale => 1,
        png::ColorType::Rgb => 3,
        _ => return None,
    };
    let texels = buf[..w * h * ch]
        .chunks(ch)
        .map(|c| match ch {
            2 => [c[0], c[1]],
            4 => [((c[0] as u16 + c[1] as u16 + c[2] as u16) / 3) as u8, c[3]],
            1 => [c[0], 255],
            _ => [((c[0] as u16 + c[1] as u16 + c[2] as u16) / 3) as u8, 255],
        })
        .collect::<Vec<[u8; 2]>>();
    let (sum, n) = texels
        .iter()
        .filter(|t| t[1] >= 128)
        .fold((0.0f32, 0usize), |(s, n), t| (s + t[0] as f32, n + 1));
    let mean = if n > 0 { sum / n as f32 } else { 128.0 }.max(8.0);
    Some(Card { w, h, texels, mean })
}

/// Card `k`'s texture (none past the six).
pub fn card(k: u8) -> Option<&'static Card> {
    static CARDS: OnceLock<Vec<Option<Card>>> = OnceLock::new();
    CARDS
        .get_or_init(|| PNGS.iter().map(|b| decode(b)).collect())
        .get(k as usize)?
        .as_ref()
}

impl Card {
    /// The texel at `uv` (the proxy's coordinates: v up), nearest.
    pub fn at(&self, uv: [f32; 2]) -> [u8; 2] {
        let x = (uv[0].rem_euclid(1.0) * self.w as f32) as usize;
        let y = ((1.0 - uv[1].rem_euclid(1.0)) * self.h as f32) as usize;
        self.texels[y.min(self.h - 1) * self.w + x.min(self.w - 1)]
    }

    /// Its strands' light at `uv` against its mean (about 0.6 … 1.4).
    pub fn shade(&self, uv: [f32; 2]) -> f32 {
        (self.at(uv)[0] as f32 / self.mean).clamp(0.55, 1.4)
    }

    /// Whether the card covers `uv` (its alpha over half).
    pub fn covers(&self, uv: [f32; 2]) -> bool {
        self.at(uv)[1] >= 128
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_card_texture_decodes_with_coverage_and_gaps() {
        for k in 0..6 {
            let c = super::card(k).expect("a card's texture");
            let covered = c.texels.iter().filter(|t| t[1] >= 128).count();
            assert!(
                covered > 0 && covered < c.texels.len(),
                "card {k}: {covered}"
            );
        }
    }
}
