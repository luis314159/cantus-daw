"""Builds logo-variants-{night,paper}.png: each variant large, at 256, and at 48/32/24/16
on Slate's top bar, beside today's #2, #10 and #03. Run from anywhere:
python sheet.py  (writes the SVGs first, renders with Edge headless, crops with PIL)."""
import os
import shutil
import subprocess
import sys
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import large  # noqa: E402
import ref2  # noqa: E402
import small  # noqa: E402
from shot import shot  # noqa: E402

OUT = os.path.normpath(os.path.join(HERE, ".."))
SVG = os.path.join(OUT, "svg")
WORK = os.path.join(os.environ.get("LOGO_WORK", r"F:\projects\daw-r39\tmp\logo"), "sheet")
ASSETS = os.path.normpath(os.path.join(OUT, "..", "..", "..", "..", "crates", "cantus-ui",
                                       "assets", "logos"))
FONT = "file:///F:/projects/daw-r39/fonts/barlow-semi-condensed/"
SMALLS = (48, 32, 24, 16)
ZOOM = {48: 2, 32: 3, 24: 4, 16: 6}

ROWS = [
    ("relicario", "1 · Relicario",
     "A gilded capital C, bevelled and engraved with a line, over enamel with rays cut "
     "beneath it; a gold rim and a border of silver pearls; a pearl where the cross's "
     "arms meet."),
    ("astrolabio", "2 · Astrolabio",
     "The C is an instrument's limb, its face engraved with a scale; in its counter a "
     "silver dial carries #10's ticks round a gilded pin."),
    ("roseton", "3 · Roseton",
     "The most medieval: an uncial-weight C with punched gilding, a small rose window in "
     "its counter, a diaper ground, gold pearls, a cross whose arms flare."),
    ("grabado", "4 · Grabado",
     "The least ornament: an even-weight gilded C and the cut-gold cross under one gold "
     "rim. The calmest beside the interface."),
    ("ref02", "#2 · today's default", "As the app draws it (its small master below 48 px)."),
    ("ref10", "#10 · the start", "Flat navy, silver ticks, the gold cross."),
    ("ref03", "#03 · the metal", "The gilding and the lapis the owner liked."),
]

THEMES = {
    "night": dict(page="#14191E", card="#1D2329", bar="#262D34", well="#1A1F25", ink="#E3E8ED",
                  ink2="#B4BDC6", ink3="#909CA8", edge="#11151A"),
    "paper": dict(page="#C5CDD5", card="#F4F6F8", bar="#E4E9EE", well="#D6DDE3", ink="#17202A",
                  ink2="#38444F", ink3="#556170", edge="#B9C3CC"),
}


def url(p):
    return "file:///" + os.path.abspath(p).replace("\\", "/")


def write_svgs():
    os.makedirs(SVG, exist_ok=True)
    for v in large.VARIANTS:
        for s in (1024, 512, 256):
            open(os.path.join(SVG, "%s-%d.svg" % (v, s)), "w", newline="\n").write(
                large.build(v, s))
        for n in SMALLS:
            open(os.path.join(SVG, "%s-%d.svg" % (v, n)), "w", newline="\n").write(
                small.build(v, n))
    os.makedirs(WORK, exist_ok=True)
    for n in (32, 24, 16):
        open(os.path.join(WORK, "ref02-%d.svg" % n), "w").write(ref2.small_master(n))


def src(row, n):
    """The SVG the app (or this design) shows at n px for a row."""
    if row.startswith("ref"):
        num = row[3:]
        if num == "02" and n < 48:
            return os.path.join(WORK, "ref02-%d.svg" % n)
        return os.path.join(ASSETS, "%s.svg" % num)
    if n >= 1024:
        return os.path.join(SVG, "%s-1024.svg" % row)
    if n in (512, 256):
        return os.path.join(SVG, "%s-%d.svg" % (row, n))
    return os.path.join(SVG, "%s-%d.svg" % (row, n))


def pixels():
    """Render every small mark at 1x on a transparent page and crop each one out."""
    items, x = [], 4
    html = ['<!doctype html><style>html,body{margin:0;background:transparent}'
            'img{position:absolute;top:4px}</style>']
    for row, _, _ in ROWS:
        for n in SMALLS:
            html.append('<img src="%s" width="%d" height="%d" style="left:%dpx">'
                        % (url(src(row, n)), n, n, x))
            items.append((row, n, x))
            x += n + 4
    page = os.path.join(WORK, "pix.html")
    open(page, "w").write("".join(html))
    png = os.path.join(WORK, "pix.png")
    shot(page, png, x + 8, 64, 1)
    im = Image.open(png).convert("RGBA")
    for row, n, x0 in items:
        c = im.crop((x0, 4, x0 + n, 4 + n))
        c.save(os.path.join(WORK, "%s-%d.png" % (row, n)))
        z = ZOOM[n]
        c.resize((n * z, n * z), Image.NEAREST).save(os.path.join(WORK, "%s-%d-z.png" % (row, n)))


CSS = """
@font-face{font-family:B;src:url('%(f)sBarlowSemiCondensed-Regular.ttf')}
@font-face{font-family:B;font-weight:500;src:url('%(f)sBarlowSemiCondensed-Medium.ttf')}
@font-face{font-family:B;font-weight:600;src:url('%(f)sBarlowSemiCondensed-SemiBold.ttf')}
html,body{margin:0;background:%(page)s;color:%(ink)s;font-family:B,sans-serif}
.wrap{padding:22px 24px}
h1{font-size:22px;font-weight:600;margin:0 0 4px}
.sub{color:%(ink3)s;font-size:13px;margin-bottom:16px}
.row{display:grid;grid-template-columns:180px 260px 256px 318px 1fr;gap:18px;
 background:%(card)s;border-radius:8px;padding:12px 16px;margin-bottom:8px;align-items:center}
.row.ref{opacity:1}
.name{font-size:17px;font-weight:600}
.idea{font-size:12px;color:%(ink2)s;line-height:1.35;margin-top:6px}
.cap{font-size:10px;color:%(ink3)s;margin-top:4px;text-align:center}
.big img{width:260px;height:260px;display:block}
.mid img{width:256px;height:256px;display:block}
.bar{height:44px;background:%(bar)s;border-radius:4px;display:flex;align-items:center;
 padding:0 10px;gap:12px;box-shadow:inset 0 -1px 0 %(edge)s}
.bar .m{font-size:12px;color:%(ink2)s}
.bar .t{font-size:13px;font-weight:600;line-height:1.05}
.bar .t small{display:block;font-size:10px;font-weight:400;color:%(ink3)s}
.sizes{display:flex;align-items:flex-end;gap:16px;background:%(bar)s;border-radius:4px;
 padding:10px 12px 6px;margin-top:8px}
.sizes div{text-align:center;font-size:10px;color:%(ink3)s}
.sizes img{display:block;margin:0 auto 3px;image-rendering:pixelated}
.zoom{display:flex;gap:14px;align-items:flex-end}
.zoom div{text-align:center;font-size:10px;color:%(ink3)s}
.zoom img{display:block;image-rendering:pixelated;margin-bottom:3px}
"""


def build_sheet(theme):
    t = THEMES[theme]
    h = ['<!doctype html><meta charset="utf-8"><style>%s</style><div class="wrap">'
         % (CSS % dict(t, f=FONT))]
    h.append('<h1>#10, enriched — four variants</h1><div class="sub">Slate %s · large, the '
             '256-px drawing, the top bar (24 px) and the small marks at 1x, then their pixels '
             'magnified · today\'s #2, #10 and #03 below</div>' % theme.title())
    for row, name, idea in ROWS:
        ref = row.startswith("ref")
        big = src(row, 1024)
        mid = src(row, 256)
        h.append('<div class="row%s"><div><div class="name">%s</div><div class="idea">%s</div>'
                 '</div>' % (" ref" if ref else "", name, idea))
        h.append('<div class="big"><img src="%s"><div class="cap">1024 drawing</div></div>'
                 % url(big))
        h.append('<div class="mid"><img src="%s"><div class="cap">256 drawing, 1:1</div></div>'
                 % url(mid))
        bar = ('<div class="bar"><img src="%s" width="24" height="24"><span class="m">File</span>'
               '<span class="m">Edit</span><span class="m">Clip</span><span class="t">Neon Tide'
               '<small>9 tracks · 64 bars</small></span></div>'
               % url(os.path.join(WORK, "%s-24.png" % row)))
        sizes = "".join('<div><img src="%s" width="%d" height="%d">%d</div>'
                        % (url(os.path.join(WORK, "%s-%d.png" % (row, n))), n, n, n)
                        for n in SMALLS)
        h.append('<div>%s<div class="sizes">%s</div><div class="cap">on the top bar, 1x</div>'
                 '</div>' % (bar, sizes))
        zoom = "".join('<div><img src="%s">%d px ×%d</div>'
                       % (url(os.path.join(WORK, "%s-%d-z.png" % (row, n))), n, ZOOM[n])
                       for n in SMALLS)
        h.append('<div class="zoom">%s</div></div>' % zoom)
    h.append('</div>')
    page = os.path.join(WORK, "sheet-%s.html" % theme)
    open(page, "w", encoding="utf-8").write("".join(h))
    png = os.path.join(OUT, "logo-variants-%s.png" % theme)
    shot(page, png, 1600, 2270, 1)
    # Edge leaves the canvas transparent where the page has no background: flatten it.
    im = Image.open(png).convert("RGBA")
    bg = Image.new("RGBA", im.size, t["page"])
    Image.alpha_composite(bg, im).convert("RGB").save(png)


if __name__ == "__main__":
    write_svgs()
    pixels()
    for th in sys.argv[1:] or ["night", "paper"]:
        build_sheet(th)
