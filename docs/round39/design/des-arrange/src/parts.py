"""des-mixer — the strip's parts: the fader and its meter on one scale, the knob, the keys,
the insert rows, the EQ thumbnail, the send rows and the output chip.

Every part takes its box; the strip modules decide the boxes per density (Compact, Wide,
Large) and per height (the desk fits first, then scrolls)."""
import math
import mglyphs as M
from base import rect, text, well, mix, tw, esc

# The fader's law, shared by the meter beside it so a level reads straight across:
# fraction of the travel from the top for each dB mark.
LAW = [(6, 0.0), (0, 0.16), (-6, 0.31), (-12, 0.45), (-18, 0.56), (-24, 0.65), (-36, 0.79),
       (-48, 0.88), (-120, 1.0)]
TICKS = (6, 0, -6, -12, -18, -24, -36, -48)


# the strip's few words, per locale (the window sets them)
WORDS_ALL = {
    'en': dict(master='Master', sends_in='{n} sends in', strips='{n} strips', track='Track', bus='Bus · {n} in',
               ret='Return · {n} in', lufs='LUFS short-term'),
    'es': dict(master='Máster', sends_in='{n} envíos', strips='{n} canales', track='Pista', bus='Bus · {n} entran',
               ret='Retorno · {n}', lufs='LUFS corto plazo'),
}
WORDS = dict(WORDS_ALL['en'])


def set_lang(lang):
    WORDS.clear()
    WORDS.update(WORDS_ALL[lang])


def frac(db):
    for (a, fa), (b, fb) in zip(LAW, LAW[1:]):
        if db >= b:
            return fa + (fb - fa) * (a - db) / (a - b)
    return 1.0


def dbs(v):
    s = '%.1f' % v
    if v > 0:
        s = '+' + s
    return s.replace('-', '−')


def fit(s, size, w, weight=400):
    """The name as it fits: whole, then the ellipsis (never cut mid-glyph)."""
    if tw(s, size, weight) <= w:
        return s
    while s and tw(s + '…', size, weight) > w:
        s = s[:-1]
    return s.rstrip() + '…'


def knob(t, cx, cy, r, val, bipolar=True, arc_col=None, auto=False, tick=True):
    """A knob: a dark ring, a lit cap, one pointer; the value as an arc outside it, from
    the centre when bipolar (pan) or from the minimum (width, send)."""
    out = f'<circle cx="{cx}" cy="{cy}" r="{r + 0.5}" fill="{t["edge"]}"/>'
    cap = t['raised'] if t['name'] == 'Night' else '#ffffff'
    out += f'<circle cx="{cx}" cy="{cy}" r="{r - 0.5}" fill="url(#kcap-{t["name"]})"/>'
    out += f'<circle cx="{cx}" cy="{cy - 0.5}" r="{r - 1.5}" fill="none" stroke="{mix(cap, t["ink1"], 0.10)}" stroke-width="1"/>'
    a = val * 135 if bipolar else -135 + val * 270
    ra = math.radians(a)
    px, py = cx + math.sin(ra) * (r - 2.5), cy - math.cos(ra) * (r - 2.5)
    qx, qy = cx + math.sin(ra) * (r * 0.25), cy - math.cos(ra) * (r * 0.25)
    out += f'<path d="M{qx:.2f} {qy:.2f}L{px:.2f} {py:.2f}" stroke="{t["ink1"]}" stroke-width="{1.6 if r >= 8 else 1.3}" stroke-linecap="round"/>'
    ar = r + 3 if r >= 8 else r + 2.5
    col = arc_col or (t['auto'] if auto else t['ink3'])
    a0, a1 = (sorted((0, a)) if bipolar else (-135, a))
    # the track of the arc, faint, then the value on it
    if not bipolar:
        out += _arc(cx, cy, ar, -135, 135, mix(t['surface'], t['ink4'], 0.6), 1.5)
    if abs(a1 - a0) > 1.5:
        out += _arc(cx, cy, ar, a0, a1, col, 1.5)
    elif tick and bipolar:
        out += f'<rect x="{cx - 0.5}" y="{cy - ar - 1}" width="1" height="2.5" fill="{t["ink3"]}"/>'
    if auto:
        out += f'<circle cx="{cx + r + 2}" cy="{cy + r + 1}" r="2" fill="{t["auto"]}"/>'
    return out


def _arc(cx, cy, r, a0, a1, col, w):
    x0, y0 = cx + math.sin(math.radians(a0)) * r, cy - math.cos(math.radians(a0)) * r
    x1, y1 = cx + math.sin(math.radians(a1)) * r, cy - math.cos(math.radians(a1)) * r
    large = 1 if (a1 - a0) > 180 else 0
    return (f'<path d="M{x0:.2f} {y0:.2f}A{r} {r} 0 {large} 1 {x1:.2f} {y1:.2f}" fill="none" '
            f'stroke="{col}" stroke-width="{w}" stroke-linecap="round"/>')


def key(t, x, y, w, h, label=None, on=None, size=10, dim=False, glyph=None):
    """A strip key: a recessed well; on = filled with the state's colour, dark ink."""
    if on:
        fill = on
        out = rect(x, y, w, h, mix(on, '#000000', 0.25) if t['name'] == 'Night' else mix(on, '#000000', 0.15), 3)
        out += rect(x, y, w, h - 1, fill, 3)
        ink = '#10151a' if t['name'] == 'Night' else '#ffffff'
    else:
        out = well(t, x, y, w, h, 3)
        ink = t['ink4'] if dim else t['ink3']
    if label:
        out += text(x + w / 2, y + h / 2 + size * 0.36, label, size, ink, 600, 'middle')
    if glyph:
        out += glyph(x + w / 2, y + h / 2 - 0.5, ink)
    return out


def ghost_ring(t, x, y, w, h, col):
    """A key silenced by another strip's solo: its well with a thin ring of the solo colour."""
    return (well(t, x, y, w, h, 3) +
            f'<rect x="{x + 1.5}" y="{y + 1.5}" width="{w - 3}" height="{h - 4}" rx="2" fill="none" '
            f'stroke="{col}" stroke-width="1" opacity="0.45"/>')


def fader(t, gx, top, bot, db, cap_w=20, cap_h=11, hue=None, quiet=False, ticks=True, tick_x=None,
          frozen=False, ghost=None):
    """The groove, the scale's ticks and the cap at the level."""
    fy = lambda d: top + (bot - top) * frac(d)  # noqa: E731
    out = rect(gx - 1.5, top - 3, 3, bot - top + 6, mix(t['well'], t['edge'], 0.55), 1.5)
    if ticks:
        tx = tick_x if tick_x is not None else gx + cap_w / 2 + 2
        for d in TICKS:
            yy = round(fy(d))
            out += rect(tx, yy, 3 if d == 0 else 2, 1, t['ink3'] if d == 0 else mix(t['ink4'], t['base'], 0.25))
    if ghost is not None:      # a VCA's offset: where the fader sits after the VCA
        gy = round(fy(ghost))
        out += rect(gx - cap_w / 2 + 2, gy, cap_w - 4, 1, t['ink3'])
    cy = round(fy(db))
    if quiet:
        return out + rect(gx - cap_w / 2 + 2, cy - 2, cap_w - 4, 4, mix(t['surface'], t['ink4'], 0.45), 2)
    hw, hh = cap_w / 2, cap_h // 2
    out += rect(gx - hw - 1, cy - hh - 1, cap_w + 2, cap_h + 2, t['edge'], 2.5)
    out += rect(gx - hw, cy - hh, cap_w, cap_h, f'url(#fcap-{t["name"]})', 2)
    capc = '#3b444e' if t['name'] == 'Night' else '#ffffff'
    grip = mix(capc, t['edge'], 0.55)
    if cap_h >= 11:
        out += rect(gx - hw + 4, cy - hh + 2, cap_w - 8, 1, grip) + rect(gx - hw + 4, cy + hh - 2, cap_w - 8, 1, grip)
    out += rect(gx - hw + 2, cy, cap_w - 4, 1, hue or t['ink1'])
    return out


def meter(t, mx, top, bot, lv=None, pk=None, mw=3, clip=False, dim=False, gap=1):
    """Two bars on the fader's law. Idle: the grooves only — no −∞, no black slab.
    Clip: the cell above the bars lights; it stays until clicked."""
    fy = lambda d: top + (bot - top) * frac(d)  # noqa: E731
    groove = mix(t['well'], t['ink4'], 0.22)
    out = ''
    for k in range(2):
        out += rect(mx + k * (mw + gap), top - 3, mw, bot - top + 6, groove, 1)
    # the clip cell: a slot above the bars, lit red when the strip went over 0 dBFS
    cw = 2 * mw + gap
    out += rect(mx, top - 8, cw, 3, t['clip'] if clip else mix(t['well'], t['ink4'], 0.30), 1)
    if lv:
        lo_c, mid_c, hi_c = t['meter_lo'], t['meter_mid'], t['meter_hi']
        if dim:
            lo_c, mid_c, hi_c = (mix(t['surface'], c, 0.38) for c in (lo_c, mid_c, hi_c))
        ylo, yhi, y0 = round(fy(-12)), round(fy(-3)), round(fy(0))
        bottom = bot + 3
        for k, v in enumerate(lv):
            x = mx + k * (mw + gap)
            top_y = round(fy(v))
            out += rect(x, max(top_y, ylo), mw, bottom - max(top_y, ylo), lo_c)
            if top_y < ylo:
                out += rect(x, max(top_y, yhi), mw, ylo - max(top_y, yhi), mid_c)
            if top_y < yhi:
                out += rect(x, max(top_y, y0), mw, yhi - max(top_y, y0), hi_c)
            if top_y < y0:
                out += rect(x, top_y, mw, y0 - top_y, t['clip'])
            if pk:
                py = round(fy(pk[k]))
                hold = t['clip'] if pk[k] > 0 else mix(lo_c if pk[k] < -12 else (mid_c if pk[k] < -3 else hi_c), t['ink1'], 0.35)
                out += rect(x, py, mw, 1, hold)
    return out


def insert_rows(t, x, y, w, rows, fx, row_h=16, size=10, show_unit=False, frozen=False, key_from=None):
    """The used inserts, one quiet line each; the first empty row offers '+' faintly;
    the rest stay empty (never 'Slot 3 … Slot 10')."""
    # no well: the rows sit on the strip; a faint panel a step darker holds them together
    out = rect(x, y, w, rows * row_h + 4, mix(t['surface'], t['well'], 0.45), 3)
    for k in range(rows):
        ry = y + 2 + k * row_h
        cy = ry + row_h / 2
        if k < len(fx):
            nm, kind, on = fx[k]
            if on:
                out += f'<circle cx="{x + 7}" cy="{cy}" r="2" fill="{t["ink3"] if frozen else t["ink2"]}"/>'
            else:
                out += f'<circle cx="{x + 7}" cy="{cy}" r="2" fill="none" stroke="{t["ink4"]}" stroke-width="1"/>'
            label = nm if show_unit else kind
            col = t['ink4'] if (not on or frozen) else t['ink1'] if show_unit else t['ink2']
            room = w - 16 - (12 if (key_from and kind == 'Comp') else 0)
            out += text(x + 13, cy + size * 0.36, fit(label, size, room), size, col)
            if show_unit and on and not frozen:
                pass
            if key_from and kind == 'Comp':
                out += M.key(x + w - 9, cy, 12, t['ink2'])
        elif k == len(fx) and not frozen:
            out += M._sp(f'M{x + 5} {cy + 0.5}H{x + 10}M{x + 7.5} {cy - 2}V{cy + 3}', t['ink4'], 1, 'butt')   # on the pixel grid
    if len(fx) > rows:
        more = f'+{len(fx) - rows}'
        out += text(x + w - 4, y + rows * row_h + 1, more, 9, t['ink3'], 600, 'end')
    return out


def eq_thumb(t, x, y, w, h, gains, dots=False, dim=False):
    """The strip's three bands as a curve in a well; a flat EQ is one faint line."""
    out = rect(x, y, w, h, mix(t['surface'], t['well'], 0.45), 3)
    mid = y + h / 2 - 0.5
    lo, mi, hi = gains
    flat = not any(gains)
    span = h / 2 - 3

    def resp(lf):
        g = lo / (1 + math.exp((lf - 2.0) * 6))
        g += mi * math.exp(-((lf - 3.0) ** 2) / 0.05)
        g += hi / (1 + math.exp(-(lf - 3.9) * 6))
        return g
    pts = []
    n = int(w - 4)
    for k in range(n + 1):
        lf = math.log10(20) + 3 * k / n
        pts.append((x + 2 + k, mid - resp(lf) * span / 6))
    d = 'M' + 'L'.join(f'{px:.1f} {py:.1f}' for px, py in pts)
    ink = t['ink4'] if (flat or dim) else t['ink2']
    if not flat and not dim:
        out += f'<path d="{d}L{x + 2 + n} {mid}L{x + 2} {mid}Z" fill="{t["accent"]}" opacity="0.12"/>'
    out += f'<path d="{d}" fill="none" stroke="{ink}" stroke-width="1"/>'
    if dots and not flat:
        for lf, col in ((2.0, t['solo']), (3.0, t['play']), (3.9, t['accent'])):
            px = x + 2 + n * (lf - math.log10(20)) / 3
            out += f'<circle cx="{px:.1f}" cy="{mid - resp(lf) * span / 6:.1f}" r="2" fill="{t["well"]}" stroke="{col}" stroke-width="1"/>'
    return out


def send_rows(t, x, y, w, sends, rows, row_h=14, size=10, show_value=False, short=True):
    """A send: the target's name and a thin bar of its level (0 dB at the bar's end)."""
    out = ''
    for k in range(rows):
        ry = y + k * row_h
        cy = ry + row_h / 2
        if k < len(sends):
            tgt, db, on = sends[k]
            col = t['ink2'] if on else t['ink4']
            bx = x + (32 if short else 44)
            out += text(x, cy + size * 0.36, fit(tgt, size, bx - x - 3), size, col)
            bw = w - (bx - x) - (30 if show_value else 0)
            fr = 1 - frac(db)
            out += rect(bx, cy - 1, bw, 2, mix(t['well'], t['ink4'], 0.35), 1)
            out += rect(bx, cy - 1, round(bw * fr), 2, (t['accent'] if on else t['ink4']), 1)
            if show_value:
                out += text(x + w, cy + size * 0.36, dbs(db), size, t['ink2'] if on else t['ink4'], 400, 'end')
        elif k == len(sends):
            out += M._sp(f'M{x + 1} {cy + 0.5}H{x + 6}M{x + 3.5} {cy - 2}V{cy + 3}', t['ink4'], 1, 'butt')
    return out


def send_rows_large(t, x, y, w, sends, rows, row_h=20):
    """Large: each send on two lines — name and value, then its bar across the strip."""
    out = ''
    for k in range(rows):
        ry = y + k * row_h
        if k < len(sends):
            tgt, db, on = sends[k]
            col = t['ink2'] if on else t['ink4']
            out += text(x, ry + 10, fit(tgt, 11, w - 34), 11, col)
            out += text(x + w, ry + 10, dbs(db), 11, t['ink1'] if on else t['ink4'], 400, 'end')
            out += rect(x, ry + 14, w, 2, mix(t['well'], t['ink4'], 0.35), 1)
            out += rect(x, ry + 14, round(w * (1 - frac(db))), 2, t['accent'] if on else t['ink4'], 1)
        elif k == len(sends):
            out += M._sp(f'M{x + 1} {ry + 7.5}H{x + 6}M{x + 3.5} {ry + 5}V{ry + 10}', t['ink4'], 1, 'butt')
    return out


def out_chip(t, cx, y, dest, w, size=10, hue=None, full=False):
    """Where the strip goes. To the master it says so quietly; to a bus, the bus's name in ink 2
    after a short arrow, the bus's colour as a 2-px dot."""
    if dest in ('Master', 'Máster') and not full:
        return text(cx, y + size, WORDS['master'], size, t['ink4'] if t['name'] == 'Night' else t['ink3'], 400, 'middle')
    label = fit(dest, size, w - 14)
    lw = tw(label, size)
    x0 = cx - (lw + 12) / 2
    out = M.out_arrow(x0 + 4, y + size * 0.64, 12, t['ink3'])
    out += text(x0 + 11, y + size, label, size, t['ink2'], 400)
    return out


def socket(t, cx, cy, lit=None, kind='out'):
    """The cable band's socket: a ring; lit when the selected strip reaches it."""
    if lit:
        return (f'<circle cx="{cx}" cy="{cy}" r="4" fill="{mix(t["base"], lit, 0.25)}"/>'
                f'<circle cx="{cx}" cy="{cy}" r="2.5" fill="{lit}"/>')
    return f'<circle cx="{cx}" cy="{cy}" r="2.5" fill="none" stroke="{mix(t["ink4"], t["base"], 0.2)}" stroke-width="1"/>'


def tag(t, x, y, s, col, size=9, fill=None):
    """A small tag (VCA 1, Frozen, Key): SemiBold 9 in a pill of its colour at low strength."""
    w = tw(s, size, 600) + 8
    bg = fill or mix(t['surface'], col, 0.20)
    return rect(x, y, w, 12, bg, 2) + text(x + w / 2, y + 9, s, size, col, 600, 'middle'), w


def hatch(t, x, y, w, h, col, op=0.10, step=5, uid='h'):
    """A frozen block: fine diagonal lines of a cold colour over the strip's sound block."""
    pid = f'hatch-{uid}-{t["name"]}'
    return (f'<defs><pattern id="{pid}" width="{step}" height="{step}" patternUnits="userSpaceOnUse" '
            f'patternTransform="rotate(45)"><rect width="1" height="{step}" fill="{col}" opacity="{op}"/></pattern></defs>'
            f'<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="url(#{pid})"/>')


__all__ = ['frac', 'dbs', 'fit', 'knob', 'key', 'fader', 'meter', 'insert_rows', 'eq_thumb', 'send_rows',
           'out_chip', 'socket', 'tag', 'hatch', 'ghost_ring', 'esc']
