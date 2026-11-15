"""The hybrid measured beside the generated kit and the recording (round 40, drums-hyb).

    python -I hybrid_measure.py <gen hits.wav> <hyb hits.wav> <rec hits.wav> [<listen dir>]

Hits files are `tympanum_measure hits`' layout (13 articulations x 4 velocities, 4 s each,
mono). Per hit: the distance from the recording (mean |dB| over 1/3-octave bands 60 Hz-16
kHz) in 0-50 ms and 50-200 ms; the analysis's attack measures (first-3-ms centroid, 6-20
kHz against 0.2-1.5 kHz in the first 3 ms) and, for cymbals, 4-8 kHz flatness, the top
three partials' share and the fine-structure stability (metrics.py's). With a listen
folder: the grooves' 6-16 kHz flatness and components per kHz (analysis-drums s06's).
"""
import os
import sys

import numpy as np
import soundfile as sf
from scipy import signal

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import metrics as M  # noqa: E402

SR = 48000
FC = [c for c in 1000 * 2 ** (np.arange(-14, 13) / 3) if 60 <= c <= 16000]


def third_db(x):
    n = max(len(x), 4096)
    P = np.abs(np.fft.rfft(x * np.hanning(len(x)), n)) ** 2
    f = np.fft.rfftfreq(n, 1 / SR)
    out = []
    for c in FC:
        s = (f >= c / 2 ** (1 / 6)) & (f < c * 2 ** (1 / 6))
        out.append(10 * np.log10(P[s].sum() + 1e-30))
    return np.array(out)


def dist(a, b):
    """Mean |dB| between two band spectra, levels aligned (the shape, not the loudness)."""
    d = a - b
    d -= np.median(d)
    return float(np.mean(np.abs(d)))


def load(p):
    x, sr = sf.read(p, always_2d=True)
    assert sr == SR
    return x.mean(1)


def hit(x, i):
    seg = x[i * 4 * SR:(i + 1) * 4 * SR]
    a = np.abs(seg)
    on = int(np.argmax(a > 0.05 * a.max())) if a.max() > 0 else 0
    return seg, on


def main():
    gen, hyb, rec = (load(p) for p in sys.argv[1:4])
    lines = ["# distance from the recording: mean |dB| over 1/3 octaves 60 Hz-16 kHz (levels aligned)",
             "# c3ms: first-3-ms centroid kHz; hi/lo3: 6-20 kHz re 0.2-1.5 kHz in the first 3 ms, dB",
             "# piece       vel |  0-50 ms gen  hyb |  50-200 ms gen  hyb |   c3ms gen  hyb  rec |  hi/lo3 gen   hyb   rec"]
    tot = {"g50": [], "h50": [], "g200": [], "h200": []}
    for a, piece in enumerate(M.PIECES):
        for v, vel in enumerate(M.VELS):
            i = a * 4 + v
            (g, og), (h, oh), (r, orr) = hit(gen, i), hit(hyb, i), hit(rec, i)
            win = lambda x, o, t0, t1: x[o + int(t0 * SR):o + int(t1 * SR)]
            R50, R200 = third_db(win(r, orr, 0, 0.05)), third_db(win(r, orr, 0.05, 0.2))
            g50 = dist(third_db(win(g, og, 0, 0.05)), R50)
            h50 = dist(third_db(win(h, oh, 0, 0.05)), R50)
            g200 = dist(third_db(win(g, og, 0.05, 0.2)), R200)
            h200 = dist(third_db(win(h, oh, 0.05, 0.2)), R200)
            for k, val in (("g50", g50), ("h50", h50), ("g200", g200), ("h200", h200)):
                tot[k].append(val)

            def att(x, o):
                c = M.centroid(x[o:o + 144]) / 1000
                hi = M.band_env(x, 6000, 20000, win=0.001)[o:o + 144].max()
                lo = M.band_env(x, 200, 1500, win=0.001)[o:o + 144].max()
                return c, 20 * np.log10(hi / lo)
            (cg, hg), (ch, hh), (cr, hr) = att(g, og), att(h, oh), att(r, orr)
            lines.append("%-11s %4d | %10.1f %4.1f | %13.1f %4.1f | %8.2f %4.2f %4.2f | %8.1f %5.1f %5.1f" % (
                piece, vel, g50, h50, g200, h200, cg, ch, cr, hg, hh, hr))
    lines.append("mean           | %10.2f %4.2f | %13.2f %4.2f |" % (
        np.mean(tot["g50"]), np.mean(tot["h50"]), np.mean(tot["g200"]), np.mean(tot["h200"])))
    # Cymbals: the analysis's texture measures.
    lines.append("")
    lines.append("# cymbals: flat4-8, top3 (%), stab — gen / hyb / rec")
    for piece in M.CYM:
        for v, vel in enumerate(M.VELS):
            i = M.PIECES.index(piece) * 4 + v
            row = []
            for x in (gen, hyb, rec):
                seg, on = hit(x, i)
                m = seg[max(0, on - int(0.02 * SR)):]
                o = on - max(0, on - int(0.02 * SR))
                fl, _, _ = M.tonal(m, o, piece, 4000, 8000)
                _, t3 = M.partials(m, o, piece)
                st = M.stability(m, o, piece)
                row.append((fl, t3, st))
            lines.append("%-11s %4d | flat %s | top3 %s | stab %s" % (
                piece, vel,
                " ".join("%.3f" % r[0] for r in row),
                " ".join("%5.1f" % r[1] for r in row),
                " ".join("%.2f" % r[2] for r in row)))
    if len(sys.argv) > 4:
        d = sys.argv[4]
        lines.append("")
        lines.append("# grooves: 6-16 kHz flatness (median per 2048 frame), components per kHz (8192)")
        for name in ("groove-1-generado.flac", "groove-2-hibrido.flac", "groove-3-grabacion.flac"):
            x, _ = sf.read(os.path.join(d, name), always_2d=True)
            mid = x.mean(1)
            f, t, S = signal.stft(mid, SR, nperseg=2048, noverlap=1024)
            P = np.abs(S) ** 2
            s = (f >= 6000) & (f < 16000)
            Ps = np.maximum(P[s], 1e-30)
            fl = np.exp(np.mean(np.log(Ps), 0)) / np.mean(Ps, 0)
            e = P[s].sum(0)
            ok = e > np.percentile(e, 30)
            f2, t2, S2 = signal.stft(mid, SR, nperseg=8192, noverlap=4096)
            P2 = np.abs(S2) ** 2
            s2 = (f2 >= 6000) & (f2 < 16000)
            pr = P2[s2].sum(0) ** 2 / np.maximum((P2[s2] ** 2).sum(0), 1e-60) / 10
            e2 = P2[s2].sum(0)
            ok2 = e2 > np.percentile(e2, 30)
            lines.append("%-26s flatness %.3f  components/kHz %.1f" % (
                name, np.median(fl[ok]), np.median(pr[ok2])))
    print("\n".join(lines))


if __name__ == "__main__":
    main()
