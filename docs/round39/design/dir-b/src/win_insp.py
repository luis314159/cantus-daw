"""The inspector (right) and the status line (bottom)."""
import math
from core import mix, HUES, UI, NARROW, CLOCK
from kit import key, knob, field, label, segmented
from glyphs2 import glyph
from metrics import width

X, Y0, Wd = 1040, 50, 240


def section(c, y, title, right=None):
    """A section: a hairline across, then its title in sentence case — no capital bar."""
    t = c.t
    c.hline(X + 12, X + Wd - 12, y, t["line_soft"] if t["name"] == "Night" else t["line"])
    c.text(X + 14, y + 17, title, size=11.5, fill=t["ink2"], wt=650)
    if right:
        c.text(X + Wd - 14, y + 17, right, size=10, fill=t["ink3"], wt=500, anchor="end", tnum=True)


def inspector(c, playing=False):
    t = c.t
    night = t["name"] == "Night"
    c.rect(X, Y0, Wd, 698 - Y0, t["panel"])
    c.vline(X, Y0, 698, t["key_edge"] if night else t["line"])
    # header: whose it is
    c.rect(X + 14, Y0 + 11, 9, 9, HUES["chords"], rx=2)
    c.text(X + 29, Y0 + 20, "Chords", size=14, fill=t["ink1"], wt=650)
    c.text(X + 82, Y0 + 20, "insert 6 · Chorda", size=10.5, fill=t["ink3"], wt=500)
    glyph(c, "chev_right", X + Wd - 16, Y0 + 15.5, 7, ink1=t["ink3"], ink2=t["ink3"])
    # inserts: only what is there, then one way to add
    y = Y0 + 34
    section(c, y, "Inserts", "2 of 10")
    y += 26
    for i, (n, kind, mixv) in enumerate((("Comprimo", "compressor", 1.0), ("Aequalis", "equaliser", 1.0))):
        ry = y + i * 26
        c.rect(X + 10, ry, Wd - 20, 23, t["raised"] if night else t["well"], rx=3,
               stroke=t["line_soft"] if night else t["line"])
        glyph(c, "grip", X + 18, ry + 11.5, 8, ink1=t["ink4"], ink2=t["ink4"])
        c.text(X + 28, ry + 15.5, n, size=12, fill=t["ink1"], wt=600)
        c.text(X + 28 + width(n, 12, wt=600) + 6, ry + 15.5, kind, size=10, fill=t["ink3"], wt=500)
        c.circle(X + Wd - 22, ry + 11.5, 3.2, fill=t["teal"])
        c.circle(X + Wd - 22, ry + 11.5, 5.5, stroke=t["teal"], sw=1, op=0.30)
    ry = y + 52
    glyph(c, "plus", X + 18, ry + 11, 7, ink1=t["ink3"], ink2=t["ink3"])
    c.text(X + 28, ry + 15, "Add an effect", size=11, fill=t["ink3"], wt=500)
    c.text(X + Wd - 14, ry + 15, "8 free", size=10, fill=t["ink4"] if night else t["ink3"],
           wt=500, anchor="end")
    # the EQ, drawn
    y = ry + 30
    section(c, y, "Equaliser", "Aequalis")
    eq(c, X + 12, y + 26, Wd - 24, 70)
    y += 104
    for i, (nm, f, g) in enumerate((("Low shelf", "100 Hz", "+2.0"), ("Bell", "1.0 kHz", "−1.5"),
                                    ("High shelf", "8.0 kHz", "+1.5"))):
        bx = X + 14 + i * 74
        sel = i == 1
        c.text(bx, y + 6, f, size=9.5, fill=t["gold"] if sel else t["ink3"], wt=600, tnum=True)
        c.text(bx, y + 21, g, size=12.5, fill=t["ink1"], wt=600, tnum=True)
        c.text(bx + width(g, 12.5, wt=600, tnum=True) + 2.5, y + 21, "dB", size=8.5, fill=t["ink3"],
               wt=600)
    # sends
    y += 30
    section(c, y, "Sends")
    for i, (n, db, v) in enumerate((("Reverb", "−16.0", 0.42), ("Delay", "−22.0", 0.30))):
        sy = y + 30 + i * 24
        c.text(X + 14, sy + 4, n, size=11.5, fill=t["ink1"], wt=550)
        slider(c, X + 70, sy, 104, v)
        c.text(X + Wd - 14, sy + 4.5, db, size=11, fill=t["ink2"], wt=600, anchor="end", tnum=True)
    # in and out
    y += 82
    section(c, y, "Routing")
    c.text(X + 14, y + 37, "Input", size=11, fill=t["ink3"], wt=500)
    field(c, X + 70, y + 24, Wd - 84, 22, "Chorda", size=11.5, drop=True, tnum=False)
    c.text(X + 14, y + 63, "Output", size=11, fill=t["ink3"], wt=500)
    field(c, X + 70, y + 50, Wd - 84, 22, "Master", size=11.5, drop=True, tnum=False)
    # the strip itself, large: the volume as the panel's one big number
    y = 562
    c.rect(X, y, Wd, 698 - y, t["chrome"] if night else t["panel"])
    c.hline(X, X + Wd, y, t["line"])
    label(c, X + 14, y + 17, "Volume", size=10.5, fill=t["ink3"], wt=600)
    c.text(X + 14, y + 46, "−3.5", size=30, fam=CLOCK, wt=250, fill=t["ink1"], tnum=True)
    c.text(X + 60, y + 46, "dB", size=11, fill=t["ink3"], wt=600)
    for i, (lab, v, val, ctr) in enumerate((("Pan", 0.5, "C", True), ("Width", 0.75, "100 %", False))):
        kx = X + 116 + i * 62
        knob(c, kx, y + 32, 13, v, centre=ctr)
        c.text(kx, y + 60, lab, size=10, fill=t["ink3"], wt=500, anchor="middle")
        c.text(kx, y + 13, val, size=10, fill=t["ink2"], wt=600, anchor="middle", tnum=True)
    ky = y + 78
    for i, (lab, g) in enumerate((("Mute", None), ("Solo", None), ("Mono", None), (None, "phase"),
                                  (None, "swap"))):
        kw = 44 if lab else 30
        kx = X + 12 + [0, 46, 92, 138, 170][i]
        key(c, kx, ky, kw, 24, g=g, gsize=11, label=lab, lsize=10.5)
        if lab:
            pass
    c.text(X + 14, ky + 48, "", size=10)


def slider(c, x, y, w, v):
    t = c.t
    night = t["name"] == "Night"
    c.rect(x, y - 1.5, w, 3, t["well"] if night else t["key"], rx=1.5)
    c.rect(x, y - 1.5, w * v, 3, t["ink3"], rx=1.5)
    tx = x + w * v
    c.rect(tx - 3.5, y - 6, 7, 12, t["raised"] if night else "#FFFFFF", rx=2, stroke=t["key_edge"])
    c.rect(tx - 0.5, y - 3, 1, 6, t["ink2"])


def eq(c, x, y, w, h):
    t = c.t
    night = t["name"] == "Night"
    c.rect(x, y, w, h, t["well"], rx=3, stroke=t["key_edge"])
    def fx(f):
        return x + w * (math.log10(f) - math.log10(20)) / 3
    def gy(db):
        return y + h / 2 - db * h / 30
    for f, s in ((100, "100"), (1000, "1k"), (10000, "10k")):
        c.vline(fx(f), y + 1, y + h - 1, t["line_soft"] if night else t["line"])
        c.text(fx(f) + 3, y + h - 4, s, size=8, fill=t["ink4"] if night else t["ink3"], wt=600)
    c.hline(x + 1, x + w - 1, gy(0), t["line"])
    pts = []
    for i in range(121):
        f = 20 * 10 ** (3 * i / 120)
        lf = math.log2(f)
        g = 2.0 / (1 + math.exp((lf - math.log2(100)) * 2.2))
        g += -1.5 * math.exp(-((lf - math.log2(1000)) ** 2) / 0.9)
        g += 1.5 / (1 + math.exp(-(lf - math.log2(8000)) * 2.2))
        pts.append((fx(f), gy(g * 2.2)))
    s = c.s
    d = "M" + " L".join("%.2f %.2f" % (px * s, py * s) for px, py in pts)
    c.add('<path d="%s L%.2f %.2f L%.2f %.2f Z" fill="%s" fill-opacity="0.10"/>' % (
        d, (x + w - 1) * s, gy(0) * s, (x + 1) * s, gy(0) * s, t["ink2"]))
    c.add('<path d="%s" fill="none" stroke="%s" stroke-width="%.2f"/>' % (d, t["ink1"], 1.5 * s))
    for f, g, sel in ((100, 2.2 * 2.0 * 0.5 + 2.2, False), (1000, -1.5 * 2.2, True), (8000, 1.5 * 2.2 / 2, False)):
        cx, cy = fx(f), gy(g if f != 100 else 2.2)
        c.circle(cx, cy, 4, fill=t["gold"] if sel else t["well"], stroke=t["gold"] if sel else t["ink1"],
                 sw=1.2)


def status(c, hint=None):
    t = c.t
    night = t["name"] == "Night"
    y = 698
    c.rect(0, y, 1280, 22, t["chrome"])
    c.hline(0, 1280, y, t["key_edge"] if night else t["line"])
    hint = hint or ("Chords · volume", "drag to set · Ctrl for fine · double-click for 0 dB")
    c.text(12, y + 15, hint[0], size=11, fill=t["ink1"], wt=600)
    c.text(12 + width(hint[0], 11, wt=600) + 7, y + 15, "—  " + hint[1], size=11, fill=t["ink3"], wt=500)
    x = 1268
    for s, dot in (("Realtek · 48 kHz · 5.3 ms", t["teal"]), ("MIDI: Keystation 49", t["teal"]),
                   ("Typing keyboard off  Ctrl+T", None)):
        c.text(x, y + 15, s, size=10.5, fill=t["ink3"] if dot is None else t["ink2"], wt=500,
               anchor="end", tnum=True)
        wdt = width(s, 10.5, wt=500, tnum=True)
        if dot:
            c.circle(x - wdt - 8, y + 11, 3, fill=dot)
            x -= wdt + 26
        else:
            x -= wdt + 18
