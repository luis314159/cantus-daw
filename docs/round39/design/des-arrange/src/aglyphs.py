"""des-arrange — the small glyphs the arrangement needs that the top bar's family does not
have, each drawn for the size it is shown at (10–12 px in a 16-px key or a 44-px row).
Placeholders in the icons agent's grammar (ink 2 body, ink 3 detail; strokes 1–1.2 px) until
that family is approved."""


def _p(d, ink, w=1.1, cap='round', join='round', extra=''):
    return (f'<path d="{d}" fill="none" stroke="{ink}" stroke-width="{w}" stroke-linecap="{cap}" '
            f'stroke-linejoin="{join}"{extra}/>')


def folder(cx, cy, ink, ink2, open_=True):
    """A folder 12 × 9: the tab on the left; open = the front leaf tilted down."""
    x, y = cx - 6, cy - 4.5
    back = f'M{x + 0.5} {y + 8.5}V{y + 0.5}H{x + 4.2}L{x + 5.4} {y + 1.9}H{x + 11.5}V{y + 3}'
    if open_:
        front = f'<path d="M{x + 0.5} {y + 8.5}L{x + 2.4} {y + 3.6}H{x + 12}L{x + 10.1} {y + 8.5}Z" fill="{ink2}"/>'
    else:
        front = f'<rect x="{x + 0.5}" y="{y + 3}" width="11" height="5.5" rx="0.5" fill="{ink2}"/>'
    return _p(back, ink, 1) + front


def auto(cx, cy, ink, w=1.1):
    """Automation: an envelope that holds, ramps and holds, with its two breakpoints (11 × 7)."""
    d = f'M{cx - 5.5} {cy + 3}H{cx - 2.2}L{cx + 2.2} {cy - 3}H{cx + 5.5}'
    return (_p(d, ink, w) + f'<circle cx="{cx - 2.2}" cy="{cy + 3}" r="1.5" fill="{ink}"/>'
            f'<circle cx="{cx + 2.2}" cy="{cy - 3}" r="1.5" fill="{ink}"/>')


def snow(cx, cy, ink, s=4.5):
    """Frozen: three strokes and their barbs."""
    out = []
    for a in (0, 60, 120):
        out.append(f'<g transform="rotate({a} {cx} {cy})">' +
                   _p(f'M{cx} {cy - s}V{cy + s}M{cx - 1.6} {cy - s + 0.6}L{cx} {cy - s + 2}L{cx + 1.6} {cy - s + 0.6}'
                      f'M{cx - 1.6} {cy + s - 0.6}L{cx} {cy + s - 2}L{cx + 1.6} {cy + s - 0.6}', ink, 1) + '</g>')
    return ''.join(out)


def more(cx, cy, ink):
    return ''.join(f'<circle cx="{cx + d}" cy="{cy}" r="1.1" fill="{ink}"/>' for d in (-4, 0, 4))


def lanes(cx, cy, ink, ink2):
    """The top lanes' menu: three rows, the middle one short."""
    return (f'<rect x="{cx - 5}" y="{cy - 4}" width="10" height="1.4" rx="0.7" fill="{ink}"/>'
            f'<rect x="{cx - 5}" y="{cy - 0.7}" width="6" height="1.4" rx="0.7" fill="{ink2}"/>'
            f'<rect x="{cx - 5}" y="{cy + 2.6}" width="10" height="1.4" rx="0.7" fill="{ink}"/>')


def keys_kind(cx, cy, ink, ink2):
    """An instrument: three white keys and two black ones (10 × 8)."""
    out = [f'<rect x="{cx - 5}" y="{cy - 4}" width="10" height="8" rx="1" fill="none" stroke="{ink2}" stroke-width="1"/>']
    out += [f'<rect x="{cx - 1.8 + d}" y="{cy - 4}" width="1" height="8" fill="{ink2}"/>' for d in (0, 3.3)]
    out += [f'<rect x="{cx - 3 + d}" y="{cy - 4}" width="2" height="4.6" fill="{ink}"/>' for d in (0, 4)]
    return ''.join(out)


def wave_kind(cx, cy, ink, ink2):
    hs = [1.2, 2.6, 4, 2.2, 3.2, 1.4]
    return ''.join(f'<rect x="{cx - 5.5 + 2 * k}" y="{cy - h:.1f}" width="1" height="{2 * h:.1f}" rx="0.5" fill="{ink if k % 2 else ink2}"/>'
                   for k, h in enumerate(hs))


def pads_kind(cx, cy, ink, ink2):
    """A drum kit: four pads, one lit."""
    out = []
    for i, (dx, dy) in enumerate(((-3, -2.2), (2, -2.2), (-3, 2.6), (2, 2.6))):
        out.append(f'<rect x="{cx + dx - 2:.1f}" y="{cy + dy - 2:.1f}" width="4" height="3.6" rx="0.8" '
                   f'fill="{ink if i == 0 else "none"}" stroke="{ink2 if i else ink}" stroke-width="{0 if i == 0 else 0.9}"/>')
    return ''.join(out)


def mic(cx, cy, ink, ink2):
    return (f'<rect x="{cx - 2}" y="{cy - 5}" width="4" height="6.4" rx="2" fill="{ink}"/>'
            + _p(f'M{cx - 3.6} {cy - 0.6}A3.6 3.6 0 0 0 {cx + 3.6} {cy - 0.6}M{cx} {cy + 3}V{cy + 5}M{cx - 2} {cy + 5}H{cx + 2}', ink2, 1))


def pencil_roll(cx, cy, ink):
    """'Open in the piano roll': three notes in a box (9 × 7)."""
    return (f'<rect x="{cx - 4.5}" y="{cy - 3.5}" width="9" height="7" rx="1.2" fill="none" stroke="{ink}" stroke-width="1"/>'
            f'<rect x="{cx - 3}" y="{cy - 1.8}" width="3" height="1.2" fill="{ink}"/>'
            f'<rect x="{cx - 0.5}" y="{cy - 0.2}" width="2.6" height="1.2" fill="{ink}"/>'
            f'<rect x="{cx + 1}" y="{cy + 1.4}" width="2" height="1.2" fill="{ink}"/>')


def marker(cx, top, ink):
    """A marker's pennant: hangs from its line, 6 × 7."""
    return f'<path d="M{cx} {top}H{cx + 6}L{cx + 4} {top + 3.5}L{cx + 6} {top + 7}H{cx}Z" fill="{ink}"/>'


def link(cx, cy, ink):
    return (f'<circle cx="{cx - 1.8}" cy="{cy}" r="2.2" fill="none" stroke="{ink}" stroke-width="1"/>'
            f'<circle cx="{cx + 1.8}" cy="{cy}" r="2.2" fill="none" stroke="{ink}" stroke-width="1"/>')


def grip_h(cx, cy, ink):
    """A resize grip across a row's foot: three dots."""
    return ''.join(f'<circle cx="{cx + d}" cy="{cy}" r="0.9" fill="{ink}"/>' for d in (-4, 0, 4))


def grip_v(cx, cy, ink):
    return ''.join(f'<circle cx="{cx}" cy="{cy + d}" r="0.9" fill="{ink}"/>' for d in (-4, 0, 4))


def close(cx, cy, ink, s=3):
    return _p(f'M{cx - s} {cy - s}L{cx + s} {cy + s}M{cx + s} {cy - s}L{cx - s} {cy + s}', ink, 1.1)


def cursor_lr(cx, cy, ink, bg):
    """The pointer over a clip's edge: a double arrow with a bar (resize)."""
    d = (f'M{cx - 7} {cy}L{cx - 3.5} {cy - 3.5}V{cy - 1.2}H{cx - 1}V{cy - 5}H{cx + 1}V{cy - 1.2}H{cx + 3.5}V{cy - 3.5}'
         f'L{cx + 7} {cy}L{cx + 3.5} {cy + 3.5}V{cy + 1.2}H{cx + 1}V{cy + 5}H{cx - 1}V{cy + 1.2}H{cx - 3.5}V{cy + 3.5}Z')
    return f'<path d="{d}" fill="{ink}" stroke="{bg}" stroke-width="1" stroke-linejoin="round"/>'


def cursor_ud(cx, cy, ink, bg):
    return f'<g transform="rotate(90 {cx} {cy})">{cursor_lr(cx, cy, ink, bg)}</g>'


def cursor_hand(cx, cy, ink, bg):
    """The pointer moving a clip: four arrows."""
    d = (f'M{cx} {cy - 7}L{cx + 2.6} {cy - 4.2}H{cx + 0.9}V{cy - 0.9}H{cx + 4.2}V{cy - 2.6}L{cx + 7} {cy}'
         f'L{cx + 4.2} {cy + 2.6}V{cy + 0.9}H{cx + 0.9}V{cy + 4.2}H{cx + 2.6}L{cx} {cy + 7}L{cx - 2.6} {cy + 4.2}'
         f'H{cx - 0.9}V{cy + 0.9}H{cx - 4.2}V{cy + 2.6}L{cx - 7} {cy}L{cx - 4.2} {cy - 2.6}V{cy - 0.9}H{cx - 0.9}'
         f'V{cy - 4.2}H{cx - 2.6}Z')
    return f'<path d="{d}" fill="{ink}" stroke="{bg}" stroke-width="1" stroke-linejoin="round"/>'
