"""Each direction's sheet: in the window (Night, Paper), at the app's sizes, key poses,
moods, palette with measured contrast, construction. usage: python sheet.py [names…]"""
import sys
from anim import char, pose_at, OUTNAME
from render import page, shot, portable, REPO, DESIGN
from rig import Pose
from svg import wrap, f
from theme import NIGHT, PAPER, contrast
from sheet_text import INFO, PALETTES, CONSTRUCTION

W = 1600
WIN = {t: (REPO / "docs" / "round39" / "design" / "dir-a" / f"window-{t}-1x.png").as_uri()
       for t in ("night", "paper")}
SHEET_BG = "#14191E"


def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;")


def text(x, y, s, size=13, fill="#E3E8ED", weight=400, anchor="start"):
    return (f'<text x="{f(x)}" y="{f(y)}" fill="{fill}" font-size="{size}" font-weight="{weight}" '
            f'text-anchor="{anchor}">{esc(s)}</text>')


def para(x, y, s, width_chars, size=14, fill="#B4BDC6", lh=1.45):
    words, lines, cur = s.split(), [], ""
    for w in words:
        if len(cur) + len(w) + 1 > width_chars:
            lines.append(cur)
            cur = w
        else:
            cur = (cur + " " + w).strip()
    lines.append(cur)
    return "".join(text(x, y + i * size * lh, ln, size, fill) for i, ln in enumerate(lines)), len(lines) * size * lh


def window_row(ch, th, y, pose):
    """The whole 1280×720 window (round 39's Slate sheet) with the dancer floating over the
    mixer's empty strips: its default view, size 1 = 0.30 of the workspace's height."""
    t = th["name"].lower()
    size = 0.30 * 656
    out = [f'<image href="{WIN[t]}" x="0" y="{y}" width="1280" height="720"/>',
           wrap(ch.draw(pose, th, cut_color=th["base"]), 797, y + 676, size)]
    nx = 1300
    out.append(text(nx, y + 22, f"In the window — {th['name']}", 16, "#E3E8ED", 500))
    s, h = para(nx, y + 50, "Floating, its default view: transparent, over the workspace, size 1 "
                "stands 0.30 of the workspace's height — 197 px in a 1280 × 720 window (shown), "
                "295 px at 150 %. The cut is the room's colour, so it separates from what it "
                "stands on.", 38, 13)
    out.append(s)
    return "".join(out)


def tile(x, y, side, ch, th, pose, selected=False, label=None):
    out = [f'<rect x="{f(x)}" y="{f(y)}" width="{side}" height="{side}" rx="6" fill="{th["surface"]}"/>']
    if selected:
        out.append(f'<rect x="{f(x + 1)}" y="{f(y + 1)}" width="{side - 2}" height="{side - 2}" rx="5" '
                   f'fill="none" stroke="{th["accent"]}" stroke-width="2"/>')
    out.append(f'<svg x="{f(x)}" y="{f(y)}" width="{side}" height="{side}" overflow="hidden">'
               + wrap(ch.draw(pose, th, cut_color=th["surface"], floor=False), side / 2, side * 0.92, side * 0.80)
               + "</svg>")
    if label:
        out.append(text(x + side / 2, y + side + 15, label, 12, th["ink1"] if selected else th["ink2"],
                        500 if selected else 400, "middle"))
    return "".join(out)


def companion(x, y, w, h, ch, th, pose):
    """Slate's companion window (ADR 0091): the song, the bar, the dancer, a quiet level strip."""
    out = [f'<rect x="{f(x - 1)}" y="{f(y - 1)}" width="{w + 2}" height="{h + 2}" rx="7" fill="{th["gutter"]}"/>',
           f'<rect x="{f(x)}" y="{f(y)}" width="{w}" height="{h}" rx="6" fill="{th["base"]}"/>',
           text(x + 14, y + 24, "Neon Tide", 14, th["ink1"], 500),
           text(x + 14, y + 41, "124 BPM · bar 41", 12, th["ink3"]),
           text(x + w - 14, y + 24, "41.3", 14, th["gold"], 500, "end")]
    for n in range(4):
        out.append(f'<circle cx="{f(x + w - 62 + n * 14)}" cy="{f(y + 38)}" r="{3.2 if n == 2 else 2.4}" '
                   f'fill="{th["gold"] if n == 2 else th["ink4"]}"/>')
    out.append(wrap(ch.draw(pose, th, cut_color=th["base"]), x + w / 2, y + h * 0.80, h * 0.56))
    import math
    for n in range(28):
        v = 0.25 + 0.65 * abs(math.sin(n * 0.61) * math.cos(n * 0.23 + 1))
        bh = 26 * v
        out.append(f'<rect x="{f(x + 16 + n * (w - 32) / 28)}" y="{f(y + h - 14 - bh)}" width="{f((w - 32) / 28 - 3)}" '
                   f'height="{f(bh)}" rx="1.5" fill="{th["ink4"]}"/>')
        out.append(f'<rect x="{f(x + 16 + n * (w - 32) / 28)}" y="{f(y + h - 14 - bh)}" width="{f((w - 32) / 28 - 3)}" '
                   f'height="2" rx="1" fill="{th["gold"]}"/>')
    return "".join(out)


def sizes_row(ch, th, x0, y, rest, hero):
    out = [f'<rect x="{x0}" y="{y}" width="790" height="420" fill="{th["base"]}"/>',
           text(x0 + 20, y + 28, f"At the app's sizes — {th['name']}", 16, th["ink1"], 500)]
    out.append(text(x0 + 20, y + 54, "Picker tiles, 88 px", 12, th["ink3"]))
    out.append(tile(x0 + 20, y + 66, 88, ch, th, rest, False, ch.NAME))
    out.append(tile(x0 + 120, y + 66, 88, ch, th, hero, True, "chosen"))
    out.append(text(x0 + 20, y + 205, "48 px — the smallest it draws crisp", 12, th["ink3"]))
    out.append(f'<rect x="{x0 + 20}" y="{y + 215}" width="190" height="64" rx="6" fill="{th["base"]}"/>')
    for n, p in enumerate((rest, hero, pose_at(ch, "steps", 1.0))):
        out.append(wrap(ch.draw(p, th, cut_color=th["base"], floor=False), x0 + 50 + n * 64, y + 272, 48))
    out.append(text(x0 + 20, y + 312, "200 px — floating at 100 %", 12, th["ink3"]))
    s, _ = para(x0 + 20, y + 332, "(the window above). The stage tab and video draw it "
                "larger with the same pieces: it is vectors.", 34, 12, th["ink2"])
    out.append(s)
    out.append(text(x0 + 300, y + 54, "The companion window, 320 × 360", 12, th["ink3"]))
    out.append(companion(x0 + 300, y + 66, 320, 340, ch, th, hero))
    return "".join(out)


def strip(ch, th, y, items, label, cell=170, height=250, size=176):
    out = [f'<rect x="0" y="{y}" width="{W}" height="{height}" fill="{th["base"]}"/>',
           text(20, y + 26, f"{label} — {th['name']}", 16, th["ink1"], 500)]
    from dataclasses import replace
    for i, (name, p) in enumerate(items):
        cx = 100 + i * cell
        p = replace(p, lift=min(p.lift, 0.05))
        out.append(f'<svg x="0" y="{y + 34}" width="{W}" height="{height - 34}" overflow="hidden">'
                   + wrap(ch.draw(p, th, cut_color=th["base"]), cx, height - 62, size) + "</svg>")
        out.append(text(cx, y + height - 8, name, 12, th["ink3"], 400, "middle"))
    return "".join(out)


def moods_row(ch, th, y):
    """Faces at a readable size (the companion's), or postures for the faceless one."""
    from body import solve
    height = 230
    out = [f'<rect x="0" y="{y}" width="{W}" height="{height}" fill="{th["base"]}"/>',
           text(20, y + 26, f"Moods — {th['name']}", 16, th["ink1"], 500)]
    if ch.__name__ == "compas":
        from moves import star, pump, HIP
        items = [("Neutral", Pose(fk=((8, 4), (8, 4)))), ("Happy", pump(0.0, ch.STYLE)),
                 ("Excited", star(1.45, ch.STYLE)), ("Calm", Pose(fk=((10, 8), (10, 8)), tilt=10, hip=(0.03, 0.01))),
                 ("Surprised", Pose(fk=((150, 40), (150, 40)), nod=-10, feet=((-0.05, 0), (0.05, 0)))),
                 ("Sleepy", Pose(fk=((4, 0), (4, 0)), nod=30, tilt=16, hip=(0, 0.05), lean=4)),
                 ("Cool", Pose(feet=((-0.18, 0), (0.18, 0)), fk=(HIP, HIP), nod=-8, eyes="cool")),
                 ("Shy", Pose(fk=((20, -60), (20, -60)), tilt=-14, nod=12, knee=(-1, -1), feet=((0.03, 0), (-0.03, 0))))]
        from dataclasses import replace
        for i, (name, p) in enumerate(items):
            cx = 100 + i * 190
            p = replace(p, lift=min(p.lift, 0.05))
            out.append(f'<svg x="0" y="{y + 34}" width="{W}" height="{height - 34}" overflow="hidden">'
                       + wrap(ch.draw(p, th, cut_color=th["base"]), cx, height - 64, 140) + "</svg>")
            out.append(text(cx, y + height - 10, name, 12, th["ink3"], 400, "middle"))
        out.append(text(W - 20, y + 26, "No face: a mood is a posture; Cool cuts a visor into the disc.",
                        12, th["ink3"], 400, "end"))
        return "".join(out)
    if getattr(ch, "MOOD_FULL", False):
        from dataclasses import replace
        for i, (name, p) in enumerate(ch.mood_items()):
            cx = 100 + i * 190
            out.append(f'<svg x="0" y="{y + 34}" width="{W}" height="{height - 34}" overflow="hidden">'
                       + wrap(ch.draw(p, th, cut_color=th["base"]), cx, height - 64, 160) + "</svg>")
            out.append(text(cx, y + height - 10, name, 12, th["ink3"], 400, "middle"))
        out.append(text(W - 20, y + 26, "No face but cut eyes: a mood is the wave's shape and the eyes' cut.",
                        12, th["ink3"], 400, "end"))
        return "".join(out)
    items = ch.mood_items()
    hy = getattr(ch, "MOOD_FOCUS", None)
    if hy is None:
        hy = solve(Pose(), ch.PROPS).head[1]
    size = getattr(ch, "MOOD_SIZE", 400)
    for i, (name, p) in enumerate(items):
        x = 20 + i * 195
        cy = y + 128
        g = wrap(ch.draw(p, th, cut_color=th["base"], floor=False), 0, 0, size)
        out.append(f'<svg x="{x}" y="{y + 40}" width="180" height="170" overflow="hidden">'
                   f'<g transform="translate(90,{f(cy - y - 40 - hy * size)})">{g}</g></svg>')
        out.append(text(x + 90, y + height - 6, name, 12, th["ink3"], 400, "middle"))
    return "".join(out)


def palette_row(ch, y):
    height = 330
    info = INFO[ch.__name__]
    out = [f'<rect x="0" y="{y}" width="{W}" height="{height}" fill="{SHEET_BG}"/>',
           text(20, y + 28, "Palette — each colour measured on Slate's rooms", 16, "#E3E8ED", 500),
           text(20, y + 48, "contrast on Night's base #1D2329 · on Paper's base #F4F6F8 (a silhouette holds 3:1)",
                12, "#909CA8")]
    for i, (name, nh, ph) in enumerate(PALETTES[ch.__name__]):
        cx, cy = 20 + (i % 4) * 200, y + 66 + (i // 4) * 96
        out.append(f'<rect x="{cx}" y="{cy}" width="40" height="40" rx="4" fill="{nh}" stroke="#2B3239"/>')
        out.append(f'<rect x="{cx + 44}" y="{cy}" width="40" height="40" rx="4" fill="{ph}" stroke="#2B3239"/>')
        out.append(text(cx, cy + 58, name, 12, "#E3E8ED", 500))
        out.append(text(cx, cy + 74, f"{nh} {contrast(nh, '#1D2329'):.1f}:1 · {ph} {contrast(ph, '#F4F6F8'):.1f}:1",
                        11, "#909CA8"))
    out.append(text(840, y + 28, "Construction (units: the figure stands 1 tall)", 16, "#E3E8ED", 500))
    for i, line in enumerate(CONSTRUCTION[ch.__name__]):
        out.append(text(840, y + 58 + i * 22, "· " + line, 13, "#B4BDC6"))
    out.append(text(840, y + 58 + 6 * 22 + 18, "Every fill opaque, no gradient (ADR 0090 §2); light never turns with the hue.",
                    13, "#909CA8"))
    s, _ = para(840, y + 58 + 7 * 22 + 30, "Risk: " + info["risk"], 100, 13, "#B4BDC6")
    out.append(s)
    return "".join(out)


def build(name):
    ch = char(name)
    info = INFO[name]
    kp = ch.key_poses()
    rest = kp[0][1]
    hero = kp[1][1]
    win_pose = pose_at(ch, "groove", 0.5 if name != "compas" else 0.0)
    parts = [f'<rect width="{W}" height="4000" fill="{SHEET_BG}"/>',
             text(24, 44, f"{info['letter']} · {info['title']}", 30, "#E3E8ED", 500),
             text(W - 24, 40, "Cantus round 40 · the dancer cast for Slate · design only", 13, "#909CA8", 400, "end")]
    s, h1 = para(24, 80, info["idea"], 112, 15, "#B4BDC6")
    parts.append(s)
    parts.append(text(840, 80, "Why it fits Slate", 14, "#E3E8ED", 500))
    for i, w in enumerate(info["why"]):
        s, _ = para(840, 104 + i * 40, "· " + w, 100, 13, "#B4BDC6")
        parts.append(s)
    y = 270
    parts.append(window_row(ch, NIGHT, y, win_pose))
    y += 730
    parts.append(window_row(ch, PAPER, y, win_pose))
    y += 730
    parts.append(sizes_row(ch, NIGHT, 0, y, rest, hero))
    parts.append(sizes_row(ch, PAPER, 810, y, rest, hero))
    y += 430
    parts.append(strip(ch, NIGHT, y, kp, "Key poses"))
    y += 255
    parts.append(strip(ch, PAPER, y, kp, "Key poses"))
    y += 260
    parts.append(moods_row(ch, NIGHT, y))
    y += 235
    parts.append(moods_row(ch, PAPER, y))
    y += 240
    parts.append(palette_row(ch, y))
    y += 330
    svg = (f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{y}">' + "".join(parts) + "</svg>")
    html = page(svg, W, y, bg=SHEET_BG)
    (DESIGN / f"{OUTNAME.get(name, name).replace('_', '-')}-sheet.html").write_text(portable(html), encoding="utf-8")
    return shot(f"sheet-{name}", html, W, y, scale=2, out=DESIGN / f"{OUTNAME.get(name, name).replace('_', '-')}-sheet.png")


if __name__ == "__main__":
    for n in (sys.argv[1:] or ["compas", "tono", "sincopa"]):
        print(build(n))
