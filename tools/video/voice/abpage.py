"""The A/B page: the two best takes of every line that passed the gate, for the owner's ear.

    python -I tools/video/voice/abpage.py --takes F:/projects/video-work/voice/en \
        [--takes F:/projects/video-work/voice/es] --out F:/projects/video-work/voice/ab.html

Among the takes that pass `gate.py`, the two whose pace is nearest the middle of the
line's window are offered, as A and B, in an order fixed by the take's name (never by a
score, so the page does not lead the ear). A line with fewer than two passing takes says
so plainly. The owner picks one per line (or "neither"); the page shows his choices as one
line of text to copy back, e.g. `en 1:01-3 2:02-1 …`. The audio is referenced from the
page's folder, so the page works opened from disk.
"""

import argparse
import html
import json
import os
from pathlib import Path

MIDDLE = {"en": 2.5, "es": 5.0}


def pace_note(r: dict, lang: str) -> str:
    if r.get("pace_ok", True):
        return ""
    fast = r["rate"] > MIDDLE[lang]
    return " · <em>faster than a calm read</em>" if fast else " · <em>slower than a calm read</em>"


def best_two(rows: list, lang: str) -> list:
    ok = [r for r in rows if r["pass"]]
    ok.sort(key=lambda r: abs(r["rate"] - MIDDLE[lang]))
    return sorted(ok[:2], key=lambda r: r["take"])


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--takes", action="append", required=True)
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    out = Path(a.out)
    parts = []
    for folder in map(Path, a.takes):
        rows = json.loads((folder / "gate.json").read_text(encoding="utf-8"))
        lang = json.loads(next(folder.glob("*-*.json")).read_text(encoding="utf-8"))["lang"]
        title = "English" if lang == "en" else "Español"
        parts.append(f'<h2>{title}</h2>')
        for n in sorted({r["line"] for r in rows}):
            line_rows = [r for r in rows if r["line"] == n]
            meta = json.loads((folder / line_rows[0]["take"].replace(".wav", ".json")).read_text(encoding="utf-8"))
            two = best_two(line_rows, lang)
            passed = sum(r["pass"] for r in line_rows)
            parts.append(f'<section><h3>{n}. {html.escape(meta["text"])}</h3>')
            parts.append(f'<p class="note">{passed} of {len(line_rows)} takes passed the gate.</p>')
            if len(two) < 2:
                parts.append('<p class="warn">Fewer than two takes passed: more are being made.</p>')
            for label, r in zip("AB", two):
                src = os.path.relpath(folder / r["take"], out.parent).replace(os.sep, "/")
                name = f"{lang}-{n}"
                value = r["take"].removesuffix(".wav")
                parts.append(
                    f'<label class="take"><input type="radio" name="{name}" value="{value}">'
                    f'<b>{label}</b><audio controls preload="none" src="{html.escape(src)}"></audio>'
                    f'<span>{r["speech_s"]} s · {r["rate"]} {r["unit"]}{pace_note(r, lang)}</span></label>'
                )
            parts.append(
                f'<label class="take"><input type="radio" name="{lang}-{n}" value="neither">'
                "<b>Neither</b><span>make more</span></label></section>"
            )
    page = TEMPLATE.replace("{{BODY}}", "\n".join(parts))
    out.write_text(page, encoding="utf-8")
    print(out)


TEMPLATE = """<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Narration takes</title>
<style>
:root { --bg: #11151c; --panel: #19202b; --ink: #e6e9ef; --muted: #9aa4b2; --line: #2a3442;
  --accent: #4f7fd9; --gold: #c9a45c; }
body { background: var(--bg); color: var(--ink); font: 16px/1.5 system-ui, sans-serif;
  margin: 0 auto; max-width: 860px; padding: 24px 16px 96px; }
h1 { font-size: 24px; margin: 0 0 4px; } h2 { margin: 32px 0 8px; color: var(--gold);
  font-size: 18px; letter-spacing: .04em; text-transform: uppercase; }
h3 { font-size: 16px; font-weight: 600; margin: 0 0 4px; }
section { background: var(--panel); border: 1px solid var(--line); border-radius: 8px;
  padding: 12px 14px; margin: 10px 0; }
.note { color: var(--muted); margin: 0 0 8px; font-size: 14px; }
.warn { color: var(--gold); margin: 0 0 8px; }
.take { display: flex; align-items: center; gap: 10px; padding: 4px 0; flex-wrap: wrap; }
.take b { width: 64px; } .take span { color: var(--muted); font-size: 14px; }
.take em { color: var(--gold); font-style: normal; }
audio { height: 32px; max-width: 100%; }
#picks { position: fixed; left: 0; right: 0; bottom: 0; background: var(--panel);
  border-top: 1px solid var(--line); padding: 10px 16px; font: 14px ui-monospace, monospace;
  overflow-wrap: anywhere; }
</style></head><body>
<h1>Narration takes</h1>
<p class="note">Two takes per line that passed the gate: every word heard back as written, no
clipping, no word cut off, no babble after the line, inside the line's time. Beside each: its
length and its pace (words a second; in Spanish, syllables, estimated). A calm read is about
2.5 words or 5 syllables a second; the stock voice reads long lines faster, and only your ear
can say whether that is too fast. The order is by take name, not by any score. Pick one per line, or Neither. The line at the
bottom holds your picks; copy it back.</p>
{{BODY}}
<div id="picks">Picks: none yet</div>
<script>
function update() {
  const picks = [...document.querySelectorAll('input[type=radio]:checked')]
    .map(i => i.name + ':' + i.value);
  document.getElementById('picks').textContent =
    'Picks: ' + (picks.length ? picks.join(' ') : 'none yet');
}
document.addEventListener('change', update);
</script></body></html>
"""

if __name__ == "__main__":
    main()
