"""Slate (dir-a), phase B — the dialog frame and its parts.

A dialog lies on a scrim over the window. The frame is the bar's surface with a lit top
edge, radius 10, one soft shadow; its title row 44 high (title 15 Medium, the close key at
the right). A side list, when there is one, sits on the darker base — separated by value.
Rows: label column 150 (11 Regular ink-2), controls from there, 32 apart; section titles
11 SemiBold ink-3 in Title case, 24 above their first row. Buttons 28 high: the primary an
accent fill with its ink, the secondary a well with ink-1; a quiet one is just words.
"""
import glyphs as G
from base import rect, text, well, mix, tw, lit_edge
from topbar import segmented
from overflow import toggle, keycap

ROW = 32
LABEL = 150


def scrim(t):
    c = 'rgba(6,9,12,0.58)' if t['name'] == 'Night' else 'rgba(32,44,56,0.34)'
    return f'<rect width="1280" height="720" fill="{c}"/>'


def frame(t, x, y, w, h, title, sub=None, uid='d'):
    edge = mix(t['surface'], t['ink1'], 0.09) if t['name'] == 'Night' else t['line']
    out = (f'<filter id="dsh{uid}" x="-20%" y="-20%" width="140%" height="140%"><feDropShadow dx="0" dy="14" '
           f'stdDeviation="22" flood-color="#000" flood-opacity="{0.6 if t["name"] == "Night" else 0.22}"/></filter>'
           f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="10" fill="{t["surface"]}" filter="url(#dsh{uid})"/>'
           f'<rect x="{x + 0.5}" y="{y + 0.5}" width="{w - 1}" height="{h - 1}" rx="9.5" fill="none" stroke="{edge}"/>')
    out += rect(x + 10, y + 1, w - 20, 1, mix(t['surface'], t['ink1'], 0.08) if t['name'] == 'Night' else '#f8fafb')
    out += text(x + 20, y + 28, title, 15, t['ink1'], 500)
    if sub:
        out += text(x + 20 + tw(title, 15, 500) + 10, y + 28, sub, 11, t['ink3'])
    out += close_key(t, x + w - 38, y + 10)
    return out


def close_key(t, x, y):
    out = well(t, x, y, 26, 24, 4)
    c = (x + 13, y + 11.5)
    s = 3.5
    out += (f'<path d="M{c[0] - s} {c[1] - s}L{c[0] + s} {c[1] + s}M{c[0] + s} {c[1] - s}L{c[0] - s} {c[1] + s}" '
            f'stroke="{t["ink2"]}" stroke-width="1.3" stroke-linecap="round"/>')
    return out


def section(t, x, y, s, right=None):
    out = text(x, y, s, 11, t['ink3'], 600, track=0.2)
    if right:
        out += right
    return out


def label(t, x, y, s, note=None):
    out = text(x, y + 17, s, 11, t['ink2'])
    if note:
        out += text(x, y + 30, note, 10, t['ink3'])
    return out


def dropdown(t, x, y, w, s, sub=None, h=24, lead=None):
    out = well(t, x, y, w, h, 4)
    tx = x + 9
    if lead:
        out += lead
        tx = x + 28
    out += text(tx, y + h / 2 + 4.5, s, 12, t['ink1'])
    if sub:
        out += text(tx + tw(s, 12) + 8, y + h / 2 + 4.5, sub, 11, t['ink3'])
    out += G.chevron_down(x + w - 11, y + h / 2, 2.5, t['ink3'], 1.2)
    return out


def field(t, x, y, w, s, h=24, ph=False, focus=False):
    out = ''
    if focus:
        out += rect(x - 2, y - 2, w + 4, h + 4, mix(t['surface'], t['accent'], 0.45), 6)
    out += well(t, x, y, w, h, 4)
    out += text(x + 9, y + h / 2 + 4.5, s, 12, t['ink3'] if ph else t['ink1'])
    if focus:
        out += rect(round(x + 10 + tw(s, 12)), y + 5, 1, h - 10, t['accent'])
    return out


def button(t, x, y, s, kind='secondary', w=None, h=28, lead=None):
    w = w or round(tw(s, 12, 500) + 28 + (18 if lead else 0))
    if kind == 'primary':
        out = rect(x, y, w, h, t['accent'], 5)
        out += rect(x + 4, y, w - 8, 1, mix(t['accent'], '#ffffff', 0.25))
        col = t['accent_ink']
    elif kind == 'quiet':
        out = ''
        col = t['ink2']
    else:
        out = well(t, x, y, w, h, 5)
        col = t['ink1']
    tx = x + w / 2 + (9 if lead else 0)
    if lead:
        out += lead(x + 16, y + h / 2)
    out += text(tx, y + h / 2 + 4.5, s, 12, col, 500, 'middle')
    return out, w


def lamp(t, cx, cy, col):
    return (f'<circle cx="{cx}" cy="{cy}" r="4.5" fill="{col}" opacity="0.2"/>'
            f'<circle cx="{cx}" cy="{cy}" r="2.5" fill="{col}"/>')


def meter_pair(t, x, y, w, lv=None, h=3):
    out = ''
    groove = mix(t['well'], t['ink4'], 0.28)
    for k in range(2):
        out += rect(x, y + k * (h + 2), w, h, groove, 1)
        if lv:
            lw = round(w * lv[k])
            out += rect(x, y + k * (h + 2), min(lw, round(w * 0.75)), h, t['meter_lo'], 1)
            if lw > w * 0.75:
                out += rect(x + round(w * 0.75), y + k * (h + 2), lw - round(w * 0.75), h, t['meter_mid'])
    return out


def check(t, x, y, on, s=None):
    if on:
        out = rect(x, y, 14, 14, t['accent'], 3)
        out += (f'<path d="M{x + 3.5} {y + 7}L{x + 6} {y + 9.5}L{x + 10.5} {y + 4.5}" fill="none" stroke="{t["accent_ink"]}" '
                f'stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>')
    else:
        out = well(t, x, y, 14, 14, 3)
    if s:
        out += text(x + 22, y + 11, s, 12, t['ink1'])
    return out


def sidebar(t, x, y, w, h, items, chosen):
    out = rect(x, y, w, h, t['base'])
    yy = y + 12
    for k, (glyph, s) in enumerate(items):
        if k == chosen:
            out += rect(x + 8, yy, w - 16, 30, t['raised'] if t['name'] == 'Night' else '#ffffff', 5)
            out += rect(x + 8, yy + 8, 2, 14, t['accent'], 1)
        col = t['ink1'] if k == chosen else t['ink2']
        if glyph:
            out += G.nat(glyph, x + 28, yy + 15, 20, col, t['ink3'])
        out += text(x + 44, yy + 19.5, s, 12, col, 500 if k == chosen else 400)
        yy += 34
    return out


__all__ = ['scrim', 'frame', 'section', 'label', 'dropdown', 'field', 'button', 'lamp', 'meter_pair',
           'check', 'sidebar', 'segmented', 'toggle', 'keycap', 'ROW', 'LABEL']
