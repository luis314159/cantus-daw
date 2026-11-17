//! Loquax's two other objects (phase B, the owner's v2 verdict: "all three", chosen like a
//! colourway, kept per insert in `obj`): the **talk-box** — its driver in a housing on the
//! floor, the clear tube rising from the driver's nozzle and clipped to a stand, its open end
//! where the mouth would be — and the **desk microphone** on its weighted base (a cast base
//! with its rubber foot and talk bar, a chrome stem with its height lock, the yoke and its
//! pivot knobs, the capsule's slotted grille). Each with the gooseneck's finish and detail:
//! the on-air lamp and the voice's glow and arcs follow the voice's level; the housing and
//! the base wear the colourway's finish (brushed aluminium, black lacquer with chrome lips,
//! the road case's steel corners). Vectors at the style's scale, colours from
//! `kit::skin::consoles`. Original to Cantus.

use super::art::{self, c, cubic, cylinder, hash, NECK};
use crate::kit::color::{mix, with_alpha};
use crate::kit::skin::consoles as pal;
use crate::kit::skin::Material;
use crate::kit::{Brush, Fill, Style};
use egui::{pos2, vec2, Color32, Pos2, Rect, Vec2};
use std::f32::consts::PI;

/// The voice reaching `centre` from the left: arcs, fainter outward.
fn arcs(b: &mut Brush, centre: Pos2, rad: f32, k: f32, lv: f32, glow: Color32) {
    if lv <= 0.01 {
        return;
    }
    for i in 0..4 {
        b.arc(
            centre,
            rad + i as f32 * 5.0 * k,
            PI - 0.5,
            PI + 0.5,
            0.9 * k,
            with_alpha(glow, lv * (0.75 - 0.17 * i as f32)),
            true,
        );
    }
}

/// An ellipse's outline round `c`, radii `r`, as points for [`Brush::polygon`].
fn ellipse(c0: Pos2, r: Vec2) -> [Pos2; 24] {
    let mut p = [Pos2::ZERO; 24];
    for (i, q) in p.iter_mut().enumerate() {
        let a = i as f32 / 24.0 * 2.0 * PI;
        *q = c0 + vec2(r.x * a.cos(), r.y * a.sin());
    }
    p
}

/// A block in the colourway's finish (the talk-box's housing, the desk microphone's base):
/// its shadow, the face lit at the top, the brushed grain or lacquer's sheen, the chamfer's
/// light; lacquer's chrome lip, the road case's steel corners and rivets, else two screws.
pub(super) fn housing(b: &mut Brush, s: &Style, r: Rect, rad: f32) {
    let k = s.k;
    let f = art::finish(s);
    b.rrect_soft(
        r.translate(vec2(0.0, 2.5 * k)),
        rad + 1.0 * k,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.55)),
        7.0 * k,
    );
    b.rrect(r, rad, Fill::V(mix(c(f.face), c(f.top), 0.45), c(f.foot)));
    let lacquer = s.skin.material == Material::Lacquer;
    let mut i = 0u32;
    let mut y = r.top() + 1.2 * k;
    while y < r.bottom() - 1.2 * k {
        let a = if lacquer {
            0.0
        } else {
            0.04 + 0.16 * hash(i + 41)
        };
        if a > 0.0 {
            b.hline(
                r.left() + rad * 0.6,
                r.right() - rad * 0.6,
                y,
                0.5 * k,
                with_alpha(c(f.grain), a),
            );
        }
        y += 1.3 * k;
        i += 1;
    }
    if lacquer {
        // The deep shine across the top third, then the chrome lip round the edge.
        let t = Rect::from_min_max(
            pos2(r.left() + 2.0 * k, r.top() + 1.2 * k),
            pos2(r.right() - 2.0 * k, r.top() + r.height() * 0.38),
        );
        b.rrect(
            t,
            rad * 0.7,
            Fill::V(with_alpha(c(f.grain), 0.24), with_alpha(c(f.grain), 0.0)),
        );
        b.rrect_stroke(
            r.shrink(0.8 * k),
            rad,
            0.9 * k,
            with_alpha(c(pal::CHROME), 0.7),
        );
    }
    b.hline(
        r.left() + rad,
        r.right() - rad,
        r.top() + 0.6 * k,
        1.0 * k,
        c(f.top),
    );
    b.rrect_stroke(r, rad, 0.6 * k, with_alpha(c(f.top), 0.45));
    if matches!(s.skin.material, Material::FineTolex | Material::Tolex) {
        // The road case's corners: steel caps over the top corners, riveted.
        let sz = (r.height() * 0.42).min(16.0 * k);
        for (j, x) in [r.left(), r.right() - sz].into_iter().enumerate() {
            let cap = Rect::from_min_size(pos2(x, r.top()), vec2(sz, sz));
            b.rrect(cap, 2.0 * k, Fill::V(c(pal::CHROME), c(pal::CHROME_DARK)));
            b.rrect_stroke(cap, 2.0 * k, 0.6 * k, with_alpha(c(pal::CHROME_LIGHT), 0.6));
            let p = cap.center() + vec2(if j == 0 { -1.0 } else { 1.0 }, -1.0) * sz * 0.12;
            art::rivet(b, p, sz * 0.14);
        }
    } else {
        let sr = (r.height() * 0.07).clamp(1.6 * k, 3.2 * k);
        for (j, x) in [r.left() + sr * 2.6, r.right() - sr * 2.6]
            .into_iter()
            .enumerate()
        {
            art::screw(b, pos2(x, r.top() + sr * 2.6), sr, hash(j as u32 + 17));
        }
    }
}

/// A knurled chrome band across `r` (a lock collar, a pivot knob seen from the side).
fn knurl(b: &mut Brush, r: Rect, k: f32) {
    b.hrect(r, &cylinder());
    let mut x = r.left() + 1.0 * k;
    while x < r.right() - 0.6 * k {
        b.vline(
            x,
            r.top() + 0.6 * k,
            r.bottom() - 0.6 * k,
            0.45 * k,
            with_alpha(c(pal::CHROME_DARK), 0.55),
        );
        x += 1.5 * k;
    }
    b.hline(
        r.left(),
        r.right(),
        r.top() + 0.4 * k,
        0.7 * k,
        with_alpha(c(pal::CHROME_LIGHT), 0.85),
    );
}

/// The talk-box standing in `r`: the stand's rod behind, the driver's housing on the floor
/// (the on-air lamp, the footswitch, the jack), the driver's can on it with its nozzle, the
/// clear tube rising and curving to its clip on the rod, the open end where the mouth
/// would be — `level` (0 … 1) the voice: the lamp, the light carried in the tube, the glow at
/// its mouth and the arcs, in `glow`. Gives the mouth's centre.
pub fn talk_box(b: &mut Brush, s: &Style, r: Rect, level: f32, glow: Color32) -> Pos2 {
    let k = s.k;
    let lv = level.clamp(0.0, 1.0);
    let (w, h) = (r.width(), r.height());
    // The housing on the floor, the driver's can on it.
    let hb = (h * 0.2).clamp(34.0 * k, 62.0 * k);
    let house = Rect::from_min_max(
        pos2(r.left() + w * 0.06, r.bottom() - hb),
        pos2(r.right() - w * 0.04, r.bottom()),
    );
    let xd = r.left() + w * 0.66;
    let cw = (w * 0.34).max(26.0 * k);
    let ch = (hb * 0.62).max(20.0 * k);
    let can = Rect::from_min_max(
        pos2(xd - cw * 0.5, house.top() - ch),
        pos2(xd + cw * 0.5, house.top() + 1.0 * k),
    );
    let nh = (ch * 0.32).max(7.0 * k);
    let nozzle = Rect::from_min_max(
        pos2(xd - cw * 0.12, can.top() - nh),
        pos2(xd + cw * 0.12, can.top() + 2.0 * k),
    );
    // The rod and the tube's mouth near the top, its open end toward the voice (left).
    let xr = r.left() + w * 0.40;
    let tw = (w * 0.085).max(6.0 * k);
    let my = r.top() + h * 0.15;
    let mouth = pos2(r.left() + w * 0.22, my);
    // The stand's rod behind everything, a chrome tube from the top down behind the housing.
    let rw = (w * 0.05).max(4.0 * k);
    let rod_top = my - h * 0.07;
    b.hrect(
        Rect::from_min_max(
            pos2(xr - rw * 0.5, rod_top),
            pos2(xr + rw * 0.5, house.top() + 2.0 * k),
        ),
        &cylinder(),
    );
    // Its cap.
    b.rrect(
        Rect::from_min_max(
            pos2(xr - rw * 0.7, rod_top - 3.0 * k),
            pos2(xr + rw * 0.7, rod_top + 1.0 * k),
        ),
        1.5 * k,
        Fill::V(c(pal::RUBBER_LIGHT), c(pal::RUBBER)),
    );
    let ctrl = [
        pos2(xd, nozzle.top() + 1.0 * k),
        pos2(xd + w * 0.04, nozzle.top() - (nozzle.top() - my) * 0.62),
        pos2(r.left() + w * 0.66, my),
        mouth,
    ];
    tube(b, k, ctrl, tw, lv, glow);
    // The clip holding the tube to the rod: a rubber saddle and its wing screw.
    let clip = Rect::from_center_size(pos2(xr, my), vec2(rw * 2.4, tw * 1.9));
    b.rrect(clip, 2.0 * k, Fill::V(c(pal::RUBBER_LIGHT), c(pal::RUBBER)));
    b.hline(
        clip.left() + 1.0 * k,
        clip.right() - 1.0 * k,
        clip.top() + 0.6 * k,
        0.6 * k,
        with_alpha(c(pal::CHROME_LIGHT), 0.35),
    );
    art::screw(
        b,
        pos2(xr, clip.bottom() + 3.0 * k),
        (rw * 0.55).max(2.0 * k),
        0.3,
    );
    // The open end, seen a little from the side: the cut's ring, dark inside, its glow.
    let end = vec2(tw * 0.28, tw * 0.55);
    if lv > 0.01 {
        b.radial(
            Rect::from_center_size(mouth, Vec2::splat(tw * 7.0)),
            mouth,
            Vec2::splat(tw * 2.6),
            &[
                (0.0, with_alpha(glow, 0.55 * lv)),
                (1.0, with_alpha(glow, 0.0)),
            ],
            10,
        );
    }
    b.polygon(&ellipse(mouth, end), c(pal::FOAM));
    b.polygon(
        &ellipse(mouth, end * 0.7),
        mix(c(pal::FOAM), glow, 0.55 * lv),
    );
    let ring = ellipse(mouth, end);
    let mut closed = [Pos2::ZERO; 25];
    closed[..24].copy_from_slice(&ring);
    closed[24] = ring[0];
    b.polyline(&closed, 1.0 * k, with_alpha(c(pal::TUBE), 0.85));
    arcs(b, mouth, tw * 1.6, k, lv, glow);
    // The housing in the colourway's finish, the driver on it.
    housing(b, s, house, 3.0 * k);
    driver(b, k, can, nozzle);
    // On the housing's face: the on-air lamp, the footswitch, the output jack.
    let fy = house.top() + house.height() * 0.6;
    art::lamp(
        b,
        pos2(house.left() + house.width() * 0.16, fy),
        (hb * 0.09).max(2.4 * k),
        glow,
        lv,
    );
    let sw = pos2(house.left() + house.width() * 0.5, fy);
    let swr = (hb * 0.17).max(4.0 * k);
    b.circle(
        sw + vec2(0.0, swr * 0.25),
        swr * 1.25,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.45)),
    );
    b.circle(sw, swr * 1.15, Fill::V(c(pal::CHROME_DARK), c(pal::CHROME)));
    b.circle(sw, swr, Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)));
    b.circle_stroke(sw, swr * 0.7, 0.5 * k, with_alpha(c(pal::CHROME_DARK), 0.7));
    b.circle(
        sw - vec2(swr, swr) * 0.3,
        swr * 0.28,
        Fill::Solid(with_alpha(c(pal::CHROME_LIGHT), 0.8)),
    );
    let jack = pos2(house.right() - house.width() * 0.13, fy);
    let jr = (hb * 0.08).max(2.2 * k);
    b.circle(
        jack,
        jr * 1.4,
        Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)),
    );
    b.circle(jack, jr * 0.75, Fill::Solid(c(pal::GASKET)));
    mouth
}

/// The clear tube along the cubic `ctrl`, `tw` thick: its shadow, the faint body, the light
/// it carries (the voice, `lv` in `glow`, brightest toward the mouth), its two walls and the
/// highlight on the side toward the light.
fn tube(b: &mut Brush, k: f32, ctrl: [Pos2; 4], tw: f32, lv: f32, glow: Color32) {
    let mut pts = [Pos2::ZERO; NECK + 1];
    for (i, p) in pts.iter_mut().enumerate() {
        *p = cubic(ctrl, i as f32 / NECK as f32);
    }
    let left = |i: usize| {
        let t = (pts[(i + 1).min(NECK)] - pts[i.saturating_sub(1)]).normalized();
        vec2(t.y, -t.x)
    };
    b.polyline(
        &pts.map(|p| p + vec2(2.5, 3.0) * k),
        tw * 0.9,
        with_alpha(c(pal::SHADOW), 0.3),
    );
    b.polyline(&pts, tw, with_alpha(c(pal::TUBE), 0.22));
    if lv > 0.01 {
        // The voice carried up the tube: four pieces, each brighter toward the mouth.
        let q = NECK / 4;
        for j in 0..4 {
            let a = (j * q).min(NECK);
            let z = ((j + 1) * q).min(NECK);
            b.glow_polyline(
                &pts[a..=z],
                tw * 0.34,
                with_alpha(glow, lv * (0.25 + 0.17 * j as f32)),
                with_alpha(glow, lv * (0.1 + 0.08 * j as f32)),
                tw * 0.3,
            );
        }
    }
    let mut wall_a = [Pos2::ZERO; NECK + 1];
    let mut wall_b = [Pos2::ZERO; NECK + 1];
    let mut shine = [Pos2::ZERO; NECK + 1];
    for i in 0..=NECK {
        wall_a[i] = pts[i] + left(i) * tw * 0.45;
        wall_b[i] = pts[i] - left(i) * tw * 0.45;
        shine[i] = pts[i] + left(i) * tw * 0.22;
    }
    b.polyline(&wall_a, 0.8 * k, with_alpha(c(pal::TUBE_EDGE), 0.85));
    b.polyline(&wall_b, 1.0 * k, with_alpha(c(pal::TUBE_EDGE), 0.95));
    b.polyline(&shine, 0.8 * k, with_alpha(c(pal::CHROME_LIGHT), 0.55));
}

/// The compression driver on the housing: a satin can with a chrome band and two screws,
/// its rubber gasket at the foot, the chrome nozzle on top the tube fits over.
fn driver(b: &mut Brush, k: f32, can: Rect, nozzle: Rect) {
    b.hrect(
        nozzle,
        &[
            (0.0, c(pal::CHROME_DARK)),
            (0.3, c(pal::CHROME_LIGHT)),
            (1.0, c(pal::CHROME_DARK)),
        ],
    );
    b.rrect(can, 3.0 * k, Fill::V(c(pal::SATIN_LIGHT), c(pal::SATIN)));
    b.hrect(
        can.shrink2(vec2(0.0, 1.0 * k)),
        &[
            (0.0, with_alpha(c(pal::SHADOW), 0.5)),
            (0.25, with_alpha(c(pal::SATIN_LIGHT), 0.25)),
            (0.5, with_alpha(c(pal::SHADOW), 0.0)),
            (1.0, with_alpha(c(pal::SHADOW), 0.6)),
        ],
    );
    // The top's rim, lit; the chrome band; the gasket at the foot.
    b.polygon(
        &ellipse(
            pos2(can.center().x, can.top() + 1.5 * k),
            vec2(can.width() * 0.5 - 1.0 * k, 2.2 * k),
        ),
        c(pal::SATIN_LIGHT),
    );
    let band = Rect::from_min_max(
        pos2(can.left(), can.top() + can.height() * 0.42),
        pos2(can.right(), can.top() + can.height() * 0.56),
    );
    b.hrect(band, &cylinder());
    for (j, x) in [band.left() + 4.0 * k, band.right() - 4.0 * k]
        .into_iter()
        .enumerate()
    {
        art::screw(b, pos2(x, band.center().y), 1.6 * k, hash(j as u32 + 29));
    }
    b.rect(
        Rect::from_min_max(
            pos2(can.left() - 1.5 * k, can.bottom() - 3.0 * k),
            pos2(can.right() + 1.5 * k, can.bottom()),
        ),
        Fill::V(c(pal::RUBBER_LIGHT), c(pal::RUBBER)),
    );
}

/// The desk microphone standing in `r`: the weighted base in the colourway's finish (its
/// rubber foot, the talk bar, the on-air lamp), the chrome stem with its knurled height lock,
/// the yoke and its two pivot knobs, the capsule — satin, chrome-banded, its slotted grille
/// lit from behind by the voice and the arcs reaching it; `level` (0 … 1) the voice, in
/// `glow`. Gives the grille's centre.
pub fn desk_mic(b: &mut Brush, s: &Style, r: Rect, level: f32, glow: Color32) -> Pos2 {
    let k = s.k;
    let lv = level.clamp(0.0, 1.0);
    let (w, h) = (r.width(), r.height());
    let cx = r.left() + w * 0.55;
    // The capsule: a tall pill near the top.
    let hw = (w * 0.44).min(h * 0.22).max(22.0 * k);
    let hh = hw * 1.6;
    let head = Rect::from_min_size(pos2(cx - hw * 0.5, r.top() + 6.0 * k), vec2(hw, hh));
    // The base at the foot: a cast block and the dome over it.
    let hb = (h * 0.17).clamp(28.0 * k, 50.0 * k);
    let base = Rect::from_min_max(
        pos2(r.left() + w * 0.06, r.bottom() - hb),
        pos2(r.right() - w * 0.04, r.bottom() - 3.0 * k),
    );
    // The stem from the base's dome up to the yoke under the capsule.
    let sw = (w * 0.07).max(5.0 * k);
    let yoke_y = head.bottom() + hw * 0.16;
    let dome_h = hb * 0.45;
    b.hrect(
        Rect::from_min_max(
            pos2(cx - sw * 0.5, yoke_y),
            pos2(cx + sw * 0.5, base.top() - dome_h * 0.5),
        ),
        &cylinder(),
    );
    let lock_y = yoke_y + (base.top() - yoke_y) * 0.55;
    knurl(
        b,
        Rect::from_min_max(
            pos2(cx - sw * 0.95, lock_y - 5.0 * k),
            pos2(cx + sw * 0.95, lock_y + 5.0 * k),
        ),
        k,
    );
    // The yoke: a chrome fork either side of the capsule, its crossbar on the stem.
    let arm = (hw * 0.09).max(2.5 * k);
    let pivot_y = head.center().y + hh * 0.08;
    for x in [head.left() - arm * 0.9, head.right() + arm * 0.9] {
        b.segment(pos2(x, pivot_y), pos2(x, yoke_y), arm, c(pal::CHROME), true);
        b.segment(
            pos2(x - arm * 0.25, pivot_y),
            pos2(x - arm * 0.25, yoke_y),
            arm * 0.3,
            with_alpha(c(pal::CHROME_LIGHT), 0.8),
            false,
        );
    }
    let bar = Rect::from_min_max(
        pos2(head.left() - arm * 1.4, yoke_y - arm * 0.5),
        pos2(head.right() + arm * 1.4, yoke_y + arm * 1.1),
    );
    b.rrect(
        bar,
        arm * 0.6,
        Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)),
    );
    capsule(b, k, head, lv, glow);
    // The pivot knobs over the arms, knurled at their edge.
    for x in [head.left() - arm * 0.9, head.right() + arm * 0.9] {
        let kr = (arm * 1.5).max(3.5 * k);
        let p = pos2(x, pivot_y);
        b.circle(
            p + vec2(0.0, kr * 0.25),
            kr * 1.1,
            Fill::Solid(with_alpha(c(pal::SHADOW), 0.45)),
        );
        b.circle(p, kr, Fill::V(c(pal::SATIN_LIGHT), c(pal::SATIN)));
        b.circle_stroke(p, kr * 0.92, 0.6 * k, with_alpha(c(pal::CHROME), 0.6));
        b.circle(
            p,
            kr * 0.45,
            Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)),
        );
    }
    // The dome the stem rises from, then the base, its foot and its talk bar.
    let dome = Rect::from_min_max(
        pos2(cx - w * 0.12, base.top() - dome_h),
        pos2(cx + w * 0.12, base.top() + 2.0 * k),
    );
    b.rrect(
        dome,
        dome_h * 0.45,
        Fill::V(c(pal::CHROME), c(pal::CHROME_DARK)),
    );
    b.hrect(dome.shrink2(vec2(0.0, dome_h * 0.3)), &cylinder());
    b.hline(
        dome.left() + dome_h * 0.4,
        dome.right() - dome_h * 0.4,
        dome.top() + 1.0 * k,
        0.8 * k,
        with_alpha(c(pal::CHROME_LIGHT), 0.9),
    );
    b.rrect(
        Rect::from_min_max(
            pos2(base.left() + 2.0 * k, base.bottom() - 1.0 * k),
            pos2(base.right() - 2.0 * k, r.bottom()),
        ),
        1.5 * k,
        Fill::V(c(pal::RUBBER_LIGHT), c(pal::RUBBER)),
    );
    housing(b, s, base, 4.0 * k);
    let talk = Rect::from_min_max(
        pos2(base.left() + base.width() * 0.34, base.top() + hb * 0.38),
        pos2(base.right() - base.width() * 0.1, base.top() + hb * 0.7),
    );
    b.rrect(talk, 2.0 * k, Fill::V(c(pal::SATIN_LIGHT), c(pal::SATIN)));
    b.hline(
        talk.left() + 2.0 * k,
        talk.right() - 2.0 * k,
        talk.top() + 0.6 * k,
        0.6 * k,
        with_alpha(c(pal::CHROME_LIGHT), 0.4),
    );
    for x in [talk.left(), talk.right()] {
        b.rect(
            Rect::from_center_size(pos2(x, talk.center().y), vec2(2.2 * k, talk.height())),
            Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)),
        );
    }
    art::lamp(
        b,
        pos2(base.left() + base.width() * 0.18, talk.center().y),
        (hb * 0.11).max(2.4 * k),
        glow,
        lv,
    );
    let centre = pos2(head.center().x, head.top() + hh * 0.42);
    arcs(b, centre, hw * 0.78, k, lv, glow);
    centre
}

/// The capsule in `head`: the satin shell, the chrome bands at its ends, the slotted grille
/// over the dark foam with the voice's glow behind it, the shell's light down its left.
fn capsule(b: &mut Brush, k: f32, head: Rect, lv: f32, glow: Color32) {
    let rad = head.width() * 0.5;
    b.rrect_soft(
        head.translate(vec2(2.0, 3.0) * k),
        rad,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.5)),
        5.0 * k,
    );
    b.rrect(head, rad, Fill::V(c(pal::SATIN_LIGHT), c(pal::SATIN)));
    // The grille's window, inset, the foam behind and the glow.
    let grille = head.shrink2(vec2(head.width() * 0.17, head.height() * 0.13));
    b.rrect(grille, grille.width() * 0.4, Fill::Solid(c(pal::FOAM)));
    if lv > 0.01 {
        b.radial(
            grille,
            grille.center(),
            vec2(grille.width() * 0.7, grille.height() * 0.55),
            &[
                (0.0, with_alpha(glow, 0.5 * lv)),
                (1.0, with_alpha(glow, 0.0)),
            ],
            8,
        );
    }
    // The slots' bars: satin, each lit along its top edge.
    let pitch = (grille.height() / 11.0).max(2.4 * k);
    let mut y = grille.top() + pitch * 0.5;
    let inset = grille.width() * 0.08;
    while y < grille.bottom() - pitch * 0.3 {
        b.hline(
            grille.left() + inset,
            grille.right() - inset,
            y,
            pitch * 0.42,
            c(pal::SATIN),
        );
        b.hline(
            grille.left() + inset,
            grille.right() - inset,
            y - pitch * 0.18,
            0.5 * k,
            with_alpha(c(pal::WIRE), 0.45),
        );
        y += pitch;
    }
    b.rrect_stroke(
        grille,
        grille.width() * 0.4,
        0.8 * k,
        with_alpha(c(pal::CHROME), 0.7),
    );
    // The chrome bands at the capsule's ends.
    for (y0, y1) in [
        (
            head.top() + head.height() * 0.05,
            head.top() + head.height() * 0.09,
        ),
        (
            head.bottom() - head.height() * 0.09,
            head.bottom() - head.height() * 0.05,
        ),
    ] {
        b.hrect(
            Rect::from_min_max(
                pos2(head.left() + rad * 0.3, y0),
                pos2(head.right() - rad * 0.3, y1),
            ),
            &cylinder(),
        );
    }
    // The shell's light and shade across it, its edge.
    b.hrect(
        head.shrink2(vec2(0.0, rad * 0.6)),
        &[
            (0.0, with_alpha(c(pal::SHADOW), 0.45)),
            (0.2, with_alpha(c(pal::SATIN_LIGHT), 0.18)),
            (0.32, with_alpha(c(pal::SHADOW), 0.0)),
            (1.0, with_alpha(c(pal::SHADOW), 0.55)),
        ],
    );
    b.rrect_stroke(head, rad, 0.7 * k, with_alpha(c(pal::SATIN_LIGHT), 0.6));
}
