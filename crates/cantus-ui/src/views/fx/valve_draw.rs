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

/// A vacuum tube standing in `r` (its socket at the bottom): the glass, the plate, the
/// mica and the getter, the filament's glow (`glow` 0 … 1).
pub fn tube(b: &mut Brush, s: &Style, r: Rect, glow: f32) {
    let k = s.k;
    let g = glow.clamp(0.0, 1.0);
    let w = r.width();
    let socket_h = w * 0.36;
    let glass = Rect::from_min_max(r.min, pos2(r.right(), r.bottom() - socket_h * 0.55));
    // The socket: black, a chrome ring.
    let sock = Rect::from_min_max(
        pos2(r.left() - w * 0.08, r.bottom() - socket_h),
        pos2(r.right() + w * 0.08, r.bottom()),
    );
    b.rrect(
        sock,
        2.0 * k,
        Fill::V(c(pal::BAKELITE_TOP), c(pal::BAKELITE)),
    );
    b.hline(
        sock.left(),
        sock.right(),
        sock.top() + 0.5 * k,
        1.2 * k,
        c(pal::CHROME_LIGHT),
    );
    // The glow inside, behind the plate: the filament's light filling the glass.
    let core = pos2(glass.center().x, glass.top() + glass.height() * 0.62);
    let warm = c(pal::FILAMENT);
    b.radial(
        glass,
        core,
        vec2(w * 0.9, glass.height() * 0.55),
        &[
            (0.0, with_alpha(warm, 0.55 + 0.45 * g)),
            (0.45, with_alpha(warm, 0.18 + 0.32 * g)),
            (1.0, with_alpha(warm, 0.0)),
        ],
        12,
    );
    // The plate: a dark box with its seams, the filament's line glowing at its middle.
    let plate = Rect::from_center_size(core, vec2(w * 0.5, glass.height() * 0.42));
    b.rect(
        plate,
        Fill::V(c(pal::PLATE), mix(c(pal::PLATE), c(pal::SHADOW), 0.4)),
    );
    for f in [0.33f32, 0.66] {
        b.vline(
            plate.left() + plate.width() * f,
            plate.top(),
            plate.bottom(),
            0.7 * k,
            c(pal::PLATE_SEAM),
        );
    }
    b.vline(
        plate.center().x,
        plate.top() - 2.0 * k,
        plate.bottom() + 2.0 * k,
        1.1 * k,
        mix(warm, c(pal::CHROME_LIGHT), 0.25 + 0.3 * g),
    );
    // The heater seen through the gap above the plate: the hottest point.
    let hot = pos2(plate.center().x, plate.top() - 1.5 * k);
    b.radial(
        glass,
        hot,
        vec2(w * 0.32, w * 0.22),
        &[
            (
                0.0,
                with_alpha(mix(warm, c(pal::CHROME_LIGHT), 0.35), 0.6 + 0.4 * g),
            ),
            (1.0, with_alpha(warm, 0.0)),
        ],
        8,
    );
    // The micas above and below the plate.
    for y in [plate.top() - 3.0 * k, plate.bottom() + 3.0 * k] {
        b.hline(
            glass.left() + w * 0.12,
            glass.right() - w * 0.12,
            y,
            0.8 * k,
            with_alpha(c(pal::MICA), 0.8),
        );
    }
    // The getter's silvering at the top of the dome.
    let dome = Rect::from_min_size(glass.min, vec2(w, w * 0.6));
    b.radial(
        dome,
        pos2(dome.center().x, dome.top() + w * 0.18),
        vec2(w * 0.42, w * 0.28),
        &[
            (0.0, with_alpha(c(pal::GETTER), 0.85)),
            (1.0, with_alpha(c(pal::GETTER), 0.0)),
        ],
        8,
    );
    // The glass: a fine outline with a dome, a highlight down its left.
    b.rrect(
        glass,
        w * 0.5,
        Fill::Solid(with_alpha(mix(c(pal::GLASS), warm, 0.5 * g), 0.10)),
    );
    b.rrect_stroke(
        glass,
        w * 0.5,
        0.8 * k,
        with_alpha(mix(c(pal::GLASS), warm, 0.3 * g), 0.55),
    );
    b.segment(
        pos2(glass.left() + w * 0.2, glass.top() + w * 0.45),
        pos2(glass.left() + w * 0.2, glass.bottom() - w * 0.3),
        1.3 * k,
        with_alpha(c(pal::CHROME_LIGHT), 0.55),
        true,
    );
    // The tip where the glass was sealed.
    b.circle(
        pos2(glass.center().x, glass.top() + 0.6 * k),
        1.2 * k,
        Fill::Solid(with_alpha(c(pal::GLASS), 0.8)),
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

/// The whole strip in `r`: the chassis, `tubes` tubes in two groups around a brass plate,
/// a jewel light at each end, all lit by `glow`.
pub fn strip(b: &mut Brush, s: &Style, r: Rect, tubes: usize, glow: f32, name: Src<'_>) {
    let k = s.k;
    b.rrect(
        r,
        6.0 * k,
        Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)),
    );
    let inner = r.shrink(2.0 * k);
    grille(b, s, inner);
    let tube_h = inner.height() - 8.0 * k;
    let tube_w = (tube_h * 0.36).max(10.0 * k);
    let plate_w = 150.0 * k;
    let gap = tube_w * 1.6;
    let per = tubes.div_ceil(2).max(1);
    let group = per as f32 * tube_w + (per as f32 - 1.0) * gap;
    let cx = inner.center().x;
    let starts = [
        cx - plate_w * 0.5 - 24.0 * k - group,
        cx + plate_w * 0.5 + 24.0 * k,
    ];
    let bottom = inner.bottom() - 3.0 * k;
    let mut n = 0;
    for x0 in starts {
        for i in 0..per {
            if n == tubes {
                break;
            }
            let x = x0 + i as f32 * (tube_w + gap);
            let tr = Rect::from_min_max(pos2(x, bottom - tube_h), pos2(x + tube_w, bottom));
            wash(b, inner, tr.center(), tube_w * 4.0, glow);
            tube(b, s, tr, glow);
            n += 1;
        }
    }
    let plate = Rect::from_center_size(pos2(cx, inner.center().y), vec2(plate_w, 20.0 * k));
    brass_plate(b, s, plate, name);
    let jr = (inner.height() * 0.16).min(9.0 * k);
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
