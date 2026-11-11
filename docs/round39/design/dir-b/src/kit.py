"""dir-b controls: keys, groups, fields, segmented, knob, fader + meter, toggle, tab, scroll."""
from core import mix, UI, NARROW, CLOCK
from glyphs2 import glyph

R_KEY = 3      # keys, fields, segments
R_PANEL = 5    # popovers, panels


def key(c, x, y, w, h, g=None, state="rest", gsize=None, label=None, lsize=11, r=R_KEY,
        lit=False, joined=None):
    """A key is a dark well: a touch darker than its bar, a 1-px dark rim, a 1-px light lip at
    the bottom; no gradient, no shadow. `joined` = 'l', 'm', 'r' for a group's members."""
    t = c.t
    night = t["name"] == "Night"
    fill = {"rest": t["key"],
            "hover": mix(t["key"], t["hover"], 0.7) if night else mix(t["key"], "#FFFFFF", 0.35),
            "pressed": t["well"] if night else mix(t["key"], t["ink3"], 0.22),
            "on": mix(t["key"], t["gold_tint"], 0.13), "disabled": t["key"]}[state]
    if joined:
        _joined_bg(c, x, y, w, h, fill, joined, r)
    else:
        c.rect(x, y, w, h, fill, rx=r, stroke=t["key_edge"])
    if state != "pressed" and state != "disabled":
        lip = t["gold"] if state == "on" else t["key_lip"]
        c.rect(x + (0 if joined in ("m", "r") else 1.5), y + h - 1, w - (0 if joined == "m" else 1.5)
               - (1.5 if joined in (None, "r") else 0), 1, lip,
               op=0.55 if state == "on" else (0.9 if t["name"] == "Night" else 1.0))
    if g:
        on = state == "on"
        off = state == "disabled"
        glyph(c, g, x + w / 2, y + h / 2 + (0.5 if state == "pressed" else 0), gsize or h / 3,
              on=on, off=off, lit=lit)
    if label:
        col = t["gold"] if state == "on" else (t["ink4"] if state == "disabled" else
                                              (t["ink1"] if state == "hover" else t["ink2"]))
        c.text(x + w / 2, y + h / 2 + lsize * 0.36, label, size=lsize, fill=col, wt=600,
               anchor="middle")


def _joined_bg(c, x, y, w, h, fill, pos, r):
    t = c.t
    c.rect(x, y, w, h, fill)
    c.hline(x, x + w, y, t["key_edge"])
    c.hline(x, x + w, y + h - 1, t["key_edge"], op=0.6)
    if pos == "l":
        c.vline(x, y, y + h, t["key_edge"])
    if pos == "r":
        c.vline(x + w - 1, y, y + h, t["key_edge"])
    if pos in ("m", "r"):
        c.vline(x, y + 4, y + h - 4, t["line_soft"] if t["name"] == "Night" else t["line"])


def group_frame(c, x, y, w, h, r=R_KEY):
    """The rounded frame a joined group sits in (drawn after its members)."""
    t = c.t
    c.rect(x, y, w, h, "none", rx=r, stroke=t["key_edge"])


def segmented(c, x, y, items, h=22, on=0, size=11, pad=10, fam=UI, widths=None):
    """Words in one well; the chosen segment is raised (a lighter cap) with ink-1 and a
    gold hairline under its word. Returns the total width."""
    t = c.t
    ws = widths or [round(len(s) * size * 0.50 + pad * 2) for s in items]
    W = sum(ws) + 2
    c.rect(x, y, W, h, t["well"] if t["name"] == "Night" else t["key"], rx=R_KEY,
           stroke=t["key_edge"])
    cx = x + 1
    for i, (s, w) in enumerate(zip(items, ws)):
        if i == on:
            c.rect(cx + 1, y + 2, w - 2, h - 4, t["raised"] if t["name"] == "Night" else
                   t["well"], rx=2)
            c.text(cx + w / 2, y + h / 2 + size * 0.36, s, size=size, fill=t["ink1"], wt=600,
                   anchor="middle", fam=fam)
            c.rect(cx + w / 2 - 6, y + h - 4, 12, 1, t["gold"])
        else:
            c.text(cx + w / 2, y + h / 2 + size * 0.36, s, size=size, fill=t["ink3"], wt=500,
                   anchor="middle", fam=fam)
        cx += w
    return W


def field(c, x, y, w, h, value, unit=None, size=12, drop=False, focus=False, tnum=True,
          align="start"):
    """A value field: an inset well, value in ink-1, its unit small in ink-3, a caret if it
    opens a list. Focus = a gold rim."""
    t = c.t
    c.rect(x, y, w, h, t["well"], rx=R_KEY, stroke=t["gold"] if focus else t["key_edge"])
    tx = x + 7 if align == "start" else x + w / 2
    c.text(tx, y + h / 2 + size * 0.36, value, size=size, fill=t["ink1"], wt=500, tnum=tnum,
           anchor="start" if align == "start" else "middle")
    if unit:
        c.text(x + w - (16 if drop else 6), y + h / 2 + 3.5, unit, size=9, fill=t["ink3"],
               wt=600, anchor="end", ls=0.4)
    if drop:
        glyph(c, "caret_down", x + w - 8, y + h / 2, 5, ink1=t["ink3"], ink2=t["ink3"])


def label(c, x, y, s, size=9.5, fill=None, anchor="start", wt=600):
    """A small label: sentence case, ink-3, tracked a little — never a heavy capital bar."""
    c.text(x, y, s, size=size, fill=fill or c.t["ink3"], wt=wt, anchor=anchor, ls=0.25)


def knob(c, cx, cy, r, v, state="rest", centre=False, auto=False, col=None):
    """A knob: a well ring, the value as an arc on it, a pointer on a graphite cap.
    v in 0..1. `centre` draws the arc from 12 o'clock (pan). `auto` = automated: the arc is
    dashed in teal and a small 'A' sits under it."""
    import math
    t = c.t
    a0, a1 = math.radians(135), math.radians(405)
    def pt(a, rr):
        return cx + rr * math.cos(a), cy + rr * math.sin(a)
    def arc(aa, ab, rr):
        (x0, y0), (x1, y1) = pt(aa, rr), pt(ab, rr)
        large = 1 if abs(ab - aa) > math.pi else 0
        sw = 1 if ab > aa else 0
        s = c.s
        return "M%.2f %.2f A%.2f %.2f 0 %d %d %.2f %.2f" % (x0 * s, y0 * s, rr * s, rr * s,
                                                           large, sw, x1 * s, y1 * s)
    track = t["well"] if t["name"] == "Night" else t["key"]
    c.add('<path d="%s" fill="none" stroke="%s" stroke-width="%.2f" stroke-linecap="round"/>'
          % (arc(a0, a1, r), track, 2.5 * c.s))
    av = a0 + (a1 - a0) * v
    start = math.radians(270) if centre else a0
    lo, hi = (min(start, av), max(start, av))
    vc = col or (t["gold"] if state in ("dragged",) else (t["teal"] if auto else t["ink2"]))
    if hi - lo > 0.01:
        dash = ' stroke-dasharray="%.2f %.2f"' % (2 * c.s, 1.6 * c.s) if auto else ""
        c.add('<path d="%s" fill="none" stroke="%s" stroke-width="%.2f" stroke-linecap="round"%s/>'
              % (arc(lo, hi, r), vc, 2.5 * c.s, dash))
    capr = r - 3.5
    capf = {"rest": t["raised"], "hover": t["hover"], "dragged": t["hover"]}.get(state, t["raised"])
    if t["name"] == "Paper":
        capf = {"rest": t["raised"], "hover": "#FFFFFF", "dragged": "#FFFFFF"}.get(state)
    c.circle(cx, cy, capr, fill=capf, stroke=t["key_edge"], sw=1)
    if t["name"] == "Night":
        c.add('<path d="M%.2f %.2f A%.2f %.2f 0 0 1 %.2f %.2f" fill="none" stroke="%s" '
              'stroke-opacity="0.35" stroke-width="%.2f"/>' % (
                  (cx - capr * 0.8) * c.s, (cy - capr * 0.35) * c.s, capr * c.s, capr * c.s,
                  (cx + capr * 0.8) * c.s, (cy - capr * 0.35) * c.s, t["ink2"], c.s))
    (px0, py0), (px1, py1) = pt(av, capr * 0.25), pt(av, capr * 0.92)
    c.add('<line x1="%.2f" y1="%.2f" x2="%.2f" y2="%.2f" stroke="%s" stroke-width="%.2f" '
          'stroke-linecap="round"/>' % (px0 * c.s, py0 * c.s, px1 * c.s, py1 * c.s,
                                        t["gold"] if state == "dragged" else t["ink1"], 1.5 * c.s))
    if state == "hover":
        c.circle(cx, cy, r + 2.5, stroke=t["ink3"], sw=1, op=0.5)
    if auto:
        c.text(cx, cy + r + 8.5, "auto", size=8, fill=t["teal"], wt=700, anchor="middle", ls=0.3)


def meter_db(db):
    """dB to a 0..1 meter position (0 dB at 0.86 so there is room for +6)."""
    if db <= -60:
        return 0.0
    if db >= 6:
        return 1.0
    if db >= -12:
        return 0.62 + (db + 12) / 18 * 0.38
    return (db + 60) / 48 * 0.62
