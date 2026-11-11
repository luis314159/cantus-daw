"""des-mixer — writes every page to ../build (render_all.sh turns them into PNGs).

python -I main.py [name ...]
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from base import NIGHT, PAPER, page  # noqa: E402
import window as W  # noqa: E402

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'build')


def write(name, html):
    os.makedirs(OUT, exist_ok=True)
    with open(os.path.join(OUT, name + '.html'), 'w', encoding='utf-8') as f:
        f.write(html)


def windows():
    for t in (NIGHT, PAPER):
        k = t['name'].lower()
        write(f'mixer-wide-{k}', page(W.svg(W.window(t, 'wide')), 1280, 720, t['gutter']))
        write(f'mixer-wide-{k}-idle', page(W.svg(W.window(t, 'wide', playing=False)), 1280, 720, t['gutter']))


if __name__ == '__main__':
    windows()
    try:
        import sheets
        sheets.main(write)
    except ImportError as e:
        if 'sheets' not in str(e):
            raise
    print('ok')
