"""Edge headless rendering and the page shell (fonts: Barlow Semi Condensed, SIL OFL 1.1,
the app's own copy in crates/cantus-ui/assets/fonts)."""
import os
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
DESIGN = HERE.parent
BUILD = DESIGN / "build"
REPO = DESIGN.parents[3]
FONTS = (REPO / "crates" / "cantus-ui" / "assets" / "fonts").as_uri()
EDGE = r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"

FONT_CSS = "\n".join(
    f"@font-face{{font-family:'Barlow SC';src:url('{FONTS}/BarlowSemiCondensed-{n}.ttf');font-weight:{w}}}"
    for n, w in (("ExtraLight", 200), ("Light", 300), ("Regular", 400), ("Medium", 500),
                 ("SemiBold", 600)))


def page(body, w, h, bg="#14191E", css=""):
    return f"""<!doctype html><html><head><meta charset="utf-8"><style>
{FONT_CSS}
html,body{{margin:0;padding:0;background:{bg};width:{w}px;height:{h}px;overflow:hidden;
font-family:'Barlow SC',sans-serif;font-feature-settings:'tnum' 1;-webkit-font-smoothing:antialiased}}
{css}
</style></head><body>{body}</body></html>"""


def shot(name, html, w, h, scale=2, out=None):
    BUILD.mkdir(exist_ok=True)
    src = BUILD / f"{name}.html"
    src.write_text(html, encoding="utf-8")
    out = Path(out) if out else BUILD / f"{name}.png"
    subprocess.run([EDGE, "--headless=new", "--disable-gpu", "--disable-lcd-text",
                    "--hide-scrollbars", f"--force-device-scale-factor={scale}",
                    f"--window-size={w},{h}", f"--screenshot={out}", src.as_uri()],
                   check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return out


def portable(html):
    """The page as committed beside its PNG: paths into the repository made relative to
    docs/round40/design/dancer/, so it opens on any machine."""
    return html.replace(REPO.as_uri() + "/", "../../../../")
