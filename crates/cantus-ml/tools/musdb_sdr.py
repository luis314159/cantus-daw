"""SDR on MUSDB18-HQ's test set, as the HT-Demucs paper reports it (ADR 0085).

    python -I musdb_sdr.py <musdb18hq/test> <separated> [label]

BSS Eval v4 through museval's own metrics (1-second windows and hops), the median of each
track's windows, then the median and the mean over the tracks, per source. Only museval's
`metrics` module is loaded (its package would want ffmpeg for the compressed MUSDB18).
MUSDB18-HQ is for non-commercial research: used here only to measure, never committed.
"""
import importlib.util
import json
import multiprocessing
import os
import sys

import numpy as np
import soundfile as sf

spec = importlib.util.find_spec("museval")
metrics_path = os.path.join(os.path.dirname(spec.origin), "metrics.py")
ms = importlib.util.spec_from_file_location("museval_metrics", metrics_path)
metrics = importlib.util.module_from_spec(ms)
ms.loader.exec_module(metrics)

SOURCES = ["drums", "bass", "other", "vocals"]
# The paper's Table 1 (arXiv 2211.08553), "HT Demucs, 800 extra songs" — the released
# `htdemucs` (the 9.00 dB of the README is the per-source fine-tuned `htdemucs_ft`).
PUBLISHED = {"drums": 10.05, "bass": 9.78, "other": 6.42, "vocals": 8.93}


def one(job):
    """One track: each source's median SDR over its 1-second windows, kept beside its stems
    (`sdr.json`) so a run cut short resumes; None when it ran out of memory (tried again
    alone)."""
    musdb, out, t = job
    cache = os.path.join(out, t, "sdr.json")
    if os.path.exists(cache):
        with open(cache) as f:
            return t, json.load(f)
    try:
        res = score(musdb, out, t)
    except MemoryError:
        return t, None
    with open(cache, "w") as f:
        json.dump(res, f)
    return t, res


def score(musdb, out, t):
    ref, est = [], []
    for s in SOURCES:
        r, _ = sf.read(os.path.join(musdb, t, f"{s}.wav"), dtype="float64", always_2d=True)
        e, _ = sf.read(os.path.join(out, t, f"{s}.wav"), dtype="float64", always_2d=True)
        n = min(len(r), len(e))
        ref.append(r[:n])
        est.append(e[:n])
    n = min(len(x) for x in ref)
    ref = np.stack([x[:n] for x in ref])
    est = np.stack([x[:n] for x in est])
    sdr, _, _, _, _ = metrics.bss_eval(ref, est, window=44100, hop=44100,
                                       compute_permutation=False, filters_len=512,
                                       framewise_filters=False, bsseval_sources_version=False)
    res = []
    for i in range(len(SOURCES)):
        frames = sdr[i][np.isfinite(sdr[i])]
        res.append(float(np.median(frames)) if len(frames) else float("nan"))
    return res


def main():
    musdb, out = sys.argv[1], sys.argv[2]
    label = sys.argv[3] if len(sys.argv) > 3 else os.path.basename(out)
    per = {s: [] for s in SOURCES}
    tracks = sorted(t for t in os.listdir(out) if os.path.isdir(os.path.join(out, t)))
    # The filters span the whole track (BSS Eval v4): about 4 GB a process at its peak.
    jobs = int(os.environ.get("SDR_JOBS", "3"))
    done = {}
    with multiprocessing.Pool(jobs) as pool:
        for t, sdrs in pool.imap_unordered(one, [(musdb, out, t) for t in tracks]):
            done[t] = sdrs
            if sdrs is not None:
                print(t, " ".join(f"{s}={v:.2f}" for s, v in zip(SOURCES, sdrs)), flush=True)
    for t in tracks:
        if done[t] is None:
            done[t] = one((musdb, out, t))[1]
            if done[t] is None:
                sys.exit(f"{t}: out of memory even alone")
    for t in tracks:
        for s, v in zip(SOURCES, done[t]):
            per[s].append(v)
    summary = {s: {"median": float(np.nanmedian(per[s])), "mean": float(np.nanmean(per[s]))} for s in SOURCES}
    print(f"\n{label}: {len(tracks)} tracks")
    for s in SOURCES:
        print(f"  {s:>7}: median {summary[s]['median']:.2f} dB (the paper: {PUBLISHED[s]:.2f}), "
              f"mean {summary[s]['mean']:.2f} dB")
    avg = np.mean([summary[s]["median"] for s in SOURCES])
    print(f"  average of the medians: {avg:.2f} dB (the paper: 8.80 dB)")
    with open(os.path.join(out, "sdr.json"), "w") as f:
        json.dump({"tracks": tracks, "per_track": per, "summary": summary, "average": avg}, f, indent=1)


if __name__ == "__main__":
    main()
