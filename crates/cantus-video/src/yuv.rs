//! Pixels as encoders take them.
//!
//! - [`rgba_to_nv12`]: tiny-skia's premultiplied RGBA to 4:2:0 YUV in NV12 layout (the Y
//!   plane, then U and V interleaved at half size), **BT.709, limited range** — what every
//!   H.264 decoder and every platform assumes for HD, and what the stream says it is (the
//!   encoder's VUI and the MP4's `colr`). Premultiplied colour is the picture composited
//!   over black, so a pixel left transparent becomes black.
//! - [`demultiply`]: premultiplied to straight alpha, for PNG.
//!
//! Integer maths with rounding (16-bit fixed point): the same bytes on every system.

/// A 4:2:0 picture in NV12 layout.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Nv12 {
    pub width: u32,
    pub height: u32,
    /// `width × height` bytes of Y, then `width × height / 2` bytes of interleaved U, V.
    pub data: Vec<u8>,
}

impl Nv12 {
    /// The Y plane.
    pub fn y(&self) -> &[u8] {
        &self.data[..(self.width * self.height) as usize]
    }

    /// The interleaved U, V plane.
    pub fn uv(&self) -> &[u8] {
        &self.data[(self.width * self.height) as usize..]
    }
}

// BT.709 limited range, × 65 536: Y = 16 + 0.182586 R + 0.614231 G + 0.062007 B; Cb and
// Cr from the same matrix, scaled to 224/255 around 128.
const YR: i32 = 11_966;
const YG: i32 = 40_254;
const YB: i32 = 4_064;
const UR: i32 = -6_596;
const UG: i32 = -22_188;
const UB: i32 = 28_784;
const VR: i32 = 28_784;
const VG: i32 = -26_145;
const VB: i32 = -2_639;
const HALF: i32 = 1 << 15;

/// Y of one (premultiplied) pixel.
#[inline]
pub fn luma(r: u8, g: u8, b: u8) -> u8 {
    let y = (YR * r as i32 + YG * g as i32 + YB * b as i32 + HALF) >> 16;
    (y + 16) as u8
}

/// U and V of a (premultiplied) colour; with four pixels summed, `shift` = 2.
#[inline]
fn chroma(r: i32, g: i32, b: i32, shift: u32) -> (u8, u8) {
    let round = HALF << shift;
    let u = (UR * r + UG * g + UB * b + round) >> (16 + shift);
    let v = (VR * r + VG * g + VB * b + round) >> (16 + shift);
    (
        (u + 128).clamp(16, 240) as u8,
        (v + 128).clamp(16, 240) as u8,
    )
}

/// Convert a premultiplied RGBA picture (`width` and `height` even) into `out`.
pub fn rgba_to_nv12(rgba: &[u8], width: u32, height: u32, out: &mut Nv12) {
    let (w, h) = (width as usize, height as usize);
    debug_assert!(w % 2 == 0 && h % 2 == 0, "4:2:0 needs an even size");
    debug_assert_eq!(rgba.len(), w * h * 4);
    out.width = width;
    out.height = height;
    out.data.clear();
    out.data.resize(w * h + w * h / 2, 0);
    let (ys, uvs) = out.data.split_at_mut(w * h);
    for (src, dst) in rgba.chunks_exact(4).zip(ys.iter_mut()) {
        *dst = luma(src[0], src[1], src[2]);
    }
    for cy in 0..h / 2 {
        let row0 = &rgba[(2 * cy) * w * 4..(2 * cy + 1) * w * 4];
        let row1 = &rgba[(2 * cy + 1) * w * 4..(2 * cy + 2) * w * 4];
        let dst = &mut uvs[cy * w..(cy + 1) * w];
        for cx in 0..w / 2 {
            let i = cx * 8;
            let r = row0[i] as i32 + row0[i + 4] as i32 + row1[i] as i32 + row1[i + 4] as i32;
            let g =
                row0[i + 1] as i32 + row0[i + 5] as i32 + row1[i + 1] as i32 + row1[i + 5] as i32;
            let b =
                row0[i + 2] as i32 + row0[i + 6] as i32 + row1[i + 2] as i32 + row1[i + 6] as i32;
            let (u, v) = chroma(r, g, b, 2);
            dst[2 * cx] = u;
            dst[2 * cx + 1] = v;
        }
    }
}

/// Premultiplied to straight alpha, in place (what PNG stores).
pub fn demultiply(rgba: &mut [u8]) {
    for p in rgba.chunks_exact_mut(4) {
        let a = p[3] as u32;
        if a != 0 && a != 255 {
            for c in &mut p[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
}

/// Premultiplied RGBA to RGB composited over black (the premultiplied colour itself), for
/// an opaque PNG.
pub fn rgba_to_rgb(rgba: &[u8], out: &mut Vec<u8>) {
    out.clear();
    out.reserve(rgba.len() / 4 * 3);
    for p in rgba.chunks_exact(4) {
        out.extend_from_slice(&p[..3]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(rgb: [u8; 3]) -> (u8, u8, u8) {
        let px: Vec<u8> = (0..4).flat_map(|_| [rgb[0], rgb[1], rgb[2], 255]).collect();
        let mut f = Nv12::default();
        rgba_to_nv12(&px, 2, 2, &mut f);
        assert!(f.y().iter().all(|&y| y == f.y()[0]));
        (f.y()[0], f.uv()[0], f.uv()[1])
    }

    #[test]
    fn bt709_limited_range_reference_colours() {
        // ITU-R BT.709 in 8-bit studio range (the standard's own table, rounded).
        assert_eq!(one([0, 0, 0]), (16, 128, 128));
        assert_eq!(one([255, 255, 255]), (235, 128, 128));
        assert_eq!(one([255, 0, 0]), (63, 102, 240));
        assert_eq!(one([0, 255, 0]), (173, 42, 26));
        assert_eq!(one([0, 0, 255]), (32, 240, 118));
        assert_eq!(one([128, 128, 128]), (126, 128, 128));
    }

    #[test]
    fn chroma_is_the_mean_of_four_pixels_and_transparent_is_black() {
        let mut px = Vec::new();
        for _ in 0..2 {
            px.extend_from_slice(&[255, 255, 255, 255, 0, 0, 0, 0]);
        }
        let mut f = Nv12::default();
        rgba_to_nv12(&px, 2, 2, &mut f);
        assert_eq!(f.y(), &[235, 16, 235, 16]);
        assert_eq!(f.uv(), &[128, 128]);
    }

    #[test]
    fn demultiply_restores_straight_colour() {
        let mut p = [64, 32, 0, 128, 10, 10, 10, 255, 0, 0, 0, 0];
        demultiply(&mut p);
        assert_eq!(p, [128, 64, 0, 128, 10, 10, 10, 255, 0, 0, 0, 0]);
    }
}
