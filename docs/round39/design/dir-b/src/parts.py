"""The parts sheet: every control at 1:1 and 4x (the 4x is the real pixels, magnified with
nearest-neighbour — it shows what the screen shows), both themes, 100 % and 150 %."""
import os
from core import Canvas, NIGHT, PAPER, page, render, mix, OUT_DOCS, OUT_BUILD, CLOCK, UI, font_css
from kit import key, knob, segmented, field, group_frame
from kit2 import fader, meter, toggle, tab, scrollbar, menu
from glyphs2 import glyph
from win_mix import strip, STRIPS, S_Y

TRANSPORT = ["to_start", "play", "pause", "stop", "record", "loop", "metronome", "follow"]
TOOLS = ["magnet", "undo", "redo", "select", "draw", "slice", "erase", "mute_tool", "zoom",
         "pick", "gear", "more"]
WINS = ["win_browser", "win_arr", "win_rack", "win_piano", "win_mixer", "win_session",
        "win_inspector", "win_plugins"]
STRIPG = ["phase", "swap", "link", "power", "k_drum", "k_synth", "k_voice", "k_audio", "plus",
          "chev_down", "grip"]
ALL = TRANSPORT + TOOLS + WINS + STRIPG


def gsize(name, k):
    """Each size its own glyph box: the transport a third-plus of its key, tools about half."""
    if name in TRANSPORT[:5]:
        return {16: 7, 20: 8, 24: 9}[k]
    return {16: 9, 20: 11, 24: 13}[k]


def atlas_keys(t, s):
    """Rows of keys at 16, 20 and 24 (logical); at s=1.5 they are 24, 30, 36 px."""
    W = 40 + len(ALL) * 30
    c = Canvas(W, 120, s, t)
    c.rect(0, 0, W, 120, t["chrome"])
    regions = {}
    y = 8
    for k in (16, 20, 24):
        x = 8
        for i, g in enumerate(ALL):
            st = "on" if g in ("loop", "magnet", "win_mixer") else "rest"
            key(c, x, y, k, k, g=g, gsize=gsize(g, k), state=st)
            x += k + (30 - k) if k < 24 else k + 6
        regions[k] = (c.p(4), c.p(y - 4), c.p(x), c.p(y + k + 4))
        y += k + 12
    return c, regions


def atlas_states(t, s):
    """One key in every state, a knob in four, a fader + meter in three, the small controls."""
    c = Canvas(800, 250, s, t)
    c.rect(0, 0, 800, 250, t["panel"])
    R = {}
    x = 12
    for st in ("rest", "hover", "pressed", "on", "disabled"):
        key(c, x, 14, 32, 28, g="play" if st != "on" else "loop", gsize=11 if st != "on" else 13,
            state=st)
        key(c, x, 50, 52, 24, label="Mono" if st != "on" else "Solo", state=st, lsize=11)
        c.text(x + 16, 92, st, size=10, fill=t["ink3"], anchor="middle")
        x += 60
    R["keys"] = (0, 0, c.p(x), c.p(100))
    x = 340
    for st, auto in (("rest", False), ("hover", False), ("dragged", False), ("rest", True)):
        knob(c, x, 40, 13, 0.66, state=st, auto=auto)
        c.text(x, 80, "automated" if auto else st, size=10, fill=t["ink3"], anchor="middle")
        x += 54
    R["knobs"] = (c.p(310), 0, c.p(x - 20), c.p(100))
    x = 580
    for lab, lv, pk, clip, st in (("silent", None, None, False, "rest"),
                                  ("playing", (-9, -10.5), (-5.5, -6.8), False, "rest"),
                                  ("clipping", (1.2, 0.4), (2.1, 1.0), True, "dragged")):
        fader(c, x, 18, 130, 0.62 if st != "dragged" else 0.86, state=st)
        meter(c, x + 16, 18, 9, 130, levels=lv, peaks=pk, clip=clip)
        c.text(x + 8, 166, lab, size=10, fill=t["ink3"], anchor="middle")
        x += 66
    R["faders"] = (c.p(560), 0, c.p(x - 20), c.p(176))
    # small controls
    y = 120
    w = tab(c, 12, y, 26, "Mixer", active=True, g="win_mixer")
    tab(c, 12 + w, y, 26, "Piano roll", g="win_piano")
    field(c, 12, y + 40, 96, 24, "124.00", unit="BPM")
    field(c, 116, y + 40, 96, 24, "1/16", drop=True, focus=True)
    segmented(c, 12, y + 78, ["Off", "1 bar", "2 bars"], h=22, on=1, widths=[48, 60, 60])
    toggle(c, 200, y + 82, False, label_s="Off")
    toggle(c, 262, y + 82, True, label_s="On")
    c.rect(330, y, 10, 120, t["canvas"])
    scrollbar(c, 332, y + 4, 112, 0.1, 0.4)
    c.rect(350, y, 10, 120, t["canvas"])
    scrollbar(c, 352, y + 4, 112, 0.4, 0.4, hover=True)
    R["small"] = (0, c.p(y - 4), c.p(372), c.p(y + 124))
    return c, R


def atlas_menu(t, s):
    c = Canvas(470, 290, s, t)
    c.rect(0, 0, 470, 290, t["canvas"])
    items = [("New track", "Ctrl+T", ""), ("Duplicate", "Ctrl+D", ""), ("", "", "sep"),
             ("Colour", "", "sub"), ("Route to", "", "sub"), ("", "", "sep"),
             ("Mute", "M", "check"), ("Freeze", "", ""), ("Delete", "Del", "dis")]
    rects, h = menu(c, 10, 10, 210, items, hi=3)
    rx, ry, rw, rh = rects[3]
    sub = [("Ember", "", ""), ("Amber", "", ""), ("Sky", "", "check"), ("Moss", "", ""),
           ("Rose", "", ""), ("", "", "sep"), ("Custom…", "", "")]
    menu(c, rx + rw - 2, ry - 5, 150, sub, hi=2)
    sw = {"Ember": "kick", "Amber": "clap", "Sky": "chords", "Moss": "pluck", "Rose": "lead"}
    from core import HUES
    yy = ry
    for nm, _, k in sub:
        if k == "sep":
            yy += 9
            continue
        if nm in sw:
            c.rect(rx + rw + 128, yy + 7, 10, 10, HUES[sw[nm]], rx=2)
        yy += 24
    return c


def atlas_strip(t, s):
    c = Canvas(84, 270, s, t)
    c.rect(0, 0, 84, 270, mix(t["panel"], t["well"], 0.25 if t["name"] == "Night" else 0.15))
    # reuse the window's strip, translated so its top sits at y=4
    c.add('<g transform="translate(%d,%d)">' % (c.p(-10), c.p(-S_Y + 2)))
    strip(c, 10 + 8, STRIPS[5], True)
    c.add("</g>")
    return c


def build(t):
    tag = t["name"].lower()
    out = {}
    for nm, fn in (("keys1", lambda: atlas_keys(t, 1)), ("keys15", lambda: atlas_keys(t, 1.5)),
                   ("states1", lambda: atlas_states(t, 1)), ("states15", lambda: atlas_states(t, 1.5)),
                   ("menu1", lambda: (atlas_menu(t, 1), None)), ("strip1", lambda: (atlas_strip(t, 1), None)),
                   ("strip15", lambda: (atlas_strip(t, 1.5), None))):
        c, R = fn()
        W, H = c.p(c.w), c.p(c.h)
        png = os.path.join(OUT_BUILD, "atlas-%s-%s.png" % (tag, nm))
        render("atlas-%s-%s" % (tag, nm), page(c.svg(), t["panel"]), W, H, png)
        out[nm] = (png, W, H, R)
    return out
