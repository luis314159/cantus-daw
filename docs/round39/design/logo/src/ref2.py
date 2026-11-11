"""#2's small master, ported line for line from crates/cantus-ui/src/kit/logo_small.rs so
the comparison sheet shows today's top-bar mark exactly as the app draws it."""
import math


def small_master(px):
    n = float(max(px, 12))
    r = round(n * 0.22)
    cx, cy = round(n * 0.44), round(n * 0.5)
    ri, ro = n * 0.17, n * 0.31
    w = 1.5 if px <= 16 else max(round(n / 16.0), 1.0) + 0.5
    body = ""
    if px <= 16:
        rm = (ri + ro) * 0.5
        a0, a1 = math.radians(40), math.radians(320)
        x0, y0 = cx + rm * math.cos(a0), cy + rm * math.sin(a0)
        x1, y1 = cx + rm * math.cos(a1), cy + rm * math.sin(a1)
        body = ('<path d="M%.2f,%.2f A%.2f,%.2f 0 1 1 %.2f,%.2f" fill="none" stroke="url(#w)" '
                'stroke-width="%.1f" stroke-linecap="round"/>' % (x0, y0, rm, rm, x1, y1,
                                                                   (ro - ri) * 0.75))
    else:
        spokes = 9 if px <= 24 else 11
        for i in range(spokes):
            a = math.radians(40.0 + 280.0 * i / (spokes - 1))
            wob = 1.0 if i % 2 == 0 else 0.86
            body += ('<line x1="%.2f" y1="%.2f" x2="%.2f" y2="%.2f" stroke="url(#w)" '
                     'stroke-width="%.1f" stroke-linecap="round"/>'
                     % (cx + ri * math.cos(a), cy + ri * math.sin(a),
                        cx + ro * wob * math.cos(a), cy + ro * wob * math.sin(a), w))
    lw = 1.0 if px <= 16 else round(n / 16.0)
    px_x = round(n * 0.72)
    top, bottom = round(n * 0.24), round(n * 0.78)
    bar_y = round(n * 0.33)
    bar_w = round(n * 0.22)
    if (bar_w - lw) % 2 != 0:
        bar_w += 1
    bx = px_x - (bar_w - lw) * 0.5
    cap = ""
    if px >= 32:
        l, rr, y = px_x - 2, px_x + lw + 2, top
        cap = '<path d="M%g,%g L%g,%g L%g,%g Z" fill="url(#ph)"/>' % (l, y, rr, y, px_x + lw / 2,
                                                                     y + 3)
    return ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {n:g} {n:g}" width="{n:g}" '
            'height="{n:g}"><defs><radialGradient id="bg" cx="0.3" cy="0.25" r="1"><stop '
            'offset="0" stop-color="#4A2213"/><stop offset="0.55" stop-color="#22110A"/><stop '
            'offset="1" stop-color="#0D0705"/></radialGradient><linearGradient id="w" '
            'gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="{n:g}" y2="{n:g}"><stop offset="0" '
            'stop-color="#FFD98A"/><stop offset="0.45" stop-color="#E07A2E"/><stop offset="1" '
            'stop-color="#9C300C"/></linearGradient><linearGradient id="ph" x1="0" y1="0" x2="0" '
            'y2="1"><stop offset="0" stop-color="#FFF6D6"/><stop offset="1" '
            'stop-color="#E9B64C"/></linearGradient></defs>'
            '<rect x="0" y="0" width="{n:g}" height="{n:g}" rx="{r:g}" fill="url(#bg)"/>{body}'
            '<rect x="{px:g}" y="{top:g}" width="{lw:g}" height="{h:g}" fill="url(#ph)"/>'
            '<rect x="{bx:g}" y="{by:g}" width="{bw:g}" height="{lw:g}" fill="url(#ph)"/>{cap}'
            '<rect x="0.5" y="0.5" width="{n1:g}" height="{n1:g}" rx="{r1:g}" fill="none" '
            'stroke="#ffffff" stroke-opacity="0.14" stroke-width="1"/></svg>').format(
        n=n, r=r, body=body, px=px_x, top=top, lw=lw, h=bottom - top, bx=bx, by=bar_y, bw=bar_w,
        cap=cap, n1=n - 1, r1=max(r - 0.5, 0))
