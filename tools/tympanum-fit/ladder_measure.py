"""The ladder's measures (round 40, ladder): the analyst's metrics (analysis-drums-2/
scripts: s04 cymbals, s09 comb and wires, s12 tails, s14 arrivals, s15 crest; metrics.py)
on every rung, plus a new one — the **cross-band envelope correlation**: how much the
1/3-octave bands from 2 to 16 kHz co-modulate (each band's log envelope band-passed to
5-40 Hz; the mean correlation of non-adjacent pairs). Independent noises under smooth
envelopes give ~0; a cymbal's crackle and a snare's wire bursts are broadband events.

    python -I ladder_measure.py <ladder dir> [model hits studio] [model hits jazz]

Writes `<ladder dir>/measures.json` and prints, per articulation, the measures per rung
and which rung first meets each target (within the tolerance of R0's own value).
"""
import json
import os
import sys
import numpy as np
import soundfile as sf
from scipy import signal, ndimage

SR = 48000

# Texture windows (s after the onset) for the co-modulation and fluctuation measures.
TEX = {"kick": (0.04, 0.30), "snare": (0.04, 0.30), "sidestick": (0.04, 0.20),
       "rimshot": (0.04, 0.30), "tom_mid": (0.04, 0.30), "tom_floor": (0.04, 0.30),
       "hat_closed": (0.02, 0.15), "hat_half": (0.02, 0.8), "hat_open": (0.05, 1.5),
       "ride": (0.1, 1.5), "ride_bell": (0.1, 1.5), "crash": (0.1, 1.5)}
# (a drum's first 40 ms is its attack, which every band shares as it falls: the attack
# has its own measures — arrival, spike, crest — and the texture is read after it)
XC_BANDS = 2000 * 2 ** (np.arange(10) / 3)        # 2 .. 16 kHz, 1/3 octave


def bandpass(m, lo, hi, order=4):
    sos = signal.butter(order, [lo, min(hi, 0.49 * SR)], btype="band", fs=SR, output="sos")
    return signal.sosfiltfilt(sos, m)


def band_env(m, lo, hi, win=0.005):
    y = bandpass(m, lo, hi)
    n = max(1, int(win * SR))
    return np.sqrt(np.convolve(y * y, np.ones(n) / n, mode="same") + 1e-30)


def db(x):
    return 20 * np.log10(np.abs(x) + 1e-15)


def xcorr(m, on, art, lo=5.0, hi=40.0):
    """Cross-band envelope correlation, 2-16 kHz 1/3-octave bands: each band's log
    envelope (Hilbert, 800 Hz) less a cubic trend in log time over the texture window
    (the decay every band shares is not co-modulation), band-passed to lo-hi Hz; the mean
    correlation of non-adjacent pairs (adjacent filters overlap). Independent noises under
    smooth envelopes read ~0 for long windows (a 0.3-s window's floor is ~0.1: compare
    against R3, the same envelopes with independent noise)."""
    a, b = TEX[art]
    d = 60
    fs = SR / d
    i0, i1 = int((on / SR + a) * fs), int((on / SR + b) * fs)
    t = np.log(np.arange(i0, i1) / fs - on / SR)
    lp = signal.butter(4, 300, fs=SR, output="sos")
    envs = []
    for c in XC_BANDS:
        y = bandpass(m, c * 2 ** (-1 / 6), c * 2 ** (1 / 6))
        e = signal.sosfiltfilt(lp, np.abs(signal.hilbert(y)))[::d]
        le = 20 * np.log10(np.maximum(e, 1e-9))[i0:i1]
        envs.append(le - np.polyval(np.polyfit(t, le, 3), t))
    E = signal.sosfiltfilt(signal.butter(2, [lo, hi], btype="band", fs=fs, output="sos"),
                           np.array(envs), axis=1)
    C = np.corrcoef(E)
    n = len(XC_BANDS)
    return float(np.mean([C[i, j] for i in range(n) for j in range(i + 2, n)]))


def fluct(m, on, art, lo, hi):
    """s04's fluctuation: the 2-ms band envelope (dB) about a cubic trend, its sd."""
    a, b = TEX[art] if art in ("hat_closed",) else ((0.1, 1.5) if art in ("hat_open", "hat_half", "ride", "ride_bell", "crash") else TEX[art])
    e = band_env(m, lo, hi, win=0.002)
    seg = 20 * np.log10(e[on + int(a * SR):on + int(b * SR)] + 1e-12)[::48]
    t = np.arange(len(seg))
    p = np.polyfit(t, seg, 3)
    return float(np.std(seg - np.polyval(p, t)))


def kurt(m, on, art):
    """Kurtosis of the 4-16 kHz amplitude envelope (1 ms) about its 30-ms running mean:
    bursts make it heavy-tailed."""
    a, b = TEX[art]
    e = band_env(m, 4000, 16000, win=0.001)[on + int(a * SR):on + int(b * SR)]
    r = e / (ndimage.uniform_filter1d(e, int(0.03 * SR)) + 1e-12) - 1
    r = r[::12]
    return float(np.mean(r ** 4) / (np.mean(r ** 2) ** 2 + 1e-30))


# ---- the analyst's metrics, per hit -------------------------------------------------------

DRUMS = ["kick", "snare", "sidestick", "rimshot", "tom_mid", "tom_floor"]
WIRES = ["snare", "rimshot"]
TONWIN = {"hat_closed": (0.01, 0.12), "hat_half": (0.1, 0.6), "hat_open": (0.1, 0.6),
          "ride": (0.15, 0.65), "ride_bell": (0.15, 0.65), "crash": (0.15, 0.65)}
TAIL = {"crash": (2.0, 3.3), "ride": (2.0, 3.3), "ride_bell": (2.0, 3.3),
        "hat_open": (1.6, 2.4), "hat_half": (0.8, 1.6)}


def flatness(p, axis=0):
    p = np.maximum(p, 1e-20)
    return np.exp(np.mean(np.log(p), axis=axis)) / np.mean(p, axis=axis)


def centroid(x):
    P = np.abs(np.fft.rfft(x * np.hanning(len(x)))) ** 2
    f = np.fft.rfftfreq(len(x), 1 / SR)
    return float((P * f).sum() / (P.sum() + 1e-30))


def energy(m, on, lo, hi, a_ms, b_ms):
    y = bandpass(m, lo, hi)[on + int(a_ms * SR / 1000):on + int(b_ms * SR / 1000)]
    return 10 * np.log10(np.mean(y ** 2) + 1e-30)


def schroeder(y, lo_db=-5, hi_db=-25):
    s = np.cumsum((y * y)[::-1])[::-1]
    s = 10 * np.log10(s / (s[0] + 1e-30) + 1e-30)
    i0, i1 = int(np.argmax(s <= lo_db)), int(np.argmax(s <= hi_db))
    if i1 <= i0 + 10 or s[-1] > hi_db:
        return np.nan
    t = np.arange(len(s)) / SR
    return float(-60.0 / np.polyfit(t[i0:i1], s[i0:i1], 1)[0])


def arrival(m, first):
    """s14: the strongest 2-16 kHz arrival in the first 12 ms, ms after the first sound
    (the first 0.1-ms envelope sample within 30 dB of it)."""
    y = bandpass(m, 2000, 16000)
    a = max(0, first - int(0.003 * SR))
    e = np.convolve(np.abs(signal.hilbert(y[a:first + int(0.012 * SR)])), np.ones(5) / 5, "same")
    d = 20 * np.log10(e + 1e-12)
    pk = int(np.argmax(d))
    st = int(np.argmax(d > d[pk] - 30))
    return (pk - st) / SR * 1000


def spike(m, on):
    """s15: 3 kHz+ in the first 1.5 ms re its peak in 2-50 ms (dB)."""
    hp = bandpass(m, 3000, 20000)
    return float(db(np.abs(hp[on:on + int(0.0015 * SR)]).max() / np.abs(hp[on + int(0.002 * SR):on + int(0.05 * SR)]).max()))


def crest(m, on):
    y = m[on:on + int(0.05 * SR)]
    return float(db(np.abs(y).max() / np.sqrt(np.mean(y ** 2))))


def cep_z(m, on):
    """s09: the largest real-cepstrum peak at 0.7-15 ms quefrency (0-80 ms), z."""
    y = m[on:on + int(0.08 * SR)]
    L = np.log(np.abs(np.fft.rfft(y * np.hanning(len(y)), 1 << 14)) + 1e-9)
    c = np.fft.irfft(L - L.mean())
    q = np.arange(len(c)) / SR * 1000
    k = (q > 0.7) & (q < 15)
    return float(np.abs(c[k]).max() / np.std(c[k]))


def flat_band(m, on, a, b, lo, hi, nfft=2048):
    y = m[on + int(a * SR):on + int(b * SR)]
    f, t, S = signal.stft(y, SR, nperseg=min(nfft, len(y)), noverlap=min(nfft, len(y)) // 2)
    P = np.abs(S) ** 2
    k = (f >= lo) & (f <= hi)
    return float(np.mean(flatness(P[k], axis=0)))


def wire_grain(m, on):
    e = band_env(m, 4000, 10000, 0.001)
    seg = 20 * np.log10(e[on + int(0.02 * SR):on + int(0.25 * SR)])[::24]
    t = np.arange(len(seg))
    return float(np.std(seg - np.polyval(np.polyfit(t, seg, 3), t)))


def tonal(m, on, art):
    """s04: 4-12 kHz flatness and peaks/kHz in the tonal window; 6-16 kHz effective
    components per kHz."""
    a, b = TONWIN[art]
    w = m[on + int(a * SR):on + int(b * SR)]
    n = len(w)
    nfft = 1 << int(np.ceil(np.log2(n)) + 1)
    P = np.abs(np.fft.rfft(w * np.hanning(n), nfft)) ** 2
    f = np.fft.rfftfreq(nfft, 1 / SR)
    df = f[1]
    d = 10 * np.log10(P + 1e-30)
    med = ndimage.median_filter(d, size=int(300 / df) | 1)
    s = (f >= 4000) & (f < 12000)
    pk, _ = signal.find_peaks(d[s] - med[s], height=12, distance=max(1, int(20 / df)))
    s2 = (f >= 6000) & (f < 16000)
    p = P[s2] / P[s2].sum()
    return float(flatness(P[s])), len(pk) / 8.0, float(1 / np.sum(p ** 2) * df / 1000 / 10)


def stability(m, on, art):
    nfft = 4096
    f = np.fft.rfftfreq(nfft, 1 / SR)
    s = (f >= 4000) & (f < 16000)
    a, b = (0.03, 0.25) if art == "hat_closed" else (0.2, 1.2)

    def fs(tt):
        i = on + int(tt * SR)
        X = 10 * np.log10(np.abs(np.fft.rfft(m[i:i + nfft] * np.hanning(nfft))) ** 2 + 1e-30)[s]
        return X - ndimage.uniform_filter1d(X, 41)
    cs, t = [], a
    while t + 0.05 + nfft / SR < b:
        cs.append(np.corrcoef(fs(t), fs(t + 0.05))[0, 1])
        t += 0.05
    return float(np.mean(cs)) if cs else np.nan


def tail(m, on, a, b):
    """s12: 4-12 kHz flatness and 6-16 kHz effective components (medians over frames)."""
    y = m[on + int(a * SR):on + int(b * SR)]
    f, t, S = signal.stft(y, SR, nperseg=4096, noverlap=2048)
    P = np.abs(S) ** 2
    k = (f >= 4000) & (f < 12000)
    k3 = (f >= 6000) & (f < 16000)
    p = P[k3] / (P[k3].sum(0) + 1e-30)
    return float(np.median(flatness(P[k], axis=0))), float(np.median(1 / np.sum(p ** 2, 0) / 10))


def measures(m, on, first, art):
    m = np.pad(m, (0, 4 * SR))
    r = {"xc": xcorr(m, on, art), "xc_fast": xcorr(m, on, art, 40, 150),
         "fl816": fluct(m, on, art, 8000, 16000), "kurt": kurt(m, on, art),
         "c3": centroid(m[on:on + int(0.003 * SR)]) / 1000}
    if art in DRUMS:
        r["arrival"] = arrival(m, first)
        r["spike"] = spike(m, on)
        r["crest"] = crest(m, on)
        r["cep"] = cep_z(m, on)
        r["c15_60"] = centroid(m[on + int(0.015 * SR):on + int(0.06 * SR)]) / 1000
        pk = energy(m, on, 20, 20000, 0, 80)
        r["sub"] = energy(m, on, 20, 55, 0, 80) - pk
        env = band_env(m, 1000, 4000, 0.02)
        r["hi500"] = float(db(env[on + int(0.5 * SR)] / env[on:on + int(0.1 * SR)].max()))
        r["T_low"] = schroeder(bandpass(m, 40, 250)[on:on + int(1.5 * SR)])
        if art in WIRES:
            r["wires"] = energy(m, on, 4000, 12000, 0, 100) - energy(m, on, 100, 400, 0, 100)
            r["flat412"] = flat_band(m, on, 0.01, 0.15, 4000, 12000)
            r["grain"] = wire_grain(m, on)
    else:
        r["hf3"] = energy(m, on, 6000, 20000, 0, 3) - energy(m, on, 200, 1500, 0, 3)
        r["flat"], r["peaks"], r["eff"] = tonal(m, on, art)
        r["stab"] = stability(m, on, art)
        r["fl48"] = fluct(m, on, art, 4000, 8000)
        eh = band_env(m, 8000, 16000, 0.005)[on:on + int(0.6 * SR)]
        r["at400"] = float(db(eh[int(0.4 * SR)] / eh.max()))
        r["T8_16"] = schroeder(bandpass(m, 8000, 16000)[on:on + int(3.5 * SR)])
        if art in TAIL:
            r["tail_flat"], r["tail_eff"] = tail(m, on, *TAIL[art])
    return r


# Tolerance per measure: (absolute, or relative when the second item is True).
TOL = {"xc": (0.06, False), "xc_fast": (0.04, False), "fl816": (0.25, False), "kurt": (0.6, False),
       "c3": (0.15, True), "arrival": (0.5, False), "spike": (2.0, False), "crest": (1.5, False),
       "cep": (1.0, False), "c15_60": (0.10, True), "sub": (3.0, False), "hi500": (3.0, False),
       "T_low": (0.15, True), "wires": (1.5, False), "flat412": (0.04, False), "grain": (0.3, False),
       "hf3": (2.0, False), "flat": (0.04, False), "peaks": (0.2, True), "eff": (0.2, True),
       "stab": (0.08, False), "fl48": (0.25, False), "at400": (2.0, False), "T8_16": (0.15, True),
       "tail_flat": (0.04, False), "tail_eff": (0.2, True)}


def meets(k, v, ref):
    if v is None or ref is None or not np.isfinite(v) or not np.isfinite(ref):
        return None
    tol, rel = TOL[k]
    return abs(v - ref) <= (tol * abs(ref) if rel else tol)


RUNGS = ["R0", "R1", "R2", "R3", "R4", "R3o", "R5"]
EXTRA = ["R3b", "R3o", "R5"]     # R3b: ladder_dense.py (a table per 11.7-Hz bin)
LADDER = ["R1", "R2", "R3", "R4"]
ENGINE = {"drs": "F:/projects/daw-r40/tmp/dg/h_c11/model-studio-hits.wav",
          "virt": "F:/projects/daw-r40/tmp/dg/hj_c11/model-jazz-hits.wav"}


def engine_measures(kit, art, vel):
    """Model 5 as the engine renders it (the drums-gen tables, twelve mics): the same
    measures, for the co-modulation table."""
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    from metrics import load, hit
    from early import onset1
    if art == "hat_half":
        return None
    x = load(ENGINE[kit])
    m, on = hit(x, art, vel, length=4.0, pre=0.0)
    m = np.concatenate([np.zeros(960), m])
    on += 960
    return measures(m, on, onset1(m, on), art)


def first_rung(cells):
    """The first ladder rung from which every later rung meets the target too."""
    for i, r in enumerate(LADDER):
        if all(cells.get(q) for q in LADDER[i:]):
            return r
    return "-"


def main():
    d = sys.argv[1]
    info = json.load(open(os.path.join(d, "ladder.json")))
    out = {}
    for key in sorted(info):
        kit, art, v = key.split("-")
        on, first = info[key]["onset"], info[key]["first"]
        res = {}
        for r in RUNGS:
            m, sr = sf.read(os.path.join(d, f"{key}-{r}.wav"))
            res[r] = measures(m, on, first, art)
        res["M5"] = engine_measures(kit, art, int(v[1:]))
        out[key] = res
        print(key, "done", flush=True)
    json.dump(out, open(os.path.join(d, "measures.json"), "w"), indent=1, default=float)
    report(d, out)


def report(d, out):
    from collections import defaultdict
    arts = defaultdict(list)
    for key in out:
        arts[key.split("-")[1]].append(key)
    lines = []
    summary = []
    for art, keys in arts.items():
        lines.append(f"\n### {art}  ({', '.join(k.split('-', 2)[0] + ' ' + k.split('-')[2] for k in keys)})\n")
        lines.append("| measure | R0 (each hit) | " + " | ".join(LADDER + [r for r in EXTRA if r in out[keys[0]]]) + " | first rung (all hits) |")
        lines.append("|---|---|" + "---|" * (len(LADDER) + 3))
        allc = {r: [] for r in LADDER + [r for r in EXTRA if r in out[keys[0]]]}
        firsts = []
        for k in out[keys[0]]["R0"]:
            refs = [out[key]["R0"][k] for key in keys]
            row = f"| {k} | " + " ".join(f"{x:.2f}" for x in refs) + " |"
            percell = []
            for r in LADDER + [r for r in EXTRA if r in out[keys[0]]]:
                ok = [meets(k, out[key][r][k], out[key]["R0"][k]) for key in keys]
                ok = [x for x in ok if x is not None]
                allc[r] += ok
                row += f" {sum(ok)}/{len(ok)} |"
            fr = []
            for key in keys:
                cells = {r: meets(k, out[key][r][k], out[key]["R0"][k]) for r in LADDER}
                if None in cells.values():
                    continue
                fr.append(first_rung(cells))
            order = LADDER + ["-"]
            worst = max(fr, key=order.index) if fr else "n/a"
            firsts.append(worst)
            lines.append(row + f" {worst} |")
        share = {r: (100 * np.mean(allc[r]) if allc[r] else np.nan) for r in allc}
        lines.append("| **share met** | | " + " | ".join(f"**{share[r]:.0f} %**" for r in LADDER + [r for r in EXTRA if r in out[keys[0]]]) + " | |")
        summary.append((art, share, firsts))
    head = ["\n## Summary: share of (measure x hit) cells within tolerance of R0\n",
            "| articulation | " + " | ".join(LADDER + [r for r in EXTRA if r in out[keys[0]]]) + " | measures never met by R4 |",
            "|---|" + "---|" * (len(LADDER) + 3)]
    for art, share, firsts in summary:
        keys0 = list(out[arts[art][0]]["R0"])
        never = [k for k, f in zip(keys0, firsts) if f == "-"]
        head.append(f"| {art} | " + " | ".join(f"{share[r]:.0f} %" for r in LADDER + [r for r in EXTRA if r in out[keys[0]]]) + f" | {', '.join(never)} |")
    xc = ["\n## Cross-band envelope correlation (2-16 kHz, 5-40 Hz; 40-150 Hz in brackets)\n",
          "| hit | R0 | model 5 (engine) | R1 | R3 | R4 | R3o |", "|---|---|---|---|---|---|---|"]
    for key in sorted(out):
        o = out[key]
        f = lambda r: "n/a" if o[r] is None else f"{o[r]['xc']:+.2f} ({o[r]['xc_fast']:+.2f})"
        xc.append(f"| {key} | " + " | ".join(f(r) for r in ["R0", "M5", "R1", "R3", "R4", "R3o"]) + " |")
    text = "\n".join(head + xc + ["\n## Per articulation\n"] + lines) + "\n"
    open(os.path.join(d, "measures.md"), "w", encoding="utf-8").write(text)
    print(text)


if __name__ == "__main__":
    if len(sys.argv) > 2 and sys.argv[2] == "report":
        report(sys.argv[1], json.load(open(os.path.join(sys.argv[1], "measures.json"))))
    else:
        main()
