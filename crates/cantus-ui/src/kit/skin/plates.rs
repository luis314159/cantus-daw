//! A unit's nameplate and the small things on its face (round 32, ADR 0074): how its name
//! sits in the header ([`Plate`], set in its [`Lettering`](super::Lettering)) and the
//! [`Ornaments`](super::Ornaments) — screws, vents, rack ears — on its ground. The rules
//! between card groups are the cards' (`band::card`, `band::section`).

use super::super::brush::{Brush, Fill};
use super::super::color::{mix, on_fill, shade, with_alpha};
use super::super::light::{self, RaisedOpts};
use super::super::text::{self, Elide, Policy, Role, Text};
use super::super::{Style, Tok};
use super::Plate;
use egui::{Align2, Color32, Context, Pos2, Rect, Vec2};

impl Plate {
    /// Every plate, the kit's first.
    pub const ALL: [Plate; 5] = [
        Plate::Plain,
        Plate::Engraved,
        Plate::Badge,
        Plate::Stripe,
        Plate::Legend,
    ];

    /// The name the gallery and the shots give it.
    pub fn name(self) -> &'static str {
        match self {
            Plate::Plain => "plain",
            Plate::Engraved => "engraved",
            Plate::Badge => "badge",
            Plate::Stripe => "stripe",
            Plate::Legend => "legend",
        }
    }

    /// The room a plate takes beside its letters, at 100 %.
    fn pad(self) -> f32 {
        match self {
            Plate::Badge => 8.0,
            Plate::Legend => 6.0,
            _ => 0.0,
        }
    }
}

/// The tokens a nameplate paints with (for inspect mode), beside the device hue.
pub fn tokens(plate: Plate) -> &'static [Tok] {
    match plate {
        Plate::Plain => &[],
        Plate::Engraved => &[Tok::Label, Tok::Panel, Tok::Light, Tok::Dark],
        Plate::Badge => &[],
        Plate::Stripe => &[Tok::Text, Tok::Light],
        Plate::Legend => &[Tok::Label],
    }
}

/// The name as the plate sets it: its text, its lettering.
fn name_text<'a>(s: &Style, name: &'a str, color: Color32) -> (Text<'a>, text::Face) {
    let (face, caps) = s.skin.lettering.title();
    let t = Text::new(name, Role::Section, color)
        .caps(caps)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    (t, face)
}

/// The width the name takes on its plate at the style's scale (letters and the plate's
/// own room) — what the header keeps for it.
pub fn name_width(ctx: &Context, s: &Style, name: &str) -> f32 {
    let (t, face) = name_text(s, name, s.t.text);
    text::measure_face(ctx, &t, s.tk(), face).x + 2.0 * s.s(s.skin.plate.pad())
}

/// Paint the unit's name in `area` (the header's, from its left), in the device `hue`.
/// Returns the width it took.
pub fn nameplate(b: &mut Brush, area: Rect, s: &Style, name: &str, hue: Color32) -> f32 {
    let t = &s.t;
    let plate = s.skin.plate;
    let pad = s.s(plate.pad());
    let w = name_width(b.ctx(), s, name).min(area.width());
    let letters = Rect::from_min_size(
        Pos2::new(area.left() + pad, area.top()),
        Vec2::new((w - 2.0 * pad).max(0.0) + 1.0, area.height()),
    );
    b.register(
        Rect::from_min_size(area.min, Vec2::new(w, area.height())),
        "nameplate",
        tokens(plate),
        // Engraved and screen-printed letters are the face's own, not the hue.
        &[(!matches!(plate, Plate::Engraved | Plate::Legend))
            .then_some(s.hue_tok)
            .flatten()],
    );
    let px = b.px();
    match plate {
        Plate::Plain | Plate::Stripe => {
            // On a stripe the name is in the text colour: the stripe carries the hue.
            let color = if plate == Plate::Stripe { t.text } else { hue };
            let (tx, face) = name_text(s, name, color);
            let _ = text::paint_face(b, letters, &tx, s.tk(), face);
        }
        Plate::Engraved => {
            // Cut into the face: the cut's upper wall in shade, its lower lip lit.
            let (fill, lip, wall) = if t.dark {
                (
                    t.label,
                    with_alpha(t.light, 0.10),
                    with_alpha(t.dark_shade, 0.85),
                )
            } else {
                (
                    shade(t.label, -0.10),
                    with_alpha(t.light, 0.85),
                    with_alpha(t.dark_shade, 0.25),
                )
            };
            let _ = shade(t.panel, 0.0);
            for (off, c) in [(px, lip), (-px, wall), (0.0, fill)] {
                let (tx, face) = name_text(s, name, c);
                let _ =
                    text::paint_face(b, letters.translate(Vec2::new(0.0, off)), &tx, s.tk(), face);
            }
        }
        Plate::Badge => {
            let badge = Rect::from_min_size(
                Pos2::new(area.left(), area.center().y - s.s(9.0)),
                Vec2::new(w, s.s(18.0)),
            );
            light::raised(
                b,
                badge,
                s.r(3.0),
                hue,
                RaisedOpts {
                    drop: true,
                    k: s.k,
                    depth: t.depth,
                    ..Default::default()
                },
            );
            let (tx, face) = name_text(s, name, on_fill(hue));
            let _ = text::paint_face(b, letters, &tx, s.tk(), face);
        }
        Plate::Legend => {
            let boxed = Rect::from_min_size(
                Pos2::new(area.left(), area.center().y - s.s(8.0)),
                Vec2::new(w, s.s(16.0)),
            );
            let boxed = b.snap_rect(boxed);
            b.rrect_stroke(
                boxed.shrink(px * 0.5),
                s.r(1.5),
                px.max(s.s(1.0)),
                with_alpha(t.label, 0.85),
            );
            let (tx, face) = name_text(s, name, t.label);
            let _ = text::paint_face(b, letters, &tx, s.tk(), face);
        }
    }
    w
}

/// The stripe a [`Plate::Stripe`] paints across the header's foot: the hue, and the
/// second accent (or a lighter hue) above it.
pub fn stripe(b: &mut Brush, header: Rect, s: &Style, hue: Color32) {
    if s.skin.plate != Plate::Stripe {
        return;
    }
    let header = b.snap_rect(header);
    let h = b.snap(s.s(3.0)).max(b.px() * 2.0);
    let foot = Rect::from_min_max(Pos2::new(header.left(), header.bottom() - h), header.max);
    b.rect(foot, Fill::V(shade(hue, 0.04), shade(hue, -0.06)));
    let second = s
        .skin
        .accent2_in(s.t.dark)
        .unwrap_or_else(|| mix(hue, s.t.light, 0.35));
    let line = b.snap(s.s(1.0)).max(b.px());
    b.rect(
        Rect::from_min_max(
            Pos2::new(header.left(), foot.top() - line - b.px()),
            Pos2::new(header.right(), foot.top() - b.px()),
        ),
        Fill::Solid(second),
    );
}

/// The tokens the ornaments paint with.
pub const ORNAMENT_TOKENS: &[Tok] = &[Tok::DeviceGround, Tok::KnobBody, Tok::Light, Tok::Dark];

/// The face's ornaments on the device ground `body` (inside the grid's padding, so no
/// card covers them): rack ears at the sides, screws in the corners (in the ears when
/// there are ears), vent slots along the top.
pub fn ornaments(b: &mut Brush, body: Rect, s: &Style) {
    let o = s.skin.ornaments;
    if !(o.screws || o.vents || o.rack_ears) {
        return;
    }
    b.register(body, "ornaments", ORNAMENT_TOKENS, &[]);
    let t = &s.t;
    let pad = s.d(8.0);
    let px = b.px();
    let d = t.depth.clamp(0.0, 1.5);
    let ground = t.device_ground;
    let metal = mix(t.knob_body, t.light, 0.25);
    if o.rack_ears {
        for left in [true, false] {
            let x0 = if left {
                body.left()
            } else {
                body.right() - pad + px
            };
            let ear = b.snap_rect(Rect::from_min_max(
                Pos2::new(x0, body.top()),
                Pos2::new(x0 + pad - px, body.bottom()),
            ));
            b.rect(
                ear,
                Fill::V(shade(ground, 0.07 * d), shade(ground, -0.03 * d)),
            );
            let edge = if left { ear.right() } else { ear.left() };
            b.vline(
                b.snap_mid(edge),
                ear.top(),
                ear.bottom(),
                px,
                with_alpha(t.dark_shade, 0.5),
            );
            if !o.screws {
                // The ear's slots, where a rack's screws would go.
                for y in [ear.top() + s.s(18.0), ear.bottom() - s.s(18.0)] {
                    let slot = Rect::from_center_size(
                        Pos2::new(ear.center().x, y),
                        Vec2::new(ear.width() * 0.5, s.s(9.0)),
                    );
                    b.rrect(
                        b.snap_rect(slot),
                        ear.width() * 0.25,
                        Fill::Solid(shade(ground, -0.32)),
                    );
                }
            }
        }
    }
    if o.screws {
        let rr = (pad * 0.36).min(s.s(3.2));
        let inset = if o.rack_ears {
            Vec2::new(pad * 0.5, s.s(18.0))
        } else {
            Vec2::splat(pad * 0.5)
        };
        let corners = [
            Pos2::new(body.left() + inset.x, body.top() + inset.y),
            Pos2::new(body.right() - inset.x, body.top() + inset.y),
            Pos2::new(body.left() + inset.x, body.bottom() - inset.y),
            Pos2::new(body.right() - inset.x, body.bottom() - inset.y),
        ];
        // Each screw left at its own turn, as a hand would.
        let turns = [0.45f32, -0.7, 1.2, 0.15];
        for (c, a) in corners.into_iter().zip(turns) {
            let c = Pos2::new(b.snap_mid(c.x), b.snap_mid(c.y));
            b.circle(
                c + Vec2::new(0.0, px),
                rr + px,
                Fill::Solid(with_alpha(t.dark_shade, 0.35 * d)),
            );
            b.circle(
                c,
                rr,
                Fill::V(shade(metal, 0.14 * d), shade(metal, -0.18 * d)),
            );
            b.circle_stroke(c, rr, px, with_alpha(t.dark_shade, 0.55));
            let dir = Vec2::angled(a) * (rr * 0.72);
            b.segment(
                c - dir + Vec2::new(0.0, px),
                c + dir + Vec2::new(0.0, px),
                px * 1.2,
                with_alpha(t.light, 0.3 * d),
                false,
            );
            b.segment(
                c - dir,
                c + dir,
                (rr * 0.32).max(px),
                shade(metal, -0.42),
                false,
            );
        }
    }
    if o.vents {
        let h = (pad * 0.42).max(2.0 * px);
        let (w, gap) = (s.s(16.0), s.s(5.0));
        // As many as fit the middle half of the face, nine at most.
        let n = (((body.width() * 0.5 + gap) / (w + gap)).floor() as usize).clamp(1, 9);
        let total = n as f32 * w + (n as f32 - 1.0) * gap;
        let x0 = body.center().x - total * 0.5;
        let y = body.top() + pad * 0.5;
        for i in 0..n {
            let slot = b.snap_rect(Rect::from_center_size(
                Pos2::new(x0 + w * 0.5 + i as f32 * (w + gap), y),
                Vec2::new(w, h),
            ));
            b.rrect(
                slot,
                h * 0.5,
                Fill::V(shade(ground, -0.40), shade(ground, -0.28)),
            );
            b.hline(
                slot.left() + h * 0.5,
                slot.right() - h * 0.5,
                slot.bottom() + px * 0.5,
                px,
                with_alpha(t.light, 0.10 * d),
            );
        }
    }
}
