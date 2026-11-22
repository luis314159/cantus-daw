"""The owner's listening kit for a Chorda retune (round 41, chorda-tune): plain before/after
pairs and a blind ABX with a preference per pair, offline, standard library only.

    python -I tools/chorda-voicing/tune_abx.py <listen dir> [--title "..."]

`<listen dir>` holds what `chorda::tests_unison::tune_listen` renders: `<name>-before.wav` /
`<name>-after.wav` (as rendered) and `matched/<name>-before.wav` / `matched/<name>-after.wav`
(the same, both brought to one RMS so a level never tells them apart). Written:

- `<listen dir>/pares.html` — every pair, named, side by side (not blind);
- `<listen dir>/abx/index.html` and `abx/audio/<random>.wav` — the blind kit: 16 ABX trials
  over the pairs, then one blind preference per pair ("which sounds more in tune?"), the
  reveal and the score (one-sided binomial p) only at the end, the answers kept in the
  browser and offered as a JSON file;
- `<listen dir>/abx-key.json` — which random file is which (never inside the kit).

The page runs from file:// (one <audio> element per clip: no fetch). Zip `abx/` to send it.
"""

import html
import json
import os
import random
import secrets
import shutil
import sys


def pairs(root):
    names = sorted(
        f[: -len("-before.wav")]
        for f in os.listdir(os.path.join(root, "matched"))
        if f.endswith("-before.wav")
        and os.path.exists(os.path.join(root, "matched", f.replace("-before.wav", "-after.wav")))
    )
    if not names:
        sys.exit(f"no matched/<name>-before.wav / -after.wav pairs in {root}")
    return names


def plain_page(root, names, title):
    rows = []
    for n in names:
        rows.append(
            "<tr><th>{0}</th><td><audio controls preload=none src='{1}'></audio></td>"
            "<td><audio controls preload=none src='{2}'></audio></td></tr>".format(
                html.escape(n), html.escape(n + "-before.wav"), html.escape(n + "-after.wav")
            )
        )
    page = PLAIN.replace("__TITLE__", html.escape(title)).replace("__ROWS__", "\n".join(rows))
    with open(os.path.join(root, "pares.html"), "w", encoding="utf-8") as f:
        f.write(page)


def kit(root, names, title):
    out = os.path.join(root, "abx")
    audio = os.path.join(out, "audio")
    shutil.rmtree(audio, ignore_errors=True)
    os.makedirs(audio)
    items, key = [], {"pairs": {}}
    used = set()
    for n in names:
        toks = {}
        for which in ("before", "after"):
            while True:
                tok = secrets.token_hex(8)
                if tok not in used:
                    used.add(tok)
                    break
            shutil.copyfile(
                os.path.join(root, "matched", f"{n}-{which}.wav"),
                os.path.join(audio, tok + ".wav"),
            )
            toks[which] = tok
        # Which token is "p" and which "q" is drawn too: the page never says before/after.
        order = ["before", "after"]
        random.SystemRandom().shuffle(order)
        items.append({"id": n, "p": "audio/" + toks[order[0]] + ".wav",
                      "q": "audio/" + toks[order[1]] + ".wav"})
        key["pairs"][n] = {"p": order[0], "q": order[1], "files": toks}
    seed = secrets.randbelow(2**31 - 1) + 1
    reveal = {n: key["pairs"][n]["p"] for n in names}
    veiled = veil(json.dumps(reveal), seed)
    page = (
        KIT.replace("__TITLE__", html.escape(title))
        .replace("__ITEMS__", json.dumps(items))
        .replace("__SEED__", str(seed))
        .replace("__REVEAL__", json.dumps(veiled))
        .replace("__KITID__", json.dumps("tune-" + secrets.token_hex(5)))
    )
    with open(os.path.join(out, "index.html"), "w", encoding="utf-8") as f:
        f.write(page)
    with open(os.path.join(root, "abx-key.json"), "w", encoding="utf-8") as f:
        json.dump(key, f, indent=1)


def veil(text, seed):
    """XOR with a small LCG's bytes, hex: keeps the answer out of plain view (not secret)."""
    x, out = seed, []
    for b in text.encode("utf-8"):
        x = (1103515245 * x + 12345) & 0x7FFFFFFF
        out.append("%02x" % (b ^ (x >> 16 & 0xFF)))
    return "".join(out)


PLAIN = None
KIT = None


def load_templates():
    global PLAIN, KIT
    here = os.path.dirname(os.path.abspath(__file__))
    with open(os.path.join(here, "tune_abx_plain.html"), encoding="utf-8") as f:
        PLAIN = f.read()
    with open(os.path.join(here, "tune_abx_kit.html"), encoding="utf-8") as f:
        KIT = f.read()


def main():
    args = sys.argv[1:]
    if not args:
        sys.exit(__doc__)
    root = args[0]
    title = "Chorda: afinación de cada cuerda (ronda 41)"
    if "--title" in args:
        title = args[args.index("--title") + 1]
    load_templates()
    names = pairs(root)
    plain_page(root, names, title)
    kit(root, names, title)
    print(f"{len(names)} pairs: {os.path.join(root, 'pares.html')}, "
          f"{os.path.join(root, 'abx', 'index.html')}, key {os.path.join(root, 'abx-key.json')}")


if __name__ == "__main__":
    main()
