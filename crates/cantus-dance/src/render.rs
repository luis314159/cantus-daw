//! The one renderer (ADR 0087 §3): tiny-skia on the processor.
//!
//! A stage is a pixmap at the exact pixel size it will be shown (or encoded) at, so a
//! character is crisp at any scale: it is drawn again from its vectors, never stretched.
//! The screen uploads the pixmap as a texture, the picker draws previews, the companion
//! window and the streaming mode draw it on a transparent or chroma background, and a video
//! export draws every frame through the same function — the same pose gives the same
//! pixels everywhere.
//!
//! The pixmap is **premultiplied RGBA** (tiny-skia's), which is what egui's textures take
//! and what an encoder with alpha wants after un-premultiplying.

use crate::character::Character;
use crate::pose::Pose;
use cantus_dsp::math;
use tiny_skia::{Color, Paint, Path, Pixmap, Stroke, Transform};

/// What is behind the dancers.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Background {
    /// Nothing: alpha 0 (over the workspace, the streaming window, a layer for an editor).
    Transparent,
    /// One colour, straight RGBA (a theme's stage, or a chroma key's green).
    Solid([u8; 4]),
}

/// The chroma keys offered (straight RGB): green, blue, magenta.
pub const CHROMA_GREEN: [u8; 4] = [0, 177, 64, 255];
pub const CHROMA_BLUE: [u8; 4] = [0, 71, 187, 255];
pub const CHROMA_MAGENTA: [u8; 4] = [255, 0, 255, 255];

impl Background {
    /// One of the chroma keys offered.
    pub fn is_chroma(&self) -> bool {
        matches!(self, Background::Solid(c) if [CHROMA_GREEN, CHROMA_BLUE, CHROMA_MAGENTA].contains(c))
    }

    /// The chroma key it is, if it is one.
    pub fn chroma_key(&self) -> Option<[u8; 4]> {
        match self {
            Background::Solid(c) if self.is_chroma() => Some(*c),
            _ => None,
        }
    }
}

/// How far a keyer sees a colour from a key: the distance of their chroma (Cb, Cr; BT.709,
/// 0…255 units), what a keyer compares (ADR 0090 §3: a character keeps 60).
pub fn key_distance(rgb: [f32; 3], key: [u8; 4]) -> f32 {
    let chroma = |r: f32, g: f32, b: f32| {
        let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        ((b - y) / 1.8556, (r - y) / 1.5748)
    };
    let (cb, cr) = chroma(rgb[0], rgb[1], rgb[2]);
    let (kb, kr) = chroma(f32::from(key[0]), f32::from(key[1]), f32::from(key[2]));
    ((cb - kb) * (cb - kb) + (cr - kr) * (cr - kr)).sqrt()
}

/// What a keyer must keep between a character's colour and the key it is drawn over.
pub const KEY_SAFE: f32 = 62.0;

/// `rgb` (0…255) moved toward the grey of its own lightness, the least that keeps it
/// [`KEY_SAFE`] from `key` (round 40, ADR 0122: the Slate cast's lapis is near the blue key;
/// over a key it is drawn a quieter blue so the keyer leaves it, and as designed elsewhere).
pub fn key_safe(rgb: [f32; 3], key: [u8; 4]) -> [f32; 3] {
    if key_distance(rgb, key) >= KEY_SAFE {
        return rgb;
    }
    let y = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
    let mut out = [y; 3];
    for k in 1..=40 {
        let t = k as f32 / 40.0;
        let m = rgb.map(|c| c + (y - c) * t);
        if key_distance(m, key) >= KEY_SAFE {
            out = m;
            break;
        }
    }
    out
}

/// The theme's light handed to the stage (round 40, ADR 0122): the colours the Slate cast
/// paints with — its ink, the accent gold, the logo's vermilion — and the **room**, the
/// colour behind the dancer that its cuts are drawn in (a piece in front is separated from
/// the one behind by a hairline of the room). Straight RGBA. The family of rounds 35–37 and
/// imported characters ignore it; only the gold turns with the pose's hue.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct StageInk {
    /// Ink 1: the strongest text colour (Compás's whole figure).
    pub ink: [u8; 4],
    pub ink2: [u8; 4],
    /// Ink 3: quiet marks (Compás's motion marks).
    pub ink3: [u8; 4],
    pub ink4: [u8; 4],
    /// The accent gold (logo #03) and its deeper tone.
    pub gold: [u8; 4],
    pub gold_deep: [u8; 4],
    /// The logo's vermilion.
    pub verm: [u8; 4],
    /// What is behind the dancer: the theme's base for a floating dancer, the stage's colour
    /// docked and in the companion window, the export's background in a video.
    pub room: [u8; 4],
    /// The contact shadow's colour (drawn translucent).
    pub shadow: [u8; 4],
    /// A dark room (Night-like): the cast picks its dark values.
    pub dark: bool,
    /// The chroma key it is drawn over, when it is: every colour keeps clear of it
    /// ([`key_safe`]). Set by the renderer from a chroma background, or by a window that
    /// paints its key itself (the companion window).
    pub key: Option<[u8; 4]>,
}

/// `0xRRGGBB` as opaque straight RGBA.
pub const fn rgb(hex: u32) -> [u8; 4] {
    [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255]
}

impl StageInk {
    /// Slate Night's tokens (ADR 0111) and the logo's gold.
    pub const NIGHT: StageInk = StageInk {
        ink: rgb(0xE3E8ED),
        ink2: rgb(0xB4BDC6),
        ink3: rgb(0x909CA8),
        ink4: rgb(0x47515B),
        gold: rgb(0xE2B85A),
        gold_deep: rgb(0xC9A55A),
        verm: rgb(0xF0784A),
        room: rgb(0x1D2329),
        shadow: rgb(0x0B0E11),
        dark: true,
        key: None,
    };
    /// Slate Paper's.
    pub const PAPER: StageInk = StageInk {
        ink: rgb(0x17202A),
        ink2: rgb(0x38444F),
        ink3: rgb(0x556170),
        ink4: rgb(0xA9B3BD),
        gold: rgb(0x9A6E1C),
        gold_deep: rgb(0x7A5A04),
        verm: rgb(0xD2541A),
        room: rgb(0xF4F6F8),
        shadow: rgb(0x8C98A4),
        dark: false,
        key: None,
    };

    /// The same light in another room (a stage's own colour, a video's background).
    pub fn in_room(self, room: [u8; 4]) -> StageInk {
        StageInk { room, ..self }
    }
}

impl Default for StageInk {
    fn default() -> Self {
        StageInk::NIGHT
    }
}

/// A stage: its size in pixels, its background, and how tall a character of size 1 stands
/// on it (a share of the stage's height).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Stage {
    pub width: u32,
    pub height: u32,
    pub background: Background,
    pub standard_height: f32,
    /// Whether the characters draw their contact shadow: not over a chroma key, where it
    /// would come out as a darker shade of the key that a keyer leaves as a smudge.
    pub shadows: bool,
    /// The theme's light (round 40): what the Slate cast paints with.
    pub ink: StageInk,
}

impl Stage {
    pub fn new(width: u32, height: u32, background: Background) -> Stage {
        Stage {
            width,
            height,
            background,
            standard_height: 0.42,
            shadows: !background.is_chroma(),
            ink: StageInk::NIGHT,
        }
    }
}

/// Where a character draws: the pixmap, and the transform from its units to the pixels.
pub struct Canvas<'a> {
    pub pixmap: &'a mut Pixmap,
    pub transform: Transform,
    /// Pixels per unit of the character (for a hairline that stays one pixel).
    pub px_per_unit: f32,
    /// Degrees its colours turn by (`Pose::hue`), applied by [`Canvas::color`].
    pub hue: f32,
    /// Whether to draw the contact shadow ([`Stage::shadows`]).
    pub shadow: bool,
    /// The theme's light ([`Stage::ink`]).
    pub ink: StageInk,
}

impl Canvas<'_> {
    /// A colour from straight RGBA, turned by the pose's hue.
    pub fn color(&self, rgba: [u8; 4]) -> Color {
        let [r, g, b, a] = rgba;
        let (r, g, b) = hue_rotate(
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
            self.hue,
        );
        self.keyed([r, g, b], a)
    }

    /// A colour as it is, untouched by the hue (eyes, outlines).
    pub fn fixed(&self, rgba: [u8; 4]) -> Color {
        let [r, g, b, a] = rgba;
        match self.ink.key {
            None => Color::from_rgba8(r, g, b, a),
            Some(_) => self.keyed(
                [
                    f32::from(r) / 255.0,
                    f32::from(g) / 255.0,
                    f32::from(b) / 255.0,
                ],
                a,
            ),
        }
    }

    /// A colour (0…1) kept clear of the key it is drawn over, if any.
    fn keyed(&self, rgb: [f32; 3], a: u8) -> Color {
        let rgb = match self.ink.key {
            Some(k) => key_safe(rgb.map(|c| c * 255.0), k).map(|c| c / 255.0),
            None => rgb,
        };
        Color::from_rgba(
            rgb[0].clamp(0.0, 1.0),
            rgb[1].clamp(0.0, 1.0),
            rgb[2].clamp(0.0, 1.0),
            f32::from(a) / 255.0,
        )
        .unwrap_or(Color::BLACK)
    }

    pub fn fill(&mut self, path: &Path, color: Color) {
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        self.pixmap.fill_path(
            path,
            &paint,
            tiny_skia::FillRule::Winding,
            self.transform,
            None,
        );
    }

    /// A stroke `width` units wide.
    pub fn stroke(&mut self, path: &Path, color: Color, width: f32) {
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        let stroke = Stroke {
            width,
            line_cap: tiny_skia::LineCap::Round,
            line_join: tiny_skia::LineJoin::Round,
            ..Stroke::default()
        };
        self.pixmap
            .stroke_path(path, &paint, &stroke, self.transform, None);
    }
}

/// Turn a colour around the wheel by `deg` (the luminance-preserving matrix), clamped.
pub fn hue_rotate(r: f32, g: f32, b: f32, deg: f32) -> (f32, f32, f32) {
    if deg == 0.0 {
        return (r, g, b);
    }
    let (s, c) = math::sin_cos(deg * std::f32::consts::PI / 180.0);
    let m = [
        0.213 + c * 0.787 - s * 0.213,
        0.715 - c * 0.715 - s * 0.715,
        0.072 - c * 0.072 + s * 0.928,
        0.213 - c * 0.213 + s * 0.143,
        0.715 + c * 0.285 + s * 0.140,
        0.072 - c * 0.072 - s * 0.283,
        0.213 - c * 0.213 - s * 0.787,
        0.715 - c * 0.715 + s * 0.715,
        0.072 + c * 0.928 + s * 0.072,
    ];
    let f = |a: f32, b2: f32, c2: f32| (a * r + b2 * g + c2 * b).clamp(0.0, 1.0);
    (
        f(m[0], m[1], m[2]),
        f(m[3], m[4], m[5]),
        f(m[6], m[7], m[8]),
    )
}

/// The transform that puts `ch` in `pose` on `stage`: its feet at (x, y) of the stage, its
/// standing height `size × standard_height` of the stage's, lifted by `lift`, spun about
/// its middle, mirrored when it faces left and does not draw its own facing.
pub fn placement(stage: &Stage, ch: &dyn Character, pose: &Pose) -> (Transform, f32) {
    let h = stage.height as f32;
    let px_per_unit = (pose.size * stage.standard_height * h).max(0.5);
    let feet_x = pose.x * stage.width as f32;
    let feet_y = pose.y * h - pose.lift * 0.6 * px_per_unit;
    let mirror = if !ch.faces_itself() && pose.turn < 0.0 {
        -1.0
    } else {
        1.0
    };
    let middle = -0.5; // half its height above the feet, in its units
    let t = Transform::from_translate(feet_x, feet_y)
        .pre_scale(px_per_unit * mirror, px_per_unit)
        .pre_translate(0.0, middle)
        .pre_concat(crate::rig::rotation(pose.spin))
        .pre_translate(0.0, -middle);
    (t, px_per_unit)
}

/// Draw `dancers` onto `pixmap` (cleared to the stage's background first).
pub fn render_into(pixmap: &mut Pixmap, stage: &Stage, dancers: &[(&dyn Character, Pose)]) {
    match stage.background {
        Background::Transparent => pixmap.fill(Color::TRANSPARENT),
        Background::Solid([r, g, b, a]) => pixmap.fill(Color::from_rgba8(r, g, b, a)),
    }
    for (ch, pose) in dancers {
        if !pose.visible {
            continue;
        }
        let (transform, px_per_unit) = placement(stage, *ch, pose);
        let mut canvas = Canvas {
            pixmap,
            transform,
            px_per_unit,
            hue: pose.hue,
            shadow: stage.shadows,
            ink: StageInk {
                key: stage.background.chroma_key().or(stage.ink.key),
                ..stage.ink
            },
        };
        ch.draw(pose, &mut canvas);
    }
}

/// A new pixmap of the stage with `dancers` drawn on it (`None` for a zero-sized stage).
pub fn render(stage: &Stage, dancers: &[(&dyn Character, Pose)]) -> Option<Pixmap> {
    let mut p = Pixmap::new(stage.width, stage.height)?;
    render_into(&mut p, stage, dancers);
    Some(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cast, pose_at, DanceFrame};

    #[test]
    fn a_transparent_stage_is_clear_around_the_dancer_and_not_under_it() {
        let ch = cast::default_character();
        let frame = DanceFrame {
            playing: true,
            beats: 1.25,
            ..DanceFrame::default()
        };
        let pose = pose_at(&frame, ch.as_ref(), 0.0);
        let stage = Stage::new(320, 320, Background::Transparent);
        let p = render(&stage, &[(ch.as_ref(), pose)]).unwrap();
        assert_eq!(
            p.pixel(2, 2).unwrap().alpha(),
            0,
            "the corner is see-through"
        );
        let opaque = p.pixels().iter().filter(|c| c.alpha() > 0).count();
        assert!(opaque > 500, "the dancer is drawn ({opaque} pixels)");
    }

    #[test]
    fn the_same_pose_gives_the_same_pixels() {
        let ch = cast::default_character();
        let frame = DanceFrame {
            playing: true,
            beats: 7.5,
            ..DanceFrame::default()
        };
        let pose = pose_at(&frame, ch.as_ref(), 0.0);
        let stage = Stage::new(200, 300, Background::Solid(CHROMA_GREEN));
        let a = render(&stage, &[(ch.as_ref(), pose)]).unwrap();
        let b = render(&stage, &[(ch.as_ref(), pose)]).unwrap();
        assert_eq!(a.data(), b.data());
    }

    #[test]
    fn a_hue_of_zero_changes_nothing() {
        assert_eq!(hue_rotate(0.2, 0.4, 0.6, 0.0), (0.2, 0.4, 0.6));
        let (r, g, b) = hue_rotate(1.0, 0.0, 0.0, 120.0);
        assert!(
            g > r && g > b,
            "red turned a third of the wheel is green-ish"
        );
    }
}
