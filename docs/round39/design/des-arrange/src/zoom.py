"""des-arrange — 4x nearest-neighbour cuts of the 1:1 renders, so the real pixels can be judged.

python zoom.py <design dir>
"""
import os
import sys
from PIL import Image

CUTS = [
    # (source, box at 1:1, name)
    ('headers-{th}-1x.png', (24, 98, 560, 246), 'x4/headers-{th}-4x-states.png'),          # rest, pointer, selected
    ('headers-{th}-1x.png', (24, 258, 560, 422), 'x4/headers-{th}-4x-mute-solo.png'),      # muted, soloed, silenced
    ('headers-{th}-1x.png', (24, 426, 560, 690), 'x4/headers-{th}-4x-arm-frozen-lanes.png'),
    ('headers-{th}-1x.png', (24, 724, 536, 786), 'x4/headers-{th}-4x-keys.png'),
    ('headers-{th}-1x.png', (24, 832, 530, 1006), 'x4/headers-{th}-4x-folder.png'),
    ('ruler-{th}-1x.png', (24, 96, 640, 272), 'x4/ruler-{th}-4x-corner-lanes.png'),
    ('ruler-{th}-1x.png', (640, 96, 1256, 182), 'x4/ruler-{th}-4x-loop-playhead.png'),
    ('canvas-{th}-1x.png', (24, 870, 628, 1046), 'x4/canvas-{th}-4x-selection.png'),
    ('canvas-{th}-1x.png', (24, 1384, 760, 1600), 'x4/canvas-{th}-4x-automation.png'),
    ('window-{th}-1x.png', (0, 46, 560, 340), 'x4/window-{th}-4x-corner.png'),
]


def main(d):
    os.makedirs(os.path.join(d, 'x4'), exist_ok=True)
    for th in ('night', 'paper'):
        for src, box, out in CUTS:
            p = os.path.join(d, src.format(th=th))
            if not os.path.exists(p):
                continue
            im = Image.open(p).convert('RGB').crop(box)
            im.resize((im.width * 4, im.height * 4), Image.NEAREST).save(os.path.join(d, out.format(th=th)))
    print('zoomed')


if __name__ == '__main__':
    main(sys.argv[1])
