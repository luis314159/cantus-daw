"""The mixer: master, eleven strips (Drums is a bus), the two returns. No label column and
no 'Slot 3…10': numbers sit where they are read, the only route drawn is one that is not
the default, and at rest a meter shows its scale and the last play's peak, not a slab."""
import math
from common import SANS, MONO, mix
from metrics import width, fit
from tokens import USER, hue
from top import key, caption
from arr import tab_row

STRIPS = [
    # n, name, colour, inserts, gain, pan, out, level when playing (L, R)
    (1, "Kick", "kick", ["Aequalis"], -2.0, 0, "Drums", (-9.0, -9.0)),
    (2, "Clap", "clap", [], -4.5, 0, "Drums", (-14.0, -14.5)),
    (3, "Hats", "hats", ["Aequalis", "Sibilus"], -8.0, 12, "Drums", (-21.0, -18.5)),
    (4, "Percussion", "perc", [], -9.5, -18, "Drums", (-24.0, -27.0)),
    (5, "Bass", "bass", ["Comprimo", "Aequalis"], -5.0, 0, None, (-10.5, -10.5)),
    (6, "Chords", "chords", ["Comprimo", "Aequalis"], -3.5, 0, None, (-13.0, -12.0)),
    (7, "Pluck", "pluck", [], -6.0, 20, None, (-17.0, -14.0)),
    (8, "Lead", "lead", ["Aequalis"], -4.0, 0, None, (-11.5, -11.0)),
    (9, "Choir", "choir", ["Comprimo"], -9.0, 0, None, (-19.0, -18.0)),
    (10, "Rise", "rise", ["Porta"], -12.0, 0, None, None),
    (11, "Drums", "bus", ["Comprimo", "Multiplex", "Terminus"], -1.5, 0, None, (-6.5, -6.8)),
]
RETURNS = [
    (12, "Reverb", None, ["Spatium", "Aequalis"], -6.0, 0, None, (-24.0, -23.0)),
    (13, "Delay", None, ["Delay"], -8.0, 0, None, (-29.0, -27.5)),
]
SEL = 6
SW = 68


def cap_fill(g, t):
    """The one object gradient of the kit: a cap or knob body lit faintly from above."""
    key_ = "_capgrad_" + t["name"]
    if not hasattr(g, key_):
        gid = g.uid("cap")
        top, bot = (("#2B3139", "#191D22") if t["name"] == "Night" else ("#FFFFFF", "#E9E9E5"))
        g.defs.append(f'<linearGradient id="{gid}" x1="0" y1="0" x2="0" y2="1">'
                      f'<stop offset="0" stop-color="{top}"/><stop offset="1" stop-color="{bot}"/></linearGradient>')
        setattr(g, key_, f"url(#{gid})")
    return getattr(g, key_)


def fy(db, top, h):
    """dB to a position on the fader and the meter (one scale for both)."""
    pts = [(6, 0), (0, .2), (-6, .36), (-12, .5), (-24, .68), (-48, .88), (-90, 1)]
    if db >= 6:
        return top
    for (a, pa), (b, pb) in zip(pts, pts[1:]):
        if b <= db <= a:
            return top + h * (pa + (pb - pa) * (a - db) / (a - b))
    return top + h


def knob(g, t, cx, cy, r, val, state="rest", unit_arc=None):
    """A knob: a 270° hairline track, the value arc from its centre detent, a dark
    body and an index line. val in -1..1 (pan) or 0..1 when unit_arc == 'min'."""
    a0, a1 = 135, 405
    def pt(a, rr):
        return cx + rr * math.cos(math.radians(a)), cy + rr * math.sin(math.radians(a))
    tr = t["ink4"] if t["name"] == "Night" else t["line"]
    sx, sy = pt(a0, r)
    ex, ey = pt(a1, r)
    g.path(f"M{sx:.2f} {sy:.2f} A{r} {r} 0 1 1 {ex:.2f} {ey:.2f}", stroke=tr, sw=1.5, cap="butt")
    if unit_arc == "min":
        av = a0 + 270 * val
        start = a0
    else:
        av = 270 + 135 * val
        start = 270
    vc = {"rest": t["ink2"], "hover": t["ink1"], "drag": t["accent"], "auto": t["accent"]}[state]
    if abs(av - start) > 0.5:
        p0, p1 = pt(min(start, av), r), pt(max(start, av), r)
        large = 1 if abs(av - start) > 180 else 0
        g.path(f"M{p0[0]:.2f} {p0[1]:.2f} A{r} {r} 0 {large} 1 {p1[0]:.2f} {p1[1]:.2f}", stroke=vc, sw=1.75, cap="butt")
    else:
        # at the detent: a short tick on the track so a centred pan is still visibly 'C'
        p0 = pt(270, r - 2.2)
        p1 = pt(270, r + 1.2)
        g.path(f"M{p0[0]:.2f} {p0[1]:.2f} L{p1[0]:.2f} {p1[1]:.2f}", stroke=t["ink3"], sw=1)
    body = r - 3.2
    g.circle(cx, cy, body, cap_fill(g, t), t["line"], 1)
    if state in ("hover", "drag"):
        g.circle(cx, cy, body, "none", t["ink3"], 1)
    i0, i1 = pt(av, body * 0.25), pt(av, body - 1.2)
    g.path(f"M{i0[0]:.2f} {i0[1]:.2f} L{i1[0]:.2f} {i1[1]:.2f}", stroke=t["ink1"], sw=1.5)
    if state == "auto":
        g.circle(cx + r * 0.78, cy - r * 0.78, 2.2, t["accent"])


def meter_v(g, t, x, top, h, lv, w=3, ghost=None, clipped=False):
    g.rect(x, top, w, h, t["track"])
    if lv is not None:
        y = fy(lv, top, h)
        gid = g.uid("mv")
        g.defs.append(
            f'<linearGradient id="{gid}" x1="0" y1="{top + h}" x2="0" y2="{top}" gradientUnits="userSpaceOnUse">'
            f'<stop offset="0" stop-color="{t["m_lo"]}"/><stop offset="0.62" stop-color="{t["m_lo"]}"/>'
            f'<stop offset="0.74" stop-color="{t["m_mid"]}"/><stop offset="0.8" stop-color="{t["m_hi"]}"/></linearGradient>')
        g.rect(x, y, w, top + h - y, f"url(#{gid})")
        pk = fy(lv + 3.0, top, h)
        g.rect(x, pk, w, 1, t["ink1"])
    elif ghost is not None:
        g.rect(x, fy(ghost, top, h), w, 1, t["ink4"])
    if clipped:
        g.rect(x, top - 4, w, 3, t["clip"])


def fader(g, t, cx, top, h, gain, sel=False, state="rest"):
    g.rect(cx - 1, top, 2, h, t["track"])
    y = fy(gain, top, h)
    cw, chh = 22, 10
    g.box(cx - cw / 2, y - chh / 2, cw, chh, cap_fill(g, t),
          t["line"] if state == "rest" else t["ink3"], rx=2)
    if t["name"] == "Night":  # a one-pixel lit top edge: the cap is an object, not a slot
        g.rect(cx - cw / 2 + 2, y - chh / 2 + g.px, cw - 4, g.px, "#FFFFFF", op=0.10)
    ic = t["accent"] if sel else t["ink1"]
    g.rect(cx - cw / 2 + 3, y - 0.5, cw - 6, 1, ic)


def strip(g, t, x, sy, sh, s, playing, master=False, ret=False):
    n, name, ck, ins, gain, pan, out, lv = s
    sel = n == SEL
    w = SW if not master else 84
    if sel:
        g.rect(x, sy, w, sh, t["raised"])
    cap = hue(t, USER[ck], "cap", sel) if ck else t["ink3"]
    g.rect(x + 3, sy + 1, w - 6, 2, cap)
    cx = x + w / 2
    if not master:
        g.text(cx, sy + 16, str(n), 9, t["ink3"], 400, fam=MONO, anchor="middle")
    nm = fit(name, 12, w - 8, 600 if sel else 500)
    g.text(cx, sy + 32, nm, 12, t["ink1"], 600 if sel else 500, anchor="middle")
    # the chain: names, two at most, then a count
    iy = sy + 47
    shown = ins if len(ins) <= 2 else ins[:1]
    for k, nm in enumerate(shown):
        g.text(x + 8, iy + k * 12, nm, 10, t["ink2"], 400)
        g.circle(x + w - 9, iy + k * 12 - 3.5, 1.5, t["ink3"])
    if len(ins) > 2:
        g.text(x + 8, iy + 12, f"+{len(ins) - 1} more", 10, t["ink3"], 400)
    # pan
    if not master:
        knob(g, t, cx, sy + 84, 9.5, pan / 50.0)
        if pan:  # a centred pan says so by its detent tick; only an offset is written
            pl = f"R{pan}" if pan > 0 else f"L{-pan}"
            g.text(cx, sy + 106, pl, 9.5, t["ink3"], 400, fam=MONO, anchor="middle")
    # mute, solo, arm
    ky = sy + 114
    keys = (["M"] if master else ["M", "S"]) + (["arm"] if not (master or ret or n == 11) else [])
    kw = 18
    kx0 = cx - (len(keys) * kw + (len(keys) - 1) * 2) / 2
    for j, L in enumerate(keys):
        kx = kx0 + j * (kw + 2)
        g.box(kx, ky, kw, 15, t["well"], None, rx=3)
        if L == "arm":
            g.circle(kx + kw / 2, ky + 7.5, 2.6, t["ink3"])
        else:
            g.text(kx + kw / 2, ky + 11, L, 9.5, t["ink3"], 600, anchor="middle")
    # fader and meter
    ft, fh = sy + 140, sh - 140 - 34
    if master:
        for d in (6, 0, -6, -12, -24, -48):
            yy = fy(d, ft, fh)
            g.text(x + 20, yy + 3, "+6" if d == 6 else str(-d) if d else "0", 8, t["ink3"], 400, fam=MONO, anchor="end")
            g.hline(x + 23, x + 27, yy, t["ink4"])
        fader(g, t, x + 40, ft, fh, gain)
        lvm = (-7.2, -7.6) if playing else (None, None)
        meter_v(g, t, x + 56, ft, fh, lvm[0], 4)
        meter_v(g, t, x + 62, ft, fh, lvm[1], 4)
    else:
        fader(g, t, cx - 3, ft, fh, gain, sel)
        L, R = lv if (playing and lv) else (None, None)
        gh = None
        meter_v(g, t, cx + 10, ft, fh, L, 3, ghost=gh)
        meter_v(g, t, cx + 15, ft, fh, R, 3, ghost=None if gh is None else gh - 0.4)
    gl = "0.0" if gain == 0 else f"{gain:+.1f}".replace("-", "−")
    g.text(cx - (0 if not master else 0), sy + sh - 18, gl, 10, t["ink2"], 400, fam=MONO, anchor="middle")
    return w


def mixer(g, t, x0, x1, y0, y1, playing=False):
    x = tab_row(g, t, x0, x1, y0, ["Mixer", "Piano roll", "Channel rack"])
    # view size: a three-way segmented control
    sx = x + 16
    g.box(sx, y0 + 5, 132, 18, t["well"], t["hair"], rx=4)
    for i, lab in enumerate(["Narrow", "Wide", "Extra"]):
        if i == 1:
            g.box(sx + 2 + i * 43, y0 + 7, 42, 14, t["field"], None, rx=3)
        g.text(sx + 23 + i * 43, y0 + 18, lab, 10.5, t["ink1"] if i == 1 else t["ink3"], 500, anchor="middle")
    sy = y0 + 28
    sh = y1 - sy
    g.rect(x0, sy, x1 - x0, sh, t["base"])
    master = (0, "Master", None, ["Aequalis", "Multiplex", "Latitudo", "Culmen"], 0.0, 0, None, None)
    strip(g, t, x0 + 8, sy, sh, master, playing, master=True)
    g.vline(x0 + 96, sy + 8, y1 - 8, t["hair"])
    sx = x0 + 102
    for s in STRIPS:
        strip(g, t, sx, sy, sh, s, playing)
        sx += SW
    # the drum strips' route: drawn once, as a bracket into the bus
    bx0 = x0 + 102 + SW / 2
    bx1 = x0 + 102 + 3 * SW + SW / 2
    by = y1 - 6
    for i in range(4):
        g.vline(bx0 + i * SW, by - 4, by, t["ink4"])
    g.hline(bx0, bx1 + 1, by, t["ink4"])
    lab = "→ 11 Drums"
    g.rect((bx0 + bx1) / 2 - width(lab, 9.5) / 2 - 5, by - 6, width(lab, 9.5) + 10, 10, t["base"])
    g.text((bx0 + bx1) / 2, by + 2.5, lab, 9.5, t["ink3"], 400, anchor="middle")
    rx = sx + 14
    g.vline(rx - 7, sy + 8, y1 - 8, t["hair"])
    for s in RETURNS:
        strip(g, t, rx, sy, sh, s, playing, ret=True)
        rx += SW
    return rx
