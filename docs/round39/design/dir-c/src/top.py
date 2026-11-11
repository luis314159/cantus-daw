"""The top bar: project, transport, tempo, metre, the clock, snap, undo, the master meter
and the overflow key. At 1280 px three groups fold into the overflow (Metronome, Windows,
CPU/RAM); the panel is drawn by overflow.py."""
from common import SANS, MONO, mix
from glyphs import glyph
from metrics import width

TOP_H = 52


def caption(g, t, x, y, s, anchor="start"):
    """An engraved caption: 8.5 px capitals, tracked, ink-3 — units and group names only."""
    g.text(x, y, s.upper(), 8.5, t["ink3"], 500, ls=0.9, anchor=anchor)


def key(g, t, x, y, w, h, name, state="rest", gk=24, ground=True):
    """A key: a flat well at rest (or no ground at all in a joined group), raised on hover,
    the field tone when pressed, the accent's own ground when on."""
    fills = {"rest": None, "hover": t["raised"], "pressed": t["field"], "on": t["accent_bg"],
             "disabled": None}
    f = fills[state]
    if f:
        g.box(x, y, w, h, f, None, rx=4)
    a = {"on": t["accent"], "disabled": t["ink4"], "hover": t["ink1"]}.get(state, t["ink2"])
    b = {"on": mix(t["accent"], "#FFFFFF", 0.35) if t["name"] == "Night" else mix(t["accent"], "#000000", 0.25),
         "disabled": t["ink4"]}.get(state, t["ink1"] if t["name"] == "Night" else t["ink3"])
    glyph(g, name, x + (w - gk) / 2, y + (h - gk) / 2, gk, a, b)


def logo(g, t, x, y):
    """A stand-in for the Cantus mark (the owner's logo goes here unchanged): a 22 px warm
    tile — the only warm object in the chrome."""
    g.box(x, y, 22, 22, "#E2742F", None, rx=5)
    g.circle(x + 11, y + 11, 5.2, "none", "#FFF4E8", 1.6)
    g.rect(x + 13.2, y + 6, 4.5, 3.6, "#E2742F", snap=False)


def top_bar(g, t, W, folded=3, more_state="rest", playing=False):
    g.rect(0, 0, W, TOP_H, t["bar"])
    g.hline(0, W, TOP_H - g.px, t["hair"])
    logo(g, t, 14, 15)
    # menus and the project, one block
    x = 48
    for m in ["File", "Edit", "Clip", "View", "Options", "Help"]:
        g.text(x, 22, m, 12, t["ink2"], 400)
        x += width(m, 12) + 13
    g.text(48, 41, "Neon Tide", 13, t["ink1"], 500)
    nx = 48 + width("Neon Tide", 13, 500) + 8
    g.text(nx, 41, "10 tracks · 13 strips · 16 bars · 0:31", 11, t["ink3"], 400)

    # pattern / song: a two-segment switch
    x0 = 318
    g.box(x0, 14, 76, 24, t["well"], t["hair"], rx=5)
    g.box(x0 + 38, 16, 36, 20, t["accent_bg"], None, rx=3.5)
    g.text(x0 + 19, 30, "Pat", 11, t["ink3"], 500, anchor="middle")
    g.text(x0 + 56, 30, "Song", 11, t["accent"], 500, anchor="middle")

    # the transport: one joined well, keys divided by hairlines
    tx, ty, kw, kh = 404, 11, 36, 30
    g.box(tx, ty, kw * 4, kh, t["well"], t["hair"], rx=6)
    for i in range(1, 4):
        g.vline(tx + kw * i, ty + 7, ty + kh - 7, t["hair"])
    names = ["to_start", "play", "stop", "record"]
    for i, n in enumerate(names):
        st = "rest"
        if n == "play" and playing:
            st = "on"
        if n == "record":
            glyph(g, n, tx + kw * i + (kw - 24) / 2, ty + (kh - 24) / 2, 24,
                  mix(t["rec"], t["well"], 0.12), mix(t["rec"], "#000000", 0.35))
            continue
        a = t["play"] if (n == "play" and playing) else t["ink2"]
        b = t["ink1"] if t["name"] == "Night" else t["ink3"]
        glyph(g, n, tx + kw * i + (kw - 24) / 2, ty + (kh - 24) / 2, 24, a, b)
    # loop: on (the excerpt loops), the accent's ground
    key(g, t, tx + kw * 4 + 6, ty, 34, kh, "loop", "on")

    # tempo, metre, position: type on the bar, separated by hairlines, no boxes
    x = 606
    caption(g, t, x, 20, "Tempo")
    g.text(x - 1, 43, "124.00", 22, t["ink1"], 300)
    x2 = x + width("124.00", 22, 300) + 14
    g.vline(x2 - 7, 14, 44, t["hair"])
    caption(g, t, x2, 20, "Metre")
    g.text(x2 - 1, 43, "4/4", 22, t["ink1"], 300)
    x3 = x2 + width("4/4", 22, 300) + 16
    g.vline(x3 - 8, 14, 44, t["hair"])
    caption(g, t, x3, 20, "Bar · Beat · Tick")
    clock = "1.1.000" if not playing else "5.3.096"
    g.text(x3 - 2, 45, clock, 31, t["ink1"], 200)
    x4 = x3 + width(clock, 31, 200) + 12
    caption(g, t, x4, 20, "Time")
    g.text(x4, 42, "0:00.00" if not playing else "0:08.76", 12, t["ink2"], 400, fam=MONO)

    # undo / redo, snap
    ux = 930
    key(g, t, ux, 13, 26, 26, "undo", gk=20)
    key(g, t, ux + 28, 13, 26, 26, "redo", gk=20)
    sx = ux + 66
    g.vline(sx - 6, 14, 38, t["hair"])
    key(g, t, sx, 13, 26, 26, "snap", "on", gk=20)
    g.box(sx + 30, 14, 50, 24, t["well"], t["hair"], rx=4)
    g.text(sx + 38, 30, "1/16", 12, t["ink1"], 400)
    g.path(f"M{sx + 68} {24.5} l3 3 l3 -3", stroke=t["ink3"], sw=1.1)

    # the master meter: a calm ladder at rest, the last play's loudness held in ink-3
    mx = 1096
    caption(g, t, mx, 20, "Master")
    if playing:
        g.text(mx + 132, 20, "−9.1 LUFS", 9.5, t["ink2"], 500, anchor="end", ls=0.2)
    else:
        g.text(mx + 132, 20, "last −9.4 LUFS", 9.5, t["ink3"], 400, anchor="end")
    meter_h(g, t, mx, 26, 132, playing)

    # the overflow key: three folded groups
    ox = W - 40
    key(g, t, ox, 13, 26, 26, "more", more_state, gk=20)
    if folded:
        g.circle(ox + 22, 16, 5.5, t["bar"])
        g.circle(ox + 22, 16, 4.5, t["ink4"] if more_state == "rest" else t["accent"])
        g.text(ox + 22, 18.6, str(folded), 7.5, t["bar"] if t["name"] == "Night" else "#FFFFFF", 600, anchor="middle")


DB_TICKS = [-48, -24, -12, -6, 0]


def db_x(db, w):
    """dB to a position on a horizontal meter: the top 24 dB get two thirds of it."""
    if db <= -48:
        return 0
    if db >= -24:
        return w * (1 / 3 + 2 / 3 * (db + 24) / 24)
    return w * (1 / 3) * (db + 48) / 24


def meter_h(g, t, x, y, w, playing):
    for k, lv in enumerate((-7.2, -8.0) if playing else (None, None)):
        yy = y + k * 5
        g.rect(x, yy, w, 3, t["track"])
        if lv is not None:
            lx = db_x(lv, w)
            gid = g.uid("mg")
            g.defs.append(
                f'<linearGradient id="{gid}" x1="{x}" x2="{x + w}" gradientUnits="userSpaceOnUse">'
                f'<stop offset="0" stop-color="{t["m_lo"]}"/><stop offset="0.72" stop-color="{t["m_lo"]}"/>'
                f'<stop offset="0.86" stop-color="{t["m_mid"]}"/><stop offset="1" stop-color="{t["m_hi"]}"/></linearGradient>')
            g.rect(x, yy, lx, 3, f"url(#{gid})")
            g.rect(x + db_x(lv + 2.4, w), yy, 1, 3, t["ink1"])
        else:
            pass
    for d in DB_TICKS:
        xx = x + db_x(d, w)
        g.vline(xx, y + 10, y + 13, t["ink4"])
        lab = "0" if d == 0 else f"{-d}"
        g.text(xx, y + 21, lab, 7.5, t["ink3"], 400, anchor="middle" if d > -48 else "start")
