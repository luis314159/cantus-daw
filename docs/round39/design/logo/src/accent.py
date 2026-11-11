"""Three accent directions for Slate taken from logo #03, each rendered as the whole
window with dir-a's own renderer (imported from its worktree, not copied, not changed):
only the accent tokens change, and #03's small master stands in the top bar.

python src/accent.py  ->  accent/window-<opt>-<theme>-<scale>.png and the two sheets.
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
DIRA = r"F:\projects\daw-r39\dir-a\docs\round39\design\dir-a\src"
sys.path.insert(0, HERE)
sys.path.insert(1, DIRA)

import base  # noqa: E402  (dir-a)
import glyphs as G  # noqa: E402  (dir-a)
import arrange  # noqa: E402  (dir-a)
import topbar2  # noqa: E402  (dir-a)
import main as dmain  # noqa: E402  (dir-a)
import m03  # noqa: E402
from colour import contrast, de2000  # noqa: E402

OUT = os.path.normpath(os.path.join(HERE, "..", "accent"))
WORK = r"F:\projects\daw-r39\tmp\logo\accent"

# ---- the options -----------------------------------------------------------------------
# Each: per theme, the tokens that change. `playhead` (option C only) takes the ruler's
# loop band, the playhead's cap and its line; everything else that was `accent` stays so.
OPTIONS = {
    "lapis": dict(
        title="A · Lapis",
        idea="#03's tile, lifted to read on Slate: a deeper, more saturated blue than "
             "today's azure, every accent role in one colour.",
        night=dict(accent="#7ea2f6", accent_ink="#0b1430"),
        paper=dict(accent="#2443a8", accent_ink="#ffffff")),
    "gold": dict(
        title="B · Gold of the C",
        idea="The C's gilding as the accent. Rejected: it is solo's amber (ΔE00 3 in Night, "
             "12 in Paper), so a lit loop or snap reads as a soloed track.",
        night=dict(accent="#e2b84a", accent_ink="#1f1606"),
        paper=dict(accent="#7a5a14", accent_ink="#ffffff")),
    "lapis-gilt": dict(
        title="C · Lapis, gilt time",
        idea="Lapis for every state (loop key, snap, focus, open window); a pale gilt for "
             "the playhead and the loop range only, as hairlines and a cap, never a lit key.",
        night=dict(accent="#7ea2f6", accent_ink="#0b1430", playhead="#f2d892"),
        paper=dict(accent="#2443a8", accent_ink="#ffffff", playhead="#6e5414")),
}
SURF = ["gutter", "base", "surface", "raised", "well"]
STATES = ["solo", "rec", "play", "auto", "clip"]


def theme(opt, which):
    t0 = base.NIGHT if which == "night" else base.PAPER
    return dict(t0, **OPTIONS[opt][which])


# ---- patches into dir-a's renderer (in memory only) -----------------------------------
_ruler, _lanes = arrange.ruler, arrange.lanes


def ruler(t, *a, **k):
    if "playhead" in t:
        t = dict(t, accent=t["playhead"])
    return _ruler(t, *a, **k)


def lanes(t, *a, **k):
    if "playhead" in t:
        t = dict(t, accent=t["playhead"])
    return _lanes(t, *a, **k)


arrange.ruler, arrange.lanes = ruler, lanes


def logo_at(x, y, n):
    s = G.SCALE[0]
    px = round(n * s)
    px = min(m03.SPEC, key=lambda k: abs(k - px))
    body = m03.svg(px, uid="l%d" % px)
    body = body.replace('<svg xmlns="http://www.w3.org/2000/svg" ',
                        '<svg x="%g" y="%g" ' % (round(x * s), round(y * s)))
    return '<g transform="scale(%.6f)">%s</g>' % (1 / s, body)


topbar2.logo_at = logo_at


def render_windows():
    from shot import shot
    os.makedirs(OUT, exist_ok=True)
    os.makedirs(WORK, exist_ok=True)
    for opt in OPTIONS:
        for which in ("night", "paper"):
            t = theme(opt, which)
            for scale, tag in ((1.0, "1x"), (1.25, "1.25x")):
                G.SCALE[0] = scale
                html = base.page(dmain.svg(t, STATE), 1280, 720, t["gutter"])
                page = os.path.join(WORK, "w-%s-%s-%s.html" % (opt, which, tag))
                open(page, "w", encoding="utf-8").write(html)
                shot(page, os.path.join(OUT, "window-%s-%s-%s.png" % (opt, which, tag)),
                     1280, 720, scale)
    G.SCALE[0] = 1.0


def table(opt, which):
    """Rows: token, hex, contrast on each surface, distance to each state colour."""
    t = theme(opt, which)
    rows = []
    toks = ["accent"] + (["playhead"] if "playhead" in t else [])
    for k in toks:
        c = t[k]
        on = {s: contrast(c, t[s]) for s in SURF}
        on["key on"] = contrast(c, base.mix(t["well"], c, 0.20))
        if k == "playhead":  # it sits only on the lanes (base) and the ruler (surface)
            on = {s: (v if s in ("base", "surface") else None) for s, v in on.items()}
        de = {s: de2000(c, t[s]) for s in STATES}
        rows.append((k, c, on, de))
    ink = contrast(t["accent_ink"], t["accent"])
    return rows, ink


# A soloed track in the window, so the accent can be judged beside solo's amber.
STATE = dict(dmain.IDLE, tracks={"Lead": "s"})

CSS = """
@font-face{font-family:B;src:url('%(f)sBarlowSemiCondensed-Regular.ttf')}
@font-face{font-family:B;font-weight:500;src:url('%(f)sBarlowSemiCondensed-Medium.ttf')}
@font-face{font-family:B;font-weight:600;src:url('%(f)sBarlowSemiCondensed-SemiBold.ttf')}
html,body{margin:0;background:%(page)s;color:%(ink)s;font-family:B,sans-serif}
.wrap{padding:20px 24px}
h1{font-size:22px;font-weight:600;margin:0 0 4px}
.sub{color:%(ink3)s;font-size:13px;margin-bottom:12px;max-width:1500px;line-height:1.4}
.rec{background:%(card)s;border-radius:8px;padding:10px 14px;font-size:14px;margin-bottom:12px}
.sec{display:grid;grid-template-columns:360px 1280px;gap:20px;background:%(card)s;
 border-radius:8px;padding:14px 16px;margin-bottom:10px}
.name{font-size:18px;font-weight:600}
.verdict{font-size:12px;font-weight:600;margin:4px 0 6px}
.idea{font-size:12px;color:%(ink2)s;line-height:1.4}
table{border-collapse:collapse;font-size:11px;margin-top:10px;width:100%%}
td,th{padding:2px 4px;text-align:right;font-variant-numeric:tabular-nums}
th{color:%(ink3)s;font-weight:500}
td:first-child,th:first-child{text-align:left}
.sw{display:inline-block;width:12px;height:12px;border-radius:3px;vertical-align:-2px;margin-right:4px}
.low{color:#d6322b;font-weight:600}
.crop{margin-top:10px}
.crop img{display:block;width:340px;height:198px;image-rendering:pixelated}
.cap{font-size:10px;color:%(ink3)s;margin-top:3px}
.win img{display:block;width:1280px;height:720px;border-radius:4px}
"""
SHEET = {
    "night": dict(page="#14191e", card="#1d2329", ink="#e3e8ed", ink2="#b4bdc6", ink3="#909ca8"),
    "paper": dict(page="#c5cdd5", card="#f4f6f8", ink="#17202a", ink2="#38444f", ink3="#556170"),
}
VERDICT = {"lapis": "Recommended", "gold": "Rejected: collides with solo",
           "lapis-gilt": "Possible, with a rule"}
RECOMMEND = ("<b>Recommendation: A · Lapis.</b> It is #03's own ground, made to read on Slate. "
             "It stays clear of every state colour (ΔE00 ≥ 15; the nearest is automation's "
             "violet) and passes 4.5:1 on every surface, the lit key included. C carries the "
             "gold into the timeline and is the richer look. But in Night its playhead sits "
             "11 ΔE from solo, so it holds only while gold stays a hairline and never fills a "
             "key. In Paper the gilt has to darken to bronze to read on white. B is out, "
             "because solo is already that gold.")


def section(opt, which):
    o = OPTIONS[opt]
    rows, ink = table(opt, which)
    h = ['<div class="sec"><div><div class="name">%s</div><div class="verdict">%s</div>'
         '<div class="idea">%s</div>' % (o["title"], VERDICT[opt], o["idea"])]
    h.append('<table><tr><th>contrast on</th>' + "".join(
        "<th>%s</th>" % k for k in list(rows[0][2])) + "</tr>")
    for k, c, on, de in rows:
        h.append('<tr><td><span class="sw" style="background:%s"></span>%s %s</td>%s</tr>' % (
            c, k, c, "".join(('<td class="%s">%.1f</td>' % ("low" if v < 4.5 else "", v)) if v else
                             '<td>—</td>' for v in on.values())))
    h.append('<tr><td>ink on accent</td><td colspan="6" style="text-align:left">%.1f : 1</td>'
             '</tr></table>' % ink)
    h.append('<table><tr><th>ΔE00 from</th>' + "".join("<th>%s</th>" % s for s in STATES)
             + "</tr>")
    for k, c, on, de in rows:
        h.append('<tr><td>%s</td>%s</tr>' % (k, "".join(
            '<td class="%s">%.0f</td>' % ("low" if v < 10 else "", v) for v in de.values())))
    h.append('</table>')
    crop = os.path.join(WORK, "crop-%s-%s.png" % (opt, which))
    h.append('<div class="crop"><img src="file:///%s"><div class="cap">1.25×, real pixels: #03 at 30 px in '
             'the top bar; the loop, metronome and snap keys; the loop range and playhead</div></div></div>'
             % crop.replace("\\", "/"))
    h.append('<div class="win"><img src="file:///%s"></div></div>' % os.path.join(
        OUT, "window-%s-%s-1x.png" % (opt, which)).replace("\\", "/"))
    return "".join(h)


def sheets():
    from PIL import Image
    from shot import shot
    for opt in OPTIONS:
        for which in ("night", "paper"):
            im = Image.open(os.path.join(OUT, "window-%s-%s-1.25x.png" % (opt, which)))
            parts = [im.crop(b) for b in ((0, 0, 340, 64), (540, 0, 880, 64), (400, 88, 740, 150))]
            cr = Image.new("RGB", (340, 64 + 64 + 62 + 8), SHEET[which]["card"])
            y = 0
            for p in parts:
                cr.paste(p, (0, y))
                y += p.size[1] + 4
            cr.save(os.path.join(WORK, "crop-%s-%s.png" % (opt, which)))
    for which in ("night", "paper"):
        c = SHEET[which]
        h = ['<!doctype html><meta charset="utf-8"><style>%s</style><div class="wrap">'
             % (CSS % dict(c, f="file:///F:/projects/daw-r39/fonts/barlow-semi-condensed/"))]
        h.append('<h1>An accent for #03 — Slate %s</h1><div class="sub">Each window is dir-a\'s '
                 'Slate with only the accent tokens changed and #03\'s small master in the '
                 'top bar; Lead is soloed so the accent can be judged beside solo. Contrast is '
                 'WCAG on each surface ("key on" = the glyph on its lit key); a red figure is '
                 'under 4.5:1, or under 10 ΔE00 from a state colour. Full windows at 1× and '
                 '1.25×: accent/window-*.png.</div>' % which.title())
        h.append('<div class="rec">%s</div>' % RECOMMEND)
        for opt in OPTIONS:
            h.append(section(opt, which))
        h.append('</div>')
        page = os.path.join(WORK, "sheet-%s.html" % which)
        open(page, "w", encoding="utf-8").write("".join(h))
        png = os.path.join(OUT, "..", "accent-options-%s.png" % which)
        shot(page, png, 1730, 2440, 1)
        im = Image.open(png).convert("RGBA")
        bg = Image.new("RGBA", im.size, c["page"])
        Image.alpha_composite(bg, im).convert("RGB").save(png)


if __name__ == "__main__":
    dmain.IDLE = STATE
    render_windows()
    sheets()
