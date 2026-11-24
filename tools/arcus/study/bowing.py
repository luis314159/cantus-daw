"""bowing.py - how bow force, speed and position set a real string's brightness (Lampis, CC BY 4.0).

Usage:  venv\\Scripts\\python.exe -I tools\\bowing.py
Reads features.csv of %CANTUS_STUDY_DOWNLOADS%\\study\\zenodo-lampis-bridge-force-static-dynamic\\
(robotic bowing of a cello A string, ~220 Hz, on a monochord: beta, bow speed vb m/s, bow force
Fb N, the bridge force's spectral centroid SC Hz) and fits
    log2(SC/f0) = c0 + cF*log2(Fb) + cB*log2(beta) + cV*log2(vb)
by least squares; writes refdb/bowing.json with the coefficients (octaves of centroid per
doubling of each), R^2 together and for each alone. No body: this is the string's own source.
"""
import os
import json
from pathlib import Path

import numpy as np

R = Path(__file__).resolve().parents[3] / "research" / "instruments-next" / "bowed-refdb"
F = Path(os.environ.get("CANTUS_STUDY_DOWNLOADS", r"F:\projects\daw-r42\downloads")) / "study" / "zenodo-lampis-bridge-force-static-dynamic" / "features.csv"


def r2(X, y):
    c = np.linalg.lstsq(X, y, rcond=None)[0]
    return c, float(1 - np.var(y - X @ c) / np.var(y))


def main():
    A = np.genfromtxt(F, delimiter=",", names=True)
    ok = (A["Fb"] > 0) & (A["SC"] > 0) & (A["vb"] > 0)
    y = np.log2(A["SC"][ok] / A["f0"][ok])
    cols = {"Fb": np.log2(A["Fb"][ok]), "beta": np.log2(A["beta"][ok]), "vb": np.log2(A["vb"][ok])}
    X = np.column_stack([np.ones(ok.sum())] + list(cols.values()))
    c, R2 = r2(X, y)
    alone = {k: r2(np.column_stack([np.ones(ok.sum()), v]), y)[1] for k, v in cols.items()}
    ratio = np.log2(A["Fb"][ok] / A["vb"][ok])
    cr, R2r = r2(np.column_stack([np.ones(ok.sum()), ratio]), y)
    out = {"n": int(ok.sum()), "f0_hz": float(np.median(A["f0"][ok])),
           "beta_values": sorted(set(np.round(A["beta"][ok], 3).tolist())),
           "vb_values_ms": sorted(set(A["vb"][ok].tolist())),
           "Fb_range_N": [float(A["Fb"][ok].min()), float(A["Fb"][ok].max())],
           "oct_per_doubling": {"Fb": float(c[1]), "beta": float(c[2]), "vb": float(c[3])},
           "R2_all": R2, "R2_alone": alone,
           "force_over_speed": {"oct_per_doubling": float(cr[1]), "R2": R2r}}
    (R / "bowing.json").write_text(json.dumps(out, indent=1))
    print(json.dumps(out, indent=1))


if __name__ == "__main__":
    main()
