"""Chorda's character presets against recorded pianos of their kind (round 42, chorda-presets).

    python -I features.py <wav root> <source> [<source> …]  > table
    python -I features.py --json out.json <wav root> <source> …

<wav root>/<source>/<phrase>.wav are chorda_compare's renders (32-bit float, 48 kHz): the
listening phrases (soft_chords, middle_c, bass, fast_run, ballad) and the single notes
(notes_pp, notes_mf, notes_ff: A0 … C8 every minor third, one every 6 s, held 3.5 s). A source
may live in other roots: `name@<root>[+<root>…]`.

The five features that define a piano's character, each measured the same way on every source:

- tilt     the long-term spectrum's slope over 100 Hz – 8 kHz (dB per octave, third-octave
           bands, a least-squares line), the five phrases' mean;
- bright   the attack's brightness: the share of each note's first 100 ms over 2 kHz (C3 … C6,
           mezzo-forte, dB, the median), and `bright_vel` how far it climbs from pp to ff
           (dB, the median over the same keys);
- decay    how long a mezzo-forte note takes to fall 20 dB from its loudest (s, the 20 ms RMS;
           3.5 at most, the note's length), the median of the bass (A0 … B2), middle (C3 … B4)
           and treble (C5 … C7) keys;
- beat     the unison's beating: the fundamental's band envelope over 0.3 – 2.5 s with its line
           taken out, its RMS (dB), the median of C3 … C6 at mezzo-forte;
- bass     the bass weight: 30 – 250 Hz against 250 Hz – 4 kHz (dB), the bass and ballad phrases'
           mean.
"""
import json
import os
import sys

import numpy as np
import soundfile as sf
from scipy import signal

FS = 48_000
PHRASES = ['soft_chords', 'middle_c', 'bass', 'fast_run', 'ballad']
EVERY, HELD = 6.0, 3.5
KEYS = list(range(21, 109, 3))


def mono(path):
    x, fs = sf.read(path, always_2d=True)
    assert fs == FS, (path, fs)
    return x.mean(axis=1)


def ltas(x):
    f, p = signal.welch(x, FS, nperseg=8192)
    return f, p


def band_db(f, p, lo, hi):
    m = (f >= lo) & (f < hi)
    return 10 * np.log10(p[m].sum() + 1e-30)


def tilt(x):
    f, p = ltas(x)
    centres = 100 * 2 ** (np.arange(0, 6.33, 1 / 3))
    db = [band_db(f, p, c / 2 ** (1 / 6), c * 2 ** (1 / 6)) for c in centres]
    oct_ = np.log2(centres / 100)
    return float(np.polyfit(oct_, db, 1)[0])


def bass_weight(x):
    f, p = ltas(x)
    return band_db(f, p, 30, 250) - band_db(f, p, 250, 4000)


def notes(x):
    """Each single note's samples: key → the 6 s from its start."""
    out = {}
    for i, k in enumerate(KEYS):
        a = int(i * EVERY * FS)
        out[k] = x[a:a + int(EVERY * FS)]
    return out


def onset(n):
    e = np.abs(n)
    th = 1e-3 * e.max()
    idx = np.argmax(e > th)
    return int(idx)


def centroid(seg):
    w = np.hanning(len(seg))
    s = np.abs(np.fft.rfft(seg * w)) ** 2
    f = np.fft.rfftfreq(len(seg), 1 / FS)
    m = (f > 30) & (f < 16_000)
    return float((f[m] * s[m]).sum() / (s[m].sum() + 1e-30))


def attack_hf(n):
    """The attack's brightness: the share of its first 100 ms over 2 kHz (dB)."""
    a = onset(n)
    seg = n[a:a + int(0.1 * FS)]
    s = np.abs(np.fft.rfft(seg * np.hanning(len(seg)))) ** 2
    f = np.fft.rfftfreq(len(seg), 1 / FS)
    return float(10 * np.log10(s[f > 2_000].sum() / (s[f > 30].sum() + 1e-30) + 1e-30))


def rms_db(n, hop=0.02):
    h = int(hop * FS)
    k = len(n) // h
    r = np.sqrt((n[:k * h].reshape(k, h) ** 2).mean(axis=1) + 1e-30)
    return 20 * np.log10(r), h


def fall(n):
    """How long the note takes to fall 20 dB from its loudest (s, the 20 ms RMS; the
    note is held 3.5 s, so 3.5 at most)."""
    a = onset(n)
    db, h = rms_db(n[a:a + int(HELD * FS)])
    top = int(np.argmax(db))
    under = np.nonzero(db[top:] < db[top] - 20)[0]
    return float((top + under[0]) * h / FS) if len(under) else HELD


def beat(n, key):
    a = onset(n)
    f0 = 440 * 2 ** ((key - 69) / 12)
    sos = signal.butter(4, [f0 * 0.94, f0 * 1.06], btype='band', fs=FS, output='sos')
    y = signal.sosfiltfilt(sos, n[a:a + int(3.0 * FS)])
    env = np.abs(signal.hilbert(y))
    h = int(0.01 * FS)
    k = len(env) // h
    e = 20 * np.log10(env[:k * h].reshape(k, h).mean(axis=1) + 1e-12)
    t = np.arange(k) * h / FS
    m = (t > 0.3) & (t < 2.5)
    line = np.polyval(np.polyfit(t[m], e[m], 1), t[m])
    return float(np.sqrt(((e[m] - line) ** 2).mean()))


def measure(root, src):
    roots = [root]
    if '@' in src:
        src, rest = src.split('@', 1)
        roots = rest.split('+')

    def find(name):
        for r in roots:
            f = os.path.join(r, src, name + '.wav')
            if os.path.exists(f):
                return f
        return None

    ph = {p: mono(find(p)) for p in PHRASES if find(p)}
    out = {'source': src}
    if ph:
        out['tilt'] = float(np.mean([tilt(x) for x in ph.values()]))
        out['bass'] = float(np.mean([bass_weight(ph[p]) for p in ('bass', 'ballad') if p in ph]))
    nn = {}
    for dyn in ('pp', 'mf', 'ff'):
        p = find(f'notes_{dyn}')
        if p:
            nn[dyn] = notes(mono(p))
    mid = [k for k in KEYS if 48 <= k <= 84]
    if 'mf' in nn:
        out['bright'] = float(np.median([attack_hf(nn['mf'][k]) for k in mid]))
        out['beat'] = float(np.median([beat(nn['mf'][k], k) for k in mid]))
        for reg, lo, hi in (('decay_bass', 21, 47), ('decay_mid', 48, 71), ('decay_treble', 72, 96)):
            out[reg] = float(np.median([fall(nn['mf'][k]) for k in KEYS if lo <= k <= hi]))
    if 'pp' in nn and 'ff' in nn:
        out['bright_vel'] = float(np.median([
            attack_hf(nn['ff'][k]) - attack_hf(nn['pp'][k]) for k in mid]))
    return out


COLS = ['tilt', 'bright', 'bright_vel', 'decay_bass', 'decay_mid', 'decay_treble', 'beat', 'bass']


def main(argv):
    js = None
    if argv and argv[0] == '--json':
        js, argv = argv[1], argv[2:]
    root, srcs = argv[0], argv[1:]
    rows = [measure(root, s) for s in srcs]
    print('| source | ' + ' | '.join(COLS) + ' |')
    print('|---' * (len(COLS) + 1) + '|')
    for r in rows:
        print(f"| {r['source']} | " + ' | '.join(
            ('%.2f' % r[c]) if c in r else '—' for c in COLS) + ' |')
    if js:
        with open(js, 'w') as f:
            json.dump(rows, f, indent=1)


if __name__ == '__main__':
    main(sys.argv[1:])
