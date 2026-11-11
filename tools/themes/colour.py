"""Colour arithmetic as the kit does it (`crates/cantus-ui/src/kit/color.rs`): WCAG
luminance and contrast, HSL, egui's gamma-space mix, and `ensure_contrast_all`.

Colours are (r, g, b, a) tuples of 0..255 ints, unmultiplied alpha.
"""


def hx(v, a=255):
    """`0xrrggbb` (an int) or '#rrggbb' as a colour."""
    if isinstance(v, str):
        v = int(v.lstrip("#"), 16)
    return ((v >> 16) & 255, (v >> 8) & 255, v & 255, a)


def to_hex(c):
    return "%02x%02x%02x" % c[:3]


def lum(c):
    def ch(v):
        v = v / 255.0
        return v / 12.92 if v <= 0.03928 else ((v + 0.055) / 1.055) ** 2.4

    return 0.2126 * ch(c[0]) + 0.7152 * ch(c[1]) + 0.0722 * ch(c[2])


def contrast(a, b):
    la, lb = lum(a), lum(b)
    hi, lo = max(la, lb), min(la, lb)
    return (hi + 0.05) / (lo + 0.05)


def hsl(c):
    r, g, b = c[0] / 255.0, c[1] / 255.0, c[2] / 255.0
    mx, mn = max(r, g, b), min(r, g, b)
    l = (mx + mn) / 2
    d = mx - mn
    if d < 1e-6:
        return (0.0, 0.0, l)
    s = d / (2 - mx - mn) if l > 0.5 else d / (mx + mn)
    if mx == r:
        h = (g - b) / d + (6 if g < b else 0)
    elif mx == g:
        h = (b - r) / d + 2
    else:
        h = (r - g) / d + 4
    return (h * 60.0, s, l)


def rgb(h, s, l, a=255):
    h = (h % 360.0) / 360.0
    s = min(max(s, 0.0), 1.0)
    l = min(max(l, 0.0), 1.0)
    if s <= 0:
        v = int(round(l * 255))
        return (v, v, v, a)
    q = l * (1 + s) if l < 0.5 else l + s - l * s
    p = 2 * l - q

    def ch(t):
        if t < 0:
            t += 1
        if t > 1:
            t -= 1
        if t < 1 / 6:
            v = p + (q - p) * 6 * t
        elif t < 0.5:
            v = q
        elif t < 2 / 3:
            v = p + (q - p) * (2 / 3 - t) * 6
        else:
            v = p
        return int(round(v * 255))

    return (ch(h + 1 / 3), ch(h), ch(h - 1 / 3), a)


def mix(a, b, t):
    """egui's `lerp_to_gamma` (opaque colours)."""
    t = min(max(t, 0.0), 1.0)
    import math

    return tuple(int(math.floor(x + (y - x) * t + 0.5)) for x, y in zip(a[:3], b[:3])) + (255,)


def alpha(c, a):
    return c[:3] + (a,)


def ensure_all(fg, bgs, need):
    """`kit::color::ensure_contrast_all`: lightness moved 1 % at a time."""
    worst = lambda c: min(contrast(c, b) for b in bgs)
    if worst(fg) >= need:
        return fg
    h, s, l = hsl(fg)
    mean = sum(lum(b) for b in bgs) / len(bgs)
    first_up = lum(fg) >= mean
    best, score = fg, worst(fg)
    for up in (first_up, not first_up):
        ll = l
        for _ in range(101):
            ll = min(max(ll + (0.01 if up else -0.01), 0.0), 1.0)
            c = rgb(h, s, ll, fg[3])
            w = worst(c)
            if w >= need:
                return c
            if w > score:
                best, score = c, w
            if ll <= 0 or ll >= 1:
                break
    return best


def at_lum(h, s, target):
    """The colour of hue `h` and saturation `s` whose luminance is `target`."""
    lo, hi = 0.0, 1.0
    for _ in range(60):
        m = (lo + hi) / 2
        if lum(rgb(h, s, m)) < target:
            lo = m
        else:
            hi = m
    best = None
    for l in (lo - 0.004, lo, hi, hi + 0.004):
        c = rgb(h, s, l)
        if best is None or abs(lum(c) - target) < abs(lum(best) - target):
            best = c
    return best


def turn(c, hue, gain, f=1.0, sat=None):
    """`c` turned to `hue` with its saturation × `gain` (or `sat`), at its luminance
    scaled in WCAG's terms by `f`: (L + 0.05) × f − 0.05, which keeps every contrast
    ratio between two colours turned with the same `f`."""
    l = lum(c)
    target = min(max((l + 0.05) * f - 0.05, 0.0), 1.0)
    s0 = hsl(c)[1]
    s = sat if sat is not None else min(s0 * gain, 0.95)
    out = at_lum(hue, s, target)
    return out[:3] + (c[3],)
