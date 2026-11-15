"""Animated previews: frames as SVG → a grid shot with Edge → sliced → a GIF (Pillow).

125 BPM, 12 frames a beat = 25 frames a second (40 ms), so every beat lands on a frame.
"""
import importlib
import math
from pathlib import Path
from PIL import Image
from moves import MOVES, routine
from render import page, shot, BUILD, DESIGN, REPO
from svg import wrap, f
from theme import NIGHT, PAPER

BPM = 125.0
FPB = 12                      # frames a beat
MS = round(60000 / BPM / FPB)  # 40 ms
CHARS = ["compas", "tono", "sincopa_s3", "bajo", "onda_o1", "voz", "glissando", "eco", "vinilo", "cinta",
         "metronomo", "espectro"]
TITLES = {"compas": "Compás", "tono": "Tono", "sincopa": "Síncopa", "sincopa_s3": "Síncopa", "bajo": "Bajo", "onda": "Onda",
          "onda_o1": "Onda", "voz": "Voz", "glissando": "Glissando", "eco": "Eco", "vinilo": "Vinilo",
          "cinta": "Cinta", "metronomo": "Metrónomo", "espectro": "Espectro"}
OUTNAME = {"sincopa_s3": "sincopa", "onda_o1": "onda"}   # the chosen variant is the cast's Síncopa
MOVE_LABEL = {"groove": "Groove", "steps": "Step and touch", "jump": "On the drop"}


def char(name):
    return importlib.import_module(name)


def pose_at(ch, move, b):
    fn, _ = MOVES[ch.MOVES.get(move, move)]
    p = fn(b, ch.STYLE)
    p.t = b
    return p


def routine_pose(ch, plan, b):
    mapped = [(ch.MOVES.get(m, m), n) for m, n in plan]
    p = routine(b, ch.STYLE, mapped)
    p.t = b
    return p


def trails_for(ch, posef, b):
    """Earlier joints for motion marks (Compás draws them; the others ignore them)."""
    if getattr(ch, "ECHO", False):      # Eco: earlier poses (the delay's taps), not joints
        return [posef(b - k * ch.ECHO_STEP) for k in range(1, ch.ECHO_N + 1)]
    if not getattr(ch, "TRAILS", ch.__name__ == "compas"):
        return None
    from body import solve
    st, n = getattr(ch, "TRAIL_STEP", 0.03), getattr(ch, "TRAIL_N", 5)
    return [solve(posef(b - k * st), ch.PROPS) for k in range(1, n + 1)]


def dancer(ch, pose, th, x, ground, size, room, trails=None):
    return wrap(ch.draw(pose, th, cut_color=room, trails=trails), x, ground, size)


# ------------------------------------------------------------------ the Slate stage panel
def stage(x, y, w, h, th, title, sub, b, beats_in_bar=4, scale=1.5):
    """Slate's stage: the base, a floor line, a quiet label top-left, beat dots below."""
    s = scale
    beat = int(math.floor(b)) % beats_in_bar
    bar = int(math.floor(b / beats_in_bar)) + 1
    out = [f'<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{th["base"]}"/>',
           f'<rect x="{x}" y="{f(y + h * 0.86)}" width="{w}" height="{f(1 * s)}" fill="{th["grid_bar"]}"/>',
           f'<text x="{f(x + 12 * s)}" y="{f(y + 20 * s)}" fill="{th["ink1"]}" font-size="{f(13 * s)}" '
           f'font-weight="500">{title}</text>',
           f'<text x="{f(x + 12 * s)}" y="{f(y + 35 * s)}" fill="{th["ink3"]}" font-size="{f(11 * s)}">{sub}</text>',
           f'<text x="{f(x + w - 12 * s)}" y="{f(y + 20 * s)}" fill="{th["gold"]}" font-size="{f(12 * s)}" '
           f'text-anchor="end" font-weight="500">{bar}.{beat + 1}</text>']
    cx = x + w / 2
    for n in range(beats_in_bar):
        on = n == beat
        dx = (n - (beats_in_bar - 1) / 2) * 14 * s
        out.append(f'<circle cx="{f(cx + dx)}" cy="{f(y + h * 0.93)}" r="{f((3.2 if on else 2.4) * s)}" '
                   f'fill="{th["gold"] if on else th["ink4"]}"/>')
    return "\n".join(out)


# ------------------------------------------------------------------ grids → frames → GIF
def render_frames(name, frames, w, h, scale=1):
    """`frames`: SVG strings w×h (CSS px). Returns PIL images."""
    per_row = max(1, min(len(frames), 7000 // w))
    rows_per_page = max(1, 15000 // h)
    per_page = per_row * rows_per_page
    images = []
    for pg in range(0, len(frames), per_page):
        chunk = frames[pg:pg + per_page]
        rows = math.ceil(len(chunk) / per_row)
        W, H = per_row * w, rows * h
        body = "".join(
            f'<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" '
            f'style="position:absolute;left:{(i % per_row) * w}px;top:{(i // per_row) * h}px" '
            f'width="{w}" height="{h}" viewBox="0 0 {w} {h}">{fr}</svg>'
            for i, fr in enumerate(chunk))
        png = shot(f"{name}-p{pg // per_page}", page(body, W, H), W, H, scale=scale)
        sheet = Image.open(png).convert("RGB")
        for i in range(len(chunk)):
            cx, cy = (i % per_row) * w * scale, (i // per_row) * h * scale
            images.append(sheet.crop((cx, cy, cx + w * scale, cy + h * scale)))
    return images


def save_gif(images, out, ms=MS):
    """One palette for the whole GIF (built from a spread of frames): no flicker."""
    n = len(images)
    pick = [images[i] for i in range(0, n, max(1, n // 12))][:12]
    w, h = pick[0].size
    strip = Image.new("RGB", (w, h * len(pick)))
    for i, im in enumerate(pick):
        strip.paste(im, (0, i * h))
    pal = strip.quantize(colors=255, method=Image.Quantize.MEDIANCUT)
    frames = [im.quantize(palette=pal, dither=Image.Dither.NONE) for im in images]
    out = Path(out)
    frames[0].save(out, save_all=True, append_images=frames[1:], duration=ms, loop=0,
                   optimize=False, disposal=1)
    return out
