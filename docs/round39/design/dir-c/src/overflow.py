"""The top bar's overflow panel, designed again from a blank sheet (BACKLOG §6j): one
floating sheet, no boxes inside it; three groups under sentence-case heads; settings in a
label/value grid on one left edge; the window keys in one row with each shortcut under
its own key; the system meters as small live graphs with their numbers; customising as a
quiet last line."""
import math
from common import MONO, mix
from glyphs import glyph
from metrics import width
from top import caption, key

PW = 324


def seg(g, t, x, y, labels, on, w=None):
    """A segmented control: one well, the chosen segment raised and inked."""
    ws = [w or (width(l, 11, 500) + 18) for l in labels]
    tot = sum(ws) + 4
    g.box(x, y, tot, 22, t["well"], t["hair"], rx=5)
    xx = x + 2
    for i, (l, ww) in enumerate(zip(labels, ws)):
        if i == on:
            g.box(xx, y + 2, ww, 18, t["field"] if t["name"] == "Night" else "#FFFFFF",
                  None if t["name"] == "Night" else t["line"], rx=3.5)
        g.text(xx + ww / 2, y + 15, l, 11, t["ink1"] if i == on else t["ink3"], 500, anchor="middle")
        xx += ww
    return tot


def toggle(g, t, x, y, on):
    w, h = 26, 14
    g.box(x, y, w, h, t["accent"] if on else t["well"], None if on else t["line"], rx=7)
    kx = x + (w - 7 if on else 7)
    g.circle(kx, y + 7, 5, "#FFFFFF" if on else t["ink3"])


def spark(g, t, x, y, w, h, vals, col):
    n = len(vals)
    pts = [(x + i * w / (n - 1), y + h - v * h) for i, v in enumerate(vals)]
    d = "M" + " L".join(f"{a:.2f} {b:.2f}" for a, b in pts)
    gid = g.uid("sp")
    g.defs.append(f'<linearGradient id="{gid}" x1="0" y1="{y}" x2="0" y2="{y + h}" gradientUnits="userSpaceOnUse">'
                  f'<stop offset="0" stop-color="{col}" stop-opacity="0.32"/>'
                  f'<stop offset="1" stop-color="{col}" stop-opacity="0.02"/></linearGradient>')
    g.path(d + f" L{x + w} {y + h} L{x} {y + h} Z", fill=f"url(#{gid})")
    g.path(d, stroke=col, sw=1.25)
    g.circle(pts[-1][0], pts[-1][1], 2, col)


def cell(g, t, x, y, w, label, value, unit, vals, col, note=None):
    caption(g, t, x, y + 9, label)
    vw = width(value, 17, 300)
    g.text(x + w - (width(unit, 10.5) + 3 if unit else 0), y + 12, value, 17, t["ink1"], 300, anchor="end")
    if unit:
        g.text(x + w, y + 12, unit, 10.5, t["ink3"], 400, anchor="end")
    g.hline(x, x + w, y + 44, t["hair"])
    if vals:
        spark(g, t, x, y + 20, w, 24, vals, col)
    elif note:
        g.text(x, y + 38, note, 10.5, t["ink3"], 400)


def overflow_panel(g, t, W, playing=False):
    x0 = W - 12 - PW
    y0 = 46
    ph = 456
    fid = g.uid("sh")
    g.defs.append(f'<filter id="{fid}" x="-20%" y="-20%" width="140%" height="140%">'
                  f'<feDropShadow dx="0" dy="10" stdDeviation="14" flood-color="{t["shadow"]}" flood-opacity="1"/></filter>')
    g.raw(f'<rect x="{x0}" y="{y0}" width="{PW}" height="{ph}" rx="8" fill="{t["float"]}" filter="url(#{fid})"/>')
    g.box(x0, y0, PW, ph, None, t["line"], rx=8)
    ix, ir = x0 + 18, x0 + PW - 18
    lab = ix          # one left edge for every label
    val = ix + 76     # one left edge for every value

    # ---- Metronome
    y = y0 + 18
    key(g, t, ix - 4, y - 2, 30, 26, "metronome", "on", gk=20)
    g.text(ix + 34, y + 15, "Metronome", 13, t["ink1"], 500)
    g.text(ir, y + 15, "Ctrl+M", 10, t["ink3"], 400, fam=MONO, anchor="end")
    y += 40
    rows = [("Count-in", "seg", (["Off", "1 bar", "2 bars"], 1)),
            ("Level", "level", None),
            ("Sound", "seg", (["Classic", "Wood", "Beep"], 1)),
            ("Accent", "accent", None)]
    for name, kind, arg in rows:
        g.text(lab, y + 15, name, 11.5, t["ink2"], 400)
        if kind == "seg":
            seg(g, t, val, y, arg[0], arg[1], w=(ir - val - 4) / 3)
        elif kind == "level":
            sw = ir - val - 48
            g.rect(val, y + 10, sw, 2, t["track"])
            g.rect(val, y + 10, sw * 0.72, 2, t["ink2"])
            g.box(val + sw * 0.72 - 1.5, y + 5, 3, 12, t["ink1"], None, rx=1)
            g.text(ir, y + 15, "−6 dB", 11.5, t["ink1"], 400, fam=MONO, anchor="end")
        else:
            toggle(g, t, val, y + 4, True)
            g.text(val + 34, y + 15, "the first beat of each bar", 11, t["ink3"], 400)
        y += 30

    # ---- Windows
    y += 4
    g.hline(x0 + 1, x0 + PW - 1, y, t["hair"])
    y += 22
    g.text(lab, y, "Windows", 11.5, t["ink2"], 500)
    g.text(ir, y, "lit = open", 10, t["ink3"], 400, anchor="end")
    y += 10
    wins = [("browser", "Alt+F8", False), ("arrange", "F5", True), ("rack", "F6", False),
            ("piano", "F7", False), ("mixer", "F9", True), ("instrument", "F8", False),
            ("inspector", "I", True)]
    kw = (ir - ix - 6 * 4) / 7
    for i, (n, sc, on) in enumerate(wins):
        kx = ix + i * (kw + 4)
        g.box(kx, y, kw, 30, t["accent_bg"] if on else t["well"], None if on else t["hair"], rx=5)
        glyph(g, n, kx + (kw - 24) / 2, y + 3, 24, t["accent"] if on else t["ink2"],
              mix(t["accent"], t["float"], 0.45) if on else (t["ink1"] if t["name"] == "Night" else t["ink3"]))
        g.text(kx + kw / 2, y + 44, sc, 9.5, t["ink2"] if on else t["ink3"], 400, fam=MONO, anchor="middle")

    # ---- Performance
    y += 62
    g.hline(x0 + 1, x0 + PW - 1, y, t["hair"])
    y += 22
    g.text(lab, y, "Performance", 11.5, t["ink2"], 500)
    g.text(ir, y, "the last minute", 10, t["ink3"], 400, anchor="end")
    y += 12
    cw = (ir - ix - 20) / 2
    cpu = [0.04 + 0.02 * math.sin(i * 1.7) for i in range(14)] + \
          [0.18 + 0.05 * math.sin(i * 2.3) + 0.03 * math.cos(i * 5.1) for i in range(20)] + \
          [0.03 + 0.015 * math.sin(i * 1.3) for i in range(10)]
    if playing:
        cpu = cpu[10:] + [0.17 + 0.04 * math.sin(i * 2.9) for i in range(10)]
    ram = [0.55 + 0.002 * i for i in range(44)]
    col = t["ink2"] if t["name"] == "Night" else t["ink2"]
    cell(g, t, ix, y, cw, "CPU", "16" if playing else "3", "%", cpu, col)
    cell(g, t, ix + cw + 20, y, cw, "Memory", "612", "MB", ram, col)
    y += 58
    vo = [0] * 12 + [0.3 + 0.1 * math.sin(i) for i in range(22)] + [0] * 10
    cell(g, t, ix, y, cw, "Voices", "23" if playing else "0", "", vo, col)
    cell(g, t, ix + cw + 20, y, cw, "Dropouts", "0", "", None, None, note="none since the audio started")

    # ---- customise: the last line, quiet
    fy = y0 + ph - 38
    g.hline(x0 + 1, x0 + PW - 1, fy, t["hair"])
    for k, (a, b) in enumerate(((0.3, 0.7), (0.65, 0.35))):
        yy = fy + 15 + k * 7
        g.hline(ix, ix + 14, yy, t["ink3"])
        g.circle(ix + 14 * a, yy + 0.5, 2.2, t["float"], t["ink2"], 1.2)
    g.text(ix + 24, fy + 23, "Customise the top bar…", 12, t["ink1"], 400)
    g.text(ir, fy + 23, "3 groups folded here", 10.5, t["ink3"], 400, anchor="end")
