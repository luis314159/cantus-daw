"""Compose the comparison sheets: FL Studio's equivalents cut from the owner's screenshots,
today's Cantus (beta.13, round 37's top bar) and the new family, each at 1:1 device pixels
and the same key height, then the same at 4x by nearest neighbour.

    python compare.py <out-dir-private> <out-dir-cantus-only>

The private sheets (with FL) go to research/ui/round39/icons/ (git add -f); the Cantus-only
before/after sheets go to docs/round39/design/icons/.
"""
import os
import sys

from PIL import Image, ImageDraw, ImageFont

import render

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", "..", "..", ".."))
FL = "F:/projects/cantus/research/ui/fl-look-2026-11/screenshots/fl-playlist.png"
TODAY = {t: os.path.join(ROOT, "docs", "round37", "finish", f"topbar-{t}-100.png") for t in ("night", "paper")}
TMP = os.path.join(os.environ.get("TEMP", "."), "cantus-icons-compare")
FONT = "C:/Windows/Fonts/segoeui.ttf"
FONTB = "C:/Windows/Fonts/seguisb.ttf"

TRANSPORT = "toStart,skipBack,play,pause,stop,record,skipFwd,loop,metronome,countIn,follow"
TOOLS = "snap,undo,redo,draw,select,slice,erase,muteTool,zoom,pick,gear"
WINDOWS = "arrangement,pianoRoll,rack,mixer,browser,plugins,inspector"

# (label, file, box) — FL's crops from fl-playlist.png at its own pixels.
FL_CROPS = {
    "transport": [("FL Studio 24 · transport", (430, 4, 668, 48)),
                  ("FL Studio 24 · metronome, wait, count-in, step, loop record", (674, 4, 880, 48))],
    "tools": [("FL Studio 24 · playlist tools (snap, draw, paint, delete, mute, slip, slice, select, zoom)", (1100, 104, 1420, 138)),
              ("FL Studio 24 · undo, cut, record, help, save…", (1455, 2, 1780, 48))],
    "windows": [("FL Studio 24 · window keys", (1160, 54, 1560, 98))],
}
TODAY_CROPS = {
    "transport": [((505, 136, 735, 174)), ((913, 136, 950, 174))],
    "tools": [((1330, 124, 1354, 154)), ((1220, 156, 1277, 186)), ((1755, 156, 1810, 186))],
    "windows": [((1364, 124, 1542, 154))],
}
NEW = {"transport": (TRANSPORT, [24, 36]), "tools": (TOOLS, [20, 24, 30]), "windows": (WINDOWS, [24, 36])}
# Only keys that can be on are shown on.
ON = {"transport": "play,record,loop,metronome,countIn,follow",
      "tools": "snap,draw,select,slice,erase,muteTool,zoom,pick", "windows": WINDOWS}
_CACHE = {}
TITLES = {"transport": "Transport", "tools": "Toolbar and top bar — tools", "windows": "Toolbar and top bar — windows"}


def strip(theme, size, ids, state="rest"):
    key = (theme, size, ids, state)
    if key in _CACHE:
        return _CACHE[key]
    os.makedirs(TMP, exist_ok=True)
    out = os.path.join(TMP, f"strip-{theme}-{size}-{abs(hash(ids)) % 10**8}-{state}.png")
    url = render.url_for("sheet.html", f"t={theme}&strip={size}&ids={ids}&state={state}")
    w, h = render.size_of(url)
    render.edge(["--force-device-scale-factor=1", f"--window-size={w},{h}", f"--screenshot={out}", url])
    _CACHE[key] = Image.open(out).convert("RGB").crop((0, 0, w, h))
    return _CACHE[key]


def compose(section, scale, with_fl, themes):
    """Rows of (label, image) for one section, then laid out at `scale`."""
    rows = []
    fl = Image.open(FL).convert("RGB")
    ids, sizes = NEW[section]
    for theme in themes:
        if with_fl and theme == "night":
            for label, box in FL_CROPS[section]:
                rows.append((label, fl.crop(box)))
        today = Image.open(TODAY[theme]).convert("RGB")
        parts = [today.crop(b) for b in TODAY_CROPS[section]]
        joined = Image.new("RGB", (sum(p.width for p in parts) + 8 * (len(parts) - 1), max(p.height for p in parts)), parts[0].getpixel((0, 0)))
        x = 0
        for p in parts:
            joined.paste(p, (x, 0))
            x += p.width + 8
        rows.append((f"Cantus today (beta.13) · {theme.capitalize()} · 100 %", joined))
        for size in sizes:
            pct = {20: "20 px · 100 %", 24: "24 px · 100 % (and 16 px · 150 %)", 30: "30 px · 20 at 150 %", 36: "36 px · 24 at 150 %"}[size]
            rows.append((f"New family · {theme.capitalize()} · {pct}", strip(theme, size, ids)))
        rows.append((f"New family · {theme.capitalize()} · {sizes[0]} px · on", strip(theme, sizes[0], ON[section], "on")))
    pad, gap, lab = 16 * scale, 10 * scale, 22 * (1 if scale == 1 else 2)
    font = ImageFont.truetype(FONT, 12 if scale == 1 else 24)
    fontb = ImageFont.truetype(FONTB, 15 if scale == 1 else 30)
    width = pad * 2 + max(img.width * scale for _, img in rows)
    height = pad + 30 * (1 if scale == 1 else 2) + sum(lab + img.height * scale + gap for _, img in rows) + pad
    sheet = Image.new("RGB", (max(width, 700 * (1 if scale == 1 else 2)), height), (24, 27, 31))
    d = ImageDraw.Draw(sheet)
    d.text((pad, pad), f"{TITLES[section]} — {'1:1 device pixels' if scale == 1 else '4x, nearest neighbour'}", font=fontb, fill=(220, 225, 230))
    y = pad + 30 * (1 if scale == 1 else 2)
    for label, img in rows:
        d.text((pad, y), label, font=font, fill=(150, 160, 170))
        y += lab
        big = img.resize((img.width * scale, img.height * scale), Image.NEAREST)
        sheet.paste(big, (pad, y))
        y += big.height + gap
    return sheet


def main():
    private, public = sys.argv[1], sys.argv[2]
    os.makedirs(private, exist_ok=True)
    os.makedirs(public, exist_ok=True)
    for section in ("transport", "tools", "windows"):
        for scale in (1, 4):
            tag = "1x" if scale == 1 else "4x"
            compose(section, scale, True, ["night", "paper"]).save(os.path.join(private, f"vs-fl-{section}-{tag}.png"))
            compose(section, scale, False, ["night", "paper"]).save(os.path.join(public, f"before-after-{section}-{tag}.png"))
            print(section, tag)


if __name__ == "__main__":
    main()
