"""The tests' imports: the lab's package and the synthetic signals, under `python -I`."""
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
LAB = HERE.parent
for p in (str(LAB), str(HERE)):
    if p not in sys.path:
        sys.path.insert(0, p)

lab = LAB
REPO = LAB.parents[1]

import os  # noqa: E402
# The references and the study's records: SOUNDLAB_REFS / SOUNDLAB_STUDY as before, or the
# shared names (tools/sound-refs: CANTUS_REFS, CANTUS_STUDY_REFDB) when those are set.
if not os.environ.get("SOUNDLAB_REFS") and os.environ.get("CANTUS_REFS"):
    os.environ["SOUNDLAB_REFS"] = os.environ["CANTUS_REFS"]


def study_refdb():
    """The study's refdb folder (notes/, catalog.json): SOUNDLAB_STUDY/refdb, else
    CANTUS_STUDY_REFDB; None when neither is set."""
    if os.environ.get("SOUNDLAB_STUDY"):
        return Path(os.environ["SOUNDLAB_STUDY"]) / "refdb"
    r = os.environ.get("CANTUS_STUDY_REFDB")
    return Path(r) if r else None


def local(path):
    """A path the study recorded (absolute, under a round's `downloads` or another
    `cantus-refs`) as it is under SOUNDLAB_REFS here."""
    p = Path(path)
    if p.exists() or not os.environ.get("SOUNDLAB_REFS"):
        return p
    for i in range(len(p.parts) - 1, -1, -1):
        if p.parts[i].lower() in ("downloads", "cantus-refs"):
            return Path(os.environ["SOUNDLAB_REFS"]).joinpath(*p.parts[i + 1:])
    return p
