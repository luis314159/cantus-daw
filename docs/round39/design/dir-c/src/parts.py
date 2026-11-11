"""The parts sheet: every control at 100 % and 150 % (each size drawn for its own pixels),
both themes. Rendered at 1:1; the 4x sheet is the 1:1 render magnified pixel for pixel,
so what is judged is exactly what would ship."""
import os
import sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import Svg, page, render, write, OUT, MONO, mix
from tokens import NIGHT, PAPER, USER
from glyphs import glyph
from top import key, caption
from mixer import knob, fader, meter_v, strip, STRIPS
from arr import tab_row
from overflow import seg, toggle
from metrics import width
from crop import crop

SW, SH = 1240, 1250


def embed(g, x, y, s, w, h, fn):
    """Draw fn(sub) in a sub-sheet snapped for scale s, placed at (x, y) on a 1:1 sheet."""
    sub = Svg(w, h, s)
    sub._id = g._id + 1000
    fn(sub)
    g._id = sub._id + 1
    g.defs += sub.defs
    g.raw(f'<g transform="translate({x} {y}) scale({s})">' + "".join(sub.parts) + "</g>")


def tag(g, t, x, y, s):
    g.text(x, y, f"{round(s * 100)} %", 10, t["ink3"], 500, ls=0.3)


def label(g, t, x, y, s, sub=None):
    g.text(x, y, s, 13, t["ink1"], 500)
    if sub:
        g.text(x + width(s, 13, 500) + 10, y, sub, 11, t["ink3"], 400)


TRANSPORT = ["to_start", "play", "stop", "record", "loop", "metronome"]
TOOLS = ["select", "draw", "slice", "erase", "snap", "undo", "redo", "more"]
WINDOWS = ["browser", "arrange", "rack", "piano", "mixer", "instrument", "inspector"]


def keys_block(g, t, y0):
    label(g, t, 24, y0, "Keys", "each size drawn for its own pixels, never one drawing scaled · rest, then on")
    names = TRANSPORT + TOOLS + WINDOWS
    rows = [(16, 1, "16 px"), (20, 1, "20 px"), (24, 1, "24 px"),
            (16, 1.5, "16 px at 150 %"), (20, 1.5, "20 px at 150 %"), (24, 1.5, "24 px at 150 %")]
    y = y0 + 22
    for k, s, lab in rows:
        P = k * s
        g.text(24, y + P / 2 + 4, lab, 11, t["ink3"], 400)
        x = 128
        def draw_row(sub, k=k):
            xx = 0
            for i, n in enumerate(names):
                if n == "record":
                    sub.box(xx, 0, k, k, t["well"], t["hair"], rx=3 if k < 20 else 4)
                    glyph(sub, n, xx, 0, k, t["rec"], mix(t["rec"], "#000000", 0.35))
                else:
                    sub.box(xx, 0, k, k, t["well"], t["hair"], rx=3 if k < 20 else 4)
                    glyph(sub, n, xx, 0, k, t["ink2"], t["ink1"] if t["name"] == "Night" else t["ink3"])
                xx += k + 6 if i not in (5, 13) else k + 16
            for n in ("loop", "snap", "metronome"):
                sub.box(xx, 0, k, k, t["accent_bg"], None, rx=3 if k < 20 else 4)
                glyph(sub, n, xx, 0, k, t["accent"], mix(t["accent"], t["bar"], 0.4))
                xx += k + 6
        embed(g, x, y, s, 1100, k + 2, draw_row)
        y += P + 14
    return y


def knobs_block(g, t, x0, y0):
    label(g, t, x0, y0, "Knob", "rest · hover · dragged · automated")
    for j, s in enumerate((1, 1.5)):
        def fn(sub):
            for i, (st, v, txt) in enumerate([("rest", -0.36, "L18"), ("hover", -0.36, "L18"),
                                               ("drag", 0.4, "R20"), ("auto", 0.24, "R12")]):
                cx = 22 + i * 46
                if st == "drag":
                    sub.box(cx - 19, 46, 38, 15, t["field"], None, rx=3)
                knob(sub, t, cx, 22, 9.5, v, st)
                sub.text(cx, 57, txt, 9.5, t["ink1"] if st == "drag" else t["accent"] if st == "auto" else t["ink3"],
                         400, fam=MONO, anchor="middle")
        tag(g, t, x0 + j * 200, y0 + 24, s)
        embed(g, x0 + j * 200, y0 + 30, s, 190, 64, fn)


def faders_block(g, t, x0, y0):
    label(g, t, x0, y0, "Fader and meter", "silent · playing · clipping")
    for j, s in enumerate((1, 1.5)):
        def fn(sub):
            for i, (lv, clip, gain) in enumerate([(None, False, -3.5), (-11.0, False, -3.5), (1.2, True, 0.0)]):
                cx = 18 + i * 44
                fader(sub, t, cx, 10, 120, gain, sel=(i == 1))
                meter_v(sub, t, cx + 13, 10, 120, lv, 3, clipped=clip)
                meter_v(sub, t, cx + 18, 10, 120, None if lv is None else lv - 0.8, 3, clipped=clip)
                gl = "0.0" if gain == 0 else f"{gain:+.1f}".replace("-", "−")
                sub.text(cx + 4, 146, gl, 10, t["ink2"], 400, fam=MONO, anchor="middle")
                if clip:
                    sub.text(cx + 4, 160, "+1.2", 9.5, t["clip"], 500, fam=MONO, anchor="middle")
        tag(g, t, x0 + j * 170, y0 + 24, s)
        embed(g, x0 + j * 170, y0 + 30, s, 160, 165, fn)


def strip_block(g, t, x0, y0, playing):
    label(g, t, x0, y0, "Mixer strip", "selected, playing · a strip at rest")
    for j, s in enumerate((1, 1.5)):
        def fn(sub):
            sub.rect(0, 0, 150, 273, t["base"])
            strip(sub, t, 4, 0, 273, STRIPS[5], playing)
            strip(sub, t, 76, 0, 273, STRIPS[2], False)
        tag(g, t, x0 + j * 170, y0 + 24, s)
        embed(g, x0 + j * 170, y0 + 30, s, 150, 273, fn)


def small_block(g, t, x0, y0):
    """Tab, field, segmented, toggle, scrollbar, menu with a submenu — at 100 % and 150 %."""
    label(g, t, x0, y0, "Tab, field, segmented, toggle, scroll bar, menu")
    for j, s in enumerate((1, 1.5)):
        def fn(sub):
            tab_row(sub, t, 0, 260, 0, ["Mixer", "Piano roll", "Channel rack"])
            # fields: rest, hover, focus, disabled
            for i, st in enumerate(["rest", "hover", "focus", "disabled"]):
                fx = i * 64
                line = {"rest": t["hair"], "hover": t["line"], "focus": t["accent"], "disabled": t["hair"]}[st]
                sub.box(fx, 40, 58, 24, t["well"], line, rx=4)
                c = t["ink4"] if st == "disabled" else t["ink1"]
                sub.text(fx + 8, 56, "1/16", 12, c, 400)
                sub.path(f"M{fx + 44} 50.5 l3 3 l3 -3", stroke=t["ink3"] if st != "disabled" else t["ink4"], sw=1.1)
            sub.box(0, 74, 250, 24, t["well"], t["hair"], rx=4)
            sub.circle(13, 85, 4, "none", t["ink3"], 1.2)
            sub.path("M16 88 l3 3", stroke=t["ink3"], sw=1.2)
            sub.text(26, 90, "Search sounds", 12, t["ink3"], 400)
            seg(sub, t, 0, 108, ["Narrow", "Wide", "Extra"], 1, w=52)
            toggle(sub, t, 176, 112, False)
            toggle(sub, t, 212, 112, True)
            # scroll bars: rest, hover, dragged
            for i, (w, c) in enumerate([(4, t["line"]), (6, t["ink4"]), (6, t["ink3"])]):
                sub.box(10 + i * 22, 144, w, 60, c, None, rx=w / 2)
            # menu with an open submenu
            mx, my = 80, 140
            sub.box(mx, my, 150, 128, t["float"], t["line"], rx=6)
            items = [("Cut", "Ctrl+X", 0), ("Copy", "Ctrl+C", 0), ("Paste", "Ctrl+V", 2), (None, None, 0),
                     ("Colour", "›", 1), ("Rename", "F2", 0)]
            yy = my + 6
            for nm, sc, st in items:
                if nm is None:
                    sub.hline(mx + 8, mx + 142, yy + 3, t["hair"])
                    yy += 7
                    continue
                if st == 1:
                    sub.box(mx + 4, yy, 142, 22, t["raised"], None, rx=4)
                c = t["ink4"] if st == 2 else t["ink1"]
                sub.text(mx + 12, yy + 15, nm, 12, c, 400)
                sub.text(mx + 138, yy + 15, sc, 10.5 if sc != "›" else 13, t["ink4"] if st == 2 else t["ink3"],
                         400, fam=MONO if sc != "›" else "PlexC", anchor="end")
                yy += 23
            sx, sy = mx + 146, my + 70
            sub.box(sx, sy, 112, 62, t["float"], t["line"], rx=6)
            for k, ck in enumerate(["kick", "clap", "hats", "perc", "bass", "chords", "pluck", "lead", "choir", "rise"]):
                cx = sx + 10 + (k % 5) * 20
                cy = sy + 10 + (k // 5) * 22
                sub.box(cx, cy, 14, 14, USER[ck], t["ink1"] if ck == "chords" else None, rx=3)
                if ck == "chords":
                    sub.box(cx - 2, cy - 2, 18, 18, None, t["accent"], rx=4)
        tag(g, t, x0 + j * 400, y0 + 24, s)
        embed(g, x0 + j * 400, y0 + 30, s, 262, 272, fn)


def sheet(t):
    g = Svg(SW, SH, 1)
    g.rect(0, 0, SW, SH, t["base"])
    g.text(24, 34, "Studio ink — parts", 20, t["ink1"], 500)
    g.text(24 + width("Studio ink — parts", 20, 500) + 12, 34, f"Cantus {t['name']} · rendered 1:1", 12, t["ink3"], 400)
    y = keys_block(g, t, 70)
    y += 18
    g.hline(24, SW - 24, y - 4, t["hair"])
    knobs_block(g, t, 24, y + 20)
    faders_block(g, t, 24, y + 150)
    strip_block(g, t, 560, y + 20, True)
    small_block(g, t, 24, y + 460)
    return g


if __name__ == "__main__":
    for t in (NIGHT, PAPER):
        n = t["name"].lower()
        g = sheet(t)
        html = os.path.join(OUT, "html", f"parts-{n}.html")
        write(html, page(g.svg(), t["base"], SW, SH))
        png = os.path.join(OUT, f"parts-{n}-100.png")
        render(html, png, SW, SH, 1)
        pass
        print(png)
