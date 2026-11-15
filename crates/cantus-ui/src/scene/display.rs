//! The display list (port of `scene/paint.py`): every model emits ops here — a filled
//! polygon, a stroked line or polyline, an ellipse — painted with a flat colour or a linear
//! or radial gradient, on a layer, at a depth, owned by a part (for the pick and the
//! selection). `draw.rs` rasterises it; `pick` reads its hit regions.

use super::geo::inside;
use crate::kit::skin::scene::C3;
use egui::Pos2;

/// The layers, back to front.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Layer {
    /// The walls and what is on them (drawn a touch soft).
    Far,
    /// The air's haze over the far walls.
    Haze,
    /// The floor.
    Back,
    /// The models again through the mirrored camera: the floor's reflection (blurred,
    /// faint, fading below the contact line).
    Mirror,
    /// Over the mirror: a rug reflects nothing.
    Rug,
    /// The area light's wide penumbra, its core, the contacts under every foot.
    ShadowWide,
    Shadow,
    Contact,
    /// The instrument, its hardware and microphones: sorted far to near.
    Objects,
    /// The light's falloff.
    Front,
}

impl Layer {
    pub const ALL: [Layer; 10] = [
        Layer::Far,
        Layer::Haze,
        Layer::Back,
        Layer::Mirror,
        Layer::Rug,
        Layer::ShadowWide,
        Layer::Shadow,
        Layer::Contact,
        Layer::Objects,
        Layer::Front,
    ];
}

/// What an op belongs to: what a click on it picks.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Part {
    #[default]
    None,
    /// A kit's piece, by its index in the kit.
    Piece(u16),
    /// A microphone, by its index in the scene's list.
    Mic(u16),
    Piano,
    Lid,
    Keys,
    Bench,
    /// The piano's main pair, and the room pair.
    Pair,
    RoomPair,
}

/// A gradient stop: offset, colour, alpha.
pub type Stop = (f32, C3, f32);

/// How an op is painted.
#[derive(Clone, Debug, PartialEq)]
pub enum Paint {
    Solid(C3),
    /// From `a` to `b`.
    Lin {
        a: Pos2,
        b: Pos2,
        stops: Vec<Stop>,
    },
    /// Round `c`, radii `rx`, `ry`.
    Rad {
        c: Pos2,
        rx: f32,
        ry: f32,
        stops: Vec<Stop>,
    },
}

/// A linear gradient from (x1, y1) to (x2, y2), stops opaque.
pub fn lin(a: Pos2, b: Pos2, stops: &[(f32, C3)]) -> Paint {
    Paint::Lin {
        a,
        b,
        stops: stops.iter().map(|&(o, c)| (o, c, 1.0)).collect(),
    }
}

/// A linear gradient with alphas.
pub fn lin_a(a: Pos2, b: Pos2, stops: &[Stop]) -> Paint {
    Paint::Lin {
        a,
        b,
        stops: stops.to_vec(),
    }
}

/// A radial gradient with alphas.
pub fn rad(c: Pos2, rx: f32, ry: f32, stops: &[Stop]) -> Paint {
    Paint::Rad {
        c,
        rx,
        ry,
        stops: stops.to_vec(),
    }
}

/// How a stroke ends.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Cap {
    Round,
    Butt,
}

/// What an op draws.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// A filled polygon; `seam` a stroke of its own paint that closes the hairline gaps
    /// between adjacent facets; `clip` a polygon it is clipped to.
    Poly {
        pts: Vec<Pos2>,
        seam: f32,
        clip: Option<Vec<Pos2>>,
    },
    Line {
        a: Pos2,
        b: Pos2,
        w: f32,
        cap: Cap,
    },
    Polyline {
        pts: Vec<Pos2>,
        w: f32,
        closed: bool,
    },
    Ellipse {
        c: Pos2,
        rx: f32,
        ry: f32,
    },
}

/// One op.
#[derive(Clone, Debug, PartialEq)]
pub struct Op {
    pub shape: Shape,
    pub paint: Paint,
    pub op: f32,
    pub layer: Layer,
    pub depth: f32,
    pub seq: u32,
    pub part: Part,
}

impl Op {
    /// The op's box on screen (stroke widths included).
    pub fn bounds(&self) -> (f32, f32, f32, f32) {
        let grow = |b: (f32, f32, f32, f32), m: f32| (b.0 - m, b.1 - m, b.2 + m, b.3 + m);
        match &self.shape {
            Shape::Poly { pts, seam, .. } => grow(super::geo::bbox(pts), *seam),
            Shape::Line { a, b, w, .. } => grow(super::geo::bbox(&[*a, *b]), *w),
            Shape::Polyline { pts, w, .. } => grow(super::geo::bbox(pts), *w),
            Shape::Ellipse { c, rx, ry } => (c.x - rx, c.y - ry, c.x + rx, c.y + ry),
        }
    }
}

/// A hit region: a part's outline on screen at its depth.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub depth: f32,
    pub part: Part,
    pub outline: Vec<Pos2>,
}

/// The display list being built: the ops, where the next ones go, the hit regions.
#[derive(Clone, Debug, Default)]
pub struct List {
    pub ops: Vec<Op>,
    pub layer: Option<Layer>,
    pub depth: f32,
    pub part: Part,
    seq: u32,
    pub hits: Vec<Hit>,
    /// A layer every op goes to (the mirror pass).
    pub force: Option<Layer>,
    /// The reflection's fade on screen: where it starts, where it has faded.
    pub mirror_fade: Option<(f32, f32)>,
}

impl List {
    fn layer_now(&self) -> Layer {
        self.force.or(self.layer).unwrap_or(Layer::Objects)
    }

    /// Every op after this shares `depth` and `part`, on the objects layer.
    pub fn at(&mut self, depth: f32, part: Part) {
        self.depth = depth;
        self.part = part;
        self.layer = Some(Layer::Objects);
    }

    pub fn on(&mut self, layer: Layer) {
        self.layer = Some(layer);
    }

    fn push(&mut self, shape: Shape, paint: Paint, op: f32) {
        self.seq += 1;
        let layer = self.layer_now();
        self.ops.push(Op {
            shape,
            paint,
            op,
            layer,
            depth: self.depth,
            seq: self.seq,
            part: self.part,
        });
    }

    pub fn poly(&mut self, pts: Vec<Pos2>, paint: Paint, op: f32, seam: f32) {
        self.push(
            Shape::Poly {
                pts,
                seam,
                clip: None,
            },
            paint,
            op,
        );
    }

    pub fn poly_clip(&mut self, pts: Vec<Pos2>, paint: Paint, op: f32, clip: Vec<Pos2>) {
        self.push(
            Shape::Poly {
                pts,
                seam: 0.0,
                clip: Some(clip),
            },
            paint,
            op,
        );
    }

    pub fn line(&mut self, a: Pos2, b: Pos2, w: f32, paint: Paint, cap: Cap, op: f32) {
        self.push(Shape::Line { a, b, w, cap }, paint, op);
    }

    pub fn polyline(&mut self, pts: Vec<Pos2>, w: f32, paint: Paint, op: f32, closed: bool) {
        self.push(Shape::Polyline { pts, w, closed }, paint, op);
    }

    pub fn ellipse(&mut self, c: Pos2, rx: f32, ry: f32, paint: Paint, op: f32) {
        self.push(Shape::Ellipse { c, rx, ry }, paint, op);
    }

    /// A part's outline on screen at its depth (none from the mirror's pass).
    pub fn hit(&mut self, part: Part, outline: Vec<Pos2>, depth: f32) {
        if self.force.is_some() {
            return;
        }
        self.hits.push(Hit {
            depth,
            part,
            outline,
        });
    }

    /// The ops of `layer`, in the order they are drawn (objects and the mirror sorted far
    /// to near, then by emission).
    pub fn layer_ops(&self, layer: Layer) -> Vec<&Op> {
        let mut v: Vec<&Op> = self.ops.iter().filter(|o| o.layer == layer).collect();
        if matches!(layer, Layer::Objects | Layer::Mirror) {
            v.sort_by(|a, b| b.depth.total_cmp(&a.depth).then(a.seq.cmp(&b.seq)));
        }
        v
    }

    /// The nearest part whose outline holds `p`.
    pub fn part_at(&self, p: Pos2) -> Option<(Part, &Hit)> {
        let mut best: Option<&Hit> = None;
        for h in &self.hits {
            if inside(&h.outline, p) && best.is_none_or(|b| h.depth < b.depth) {
                best = Some(h);
            }
        }
        best.map(|h| (h.part, h))
    }
}
