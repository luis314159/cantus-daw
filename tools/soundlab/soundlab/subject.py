"""The subject file: everything the lab needs to know about one instrument.

```json
{
 "id": "arcus-cello", "name": "Arcus, the cello",
 "kind": {"arco": "sustained", "pizz": "decaying"},     // or one kind for every articulation
 "f_range": [60, 1100], "lowest_note": 36,
 "features": {"arco": ["loudness", "pitch", ...], "pizz": [...]},   // or one list
 "references": {"root": "$SOUNDLAB_REFS", "catalogue": "references.json",
                "also_allowed": {"Iowa-MIS": "<the terms, verbatim>"}},
 "split": {"rule": "hash", "seed": 42} | {"rule": "mod", "expr": "...", "dyn_index": {...}},
 "candidate": {"inst": "arcus", "render": ["cmd", "{out}", ...], "cwd": "{repo}",
               "env": {...}, "files": "<regex with (?P<art>) (?P<note>) (?P<dyn>)>"},
 "listening": { ... the blind kit, kit.py ... }
}
```

A catalogue entry (one reference file): `path` (under the root), `source`, `licence` (an SPDX
id: CC0-1.0, CC-BY-3.0, CC-BY-4.0, or one the subject allows with its terms), `credit`,
`inst` (one physical instrument: the band weighs each as one), `art`, `note` (MIDI; null with
`multi` for a file of several notes, each then named by its own pitch), `dyn`.
Paths and strings may hold `$NAME` / `${NAME}` (the environment) and `{repo}`, `{lab}`,
`{subject}` (this file's folder), `{work}`.
"""
import json
import os
import re
from pathlib import Path

LAB = Path(__file__).resolve().parents[1]
REPO = LAB.parents[1]
# Where the listening renders live on this machine (the kits' voices may name it).
os.environ.setdefault("SOUNDLAB_LISTEN", str(REPO.parent / "listen"))
FREE = {"CC0-1.0", "CC-BY-3.0", "CC-BY-4.0", "PD"}
REQUIRED = ("id", "name", "kind", "f_range", "features", "references", "split")


class SubjectError(ValueError):
    pass


def expand(s, subject=None):
    if not isinstance(s, str):
        return s
    s = os.path.expandvars(s)
    ctx = {"repo": str(REPO), "lab": str(LAB)}
    if subject is not None:
        ctx["subject"] = str(subject.dir)
        ctx["work"] = str(subject.work)
    for k, v in ctx.items():
        s = s.replace("{" + k + "}", v)
    return s


class Subject:
    def __init__(self, path):
        self.path = Path(path).resolve()
        self.dir = self.path.parent
        try:
            self.d = json.loads(self.path.read_text(encoding="utf-8"))
        except json.JSONDecodeError as e:
            raise SubjectError(f"{self.path}: not JSON: {e}") from e
        for k in REQUIRED:
            if k not in self.d:
                raise SubjectError(f"{self.path}: the subject names no `{k}`")
        self.id = self.d["id"]
        w = os.environ.get("SOUNDLAB_WORK") or str(REPO.parent / "soundlab-work")
        self.work = Path(expand(self.d.get("work", w + "/" + self.id)))

    # ---- what each articulation is
    def kind(self, art):
        k = self.d["kind"]
        return k if isinstance(k, str) else k.get(art, k.get("*", "sustained"))

    def features(self, art):
        f = self.d["features"]
        return f if isinstance(f, list) else f.get(art, f.get("*", []))

    @property
    def f_range(self):
        return tuple(self.d["f_range"])

    @property
    def lowest(self):
        return int(self.d.get("lowest_note", 0))

    @property
    def split(self):
        return self.d["split"]

    # ---- the references
    def ref_root(self):
        return Path(expand(self.d["references"].get("root", "$SOUNDLAB_REFS"), self))

    def catalogue(self):
        r = self.d["references"]
        cat = r.get("catalogue", [])
        if isinstance(cat, str):
            cat = json.loads((self.dir / expand(cat, self)).read_text(encoding="utf-8"))
        return cat

    def allowed(self):
        return FREE | set((self.d["references"].get("also_allowed") or {}).keys())

    def check(self, need_files=True):
        """Every problem a subject has, as text (empty: fine): a licence outside CC0 / CC BY
        (or the subject's own allowed terms), a file missing, an entry without its fields."""
        problems = []
        allowed = self.allowed()
        for k, terms in (self.d["references"].get("also_allowed") or {}).items():
            if not isinstance(terms, str) or len(terms) < 20:
                problems.append(f"also_allowed {k}: give the terms verbatim")
        root = self.ref_root()
        if not self.d["references"].get("root") and not os.environ.get("SOUNDLAB_REFS"):
            problems.append("references: no root (set SOUNDLAB_REFS to the downloads folder)")
        try:
            if root.resolve().is_relative_to(REPO.resolve()):
                problems.append(f"references: the root {root} is inside the repository: downloads stay outside it")
        except OSError:
            pass
        for i, e in enumerate(self.catalogue()):
            for k in ("path", "source", "licence", "inst", "art", "dyn"):
                if e.get(k) in (None, ""):
                    problems.append(f"reference {i}: no `{k}`")
            if e.get("licence") not in allowed:
                problems.append(f"reference {i} ({e.get('path')}): licence {e.get('licence')!r} is not CC0 / CC BY")
            if e.get("note") is None and not (e.get("multi") or e.get("auto")):
                problems.append(f"reference {i} ({e.get('path')}): no note (or `multi` / `auto`)")
            if need_files and not (root / e.get("path", "")).exists():
                problems.append(f"reference {i}: missing {root / e.get('path', '')}")
        c = self.d.get("candidate") or {}
        if c and not c.get("files"):
            problems.append("candidate: no `files` pattern")
        elif c:
            try:
                rx = re.compile(c["files"])
                for g in ("art", "note", "dyn"):
                    if g not in rx.groupindex and g not in c.get("fixed", {}):
                        problems.append(f"candidate.files names no (?P<{g}>) and candidate.fixed no {g}")
            except re.error as e:
                problems.append(f"candidate.files: {e}")
        return problems

    # ---- the candidate
    def run_dir(self, run):
        return self.work / "runs" / run

    def candidate_cells(self, renders):
        """[(path, (art, note, dyn))] of the render folder's files the pattern names."""
        c = self.d["candidate"]
        rx = re.compile(c["files"])
        fixed = c.get("fixed", {})
        dyn_map = c.get("dyn_map", {})
        out = []
        for p in sorted(Path(renders).iterdir()):
            m = rx.fullmatch(p.name)
            if not m:
                continue
            g = {**fixed, **m.groupdict()}
            out.append((p, (g["art"], int(g["note"]), dyn_map.get(g["dyn"], g["dyn"]))))
        return out
