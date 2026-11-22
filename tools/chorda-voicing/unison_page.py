"""The owner's page of every unison before and after a retune (round 41, chorda-tune).

    python -I tools/chorda-voicing/unison_page.py <before.json> <after.json> <out.html>

Each JSON is what `chorda::tests_unison::unison_table` writes with `CHORDA_UNISON_JSON` (one
object per key: the fastest audible beat on partials 1–3 and its depth, the swells heard in
five seconds, the widest strings alone, the unison together's partials). The page: one row
per key, the beat rate and the swells before and after, drawn as bars against the bound;
self-contained, light and dark.
"""

import html
import json
import sys

NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"]
NAMED = {64, 49, 50, 48, 47, 76}


def name(k):
    return f"{NAMES[k % 12]}{k // 12 - 1}"


def bar(v, top, cls):
    w = 0 if v is None else max(0.0, min(1.0, v / top)) * 100
    return f'<span class="bar {cls}" style="width:{w:.1f}%"></span>'


def main():
    if len(sys.argv) < 4:
        sys.exit(__doc__)
    before = {r["key"]: r for r in json.load(open(sys.argv[1], encoding="utf-8"))}
    after = {r["key"]: r for r in json.load(open(sys.argv[2], encoding="utf-8"))}
    rows = []
    for k in range(21, 109):
        b, a = before.get(k), after.get(k)
        if not b or not a:
            continue
        f = lambda x, p=3: "–" if x is None else f"{x:.{p}f}"
        rows.append(
            "<tr class='{cls}'><th>{n} <small>{k}</small></th><td>{s}</td>"
            "<td class='num'>{bb}</td><td class='viz'>{bbar}</td>"
            "<td class='num'>{ab}</td><td class='viz'>{abar}</td>"
            "<td class='num'>{bc}</td><td class='num'>{ac}</td>"
            "<td class='num'>{bs}</td><td class='num'>{as_}</td></tr>".format(
                cls="named" if k in NAMED else "",
                n=html.escape(name(k)), k=k, s=a["strings"],
                bb=f(b["beat"]), bbar=bar(b["beat"], 6.0, "was"),
                ab=f(a["beat"]), abar=bar(a["beat"], 6.0, "now"),
                bc=f(b.get("cycles"), 2), ac=f(a.get("cycles"), 2),
                bs=f(b["spread"]), as_=f(a["spread"]),
            )
        )
    page = PAGE.replace("__ROWS__", "\n".join(rows))
    with open(sys.argv[3], "w", encoding="utf-8") as fh:
        fh.write(page)
    print(f"wrote {sys.argv[3]} ({len(rows)} keys)")


PAGE = """<!doctype html>
<html lang="es"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Chorda unisons</title>
<style>
:root { --bg:#f7f6f2; --fg:#1d1d1b; --muted:#5b5a55; --line:#d9d6cc; --was:#c2653a; --now:#2f7d5b; --hi:#fff3d6; }
@media (prefers-color-scheme: dark) { :root { --bg:#17181a; --fg:#ecebe6; --muted:#a8a69e; --line:#34363a; --was:#e08a5f; --now:#5cc398; --hi:#3a3220; } }
body { background:var(--bg); color:var(--fg); font:15px/1.45 system-ui,sans-serif; margin:0; padding:24px 16px; }
main { max-width:1000px; margin:0 auto; }
h1 { font-size:1.35rem; margin:0 0 6px; }
p { color:var(--muted); margin:0 0 14px; max-width:70ch; }
.wrap { overflow-x:auto; }
table { border-collapse:collapse; width:100%; min-width:720px; }
th, td { padding:4px 6px; border-bottom:1px solid var(--line); text-align:left; white-space:nowrap; }
thead th { color:var(--muted); font-weight:600; font-size:.85rem; vertical-align:bottom; }
td.num { text-align:right; font-variant-numeric:tabular-nums; }
td.viz { width:18%; }
.bar { display:inline-block; height:9px; border-radius:2px; vertical-align:middle; }
.bar.was { background:var(--was); } .bar.now { background:var(--now); }
tr.named { background:var(--hi); }
small { color:var(--muted); }
</style></head><body><main>
<h1>Chorda: cada unísono, antes y después</h1>
<p>Por tecla: el batido más rápido que se oye en los parciales 1–3 (Hz; barra hasta 6 Hz), cuántos batidos completos se oyen en los primeros cinco segundos (el límite es menos de uno), y cuánto se separan las cuerdas solas (Hz). Antes: beta.16 (<code>DSP_VERSION</code> 10). Después: cada cuerda afinada. Resaltadas, las seis teclas que nombraste.<br>
<small>Per key: the fastest audible beat on partials 1–3 (Hz), whole beats heard in the first five seconds (the bound is under one), and how far the strings alone stand apart (Hz). Before: beta.16; after: every string tuned.</small></p>
<div class="wrap"><table>
<thead><tr><th>Tecla</th><th>Cuerdas</th><th colspan="2">Batido antes (Hz)</th><th colspan="2">Batido después (Hz)</th><th>Batidos en 5 s antes</th><th>después</th><th>Cuerdas solas antes (Hz)</th><th>después</th></tr></thead>
<tbody>
__ROWS__
</tbody></table></div>
</main></body></html>
"""

if __name__ == "__main__":
    main()
