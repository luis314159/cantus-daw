"""Round 42 (chorda-c17): the timbre's signature — what sets one source apart from a reference
on every listening phrase, per band and per feature, at matched loudness (-20 LUFS, as the
kit plays them).

    python -I sig.py <ref label>=<dir> <label>=<dir> [...] [--phrases a,b] [--json out.json]

Each <dir> holds <phrase>.wav. The first source is the reference; a second recorded piano
(the bitKlavier Grand) beside it tells what is the reference's own from what every piano
shares: a band where a source stands outside the pianos' spread on every phrase is its
signature. Per phrase and source, in third-octave bands 40 Hz - 16 kHz, the level difference
against the reference in three windows locked to the phrase's onsets — the attack (0-30 ms),
the bloom (30-250 ms) and the rest — and the whole phrase; then features: the spectral
centroid, the flatness (noise against tone) in 1-4 kHz, the crest, the stereo correlation,
the side/mid ratio, the attack's rise (-20 to -3 dB of its peak, the median onset).
"""
import os, sys, json
import numpy as np
import soundfile as sf
from scipy import signal

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
from heard import lufs  # noqa: E402

SR = 48000
PHRASES = ['middle_c', 'soft_chords', 'fast_run', 'ballad', 'ff_chord', 'bass', 'top']
THIRDS = [40 * 2 ** (i / 3) for i in range(0, 27)]  # 40 Hz ... ~16 kHz


def onsets(ph):
    """The phrases' note onsets (s), as chorda_kit/phrases.rs writes them."""
    if ph == 'middle_c':
        return [0.0]
    if ph == 'soft_chords':
        return [i * 2.2 for i in range(4)]
    if ph == 'fast_run':
        return [0.125 * i for i in range(29)]
    if ph == 'ballad':
        bar = 60 / 72 * 4
        e = bar / 8
        t = []
        mel = [[0, 3, 4], [0, 2, 4], [0, 3, 4, 6], [0, 4]]
        for b in range(4):
            t += [b * bar + j * e * 1.15 for j in range(7)]
            t += [b * bar + s * e for s in mel[b]]
        return sorted(set(round(x, 4) for x in t))
    if ph == 'ff_chord':
        return [0.0]
    if ph == 'bass':
        return [3.0 * i for i in range(5)]
    if ph == 'top':
        return sorted([0.3 * i for i in range(4)] + [1.4 + 0.7 * i for i in range(4)]
                      + [4.4 + 0.09 * i for i in range(16)])
    raise KeyError(ph)


def load(d, ph):
    x, sr = sf.read(os.path.join(d, ph + '.wav'), dtype='float64', always_2d=True)
    assert sr == SR
    g = 10 ** ((-20 - lufs(x)) / 20)
    return x * g


def first_sound(m):
    """The reference's samples carry a pre-attack; find where each source's first onset is."""
    e = np.abs(m)
    pk = e[:int(0.5 * SR)].max()
    return int(np.argmax(e > pk * 10 ** (-30 / 20)))


def band_db(P, f):
    out = []
    for c in THIRDS:
        lo, hi = c / 2 ** (1 / 6), c * 2 ** (1 / 6)
        sel = (f >= lo) & (f < hi)
        out.append(10 * np.log10(P[sel].sum() + 1e-30))
    return np.array(out)


def spectra(x, ph):
    """Power spectra summed over the windows: attack / bloom / rest / all."""
    m = x.mean(1)
    n = 4096
    win = signal.windows.hann(n, sym=False)
    f = np.fft.rfftfreq(n, 1 / SR)
    off = first_sound(m)
    t0 = [int(t * SR) + off for t in onsets(ph)]
    hop = 512
    acc = {k: np.zeros(len(f)) for k in ('attack', 'bloom', 'rest', 'all')}
    for s in range(0, len(m) - n, hop):
        c = s + n // 2
        X = np.abs(np.fft.rfft(m[s:s + n] * win)) ** 2
        acc['all'] += X
        dt = min([(c - o) / SR for o in t0 if c >= o] or [9e9])
        k = 'attack' if dt < 0.03 else 'bloom' if dt < 0.25 else 'rest'
        acc[k] += X
    return f, acc, m, off, t0


def features(x, f, acc, m, off, t0):
    P = acc['all']
    sel = (f > 30) & (f < 16000)
    cen = (f[sel] * P[sel]).sum() / P[sel].sum()
    # Flatness between 1 and 4 kHz: tone (partials) against noise (between them).
    b = (f > 1000) & (f < 4000)
    flat = np.exp(np.mean(np.log(P[b] + 1e-30))) / P[b].mean()
    crest = 20 * np.log10(np.abs(x).max() / np.sqrt((x ** 2).mean()))
    L, R = x[:, 0], x[:, 1]
    corr = (L * R).sum() / np.sqrt((L * L).sum() * (R * R).sum())
    side = 10 * np.log10(((L - R) ** 2).sum() / ((L + R) ** 2).sum())
    # The rise of each onset (ms from -20 to -3 dB of the peak within 60 ms).
    env = np.sqrt(signal.lfilter([1], [1, -np.exp(-1 / (0.0005 * SR))], m * m) * (1 - np.exp(-1 / (0.0005 * SR))))
    rises = []
    for o in t0:
        seg = env[max(0, o - int(0.01 * SR)):o + int(0.06 * SR)]
        if len(seg) < 10:
            continue
        pk = seg.max()
        i3 = np.argmax(seg > pk * 10 ** (-3 / 20))
        i20 = np.argmax(seg > pk * 10 ** (-20 / 20))
        rises.append((i3 - i20) / SR * 1000)
    return dict(centroid=cen, flat_1_4k=10 * np.log10(flat), crest=crest, corr=corr,
                side_db=side, rise_ms=float(np.median(rises)) if rises else float('nan'))


def main():
    args = sys.argv[1:]
    phrases, js = PHRASES, None
    if '--phrases' in args:
        i = args.index('--phrases')
        phrases = args[i + 1].split(',')
        del args[i:i + 2]
    if '--json' in args:
        i = args.index('--json')
        js = args[i + 1]
        del args[i:i + 2]
    srcs = [a.split('=', 1) for a in args]
    res = {}
    for ph in phrases:
        res[ph] = {}
        for lab, d in srcs:
            x = load(d, ph)
            f, acc, m, off, t0 = spectra(x, ph)
            res[ph][lab] = dict(bands={k: band_db(v, f).tolist() for k, v in acc.items()},
                                feat=features(x, f, acc, m, off, t0))
    ref = srcs[0][0]
    names = [l for l, _ in srcs[1:]]
    hdr = ''.join(f'{c:>6.0f}' if c < 1000 else f'{c / 1000:>5.1f}k' for c in THIRDS)
    for win in ('all', 'attack', 'bloom'):
        print(f'\n== {win}: dB against {ref} (third octaves)')
        print(f'{"phrase/source":<24}' + hdr)
        for ph in phrases:
            r = np.array(res[ph][ref]['bands'][win])
            for n in names:
                d = np.array(res[ph][n]['bands'][win]) - r
                print(f'{ph[:11] + "/" + n[:12]:<24}' + ''.join(f'{v:6.1f}' for v in d))
    print(f'\n== features ({ref} first)')
    keys = ['centroid', 'flat_1_4k', 'crest', 'corr', 'side_db', 'rise_ms']
    print(f'{"phrase/source":<24}' + ''.join(f'{k:>11}' for k in keys))
    for ph in phrases:
        for lab, _ in srcs:
            ft = res[ph][lab]['feat']
            print(f'{ph[:11] + "/" + lab[:12]:<24}' + ''.join(f'{ft[k]:11.2f}' for k in keys))
    # The signature: per source, per band, the median difference over the phrases and how
    # many phrases share its sign.
    for win in ('all', 'attack'):
        print(f'\n== signature ({win}): median dB over {len(phrases)} phrases [phrases of that sign]')
        print(f'{"source":<24}' + hdr)
        for n in names:
            D = np.array([np.array(res[ph][n]['bands'][win]) - np.array(res[ph][ref]['bands'][win])
                          for ph in phrases])
            med = np.median(D, 0)
            same = [(np.sign(D[:, j]) == np.sign(med[j])).sum() for j in range(len(THIRDS))]
            print(f'{n[:24]:<24}' + ''.join(f'{v:6.1f}' for v in med))
            print(f'{"":<24}' + ''.join(f'{s:>6d}' for s in same))
    if js:
        json.dump(res, open(js, 'w'), indent=1)


if __name__ == '__main__':
    main()
