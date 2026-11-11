"""The inspector: the selected strip (Chords) in label/value units. Section heads are
sentence-case words on a hairline, not capital bars; empty slots are one 'Add' row; what
is seldom changed folds to one line that still says its value."""
import math
from common import SANS, MONO, mix
from metrics import width
from tokens import USER, hue
from arr import tab_row


def head(g, t, x, x1, y, label, right=None):
    g.text(x, y, label, 11.5, t["ink2"], 500)
    if right:
        g.text(x1, y, right, 10.5, t["ink3"], 400, anchor="end")


def eq_graph(g, t, x, y, w, h, c):
    g.box(x, y, w, h, t["well"], t["hair"], rx=4)
    lo, hi = math.log10(20), math.log10(20000)
    def fx(f):
        return x + (math.log10(f) - lo) / (hi - lo) * w
    def gy(db):
        return y + h / 2 - db / 18 * (h / 2 - 6)
    for f in (100, 1000, 10000):
        g.vline(fx(f), y + 1, y + h - 1, t["grid_bar"])
    for f in (50, 200, 500, 2000, 5000):
        g.vline(fx(f), y + 1, y + h - 1, t["grid_beat"])
    for db in (12, -12):
        g.hline(x + 1, x + w - 1, gy(db), t["grid_beat"])
    g.hline(x + 1, x + w - 1, gy(0), t["grid_4"])
    pts = []
    for i in range(int(w) + 1):
        f = 10 ** (lo + (hi - lo) * i / w)
        db = -10 * math.log10(1 + (220 / f) ** 8)
        pts.append((x + i, min(gy(-17), gy(db))))
    line = "M" + " L".join(f"{a:.1f} {b:.2f}" for a, b in pts)
    note = hue(t, c, "note", True)
    g.path(line + f" L{x + w} {gy(0)} L{x} {gy(0)} Z", fill=note, op=0.2 if t["name"] == "Night" else 0.14)
    g.path(line, stroke=note, sw=1.5)
    nx, ny = fx(220), gy(-3)
    g.circle(nx, ny, 4.5, t["well"], note, 1.5)
    for f, lab in ((100, "100"), (1000, "1k"), (10000, "10k")):
        g.text(fx(f) + 3, y + h - 5, lab, 8.5, t["ink3"], 400)
    g.text(x + w - 5, gy(12) + 3, "+12", 8.5, t["ink3"], 400, anchor="end")
    g.text(x + w - 5, gy(-12) + 3, "−12", 8.5, t["ink3"], 400, anchor="end")


def param(g, t, x, x1, y, label, value, frac, centre=None):
    g.text(x, y, label, 11, t["ink3"], 400)
    g.text(x1, y, value, 12, t["ink1"], 400, fam=MONO, anchor="end")
    ty = y + 9
    g.rect(x, ty, x1 - x, 2, t["track"])
    base = x + (x1 - x) * (centre if centre is not None else 0)
    v = x + (x1 - x) * frac
    g.rect(min(base, v), ty, abs(v - base), 2, t["ink2"])
    g.box(v - 1.5, ty - 4, 3, 10, t["ink1"], None, rx=1)


def inspector(g, t, x0, x1, y0, y1):
    g.rect(x0, y0, x1 - x0, y1 - y0, t["base"])
    g.vline(x0, y0, y1, t["hair"])
    tab_row(g, t, x0 + 1, x1, y0, ["Inspector", "Browser"])
    ix, ir = x0 + 13, x1 - 13
    c = USER["chords"]
    y = y0 + 28
    g.rect(x0 + 1, y, 3, 48, c)
    g.text(ix, y + 24, "Chords", 17, t["ink1"], 500)
    g.text(ix, y + 40, "Strip 6 · Calor, Unison saw", 11, t["ink3"], 400)
    g.hline(x0 + 1, x1, y + 50, t["hair"])

    y += 70
    head(g, t, ix, ir, y, "Inserts", "2 of 10")
    rows = [("1", "Comprimo", "keyed by Kick"), ("2", "Aequalis", "low cut 220 Hz")]
    for i, (n, nm, det) in enumerate(rows):
        ry = y + 10 + i * 24
        if i == 1:
            g.box(x0 + 5, ry, x1 - x0 - 10, 23, t["raised"], None, rx=3)
        g.text(ix + 1, ry + 16, n, 10, t["ink3"], 400, fam=MONO)
        g.text(ix + 14, ry + 16, nm, 12, t["ink1"], 500 if i == 1 else 400)
        g.text(ir - 14, ry + 16, det, 10.5, t["ink3"], 400, anchor="end")
        g.circle(ir - 3, ry + 12, 2.2, t["accent"])
    ay = y + 10 + 2 * 24
    g.path(f"M{ix + 1} {ay + 12} h7 M{ix + 4.5} {ay + 8.5} v7", stroke=t["ink3"], sw=1.1, cap="butt")
    g.text(ix + 14, ay + 16, "Add an effect", 11.5, t["ink3"], 400)

    y = ay + 40
    g.hline(x0 + 13, x1 - 13, y - 14, t["hair"])
    head(g, t, ix, ir, y, "Equaliser", "Aequalis")
    eq_graph(g, t, ix, y + 9, ir - ix, 80, c)
    y += 9 + 80 + 18
    g.text(ix, y, "Low cut", 11, t["ink3"], 400)
    g.text(ix + 62, y, "220 Hz", 12, t["ink1"], 400, fam=MONO)
    g.text(ir, y, "24 dB/oct", 12, t["ink1"], 400, fam=MONO, anchor="end")

    y += 28
    g.hline(x0 + 13, x1 - 13, y - 14, t["hair"])
    head(g, t, ix, ir, y, "Sends")
    sy = y + 22
    g.text(ix, sy, "Reverb", 12, t["ink1"], 400)
    g.text(ix + 46, sy, "post", 9.5, t["ink3"], 400)
    g.rect(ix + 72, sy - 4, 84, 2, t["track"])
    g.rect(ix + 72, sy - 4, 84 * 0.46, 2, hue(t, c, "note", True))
    g.text(ir, sy, "−16.0 dB", 12, t["ink1"], 400, fam=MONO, anchor="end")
    g.path(f"M{ix + 1} {sy + 20} h7 M{ix + 4.5} {sy + 16.5} v7", stroke=t["ink3"], sw=1.1, cap="butt")
    g.text(ix + 14, sy + 24, "Add a send", 11.5, t["ink3"], 400)

    y = sy + 50
    for lab, val in (("Sidechain", "Kick → Comprimo"), ("Routing", "Master"), ("Recording", "Not armed")):
        g.hline(x0 + 13, x1 - 13, y - 16, t["hair"])
        g.text(ix, y, lab, 11.5, t["ink2"], 500)
        g.text(ir - 12, y, val, 11, t["ink2"] if lab != "Recording" else t["ink3"], 400, anchor="end")
        g.path(f"M{ir - 4} {y - 7.5} l3 3 l-3 3", stroke=t["ink3"], sw=1.1)
        y += 26

    by = y1 - 128
    g.rect(x0 + 1, by, x1 - x0 - 1, y1 - by, t["bar"])
    g.hline(x0 + 1, x1, by, t["hair"])
    param(g, t, ix, ir, by + 24, "Volume", "−3.5 dB", 0.74, centre=0.0)
    param(g, t, ix, ir, by + 62, "Pan", "C", 0.5, centre=0.5)
    param(g, t, ix, ir, by + 100, "Width", "100 %", 0.5, centre=0.0)
