"""Slate (dir-a), phase B — 4× nearest-neighbour cuts of the build sheets, so the owner
judges the real pixels: the top bar's groups, the mark, the toolbar's states.

python zoom_b.py <dir of b-*.png>
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from PIL import Image  # noqa: E402

import sheets_b as S  # noqa: E402
import topbar2 as TB  # noqa: E402
from base import tw  # noqa: E402


def cut(src, box, out):
    im = Image.open(src).convert('RGB').crop(box)
    im.resize((im.width * 4, im.height * 4), Image.NEAREST).save(out)


def main(d):
    for th in ('night', 'paper'):
        src = os.path.join(d, f'b-topbar-groups-{th}-1x.png')
        y = 74
        for g, nm, states, spec in S.GROUPS:
            rowh = max(78, 30 + 15 * len(spec))
            if g in ('transport', 'tempo', 'meter', 'perf', 'device', 'more', 'brand'):
                x1 = 560
                for form, st, cap in states:
                    w = TB.width(g, form, dict(S.IDLE, **st))
                    x1 += max(w, tw(cap, 10) + 24) + 28
                cut(src, (552, y, min(1600, int(x1)), y + 50), os.path.join(d, f'b-topbar-groups-{th}-4x-{g}.png'))
            y += rowh + 18
        cut(os.path.join(d, f'b-mark-{th}-1x.png'), (24, 78, 736, 148), os.path.join(d, f'b-mark-{th}-4x.png'))
        ty = 78 + 6 * 70 + 10 + 16
        cut(os.path.join(d, f'b-toolbar-{th}-1x.png'), (24, ty, 560, ty + 28), os.path.join(d, f'b-toolbar-{th}-4x-tools.png'))
    print('zoomed')


if __name__ == '__main__':
    main(sys.argv[1])
