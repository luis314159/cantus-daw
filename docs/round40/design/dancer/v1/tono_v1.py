"""Direction B — **Tono**, the knob: Slate's own parts come alive.

Its head is Slate's knob (the knob body, its rim, a lit lip at the top), the scale of dots
round it lights in gold with the music's level (a visualiser that is a face's halo), and the
gold pointer on its rim is a tuft that swings late on the beat. Its body is a recessed key
of the theme's surface with the logo's five vermilion bars in a well, its limbs Slate's
cables, its hands jack tips, its feet fader caps. It wears the theme: in Paper it is the
paper knob. Bouncy: squash and stretch, the cables follow through.
"""
import math
from body import Props, solve, place
from moves import Style
from rig import add, sub, mul, rot, length, lerp
from svg import Doc, tube_d, rrect_d, bezier_pts, f

NAME = "Tono"
PROPS = Props(leg=0.235, thigh=0.13, shin=0.13, torso=0.215, neck=0.03, head=0.205,
              shoulder=0.135, hipw=0.07, stance=0.085, upper=0.15, fore=0.15, foot=0.10)
STYLE = Style(bounce=0.16, sway=0.10, step=0.32, arms=1.45, lag=0.14, hop=0.24, squash=0.16,
              snap=0.0)
MOVES = {"groove": "groove", "steps": "steps_wave", "jump": "jump"}
CUT = 0.013


def colours(th):
    """Slate's knob and key, each silhouette held at 3:1 on the room by the knob's rim."""
    night = th["name"] == "Night"
    return {
        "knob": "#2F373F" if night else "#FBFCFD",
        "knob_lip": "#3B444D" if night else "#FFFFFF",
        "rim": "#76818D" if night else "#7A8692",
        "body": "#2B3239" if night else "#E4E9EE",
        "body_lip": "#3A434C" if night else "#FBFCFD",
        "edge": "#0F1317" if night else th["base"],
        "well": "#14191E" if night else "#C9D2DA",
        "cable": "#7D8996" if night else "#7A8692",
        "jack": th["ink2"] if night else "#38444F",
        "cap": "#434C56" if night else "#FBFCFD",
        "eye": th["ink1"] if night else "#17202A",
        "dot_off": "#3A434C" if night else "#C5CDD5",
    }


def _hose(d, a, joint, b, w, col, room):
    c = add(mul(joint, 1.35), mul(add(a, b), -0.175))
    pts = bezier_pts(a, c, b, 12)
    d.line(pts, w + 2 * CUT, room, cut=False, smooth=True)
    d.line(pts, w, col, cut=False, smooth=True)


def draw(pose, th, cut_color=None, trails=None, floor=True, face=True):
    j = solve(pose, PROPS)
    P = lambda p: place(p, j)
    C = colours(th)
    room = cut_color or th["base"]
    d = Doc(cut_color=room, cut=CUT)
    gx = (P(j.feet[0])[0] + P(j.feet[1])[0]) / 2
    if floor:
        k = max(0.0, 1.0 - pose.lift * 3.0)
        d.raw(f'<ellipse cx="{f(gx)}" cy="0.01" rx="{f(0.20 * k + 0.05)}" ry="{f(0.028 * k + 0.008)}" '
              f'fill="{th["shadow"]}" opacity="0.55"/>')
    # legs: cables to fader-cap feet
    for i in range(2):
        hip, knee, foot = P(j.hips[i]), P(j.knees[i]), P(j.feet[i])
        _hose(d, hip, knee, add(foot, (0, -0.03)), 0.046, C["cable"], room)
    for i, s in enumerate((-1, 1)):
        foot = P(j.feet[i])
        dd, tr = rrect_d(foot[0] + s * 0.012, foot[1] - 0.028, 0.11, 0.056, 0.014, s * j.toes[i] * -1)
        d.path(dd, C["rim"], transform=tr)
        dd, tr = rrect_d(foot[0] + s * 0.012, foot[1] - 0.030, 0.098, 0.044, 0.011, s * j.toes[i] * -1)
        d.path(dd, C["cap"], cut=False, transform=tr)
        d.raw(f'<line x1="-0.03" y1="0" x2="0.03" y2="0" transform="{tr}" stroke="{th["gold"]}" '
              f'stroke-width="0.008" stroke-linecap="round"/>')
    # the body: a recessed key — edge, surface, a lit lip on top, the well with five bars
    top, bot = P(j.neck), P(j.pelvis)
    mid = mul(add(top, bot), 0.5)
    h = length(sub(top, bot)) + 0.05
    deg = j.torso_deg
    w = 0.30 * (1 + j.squash * 0.25)
    dd, tr = rrect_d(mid[0], mid[1], w + 0.020, h + 0.020, 0.064, deg)
    d.path(dd, C["edge"], transform=tr)
    dd, tr = rrect_d(mid[0], mid[1], w + 0.008, h + 0.008, 0.059, deg)
    d.path(dd, C["rim"], cut=False, transform=tr)
    dd, tr = rrect_d(mid[0], mid[1], w, h, 0.055, deg)
    d.path(dd, C["body_lip"], cut=False, transform=tr)
    dd, tr = rrect_d(mid[0], mid[1] + 0.006, w - 0.004, h - 0.012, 0.052, deg)
    d.path(dd, C["body"], cut=False, transform=tr)
    well_c = add(mid, rot((0.0, 0.012), deg))
    dd, tr = rrect_d(well_c[0], well_c[1], 0.19, 0.105, 0.022, deg)
    d.path(dd, C["well"], cut=False, transform=tr)
    for n, x in enumerate((-0.06, -0.03, 0.0, 0.03, 0.06)):
        shape = (0.35, 0.65, 1.0, 0.65, 0.35)[n]
        lvl = 0.25 + 0.75 * pose.level * (0.7 + 0.3 * math.sin(n * 1.7 + pose.level * 9))
        bh = 0.075 * shape * lvl + 0.012
        c = add(well_c, rot((x, 0.0), deg))
        dd, tr = rrect_d(c[0], c[1], 0.016, bh, 0.008, deg)
        d.path(dd, th["verm"], cut=False, transform=tr)
    # arms: cables from the key's shoulders, jack tips for hands
    for i in range(2):
        sh, el, ha = P(j.shoulders[i]), P(j.elbows[i]), P(j.hands[i])
        _hose(d, sh, el, ha, 0.044, C["cable"], room)
        d.circle(ha, 0.036, C["edge"])
        d.circle(ha, 0.030, C["jack"], cut=False)
    # the head: Slate's knob with its scale of dots and the gold pointer
    hc = P(j.head)
    R = PROPS.head
    hd = j.head_deg
    lit = int(round(pose.level * 11))
    for n in range(11):
        a = 135 + n * 27  # 7 o'clock to 5 o'clock, clockwise over the top
        pnt = add(hc, rot((math.cos(math.radians(a)) * (R + 0.045), math.sin(math.radians(a)) * (R + 0.045)), hd))
        on = n < lit
        d.circle(pnt, 0.013 if on else 0.011, th["gold"] if on else C["dot_off"], cut=False)
    d.circle(hc, R + 0.010, C["edge"])
    d.circle(hc, R, C["rim"], cut=False)
    d.circle(hc, R - 0.012, C["knob_lip"], cut=False)
    d.circle(add(hc, (0.004, 0.006)), R - 0.016, C["knob"], cut=False)
    # the pointer: a gold notch on the rim that swings late (the tuft)
    pa = -90 + pose.point + hd
    p0 = add(hc, (math.cos(math.radians(pa)) * (R - 0.075), math.sin(math.radians(pa)) * (R - 0.075)))
    p1 = add(hc, (math.cos(math.radians(pa)) * (R - 0.012), math.sin(math.radians(pa)) * (R - 0.012)))
    d.line([p0, p1], 0.026, th["gold"], cut=False)
    if face:
        _face(d, hc, R, pose, hd, C, th)
    return d.svg()


def _face(d, hc, R, pose, hd, C, th):
    e = pose.eyes
    eye = C["eye"]
    for s in (-1, 1):
        c = add(hc, rot((s * R * 0.34, R * 0.10), hd))
        if e in ("happy", "closed"):
            if e == "happy":   # ^ arcs
                pts = [add(c, rot((x * R * 0.13, -abs(x) * -R * 0.10 - R * 0.04), hd)) for x in (-1, 0, 1)]
                pts = [add(c, rot((-R * 0.12, R * 0.03), hd)), add(c, rot((0, -R * 0.08), hd)),
                       add(c, rot((R * 0.12, R * 0.03), hd))]
            else:              # closed: a gentle down curve
                pts = [add(c, rot((-R * 0.12, -R * 0.01), hd)), add(c, rot((0, R * 0.05), hd)),
                       add(c, rot((R * 0.12, -R * 0.01), hd))]
            d.line(pts, R * 0.075, eye, cut=False, smooth=True)
        elif e == "cool":
            pass
        else:
            big = 1.25 if e == "wide" else 1.0
            dd, tr = rrect_d(c[0], c[1], R * 0.17 * big, R * 0.28 * big, R * 0.085 * big, hd)
            d.path(dd, eye, cut=False, transform=tr)
            hl = add(c, rot((-R * 0.03, -R * 0.07), hd))
            d.circle(hl, R * 0.032 * big, C["knob"], cut=False)
    if e == "cool":   # a visor: one dark glass bar with a gold lip
        c = add(hc, rot((0, R * 0.08), hd))
        dd, tr = rrect_d(c[0], c[1], R * 1.15, R * 0.30, R * 0.12, hd)
        d.path(dd, "#11151A", cut=False, transform=tr)
        a = add(c, rot((-R * 0.42, -R * 0.06), hd))
        b = add(c, rot((R * 0.42, -R * 0.06), hd))
        d.line([a, b], R * 0.04, th["gold"], cut=False)
    # the mouth: a small smile; open (a well) when it sings
    mc = add(hc, rot((0, R * 0.42), hd))
    if pose.mouth > 0.2:
        dd, tr = rrect_d(mc[0], mc[1], R * 0.24, R * (0.08 + 0.16 * pose.mouth), R * 0.08, hd)
        d.path(dd, "#11151A" if th["name"] == "Night" else "#38444F", cut=False, transform=tr)
    else:
        pts = [add(mc, rot((-R * 0.11, -R * 0.02), hd)), add(mc, rot((0, R * 0.04), hd)),
               add(mc, rot((R * 0.11, -R * 0.02), hd))]
        d.line(pts, R * 0.05, eye, cut=False, smooth=True)


def key_poses():
    from moves import groove, steps_wave as steps, jump
    from rig import Pose
    return [
        ("Idle", Pose(eyes="open", level=0.35)),
        ("Clap", groove(0.0, STYLE)),
        ("Snap", steps(1.0, STYLE)),
        ("Sing", groove(1.0, STYLE)),
        ("Crouch", jump(0.95, STYLE)),
        ("Jump", jump(1.16, STYLE)),
        ("Cool", Pose(eyes="cool", hands=((-0.62, 0.40), (0.95, -0.45)), level=0.8, point=-25)),
        ("Wave", Pose(eyes="happy", hands=((-0.55, 0.85), (1.0, -0.62)), lean=-5, tilt=-8, point=30)),
        ("Sleepy", Pose(eyes="closed", nod=10, tilt=10, level=0.15, point=-40,
                        hands=((-0.38, 0.95), (0.38, 0.95)), hip=(0, 0.05))),
    ]
