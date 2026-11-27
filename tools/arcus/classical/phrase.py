"""r42 arcus-classical — the solos' phrase measures (the analysis's, `tools/arcus/expr/phrase_study.py`'s
note cutter): per instrument and take (plain, performed), the notes' intonation against the
nearest semitone (p90 of |deviation| after the take's own offset), the long notes' (>= 0.6 s)
start and end under their peak, the vibrato's extent and onset — beside the HSD players' numbers
the analysis measured (`ref`, a JSON of phrase.py's own rows, e.g. the analysis's phrase.json).

    python -I tools/arcus/classical/phrase.py <out.json> <solos dir> [<ref.json>]

The solos are `cargo run -p cantus-ui --example arcus_demo -- <dir> solos`'s
`<name>-{plano,interpretado}.wav` (violin, viola, violonchelo, contrabajo).
"""
import json
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "expr"))
sys.path.insert(0, str(HERE.parent / "study"))
import phrase_study as ps  # noqa: E402
from bowlib_core import load  # noqa: E402

FILES = {"violin": "violin", "viola": "viola", "cello": "violonchelo", "bass": "contrabajo"}


def one(path, fam):
    x, sr = load(str(path))
    rows = ps.per_note(ps.notes_of(x, sr, fam))
    dev = np.array([100 * (r["midi"] - round(r["midi"])) for r in rows if r["dur"] >= 0.25])
    off = float(np.median(dev)) if len(dev) else 0.0
    rel = dev - off
    long = [r for r in rows if r["dur"] >= 0.6]
    return {
        "notes": len(rows),
        "tuning_offset_cents": off,
        "rel_dev_cents_p90_abs": float(np.percentile(np.abs(rel), 90)) if len(rel) else None,
        "per_note_dev": [round(float(v), 2) for v in dev],
        "vib_pp_long": ps.q([r["vib_pp"] for r in long]),
        "vib_onset_long": ps.q([r["vib_onset"] for r in long if (r["vib_pp"] or 0) >= 8]),
        "env_start_under_peak_long": ps.q([r["env"][1] for r in long if r["env"]]),
        "env_end_under_peak_long": ps.q([r["env"][2] for r in long if r["env"]]),
        "env_peak_share_long": ps.q([r["env"][0] for r in long if r["env"]]),
    }


def main(out, solos, ref=None):
    refs = json.loads(Path(ref).read_text(encoding="utf-8")) if ref else {}
    res = {}
    for fam, name in FILES.items():
        for tag in ("plano", "interpretado"):
            p = Path(solos) / f"{name}-{tag}.wav"
            if p.exists():
                res[f"{fam}-cantus-{tag}"] = one(p, fam)
        if f"{fam}-grabacion" in refs:
            res[f"{fam}-grabacion"] = refs[f"{fam}-grabacion"]
    Path(out).write_text(json.dumps(res, indent=1), encoding="utf-8")

    def med(v):
        return "—" if not v or v[1] is None else f"{v[1]:.1f}"

    print(f"{'':28s}{'p90 |dev| ct':>14s}{'start dB':>10s}{'end dB':>10s}{'peak at':>9s}{'vib ct':>8s}{'vib on s':>10s}")
    for k, r in res.items():
        print(f"{k:28s}{(r.get('rel_dev_cents_p90_abs') or 0):14.1f}{med(r['env_start_under_peak_long']):>10s}"
              f"{med(r['env_end_under_peak_long']):>10s}{med(r.get('env_peak_share_long')):>9s}"
              f"{med(r['vib_pp_long']):>8s}{med(r['vib_onset_long']):>10s}")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(*sys.argv[1:4])
