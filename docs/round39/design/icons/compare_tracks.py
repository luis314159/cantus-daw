"""Set 6 comparison: track icons in other DAWs beside Cantus's, at the same size.

    python compare_tracks.py <private-dir>
"""
import os
import sys

import compare as C
import compare_sets as CS

SHOTS = C.SHOTS
FLM = "F:/projects/cantus/research/ui/fl-look-2026-11/screenshots/fl-mixer.png"
REFS = [
    ("FL Studio 24 · mixer strip icons (the user's)", FLM, (675, 410, 1600, 440), 30),
    ("FL Studio 24 · playlist track icons", C.FL, (1375, 255, 1425, 680), 70),
    ("Logic Pro · track icons", SHOTS + "logic-studio-one/logic-main-window-overview.png", (45, 197, 92, 860), 44),
]
IDS = "kick,snare,hihat,cymbal,tom,clap,shaker,drumkit,bass,sub,piano,keys,eguitar,aguitar,strings,brass,woodwind,synth,pad,lead,pluck,arp,mic,voice,choir,riser,impact,noise,loop,oneshot,midi,video,bus,reverb,delay,master,star,heart,flag,circle,square,triangle,diamond"


def main():
    private = sys.argv[1]
    rows = []
    for k, what in ((14, "14 px (header, strip head)"), (24, "24 px (picker; 16 @150 %)")):
        for label, path, box, kh in REFS:
            rows.append((f"{label} · scaled to {k} px", C.ref(path, box, kh, k + 6)))
        for theme in ("night", "paper"):
            rows.append((f"Cantus track icons · Slate {theme.capitalize()} · {what}", CS._page("sheet-tracks.html", f"t={theme}&strip={k}")))
    for scale, tag in ((1, "1x"), (4, "4x")):
        C.lay(f"Track icons — Cantus beside FL Studio and Logic Pro — {'1:1' if scale == 1 else '4x nearest'}", rows, scale).save(
            os.path.join(private, f"vs-daws-tracks-{tag}.png"))
    print("tracks done")


if __name__ == "__main__":
    main()
