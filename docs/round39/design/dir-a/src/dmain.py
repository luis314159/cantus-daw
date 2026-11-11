"""Slate (dir-a), phase B — the dialogs over the window, Night and Paper, 1× and 150 %.

python -I dmain.py  →  ../build/dlg-*.html
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import glyphs as G  # noqa: E402
import main as M  # noqa: E402
import dialogs as D  # noqa: E402
from base import NIGHT, PAPER, page  # noqa: E402

PAGES = {
    'settings-audio': lambda t: D.settings(t, 'Audio'),
    'settings-look': lambda t: D.settings(t, 'Appearance'),
}
try:
    import dialogs2 as D2  # noqa: E402
    PAGES['firstrun'] = D2.firstrun
    PAGES['export'] = lambda t: D2.export(t, False)
    PAGES['export-progress'] = lambda t: D2.export(t, True)
except ImportError:
    pass


def doc(t, fn):
    body = M.window(t, M.IDLE) + fn(t)
    return f'<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="720" viewBox="0 0 1280 720">{body}</svg>'


if __name__ == '__main__':
    for scale, tag in ((1.0, ''), (1.5, '@15')):
        G.SCALE[0] = scale
        for t in (NIGHT, PAPER):
            for name, fn in PAGES.items():
                M.write(f'dlg-{name}-{t["name"].lower()}{tag}.html', page(doc(t, fn), 1280, 720, t['gutter']))
    G.SCALE[0] = 1.0
    print('ok', list(PAGES))
