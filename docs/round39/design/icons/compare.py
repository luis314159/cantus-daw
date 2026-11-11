"""Compose the comparison sheets of the second pass.

Private (they show other products), to research/ui/round39/icons/:
    vs-daws-{transport,tools,windows}-{1x,4x}.png
        FL Studio, Ableton Live, Bitwig Studio, Logic Pro / Studio One — each crop scaled
        (Lanczos) so that its key is as tall as ours — then Cantus at the same key height,
        for a 24-px key (100 %) and a 36-px key (150 %). The 4x is nearest neighbour.
Cantus only, to docs/round39/design/icons/:
    before-after-{transport,tools,windows}-{1x,4x}.png
        today's Cantus (beta.13, round 37's top bar), the first pass, the second pass.

    python compare.py <private-dir> <public-dir>
"""
import os
import sys

from PIL import Image, ImageDraw, ImageFont

import render

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", "..", "..", ".."))
SHOTS = "F:/projects/cantus/research/ui/screenshots/"
FL = "F:/projects/cantus/research/ui/fl-look-2026-11/screenshots/fl-playlist.png"
TODAY = {t: os.path.join(ROOT, "docs", "round37", "finish", f"topbar-{t}-100.png") for t in ("night", "paper")}
TMP = os.path.join(os.environ.get("TEMP", "."), "cantus-icons-compare2")
FONT, FONTB = "C:/Windows/Fonts/segoeui.ttf", "C:/Windows/Fonts/seguisb.ttf"

IDS = {
    "transport": "toStart,skipBack,play,pause,stop,record,skipFwd,loop,metronome,countIn,follow",
    "tools": "snap,undo,redo,draw,select,slice,erase,muteTool,zoom,pick,gear",
    "windows": "arrangement,pianoRoll,rack,mixer,browser,plugins,inspector",
}
ON = {"transport": "play,record,loop,metronome,countIn,follow",
      "tools": "snap,draw,select,slice,erase,muteTool,zoom,pick", "windows": IDS["windows"]}

# (label, file, box, the reference's own key height in its pixels)
REFS = {
    "transport": [
        ("FL Studio 24", FL, (430, 4, 668, 48), 40), ("FL Studio 24", FL, (674, 4, 880, 48), 40),
        ("Ableton Live 12", SHOTS + "ableton-live/control-bar.png", (1400, 50, 1780, 98), 34),
        ("Ableton Live 12 (punch, loop, punch)", SHOTS + "ableton-live/control-bar.png", (1960, 50, 2120, 98), 34),
        ("Bitwig Studio 6", SHOTS + "bitwig-studio/crop-03-bws6-toolbar-left.png", (170, 56, 600, 122), 62),
        ("Logic Pro", SHOTS + "logic-studio-one/logic-main-window-overview.png", (335, 84, 530, 118), 30)],
    "tools": [
        ("FL Studio 24 (playlist tools)", FL, (1100, 104, 1420, 138), 30), ("FL Studio 24", FL, (1455, 2, 1780, 48), 40),
        ("Ableton Live 12 (draw mode)", SHOTS + "ableton-live/control-bar.png", (2270, 50, 2420, 98), 34),
        ("Bitwig Studio 6", SHOTS + "bitwig-studio/crop-04-bws6-toolbar-right.png", (630, 56, 1050, 122), 62),
        ("Logic Pro", SHOTS + "logic-studio-one/logic-main-window-overview.png", (8, 84, 240, 118), 30),
        ("Logic Pro (edit tools)", SHOTS + "logic-studio-one/logic-main-window-overview.png", (240, 134, 420, 160), 24)],
    "windows": [
        ("FL Studio 24", FL, (1160, 54, 1560, 98), 40),
        ("Bitwig Studio 6", SHOTS + "bitwig-studio/crop-03-bws6-toolbar-left.png", (600, 56, 980, 122), 62),
        ("Bitwig Studio 6", SHOTS + "bitwig-studio/crop-04-bws6-toolbar-right.png", (400, 56, 620, 122), 62),
        ("Logic Pro", SHOTS + "logic-studio-one/logic-main-window-overview.png", (1170, 84, 1300, 118), 30),
        ("Studio One (navigation, turned to a row)", SHOTS + "logic-studio-one/s1-console-nav-column-wrench.png", (0, 555, 115, 963), 44)],
}
TODAY_CROPS = {
    "transport": [(505, 136, 735, 174), (913, 136, 950, 174)],
    "tools": [(1330, 124, 1354, 154), (1220, 156, 1277, 186), (1755, 156, 1810, 186)],
    "windows": [(1364, 124, 1542, 154)],
}
_CACHE = {}


def page(query):
    key = query
    if key in _CACHE:
        return _CACHE[key]
    os.makedirs(TMP, exist_ok=True)
    out = os.path.join(TMP, f"p{len(_CACHE)}.png")
    url = render.url_for("sheet.html", query)
    w, h = render.size_of(url)
    render.edge(["--force-device-scale-factor=1", f"--window-size={w},{h}", f"--screenshot={out}", url])
    _CACHE[key] = Image.open(out).convert("RGB").crop((0, 0, w, h))
    return _CACHE[key]


def strip(theme, k, ids, state="rest"):
    return page(f"t={theme}&strip={k}&ids={ids}&state={state}")


def ref(path, box, kh, k):
    im = Image.open(path).convert("RGB").crop(box)
    if box[3] - box[1] > 2 * (box[2] - box[0]):  # a column: lay its cells out as a row
        cells = [im.crop((0, y, im.width, min(y + kh, im.height))) for y in range(0, im.height - kh // 2, kh)]
        row = Image.new("RGB", (im.width * len(cells), kh), im.getpixel((2, 2)))
        for i, c in enumerate(cells):
            row.paste(c, (i * im.width, 0))
        im = row
    f = k / kh
    return im.resize((max(1, round(im.width * f)), max(1, round(im.height * f))), Image.LANCZOS)


def lay(title, rows, scale):
    pad, gap = 16 * scale, 8 * scale
    lab = 20 if scale == 1 else 40
    font = ImageFont.truetype(FONT, 12 if scale == 1 else 24)
    fontb = ImageFont.truetype(FONTB, 14 if scale == 1 else 28)
    width = max(pad * 2 + max(img.width * scale for _, img in rows), 600 * (1 if scale == 1 else 2))
    height = pad + 28 * (1 if scale == 1 else 2) + sum(lab + img.height * scale + gap for _, img in rows) + pad
    sheet = Image.new("RGB", (width, height), (20, 25, 30))
    d = ImageDraw.Draw(sheet)
    d.text((pad, pad), title, font=fontb, fill=(227, 232, 237))
    y = pad + 28 * (1 if scale == 1 else 2)
    for label, img in rows:
        d.text((pad, y), label, font=font, fill=(166, 176, 186))
        y += lab
        big = img.resize((img.width * scale, img.height * scale), Image.NEAREST)
        sheet.paste(big, (pad, y))
        y += big.height + gap
    return sheet


def vs_daws(section):
    rows = []
    for k, pct in ((24, "a 24-px key (100 %)"), (36, "a 36-px key (150 %)")):
        for label, path, box, kh in REFS[section]:
            rows.append((f"{label} · scaled to {pct}", ref(path, box, kh, k)))
        for theme in ("night", "paper"):
            rows.append((f"Cantus, second pass · Slate {theme.capitalize()} · {pct}", strip(theme, k, IDS[section])))
        rows.append((f"Cantus, second pass · Slate Night · {pct} · on", strip("night", k, ON[section], "on")))
    return rows


def before_after(section):
    rows = []
    for theme in ("night", "paper"):
        today = Image.open(TODAY[theme]).convert("RGB")
        parts = [today.crop(b) for b in TODAY_CROPS[section]]
        joined = Image.new("RGB", (sum(p.width + 8 for p in parts), max(p.height for p in parts)), parts[0].getpixel((0, 0)))
        x = 0
        for p in parts:
            joined.paste(p, (x, 0))
            x += p.width + 8
        rows.append((f"Cantus today (beta.13) · {theme.capitalize()} · 100 %", joined))
        for k in (20, 24):
            rows.append((f"Second pass · Slate {theme.capitalize()} · {k}-px key", strip(theme, k, IDS[section])))
        rows.append((f"Second pass · Slate {theme.capitalize()} · 24-px key · hover", strip(theme, 24, IDS[section], "hover")))
        rows.append((f"Second pass · Slate {theme.capitalize()} · 24-px key · on", strip(theme, 24, ON[section], "on")))
    return rows


def main():
    private, public = sys.argv[1], sys.argv[2]
    os.makedirs(private, exist_ok=True)
    os.makedirs(public, exist_ok=True)
    titles = {"transport": "Transport", "tools": "Tools", "windows": "Window keys"}
    for s in ("transport", "tools", "windows"):
        a, b = vs_daws(s), before_after(s)
        for scale, tag in ((1, "1x"), (4, "4x")):
            lay(f"{titles[s]} — Cantus beside FL, Ableton, Bitwig, Logic/Studio One — {'1:1' if scale == 1 else '4x nearest'}", a, scale).save(os.path.join(private, f"vs-daws-{s}-{tag}.png"))
            lay(f"{titles[s]} — today's Cantus and the second pass — {'1:1' if scale == 1 else '4x nearest'}", b, scale).save(os.path.join(public, f"before-after-{s}-{tag}.png"))
        print(s)


if __name__ == "__main__":
    main()
