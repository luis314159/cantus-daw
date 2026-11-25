"""vibrato.py - Arcus pass 5: how string players' vibrato really is, measured cycle by cycle on
the study's recordings, and on Arcus's renders the same way.

Per note (the study's tracker: `bowlib_feat._track`, YIN over two periods, hop 2.5 ms, on the
note from its onset to its end), the cents track against the note's median is detrended
(`detrend_at`, two vibrato periods) and cut into cycles at its alternate extrema:

- `rate_hz` (the cycles' median), `rate_drift_hz_s` (the cycles' rates against time, a
  least-squares slope over the vibrato's full part), `rate_jitter` (the mean |change| of the
  period from one cycle to the next over the mean period);
- `extent_pp_cents` (peak to trough, the cycles' median over the full part),
  `extent_jitter` (the mean |change| from cycle to cycle over the mean extent);
- `onset_s` (from the note's onset to the first cycle at a quarter of the full extent, the
  first 100 ms — the attack's own glide — not counted) and `rise_s` (from there to the first
  at 90 %), `plain_s` (how long the note starts within
  +-3 cents of its plain pitch, the vibrato not yet there);
- `centre_cents` (the vibrato's mean over its full part against the plain start's pitch,
  where a plain start of 80 ms or more exists: negative, the vibrato sits under the note);
- `asymmetry` (the cycles' share of the period spent rising, trough to peak, minus 0.5) and
  `h2` (the detrended track's second harmonic of the rate over its fundamental, dB);
- `level_extent_r` (Pearson's r of each cycle's extent against its level, within the note).

    python -I tools/arcus/pass5/vibrato.py refs <out.json>       the references
    python -I tools/arcus/pass5/vibrato.py note <file> [<file>...] one row per file

`refs` reads the catalogue (`F:/projects/daw-r42/study/refdb/catalog.json`, or
`$CANTUS_STUDY_REFDB`): every single-note arco file of each family (not the sections, not the
non-vibrato takes), and Philharmonia's legato and crescendo phrases (the notes after a
transition: slurred notes; the crescendo's cycles: extent against level). A `.f32` file is raw
little-endian float32 at 48 kHz.
"""
import json
import os
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "study"))
from bowlib_core import load, rms_db  # noqa: E402
from bowlib_feat import _f0_steady, _track, detrend_at  # noqa: E402
from bowlib_note import onset_end  # noqa: E402

FAMILY_RANGE = {"cello": (60, 1100), "bass": (38, 450), "viola": (120, 1500), "violin": (180, 3200)}
LOWEST = {"cello": 36, "bass": 28, "viola": 48, "violin": 55}


def read(path):
    p = str(path)
    if p.endswith(".f32"):
        return np.fromfile(p, dtype="<f4").astype(np.float64), 48000
    return load(p)


def extrema(v, dt, rate):
    """Alternate extrema of `v`: (index, +1 peak / -1 trough), at least a third of a period
    apart."""
    gap = max(int(0.33 / rate / dt), 2)
    out = []
    i = 1
    while i < len(v) - 1:
        lo, hi = max(i - gap, 0), min(i + gap + 1, len(v))
        if v[i] == v[lo:hi].max() and v[i] > 0:
            kind = 1
        elif v[i] == v[lo:hi].min() and v[i] < 0:
            kind = -1
        else:
            i += 1
            continue
        if out and out[-1][1] == kind:
            if (kind == 1 and v[i] > v[out[-1][0]]) or (kind == -1 and v[i] < v[out[-1][0]]):
                out[-1] = (i, kind)
        else:
            out.append((i, kind))
        i += gap
    return out


def cycles(t, v, rate):
    """Each cycle peak to peak: (centre time, period, extent peak to trough, rising share)."""
    ex = extrema(v, np.median(np.diff(t)), rate)
    peaks = [k for k, (i, s) in enumerate(ex) if s == 1]
    out = []
    for a, b in zip(peaks, peaks[1:]):
        if b - a != 2:
            continue
        i0, i1, i2 = ex[a][0], ex[a + 1][0], ex[b][0]
        per = t[i2] - t[i0]
        if not (0.08 < per < 0.4):
            continue
        ext = 0.5 * (v[i0] + v[i2]) - v[i1]
        rising = (t[i2] - t[i1]) / per
        out.append((0.5 * (t[i0] + t[i2]), per, ext, rising))
    return out


def analyse(x, sr, family, t_on=None, t_end=None):
    """One note's vibrato (the module's notes), or None."""
    fmin, fmax = FAMILY_RANGE[family]
    if t_on is None:
        _, _, t_on, t_end, _ = onset_end(x, sr)
    if t_end - t_on < 0.35:
        return None
    seg = x[int(t_on * sr):int(t_end * sr)]
    f_med = _f0_steady(seg, sr, fmin, fmax)
    if not f_med:
        return None
    t, f, ap = _track(seg, sr, f_med)
    if len(f) < 60:
        return None
    c = 1200 * np.log2(f / f_med)
    dt = float(np.median(np.diff(t)))
    # the rate: the detrended track's spectral peak in 3-9 Hz over the note's second half
    half = t >= t[len(t) // 2]
    v0 = c[half] - np.polyval(np.polyfit(t[half], c[half], 1), t[half])
    nfft = 1 << int(np.ceil(np.log2(len(v0) * 8)))
    sp = np.abs(np.fft.rfft(v0 * np.hanning(len(v0)), nfft))
    fr = np.fft.rfftfreq(nfft, dt)
    band = (fr >= 3.0) & (fr <= 9.0)
    r0 = float(fr[band][np.argmax(sp[band])])
    v = detrend_at(c, dt, r0)
    cy = cycles(t, v, r0)
    if len(cy) < 3:
        return {"f0_hz": f_med, "dur_s": float(t_end - t_on), "present": False}
    ct = np.array([q[0] for q in cy])
    per = np.array([q[1] for q in cy])
    ext = np.array([q[2] for q in cy])
    full = float(np.median(np.sort(ext)[len(ext) // 2:]))
    if full < 8.0:
        return {"f0_hz": f_med, "dur_s": float(t_end - t_on), "present": False,
                "extent_pp_cents": full}
    # the first 100 ms are the attack's own pitch (a glide into the note), not a cycle
    late = ct >= t[0] + 0.1
    k25 = np.nonzero((ext >= 0.25 * full) & late)[0]
    k90 = np.nonzero((ext >= 0.9 * full) & late)[0]
    onset = float(ct[k25[0]] - 0.5 * per[k25[0]]) if len(k25) else float("nan")
    rise = float(ct[k90[0]] - ct[k25[0]]) if len(k90) and len(k25) else float("nan")
    on_full = ct >= (ct[k90[0]] if len(k90) else ct[0])
    # the plain start: the track within +-3 cents of its own first 40 ms median
    first = c[t <= t[0] + 0.04]
    p0 = float(np.median(first)) if len(first) else 0.0
    plain = 0
    while plain < len(c) and abs(c[plain] - p0) <= 3.0:
        plain += 1
    plain_s = float(t[min(plain, len(t) - 1)] - t[0])
    full_part = t >= (ct[k90[0]] if len(k90) else t[0])
    centre = float(np.mean(c[full_part]) - p0) if plain_s >= 0.08 else float("nan")
    pf, ef = per[on_full], ext[on_full]
    rates = 1.0 / pf
    drift = float(np.polyfit(ct[on_full], rates, 1)[0]) if len(pf) >= 4 else float("nan")
    rj = float(np.mean(np.abs(np.diff(pf))) / np.mean(pf)) if len(pf) >= 3 else float("nan")
    ej = float(np.mean(np.abs(np.diff(ef))) / np.mean(ef)) if len(ef) >= 3 else float("nan")
    asym = float(np.median([q[3] for q in cy]) - 0.5)
    vv = v[full_part]
    tt = t[full_part]
    z1 = abs(np.sum(vv * np.exp(-2j * np.pi * r0 * tt)))
    z2 = abs(np.sum(vv * np.exp(-4j * np.pi * r0 * tt)))
    h2 = float(20 * np.log10(z2 / max(z1, 1e-12) + 1e-12))
    # each cycle's level, its extent against it
    lt, ld = rms_db(seg, sr, 0.04, 0.01)
    lvl = np.interp(ct, lt, ld)
    r = float(np.corrcoef(lvl[on_full], ext[on_full])[0, 1]) if on_full.sum() >= 5 and np.std(lvl[on_full]) > 0.3 else float("nan")
    return {
        "f0_hz": f_med, "dur_s": float(t_end - t_on), "present": True,
        "rate_hz": float(np.median(rates)), "rate_drift_hz_s": drift, "rate_jitter": rj,
        "extent_pp_cents": full, "extent_jitter": ej, "onset_s": onset, "rise_s": rise,
        "plain_s": plain_s, "centre_cents": centre, "asymmetry": asym, "h2_db": h2,
        "level_extent_r": r, "cycles": int(len(cy)),
    }


def summ(v):
    v = np.array([x for x in v if x is not None and x == x])
    if len(v) == 0:
        return None
    return {"n": int(len(v)), "p10": float(np.percentile(v, 10)), "p25": float(np.percentile(v, 25)),
            "median": float(np.median(v)), "p75": float(np.percentile(v, 75)),
            "p90": float(np.percentile(v, 90))}


KEYS = ("rate_hz", "rate_drift_hz_s", "rate_jitter", "extent_pp_cents", "extent_jitter",
        "onset_s", "rise_s", "plain_s", "centre_cents", "asymmetry", "h2_db", "level_extent_r")


def table(rows):
    pres = [r for r in rows if r.get("present")]
    out = {"notes": len(rows), "with_vibrato": len(pres)}
    for k in KEYS:
        out[k] = summ([r.get(k) for r in pres])
    return out


def midi_of(f):
    return 69 + 12 * np.log2(f / 440.0)


def _one(j):
    try:
        x, sr = read(j["path"])
    except Exception:
        return j, None
    r = analyse(x, sr, j["family"])
    if r is not None:
        r["dyn"] = j["dyn_id"]
        r["source"] = j["source"]
    return j, r


def refs(out):
    refdb = Path(os.environ.get("CANTUS_STUDY_REFDB", r"F:\projects\daw-r42\study\refdb"))
    cat = json.loads((refdb / "catalog.json").read_text())
    rows = {}
    jobs = [j for j in cat if j["tech"] == "arco" and not j["section"] and not j["multi"]]
    import multiprocessing as mp
    with mp.Pool(int(os.environ.get("ARCUS_WORKERS", "3"))) as pool:
        for j, r in pool.imap(_one, jobs, chunksize=8):
            if r is None:
                continue
            rows.setdefault(j["family"], []).append(r)
            print(j["family"], Path(j["path"]).name, r.get("rate_hz"), r.get("extent_pp_cents"), flush=True)
    # slurred notes and crescendos: Philharmonia's phrases (transitions from phrases.jsonl)
    phrases = [json.loads(l) for l in (refdb / "phrases.jsonl").read_text().splitlines() if l.strip()]
    slurred, cresc = {}, {}
    for p in phrases:
        if p["source"] != "philharmonia":
            continue
        legato = "legato" in p["tech"]
        cr = "cresc" in p["dyn"]
        if not (legato or cr):
            continue
        try:
            x, sr = read(p["path"])
        except Exception:
            continue
        edges = [tr["t_s"] for tr in p["transitions"]]
        _, _, t_on, t_end, _ = onset_end(x, sr)
        bounds = [t_on] + edges + [t_end]
        for i, (a, b) in enumerate(zip(bounds, bounds[1:])):
            r = analyse(x, sr, p["family"], a + 0.01, b - 0.02)
            if r is None:
                continue
            if legato and i > 0:
                slurred.setdefault(p["family"], []).append(r)
            if cr:
                cresc.setdefault(p["family"], []).append(r)
    Path(str(out) + ".rows.json").write_text(json.dumps({"notes": rows, "slurred": slurred, "crescendo": cresc}))
    summarise(rows, slurred, cresc, out)


def summarise(rows, slurred, cresc, out):
    res = {}
    for fam, rs in rows.items():
        d = {"all": table(rs)}
        for name, sel in (("soft", lambda r: (r["dyn"] or 0) in (1, 2)),
                          ("medium", lambda r: (r["dyn"] or 0) in (3, 4)),
                          ("loud", lambda r: (r["dyn"] or 0) >= 5)):
            d[name] = table([r for r in rs if sel(r)])
        for name, sel in (("short", lambda r: r["dur_s"] < 1.0), ("long", lambda r: r["dur_s"] >= 2.0)):
            d[name] = table([r for r in rs if sel(r)])
        lo = LOWEST[fam]
        for o in range(4):
            d[f"octave_{o}"] = table([r for r in rs if o <= (midi_of(r["f0_hz"]) - lo) / 12 < o + 1])
        d["slurred"] = table(slurred.get(fam, []))
        d["crescendo"] = table(cresc.get(fam, []))
        res[fam] = d
    Path(out).write_text(json.dumps(res, indent=1))


def main():
    if sys.argv[1] == "refs":
        refs(sys.argv[2])
        return
    if sys.argv[1] == "summary":
        d = json.loads(Path(sys.argv[2] + ".rows.json").read_text())
        summarise(d["notes"], d["slurred"], d["crescendo"], sys.argv[2])
        return
    fam = os.environ.get("ARCUS_FAMILY", "cello")
    for f in sys.argv[2:]:
        x, sr = read(f)
        r = analyse(x, sr, fam)
        print(Path(f).name, json.dumps({k: (round(v, 3) if isinstance(v, float) else v) for k, v in (r or {}).items()}))


if __name__ == "__main__":
    main()
