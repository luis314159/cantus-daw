"""The comparison beside FL Studio, at the same physical scale (private: it shows FL, so it
is written under research/ui/round39/dir-c/ and added with git add -f).

Row 1: Studio ink Night at 150 % | FL Studio 24 playlist | Cantus beta.13 startup.png (1280x720, upscaled 1.5)
Row 2: Studio ink Paper at 150 % | FL Studio 24 mixer    | Cantus beta.13 real-night.png (the real app at 150 %)
Then the detail sheet: the same regions cut at the same physical scale, magnified 2x."""
import os
import sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import render, write, OUT, FONT_CSS

WT = os.path.normpath(os.path.join(OUT, "..", "..", "..", ".."))
PRIV = os.path.join(WT, "research", "ui", "round39", "dir-c")
FL = "F:/projects/cantus/research/ui/fl-look-2026-11/screenshots/"


def u(p):
    return "file:///" + p.replace("\\", "/")


ME_N = u(os.path.join(OUT, "window-night-150.png"))
ME_P = u(os.path.join(OUT, "window-paper-150.png"))
OLD = u(os.path.join(WT, "docs", "manual", "img", "startup.png"))
OLD_REAL = u(os.path.join(WT, "docs", "round37", "finish", "real-night.png"))

CSS = FONT_CSS + """
body{margin:0;background:#050607;color:#E7EBEF;font-family:PlexC}
h1{font-weight:500;font-size:44px;margin:0 0 6px}
.sub{color:#7A838E;font-size:24px}
.grid{display:grid;grid-template-columns:1920px 1920px 1920px;gap:40px;padding:40px}
.cell{width:1920px}
.cap{font-size:26px;font-weight:500;margin:0 0 12px}
.cap span{color:#7A838E;font-weight:400}
.img{width:1920px;height:1080px;background:#000;overflow:hidden}
.img img{display:block}
"""


def sheet():
    cells = [
        ("Studio ink · Night", "150 % · 1920×1080", f"<img src='{ME_N}' width=1920 height=1080>"),
        ("FL Studio 24.1.2 · playlist", "1920×1020, as captured", f"<img src='{u(FL + 'fl-playlist.png')}'>"),
        ("Cantus beta.13 · startup.png", "1280×720 shown at 1.5", f"<img src='{OLD}' width=1920 height=1080>"),
        ("Studio ink · Paper", "150 % · 1920×1080", f"<img src='{ME_P}' width=1920 height=1080>"),
        ("FL Studio 24.1.2 · mixer", "1920×1020, as captured", f"<img src='{u(FL + 'fl-mixer.png')}'>"),
        ("Cantus beta.13 · the real app", "real-night.png, 150 %", f"<img src='{OLD_REAL}'>"),
    ]
    body = ("<div style='padding:40px 40px 0'><h1>Studio ink beside FL Studio and today's Cantus</h1>"
            "<div class='sub'>dir-c, round 39 · every picture at its physical pixels, side by side · private: "
            "shows FL Studio</div></div><div class='grid'>")
    for cap, sub, img in cells:
        body += f"<div class='cell'><div class='cap'>{cap} <span>· {sub}</span></div><div class='img'>{img}</div></div>"
    body += "</div>"
    return "<!doctype html><html><head><meta charset='utf-8'><style>" + CSS + "</style></head><body>" + body + "</body></html>"


# detail regions, in each picture's own physical pixels: (src, x, y, w, h)
DETAIL = [
    ("Transport, tempo, clock", [
        ("Studio ink", ME_N, 470, 8, 900, 72), ("FL Studio", u(FL + "fl-playlist.png"), 428, 0, 690, 52),
        ("Cantus beta.13", OLD_REAL, 620, 0, 640, 80)]),
    ("Mixer strips", [
        ("Studio ink", ME_N, 150, 640, 520, 400), ("FL Studio", u(FL + "fl-mixer.png"), 670, 390, 360, 560),
        ("Cantus beta.13", OLD_REAL, 680, 600, 480, 330)]),
    ("Clips: is the music readable?", [
        ("Studio ink", ME_N, 250, 160, 640, 430), ("FL Studio", u(FL + "fl-playlist.png"), 1436, 210, 460, 480),
        ("Cantus beta.13", OLD_REAL, 760, 230, 640, 300)]),
]


def detail():
    rows = ""
    for title, cuts in DETAIL:
        dirn = "column" if title.startswith("Transport") else "row"
        rows += f"<div class='cap' style='margin:30px 0 10px'>{title} <span>· same physical scale, magnified 2×</span></div><div style='display:flex;flex-direction:{dirn};gap:30px;align-items:flex-start'>"
        for name, src, x, y, w, h in cuts:
            rows += (f"<div><div style='color:#A2ABB5;font-size:22px;margin-bottom:8px'>{name}</div>"
                     f"<div style='width:{w * 2}px;height:{h * 2}px;overflow:hidden;position:relative;background:#000'>"
                     f"<img src='{src}' style='position:absolute;left:{-x}px;top:{-y}px;zoom:2;image-rendering:pixelated'></div></div>")
        rows += "</div>"
    body = ("<div style='padding:40px'><h1>Studio ink · details beside FL Studio</h1>"
            "<div class='sub'>cut at the same physical scale (FL as captured, Studio ink at 150 %), then magnified 2×</div>"
            + rows + "</div>")
    return "<!doctype html><html><head><meta charset='utf-8'><style>" + CSS + "</style></head><body>" + body + "</body></html>"


if __name__ == "__main__":
    os.makedirs(PRIV, exist_ok=True)
    W, H = 3 * 1920 + 4 * 40, 2 * (1080 + 50) + 3 * 40 + 110
    h = os.path.join(PRIV, "compare.html")
    write(h, sheet())
    render(h, os.path.join(PRIV, "compare.png"), W, H, 1)
    render(h, os.path.join(PRIV, "compare-overview.png"), W, H, 0.33)
    h2 = os.path.join(PRIV, "compare-detail.html")
    write(h2, detail())
    render(h2, os.path.join(PRIV, "compare-detail.png"), 3100, 2900, 1)
    print(PRIV)
