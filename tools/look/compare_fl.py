"""Side-by-side crops, Cantus vs FL Studio, at the same physical scale, upscaled with nearest
neighbour so every pixel of finish shows (round 37, research/ui/fl-look-2026-11/notes.md).

    python tools/look/compare_fl.py <out dir> [<cantus shots dir>] [<fl shots dir>]

The Cantus shots default to docs/images (window-night.png at 1920x1080, rack.png,
pianoroll.png, units/aequalis-dark.png); re-shoot them and point the second argument at the
new folder to compare a change. The FL shots are the owner's private reference captures
(never committed): research/ui/fl-look-2026-11/screenshots in the owner's tree, taken with
tools/look/shots_of_window.ps1. The crop boxes are fixed so every run compares the same 13
elements."""
import os, sys
from PIL import Image, ImageDraw, ImageFont

OUT = sys.argv[1]
os.makedirs(OUT, exist_ok=True)
HERE = os.path.dirname(os.path.abspath(__file__))
C = sys.argv[2] if len(sys.argv) > 2 else os.path.join(HERE, "..", "..", "docs", "images")
F = sys.argv[3] if len(sys.argv) > 3 else os.path.join(HERE, "..", "..", "research", "ui", "fl-look-2026-11", "screenshots")
CW = os.path.join(C, "window-night.png")
FMIX = os.path.join(F, "fl-mixer.png")
FPL = os.path.join(F, "fl-playlist.png")
FEQ = os.path.join(F, "fl-parametric-eq.png")
FRACK = os.path.join(F, "fl-channel-rack.png")
FPR = os.path.join(F, "fl-piano-roll.png")

try:
    FONT = ImageFont.truetype("segoeui.ttf", 18)
except OSError:
    FONT = ImageFont.load_default()

def crop(path, box, zoom):
    im = Image.open(path).convert("RGB").crop(box)
    if zoom != 1:
        im = im.resize((im.width * zoom, im.height * zoom), Image.NEAREST)
    return im

def label(im, text):
    pad = 30
    out = Image.new("RGB", (im.width, im.height + pad), (24, 24, 24))
    out.paste(im, (0, pad))
    ImageDraw.Draw(out).text((8, 5), text, fill=(235, 235, 235), font=FONT)
    return out

def pair(name, a, b, stacked=False):
    a = label(a, "Cantus (round 36)")
    b = label(b, "FL Studio 24.1.2")
    gap = 16
    if stacked:
        w = max(a.width, b.width); h = a.height + b.height + gap
        out = Image.new("RGB", (w, h), (12, 12, 12)); out.paste(a, (0, 0)); out.paste(b, (0, a.height + gap))
    else:
        w = a.width + b.width + gap; h = max(a.height, b.height)
        out = Image.new("RGB", (w, h), (12, 12, 12)); out.paste(a, (0, 0)); out.paste(b, (a.width + gap, 0))
    out.save(os.path.join(OUT, name + ".png"))
    print(name, out.size)

pair("01-top-bar", crop(CW, (0, 0, 1920, 58), 1), crop(FMIX, (0, 0, 1920, 100), 1), stacked=True)
pair("02-transport-zoom", crop(CW, (680, 0, 1300, 55), 2), crop(FMIX, (380, 0, 1050, 50), 2), stacked=True)
pair("03-toolbar-icons-zoom", crop(CW, (1540, 6, 1800, 46), 3), crop(FMIX, (1160, 54, 1530, 98), 3))
pair("04-mixer", crop(CW, (300, 560, 1590, 1050), 1), crop(FMIX, (490, 360, 1905, 975), 1), stacked=True)
pair("05-strip-zoom", crop(CW, (464, 572, 544, 1045), 2), crop(FMIX, (672, 390, 736, 975), 2))
pair("06-fader-meter-knob-zoom", crop(CW, (464, 600, 544, 830), 4), crop(FMIX, (672, 560, 736, 800), 4))
pair("07-arrangement", crop(CW, (300, 125, 1590, 515), 1), crop(FPL, (1095, 140, 1905, 1010), 1))
pair("08-headers-clips-zoom", crop(CW, (303, 150, 700, 340), 3), crop(FPL, (1298, 205, 1700, 420), 3))
pair("09-channel-rack", crop(os.path.join(C, "rack.png"), (0, 55, 700, 545), 2), crop(FRACK, (472, 160, 1085, 650), 2))
pair("10-piano-keys-zoom", crop(os.path.join(C, "pianoroll.png"), (0, 60, 160, 560), 3), crop(FPR, (962, 230, 1100, 780), 3))
pair("11-eq", crop(os.path.join(C, "units", "aequalis-dark.png"), (0, 0, 1180, 640), 1), crop(FEQ, (578, 320, 1460, 790), 1), stacked=True)
pair("12-browser", crop(CW, (0, 58, 300, 420), 2), crop(FEQ, (0, 100, 458, 1020), 1))
pair("13-type-zoom", crop(CW, (466, 590, 1000, 612), 4), crop(FMIX, (676, 470, 1340, 560), 2), stacked=True)
