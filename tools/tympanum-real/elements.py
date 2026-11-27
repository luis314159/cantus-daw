#!/usr/bin/env python3
"""Element by element: every piece of Tympanum's kits beside the recorded kits (round 42,
tympanum-real; the owner's "suena horroroso" on model 6's rock).

    python -I tools/tympanum-real/elements.py <render dir> <out dir>

`<render dir>` holds `tympanum_real hits` renders (`--vels 24,72,120 --takes 8`): the recorded
kits (`drskit`, `bigrusty`, `virtuosity`, `gogodze`) and Tympanum's (`studio5`, `studio6`,
`jazz5`, `jazz6`: model 5 and model 6), and the snare alone through its close, overhead and room
microphones (`<kit>-close`, `-oh`, `-room`). For each piece of `PIECES` it writes, into
`<out dir>`: the hits at pp/mf/ff as 16-bit WAVs at one loudness, spectrograms (0–20 kHz, log
frequency, 1.5 s and the first 100 ms), the third-octave spectrum against the recordings'
spread, the decay and the first 100 ms per octave band, the partials, the stroke-to-stroke
variation of 8 takes, the microphones' image (the snare), and `numbers.json`; the page itself is
written by `elements_page.py` from those and the verdicts.
"""
import csv
import json
import sys
from pathlib import Path

import numpy as np
import soundfile as sf
from scipy import signal

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

sys.path.insert(0, str(Path(__file__).resolve().parent))
from features import onset  # noqa: E402

REFS = [("drskit", "DRSKit"), ("bigrusty", "Big Rusty"), ("virtuosity", "Virtuosity"), ("gogodze", "Gogodze Phu II")]
TYMP = [("studio5", "Studio · modelo 5"), ("studio6", "Studio · modelo 6"),
        ("jazz5", "Jazz · modelo 5"), ("jazz6", "Jazz · modelo 6")]
KITS = REFS + TYMP
VELS = [(24, "pp"), (72, "mf"), (120, "ff")]
# id, Spanish name, kind, how to pick its articulation
PIECES = [
    ("bombo", "Bombo", "kick", "center"),
    ("caja", "Caja (centro)", "snare", "center"),
    ("caja-aro", "Caja: golpe de aro (rimshot)", "snare", "rimshot"),
    ("caja-fantasma", "Caja: nota fantasma (pp, centro)", "snare", "center"),
    ("charles-cerrado", "Charles cerrado", "hihat", "closed"),
    ("charles-abierto", "Charles abierto", "hihat", "open"),
    ("charles-pedal", "Charles con el pie", "hihat", "pedal"),
    ("tom-aire", "Tom de aire", "racktom", "center"),
    ("tom-piso", "Tom de piso", "floortom", "center"),
    ("ride-arco", "Ride: arco", "ride", "bow"),
    ("ride-campana", "Ride: campana", "ride", "bell"),
    ("crash", "Crash", "crash", "edge"),
]
THIRDS = np.array([25 * 2 ** (k / 3) for k in range(30)])  # 25 Hz … 16 kHz
OCT = [63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000]
COL = {"studio5": "#2a78d6", "studio6": "#0b3d91", "jazz5": "#eb6834", "jazz6": "#a33a00"}


def rows_of(path):
    return list(csv.DictReader(open(path, encoding="utf-8")))


def pick(rows, kind, art):
    """The rows of one articulation of the first piece of `kind` (a hat's openness by name or
    by its CC4)."""
    rs = [r for r in rows if r["kind"] == kind]
    if not rs:
        return []
    piece = rs[0]["piece"]
    rs = [r for r in rs if r["piece"] == piece]
    if kind == "hihat" and art in ("closed", "open"):
        st = [r for r in rs if r["art"] != "pedal"]
        if not st:
            return []
        opens = sorted({float(r["open"]) for r in st})
        want = opens[0] if art == "closed" else opens[-1]
        # a kit with named closed/open articulations
        named = [r for r in st if r["art"] == art]
        return named or [r for r in st if float(r["open"]) == want]
    out = [r for r in rs if r["art"] == art]
    if not out and kind == "kick":
        out = [r for r in rs if r["art"] == rs[0]["art"]]
    return out


class Render:
    def __init__(self, path):
        self.path = Path(path)
        self.info = sf.info(str(path))
        self.fs = self.info.samplerate
        self.rows = rows_of(self.path.with_suffix(".csv"))

    def hit(self, r, secs):
        a = int(r["start"]) - int(0.01 * self.fs)
        x, _ = sf.read(str(self.path), start=max(0, a), frames=int((secs + 0.01) * self.fs),
                       dtype="float64", always_2d=True)
        m = x.mean(axis=1)
        i0 = onset(m, self.fs)
        return x[i0:], m[i0:]


def rms_db(x):
    return 20 * np.log10(np.sqrt(np.mean(x ** 2)) + 1e-20)


def thirds(m, fs):
    n = 1 << int(np.ceil(np.log2(max(len(m), 2048))))
    X = np.abs(np.fft.rfft(m * np.hanning(len(m)), n)) ** 2
    f = np.fft.rfftfreq(n, 1 / fs)
    out = []
    for c in THIRDS:
        k = (f >= c * 2 ** (-1 / 6)) & (f < c * 2 ** (1 / 6))
        out.append(10 * np.log10(X[k].sum() + 1e-30))
    out = np.array(out)
    return out - 10 * np.log10(np.sum(10 ** (out / 10)))


def band(m, fs, c):
    sos = signal.butter(4, [c / 2 ** 0.5, min(c * 2 ** 0.5, 0.45 * fs)], btype="band", fs=fs, output="sos")
    return signal.sosfilt(sos, m)


def t30(y, fs):
    e = y ** 2
    if e.sum() <= 0:
        return np.nan
    edc = np.cumsum(e[::-1])[::-1]
    edc = 10 * np.log10(edc / edc[0] + 1e-30)
    t = np.arange(len(edc)) / fs
    i5, i25 = np.argmax(edc <= -5), np.argmax(edc <= -25)
    if edc[-1] > -25 or i25 <= i5:
        return np.nan
    return float((t[i25] - t[i5]) * 3.0 * 1000)  # T20 → T60, ms


def env_db(y, fs, ms=100):
    h = max(1, int(0.002 * fs))
    a = np.abs(signal.hilbert(y[: int((ms + 20) / 1000 * fs)]))
    e = np.convolve(a, np.ones(h) / h, mode="same")[: int(ms / 1000 * fs)]
    return 20 * np.log10(e + 1e-12)


def peaks(m, fs, kind):
    a, b = (0.02, 0.3) if kind in ("kick", "snare", "racktom", "floortom") else (0.05, 0.6)
    y = m[int(a * fs):int(b * fs)]
    N = 1 << 16
    X = 20 * np.log10(np.abs(np.fft.rfft(y * np.hanning(len(y)), N)) + 1e-12)
    f = np.fft.rfftfreq(N, 1 / fs)
    lo, hi = (30, 2000) if kind in ("kick", "snare", "racktom", "floortom") else (200, 16000)
    k = (f > lo) & (f < hi)
    base = signal.medfilt(X, 801)
    pk, _ = signal.find_peaks(np.where(k, X - base, -300), height=8, distance=int(6 / (fs / N)))
    pk = sorted(pk, key=lambda i: -X[i])[:12]
    return f, X, sorted((float(f[i]), float(X[i] - X.max())) for i in pk)


def spectro(ax, m, fs, secs, nfft, hop, vmin=-110, vmax=-10):
    y = m[: int(secs * fs)]
    f, t, S = signal.stft(y, fs, nperseg=nfft, noverlap=nfft - hop, window="hann", boundary=None)
    S = 20 * np.log10(np.abs(S) * 2 + 1e-12)
    ax.pcolormesh(t * (1000 if secs < 0.5 else 1), f, S, shading="auto", cmap="magma", vmin=vmin, vmax=vmax)
    ax.set_yscale("log")
    ax.set_ylim(20, 20000)


def main():
    src, out = Path(sys.argv[1]), Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    R = {k: Render(src / f"{k}.wav") for k, _ in KITS if (src / f"{k}.wav").exists()}
    names = dict(KITS)
    numbers = {}
    rng = np.random.default_rng(7)
    for pid, pname, kind, art in PIECES:
        d = out / pid
        d.mkdir(exist_ok=True)
        secs = 2.0 if kind in ("kick", "snare", "racktom", "floortom") else 3.5
        if pid == "charles-cerrado" or pid == "charles-pedal":
            secs = 1.5
        vels = [(24, "pp")] if pid == "caja-fantasma" else VELS
        have = {}
        for k, r in R.items():
            rs = pick(r.rows, kind, art)
            if rs:
                have[k] = rs
        if not have:
            continue
        num = {"kits": [names[k] for k in have], "clips": {}, "thirds": {}, "t30": {}, "partials": {}, "var": {}}
        # 1. clips at one loudness (the first 300 ms at −24 dBFS RMS)
        for v, vn in vels:
            for k, rs in have.items():
                rv = [r for r in rs if int(r["vel"]) == v]
                if not rv:
                    continue
                x, m = R[k].hit(rv[0], secs)
                g = -24 - rms_db(m[: int(0.3 * R[k].fs)])
                y = x * 10 ** (g / 20)
                y = y / max(1.0, np.abs(y).max() / 0.98)
                fn = f"{vn}-{k}.wav"
                yy = np.clip(y + rng.uniform(-0.5, 0.5, y.shape) / 32768, -1, 1)
                sf.write(str(d / fn), (yy * 32767).astype(np.int16), R[k].fs, subtype="PCM_16")
                num["clips"].setdefault(vn, []).append((names[k], f"{pid}/{fn}"))
        vmain = 24 if pid == "caja-fantasma" else 120
        hits = {}
        for k, rs in have.items():
            rv = [r for r in rs if int(r["vel"]) == vmain]
            if rv:
                hits[k] = R[k].hit(rv[0], secs)[1]
        ks = list(hits)
        fs = 48000
        # 2. spectrograms
        fig, axs = plt.subplots(len(ks), 2, figsize=(13, 1.7 * len(ks)), squeeze=False)
        for i, k in enumerate(ks):
            m = hits[k] * 10 ** ((-24 - rms_db(hits[k][: int(0.3 * fs)])) / 20)
            spectro(axs[i, 0], m, fs, 1.5, 2048, 256)
            spectro(axs[i, 1], m, fs, 0.1, 256, 16)
            axs[i, 0].set_ylabel(names[k], fontsize=8)
            for a in axs[i]:
                a.tick_params(labelsize=7)
        axs[0, 0].set_title("0–1,5 s (s)", fontsize=9)
        axs[0, 1].set_title("0–100 ms (ms)", fontsize=9)
        plt.tight_layout()
        plt.savefig(d / "espectrograma.png", dpi=72)
        plt.close()
        # 3. third-octave spectrum (the first 1.5 s), the recordings' spread shaded
        sp = {k: thirds(hits[k][: int(1.5 * fs)], fs) for k in ks}
        refk = [k for k, _ in REFS if k in sp]
        fig, ax = plt.subplots(figsize=(11, 4))
        if refk:
            lo = np.min([sp[k] for k in refk], axis=0)
            hi = np.max([sp[k] for k in refk], axis=0)
            ax.fill_between(THIRDS, lo, hi, color="#bbbbbb", alpha=0.6, label="grabadas (rango)")
        for k in ks:
            if k in COL:
                ax.semilogx(THIRDS, sp[k], color=COL[k], lw=1.6, label=names[k])
                if refk:
                    over = np.where(sp[k] > hi, sp[k] - hi, np.where(sp[k] < lo, sp[k] - lo, 0.0))
                    num["thirds"][names[k]] = [(float(c), round(float(o), 1)) for c, o in zip(THIRDS, over) if abs(o) >= 3]
            else:
                ax.semilogx(THIRDS, sp[k], color="#777777", lw=0.7, ls="--")
        ax.set_xlim(25, 16000)
        ax.set_ylim(-60, 0)
        ax.grid(alpha=0.3, which="both")
        ax.set_ylabel("dB (re el total)")
        ax.legend(fontsize=7, ncol=3)
        plt.tight_layout()
        plt.savefig(d / "tercios.png", dpi=72)
        plt.close()
        # 4. decay per octave and the first 100 ms per band
        fig, axs = plt.subplots(1, 5, figsize=(15, 3.2))
        for k in ks:
            tt = [t30(band(hits[k], fs, c), fs) for c in OCT]
            num["t30"][names[k]] = [None if not np.isfinite(v) else round(v) for v in tt]
            st = dict(color=COL.get(k, "#777777"), lw=1.6 if k in COL else 0.8, ls="-" if k in COL else "--")
            axs[0].semilogx(OCT, tt, marker="o", ms=3, **st)
            for j, c in enumerate([125, 500, 2000, 8000]):
                e = env_db(band(hits[k], fs, c), fs)
                ref = env_db(band(hits[k], fs, c), fs).max()
                axs[j + 1].plot(np.arange(len(e)) / fs * 1000, e - ref, **st)
                axs[j + 1].set_title(f"{c} Hz, primeros 100 ms", fontsize=8)
                axs[j + 1].set_ylim(-40, 3)
        axs[0].set_title("Caída T60 por octava (ms)", fontsize=8)
        axs[0].set_yscale("log")
        for a in axs:
            a.grid(alpha=0.3)
            a.tick_params(labelsize=7)
        plt.tight_layout()
        plt.savefig(d / "caida.png", dpi=72)
        plt.close()
        # 5. partials
        fig, axs = plt.subplots(len(ks), 1, figsize=(11, 1.4 * len(ks)), squeeze=False)
        for i, k in enumerate(ks):
            f, X, pk = peaks(hits[k], fs, kind)
            num["partials"][names[k]] = [(round(a), round(b, 1)) for a, b in pk]
            a = axs[i, 0]
            a.semilogx(f[1:], X[1:] - X.max(), color=COL.get(k, "#555555"), lw=0.6)
            for fr, lv in pk:
                a.axvline(fr, color="#cc0000", lw=0.4, alpha=0.5)
            a.set_xlim(30, 16000)
            a.set_ylim(-70, 2)
            a.set_ylabel(names[k], fontsize=7)
            a.tick_params(labelsize=7)
        plt.tight_layout()
        plt.savefig(d / "parciales.png", dpi=72)
        plt.close()
        # 6. stroke to stroke: 8 takes at mf (pp for a ghost note)
        vv = 24 if pid == "caja-fantasma" else 72
        fig, axs = plt.subplots(2, (len(ks) + 1) // 2, figsize=(15, 5.5), squeeze=False)
        for i, k in enumerate(ks):
            rv = [r for r in have[k] if int(r["vel"]) == vv][:8]
            specs, lv = [], []
            for r in rv:
                m = R[k].hit(r, 0.25)[1][: int(0.2 * fs)]
                specs.append(thirds(m, fs) + rms_db(m))
                lv.append(rms_db(m[: int(0.08 * fs)]))
            if len(specs) < 2:
                continue
            S = np.array(specs)
            S0 = S - S.max(axis=1, keepdims=True)
            S0 = np.maximum(S0, -60)
            dv = [np.sqrt(np.mean((S0[a, 8:] - S0[b, 8:]) ** 2)) for a in range(len(S0)) for b in range(a + 1, len(S0))]
            num["var"][names[k]] = {"espectro_db": round(float(np.median(dv)), 2), "nivel_db": round(float(np.std(lv)), 2),
                                    "n": len(S)}
            a = axs[i // axs.shape[1], i % axs.shape[1]]
            for s in S:
                a.semilogx(THIRDS, s - S.mean(axis=0).max(), color=COL.get(k, "#555555"), lw=0.6, alpha=0.8)
            a.set_title(f"{names[k]}: {np.median(dv):.1f} dB espectro, {np.std(lv):.1f} dB nivel", fontsize=8)
            a.set_xlim(60, 16000)
            a.set_ylim(-60, 5)
            a.tick_params(labelsize=7)
            a.grid(alpha=0.3)
        plt.tight_layout()
        plt.savefig(d / "variacion.png", dpi=72)
        plt.close()
        # 7. the snare's microphones
        if pid == "caja":
            fig, axs = plt.subplots(1, 3, figsize=(15, 3.4))
            img = {}
            for k in ks:
                lv = {}
                for j, mic in enumerate(["close", "oh", "room"]):
                    p = src / f"{k}-{mic}.wav"
                    if not p.exists():
                        continue
                    rr = Render(p)
                    rv = [r for r in pick(rr.rows, "snare", "center") if int(r["vel"]) == 72]
                    if not rv:
                        continue
                    x, m = rr.hit(rv[0], 1.0)
                    lv[mic] = round(float(rms_db(m[: int(0.3 * fs)])), 1)
                    axs[j].semilogx(THIRDS, thirds(m, fs), color=COL.get(k, "#777777"),
                                    lw=1.5 if k in COL else 0.8, ls="-" if k in COL else "--", label=names[k])
                    axs[j].set_title({"close": "cercano", "oh": "aéreos", "room": "sala"}[mic], fontsize=9)
                if lv:
                    img[names[k]] = lv
            for a in axs:
                a.set_xlim(40, 16000)
                a.set_ylim(-50, 0)
                a.grid(alpha=0.3)
                if a.get_legend_handles_labels()[0]:
                    a.legend(fontsize=6)
            plt.tight_layout()
            plt.savefig(d / "micros.png", dpi=72)
            plt.close()
            num["mics"] = img
        numbers[pid] = num
        print(pid, "done", flush=True)
    (out / "numbers.json").write_text(json.dumps(numbers, indent=1, ensure_ascii=False), encoding="utf-8")


if __name__ == "__main__":
    main()
