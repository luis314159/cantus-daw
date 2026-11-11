"""Slate (dir-a) — cut the 1:1 parts sheet and enlarge by 4 with nearest-neighbour, so the
pixels are judged as they are (no resampling hides a soft edge).

python zoom.py <parts-1x.png> <out-prefix>
"""
import sys
from PIL import Image

CROPS = {
    'transport': (130, 120, 452, 362),
    'tools': (452, 120, 948, 362),
    'windows': (948, 120, 1260, 362),
    'knobs-fader': (24, 410, 520, 870),
}


def main(src, prefix):
    im = Image.open(src).convert('RGB')
    for name, box in CROPS.items():
        c = im.crop(box)
        c = c.resize((c.width * 4, c.height * 4), Image.NEAREST)
        c.save(f'{prefix}-4x-{name}.png')
        print(f'{prefix}-4x-{name}.png', c.size)


if __name__ == '__main__':
    main(sys.argv[1], sys.argv[2])
