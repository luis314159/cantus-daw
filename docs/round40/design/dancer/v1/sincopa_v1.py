"""Direction C — **Síncopa**, the figure in values: an editorial illustration that dances.

No ink lines at all: every piece is two flat values (Slate's rule, *separated by value,
never by borders*, applied to a body — the rig's hard cel from ADR 0105 without its rim),
and the outfit is the logo's three colours: a vermilion hoodie, lapis wide trousers, gold
headphones, with white sneakers. Adult proportions (about 7 heads), a small calm face.
Fluid and human: the hips lead, the head and hands follow late, it grooves rather than
bounces — it dances *off* the beat as well as on it (its name: syncopation).
"""
import math
from body import Props, solve, place
from moves import Style
from rig import add, sub, mul, rot, length, lerp
from svg import Doc, tube_d, rrect_d, curve_d, bezier_pts, f

NAME = "Síncopa"
PROPS = Props(leg=0.465, thigh=0.243, shin=0.232, torso=0.262, neck=0.036, head=0.088,
              shoulder=0.084, hipw=0.048, stance=0.064, upper=0.150, fore=0.145, foot=0.10)
HS = 1.18  # the head drawn larger than the skeleton's (a readable face at 200 px)
STYLE = Style(bounce=0.075, sway=0.10, step=0.17, arms=1.0, lag=0.12, hop=0.17, squash=0.06,
              snap=0.0, lean=1.3, open=(0.56, 0.95), chest=-0.06)
MOVES = {"groove": "groove", "steps": "steps_human", "jump": "jump"}

PAL = {
    "skin": "#C98F6D", "skin_s": "#A6704F",
    "hair": "#211A1A", "hair_l": "#4B3A36",
    "hood": "#E0693B", "hood_s": "#B24E29", "hood_d": "#8E3D20",
    "pants": "#4664C0", "pants_s": "#2E4794",
    "shoe": "#F3F5F7", "shoe_s": "#BAC3CC", "sole": "#2B3239",
    "gold": "#E2B85A", "gold_s": "#B38A35",
    "string": "#F4E9DA", "eye": "#1C2126", "lip": "#8E4A3A", "glass": "#14191E",
}
CUT = 0.011


def _local(origin, deg):
    return lambda x, y: add(origin, rot((x, y), deg))


def draw(pose, th, cut_color=None, trails=None, floor=True, face=True):
    j = solve(pose, PROPS)
    P = lambda p: place(p, j)
    c = PAL
    room = cut_color or th["base"]
    d = Doc(cut_color=room, cut=CUT)
    gx = (P(j.feet[0])[0] + P(j.feet[1])[0]) / 2
    if floor:
        k = max(0.0, 1.0 - pose.lift * 3.0)
        d.raw(f'<ellipse cx="{f(gx)}" cy="0.008" rx="{f(0.17 * k + 0.05)}" ry="{f(0.024 * k + 0.008)}" '
              f'fill="{th["shadow"]}" opacity="0.5"/>')
    # legs: wide lapis trousers, then sneakers
    for i, s in enumerate((-1, 1)):
        hip, knee, foot = P(j.hips[i]), P(j.knees[i]), P(j.feet[i])
        top = add(hip, mul(sub(P(j.pelvis), hip), 0.25))
        top = add(top, (0, -0.03))
        ankle = add(foot, (0, -0.045))
        d.cel(tube_d([top, knee, ankle], [0.098, 0.080, 0.092]), c["pants"], c["pants_s"],
              off=(-0.014 * s if s < 0 else -0.012, -0.004), cut=False)
    for i, s in enumerate((-1, 1)):
        foot = P(j.feet[i])
        ang = -s * j.toes[i] * 0.5
        dd, tr = rrect_d(foot[0] + s * 0.020, foot[1] - 0.010, 0.128, 0.022, 0.011, ang)
        d.path(dd, c["sole"], cut=False, transform=tr)
        up = [(-0.058, 0.0), (-0.052, -0.034), (-0.020, -0.050), (0.030, -0.040), (0.062, -0.012), (0.060, 0.0)]
        up = [(x * s, y) for x, y in up]
        base = add(foot, (s * 0.020, -0.020))
        pts = [add(base, rot(p, ang)) for p in up]
        d.cel(curve_d(pts, closed=True), c["shoe"], c["shoe_s"], off=(-0.006, -0.008), cut=False)
        tab = add(base, rot((-s * 0.050, -0.030), ang))
        d.circle(tab, 0.011, c["gold"], cut=False)
    # the torso: an oversized hoodie with a ribbed hem, a pocket, the hood round the neck
    T = PROPS.torso
    L = _local(P(j.neck), j.torso_deg)
    hl = T + 0.035
    body = [L(-0.030, -0.012), L(-0.088, 0.010), L(-0.112, 0.050), L(-0.108, 0.12),
            L(-0.096, 0.20), L(-0.100, hl), L(0.100, hl), L(0.096, 0.20), L(0.108, 0.12),
            L(0.112, 0.050), L(0.088, 0.010), L(0.030, -0.012)]
    d.ellipse(L(0, -0.002), 0.066, 0.030, c["hood_d"], deg=j.torso_deg)
    d.cel(curve_d(body, closed=True), c["hood"], c["hood_s"], off=(-0.016, -0.012), cut=False)
    hem = [L(-0.099, hl - 0.030), L(0.099, hl - 0.030), L(0.100, hl), L(-0.100, hl)]
    d.path("M" + " L".join(f"{f(x)},{f(y)}" for x, y in hem) + " Z", c["hood_s"], cut=False)
    pocket = [L(-0.058, T - 0.080), L(0.058, T - 0.080), L(0.074, T - 0.004), L(-0.074, T - 0.004)]
    d.path(curve_d(pocket, closed=True), c["hood_s"], cut=False)
    # the hood lying round the neck, the strings
    hood = [L(-0.060, -0.010), L(-0.030, 0.034), L(0.0, 0.044), L(0.030, 0.034), L(0.060, -0.010),
            L(0.030, 0.006), L(0.0, 0.010), L(-0.030, 0.006)]
    d.path(curve_d(hood, closed=True), c["hood_d"], cut=False)
    for s in (-1, 1):
        sway = pose.chest[0] * 0.3 + math.sin(pose.accent * 2) * 0.004 * s
        d.line([L(s * 0.020, 0.030), L(s * 0.024 + sway, 0.080), L(s * 0.022 + sway * 1.6, 0.112)],
               0.008, c["string"], cut=False, smooth=True)
    # neck and head
    hc = P(j.head)
    Hraw = _local(hc, j.head_deg)
    H = lambda x, y: Hraw(x * HS, y * HS - 0.012)
    d.path(tube_d([L(0, 0.0), H(0, 0.04)], [0.044, 0.044]), c["skin_s"], cut=False)
    _head(d, H, pose, c, face)
    # arms: vermilion sleeves (cut from the hoodie where they cross it), skin hands
    for i, s in enumerate((-1, 1)):
        sh, el, ha = P(j.shoulders[i]), P(j.elbows[i]), P(j.hands[i])
        sh = add(sh, mul(sub(P(j.neck), sh), 0.18))
        fdir = sub(ha, el)
        fl = length(fdir) or 1e-6
        fdir = mul(fdir, 1 / fl)
        wrist = sub(ha, mul(fdir, 0.012))
        free = add(sh, mul(sub(el, sh), 0.6))
        d.line([free, el, wrist], 0.064 + 2 * CUT, room, cut=False)
        d.cel(tube_d([sh, el, wrist], [0.072, 0.060, 0.054]), c["hood"], c["hood_s"],
              off=(-0.010, -0.009), cut=False)
        cuff = sub(wrist, mul(fdir, 0.026))
        d.path(tube_d([cuff, wrist], [0.056, 0.054]), c["hood_s"], cut=False)
        d.ellipse(add(ha, mul(fdir, 0.018)), 0.029, 0.032, c["skin"], deg=math.degrees(math.atan2(fdir[1], fdir[0])))
    return d.svg()


def _head(d, H, pose, c, face):
    # head: an oval with a jaw, two values
    head = [H(-0.062, -0.010), H(-0.060, -0.050), H(-0.035, -0.080), H(0.0, -0.086),
            H(0.035, -0.080), H(0.060, -0.050), H(0.062, -0.010), H(0.050, 0.035),
            H(0.022, 0.066), H(0.0, 0.070), H(-0.022, 0.066), H(-0.050, 0.035)]
    d.cel(curve_d(head, closed=True), c["skin"], c["skin_s"], off=(-0.012, -0.004), cut=False)
    # hair: short curls round the head, a soft bumpy crown that bounces late
    bob = -0.006 * pose.accent
    crown = []
    for n in range(21):
        a = math.radians(176 + n * 188 / 20)
        r = 0.090 + (0.010 if n % 2 else 0.0)
        crown.append(H(math.cos(a) * r * 0.92, math.sin(a) * r + bob - 0.010))
    crown += [H(0.072, 0.0), H(0.062, -0.006), H(0.044, -0.040), H(0.0, -0.050),
              H(-0.044, -0.040), H(-0.062, -0.006), H(-0.072, 0.0)]
    d.cel(curve_d(crown, closed=True), c["hair_l"], c["hair"], off=(-0.008, -0.010), cut=False)
    for (x, y, a0) in ((-0.040, -0.062, 200), (0.004, -0.074, 210), (-0.016, -0.034, 190), (0.040, -0.050, 215)):
        pts = [H(x + 0.014 * math.cos(math.radians(a0 + t)), y + 0.012 * math.sin(math.radians(a0 + t)) + bob)
               for t in (0, 60, 120)]
        d.line(pts, 0.006, c["hair"], cut=False, smooth=True)
    if face:
        _face(d, H, pose, c)
    # gold headphones: the band over the hair, the cups over the ears
    band = [H(0.080 * math.cos(math.radians(a)), 0.080 * math.sin(math.radians(a)) - 0.016)
            for a in range(195, 346, 15)]
    d.line(band, 0.013 * HS, c["gold_s"], cut=False, smooth=True)
    d.line([add(p, (-0.002, -0.003)) for p in band], 0.008 * HS, c["gold"], cut=False, smooth=True)
    for s in (-1, 1):
        cx, cy = H(s * 0.066, 0.008)
        dd, tr = rrect_d(cx, cy, 0.028 * HS, 0.046 * HS, 0.012 * HS, 0)
        d.cel(dd, c["gold"], c["gold_s"], off=(-0.006, -0.006), cut=False, transform=tr)


def _face(d, H, pose, c):
    e = pose.eyes
    for s in (-1, 1):
        ex, ey = H(s * 0.026, 0.004)
        if e == "happy":
            d.line([H(s * 0.026 - 0.011, 0.008), H(s * 0.026, -0.002), H(s * 0.026 + 0.011, 0.008)],
                   0.006, c["eye"], cut=False, smooth=True)
        elif e == "closed":
            d.line([H(s * 0.026 - 0.011, 0.002), H(s * 0.026, 0.008), H(s * 0.026 + 0.011, 0.002)],
                   0.006, c["eye"], cut=False, smooth=True)
        elif e != "cool":
            big = 1.3 if e == "wide" else 1.0
            d.ellipse((ex, ey), 0.0085 * big, 0.012 * big, c["eye"])
        # brows
        lift = 0.006 if e == "wide" else 0.0
        d.line([H(s * 0.014, -0.020 - lift), H(s * 0.036, -0.024 - lift)], 0.006, c["hair"], cut=False)
    if e == "cool":
        for s in (-1, 1):
            cx, cy = H(s * 0.026, 0.002)
            dd, tr = rrect_d(cx, cy, 0.040, 0.024, 0.009, 0)
            d.path(dd, c["glass"], cut=False, transform=tr)
        d.line([H(-0.008, -0.002), H(0.008, -0.002)], 0.006, c["glass"], cut=False)
        d.line([H(-0.040, -0.004), H(-0.030, 0.006)], 0.004, c["gold"], cut=False)
    # nose and mouth
    d.line([H(0.004, 0.018), H(0.008, 0.030)], 0.006, c["skin_s"], cut=False)
    if pose.mouth > 0.25:
        mx, my = H(0.0, 0.046)
        d.ellipse((mx, my), 0.014, 0.006 + 0.010 * pose.mouth, c["lip"])
    else:
        d.line([H(-0.013, 0.042), H(0.0, 0.048), H(0.013, 0.042)], 0.0055, c["lip"], cut=False, smooth=True)


def key_poses():
    from moves import groove, steps_human, jump
    from rig import Pose
    return [
        ("Idle", Pose(eyes="open", hip=(0.02, 0.01), lean=-2, tilt=-3)),
        ("Clap", groove(0.0, STYLE)),
        ("Snap", steps_human(1.0, STYLE)),
        ("Sing", groove(1.0, STYLE)),
        ("Crouch", jump(0.95, STYLE)),
        ("Star jump", jump(1.16, STYLE)),
        ("Point", Pose(hip=(-0.07, 0.03), lean=6, hands=((-0.44, 0.42), (0.62, -0.95)),
                       feet=((0, 0), (0.14, 0)), toe=(0, 14), tilt=-8, nod=-6, eyes="cool")),
        ("Lean back", Pose(hip=(0.05, 0.03), lean=-8, chest=(-0.04, 0), hands=((-0.42, 0.40), (0.45, 0.42)),
                           feet=((-0.12, 0), (0.08, 0)), tilt=-10, eyes="happy", mouth=0.0)),
        ("Wave", Pose(eyes="happy", hands=((-0.40, 0.92), (0.70, -0.70)), lean=-4, tilt=-6)),
    ]
