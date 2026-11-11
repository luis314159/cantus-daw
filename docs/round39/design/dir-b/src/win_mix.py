"""The mixer: tabs, master, 13 strips, the Reverb and Delay returns — no label column."""
from core import mix, HUES, UI, NARROW, CLOCK
from kit import key, knob, segmented, label, meter_db
from kit2 import fader, meter, tab
from glyphs2 import glyph

INSP_X = 1040
MIX_Y = 412
TAB_H = 26
S_Y = MIX_Y + TAB_H
S_H = 698 - S_Y            # 260
SW, MW = 62, 72

STRIPS = [  # number, name, hue, inserts, pan (0..1), fader dB, out, levels while playing
    (1, "Kick", "kick", "Comp · EQ", 0.5, -2.0, "Drums", (-6.5, -6.5)),
    (2, "Clap", "clap", "EQ · Verb", 0.5, -6.0, "Drums", (-12, -11.5)),
    (3, "Hats", "hats", "EQ · De-ess", 0.62, -9.5, "Drums", (-17, -15.5)),
    (4, "Percussion", "perc", "EQ", 0.38, -8.0, "Drums", (-14, -16)),
    (5, "Bass", "bass", "Comp · Sat", 0.5, -4.5, "", (-8, -8)),
    (6, "Chords", "chords", "Comp · EQ", 0.5, -3.5, "", (-10.5, -9.8)),
    (7, "Pluck", "pluck", "EQ · Delay", 0.66, -7.0, "", (-15, -13.5)),
    (8, "Lead", "lead", "Sat · EQ", 0.5, -5.0, "", (-11, -11.5)),
    (9, "Choir", "choir", "EQ · Space", 0.5, -8.5, "", (-19, -18)),
    (10, "Drums", "drums", "Glue · Clip", 0.5, -1.5, "", (-5.5, -5.8)),
    (11, None, None, "", 0.5, 0, "", None),
    (12, None, None, "", 0.5, 0, "", None),
    (13, None, None, "", 0.5, 0, "", None),
]
RETURNS = [("R1", "Reverb", "Space", -6.0, (-21, -20)), ("R2", "Delay", "Delay", -9.0, (-24, -23))]
SELECTED = 6


def db_to_v(db):
    return meter_db(db)


def mixer(c, playing=False):
    t = c.t
    night = t["name"] == "Night"
    c.rect(0, MIX_Y, INSP_X, 698 - MIX_Y, t["panel"])
    # tabs: words, the active one with a gold rule
    x = 4
    x += tab(c, x, MIX_Y, TAB_H, "Mixer", active=True, g="win_mixer")
    x += tab(c, x, MIX_Y, TAB_H, "Piano roll", g="win_piano")
    x += tab(c, x, MIX_Y, TAB_H, "Channel rack", g="win_rack")
    tab(c, x, MIX_Y, TAB_H, "Session", g="win_session")
    segmented(c, 880, MIX_Y + 3, ["Compact", "Wide"], h=20, on=0, size=10.5, pad=10)
    c.hline(0, INSP_X, S_Y - 1, t["line_soft"] if night else t["line"])
    # the strips sit on the console's floor, one slightly darker than the panel
    c.rect(0, S_Y, INSP_X, S_H, mix(t["panel"], t["well"], 0.25 if night else 0.15))
    x = 8
    master(c, x, playing)
    x += MW + 6
    for st in STRIPS:
        strip(c, x, st, playing)
        x += SW
    x += 12
    c.vline(x - 7, S_Y + 8, S_Y + S_H - 8, t["line"])
    for r in RETURNS:
        ret(c, x, r, playing)
        x += SW


def strip_body(c, x, w, hue, sel=False, empty=False):
    t = c.t
    night = t["name"] == "Night"
    base = t["raised"] if sel else t["panel"]
    fill = mix(base, hue, 0.06 if night else 0.07) if hue else base
    c.rect(x, S_Y + 4, w - 2, S_H - 8, fill, rx=3, op=0.55 if empty else None)
    if hue:
        c.rect(x, S_Y + 4, w - 2, 3, hue, rx=1.5)
    if sel:
        c.rect(x, S_Y + 4, w - 2, S_H - 8, "none", rx=3, stroke=t["gold"],
               extra=' stroke-opacity="0.75"')


def strip(c, x, st, playing):
    t = c.t
    night = t["name"] == "Night"
    n, name, hk, ins, pan, db, out, lv = st
    sel = n == SELECTED
    if name is None:
        strip_body(c, x, SW, None, empty=True)
        c.text(x + 7, S_Y + 21, str(n), size=9.5, fill=t["ink4"], wt=600, tnum=True)
        glyph(c, "plus", x + SW / 2 - 1, S_Y + S_H / 2, 9, ink1=t["ink4"], ink2=t["ink4"])
        return
    hue = HUES[hk]
    strip_body(c, x, SW, hue, sel)
    cx = x + (SW - 2) / 2
    c.text(x + 6, S_Y + 21, name, size=11.5 if len(name) < 9 else 10.5, fill=t["ink1"],
           wt=650 if sel else 600, fam=NARROW)
    c.text(x + SW - 8, S_Y + 21, str(n), size=8.5, fill=t["ink3"], wt=600, anchor="end", tnum=True)
    c.text(cx, S_Y + 36, ins, size=9, fill=t["ink3"], wt=500, anchor="middle", fam=NARROW)
    knob(c, cx, S_Y + 56, 10.5, pan, centre=True, state="rest")
    if abs(pan - 0.5) > 0.01:
        side = "R" if pan > 0.5 else "L"
        c.text(cx - 13, S_Y + 67, "%s%d" % (side, round(abs(pan - 0.5) * 200)), size=8.5,
               fill=t["ink3"], wt=600, tnum=True, anchor="end")
    fz(c, x, cx, db, lv if playing else None, sel)
    out_row(c, cx, out)


def fz(c, x, cx, db, lv, sel, top=S_Y + 76, h=128):
    """Fader + meter + value + M/S."""
    t = c.t
    fader(c, cx - 7, top, h, db_to_v(db), sel=sel)
    pk = (lv[0] + 3.2, lv[1] + 2.8) if lv else None
    meter(c, cx + 9, top, 9, h, levels=lv, peaks=pk)
    if lv:
        c.text(cx + 13.5, top - 4, "%.1f" % max(pk), size=8, fill=t["ink2"], wt=600,
               anchor="middle", tnum=True)
    v = ("%+.1f" % db).replace("-", "−") if db else "0.0"
    c.text(cx, top + h + 14, v, size=10.5, fill=t["ink1"] if sel else t["ink2"], wt=600,
           anchor="middle", tnum=True)
    for j, s in enumerate(("M", "S")):
        kx = cx - 19 + j * 20
        c.rect(kx, top + h + 20, 18, 14, t["key"], rx=2.5, stroke=t["key_edge"])
        c.text(kx + 9, top + h + 30.6, s, size=9, fill=t["ink3"], wt=700, anchor="middle")


def out_row(c, cx, out):
    t = c.t
    if out:
        c.text(cx, S_Y + S_H - 9, "→ " + out, size=9, fill=t["ink3"], wt=500, anchor="middle",
               fam=NARROW)


def master(c, x, playing):
    t = c.t
    night = t["name"] == "Night"
    c.rect(x, S_Y + 4, MW - 2, S_H - 8, t["raised"] if night else t["well"], rx=3)
    c.rect(x, S_Y + 4, MW - 2, 3, t["ink3"], rx=1.5)
    c.text(x + 7, S_Y + 21, "Master", size=11.5, fill=t["ink1"], wt=650, fam=NARROW)
    c.text(x + MW / 2 - 1, S_Y + 36, "Limit · EQ", size=9, fill=t["ink3"], wt=500, anchor="middle",
           fam=NARROW)
    cx = x + MW / 2 - 1
    # the master's scale is written out: its numbers serve every strip beside it
    top, h = S_Y + 76, 128
    for db in (6, 0, -6, -12, -24, -48):
        ty = top + 1 + (h - 2) * (1 - meter_db(db))
        c.text(x + 19, ty + 3, ("%d" % db).replace("-", "−") if db else "0", size=8,
               fill=t["ink3"], wt=600, anchor="end", tnum=True)
    lv = (-7.5, -8.4) if playing else None
    fader(c, cx - 1, top, h, db_to_v(0))
    meter(c, cx + 14, top, 13, h, levels=lv, peaks=(-3.1, -3.9) if playing else None)
    c.text(cx + 3, top - 26, "LUFS", size=8, fill=t["ink3"], wt=650, anchor="middle", ls=0.4)
    c.text(cx + 3, top - 12, "−9.8" if playing else "—", size=12, fam=CLOCK, wt=400,
           fill=t["ink1"] if playing else t["ink4"], anchor="middle", tnum=True)
    c.text(cx, top + h + 14, "0.0", size=10.5, fill=t["ink2"], wt=600, anchor="middle", tnum=True)
    key(c, cx - 19, top + h + 20, 38, 14, label="Mono", lsize=8.5)


def ret(c, x, r, playing):
    t = c.t
    code, name, ins, db, lv = r
    strip_body(c, x, SW, t["ink3"])
    cx = x + (SW - 2) / 2
    c.text(x + 6, S_Y + 21, name, size=11.5, fill=t["ink1"], wt=600, fam=NARROW)
    c.text(x + SW - 8, S_Y + 21, code, size=8.5, fill=t["ink3"], wt=600, anchor="end")
    c.text(cx, S_Y + 36, ins, size=9, fill=t["ink3"], wt=500, anchor="middle", fam=NARROW)
    knob(c, cx, S_Y + 56, 10.5, 0.5, centre=True)
    fz(c, x, cx, db, lv if playing else None, False)
    c.text(cx, S_Y + S_H - 9, "return", size=9, fill=t["ink4"], wt=500, anchor="middle")
