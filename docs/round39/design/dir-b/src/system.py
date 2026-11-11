"""The system on one page: tokens with contrast, the type, spacing, radii, lines, states."""
import os
from core import (NIGHT, PAPER, HUES, Canvas, contrast, mix, font_css, write_build, OUT_DOCS,
                  OUT_BUILD, EDGE, clip_tones)
from kit import key, segmented, field
from kit2 import toggle, meter, fader
import subprocess

SURF = [("well", "fields, meters, the clock's well"), ("canvas", "the arrangement's lanes"),
        ("lane_alt", "every other four bars"), ("panel", "mixer, inspector, headers"),
        ("chrome", "top bar, status line"), ("raised", "selection, popovers, caps"),
        ("key", "a key at rest — a well, darker than its bar")]
INKS = [("ink1", "names, values"), ("ink2", "labels, glyph bodies"), ("ink3", "units, hints"),
        ("ink4", "disabled, empty")]
LINES = [("line_soft", "beats, rows"), ("line", "dividers, bars"), ("line_strong", "popover rim"),
         ("key_edge", "a key's rim")]
STATE = [("gold", "on, selected, focus, the playhead, loop — from the logo's playhead"),
         ("teal", "signal, healthy, an effect on — from the logo's patina"),
         ("red", "record, clip"), ("ember", "warning — from the logo's wave")]


def sw(hexv, big=False):
    return '<span class="sw%s" style="background:%s"></span>' % (" big" if big else "", hexv)


def ratio(a, b):
    r = contrast(a, b)
    cls = "ok" if r >= 4.5 else ("mid" if r >= 3 else "low")
    return '<span class="r %s">%.1f</span>' % (cls, r)


def theme_col(t):
    rows = []
    rows.append("<h3>Surfaces</h3><table>")
    for k, what in SURF:
        rows.append("<tr><td>%s</td><td class=tok>%s</td><td class=hex>%s</td><td>ink-1 %s</td>"
                    "<td class=what>%s</td></tr>" % (sw(t[k]), k.replace("_", "-"), t[k],
                                                     ratio(t["ink1"], t[k]), what))
    rows.append("</table><h3>Ink <span>contrast against panel · canvas · chrome</span></h3><table>")
    for k, what in INKS:
        rows.append("<tr><td>%s</td><td class=tok>%s</td><td class=hex>%s</td><td>%s %s %s</td>"
                    "<td class=what>%s</td></tr>" % (sw(t[k]), k.replace("ink", "ink-"), t[k],
                                                     ratio(t[k], t["panel"]), ratio(t[k], t["canvas"]),
                                                     ratio(t[k], t["chrome"]), what))
    rows.append("</table><h3>Lines</h3><table>")
    for k, what in LINES:
        rows.append("<tr><td>%s</td><td class=tok>%s</td><td class=hex>%s</td><td>%s</td>"
                    "<td class=what>%s</td></tr>" % (sw(t[k]), k.replace("_", "-"), t[k],
                                                     ratio(t[k], t["panel"]), what))
    rows.append("</table><h3>Colour only for state <span>against panel · key</span></h3><table>")
    for k, what in STATE:
        rows.append("<tr><td>%s</td><td class=tok>%s</td><td class=hex>%s</td><td>%s %s</td>"
                    "<td class=what>%s</td></tr>" % (sw(t[k]), k, t[k], ratio(t[k], t["panel"]),
                                                     ratio(t[k], t["key"]), what))
    rows.append("</table><h3>The user's colours <span>lit only in a stripe and the content; the body "
                "muted — content against its body</span></h3><div class=hues>")
    for k, v in HUES.items():
        body, band, ink, edge = clip_tones(t, v)
        rows.append('<div class=hue><div class=clipx style="background:%s;border-color:%s">'
                    '<div style="background:%s;color:%s">%s</div><i style="background:%s"></i>'
                    '<i style="background:%s;width:40%%;margin-left:30%%"></i></div>'
                    '<small>%s · %s %s</small></div>' % (body, edge, band, ink, k, ink, ink, v,
                                                          ratio(ink, body), ""))
    rows.append("</div>")
    # the states of a key and of the small controls, drawn by the kit
    c = Canvas(560, 92, 1.5, t)
    c.rect(0, 0, 560, 92, t["chrome"])
    x = 10
    for st in ("rest", "hover", "pressed", "on", "disabled"):
        key(c, x, 10, 34, 30, g="play" if st != "on" else "loop", gsize=11 if st != "on" else 13,
            state=st)
        key(c, x + 40, 10, 52, 30, label="Solo", state=st, lsize=11.5)
        c.text(x + 46, 58, st, size=10.5, fill=t["ink3"], anchor="middle")
        x += 108
    c.text(10, 82, "A key is a well: rest a touch darker than its bar; hover lifts it; pressed "
           "sinks it; on is gold.", size=10.5, fill=t["ink2"])
    rows.append("<h3>Button states <span>drawn by the kit at 150 %%</span></h3>%s" % c.svg())
    return "".join(rows)


TYPE = [("Clock", "SofiaXC", 200, 34, "41.1.000", "Extra Condensed 200 · tabular · the position"),
        ("Display", "SofiaXC", 300, 23, "124.00  −3.5 dB", "Extra Condensed 300 · tempo, the big value"),
        ("Title", "SofiaSemi", 650, 14, "Chords   Metronome", "Semi Condensed 650 · panel heads"),
        ("Name", "SofiaSemi", 600, 12.5, "Percussion  Neon Tide", "Semi Condensed 600 · tracks, inserts"),
        ("Body", "SofiaSemi", 500, 12, "File  Edit  Clip   Grid  1 bar", "Semi Condensed 500 · menus, labels"),
        ("Strip", "SofiaCond", 600, 11.5, "Percussion  Comp · EQ  → Drums", "Condensed 600 · narrow places"),
        ("Small", "SofiaSemi", 500, 10.5, "insert 6 · Chorda   1:17.42", "Semi Condensed 500 · secondary"),
        ("Micro", "SofiaSemi", 650, 8, "BAR · BEAT · TICK   LUFS   TIME", "650, tracked +0.4 · units over values (never below 8)")]


def page_html():
    tcols = "".join('<div class="col %s"><h2>Cantus %s</h2>%s</div>' % (
        t["name"].lower(), t["name"], theme_col(t)) for t in (NIGHT, PAPER))
    types = "".join('<tr><td class=tok>%s</td><td style="font-family:%s;font-weight:%d;font-size:%spx;'
                    'font-feature-settings:\'tnum\' 1">%s</td><td class=what>%s px · %s</td></tr>'
                    % (n, f, w, s, txt, s, d) for n, f, w, s, txt, d in TYPE)
    N, P = NIGHT, PAPER
    css = """%s
body{margin:0;padding:40px 48px;background:#1B1B1A;color:#ECEAE4;font-family:'SofiaSemi';width:1904px}
h1{font-family:'Sofia';font-weight:300;font-size:42px;margin:0}h1 span{font-size:18px;color:#8F8B84;margin-left:12px;font-weight:500}
.lede{font-size:15.5px;color:#ABA79F;max-width:1250px;line-height:1.5;margin:10px 0 26px}
.cols{display:flex;gap:28px}.col{flex:1;padding:22px 26px;border-radius:8px}
.col.night{background:%s;color:%s}.col.paper{background:%s;color:%s}
h2{font-size:22px;font-weight:650;margin:0 0 6px}h3{font-size:14.5px;font-weight:650;margin:22px 0 8px}
h3 span{font-weight:500;opacity:.6;font-size:12.5px;margin-left:6px}
table{border-collapse:collapse;font-size:13px}td{padding:3px 10px 3px 0;vertical-align:middle}
.tok{font-weight:650;min-width:90px}.hex{font-family:'SofiaSemi';font-feature-settings:'tnum' 1;opacity:.85}
.what{opacity:.6}.sw{display:inline-block;width:34px;height:20px;border-radius:3px;border:1px solid rgba(128,128,128,.35)}
.r{display:inline-block;min-width:32px;font-feature-settings:'tnum' 1;font-weight:650}.r.ok{color:#5CC2A9}.r.mid{color:#D9B04A}.r.low{color:#E07A6A}
.paper .r.ok{color:#1C8670}.paper .r.mid{color:#9C6A0E}.paper .r.low{color:#B8402E}
.hues{display:flex;flex-wrap:wrap;gap:10px}.hue{width:150px;font-size:11.5px}
.clipx{height:34px;border:1px solid;border-radius:3px;overflow:hidden}.clipx div{height:11px;font-size:9px;font-weight:650;padding:0 4px;line-height:11px}
.clipx i{display:block;height:3px;margin:4px 6px 0;width:55%%;border-radius:1px}
.sys{display:flex;gap:28px;margin-top:28px}.box{flex:1;background:#232322;border-radius:8px;padding:22px 26px}
.box table td{font-size:13.5px}.grid{display:flex;gap:4px;align-items:flex-end;margin:8px 0}
.grid div{background:#E6B450;opacity:.8}
""" % (font_css(), N["panel"], N["ink1"], P["panel"], P["ink1"])
    spacing = "".join('<div style="width:%dpx;height:%dpx"></div>' % (v, v) for v in (2, 4, 8, 12, 16, 24, 32))
    body = """<h1>Cantus — Graphite and gold <span>the system · direction dir-b</span></h1>
<p class="lede">A neutral graphite (R = G = B + 1 at most: neither brown nor blue) and Cantus's own
warm metal taken from the logo — gold from its playhead, teal from its patina, ember from its wave.
One humanist family in three widths gives the type its character: a large thin clock, condensed names
for narrow strips, a semi-condensed body. Colour is spent on state and on the user's own colours,
which are muted in bodies and lit only in stripes and content. Ratios are WCAG 2 contrast:
<b style="color:#5CC2A9">≥ 4.5</b> body text, <b style="color:#D9B04A">≥ 3</b> large text and
non-text, <b style="color:#E07A6A">&lt; 3</b> only for decoration, rules and disabled.</p>
<div class="cols">%s</div>
<div class="sys"><div class="box"><h3>Type — Sofia Sans, one family in three widths</h3>
<table>%s</table><p class="what" style="font-size:12.5px;line-height:1.5;margin-top:12px">
Sofia Sans, Sofia Sans Semi Condensed, Sofia Sans Condensed, Sofia Sans Extra Condensed — version 4.101,
variable weight 1–1000, tabular figures (tnum) and small caps. © 2019 The Sofia Sans Project Authors,
SIL Open Font License 1.1. Source: https://github.com/lettersoup/Sofia-Sans (as published in
https://github.com/google/fonts/tree/main/ofl/sofiasans*). Downloaded to F:\\projects\\daw-r39\\fonts\\sofia.</p></div>
<div class="box"><h3>Spacing, radii, lines, density</h3><table>
<tr><td class=tok>Grid</td><td>4 px base; 8 between groups; 12 panel padding; 16 popover padding</td></tr>
<tr><td></td><td><div class="grid">%s</div><span class=what>2 · 4 · 8 · 12 · 16 · 24 · 32</span></td></tr>
<tr><td class=tok>Radii</td><td>3 keys, fields, segments · 2.5 clips and fader caps · 5 popovers · full for toggles</td></tr>
<tr><td class=tok>Lines</td><td>1 px everywhere (a physical pixel at every zoom); 2 px only for the active tab's rule
and the selected track's edge; glyph strokes 1 / 1.5 / 2 px by size</td></tr>
<tr><td class=tok>Depth</td><td>none on keys (a rim and a lip, no gradient); one soft shadow, only under popovers
and fader caps</td></tr>
<tr><td class=tok>Heights</td><td>top bar 50 · toolbar 30 · ruler 26 · track 34 · mixer tab 26 · strip 260 · status 22</td></tr>
<tr><td class=tok>Keys</td><td>16 / 20 / 24 px (and 24 / 30 / 36 at 150 %%); transport 32 in the top bar.
Glyph box: transport ≈ ⅓ of the key, tools and windows ≈ ½</td></tr>
<tr><td class=tok>Strips</td><td>62 px (master 72), names whole in the condensed face; no label column</td></tr>
<tr><td class=tok>At rest</td><td>no −∞: meters are a quiet scale; the master shows the last peak, dimmed;
LUFS shows a dash</td></tr></table></div></div>""" % (tcols, types, spacing)
    return '<!doctype html><html><head><meta charset="utf-8"><style>%s</style></head><body>%s</body></html>' % (css, body)


if __name__ == "__main__":
    src = write_build("system", page_html())
    out = os.path.join(OUT_DOCS, "system.png")
    subprocess.run([EDGE, "--headless=new", "--disable-gpu", "--hide-scrollbars",
                    "--force-device-scale-factor=1", "--user-data-dir=" + os.path.join(OUT_BUILD, "edge-profile"),
                    "--virtual-time-budget=3000", "--window-size=2000,2050", "--screenshot=" + out,
                    "file:///" + src.replace("\\", "/")], check=True, capture_output=True, timeout=180)
    print(out)
