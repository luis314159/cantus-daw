"""Quick look: a character's key poses and a strip of one move, Night over Paper."""
import importlib
import sys
from moves import key_poses, MOVES
from render import page, shot
from svg import wrap
from theme import NIGHT, PAPER

name = sys.argv[1] if len(sys.argv) > 1 else "compas"
ch = importlib.import_module(name)
H = 200  # px a unit: the floating size in a 1280×720 window
cols = 9
cw, rh = 150, 250


def row(th, poses, y):
    out = [f'<rect x="0" y="{y}" width="{cols * cw}" height="{rh}" fill="{th["base"]}"/>']
    for i, (label, p) in enumerate(poses):
        x = i * cw + cw / 2
        out.append(wrap(ch.draw(p, th), x, y + rh - 22, H))
        out.append(f'<text x="{x}" y="{y + rh - 6}" fill="{th["ink3"]}" font-size="11" text-anchor="middle">{label}</text>')
    return "\n".join(out)


kp = ch.key_poses() if hasattr(ch, 'key_poses') else key_poses(ch.STYLE)
mv = sys.argv[2] if len(sys.argv) > 2 else "groove"
fn, n = MOVES[getattr(ch, "MOVES", {}).get(mv, mv)]
strip = [(f"{b:.2f}", fn(b, ch.STYLE)) for b in [i * n / cols for i in range(cols)]]
svg = (f'<svg xmlns="http://www.w3.org/2000/svg" width="{cols * cw}" height="{rh * 4}">'
       + row(NIGHT, kp, 0) + row(PAPER, kp, rh) + row(NIGHT, strip, 2 * rh) + row(PAPER, strip, 3 * rh)
       + "</svg>")
print(shot(f"try-{name}-{mv}", page(svg, cols * cw, rh * 4), cols * cw, rh * 4, scale=1))
