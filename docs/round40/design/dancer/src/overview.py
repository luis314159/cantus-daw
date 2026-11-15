"""The page to open first: the three directions side by side, Night over Paper, each on
Slate's stage in two poses, with its one-line idea."""
from dataclasses import replace
from anim import char, pose_at, stage
from render import page, shot, portable, DESIGN
from sheet import text, para, SHEET_BG
from sheet_text import INFO
from svg import wrap
from theme import NIGHT, PAPER

LINE = {
    "compas": "A sign figure come alive: one ink, one gold accent, pop and lock on the beat. Approved.",
    "tono": "v2: a knob-headed robot with an LED visor and a patch-cable scarf. Signature: Turn it up.",
    "sincopa": "v2: streetwear in the logo's colours, braids that swing, swagger. Signature: Shades.",
    "bajo": "New: a studio monitor, heavy and calm; tweeter eyes, a pumping woofer. Signature: Drop the bass.",
    "onda": "New: a living ribbon, lapis and gold as it twists; faceless. Signature: Square drop.",
    "voz": "New: a mic-headed crooner whose dinner jacket inverts with the theme. Signature: Hat tip.",
    "sincopa_s3": "Chosen: noir — one ink, a gold rim light, a hood, two glints. Signature: Arm wave.",
    "glissando": "New: a figure of light — lapis neon on a dark suit, light trails. Signature: Halo.",
    "eco": "New: the delay's ghost — its echoes repeat each move late. Signature: Feedback.",
    "vinilo": "New: a turntable DJ — a record for a head, the tonearm for an arm. Signature: Backspin.",
    "cinta": "New: an 80s cassette b-boy, its tape swirling. Signature: Rewind.",
    "metronomo": "New: a wooden metronome with a butler's dignity. Signature: Doble tiempo.",
    "espectro": "New: a body of spectrum bars — its body is the music. Signature: Pico.",
    "onda_o1": "Chosen: the oscilloscope — a phosphor trace, a Lissajous head. Signature: Square.",
}


def build_cast():
    """The whole cast in blocks of six: two poses each (its groove, its signature), Night over Paper."""
    from anim import CHARS
    cw, ph, per = 300, 380, 6
    W = cw * per
    sig_at = {"onda_o1": 0.5, "sincopa_s3": 0.9, "glissando": 0.98, "eco": 0.9, "vinilo": 1.2, "cinta": 0.7,
              "metronomo": 0.85, "espectro": 1.08}
    parts = [f'<rect width="{W}" height="4000" fill="{SHEET_BG}"/>',
             text(24, 42, "The dancer cast for Slate — round 40, twelve", 28, "#E3E8ED", 500),
             text(W - 24, 40, "design only · the owner chooses what is built", 13, "#909CA8", 400, "end")]
    y0 = 70
    for blk in range(0, len(CHARS), per):
        names = CHARS[blk:blk + per]
        for i, name in enumerate(names):
            ch = char(name)
            x = i * cw
            parts.append(text(x + 16, y0 + 24, f"{INFO[name]['letter']} · {ch.NAME}", 18, "#E3E8ED", 500))
            s, _ = para(x + 16, y0 + 46, LINE[name], 46, 12, "#B4BDC6")
            parts.append(s)
            for r, th in enumerate((NIGHT, PAPER)):
                y = y0 + 90 + r * (ph + 8)
                parts.append(f'<svg x="{x + 5}" y="{y}" width="{cw - 10}" height="{ph}" overflow="hidden">'
                             + stage(0, 0, cw - 10, ph, th, th["name"], "", 0.0, scale=1.0))
                for k, (mv, b) in enumerate((("groove", 0.0), ("jump", sig_at.get(name, 1.3)))):
                    p = pose_at(ch, mv, b)
                    p = replace(p, lift=min(p.lift, 0.04))
                    tr = None
                    if getattr(ch, "ECHO", False):
                        tr = [pose_at(ch, mv, b - 0.25 * n) for n in (1, 2, 3)]
                    parts.append(wrap(ch.draw(p, th, cut_color=th["base"], trails=tr), 75 + k * 140, ph * 0.86, ph * 0.46))
                parts.append("</svg>")
        y0 += 90 + 2 * (ph + 8) + 24
    H = y0
    svg = f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}">' + "".join(parts) + "</svg>"
    html = page(svg, W, H, bg=SHEET_BG)
    (DESIGN / "overview-cast.html").write_text(portable(html), encoding="utf-8")
    return shot("overview-cast", html, W, H, scale=2, out=DESIGN / "overview-cast.png")


def build_variants(which="sincopa"):
    """A character's variants, each in its groove and its signature, Night over Paper."""
    from gifs import VARIANTS, ONDA_VARIANTS
    VAR = VARIANTS if which == "sincopa" else ONDA_VARIANTS
    cw, ph = 390, 430
    W = cw * len(VAR)
    title = ("Síncopa — three new answers beside v2" if which == "sincopa"
             else "Onda — three new answers beside the ribbon")
    sub_ = ("the human dancer of the cast" if which == "sincopa" else "a character that is a waveform")
    parts = [f'<rect width="{W}" height="1300" fill="{SHEET_BG}"/>',
             text(24, 42, title, 28, "#E3E8ED", 500),
             text(W - 24, 40, sub_ + " · design only · the owner chooses", 13, "#909CA8", 400, "end")]
    lines = {"onda": "The ribbon: lapis and gold as it twists, faceless (the owner: not this).",
             "onda_o1": "The oscilloscope: a phosphor trace, a Lissajous head; Square.",
             "onda_o2": "The wave-jelly: its top is the waveform — a crown, then shark teeth; Shape shift.",
             "onda_o3": "The surfer: rides its own wave as it turns square, then saw; Square set.","sincopa": "v2: streetwear, braids, swagger; Shades on the drop (the owner: not yet).",
             "sincopa_s1": "Graphic: flat geometric shapes, a vermilion bob; Spin and point.",
             "sincopa_s2": "The sidekick: big head, a face that acts, energy; Hype jump.",
             "sincopa_s3": "Noir: one ink, a gold rim light, a hood, two glints; Arm wave."}
    for i, (name, title) in enumerate(VAR):
        ch = char(name)
        x = i * cw
        parts.append(text(x + 16, 84, title, 18, "#E3E8ED", 500))
        s, _ = para(x + 16, 108, lines[name], 60, 12, "#B4BDC6")
        parts.append(s)
        for r, th in enumerate((NIGHT, PAPER)):
            y = 140 + r * (ph + 10)
            parts.append(f'<svg x="{x + 6}" y="{y}" width="{cw - 12}" height="{ph}" overflow="hidden">'
                         + stage(0, 0, cw - 12, ph, th, th["name"], "", 0.0, scale=1.0))
            sig_b = {"sincopa": 1.3, "sincopa_s1": 1.3, "sincopa_s2": 1.15, "sincopa_s3": 0.9, "onda": 1.0,
                     "onda_o1": 0.5, "onda_o2": 1.4, "onda_o3": 1.5}[name]
            for k, (mv, b) in enumerate((("groove", 0.5), ("jump", sig_b))):
                p = pose_at(ch, mv, b)
                p = replace(p, lift=min(p.lift, 0.05))
                parts.append(f'<svg x="{k * (cw - 12) / 2}" y="0" width="{(cw - 12) / 2}" height="{ph}" overflow="hidden">'
                             + wrap(ch.draw(p, th, cut_color=th["base"]), (cw - 12) / 4, ph * 0.86, ph * 0.50) + "</svg>")
            parts.append("</svg>")
    H = 140 + 2 * (ph + 10)
    svg = f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}">' + "".join(parts) + "</svg>"
    html = page(svg, W, H, bg=SHEET_BG)
    name = f"{which}-variants"
    (DESIGN / f"{name}.html").write_text(portable(html), encoding="utf-8")
    return shot(name, html, W, H, scale=2, out=DESIGN / f"{name}.png")


if __name__ == "__main__":
    import sys
    if "onda-variants" in sys.argv:
        print(build_variants("onda"))
    elif "variants" in sys.argv:
        print(build_variants())
    else:
        print(build_cast())
