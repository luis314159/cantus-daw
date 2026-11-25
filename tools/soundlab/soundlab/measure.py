"""Measuring: the references (cached per file) and a candidate's run, the same estimators.

Every measured note is a row: {"id", "role": "ref" | "cand", "source", "licence", "inst",
"cell": [art, note, dyn], "group", "held_out", "f": flat features}. The references' rows are
cached in `<work>/refs/` by file (path, size, mtime) and by what was asked (features, kind,
f0 range, the lab's VERSION): a second report reads them back.
"""
import hashlib
import json
import os
import subprocess
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

import numpy as np

from . import VERSION, audio
from .band import flatten, group_of, held_out
from .core import segment
from .features import Note, analyse
from .notes import _f0_steady
from .subject import expand

SR = 48000


def _clean(o):
    if isinstance(o, dict):
        return {k: _clean(v) for k, v in o.items()}
    if isinstance(o, (list, tuple)):
        return [_clean(v) for v in o]
    if isinstance(o, (np.floating, float)):
        return float(o) if np.isfinite(o) else None
    if isinstance(o, np.integer):
        return int(o)
    return o


def notes_of(x, sr, entry, f_range):
    """[(clip, nominal)] of one file: the whole file (one note) or, for `multi`, each note cut
    at its silences and named by its own pitch (the study's way with Iowa's runs); `auto`, one
    note named by its own pitch."""
    if not entry.get("multi"):
        if entry.get("note") is None and entry.get("auto"):
            f = _f0_steady(x[:, 0], sr, *f_range)
            return [(x, int(round(69 + 12 * np.log2(f / 440.0))) if f else None)]
        return [(x, entry.get("note"))]
    out = []
    m = x[:, 0]
    for a, b in segment(m, sr):
        seg = x[a:b]
        f = _f0_steady(seg[:, 0], sr, *f_range)
        if f:
            out.append((seg, int(round(69 + 12 * np.log2(f / 440.0)))))
    return out


def measure_file(job):
    """One reference or render file -> its rows (run in a worker)."""
    path, entry, kinds, feats, f_range = job
    x, sr = audio.read(path)
    if sr != SR:
        x, sr = audio.resample(x, sr, SR), SR
    rows = []
    for i, (clip, nominal) in enumerate(notes_of(x, sr, entry, f_range)):
        art = entry["art"]
        note = Note(clip, sr, kinds[art], f_range, nominal)
        f = analyse(note, feats[art])
        rows.append({"index": i, "note": nominal, "errors": f.pop("_errors", None), "f": _clean(flatten(f))})
    return rows


def _key(path, entry, kinds, feats, f_range):
    st = os.stat(path)
    h = hashlib.sha256(json.dumps([str(path), st.st_size, int(st.st_mtime), entry, kinds, feats,
                                   list(f_range), VERSION], sort_keys=True).encode()).hexdigest()
    return h[:24]


def _row(base, r, subject, role):
    cell = (base["art"], r["note"], base["dyn"])
    return {"id": f"{base.get('path')}#{r['index']}", "role": role, "source": base.get("source"),
            "licence": base.get("licence"), "inst": base["inst"], "cell": list(cell),
            "group": group_of(cell, subject.lowest, subject.d.get("dyn_classes")),
            "held_out": held_out(cell, subject.split), "f": r["f"], "errors": r.get("errors")}


def _jobs(subject, files):
    arts = sorted({e["art"] for _, e in files})
    kinds = {a: subject.kind(a) for a in arts}
    feats = {a: subject.features(a) for a in arts}
    return [(str(p), e, kinds, feats, subject.f_range) for p, e in files]


def _run(jobs, workers, log):
    if workers <= 1:
        return [measure_file(j) for j in jobs]
    out = []
    with ProcessPoolExecutor(workers) as ex:
        for i, r in enumerate(ex.map(measure_file, jobs, chunksize=1)):
            out.append(r)
            if log and (i + 1) % 25 == 0:
                log(f"  {i + 1}/{len(jobs)}")
    return out


def measure_refs(subject, workers=None, log=print):
    workers = workers or int(os.environ.get("SOUNDLAB_WORKERS", "3"))
    root = subject.ref_root()
    cache = subject.work / "refs"
    cache.mkdir(parents=True, exist_ok=True)
    cat = subject.catalogue()
    files = [(root / e["path"], e) for e in cat]
    jobs = _jobs(subject, files)
    keys = [_key(*j) for j in jobs]
    todo = [(j, k) for j, k in zip(jobs, keys) if not (cache / f"{k}.json").exists()]
    if todo:
        log(f"measuring {len(todo)} of {len(jobs)} reference files ({workers} workers)")
        for (j, k), r in zip(todo, _run([t[0] for t in todo], workers, log)):
            (cache / f"{k}.json").write_text(json.dumps(r))
    rows = []
    for (p, e), k in zip(files, keys):
        for r in json.loads((cache / f"{k}.json").read_text()):
            rows.append(_row(e, r, subject, "ref"))
    return rows


def render(subject, run, log=print):
    """The candidate's command into <work>/runs/<run>/renders ({out} in its arguments)."""
    c = subject.d["candidate"]
    out = subject.run_dir(run) / "renders"
    out.mkdir(parents=True, exist_ok=True)
    cmd = [expand(a, subject).replace("{out}", str(out)) for a in c["render"]]
    env = dict(os.environ, **{k: expand(v, subject) for k, v in (c.get("env") or {}).items()})
    cwd = expand(c.get("cwd", "{repo}"), subject)
    log("render: " + " ".join(cmd))
    subprocess.run(cmd, cwd=cwd, env=env, check=True)
    return out


def measure_run(subject, run, renders=None, workers=None, log=print):
    workers = workers or int(os.environ.get("SOUNDLAB_WORKERS", "3"))
    renders = Path(renders) if renders else subject.run_dir(run) / "renders"
    cells = subject.candidate_cells(renders)
    if not cells:
        raise SystemExit(f"no candidate file in {renders} matches {subject.d['candidate']['files']}")
    inst = subject.d["candidate"].get("inst", "candidate")
    files = [(p, {"path": p.name, "source": "candidate", "licence": None, "inst": inst,
                  "art": c[0], "note": c[1], "dyn": c[2]}) for p, c in cells]
    log(f"measuring {len(files)} candidate files of run {run}")
    rows = []
    for (p, e), rs in zip(files, _run(_jobs(subject, files), workers, log)):
        for r in rs:
            rows.append(_row(e, r, subject, "cand"))
    d = subject.run_dir(run)
    d.mkdir(parents=True, exist_ok=True)
    (d / "measures.json").write_text(json.dumps({"run": run, "renders": str(renders), "rows": rows}))
    return rows
