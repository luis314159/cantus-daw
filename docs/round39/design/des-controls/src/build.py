"""des-controls — build every sheet and render it with Edge headless.

python -I build.py [sheet ...]       (no argument: every sheet)

Each sheet is drawn natively at 100 % and at 150 % (physical pixels, scale factor 1), in
Night and Paper. Then zoom.py cuts the 4x crops with nearest-neighbour.
"""
import importlib
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from slate import THEMES  # noqa: E402

OUT = os.path.normpath(os.path.join(HERE, '..'))
BUILD = os.path.join(OUT, 'build')
EDGE = r'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'
SHEETS = ['knobs', 'faders', 'fields', 'menus', 'switches', 'tips', 'dialogs']


def render(html_path, w, h, png):
    url = 'file:///' + html_path.replace('\\', '/')
    subprocess.run([EDGE, '--headless=new', '--disable-gpu', '--disable-lcd-text', '--hide-scrollbars',
                    '--force-device-scale-factor=1', f'--window-size={w},{h}',
                    f'--screenshot={png}', url], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                   check=False, timeout=120)


def main(names):
    os.makedirs(BUILD, exist_ok=True)
    for name in names:
        mod = importlib.import_module('sheet_' + name)
        for th, t in THEMES.items():
            for s, tag in ((1, '1x'), (1.5, '1.5x')):
                sh = mod.build(t, s)
                hp = os.path.join(BUILD, f'{name}-{th}-{tag}.html')
                with open(hp, 'w', encoding='utf-8') as fh:
                    fh.write(sh.html())
                png = os.path.join(OUT, f'{name}-{th}-{tag}.png')
                render(hp, sh.P(sh.w), sh.P(sh.h), png)
                print(png)


if __name__ == '__main__':
    main(sys.argv[1:] or SHEETS)
