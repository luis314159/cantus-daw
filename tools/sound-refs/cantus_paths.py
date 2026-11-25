"""Where the sound tools find their data (round 42, sound-data): one place for every script.

    import sys; from pathlib import Path
    sys.path.insert(0, str(Path(__file__).resolve().parents[N] / "sound-refs"))  # tools/sound-refs
    import cantus_paths as cp
    cp.refs("study", "cnsm-violin")      # a reference set's folder
    cp.work("arcus", "p4")               # scratch for a tool's run

- `CANTUS_REFS`: the reference recordings and measured data (`tools/sound-refs/catalog.toml`
  names every set and the sub-path each lives at). Default `F:\\projects\\cantus-refs` on
  Windows, `~/cantus-refs` elsewhere.
- `CANTUS_WORK`: scratch and renders (the lab's runs, the listening renders). Default the
  folder `cantus-work` beside `CANTUS_REFS`.
- `CANTUS_STUDY_REFDB`: the bowed-strings study's measured database (per-note records, the
  catalogue of jobs, the plots). Default `$CANTUS_REFS/derived/refdb`.

Both stay outside every repository and every round's folder, so deleting a round deletes no
data. The older names (`SOUNDLAB_REFS`, `CANTUS_STUDY_DOWNLOADS`, `ARCUS_REFS`, …) still win
where a script read them before; this module only gives their defaults. Stdlib only.
"""
import os
from pathlib import Path

REFS_DEFAULT = r"F:\projects\cantus-refs" if os.name == "nt" else str(Path.home() / "cantus-refs")


def refs(*parts):
    """`$CANTUS_REFS` (default F:\\projects\\cantus-refs), joined with `parts`."""
    return Path(os.environ.get("CANTUS_REFS") or REFS_DEFAULT).joinpath(*parts)


def work(*parts):
    """`$CANTUS_WORK` (default `cantus-work` beside `CANTUS_REFS`), joined with `parts`."""
    w = os.environ.get("CANTUS_WORK")
    return (Path(w) if w else refs().parent / "cantus-work").joinpath(*parts)


def derived(*parts):
    """`$CANTUS_REFS/derived`: what the tools compute from the references (regenerable)."""
    return refs("derived", *parts)


def refdb(*parts):
    """The study's measured database: `$CANTUS_STUDY_REFDB`, else `$CANTUS_REFS/derived/refdb`."""
    r = os.environ.get("CANTUS_STUDY_REFDB")
    return (Path(r) if r else derived("refdb")).joinpath(*parts)


def env_or(name, default):
    """`$name` as a Path when it is set and not empty, else `default`."""
    v = os.environ.get(name)
    return Path(v) if v else Path(default)


# Folder names an absolute path recorded on another machine or in a deleted round may carry
# before its path under the references root (`…\daw-r42\downloads\arcus\…`).
_OLD_ROOTS = ("downloads", "cantus-refs")


def rebase(path):
    """A reference path as it is on this machine: unchanged when it exists; else a path
    recorded under an older root (a round's `downloads` folder, another `cantus-refs`)
    moved under `CANTUS_REFS`. The study's catalogue and records hold absolute paths."""
    p = Path(path)
    if p.exists():
        return p
    parts = p.parts
    for i in range(len(parts) - 1, -1, -1):
        if parts[i].lower() in _OLD_ROOTS:
            q = refs(*parts[i + 1:])
            if q.exists():
                return q
    return p
