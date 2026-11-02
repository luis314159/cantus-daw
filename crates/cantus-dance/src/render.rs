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
}

impl Stage {
    pub fn new(width: u32, height: u32, background: Background) -> Stage {
        Stage {
            width,
            height,
            background,
            standard_height: 0.42,
            shadows: !background.is_chroma(),
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
        Color::from_rgba(r, g, b, f32::from(a) / 255.0).unwrap_or(Color::BLACK)
    }

    /// A colour as it is, untouched by the hue (eyes, outlines).
    pub fn fixed(&self, rgba: [u8; 4]) -> Color {
        let [r, g, b, a] = rgba;
        Color::from_rgba8(r, g, b, a)
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
