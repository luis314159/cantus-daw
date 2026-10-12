//! The user's ten logos (ADR 0024 §12), drawn natively in vectors: `resvg` renders each
//! one at the exact physical size it is shown at, on a worker thread (the detailed
//! logos' filters cost 36 ms at 256 px and 526 ms at 1024), cached in memory and — when
//! the app names a directory — on disk, keyed by (SVG, tint, pixels). The previous
//! texture stays on screen until the new one is ready.
//!
//! **Tinting (method A).** Every colour in the SVGs carries a role — `tile`, `wave`,
//! `playhead`, `patina`, `neutral` — as a `data-role` attribute. In *Theme* colours each
//! role takes its hue and saturation from a token (tile ← the brand colour at low
//! saturation, wave ← the highlight, playhead ← the bright brand colour, patina ← the
//! selection) and **keeps its designed lightness**, so the gradients keep their depth.
//! The rewrite is done on the SVG text before parsing. In *Icon* colours (round 25, ADR
//! 0046) the wave and the playhead take the taskbar icon's colours instead, so the
//! title-bar mark and the splash can follow the icon.
//!
//! **The default is #2, "C de onda con playhead-cruz", in its original colours.** Below
//! 48 px #2's glow turns to blur, so [`small_master`] draws a simplified #2 on the pixel
//! grid for 16, 24 and 32 px.

use super::color::{opaque, Hsl};
use super::tokens::Tokens;
use egui::{Color32, ColorImage, Context, Id, TextureHandle, TextureOptions};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};

/// The SVG sources, `assets/logos/01.svg … 10.svg`.
pub const SOURCES: [&str; 10] = [
    include_str!("../../assets/logos/01.svg"),
    include_str!("../../assets/logos/02.svg"),
    include_str!("../../assets/logos/03.svg"),
    include_str!("../../assets/logos/04.svg"),
    include_str!("../../assets/logos/05.svg"),
    include_str!("../../assets/logos/06.svg"),
    include_str!("../../assets/logos/07.svg"),
    include_str!("../../assets/logos/08.svg"),
    include_str!("../../assets/logos/09.svg"),
    include_str!("../../assets/logos/10.svg"),
];

/// One of the ten logos, numbered as the user designed them (1…10).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Logo(pub u8);

impl Logo {
    /// #2, the default.
    pub const DEFAULT: Logo = Logo(2);

    pub fn all() -> impl Iterator<Item = Logo> {
        (1..=10).map(Logo)
    }

    /// Its name's i18n key.
    pub fn name_key(self) -> &'static str {
        const KEYS: [&str; 10] = [
            "kit.logo.1",
            "kit.logo.2",
            "kit.logo.3",
            "kit.logo.4",
            "kit.logo.5",
            "kit.logo.6",
            "kit.logo.7",
            "kit.logo.8",
            "kit.logo.9",
            "kit.logo.10",
        ];
        KEYS[self.index()]
    }

    fn index(self) -> usize {
        (self.0.clamp(1, 10) - 1) as usize
    }

    pub fn source(self) -> &'static str {
        SOURCES[self.index()]
    }
}

/// How a logo is coloured.
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum Tint {
    /// Exactly as designed (the default).
    #[default]
    Original,
    /// In the theme's colours, role by role.
    Theme,
    /// In the taskbar icon's colours (the wave and the playhead), the rest as *Theme*.
    Icon,
}

/// Which logo the interface shows, and how it is coloured: #2 in its original colours
/// unless the user chooses otherwise (ADR 0024 §12).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct LogoChoice {
    /// 1…10, as the user numbered them.
    pub number: u8,
    pub tint: Tint,
}

impl Default for LogoChoice {
    fn default() -> Self {
        LogoChoice {
            number: Logo::DEFAULT.0,
            tint: Tint::Original,
        }
    }
}

impl LogoChoice {
    pub fn is_default(&self) -> bool {
        *self == LogoChoice::default()
    }

    pub fn logo(&self) -> Logo {
        Logo(self.number.clamp(1, 10))
    }
}

/// The taskbar icon's colours, which the *Icon* tint follows (set by the application
/// whenever the appearance changes).
fn icon_colours() -> &'static std::sync::RwLock<super::mark::MarkColours> {
    static C: OnceLock<std::sync::RwLock<super::mark::MarkColours>> = OnceLock::new();
    C.get_or_init(|| std::sync::RwLock::new(super::mark::MarkColours::ORIGINAL))
}

/// Tell the *Icon* tint which colours the taskbar icon has now.
pub fn set_icon_colours(c: super::mark::MarkColours) {
    *icon_colours().write().unwrap_or_else(|e| e.into_inner()) = c;
}

fn rgb3(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// The token each role takes its hue and saturation from, and a saturation scale.
fn role_target(role: &str, tint: Tint, t: &Tokens) -> Option<(Color32, f32)> {
    if tint == Tint::Icon {
        let ic = *icon_colours().read().unwrap_or_else(|e| e.into_inner());
        match role {
            "wave" => return Some((rgb3(ic.wave[1]), 1.0)),
            "playhead" => return Some((rgb3(ic.playhead), 1.0)),
            "tile" => return Some((rgb3(ic.wave[2]), 0.35)),
            _ => {}
        }
    }
    match role {
        "tile" => Some((t.brand, 0.35)),
        "wave" => Some((t.highlight, 1.0)),
        "playhead" => Some((t.brand_bright, 1.0)),
        "patina" => Some((t.selection, 1.0)),
        _ => None,
    }
}

fn hex_of(c: Color32) -> String {
    let [r, g, b, _] = c.to_array();
    format!("#{r:02X}{g:02X}{b:02X}")
}

/// The SVG text of `logo` tinted by the theme `t` (method A). `Original` returns the
/// source unchanged.
pub fn tinted(src: &str, tint: Tint, t: &Tokens) -> String {
    if tint == Tint::Original {
        return src.to_string();
    }
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('>') else {
            out.push_str(&rest[open..]);
            return out;
        };
        let tag = &rest[open..open + close + 1];
        out.push_str(&retint_tag(tag, tint, t));
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    out
}

fn retint_tag(tag: &str, tint: Tint, t: &Tokens) -> String {
    let Some(i) = tag.find("data-role=\"") else {
        return tag.to_string();
    };
    let role_start = i + "data-role=\"".len();
    let role = &tag[role_start..role_start + tag[role_start..].find('"').unwrap_or(0)];
    let Some((target, sat_scale)) = role_target(role, tint, t) else {
        return tag.to_string();
    };
    let th = Hsl::of(opaque(target));
    let mut out = String::with_capacity(tag.len());
    let mut rest = tag;
    while let Some(q) = rest.find("=\"#") {
        let name_start = rest[..q].rfind(' ').map(|n| n + 1).unwrap_or(0);
        let attr = &rest[name_start..q];
        let hex_start = q + 3;
        let colour_attr = matches!(attr, "fill" | "stroke" | "stop-color" | "flood-color");
        match (colour_attr, rest.get(hex_start..hex_start + 6)) {
            (true, Some(hex)) if hex.chars().all(|c| c.is_ascii_hexdigit()) => {
                out.push_str(&rest[..q + 2]);
                let c = crate::theme::parse_hex(hex).unwrap_or(Color32::BLACK);
                let designed = Hsl::of(c);
                let new = Hsl {
                    h: th.h,
                    s: (th.s * sat_scale).clamp(0.0, 1.0),
                    l: designed.l,
                }
                .rgb();
                out.push_str(&hex_of(new));
                rest = &rest[hex_start + 6..];
            }
            _ => {
                out.push_str(&rest[..q + 3]);
                rest = &rest[q + 3..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Render SVG text to a square image of `px` pixels (premultiplied). Blocking: call it
/// on a worker for anything large.
pub fn render_svg(svg: &str, px: u32) -> Option<ColorImage> {
    use resvg::{tiny_skia, usvg};
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let size = tree.size();
    let mut pix = tiny_skia::Pixmap::new(px.max(1), px.max(1))?;
    let scale = px as f32 / size.width().max(size.height());
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pix.as_mut(),
    );
    Some(ColorImage::from_rgba_premultiplied(
        [px as usize, px as usize],
        pix.data(),
    ))
}

/// The pixel size below which #2 is drawn from its small master.
pub const SMALL: u32 = 48;

pub use super::logo_small::small_master;

/// The SVG to render for `logo` at `px`: #2 below [`SMALL`] comes from its small master.
pub fn svg_for(logo: Logo, tint: Tint, t: &Tokens, px: u32) -> String {
    if logo == Logo::DEFAULT && px < SMALL {
        tinted(&small_master(px), tint, t)
    } else {
        tinted(logo.source(), tint, t)
    }
}

// ---- the cache and the worker --------------------------------------------------------

fn disk() -> &'static Mutex<Option<PathBuf>> {
    static D: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
    D.get_or_init(|| Mutex::new(None))
}

/// Keep rendered logos in `dir` between launches (the app passes its cache directory;
/// by default they live in memory only).
pub fn set_disk_cache(dir: Option<PathBuf>) {
    *disk().lock().unwrap_or_else(|e| e.into_inner()) = dir;
}

fn disk_path(key: u64) -> Option<PathBuf> {
    let d = disk().lock().unwrap_or_else(|e| e.into_inner()).clone()?;
    Some(d.join(format!("logo-{key:016x}.png")))
}

fn disk_load(key: u64, px: u32) -> Option<ColorImage> {
    let bytes = std::fs::read(disk_path(key)?).ok()?;
    let pix = resvg::tiny_skia::Pixmap::decode_png(&bytes).ok()?;
    (pix.width() == px && pix.height() == px)
        .then(|| ColorImage::from_rgba_premultiplied([px as usize, px as usize], pix.data()))
}

fn disk_store(key: u64, img: &ColorImage) {
    let Some(path) = disk_path(key) else {
        return;
    };
    let [w, h] = img.size;
    let Some(mut pix) = resvg::tiny_skia::Pixmap::new(w as u32, h as u32) else {
        return;
    };
    for (d, s) in pix.pixels_mut().iter_mut().zip(&img.pixels) {
        if let Some(c) =
            resvg::tiny_skia::PremultipliedColorU8::from_rgba(s.r(), s.g(), s.b(), s.a())
        {
            *d = c;
        }
    }
    if let (Some(dir), Ok(bytes)) = (path.parent(), pix.encode_png()) {
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::write(path, bytes);
    }
}

type Done = (u64, Logo, Tint, ColorImage);

struct Store {
    textures: HashMap<u64, TextureHandle>,
    latest: HashMap<(Logo, Tint), u64>,
    pending: std::collections::HashSet<u64>,
    tx: Sender<Done>,
    rx: Receiver<Done>,
}

impl Default for Store {
    fn default() -> Self {
        let (tx, rx) = channel();
        Store {
            textures: HashMap::new(),
            latest: HashMap::new(),
            pending: Default::default(),
            tx,
            rx,
        }
    }
}

fn store(ctx: &Context) -> Arc<Mutex<Store>> {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_insert_with(Id::new("cantus-kit-logos"), || {
            Arc::new(Mutex::new(Store::default()))
        })
        .clone()
    })
}

impl Store {
    fn land(&mut self, ctx: &Context) {
        while let Ok((key, logo, tint, img)) = self.rx.try_recv() {
            self.insert(ctx, key, logo, tint, img);
        }
    }

    fn insert(&mut self, ctx: &Context, key: u64, logo: Logo, tint: Tint, img: ColorImage) {
        self.pending.remove(&key);
        let tex = ctx.load_texture(
            format!("cantus-logo-{key:016x}"),
            img,
            TextureOptions::LINEAR,
        );
        self.textures.insert(key, tex);
        self.latest.insert((logo, tint), key);
    }
}

fn key_of(svg: &str, px: u32) -> u64 {
    super::cache::key((svg, px))
}

/// The logo's texture at `px` physical pixels. The first ask starts a render on a worker
/// thread and returns the last texture of the same logo and tint (another size or
/// theme) until it lands — or `None` if there is none yet.
pub fn texture(
    ctx: &Context,
    logo: Logo,
    tint: Tint,
    t: &Tokens,
    px: u32,
) -> Option<egui::TextureId> {
    let st = store(ctx);
    let mut s = st.lock().unwrap_or_else(|e| e.into_inner());
    s.land(ctx);
    let svg = svg_for(logo, tint, t, px);
    let key = key_of(&svg, px);
    if let Some(tex) = s.textures.get(&key) {
        return Some(tex.id());
    }
    if s.pending.insert(key) {
        let tx = s.tx.clone();
        let ctx2 = ctx.clone();
        std::thread::spawn(move || {
            let img = disk_load(key, px).or_else(|| {
                let img = render_svg(&svg, px)?;
                disk_store(key, &img);
                Some(img)
            });
            if let Some(img) = img {
                let _ = tx.send((key, logo, tint, img));
                ctx2.request_repaint();
            }
        });
    }
    s.latest
        .get(&(logo, tint))
        .and_then(|k| s.textures.get(k))
        .map(|t| t.id())
}

/// The logo's texture at `px`, rendered on this thread if it is not ready — for the
/// splash's first frame (from a baked image, normally), tests and the proof render.
pub fn texture_now(
    ctx: &Context,
    logo: Logo,
    tint: Tint,
    t: &Tokens,
    px: u32,
) -> Option<egui::TextureId> {
    let st = store(ctx);
    let mut s = st.lock().unwrap_or_else(|e| e.into_inner());
    s.land(ctx);
    let svg = svg_for(logo, tint, t, px);
    let key = key_of(&svg, px);
    if let Some(tex) = s.textures.get(&key) {
        return Some(tex.id());
    }
    let img = disk_load(key, px).or_else(|| render_svg(&svg, px))?;
    s.insert(ctx, key, logo, tint, img);
    s.textures.get(&key).map(|t| t.id())
}

/// Paint `logo` filling the square `rect` (snapped to whole pixels), at the exact pixel
/// size it covers. `now` renders on this thread when the texture is not ready.
pub fn paint(
    b: &mut super::Brush,
    rect: egui::Rect,
    logo: Logo,
    tint: Tint,
    t: &Tokens,
    now: bool,
) {
    let rect = b.snap_rect(rect);
    let px = (rect.width().min(rect.height()) * b.ppp()).round().max(1.0) as u32;
    let ctx = b.ctx().clone();
    let tex = if now {
        texture_now(&ctx, logo, tint, t, px)
    } else {
        texture(&ctx, logo, tint, t, px)
    };
    if let Some(tex) = tex {
        let side = px as f32 / b.ppp();
        let r = egui::Rect::from_min_size(rect.min, egui::Vec2::splat(side));
        b.foreign(tex, r);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_logo_parses_and_renders() {
        for logo in Logo::all() {
            let img = render_svg(logo.source(), 48).expect("renders");
            assert_eq!(img.size, [48, 48]);
            // Something was drawn in the middle of the tile.
            let c = img.pixels[24 * 48 + 24];
            assert!(c.a() > 0, "logo {} is empty in the middle", logo.0);
        }
    }

    #[test]
    fn every_colour_in_every_logo_has_a_role() {
        for logo in Logo::all() {
            let src = logo.source();
            let mut rest = src;
            while let Some(open) = rest.find('<') {
                let close = rest[open..]
                    .find('>')
                    .map(|c| open + c)
                    .unwrap_or(rest.len());
                let tag = &rest[open..close];
                let has_hex = [
                    "fill=\"#",
                    "stroke=\"#",
                    "stop-color=\"#",
                    "flood-color=\"#",
                ]
                .iter()
                .any(|a| tag.contains(a));
                assert!(
                    !has_hex || tag.contains("data-role=\""),
                    "logo {}: a colour without a role in {tag}",
                    logo.0
                );
                rest = &rest[close..];
            }
        }
    }

    #[test]
    fn theme_tint_keeps_lightness_and_takes_the_theme_s_hue() {
        let t = Tokens::paper();
        let out = tinted(Logo(6).source(), Tint::Theme, &t);
        assert_ne!(out, Logo(6).source());
        // #06's wave #E07A2E becomes the highlight's hue at the wave's own lightness.
        let designed = Hsl::of(Color32::from_rgb(0xe0, 0x7a, 0x2e));
        let target = Hsl::of(t.highlight);
        let found = out
            .split("stroke=\"")
            .nth(1)
            .and_then(|s| crate::theme::parse_hex(&s[..7]))
            .expect("a stroke colour");
        let h = Hsl::of(found);
        assert!((h.l - designed.l).abs() < 0.02);
        assert!((h.h - target.h).abs() < 3.0);
        // Icon colours: the wave follows the icon's middle stop.
        set_icon_colours(crate::kit::mark::MarkColours::from_one([0x2f, 0x7c, 0xf6]));
        let blue = tinted(Logo(6).source(), Tint::Icon, &t);
        let found = blue
            .split("stroke=\"")
            .nth(1)
            .and_then(|s| crate::theme::parse_hex(&s[..7]))
            .expect("a stroke colour");
        assert!((Hsl::of(found).h - Hsl::of(Color32::from_rgb(0x2f, 0x7c, 0xf6)).h).abs() < 3.0);
        set_icon_colours(crate::kit::mark::MarkColours::ORIGINAL);
        // Original is untouched.
        assert_eq!(
            tinted(Logo(2).source(), Tint::Original, &t),
            Logo(2).source()
        );
    }

    #[test]
    fn the_small_master_renders_at_icon_sizes() {
        for px in [16, 24, 32] {
            let svg = small_master(px);
            let img = render_svg(&svg, px).expect("the small master renders");
            assert_eq!(img.size, [px as usize, px as usize]);
            assert!(svg.contains("data-role=\"wave\""));
        }
        assert!(svg_for(Logo::DEFAULT, Tint::Original, &Tokens::slate(), 32)
            .contains("viewBox=\"0 0 32 32\""));
        assert!(svg_for(Logo::DEFAULT, Tint::Original, &Tokens::slate(), 64).contains("1024"));
    }
}
