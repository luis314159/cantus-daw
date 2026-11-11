"""The whole window, Night and Paper, 1x and 1.5x, at rest, playing, and with the top bar's
overflow panel open. Run: python window.py [quick]"""
import os
import sys
from core import Canvas, NIGHT, PAPER, page, render, mix, OUT_DOCS, CLOCK, NARROW
from win_top import topbar, toolbar
from win_arr import arrangement
from win_mix import mixer
from win_insp import inspector, status
from kit import key, segmented, field, label, group_frame
from kit2 import popover, toggle, spark
from glyphs2 import glyph
from metrics import width


def window(t, s, overflow=False, playing=False):
    c = Canvas(1280, 720, s, t)
    c.rect(0, 0, 1280, 720, t["panel"])
    toolbar(c)
    arrangement(c, playing)
    mixer(c, playing)
    inspector(c, playing)
    topbar(c, overflow_open=overflow, playing=playing)
    if overflow:
        status(c, ("More", "3 groups are folded here — the window is too narrow for them"))
        overflow_panel(c)
    else:
        status(c)
    return c


WINDOWS = [("win_browser", "Alt+F8", False), ("win_arr", "F5", True), ("win_rack", "F6", False),
           ("win_piano", "F7", False), ("win_mixer", "F9", True), ("win_session", "Tab", False),
           ("win_inspector", "I", True)]


def keycap(c, cx, y, s, anchor="middle"):
    """A shortcut drawn as a small keycap: a hairline outline, the key in ink-2."""
    t = c.t
    w = max(16, width(s, 9, wt=600, tnum=True) + 10)
    x = cx - w / 2 if anchor == "middle" else (cx - w if anchor == "end" else cx)
    c.rect(x, y, w, 14, "none", rx=3, stroke=t["line_strong"] if t["name"] == "Night" else t["ink4"])
    c.rect(x + 1, y + 12, w - 2, 1, t["line_strong"] if t["name"] == "Night" else t["ink4"], op=0.6)
    c.text(x + w / 2, y + 10.2, s, size=9, fill=t["ink2"], wt=600, anchor="middle", tnum=True)
    return w


def overflow_panel(c):
    t = c.t
    night = t["name"] == "Night"
    W, X, Y = 348, 1280 - 348 - 6, 46
    H = 404
    popover(c, X, Y, W, H)
    # the notch that ties it to the ⋯ key
    c.path("M%.1f %.1f L%.1f %.1f L%.1f %.1f" % (1247, Y + 0.5, 1253, Y - 5.5, 1259, Y + 0.5),
           fill=t["raised"] if night else "#FBFAF8", stroke=t["line_strong"] if night else t["line"],
           sw=1)
    c.rect(1247.5, Y, 11, 1.2, t["raised"] if night else "#FBFAF8")
    L, C = X + 16, X + 104          # the label column and the control column
    def head(y, title, sc=None, note=None):
        c.text(L, y, title, size=13, fill=t["ink1"], wt=650)
        if note:
            c.text(L + width(title, 13, wt=650) + 8, y, note, size=10.5, fill=t["ink3"], wt=500)
        if sc:
            keycap(c, X + W - 16, y - 10.5, sc, anchor="end")
    # -- metronome
    y = Y + 26
    head(y, "Metronome", "Ctrl+M")
    y += 14
    c.text(L, y + 17, "Click", size=11.5, fill=t["ink2"], wt=500)
    key(c, C, y + 2, 34, 24, g="metronome", gsize=12, state="on")
    field(c, C + 40, y + 3, 112, 22, "Woodblock", drop=True, size=11.5, tnum=False)
    toggle(c, C + 160, y + 7, True, w=24, h=13)
    c.text(C + 190, y + 17.5, "Accent", size=11, fill=t["ink2"], wt=500)
    y += 32
    c.text(L, y + 15, "Count-in", size=11.5, fill=t["ink2"], wt=500)
    segmented(c, C, y + 1, ["Off", "1 bar", "2 bars"], h=22, on=1, size=11, widths=[52, 64, 64])
    y += 32
    c.text(L, y + 12, "Level", size=11.5, fill=t["ink2"], wt=500)
    sx, sw = C, 168
    c.rect(sx, y + 6.5, sw, 3, t["well"] if night else t["key"], rx=1.5)
    c.rect(sx, y + 6.5, sw * 0.72, 3, t["ink2"], rx=1.5)
    for k in range(0, 5):
        c.vline(sx + sw * k / 4, y + 13, y + 16, t["ink4"])
    c.rect(sx + sw * 0.72 - 4, y + 2, 8, 12, t["raised"] if night else "#FFFFFF", rx=2, stroke=t["key_edge"])
    c.text(X + W - 16, y + 13, "−6.0 dB", size=11.5, fill=t["ink1"], wt=600, anchor="end", tnum=True)
    # -- windows: each key with its own shortcut under it; lit = open now
    y += 34
    c.hline(X + 12, X + W - 12, y, t["line_soft"] if night else t["line"])
    y += 24
    head(y, "Windows", note="lit when open")
    y += 12
    kw, gap = 40, 6
    for i, (g, sc, open_) in enumerate(WINDOWS):
        kx = L + i * (kw + gap)
        key(c, kx, y, kw, 32, g=g, gsize=14, state="on" if open_ else "rest")
        c.text(kx + kw / 2, y + 47, sc, size=10, fill=t["ink2"] if open_ else t["ink3"], wt=600,
               anchor="middle", tnum=True)
    # -- system: graphs with their numbers
    y += 64
    c.hline(X + 12, X + W - 12, y, t["line_soft"] if night else t["line"])
    y += 24
    head(y, "System", note="last 30 s")
    y += 12
    cpu = [0.05, 0.07, 0.06, 0.08, 0.12, 0.09, 0.07, 0.08, 0.10, 0.08, 0.07, 0.06, 0.09, 0.11,
           0.08, 0.07, 0.07, 0.06, 0.08, 0.07]
    ram = [0.40, 0.40, 0.41, 0.41, 0.41, 0.42, 0.42, 0.42, 0.42, 0.43, 0.43, 0.43, 0.43, 0.43,
           0.43, 0.44, 0.44, 0.44, 0.44, 0.44]
    for i, (lab, val, unit, vals, note) in enumerate((("CPU", "7", "%", cpu, "of one block"),
                                                      ("Memory", "412", "MB", ram, "working set"))):
        gx = L + i * 160
        c.text(gx, y + 12, lab, size=10.5, fill=t["ink3"], wt=600)
        c.text(gx, y + 40, val, size=26, fam=CLOCK, wt=300, fill=t["ink1"], tnum=True)
        c.text(gx + width(val, 26, fam=CLOCK, wt=300, tnum=True) + 6, y + 40, unit, size=10.5, fill=t["ink3"], wt=600)
        sv = [min(1, v * 3.2) for v in vals] if i == 0 else vals
        spark(c, gx + 62, y + 18, 84, 26, sv, warn=None)
    y += 56
    for i, (lab, val, note, dot) in enumerate((("Voices", "24", "peak 61", None),
                                               ("Dropouts", "0", "since audio started", t["teal"]))):
        gx = L + i * 160
        c.text(gx, y + 12, lab, size=11, fill=t["ink3"], wt=500)
        c.text(gx + 64, y + 12, val, size=13, fill=t["ink1"], wt=650, anchor="end", tnum=True)
        if dot:
            c.circle(gx + 71, y + 8, 2.8, fill=dot)
        c.text(gx + (78 if dot else 70), y + 12, note, size=10, fill=t["ink3"], wt=500)
    # -- customise: a quiet action at the foot, not a big grey button
    y += 26
    c.hline(X, X + W, y, t["line_soft"] if night else t["line"])
    c.rect(X + 1, y + 1, W - 2, H - (y - Y) - 2, mix(t["raised"], t["panel"], 0.5) if night else
           t["panel"], rx=4)
    glyph(c, "grip", L + 4, y + 18, 10, ink1=t["ink3"], ink2=t["ink3"])
    c.text(L + 16, y + 22, "Customise top bar…", size=12, fill=t["ink1"], wt=600)
    c.text(X + W - 16, y + 22, "or right-click the bar", size=10.5, fill=t["ink3"], wt=500,
           anchor="end")


def main(quick=False):
    out = os.path.join(OUT_DOCS)
    jobs = [("window-night-1x", NIGHT, 1, {}), ("window-paper-1x", PAPER, 1, {}),
            ("overflow-night-1x", NIGHT, 1, {"overflow": True}),
            ("overflow-paper-1x", PAPER, 1, {"overflow": True})]
    if not quick:
        jobs += [("window-night-1.5x", NIGHT, 1.5, {}), ("window-paper-1.5x", PAPER, 1.5, {}),
                 ("overflow-night-1.5x", NIGHT, 1.5, {"overflow": True}),
                 ("overflow-paper-1.5x", PAPER, 1.5, {"overflow": True}),
                 ("window-night-playing-1x", NIGHT, 1, {"playing": True}),
                 ("window-paper-playing-1x", PAPER, 1, {"playing": True})]
    only = [a for a in sys.argv[1:] if a != "quick"]
    for name, t, s, kw in jobs:
        if only and not any(o in name for o in only):
            continue
        c = window(t, s, **kw)
        W, H = round(1280 * s), round(720 * s)
        render(name, page(c.svg(), t["panel"]), W, H, os.path.join(out, name + ".png"))
        print(name)


if __name__ == "__main__":
    main(quick="quick" in sys.argv)
