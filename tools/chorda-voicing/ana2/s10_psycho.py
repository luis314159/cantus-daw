"""Listen-proxies per phrase (loudness-matched previews): Zwicker-style specific loudness on 24
Bark bands (simplified, not a certified ISO 532-1), sharpness (DIN 45692 weighting),
and modulation-based proxies for roughness (20–150 Hz envelope modulation) and fluctuation
strength (1–12 Hz, the beating / pulsing range). Playback assumed: -20 LUFS = 72 dB SPL."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np, csv, os
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from common import *

CAL = 92.0  # dB SPL for a 0-dBFS rms


def bark(f):
    return 26.81 * f / (1960 + f) - 0.53


def tq(f):
    f = np.maximum(f, 20) / 1000
    return 3.64 * f ** -0.8 - 6.5 * np.exp(-0.6 * (f - 3.3) ** 2) + 1e-3 * f ** 4


edges = np.arange(0, 25)  # Bark band edges 0..24


def specific_loudness(seg):
    """seg: mono frame. Returns N'(z) for 24 bands (sone/Bark)."""
    n = len(seg)
    F = np.abs(np.fft.rfft(seg * np.hanning(n))) ** 2 / (np.sum(np.hanning(n) ** 2) * n / 2)
    f = np.fft.rfftfreq(n, 1 / SR)
    z = bark(f)
    Nz = np.zeros(24)
    for i in range(24):
        m = (z >= edges[i]) & (z < edges[i + 1]) & (f > 20)
        if not m.any():
            continue
        E = F[m].sum()
        L = 10 * np.log10(E + 1e-30) + CAL
        fc = f[m].mean()
        LTQ = tq(fc)
        e, etq = 10 ** (L / 10), 10 ** (LTQ / 10)
        Nz[i] = max(0.0, 0.08 * etq ** 0.23 * ((0.5 + 0.5 * e / etq) ** 0.23 - 1))
    return Nz


zc = edges[:-1] + 0.5
gz = np.where(zc < 15.8, 1.0, 0.066 * np.exp(0.171 * zc))


def bark_bands_signal(m):
    """Band-split the mono signal into 24 Bark bands (Butterworth), for modulation analysis."""
    out = []
    for i in range(24):
        # invert bark: f = 1960 (z+0.53)/(26.28 - z)
        lo = 1960 * (edges[i] + 0.53) / (26.28 - edges[i])
        hi = 1960 * (edges[i + 1] + 0.53) / (26.28 - edges[i + 1])
        lo, hi = max(lo, 25), min(hi, 20000)
        sos = signal.butter(3, [lo, hi], 'bandpass', fs=SR, output='sos')
        out.append(signal.sosfilt(sos, m))
    return out


def mod_index(env, lo, hi):
    dc = env.mean()
    sos = signal.butter(2, [lo, hi], 'bandpass', fs=SR / 48, output='sos')
    ac = signal.sosfiltfilt(sos, env)
    return np.sqrt(2) * ac.std() / (dc + 1e-20)


rows = []
for ph in PHRASES:
    for s in ['b14', 'c5', 'sal']:
        x = phrase(ph, s)
        m = x.mean(1)
        hop = int(0.05 * SR); w = int(0.1 * SR)
        Ns, Ss = [], []
        for i in range(0, len(m) - w, hop):
            Nz = specific_loudness(m[i:i + w])
            N = Nz.sum()
            Ns.append(N)
            Ss.append(0.11 * np.sum(Nz * gz * zc) / N if N > 0.05 else np.nan)
        Ns = np.array(Ns); Ss = np.array(Ss)
        act = Ns > 0.1 * Ns.max()
        # modulation proxies over the sounding part
        bands = bark_bands_signal(m)
        rough, fluct = 0.0, 0.0
        Nmean = np.array([specific_loudness(m[i:i + w]) for i in range(0, len(m) - w, int(0.5 * SR))]).mean(0)
        for i, b in enumerate(bands):
            env = np.abs(signal.hilbert(b))[::48]  # 1 kHz envelope rate
            k = act.repeat(int(0.05 * SR / 48))[:len(env)]
            e2 = env[:len(k)][k] if k.any() else env
            rough += mod_index(e2, 20, 150) * Nmean[i]
            fluct += mod_index(e2, 1, 12) * Nmean[i]
        r = dict(phrase=ph, src=s, N5=round(np.percentile(Ns, 95), 2), Nmean=round(Ns[act].mean(), 2),
                 S_mean=round(np.nanmean(Ss[act]), 3), S_max=round(np.nanpercentile(Ss[act], 95), 3),
                 rough_proxy=round(rough, 3), fluct_proxy=round(fluct, 3))
        rows.append(r)
        print(r, flush=True)
with open(os.path.join(CSV, 'psycho.csv'), 'w', newline='') as fh:
    wr = csv.DictWriter(fh, rows[0].keys()); wr.writeheader(); wr.writerows(rows)

fig, ax = plt.subplots(1, 4, figsize=(16, 4))
for j, col in enumerate(['Nmean', 'S_mean', 'rough_proxy', 'fluct_proxy']):
    for k, s in enumerate(['b14', 'c5', 'sal']):
        v = [r[col] for r in rows if r['src'] == s]
        ax[j].bar(np.arange(len(PHRASES)) + (k - 1) * 0.27, v, 0.27, color=COL[s], label=NAME[s])
    ax[j].set_xticks(np.arange(len(PHRASES))); ax[j].set_xticklabels(PHRASES, rotation=45, fontsize=8)
    ax[j].set_title({'Nmean': 'Loudness (sone, sounding part)', 'S_mean': 'Sharpness (acum)',
                     'rough_proxy': 'Roughness proxy (20–150 Hz mod.)', 'fluct_proxy': 'Fluctuation proxy (1–12 Hz mod.)'}[col], fontsize=9)
    ax[j].grid(alpha=.3, axis='y')
ax[0].legend(fontsize=7)
fig.tight_layout(); fig.savefig(os.path.join(FIG, 'psychoacoustic.png'), dpi=100); plt.close(fig)
