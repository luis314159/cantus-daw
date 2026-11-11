"""The contact sheet of every theme on the same view (round 39, *themes*): the full shots
`shoot_themes` writes (`<dir>/full/<id>.png`), reduced to tiles, four to a row, in the order
Settings ▸ Appearance lists them, each with its name and light.

    python tools/themes/contact_sheet.py docs/round39/themes
"""

import re
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[2]

NAMES = {
    "slate_night": "Slate Night", "slate_paper": "Slate Paper",
    "slate_lapis_night": "Slate Lapis Night", "slate_lapis_paper": "Slate Lapis Paper",
    "graphite_night": "Graphite Night", "graphite_paper": "Graphite Paper",
    "studio_ink_night": "Studio Ink Night", "studio_ink_paper": "Studio Ink Paper",
    "cantus_night": "Cantus Night", "cantus_paper": "Cantus Paper",
    "blue_hour": "Blue Hour", "daylight": "Daylight", "slate": "Pewter", "paper": "Chalk",
    "ink_gold": "Ink & Gold", "parchment": "Parchment", "ultraviolet": "Ultraviolet",
    "lagoon": "Lagoon", "evergreen_night": "Evergreen Night",
    "evergreen_paper": "Evergreen Paper", "bordeaux": "Bordeaux", "print": "Print",
    "contrast": "High contrast", "contrast_light": "High contrast light",
}
NEW = {"evergreen_night", "evergreen_paper", "bordeaux", "print"}
RESTORED = {"cantus_night", "cantus_paper", "blue_hour", "daylight", "slate", "paper",
            "ink_gold", "parchment", "ultraviolet", "lagoon"}


def preset_ids():
    src = (ROOT / "crates/cantus-ui/src/kit/appearance.rs").read_text(encoding="utf-8")
    body = src[src.index("pub const PRESETS"):]
    body = body[: body.index("];")]
    return re.findall(r'\("([a-z_]+)",', body)


def font(size):
    for f in ["C:/Windows/Fonts/segoeui.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"]:
        try:
            return ImageFont.truetype(f, size)
        except OSError:
            pass
    return ImageFont.load_default()


def main(out_dir):
    out_dir = Path(out_dir)
    ids = [i for i in preset_ids() if (out_dir / "full" / f"{i}.png").exists()]
    cols, tw, th, label, gap = 4, 640, 360, 34, 16
    rows = (len(ids) + cols - 1) // cols
    sheet = Image.new("RGB", (cols * tw + (cols + 1) * gap, rows * (th + label) + (rows + 1) * gap),
                      (44, 46, 50))
    draw = ImageDraw.Draw(sheet)
    f = font(20)
    for n, id_ in enumerate(ids):
        shot = Image.open(out_dir / "full" / f"{id_}.png").convert("RGB")
        shot = shot.resize((tw, th), Image.LANCZOS)
        x = gap + (n % cols) * (tw + gap)
        y = gap + (n // cols) * (th + label + gap)
        sheet.paste(shot, (x, y))
        tag = " (redrawn)" if id_ in RESTORED else " (new)" if id_ in NEW else ""
        draw.text((x + 2, y + th + 6), NAMES.get(id_, id_) + tag, fill=(236, 238, 240), font=f)
    path = out_dir / "contact-sheet.png"
    sheet.save(path, optimize=True)
    print(path)


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "docs/round39/themes")
