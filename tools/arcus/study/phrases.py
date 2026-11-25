"""phrases.py - note-to-note transitions, re-bows and vibrato in played phrases (Arcus study).

Usage:  venv\\Scripts\\python.exe -I tools\\phrases.py [refdb\\phrases.jsonl]
Analyses the Philharmonia phrase files (legato, detache, martele, portato, tenuto, ... for the
four families) and the PHENICX-Anechoic / Aalto anechoic orchestral parts (real repertoire,
anechoic) and the HSD Mozart K465 quartet's separately recorded close-miked stems (CC BY 4.0). For each file: the pitch track (YIN, 5 ms), its stable notes (>= 80 ms within
+-50 cents of their median), and between consecutive notes of different pitch:
  - `trans_ms`: the time the pitch spends outside both notes' +-50-cent bands;
  - `dip_db`: the deepest level dip in the transition re the mean of the 50 ms either side;
  - `ap_max`: YIN's aperiodicity peak in it (0 periodic ... 1 noise): the bow change's noise;
  - `interval_st`: the interval in semitones.
Inside a stable note, level dips >= 4 dB lasting >= 10 ms are counted as re-bows (`rebows`).
Notes >= 0.6 s give vibrato rate and extent (bowlib_feat.vibrato_stats).
"""
import os
import json
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from bowlib_core import load, yin_track, rms_db, median_filter  # noqa: E402
from bowlib_feat import vibrato_stats  # noqa: E402
from catalog import FAM, cp  # noqa: E402

S = (Path(os.environ.get("CANTUS_STUDY_DOWNLOADS") or cp.refs())) / "study"
PH = {"cello": "cello", "double-bass": "bass", "viola": "viola", "violin": "violin"}
ANE = {"cello": "cello", "doublebass": "bass", "viola": "viola", "violin": "violin", "bass": "bass"}


def files():
    out = []
    for p in sorted(S.glob("philharmonia*/**/*_phrase_*.mp3")):
        parts = p.stem.split("_")
        if parts[0] in PH:
            out.append({"path": str(p), "source": "philharmonia", "family": PH[parts[0]],
                        "inst": "philharmonia-" + PH[parts[0]], "tech": parts[-1], "dyn": parts[3],
                        "lossy": True})
    for p in sorted(S.glob("phenicx-anechoic/**/audio/**/*.wav")):
        key = p.stem.lower().rstrip("0123456789_")
        fam = next((v for k, v in ANE.items() if key.startswith(k)), None)
        if fam:
            out.append({"path": str(p), "source": "phenicx", "family": fam, "inst": "aalto-" + fam,
                        "tech": "repertoire", "dyn": None, "lossy": False})
    hsd = {"Vl1": "violin", "Vl2": "violin", "Vla": "viola", "Vlc": "cello"}
    for p in sorted(S.glob("hsd-string-quartet-k465/**/*.wav")):
        k = p.stem.split("_")[-1]
        if k in hsd:
            out.append({"path": str(p), "source": "hsd", "family": hsd[k], "inst": f"hsd-{k.lower()}",
                        "tech": "repertoire", "dyn": None, "lossy": False})
    return out


def analyse(path, family):
    x, sr = load(path)
    fmin, fmax = FAM[family]
    t, f, ap = yin_track(x, sr, fmin, fmax, hop_s=0.005)
    if len(t) < 50:
        return None
    et, e = rms_db(x, sr, 0.02, 0.005)
    lv = np.interp(t, et, e)
    peak = np.percentile(lv, 99)
    voiced = (ap < 0.25) & (lv > peak - 35)
    st = 12 * np.log2(np.maximum(f, 1) / 440.0) + 69
    st = median_filter(st, 7)
    notes, i, n = [], 0, len(t)
    while i < n:
        if not voiced[i]:
            i += 1
            continue
        j = i
        med = st[i]
        while j + 1 < n and voiced[j + 1] and abs(st[j + 1] - med) < 0.5:
            j += 1
            if (j - i) % 8 == 0:
                med = float(np.median(st[i:j + 1]))
        if t[j] - t[i] >= 0.08:
            notes.append((i, j, float(np.median(st[i:j + 1]))))
        i = j + 1
    trans, rebows, vibs = [], [], []
    for (a0, a1, pa), (b0, b1, pb) in zip(notes, notes[1:]):
        gap = t[b0] - t[a1]
        if abs(pb - pa) < 0.5 or gap > 0.3:
            continue
        ctx = np.concatenate([lv[max(a1 - 10, a0):a1 + 1], lv[b0:min(b0 + 11, b1 + 1)]])
        seg = lv[a1:b0 + 1]
        trans.append({"t_s": float(t[a1]), "trans_ms": float(1000 * gap),
                      "dip_db": float(np.mean(ctx) - seg.min()) if len(seg) else 0.0,
                      "ap_max": float(ap[a1:b0 + 1].max()), "interval_st": float(pb - pa)})
    for a0, a1, p in notes:
        seg = lv[a0:a1 + 1]
        if len(seg) > 40:
            base = median_filter(seg, 21)
            d = base - seg
            k, m = 0, len(d)
            while k < m:
                if d[k] >= 4:
                    q = k
                    while q + 1 < m and d[q + 1] >= 2:
                        q += 1
                    if (q - k + 1) * 0.005 >= 0.01:
                        rebows.append({"depth_db": float(d[k:q + 1].max()), "dur_ms": float((q - k + 1) * 5)})
                    k = q + 1
                else:
                    k += 1
        if t[a1] - t[a0] >= 0.6:
            fm = 440 * 2 ** ((p - 69) / 12)
            v = vibrato_stats(t[a0:a1 + 1], f[a0:a1 + 1], fm, win_s=2.0 / fmin)
            if v:
                v["dur_s"] = float(t[a1] - t[a0])
                v["midi"] = p
                vibs.append(v)
    return {"n_notes": len(notes), "transitions": trans, "rebows": rebows, "vibrato": vibs,
            "dur_s": float(len(x) / sr)}


def main():
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else cp.env_or("CANTUS_STUDY_REFDB", Path(__file__).resolve().parents[3] / "research" / "instruments-next" / "bowed-refdb") / "phrases.jsonl"
    with open(out, "w") as fh:
        for k, j in enumerate(files()):
            try:
                r = analyse(j["path"], j["family"])
            except Exception as e:  # noqa: BLE001
                print("error", j["path"], e)
                continue
            if r:
                j.update(r)
                fh.write(json.dumps(j) + "\n")
            if k % 20 == 0:
                print(k, j["inst"], j["tech"], r and r["n_notes"], flush=True)


if __name__ == "__main__":
    main()
