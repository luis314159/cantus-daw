//! #2's small master (ADR 0024 §12), in a file of its own with no dependency beyond
//! `std`, so the application's build script can include it (`#[path]`) and render the
//! window's and the `.exe`'s icon entries from the same drawing the interface uses.

/// #2 redrawn for 16, 24 and 32 px on the pixel grid: the same tile, fewer and thicker
/// spokes to the C (a solid arc at 16), the playhead cross on whole pixels. Roles are
/// marked like the full logo's, so it tints the same way.
pub fn small_master(px: u32) -> String {
    let n = px.max(12) as f32;
    let r = (n * 0.22).round();
    let (cx, cy) = ((n * 0.44).round(), (n * 0.5).round());
    let (ri, ro) = (n * 0.17, n * 0.31);
    let w = if px <= 16 {
        1.5
    } else {
        (n / 16.0).round().max(1.0) + 0.5
    };
    let mut body = String::new();
    if px <= 16 {
        // A solid C: an arc of the wave, open to the right.
        let rm = (ri + ro) * 0.5;
        let a0 = 40f32.to_radians();
        let a1 = 320f32.to_radians();
        let (x0, y0) = (cx + rm * a0.cos(), cy + rm * a0.sin());
        let (x1, y1) = (cx + rm * a1.cos(), cy + rm * a1.sin());
        body.push_str(&format!(
            r##"<path d="M{x0:.2},{y0:.2} A{rm:.2},{rm:.2} 0 1 1 {x1:.2},{y1:.2}" fill="none" stroke="url(#w)" stroke-width="{:.1}" stroke-linecap="round"/>"##,
            (ro - ri) * 0.75
        ));
    } else {
        let spokes = if px <= 24 { 9 } else { 11 };
        for i in 0..spokes {
            let a = (40.0 + 280.0 * i as f32 / (spokes - 1) as f32).to_radians();
            let wob = if i % 2 == 0 { 1.0 } else { 0.86 };
            let (x0, y0) = (cx + ri * a.cos(), cy + ri * a.sin());
            let (x1, y1) = (cx + ro * wob * a.cos(), cy + ro * wob * a.sin());
            body.push_str(&format!(
                r##"<line x1="{x0:.2}" y1="{y0:.2}" x2="{x1:.2}" y2="{y1:.2}" stroke="url(#w)" stroke-width="{w:.1}" stroke-linecap="round"/>"##
            ));
        }
    }
    let lw = if px <= 16 { 1.0 } else { (n / 16.0).round() };
    let px_x = (n * 0.72).round();
    let (top, bottom) = ((n * 0.24).round(), (n * 0.78).round());
    let bar_y = (n * 0.33).round();
    // The crossbar, centred on the line and on whole pixels.
    let mut bar_w = (n * 0.22).round();
    if (bar_w - lw) % 2.0 != 0.0 {
        bar_w += 1.0;
    }
    let bx = px_x - (bar_w - lw) * 0.5;
    let cap = if px >= 32 {
        let (l, r, y) = (px_x - 2.0, px_x + lw + 2.0, top);
        format!(
            r##"<path d="M{l},{y} L{r},{y} L{m},{y2} Z" fill="url(#ph)"/>"##,
            m = px_x + lw * 0.5,
            y2 = y + 3.0
        )
    } else {
        String::new()
    };
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {n} {n}" width="{n}" height="{n}">
<defs><radialGradient id="bg" cx="0.3" cy="0.25" r="1"><stop offset="0" stop-color="#4A2213" data-role="tile"/><stop offset="0.55" stop-color="#22110A" data-role="tile"/><stop offset="1" stop-color="#0D0705" data-role="tile"/></radialGradient>
<linearGradient id="w" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="{n}" y2="{n}"><stop offset="0" stop-color="#FFD98A" data-role="wave"/><stop offset="0.45" stop-color="#E07A2E" data-role="wave"/><stop offset="1" stop-color="#9C300C" data-role="wave"/></linearGradient>
<linearGradient id="ph" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#FFF6D6" data-role="playhead"/><stop offset="1" stop-color="#E9B64C" data-role="playhead"/></linearGradient></defs>
<rect x="0" y="0" width="{n}" height="{n}" rx="{r}" fill="url(#bg)"/>
{body}
<rect x="{px_x}" y="{top}" width="{lw}" height="{h}" fill="url(#ph)"/>
<rect x="{bx}" y="{bar_y}" width="{bar_w}" height="{lw}" fill="url(#ph)"/>
{cap}
<rect x="0.5" y="0.5" width="{n1}" height="{n1}" rx="{r1}" fill="none" stroke="#ffffff" stroke-opacity="0.14" stroke-width="1" data-role="neutral"/>
</svg>"##,
        h = bottom - top,
        n1 = n - 1.0,
        r1 = (r - 0.5).max(0.0),
    )
}
