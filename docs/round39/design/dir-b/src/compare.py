"""The comparison beside FL Studio (private: it shows FL) — written to
research/ui/round39/dir-b/. FL's screenshots are 1920 px at 150 %; ours is drawn at 1.5x
(1920 x 1080), so the crops are at the same physical scale. Beta.13's startup.png is 1280 px
at 100 %, shown enlarged 1.5x (smooth) and labelled so."""
import os
import subprocess
from core import NIGHT, PAPER, page, render, font_css, write_build, OUT_DOCS, OUT_BUILD, EDGE
from window import window

FL = r"F:/projects/cantus/research/ui/fl-look-2026-11/screenshots/"
HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "..", "..", "..", "..", ".."))
OUT = os.path.join(REPO, "research", "ui", "round39", "dir-b")
B13 = os.path.join(REPO, "docs", "manual", "img", "startup.png")


def u(p):
    return "file:///" + p.replace("\\", "/")


def whole(src, w_img, cap, scale=1.0):
    return ('<figure><img src="%s" style="width:%dpx"><figcaption>%s</figcaption></figure>'
            % (u(src), w_img, cap))


def crop(src, iw, x, y, w, h, cap, k=1.0):
    """A crop at physical scale: `k` enlarges the source first (1.5 for beta.13)."""
    return ('<figure><div class="crop" style="width:%dpx;height:%dpx"><img src="%s" '
            'style="width:%dpx;margin-left:%dpx;margin-top:%dpx"></div><figcaption>%s</figcaption>'
            '</figure>' % (w, h, u(src), iw * k, -x, -y, cap))


def main():
    os.makedirs(OUT, exist_ok=True)
    ours = os.path.join(OUT_BUILD, "cmp-ours-night-15.png")
    play = os.path.join(OUT_BUILD, "cmp-ours-night-play-15.png")
    paper = os.path.join(OUT_BUILD, "cmp-ours-paper-15.png")
    for path, t, pl in ((ours, NIGHT, False), (play, NIGHT, True), (paper, PAPER, False)):
        c = window(t, 1.5, playing=pl)
        render(os.path.basename(path)[:-4], page(c.svg(), t["panel"]), 1920, 1080, path)
    fl_p, fl_m = FL + "fl-playlist.png", FL + "fl-mixer.png"
    W = 900
    rows = []
    rows.append('<h2>Whole windows <span>each fitted to the same width</span></h2><div class=row>%s%s%s</div>'
                % (whole(fl_p, W, "FL Studio 24.1.2 — playlist (150 %)"),
                   whole(ours, W, "Cantus · Graphite and gold — Night, at rest (1.5×)"),
                   whole(B13, W, "Cantus beta.13 today — startup.png")))
    rows.append('<div class=row>%s%s%s</div>'
                % (whole(fl_m, W, "FL Studio — mixer (150 %)"),
                   whole(play, W, "Graphite and gold — Night, playing (1.5×)"),
                   whole(paper, W, "Graphite and gold — Paper, at rest (1.5×)")))
    rows.append('<h2>Same physical scale <span>the transport, tempo and clock</span></h2><div class=col>%s%s%s</div>'
                % (crop(fl_p, 1920, 380, 0, 900, 52, "FL Studio"),
                   crop(ours, 1920, 395, 0, 1000, 75, "Graphite and gold"),
                   crop(B13, 1280, 680, 0, 1000, 64, "beta.13 (enlarged 1.5×)", 1.5)))
    rows.append('<h2>Same physical scale <span>mixer strips</span></h2><div class=row>%s%s%s</div>'
                % (crop(fl_m, 1920, 545, 385, 600, 590, "FL Studio"),
                   crop(play, 1920, 0, 655, 600, 420, "Graphite and gold, playing"),
                   crop(B13, 1280, 0, 560, 600, 520, "beta.13 (enlarged 1.5×)", 1.5)))
    rows.append('<h2>Same physical scale <span>track headers and clips</span></h2><div class=row>%s%s%s</div>'
                % (crop(fl_p, 1920, 1020, 200, 900, 420, "FL Studio"),
                   crop(ours, 1920, 0, 155, 900, 420, "Graphite and gold"),
                   crop(B13, 1280, 0, 140, 900, 420, "beta.13 (enlarged 1.5×)", 1.5)))
    css = ("%s\nbody{margin:0;padding:36px 40px;background:#141414;color:#ECEAE4;font-family:'SofiaSemi';"
           "width:2760px}h1{font-family:'Sofia';font-weight:300;font-size:40px;margin:0}"
           "h1 span{font-size:17px;color:#8E8A83;margin-left:10px}p{color:#B3AFA7;font-size:15px;max-width:1500px}"
           "h2{font-size:18px;font-weight:650;margin:30px 0 12px}h2 span{color:#8E8A83;font-weight:500;"
           "font-size:14px;margin-left:8px}.row{display:flex;gap:24px;align-items:flex-start;margin-bottom:18px}"
           ".col{display:flex;flex-direction:column;gap:12px}figure{margin:0}figcaption{font-size:13px;"
           "color:#8E8A83;margin-top:6px}img{display:block}.crop{overflow:hidden;outline:1px solid #333}"
           % font_css())
    body = ('<h1>Graphite and gold beside FL Studio <span>private — research/ui/round39/dir-b · FL '
            'shown for reference, nothing traced</span></h1><p>FL Studio\'s screenshots are 1920 px '
            'wide at 150 %%; ours is drawn at 1.5× for the same pixels. Beta.13\'s picture is 100 %%, '
            'enlarged here for the same scale (it is softer for that reason, not for its drawing).</p>%s'
            % "".join(rows))
    html = '<!doctype html><html><head><meta charset="utf-8"><style>%s</style></head><body>%s</body></html>' % (css, body)
    src = write_build("compare", html)
    out = os.path.join(OUT, "compare-fl.png")
    subprocess.run([EDGE, "--headless=new", "--disable-gpu", "--hide-scrollbars",
                    "--force-device-scale-factor=1", "--user-data-dir=" + os.path.join(OUT_BUILD, "edge-profile"),
                    "--virtual-time-budget=4000", "--window-size=2840,2780", "--screenshot=" + out,
                    u(src)], check=True, capture_output=True, timeout=180)
    print(out)


if __name__ == "__main__":
    main()
