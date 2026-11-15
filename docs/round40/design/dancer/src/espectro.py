"""New — **Espectro**, the analyser: a figure made of spectrum bars; its body is the music.

Every part of its body is a little analyser of segmented bars, lit from the part's bottom up
by a band of the spectrum:
- the legs show the low frequencies (the kick);
- the torso shows the mids;
- the arms show the snare and claps;
- the head shows the highs (the hats).

The segments light lapis, then gold, then vermilion near the top (the logo's colours as an
analyser's), with a gold peak-hold cap on each bar. The unlit segments are the room's well,
so the silhouette always shows. Its eyes and mouth are negative space cut through the head's
bars.

It dances loose and bouncy, the bars jumping with every hit. Signature, **Pico**: it crouches
(every bar drops), rises (every bar climbs, the figure stretching), and on the beat every
bar hits the top and breaks into gold sparks that fly and fall.

In the app the bars read the **real audio** (the master's spectrum, as the companion's
visualiser does); here a spectrum is synthesised from the beat (kick on the beats, snare on
2 and 4, hats on the eighths).
"""
import math
from body import Props, solve, place
from moves import Style, MOVES as ALL_MOVES, steps, groove
from rig import (Pose, add, sub, mul, rot, length, lerp, beat_down, cos1, smooth, ease_in,
                 ease_out, ease_out_back, clamp01, arc)
from svg import Doc, tube_d, curve_d, f

NAME = "Espectro"
PROPS = Props(leg=0.46, thigh=0.240, shin=0.230, torso=0.27, neck=0.03, head=0.095,
              shoulder=0.100, hipw=0.055, stance=0.070, upper=0.155, fore=0.150, foot=0.10)
STYLE = Style(bounce=0.10, sway=0.08, step=0.18, lag=0.10, hop=0.16, squash=0.08, snap=0.0,
              open=(0.60, 0.90), lift_step=0.08)
MOVES = {"groove": "esp_groove", "steps": "esp_step", "jump": "esp_pico"}
BAR, GAP, SEG = 0.020, 0.008, 0.022


def esp_groove(b, st):
    p = groove(b, st)
    p.eyes = "open"
    p.sig = 0.0
    return p


def esp_step(b, st):
    p = steps(b, st)
    p.eyes = "happy" if (b % 2.0) > 0.8 else "open"
    return p


def pico(b, st):
    """Two beats: it crouches as every bar drops; it rises, stretching, as every bar
    climbs; on beat 2 every bar hits the top and breaks into sparks; it lands, bars low,
    and the music comes back."""
    p = Pose()
    bb = b % 2.0
    if bb < 1.0:
        down = smooth(bb / 0.35) * (1 - smooth((bb - 0.35) / 0.65))
        rise = smooth((bb - 0.35) / 0.65)
        p.squash = 0.14 * down - 0.12 * rise
        p.hip = (0.0, 0.18 * down)
        p.hands = ((-0.62 - 0.1 * rise, 0.90 - 1.85 * rise), (0.62 + 0.1 * rise, 0.90 - 1.85 * rise))
        p.level = 0.12 * (1 - rise) + 1.0 * rise ** 2
        p.eyes = "closed" if down > 0.5 else "wide"
        p.sig = 0.0
    else:
        t = bb - 1.0
        p.sig = t                                   # sparks' age
        land = smooth(t / 0.3)
        p.squash = 0.08 * (1 - land) * (t < 0.3)
        p.hands = ((-0.95, 0.30), (0.95, 0.30))
        p.level = 0.15 + 0.5 * smooth((t - 0.4) / 0.6)
        p.eyes = "happy"
        p.mouth = 0.9 * (1 - land)
    p.accent = beat_down(b % 1.0, 8)
    return p


ALL_MOVES.update({"esp_groove": (esp_groove, 2.0), "esp_step": (esp_step, 4.0), "esp_pico": (pico, 2.0)})


def colours(th):
    night = th["name"] == "Night"
    return {"dim": "#323B44" if night else "#D6DDE3", "low": "#6A86DA" if night else "#4664C0",
            "mid": "#E2B85A" if night else "#9C711C", "high": "#F0784A" if night else "#D2541A",
            "cap": "#F2C766" if night else "#7A5A04", "edge": "#59636E" if night else "#A3ADB8"}


def spectrum(band, x, t, level):
    """A band's level at column x (0…1 across the part) at beat t: kick, snare, hats."""
    ph = t % 1.0
    kick = beat_down(ph, 2.5)
    snare = beat_down((t - 1.0) % 2.0 / 2.0, 6.0) if True else 0
    hats = beat_down((2 * t) % 1.0, 3.0)
    wob = 0.10 * math.sin(x * 11.0 + t * 7.3) + 0.06 * math.sin(x * 23.0 - t * 5.1)
    base = {"low": 0.30 + 0.60 * kick, "mid": 0.35 + 0.30 * kick + 0.25 * snare,
            "arms": 0.25 + 0.65 * snare, "high": 0.25 + 0.55 * hats}[band]
    return clamp01((base + wob) * (0.45 + 0.75 * level))


def _local(origin, deg):
    return lambda x, y: add(origin, rot((x, y), deg))


def _bbox(pts, pad=0.0):
    xs, ys = [p[0] for p in pts], [p[1] for p in pts]
    return min(xs) - pad, min(ys) - pad, max(xs) + pad, max(ys) + pad


def _meter(d, shape_d, box, band, pose, C, room, seed):
    """One part of the body as an analyser: the shape filled with the unlit well, bars lit
    from the part's bottom by its band (lapis → gold → vermilion), peak caps, the gaps."""
    Doc.ids += 1
    cid = f"m{Doc.ids}"
    x0, y0, x1, y1 = box
    hgt = max(y1 - y0, 0.02)
    d.raw(f'<clipPath id="{cid}"><path d="{shape_d}"/></clipPath><g clip-path="url(#{cid})">')
    d.raw(f'<path d="{shape_d}" fill="{C["dim"]}"/>')
    n = max(1, int((x1 - x0) / (BAR + GAP)))
    for i in range(n + 1):
        x = x0 + i * (BAR + GAP)
        u = (i + seed * 0.37) / (n + 1)
        lv = spectrum(band, u + seed, pose.t, pose.level)
        if pose.sig > 0 and pose.sig < 0.15:
            lv = 1.0
        peak = max(lv, spectrum(band, u + seed, pose.t - 0.12, pose.level) - 0.08,
                   spectrum(band, u + seed, pose.t - 0.24, pose.level) - 0.16)
        for (a, b_, col) in ((0.0, 0.62, C["low"]), (0.62, 0.86, C["mid"]), (0.86, 1.0, C["high"])):
            top = min(lv, b_)
            if top > a:
                d.raw(f'<rect x="{f(x)}" y="{f(y1 - top * hgt)}" width="{f(BAR)}" height="{f((top - a) * hgt)}" fill="{col}"/>')
        d.raw(f'<rect x="{f(x)}" y="{f(y1 - peak * hgt - 0.010)}" width="{f(BAR)}" height="0.006" fill="{C["cap"]}"/>')
        d.raw(f'<rect x="{f(x + BAR)}" y="{f(y0)}" width="{f(GAP)}" height="{f(hgt)}" fill="{room}"/>')
    y = y1 - SEG
    while y > y0:
        d.raw(f'<rect x="{f(x0)}" y="{f(y)}" width="{f(x1 - x0)}" height="0.0045" fill="{room}"/>')
        y -= SEG
    d.raw("</g>")


def draw(pose, th, cut_color=None, trails=None, floor=True, face=True):
    j = solve(pose, PROPS)
    P = lambda p: place(p, j)
    C = colours(th)
    room = cut_color or th["base"]
    d = Doc()
    gx = (P(j.feet[0])[0] + P(j.feet[1])[0]) / 2
    if floor:
        k = max(0.0, 1.0 - pose.lift * 3.0)
        d.raw(f'<ellipse cx="{f(gx)}" cy="0.008" rx="{f(0.17 * k + 0.05)}" ry="{f(0.024 * k + 0.008)}" '
              f'fill="{th["shadow"]}" opacity="0.5"/>')
    T = PROPS.torso
    L = _local(P(j.neck), j.torso_deg)
    parts = []
    for i, s in enumerate((-1, 1)):
        hip, knee, foot = P(j.hips[i]), P(j.knees[i]), P(j.feet[i])
        top = add(add(hip, mul(sub(P(j.pelvis), hip), 0.3)), (0, -0.03))
        pts = [top, knee, add(foot, (0, -0.010))]
        parts.append((tube_d(pts, [0.096, 0.082, 0.090]), _bbox(pts, 0.05), "low", 0.1 + i * 0.5))
    torso = [L(-0.050, -0.004), L(-0.118, 0.012), L(-0.130, 0.060), L(-0.105, 0.170), L(-0.100, T + 0.030),
             L(0.100, T + 0.030), L(0.105, 0.170), L(0.130, 0.060), L(0.118, 0.012), L(0.050, -0.004)]
    parts.append((curve_d(torso, closed=True), _bbox(torso), "mid", 0.3))
    hc = P(j.head)
    R = PROPS.head
    head = [add(hc, (R * 1.05 * math.cos(math.radians(a)), R * 1.10 * math.sin(math.radians(a)))) for a in range(0, 360, 20)]
    parts.append((curve_d(head, closed=True), _bbox(head), "high", 0.7))
    arms = []
    for i, s in enumerate((-1, 1)):
        sh, el, ha = P(j.shoulders[i]), P(j.elbows[i]), P(j.hands[i])
        sh = add(sh, mul(sub(P(j.neck), sh), 0.10))
        fd = sub(ha, el)
        fl = length(fd) or 1e-6
        pts = [sh, el, add(ha, mul(fd, 0.030 / fl))]
        arms.append((tube_d(pts, [0.080, 0.070, 0.074]), _bbox(pts, 0.045), "arms", 0.2 + i * 0.4))
    for dd, box, band, seed in parts:
        _meter(d, dd, box, band, pose, C, room, seed)
    if face:
        _face(d, hc, R, pose, room)
    for dd, box, band, seed in arms:
        d.raw(f'<path d="{dd}" fill="{room}" stroke="{room}" stroke-width="0.022" stroke-linejoin="round"/>')
        _meter(d, dd, box, band, pose, C, room, seed)
    if pose.sig > 0:      # Pico: sparks out of every part's top, flying and falling
        t = pose.sig
        for n, (dd, box, band, seed) in enumerate(parts + arms):
            x0, y0, x1, y1 = box
            for k in range(6):
                h1 = math.sin((n * 7 + k) * 12.9898) * 43758.5453
                r = h1 - math.floor(h1)
                vx = (r - 0.5) * 0.9
                vy = -0.9 - 0.6 * ((h1 * 1.7) % 1.0)
                sx = x0 + (x1 - x0) * ((k + 0.5) / 6)
                px, py = sx + vx * t * 0.6, y0 + vy * t * 0.6 + 1.6 * (t * 0.6) ** 2
                a = max(0.0, 1 - t * 1.1)
                if a > 0:
                    d.raw(f'<rect x="{f(px)}" y="{f(py)}" width="0.016" height="0.016" fill="{C["cap"]}" '
                          f'opacity="{f(a)}" transform="rotate({f(r * 90)} {f(px)} {f(py)})"/>')
    return d.svg()


def _face(d, hc, R, pose, room):
    """Eyes and mouth cut through the head's bars."""
    e = pose.eyes
    for s in (-1, 1):
        c = add(hc, (s * 0.034, -0.010))
        if e == "happy":
            d.line([add(c, (-0.014, 0.008)), add(c, (0, -0.006)), add(c, (0.014, 0.008))], 0.010, room, cut=False, cap="butt")
        elif e in ("closed", "sleepy"):
            d.line([add(c, (-0.014, 0.002)), add(c, (0.014, 0.002))], 0.008, room, cut=False, cap="butt")
        elif e == "cool":
            if s < 0:
                d.line([add(hc, (-0.060, -0.010)), add(hc, (0.060, -0.010))], 0.022, room, cut=False, cap="butt")
        else:
            w = 0.026 if e == "wide" else 0.020
            d.raw(f'<rect x="{f(c[0] - w / 2)}" y="{f(c[1] - w * 0.7)}" width="{f(w)}" height="{f(w * 1.4)}" fill="{room}"/>')
    mw = 0.050 if pose.mouth > 0.3 else 0.036
    mh = 0.010 + 0.020 * pose.mouth
    d.raw(f'<rect x="{f(hc[0] - mw / 2)}" y="{f(hc[1] + 0.038)}" width="{f(mw)}" height="{f(mh)}" fill="{room}"/>')


def key_poses():
    def at(fn, b):
        p = fn(b, STYLE)
        p.t = b
        return p
    return [("Idle", Pose(eyes="open", level=0.5, t=0.3)), ("Groove", at(esp_groove, 0.0)), ("Snare", at(esp_groove, 1.0)),
            ("Step", at(esp_step, 1.0)), ("Drop", at(pico, 0.3)), ("Rise", at(pico, 0.85)),
            ("Pico!", at(pico, 1.05)), ("Sparks", at(pico, 1.3)), ("Back", at(pico, 1.8))]


def mood_items():
    return [("Neutral", Pose(eyes="open", level=0.5)), ("Happy", Pose(eyes="happy", level=0.7)),
            ("Excited", Pose(eyes="wide", level=1.0, mouth=0.8)), ("Calm", Pose(eyes="closed", level=0.3)),
            ("Surprised", Pose(eyes="wide", level=0.6, mouth=0.5)), ("Sleepy", Pose(eyes="sleepy", level=0.1, tilt=10)),
            ("Cool", Pose(eyes="cool", level=0.6)), ("Loud", Pose(eyes="happy", level=1.0, mouth=1.0, t=0.0))]


MOOD_SIZE = 420
