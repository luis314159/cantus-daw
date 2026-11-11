"""Comparison sheets for the mixer-strip/inspector set and the browser set.

    python compare_sets.py mixer|browser <private-dir> <public-dir>

Private: vs-daws-<set>-{1x,4x}.png — other DAWs' equivalents, each scaled so its key or row
is as tall as ours, beside Cantus. Public: before-after-<set>-{1x,4x}.png — today's Cantus
(round 37's captures) beside this family.
"""
import os
import sys

from PIL import Image

import compare as C

SHOTS = C.SHOTS
FLM = "F:/projects/cantus/research/ui/fl-look-2026-11/screenshots/fl-mixer.png"
ROUND37 = os.path.join(C.ROOT, "docs", "round37", "finish")

SETS = {
    "mixer": {
        "title": "Mixer strip and inspector",
        "refs": [
            ("FL Studio 24 · strip: LED, pan, polarity and swap", FLM, (675, 555, 905, 700), 22),
            ("FL Studio 24 · strip: arm, routing", FLM, (675, 800, 905, 935), 22),
            ("FL Studio 24 · inserts panel", FLM, (1617, 395, 1900, 560), 25),
            ("Ableton Live 12 · track number, solo, arm", SHOTS + "ableton-live/mixer-controls-annotated.png", (55, 310, 215, 425), 26),
            ("Ableton Live 12 · Utility: Ø L, Ø R", SHOTS + "ableton-live/control-bar.png", (2165, 1462, 2290, 1500), 30),
            ("Bitwig Studio 6 · arm, solo, mute", SHOTS + "bitwig-studio/crop-05-bws6-mixer-strip.png", (14, 228, 195, 272), 44),
            ("Logic Pro · format and channel mode", SHOTS + "logic-studio-one/logic-channel-strip-anatomy.png", (10, 218, 420, 255), 34),
            ("Logic Pro · record, monitor, mute, solo", SHOTS + "logic-studio-one/logic-channel-strip-anatomy.png", (10, 1200, 280, 1282), 34),
            ("Studio One · mute, solo, record, monitor", SHOTS + "logic-studio-one/s1-console-nav-column-wrench.png", (135, 278, 280, 362), 44),
        ],
        "rows": [
            (16, "phase,swap,mono,link,arm,enable,fx,send,vca,miniView,more", "16 px (Wide strip, polarity row and keys)"),
            (12, "phase,swap,mono,link,enable,send,out,into,ret,key,freeze,grip,miniView,more,add", "12 px (inline in rows and fields)"),
            (24, "phase,swap,mono,link,arm,enable,fx,send,vca,miniView,more", "24 px (16 @150 %)"),
            (18, "phase,swap,mono,link,arm,enable,send,out,into,ret,key,freeze,grip,miniView,more,add", "18 px (12 @150 %; Large strip at 100 %)"),
        ],
        "on": "phase,swap,mono,link,arm,enable,fx,send,vca",
        "today": [("strip-buttons-{t}.png", None)],
        "key": 16,
    },
    "browser": {
        "title": "Browser",
        "refs": [
            ("FL Studio 24 · browser head and search", C.FL, (0, 180, 455, 250), 30),
            ("FL Studio 24 · sound rows (loop, add, menu)", C.FL, (0, 425, 455, 545), 28),
            ("Ableton Live 12 · library places", SHOTS + "ableton-live/control-bar.png", (20, 320, 240, 980), 38),
            ("Bitwig Studio 6 · browser kinds, star", SHOTS + "bitwig-studio/ug-13-browser-panel.png", (20, 20, 440, 200), 44),
            ("Bitwig Studio 6 · folders and samples", SHOTS + "bitwig-studio/ug-13-browser-panel.png", (20, 440, 340, 820), 46),
            ("Logic Pro · list, notes, loops, browser", SHOTS + "logic-studio-one/logic-main-window-overview.png", (1170, 84, 1300, 118), 30),
        ],
        "rows": [
            (12, "folder,folderOpen,file,audio,midi,preset,pack,star,starFilled,search,history,clear,speaker,check,download", "12 px (browser rows)"),
            (14, "folder,folderOpen,file,audio,midi,pack,star,starFilled,search,history,download", "14 px"),
            (16, "audio,midi,preset,instrument,effect", "16 px (unit marks, kinds)"),
            (18, "folder,folderOpen,file,audio,midi,preset,pack,star,starFilled,search,history,clear,speaker,check,download", "18 px (12 @150 %)"),
        ],
        "on": "folder,file,audio,midi,preset,pack,starFilled,search,history,check,download",
        "today": [("icons-{t}-100.png", None)],
        "key": 16,
    },
    "rest": {
        "title": "The rest of glyph.rs",
        "refs": [
            ("FL Studio 24 · top bar right (undo, cut, record, help, save, chat, get)", C.FL, (1455, 2, 1780, 48), 40),
            ("FL Studio 24 · window controls", C.FL, (1835, 8, 1915, 42), 34),
            ("Ableton Live 12 · control bar left (link, follow, metronome, quantise)", SHOTS + "ableton-live/control-bar.png", (20, 50, 1000, 98), 34),
            ("Bitwig Studio 6 · toolbar (file, play, record, automation, add)", SHOTS + "bitwig-studio/crop-03-bws6-toolbar-left.png", (0, 56, 980, 122), 62),
            ("Logic Pro · library, smart controls, help, editors", SHOTS + "logic-studio-one/logic-main-window-overview.png", (8, 84, 240, 118), 30),
        ],
        "rows": [
            (16, "help,warning,agent,device,keyboard,score,mouseMenu,automate,filter,replace,warp,pluginStopped,controller,lanePencil,laneLine,laneCurve,session,stage", "16-px key"),
            (24, "help,warning,agent,device,keyboard,score,mouseMenu,automate,filter,replace,warp,pluginStopped,controller,lanePencil,laneLine,laneCurve,session,stage", "24-px key (16 @150 %)"),
            (12, "menu,prev,next,close,minimise,maximise,restore,resizeGrip,resize,typed,sendOpen", "12 px marks"),
            (18, "menu,prev,next,close,minimise,maximise,restore,resizeGrip,resize,typed,sendOpen", "18 px (12 @150 %)"),
        ],
        "on": "help,warning,agent,device,keyboard,score,automate,filter,replace,warp,pluginStopped,controller,lanePencil,laneLine,laneCurve,session,stage",
        "today": [("icons-{t}-150.png", None)],
        "key": 16,
    },
}


def strip(theme, size, ids, state="rest", bare=False, set_="mixer"):
    return _page("sheet-mixer.html", f"t={theme}&set={set_}&strip={size}&ids={ids}&state={state}&bare={1 if bare else 0}")


_CACHE = {}


def _page(page, query):
    key = (page, query)
    if key in _CACHE:
        return _CACHE[key]
    os.makedirs(C.TMP, exist_ok=True)
    out = os.path.join(C.TMP, f"s{len(_CACHE)}-{page.split('.')[0]}.png")
    url = C.render.url_for(page, query)
    w, h = C.render.size_of(url)
    C.render.edge(["--force-device-scale-factor=1", f"--window-size={w},{h}", f"--screenshot={out}", url])
    _CACHE[key] = Image.open(out).convert("RGB").crop((0, 0, w, h))
    return _CACHE[key]


def ours(name, spec, themes=("night", "paper")):
    rows = []
    for size, ids, what in spec["rows"]:
        for theme in themes:
            rows.append((f"Cantus · Slate {theme.capitalize()} · {what} · bare", strip(theme, size, ids, bare=True, set_=name)))
            rows.append((f"Cantus · Slate {theme.capitalize()} · {what} · in wells", strip(theme, size, ids, set_=name)))
        rows.append((f"Cantus · Slate Night · {what} · on", strip("night", size, spec["on"], "on", set_=name)))
    return rows


def main():
    name, private, public = sys.argv[1], sys.argv[2], sys.argv[3]
    spec = SETS[name]
    k = spec["key"]
    refs = [(f"{label} · scaled to a {k}-px key", C.ref(path, box, kh, k)) for label, path, box, kh in spec["refs"]]
    refs += [(f"{label} · scaled to a {int(k * 1.5)}-px key (150 %)", C.ref(path, box, kh, int(k * 1.5))) for label, path, box, kh in spec["refs"]]
    mine = ours(name, spec)
    today = []
    for theme in ("night", "paper"):
        for pattern, box in spec["today"]:
            p = os.path.join(ROUND37, pattern.format(t=theme))
            if os.path.exists(p):
                im = Image.open(p).convert("RGB")
                today.append((f"Cantus today (beta.13, round 37) · {theme.capitalize()}", im.crop(box) if box else im))
    for scale, tag in ((1, "1x"), (4, "4x")):
        C.lay(f"{spec['title']} — Cantus beside FL, Ableton, Bitwig, Logic, Studio One — {'1:1' if scale == 1 else '4x nearest'}",
              refs + mine, scale).save(os.path.join(private, f"vs-daws-{name}-{tag}.png"))
        C.lay(f"{spec['title']} — today's Cantus and this family — {'1:1' if scale == 1 else '4x nearest'}",
              today + mine, scale).save(os.path.join(public, f"before-after-{name}-{tag}.png"))
    print(name, "done")


if __name__ == "__main__":
    main()
