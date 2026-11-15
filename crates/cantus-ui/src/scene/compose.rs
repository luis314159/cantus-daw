//! A scene, composed (port of `scene/compose.py`): the room, the floor's mirror, the key's
//! shadows, the instrument and its microphones sorted far to near, the light's haze and the
//! picture's falloff. Returns the display list, its hit regions, and where each piece and
//! microphone landed on screen.

use super::display::{rad, Layer, List, Part};
use super::gear::{self, KitMic};
use super::geo::{Camera, V3};
use super::kit::{self, Piece};
use super::light::Light;
use super::prims::{shadows, Ctx, Lod, HIGH, LOW};
use super::{piano, rooms, Room};
use crate::kit::skin::scene::{ink, Mat};
use cantus_core::inst::chorda_look::Body;
use cantus_core::inst::room_look::RoomLook;
use egui::{Pos2, Vec2};

/// How the instrument is framed in the picture: the box it is fitted into (insets from the
/// picture's edges, points), a margin, where its box sits in it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Frame {
    pub inset: [f32; 4],
    pub margin: f32,
    pub anchor: (f32, f32),
}

/// Chorda's play view (the designer's `chorda_play.stage`: its fit box `x + 40, y + 50,
/// w − 80, h − 112`).
pub const PIANO_FRAME: Frame = Frame {
    inset: [40.0, 50.0, 40.0, 62.0],
    margin: 0.0,
    anchor: (0.5, 0.45),
};
/// The designer's picture-alone sheets of the piano (`main.py`'s `pic-chorda-*`: fit box
/// `30, 40, 696, 372` in 756 × 452).
pub const PIANO_PICTURE_FRAME: Frame = Frame {
    inset: [30.0, 40.0, 30.0, 40.0],
    margin: 0.0,
    anchor: (0.5, 0.45),
};
/// Tympanum's kit view (the designer's `kit_view.stage`).
pub const KIT_FRAME: Frame = Frame {
    inset: [60.0, 24.0, 60.0, 60.0],
    margin: 8.0,
    anchor: (0.5, 0.5),
};

impl Frame {
    fn fit_box(&self, size: Vec2) -> (f32, f32, f32, f32) {
        let [l, t, r, b] = self.inset;
        (l, t, (size.x - l - r).max(8.0), (size.y - t - b).max(8.0))
    }
}

/// A piano to compose.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PianoSpec {
    pub body: Body,
    pub finish: Mat,
    pub white: bool,
    pub black: bool,
    /// Degrees.
    pub lid: f32,
    pub persp: f32,
    pub width: f32,
    pub room_level: f32,
    pub mics: bool,
}

/// What the composer was given beyond the subject.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Stage {
    pub room: Room,
    pub look: RoomLook,
    pub evening: bool,
    /// The picture's size (points) and its pixels a point (the level of detail).
    pub size: Vec2,
    pub ppp: f32,
    pub frame: Frame,
    /// Keep the 3-D primitives for a per-pixel backend.
    pub capture: bool,
}

/// A scene built: its list, the pieces' outlines (part, depth, screen polygon), the mics'
/// heads on screen, the camera.
#[derive(Clone, Debug)]
pub struct Built {
    pub list: List,
    pub outlines: Vec<(Part, f32, Vec<Pos2>)>,
    pub mics: Vec<(Part, Pos2)>,
    pub cam: Camera,
    pub gloss: f32,
    pub evening: bool,
    pub size: Vec2,
    pub pieces: Vec<Piece>,
    /// The scene's 3-D model, when the stage asked for it (a per-pixel backend's input).
    pub model: Option<super::backend::Model>,
}

/// The picture's falloff: a radial darkening toward the corners.
fn finish(list: &mut List, size: Vec2, light: &Light) {
    let (w, h) = (size.x, size.y);
    list.on(Layer::Front);
    let pts = vec![
        Pos2::new(0.0, 0.0),
        Pos2::new(w, 0.0),
        Pos2::new(w, h),
        Pos2::new(0.0, h),
    ];
    let g = rad(
        Pos2::new(w / 2.0, h * 0.55),
        w * 0.72,
        h * 0.95,
        &[(0.55, ink::SHADE, 0.0), (1.0, ink::SHADE, light.m.falloff)],
    );
    list.poly(pts, g, 1.0, 0.0);
}

/// The level of detail for a picture `size` points large at `ppp`.
fn lod(size: Vec2, ppp: f32) -> Lod {
    // Never coarser on what is looked at.
    if size.x.min(size.y) * ppp < 200.0 {
        LOW
    } else {
        HIGH
    }
}

/// Chorda's piano in its room.
pub fn piano_scene(p: &PianoSpec, st: &Stage) -> Built {
    let upright = p.body == Body::Upright;
    let stage = st.room == Room::Stage;
    let mut light = Light::new(
        st.evening,
        stage,
        V3 {
            x: -80.0,
            y: 420.0,
            z: 120.0,
        },
        V3 {
            x: 0.0,
            y: 0.0,
            z: if upright { -10.0 } else { -90.0 },
        },
    );
    if stage {
        light.m.spot_r = if upright { 170.0 } else { 240.0 };
    }
    light.floor = rooms::floor_paint(st.look.floor.unwrap_or(rooms::def(st.room).floor)).a;
    let mut cam = piano::camera(p.body, st.room);
    // Every grand is framed as the concert grand with its lid on the full stick (sizes read
    // true); an upright as it stands, its top as open as it is (the designer's `points`).
    let fit_lid = if upright { p.lid } else { piano::GRAND_LID };
    cam.fit(
        &piano::points(p.body, fit_lid),
        st.frame.fit_box(st.size),
        st.frame.margin,
        st.frame.anchor,
    );
    let mut list = List::default();
    let lod = lod(st.size, st.ppp);
    let centre_z = if upright { -20.0 } else { -90.0 };
    let info = {
        let mut c = Ctx::new(cam, light, &mut list, lod);
        rooms::build(
            &mut c,
            st.room,
            V3 {
                x: 0.0,
                y: 0.0,
                z: centre_z,
            },
            st.look,
        )
    };
    let y0 = cam
        .p(V3 {
            x: 0.0,
            y: 0.0,
            z: if upright { 0.0 } else { -40.0 },
        })
        .y;
    list.mirror_fade = Some((y0 - 30.0, y0 + st.size.y * 0.45));
    if info.gloss > 0.15 {
        list.force = Some(Layer::Mirror);
        let mut m = Ctx::new(cam.mirrored(), light, &mut list, lod);
        piano::build(&mut m, p.body, &p.finish, p.white, p.black, p.lid);
        list.force = None;
    }
    let mut c = Ctx::new(cam, light, &mut list, lod);
    c.capture = st.capture.then(Vec::new);
    piano::build(&mut c, p.body, &p.finish, p.white, p.black, p.lid);
    let anchors = if p.mics {
        gear::piano_mics(&mut c, p.persp, p.width, p.room_level, upright, info.zb)
    } else {
        Vec::new()
    };
    shadows(&mut c);
    let prims = c.capture.take();
    drop(c);
    let model = prims.map(|prims| super::backend::Model {
        prims,
        cam,
        light,
        room: st.room,
        look: st.look,
        gloss: info.gloss,
        frame: st.frame,
        size: st.size,
        subject: None,
        evening: st.evening,
    });
    finish(&mut list, st.size, &light);
    let outlines = list
        .hits
        .iter()
        .map(|h| (h.part, h.depth, h.outline.clone()))
        .collect();
    let mics: Vec<(Part, Pos2)> = anchors.iter().map(|&(pt, a)| (pt, cam.p(a))).collect();
    for &(pt, s) in &mics {
        list.hit(pt, disc(s, 12.0), 0.0);
    }
    Built {
        list,
        outlines,
        mics,
        cam,
        gloss: info.gloss,
        evening: st.evening,
        size: st.size,
        pieces: Vec::new(),
        model,
    }
}

/// A small disc round `c` (a microphone's pick).
fn disc(c: Pos2, r: f32) -> Vec<Pos2> {
    (0..16)
        .map(|k| {
            let a = std::f32::consts::TAU * k as f32 / 16.0;
            Pos2::new(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect()
}

/// Tympanum's kit in its room; `mics` drawn when given.
pub fn kit_scene(pieces: &[Piece], fin: &Mat, mics: Option<&[KitMic]>, st: &Stage) -> Built {
    let mut light = Light::new(
        st.evening,
        st.room == Room::Stage,
        V3 {
            x: -60.0,
            y: 380.0,
            z: 170.0,
        },
        V3 {
            x: -6.0,
            y: 0.0,
            z: -4.0,
        },
    );
    light.floor = rooms::floor_paint(st.look.floor.unwrap_or(rooms::def(st.room).floor)).a;
    let el = match st.room {
        Room::Stage => 15.0,
        Room::Hall => 18.0,
        _ => 22.0,
    };
    let mut cam = Camera::new(
        V3 {
            x: -4.0,
            y: 80.0,
            z: 0.0,
        },
        16.0,
        el,
        600.0,
    );
    cam.fit(
        &kit::points(pieces),
        st.frame.fit_box(st.size),
        st.frame.margin,
        st.frame.anchor,
    );
    let mut list = List::default();
    let lod = lod(st.size, st.ppp);
    let info = {
        let mut c = Ctx::new(cam, light, &mut list, lod);
        rooms::build(
            &mut c,
            st.room,
            V3 {
                x: -8.0,
                y: 0.0,
                z: -4.0,
            },
            st.look,
        )
    };
    let y0 = cam
        .p(V3 {
            x: 0.0,
            y: 0.0,
            z: -30.0,
        })
        .y;
    list.mirror_fade = Some((y0 - 30.0, y0 + st.size.y * 0.40));
    if info.gloss > 0.15 {
        list.force = Some(Layer::Mirror);
        let mut m = Ctx::new(cam.mirrored(), light, &mut list, lod);
        kit::build(&mut m, pieces, fin);
        list.force = None;
    }
    let mut c = Ctx::new(cam, light, &mut list, lod);
    c.capture = st.capture.then(Vec::new);
    let outs = kit::build(&mut c, pieces, fin);
    let anchors = mics.map(|m| gear::kit_mics(&mut c, m)).unwrap_or_default();
    shadows(&mut c);
    let prims = c.capture.take();
    drop(c);
    let model = prims.map(|prims| super::backend::Model {
        prims,
        cam,
        light,
        room: st.room,
        look: st.look,
        gloss: info.gloss,
        frame: st.frame,
        size: st.size,
        subject: None,
        evening: st.evening,
    });
    finish(&mut list, st.size, &light);
    let mics: Vec<(Part, Pos2)> = anchors.iter().map(|&(pt, a)| (pt, cam.p(a))).collect();
    for &(pt, s) in &mics {
        list.hit(pt, disc(s, 12.0), 0.0);
    }
    Built {
        list,
        outlines: outs
            .into_iter()
            .map(|(i, d, o)| (Part::Piece(i), d, o))
            .collect(),
        mics,
        cam,
        gloss: info.gloss,
        evening: st.evening,
        size: st.size,
        pieces: pieces.to_vec(),
        model,
    }
}
