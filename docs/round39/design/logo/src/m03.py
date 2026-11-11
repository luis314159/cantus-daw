"""#03 ("the metal") redrawn small, for the sizes the app shows it at below 64 px.

The full drawing scaled down keeps its 100-px margin and its shadow (the tile fills only
80 % of the square), turns the corner flourishes to noise and the waveform to an orange
smudge, and thins the C. These masters fill the square as #2's small master does, thicken
the C, keep its lozenge terminals from 30 px, centred on the stroke (at the outer corner, as in the large drawing, they read as arrowheads, a reload sign), and redraw the waveform as whole-pixel
bars: five at 48, 30 and 32, three at 24 and 16. The gold rim stays from 30 px.
Roles are marked as in the ten logos, so the theme tint works.
"""
import math

# n: C outer/inner radius, lozenge half-diagonal (0 = none), bars [(x, h)] 1 px or 2 px
# wide (bw), rim (inset or 0)
SPEC = {
    16: dict(ro=6.0, ri=3.8, dz=0, bw=1, bars=[(7, 2), (9, 4), (11, 2)], rim=0),
    20: dict(ro=7.3, ri=4.7, dz=0, bw=1, bars=[(8, 4), (10, 6), (12, 4)], rim=0),
    24: dict(ro=8.6, ri=5.6, dz=0, bw=1, bars=[(11, 4), (13, 8), (15, 4)], rim=0),
    30: dict(ro=10.6, ri=7.1, dz=2.4, bw=1, bars=[(12, 4), (14, 8), (16, 10), (18, 6), (20, 4)],
             rim=2.5),
    32: dict(ro=11.3, ri=7.6, dz=2.5, bw=1, bars=[(13, 4), (15, 8), (17, 12), (19, 8), (21, 4)],
             rim=2.5),
    48: dict(ro=16.8, ri=11.6, dz=3.6, bw=2, bars=[(18, 6), (21, 10), (24, 16), (27, 10), (30, 6)],
             rim=3.5),
}
AE = 44.0  # the C's ends, degrees either side of the opening


def svg(n, uid="m"):
    s = SPEC[n]
    c = n / 2
    rr = round(n * 0.22)
    u = uid

    def P(r, a):
        return (c + r * math.cos(math.radians(a)), c + r * math.sin(math.radians(a)))
    o0, o1 = P(s["ro"], AE), P(s["ro"], 360 - AE)
    i0, i1 = P(s["ri"], AE), P(s["ri"], 360 - AE)
    cpath = ("M%.2f,%.2f A%.2f,%.2f 0 1 1 %.2f,%.2f L%.2f,%.2f A%.2f,%.2f 0 1 0 %.2f,%.2f Z"
             % (*o0, s["ro"], s["ro"], *o1, *i1, s["ri"], s["ri"], *i0))
    out = [
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 %d %d" width="%d" height="%d">'
        % (n, n, n, n),
        '<defs><radialGradient id="%sbg" cx="0.4" cy="0.35" r="0.85">'
        '<stop offset="0" stop-color="#2E4AA0" data-role="tile"/>'
        '<stop offset="0.6" stop-color="#1A2A66" data-role="tile"/>'
        '<stop offset="1" stop-color="#0A1234" data-role="tile"/></radialGradient>'
        '<linearGradient id="%sleaf" gradientUnits="userSpaceOnUse" x1="%.1f" y1="%.1f" '
        'x2="%.1f" y2="%.1f"><stop offset="0" stop-color="#FFF0B8" data-role="wave"/>'
        '<stop offset="0.35" stop-color="#EBC455" data-role="wave"/>'
        '<stop offset="0.75" stop-color="#C9962F" data-role="wave"/>'
        '<stop offset="1" stop-color="#8A5E14" data-role="wave"/></linearGradient>'
        '<linearGradient id="%sverm" x1="0" y1="0" x2="0" y2="1">'
        '<stop offset="0" stop-color="#FF9A5C" data-role="patina"/>'
        '<stop offset="1" stop-color="#D2541A" data-role="patina"/></linearGradient></defs>'
        % (u, u, n * 0.15, n * 0.15, n * 0.8, n * 0.85, u),
        '<rect width="%d" height="%d" rx="%d" fill="url(#%sbg)"/>' % (n, n, rr, u),
    ]
    if s["rim"]:
        i = s["rim"]
        out.append('<rect x="%.1f" y="%.1f" width="%.1f" height="%.1f" rx="%.1f" fill="none" '
                   'stroke="url(#%sleaf)" stroke-width="1" stroke-opacity="0.85"/>'
                   % (i, i, n - 2 * i, n - 2 * i, max(rr - i + 0.5, 1), u))
    out.append('<path d="%s" fill="url(#%sleaf)"/>' % (cpath, u))
    if n >= 30:
        # The gilding's lit edge on the outer upper left.
        r = s["ro"] - 0.6
        a, b = P(r, 160), P(r, 260)
        out.append('<path d="M%.2f,%.2f A%.2f,%.2f 0 0 1 %.2f,%.2f" fill="none" stroke="#FFF8DA" '
                   'stroke-opacity="0.7" stroke-width="%.1f" stroke-linecap="round" '
                   'data-role="neutral"/>' % (*a, r, r, *b, 1.0 if n < 48 else 1.4))
    if s["dz"]:
        rm = (s["ro"] + s["ri"]) / 2
        for p in (P(rm, AE + 3), P(rm, 360 - AE - 3)):
            d = s["dz"]
            out.append('<path d="M%.2f,%.2f l%.2f,%.2f l%.2f,%.2f l%.2f,%.2f Z" fill="url(#%sleaf)" '
                       'stroke="#5A3A08" stroke-opacity="0.45" stroke-width="0.6" '
                       'data-role="neutral"/>' % (p[0], p[1] - d, d, d, -d, d, -d, -d, u))
    for x, h in s["bars"]:
        y = round(c - h / 2)
        out.append('<rect x="%d" y="%d" width="%d" height="%d" fill="url(#%sverm)"%s/>'
                   % (x, y, s["bw"], h, u, ' rx="1"' if s["bw"] >= 2 else ""))
    out.append('<rect x="0.5" y="0.5" width="%d" height="%d" rx="%.1f" fill="none" '
               'stroke="#FFFFFF" stroke-opacity="0.14" stroke-width="1" data-role="neutral"/>'
               % (n - 1, n - 1, max(rr - 0.5, 0)))
    out.append('</svg>')
    return "".join(out)


if __name__ == "__main__":
    import os
    import sys
    out = sys.argv[1] if len(sys.argv) > 1 else "."
    os.makedirs(out, exist_ok=True)
    for n in SPEC:
        with open(os.path.join(out, "03-%d.svg" % n), "w", encoding="utf-8", newline="\n") as f:
            f.write(svg(n))
