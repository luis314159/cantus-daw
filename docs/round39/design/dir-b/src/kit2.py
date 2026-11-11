"""dir-b controls, part 2: fader + meter, toggle, tab, scrollbar, mini graph, menu."""
from core import mix, UI, NARROW, CLOCK
from kit import meter_db, R_KEY, R_PANEL
from glyphs2 import glyph

TICKS = (6, 0, -6, -12, -24, -48)


def meter(c, x, y, w, h, levels=None, peaks=None, clip=False, bars=2, gap=1, ticks=True):
    """A stereo meter in one narrow well. At rest it is a scale (hairline ticks across the
    well), never a dead black slab. `levels` in dB per bar or None (silent)."""
    t = c.t
    night = t["name"] == "Night"
    if levels:
        c.rect(x, y, w, h, t["well"] if night else t["key"], rx=1)
    else:  # quiet: a ghost of the well, its scale still there
        c.rect(x, y, w, h, mix(t["well"], t["panel"], 0.45) if night else
               mix(t["key"], t["panel"], 0.45), rx=1)
    bw = (w - 2 - gap * (bars - 1)) / bars
    gid = c.uid("mg")
    stops = [(0.0, t["meter_hi"]), (1 - meter_db(0), t["meter_hi"]),
             (1 - meter_db(-2), t["meter_mid"]), (1 - meter_db(-8), t["meter_mid"]),
             (1 - meter_db(-16), t["meter_lo"]), (1.0, mix(t["meter_lo"], t["well"], 0.35))]
    c.defs.append('<linearGradient id="%s" gradientUnits="userSpaceOnUse" x1="0" y1="%d" '
                  'x2="0" y2="%d">%s</linearGradient>' % (
                      gid, c.p(y + 1), c.p(y + h - 1),
                      "".join('<stop offset="%.3f" stop-color="%s"/>' % s for s in stops)))
    for i in range(bars):
        bx = x + 1 + i * (bw + gap)
        if levels:
            lv = meter_db(levels[i])
            top = y + 1 + (h - 2) * (1 - lv)
            c.rect(bx, top, bw, y + h - 1 - top, "url(#%s)" % gid)
        if peaks:
            pk = meter_db(peaks[i])
            py = y + 1 + (h - 2) * (1 - pk)
            c.rect(bx, py, bw, 1, t["meter_hi"] if peaks[i] > 0 else t["ink1"], op=0.9)
    if ticks:
        for db in TICKS:
            ty = y + 1 + (h - 2) * (1 - meter_db(db))
            c.hline(x, x + w, ty, t["panel"] if night else t["canvas"], op=0.9 if db else 1)
    if clip:
        c.rect(x, y - 3, w, 2, t["red"], rx=1)


def fader(c, x, y, h, v, state="rest", sel=False, capw=22, caph=11):
    """The fader: a 3-px slot, ticks at its left, a graphite cap with an index line."""
    t = c.t
    night = t["name"] == "Night"
    c.rect(x - 1.5, y, 3, h, t["well"] if night else t["key"], rx=1.5)
    for db in TICKS:
        ty = round(y + h * (1 - meter_db(db)))
        c.hline(x - capw / 2 + 1, x - 4, ty, t["ink4"] if db else t["ink3"])
    cy = y + h * (1 - v)
    cx0 = x - capw / 2
    body = t["raised"] if night else "#FFFFFF"
    if state in ("hover", "dragged"):
        body = t["hover"] if night else "#FFFFFF"
    # a soft contact shadow under the cap, then the cap
    c.rect(cx0, cy - caph / 2 + 1.5, capw, caph, t["shadow"], rx=2.5, op=0.35 if night else 0.18)
    c.rect(cx0, cy - caph / 2, capw, caph, body, rx=2.5, stroke=t["key_edge"])
    c.hline(cx0 + 2, cx0 + capw - 2, cy - caph / 2 + 1, t["ink2"] if night else "#FFFFFF",
            op=0.30 if night else 1)
    idx = t["gold"] if (sel or state == "dragged") else t["ink1"]
    c.rect(cx0 + 3, cy - 0.5, capw - 6, 1, idx)
    if state == "dragged":
        c.rect(cx0 - 1, cy - caph / 2 - 1, capw + 2, caph + 2, "none", rx=3.5, stroke=t["gold"])


def toggle(c, x, y, on, w=26, h=14, label_s=None):
    t = c.t
    track = mix(t["well"], t["gold"], 0.28) if on else (t["well"] if t["name"] == "Night" else t["key"])
    c.rect(x, y, w, h, track, rx=h / 2, stroke=t["gold"] if on else t["key_edge"])
    kx = x + w - h / 2 if on else x + h / 2
    c.circle(kx, y + h / 2, h / 2 - 2.5, fill=t["gold_hi"] if on else t["ink3"])
    if label_s:
        c.text(x + w + 7, y + h / 2 + 4, label_s, size=11.5, fill=t["ink1"] if on else t["ink2"])


def tab(c, x, y, h, text, active=False, size=12, g=None):
    """A tab is a word; the active one is ink-1 with a 2-px gold rule under it."""
    t = c.t
    w = round(len(text) * size * 0.50) + (18 if g else 0) + 20
    gx = x + 10
    if g:
        glyph(c, g, gx + 6, y + h / 2, 11, off=not active)
        gx += 18
    c.text(gx, y + h / 2 + size * 0.36, text, size=size, fill=t["ink1"] if active else t["ink3"],
           wt=600 if active else 500)
    if active:
        c.rect(x + 8, y + h - 2, w - 16, 2, t["gold"], rx=1)
    return w


def scrollbar(c, x, y, h, pos, frac, hover=False, w=6, horiz=False):
    t = c.t
    if horiz:
        c.rect(x + pos * h, y, max(24, frac * h), w, t["ink3"] if hover else t["ink4"], rx=w / 2,
               op=0.9 if hover else 0.7)
        return
    c.rect(x, y + pos * h, w, max(24, frac * h), t["ink3"] if hover else t["ink4"], rx=w / 2,
           op=0.9 if hover else 0.7)


def spark(c, x, y, w, h, vals, warn=None, fill=True, col=None):
    """A small live graph in a well: a filled line, newest at the right."""
    t = c.t
    c.rect(x, y, w, h, t["well"], rx=2)
    col = col or t["ink2"]
    n = len(vals)
    pts = [(x + 1 + (w - 2) * i / (n - 1), y + h - 1 - (h - 3) * v) for i, v in enumerate(vals)]
    s = c.s
    d = "M" + " L".join("%.2f %.2f" % (px * s, py * s) for px, py in pts)
    if fill:
        c.add('<path d="%s L%.2f %.2f L%.2f %.2f Z" fill="%s" fill-opacity="0.22"/>' % (
            d, (x + w - 1) * s, (y + h - 1) * s, (x + 1) * s, (y + h - 1) * s, col))
    c.add('<path d="%s" fill="none" stroke="%s" stroke-width="%.2f" stroke-linejoin="round"/>'
          % (d, col, 1.0 * s))
    if warn is not None:
        wy = y + h - 1 - (h - 3) * warn
        c.hline(x + 1, x + w - 1, wy, t["ember"], op=0.45)


def popover(c, x, y, w, h):
    """A floating panel: raised surface, 1-px rim, a soft shadow (the one place that has one)."""
    t = c.t
    night = t["name"] == "Night"
    for i, op in ((8, 0.06), (5, 0.10), (2.5, 0.16)):
        c.rect(x - i / 2, y + i / 2, w + i, h + i / 2, t["shadow"], rx=R_PANEL + i / 2,
               op=op * (1.6 if night else 0.8))
    c.rect(x, y, w, h, t["raised"] if night else "#FBFAF8", rx=R_PANEL,
           stroke=t["line_strong"] if night else t["line"])


def menu(c, x, y, w, items, hi=None, sub_at=None):
    """items: (text, shortcut, kind) with kind in '', 'sub', 'check', 'sep', 'dis'."""
    t = c.t
    row = 24
    h = sum(9 if k == "sep" else row for _, _, k in items) + 10
    popover(c, x, y, w, h)
    yy = y + 5
    rects = {}
    for i, (s, sc, k) in enumerate(items):
        if k == "sep":
            c.hline(x + 10, x + w - 10, yy + 4, t["line"])
            yy += 9
            continue
        if i == hi:
            c.rect(x + 4, yy, w - 8, row, mix(t["raised"], t["gold"], 0.16) if t["name"] == "Night"
                   else mix("#FBFAF8", t["gold_hi"], 0.25), rx=3)
        col = t["ink4"] if k == "dis" else t["ink1"]
        if k == "check":
            c.path("M%.1f %.1f L%.1f %.1f L%.1f %.1f" % (x + 12, yy + 12, x + 15, yy + 15,
                                                     x + 20, yy + 9), stroke=t["gold"], sw=1.5)
        c.text(x + 28, yy + 16, s, size=12, fill=col, wt=500)
        if sc:
            c.text(x + w - 14, yy + 16, sc, size=11, fill=t["ink3"], wt=500, anchor="end", tnum=True)
        if k == "sub":
            glyph(c, "chev_right", x + w - 14, yy + row / 2, 7, ink1=t["ink2"], ink2=t["ink2"])
        rects[i] = (x, yy, w, row)
        yy += row
    return rects, h
