"""des-arrange — writes every page to ../build/*.html (render_all.sh makes the PNGs).

python -I main.py [only]
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from base import NIGHT, PAPER, page  # noqa: E402
import win  # noqa: E402

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'build')
PAGES = []      # (name, css w, css h, scale, png name)


def write(name, html, w, h, scale, png):
    os.makedirs(OUT, exist_ok=True)
    with open(os.path.join(OUT, name + '.html'), 'w', encoding='utf-8') as f:
        f.write(html)
    PAGES.append((name, w, h, scale, png))


def windows():
    for t in (NIGHT, PAPER):
        k = t['name'].lower()
        # 1920 × 1080 at 100 % and at 125 % (1536 × 864 logical): both 1920 × 1080 physical
        for W, H, s, tag, b0 in ((1920, 1080, 1, '1x', 33.0), (1536, 864, 1.25, '1.25x', 35.0)):
            body = win.window(t, W, H, playing=False, bar0=b0)
            write(f'window-{k}-{tag}', page(win.svg(body, W, H), W, H, t['gutter']), W, H, s, f'window-{k}-{tag}.png')
        for W, H, s, tag in ((1920, 1080, 1, '1x'), (1536, 864, 1.25, '1.25x')):
            body = win.window(t, W, H, playing=False, docked=False, bar0=27.0,
                              view_kw=dict(tempo=True, sel_clips=[('Chords', 4)], edited=('Chords', 4)),
                              hint=('Chords · Stabs', 'open in the piano roll · drag to move · Alt-drag to copy'))
            write(f'max-{k}-{tag}', page(win.svg(body, W, H), W, H, t['gutter']), W, H, s, f'max-{k}-{tag}.png')
        body = win.window(NIGHT if k == 'night' else PAPER, 1536, 864, playing=True, bar0=35.0,
                          hint=('Playing · bar 44', 'Space to stop · the clips under the playhead light a step'))
        write(f'window-{k}-playing', page(win.svg(body, 1536, 864), 1536, 864, t['gutter']), 1536, 864, 1.25,
              f'window-{k}-playing-1.25x.png')


def sheets(only):
    import importlib
    for mod, name in (('sheet_headers', 'headers'), ('sheet_ruler', 'ruler'), ('sheet_canvas', 'canvas')):
        if only and only not in name and not name.startswith(only.split('-')[0]):
            continue
        m = importlib.import_module(mod)
        for t in (NIGHT, PAPER):
            k = t['name'].lower()
            body, bottom = m.sheet(t)
            Hh = int(bottom)
            W = m.W
            html = page(f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{Hh}" viewBox="0 0 {W} {Hh}">{body}</svg>',
                        W, Hh, t['gutter'])
            write(f'{name}-{k}', html, W, Hh, 1, f'{name}-{k}-1x.png')
            write(f'{name}-{k}@125', html, W, Hh, 1.25, f'{name}-{k}-1.25x.png')


if __name__ == '__main__':
    only = sys.argv[1] if len(sys.argv) > 1 else ''
    if not only or only.startswith(('window', 'max')):
        windows()
    if not only or not only.startswith(('window', 'max')):
        sheets(only)
    with open(os.path.join(OUT, 'pages.txt'), 'w', encoding='utf-8', newline='\n') as f:
        for p in PAGES:
            if only in p[0]:
                f.write(' '.join(str(x) for x in p) + '\n')
    print(len(PAGES), 'pages')
