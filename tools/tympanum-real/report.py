#!/usr/bin/env python3
"""The owner's analysis report (round 42, tympanum-real): `informe.html` in Spanish with the
comparison's charts, beside the previews.

    python -I tools/tympanum-real/report.py <compare out dir> <listen dir> <narrative.md>

`narrative.md` is the report's text (a small Markdown: `#`/`##` headings, paragraphs, `- `
lists, `|` tables, `**bold**`, `![caption](fig_kick.png)` for a chart of the compare folder).
The charts are copied next to the page (`informe/`); `table.md` (every measure, the
references' range, each Tympanum) is appended as the annex.
"""
import html
import re
import shutil
import sys
from pathlib import Path

CSS = """
:root{--bg:#fcfcfb;--ink:#0b0b0b;--muted:#52514e;--line:#e4e3df;--card:#fff;--acc:#2a78d6}
@media (prefers-color-scheme:dark){:root{--bg:#1a1a19;--ink:#f0efec;--muted:#c3c2b7;--line:#33322f;--card:#232321;--acc:#3987e5}
img{filter:invert(.92) hue-rotate(180deg)}}
body{margin:0;background:var(--bg);color:var(--ink);font:16px/1.55 system-ui,sans-serif}
main{max-width:960px;margin:0 auto;padding:24px 16px 64px}
h1{font-size:1.7rem}h2{font-size:1.25rem;margin-top:2em;border-top:1px solid var(--line);padding-top:.6em}
h3{font-size:1.05rem}p,li{max-width:72ch}figure{margin:1em 0}img{max-width:100%;height:auto;
border:1px solid var(--line);border-radius:6px;background:#fcfcfb}figcaption{color:var(--muted);font-size:.9rem}
.tw{overflow-x:auto}table{border-collapse:collapse;font-size:.85rem;margin:.6em 0}
td,th{border-bottom:1px solid var(--line);padding:4px 8px;text-align:left;vertical-align:top}
th{color:var(--muted);font-weight:600}code{font-size:.9em}
"""


def inline(t):
    t = html.escape(t)
    t = re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", t)
    t = re.sub(r"`(.+?)`", r"<code>\1</code>", t)
    t = re.sub(r"\[(.+?)\]\((.+?)\)", r"<a href='\2'>\1</a>", t)
    return t


def md(text):
    out, para, lst, tab = [], [], [], []

    def flush():
        if para:
            out.append("<p>" + inline(" ".join(para)) + "</p>")
            para.clear()
        if lst:
            out.append("<ul>" + "".join(f"<li>{inline(x)}</li>" for x in lst) + "</ul>")
            lst.clear()
        if tab:
            rows = [r for r in tab if not re.match(r"^\|[-\s|:]+\|$", r)]
            cells = [[c.strip() for c in r.strip("|").split("|")] for r in rows]
            h = "".join(f"<th>{inline(c)}</th>" for c in cells[0])
            b = "".join("<tr>" + "".join(f"<td>{inline(c)}</td>" for c in r) + "</tr>" for r in cells[1:])
            out.append(f"<div class='tw'><table><thead><tr>{h}</tr></thead><tbody>{b}</tbody></table></div>")
            tab.clear()
    for line in text.splitlines():
        s = line.rstrip()
        m = re.match(r"^!\[(.*?)\]\((.+?)\)$", s)
        if m:
            flush()
            out.append(f"<figure><img src='informe/{m.group(2)}' alt='{html.escape(m.group(1))}' loading='lazy'>"
                       f"<figcaption>{inline(m.group(1))}</figcaption></figure>")
        elif s.startswith("|"):
            if para or lst:
                flush()
            tab.append(s)
        elif s.startswith("### "):
            flush(); out.append(f"<h3>{inline(s[4:])}</h3>")
        elif s.startswith("## "):
            flush(); out.append(f"<h2>{inline(s[3:])}</h2>")
        elif s.startswith("# "):
            flush(); out.append(f"<h1>{inline(s[2:])}</h1>")
        elif s.startswith("- "):
            if para or tab:
                flush()
            lst.append(s[2:])
        elif not s:
            flush()
        else:
            if tab:
                flush()
            para.append(s)
    flush()
    return "\n".join(out)


def main():
    comp, listen, narr = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
    figs = listen / "informe"
    figs.mkdir(parents=True, exist_ok=True)
    for p in comp.glob("*.png"):
        shutil.copyfile(p, figs / p.name)
    body = md(narr.read_text(encoding="utf-8"))
    annex = (comp / "table.md").read_text(encoding="utf-8") if (comp / "table.md").exists() else ""
    annex = re.sub(r"^# .*$", "## Anexo: cada medida", annex, count=1, flags=re.M)
    page = ("<!doctype html><html lang='es'><head><meta charset='utf-8'><meta name='viewport' "
            "content='width=device-width,initial-scale=1'><title>Tympanum: informe</title><style>"
            + CSS + "</style></head><body><main>" + body + md(annex) + "</main></body></html>")
    (listen / "informe.html").write_text(page, encoding="utf-8")
    print(f"{listen / 'informe.html'}")


if __name__ == "__main__":
    main()
