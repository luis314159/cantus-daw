"""Lay the atlases out as one sheet per theme: 1:1 first, then 4x crops of the real pixels."""
import os
import subprocess
import sys
from core import NIGHT, PAPER, font_css, write_build, OUT_DOCS, OUT_BUILD, EDGE
from parts import build, ALL


def uri(p):
    return "file:///" + p.replace("\\", "/")


def crop(png, W, x0, y0, x1, y1, z=4):
    return ('<div class="crop" style="width:%dpx;height:%dpx"><img src="%s" style="width:%dpx;'
            'margin-left:%dpx;margin-top:%dpx"></div>' % ((x1 - x0) * z, (y1 - y0) * z, uri(png),
                                                          W * z, -x0 * z, -y0 * z))


def img(png, W):
    return '<img class="one" src="%s" style="width:%dpx">' % (uri(png), W)


def sheet(t):
    a = build(t)
    night = t["name"] == "Night"
    bg, ink, ink3, line = t["canvas"], t["ink1"], t["ink3"], t["line"]
    k1, W1, H1, R1 = a["keys1"]
    k15, W15, H15, R15 = a["keys15"]
    s1, SW1, SH1, SR1 = a["states1"]
    s15, SW15, SH15, SR15 = a["states15"]
    m1, MW1, _, _ = a["menu1"]
    st1, STW1, _, _ = a["strip1"]
    st15, STW15, _, _ = a["strip15"]
    names = " · ".join(ALL)
    per = 30  # logical pitch of a key in the atlas
    def keycrop(png, W, R, k, s, n0, n1):
        x0, y0, x1, y1 = R[k]
        return crop(png, W, round((8 + n0 * per) * s) - 4, y0, round((8 + n1 * per) * s) - 2, y1)
    body = []
    body.append('<h1>Cantus — Graphite and gold <span>parts sheet · %s</span></h1>' % t["name"])
    body.append('<p class="lede">Every control drawn for its size. 1:1 is the screen; 4× is '
                'the same pixels magnified (nearest neighbour), not a vector blown up. Keys: dark '
                'wells, a 1-px rim, a light lip at the bottom; glyphs in ink-2 with a 1-px ink-1 edge '
                'where light falls from above; gold only when on.</p>')
    body.append('<h2>Keys at 16 · 20 · 24 px <span>100 %%</span></h2>%s' % img(k1, W1))
    body.append('<h2>Keys at 24 · 30 · 36 px <span>150 %% — drawn again for these pixels</span></h2>%s'
                % img(k15, W15))
    body.append('<p class="small">In order: %s. Lit: loop, magnet, mixer (on).</p>' % names)
    body.append('<h2>4× — transport and edit keys</h2><div class="row">')
    for k in (16, 20, 24):
        body.append('<figure>%s<figcaption>%d px · 100 %%</figcaption></figure>'
                    % (keycrop(k1, W1, R1, k, 1, 0, 11), k))
    body.append('</div><div class="row">')
    for k, p in ((16, 24), (20, 30), (24, 36)):
        body.append('<figure>%s<figcaption>%d px · 150 %%</figcaption></figure>'
                    % (keycrop(k15, W15, R15, k, 1.5, 0, 8), p))
    body.append('</div><h2>4× — windows, strip and track glyphs</h2><div class="row">')
    for k in (16, 24):
        body.append('<figure>%s<figcaption>%d px · 100 %%</figcaption></figure>'
                    % (keycrop(k1, W1, R1, k, 1, 20, 34), k))
    body.append('<figure>%s<figcaption>36 px · 150 %%</figcaption></figure>'
                % keycrop(k15, W15, R15, 24, 1.5, 20, 30))
    body.append('</div>')
    body.append('<h2>States, knobs, faders, small controls <span>1:1 at 100 %% and 150 %%</span></h2>'
                '<div class="row"><figure>%s<figcaption>100 %%</figcaption></figure></div>'
                '<div class="row"><figure>%s<figcaption>150 %%</figcaption></figure></div>'
                % (img(s1, SW1), img(s15, SW15)))
    x0, y0, x1, y1 = SR1["knobs"]
    fx0, fy0, fx1, fy1 = SR1["faders"]
    kx0, ky0, kx1, ky1 = SR1["keys"]
    body.append('<h2>4× — a knob (rest, hover, dragged, automated) and the fader with its meter '
                '(silent, playing, clipping)</h2><div class="row"><figure>%s</figure><figure>%s'
                '</figure></div>' % (crop(s1, SW1, x0, y0, x1, y1), crop(s1, SW1, fx0, fy0, fx1, fy1)))
    body.append('<div class="row"><figure>%s<figcaption>key states: rest · hover · pressed · on · '
                'disabled</figcaption></figure></div>' % crop(s1, SW1, kx0, ky0, kx1, ky1, 3))
    body.append('<h2>A mixer strip, a menu with its submenu</h2><div class="row">'
                '<figure>%s<figcaption>strip · 100 %%</figcaption></figure>'
                '<figure>%s<figcaption>strip · 150 %%</figcaption></figure>'
                '<figure>%s<figcaption>strip, upper half · 3×</figcaption></figure>'
                '<figure>%s<figcaption>menu + submenu · 100 %%</figcaption></figure></div>'
                % (img(st1, STW1), img(st15, STW15), crop(st1, STW1, 0, 0, STW1, 150, 3),
                   img(m1, MW1)))
    css = ("%s\nbody{margin:0;padding:40px 48px;background:%s;color:%s;font-family:'SofiaSemi';"
           "width:1900px}h1{font-family:'Sofia';font-weight:300;font-size:40px;margin:0 0 8px}"
           "h1 span{font-size:18px;color:%s;font-weight:500;margin-left:12px}"
           "h2{font-size:17px;font-weight:650;margin:36px 0 12px;padding-top:14px;border-top:1px solid %s}"
           "h2 span{color:%s;font-weight:500;font-size:14px;margin-left:8px}"
           ".lede{font-size:15px;color:%s;max-width:1100px;line-height:1.45}.small{font-size:12px;color:%s}"
           ".row{display:flex;gap:28px;align-items:flex-start;flex-wrap:wrap;margin:10px 0}"
           "figure{margin:0}figcaption{font-size:12px;color:%s;margin-top:6px}"
           ".crop{overflow:hidden;outline:1px solid %s}.crop img{image-rendering:pixelated;display:block}"
           "img.one{display:block;image-rendering:pixelated}"
           % (font_css(), bg, ink, ink3, line, ink3, t["ink2"], ink3, ink3, line))
    html = '<!doctype html><html><head><meta charset="utf-8"><style>%s</style></head><body>%s</body></html>' % (
        css, "".join(body))
    name = "parts-%s" % t["name"].lower()
    src = write_build(name, html)
    out = os.path.join(OUT_DOCS, name + ".png")
    prof = os.path.join(OUT_BUILD, "edge-profile")
    subprocess.run([EDGE, "--headless=new", "--disable-gpu", "--hide-scrollbars",
                    "--force-device-scale-factor=1", "--user-data-dir=" + prof,
                    "--allow-file-access-from-files", "--virtual-time-budget=4000",
                    "--window-size=1996,%d" % 5000, "--screenshot=" + out,
                    "file:///" + src.replace("\\", "/")], check=True, capture_output=True, timeout=180)
    print(out)


if __name__ == "__main__":
    for t in (NIGHT, PAPER):
        if len(sys.argv) < 2 or t["name"].lower() in sys.argv[1:]:
            sheet(t)
