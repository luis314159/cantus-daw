"""des-controls — cut each 100 % sheet into crops and enlarge them x4 with nearest-neighbour,
so every pixel is judged as it is (no resampling hides a soft edge).

python -I zoom.py            (all sheets, both themes) -> ../zoom/<sheet>-<theme>-4x-<crop>.png
"""
import os
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.normpath(os.path.join(HERE, '..'))
CROPS = {
    'knobs': {'unipolar': (100, 95, 560, 420), 'driven': (560, 95, 930, 420),
              'bipolar-stepped': (930, 95, 1230, 420), 'signs': (20, 480, 340, 650)},
    'faders': {'strips': (20, 100, 372, 372), 'strips-driven': (370, 100, 656, 372),
               'master': (664, 96, 882, 400), 'sliders': (20, 430, 540, 560), 'valuebars': (20, 580, 500, 700)},
    'fields': {'numeric': (150, 100, 640, 240), 'numeric-states': (640, 100, 1200, 240),
               'text': (20, 310, 620, 440), 'dropdowns': (20, 515, 520, 700)},
    'menus': {'menu': (20, 92, 480, 530), 'clip-menu': (560, 92, 1000, 380), 'knob-menu': (990, 92, 1262, 400),
              'overflow': (20, 570, 270, 830)},
    'switches': {'tabs': (20, 95, 600, 200), 'segmented': (20, 262, 700, 362), 'toggles': (20, 400, 680, 560),
                 'keys': (690, 400, 1110, 560), 'scroll': (20, 610, 560, 800),
                 'zoombar': (556, 620, 1150, 760)},
    'tips': {'tooltips': (20, 100, 540, 250), 'tip-automated': (630, 100, 920, 250),
             'helpline': (160, 485, 760, 720)},
    'dialogs': {'buttons': (140, 100, 620, 350), 'buttons-states': (590, 100, 1010, 350),
                'confirm': (100, 420, 560, 680)},
}


def main():
    zd = os.path.join(OUT, 'zoom')
    os.makedirs(zd, exist_ok=True)
    for sheet, crops in CROPS.items():
        for th in ('night', 'paper'):
            src = os.path.join(OUT, f'{sheet}-{th}-1x.png')
            im = Image.open(src).convert('RGB')
            for name, box in crops.items():
                c = im.crop(box)
                c = c.resize((c.width * 4, c.height * 4), Image.NEAREST)
                p = os.path.join(zd, f'{sheet}-{th}-4x-{name}.png')
                c.save(p, optimize=True)
                print(p, c.size)


if __name__ == '__main__':
    main()
