//! Images a character file names: PNG and JPEG, measured from their headers before they
//! are decoded (ADR 0089 §5), turned into tiny-skia's premultiplied pixmaps; `data:` URIs
//! (Lottie's embedded images) decoded here; a hue turn applied once per image and step.

use super::limits::Limits;
use super::ImportError;
use tiny_skia::Pixmap;

/// Counts the pixels of every image of one character against its total.
#[derive(Debug, Default)]
pub struct PixelBudget {
    used: u64,
}

impl PixelBudget {
    fn take(&mut self, w: u32, h: u32, limits: &Limits) -> Result<(), ImportError> {
        self.used += u64::from(w) * u64::from(h);
        if self.used > limits.image_pixels_total {
            return Err(ImportError::TooLarge(format!(
                "more than {} pixels of images in one character",
                limits.image_pixels_total
            )));
        }
        Ok(())
    }
}

fn side_check(w: u32, h: u32, limits: &Limits) -> Result<(), ImportError> {
    if w == 0 || h == 0 {
        return Err(ImportError::Invalid("an image has no pixels".into()));
    }
    if w > limits.image_side || h > limits.image_side {
        return Err(ImportError::TooLarge(format!(
            "an image of {w}×{h} pixels, at most {0}×{0}",
            limits.image_side
        )));
    }
    Ok(())
}

/// Decode a PNG or a JPEG (found from its first bytes).
pub fn decode(
    bytes: &[u8],
    limits: &Limits,
    budget: &mut PixelBudget,
) -> Result<Pixmap, ImportError> {
    if bytes.len() > limits.image_bytes {
        return Err(ImportError::TooLarge(format!(
            "an image of {} bytes, at most {}",
            bytes.len(),
            limits.image_bytes
        )));
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        decode_png(bytes, limits, budget)
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        decode_jpeg(bytes, limits, budget)
    } else {
        Err(ImportError::Invalid(
            "an image that is neither PNG nor JPEG".into(),
        ))
    }
}

fn decode_png(
    bytes: &[u8],
    limits: &Limits,
    budget: &mut PixelBudget,
) -> Result<Pixmap, ImportError> {
    let bad = |e: png::DecodingError| ImportError::Invalid(format!("PNG: {e}"));
    let mut dec = png::Decoder::new_with_limits(
        std::io::Cursor::new(bytes),
        png::Limits {
            bytes: (limits.image_side as usize).pow(2) * 8,
        },
    );
    dec.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = dec.read_info().map_err(bad)?;
    let (w, h) = {
        let info = reader.info();
        (info.width, info.height)
    };
    side_check(w, h, limits)?;
    budget.take(w, h, limits)?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| ImportError::TooLarge("a PNG frame too large to hold".into()))?;
    let mut buf = vec![0u8; size];
    let frame = reader.next_frame(&mut buf).map_err(bad)?;
    let (color, depth) = (frame.color_type, frame.bit_depth);
    if depth != png::BitDepth::Eight {
        return Err(ImportError::Invalid(format!(
            "a PNG of {depth:?} bits per channel"
        )));
    }
    let channels = match color {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => {
            return Err(ImportError::Invalid(
                "an indexed PNG left unexpanded".into(),
            ))
        }
    };
    let stride = frame.line_size;
    to_pixmap(w, h, |x, y| {
        let i = y as usize * stride + x as usize * channels;
        let p = &buf[i..i + channels];
        match channels {
            1 => [p[0], p[0], p[0], 255],
            2 => [p[0], p[0], p[0], p[1]],
            3 => [p[0], p[1], p[2], 255],
            _ => [p[0], p[1], p[2], p[3]],
        }
    })
}

fn decode_jpeg(
    bytes: &[u8],
    limits: &Limits,
    budget: &mut PixelBudget,
) -> Result<Pixmap, ImportError> {
    use zune_core::bytestream::ZCursor;
    use zune_core::colorspace::ColorSpace;
    use zune_core::options::DecoderOptions;
    let side = limits.image_side as usize;
    let opts = DecoderOptions::default()
        .set_max_width(side)
        .set_max_height(side)
        .jpeg_set_out_colorspace(ColorSpace::RGB);
    let mut dec = zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(bytes), opts);
    let bad = |e: zune_jpeg::errors::DecodeErrors| ImportError::Invalid(format!("JPEG: {e:?}"));
    dec.decode_headers().map_err(bad)?;
    let info = dec
        .info()
        .ok_or_else(|| ImportError::Invalid("JPEG: no header".into()))?;
    let (w, h) = (u32::from(info.width), u32::from(info.height));
    side_check(w, h, limits)?;
    budget.take(w, h, limits)?;
    let rgb = dec.decode().map_err(bad)?;
    if rgb.len() < (w * h * 3) as usize {
        return Err(ImportError::Invalid(
            "JPEG: fewer pixels than its size".into(),
        ));
    }
    to_pixmap(w, h, |x, y| {
        let i = ((y * w + x) * 3) as usize;
        [rgb[i], rgb[i + 1], rgb[i + 2], 255]
    })
}

/// A premultiplied pixmap from straight RGBA.
pub fn to_pixmap(w: u32, h: u32, px: impl Fn(u32, u32) -> [u8; 4]) -> Result<Pixmap, ImportError> {
    let mut out =
        Pixmap::new(w, h).ok_or_else(|| ImportError::Invalid("an image has no pixels".into()))?;
    let data = out.data_mut();
    for y in 0..h {
        for x in 0..w {
            let [r, g, b, a] = px(x, y);
            let i = ((y * w + x) * 4) as usize;
            let m = |c: u8| ((u32::from(c) * u32::from(a) + 127) / 255) as u8;
            data[i..i + 4].copy_from_slice(&[m(r), m(g), m(b), a]);
        }
    }
    Ok(out)
}

/// The bytes of a `data:` URI (`data:image/png;base64,…`), or `None` when `s` is not one.
pub fn data_uri(s: &str) -> Option<Result<Vec<u8>, ImportError>> {
    let rest = s.strip_prefix("data:")?;
    let Some((head, body)) = rest.split_once(',') else {
        return Some(Err(ImportError::Invalid("a data URI without data".into())));
    };
    if !head.ends_with(";base64") {
        return Some(Err(ImportError::Invalid(
            "a data URI that is not base64".into(),
        )));
    }
    Some(base64(body))
}

/// Standard base64 (padding and whitespace allowed).
pub fn base64(s: &str) -> Result<Vec<u8>, ImportError> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return Err(ImportError::Invalid("base64 with a stray character".into())),
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

/// `img` with its colours turned by `deg` around the wheel (the same matrix as
/// `Canvas::color`), alpha kept.
pub fn hue_turned(img: &Pixmap, deg: f32) -> Pixmap {
    let mut out = img.clone();
    if deg == 0.0 {
        return out;
    }
    for p in out.data_mut().chunks_exact_mut(4) {
        let a = p[3];
        if a == 0 {
            continue;
        }
        let af = f32::from(a);
        let (r, g, b) = crate::render::hue_rotate(
            f32::from(p[0]) / af,
            f32::from(p[1]) / af,
            f32::from(p[2]) / af,
            deg,
        );
        let q = |c: f32| (c * af + 0.5).min(af) as u8;
        p[0] = q(r);
        p[1] = q(g);
        p[2] = q(b);
    }
    out
}

/// The hue a turn is cached at: whole degrees in 0…359.
pub fn hue_step(deg: f32) -> i32 {
    if !deg.is_finite() {
        return 0;
    }
    (deg.round() as i32).rem_euclid(360)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn png_bytes(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut wr = enc.write_header().unwrap();
        let data: Vec<u8> = (0..w * h).flat_map(|_| rgba).collect();
        wr.write_image_data(&data).unwrap();
        drop(wr);
        out
    }

    #[test]
    fn a_png_decodes_premultiplied() {
        let p = decode(
            &png_bytes(3, 2, [200, 100, 50, 128]),
            &Limits::DEFAULT,
            &mut PixelBudget::default(),
        )
        .unwrap();
        assert_eq!((p.width(), p.height()), (3, 2));
        let px = p.pixel(1, 1).unwrap();
        assert_eq!(
            (px.red(), px.green(), px.blue(), px.alpha()),
            (100, 50, 25, 128)
        );
    }

    #[test]
    fn an_image_too_large_is_refused_from_its_header() {
        let l = Limits {
            image_side: 64,
            ..Limits::DEFAULT
        };
        let r = decode(&png_bytes(65, 1, [0; 4]), &l, &mut PixelBudget::default());
        assert!(matches!(r, Err(ImportError::TooLarge(_))));
        let l = Limits {
            image_pixels_total: 100,
            ..Limits::DEFAULT
        };
        let mut b = PixelBudget::default();
        decode(&png_bytes(10, 10, [0; 4]), &l, &mut b).unwrap();
        assert!(decode(&png_bytes(1, 1, [0; 4]), &l, &mut b).is_err());
    }

    #[test]
    fn data_uris_and_base64() {
        assert_eq!(base64("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(
            data_uri("data:image/png;base64,aGk=").unwrap().unwrap(),
            b"hi"
        );
        assert!(data_uri("images/a.png").is_none());
        assert!(data_uri("data:text/plain,hi").unwrap().is_err());
        assert!(base64("a$b").is_err());
    }

    #[test]
    fn garbage_is_refused_in_words() {
        for b in [
            &b"\x89PNG\r\n\x1a\nxx"[..],
            &[0xFF, 0xD8, 0xFF, 0, 1],
            b"GIF89a",
        ] {
            assert!(decode(b, &Limits::DEFAULT, &mut PixelBudget::default()).is_err());
        }
    }

    #[test]
    fn a_hue_of_zero_keeps_the_image_and_a_turn_keeps_alpha() {
        let p = decode(
            &png_bytes(2, 2, [255, 0, 0, 200]),
            &Limits::DEFAULT,
            &mut PixelBudget::default(),
        )
        .unwrap();
        assert_eq!(hue_turned(&p, 0.0).data(), p.data());
        let t = hue_turned(&p, 120.0);
        let px = t.pixel(0, 0).unwrap();
        assert_eq!(px.alpha(), 200);
        assert!(px.green() > px.red());
        assert_eq!(hue_step(-30.4), 330);
    }
}
