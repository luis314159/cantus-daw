//! The picture's backend (round 40 phase 3, the owner's decision of 2026-11-12): the scene
//! is drawn **Vector** by this module (fast, light: the default), or **PerPixel** by a
//! second renderer (photographic: `scene-px`'s, ADR 0104's raster grown up) that registers
//! itself with [`register_pixel`]. Either way the scene, its camera, its light, its pick and
//! its cache are this module's: a per-pixel renderer receives the scene's [`Model`] — the
//! same 3-D primitives the vector picture is drawn from, in centimetres, with their
//! materials and the part each belongs to — and returns the picture's pixels.
//!
//! The choice is an app setting (*Settings ▸ Appearance ▸ Instrument pictures*), never the
//! project's: [`set_default`] sets it for every scene; a scene without a registered
//! per-pixel renderer draws vectors.

use super::compose::Frame;
use super::display::Part;
use super::geo::{Camera, V3};
use super::light::Light;
use super::Room;
use crate::kit::skin::scene::{Mat, C3};
use cantus_core::inst::room_look::RoomLook;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, OnceLock};

/// How the scene's picture is made (*Settings ▸ Appearance ▸ Instrument pictures*).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SceneBackend {
    /// Projected vectors, rasterised with antialiasing at the screen's pixels (this module).
    Vector,
    /// A per-pixel renderer (shadow maps, occlusion, traced reflections), when one is
    /// registered; vectors otherwise.
    PerPixel,
}

/// One 3-D primitive of the scene, as the vector picture is built from it (world
/// centimetres: x right, y up, z toward the viewer; the floor at y = 0).
#[derive(Clone, Debug, PartialEq)]
pub enum Prim {
    /// A flat facet of a material: its corners (a planar convex polygon, usually a quad),
    /// its outward normal, its grain (a wood's figure, 1 plain; a bronze's band).
    Facet {
        pts: Vec<V3>,
        n: V3,
        mat: Mat,
        grain: f32,
        part: Part,
    },
    /// A tube (a round rod) from `a` to `b` of radius `r`.
    Tube {
        a: V3,
        b: V3,
        r: f32,
        mat: Mat,
        part: Part,
    },
    /// A flat-coloured polygon with no material (a room's painted detail, a light's own
    /// colour: a lamp, a window), at `alpha`.
    Flat {
        pts: Vec<V3>,
        col: C3,
        alpha: f32,
        part: Part,
    },
}

/// How a fresh install draws the instruments' pictures — the one place to flip it (the
/// owner may make *Photographic* the default once it is fast and light enough). A settings
/// file that names a choice keeps it; one written before the choice existed reads this.
pub const DEFAULT_PICTURES: SceneBackend = SceneBackend::Vector;

impl Default for SceneBackend {
    fn default() -> Self {
        DEFAULT_PICTURES
    }
}

impl SceneBackend {
    pub const ALL: [SceneBackend; 2] = [SceneBackend::Vector, SceneBackend::PerPixel];

    /// Its words' key: *Drawn*, *Photographic*.
    pub fn key(self) -> &'static str {
        match self {
            SceneBackend::Vector => "scene.pictures.drawn",
            SceneBackend::PerPixel => "scene.pictures.photo",
        }
    }
}

/// What a per-pixel renderer is given: the scene's primitives (the instrument, its stands
/// and microphones, the room's walls and floor), its camera (already fitted to the picture
/// in points: multiply its screen transform by the pixels a point), its light (the mood,
/// the key's place and focus, the stage's tight spot), the room and its look.
#[derive(Clone, Debug)]
pub struct Model {
    pub prims: Vec<Prim>,
    pub cam: Camera,
    pub light: Light,
    pub room: Room,
    pub look: RoomLook,
    /// The floor's gloss (0 matte … 1 a mirror's), as the vector picture mirrors it.
    pub gloss: f32,
    pub frame: Frame,
    /// The picture's size in points.
    pub size: egui::Vec2,
    /// What the scene shows, as the caller described it — the piano's look, lid,
    /// Perspective, Width, Room level and mics, or the kit's definition — for a renderer
    /// that builds its own geometry from it (always set by the scene's build).
    pub subject: Option<super::Subject>,
    /// A dark theme's evening, or a light one's daylight.
    pub evening: bool,
}

/// A per-pixel renderer: the scene's model to its picture, `ppp` pixels a point. Called on
/// the scene's worker thread, never the UI's; a stale build is dropped by the caller.
/// `quick` while a live control glides (the lid, Perspective, Width, Room level): a fast
/// bake is wanted; once it rests, the scene asks again with `quick` false for the full one.
pub trait PixelBackend: Send + Sync {
    fn render(&self, model: &Model, ppp: f32, quick: bool) -> egui::ColorImage;
}

static PIXEL: OnceLock<Arc<dyn PixelBackend>> = OnceLock::new();
static DEFAULT: AtomicU8 = AtomicU8::new(DEFAULT_PICTURES as u8);

/// Register the per-pixel renderer (once, at start-up). Returns false when one already is.
pub fn register_pixel(r: Arc<dyn PixelBackend>) -> bool {
    PIXEL.set(r).is_ok()
}

/// The registered per-pixel renderer.
pub fn pixel() -> Option<Arc<dyn PixelBackend>> {
    PIXEL.get().cloned()
}

/// Set how every scene's picture is made (the app setting).
pub fn set_default(b: SceneBackend) {
    DEFAULT.store(b as u8, Ordering::Relaxed);
}

/// How scenes' pictures are made now: the setting, or vectors while no per-pixel renderer
/// is registered.
pub fn current() -> SceneBackend {
    if DEFAULT.load(Ordering::Relaxed) == SceneBackend::PerPixel as u8 && PIXEL.get().is_some() {
        SceneBackend::PerPixel
    } else {
        SceneBackend::Vector
    }
}
