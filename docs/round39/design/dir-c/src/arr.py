"""The arrangement: the focal point. Content glows on the deepest ground; the headers are
quiet; the selected track is the one lit thing in its column."""
from common import SANS, MONO, mix
from glyphs import glyph
from metrics import width
from tokens import USER, hue
import song
from top import TOP_H, key, caption

HW = 168          # header column
PB = 52           # px per bar
ROW = 32
TAB_H = 28
RULER = 24
SEL = "chords"


def lanes_x(W):
    return HW, W - 8


def track_icon(g, t, kind, x, y, c):
    if kind == "drum":
        g.raw(f'<ellipse cx="{x + 6}" cy="{y + 3.5}" rx="4.6" ry="1.7" fill="none" stroke="{c}" stroke-width="1"/>')
        g.path(f"M{x + 1.4} {y + 3.5} V{y + 8.6} A4.6 1.7 0 0 0 {x + 10.6} {y + 8.6} V{y + 3.5}", stroke=c, sw=1)
    elif kind == "synth":
        g.path(f"M{x} {y + 6} C{x + 2} {y + 0.5} {x + 4} {y + 0.5} {x + 6} {y + 6} S{x + 10} {y + 11.5} {x + 12} {y + 6}",
               stroke=c, sw=1.15)
    elif kind == "voice":
        g.box(x + 4, y + 0.5, 4, 7, None, c, rx=2)
        g.path(f"M{x + 2} {y + 5.5} A4 4 0 0 0 {x + 10} {y + 5.5} M{x + 6} {y + 9.6} V{y + 11.5}", stroke=c, sw=1)


def tab_row(g, t, x0, x1, y, tabs, active=0):
    g.rect(x0, y, x1 - x0, TAB_H, t["bar"])
    g.hline(x0, x1, y + TAB_H - g.px, t["hair"])
    x = x0 + 4
    out = []
    for i, name in enumerate(tabs):
        w = width(name, 12, 500) + 24
        if i == active:
            g.rect(x, y + 4, w, TAB_H - 4, t["base"])
            g.vline(x, y + 4, y + TAB_H, t["hair"])
            g.vline(x + w - g.px, y + 4, y + TAB_H, t["hair"])
            g.hline(x, x + w, y + 4, t["hair"])
        g.text(x + 12, y + 19, name, 12, t["ink1"] if i == active else t["ink3"], 500)
        out.append((x, w))
        x += w
    return x


def arrangement(g, t, W, y0, y1, playing=False):
    lx0, lx1 = lanes_x(W)
    # the tab row: the panel's name, then its tools
    x = tab_row(g, t, 0, W, y0, ["Arrangement", "Clips"])
    tx = x + 14
    g.vline(tx - 8, y0 + 8, y0 + 20, t["hair"])
    for i, (n, st) in enumerate([("select", "on"), ("draw", "rest"), ("slice", "rest"), ("erase", "rest")]):
        key(g, t, tx + i * 28, y0 + 3, 26, 22, n, st, gk=20)
    fx = W - 92
    g.box(fx, y0 + 5, 82, 18, None, t["line"], rx=9)
    g.circle(fx + 11, y0 + 14, 3, t["accent"])
    g.text(fx + 19, y0 + 18, "Follow play", 10.5, t["ink2"], 500)

    # ruler
    ry = y0 + TAB_H
    g.rect(0, ry, W, RULER, t["base"])
    g.hline(0, W, ry + RULER - g.px, t["hair"])
    g.text(14, ry + 16, "10 tracks", 10.5, t["ink3"], 400)
    g.path(f"M{HW - 22} {ry + 12} h10 M{HW - 17} {ry + 7} v10", stroke=t["ink2"], sw=1.25, cap="butt")
    # the loop: a thin accent rule over the looped bars, with its two ends
    g.rect(lx0 + 1, ry + 2, 16 * PB - 2, 2, t["accent"], op=0.9)
    for xe in (lx0 + 1, lx0 + 16 * PB - 2):
        g.rect(xe, ry + 2, 1, 5, t["accent"])
    g.text(lx0 + 4, ry + 16, "1", 10.5, t["ink2"], 400)
    g.text(lx0 + 13, ry + 16, "Drop", 10.5, t["ink1"], 600)
    for b in range(17):
        bx = lx0 + b * PB
        if bx > lx1:
            break
        g.vline(bx, ry + RULER - (9 if b % 4 == 0 else 6), ry + RULER, t["ink4"] if b % 4 else t["ink3"])
        if b < 16 and b % 1 == 0 and b > 0:
            g.text(bx + 4, ry + 16, str(b + 1), 10.5, t["ink2"] if b % 4 == 0 else t["ink3"], 400)
        for q in range(1, 4):
            g.vline(bx + q * PB / 4, ry + RULER - 3, ry + RULER, t["ink4"], op=0.7)

    # lanes and headers
    ty = ry + RULER
    g.rect(lx0, ty, lx1 - lx0, y1 - ty, t["well"])
    g.rect(0, ty, HW, y1 - ty, t["base"])
    for b in range(0, 17 * 4):
        bx = lx0 + b * PB / 4
        if bx >= lx1:
            break
        c = t["grid_4"] if b % 16 == 0 else t["grid_bar"] if b % 4 == 0 else t["grid_beat"]
        g.vline(bx, ty, y1, c)
    for i, (k, name, icon, ck, fn, kind) in enumerate(song.TRACKS):
        yy = ty + i * ROW
        sel = k == SEL
        header(g, t, i, k, name, icon, ck, yy, sel, kind, playing)
        g.hline(lx0, lx1, yy + ROW - g.px, t["grid_beat"])
        for c0 in (0, 8):
            clip(g, t, lx0 + c0 * PB, yy, 8 * PB, ck, fn, kind, c0, sel)
    g.vline(HW - g.px, ty, y1, t["hair"])
    # scroll bars: thin, no troughs
    sb_y = ty + 2
    g.box(lx1 + 2, sb_y, 4, (y1 - ty - 4) * 0.9, t["line"], None, rx=2)
    # the playhead
    ph = lx0 + (4 + 2 / 4 + 0.1 / 4) * PB if playing else lx0
    pc = t["ink1"] if playing else t["ink2"]
    g.path(f"M{ph - 4.5} {ry + 8} h9 l-4.5 5 z", fill=pc)
    g.vline(ph, ry + 12, y1, pc, op=0.9 if playing else 0.7)


def header(g, t, i, k, name, icon, ck, y, sel, kind, playing):
    c = USER[ck]
    if sel:
        g.rect(0, y, HW, ROW, t["raised"])
    g.rect(0, y + 1, 3, ROW - 2, hue(t, c, "stripe", sel))
    g.hline(0, HW, y + ROW - g.px, t["hair"])
    g.text(19, y + 20, str(i + 1), 9.5, t["ink3"], 400, fam=MONO, anchor="end")
    track_icon(g, t, icon, 25, y + 10, t["ink2"] if sel else t["ink3"])
    g.text(44, y + 20.5, name, 12.5, t["ink1"], 500 if sel else 400)
    if kind == "audio":  # frozen: a small six-point star after the name
        fx = 44 + width(name, 12.5, 400) + 9
        for a in (0, 60, 120):
            g.raw(f'<path d="M{fx} {y + 11.5} v8" transform="rotate({a} {fx} {y + 15.5})" stroke="{t["accent"]}" stroke-width="1" stroke-linecap="round"/>')
    # M, S, arm: letters at rest, no caps — they light only when on
    for j, L in enumerate("MS"):
        kx = 116 + j * 16
        g.text(kx + 6, y + 20, L, 10, t["ink3"], 600, anchor="middle")
    g.circle(150, y + 16.5, 2.8, t["ink3"])
    # the header's level: a 2 px meter on the right edge
    g.rect(HW - 6, y + 5, 2, ROW - 10, t["well"])
    if playing:
        lv = {"kick": .86, "clap": .7, "hats": .55, "perc": .45, "bass": .8, "chords": .66,
              "pluck": .6, "lead": .72, "choir": .5}[k]
        h = (ROW - 10) * lv
        g.rect(HW - 6, y + ROW - 5 - h, 2, h, t["m_lo"])


def clip(g, t, x, y, w, ck, fn, kind, bar0, sel):
    c = USER[ck]
    cx, cy, cw, ch = x + 1, y + 2, w - 2, ROW - 4
    body, edge, note = hue(t, c, "body", sel), hue(t, c, "edge", sel), hue(t, c, "note", sel)
    g.box(cx, cy, cw, ch, body, None, rx=3)
    g.rect(cx + 2, cy, cw - 4, 1, edge)
    for b in range(1, 8):
        g.vline(cx + b * PB - 1, cy + 3, cy + ch - 2, mix(body, edge, 0.18))
    # no name: the ruler says "Drop" once; a clip is named only when it says more
    top, bot = cy + 5, cy + ch - 4
    beat0 = bar0 * 4
    pbeat = PB / 4
    evs = [e for e in fn() if beat0 <= e[0] < beat0 + 32] if kind != "audio" else []
    if kind == "drum":
        pads = sorted({e[2] for e in evs}, reverse=True)
        n = len(pads)
        lane = (bot - top) / n
        for at, ln, p, v in evs:
            r = pads.index(p)
            hh = max(3.0, (lane - 1) * (v / 127) ** 1.1)
            xb = x + (at - beat0) * pbeat + 1.5
            g.rect(xb, top + r * lane + (lane - 1 - hh), 1.6 if n > 1 else 2, hh, note, snap=False)
    elif kind == "keys":
        ps = [e[2] for e in fn()]
        lo, hi = min(ps), max(ps)
        span = max(1, hi - lo)
        nh = 2.0 if ck in ("chords", "choir") else 2.5
        for at, ln, p, v in evs:
            xb = x + (at - beat0) * pbeat + 1.5
            wb = max(2.5, ln * pbeat - 0.8)
            yb = bot - nh - (p - lo) / span * (bot - top - nh)
            g.rect(xb, yb, wb, nh, note, snap=False, extra='rx="1"')
    else:
        n = int(cw - 4)
        env = song.choir_wave(int(16 * PB))
        mid = (top + bot) / 2 + 0.5
        amp = (bot - top) / 2 * 0.9
        up, rup = [], []
        for i in range(n):
            pk, rm = env[int(bar0 * PB) + i]
            xx = cx + 2 + i
            up.append((xx, pk))
            rup.append((xx, rm))
        for data, col, op in ((up, mix(note, body, 0.35), 1), (rup, note, 1)):
            d = "M" + " L".join(f"{xx:.1f} {mid - a * amp:.2f}" for xx, a in data)
            d += " L" + " L".join(f"{xx:.1f} {mid + a * amp:.2f}" for xx, a in reversed(data)) + " Z"
            g.path(d, fill=col, op=op)
