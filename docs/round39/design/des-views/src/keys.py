"""des-views (round 39, Slate) — the piano keyboard, drawn from a blank sheet.

What makes it read as a piano (the owner's open item since round 37):
- the white keys are *ivory*, in both themes, and as wide as a real piano's: an octave's
  twelve rows are shared by seven white keys, C-D-E taking 5 rows (each 5/3 of a row) and
  F-G-A-B taking 7 (each 7/4). Only E|F and B|C fall on row lines, as on the instrument;
- the black keys are 0.60 of the keyboard's depth and exactly one row high, so a black key
  is about 0.58 of a white key's width — the real ratio;
- light comes from the player's side (the right, beside the grid): a soft fallboard shadow
  at the back (left), the black keys' top face inset from their sides, their front slope a
  step lighter;
- every C is named in the C key itself (C4, middle C, in ink 1 and SemiBold; the others in
  ink 3); at a tall zoom every white key carries its name, quieter.
"""
from base import rect, text, mix

NAMES = ['C', 'C♯', 'D', 'D♯', 'E', 'F', 'F♯', 'G', 'G♯', 'A', 'A♯', 'B']
BLACK = {1, 3, 6, 8, 10}
# white key -> (start, end) in rows from the octave's C
WHITE_SPAN = {0: (0, 5 / 3), 2: (5 / 3, 10 / 3), 4: (10 / 3, 5),
              5: (5, 5 + 7 / 4), 7: (5 + 7 / 4, 5 + 7 / 2), 9: (5 + 7 / 2, 5 + 21 / 4), 11: (5 + 21 / 4, 12)}


def name(p):
    return f'{NAMES[p % 12]}{p // 12 - 1}'


def is_black(p):
    return p % 12 in BLACK


def kb_colours(t):
    """The keyboard's own tokens: ivory and ebony, a little cooler on Night to sit in Slate."""
    if t['name'] == 'Night':
        return dict(ivory='#cfd5db', ivory_hi='#e2e6ea', ivory_sh='#aeb6be', sep='#59636e',
                    ebony='#161a1f', ebony_top='#262d34', ebony_front='#3a434c', front='#9ea7b0',
                    lab='#4a5664', lab_c='#17202a', bed='#11151a')
    return dict(ivory='#fbfcfd', ivory_hi='#ffffff', ivory_sh='#e3e8ec', sep='#a3adb8',
                ebony='#262c33', ebony_top='#39414a', ebony_front='#58626c', front='#d3dae0',
                lab='#6a7684', lab_c='#17202a', bed='#b9c3cc')


def keyboard(t, x, y_top, w, rh, top, n, lit=None, names='c', uid='kb'):
    """A keyboard column. Rows run from pitch `top` (first row, y_top) down `n` rows.

    lit: {pitch: kind} — 'hover' (the note under the pointer: a faint accent wash),
         'press' (auditioned by a click: accent), 'sound' (sounding while playing: the
         track's colour is passed as kind 'sound:#hex').
    names: 'c' (only the Cs), 'all' (every white key, the Cs stronger), 'none'.
    """
    K = kb_colours(t)
    bottom = top - n + 1
    y_bot = y_top + n * rh                     # bottom edge of the lowest row

    def row_y(p):                              # top edge of pitch p's row
        return y_top + (top - p) * rh

    lit = lit or {}
    out = [f'<clipPath id="{uid}"><rect x="{x}" y="{y_top}" width="{w}" height="{n * rh}"/></clipPath>',
           f'<g clip-path="url(#{uid})">', rect(x, y_top, w, n * rh, K['sep'])]

    def tint(base, kind, black):
        if kind == 'hover':
            return mix(base, t['accent'], 0.16 if not black else 0.30)
        if kind == 'press':
            return mix(base, t['accent'], 0.55 if not black else 0.70)
        if kind and kind.startswith('sound:'):
            return mix(base, kind[6:], 0.55 if not black else 0.75)
        return base

    # white keys, octave by octave (one more each side so cut keys draw whole)
    o0, o1 = (bottom // 12) - 1, (top // 12) + 1
    labels = []
    for o in range(o0, o1 + 1):
        c = o * 12
        cb = row_y(c) + rh                     # bottom edge of C's row = the octave's floor
        for k, (a, b) in WHITE_SPAN.items():
            p = c + k
            y1 = round(cb - b * rh)
            y2 = round(cb - a * rh)
            col = tint(K['ivory'], lit.get(p), False)
            # each white key a piece of its own: a 1-px gap under it (the bed shows through),
            # its front corners rounded, a lit top edge and a soft shade at its foot
            gap = 1
            out.append(rect(x - 3, y1, w + 2, y2 - y1 - gap, col, 2 if rh >= 10 else 1.5))
            out.append(rect(x, y1, w - 2, 1, K['ivory_hi'] if col == K['ivory'] else mix(col, '#ffffff', 0.3)))
            out.append(rect(x, y2 - gap - 1, w - 2, 1, mix(col, K['sep'], 0.28)))
            if (k == 0 and names != 'none') or (names == 'all'):
                big = rh >= 16
                is_c = k == 0
                size = 10 if rh < 16 else 11
                if is_c or big:
                    mid_c = p == 60
                    col_l = K['lab_c'] if (is_c and mid_c) else (K['lab'] if is_c else mix(K['lab'], K['ivory'], 0.35))
                    wt = 600 if mid_c else (500 if is_c else 400)
                    labels.append(text(x + w - 5, y2 - max(3, round((y2 - y1 - size * 0.72) / 2)), name(p), size, col_l, wt, 'end'))
    # the keys' depth: shaded at the back (under the fallboard), a touch of light at the front
    night = t['name'] == 'Night'
    out.append(f'<defs><linearGradient id="{uid}w" x1="0" y1="0" x2="1" y2="0">'
               f'<stop offset="0" stop-color="#000" stop-opacity="{0.20 if night else 0.12}"/>'
               f'<stop offset="0.10" stop-color="#000" stop-opacity="{0.07 if night else 0.04}"/>'
               f'<stop offset="0.55" stop-color="#000" stop-opacity="0"/>'
               f'<stop offset="0.92" stop-color="#fff" stop-opacity="{0.10 if night else 0.0}"/>'
               f'<stop offset="1" stop-color="#000" stop-opacity="{0.06 if night else 0.05}"/></linearGradient>'
               f'<linearGradient id="{uid}b" x1="0" y1="0" x2="1" y2="0">'
               f'<stop offset="0" stop-color="{K["ebony"]}"/><stop offset="0.62" stop-color="{K["ebony_top"]}"/>'
               f'<stop offset="1" stop-color="{K["ebony_front"]}"/></linearGradient></defs>')
    out.append(rect(x, y_top, w, n * rh, f'url(#{uid}w)'))
    # black keys: a body lit toward the front, a bevel of light on the upper side, a shade below
    bw = round(w * 0.60)
    for p in range(bottom - 1, top + 2):
        if not is_black(p):
            continue
        yy = row_y(p)
        kind = lit.get(p)
        if kind:
            fill = tint(K['ebony_top'], kind, True)
        else:
            fill = f'url(#{uid}b)'
        # its shadow on the white keys, toward the front and below: two soft steps
        sh = 0.26 if night else 0.18
        out.append(rect(x, yy + 1, bw + 2, rh + 1, '#000000', 2, f' fill-opacity="{sh * 0.45:.2f}"'))
        out.append(rect(x, yy + 0.5, bw + 1, rh + 0.5, '#000000', 1.5, f' fill-opacity="{sh:.2f}"'))
        out.append(rect(x - 2, yy, bw + 2, rh, K['ebony'] if not kind else tint(K['ebony'], kind, True), 1.5))
        out.append(rect(x, yy + 1, bw - 1, rh - 2, fill, 1))
        # a light edge on the upper side and on the front end: the key has a top and a face
        edge = mix(K['ebony_front'], '#ffffff', 0.12 if night else 0.18) if not kind else mix(fill, '#ffffff', 0.3)
        out.append(rect(x + 2, yy + 1, bw - 4, 1, edge))
        out.append(rect(x + bw - 2, yy + 1, 1, rh - 2, edge))
    out.extend(labels)
    out.append('</g>')
    # the bed the keys sit in: a 1-px edge toward the grid
    out.append(rect(x + w - 1, y_top, 1, n * rh, K['bed']))
    return ''.join(out)
