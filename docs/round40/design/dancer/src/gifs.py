"""Build the GIF previews.

usage: python gifs.py moves|window|cast|all [names…]
"""
import sys
from PIL import Image
from anim import (OUTNAME, char, pose_at, routine_pose, trails_for, dancer, stage, render_frames,
                  save_gif, FPB, CHARS, TITLES, MOVE_LABEL)
from render import BUILD, DESIGN, REPO
from theme import NIGHT, PAPER

OUT = DESIGN
PLAN = [("groove", 4), ("steps", 4), ("jump", 2), ("groove", 2)]  # 12 beats, 3 bars


def moves_gif(name, th):
    """Three moves side by side on Slate's stage, four beats (two cycles of the two-beat ones)."""
    ch = char(name)
    pw, ph = 420, 495
    frames = []
    for k in range(4 * FPB):
        b = k / FPB
        parts = []
        for i, mv in enumerate(("groove", "steps", "jump")):
            x = i * pw
            parts.append(stage(x, 0, pw, ph, th, MOVE_LABEL[mv], f"{ch.NAME} · {th['name']}", b))
            pose = pose_at(ch, mv, b)
            tr = trails_for(ch, lambda t, mv=mv: pose_at(ch, mv, t), b)
            parts.append(dancer(ch, pose, th, x + pw / 2, ph * 0.86, ph * 0.56, th["base"], tr))
            if i:
                parts.append(f'<rect x="{x}" y="0" width="2" height="{ph}" fill="{th["gutter"]}"/>')
        frames.append("".join(parts))
    imgs = render_frames(f"g-moves-{name}-{th['name']}", frames, pw * 3, ph)
    return save_gif(imgs, OUT / f"{OUTNAME.get(name, name).replace('_', '-')}-moves-{th['name'].lower()}.gif")


def crop_window(th):
    """The Slate window at 150 % (round 39's dir-a sheet), cropped to the arrangement's
    lower tracks and the mixer's empty strips: where a floating dancer stands."""
    src = REPO / "docs" / "round39" / "design" / "dir-a" / f"window-{th['name'].lower()}-1.5x.png"
    out = BUILD / f"crop-{th['name'].lower()}.png"
    BUILD.mkdir(exist_ok=True)
    Image.open(src).convert("RGB").crop((860, 330, 1548, 1050)).save(out)
    return out


def window_gif(name, th):
    """The dancer floating over the window (its default view: floating, size 1, 0.30 of the
    workspace's height), dancing a 12-beat routine with blends between the moves."""
    ch = char(name)
    bg = crop_window(th).as_uri()
    w, h = 688, 720
    size = 0.30 * 984  # FLOAT_STANDARD × the workspace's height at 1920×1080
    room = th["base"]
    frames = []
    for k in range(12 * FPB):
        b = k / FPB
        pose = routine_pose(ch, PLAN, b)
        tr = trails_for(ch, lambda t: routine_pose(ch, PLAN, t), b)
        fr = (f'<image href="{bg}" x="0" y="0" width="{w}" height="{h}"/>'
              + dancer(ch, pose, th, 352, 668, size, room, tr))
        frames.append(fr)
    imgs = render_frames(f"g-window-{name}-{th['name']}", frames, w, h)
    return save_gif(imgs, OUT / f"{OUTNAME.get(name, name).replace('_', '-')}-window-{th['name'].lower()}.gif")


def cast_gif(th):
    """The whole cast, the same routine on Slate's stage, in rows of six (twelve side by
    side would be too narrow to read); each does its own signature on the drop."""
    pw, ph, per = 300, 440, 6
    chars = [char(n) for n in CHARS]
    rows = (len(chars) + per - 1) // per
    frames = []
    for k in range(12 * FPB):
        b = k / FPB
        parts = []
        for i, ch in enumerate(chars):
            x, y = (i % per) * pw, (i // per) * ph
            parts.append(f'<svg x="{x}" y="{y}" width="{pw}" height="{ph}" overflow="hidden">')
            parts.append(stage(0, 0, pw, ph, th, TITLES[ch.__name__], th["name"], b, scale=1.0))
            pose = routine_pose(ch, PLAN, b)
            tr = trails_for(ch, lambda t, ch=ch: routine_pose(ch, PLAN, t), b)
            parts.append(dancer(ch, pose, th, pw / 2, ph * 0.86, ph * 0.52, th["base"], tr))
            parts.append("</svg>")
            if i % per:
                parts.append(f'<rect x="{x}" y="{y}" width="2" height="{ph}" fill="{th["gutter"]}"/>')
        for r in range(1, rows):
            parts.append(f'<rect x="0" y="{r * ph - 1}" width="{pw * per}" height="3" fill="{th["gutter"]}"/>')
        frames.append("".join(parts))
    imgs = render_frames(f"g-cast-{th['name']}", frames, pw * per, ph * rows)
    return save_gif(imgs, OUT / f"cast-{th['name'].lower()}.gif")


VARIANTS = [("sincopa", "v2 · the figure in values"), ("sincopa_s1", "S1 · graphic"),
            ("sincopa_s2", "S2 · the sidekick"), ("sincopa_s3", "S3 · noir")]


ONDA_VARIANTS = [("onda", "the ribbon (not liked)"), ("onda_o1", "O1 · the oscilloscope"),
                 ("onda_o2", "O2 · the wave-jelly"), ("onda_o3", "O3 · the surfer")]


def variants_gif(th, variants=None, out="sincopa-variants"):
    """A character's variants side by side, the same routine (each its signature on the drop)."""
    pw, ph = 330, 470
    chars = [(char(n), t) for n, t in (variants or VARIANTS)]
    frames = []
    for k in range(12 * FPB):
        b = k / FPB
        parts = []
        for i, (ch, title) in enumerate(chars):
            x = i * pw
            parts.append(stage(x, 0, pw, ph, th, title, th["name"], b, scale=1.0))
            pose = routine_pose(ch, PLAN, b)
            tr = trails_for(ch, lambda t, ch=ch: routine_pose(ch, PLAN, t), b)
            parts.append(f'<svg x="{x}" y="0" width="{pw}" height="{ph}" overflow="hidden">'
                         + dancer(ch, pose, th, pw / 2, ph * 0.86, ph * 0.55, th["base"], tr) + "</svg>")
            if i:
                parts.append(f'<rect x="{x}" y="0" width="2" height="{ph}" fill="{th["gutter"]}"/>')
        frames.append("".join(parts))
    imgs = render_frames(f"g-{out}-{th['name']}", frames, pw * len(chars), ph)
    return save_gif(imgs, OUT / f"{out}-{th['name'].lower()}.gif")


if __name__ == "__main__":
    what = sys.argv[1] if len(sys.argv) > 1 else "all"
    names = sys.argv[2:] or CHARS
    for th in (NIGHT, PAPER):
        if what in ("moves", "all"):
            for n in names:
                print(moves_gif(n, th))
        if what in ("window", "all"):
            for n in names:
                print(window_gif(n, th))
        if what == "variants":
            print(variants_gif(th))
        if what == "onda-variants":
            print(variants_gif(th, ONDA_VARIANTS, "onda-variants"))
        if what in ("cast", "all"):
            print(cast_gif(th))
