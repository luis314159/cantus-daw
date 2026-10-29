//! PDF: the engraved pages as vector PDF — every music glyph and word set in its font,
//! the fonts embedded as subsets (Bravura's CFF and the text faces' TrueType, each a CID
//! font with a ToUnicode map so the words can be searched and copied), lines, beams and
//! ties as filled paths. A4 or Letter, title and composer in the document's info.

use crate::display::{Anchor, Item};
use crate::glyphs::{Glyph, BRAVURA, EM_IN_SPACES};
use crate::layout::{Engraved, Paper};
use crate::text::{Face, TextFaces, TextMeasure};
use pdf_writer::types::{CidFontType, FontFlags, SystemInfo, UnicodeCmap};
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str, TextStr};
use skrifa::instance::{LocationRef, Size};
use skrifa::raw::TableProvider;
use skrifa::{MetadataProvider, Tag};
use std::collections::BTreeMap;

/// Why a PDF could not be made.
#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    #[error("a font could not be read: {0}")]
    Font(String),
    #[error("a font could not be subset: {0}")]
    Subset(String),
}

/// One embedded font: its bytes, its glyphs used (old id → new id), the characters each
/// stands for.
struct Embedded {
    name: &'static str,
    bytes: &'static [u8],
    cff: bool,
    remap: subsetter::GlyphRemapper,
    chars: BTreeMap<u16, char>,
}

impl Embedded {
    fn new(name: &'static str, bytes: &'static [u8]) -> Result<Embedded, PdfError> {
        let font = skrifa::FontRef::new(bytes).map_err(|e| PdfError::Font(e.to_string()))?;
        let cff = font.table_data(Tag::new(b"CFF ")).is_some();
        let mut remap = subsetter::GlyphRemapper::new();
        remap.remap(0);
        Ok(Embedded {
            name,
            bytes,
            cff,
            remap,
            chars: BTreeMap::new(),
        })
    }

    /// The new glyph id (the CID) of `c`, adding it to the subset.
    fn cid(&mut self, c: char) -> u16 {
        let Ok(font) = skrifa::FontRef::new(self.bytes) else {
            return 0;
        };
        let gid = font.charmap().map(c).map_or(0, |g| g.to_u32() as u16);
        let new = self.remap.remap(gid);
        self.chars.entry(new).or_insert(c);
        new
    }
}

/// Points per staff space on `paper`.
pub fn points_per_space(paper: &Paper) -> f32 {
    paper.space_mm * 72.0 / 25.4
}

/// Write `engraved` (laid out on `paper`) as a PDF.
pub fn write(
    engraved: &Engraved,
    paper: &Paper,
    faces: TextFaces,
    title: &str,
    composer: &str,
) -> Result<Vec<u8>, PdfError> {
    let k = points_per_space(paper);
    let page_w = paper.width_mm * 72.0 / 25.4;
    let page_h = paper.height_mm * 72.0 / 25.4;
    let measure = TextMeasure::new(Some(faces));
    let mut fonts: Vec<Embedded> = vec![
        Embedded::new("Bravura", BRAVURA)?,
        Embedded::new("IBMPlexSans", faces.regular)?,
        Embedded::new("IBMPlexSans-SemiBold", faces.bold)?,
        Embedded::new("IBMPlexSansCondensed-Italic", faces.italic)?,
    ];
    let face_index = |f: Face| match f {
        Face::Regular => 1,
        Face::Bold => 2,
        Face::Italic => 3,
    };
    // Content streams first: they decide which glyphs each subset keeps.
    let mut contents: Vec<Vec<u8>> = Vec::with_capacity(engraved.pages.len());
    for page in &engraved.pages {
        let mut c = Content::new();
        c.set_fill_gray(0.0);
        let x = |v: f32| v * k;
        let y = |v: f32| page_h - v * k;
        for it in &page.list.items {
            match it {
                Item::Glyph {
                    glyph,
                    x: gx,
                    y: gy,
                    scale,
                    ..
                } => {
                    let cid = fonts[0].cid(glyph.codepoint());
                    let size = EM_IN_SPACES * scale * k;
                    c.begin_text();
                    c.set_font(Name(b"F0"), size);
                    c.set_text_matrix([1.0, 0.0, 0.0, 1.0, x(*gx), y(*gy)]);
                    c.show(Str(&cid.to_be_bytes()));
                    c.end_text();
                }
                Item::Text {
                    text,
                    x: tx,
                    y: ty,
                    size,
                    face,
                    anchor,
                } => {
                    let fi = face_index(*face);
                    let w = measure.width(text, *size, *face);
                    let x0 = match anchor {
                        Anchor::Left => *tx,
                        Anchor::Centre => tx - w / 2.0,
                        Anchor::Right => tx - w,
                    };
                    let mut bytes = Vec::with_capacity(text.len() * 2);
                    for ch in text.chars() {
                        bytes.extend_from_slice(&fonts[fi].cid(ch).to_be_bytes());
                    }
                    let font_name = format!("F{fi}");
                    c.begin_text();
                    c.set_font(Name(font_name.as_bytes()), size * k);
                    c.set_text_matrix([1.0, 0.0, 0.0, 1.0, x(x0), y(*ty)]);
                    c.show(Str(&bytes));
                    c.end_text();
                }
                _ => {}
            }
        }
        let shapes = fill_polygons(&page.list);
        for poly in &shapes {
            if poly.len() < 3 {
                continue;
            }
            c.move_to(x(poly[0][0]), y(poly[0][1]));
            for p in &poly[1..] {
                c.line_to(x(p[0]), y(p[1]));
            }
            c.close_path();
        }
        if !shapes.is_empty() {
            c.fill_nonzero();
        }
        contents.push(c.finish().into_vec());
    }

    let mut pdf = Pdf::new();
    let mut next = 1;
    let mut alloc = || {
        let r = Ref::new(next);
        next += 1;
        r
    };
    let catalog = alloc();
    let tree = alloc();
    let info = alloc();
    let page_ids: Vec<(Ref, Ref)> = contents.iter().map(|_| (alloc(), alloc())).collect();
    let font_ids: Vec<[Ref; 5]> = fonts
        .iter()
        .map(|_| [alloc(), alloc(), alloc(), alloc(), alloc()])
        .collect();
    pdf.catalog(catalog).pages(tree);
    pdf.pages(tree)
        .kids(page_ids.iter().map(|p| p.0))
        .count(page_ids.len() as i32);
    {
        let mut di = pdf.document_info(info);
        di.title(TextStr(title));
        if !composer.is_empty() {
            di.author(TextStr(composer));
        }
        di.creator(TextStr("Cantus"));
        di.producer(TextStr("Cantus (cantus-score)"));
        di.finish();
    }
    for ((pid, cid), data) in page_ids.iter().zip(&contents) {
        let mut p = pdf.page(*pid);
        p.media_box(Rect::new(0.0, 0.0, page_w, page_h));
        p.parent(tree);
        p.contents(*cid);
        let mut res = p.resources();
        let mut f = res.fonts();
        for (i, ids) in font_ids.iter().enumerate() {
            f.pair(Name(format!("F{i}").as_bytes()), ids[0]);
        }
        f.finish();
        res.finish();
        p.finish();
        pdf.stream(*cid, data);
    }
    for (font, ids) in fonts.iter_mut().zip(&font_ids) {
        embed(&mut pdf, font, ids)?;
    }
    Ok(pdf.finish())
}

/// Every filled shape of a page — lines, beams, dashed lines, ties, the brace — as the
/// polygons the PDF fills as one nonzero path.
pub fn fill_polygons(list: &crate::display::DisplayList) -> Vec<Vec<[f32; 2]>> {
    let mut shapes: Vec<Vec<[f32; 2]>> = Vec::new();
    for it in &list.items {
        match it {
            Item::Brace { x: bx, y0, y1 } => {
                let h = y1 - y0;
                let (sx, sy) = (crate::outline::brace_width_scale(h), h / EM_IN_SPACES);
                for cont in crate::outline::contours(Glyph::Brace) {
                    shapes.push(
                        cont.iter()
                            .map(|p| [bx + p[0] * sx, y1 + p[1] * sy])
                            .collect(),
                    );
                }
            }
            Item::Glyph { .. } | Item::Text { .. } => {}
            other => shapes.extend(polygons(other)),
        }
    }
    shapes.retain(|p| p.len() >= 3);
    // One winding for all (review, finding 1): filled as one nonzero path, a shape wound
    // the other way would cancel where it overlaps another — a white stripe where a beam
    // crosses a staff line, a stem's end knocked out, a gap in a tie.
    for p in shapes.iter_mut() {
        if signed_area(p) < 0.0 {
            p.reverse();
        }
    }
    shapes
}

fn signed_area(p: &[[f32; 2]]) -> f32 {
    let mut a = 0.0;
    for i in 0..p.len() {
        let (q, r) = (p[i], p[(i + 1) % p.len()]);
        a += q[0] * r[1] - r[0] * q[1];
    }
    a / 2.0
}

/// The filled outline of a line, beam, dashed line or tie.
fn polygons(it: &Item) -> Vec<Vec<[f32; 2]>> {
    match it {
        Item::Line {
            x0, y0, x1, y1, w, ..
        } => {
            let (dx, dy) = (x1 - x0, y1 - y0);
            let l = (dx * dx + dy * dy).sqrt().max(1e-6);
            let (nx, ny) = (-dy / l * w / 2.0, dx / l * w / 2.0);
            vec![vec![
                [x0 + nx, y0 + ny],
                [x1 + nx, y1 + ny],
                [x1 - nx, y1 - ny],
                [x0 - nx, y0 - ny],
            ]]
        }
        Item::Dashed {
            x0,
            y0,
            x1,
            w,
            dash,
            gap,
        } => {
            let mut out = Vec::new();
            let mut x = *x0;
            while x < *x1 {
                let e = (x + dash).min(*x1);
                out.push(vec![
                    [x, y0 - w / 2.0],
                    [e, y0 - w / 2.0],
                    [e, y0 + w / 2.0],
                    [x, y0 + w / 2.0],
                ]);
                x = e + gap;
            }
            out
        }
        Item::Beam {
            x0, y0, x1, y1, h, ..
        } => vec![vec![[*x0, *y0], [*x1, *y1], [*x1, y1 + h], [*x0, y0 + h]]],
        Item::Curve {
            p, end, mid, up, ..
        } => vec![crescent(*p, *end, *mid, *up)],
        _ => Vec::new(),
    }
}

/// A tie's crescent: its middle line thickened from `end` at its ends to `mid`.
pub fn crescent(p: [[f32; 2]; 4], end: f32, mid: f32, up: bool) -> Vec<[f32; 2]> {
    const N: usize = 48;
    let pt = |t: f32| {
        let u = 1.0 - t;
        let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
        [
            a * p[0][0] + b * p[1][0] + c * p[2][0] + d * p[3][0],
            a * p[0][1] + b * p[1][1] + c * p[2][1] + d * p[3][1],
        ]
    };
    let side = if up { -1.0 } else { 1.0 };
    let mut outer = Vec::with_capacity(2 * N + 2);
    let mut inner = Vec::with_capacity(N + 1);
    for i in 0..=N {
        let t = i as f32 / N as f32;
        let q = pt(t);
        let a = pt((t - 0.005).max(0.0));
        let b = pt((t + 0.005).min(1.0));
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l = (dx * dx + dy * dy).sqrt().max(1e-6);
        let (mut nx, mut ny) = (-dy / l, dx / l);
        if ny * side < 0.0 {
            nx = -nx;
            ny = -ny;
        }
        let w = end + (mid - end) * 4.0 * t * (1.0 - t);
        outer.push([q[0] + nx * w / 2.0, q[1] + ny * w / 2.0]);
        inner.push([q[0] - nx * w / 2.0, q[1] - ny * w / 2.0]);
    }
    inner.reverse();
    outer.extend(inner);
    outer
}

fn embed(pdf: &mut Pdf, font: &mut Embedded, ids: &[Ref; 5]) -> Result<(), PdfError> {
    let [type0, cid, desc, file, cmap] = *ids;
    let face = skrifa::FontRef::new(font.bytes).map_err(|e| PdfError::Font(e.to_string()))?;
    let upem = face.head().map(|h| h.units_per_em()).unwrap_or(1000) as f32;
    let metrics = face.glyph_metrics(Size::unscaled(), LocationRef::default());
    let olds: Vec<u16> = font.remap.remapped_gids().collect();
    let widths: Vec<f32> = olds
        .iter()
        .map(|g| {
            metrics
                .advance_width(skrifa::GlyphId::new(*g as u32))
                .unwrap_or(0.0)
                * 1000.0
                / upem
        })
        .collect();
    let data = subsetter::subset(font.bytes, 0, &font.remap)
        .map_err(|e| PdfError::Subset(format!("{e:?}")))?;
    // A subset's name: six capitals that change with what it holds, then the font's.
    let mut h: u32 = 2166136261;
    for g in &olds {
        h = (h ^ *g as u32).wrapping_mul(16777619);
    }
    let tag: String = (0..6)
        .map(|i| (b'A' + ((h >> (i * 4)) % 26) as u8) as char)
        .collect();
    let base = format!("{tag}+{}", font.name);
    let info = SystemInfo {
        registry: Str(b"Adobe"),
        ordering: Str(b"Identity"),
        supplement: 0,
    };
    pdf.type0_font(type0)
        .base_font(Name(base.as_bytes()))
        .encoding_predefined(Name(b"Identity-H"))
        .descendant_font(cid)
        .to_unicode(cmap);
    {
        let mut c = pdf.cid_font(cid);
        c.subtype(if font.cff {
            CidFontType::Type0
        } else {
            CidFontType::Type2
        });
        c.base_font(Name(base.as_bytes()));
        c.system_info(info);
        c.font_descriptor(desc);
        c.default_width(0.0);
        if !font.cff {
            c.cid_to_gid_map_predefined(Name(b"Identity"));
        }
        c.widths().consecutive(0, widths.iter().copied());
        c.finish();
    }
    let bbox = face
        .head()
        .map(|h| {
            Rect::new(
                h.x_min() as f32 * 1000.0 / upem,
                h.y_min() as f32 * 1000.0 / upem,
                h.x_max() as f32 * 1000.0 / upem,
                h.y_max() as f32 * 1000.0 / upem,
            )
        })
        .unwrap_or(Rect::new(0.0, -300.0, 1000.0, 1000.0));
    let (ascent, descent) = face
        .hhea()
        .map(|h| {
            (
                h.ascender().to_i16() as f32 * 1000.0 / upem,
                h.descender().to_i16() as f32 * 1000.0 / upem,
            )
        })
        .unwrap_or((800.0, -200.0));
    {
        let mut d = pdf.font_descriptor(desc);
        d.name(Name(base.as_bytes()));
        d.flags(if font.cff {
            FontFlags::SYMBOLIC
        } else {
            FontFlags::NON_SYMBOLIC
        });
        d.bbox(bbox);
        d.italic_angle(0.0);
        d.ascent(ascent);
        d.descent(descent);
        d.cap_height(ascent * 0.7);
        d.stem_v(80.0);
        if font.cff {
            d.font_file3(file);
        } else {
            d.font_file2(file);
        }
        d.finish();
    }
    let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&data, 6);
    {
        let mut s = pdf.stream(file, &compressed);
        s.filter(pdf_writer::Filter::FlateDecode);
        if font.cff {
            s.pair(Name(b"Subtype"), Name(b"OpenType"));
        } else {
            s.pair(Name(b"Length1"), data.len() as i32);
        }
        s.finish();
    }
    let mut cm = UnicodeCmap::new(Name(b"Custom"), info);
    for (g, c) in &font.chars {
        cm.pair(*g, *c);
    }
    let cm = cm.finish();
    pdf.cmap(cmap, cm.as_slice());
    Ok(())
}
