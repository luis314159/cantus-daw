"""Every interface picture outside the units, from the shoot examples' PNGs, to where the site
and the README read them (round 39, `shots`). `tools/shots/reshoot.sh` runs the examples and
then this; by hand:

    python tools/shots/place.py <shots-dir> [family…]     # needs Pillow; oxipng if installed

`<shots-dir>` holds one folder per example (`site/`, `readme/`, `showcase/`, …), as
`reshoot.sh` writes them. Families: program, readme, studio, session, score, ai, manual. With none
named, every family.

The table below is the whole recipe: which PNG, which crop, which file it becomes and how it is
encoded. A crop is either the name of a rect the example measured on the layout and wrote
beside its PNG (`<png>.json`, `shots_common/studio.rs`'s `sidecar`: the Settings window, a
dialog, the sync group and its neighbours, the external gear page's two bands), so it follows
the layout when a panel changes shape; or fixed pixels (x, y, w, h). The encodings are the ones the committed files
were made with (checked byte for byte against the round 36–38 sources):
- the program picture: lossless WebP, then 960 and 1280 px wide at q82 (`web/src/picture.rs`);
- studio/ and ai/: q82, except external/surfaces at q84;
- session/ and score/: q88, from the README's PNG;
- the README's PNGs: lossless, optimised with oxipng when it is installed.
When a picture's size changes, the size the site declares for it in `web/src/*.rs` (an
`<img src=… width=… height=…>`, or studio.rs's `(w, h)`) is rewritten to match; picture.rs's
`SIZE` (the program picture, which the tour crops) is only checked.
"""
import json
import os
import re
import shutil
import subprocess
import sys

from PIL import Image

ROOT = os.path.normpath(os.path.join(os.path.dirname(__file__), "..", ".."))
WEB = os.path.join(ROOT, "web", "assets")
DOC = os.path.join(ROOT, "docs", "images")

# (family, source png relative to the shots dir, crop or None, resize (w, h) or None,
#  destination relative to the repository, encoding)
# Encodings: "png" (lossless PNG), "lossless" (lossless WebP), ("webp", q).
README = [
    ("readme/window-night.png", None, None, "docs/images/window-night.png"),
    ("readme/window-night-720.png", None, None, "docs/images/window-night-720.png"),
    ("readme/window-paper-720.png", None, None, "docs/images/window-paper-720.png"),
    ("readme/mixer.png", None, None, "docs/images/mixer.png"),
    ("readme/inspector.png", None, None, "docs/images/inspector.png"),
    ("readme/matrix.png", None, None, "docs/images/matrix.png"),
    ("readme/linked.png", None, None, "docs/images/linked.png"),
    ("readme/pianoroll.png", None, None, "docs/images/pianoroll.png"),
    ("readme/rack.png", None, None, "docs/images/rack.png"),
    ("readme/appearance.png", None, None, "docs/images/appearance.png"),
    ("readme/themes.png", None, None, "docs/images/themes.png"),
    ("readme/mark.png", None, None, "docs/images/mark.png"),
    ("readme/packs.png", None, None, "docs/images/packs.png"),
    ("showcase/record-on-night-en.png", None, None, "docs/images/record-mode.png"),
    ("showcase/record-ask-night-en.png", None, None, "docs/images/record-ask.png"),
    ("showcase/mix-snap-night-en.png", None, None, "docs/images/mix-vca-snapshots.png"),
    ("editor/editor-both-night-en-k100.png", None, None, "docs/images/audio-editor.png"),
    ("external/external-night-en-150.png", None, None, "docs/images/external-gear.png"),
    ("ara/ara-menu-bound-slate.png", None, None, "docs/images/ara-menu.png"),
    ("ara/ara-clips-slate.png", None, None, "docs/images/ara-clips.png"),
]
SESSION_DOC = [
    ("session/session-grid-slate-100.png", None, None, "docs/images/session-night.png"),
    ("session/session-grid-paper-100.png", None, None, "docs/images/session-paper.png"),
    ("session/session-ring-slate-100.png", None, None, "docs/images/session-ring.png"),
    ("session/settings-midi-slate-100.png", "settings", None, "docs/images/midi-ports.png"),
    ("sync/sync-bar-link-night-en-100.png", "sync-bar", None, "docs/images/sync-bar.png"),
    ("mpe/mpe-roll-night-en-100.png", None, None, "docs/images/mpe-lanes.png"),
]
SCORE_DOC = [
    ("readme/score-roll.png", None, None, "docs/images/score-roll.png"),
    ("readme/score-pane.png", None, None, "docs/images/score-pane.png"),
    ("readme/score-page.png", None, None, "docs/images/score-page.png"),
]
AI_DOC = [
    ("ai/ai-activity-night-en.png", None, None, "docs/images/ai-agent.png"),
    ("ai/ai-settings-paper-en.png", "settings", None, "docs/images/ai-settings.png"),
    ("mastering/mastering-proposal-slate-en-100.png", "dialog", None,
     "docs/images/mastering-assistant.png"),
    ("stems/stems-arrangement-night-en-100.png", None, None, "docs/images/stems-tracks.png"),
    ("stems/stems-panel-before-paper-en-100.png", "dialog", None,
     "docs/images/stems-panel.png"),
    ("transcribe/transcribe-poly-night-en-k100.png", "dialog", None,
     "docs/images/audio-to-midi.png"),
]


def studio():
    rows = []
    for theme in ("night", "paper"):
        for lang in ("en", "es"):
            for kind in ("record-on", "record-ask", "mix-snap"):
                rows.append((f"showcase/{kind}-{theme}-{lang}.png", None, None,
                             f"web/assets/studio/{kind}-{theme}-{lang}.webp", ("webp", 82)))
            sfx = "" if lang == "en" else "-es"
            src = f"external/external-{theme}-{lang}-150.png"
            rows.append((src, "external", None,
                         f"web/assets/studio/external-{theme}{sfx}.webp", ("webp", 84)))
            rows.append((src, "surfaces", None,
                         f"web/assets/studio/surfaces-{theme}{sfx}.webp", ("webp", 84)))
    rows += [
        ("editor/editor-both-night-en-k200.png", None, (1800, 1080),
         "web/assets/studio/editor-night.webp", ("webp", 82)),
        ("editor/editor-both-paper-en-k100.png", None, None,
         "web/assets/studio/editor-paper.webp", ("webp", 82)),
        ("editor/editor-region-night-es-k100.png", None, None,
         "web/assets/studio/editor-night-es.webp", ("webp", 82)),
        ("editor/editor-region-paper-es-k100.png", None, None,
         "web/assets/studio/editor-paper-es.webp", ("webp", 82)),
        ("ara/ara-clips-slate.png", None, None, "web/assets/studio/ara-clips.webp", ("webp", 82)),
        ("ara/ara-menu-bound-slate.png", None, None, "web/assets/studio/ara-menu.webp",
         ("webp", 82)),
    ]
    return rows


def manual():
    """The manual's pictures (web/assets/manual/img, and docs/manual/img for the installed
    manual): scaled with Lanczos and palettised to 256 colours ("pal"), as round 22 made them
    (the manual holds them under 4 MB, xtask's manual_tests); the round 31/34 ones as shot."""
    rows = [
        ("manual/startup-dark.png", None, (1280, 720), "startup.png", "pal"),
        ("manual/startup-dark.png", (0, 520, 1920, 560), (1280, 373), "mixer.png", "pal"),
        ("manual/master-dark.png", None, (1200, 752), "master.png", "pal"),
        ("manual/rec-window-slate.png", (0, 0, 1920, 656), (1200, 410), "record-options.png", "pal"),
        ("manual/picker-instruments-dark.png", None, (1280, 720), "picker.png", "pal"),
        ("manual/audio-slate-first-run.png", None, (1100, 653), "first-run.png", "pal"),
        ("manual/audio-slate-popover.png", "popover", None, "audio-popover.png", "pal"),
        ("manual/samples-browser-night.png", None, None, "browser.png", "pal"),
        ("manual/pianoroll-night-welcome-1280x720.png", None, (880, 495), "piano-roll.png", "pal"),
        ("manual/notefx-chain-night-en-100.png", None, None, "notefx-chain.png", "png"),
        ("manual/notefx-roll-night-en-100.png", None, None, "notefx-roll.png", "png"),
        ("manual/notefx-clip-night-en-100.png", None, None, "notefx-groove.png", "png"),
        ("transcribe/transcribe-poly-night-en-k100.png", None, None, "transcribe-dialog.png", "png"),
        ("transcribe/transcribe-poly-night-es-k100.png", None, None, "transcribe-dialog-es.png", "png"),
    ]
    out = []
    for src, crop, size, name, enc in rows:
        for d in ("web/assets/manual/img", "docs/manual/img"):
            out.append((src, crop, size, f"{d}/{name}", enc))
    return out


def png(rows):
    return [(s, c, r, d, "png") for (s, c, r, d) in rows]


def from_doc(pairs, q):
    """Site pictures made from the README's PNG (written first, so the two agree)."""
    return [(d, None, size, w, ("webp", q), True) for (d, size, w) in pairs]


FAMILIES = {
    "program": [
        ("site/app-mixer-xl.png", None, None, "web/assets/program/cantus.webp", "lossless"),
        ("site/app-mixer-xl-es.png", None, None, "web/assets/program/cantus-es.webp", "lossless"),
        ("site/app-mixer-xl.png", None, (960, 513), "web/assets/program/cantus-960.webp", ("webp", 82)),
        ("site/app-mixer-xl.png", None, (1280, 684), "web/assets/program/cantus-1280.webp", ("webp", 82)),
        ("site/app-mixer-xl-es.png", None, (960, 513), "web/assets/program/cantus-es-960.webp", ("webp", 82)),
        ("site/app-mixer-xl-es.png", None, (1280, 684), "web/assets/program/cantus-es-1280.webp", ("webp", 82)),
    ],
    "readme": png(README),
    "studio": studio(),
    "session": png(SESSION_DOC) + from_doc([
        ("docs/images/session-night.png", None, "web/assets/session/grid.webp"),
        ("docs/images/midi-ports.png", None, "web/assets/session/midi-ports.webp"),
        ("docs/images/mpe-lanes.png", None, "web/assets/session/mpe-lanes.webp"),
        ("docs/images/sync-bar.png", None, "web/assets/session/sync-bar.webp"),
    ], 88),
    "score": png(SCORE_DOC) + from_doc([
        ("docs/images/score-roll.png", None, "web/assets/score/roll.webp"),
        ("docs/images/score-pane.png", None, "web/assets/score/pane.webp"),
        ("docs/images/score-page.png", None, "web/assets/score/page.webp"),
    ], 88),
    "manual": manual(),
    "ai": png(AI_DOC) + from_doc([
        ("docs/images/ai-agent.png", (1400, 875), "web/assets/ai/agent.webp"),
        ("docs/images/stems-tracks.png", None, "web/assets/ai/stems.webp"),
        ("docs/images/mastering-assistant.png", None, "web/assets/ai/mastering.webp"),
        ("docs/images/ai-settings.png", None, "web/assets/ai/settings.webp"),
        ("docs/images/audio-to-midi.png", None, "web/assets/ai/midi.webp"),
    ], 82),
}


def place(shots, row):
    src, crop, size, dst, enc = row[:5]
    from_repo = len(row) > 5 and row[5]
    path = os.path.join(ROOT, src) if from_repo else os.path.join(shots, src)
    if not os.path.exists(path):
        print(f"MISSING {path} (for {dst})", flush=True)
        return None
    im = Image.open(path)
    if isinstance(crop, str):
        # A rect the example measured on the layout and wrote beside its PNG.
        side = path + ".json"
        rects = json.load(open(side, encoding="utf-8")) if os.path.exists(side) else {}
        if crop not in rects:
            print(f"NO RECT {crop} in {side} (for {dst})", flush=True)
            return None
        crop = rects[crop]
    if crop:
        x, y, w, h = crop
        if x + w > im.width or y + h > im.height:
            print(f"CROP OUTSIDE {src}: {im.size} vs {crop} (for {dst})", flush=True)
            return None
        im = im.crop((x, y, x + w, y + h))
    if size:
        im = im.convert("RGB").resize(size, Image.LANCZOS)
    out = os.path.join(ROOT, dst)
    os.makedirs(os.path.dirname(out), exist_ok=True)
    if enc == "png":
        im.save(out, optimize=True)
    elif enc == "pal":
        im.convert("RGB").quantize(256, method=Image.Quantize.MEDIANCUT).save(out, optimize=True)
    elif enc == "lossless":
        im.convert("RGB").save(out, "WEBP", lossless=True, quality=100, method=6)
    else:
        im.convert("RGB").save(out, "WEBP", quality=enc[1], method=6)
    print(f"{dst} {im.width}x{im.height}", flush=True)
    return out


def sync_sizes(written):
    """The sizes the site declares, rewritten to the pictures just made."""
    src = os.path.join(ROOT, "web", "src")
    files = [os.path.join(d, f) for d, _, fs in os.walk(src) for f in fs if f.endswith(".rs")]
    texts = {f: open(f, encoding="utf-8").read() for f in files}
    new = dict(texts)
    size = {}
    for out in written:
        rel = os.path.relpath(out, os.path.join(ROOT, "web", "assets")).replace(os.sep, "/")
        if rel.startswith(".."):
            continue
        size["/" + rel] = Image.open(out).size
    # `<img src="/session/grid.webp" width="1400" height="720"`.
    for f, t in new.items():
        def img(m):
            wh = size.get(m[1])
            return m[0] if wh is None else f'src="{m[1]}" width="{wh[0]}" height="{wh[1]}"'
        new[f] = re.sub(r'src="(/[^"]+\.webp)" width="\d+" height="\d+"', img, t)
    # studio.rs's cards: `format!("/studio/external-{theme}{es}.webp"), …, (1500, 412)) }`,
    # one size for the four (Night and Paper, English and Spanish), Night in English's.
    for prefix in ("external", "surfaces", "ara-clips"):
        variants = [k for k in size if k.startswith(f"/studio/{prefix}")]
        if not variants:
            continue
        ref = size.get(f"/studio/{prefix}-night.webp") or size[variants[0]]
        for k in variants:
            if size[k] != ref:
                print(f"SIZE {k} is {size[k]}, the card declares one size, {ref}", flush=True)
        for f, t in new.items():
            new[f] = re.sub(
                r"(/studio/" + re.escape(prefix) + r"[^\n]*?\()\d+, \d+(\)\) \})",
                lambda m: f"{m[1]}{ref[0]}, {ref[1]}{m[2]}",
                t,
            )
    for f in files:
        if new[f] != texts[f]:
            open(f, "w", encoding="utf-8", newline="").write(new[f])
            print(f"{os.path.relpath(f, ROOT)}: sizes written", flush=True)
    # The program picture: the tour crops it in these units, so its size is only checked.
    prog = size.get("/program/cantus.webp")
    pic = texts.get(os.path.join(src, "picture.rs"), "")
    m = re.search(r"pub const SIZE: \(u32, u32\) = \((\d+), (\d+)\)", pic)
    if prog and m and prog != (int(m[1]), int(m[2])):
        print(f"SIZE program/cantus.webp is {prog}, picture.rs says ({m[1]}, {m[2]})", flush=True)


def main(shots, families):
    pngs = []
    bad = 0
    written = []
    for fam in families:
        for row in FAMILIES[fam]:
            out = place(shots, row)
            if out is None:
                bad += 1
                continue
            written.append(out)
            if out.endswith(".png"):
                pngs.append(out)
    sync_sizes(written)
    if pngs and shutil.which("oxipng"):
        subprocess.run(["oxipng", "-o", "4", "--strip", "safe", "-q", *pngs], check=False)
    if bad:
        print(f"{bad} picture(s) not made", flush=True)
        sys.exit(1)


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    names = sys.argv[2:] or list(FAMILIES)
    for n in names:
        if n not in FAMILIES:
            sys.exit(f"no family {n}: {', '.join(FAMILIES)}")
    # The README's PNGs first: session/, score/ and ai/ are made from them.
    order = [f for f in ("program", "readme", "studio", "session", "score", "ai", "manual")
             if f in names]
    main(sys.argv[1], order)
