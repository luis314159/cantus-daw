"""The sound lab's one command (tools/soundlab/README.md has the protocol).

    python -I tools/soundlab/lab.py check   <subject.json>
    python -I tools/soundlab/lab.py refs    <subject.json>                      measure the references (cached)
    python -I tools/soundlab/lab.py render  <subject.json> <run>                run the candidate's command
    python -I tools/soundlab/lab.py measure <subject.json> <run> [--from <dir>] measure a run's renders
    python -I tools/soundlab/lab.py report  <subject.json> <run> [--before <run>]
    python -I tools/soundlab/lab.py all     <subject.json> <run> [--before <run>] [--from <dir>]
    python -I tools/soundlab/lab.py kit     <subject.json> <kit dir> [--wav] [--wav-twin <dir>] [--run <run>]

The work folder is <SOUNDLAB_WORK or $CANTUS_WORK/soundlab>/<subject id>:
refs/ (the references' cache), runs/<run>/ (renders/, measures.json, report.json, report.html).
The references live under the subject's root (SOUNDLAB_REFS, else CANTUS_REFS), never in
the repository. SOUNDLAB_WORKERS sets the parallel measurers (3).
"""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from soundlab.subject import Subject  # noqa: E402


def opt(args, name, default=None):
    if name in args:
        i = args.index(name)
        v = args[i + 1]
        del args[i:i + 2]
        return v
    return default


def flag(args, name):
    if name in args:
        args.remove(name)
        return True
    return False


def load_run(subject, run):
    p = subject.run_dir(run) / "measures.json"
    if not p.exists():
        raise SystemExit(f"run {run} is not measured ({p}): `lab.py measure` it first")
    return json.loads(p.read_text())["rows"]


def main(argv):
    if len(argv) < 2:
        print(__doc__)
        return 2
    cmd, args = argv[0], argv[1:]
    from soundlab import measure, report
    if cmd == "kit":
        from soundlab import kit
        wav = flag(args, "--wav")
        twin = opt(args, "--wav-twin")
        run = opt(args, "--run")
        s = Subject(args[0])
        kit.build(s, Path(args[1]), flac=not wav, twin=Path(twin) if twin else None, run=run)
        return 0
    src = opt(args, "--from")
    before = opt(args, "--before")
    s = Subject(args[0])
    if cmd == "check":
        p = s.check()
        for line in p:
            print("problem:", line)
        cat = s.catalogue()
        print(f"{s.id}: {len(cat)} references, licences {sorted({e['licence'] for e in cat})}; "
              f"{'no problem' if not p else str(len(p)) + ' problem(s)'}")
        return 1 if p else 0
    if cmd in ("refs", "all"):
        p = [x for x in s.check() if not x.startswith("reference") or "missing" not in x]
        if p:
            for line in p:
                print("problem:", line)
            return 1
        rows = measure.measure_refs(s)
        print(f"references: {len(rows)} notes, {sum(r['held_out'] for r in rows)} in held-out cells")
        if cmd == "refs":
            return 0
    run = args[1]
    if cmd == "render" or (cmd == "all" and not src):
        measure.render(s, run)
        if cmd == "render":
            return 0
    if cmd in ("measure", "all"):
        measure.measure_run(s, run, renders=src)
        if cmd == "measure":
            return 0
    if cmd in ("report", "all"):
        refs = measure.measure_refs(s)
        rep = report.build(s, refs, load_run(s, run), run,
                           load_run(s, before) if before else None, before)
        out = report.write(rep, s.run_dir(run), s)
        for k, v in rep["summary"].items():
            c = rep.get("compare", {}).get(k)
            tail = f"   before {c['before']:.3g} -> {c['now']:.3g} ({c['verdict']})" if c else ""
            if "in" in v:
                print(f"  {k:44s} {v['in']:2d}/{v['groups']:<2d} in band, {v['widths_mean']:.2f} widths out (mean){tail}")
            else:
                print(f"  {k:44s} {100 * v['in_share']:3.0f} % in, RMS out {v['outside_rms_mean']:.2f} (mean){tail}")
        print(f"report: {out}")
        return 0
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
