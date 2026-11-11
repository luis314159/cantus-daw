"""des-mixer — cut the 1:1 renders and enlarge them x4 with nearest-neighbour, so the real
pixels are judged (no resampling hides a soft edge).

python zoom.py <design dir>
"""
import os
import sys
from PIL import Image

# (source stem, box in 1x px, cut name)
CUTS = [
    ('mixer-wide-{t}-1x', (392, 74, 520, 284), 'wide-top'),          # Chords + Pluck: head, inserts, EQ, keys
    ('mixer-wide-{t}-1x', (392, 284, 520, 470), 'wide-fader'),       # the fader, the meter, the scale
    ('mixer-wide-{t}-1x', (0, 590, 520, 700), 'wide-bottom'),        # level, sends, output, the cable band
    ('mixer-wide-{t}-1x', (0, 74, 84, 380), 'master'),               # the master: scale, meter, mono
    ('mixer-compact-{t}-1x', (376, 74, 504, 300), 'compact-top'),    # rotated names, pips, keys, arm
    ('mixer-large-{t}-1x', (604, 74, 702, 380), 'large-top'),
    ('mixer-wide-{t}-1x', (1034, 46, 1280, 320), 'inspector-inserts'),
    ('states-{t}-1x', (186, 296, 696, 330), 'keys'),                 # clip, arm, mute, solo, silenced keys
    ('states-{t}-1x', (1006, 110, 1170, 290), 'vca-frozen'),
]


def main(d):
    out = os.path.join(d, 'zoom')
    os.makedirs(out, exist_ok=True)
    for th in ('night', 'paper'):
        for stem, box, name in CUTS:
            src = os.path.join(d, stem.format(t=th) + '.png')
            if not os.path.exists(src):
                continue
            im = Image.open(src).convert('RGB').crop(box)
            im = im.resize((im.width * 4, im.height * 4), Image.NEAREST)
            p = os.path.join(out, f'{name}-{th}-4x.png')
            im.save(p)
            print(' ', p, im.size)


if __name__ == '__main__':
    main(sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))
