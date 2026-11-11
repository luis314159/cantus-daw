//! #2's small master (ADR 0024 §12; round 39, `dir-a`'s phase B mark, `src/logo.py`), in a
//! file of its own with no dependency beyond `std`, so the application's build script can
//! include it (`#[path]`) and render the window's and the `.exe`'s icon entries from the
//! same drawing the interface uses.
//!
//! The idea of logo #2 — a dark warm tile, a C made of a waveform's spokes open to the
//! right, a playhead with its crossbar where the C opens — redrawn for each size it is
//! shown at, every straight edge on whole pixels:
//! - **16 px**: the C one solid arc, the playhead one pixel wide, no cap;
//! - **20 and 24 px**: the C a thick arc cut into teeth (the spokes read as a comb), the
//!   playhead two pixels; the cap and the tile's lit top edge from 24;
//! - **30 px and up**: real spokes of alternating length, as the original.
//!
//! The tile is the logo's own warm black: the one warm note in the slate bar. Roles are
//! marked like the full logo's (`data-role`), so it tints the same way — the taskbar
//! tints recolour the wave.

/// How the C is drawn at a size.
#[derive(Copy, Clone, PartialEq)]
enum Form {
    Arc,
    Teeth,
    Spokes,
}

/// One hand-drawn size: its side, corner, the C's centre x, inner and outer radius, form,
/// teeth or spokes, a spoke's width, the playhead's x and width.
#[derive(Copy, Clone)]
struct Spec {
    n: f32,
    r: f32,
    cx: f32,
    ri: f32,
    ro: f32,
    form: Form,
    k: u32,
    sw: f32,
    phx: f32,
    lw: f32,
}

#[allow(clippy::too_many_arguments)]
const fn spec(
    n: f32,
    r: f32,
    cx: f32,
    ri: f32,
    ro: f32,
    form: Form,
    k: u32,
    sw: f32,
    phx: f32,
    lw: f32,
) -> Spec {
    Spec {
        n,
        r,
        cx,
        ri,
        ro,
        form,
        k,
        sw,
        phx,
        lw,
    }
}

/// The sizes drawn by hand (`logo.py`'s `SPEC`).
const SPECS: [Spec; 7] = [
    spec(16.0, 4.0, 7.0, 2.2, 5.4, Form::Arc, 0, 0.0, 11.0, 1.0),
    spec(20.0, 4.0, 8.0, 2.6, 6.4, Form::Teeth, 8, 0.0, 14.0, 2.0),
    spec(24.0, 5.0, 10.0, 3.2, 7.6, Form::Teeth, 9, 0.0, 17.0, 2.0),
    spec(30.0, 7.0, 12.0, 4.2, 9.4, Form::Spokes, 11, 1.5, 21.0, 2.0),
    spec(32.0, 7.0, 13.0, 4.4, 10.0, Form::Spokes, 11, 1.6, 23.0, 2.0),
    spec(36.0, 8.0, 15.0, 5.0, 11.2, Form::Spokes, 13, 1.7, 26.0, 3.0),
    spec(
        48.0,
        10.0,
        20.0,
        6.6,
        15.0,
        Form::Spokes,
        17,
        2.0,
        34.0,
        3.0,
    ),
];

/// The drawing for `px` pixels: its own where it is drawn by hand, else the nearest hand
/// drawing scaled to it (whole pixels kept whole).
fn spec_for(px: u32) -> Spec {
    let n = px.max(12) as f32;
    let near = SPECS
        .iter()
        .min_by(|a, b| (a.n - n).abs().total_cmp(&(b.n - n).abs()))
        .copied()
        .unwrap_or(SPECS[2]);
    let f = n / near.n;
    Spec {
        n,
        r: (near.r * f).round(),
        cx: (near.cx * f).round(),
        ri: near.ri * f,
        ro: near.ro * f,
        sw: near.sw * f,
        phx: (near.phx * f).round(),
        lw: (near.lw * f).round().max(1.0),
        ..near
    }
}

/// #2 at `px` pixels, as SVG.
pub fn small_master(px: u32) -> String {
    let Spec {
        n,
        r,
        cx,
        ri,
        ro,
        form,
        k,
        sw,
        phx,
        lw,
    } = spec_for(px);
    let cy = n / 2.0;
    // The C opens to the right, toward the playhead.
    let (a0, a1) = (52f32, 308f32);
    let mut body = String::new();
    if n >= 24.0 {
        // The tile's lit top edge.
        body.push_str(&format!(
            r##"<rect x="{r}" y="0" width="{w}" height="1" fill="#ffffff" fill-opacity="0.10" data-role="neutral"/>"##,
            w = n - 2.0 * r
        ));
    }
    match form {
        Form::Arc | Form::Teeth => {
            let rm = (ri + ro) * 0.5;
            let th = ro - ri;
            let (s0, c0) = a0.to_radians().sin_cos();
            let (s1, c1) = a1.to_radians().sin_cos();
            let (x0, y0) = (cx + rm * c0, cy + rm * s0);
            let (x1, y1) = (cx + rm * c1, cy + rm * s1);
            let dash = if form == Form::Teeth {
                let seg = (a1 - a0).to_radians() * rm / k as f32;
                format!(r#" stroke-dasharray="{:.2} {:.2}""#, seg * 0.62, seg * 0.38)
            } else {
                String::new()
            };
            body.push_str(&format!(
                r##"<path d="M{x0:.2} {y0:.2}A{rm:.2} {rm:.2} 0 1 1 {x1:.2} {y1:.2}" fill="none" stroke="url(#w)" stroke-width="{th:.2}" stroke-linecap="butt"{dash}/>"##
            ));
        }
        Form::Spokes => {
            for i in 0..k {
                let a = (a0 + (a1 - a0) * i as f32 / (k - 1) as f32).to_radians();
                let wob = if i % 2 == 0 { 1.0 } else { 0.84 };
                let (s, c) = a.sin_cos();
                let (x0, y0) = (cx + ri * c, cy + ri * s);
                let (x1, y1) = (cx + ro * wob * c, cy + ro * wob * s);
                body.push_str(&format!(
                    r##"<path d="M{x0:.2} {y0:.2}L{x1:.2} {y1:.2}" stroke="url(#w)" stroke-width="{sw:.2}" stroke-linecap="round"/>"##
                ));
            }
        }
    }
    let (top, bottom) = ((n * 0.22).round(), (n * 0.79).round());
    let bar_w = lw + 2.0 * (n * 0.1).round().max(2.0);
    let bar_y = (n * 0.34).round();
    let bx = phx - ((bar_w - lw) / 2.0).floor();
    let cap = if n >= 24.0 {
        let h = (n * 0.08).round().max(2.0);
        format!(
            r##"<path d="M{l} {top}H{rr}L{c} {y2}Z" fill="url(#ph)"/>"##,
            l = phx - 2.0,
            rr = phx + lw + 2.0,
            c = phx + lw * 0.5,
            y2 = top + h
        )
    } else {
        String::new()
    };
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {n} {n}" width="{n}" height="{n}">
<defs><radialGradient id="bg" cx="0.32" cy="0.26" r="0.9"><stop offset="0" stop-color="#2C1810" data-role="tile"/><stop offset="0.6" stop-color="#160C07" data-role="tile"/><stop offset="1" stop-color="#0B0604" data-role="tile"/></radialGradient>
<linearGradient id="w" gradientUnits="userSpaceOnUse" x1="{wx0}" y1="{wx0}" x2="{wx1}" y2="{wy1}"><stop offset="0" stop-color="#FFD98A" data-role="wave"/><stop offset="0.45" stop-color="#E07A2E" data-role="wave"/><stop offset="1" stop-color="#9C300C" data-role="wave"/></linearGradient>
<linearGradient id="ph" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#FFF6D6" data-role="playhead"/><stop offset="1" stop-color="#E9B64C" data-role="playhead"/></linearGradient></defs>
<rect x="0" y="0" width="{n}" height="{n}" rx="{r}" fill="url(#bg)"/>
{body}
<rect x="{phx}" y="{top}" width="{lw}" height="{h}" fill="url(#ph)"/>
<rect x="{bx}" y="{bar_y}" width="{bar_w}" height="{lw}" fill="url(#ph)"/>
{cap}
</svg>"##,
        wx0 = n * 0.2,
        wx1 = n * 0.7,
        wy1 = n * 0.85,
        h = bottom - top,
    )
}

/// #03 "the metal", the default logo (round 39, the owner's choice of 2026-11-09), as its
/// designer drew it for the sizes it is shown at (`docs/round39/design/logo/m03/`, made by
/// `src/m03.py`): the tile filling the square, the C thicker (2.2 px at 16), the waveform as
/// whole-pixel vermilion bars (five from 30 px, three below), the gold rim and the lozenge
/// terminals from 30 px, no corner flourishes. Roles marked as the full drawing's, so it
/// tints the same way.
pub const M03: [(u32, &str); 6] = [
    (16, include_str!("../../assets/logos/small/03-16.svg")),
    (20, include_str!("../../assets/logos/small/03-20.svg")),
    (24, include_str!("../../assets/logos/small/03-24.svg")),
    (30, include_str!("../../assets/logos/small/03-30.svg")),
    (32, include_str!("../../assets/logos/small/03-32.svg")),
    (48, include_str!("../../assets/logos/small/03-48.svg")),
];

/// #03's small master for `px`: the smallest drawn size that is at least `px` (48 above
/// it), rendered at `px`.
pub fn small_master_03(px: u32) -> &'static str {
    M03.iter()
        .find(|(n, _)| *n >= px)
        .unwrap_or(&M03[M03.len() - 1])
        .1
}

/// The small master of logo `number` at `px`, for the logos that have one (#2 and #3).
pub fn small_for(number: u8, px: u32) -> Option<String> {
    match number {
        2 => Some(small_master(px)),
        3 => Some(small_master_03(px).to_string()),
        _ => None,
    }
}
