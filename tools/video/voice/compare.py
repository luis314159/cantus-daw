"""The comparison page: each line's Chatterbox take the owner preferred (the A of `ab.html`)
beside the best gated take of every new voice, so he hears the difference directly.

    python -I tools/video/voice/compare.py --pair en $WORK/voice/pool/en $WORK/voice/qwen-pool/en \
        --pair es $WORK/voice/pool/es $WORK/voice/qwen-pool/es --out $WORK/voice/ab-qwen.html

For each line and each voice of the new folder (the `voice` its takes' `.json` name), the
passing take nearest a calm pace is shown; a voice with no passing take for a line says so.
The owner picks one per line; the page collects the picks as one line of text to copy back.
"""

import argparse
import html
import json
import os
import sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from abpage import MIDDLE, TEMPLATE, best_two, pace_note  # noqa: E402


def rows_of(folder: Path) -> list:
    rows = json.loads((folder / "gate.json").read_text(encoding="utf-8"))
    for r in rows:
        meta = json.loads((folder / r["take"].replace(".wav", ".json")).read_text(encoding="utf-8"))
        r["voice"] = meta.get("voice", "Chatterbox stock")
        r["text"] = meta["text"]
        r["path"] = folder / r["take"]
    return rows


def take_html(name: str, value: str, label: str, r: dict, lang: str, out: Path) -> str:
    src = os.path.relpath(r["path"], out.parent).replace(os.sep, "/")
    return (
        f'<label class="take"><input type="radio" name="{name}" value="{html.escape(value)}">'
        f"<b>{html.escape(label)}</b>"
        f'<audio controls preload="none" src="{html.escape(src)}"></audio>'
        f'<span>{r["speech_s"]} s · {r["rate"]} {r["unit"]}{pace_note(r, lang)}</span></label>'
    )


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--pair", nargs=3, action="append", metavar=("LANG", "OLD", "NEW"), required=True)
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    out = Path(a.out)
    parts = []
    for lang, old_dir, new_dir in a.pair:
        old, new = rows_of(Path(old_dir)), rows_of(Path(new_dir))
        parts.append(f"<h2>{'English' if lang == 'en' else 'Español'}</h2>")
        for n in sorted({r["line"] for r in old} | {r["line"] for r in new}):
            o = [r for r in old if r["line"] == n]
            text = (o or [r for r in new if r["line"] == n])[0]["text"]
            parts.append(f"<section><h3>{n}. {html.escape(text)}</h3>")
            name = f"{lang}-{n}"
            a_take = best_two(o, lang)
            if a_take:
                parts.append(take_html(name, "chatterbox:" + a_take[0]["take"][:-4], "Chatterbox A", a_take[0], lang, out))
            by_voice = defaultdict(list)
            for r in new:
                if r["line"] == n:
                    by_voice[r["voice"]].append(r)
            for voice in sorted(by_voice):
                ok = sorted((r for r in by_voice[voice] if r["pass"]), key=lambda r: abs(r["rate"] - MIDDLE[lang]))
                label = "Qwen · " + voice.replace("built-in ", "").replace("designed ", "").split(" (")[0]
                if ok:
                    parts.append(take_html(name, f"qwen:{ok[0]['take'][:-4]}", label, ok[0], lang, out))
                else:
                    parts.append(f'<p class="warn">{html.escape(label)}: no take of {len(by_voice[voice])} passed the gate.</p>')
            parts.append(f'<label class="take"><input type="radio" name="{name}" value="neither"><b>Neither</b></label></section>')
    page = TEMPLATE.replace("{{BODY}}", "\n".join(parts))
    page = page.replace("<title>Narration takes</title>", "<title>Qwen vs Chatterbox</title>")
    page = page.replace("<h1>Narration takes</h1>", "<h1>Qwen3-TTS beside Chatterbox</h1>")
    page = page.replace(
        page[page.index('<p class="note">Two takes'):page.index("</p>", page.index('<p class="note">Two takes')) + 4],
        '<p class="note">Each line: the Chatterbox take you preferred (A), then the best gated take of each '
        "Qwen3-TTS voice: every word heard back as written, no clipping, no word cut off, no babble, inside "
        "the line's time. Beside each, its length and pace (a calm read is about 2.5 words or 5 syllables a "
        "second). The Qwen voices are built-in or designed from a description: nobody's voice. Pick one per "
        "line, or Neither; the line at the bottom holds your picks.</p>",
    )
    out.write_text(page, encoding="utf-8")
    print(out)


if __name__ == "__main__":
    main()
