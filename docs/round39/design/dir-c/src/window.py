"""The whole window, 1280x720 logical: top bar, arrangement, mixer, inspector, status.
python -I window.py renders Night and Paper at 1x and 1.5x (and the overflow and the
playing variants)."""
import os
import sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import Svg, page, render, write, OUT, MONO
from tokens import NIGHT, PAPER
from top import top_bar, TOP_H
from arr import arrangement
from mixer import mixer
from insp import inspector
from metrics import width

W, H = 1280, 720
AW = 1036
ARR_B = 396
STATUS = 22


def status(g, t, playing):
    y = H - STATUS
    g.rect(0, y, W, STATUS, t["bar"])
    g.hline(0, W, y, t["hair"])
    hint = "Fader · Chords · −3.5 dB" if not playing else "Playing · loop 1–17"
    g.text(14, y + 15, hint, 11, t["ink1"], 500)
    g.text(14 + width(hint, 11, 500) + 10, y + 15,
           "drag to set · double-click to type a value · Alt-click for 0 dB", 11, t["ink3"], 400)
    r = "Realtek(R) Audio · 48 kHz · 256 samples · 5.3 ms"
    rx = W - 14
    g.text(rx, y + 15, r, 11, t["ink3"], 400, anchor="end")
    g.circle(rx - width(r, 11) - 9, y + 11, 3, t["play"])
    sx = rx - width(r, 11) - 24
    g.text(sx, y + 15, "Saved", 11, t["ink3"], 400, anchor="end")


def window(t, s=1.0, overflow=False, playing=False):
    g = Svg(W, H, s)
    g.rect(0, 0, W, H, t["base"])
    arrangement(g, t, AW, TOP_H, ARR_B, playing)
    g.hline(0, AW, ARR_B, t["hair"])
    mixer(g, t, 0, AW, ARR_B + 1, H - STATUS, playing)
    inspector(g, t, AW, W, TOP_H, H - STATUS)
    top_bar(g, t, W, folded=3, more_state="pressed" if overflow else "rest", playing=playing)
    status(g, t, playing)
    if overflow:
        from overflow import overflow_panel
        overflow_panel(g, t, W, playing)
    return g


def shoot(name, t, s, **kw):
    g = window(t, s, **kw)
    html = os.path.join(OUT, "html", name + ".html")
    write(html, page(g.svg(), t["base"], W, H))
    png = os.path.join(OUT, name + ".png")
    render(html, png, W, H, s)
    return png


if __name__ == "__main__":
    which = sys.argv[1:] or ["all"]
    jobs = []
    for t in (NIGHT, PAPER):
        n = t["name"].lower()
        jobs += [(f"window-{n}-100", t, 1.0, {}), (f"window-{n}-150", t, 1.5, {}),
                 (f"window-{n}-overflow-100", t, 1.0, dict(overflow=True)),
                 (f"window-{n}-overflow-150", t, 1.5, dict(overflow=True)),
                 (f"window-{n}-playing-100", t, 1.0, dict(playing=True))]
    for name, t, s, kw in jobs:
        if "all" in which or any(w in name for w in which):
            print(shoot(name, t, s, **kw))
