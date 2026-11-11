"""des-views — writes every page to ../build (render.sh turns them into PNGs).

python -I sheets.py [pianoroll|rack|clips|keys|browser ...]
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from base import NIGHT, PAPER, defs, page, rect  # noqa: E402

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'build')
IDLE = dict(playing=False, loop=True, pos=('41', '1', '000'), time='1:17.42', head=41.0, master=None)


def svg(body, w, h):
    return f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">{body}</svg>'


def write(name, body, w, h, bg):
    os.makedirs(OUT, exist_ok=True)
    with open(os.path.join(OUT, name + '.html'), 'w', encoding='utf-8') as f:
        f.write(page(svg(body, w, h), w, h, bg))


def rack_window(t):
    import topbar
    import browser
    import rack_panel
    import pianoroll
    out = [rect(0, 0, 1280, 720, t['gutter']), topbar.topbar(t, IDLE)]
    out.append(browser.panel(t, 0, 46, 288, 652))
    out.append(rack_panel.panel(t, 290, 46, 990, 652))
    out.append(pianoroll.status(t, ('Acoustic Kick hard 1 · 0.42 s',
                                    'click to hear · drag onto a channel, a pad or the arrangement · Enter loads it into the selected channel')))
    return ''.join(out)


def main(only=None):
    import pianoroll
    for t in (NIGHT, PAPER):
        k = t['name'].lower()
        if not only or 'pianoroll' in only:
            write(f'pianoroll-{k}', defs(t) + pianoroll.window(t, IDLE), 1280, 720, t['gutter'])
        if not only or 'rack' in only:
            write(f'rack-{k}', defs(t) + rack_window(t), 1280, 720, t['gutter'])
        if not only or 'clips' in only:
            import clips
            write(f'clips-{k}', defs(t) + clips.sheet(t), clips.W, clips.H, t['gutter'])
        if not only or 'keys' in only:
            import keysheet
            write(f'keys-{k}', defs(t) + keysheet.sheet(t), keysheet.W, keysheet.H, t['gutter'])
        if not only or 'browser' in only:
            import browser_pages
            write(f'browser-{k}', defs(t) + browser_pages.sheet(t), browser_pages.W, browser_pages.H, t['gutter'])
    print('ok')


if __name__ == '__main__':
    main(sys.argv[1:])
