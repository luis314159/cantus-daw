//! Static richness, rasterised once (ADR 0024 §1, `rendering-performance.md` §5.1, §7.3).
//!
//! One **layer atlas** per egui context: a texture whose top-left corner is opaque white
//! (so flat geometry samples it too, and a whole view is one mesh on one texture) and
//! whose other cells hold layers rasterised with `tiny-skia` at the exact physical size
//! they are shown at — knob bodies, LED glows, logos. A layer is keyed by the caller's
//! hash of what it draws (its recipe, its colours and so the theme, its size in pixels);
//! when the atlas fills up it is cleared and layers come back on demand.
//!
//! Beside it, a **galley cache**: text laid out once per (string, role, box, policy,
//! pixels-per-point) and reused as an `Arc`, so painting a label that did not change
//! costs a hash and a lookup — no `String`, no allocation.

use egui::text::LayoutJob;
use egui::{Color32, ColorImage, Context, Galley, Id, Pos2, Rect, TextureHandle, TextureOptions};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

/// Side of the atlas, in texels.
pub const ATLAS: usize = 1024;
/// The white corner: 4 × 4 texels, sampled at its centre.
const WHITE: usize = 4;
/// Space kept around every layer so linear filtering never bleeds a neighbour in.
const PAD: usize = 1;

type Fitted = Option<(Arc<Galley>, bool)>;

/// Counters for tests and benches.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub layers_rasterised: u64,
    pub layer_hits: u64,
    pub atlas_resets: u64,
    pub galley_hits: u64,
    pub galley_misses: u64,
    /// egui font atlases noticed rebuilt, so every laid-out text was dropped.
    pub font_rebuilds: u64,
}

#[derive(Default)]
pub struct KitCache {
    atlas: Option<TextureHandle>,
    shelf_y: usize,
    shelf_h: usize,
    cursor_x: usize,
    layers: FxMap<u64, Rect>,
    galleys: FxMap<u64, (Arc<Galley>, u64)>,
    fits: FxMap<u64, (Fitted, u64)>,
    font_fill: f32,
    /// The text options and the first font face of the atlas last seen: egui rebuilds the
    /// atlas when either changes (a dark↔light theme switch changes the options).
    font_options: Option<egui::epaint::text::TextOptions>,
    font_face: Option<Arc<egui::FontData>>,
    pass: u64,
    /// Inspect mode's registry; `None` while the mode is off.
    pub inspect: Option<super::inspect::Registry>,
    pub stats: Stats,
}

/// FxHash (rustc's): one multiply per word. Keys here are ours, not an attacker's, so
/// SipHash's protection buys nothing and costs a lot per lookup.
#[derive(Default, Clone, Copy)]
pub struct Fx(u64);

impl Hasher for Fx {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for c in bytes.chunks(8) {
            let mut w = [0u8; 8];
            w[..c.len()].copy_from_slice(c);
            self.write_u64(u64::from_le_bytes(w));
        }
    }
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.0 = (self.0.rotate_left(5) ^ i).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.write_u64(i as u64);
    }
    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.write_u64(i as u64);
    }
    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.write_u64(i as u64);
    }
}

type FxMap<K, V> = HashMap<K, V, std::hash::BuildHasherDefault<Fx>>;

/// A stable hash of anything hashable, for cache keys.
pub fn key(v: impl Hash) -> u64 {
    let mut h = Fx::default();
    v.hash(&mut h);
    h.finish()
}

/// A colour as a hashable value.
pub fn ckey(c: Color32) -> u32 {
    u32::from_le_bytes(c.to_array())
}

/// This context's kit cache (a brush keeps the handle for its frame).
pub fn handle(ctx: &Context) -> Arc<Mutex<KitCache>> {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_insert_with(Id::new("cantus-kit-cache"), || {
            Arc::new(Mutex::new(KitCache::default()))
        })
        .clone()
    })
}

/// Run `f` with this context's kit cache, without the once-a-pass bookkeeping (which
/// needs the context's fonts, so cannot run before its first frame).
pub fn with_raw<R>(ctx: &Context, f: impl FnOnce(&mut KitCache) -> R) -> R {
    let cache = handle(ctx);
    let mut c = cache.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut c)
}

/// Run `f` with this context's kit cache.
pub fn with<R>(ctx: &Context, f: impl FnOnce(&mut KitCache) -> R) -> R {
    let cache = handle(ctx);
    let mut c = cache.lock().unwrap_or_else(|e| e.into_inner());
    c.begin_pass(ctx);
    f(&mut c)
}

impl KitCache {
    /// Called once a pass (by [`with`] and by a brush's `begin`): notices a rebuilt font
    /// atlas and forgets text nobody asked for in a while.
    pub fn begin_pass(&mut self, ctx: &Context) {
        let pass = ctx.cumulative_pass_nr();
        if pass == self.pass && self.atlas.is_some() {
            return;
        }
        self.pass = pass;
        // egui rebuilds its font atlas when it fills, when its text options change (a
        // dark↔light theme switch changes the colour transfer function) and when the font
        // definitions change; every galley laid out before then points at glyphs that
        // moved — painted from the new atlas they show other glyphs or nothing (round 23,
        // "the labels look strange after a theme change"). Each is watched: the fill ratio
        // only grows between rebuilds, so a drop means one happened — but a whole
        // session's glyphs can sit in the first row of a 16 384-texel atlas, where the
        // ratio of a fresh atlas is the same, so the options and the faces are compared
        // as well.
        let (fill, options, face) = ctx.fonts(|f| {
            let face = f.definitions().font_data.values().next().cloned();
            (f.font_atlas_fill_ratio(), *f.options(), face)
        });
        let same_face = match (&self.font_face, &face) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => self.font_options.is_none(),
        };
        let rebuilt =
            fill < self.font_fill || self.font_options.is_some_and(|o| o != options) || !same_face;
        if rebuilt {
            self.galleys.clear();
            self.fits.clear();
            self.stats.font_rebuilds += 1;
        }
        self.font_fill = fill;
        self.font_options = Some(options);
        self.font_face = face;
        if pass.is_multiple_of(60) {
            self.galleys
                .retain(|_, (_, used)| pass.saturating_sub(*used) < 120);
            self.fits
                .retain(|_, (_, used)| pass.saturating_sub(*used) < 120);
        }
    }

    /// The atlas texture and the uv of its white texel.
    pub fn atlas(&mut self, ctx: &Context) -> (egui::TextureId, Pos2) {
        if self.atlas.is_none() {
            let mut img = ColorImage::filled([ATLAS, ATLAS], Color32::TRANSPARENT);
            for y in 0..WHITE {
                for x in 0..WHITE {
                    img.pixels[y * ATLAS + x] = Color32::WHITE;
                }
            }
            self.atlas = Some(ctx.load_texture("cantus-kit-atlas", img, TextureOptions::LINEAR));
            self.reset_packer();
        }
        let id = self.atlas.as_ref().map(|t| t.id()).unwrap_or_default();
        let w = WHITE as f32 * 0.5 / ATLAS as f32;
        (id, Pos2::new(w, w))
    }

    fn reset_packer(&mut self) {
        self.layers.clear();
        self.shelf_y = 0;
        self.shelf_h = WHITE + PAD;
        self.cursor_x = WHITE + PAD;
    }

    fn pack(&mut self, w: usize, h: usize) -> Option<(usize, usize)> {
        let (w, h) = (w + 2 * PAD, h + 2 * PAD);
        if w > ATLAS || h > ATLAS {
            return None;
        }
        if self.cursor_x + w > ATLAS {
            self.shelf_y += self.shelf_h;
            self.shelf_h = 0;
            self.cursor_x = 0;
        }
        if self.shelf_y + h > ATLAS {
            return None;
        }
        let at = (self.cursor_x + PAD, self.shelf_y + PAD);
        self.cursor_x += w;
        self.shelf_h = self.shelf_h.max(h);
        Some(at)
    }

    /// The uv rect of the layer `key`, `w` × `h` physical pixels, drawing it with
    /// `draw` the first time (and after the atlas is cleared). `None` if it cannot fit.
    pub fn layer(
        &mut self,
        ctx: &Context,
        key: u64,
        w: usize,
        h: usize,
        draw: impl FnOnce(&mut tiny_skia::Pixmap),
    ) -> Option<Rect> {
        if let Some(uv) = self.layers.get(&key) {
            self.stats.layer_hits += 1;
            return Some(*uv);
        }
        self.atlas(ctx);
        let (w, h) = (w.max(1), h.max(1));
        let spot = match self.pack(w, h) {
            Some(s) => s,
            None => {
                self.stats.atlas_resets += 1;
                self.reset_packer();
                self.pack(w, h)?
            }
        };
        let mut pix = tiny_skia::Pixmap::new(w as u32, h as u32)?;
        draw(&mut pix);
        let img = ColorImage::from_rgba_premultiplied([w, h], pix.data());
        if let Some(t) = self.atlas.as_mut() {
            t.set_partial([spot.0, spot.1], img, TextureOptions::LINEAR);
        }
        let s = ATLAS as f32;
        let uv = Rect::from_min_max(
            Pos2::new(spot.0 as f32 / s, spot.1 as f32 / s),
            Pos2::new((spot.0 + w) as f32 / s, (spot.1 + h) as f32 / s),
        );
        self.layers.insert(key, uv);
        self.stats.layers_rasterised += 1;
        Some(uv)
    }

    /// Upload a ready image (a logo rendered on a worker) as a layer.
    pub fn layer_image(&mut self, ctx: &Context, key: u64, img: &ColorImage) -> Option<Rect> {
        if let Some(uv) = self.layers.get(&key) {
            return Some(*uv);
        }
        let [w, h] = img.size;
        self.layer(ctx, key, w, h, |pix| {
            for (d, s) in pix.pixels_mut().iter_mut().zip(&img.pixels) {
                if let Some(c) =
                    tiny_skia::PremultipliedColorU8::from_rgba(s.r(), s.g(), s.b(), s.a())
                {
                    *d = c;
                }
            }
        })
    }

    /// Forget every cached layer (they come back on demand): after a theme change, or
    /// in a test that compares what two themes rasterise.
    pub fn clear_layers(&mut self) {
        self.reset_packer();
    }

    /// Whether the layer `key` is ready.
    pub fn has_layer(&self, key: u64) -> bool {
        self.layers.contains_key(&key)
    }

    /// A text fitted to a box: the galley and whether it was shortened, or `None` when
    /// nothing fits — computed by `make` the first time.
    pub fn fitted(
        &mut self,
        key: u64,
        make: impl FnOnce() -> Option<(Arc<Galley>, bool)>,
    ) -> Option<(Arc<Galley>, bool)> {
        let pass = self.pass;
        if let Some((v, used)) = self.fits.get_mut(&key) {
            *used = pass;
            self.stats.galley_hits += 1;
            return v.clone();
        }
        self.stats.galley_misses += 1;
        let v = make();
        self.fits.insert(key, (v.clone(), pass));
        v
    }

    /// The galley `key`, laid out by `make` the first time.
    pub fn galley(
        &mut self,
        ctx: &Context,
        key: u64,
        make: impl FnOnce() -> LayoutJob,
    ) -> Arc<Galley> {
        let pass = self.pass;
        if let Some((g, used)) = self.galleys.get_mut(&key) {
            *used = pass;
            self.stats.galley_hits += 1;
            return g.clone();
        }
        self.stats.galley_misses += 1;
        let job = make();
        // A name in a script no bundled font has asks for the system's faces (round 26).
        crate::font_fallback::note_text(ctx, &job.text);
        let g = ctx.fonts_mut(|f| f.layout_job(job));
        self.galleys.insert(key, (g.clone(), pass));
        g
    }
}
