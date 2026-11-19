"""Per-note measures on the single-note renders (A0..C8 every minor third, pp/mf/ff), Chorda
candidate 5 vs the Salamander. Writes csv/notes.csv and csv/partials.csv."""
import os as _os, sys as _sys  # noqa: E401
_sys.path[:0] = [_os.path.dirname(_os.path.abspath(__file__)), _os.path.normpath(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))]
from labpaths import *  # noqa: E402,F401,F403 (LAB, TMP, SHARE, PY, EXE, tool_path)
import sys; sys.path.insert(0, tool_path('ana2'))
import numpy as np, csv, os
from common import *
from partials import find_partials, hetero_env, slope


def find_onset(x):
    a = np.abs(x).max(1)
    pk = a[:int(1.0 * SR)].max()
    i = int(np.argmax(a > pk * 10 ** (-30 / 20)))
    # walk back to where a 0.5 ms envelope sits under -55 dB of peak
    env = np.convolve(a, np.ones(24) / 24, 'same')
    j = i
    while j > 0 and env[j] > pk * 10 ** (-55 / 20) and i - j < int(0.02 * SR):
        j -= 1
    return j, i


def env_ms(m2, win=0.001):
    w = int(win * SR)
    c = np.concatenate([[0], np.cumsum(m2)])
    e = (c[w:] - c[:-w]) / w
    return e


def centroid(seg):
    m = seg.mean(1)
    f, P = signal.periodogram(m * signal.windows.hann(len(m)), SR)
    s = (f > 20) & (f < 20000)
    return float((f[s] * P[s]).sum() / P[s].sum())


def band_energy(seg, lo, hi):
    f, P = signal.periodogram(seg.mean(1) * signal.windows.hann(len(seg)), SR)
    s = (f >= lo) & (f < hi)
    return P[s].sum()


rows, prow = [], []
for src in ['c5', 'sal']:
    for dyn in ['pp', 'mf', 'ff']:
        for key in KEYS:
            x = note(src, dyn, key)
            j, i = find_onset(x)
            on = j
            y = x[on:]
            m = y.mean(1)
            f0n = kfreq(key)
            r = dict(src=src, dyn=dyn, key=key, name=kname(key))
            r['pre_onset_ms'] = round((i - j) / SR * 1000, 2)
            pre = x[max(0, on - int(0.25 * SR)):max(1, on - int(0.005 * SR))]
            r['pre_noise_dbfs'] = round(db(np.mean(pre ** 2) + 1e-30), 1) if len(pre) > 100 else np.nan
            # level: K-weighted energy over the first second
            kw = kweight(y[:int(1.0 * SR)])
            r['level_1s'] = round(-0.691 + 10 * np.log10(np.mean(kw ** 2, 0).sum() + 1e-30), 2)
            r['peak_dbfs'] = round(db20(np.abs(y[:SR]).max()), 2)
            # attack: 1-ms envelope
            e = env_ms((y[:int(0.3 * SR)] ** 2).sum(1))
            pk = e.max(); ipk = int(np.argmax(e))
            a40 = int(np.argmax(e > pk * 1e-4)); a20 = int(np.argmax(e > pk * 1e-2)); a3 = int(np.argmax(e > pk * 10 ** (-0.3)))
            r['rise_40_3_ms'] = round((a3 - a40) / SR * 1000, 2)
            r['rise_20_3_ms'] = round((a3 - a20) / SR * 1000, 2)
            r['t_peak_ms'] = round(ipk / SR * 1000, 1)
            r['cent_0_50'] = round(centroid(y[:int(0.05 * SR)]))
            r['cent_300_1000'] = round(centroid(y[int(0.3 * SR):int(1.0 * SR)]))
            # high band in the attack relative to the body
            ea = band_energy(y[:int(0.03 * SR)], 2000, 10000) / max(band_energy(y[:int(0.03 * SR)], 20, 20000), 1e-30)
            r['att_2k10k_frac_db'] = round(db(ea), 1)
            # thump under the fundamental in the first 60 ms
            if f0n > 120:
                th = band_energy(y[:int(0.06 * SR)], 20, 0.7 * f0n) / band_energy(y[:int(0.06 * SR)], 20, 20000)
                r['thump_db'] = round(db(th), 1)
            else:
                r['thump_db'] = np.nan
            # partials
            f0, B, pl = find_partials(m, key)
            r['f0_fit'] = round(f0, 3); r['B'] = B
            r['cents_f0'] = round(1200 * np.log2(f0 / f0n), 2)
            p1 = [p for p in pl if p[0] == 1]
            r['cents_p1'] = round(1200 * np.log2(p1[0][1] / f0n), 2) if p1 else np.nan
            r['n_partials'] = len(pl)
            # inter-partial (non-tonal) energy: attack window and sustain window
            for nm, a, b in [('nt_att', 0.0, 0.04), ('nt_sus', 0.5, 1.5)]:
                seg = m[int(a * SR):int(b * SR)]
                N = 1 << int(np.ceil(np.log2(len(seg) * 2)))
                F = np.abs(np.fft.rfft(seg * signal.windows.hann(len(seg)), N)) ** 2
                ff = np.fft.rfftfreq(N, 1 / SR)
                mask = np.zeros_like(ff, bool)
                bw = max(2.5 * SR / len(seg), 3.0)
                nmax = int(16000 / f0) + 1
                for n in range(1, nmax):
                    fc = n * f0 * np.sqrt(1 + B * n * n)
                    mask |= np.abs(ff - fc) < bw
                band = (ff > 40) & (ff < 16000)
                r[nm + '_db'] = round(db(F[band & ~mask].sum() / F[band].sum()), 1)
            # release: key up at onset + HELD
            ku = int(HELD * SR)
            e10 = env_ms((y ** 2).sum(1), 0.01)
            ref = e10[ku - int(0.06 * SR):ku - int(0.01 * SR)].mean()
            for ms in [50, 100, 250, 500, 1000]:
                k2 = ku + int(ms / 1000 * SR)
                r[f'rel_{ms}'] = round(db(e10[k2] / ref), 1) if k2 < len(e10) else np.nan
            hb = bandpass(y[ku - int(0.2 * SR):ku + int(0.3 * SR)], 1000, 8000)
            eh = env_ms((hb ** 2).sum(1), 0.005)
            pre_h = eh[int(0.1 * SR):int(0.19 * SR)].mean()
            r['rel_hf_burst_db'] = round(db(eh[int(0.2 * SR):int(0.3 * SR)].max() / pre_h), 1)
            lb = bandpass(y[ku - int(0.2 * SR):ku + int(0.3 * SR)], 20, 150)
            el = env_ms((lb ** 2).sum(1), 0.005)
            r['rel_lf_burst_db'] = round(db(el[int(0.2 * SR):int(0.3 * SR)].max() / el[int(0.1 * SR):int(0.19 * SR)].mean()), 1)
            tail = y[int(4.9 * SR):int(5.9 * SR)] if len(y) > int(5.9 * SR) else y[-int(0.5 * SR):]
            r['tail_dbfs'] = round(db(np.mean(tail ** 2) + 1e-30), 1)
            # stereo
            L, R = y[:, 0], y[:, 1]
            for nm, a, b in [('att', 0, 0.1), ('sus', 0.5, 2.0)]:
                l, rr = L[int(a * SR):int(b * SR)], R[int(a * SR):int(b * SR)]
                r['ild_' + nm] = round(db(np.mean(l ** 2) / np.mean(rr ** 2)), 2)
                r['corr_' + nm] = round(float(np.sum(l * rr) / np.sqrt(np.sum(l ** 2) * np.sum(rr ** 2))), 3)
                mm, ss = (l + rr) / 2, (l - rr) / 2
                r['side_' + nm] = round(db(np.mean(ss ** 2) / np.mean(mm ** 2)), 1)
            # ITD from the first 30 ms, high-passed at 150 Hz
            hp = bandpass(y[:int(0.03 * SR) + 200], 150, 12000)
            lag = np.arange(-96, 97)
            cc = [np.sum(hp[100:-100, 0] * np.roll(hp[:, 1], k)[100:-100]) for k in lag]
            k = lag[int(np.argmax(cc))]
            r['itd_ms'] = round(k / SR * 1000, 3)  # >0: right lags (left first)
            r['itd_cc'] = round(max(cc) / np.sqrt(np.sum(hp[100:-100, 0] ** 2) * np.sum(hp[100:-100, 1] ** 2)), 3)
            rows.append(r)
            # per partial (mf and ff only)
            if dyn in ('mf', 'ff'):
                for n, fn, pdb in pl[:24]:
                    if n > 20 or fn > 12000:
                        continue
                    bw = min(0.3 * f0, 25.0)
                    t, ev = hetero_env(m[:int(HELD * SR)], fn, bw)
                    top = ev[(t > 0.005) & (t < 0.3)].max()
                    ev = ev - top
                    p = dict(src=src, dyn=dyn, key=key, n=n, f=round(fn, 2), rel_db=round(pdb, 1))
                    p['dec_early'] = round(slope(t, ev, 0.05, 0.5), 2)
                    p['dec_mid'] = round(slope(t, ev, 0.5, 1.5), 2)
                    p['dec_late'] = round(slope(t, ev, 1.5, 3.3), 2)
                    p['lvl_3s'] = round(ev[np.searchsorted(t, 3.2)], 1)
                    mk = (t > 0.15) & (t < 3.3) & (ev > -90)
                    if mk.sum() > 100:
                        tt, vv = t[mk], ev[mk]
                        res = vv - np.polyval(np.polyfit(tt, vv, 3), tt)
                        p['beat_depth'] = round(np.percentile(res, 97) - np.percentile(res, 3), 1)
                        R_ = np.abs(np.fft.rfft(res * np.hanning(len(res)), 8192))
                        fr = np.fft.rfftfreq(8192, t[1] - t[0])
                        s = (fr > 0.3) & (fr < 12)
                        p['beat_hz'] = round(fr[s][np.argmax(R_[s])], 2)
                    else:
                        p['beat_depth'] = np.nan; p['beat_hz'] = np.nan
                    prow.append(p)
            print(src, dyn, kname(key), r['level_1s'], r['rise_40_3_ms'], r['cent_0_50'], r['B'], r['cents_p1'], flush=True)

def wcsv(name, rr):
    keys = list(rr[0].keys())
    for q in rr:
        for k in q:
            if k not in keys:
                keys.append(k)
    with open(os.path.join(CSV, name), 'w', newline='') as fh:
        w = csv.DictWriter(fh, keys); w.writeheader(); w.writerows(rr)

wcsv('notes.csv', rows)
wcsv('partials.csv', prow)
