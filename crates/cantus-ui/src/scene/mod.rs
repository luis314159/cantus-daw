//! The instruments' scenes drawn as projected vectors (round 40 phase 3, ADR 0126): a 3-D
//! model in centimetres → the camera's projection → facet lighting (a key light, strip
//! lights, a reflected environment) → a depth-sorted display list → drawn at the screen's
//! pixels. Chorda's piano in its room and Tympanum's kit in its room; live and interactive
//! ([`Scene::pick`]); the picture rebuilt off the UI thread only when what it shows changes;
//! at rest, no repaint and no allocation.
//!
//! A port of the designer's generator (`F:\projects\daw-r40\design-p3\src\scene\`): `geo`,
//! `light` (its `light.py` and `materials.py`), `display` (`paint.py`), `prims`, `kit`,
//! `piano`, `gear`, `rooms`, `compose`; `draw` is its SVG backend rasterised. Owned by the
//! builder `scene`; `tymp-ui` and `chorda-src` call it and never edit it.

pub mod backend;
pub mod compose;
pub mod display;
pub mod draw;
pub mod gear;
pub mod geo;
pub mod kit;
pub mod light;
mod live;
pub mod piano;
pub mod piano_geo;
pub mod prims;
pub mod rooms;
pub mod stand;

pub use backend::{Model, PixelBackend, Prim, SceneBackend};
pub use live::{halo_for, set_blocking, workers, Scene};

use crate::kit::skin::scene::{self as pal, Mat};
use cantus_core::inst::chorda_look::{Body, Finish, Look};
use cantus_core::inst::room_look::RoomLook;
use cantus_core::inst::tympanum::{KitRef, RoomChoice};
use cantus_core::inst::tympanum_kit::KitDef;
use cantus_core::OutputId;
use display::Part;
use egui::{Pos2, Rect, Vec2};

/// The rooms a scene can stand in: the stage (the default, delivery 6: the instrument in
/// its own pool of light, the dark around it), a booth, a studio's live room, a hall, a
/// church.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Room {
    Stage,
    Booth,
    Studio,
    Hall,
    Church,
}

impl Room {
    pub const ALL: [Room; 5] = [
        Room::Stage,
        Room::Booth,
        Room::Studio,
        Room::Hall,
        Room::Church,
    ];

    /// The room Tympanum's `room` choice draws when no room look is kept: its own room is
    /// drawn as the stage (delivery 6's default: the kit in its own pool of light).
    pub fn of_kit(choice: RoomChoice) -> Room {
        match choice {
            RoomChoice::Own => Room::Stage,
            RoomChoice::Studio => Room::Studio,
            RoomChoice::Small => Room::Booth,
            RoomChoice::Hall => Room::Hall,
        }
    }

    /// The room a stored look names.
    pub fn of_look(r: cantus_core::inst::room_look::RoomKind) -> Room {
        use cantus_core::inst::room_look::RoomKind as K;
        match r {
            K::Stage => Room::Stage,
            K::Booth => Room::Booth,
            K::Studio => Room::Studio,
            K::Hall => Room::Hall,
            K::Church => Room::Church,
        }
    }

    /// The look's word for this room.
    pub fn kind(self) -> cantus_core::inst::room_look::RoomKind {
        use cantus_core::inst::room_look::RoomKind as K;
        match self {
            Room::Stage => K::Stage,
            Room::Booth => K::Booth,
            Room::Studio => K::Studio,
            Room::Hall => K::Hall,
            Room::Church => K::Church,
        }
    }

    /// The room a Tympanum's scene draws: the look's room when one is kept (text keys of
    /// `cantus_core::inst::room_look`, schema 69), else its `room` choice's.
    pub fn of_tympanum(spec: &cantus_core::NativeSpec, choice: RoomChoice) -> Room {
        cantus_core::inst::room_look::of(spec)
            .room
            .map_or(Room::of_kit(choice), Room::of_look)
    }

    /// The room Chorda's Room size (0 … 1) draws: a booth, a live room, a hall, a church.
    pub fn of_size(size: f32) -> Room {
        if size < 0.25 {
            Room::Booth
        } else if size < 0.5 {
            Room::Studio
        } else if size < 0.8 {
            Room::Hall
        } else {
            Room::Church
        }
    }
}

/// What a scene shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Subject {
    /// Chorda: the stored look (ADR 0104 §2), the lid (0 closed … 1 the full stick), where
    /// Perspective and Width put the pair, the room pair's level, whether its microphones
    /// are drawn.
    Piano {
        look: Look,
        lid: f32,
        persp: f32,
        width: f32,
        room_level: f32,
        mics: bool,
    },
    /// Tympanum: the kit's definition (pieces and mics where they stand, ADR 0124) — the
    /// mics' moves (`m{n}.*`) already applied by the caller — its shells' finish, whether
    /// its microphones are drawn.
    Kit {
        kit: KitRef,
        def: Box<KitDef>,
        finish: Finish,
        mics: bool,
    },
}

/// One scene to draw, and what is lit in it.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneView {
    pub subject: Subject,
    pub room: Room,
    /// The floor, walls and rug (each `None` the room's own).
    pub look: RoomLook,
    /// A dark theme's evening, or a light one's daylight.
    pub dark: bool,
    /// The selected piece or mic, drawn with the accent outline.
    pub selected: Option<Picked>,
    /// What the pointer is over: a quieter outline.
    pub hovered: Option<Picked>,
    /// The pieces sounding now (a short flash), by piece index.
    pub sounding: Vec<u16>,
    /// The theme's accent (the selection) and ink (the hover).
    pub accent: egui::Color32,
    pub ink: egui::Color32,
}

/// What is under a point of the scene.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Picked {
    /// A piece of a kit, by its index (`p{n}`); the zone of it struck (0 the centre, 1 the
    /// edge, 2 the rim; a cymbal's 0 the bell, 1 the bow, 2 the edge) when the point says
    /// one.
    Piece { piece: u16, zone: Option<u8> },
    /// A microphone (a named output).
    Mic(OutputId),
    /// A piano's lid.
    Lid,
}

/// A finish as the scene draws it.
pub fn finish_mat(f: Finish) -> Mat {
    match f {
        Finish::Black => pal::FINISH_BLACK,
        Finish::White => pal::FINISH_WHITE,
        Finish::Walnut => pal::FINISH_WALNUT,
        Finish::Rosewood => pal::FINISH_ROSEWOOD,
        Finish::Maple => pal::FINISH_MAPLE,
    }
}

/// What a build depends on: the view less what only the overlay draws.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Want {
    pub subject: Subject,
    pub room: Room,
    pub look: RoomLook,
    pub dark: bool,
    pub size: Vec2,
    pub ppp: f32,
    pub backend: SceneBackend,
    /// A framing other than the subject's own (the designer's picture-alone sheets).
    pub frame: Option<compose::Frame>,
    /// A per-pixel renderer's quick bake (a control gliding).
    pub quick: bool,
}

impl Want {
    pub fn of(v: &SceneView, size: Vec2, ppp: f32) -> Want {
        Want {
            subject: v.subject.clone(),
            room: v.room,
            look: v.look,
            dark: v.dark,
            size,
            ppp,
            backend: backend::current(),
            frame: None,
            quick: false,
        }
    }
}

/// Build the scene `w` (its display list, outlines and picks; no pixels).
pub(crate) fn build(w: &Want) -> compose::Built {
    let st = compose::Stage {
        room: w.room,
        look: w.look,
        evening: w.dark,
        size: w.size,
        ppp: w.ppp,
        capture: w.backend == SceneBackend::PerPixel,
        frame: w.frame.unwrap_or(match w.subject {
            Subject::Piano { .. } => compose::PIANO_FRAME,
            Subject::Kit { .. } => compose::KIT_FRAME,
        }),
    };
    let mut b = match &w.subject {
        Subject::Piano {
            look,
            lid,
            persp,
            width,
            room_level,
            mics,
        } => {
            let full = if look.body == Body::Upright {
                piano::UPRIGHT_LID
            } else {
                piano::GRAND_LID
            };
            let p = compose::PianoSpec {
                body: look.body,
                finish: finish_mat(look.finish),
                white: look.finish == Finish::White,
                black: look.finish == Finish::Black,
                lid: lid.clamp(0.0, 1.0) * full,
                persp: *persp,
                width: *width,
                room_level: *room_level,
                mics: *mics,
            };
            compose::piano_scene(&p, &st)
        }
        Subject::Kit {
            def, finish, mics, ..
        } => {
            let pieces = kit::of_def(def);
            let km = mics.then(|| kit_mics(def));
            compose::kit_scene(&pieces, &finish_mat(*finish), km.as_deref(), &st)
        }
    };
    if let Some(m) = &mut b.model {
        m.subject = Some(w.subject.clone());
    }
    b
}

/// A kit definition's microphones as the scene draws them (the studio kit's when the
/// definition has none).
pub fn kit_mics(def: &KitDef) -> Vec<gear::KitMic> {
    if def.mics.is_empty() || def.pieces.is_empty() {
        return gear::studio_mics();
    }
    use cantus_core::OutputFamily as F;
    def.mics
        .iter()
        .map(|m| {
            let p = geo::v(m.place.x, m.place.z, m.place.y);
            let (az, el) = (m.place.azimuth.to_radians(), m.place.elevation.to_radians());
            let aim = geo::v(az.sin() * el.cos(), el.sin(), -az.cos() * el.cos());
            let kind = match (m.family, m.channels) {
                (F::Overheads, _) => gear::MicKind::Pair,
                (F::Rooms, 2) => gear::MicKind::LdcPair,
                (F::Rooms, _) => gear::MicKind::Ribbon,
                (F::Kick, _) => gear::MicKind::Ldc,
                (F::HiHat | F::Cymbals, _) => gear::MicKind::Pencil,
                _ => gear::MicKind::Dynamic,
            };
            let clip = kind == gear::MicKind::Dynamic;
            let stand = (!clip && !matches!(kind, gear::MicKind::Pair | gear::MicKind::LdcPair))
                .then(|| geo::v(p.x, 0.0, p.z + 20.0));
            gear::KitMic {
                p,
                aim,
                kind,
                stand,
                clip,
                spread: if kind == gear::MicKind::Pair {
                    56.0
                } else {
                    230.0
                },
            }
        })
        .collect()
}

/// The output a microphone part stands for.
fn mic_output(w: &Want, part: Part) -> Option<OutputId> {
    match (&w.subject, part) {
        (Subject::Kit { def, .. }, Part::Mic(i)) => {
            Some(def.mics.get(i as usize).map_or(OutputId(i), |m| m.output))
        }
        (Subject::Piano { persp, .. }, Part::Pair) => {
            // Chorda's model outputs (ADR 0125): hammers, player, audience; the room pair.
            Some(OutputId(if *persp < 0.18 {
                0
            } else if *persp < 0.6 {
                1
            } else {
                2
            }))
        }
        (Subject::Piano { .. }, Part::RoomPair) => Some(OutputId(3)),
        _ => None,
    }
}

/// What the built scene shows at `p` (points from the picture's corner).
pub(crate) fn pick_in(w: &Want, b: &compose::Built, p: Pos2) -> Option<Picked> {
    let (part, _) = b.list.part_at(p)?;
    match part {
        Part::Piece(i) => {
            let piece = b.pieces.iter().find(|q| q.index == i)?;
            let zone = kit::zone_at(&b.cam, piece, p).map(|r| match piece.kind {
                kit::Kind::Cymbal | kit::Kind::Hats => {
                    if r < 0.22 {
                        0
                    } else if r < 0.88 {
                        1
                    } else {
                        2
                    }
                }
                _ => {
                    if r < 0.6 {
                        0
                    } else if r < 0.97 {
                        1
                    } else {
                        2
                    }
                }
            });
            Some(Picked::Piece { piece: i, zone })
        }
        Part::Lid => Some(Picked::Lid),
        Part::Mic(_) | Part::Pair | Part::RoomPair => mic_output(w, part).map(Picked::Mic),
        _ => None,
    }
}

/// Whether the module draws the scenes (a caller's stand-in, such as Tympanum's plan from
/// above, retires on it): true since round 40 phase 3's port.
pub const DRAWS: bool = true;

/// Where a tag of `p` stands in a build (points from the picture's corner): a microphone's
/// head; a piece's, the lid's top middle — the middle of its outline's top edge, so a tag
/// put just above it points at the piece.
pub(crate) fn anchor_in(w: &Want, b: &compose::Built, p: Picked) -> Option<Pos2> {
    let part = match p {
        Picked::Piece { piece, .. } => Part::Piece(piece),
        Picked::Lid => Part::Lid,
        Picked::Mic(o) => {
            return b
                .mics
                .iter()
                .find(|m| mic_output(w, m.0) == Some(o))
                .map(|m| m.1);
        }
    };
    let o = &b.outlines.iter().find(|o| o.0 == part)?.2;
    let (x0, y0, x1, _) = geo::bbox(o);
    let cx = (x0 + x1) * 0.5;
    // The outline's highest point near its middle column (a cymbal's rim, a drum's hoop).
    let top = o
        .iter()
        .filter(|q| (q.x - cx).abs() <= (x1 - x0) * 0.2 + 1.0)
        .map(|q| q.y)
        .fold(f32::MAX, f32::min);
    Some(Pos2::new(
        cx,
        if top.is_finite() && top < f32::MAX {
            top
        } else {
            y0
        },
    ))
}

/// Where a tag of `p` stands when `view` is drawn into `rect` (screen points), from the
/// same geometry that is drawn — built here; a view on screen asks its [`Scene`]
/// (`Scene::anchor`), which answers from the build it shows without building.
pub fn anchor(rect: Rect, view: &SceneView, p: Picked) -> Option<Pos2> {
    let w = Want::of(view, rect.size(), 1.0);
    let b = build(&w);
    anchor_in(&w, &b, p).map(|q| q + rect.min.to_vec2())
}

/// What `view`, drawn into `rect`, shows at `point` — from the same projected geometry that
/// is drawn (built here; a [`Scene`] keeps its build and answers from it).
pub fn pick(rect: Rect, view: &SceneView, point: Pos2) -> Option<Picked> {
    let w = Want::of(view, rect.size(), 1.0);
    let b = build(&w);
    pick_in(&w, &b, (point - rect.min).to_pos2())
}

/// The picture of `view` `size` points large at `ppp` pixels a point (screenshots, tests).
pub fn image(view: &SceneView, size: Vec2, ppp: f32) -> egui::ColorImage {
    let w = Want::of(view, size, ppp);
    let b = build(&w);
    live::to_image(&b, ppp, false)
}

/// [`image`] framed by `frame` instead of the subject's own framing (the designer's
/// picture-alone sheets fit the piano wider than the play view does).
pub fn image_framed(
    view: &SceneView,
    size: Vec2,
    ppp: f32,
    frame: compose::Frame,
) -> egui::ColorImage {
    let mut w = Want::of(view, size, ppp);
    w.frame = Some(frame);
    let b = build(&w);
    live::to_image(&b, ppp, false)
}

/// What a build of `view` holds (tests): its ops, whether every point is finite, the box
/// the instrument's outlines (its pieces, lid, keys, bench) cover on screen.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Stats {
    pub ops: usize,
    pub finite: bool,
    pub instrument: (f32, f32, f32, f32),
}

/// The [`Stats`] of `view` built `size` points large.
pub fn stats(view: &SceneView, size: Vec2) -> Stats {
    let w = Want::of(view, size, 1.0);
    let b = build(&w);
    let ok = |p: &Pos2| p.x.is_finite() && p.y.is_finite();
    let finite = b.list.ops.iter().all(|o| match &o.shape {
        display::Shape::Poly { pts, .. } | display::Shape::Polyline { pts, .. } => {
            pts.iter().all(ok)
        }
        display::Shape::Line { a, b, w, .. } => ok(a) && ok(b) && w.is_finite(),
        display::Shape::Ellipse { c, rx, ry } => ok(c) && rx.is_finite() && ry.is_finite(),
    });
    let pts: Vec<Pos2> = b
        .outlines
        .iter()
        .filter(|o| !matches!(o.0, Part::Mic(_) | Part::Pair | Part::RoomPair))
        .flat_map(|o| o.2.iter().copied())
        .collect();
    Stats {
        ops: b.list.ops.len(),
        finite,
        instrument: geo::bbox(&pts),
    }
}
