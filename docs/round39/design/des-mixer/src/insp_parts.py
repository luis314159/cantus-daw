"""des-mixer — the inspector's parts: section titles (open and folded), label/value rows,
fields with a menu, a level bar, a toggle, an LED, a small segmented control."""
import glyphs as G
from base import rect, text, well, mix, tw
from topbar import segmented

P = 12          # side padding
ROW = 24        # a label/value row
LAB_W = 64      # the label column (values start here)


def title(t, x, w, y, s, right=None, folded=False, right_col=None):
    """A section's title: a small chevron (sections fold), the title, its state at the right.
    Open: SemiBold 11 in ink 3. Folded: Medium 11 in ink 2, the summary in ink 3."""
    out = rect(x + P, y - 13, w - 2 * P, 1, mix(t['base'], t['line'], 0.55))
    if folded:
        out += G.chevron_right(x + P + 3, y - 3.5, 2.5, t['ink3'], 1.2)
        out += text(x + P + 12, y, s, 11, t['ink2'], 500)
    else:
        out += G.chevron_down(x + P + 3, y - 4, 2.5, t['ink4'] if t['name'] == 'Night' else t['ink3'], 1.2)
        out += text(x + P + 12, y, s, 11, t['ink3'], 600, track=0.2)
    if right:
        out += text(x + w - P, y, right, 10 if not folded else 11, right_col or t['ink3'], 400, 'end')
    return out


def label(t, x, y, s):
    return text(x + P, y, s, 11, t['ink3'])


def field(t, x, y, w, s, sub=None, chevron=True, h=24, ink=None, icon=None):
    """A field that opens a menu: a recessed well, the value, its detail, a chevron."""
    out = well(t, x, y, w, h, 4)
    tx = x + 8
    if icon:
        out += icon(x + 12, y + h / 2 - 0.5)
        tx = x + 22
    out += text(tx, y + h / 2 + 4, s, 12, ink or t['ink1'])
    if sub:
        out += text(tx + tw(s, 12) + 6, y + h / 2 + 4, sub, 10, t['ink3'])
    if chevron:
        out += G.chevron_down(x + w - 10, y + h / 2 - 0.5, 2.5, t['ink3'], 1.2)
    return out


def hbar(t, x, y, w, frac, col=None, bipolar=False, knob=True):
    """A level bar: the groove, the value from the minimum (or the centre), a small handle."""
    out = rect(x, y, w, 3, mix(t['well'], t['ink4'], 0.3), 1.5)
    if bipolar:
        c = x + w / 2
        v = x + w * frac
        a, b = sorted((c, v))
        out += rect(c - 0.5, y - 2, 1, 7, t['ink4'])
        if b - a > 0.5:
            out += rect(a, y, b - a, 3, col or t['ink2'], 1.5)
    else:
        out += rect(x, y, round(w * frac), 3, col or t['ink2'], 1.5)
    if knob:
        hc = t['raised'] if t['name'] == 'Night' else '#ffffff'
        out += f'<circle cx="{x + w * frac:.1f}" cy="{y + 1.5}" r="4" fill="{hc}" stroke="{t["edge"]}" stroke-width="1"/>'
    return out


def toggle(t, x, y, on):
    w, h = 26, 14
    if on:
        out = rect(x, y, w, h, t['accent'], 7)
        out += f'<circle cx="{x + w - 7}" cy="{y + 7}" r="5" fill="{t["accent_ink"] if t["name"] == "Paper" else "#eaf4fc"}"/>'
    else:
        out = well(t, x, y, w, h, 7)
        out += f'<circle cx="{x + 7}" cy="{y + 7}" r="4.5" fill="{t["ink3"]}"/>'
    return out


def led(t, cx, cy, on=True):
    if on:
        return (f'<circle cx="{cx}" cy="{cy}" r="4.5" fill="{t["play"]}" opacity="0.18"/>'
                f'<circle cx="{cx}" cy="{cy}" r="2.5" fill="{t["play"]}"/>')
    return f'<circle cx="{cx}" cy="{cy}" r="2.5" fill="none" stroke="{t["ink4"]}" stroke-width="1"/>'


def seg(t, x, y, labels, sel, h=20, size=11, pad=16, minw=34):
    ws = [max(minw, round(tw(s, size) + pad)) for s in labels]
    return segmented(t, x, y, labels, ws, h, sel, size), sum(ws)


def add_line(t, x, y, s, G_plus=True):
    out = G.plus(x + P + 6, y - 4, 3.5, t['ink3'], 1.2) if G_plus else ''
    return out + text(x + P + 16, y, s, 11, t['ink3'])


def sparkline(t, x, y, w, h, pts, col):
    """An automation lane in miniature: its curve, its points as dots."""
    d = 'M' + 'L'.join(f'{x + w * px:.1f} {y + h * (1 - py):.1f}' for px, py in pts)
    out = rect(x, y + h, w, 1, mix(t['base'], t['ink4'], 0.5))
    out += f'<path d="{d}" fill="none" stroke="{col}" stroke-width="1.2"/>'
    for px, py in pts:
        out += f'<circle cx="{x + w * px:.1f}" cy="{y + h * (1 - py):.1f}" r="1.6" fill="{col}"/>'
    return out
