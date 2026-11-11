"""The top bar (0..50) and the arrangement's toolbar (50..80)."""
from core import mix, UI, NARROW, CLOCK
from kit import key, group_frame, segmented, field, label
from kit2 import meter, spark
from glyphs2 import glyph
from metrics import width

W, TOP_H, TB_Y, TB_H, INSP_X = 1280, 50, 50, 30, 1040


def logo(c, x, y, sz=24):
    """Cantus's mark at bar size: the wave's C and the playhead cross, on a graphite tile."""
    t = c.t
    c.rect(x, y, sz, sz, "#1F1D1A", rx=6)
    g = c.lingrad([(0, "#FFD98A", 1), (0.5, "#E07A2E", 1), (1, "#9C300C", 1)], 0, 0, 1, 1)
    k = sz / 24
    c.path("M%.1f %.1f C%.1f %.1f %.1f %.1f %.1f %.1f C%.1f %.1f %.1f %.1f %.1f %.1f" % (
        x + 15.5 * k, y + 7.2 * k, x + 13.8 * k, y + 5.4 * k, x + 5 * k, y + 4.6 * k,
        x + 5.2 * k, y + 12 * k, x + 5 * k, y + 19.4 * k, x + 13.8 * k, y + 18.6 * k,
        x + 15.5 * k, y + 16.8 * k), stroke=g, sw=2.4 * k)
    c.rect(x + 16.5 * k, y + 4.5 * k, 1.6 * k, 15 * k, "#F3CF7E", rx=0.8)
    c.rect(x + 14.6 * k, y + 7.4 * k, 5.4 * k, 1.4 * k, "#F3CF7E", rx=0.7)


def topbar(c, overflow_open=False, playing=False):
    t = c.t
    night = t["name"] == "Night"
    c.rect(0, 0, W, TOP_H, t["chrome"])
    c.hline(0, W, TOP_H - 1, t["key_edge"] if night else t["line"])
    logo(c, 12, 13)
    # menus over the project's name
    x = 46
    for m in ("File", "Edit", "Clip", "View", "Options", "Help"):
        c.text(x, 21, m, size=12, fill=t["ink2"], wt=500)
        x += width(m, 12, wt=500) + 11
    c.text(46, 39, "Neon Tide", size=13, fill=t["ink1"], wt=650)
    c.text(46 + width("Neon Tide", 13, wt=650) + 8, 39, "9 tracks · 64 bars · 2:03", size=10.5, fill=t["ink3"], wt=500, tnum=True)
    sep(c, 252)
    # pattern / song
    segmented(c, 264, 14, ["Pat", "Song"], h=22, on=1, size=11, pad=9)
    # transport: one joined group, glyphs a third of the key
    tx, ty, kw = 340, 9, 34
    for i, (g, st) in enumerate((("to_start", "rest"), ("pause" if playing else "play",
                                                          "on" if playing else "rest"),
                                 ("stop", "rest"), ("record", "rest"))):
        key(c, tx + i * kw, ty, kw, 32, g=g, state=st, gsize=11, joined="lmmr"[i])
    group_frame(c, tx, ty, kw * 4, 32)
    key(c, tx + kw * 4 + 6, ty, 32, 32, g="loop", state="on", gsize=13)
    # tempo
    x = 528
    c.rect(x, ty, 92, 32, t["well"], rx=3, stroke=t["key_edge"])
    c.text(x + 8, ty + 25, "124.00", size=23, fam=CLOCK, wt=300, fill=t["ink1"], tnum=True)
    label(c, x + 86, ty + 13, "BPM", size=8.5, anchor="end")
    c.text(x + 86, ty + 26, "4/4", size=11, fill=t["ink2"], wt=500, anchor="end")
    key(c, x + 96, ty, 34, 32, label="Tap", lsize=11)
    # the clock — the bar's focal point
    x = 676
    c.rect(x, 4, 196, 42, t["well"], rx=4, stroke=t["key_edge"])
    label(c, x + 11, 14.5, "BAR · BEAT · TICK", size=8, fill=t["ink3"])
    c.text(x + 9, 41, "41.1", size=34, fam=CLOCK, wt=200, fill=t["ink1"], tnum=True, ls=0.3)
    c.text(x + 66, 41, ".880" if playing else ".000", size=19, fam=CLOCK, wt=300, fill=t["ink3"], tnum=True)
    c.vline(x + 120, 11, 39, t["line"])
    label(c, x + 130, 14.5, "TIME", size=8)
    c.text(x + 130, 31, "1:17.86" if playing else "1:17.42", size=15, fam=CLOCK, wt=400, fill=t["ink2"], tnum=True)
    c.text(x + 130, 41.5, "of 2:03", size=9.5, fill=t["ink3"], wt=500, tnum=True)
    # snap
    x = 886
    key(c, x, ty + 2, 28, 28, g="magnet", state="on", gsize=12)
    field(c, x + 32, ty + 2, 52, 28, "1/16", drop=True, size=12)
    # undo / redo
    x = 980
    key(c, x, ty + 2, 28, 28, g="undo", gsize=12, joined="l")
    key(c, x + 28, ty + 2, 28, 28, g="redo", gsize=12, joined="r")
    group_frame(c, x, ty + 2, 56, 28)
    sep(c, 1046)
    # master meter: a scale when quiet, the last pass's peak remembered, dimmed
    x = 1052
    label(c, x, 16, "Master", size=9, fill=t["ink3"])
    lv = [-7.5, -8.4] if playing else None
    meter_h(c, x, 21, 92, 11, lv, [-3.1, -3.9] if playing else None)
    for db, tx_ in ((-24, 0.30), (-12, 0.55), (-6, 0.72), (0, 0.87)):
        c.text(x + 92 * tx_, 43, "%d" % db if db else "0", size=8, fill=t["ink3"] if night else
               t["ink3"], wt=600, anchor="middle", tnum=True)
    if playing:
        c.text(x + 138, 31, "−3.1", size=15, fam=CLOCK, wt=400, fill=t["ink1"], anchor="end", tnum=True)
        label(c, x + 138, 42, "peak", size=8, anchor="end")
    else:
        c.text(x + 138, 31, "−4.1", size=15, fam=CLOCK, wt=400, fill=t["ink3"], anchor="end", tnum=True)
        label(c, x + 138, 42, "last peak", size=8, anchor="end", fill=t["ink4"] if night else t["ink3"])
    # settings and the overflow key
    key(c, 1200, ty + 2, 28, 28, g="gear", gsize=12)
    st = "on" if overflow_open else "rest"
    key(c, 1234, ty + 2, 38, 28, state=st)
    glyph(c, "more", 1247, ty + 16, 10, on=overflow_open)
    c.text(1262, ty + 20, "3", size=10.5, fill=t["gold"] if overflow_open else t["ink2"], wt=700,
           anchor="middle", tnum=True)


def sep(c, x):
    c.vline(x, 12, 38, c.t["line"])


def meter_h(c, x, y, w, h, lv, pk):
    """The master's horizontal stereo meter, quiet = a scale."""
    from kit import meter_db
    t = c.t
    night = t["name"] == "Night"
    c.rect(x, y, w, h, t["well"], rx=2, stroke=t["key_edge"])
    gid = c.uid("mh")
    stops = [(0, mix(t["meter_lo"], t["well"], 0.35)), (meter_db(-16), t["meter_lo"]),
             (meter_db(-8), t["meter_mid"]), (meter_db(-2), t["meter_mid"]),
             (meter_db(0), t["meter_hi"]), (1, t["meter_hi"])]
    c.defs.append('<linearGradient id="%s" gradientUnits="userSpaceOnUse" x1="%d" y1="0" x2="%d" '
                  'y2="0">%s</linearGradient>' % (gid, c.p(x + 1), c.p(x + w - 1), "".join(
                      '<stop offset="%.3f" stop-color="%s"/>' % s for s in stops)))
    bh = (h - 3) / 2
    for i in range(2):
        by = y + 1 + i * (bh + 1)
        if lv:
            c.rect(x + 1, by, (w - 2) * meter_db(lv[i]), bh, "url(#%s)" % gid)
        if pk:
            c.rect(x + 1 + (w - 2) * meter_db(pk[i]), by, 1, bh, t["ink1"])
    for db in (-48, -24, -12, -6, 0):
        c.vline(x + 1 + (w - 2) * meter_db(db), y + 1, y + h - 1,
                t["panel"] if night else t["chrome"], op=0.9)


def toolbar(c):
    """The arrangement's own toolbar: its title, the tools as one group, grid, follow."""
    t = c.t
    night = t["name"] == "Night"
    c.rect(0, TB_Y, INSP_X, TB_H, t["panel"])
    c.hline(0, INSP_X, TB_Y + TB_H - 1, t["line_soft"] if night else t["line"])
    glyph(c, "win_arr", 17, TB_Y + 15, 11, ink1=t["ink2"], ink2=t["ink3"])
    c.text(30, TB_Y + 19.5, "Arrangement", size=12.5, fill=t["ink1"], wt=650)
    glyph(c, "chev_down", 111, TB_Y + 15.5, 6, ink1=t["ink3"], ink2=t["ink3"])
    x = 134
    tools = ("select", "draw", "slice", "erase", "mute_tool", "zoom")
    for i, g in enumerate(tools):
        key(c, x + i * 28, TB_Y + 4, 28, 22, g=g, gsize=11, state="on" if i == 0 else "rest",
            joined="l" if i == 0 else ("r" if i == len(tools) - 1 else "m"))
    group_frame(c, x, TB_Y + 4, 28 * len(tools), 22)
    x += 28 * len(tools) + 16
    label(c, x, TB_Y + 19, "Grid", size=11, fill=t["ink3"], wt=500)
    field(c, x + 28, TB_Y + 4, 58, 22, "1 bar", drop=True, size=11.5, tnum=False)
    key(c, x + 92, TB_Y + 4, 28, 22, g="follow", gsize=11, state="on")
    # right: what is selected, then the zoom
    c.text(860, TB_Y + 19.5, "1 clip · Chords drop", size=11, fill=t["ink3"], wt=500, anchor="end")
    glyph(c, "zoom", 878, TB_Y + 15, 9, ink1=t["ink3"], ink2=t["ink3"])
    c.rect(892, TB_Y + 14, 120, 2, t["well"] if night else t["key"], rx=1)
    c.rect(892, TB_Y + 14, 64, 2, t["ink3"], rx=1)
    c.circle(956, TB_Y + 15, 5, fill=t["raised"] if night else "#FFFFFF", stroke=t["key_edge"])
    c.text(1030, TB_Y + 19.5, "16 bars", size=10.5, fill=t["ink3"], wt=500, anchor="end")
