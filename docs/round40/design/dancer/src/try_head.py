"""Quick look: heads and moods, large (the face at the companion's and the sheet's size)."""
import importlib
import sys
from rig import Pose
from render import page, shot
from svg import wrap
from theme import NIGHT, PAPER

name = sys.argv[1]
ch = importlib.import_module(name)
moods = [("open", 0.0), ("happy", 0.0), ("happy", 0.8), ("wide", 0.5), ("closed", 0.0), ("cool", 0.0)]
cw, rh, H = 200, 220, 640
out = []
for r, th in enumerate((NIGHT, PAPER)):
    out.append(f'<rect x="0" y="{r * rh}" width="{cw * len(moods)}" height="{rh}" fill="{th["base"]}"/>')
    for i, (e, mo) in enumerate(moods):
        p = Pose(eyes=e, mouth=mo, level=0.7, point=15)
        g = wrap(ch.draw(p, th, floor=False), i * cw + cw / 2, r * rh + rh + H * float(sys.argv[2] if len(sys.argv) > 2 else 0.74), H)
        out.append(f'<svg x="{i * cw}" y="{r * rh}" width="{cw}" height="{rh}" overflow="hidden">'
                   f'<g transform="translate({-i * cw},{-r * rh})">{g}</g></svg>')
w, h = cw * len(moods), rh * 2
svg = f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}">' + "".join(out) + "</svg>"
print(shot(f"head-{name}", page(svg, w, h), w, h, scale=1))
