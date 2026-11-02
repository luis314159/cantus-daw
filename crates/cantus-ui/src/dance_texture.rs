//! A stage drawn by `cantus-dance` and shown by egui (round 35 contract, ADR 0087 §3): the
//! one path from the processor's pixmap to a texture, for the stage over the workspace, the
//! companion window, the picker's previews and the export's preview.
//!
//! The pixmap is drawn at the **physical** pixel size it is shown at (points × the
//! screen's pixels per point), so a character is crisp at every zoom; it is kept between
//! frames and drawn again only when asked. egui takes an owned image to update a texture,
//! so each redraw hands it one: callers redraw only when the pose changed and no faster
//! than their own rate (the stage's measured cost, ADR 0091).

use cantus_dance::character::Character;
use cantus_dance::pose::Pose;
use cantus_dance::render::{render_into, Stage};
use egui::{ColorImage, Context, TextureHandle, TextureId, TextureOptions, Vec2};
use tiny_skia::Pixmap;

pub struct DanceTexture {
    name: String,
    tex: Option<TextureHandle>,
    pixmap: Option<Pixmap>,
}

impl DanceTexture {
    /// `name` is egui's debug name for the texture (one per place it is shown).
    pub fn new(name: impl Into<String>) -> DanceTexture {
        DanceTexture {
            name: name.into(),
            tex: None,
            pixmap: None,
        }
    }

    /// Draw `dancers` on `stage` and upload it. `None` for a zero-sized stage.
    pub fn draw(
        &mut self,
        ctx: &Context,
        stage: &Stage,
        dancers: &[(&dyn Character, Pose)],
    ) -> Option<TextureId> {
        let fits = self
            .pixmap
            .as_ref()
            .is_some_and(|p| p.width() == stage.width && p.height() == stage.height);
        if !fits {
            self.pixmap = Pixmap::new(stage.width, stage.height);
        }
        let pixmap = self.pixmap.as_mut()?;
        render_into(pixmap, stage, dancers);
        let size = [stage.width as usize, stage.height as usize];
        let image = ColorImage::from_rgba_premultiplied(size, pixmap.data());
        match &mut self.tex {
            Some(t) => t.set(image, TextureOptions::LINEAR),
            None => {
                self.tex = Some(ctx.load_texture(&self.name, image, TextureOptions::LINEAR));
            }
        }
        self.id()
    }

    /// The texture as last drawn.
    pub fn id(&self) -> Option<TextureId> {
        self.tex.as_ref().map(|t| t.id())
    }

    /// Its size in physical pixels, as last drawn.
    pub fn size_px(&self) -> Option<Vec2> {
        self.pixmap
            .as_ref()
            .map(|p| Vec2::new(p.width() as f32, p.height() as f32))
    }

    /// The pixmap as last drawn (a test reads it; the export's preview copies it).
    pub fn pixmap(&self) -> Option<&Pixmap> {
        self.pixmap.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_dance::render::Background;
    use cantus_dance::{cast, pose_at, DanceFrame};

    #[test]
    fn a_stage_becomes_a_texture_of_its_pixel_size() {
        let ctx = Context::default();
        let ch = cast::default_character();
        let pose = pose_at(&DanceFrame::default(), ch.as_ref(), 0.0);
        let mut t = DanceTexture::new("test-stage");
        let stage = Stage::new(64, 96, Background::Transparent);
        assert!(t.draw(&ctx, &stage, &[(ch.as_ref(), pose)]).is_some());
        assert_eq!(t.size_px(), Some(Vec2::new(64.0, 96.0)));
        let stage = Stage::new(0, 0, Background::Transparent);
        assert!(t.draw(&ctx, &stage, &[]).is_none());
    }
}
