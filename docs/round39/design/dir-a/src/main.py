"""Slate (dir-a) — composes the whole window and writes the HTML pages to render.

python -I main.py  →  ../build/*.html   (then render.sh makes the PNGs)
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from base import NIGHT, PAPER, defs, rect, page, TOP_H, INSP_X, MIX_Y, STATUS_Y  # noqa: E402
import topbar  # noqa: E402
import arrange  # noqa: E402

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'build')

IDLE = dict(playing=False, transport='stopped', loop=True, pos=('41', '1', '000'), time='1:17.42', head=41.0,
            master=None)
PLAYING = dict(playing=True, transport='playing', cpu=11, loop=True, pos=('44', '3', '112'), time='1:24.68', head=44.55,
               master=(0.83, 0.80), meters=True)


def window(t, state):
    parts = [defs(t), rect(0, 0, 1280, 720, t['gutter'])]
    import topbar2
    bar_svg, forms, more_x = topbar2.bar(t, 1280, state)
    parts.append(bar_svg)
    parts.append(arrange.arrangement(t, state))
    try:
        import mixer
        parts.append(mixer.mixer(t, state))
    except ImportError:
        pass
    try:
        import inspector
        parts.append(inspector.inspector(t, state))
        parts.append(inspector.status(t, state))
    except ImportError:
        pass
    if state.get('overflow_open'):
        import overflow2
        parts.append(overflow2.panel(t, forms, more_x + 30, state)[0])
    return ''.join(parts)


def svg(t, state, w=1280, h=720):
    body = window(t, state)
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" '
            f'viewBox="0 0 1280 720">{body}</svg>')


def write(name, html):
    os.makedirs(OUT, exist_ok=True)
    with open(os.path.join(OUT, name), 'w', encoding='utf-8') as f:
        f.write(html)


if __name__ == '__main__':
    import glyphs as G
    for scale, tag in ((1.0, ''), (1.5, '@15')):
        G.SCALE[0] = scale
        for t in (NIGHT, PAPER):
            k = t['name'].lower()
            write(f'window-{k}{tag}.html', page(svg(t, IDLE), 1280, 720, t['gutter']))
            write(f'window-{k}-playing{tag}.html', page(svg(t, PLAYING), 1280, 720, t['gutter']))
            write(f'overflow-{k}{tag}.html', page(svg(t, dict(IDLE, overflow_open=True)), 1280, 720, t['gutter']))
    G.SCALE[0] = 1.0
    print('ok')
