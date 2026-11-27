"""r42:arcus-hands, pass 2 — sheet v14: ``hand_sheet_v12.py``'s sheet from the realistic
violinists' camera of polish2's d630eb474 (8 degrees under the room's): both figures' left
hand in first position (``violin-play`` with ``--note=2,71``, B4 with the first finger),
third (``violin-play``) and high (``violin-high``), and the bow hand, at 100 % (its own
pixels and 2x nearest) and 200 %, Night and Paper, Photographic and Drawn, before and after;
with ``--refs=<dir>`` the photographs beside (keep those sheets outside the repository).

    python tools/arcus-figures/hand_sheet_v14.py <before shots> <after shots> <out dir> [--refs=<dir>]

Each shot folder holds ``shoot_arcus <dir> --hold=woman,man --scales=1,2 --only=<state>``
output (``violin-play`` and ``violin-high``) and the first position's, taken with
``--note=2,71 --only=violin-play`` and renamed ``arcus-violin-first-…``.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import hand_sheet_v12 as v12  # noqa: E402

# (case, state, who, box as fractions of the shot) — the camera of d630eb474.
v12.CROPS = [
    ("left hand, first position", "violin-first", "woman", (0.36, 0.27, 0.49, 0.46)),
    ("left hand, third position", "violin-play", "woman", (0.36, 0.27, 0.49, 0.46)),
    ("left hand, high", "violin-high", "woman", (0.32, 0.27, 0.45, 0.46)),
    ("bow hand", "violin-play", "woman", (0.17, 0.40, 0.31, 0.58)),
    ("left hand, first position", "violin-first", "man", (0.36, 0.27, 0.49, 0.46)),
    ("left hand, third position", "violin-play", "man", (0.36, 0.27, 0.49, 0.46)),
    ("left hand, high", "violin-high", "man", (0.32, 0.27, 0.45, 0.46)),
    ("bow hand", "violin-play", "man", (0.17, 0.40, 0.31, 0.58)),
]
v12.REFS["left hand, third position"] = v12.REFS["left hand, first position"]
v12.REFS["left hand, high"] = [("crop-02.jpg", (0.0, 0.0, 1.0, 1.0)),
                               ("crop-11.jpg", (0.0, 0.0, 1.0, 1.0))]

if __name__ == "__main__":
    v12.main()
