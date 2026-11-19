"""Single notes, A0..C8 every minor third at pp/mf/ff: partial frequencies and inharmonicity,
per-partial envelopes (level, prompt/aftersound decay, beating), band decays, attack, release,
stereo, noise floor. Writes csv/notes_*.csv and a cache (npz) for the plots."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana'))
import numpy as np, csv, os, pickle
from common import *

SRCS = sys.argv[1:] or ['sal', 'c2', 'b14']
SR = 48000
NENV = 1024  # envelope samples per 6-s slot (~5.9 ms)
TE = np.arange(NENV) * EVERY / NENV


def find_peak(F, mag, fc, half):
    m = (F > fc - half) & (F < fc + half)
    if not m.any():
        return None, -300
    idx = np.where(m)[0]
    i = idx[np.argmax(mag[idx])]
    if 0 < i < len(mag) - 1:
        a, b, c = 20 * np.log10(mag[i - 1:i + 2] + 1e-20)
        p = 0.5 * (a - c) / (a - 2 * b + c) if (a - 2 * b + c) != 0 else 0
        return F[i] + p * (F[1] - F[0]), b - 0.25 * (a - c) * p
    return F[i], 20 * np.log10(mag[i] + 1e-20)


def partials(m, key):
    """Frequencies of partials 1..N from 0.05–1.55 s, iteratively fitting f0 and B."""
    seg = m[int(0.05 * SR):int(1.55 * SR)]
    w = np.hanning(len(seg))
    nfft = 1 << 20
    M = np.abs(np.fft.rfft(seg * w, nfft))
    F = np.fft.rfftfreq(nfft, 1 / SR)
    top = 20 * np.log10(M.max())
    f0n = kfreq(key)
    f0, B = f0n, 0.0
    found = {}
    nmax = int(min(18000, 0.45 * SR) / f0n)
    nmax = min(nmax, 80)
    for n in range(1, nmax + 1):
        pred = n * f0 * np.sqrt(1 + B * n * n)
        if pred > 16000:
            break
        f, a = find_peak(F, M, pred, 0.25 * f0n)
        if f is None or a < top - 75:
            continue
        found[n] = (f, a - top)
        ns = np.array(sorted(found))
        if len(ns) >= 3:
            fs = np.array([found[k][0] for k in ns])
            y = (fs / ns) ** 2
            A = np.stack([np.ones(len(ns)), ns ** 2], 1)
            c, *_ = np.linalg.lstsq(A, y, rcond=None)
            if c[0] > 0:
                f0, B = np.sqrt(c[0]), max(c[1] / c[0], 0.0)
        elif len(ns) >= 1:
            f0 = found[ns[0]][0] / ns[0]
    return f0, B, found, (F, M, top)


def envelope(X, Fx, f, half):
    """Amplitude envelope (dB) of the band f±half, by frequency-domain demodulation."""
    m = (Fx > f - half) & (Fx < f + half)
    idx = np.where(m)[0]
    if len(idx) < 4:
        return np.full(NENV, -300.0)
    seg = X[idx] * np.hanning(len(idx))
    y = np.fft.ifft(seg, NENV) * NENV / (len(X) - 1)
    return 20 * np.log10(np.abs(y) + 1e-12)


def slope(t, e, t0, t1):
    m = (t >= t0) & (t < t1) & (e > -250)
    if m.sum() < 4:
        return np.nan, np.nan
    a = np.polyfit(t[m], e[m], 1)
    res = e[m] - np.polyval(a, t[m])
    return a[0], res.std()


OCT = [63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000]


def analyse(src, dyn, key):
    x, sr = note(src, dyn, key)
    x = x.copy()
    m = x.mean(1)
    i0 = onset(x, sr, -50)
    r = dict(src=src, dyn=dyn, vel=DYN[dyn], key=key, name=kname(key))
    r['onset_ms'] = i0 / sr * 1000
    # shift so the onset sits at t = 0.01 s (keeps every slot comparable)
    sh = i0 - int(0.01 * sr)
    if sh > 0:
        x = np.concatenate([x[sh:], np.zeros((sh, 2))])
    elif sh < 0:
        x = np.concatenate([np.zeros((-sh, 2)), x[:sh]])
    m = x.mean(1)
    held = x[:int(HELD * sr)]
    r['peak_db'] = 20 * np.log10(np.abs(x).max())
    r['lufs1s'] = lufs(x[:int(1.0 * sr)] if len(x) > sr else x)
    r['rms_held_db'] = 10 * np.log10((held ** 2).mean())
    r['rms_50ms_db'] = 10 * np.log10((x[:int(0.06 * sr)] ** 2).mean())
    # rise time 10→90 % of the 1-ms envelope peak
    e = np.sqrt(np.convolve(m ** 2, np.ones(48) / 48, 'same'))
    ip = np.argmax(e[:int(0.3 * sr)])
    r['rise_ms'] = (ip - np.argmax(e > 0.1 * e[ip])) / sr * 1000
    # spectra of windows
    def tspec(a, b):
        f, p = psd(m[int(a * sr):int(b * sr)], sr, nper=2048)
        return f, p
    for nm, (a_, b_) in (('att', (0.0, 0.05)), ('early', (0.06, 0.31)), ('late', (1.5, 3.4))):
        f, p = tspec(a_, b_)
        r[f'centroid_{nm}'] = np.sum(f * p) / np.sum(p)
        bo = bands(f, p, np.array(OCT, float), 1)
        tot = 10 * np.log10(p.sum())
        for c, v in zip(OCT, bo):
            r[f'{nm}_{c}'] = v - tot
    f0n = kfreq(key)
    # knock: energy below 0.7·f0 (bass: below 60 Hz excluded) in the first 60 ms vs total
    f, pk = psd(m[int(0.0):int(0.07 * sr)], sr, nper=2048)
    lim = 0.7 * f0n if f0n > 60 else None
    r['thump_db'] = 10 * np.log10(pk[(f >= 20) & (f < lim)].sum() / pk.sum()) if lim else np.nan
    # band decays: octave-band envelopes
    X = np.fft.rfft(m)
    Fx = np.fft.rfftfreq(len(m), 1 / sr)
    for c in OCT[:-1]:
        lo, hi = c / np.sqrt(2), c * np.sqrt(2)
        mm = (Fx >= lo) & (Fx < hi)
        y = np.fft.irfft(np.where(mm, X, 0), len(m))
        env = 10 * np.log10(np.convolve(y ** 2, np.ones(2400) / 2400, 'same')[::480] + 1e-30)
        te = np.arange(len(env)) * 0.01
        r[f'dec_early_{c}'], _ = slope(te, env, 0.1, 0.6)
        r[f'dec_late_{c}'], _ = slope(te, env, 1.5, 3.4)
        pre = env[(te > 3.3) & (te < 3.5)].mean()
        r[f'rel_drop100_{c}'] = pre - env[(te > 3.58) & (te < 3.62)].mean()
        r[f'rel_drop500_{c}'] = pre - env[(te > 3.98) & (te < 4.02)].mean()
    # overall release: level 3.3–3.5 s vs windows after key-up (3.5 s + 10 ms offset)
    en = 10 * np.log10(np.convolve(m ** 2, np.ones(480) / 480, 'same')[::240] + 1e-30)
    tn = np.arange(len(en)) * 0.005
    pre = en[(tn > 3.3) & (tn < 3.5)].mean()
    for d in (0.05, 0.1, 0.25, 0.5, 1.0, 2.0):
        r[f'rel_{int(d * 1000)}ms_db'] = en[(tn > 3.5 + d - 0.01) & (tn < 3.5 + d + 0.01)].mean() - pre
    # release noise: 1–8 kHz energy in 3.5–3.65 s vs 3.3–3.5 s
    def bandE(a, b, lo, hi):
        f2, p2 = psd(m[int(a * sr):int(b * sr)], sr, nper=2048)
        return 10 * np.log10(p2[(f2 >= lo) & (f2 < hi)].sum() + 1e-30)
    r['rel_noise_hf_db'] = bandE(3.5, 3.65, 1000, 8000) - bandE(3.3, 3.5, 1000, 8000)
    r['rel_noise_lf_db'] = bandE(3.5, 3.65, 20, 200) - bandE(3.3, 3.5, 20, 200)
    # floor: 5.6–5.95 s, relative to peak
    tail = x[int(5.6 * sr):int(5.95 * sr)]
    r['floor_rel_db'] = 10 * np.log10((tail ** 2).mean() + 1e-30) - r['peak_db']
    # stereo
    L, R = held[:, 0], held[:, 1]
    r['corr'] = np.sum(L * R) / np.sqrt(np.sum(L * L) * np.sum(R * R))
    r['balance_db'] = 10 * np.log10(np.sum(L * L) / np.sum(R * R))
    s = (L - R) / 2
    r['side_db'] = 10 * np.log10(np.sum(s * s) / np.sum(((L + R) / 2) ** 2))
    r['dc_db'] = 20 * np.log10(abs(m.mean()) + 1e-15) - r['peak_db']
    # partials
    f0, B, found, spec = partials(m, key)
    r['f0'], r['B'] = f0, B
    r['cents_vs_et'] = 1200 * np.log2(f0 / f0n)
    r['npart'] = len(found)
    parts = []
    for n, (fn, an) in sorted(found.items()):
        half = min(0.4 * f0n, 12.0) if f0n < 200 else min(0.4 * f0n, 25.0)
        env = envelope(X, Fx, fn, half)
        lvl0 = env[(TE > 0.03) & (TE < 0.13)].mean()
        pr, _ = slope(TE, env, 0.05, 0.5)
        lt, _ = slope(TE, env, 1.5, 3.4)
        _, rough = slope(TE, env, 0.3, 3.4)
        rel, _ = slope(TE, env, 3.53, 3.75)
        parts.append(dict(src=src, dyn=dyn, key=key, n=n, f=fn, ratio=fn / (n * f0), amp_spec=an,
                          lvl_db=lvl0, prompt_dbs=pr, late_dbs=lt, beat_rms_db=rough, rel_dbs=rel))
    return r, parts, (f0, B, found, spec)


if __name__ == '__main__':
    allr, allp = [], []
    cache = {}
    # Round 40 (the builder's copy): the reference's and beta.14's rows never change - kept
    # once in a cache beside these scripts, only the candidate is analysed each try.
    fixed = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'fixed_rows.pkl')
    done = set()
    if os.path.exists(fixed):
        fr, fp, fc = pickle.load(open(fixed, 'rb'))
        allr.extend(fr); allp.extend(fp); cache.update(fc)
        done = {'sal', 'b14'}
    for src in SRCS:
        if src in done:
            continue
        for dyn in DYN:
            for key in KEYS:
                r, p, sp = analyse(src, dyn, key)
                allr.append(r); allp.extend(p)
                if dyn == 'ff' and key <= 36:
                    F, M, top = sp[3]
                    mm = F < 3000
                    cache[(src, key)] = (F[mm][::4], (20 * np.log10(M[mm] + 1e-20) - top)[::4], sp[0], sp[1], sp[2])
            print(src, 'done', flush=True)
    if not done:
        keep = ('sal', 'b14')
        pickle.dump(([r for r in allr if r['src'] in keep], [p for p in allp if p['src'] in keep],
                     {k: v for k, v in cache.items() if k[0] in keep}), open(fixed, 'wb'))
    for name, rows in (('notes_features.csv', allr), ('notes_partials.csv', allp)):
        with open(os.path.join(CSV, name), 'w', newline='') as fh:
            w = csv.DictWriter(fh, fieldnames=list(rows[0].keys()))
            w.writeheader()
            for row in rows:
                w.writerow({k: (round(v, 5) if isinstance(v, float) else v) for k, v in row.items()})
    with open(os.path.join(CSV, 'bass_spectra.pkl'), 'wb') as fh:
        pickle.dump(cache, fh)
