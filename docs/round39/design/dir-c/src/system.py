"""The system on one page: palette tokens with hex and contrast, the type and its scale,
the spacing grid, radii, line weights, density, and a key's five states — both themes."""
import os
import sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import Svg, page, render, write, OUT, MONO, SANS, contrast, mix
from tokens import NIGHT, PAPER, USER, hue
from top import key, caption
from overflow import toggle
from metrics import width

PW, PH = 1400, 1590


def swatch_rows(g, t, x, y, w):
    """The palette of one theme on its own ground."""
    g.box(x, y, w, 790, t["base"], t["hair"], rx=8)
    g.text(x + 20, y + 32, f"Cantus {t['name']}", 16, t["ink1"], 500)
    g.text(x + w - 20, y + 32, "contrast is against the surface named", 10.5, t["ink3"], 400, anchor="end")
    groups = [
        ("Surfaces", [("well", "lanes, graphs: where content lives", "base"),
                      ("base", "panels", None), ("bar", "top bar, tab rows, status", "base"),
                      ("raised", "hover, selected row and strip", "base"), ("field", "fields, pressed keys", "base"),
                      ("float", "menus, the overflow panel", "base"), ("track", "meter, fader and slider slots", "base")]),
        ("Lines", [("hair", "between areas — 1 physical px", "base"), ("line", "control outlines, menu edges", "base"),
                   ("grid_bar", "a bar line in the lanes", "well")]),
        ("Inks", [("ink1", "names, values", "base"), ("ink2", "secondary text, glyphs at rest", "base"),
                  ("ink3", "captions, units", "base"), ("ink4", "disabled, ghosts", "base")]),
        ("State — colour means state, nothing else", [
            ("accent", "on, selected, focus", "base"), ("accent_bg", "the ground of a key that is on", "base"),
            ("play", "playing", "bar"), ("rec", "record, arm", "bar"), ("solo", "solo", "base"),
            ("clip", "clipping", "track")]),
        ("Meter", [("m_lo", "below −12 dB", "track"), ("m_mid", "−12 to −3 dB", "track"), ("m_hi", "above −3 dB", "track")]),
    ]
    yy = y + 58
    for title, rows in groups:
        g.text(x + 20, yy, title, 11.5, t["ink2"], 500)
        yy += 10
        for name, use, against in rows:
            c = t[name]
            g.box(x + 20, yy, 44, 20, c, t["line"] if name in ("well", "base", "float", "field") else None, rx=3)
            g.text(x + 76, yy + 14, name.replace("_", "-"), 11.5, t["ink1"], 500)
            g.text(x + 150, yy + 14, c.upper(), 11, t["ink2"], 400, fam=MONO)
            g.text(x + 222, yy + 14, use, 11, t["ink3"], 400)
            if against:
                cr = contrast(c, t[against])
                g.text(x + w - 20, yy + 14, f"{cr:.2f} : 1  vs {against}", 11, t["ink2"], 400, fam=MONO, anchor="end")
            yy += 24
        yy += 12
    # the user's colour spent per role
    g.text(x + 20, yy, "The user's colour, spent by role (Chords)", 11.5, t["ink2"], 500)
    yy += 10
    c = USER["chords"]
    for i, role in enumerate(["stripe", "body", "edge", "note", "cap"]):
        cx = x + 20 + i * ((w - 40) / 5)
        rc = hue(t, c, role)
        g.box(cx, yy, 44, 20, rc, t["line"], rx=3)
        g.text(cx + 52, yy + 9, role, 11, t["ink1"], 500)
        g.text(cx + 52, yy + 21, rc.upper(), 10, t["ink3"], 400, fam=MONO)
    yy += 30
    cr = contrast(hue(t, c, "note"), hue(t, c, "body"))
    g.text(x + 20, yy + 10, f"notes on their clip body: {cr:.2f} : 1 — at rest muted, lit (selected): "
           f"{contrast(hue(t, c, 'note', True), hue(t, c, 'body', True)):.2f} : 1", 11, t["ink3"], 400)


def states(g, t, x, y, w):
    g.box(x, y, w, 132, t["bar"], t["hair"], rx=8)
    g.text(x + 20, y + 26, f"A key, {t['name']}", 12, t["ink1"], 500)
    sts = [("rest", "Rest"), ("hover", "Hover"), ("pressed", "Pressed"), ("on", "On"), ("disabled", "Disabled")]
    for i, (st, lab) in enumerate(sts):
        cx = x + 20 + i * ((w - 40) / 5)
        g.box(cx, y + 42, 30, 28, t["well"], t["hair"], rx=5) if st == "rest" else None
        key(g, t, cx, y + 42, 30, 28, "loop", st, gk=24)
        # a text key in the same state
        f = {"rest": t["well"], "hover": t["raised"], "pressed": t["field"], "on": t["accent_bg"], "disabled": t["well"]}[st]
        ln = {"rest": t["hair"], "hover": t["line"], "pressed": t["line"], "on": None, "disabled": t["hair"]}[st]
        tc = {"rest": t["ink2"], "hover": t["ink1"], "pressed": t["ink1"], "on": t["accent"], "disabled": t["ink4"]}[st]
        g.box(cx + 38, y + 44, 58, 24, f, ln, rx=4)
        g.text(cx + 67, y + 60, "Snap", 11.5, tc, 500, anchor="middle")
        g.text(cx, y + 96, lab, 11.5, t["ink1"], 500)
        desc = {"rest": "a well, no ground on a bar", "hover": "raised, ink-1",
                "pressed": "the field tone", "on": "accent on its own ground", "disabled": "ink-4, no hover"}[st]
        g.text(cx, y + 114, desc, 10.5, t["ink3"], 400)


def type_block(g, t, x, y, w):
    g.box(x, y, w, 520, t["base"], t["hair"], rx=8)
    g.text(x + 20, y + 32, "Type", 16, t["ink1"], 500)
    g.text(x + 20, y + 54, "IBM Plex Sans Condensed 1.3 and IBM Plex Mono 2.3 — SIL Open Font License 1.1, "
           "© IBM Corp., Reserved Font Name “Plex”", 11, t["ink2"], 400)
    g.text(x + 20, y + 72, "Source: github.com/google/fonts — ofl/ibmplexsanscondensed, ofl/ibmplexmono (IBM/plex)", 11, t["ink3"], 400)
    rows = [("Clock", "1.1.000", 31, 200, SANS, "ExtraLight 31 — the position, read across a room"),
            ("Tempo, metre", "124.00  4/4", 22, 300, SANS, "Light 22"),
            ("Title", "Chords", 17, 500, SANS, "Medium 17 — the inspector's subject"),
            ("Project", "Neon Tide", 13, 500, SANS, "Medium 13 — names that lead a panel; tabs 12"),
            ("Track name", "Percussion", 12.5, 400, SANS, "Regular 12.5, Medium when selected"),
            ("Body", "Count-in · Customise the top bar…", 12, 400, SANS, "Regular 12 — menus, rows, settings"),
            ("Secondary", "keyed by Kick · 10 tracks · 13 strips", 11, 400, SANS, "Regular 11, ink-2 or ink-3"),
            ("Strip text", "Comprimo · Aequalis", 10, 400, SANS, "Regular 10 — the chain in a strip"),
            ("Caption", "BAR · BEAT · TICK", 8.5, 500, SANS, "Medium 8.5, capitals tracked 0.9 — units only"),
            ("Numbers", "−3.5 −16.0 dB 220 Hz Alt+F8", 12, 400, MONO, "Plex Mono 10–12, tabular — values that change")]
    yy = y + 104
    for lab, sample, size, wt, fam, note in rows:
        g.text(x + 20, yy + 4, lab, 11, t["ink3"], 400)
        g.text(x + 140, yy + 4, sample, size, t["ink1"], wt, fam=fam, ls=0.9 if lab == "Caption" else 0)
        g.text(x + w - 20, yy + 4, note, 11, t["ink3"], 400, anchor="end")
        g.hline(x + 20, x + w - 20, yy + 14, t["hair"])
        yy += 40 if size < 20 else 50


def grid_block(g, t, x, y, w):
    g.box(x, y, w, 520, t["base"], t["hair"], rx=8)
    g.text(x + 20, y + 32, "Space, corners, lines, density", 16, t["ink1"], 500)
    # spacing
    g.text(x + 20, y + 62, "Grid: 4 px; gaps of 4, 8, 12, 16, 24", 11.5, t["ink2"], 500)
    xx = x + 20
    for s in (4, 8, 12, 16, 24):
        g.rect(xx, y + 74, s, 24, t["accent"], op=0.7)
        g.text(xx + s / 2, y + 112, str(s), 10, t["ink3"], 400, fam=MONO, anchor="middle")
        xx += s + 22
    # radii
    g.text(x + 20, y + 156, "Corners", 11.5, t["ink2"], 500)
    for i, (r, use) in enumerate([(3, "clips, small keys"), (4, "keys, fields"), (6, "groups, wells"), (8, "floating panels")]):
        bx = x + 20 + i * 150
        g.box(bx, y + 168, 40, 28, t["raised"], t["line"], rx=r)
        g.text(bx + 50, y + 180, f"{r} px", 10.5, t["ink1"], 400, fam=MONO)
        g.text(bx + 50, y + 194, use, 10, t["ink3"], 400)
    # lines
    g.text(x + 20, y + 232, "Lines", 11.5, t["ink2"], 500)
    for i, (lw, use) in enumerate([(1, "hairline: 1 physical px at every zoom"), (1.5, "knob track"),
                                   (2, "fader slot, loop rule"), (3, "meter bar")]):
        ly = y + 250 + i * 18
        g.rect(x + 20, ly, 80, lw, t["ink2"])
        g.text(x + 112, ly + 4, f"{lw} px — {use}", 10.5, t["ink3"], 400)
    # density
    g.text(x + 20, y + 346, "Density (logical px at 100 %)", 11.5, t["ink2"], 500)
    dens = [("top bar", "52"), ("tab row", "28"), ("ruler", "24"), ("track row", "32"), ("clip", "28"),
            ("mixer strip", "68 × 273"), ("inspector", "244 wide"), ("key", "24–36 × 22–30"), ("status", "22")]
    for i, (k, v) in enumerate(dens):
        cx = x + 20 + (i % 5) * ((w - 40) / 5)
        cy = y + 374 + (i // 5) * 42
        g.text(cx, cy, v, 15, t["ink1"], 300)
        g.text(cx, cy + 16, k, 10.5, t["ink3"], 400)
    g.text(x + 20, y + 470, "Glyphs: about a third of their key, drawn per size (16, 20, 24, 30, 36 px); "
           "strokes 1–2 px by size;", 11, t["ink3"], 400)
    g.text(x + 20, y + 488, "two inks — the body and its edges; colour only when on, playing or recording.", 11, t["ink3"], 400)


def system():
    t = NIGHT
    g = Svg(PW, PH, 1)
    g.rect(0, 0, PW, PH, "#0A0B0D")
    g.text(32, 46, "Studio ink — the system", 24, t["ink1"], 500)
    g.text(32 + width("Studio ink — the system", 24, 500) + 14, 46,
           "content is light, chrome is ink; colour means state", 13, t["ink3"], 400)
    w = (PW - 32 * 2 - 24) / 2
    swatch_rows(g, NIGHT, 32, 70, w)
    swatch_rows(g, PAPER, 32 + w + 24, 70, w)
    states(g, NIGHT, 32, 880, w)
    states(g, PAPER, 32 + w + 24, 880, w)
    type_block(g, NIGHT, 32, 1036, w)
    grid_block(g, NIGHT, 32 + w + 24, 1036, w)
    return g


if __name__ == "__main__":
    g = system()
    html = os.path.join(OUT, "html", "system.html")
    write(html, page(g.svg(), "#0A0B0D", PW, PH))
    png = os.path.join(OUT, "system.png")
    render(html, png, PW, PH, 1)
    print(png)
