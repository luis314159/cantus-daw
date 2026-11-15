"""Direction A — **Compás**, the pictogram: a sign figure come alive.

One ink (the theme's ink 1), solid pieces of constant weight with round ends — a disc head,
a capsule torso, capsule limbs — each piece **cut** from the one behind it by a hairline of
the room's colour (the rig's outline-as-a-fill in the room's colour). One gold accent: the
headphones. No face: moods are negative space cut into the disc. It dances **pose to pose**:
it snaps into a key on the beat and holds it (snap time), like the kinetic pictograms of a
games' opening ceremony — a figure the eye reads at 48 px.
"""
from body import Props, solve, place
from moves import Style
from rig import add, sub, mul, rot, length
from svg import Doc, tube_d, f

NAME = "Compás"
PROPS = Props(leg=0.43, thigh=0.222, shin=0.222, torso=0.275, neck=0.030, head=0.092,
              shoulder=0.066, hipw=0.040, stance=0.056, upper=0.148, fore=0.150, foot=0.05)
MOVES = {"groove": "pump", "steps": "steps_open", "jump": "star"}
STYLE = Style(bounce=0.06, sway=0.05, step=0.15, arms=1.0, lag=0.0, hop=0.20, squash=0.08,
              snap=1.0)

W_ARM, W_LEG, W_TORSO_TOP, W_TORSO_BOT = 0.066, 0.078, 0.162, 0.120
CUT = 0.017


def draw(pose, th, cut_color=None, trails=None, floor=True, face=True):
    """The character in its units (SVG). `cut_color` is the room behind it (None: the
    theme's base). `trails` is a list of earlier Joints (motion marks)."""
    j = solve(pose, PROPS)
    P = lambda p: place(p, j)
    ink = th["ink1"]
    room = cut_color or th["base"]
    d = Doc(cut_color=room, cut=CUT)
    gx = (P(j.feet[0])[0] + P(j.feet[1])[0]) / 2
    if floor:
        # contact shadow and the beat ring (gold, on the beat)
        k = max(0.0, 1.0 - pose.lift * 3.5)
        d.raw(f'<ellipse cx="{f(gx)}" cy="0.012" rx="{f(0.17 * k + 0.04)}" ry="{f(0.026 * k + 0.006)}" '
              f'fill="{th["shadow"]}" opacity="0.55"/>')
        if pose.accent > 0.04:
            r = 0.23 + 0.07 * (1 - pose.accent)
            d.raw(f'<ellipse cx="{f(gx)}" cy="0.012" rx="{f(r)}" ry="{f(r * 0.16)}" fill="none" '
                  f'stroke="{th["gold"]}" stroke-width="{f(0.012 * (0.4 + 0.6 * pose.accent))}" '
                  f'opacity="{f(min(1.0, pose.accent * 1.4))}"/>')
    # motion marks: two short arcs along the path a hand or foot just travelled (ink 3),
    # a gap behind the limb's end — only while it moves fast (the snaps)
    if trails:
        for idx in range(2):
            for limb, w in (("hands", W_ARM), ("feet", W_LEG)):
                now = P(getattr(j, limb)[idx])
                path = [now] + [place(getattr(t, limb)[idx], t) for t in trails]
                travel = sum(length(sub(path[n], path[n + 1])) for n in range(len(path) - 1))
                if travel < 0.10:
                    continue
                # keep the part of the path more than a limb's half-width + a gap behind
                keep = [p for p in path if length(sub(p, now)) > w * 0.5 + 0.035]
                if len(keep) < 2:
                    continue
                a, z = keep[0], keep[-1]
                dirv = sub(a, z)
                ln = length(dirv) or 1e-6
                nrm = (-dirv[1] / ln, dirv[0] / ln)
                for o in (-0.42, 0.42):
                    seg = [add(p, mul(nrm, o * w)) for p in keep[:3]]
                    d.line(seg, 0.010, th["ink3"], cut=False, smooth=True)
    # legs and torso: one silhouette (their cuts first, then their fills: no line inside)
    legs = [[P(j.hips[i]), P(j.knees[i]), P(j.feet[i])] for i in range(2)]
    top = P(add(j.neck, rot((0.0, W_TORSO_TOP * 0.42), j.torso_deg)))
    bot = P(j.pelvis)
    torso = tube_d([top, bot], [W_TORSO_TOP, W_TORSO_BOT])
    for leg in legs:
        d.line(leg, W_LEG + 2 * CUT, room, cut=False)
    d.raw(f'<path d="{torso}" fill="{room}" stroke="{room}" stroke-width="{f(2 * CUT)}" stroke-linejoin="round"/>')
    for leg in legs:
        d.line(leg, W_LEG, ink, cut=False)
    d.path(torso, ink, cut=False)
    # arms: in front, cut from the torso where they cross it, joined at the shoulder
    for i in range(2):
        sh, el, ha = P(j.shoulders[i]), P(j.elbows[i]), P(j.hands[i])
        free = add(sh, mul(sub(el, sh), 0.55))
        d.line([free, el, ha], W_ARM + 2 * CUT, room, cut=False)
        d.line([sh, el, ha], W_ARM, ink, cut=False)
    # head and the gold headphones
    hc = P(j.head)
    r = PROPS.head
    d.circle(hc, r, ink)
    if face:
        _face(d, hc, r, pose, j, room)
    band_r = r + 0.022
    deg = j.head_deg
    a0, a1 = 200, 340
    import math
    arcp = [add(hc, rot((band_r * math.cos(math.radians(a)), band_r * math.sin(math.radians(a))), deg))
            for a in range(a0, a1 + 1, 10)]
    d.line(arcp, 0.017, th["gold"], cut=True, smooth=True)
    for s in (-1, 1):
        cup = add(hc, rot((s * (r + 0.006), 0.012), deg))
        from svg import rrect_d
        dd, tr = rrect_d(cup[0], cup[1], 0.040, 0.066, 0.016, deg)
        d.path(dd, th["gold"], transform=tr)
    return d.svg()


def _face(d, hc, r, pose, j, room):
    """Negative space cut into the disc: nothing at rest; a mood shows as a cut."""
    deg = j.head_deg
    e = pose.eyes
    if e == "cool":   # a visor band: sunglasses as negative space
        a = add(hc, rot((-r * 0.78, -r * 0.05), deg))
        b = add(hc, rot((r * 0.78, -r * 0.05), deg))
        d.line([a, b], r * 0.30, room, cut=False, cap="butt")


def key_poses():
    from rig import Pose
    from moves import pump, steps_open, star, UP, DOWN, OUT, W, HIP
    return [
        ("Idle", Pose(fk=((8, 4), (8, 4)))),
        ("Pump", pump(0.0, STYLE)),
        ("Flex", pump(0.5, STYLE)),
        ("Point", steps_open(1.0, STYLE)),
        ("Crouch", star(0.95, STYLE)),
        ("Star jump", star(1.16, STYLE)),
        ("Kick", Pose(hip=(-0.10, 0.04), lean=-8, fk=((95, 0), (110, 40)),
                      feet=((0, 0), (0.62, -0.42)), tilt=-6)),
        ("Hero", Pose(feet=((-0.20, 0), (0.20, 0)), fk=(HIP, HIP), nod=-8, eyes="cool")),
        ("Bow", Pose(hip=(0.0, 0.05), chest=(0.0, 0.08), nod=24, tilt=4,
                     fk=((30, -100), (80, 10)), feet=((0, 0), (0.10, 0)))),
    ]
