"""fiddle_ltas.py - r42 fiddle: the long-term third-octave spectra of a few files beside the
references' (the study's `timbre`, dB re each file's total over 80 Hz), 250 Hz - 10 kHz.

    python -I tools/arcus/fiddle/fiddle_ltas.py <wav> [<wav> ...]

The references' columns come from $CANTUS_WORK/arcus/fiddle/fiddle_measures.json
(`fiddle_measure.py run`): the bluegrass fiddle and the classical scales.
"""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import fiddle_lib as fl  # noqa: E402
import fiddle_groups as fg  # noqa: E402
import fiddle_measure as fm  # noqa: E402

BANDS = ["250", "315", "397", "500", "630", "794", "1000", "1260", "1587", "2000", "2520",
         "3175", "4000", "5040", "6350", "8000", "10079"]


def main(paths):
    ref = json.load(open(fg.cp.work("arcus", "fiddle", "fiddle_measures.json"), encoding="utf-8"))
    cols = [("bluegrass", ref["fiddle-bluegrass"]["timbre.ltas"]),
            ("cl.scales", ref["classical-scales"]["timbre.ltas"])]
    for p in paths:
        x, sr = fl.load(p)
        t = fm.timbre(fl.Track(x, sr))
        cols.append((Path(p).stem[:12], t["ltas_third_oct_db"]))
        print(f"{Path(p).name}: centroid/f0 {t.get('centroid_over_f0', float('nan')):.2f}, "
              f">2k {t['share_above_2k_db']:.1f} dB")

    def val(v):
        return v[1] if isinstance(v, list) else v
    print("band   " + " ".join(f"{n:>12s}" for n, _ in cols))
    for b in BANDS:
        print(f"{b:>6s} " + " ".join(f"{val(c[b]):12.1f}" for _, c in cols))


if __name__ == "__main__":
    main(sys.argv[1:])
