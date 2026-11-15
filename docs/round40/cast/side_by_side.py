"""Side-by-side GIFs: the designer's approved GIF above, the app's own renderer below.

usage: python side_by_side.py <frames-dir> <name> <night|paper> <moves|routine> <out.gif>

`<frames-dir>` holds the app's frames from
`cargo run -p cantus-dance --example slate_frames -- <frames-dir> <name> <theme> <mode>`.
- `moves`: the design's `<name>-moves-<theme>.gif` (groove, step-and-touch, the drop; four
  beats) against the app's three panels;
- `routine`: the character's tile of the design's `cast-<theme>.gif` (the 12-beat routine)
  against the app dancing its own routine through `pose_at`.
Pillow (MIT-CMU), already used by the designer's tools; nothing downloaded.
"""
import sys
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

HERE = Path(__file__).resolve().parent
DESIGN = HERE.parent / "design" / "dancer"
CAST = ["compas", "tono", "sincopa", "bajo", "onda", "voz", "glissando", "eco", "vinilo", "cinta",
        "metronomo", "espectro"]
THEMES = {
    "night": {"base": "#1D2329", "grid_bar": "#323B44", "gutter": "#14191E", "ink1": "#E3E8ED",
              "ink3": "#909CA8", "gold": "#E2B85A"},
    "paper": {"base": "#F4F6F8", "grid_bar": "#C8D0D8", "gutter": "#C5CDD5", "ink1": "#17202A",
              "ink3": "#556170", "gold": "#9A6E1C"},
}
MS = 40
FONT = Path(__file__).resolve().parents[3] / "crates" / "cantus-ui" / "assets" / "fonts"


def font(size):
    for name in ("BarlowSemiCondensed-Medium.ttf", "BarlowSemiCondensed-Regular.ttf"):
        p = FONT / name
        if p.exists():
            return ImageFont.truetype(str(p), size)
    return ImageFont.load_default()


def gif_frames(path, n):
    """The GIF's frames at 40 ms each (merged frames expanded by their duration)."""
    im = Image.open(path)
    out = []
    for i in range(im.n_frames):
        im.seek(i)
        k = max(1, round(im.info.get("duration", MS) / MS))
        fr = im.convert("RGB").copy()
        out.extend([fr] * k)
    return out[:n]


def floor(img, th, w, h, panels):
    d = ImageDraw.Draw(img)
    for p in range(panels):
        x = p * w
        d.rectangle([x, int(h * 0.86), x + w, int(h * 0.86) + 1], fill=th["grid_bar"])
        if p:
            d.rectangle([x, 0, x + 1, h], fill=th["gutter"])


def band(text, w, th, sub=""):
    img = Image.new("RGB", (w, 34), th["gutter"])
    d = ImageDraw.Draw(img)
    d.text((12, 6), text, fill=th["ink1"], font=font(18))
    if sub:
        d.text((12 + d.textlength(text, font=font(18)) + 14, 9), sub, fill=th["ink3"], font=font(14))
    return img


def save_gif(images, out):
    n = len(images)
    pick = [images[i] for i in range(0, n, max(1, n // 12))][:12]
    w, h = pick[0].size
    strip = Image.new("RGB", (w, h * len(pick)))
    for i, im in enumerate(pick):
        strip.paste(im, (0, i * h))
    pal = strip.quantize(colors=255, method=Image.Quantize.MEDIANCUT)
    frames = [im.quantize(palette=pal, dither=Image.Dither.NONE) for im in images]
    frames[0].save(out, save_all=True, append_images=frames[1:], duration=MS, loop=0,
                   optimize=False, disposal=1)


def main():
    frames_dir, name, theme, mode, out = sys.argv[1:6]
    th = THEMES[theme]
    title = name.capitalize().replace("Compas", "Compás").replace("Metronomo", "Metrónomo") \
        .replace("Sincopa", "Síncopa")
    fd = Path(frames_dir)
    if mode == "moves":
        n, w, h, panels = 48, 420, 495, 3
        design = gif_frames(DESIGN / f"{name}-moves-{theme}.gif", n)
        app = []
        for k in range(n):
            strip = Image.new("RGB", (w * panels, h), th["base"])
            for p in range(panels):
                strip.paste(Image.open(fd / f"f{k:03}_{p}.png").convert("RGB"), (p * w, 0))
            floor(strip, th, w, h, panels)
            app.append(strip)
    else:
        n, w, h = 144, 300, 440
        idx = CAST.index(name)
        cx, cy = (idx % 6) * w, (idx // 6) * h
        design = [f.crop((cx, cy, cx + w, cy + h)) for f in gif_frames(DESIGN / f"cast-{theme}.gif", n)]
        app = []
        for k in range(n):
            fr = Image.open(fd / f"f{k:03}.png").convert("RGB")
            floor(fr, th, w, h, 1)
            app.append(fr)
    W = design[0].size[0]
    top = band("Design", W, th, f"{title} · approved GIF")
    bot = band("App", W, th, "the app's own renderer (tiny-skia)")
    frames = []
    for d, a in zip(design, app):
        img = Image.new("RGB", (W, 34 + d.size[1] + 34 + a.size[1]), th["gutter"])
        img.paste(top, (0, 0))
        img.paste(d, (0, 34))
        img.paste(bot, (0, 34 + d.size[1]))
        img.paste(a, (0, 68 + d.size[1]))
        frames.append(img)
    save_gif(frames, out)
    print(out)


if __name__ == "__main__":
    main()
