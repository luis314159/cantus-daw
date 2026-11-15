"""The groove's fine highs (analysis-drums s06's measure), whole and per section.

    python -I groove_hf.py <groove.flac> [<groove.flac> ...]

Per file: 6-16 kHz median flatness (2048 frames) and effective components per kHz (the
participation ratio of 8192-point frames / 10), over the whole groove and over each bar
(100 BPM: 2.4 s a bar; bars 1-2 closed/loose hat, 3 half open, 4 open + fill, 5 the crash);
'white' the same after dividing each frame by the file's long-term spectrum (the tilt and
fixed colour removed: what is left is the fine structure).
"""
import sys
import numpy as np
import soundfile as sf
from scipy import signal

SR = 48000
BAR = 2.4


def flat(p, axis=0):
    p = np.maximum(p, 1e-30)
    return np.exp(np.mean(np.log(p), axis=axis)) / np.mean(p, axis=axis)


def measure(mid, white=False):
    f, t, S = signal.stft(mid, SR, nperseg=2048, noverlap=1024)
    P = np.abs(S) ** 2
    s = (f >= 6000) & (f < 16000)
    Ps = P[s]
    if white:
        Ps = Ps / (Ps.mean(1, keepdims=True) + 1e-30)
    fl = flat(Ps, 0)
    e = P[s].sum(0)
    ok = e > np.percentile(e, 30)
    f2, t2, S2 = signal.stft(mid, SR, nperseg=8192, noverlap=4096)
    P2 = np.abs(S2) ** 2
    s2 = (f2 >= 6000) & (f2 < 16000)
    Q = P2[s2]
    if white:
        Q = Q / (Q.mean(1, keepdims=True) + 1e-30)
    pr = Q.sum(0) ** 2 / (Q ** 2).sum(0) / 10
    e2 = P2[s2].sum(0)
    ok2 = e2 > np.percentile(e2, 30)
    return np.median(fl[ok]), np.median(pr[ok2])


def run(path):
    x, sr = sf.read(path, always_2d=True)
    assert sr == SR
    mid = x.mean(1)
    out = [path.replace("\\", "/").split("/")[-1][:34].ljust(34)]
    a, b = measure(mid)
    c, d = measure(mid, True)
    out.append(f"all {a:.3f} {b:5.1f} | white {c:.3f} {d:5.1f} ||")
    for k in range(5):
        seg = mid[int(k * BAR * SR):int((k + 1) * BAR * SR)]
        a, b = measure(seg)
        out.append(f"b{k + 1} {a:.2f} {b:4.1f}")
    return " ".join(out)


if __name__ == "__main__":
    print("file".ljust(34), "flat comps | whitened flat comps || per bar flat comps")
    for p in sys.argv[1:]:
        print(run(p))
