//! Valve-amp parts, drawn (Fragor's tube strip, ADR 0131 — reusable by any unit with an
//! amplifier's character): vacuum tubes standing before a perforated chassis, their
//! filaments' glow, amber jewel pilot lights, a brass plate. Vectors at the style's scale,
//! colours from `kit::skin::pedals`; nothing animates by itself — the caller hands the glow
//! (steady at rest, following the signal slowly while playing).

use crate::kit::color::{mix, with_alpha, Hsl};
use crate::kit::skin::pedals as pal;
use crate::kit::text::{self as ktext, Elide, Policy, Role, Src, Text};
use crate::kit::{Brush, Fill, Style};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};

fn c(h: Hsl) -> Color32 {
    h.rgb()
}

/// A perforated metal plate filling `r`: holes in staggered rows.
pub fn grille(b: &mut Brush, s: &Style, r: Rect) {
    let k = s.k;
    b.rect(r, Fill::V(c(pal::GRILLE_TOP), c(pal::GRILLE)));
    let step = 7.0 * k;
    let hole = 1.9 * k;
    let mut row = 0;
    let mut y = r.top() + step * 0.6;
    while y < r.bottom() - hole {
        let off = if row % 2 == 1 { step * 0.5 } else { 0.0 };
        let mut x = r.left() + step * 0.5 + off;
        while x < r.right() - hole {
            b.circle(pos2(x, y), hole, Fill::Solid(c(pal::GRILLE_HOLE)));
            x += step;
        }
        y += step * 0.866;
        row += 1;
    }
}

/// Warm light thrown on what is behind a tube: a radial wash at `p`, `glow` 0 … 1.
pub fn wash(b: &mut Brush, r: Rect, p: Pos2, radius: f32, glow: f32) {
    let g = glow.clamp(0.0, 1.0);
    let warm = c(pal::FILAMENT);
    b.radial(
        r,
        p,
        vec2(radius, radius * 0.8),
        &[
            (0.0, with_alpha(warm, 0.42 * g)),
            (0.5, with_alpha(warm, 0.14 * g)),
            (1.0, with_alpha(warm, 0.0)),
        ],
        12,
    );
}

/// A vacuum tube standing in `r` (its base at the bottom): the glass with its highlights,
/// the getter's silvering in the dome, the micas and the support rods, the plate with its
/// depth and flanges, the heater glowing orange through the gaps (`glow` 0 … 1), and its
/// light filling the glass. `power`: a power tube — taller, a bakelite base, a bigger plate.
pub fn tube(b: &mut Brush, s: &Style, r: Rect, glow: f32, power: bool) {
    let k = s.k;
    let g = glow.clamp(0.0, 1.0);
    let w = r.width();
    let warm = c(pal::FILAMENT);
    let hot = mix(warm, c(pal::CHROME_LIGHT), 0.45);
    let base_h = if power { w * 0.42 } else { w * 0.26 };
    let glass = Rect::from_min_max(r.min, pos2(r.right(), r.bottom() - base_h * 0.8));
    // The base.
    if power {
        let base = Rect::from_min_max(
            pos2(r.left() - w * 0.04, r.bottom() - base_h),
            pos2(r.right() + w * 0.04, r.bottom()),
        );
        b.rrect(
            base,
            w * 0.14,
            Fill::V(c(pal::BAKELITE_BROWN_TOP), c(pal::BAKELITE_BROWN)),
        );
        b.vline(
            base.center().x,
            base.top() + 2.0 * k,
            base.bottom() - 2.0 * k,
            0.8 * k,
            with_alpha(c(pal::SHADOW), 0.5),
        );
    } else {
        let base = Rect::from_min_max(
            pos2(r.left() - w * 0.1, r.bottom() - base_h),
            pos2(r.right() + w * 0.1, r.bottom()),
        );
        b.rrect(
            base,
            1.5 * k,
            Fill::V(c(pal::BAKELITE_TOP), c(pal::BAKELITE)),
        );
        b.hline(
            base.left(),
            base.right(),
            base.top() + 0.5 * k,
            1.0 * k,
            c(pal::CHROME_LIGHT),
        );
    }
    // The light inside, filling the glass.
    let plate_h = glass.height() * if power { 0.46 } else { 0.40 };
    let core = pos2(glass.center().x, glass.top() + glass.height() * 0.58);
    b.radial(
        glass,
        core,
        vec2(w * 0.95, glass.height() * 0.6),
        &[
            (0.0, with_alpha(warm, 0.6 + 0.4 * g)),
            (0.5, with_alpha(warm, 0.12 + 0.22 * g)),
            (1.0, with_alpha(warm, 0.0)),
        ],
        12,
    );
    // The support rods, top mica to bottom mica.
    let (m_top, m_bot) = (
        core.y - plate_h * 0.5 - 4.0 * k,
        core.y + plate_h * 0.5 + 4.0 * k,
    );
    for f in [0.22f32, 0.78] {
        let x = glass.left() + w * f;
        b.vline(x, m_top, m_bot, 0.6 * k, with_alpha(c(pal::GETTER), 0.75));
    }
    // The heater, seen through the gaps above and below the plate: the hottest light.
    for y in [
        core.y - plate_h * 0.5 - 1.5 * k,
        core.y + plate_h * 0.5 + 1.5 * k,
    ] {
        b.radial(
            glass,
            pos2(core.x, y),
            vec2(w * 0.46, w * 0.28),
            &[
                (0.0, with_alpha(hot, 0.88 + 0.12 * g)),
                (0.5, with_alpha(warm, 0.45 + 0.3 * g)),
                (1.0, with_alpha(warm, 0.0)),
            ],
            8,
        );
        b.segment(
            pos2(core.x - w * 0.12, y),
            pos2(core.x + w * 0.12, y),
            0.9 * k,
            with_alpha(hot, 0.7 + 0.3 * g),
            true,
        );
    }
    // The plate: lit on the left, in shade on the right, its flanges and ribs.
    let pw = w * if power { 0.56 } else { 0.48 };
    let plate = Rect::from_center_size(core, vec2(pw, plate_h));
    let flange = w * 0.07;
    for side in [-1.0f32, 1.0] {
        let x = if side < 0.0 {
            plate.left() - flange
        } else {
            plate.right()
        };
        b.rect(
            Rect::from_min_size(
                pos2(x, plate.top() + 1.0 * k),
                vec2(flange, plate.height() - 2.0 * k),
            ),
            Fill::Solid(mix(c(pal::PLATE), c(pal::SHADOW), 0.35)),
        );
    }
    b.hrect(
        plate,
        &[
            (0.0, mix(c(pal::PLATE), c(pal::GETTER), 0.35)),
            (0.45, c(pal::PLATE)),
            (1.0, mix(c(pal::PLATE), c(pal::SHADOW), 0.55)),
        ],
    );
    for f in [0.3f32, 0.5, 0.7] {
        b.vline(
            plate.left() + plate.width() * f,
            plate.top() + 1.0 * k,
            plate.bottom() - 1.0 * k,
            0.6 * k,
            c(pal::PLATE_SEAM),
        );
    }
    // The cathode's glow seen between the plate's halves: an incandescent slit.
    b.vline(
        plate.center().x,
        plate.top(),
        plate.bottom(),
        4.0 * k,
        with_alpha(warm, 0.25 + 0.2 * g),
    );
    b.vline(
        plate.center().x,
        plate.top(),
        plate.bottom(),
        1.6 * k,
        with_alpha(hot, 0.65 + 0.35 * g),
    );
    // The glow's edge on the plate's sides.
    b.vline(
        plate.left(),
        plate.top(),
        plate.bottom(),
        0.8 * k,
        with_alpha(warm, 0.35 + 0.4 * g),
    );
    // The micas.
    for y in [m_top, m_bot] {
        b.hline(
            glass.left() + w * 0.1,
            glass.right() - w * 0.1,
            y,
            1.0 * k,
            with_alpha(c(pal::MICA), 0.85),
        );
    }
    // The getter's silvering at the top of the dome.
    let dome_h = w * if power { 0.75 } else { 0.62 };
    let dome = Rect::from_min_size(glass.min, vec2(w, dome_h));
    b.radial(
        dome,
        pos2(dome.center().x, dome.top() + dome_h * 0.42),
        vec2(w * 0.55, dome_h * 0.5),
        &[
            (0.0, with_alpha(c(pal::GETTER_DARK), 0.9)),
            (0.6, with_alpha(c(pal::GETTER), 0.75)),
            (1.0, with_alpha(c(pal::GETTER), 0.0)),
        ],
        10,
    );
    // The glass: a fine outline warmed by the light, highlights down both sides.
    b.rrect(
        glass,
        w * 0.5,
        Fill::Solid(with_alpha(mix(c(pal::GLASS), warm, 0.5 * g), 0.08)),
    );
    b.rrect_stroke(
        glass,
        w * 0.5,
        0.8 * k,
        with_alpha(mix(c(pal::GLASS), warm, 0.35 * g), 0.6),
    );
    b.segment(
        pos2(glass.left() + w * 0.17, glass.top() + dome_h * 0.6),
        pos2(glass.left() + w * 0.17, glass.bottom() - w * 0.25),
        1.4 * k,
        with_alpha(c(pal::CHROME_LIGHT), 0.6),
        true,
    );
    b.segment(
        pos2(glass.right() - w * 0.14, glass.top() + dome_h * 0.8),
        pos2(glass.right() - w * 0.14, glass.bottom() - w * 0.4),
        0.8 * k,
        with_alpha(c(pal::CHROME_LIGHT), 0.25),
        true,
    );
    b.circle(
        pos2(glass.left() + w * 0.3, glass.top() + dome_h * 0.35),
        1.2 * k,
        Fill::Solid(with_alpha(c(pal::CHROME_LIGHT), 0.8)),
    );
    // The tip where the glass was sealed.
    b.circle(
        pos2(glass.center().x, glass.top() + 0.6 * k),
        1.3 * k,
        Fill::Solid(with_alpha(c(pal::GLASS), 0.85)),
    );
}

/// An amber jewel pilot light at `p`: a chrome bezel, a faceted lens, its glow (`lit` 0 … 1).
pub fn jewel(b: &mut Brush, s: &Style, p: Pos2, radius: f32, lit: f32) {
    let k = s.k;
    let l = lit.clamp(0.0, 1.0);
    let amber = c(pal::JEWEL);
    b.radial(
        Rect::from_center_size(p, vec2(radius, radius) * 5.0),
        p,
        vec2(radius, radius) * 2.4,
        &[
            (0.0, with_alpha(amber, 0.45 * l)),
            (1.0, with_alpha(amber, 0.0)),
        ],
        10,
    );
    b.circle(
        p,
        radius * 1.3,
        Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)),
    );
    b.circle(
        p,
        radius,
        Fill::V(
            mix(amber, c(pal::CHROME_LIGHT), 0.35 * l),
            mix(amber, c(pal::SHADOW), 0.5 - 0.3 * l),
        ),
    );
    // The facets: a star of fine lines and a ring.
    for i in 0..8 {
        let a = std::f32::consts::TAU * i as f32 / 8.0;
        let d = vec2(a.cos(), a.sin());
        b.segment(
            p + d * radius * 0.35,
            p + d * radius * 0.92,
            0.6 * k,
            with_alpha(c(pal::CHROME_LIGHT), 0.35),
            false,
        );
    }
    b.circle_stroke(
        p,
        radius * 0.35,
        0.6 * k,
        with_alpha(c(pal::CHROME_LIGHT), 0.45),
    );
    b.circle(
        p + vec2(-0.35, -0.35) * radius,
        radius * 0.18,
        Fill::Solid(with_alpha(c(pal::CHROME_LIGHT), 0.85)),
    );
}

/// A brass plate in `r` with `name` engraved, two screws.
pub fn brass_plate(b: &mut Brush, s: &Style, r: Rect, name: Src<'_>) {
    let k = s.k;
    b.rrect(r, 2.5 * k, Fill::V(c(pal::BRASS_LIGHT), c(pal::BRASS)));
    b.rrect_stroke(
        r.shrink(1.5 * k),
        1.5 * k,
        0.7 * k,
        with_alpha(c(pal::BRASS_DARK), 0.7),
    );
    for x in [r.left() + 6.0 * k, r.right() - 6.0 * k] {
        let p = pos2(x, r.center().y);
        b.circle(p, 2.2 * k, Fill::V(c(pal::BRASS_LIGHT), c(pal::BRASS_DARK)));
        b.segment(
            p - vec2(1.4, 0.0) * k,
            p + vec2(1.4, 0.0) * k,
            0.6 * k,
            c(pal::BRASS_DARK),
            false,
        );
    }
    let t = Text {
        src: name,
        role: Role::Section,
        caps: true,
        align: Align2::CENTER_CENTER,
        policy: Policy::Elide(Elide::End),
        color: c(pal::BRASS_DARK),
    };
    ktext::paint(b, r.shrink2(vec2(14.0 * k, 1.0 * k)), &t, k);
}

/// The whole strip in `r`: the chassis's perforated grille, preamp tubes and power tubes in
/// two mirrored groups around a brass plate, a jewel light at each end, the tubes' warm
/// light spilling on the grille and along the chassis floor — all lit by `glow`.
pub fn strip(b: &mut Brush, s: &Style, r: Rect, glow: f32, name: Src<'_>) {
    let k = s.k;
    b.rrect(
        r,
        6.0 * k,
        Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)),
    );
    let inner = r.shrink(2.0 * k);
    grille(b, s, inner);
    let floor = inner.bottom() - 3.0 * k;
    let power_h = inner.height() - 7.0 * k;
    let pre_h = power_h * 0.74;
    let power_w = power_h * 0.40;
    let pre_w = pre_h * 0.34;
    let gap = 14.0 * k;
    let plate_w = 150.0 * k;
    let cx = inner.center().x;
    // From the plate outwards: two power tubes, then three preamp tubes, each side.
    let mut slots: Vec<(f32, bool)> = vec![];
    for side in [-1.0f32, 1.0] {
        let mut x = cx + side * (plate_w * 0.5 + 22.0 * k);
        for i in 0..5 {
            let power = i < 2;
            let w = if power { power_w } else { pre_w };
            let left = if side > 0.0 { x } else { x - w };
            slots.push((left, power));
            x += side * (w + gap);
        }
    }
    let warm = c(pal::FILAMENT);
    b.radial(
        inner,
        pos2(cx, floor),
        vec2(inner.width() * 0.45, inner.height() * 0.5),
        &[
            (0.0, with_alpha(warm, 0.16 * glow)),
            (1.0, with_alpha(warm, 0.0)),
        ],
        12,
    );
    for (left, power) in &slots {
        let (w, h) = if *power {
            (power_w, power_h)
        } else {
            (pre_w, pre_h)
        };
        let tr = Rect::from_min_max(pos2(*left, floor - h), pos2(left + w, floor));
        wash(
            b,
            inner,
            tr.center(),
            w * 4.5,
            glow * if *power { 1.0 } else { 0.8 },
        );
    }
    for (left, power) in &slots {
        let (w, h) = if *power {
            (power_w, power_h)
        } else {
            (pre_w, pre_h)
        };
        let tr = Rect::from_min_max(pos2(*left, floor - h), pos2(left + w, floor));
        tube(b, s, tr, glow, *power);
    }
    let plate = Rect::from_center_size(pos2(cx, inner.center().y), vec2(plate_w, 20.0 * k));
    brass_plate(b, s, plate, name);
    let jr = (inner.height() * 0.13).min(9.0 * k);
    jewel(
        b,
        s,
        pos2(inner.left() + 22.0 * k, inner.center().y),
        jr,
        0.55 + 0.45 * glow,
    );
    jewel(
        b,
        s,
        pos2(inner.right() - 22.0 * k, inner.center().y),
        jr,
        0.55 + 0.45 * glow,
    );
}

/// Glass over a screen in `r`: a faint diagonal reflection and a lit top edge, so the
/// display reads as a screen behind a pane.
pub fn glass(b: &mut Brush, s: &Style, r: Rect) {
    let k = s.k;
    let w = r.width();
    let sheen = [
        pos2(r.left() + w * 0.08, r.top()),
        pos2(r.left() + w * 0.30, r.top()),
        pos2(r.left() + w * 0.18, r.bottom()),
        pos2(r.left() - w * 0.04, r.bottom()),
    ];
    b.polygon(&sheen, with_alpha(c(pal::GLASS), 0.035));
    b.hline(
        r.left() + 2.0 * k,
        r.right() - 2.0 * k,
        r.top() + 0.5 * k,
        k,
        with_alpha(c(pal::GLASS), 0.18),
    );
}
