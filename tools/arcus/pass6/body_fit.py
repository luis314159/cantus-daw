"""body_fit.py - r42 pass 6: fit an instrument's match correction (`arcus_tables.rs`'s
`<INST>_MATCH.eq_db`) to the measured instruments' body curve through the study's own
harmonic-peak estimator (`body.body_estimate`), instead of to the recordings' third-octave
spectrum (which carries their bow and room noise: matched with harmonics, it made the cello's
4-7 kHz 7 dB too hot).

    python -I tools/arcus/pass6/body_fit.py <study dir> <renders dir> <cello|bass> [step] [write]

Renders are `arcus_match render`'s (`ARCUS_SPLIT=train`: the two thirds the fit may see). Each
EQ band (`EQ_HZ`) moves by `step` (0.5) x the mean gap between the references' median curve and
Arcus's over the bins within a third of an octave of it; bands where the curve is not defined
(under A0) keep their value; the mean move is taken out (the level is the presets'). `write`
writes the new correction into `crates/cantus-dsp/src/arcus_tables.rs`.
"""
import json, re, sys
from pathlib import Path
import numpy as np

study, renders, fam = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3]
step = float(sys.argv[4]) if len(sys.argv) > 4 else 0.5
write = "write" in sys.argv[5:]
sys.path.insert(0, str(study / "tools"))
from bowlib_core import load  # noqa: E402
from bowlib_note import analyse_arco  # noqa: E402
from body import body_estimate  # noqa: E402

EQ_HZ = [63, 100, 160, 250, 400, 630, 1000, 1600, 2500, 4000, 6300, 10000]
ref = json.loads((study / "refdb" / f"{fam}.json").read_text())["body"]
rows = []
for p in sorted(renders.glob(f"arcus-{fam}-arco-*.wav")):
    x, sr = load(str(p))
    r = analyse_arco(x, sr, {"fmin": 30, "fmax": 1100, "pre": None})
    if r:
        rows.append(r)
fc, H, _, _ = body_estimate(rows)
fc = np.array(fc)
H = np.array([np.nan if v is None else v for v in H])
R = np.array([[np.nan if v is None else v for v in b["H_db"]] for b in ref.values()])
med = np.nanmedian(R, axis=0)
lo, hi = np.nanmin(R, axis=0), np.nanmax(R, axis=0)

tab = Path("crates/cantus-dsp/src/arcus_tables.rs")
src = tab.read_text(encoding="utf-8")
name = {"cello": "CELLO_MATCH", "bass": "BASS_MATCH"}[fam]
m = re.search(r"pub const %s: Match = Match \{.*?eq_db: \[([^\]]*)\]" % name, src, re.S)
eq = np.array([float(v) for v in m.group(1).replace("\n", " ").split(",") if v.strip()])
move = np.zeros(len(EQ_HZ))
ok = np.zeros(len(EQ_HZ), bool)
for i, f in enumerate(EQ_HZ):
    sel = (fc >= f * 2 ** (-1 / 3)) & (fc <= f * 2 ** (1 / 3)) & np.isfinite(H) & np.isfinite(med)
    if sel.sum() >= 2:
        move[i] = step * np.mean(med[sel] - H[sel])
        ok[i] = True
move[ok] -= move[ok].mean()
new = np.clip(eq + move, -12, 12).round(1)
outside = np.sqrt(np.nanmean(np.square(np.maximum(np.maximum(lo - H, 0), H - hi)[(fc >= 130) & (fc <= 10000)])))
print(f"{fam}: body curve RMS outside the band 130 Hz-10 kHz {outside:.2f} dB")
for f, a, b, c in zip(EQ_HZ, eq, move, new):
    print(f"  {f:6d} Hz  {a:+5.1f} -> {c:+5.1f}  (moved {b:+.1f})")
if write:
    old = m.group(1)
    rep = "\n        " + ", ".join(f"{v:.1f}" for v in new) + ",\n    "
    src = src[:m.start(1)] + rep + src[m.end(1):]
    tab.write_text(src, encoding="utf-8")
    print("written", tab)
