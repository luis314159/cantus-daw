"""des-mixer — the build sheets: the strips by density, the redlines, the states, the
inspector, the overflow cases. Each writes one HTML page per theme."""
import copy
import data as D
import strip as ST
import desk as DK
import window as W
from base import NIGHT, PAPER, defs, rect, text, mix, tw, page


def sheet_bg(t):
    return t['gutter']


def heading(t, x, y, s, sub=None):
    out = text(x, y, s, 20, t['ink1'], 500)
    if sub:
        out += text(x, y + 20, sub, 11, t['ink3'])
    return out


def cap(t, x, y, s, sub=None, w=None, anchor='start'):
    out = text(x, y, s, 12, t['ink1'], 500, anchor)
    if sub:
        out += text(x, y + 15, sub, 10, t['ink3'], 400, anchor)
    return out


def panel(t, x, y, w, h):
    return rect(x, y, w, h, t['base'], 8)


def song(name):
    for s in D.STRIPS + D.RETURNS + [D.MASTER, D.VCA]:
        if s.get('name') == name:
            return copy.deepcopy(s)
    raise KeyError(name)


# ------------------------------------------------------------------ strips by density
def strips_sheet(t):
    H = 616
    out = [defs(t), rect(0, 0, 1280, 800, sheet_bg(t))]
    out.append(heading(t, 24, 36, 'The strips — master, track, return in Compact, Wide and Large',
                       'Slate · 1:1 · the mixer maximised (616 px of strip) · playing at bar 44 · Lead at rest, Reverb a return'))
    x = 24
    y = 96
    lead = song('Lead')
    rev = song('Reverb')
    mas = song('Master')
    for dens, label in (('compact', 'Compact · 30 (pitch 32)'), ('wide', 'Wide · 60 (pitch 62)'),
                        ('large', 'Large · 94 (pitch 96)')):
        gw = ST.MASTER_W[dens] + 2 * ST.WIDTH[dens] + 4 * 8
        out.append(panel(t, x - 8, y - 8, gw, H + 56))
        out.append(cap(t, x, y + 8, label, 'master · track · return'))
        sx = x
        sy = y + 28
        out.append(ST.strip(t, sx, sy, H, mas, dens))
        sx += ST.MASTER_W[dens] + 8
        out.append(ST.strip(t, sx, sy, H, lead, dens))
        sx += ST.WIDTH[dens] + 8
        out.append(ST.strip(t, sx, sy, H, rev, dens))
        x += gw + 16
    # the desk fits first, then scrolls: Wide at three heights
    fx = x
    for Hh, label in ((440, 'Wide · 440'), (288, 'Wide · 288 (docked under the arrangement)')):
        pl = ST.plan('wide', Hh)
        gone = ', '.join(pl['_dropped']).replace('ins_fold', 'inserts fold to a line') or 'nothing'
        gw = 2 * ST.WIDTH['wide'] + 8 + 16
        out.append(panel(t, fx - 8, y - 8, gw, Hh + 56 + 30))
        out.append(cap(t, fx, y + 8, label))
        out.append(text(fx, y + Hh + 56, 'gone: ' + gone, 10, t['ink3']))
        out.append(ST.strip(t, fx, y + 28, Hh, lead, 'wide'))
        out.append(ST.strip(t, fx + 62 + 8, y + 28, Hh, rev, 'wide'))
        fx += gw + 16
    return ''.join(out), 1280, 800


# ------------------------------------------------------------------ redlines
def redline(t):
    """Each density's track strip at x1.5 with its rows measured: name, y, height."""
    S = 1.5
    H = 616
    out = [defs(t), rect(0, 0, 1280, 1040, sheet_bg(t))]
    out.append(heading(t, 24, 36, 'Redlines — the track strip, row by row (drawn x1.5, measured in 1x px)',
                       'y from the strip top · h the row height · the fader row is flexible; gaps 4 px (Large 6) · the cap overhangs 10 px above and below'))
    lead = song('Lead')
    x = 40
    acc = t['accent']
    for dens in ('compact', 'wide', 'large'):
        w = ST.WIDTH[dens]
        pl = ST.plan(dens, H)
        g = f'<g transform="translate({x},{80}) scale({S})">' + ST.strip(t, 0, 0, H, lead, dens) + '</g>'
        out.append(g)
        out.append(text(x, 72, f'{dens.capitalize()} · {w} px', 12, t['ink1'], 500))
        lx = x + w * S + 10
        for name, v in pl.items():
            if name.startswith('_'):
                continue
            ry, rh = v
            y0 = 80 + ry * S
            y1 = 80 + (ry + rh) * S
            out.append(rect(x - 6, y0, w * S + 12, 1, acc, extra=' opacity="0.55"'))
            out.append(f'<path d="M{lx + 0.5} {y0 + 1}V{y1 - 1}" stroke="{acc}" stroke-width="1"/>')
            out.append(rect(lx - 2, y0, 5, 1, acc) + rect(lx - 2, y1 - 1, 5, 1, acc))
            out.append(text(lx + 6, (y0 + y1) / 2 + 4, f'{name}  y{ry} · h{rh}', 10, t['ink2']))
        out.append(rect(x - 6, 80 + H * S, w * S + 12, 1, acc, extra=' opacity="0.55"'))
        x += w * S + 190
    # the strip's horizontal measures, Wide (x from the strip's left edge)
    hx = x - 20
    rows = [('Body', 'w 60, r 3, pitch 62 (2 px of base between)'), ('Stripe', 'h 3, user colour at 72 % (lit 100 % selected)'),
            ('Number', 'SemiBold 10 / Regular 10 ink 3, centre, baseline 15'),
            ('Name', 'Regular 12 ink 1 (Medium selected), baseline 30; 11 px if wider than 54; then two lines, the number leading; then …'),
            ('Inserts', 'panel x3 w54, rows 16; dot r2 at x10; name Regular 10 at x16'),
            ('EQ', 'panel x3 w54 h22, curve 1 px ink 2, fill accent 12 %'),
            ('Polarity row', 'Ø and ⇄ drawn for 16 px, centred at x11 and x29, no well at rest; width knob r6 at x48'),
            ('Pan', 'knob r9 at x30; arc r12 from the centre, 1.5 px'),
            ('M · S · arm', '3 keys share x3–x57, 2 apart (17 · 16 · 17), h16; SemiBold 10'),
            ('Fader', 'groove 3 px at x21; cap 22 x 13 (edge 1); ticks at x35'),
            ('Meter', '2 bars 4 px, 1 apart, at x37; clip cell 9 x 3, 8 above the travel'),
            ('Level', 'Regular 11 ink 2 (ink 1 selected), centre'),
            ('Sends', 'rows 14; name Regular 10 at x5 (fits 29); bar 2 px from x37 to x55'),
            ('Output', 'arrow + bus name Regular 10 ink 2; "Master" ink 4'),
            ('Socket', 'ring r2.5 at the centre, 7 below the row top; lit r2.5 + halo r4')]
    out.append(text(hx, 72, 'Wide — across', 12, t['ink1'], 500))
    for k, (a, b) in enumerate(rows):
        yy = 96 + k * 34
        out.append(text(hx, yy, a, 11, t['ink2'], 600))
        out.append(text(hx, yy + 14, b, 10, t['ink3']))
    return ''.join(out), 1280, 1040


def main(write):
    import sheets2
    pages = {'strips': strips_sheet, 'redline': redline}
    pages.update(sheets2.PAGES)
    for t in (NIGHT, PAPER):
        k = t['name'].lower()
        for name, fn in pages.items():
            body, w, h = fn(t)
            write(f'{name}-{k}', page(f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">{body}</svg>',
                                      w, h, sheet_bg(t)))
