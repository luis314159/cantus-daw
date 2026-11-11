"""The four variants' small marks, drawn for 48, 32, 24 and 16 px on the pixel grid
(the top bar uses 24). Detail drops away by size; the cross stays on whole pixels.
Run: python small.py <outdir>."""
import math
import sys
import geo
from geo import GOLD

SIZES = (48, 32, 24, 16)

# Per size: the C (centre, outer radius, belly and terminal weights, terminal angle)
# and the cross on whole pixels (x, width, top, bottom, bar y, bar height, left, right).
LAY = {
    48: dict(c=(21.2, 24.0), R=15.3, t=(5.6, 3.5), ae=50,
             x=(36, 3, 9, 39, 13, 3, 33, 42)),
    32: dict(c=(13.8, 16.0), R=10.3, t=(3.9, 2.5), ae=52,
             x=(24, 2, 6, 26, 9, 2, 22, 28)),
    24: dict(c=(10.2, 12.0), R=7.7, t=(3.1, 2.1), ae=56,
             x=(18, 2, 4, 20, 7, 2, 16, 22)),
    16: dict(c=(6.7, 8.0), R=5.2, t=(2.3, 1.6), ae=56,
             x=(12, 2, 3, 13, 5, 2, 11, 15)),
}
# The variants' letter weights, relative to the default (Roseton more contrast, Grabado
# more even), as in the large drawings.
WT = {"relicario": (1.0, 1.0), "astrolabio": (1.0, 1.0), "roseton": (1.12, 0.88),
      "grabado": (0.92, 1.1)}


def tile_defs(n):
    return ('<radialGradient id="bg" cx="0.34" cy="0.28" r="0.95">'
            '<stop offset="0" stop-color="#2B4796" data-role="tile"/>'
            '<stop offset="0.5" stop-color="#16295E" data-role="tile"/>'
            '<stop offset="1" stop-color="#0B1738" data-role="tile"/></radialGradient>'
            '<linearGradient id="sv" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="%d" '
            'y2="%d"><stop offset="0" stop-color="#FFFFFF" data-role="wave"/>'
            '<stop offset="0.45" stop-color="#C9D3E6" data-role="wave"/>'
            '<stop offset="1" stop-color="#8391B0" data-role="wave"/></linearGradient>'
            '<linearGradient id="gd" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="%d" '
            'y2="%d"><stop offset="0" stop-color="#FFE9A0" data-role="playhead"/>'
            '<stop offset="0.5" stop-color="#E2B84A" data-role="playhead"/>'
            '<stop offset="1" stop-color="#A97A24" data-role="playhead"/></linearGradient>'
            % (n, n, n, n))


def cross_px(x, w, top, bot, by, bh, bl, br, light, dark, mid=None):
    """The cross on whole pixels, cut in two (or three) tones: the lit flank and the
    shaded one, like the large drawing's bevel."""
    r = []

    def rect(x0, y0, ww, hh, col):
        r.append('<rect x="%d" y="%d" width="%d" height="%d" fill="%s" data-role="playhead"/>'
                 % (x0, y0, ww, hh, col))
    cols = [light, dark] if w == 2 else ([light, mid, dark] if w == 3 else [light])
    rows = [light, dark] if bh == 2 else ([light, mid, dark] if bh == 3 else [light])
    # Vertical bar, column by column.
    for i, col in enumerate(cols):
        rect(x + i, top, 1, bot - top, col)
    # The crossbar's arms, row by row (the crossing keeps the bar's columns).
    for j, col in enumerate(rows):
        rect(bl, by + j, x - bl, 1, col)
        rect(x + w, by + j, br - x - w, 1, col)
    return "".join(r)


def build(variant, n):
    L = LAY[n]
    rr = round(n * 0.22)
    tmax, tmin = L["t"][0] * WT[variant][0], L["t"][1] * WT[variant][1]
    tfn = geo.c_thickness(tmax, tmin)
    body = []
    if n == 48:
        body.append(geo.gilded_c2(L["c"], L["R"], tfn, L["ae"], 1.4, "url(#gd)", GOLD, "wave",
                                  tilt=0.8, step=6.0))
    else:
        poly, _ = geo.c_outline(L["c"], L["R"], tfn, L["ae"], step=6.0)
        body.append('<polygon points="%s" fill="url(#gd)"/>' % geo.pts(poly))
        if n >= 32:
            # One lit edge on the outer upper-left, the gilding's highlight.
            r = L["R"] - 0.5
            p0, p1 = geo.polar(L["c"], r, 200), geo.polar(L["c"], r, 285)
            body.append('<path d="M%.2f,%.2f A%.2f,%.2f 0 0 1 %.2f,%.2f" fill="none" '
                        'stroke="#FFF4C8" stroke-opacity="0.75" stroke-width="1" '
                        'data-role="wave"/>' % (*p0, r, r, *p1))
    extra = []
    cc = (L["c"][0] + 0.6, L["c"][1])
    if variant == "relicario" and n >= 32:
        i = 2.5
        extra.append('<rect x="%.1f" y="%.1f" width="%.1f" height="%.1f" rx="%.1f" fill="none" '
                     'stroke="url(#gd)" stroke-width="1"/>' % (i, i, n - 2 * i, n - 2 * i, rr - 2))
    if variant == "astrolabio" and n >= 32:
        if n == 48:
            extra.append('<circle cx="%.1f" cy="%.1f" r="5" fill="none" stroke="url(#sv)" '
                         'stroke-width="1"/>' % cc)
        extra.append('<circle cx="%.1f" cy="%.1f" r="%.1f" fill="url(#sv)"/>'
                     % (*cc, 1.6 if n == 48 else 1.3))
    if variant == "roseton" and n >= 32:
        if n == 48:
            extra.append('<circle cx="%.1f" cy="%.1f" r="5" fill="none" stroke="url(#gd)" '
                         'stroke-width="1"/>' % cc)
        extra.append('<circle cx="%.1f" cy="%.1f" r="%.1f" fill="url(#gd)"/>'
                     % (*cc, 1.8 if n == 48 else 1.4))
    x, w, top, bot, by, bh, bl, br = L["x"]
    cross = cross_px(x, w, top, bot, by, bh, bl, br, "#FFE08A", "#B4842A", "#E2B84A")
    if variant == "relicario" and n == 48:
        cross += ('<rect x="%d" y="%d" width="%d" height="%d" fill="#E6ECF7" data-role="wave"/>'
                  % (x, by, w, bh))
    if n >= 32:
        # A one-pixel shadow to the lower right lifts the cross off the enamel.
        sh = cross_px(x + 1, w, top + 1, bot + 1, by + 1, bh, bl + 1, br + 1,
                      "#000000", "#000000", "#000000").replace(
            'data-role="playhead"', 'fill-opacity="0.3" data-role="neutral"')
        cross = sh + cross
    return ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {n} {n}" width="{n}" '
            'height="{n}"><defs>{d}</defs>'
            '<rect x="0" y="0" width="{n}" height="{n}" rx="{r}" fill="url(#bg)"/>'
            '{extra}{body}{cross}'
            '<rect x="0.5" y="0.5" width="{n1}" height="{n1}" rx="{r1}" fill="none" '
            'stroke="#FFFFFF" stroke-opacity="0.14" stroke-width="1" data-role="neutral"/>'
            '</svg>').format(n=n, d=tile_defs(n), r=rr, extra="".join(extra),
                             body="".join(body), cross=cross, n1=n - 1, r1=max(rr - 0.5, 0))


if __name__ == "__main__":
    out = sys.argv[1] if len(sys.argv) > 1 else "."
    for v in ("relicario", "astrolabio", "roseton", "grabado"):
        for n in SIZES:
            with open("%s/%s-%d.svg" % (out, v, n), "w", newline="\n") as f:
                f.write(build(v, n))
